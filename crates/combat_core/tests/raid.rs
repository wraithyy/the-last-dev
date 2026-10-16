//! The raids "All-Hands Offsite" and "VibeCon" (`raid.rs`) and the Weekly Sprint ladder
//! (`sprint.rs`): difficulty by group size, the level-gated doors, Pair Up tethers, the Synergy
//! Twins, the blank slide, the family dinner, weekly lockouts with their save keys, raid loot,
//! VibeCon's exploding PRs, reversed controls, the interruptible demo, the three-seat Hype
//! Panel and the keynote's phases, and the weekly seed with its ladder. Balance lives in
//! tests/coop.rs.

mod common;

use combat_core::{
    Command, Content, EntityId, Event, GearSource, SaveData, Sim, SpellId, Vec3, weekly_sprint,
};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const DAY: u64 = 86_400;
const SECOND: u32 = 30;
const PAIR_UP: SpellId = SpellId(1100);
const SLIDES: SpellId = SpellId(1106);

/// Packs far away, the boss 20 m north of the arrival (outside its aggro range).
fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") || n == "npc_dummy" => at(400.0, 400.0),
        "boss" => at(0.0, -20.0),
        n if n.starts_with("night_") || n.starts_with("npc_") || n.starts_with("secret:") => {
            at(60.0, 60.0)
        }
        n if n.starts_with("prop:") => at(1.0, 0.0),
        // Optional rares stay out of these scenarios.
        n if n.starts_with("rare_") => None,
        _ => at(0.0, 0.0),
    }
}

fn run(sim: &mut Sim, ticks: u32) -> Vec<Event> {
    (0..ticks).flat_map(|_| sim.tick()).collect()
}

/// `n` level-`level` players (fullstack, then others) on the campus; players are made sturdy
/// so mechanics can be watched without anyone dying.
fn group(n: usize, level: u8, tweak: impl FnOnce(&mut Content)) -> (Sim, Vec<EntityId>) {
    let mut c = content();
    for s in &mut c.spells {
        s.yells.clear();
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, 33);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let classes = ["fullstack", "backend", "devops", "frontend", "fullstack"];
    let ids: Vec<EntityId> = (0..n)
        .map(|k| {
            let p = sim.spawn_player(classes[k], Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, level);
            p
        })
        .collect();
    if let Err(e) = sim.load_zone("campus", &anchors, Vec::new()) {
        panic!("{e}");
    }
    (sim, ids)
}

fn sturdy(c: &mut Content) {
    for k in [
        "senior_engineer",
        "backend_dev",
        "devops_eng",
        "frontend_dev",
    ] {
        def_mut(c, k).max_hp = 1_000_000;
    }
}

fn load(sim: &mut Sim, zone: &str) {
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{zone}: {e}");
    }
}

