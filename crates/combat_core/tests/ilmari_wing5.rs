//! Ilmari Wing 5, The Alignment Lab (docs/ILMARI_1_0.md D6, D7): the Consultant's offers, the
//! kiosks and the Tailgating loophole, Red Team Blue Team's tabletop standby and release, and the
//! wing's door, drops and locks. The balance gates (Story solo, Full trio, refuse/accept split)
//! live in tests/playthrough.rs.
#![allow(clippy::expect_used)]

mod common;

use combat_core::{Command, EntityId, Event, GearSource, PROP_OK, Sim, SpellId, Unlock, Vec3};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;
const BOSS_Z: f32 = -30.0;

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, BOSS_Z),
        n if n.starts_with("prop:") => at(1.0, 0.0),
        _ => at(0.0, 0.0),
    }
}

fn setup(
    zone: &str,
    boss_key: &str,
    classes: &[&'static str],
    story: bool,
    seed: u64,
) -> (Sim, Vec<EntityId>, EntityId) {
    let mut c = content();
    for k in [
        "senior_engineer",
        "backend_dev",
        "devops_eng",
        "frontend_dev",
        "security_eng",
    ] {
        if c.unit_def(k).is_some() {
            def_mut(&mut c, k).max_hp = 10_000_000;
        }
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
    let mut sim = Sim::new(c, seed);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids: Vec<EntityId> = classes
        .iter()
        .enumerate()
        .map(|(k, class)| {
            let p = sim.spawn_player(class, Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, 10);
            p
        })
        .collect();
    sim.debug_force_raid_async(Some(story));
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{zone}: {e}");
    }
    let boss = sim
        .units()
        .find(|u| u.key == boss_key)
        .map(|u| u.id)
        .expect("boss");
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(2.0, 0.0, BOSS_Z + 8.0));
    }
    sim.apply_as(
        ids[0],
        Command::SetTarget {
            unit: ids[0],
            target: Some(boss),
        },
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: SpellId(1),
        },
    );
    (sim, ids, boss)
}

fn move_to(sim: &mut Sim, p: EntityId, pos: Vec3) {
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos,
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
}

fn run(sim: &mut Sim, secs: u32) -> Vec<Event> {
    (0..secs * SECOND).flat_map(|_| sim.tick()).collect()
}

fn pending(sim: &Sim, p: EntityId, prefix: &str) -> usize {
    sim.unit(p).map_or(0, |u| {
        u.auras.iter().filter(|a| a.key.starts_with(prefix)).count()
    })
}

fn kiosk(sim: &mut Sim, p: EntityId, n: u32) -> Vec<u32> {
    let prop = format!("il_optout_{n}");
    // The synthetic anchors put every prop at (1, 0); a player must stand in reach of it.
    move_to(sim, p, Vec3::new(1.0, 0.0, 0.0));
    sim.apply_as(p, Command::Interact { prop: prop.clone() });
    sim.tick()
        .into_iter()
        .filter_map(|e| match e {
            Event::Interacted {
                unit,
                prop: k,
                outcome,
            } if unit == p && k == prop => Some(outcome),
            _ => None,
        })
        .collect()
}

/// Ticks until `who` has a pending offer (any of `prefix`), at most `secs`.
fn wait_offer(sim: &mut Sim, who: EntityId, prefix: &str, secs: u32) -> bool {
    for _ in 0..secs * SECOND {
        sim.tick();
        if pending(sim, who, prefix) > 0 {
            return true;
        }
    }
    false
}

