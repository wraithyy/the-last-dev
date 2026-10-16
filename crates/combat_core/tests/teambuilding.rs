//! "Mandatory Fun: Teambuilding" and the minigame frame: the start from the campus, patrol
//! loops, vision cones and noise, the four catch outcomes, hiding spots, objectives, scoring,
//! medals with their rewards, leaving early, co-op group scores and the saved best medal.
//! `minigame.rs`, `teambuilding.rs`, docs/FORMULAS.md.

mod common;

use combat_core::{
    Aabb, Command, EntityId, Event, Medal, MinigameResult, SaveData, Sim, Vec3, WatcherKind,
};
use common::content;

const SECOND: u32 = 30;
const VENUE: &str = "teambuilding/1";
/// Where every watcher but the one under test waits: far away, out of sight.
const FAR: (f32, f32) = (500.0, 500.0);

/// The venue under test: start at the origin, objectives in a row to the north, the exit east,
/// the icebreaker south, one hiding spot, one canape, and watcher route `near` (if any) as a
/// single waypoint at `at`, looking north (-Z) with its scan.
fn venue(near: Option<&'static str>, at: (f32, f32)) -> impl Fn(&str) -> Option<(Vec3, f32)> {
    move |name: &str| {
        let p = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
        match name {
            "tb_start" | "player_spawn" => p(0.0, 0.0),
            "tb_exit" => p(60.0, 0.0),
            "tb_icebreaker" => p(0.0, 30.0),
            "obj_canape" => p(20.0, -40.0),
            "obj_drink" => p(30.0, -40.0),
            "obj_swag" => p(40.0, -40.0),
            "obj_photo" => p(50.0, -40.0),
            "obj_incident" => p(60.0, -40.0),
            "hide_1" => p(2.0, -15.0),
            "snack_1" => p(5.0, 0.0),
            n if n.starts_with("hide_") || n.starts_with("snack_") => p(300.0, 300.0),
            n if n.starts_with("route:") => {
                let (route, k) = n["route:".len()..].rsplit_once(':')?;
                match (k, Some(route) == near) {
                    ("0", true) => p(at.0, at.1),
                    ("0", false) => p(FAR.0 + route.len() as f32, FAR.1),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// Campus props and boards next to the player, packs far away.
fn campus(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") || n == "boss" => at(400.0, 400.0),
        n if n.starts_with("npc_") || n.starts_with("secret:") || n.starts_with("night_") => {
            at(60.0, 60.0)
        }
        "world_boss" | "graveyard" => at(0.0, -60.0),
        _ => at(1.0, 0.0),
    }
}

fn run(sim: &mut Sim, ticks: u32) -> Vec<Event> {
    (0..ticks).flat_map(|_| sim.tick()).collect()
}

fn on_campus(players: usize) -> (Sim, Vec<EntityId>) {
    let mut sim = Sim::new(content(), 7);
    sim.set_wandering(false);
    let ids: Vec<EntityId> = (0..players)
        .map(|_| sim.spawn_player("fullstack", Vec3::default(), 0.0))
        .collect();
    if let Err(e) = sim.load_zone("campus", &campus, Vec::new()) {
        panic!("{e}");
    }
    run(&mut sim, 1);
    (sim, ids)
}

/// The lead starts the minigame; the host follows the travel into `anchors`.
fn enter(
    sim: &mut Sim,
    lead: EntityId,
    anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    blockers: Vec<Aabb>,
) -> Vec<Event> {
    sim.apply_as(
        lead,
        Command::StartMinigame {
            key: "teambuilding".into(),
        },
    );
    let mut out = Vec::new();
    for _ in 0..5 * SECOND {
        out.extend(sim.tick());
        if sim.minigame_travel() == Some(VENUE) {
            break;
        }
    }
    assert_eq!(sim.minigame_travel(), Some(VENUE));
    if let Err(e) = sim.load_zone(VENUE, anchors, blockers) {
        panic!("{e}");
    }
    out.extend(run(sim, 1));
    out
}

fn move_to(sim: &mut Sim, p: EntityId, x: f32, z: f32) {
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos: Vec3::new(x, 0.0, z),
            yaw: 0.0,
            moving: true,
            jumping: false,
        },
    );
}

/// Holds `p` at (x, z) for `ticks`.
fn hold(sim: &mut Sim, p: EntityId, x: f32, z: f32, ticks: u32) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        move_to(sim, p, x, z);
        out.extend(sim.tick());
    }
    out
}