fn unit_of(sim: &Sim, key: &str) -> Option<EntityId> {
    sim.units().find(|u| u.key == key && u.alive).map(|u| u.id)
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

#[test]
fn the_door_needs_the_cap_and_the_group_size_picks_the_difficulty() {
    let door = |sim: &Sim| {
        sim.zone_links("campus")
            .into_iter()
            .find(|l| l.to == "offsite/1")
            .is_some_and(|l| l.unlocked)
    };
    let (mut sim, ids) = group(2, 9, |_| {});
    assert!(!door(&sim), "level 9 members keep the shuttle shut");
    sim.set_level_of(ids[1], 10);
    assert!(!door(&sim), "everyone needs the cap");
    sim.set_level_of(ids[0], 10);
    assert!(door(&sim));
    // Inside, the way back and the walk between rooms follow the content links.
    let back = sim
        .zone_links("offsite/1")
        .into_iter()
        .find(|l| l.to == "campus")
        .is_some_and(|l| l.unlocked);
    assert!(back);

    load(&mut sim, "offsite/1");
    let view = sim.raid_view();
    assert_eq!(view.zone, "offsite/1");
    assert!(view.async_mode, "two players attend asynchronously");
    let (mut trio, _) = group(3, 10, |_| {});
    let ev = {
        load(&mut trio, "offsite/1");
        run(&mut trio, 1)
    };
    assert!(!trio.raid_view().async_mode, "three is a full programme");
    assert!(
        trio.units()
            .any(|u| u.key == "icebreaker_facilitator" && u.alive),
        "the boss is in"
    );
    let _ = ev;
    load(&mut trio, "campus");
    assert_eq!(
        trio.raid_view().zone,
        "",
        "leaving the raid clears the view"
    );
}

#[test]
fn async_attendance_scales_enemies_down() {
    let hp = |players: usize| {
        let (mut sim, _) = group(players, 10, |_| {});
        sim.debug_force_raid_async(Some(players < 3));
        load(&mut sim, "offsite/1");
        sim.units()
            .find(|u| u.key == "icebreaker_facilitator")
            .map_or(0, |u| u.max_hp)
    };
    let (solo, trio) = (hp(1), hp(3));
    let c = content();
    let full = c.unit_def("icebreaker_facilitator").map_or(0, |u| u.max_hp);
    assert_eq!(solo, full * c.raids[0].async_hp_pct / 100);
    assert!(trio > full * 2, "{trio}: group scaling on top of Normal");
}

#[test]
fn pair_up_hurts_a_pair_only_while_it_drifts_apart() {
    let (mut sim, ids) = group(2, 10, sturdy);
    load(&mut sim, "offsite/1");
    // Pull: both stand close to each other and the facilitator.
    for (k, &p) in ids.iter().enumerate() {
        move_to(&mut sim, p, Vec3::new(k as f32, 0.0, -10.0));
    }
    let mut paired = false;
    for _ in 0..15 * SECOND {
        let ev = sim.tick();
        if ev
            .iter()
            .any(|e| matches!(e, Event::Raid { view } if !view.tethers.is_empty()))
        {
            paired = true;
            break;
        }
    }
    assert!(paired, "the facilitator paired the group up");
    let view = sim.raid_view();
    assert_eq!(view.tethers.len(), 1);
    let (x, y) = view.tethers[0];
    assert!((x, y) == (ids[0], ids[1]) || (y, x) == (ids[0], ids[1]));
    assert_eq!(view.tether_m, 8);
    let hurt = |ev: &[Event]| {
        ev.iter()
            .filter(|e| matches!(e, Event::Damage { spell: Some(s), .. } if *s == PAIR_UP))
            .count()
    };
    // Together: nothing.
    let ev = run(&mut sim, 3 * SECOND);
    assert_eq!(hurt(&ev), 0);
    // Apart: both partners take it every second.
    move_to(&mut sim, ids[1], Vec3::new(20.0, 0.0, -10.0));
    let ev = run(&mut sim, 3 * SECOND);
    assert!(hurt(&ev) >= 4, "{} tether ticks", hurt(&ev));
    let targets: Vec<EntityId> = ev
        .iter()
        .filter_map(|e| match e {
            Event::Damage {
                spell: Some(s),
                target,
                ..
            } if *s == PAIR_UP => Some(*target),
            _ => None,
        })
        .collect();
    assert!(ids.iter().all(|p| targets.contains(p)));
    // It runs out after 12 s.
    run(&mut sim, 12 * SECOND);
    assert!(sim.raid_view().tethers.is_empty());
}

#[test]
fn a_lone_attendee_is_paired_with_the_facilitator() {
    let (mut sim, ids) = group(1, 10, sturdy);
    load(&mut sim, "offsite/1");
    move_to(&mut sim, ids[0], Vec3::new(0.0, 0.0, -10.0));
    let boss = unit_of(&sim, "icebreaker_facilitator").expect("boss");
    let mut found = None;
    for _ in 0..15 * SECOND {
        sim.tick();
        if let Some(t) = sim.raid_view().tethers.first() {
            found = Some(*t);
            break;
        }
    }
    assert_eq!(found, Some((ids[0], boss)));
}

#[test]
fn the_synergy_twins_share_one_health_bar_and_swap_the_shield() {
    let (mut sim, ids) = group(3, 10, sturdy);
    load(&mut sim, "offsite/2");
    let a = unit_of(&sim, "synergy_alignment").expect("Alignment");
    let b = unit_of(&sim, "synergy_leverage").expect("Leverage");
    let shielded = |sim: &Sim, id: EntityId| {
        sim.unit(id)
            .is_some_and(|u| u.auras.iter().any(|x| x.key == "synergy_shield"))
    };
    run(&mut sim, 1);
    assert!(
        shielded(&sim, b) && !shielded(&sim, a),
        "Leverage starts aligned"
    );
    // Linked HP: a hit on one shows on both.
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(0.0, 0.0, -14.0));
    }
    let p = ids[0];
    sim.apply_as(
        p,
        Command::SetTarget {
            unit: p,
            target: Some(a),
        },
    );
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SpellId(1),
        },
    );
    run(&mut sim, 3 * SECOND);
    let (ua, ub) = (sim.unit(a).cloned(), sim.unit(b).cloned());
    let (ua, ub) = (ua.expect("a"), ub.expect("b"));
    assert!(ua.hp < ua.max_hp, "Alignment took the hit");
    assert_eq!(
        ua.hp * 100 / ua.max_hp,
        ub.hp * 100 / ub.max_hp,
        "shared health"
    );
    // The swap: places and the shield change hands.
    let (pa, pb) = (ua.pos, ub.pos);
    run(&mut sim, 19 * SECOND);
    assert!(shielded(&sim, a) && !shielded(&sim, b), "the shield moved");
    let na = sim.unit(a).map(|u| u.pos).unwrap_or_default();
    assert!(combat_core::distance(na, pb) < combat_core::distance(na, pa) + 6.0);
    // One dies, both die; the floor boss pays out and locks the encounter.
    assert!(sim.debug_kill(b));
    let ev = run(&mut sim, 1);
    assert!(sim.unit(a).is_some_and(|u| !u.alive), "Alignment went too");
    assert!(sim.raid_locked("synergy_alignment", false));
    let drops: usize = ev
        .iter()
        .filter_map(|e| match e {
            Event::LootDropped { items, .. } => Some(items.len()),
            _ => None,
        })
        .sum();
    assert!(drops > 0, "the floor boss dropped its pieces");
}