#[test]
fn the_consultant_offers_and_a_kiosk_cancels_it_on_full() {
    let (mut sim, ids, _) = setup(
        "ilmari5/2",
        "il_align_consultant",
        &["backend", "devops", "fullstack"],
        false,
        5,
    );
    let who = ids.iter().copied().find(|_| true).expect("a player");
    let mut got = None;
    for _ in 0..40 * SECOND {
        sim.tick();
        got = ids
            .iter()
            .copied()
            .find(|&p| pending(&sim, p, "il_ok_") > 0);
        if got.is_some() {
            break;
        }
    }
    let p = got.unwrap_or(who);
    assert!(
        pending(&sim, p, "il_ok_") > 0,
        "an offer shows up as an aura"
    );
    let outcome = kiosk(&mut sim, p, 1);
    assert_eq!(outcome, vec![PROP_OK], "the kiosk takes it back");
    assert_eq!(pending(&sim, p, "il_ok_"), 0);
}

#[test]
fn story_offers_name_their_terms_and_never_stack_aligned() {
    let (mut sim, ids, _) = setup("ilmari5/2", "il_align_consultant", &["fullstack"], true, 6);
    assert!(
        wait_offer(&mut sim, ids[0], "il_oks_", 40),
        "a Story offer arrives"
    );
    let a = sim
        .unit(ids[0])
        .and_then(|u| {
            u.auras
                .iter()
                .find(|a| a.key.starts_with("il_oks_"))
                .cloned()
        })
        .expect("aura");
    assert!(a.key.starts_with("il_oks_"));
    run(&mut sim, 20);
    let aligned = sim.unit(ids[0]).map_or(0, |u| {
        u.auras
            .iter()
            .filter(|a| a.key == "il_aligned_stack")
            .map(|a| a.stacks as usize)
            .sum()
    });
    assert_eq!(aligned, 0, "Story never aligns anyone");
}

#[test]
fn tailgating_drops_every_pending_offer_without_a_stagger() {
    let (mut sim, ids, _) = setup("ilmari5/2", "il_align_consultant", &["security"], false, 7);
    assert!(
        wait_offer(&mut sim, ids[0], "il_ok_", 40),
        "an offer arrives"
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: SpellId(75),
        },
    );
    let events = run(&mut sim, 1);
    assert_eq!(pending(&sim, ids[0], "il_ok_"), 0, "the loophole clears it");
    assert!(
        !events.iter().any(|e| matches!(e, Event::AuraApplied { target, key } if *target == ids[0] && key == "il_stagger")),
        "a loophole is not a refusal"
    );
}

#[test]
fn the_tabletop_holds_red_back_until_it_is_hurt() {
    let (mut sim, _ids, boss) = setup(
        "ilmari5/3",
        "il_red_team",
        &["backend", "fullstack", "frontend"],
        false,
        8,
    );
    // Code Review lands after about 2 s; that pull arms the encounter. The first tick with the
    // squads placed is the moment Red stands by (a cone sighting soon after would end it).
    let mut armed = false;
    for _ in 0..4 * SECOND {
        armed = sim
            .tick()
            .iter()
            .any(|e| matches!(e, Event::Exercise { standby: true, sensors, .. } if *sensors > 0));
        if armed {
            break;
        }
    }
    assert!(armed, "the exercise is announced");
    let standing = |sim: &Sim| {
        sim.units()
            .filter(|u| u.key.starts_with("il_red_") && u.key != "il_red_team" && u.alive)
            .filter(|u| u.auras.iter().any(|a| a.key == "il_standby"))
            .count()
    };
    assert!(standing(&sim) >= 2, "Red stands by at the start");
    assert!(
        sim.unit(boss)
            .is_some_and(|u| u.auras.iter().any(|a| a.key == "il_tabletop")),
        "the boss wears the tabletop"
    );
    let red = sim
        .units()
        .find(|u| u.key.starts_with("il_red_") && u.key != "il_red_team")
        .map(|u| u.id)
        .expect("a red unit");
    let p = sim.units().find(|u| u.key == "backend_dev").map(|u| u.id);
    let _ = p;
    sim.debug_hit(boss, red, 10);
    run(&mut sim, 1);
    assert_eq!(standing(&sim), 0, "hurting Red starts the exercise");
    assert!(
        sim.unit(boss)
            .is_some_and(|u| !u.auras.iter().any(|a| a.key == "il_tabletop")),
        "and the boss loses the tabletop"
    );
}