/// Holds `p` at (x, z) until the first catch (at most `ticks`), like a player who stops there.
fn until_caught(sim: &mut Sim, p: EntityId, x: f32, z: f32, ticks: u32) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        move_to(sim, p, x, z);
        let ev = sim.tick();
        let done = !caught(&ev).is_empty();
        out.extend(ev);
        if done {
            break;
        }
    }
    out
}

fn caught(events: &[Event]) -> Vec<(WatcherKind, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::MinigameCaught { kind, line, .. } => Some((*kind, line.clone())),
            _ => None,
        })
        .collect()
}

fn ended(events: &[Event]) -> Vec<MinigameResult> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::MinigameEnded { result } => Some(result.clone()),
            _ => None,
        })
        .collect()
}

/// Walks every objective (with its dwell), then the exit.
fn do_everything(sim: &mut Sim, p: EntityId) -> Vec<Event> {
    let mut out = Vec::new();
    for (x, secs) in [(20.0, 2), (30.0, 3), (40.0, 2), (50.0, 2), (60.0, 21)] {
        out.extend(hold(sim, p, x, -40.0, secs * SECOND));
    }
    out.extend(hold(sim, p, 60.0, 0.0, 2));
    out
}

#[test]
fn the_lead_starts_it_from_the_campus_and_everyone_gets_a_run() {
    let (mut sim, ids) = on_campus(2);
    // Only the lead may commit the group.
    sim.apply_as(
        ids[1],
        Command::StartMinigame {
            key: "teambuilding".into(),
        },
    );
    run(&mut sim, 2 * SECOND);
    assert_eq!(sim.minigame_travel(), None);
    let events = enter(&mut sim, ids[0], &venue(None, FAR), Vec::new());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::MinigameStarted { key, .. } if key == "teambuilding"))
    );
    assert_eq!(
        sim.minigame_active().map(|m| m.key.as_str()),
        Some("teambuilding")
    );
    let views = run(&mut sim, SECOND);
    for p in &ids {
        assert!(
            views
                .iter()
                .any(|e| matches!(e, Event::Minigame { unit, view } if unit == p && view.collectibles == 10)),
            "no view for {p}"
        );
    }
}

#[test]
fn watchers_walk_their_loops_and_wander_does_not_move_them() {
    let (mut sim, ids) = on_campus(1);
    sim.set_wandering(true);
    enter(&mut sim, ids[0], &venue(None, FAR), Vec::new());
    let ceo = sim
        .units()
        .find(|u| u.key == "tb_ceo")
        .map(|u| (u.id, u.pos))
        .unwrap_or_else(|| panic!("no CEO"));
    // Single-waypoint loops: everyone stays put on their spot through a long wait.
    run(&mut sim, 20 * SECOND);
    assert_eq!(sim.unit(ceo.0).map(|u| u.pos), Some(ceo.1));
}

#[test]
fn a_manager_in_whose_cone_you_stand_circles_you_back_to_the_start() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, &venue(Some("mgr_a"), (0.0, -8.0)), Vec::new());
    let events = hold(&mut sim, p, 0.0, -12.0, 4 * SECOND);
    let c = caught(&events);
    assert_eq!(c.first().map(|c| c.0), Some(WatcherKind::Manager), "{c:?}");
    assert!(c[0].1.contains("circle back"), "{}", c[0].1);
    assert!(events.iter().any(|e| matches!(e,
        Event::Teleported { unit, pos } if *unit == p && pos.x == 0.0 && pos.z == 0.0)));
}