#[test]
fn the_slide_wall_leaves_one_blank_slide() {
    let (mut sim, ids) = group(3, 10, sturdy);
    load(&mut sim, "offsite/3");
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(0.0, 0.0, -12.0));
    }
    let mut lanes = Vec::new();
    for _ in 0..12 * SECOND {
        sim.tick();
        lanes = sim
            .telegraphs()
            .into_iter()
            .filter(|t| t.spell == SLIDES)
            .collect();
        if !lanes.is_empty() {
            break;
        }
    }
    let count = sim
        .content()
        .spell(SLIDES)
        .and_then(|s| s.telegraph.as_ref())
        .map_or(0, |t| usize::from(t.count));
    assert_eq!(lanes.len(), count - 1, "one slide is blank");
}

#[test]
fn ignoring_the_family_dinner_wipes_the_raid() {
    let (mut sim, ids) = group(3, 10, |_| {});
    load(&mut sim, "offsite/4");
    let founder = unit_of(&sim, "the_founder").expect("Founder");
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(0.0, 0.0, -14.0));
    }
    run(&mut sim, SECOND);
    // Straight to phase two: the family sits down.
    assert!(sim.debug_set_hp(
        founder,
        sim.unit(founder).map_or(1, |u| u.max_hp * 60 / 100)
    ));
    let mut seated = false;
    let mut dead = 0;
    for _ in 0..20 * SECOND {
        for e in sim.tick() {
            match e {
                Event::Summoned { .. } => seated = true,
                Event::Death { unit, .. } if ids.contains(&unit) => dead += 1,
                _ => {}
            }
        }
    }
    assert!(seated, "the family took their seats");
    assert_eq!(dead, ids.len(), "nobody survives dessert");
}