const WASH: SpellId = SpellId(2636);

fn wash_heals(events: &[Event], boss: EntityId) -> u32 {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Heal {
                target,
                spell: Some(s),
                amount,
                ..
            } if *target == boss && *s == WASH => Some(*amount),
            _ => None,
        })
        .sum()
}

/// Runs until the Consultant wears the wash (at most `secs`).
fn wait_wash(sim: &mut Sim, boss: EntityId, secs: u32) -> bool {
    for _ in 0..secs * SECOND {
        sim.tick();
        if sim
            .unit(boss)
            .is_some_and(|u| u.auras.iter().any(|a| a.key == "il_ethics_wash"))
        {
            return true;
        }
    }
    false
}

#[test]
fn ethics_washing_left_standing_heals_him_and_a_broken_one_does_not() {
    for broken in [false, true] {
        let (mut sim, ids, boss) =
            setup("ilmari5/2", "il_align_consultant", &["devops"], false, 11);
        sim.apply_as(
            ids[0],
            Command::SetTarget {
                unit: ids[0],
                target: None,
            },
        );
        let max = sim.unit(boss).expect("boss").max_hp;
        sim.debug_set_hp(boss, max / 10 * 6);
        assert!(wait_wash(&mut sim, boss, 30), "he washes himself");
        if broken {
            sim.debug_hit(ids[0], boss, 700);
        }
        let healed = wash_heals(&run(&mut sim, 14), boss);
        if broken {
            assert_eq!(healed, 0, "the room broke it, nothing to feed on");
        } else {
            assert_eq!(
                healed,
                max * 6 / 100,
                "6 % of his bar when nobody touched it"
            );
        }
    }
}

#[test]
fn pivot_to_safety_moves_the_kiosks_and_only_the_standing_site_works() {
    let (mut sim, ids, boss) = setup("ilmari5/2", "il_align_consultant", &["devops"], false, 12);
    let p = ids[0];
    sim.apply_as(
        p,
        Command::SetTarget {
            unit: p,
            target: None,
        },
    );
    let max = sim.unit(boss).expect("boss").max_hp;
    sim.debug_set_hp(boss, max * 45 / 100);
    let mut moved = false;
    'wait: for _ in 0..40 * SECOND {
        for e in sim.tick() {
            if matches!(e, Event::Kiosks { alt: true }) {
                moved = true;
            }
        }
        if moved && pending(&sim, p, "il_ok_") > 0 {
            break 'wait;
        }
    }
    assert!(moved, "the first pair of phase 2 moves the kiosks");
    assert!(pending(&sim, p, "il_ok_") > 0, "an offer is waiting");
    assert_eq!(
        kiosk(&mut sim, p, 1),
        vec![0],
        "the old site is switched off"
    );
    assert!(pending(&sim, p, "il_ok_") > 0, "and it took nothing");
    assert_eq!(kiosk(&mut sim, p, 5), vec![PROP_OK], "the new one works");
}

#[test]
fn the_squads_wear_player_names_and_the_copies_cut_damage_while_they_stand() {
    let (mut sim, ids, boss) = setup(
        "ilmari5/3",
        "il_red_team",
        &["backend", "fullstack", "frontend"],
        false,
        9,
    );
    let mut mirrored = Vec::new();
    for _ in 0..4 * SECOND {
        for e in sim.tick() {
            if let Event::Mirrored { mirror, player } = e {
                mirrored.push((mirror, player));
            }
        }
    }
    assert!(!mirrored.is_empty(), "role units name their player");
    assert!(
        mirrored.iter().all(|(_, p)| ids.contains(p)),
        "only humans are copied"
    );
    let cut = |sim: &Sim, p: EntityId| pending(sim, p, "il_squad_mirrored");
    assert!(
        ids.iter().all(|&p| cut(&sim, p) == 0),
        "not before the merge"
    );
    let max = sim.unit(boss).expect("boss").max_hp;
    sim.debug_set_hp(boss, max / 2);
    run(&mut sim, 3);
    let copied: Vec<EntityId> = mirrored.iter().map(|(_, p)| *p).collect();
    assert!(
        copied.iter().all(|&p| cut(&sim, p) == 1),
        "the merge starts the -20 %"
    );
    for u in sim
        .squad_units(combat_core::sight::SquadSide::Red)
        .into_iter()
        .chain(sim.squad_units(combat_core::sight::SquadSide::Blue))
    {
        sim.debug_hit(boss, u, 1_000_000);
    }
    run(&mut sim, 2);
    assert!(
        ids.iter().all(|&p| cut(&sim, p) == 0),
        "no copy left, no cut"
    );
}