#[test]
fn walls_block_the_cone() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    let wall = Aabb {
        min: Vec3::new(-5.0, 0.0, -10.2),
        max: Vec3::new(5.0, 3.0, -9.8),
    };
    enter(&mut sim, p, &venue(Some("mgr_a"), (0.0, -8.0)), vec![wall]);
    let events = hold(&mut sim, p, 0.0, -12.0, 4 * SECOND);
    assert!(caught(&events).is_empty());
}

#[test]
fn a_hiding_spot_keeps_you_out_of_sight() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, &venue(Some("mgr_a"), (0.0, -8.0)), Vec::new());
    // `hide_1` is at (2, -15), right in the manager's cone.
    let events = hold(&mut sim, p, 2.0, -15.0, 5 * SECOND);
    assert!(caught(&events).is_empty());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Minigame { view, .. } if view.hidden))
    );
}

#[test]
fn an_ai_enthusiast_corners_you_with_a_take_you_can_only_wait_out() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, &venue(Some("ai_a"), (0.0, -8.0)), Vec::new());
    let events = until_caught(&mut sim, p, 0.0, -12.0, 4 * SECOND);
    let c = caught(&events);
    assert_eq!(c.first().map(|c| c.0), Some(WatcherKind::Talker));
    let takes = &content().minigames[0]
        .teambuilding
        .clone()
        .unwrap_or_else(|| panic!("rules"))
        .ai_takes;
    assert!(takes.contains(&c[0].1));
    assert!(sim.unit(p).is_some_and(|u| u.stunned));
    // No teleport: the talker keeps you where you are.
    assert!(!events.iter().any(|e| matches!(e, Event::Teleported { .. })));
    // The root ends after a few seconds, and the grace stops an instant second catch.
    let later = hold(&mut sim, p, 0.0, -12.0, 5 * SECOND + 5);
    assert!(sim.unit(p).is_some_and(|u| !u.stunned));
    assert!(caught(&later).is_empty(), "{:?}", caught(&later));
}

#[test]
fn the_hr_fun_committee_detours_you_through_the_icebreaker() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, &venue(Some("hr"), (0.0, -8.0)), Vec::new());
    let events = until_caught(&mut sim, p, 0.0, -12.0, 4 * SECOND);
    assert_eq!(caught(&events).first().map(|c| c.0), Some(WatcherKind::Hr));
    assert!(events.iter().any(|e| matches!(e,
        Event::Teleported { unit, pos } if *unit == p && pos.z == 30.0)));
    assert!(sim.unit(p).is_some_and(|u| u.stunned));
}

#[test]
fn the_ceo_notices_at_once_and_a_manager_takes_a_moment() {
    let catch_tick = |route: &'static str| {
        let (mut sim, ids) = on_campus(1);
        let p = ids[0];
        enter(&mut sim, p, &venue(Some(route), (0.0, -8.0)), Vec::new());
        // Wait for the scan to face the spot first, outside the cone.
        hold(&mut sim, p, 30.0, 0.0, 10);
        for t in 0..6 * SECOND {
            move_to(&mut sim, p, 0.0, -16.0);
            if !caught(&sim.tick()).is_empty() {
                return t;
            }
        }
        u32::MAX
    };
    let (ceo, mgr) = (catch_tick("ceo"), catch_tick("mgr_a"));
    assert!(ceo < mgr, "ceo {ceo} manager {mgr}");
    assert!(mgr < u32::MAX);
}

#[test]
fn running_is_heard_behind_a_watcher_and_sneaking_is_not() {
    let pass = |step: f32| {
        let (mut sim, ids) = on_campus(1);
        let p = ids[0];
        enter(&mut sim, p, &venue(Some("mgr_a"), (0.0, -8.0)), Vec::new());
        // Behind the manager (south of it), back and forth at `step` metres per tick.
        let mut events = Vec::new();
        for t in 0..6 * SECOND {
            let x = -3.0 + ((t % 40) as f32 * step).min(6.0);
            move_to(&mut sim, p, x, -4.0);
            events.extend(sim.tick());
        }
        caught(&events).len()
    };
    // 7.5 m/s is running, 3 m/s is sneaking (`noisy_mps` 5).
    assert!(pass(0.25) > 0);
    assert_eq!(pass(0.1), 0);
}