#[test]
fn raid_bosses_lock_per_difficulty_for_the_week_and_the_lockout_is_saved() {
    let (mut sim, ids) = group(1, 10, |_| {});
    load(&mut sim, "offsite/1");
    assert!(sim.raid_async());
    let boss = unit_of(&sim, "icebreaker_facilitator").expect("boss");
    assert!(sim.debug_kill(boss));
    let ev = run(&mut sim, 1);
    let items: Vec<String> = ev
        .iter()
        .filter_map(|e| match e {
            Event::LootDropped { items, .. } => Some(items.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    let c = content();
    assert!(
        items.iter().all(|n| c
            .economy
            .gear(n)
            .is_none_or(|g| matches!(&g.source, GearSource::Raid | GearSource::Boss(_)))),
        "{items:?}"
    );
    assert!(sim.raid_locked("icebreaker_facilitator", true));
    assert!(!sim.raid_locked("icebreaker_facilitator", false));
    assert_eq!(sim.raid_view().locked, ["async:icebreaker_facilitator"]);
    let achieved = |sim: &Sim| {
        sim.unit(ids[0])
            .map(|u| u.achievements.clone())
            .unwrap_or_default()
    };
    assert!(achieved(&sim).contains(&"offsite_icebreaker".to_string()));
    // Locked: the boss stays away on this difficulty, the next room is open for good.
    load(&mut sim, "campus");
    load(&mut sim, "offsite/1");
    assert!(unit_of(&sim, "icebreaker_facilitator").is_none());
    assert!(
        sim.zone_links("offsite/1")
            .iter()
            .any(|l| l.to == "offsite/2" && l.unlocked)
    );
    sim.debug_force_raid_async(Some(false));
    load(&mut sim, "offsite/1");
    assert!(
        unit_of(&sim, "icebreaker_facilitator").is_some(),
        "Normal is a separate lockout"
    );
    // Saved and restored in the same week; gone after Monday.
    let save = sim.export_save(ids[0]).expect("save");
    let back = SaveData::decode(&save.encode()).expect("decode");
    assert_eq!(back.raid_locks, ["async:icebreaker_facilitator"]);
    let (mut fresh, q) = group(1, 10, |_| {});
    fresh.set_clock(THURSDAY + DAY);
    assert!(fresh.import_save(q[0], &back));
    assert!(fresh.raid_locked("icebreaker_facilitator", true));
    fresh.set_clock(THURSDAY + 4 * DAY);
    assert!(!fresh.raid_locked("icebreaker_facilitator", true));
    let old = "the-last-dev save\nversion=3\nclass=devops\nlevel=10\n";
    assert!(
        SaveData::decode(old)
            .is_some_and(|s| s.raid_locks.is_empty() && s.ladder.entries.is_empty())
    );
}

#[test]
fn raid_bosses_pay_every_department_and_relieve_burnout() {
    let (mut sim, ids) = group(3, 10, |_| {});
    load(&mut sim, "offsite/4");
    sim.debug_set_burnout(ids[0], 500);
    let founder = unit_of(&sim, "the_founder").expect("Founder");
    assert!(sim.debug_kill(founder));
    run(&mut sim, 1);
    let u = sim.unit(ids[0]).cloned().expect("player");
    let c = content();
    assert!(
        u.career.rep.iter().all(|&r| r == c.raids[0].boss_rep),
        "{:?}",
        u.career.rep
    );
    assert!(
        u.career.burnout < 500,
        "management defeated: {}",
        u.career.burnout
    );
    for k in ["offsite_founder", "offsite_full_programme"] {
        assert!(u.achievements.contains(&k.to_string()), "{k}");
    }
}

#[test]
fn the_weekly_sprint_is_fixed_per_week_and_its_ladder_is_kept() {
    let c = content();
    let a = weekly_sprint(&c.sprint, 2960).expect("weekly");
    assert_eq!(
        Some(&a),
        weekly_sprint(&c.sprint, 2960).as_ref(),
        "same week, same sprint"
    );
    assert_eq!(a.level, c.sprint.weekly_level);
    assert_eq!(a.affixes.len(), 3, "level 7 has three affix slots");
    let weeks: Vec<_> = (2960..2972)
        .filter_map(|w| weekly_sprint(&c.sprint, w))
        .map(|w| (w.building, w.affixes))
        .collect();
    assert!(weeks.windows(2).any(|p| p[0] != p[1]), "it rotates");

    let (mut sim, ids) = group(1, 10, |_| {});
    let w = sim.weekly_sprint().expect("this week");
    sim.set_ladder_name(ids[0], "Ada|Lovelace");
    sim.apply_as(ids[0], Command::StartWeeklySprint);
    run(&mut sim, 1);
    let st = sim.sprint_status().expect("running");
    assert_eq!(
        (st.building.as_str(), st.level),
        (w.building.as_str(), w.level)
    );
    assert_eq!(st.affixes, w.affixes);
    assert!(sim.debug_end_sprint(true));
    let ev = run(&mut sim, 1);
    let ladder = ev
        .iter()
        .find_map(|e| match e {
            Event::Ladder { view } => Some(view.clone()),
            _ => None,
        })
        .expect("ladder event");
    assert_eq!(ladder.entries.len(), 1);
    assert_eq!(ladder.entries[0].crew, ["AdaLovelace (Fullstack Engineer)"]);
    sim.debug_ladder_add(5_000, 0, &["Grace (Backend Engineer)"]);
    let view = sim.ladder_view();
    assert_eq!(view.entries.len(), 2);
    assert!(
        view.entries[0].time_ms <= view.entries[1].time_ms,
        "fastest first"
    );
    // The local ladder survives a save; a joiner online sees the room's.
    let save = sim.export_save(ids[0]).expect("save");
    let back = SaveData::decode(&save.encode()).expect("decode");
    assert_eq!(back.ladder, view);
    let (mut fresh, q) = group(1, 10, |_| {});
    assert!(fresh.import_save(q[0], &back));
    assert_eq!(fresh.ladder_view(), view);
    let joiner = fresh.spawn_player("devops", Vec3::new(3.0, 0.0, 0.0), 0.0);
    let ev = run(&mut fresh, 1);
    let _ = joiner;
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Ladder { view: v } if *v == view))
    );
    // A new week starts an empty ladder.
    fresh.set_clock(THURSDAY + 7 * DAY);
    assert!(fresh.ladder_view().entries.is_empty());
}