fn fight_log(zone: &str, boss_key: &str, story: bool, seed: u64) -> String {
    let classes: &[&'static str] = if story {
        &["fullstack"]
    } else {
        &["backend", "fullstack", "frontend"]
    };
    let (mut sim, _ids, _) = setup(zone, boss_key, classes, story, seed);
    let mut log = String::new();
    for k in 0..60 * SECOND {
        for e in sim.tick() {
            log.push_str(&log_line(&sim, u64::from(k), &e));
            log.push('\n');
        }
    }
    log
}

#[test]
fn both_fights_replay_bit_identically() {
    for (zone, boss) in [
        ("ilmari5/2", "il_align_consultant"),
        ("ilmari5/3", "il_red_team"),
    ] {
        for story in [false, true] {
            let a = fight_log(zone, boss, story, 3);
            let b = fight_log(zone, boss, story, 3);
            assert!(!a.is_empty());
            assert_eq!(a, b, "{boss} story {story}");
        }
    }
}

#[test]
fn the_wing_door_and_drops_follow_the_quests_and_bosses() {
    let c = content();
    let door = c
        .zone("campus_occupied")
        .and_then(|z| z.links.iter().find(|l| l.to == "ilmari5/1"))
        .expect("door");
    assert!(matches!(&door.requires, Some(Unlock::QuestDone(q)) if q == "il_12_wing5_attune"));
    let w5 = c.raid_of("ilmari5/1").expect("wing");
    assert!(w5.ilmari && w5.zones.len() == 3);
    let z2 = c.zone("ilmari5/2").expect("zone");
    assert!(z2.links.iter().any(|l| l.to == "ilmari5/3"
        && matches!(&l.requires, Some(Unlock::BossKilled(b)) if b == "il_align_consultant")));
    for boss in ["il_align_consultant", "il_red_team"] {
        let own: Vec<_> = c
            .economy
            .gear
            .iter()
            .filter(|g| g.drops_from.as_deref() == Some(boss))
            .collect();
        assert_eq!(own.len(), 7, "{boss}: 3 loose pieces and 4 tier pieces");
        assert!(own.iter().all(|g| g.source == GearSource::Ilmari));
        assert_eq!(own.iter().filter(|g| g.ilvl == 18).count(), 3, "{boss}");
        assert_eq!(own.iter().filter(|g| g.ilvl == 20).count(), 4, "{boss}");
    }
}

#[test]
fn killing_a_wing_five_boss_locks_its_own_difficulty() {
    for (story, zone, boss) in [
        (true, "ilmari5/2", "il_align_consultant"),
        (false, "ilmari5/3", "il_red_team"),
    ] {
        let classes: &[&'static str] = if story {
            &["fullstack"]
        } else {
            &["backend", "devops", "fullstack"]
        };
        let (mut sim, _ids, id) = setup(zone, boss, classes, story, 13);
        run(&mut sim, 1);
        assert!(sim.debug_kill(id));
        run(&mut sim, 1);
        assert!(
            sim.raid_locked(boss, story),
            "{boss} locked on its difficulty"
        );
        assert!(
            !sim.raid_locked(boss, !story),
            "{boss}: the other difficulty is free"
        );
    }
}
