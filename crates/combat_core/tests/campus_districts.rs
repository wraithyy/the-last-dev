//! The open-world campus districts: fixed-level district packs, `Interact` quest objectives,
//! the shuttle loop (fast travel) and the public world events (`world_events.rs`).

mod common;

use combat_core::{Command, Content, EntityId, Event, QuestStatus, Sim, Vec3, WorldEventPhase};
use common::content;

/// 2026-10-15 00:00 UTC.
const CLOCK: u64 = 1_792_022_400;
const SECOND: u32 = 30;

/// Packs far away, the district NPCs, props and both event spots around the origin.
fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") || n.starts_with("rare_") || n == "boss" => at(400.0, 400.0),
        "npc_ranger" | "npc_food_truck" | "npc_marshal" => at(1.5, 0.0),
        "event:free_pizza" | "event:fire_drill" => at(0.0, -6.0),
        "prop:hydration_1" => at(1.0, 1.0),
        "prop:hydration_2" => at(-1.0, 1.0),
        "prop:hydration_3" => at(0.0, -1.0),
        "prop:shuttle_plaza" => at(1.0, 0.0),
        "prop:shuttle_park" => at(50.0, 20.0),
        n if n.starts_with("npc_") => at(-60.0, 60.0),
        n if n.starts_with("night_") => at(-200.0, 200.0),
        "graveyard" => at(0.0, 10.0),
        _ => None,
    }
}

fn campus(level: u8, tweak: impl FnOnce(&mut Content)) -> (Sim, EntityId) {
    let mut c = content();
    for s in &mut c.spells {
        s.yells.clear();
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, 5);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_player_level(level);
    if let Err(e) = sim.load_zone("campus", &anchors, Vec::new()) {
        panic!("{e}");
    }
    (sim, p)
}

fn run(sim: &mut Sim, ticks: u32) -> Vec<Event> {
    (0..ticks).flat_map(|_| sim.tick()).collect()
}

fn phase(events: &[Event], key: &str, want: WorldEventPhase) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::WorldEvent { key: k, phase, .. } if k == key && *phase == want))
}

/// Kills every living unit with this key (a player kill), as a group mopping up waves would.
fn kill_all(sim: &mut Sim, key: &str) -> usize {
    let ids: Vec<EntityId> = sim
        .units()
        .filter(|u| u.alive && u.key == key)
        .map(|u| u.id)
        .collect();
    ids.into_iter().filter(|id| sim.debug_kill(*id)).count()
}

#[test]
fn district_packs_keep_their_own_level() {
    let (sim, _) = campus(1, |_| {});
    let level_of = |key: &str| {
        sim.units()
            .filter(|u| u.key == key)
            .map(|u| u.level)
            .max()
            .unwrap_or(0)
    };
    assert_eq!(level_of("site_foreman"), 5);
    assert_eq!(level_of("rto_commuter"), 5);
    assert_eq!(level_of("lunch_optimizer"), 2);
    // The core keeps its 1-2 split: the first half low, the rest high.
    assert_eq!(level_of("chief_happiness_officer"), 2);
    assert_eq!(
        level_of("delivery_robot"),
        2,
        "pack_12 brings a level-2 robot"
    );
}

#[test]
fn interact_objectives_count_prop_uses() {
    let (mut sim, p) = campus(2, |_| {});
    sim.apply_as(
        p,
        Command::AcceptQuest {
            quest: "pk_hydration_kpi".into(),
        },
    );
    run(&mut sim, 1);
    assert_eq!(sim.quest_status("pk_hydration_kpi"), QuestStatus::Active);
    for prop in ["hydration_1", "hydration_2", "hydration_3"] {
        sim.apply_as(p, Command::Interact { prop: prop.into() });
        run(&mut sim, 2);
    }
    assert_eq!(sim.quest_status("pk_hydration_kpi"), QuestStatus::Ready);
}

#[test]
fn the_shuttle_takes_you_to_the_next_stop() {
    let (mut sim, p) = campus(3, |_| {});
    sim.apply_as(
        p,
        Command::Interact {
            prop: "shuttle_plaza".into(),
        },
    );
    let out = run(&mut sim, 1);
    let want = Vec3::new(50.0, 0.0, 20.0);
    assert!(
        out.iter()
            .any(|e| matches!(e, Event::Teleported { unit, pos } if *unit == p && *pos == want))
    );
    assert_eq!(sim.unit(p).map(|u| u.pos), Some(want));
}

