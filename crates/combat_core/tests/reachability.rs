//! Every Kill, Loot and task goal must be finishable from what one fresh load of a zone
//! guarantees. Trash never respawns inside a zone, so a count above the guaranteed supply is a
//! quest nobody can finish.

mod common;

use combat_core::{Content, GoalSpec, Objective, QuestDef, Sim, Vec3};
use common::*;

/// Zones one quest may draw on. Quest text names "both halls" or "campus, tower, anywhere", so
/// the data centre halls, the lab rooms and the HQ circuit (campus plus tower floors) each count
/// as one area; the giver's area is the one that has to deliver.
fn area(zone: &str) -> &str {
    match zone.split('/').next().unwrap_or(zone) {
        "campus" | "tower" => "hq",
        other => other,
    }
}

fn in_area<'a>(c: &'a Content, zone: &str) -> impl Iterator<Item = &'a str> {
    let a = area(zone).to_string();
    c.zones
        .iter()
        .filter(move |z| area(&z.id) == a)
        .map(|z| z.id.as_str())
}

/// Copies of `item` that one pass of the area is certain to hand `quest`'s owner: units with a
/// 100% rule that is either open to everyone or tied to this quest.
fn loot_supply(c: &Content, zone: &str, item: &str, quest: &str) -> u32 {
    in_area(c, zone)
        .flat_map(|z| c.zone_hostiles(z))
        .filter_map(|k| c.unit_def(k))
        .filter(|u| {
            u.loot.iter().any(|l| {
                l.item == item
                    && l.chance_pct >= 100
                    && l.quest.as_deref().is_none_or(|q| q == quest)
            })
        })
        .count() as u32
}

/// Hostiles of one kind in a single zone, rares included (the layouts carry their anchors).
fn zone_kills(c: &Content, zone: &str, unit: &str) -> u32 {
    let rares = c
        .zone(zone)
        .map_or(0, |z| z.rares.iter().filter(|r| r.unit == unit).count());
    // A linked twin (or panel member) spawns beside its raid-zone boss without being a pack member.
    let twin = c
        .raids
        .iter()
        .filter(|r| r.zones.iter().any(|z| z == zone))
        .filter_map(|r| r.twins.as_ref())
        .filter(|t| {
            (t.twin == unit || t.extra.iter().any(|e| e == unit))
                && c.zone(zone)
                    .is_some_and(|z| z.boss.as_ref().is_some_and(|b| b.unit == t.boss))
        })
        .count();
    // A unit the zone's boss summons in its phases (the Golem's Bullet Points) comes as often as
    // the fight lasts; the content check is that it is summoned at all.
    let summoned = c
        .zone(zone)
        .and_then(|z| z.boss.as_ref())
        .and_then(|b| c.unit_def(&b.unit))
        .and_then(|d| d.ai.as_ref())
        .map_or(0, |ai| {
            let spells = ai
                .abilities
                .iter()
                .map(|a| a.spell)
                .chain(ai.phases.iter().filter_map(|p| p.spell));
            spells
                .filter_map(|id| c.spell(id))
                .filter(|s| matches!(&s.effect, combat_core::Effect::Summon { unit: u, .. } if u == unit))
                .count() as u32
                * 8
        });
    (c.zone_hostiles(zone).iter().filter(|k| **k == unit).count() + rares + twin) as u32 + summoned
}

fn kill_supply(c: &Content, zone: &str, unit: &str) -> u32 {
    in_area(c, zone).map(|z| zone_kills(c, z, unit)).sum()
}

/// Best area for an objective whose sources are not in the giver's area (a quest that sends you
/// elsewhere on purpose).
fn best(c: &Content, f: impl Fn(&str) -> u32) -> u32 {
    c.zones.iter().map(|z| f(&z.id)).max().unwrap_or(0)
}

/// Whether any unit of the giver's area could ever source the objective; if none can, the
/// quest sends the player to another area on purpose and that area has to deliver.
fn sourced_here(c: &Content, zone: &str, o: &Objective) -> bool {
    in_area(c, zone).any(|z| match o {
        Objective::Kill(k, _) | Objective::KillClean(k, _) => zone_kills(c, z, k) > 0,
        Objective::Loot(it, _) => c
            .zone_hostiles(z)
            .into_iter()
            .filter_map(|k| c.unit_def(k))
            .any(|u| u.loot.iter().any(|l| l.item == *it)),
        _ => false,
    })
}

fn shortfalls(c: &Content, q: &QuestDef) -> Vec<String> {
    let mut out = Vec::new();
    let giver = q.giver.zone.as_str();
    for o in &q.objectives {
        let here = sourced_here(c, giver, o);
        let (need, have, what) = match o {
            Objective::Kill(k, n) | Objective::KillClean(k, n) => {
                let have = match here {
                    true => kill_supply(c, giver, k),
                    false => best(c, |z| kill_supply(c, z, k)),
                };
                (*n, have, format!("kill {k}"))
            }
            Objective::Loot(it, n) => {
                let have = match here {
                    true => loot_supply(c, giver, it, &q.id),
                    false => best(c, |z| loot_supply(c, z, it, &q.id)),
                };
                (*n, have, format!("loot {it}"))
            }
            _ => continue,
        };
        if have < need {
            out.push(format!(
                "{}: {what} needs {need}, the area guarantees {have}",
                q.id
            ));
        }
    }
    out
}