// ---- VibeCon, the second raid: same framework, its own mechanics ----

fn has_aura(sim: &Sim, id: EntityId, key: &str) -> bool {
    sim.unit(id)
        .is_some_and(|u| u.auras.iter().any(|a| a.key == key))
}

/// Everyone at `pos`, the first member targeting `t` with Code Review (pulls it).
fn pull(sim: &mut Sim, ids: &[EntityId], pos: Vec3, t: EntityId) {
    for &p in ids {
        move_to(sim, p, pos);
    }
    let p = ids[0];
    sim.apply_as(
        p,
        Command::SetTarget {
            unit: p,
            target: Some(t),
        },
    );
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SpellId(1),
        },
    );
}

#[test]
fn vibecon_has_its_own_door_rooms_lockout_and_loot() {
    let c = content();
    let vibecon = c.raid_of("vibecon/3").expect("VibeCon");
    assert_eq!(vibecon.name, "VibeCon");
    assert_eq!(vibecon.bosses(&c).len(), 5);
    assert!(
        c.raid_of("offsite/1")
            .is_some_and(|r| r.name != vibecon.name)
    );
    let door = |sim: &Sim, from: &str, to: &str| {
        sim.zone_links(from)
            .into_iter()
            .find(|l| l.to == to)
            .is_some_and(|l| l.unlocked)
    };
    let (mut sim, ids) = group(1, 9, |_| {});
    assert!(!door(&sim, "campus", "vibecon/1"), "level 9 stays outside");
    sim.set_level_of(ids[0], 10);
    assert!(door(&sim, "campus", "vibecon/1"));
    assert!(
        !door(&sim, "vibecon/1", "vibecon/2"),
        "rooms open boss by boss"
    );
    load(&mut sim, "vibecon/1");
    assert_eq!(sim.raid_view().zone, "vibecon/1");
    assert!(sim.raid_async(), "one player attends asynchronously");
    // The Vibe Coder drops VibeCon swag only, locks for the week and opens the next room.
    let boss = unit_of(&sim, "vc_vibe_coder").expect("Vibe Coder");
    assert!(sim.debug_kill(boss));
    let ev = run(&mut sim, 1);
    let ours = ["booth_duty", "volunteer_crew", "speaker_kit", "all_nighter"];
    for n in ev
        .iter()
        .filter_map(|e| match e {
            Event::LootDropped { items, .. } => Some(items.clone()),
            _ => None,
        })
        .flatten()
    {
        if let Some(g) = c.economy.gear(&n).filter(|g| g.source == GearSource::Raid) {
            assert!(g.set.as_deref().is_some_and(|s| ours.contains(&s)), "{n}");
        }
    }
    assert!(sim.raid_locked("vc_vibe_coder", true));
    assert!(door(&sim, "vibecon/1", "vibecon/2"));
    let achieved = sim
        .unit(ids[0])
        .map(|u| u.achievements.clone())
        .unwrap_or_default();
    assert!(achieved.contains(&"vibecon_vibe_coder".to_string()));
    // Both raids share one lockout list in the save.
    sim.debug_set_raid_lockout(&[
        "async:vc_vibe_coder".to_string(),
        "icebreaker_facilitator".to_string(),
        "async:nobody".to_string(),
    ]);
    let save = sim.export_save(ids[0]).expect("save");
    let back = SaveData::decode(&save.encode()).expect("decode");
    assert_eq!(
        back.raid_locks,
        ["async:vc_vibe_coder", "icebreaker_facilitator"]
    );
}