#[test]
fn a_clean_run_takes_gold_with_every_reward_once() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    let burnout_before = 400;
    sim.debug_set_burnout(p, burnout_before);
    enter(&mut sim, p, &venue(None, FAR), Vec::new());
    // The exit does nothing before the objectives are done.
    let early = hold(&mut sim, p, 60.0, 0.0, SECOND);
    assert!(ended(&early).is_empty());
    let mut events = hold(&mut sim, p, 5.0, 0.0, 2);
    events.extend(do_everything(&mut sim, p));
    let r = ended(&events);
    assert_eq!(r.len(), 1, "{r:?}");
    let r = &r[0];
    assert_eq!(
        (r.medal, r.catches, r.collected, r.quit),
        (Some(Medal::Gold), 0, 1, false)
    );
    // Time plus collectibles (FORMULAS.md): 2400 + 60 per canape - 3 per second.
    assert_eq!(r.score, 2400 + 60 - 3 * (r.time_ms / 1000));
    assert_eq!(r.group_score, r.score);
    let u = sim.unit(p).unwrap_or_else(|| panic!("player"));
    for item in [
        "Teambuilding Name Tag (Hello, I Am Busy)",
        "Party Leftovers Container (Sealed)",
        "Party Hat (Was Never Seen)",
    ] {
        assert!(u.bag.iter().any(|b| b == item), "{item} missing");
    }
    assert_eq!(u.title, "ghosted_teambuilding");
    assert!(u.career.rep.iter().any(|r| *r >= 200));
    assert_eq!(u.career.burnout, burnout_before - 150);
    assert_eq!(sim.minigame_best(p, "teambuilding"), Some(Medal::Gold));
    // Everyone is done: the group goes back to the campus after the medal screen.
    run(&mut sim, 11 * SECOND);
    assert_eq!(sim.minigame_travel(), Some("campus"));
}

#[test]
fn a_catch_costs_gold_and_points_and_items_are_never_granted_twice() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, &venue(Some("mgr_a"), (0.0, -8.0)), Vec::new());
    assert!(!caught(&hold(&mut sim, p, 0.0, -12.0, 4 * SECOND)).is_empty());
    let r = ended(&do_everything(&mut sim, p));
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].catches, 1);
    assert_eq!(r[0].medal, Some(Medal::Silver), "{:?}", r[0]);
    assert_eq!(r[0].score, 2400 - 250 - 3 * (r[0].time_ms / 1000));
    assert!(
        r[0].items
            .contains(&"Party Leftovers Container (Sealed)".to_string())
    );

    // A second outing: the name tag and the container are already in the bag.
    if let Err(e) = sim.load_zone("campus", &campus, Vec::new()) {
        panic!("{e}");
    }
    run(&mut sim, 1);
    enter(&mut sim, p, &venue(None, FAR), Vec::new());
    let again = ended(&do_everything(&mut sim, p));
    assert_eq!(again[0].medal, Some(Medal::Gold));
    assert_eq!(
        again[0].items,
        vec!["Party Hat (Was Never Seen)".to_string()]
    );
    let tags = sim.unit(p).map_or(0, |u| {
        u.bag
            .iter()
            .filter(|b| b.starts_with("Teambuilding Name Tag"))
            .count()
    });
    assert_eq!(tags, 1);
}