#[test]
fn every_quest_objective_is_reachable_from_guaranteed_sources() {
    let c = content();
    let bad: Vec<String> = c.quests.iter().flat_map(|q| shortfalls(&c, q)).collect();
    assert!(
        bad.is_empty(),
        "unreachable objectives:\n{}",
        bad.join("\n")
    );
}

#[test]
fn quest_only_drops_are_guaranteed() {
    let c = content();
    let bad: Vec<String> = c
        .units
        .iter()
        .flat_map(|u| {
            u.loot
                .iter()
                .filter(|l| l.quest.is_some() && l.chance_pct < 100)
                .map(|l| format!("{} drops {} at {}%", u.key, l.item, l.chance_pct))
        })
        .collect();
    assert!(
        bad.is_empty(),
        "quest drops that can miss on units that never respawn:\n{}",
        bad.join("\n")
    );
}

#[test]
fn every_task_goal_fits_the_work_zone() {
    let mut c = content();
    // Every zone open, so each level's work zone is the one the player would really get.
    for l in c.zones.iter_mut().flat_map(|z| z.links.iter_mut()) {
        l.requires = None;
    }
    let top = c.max_level();
    let mut bad = Vec::new();
    // A new UTC day rotates the dailies, so sweep enough of them to meet every template.
    for day in 0..60u64 {
        for level in 1..=top {
            let mut sim = Sim::new(c.clone(), 1);
            sim.set_clock(day * 86_400 + 3_600);
            let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
            sim.set_player_level(level);
            let zone = sim.work_zone(level);
            let board = sim.task_board(p);
            for t in &board.tasks {
                for g in &t.goals {
                    let (have, what) = match &g.goal {
                        GoalSpec::Kill { zone: z, unit } => {
                            let z = z.as_deref().unwrap_or(&zone);
                            let n = match unit {
                                Some(u) => zone_kills(&c, z, u),
                                None => c.zone_hostiles(z).len() as u32,
                            };
                            (n, format!("kill {unit:?} in {z}"))
                        }
                        // Loot rolls are random, so one corpse per hostile is the best bound.
                        GoalSpec::Loot => (c.zone_hostiles(&zone).len() as u32, "loot".into()),
                        _ => continue,
                    };
                    if have < g.need {
                        bad.push(format!(
                            "{} L{level}: {what} needs {}, zone has {have}",
                            t.key, g.need
                        ));
                    }
                }
            }
        }
    }
    bad.sort();
    bad.dedup();
    assert!(
        bad.is_empty(),
        "task goals past the zone:\n{}",
        bad.join("\n")
    );
}

/// Interactive objectives need things placed in the shipped layouts: enough anchored props
/// per tag, every device of a sequence, units an item can go on, a recipient and an escort
/// with its goal anchor in the same zone.
#[test]
fn every_interactive_objective_has_its_props_units_and_anchors() {
    let c = content();
    let geo = geometry();
    let anchored =
        |zone: &str, name: &str| geo.get(zone).is_some_and(|g| g.anchors.contains_key(name));
    let placed = |f: &dyn Fn(&combat_core::PropDef) -> bool| {
        c.world
            .props
            .iter()
            .filter(|p| f(p) && anchored(&p.zone, &format!("prop:{}", p.key)))
            .count() as u32
    };
    // A friendly unit spawned at an anchor the zone's layout has.
    let stands = |unit: &str| {
        c.zones.iter().any(|z| {
            z.friendly
                .iter()
                .any(|f| f.unit == unit && anchored(&z.id, &f.anchor))
        })
    };
    let mut bad = Vec::new();
    for q in &c.quests {
        for o in &q.objectives {
            let ok = match o {
                Objective::Collect(tag, n) | Objective::Inspect(tag, n) => {
                    placed(&|p| p.tag == *tag) >= *n
                }
                Objective::Interact(k, _) => placed(&|p| p.key == *k) == 1,
                Objective::Sequence { props, .. } => {
                    props.iter().all(|k| placed(&|p| p.key == *k) == 1)
                }
                Objective::UseOn { target, n, .. } => match c.unit_def(target) {
                    Some(u) if u.hostile => best(&c, |z| kill_supply(&c, z, target)) >= *n,
                    Some(_) => stands(target),
                    None => placed(&|p| p.key == *target || p.tag == *target) >= *n,
                },
                Objective::Deliver { npc, .. } => stands(npc),
                Objective::Escort { unit, to, .. } => c.zones.iter().any(|z| {
                    z.friendly
                        .iter()
                        .any(|f| f.unit == *unit && anchored(&z.id, &f.anchor))
                        && anchored(&z.id, to)
                }),
                _ => continue,
            };
            if !ok {
                bad.push(format!("{}: {o:?}", q.id));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "unplaceable objectives:\n{}",
        bad.join("\n")
    );
}

/// The hybrid-work side quests rely on the interactive checks above (a placed enforcer and
/// three anchored desks); this pins that they are the kind those checks cover.
#[test]
fn the_hybrid_work_quests_are_interactive_and_covered() {
    let c = content();
    for id in ["coffee_badging", "desk_census"] {
        let q = c.quests.iter().find(|q| q.id == id);
        assert!(
            q.is_some_and(|q| q
                .objectives
                .iter()
                .all(|o| matches!(o, Objective::Deliver { .. } | Objective::Inspect(..)))),
            "{id}"
        );
    }
}