#[test]
fn an_unreviewed_pr_explodes_in_prod_unless_it_is_killed() {
    let shot = |kill_prs: bool| {
        let (mut sim, ids) = group(3, 10, sturdy);
        load(&mut sim, "vibecon/1");
        let boss = unit_of(&sim, "vc_vibe_coder").expect("boss");
        pull(&mut sim, &ids, Vec3::new(0.0, 0.0, -12.0), boss);
        let (mut spawned, mut blasts) = (0, 0);
        for _ in 0..22 * SECOND {
            for e in sim.tick() {
                match e {
                    Event::Summoned { unit, .. } => {
                        spawned += 1;
                        if kill_prs {
                            assert!(sim.debug_kill(unit));
                        }
                    }
                    Event::Damage {
                        spell: Some(SpellId(1223)),
                        ..
                    } => blasts += 1,
                    _ => {}
                }
            }
        }
        let stacks = sim
            .unit(boss)
            .and_then(|u| {
                u.auras
                    .iter()
                    .find(|a| a.key == "shipped_to_prod")
                    .map(|a| a.stacks)
            })
            .unwrap_or(0);
        let prs = sim
            .units()
            .filter(|u| u.key == "vc_unreviewed_pr" && u.alive)
            .count();
        (spawned, blasts, stacks, prs)
    };
    let (spawned, blasts, stacks, _) = shot(false);
    assert!(spawned >= 1, "the Vibe Coder shipped a PR");
    assert!(blasts >= 3, "the merge hit the whole group: {blasts}");
    assert!(stacks >= 1, "and he shipped to prod");
    let (spawned, blasts, stacks, prs) = shot(true);
    assert!(spawned >= 1);
    assert_eq!((blasts, stacks, prs), (0, 0, 0), "a killed PR never merges");
}

#[test]
fn ignore_previous_instructions_reverses_only_whoever_stays_in_the_circle() {
    let (mut sim, ids) = group(2, 10, sturdy);
    assert!(sim.content().reverses_controls(["vc_reversed"].into_iter()));
    assert!(!sim.content().reverses_controls(["paired_up"].into_iter()));
    load(&mut sim, "vibecon/2");
    let boss = unit_of(&sim, "vc_prompt_whisperer").expect("boss");
    pull(&mut sim, &ids, Vec3::new(0.0, 0.0, -12.0), boss);
    // Wait for the circles, then the first member steps out and the second stays.
    let mut warned = false;
    for _ in 0..14 * SECOND {
        sim.tick();
        if sim.telegraphs().iter().any(|t| t.spell == SpellId(1225)) {
            warned = true;
            break;
        }
    }
    assert!(warned, "the circles show before anything happens");
    move_to(&mut sim, ids[0], Vec3::new(8.0, 0.0, -4.0));
    move_to(&mut sim, ids[1], Vec3::new(0.0, 0.0, -12.0));
    let mut hit = false;
    for _ in 0..4 * SECOND {
        sim.tick();
        hit |= has_aura(&sim, ids[1], "vc_reversed");
        assert!(
            !has_aura(&sim, ids[0], "vc_reversed"),
            "stepped out in time"
        );
    }
    assert!(hit, "standing in it reverses the controls");
    run(&mut sim, 5 * SECOND);
    assert!(
        !has_aura(&sim, ids[1], "vc_reversed"),
        "only for a few seconds"
    );
}

#[test]
fn the_demo_god_breaks_the_fight_unless_interrupted() {
    let demo = |kick: bool| {
        let (mut sim, ids) = group(1, 10, sturdy);
        sim.debug_force_raid_async(Some(false));
        load(&mut sim, "vibecon/3");
        let boss = unit_of(&sim, "vc_demo_god").expect("boss");
        pull(&mut sim, &ids, Vec3::new(0.0, 0.0, -10.0), boss);
        let mut bugs = 0;
        for _ in 0..16 * SECOND {
            let casting = sim
                .unit(boss)
                .and_then(|u| u.cast)
                .is_some_and(|c| c.spell == SpellId(1230));
            if kick && casting && sim.cooldown(ids[0], SpellId(4)).usable.is_ok() {
                sim.apply_as(
                    ids[0],
                    Command::Cast {
                        caster: ids[0],
                        spell: SpellId(4),
                    },
                );
            }
            for e in sim.tick() {
                if let Event::Summoned { unit, .. } = e
                    && sim.unit(unit).is_some_and(|u| u.key == "vc_demo_bug")
                {
                    bugs += 1;
                }
            }
        }
        bugs
    };
    assert_eq!(demo(false), 2, "an unstopped demo releases two bugs");
    assert_eq!(demo(true), 0, "an interrupted demo releases none");
}