#[test]
fn leaving_early_or_running_out_of_time_earns_nothing() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, &venue(None, FAR), Vec::new());
    sim.apply_as(p, Command::LeaveMinigame);
    let r = ended(&run(&mut sim, 1));
    assert_eq!(r.len(), 1);
    assert!(r[0].quit && r[0].medal.is_none() && r[0].score == 0);
    assert_eq!(sim.minigame_best(p, "teambuilding"), None);

    let (mut sim, ids) = on_campus(1);
    enter(&mut sim, ids[0], &venue(None, FAR), Vec::new());
    let limit = content().minigames[0].limit_s;
    let r = ended(&run(&mut sim, limit * SECOND + SECOND));
    assert_eq!(r.len(), 1);
    assert!(r[0].quit && r[0].medal.is_none());
}

#[test]
fn co_op_members_sneak_on_their_own_and_the_group_score_adds_up() {
    let (mut sim, ids) = on_campus(2);
    let (a, b) = (ids[0], ids[1]);
    enter(&mut sim, a, &venue(Some("mgr_a"), (0.0, -8.0)), Vec::new());
    // B gets caught; A does not notice anything of it.
    let events = hold(&mut sim, b, 0.0, -12.0, 4 * SECOND);
    let caught_units: Vec<EntityId> = events
        .iter()
        .filter_map(|e| match e {
            Event::MinigameCaught { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect();
    assert_eq!(caught_units.first(), Some(&b));
    let first = ended(&do_everything(&mut sim, a));
    assert_eq!(first.len(), 1);
    // One run is still going: no travel yet.
    run(&mut sim, 8 * SECOND);
    assert_eq!(sim.minigame_travel(), None);
    let second = ended(&do_everything(&mut sim, b));
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].group_score, first[0].score + second[0].score);
    assert_eq!(sim.minigame_group_score(), first[0].score + second[0].score);
    assert!(first[0].medal > second[0].medal);
}

#[test]
fn the_best_medal_survives_a_save() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, &venue(None, FAR), Vec::new());
    hold(&mut sim, p, 5.0, 0.0, 2);
    do_everything(&mut sim, p);
    let save = sim.export_save(p).unwrap_or_else(|| panic!("no save"));
    let text = save.encode();
    assert!(text.contains("minigames=teambuilding*2"), "{text}");
    let back = SaveData::decode(&text).unwrap_or_else(|| panic!("decode"));
    let mut fresh = Sim::new(content(), 1);
    let q = fresh.spawn_player("fullstack", Vec3::default(), 0.0);
    assert!(fresh.import_save(q, &back));
    assert_eq!(fresh.minigame_best(q, "teambuilding"), Some(Medal::Gold));
}

#[test]
fn discarded_medal_items_do_not_come_back_and_repeat_relief_tapers() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, &venue(None, FAR), Vec::new());
    let first = ended(&do_everything(&mut sim, p));
    assert_eq!(first[0].medal, Some(Medal::Gold));
    // Throw every reward away, then take Gold again.
    let n = sim.unit(p).map_or(0, |u| u.bag.len());
    for _ in 0..n {
        sim.apply_as(p, Command::Discard { slot: 0 });
        run(&mut sim, 1);
    }
    let rewards = [
        "Teambuilding Name Tag (Hello, I Am Busy)",
        "Party Leftovers Container (Sealed)",
        "Party Hat (Was Never Seen)",
    ];
    let held = |sim: &Sim| {
        sim.unit(p).map_or(0, |u| {
            u.bag
                .iter()
                .filter(|b| rewards.contains(&b.as_str()))
                .count()
        })
    };
    assert_eq!(held(&sim), 0, "rewards should be gone before the rerun");
    sim.debug_set_burnout(p, 400);
    if let Err(e) = sim.load_zone("campus", &campus, Vec::new()) {
        panic!("{e}");
    }
    run(&mut sim, 1);
    enter(&mut sim, p, &venue(None, FAR), Vec::new());
    let again = ended(&do_everything(&mut sim, p));
    assert_eq!(again[0].medal, Some(Medal::Gold));
    assert!(again[0].items.is_empty(), "{:?}", again[0].items);
    assert_eq!(held(&sim), 0);
    let u = sim.unit(p).unwrap_or_else(|| panic!("player"));
    assert_eq!(u.career.burnout, 400 - 150 / 4);
}