#[test]
fn free_pizza_pays_everyone_who_held_the_line() {
    let (mut sim, p) = campus(3, |_| {});
    // A second member far from the pizza takes no part.
    let far = sim.spawn_player("devops", Vec3::new(-80.0, 0.0, -80.0), 0.0);
    sim.set_clock(CLOCK);
    assert!(sim.debug_world_event_in("free_pizza", 3));
    let mut out = run(&mut sim, 4 * SECOND);
    assert!(phase(&out, "free_pizza", WorldEventPhase::Warning));
    assert!(phase(&out, "free_pizza", WorldEventPhase::Started));
    assert!(sim.units().any(|u| u.key == "pizza_table" && u.alive));
    let xp_before = |sim: &Sim, id| sim.unit(id).map_or(0, |u| u.xp);
    let (mine, theirs) = (xp_before(&sim, p), xp_before(&sim, far));
    let mut waves = 0;
    for _ in 0..95 {
        out = run(&mut sim, SECOND);
        waves += kill_all(&mut sim, "pizza_vulture");
        if phase(&out, "free_pizza", WorldEventPhase::Won) {
            break;
        }
    }
    assert!(waves >= 6, "only {waves} waves came");
    assert!(phase(&out, "free_pizza", WorldEventPhase::Won));
    assert!(xp_before(&sim, p) > mine, "the defender got XP");
    assert_eq!(xp_before(&sim, far), theirs, "the bystander got nothing");
    let has = |id| {
        sim.unit(id)
            .is_some_and(|u| u.achievements.iter().any(|a| a == "pizza_defender"))
    };
    assert!(has(p) && !has(far));
    assert!(
        !sim.units().any(|u| u.key == "pizza_table" && u.alive),
        "the table leaves with the event"
    );
    assert!(sim.world_event_in("free_pizza").is_some_and(|s| s > 600));
}

#[test]
fn a_fallen_pizza_fails_the_event() {
    let (mut sim, _) = campus(3, |c| {
        if let Some(u) = c.units.iter_mut().find(|u| u.key == "pizza_table") {
            u.max_hp = 1;
        }
    });
    sim.set_clock(CLOCK);
    assert!(sim.debug_world_event_in("free_pizza", 1));
    let out = run(&mut sim, 30 * SECOND);
    assert!(phase(&out, "free_pizza", WorldEventPhase::Failed));
    assert!(!phase(&out, "free_pizza", WorldEventPhase::Won));
}

#[test]
fn the_fire_drill_ends_early_when_enough_wardens_fall() {
    let (mut sim, p) = campus(4, |_| {});
    sim.set_clock(CLOCK);
    assert!(sim.debug_world_event_in("fire_drill_event", 1));
    let mut all = run(&mut sim, 2 * SECOND);
    assert!(phase(&all, "fire_drill_event", WorldEventPhase::Started));
    for _ in 0..110 {
        let out = run(&mut sim, SECOND);
        kill_all(&mut sim, "fire_warden");
        let done = phase(&out, "fire_drill_event", WorldEventPhase::Won);
        all.extend(out);
        if done {
            break;
        }
    }
    assert!(phase(&all, "fire_drill_event", WorldEventPhase::Won));
    let counts: Vec<u32> = all
        .iter()
        .filter_map(|e| match e {
            Event::WorldEvent {
                phase: WorldEventPhase::Progress,
                count,
                ..
            } => Some(*count),
            _ => None,
        })
        .collect();
    assert!(counts.windows(2).all(|w| w[0] <= w[1]) && counts.last() == Some(&12));
    assert!(
        sim.unit(p)
            .is_some_and(|u| u.achievements.iter().any(|a| a == "fire_drill_veteran"))
    );
}

#[test]
fn world_events_wait_for_the_clock_and_the_zone() {
    let (mut sim, _) = campus(3, |_| {});
    let out = run(&mut sim, 10 * SECOND);
    assert!(!out.iter().any(|e| matches!(e, Event::WorldEvent { .. })));
    sim.set_clock(CLOCK);
    let first = content()
        .live
        .events
        .iter()
        .find(|e| e.key == "free_pizza")
        .map_or(0, |e| e.first_ms / 1000);
    assert_eq!(sim.world_event_in("free_pizza"), Some(first));
}