#[test]
fn the_hype_panel_shares_one_health_bar_and_passes_the_mic_in_turn() {
    let (mut sim, ids) = group(3, 10, sturdy);
    load(&mut sim, "vibecon/4");
    let keys = ["vc_hype_moderator", "vc_hype_vc", "vc_hype_caio"];
    let panel: Vec<EntityId> = keys
        .iter()
        .map(|k| unit_of(&sim, k).expect("panellist"))
        .collect();
    run(&mut sim, 1);
    let open = |sim: &Sim| -> Vec<usize> {
        (0..3)
            .filter(|&k| !has_aura(sim, panel[k], "panel_muted"))
            .collect()
    };
    assert_eq!(open(&sim), [0], "the moderator starts with the mic");
    let seats: Vec<Vec3> = panel
        .iter()
        .map(|&u| sim.unit(u).map(|u| u.pos).unwrap_or_default())
        .collect();
    pull(&mut sim, &ids, Vec3::new(0.0, 0.0, -14.0), panel[0]);
    run(&mut sim, 3 * SECOND);
    let pct = |sim: &Sim, u: EntityId| sim.unit(u).map_or(0, |u| u.hp * 100 / u.max_hp);
    assert!(pct(&sim, panel[0]) < 100, "the moderator took the hit");
    assert!(
        panel.iter().all(|&u| pct(&sim, u) == pct(&sim, panel[0])),
        "one health bar"
    );
    run(&mut sim, 15 * SECOND);
    assert_eq!(open(&sim), [1], "the mic passed to the VC");
    run(&mut sim, 15 * SECOND);
    assert_eq!(open(&sim), [2], "then to the Chief AI Officer");
    for (k, &u) in panel.iter().enumerate().skip(1) {
        let at = sim.unit(u).map(|u| u.pos).unwrap_or_default();
        assert!(
            combat_core::distance(at, seats[k]) < 0.5,
            "panellists stay seated"
        );
    }
    assert!(sim.debug_kill(panel[2]));
    run(&mut sim, 1);
    assert!(panel.iter().all(|&u| sim.unit(u).is_some_and(|u| !u.alive)));
    assert!(sim.raid_locked("vc_hype_moderator", false));
}

#[test]
fn the_keynote_has_one_more_thing_per_phase_and_counts_as_management() {
    let (mut sim, ids) = group(3, 10, sturdy);
    load(&mut sim, "vibecon/5");
    let boss = unit_of(&sim, "vc_keynote_agi").expect("keynote");
    pull(&mut sim, &ids, Vec3::new(0.0, 0.0, -14.0), boss);
    run(&mut sim, SECOND);
    let max = sim.unit(boss).map_or(1, |u| u.max_hp);
    assert!(sim.debug_set_hp(boss, max * 60 / 100));
    let mut beta = false;
    for _ in 0..4 * SECOND {
        for e in sim.tick() {
            if let Event::Summoned { unit, .. } = e {
                beta |= sim.unit(unit).is_some_and(|u| u.key == "vc_agi_beta");
            }
        }
    }
    assert!(beta, "one more thing: AGI (Beta)");
    assert!(sim.debug_set_hp(boss, max * 30 / 100));
    run(&mut sim, 2 * SECOND);
    assert!(
        has_aura(&sim, boss, "enterprise_pricing"),
        "one more thing: pricing"
    );
    sim.debug_set_burnout(ids[0], 500);
    assert!(sim.debug_kill(boss));
    run(&mut sim, 1);
    let u = sim.unit(ids[0]).cloned().expect("player");
    assert!(u.career.burnout < 500, "the keynote was management");
    for k in ["vibecon_keynote", "vibecon_full_pass"] {
        assert!(u.achievements.contains(&k.to_string()), "{k}");
    }
    let title = sim
        .content()
        .world
        .achievements
        .iter()
        .find(|a| a.key == "vibecon_keynote")
        .map(|a| a.title.clone());
    assert_eq!(title.as_deref(), Some("Speaker"));
}
