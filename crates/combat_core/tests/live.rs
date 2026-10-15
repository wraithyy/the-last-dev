//! The live world: daily/weekly tasks (rotation, scaling, progress, payout, persistence,
//! resets), the campus world boss (warning, spawn, cancel, kill), the office clock and
//! Crunch-mode nights (`live.rs`, `dailies.rs`).

mod common;

use combat_core::live::{DUSK_MIN, GAME_DAY_S, day_index, week_index};
use combat_core::{
    Command, Content, EntityId, Event, GoalSpec, SaveData, Sim, TaskState, Vec3, WorldBossState,
};
use common::content;

/// 2026-10-15 00:00 UTC, a Thursday.
const THURSDAY: u64 = 1_792_022_400;
const DAY: u64 = 86_400;
const SECOND: u32 = 30;

/// Packs and the boss far away, the board, the world boss and the night shift nearby.
fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") || n == "boss" => at(400.0, 400.0),
        "scrum_bot" => at(2.0, 0.0),
        "prop:scrum_board" => at(-2.0, 0.0),
        "world_boss" => at(0.0, -60.0),
        n if n.starts_with("night_") => at(60.0, 0.0),
        n if n.starts_with("npc_") => at(-30.0, 30.0),
        "graveyard" => at(0.0, 10.0),
        _ => None,
    }
}

fn world(level: u8) -> (Sim, EntityId) {
    let mut c = content();
    for s in &mut c.spells {
        s.yells.clear();
    }
    let mut sim = Sim::new(c, 11);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_player_level(level);
    if let Err(e) = sim.load_zone("campus", &anchors, Vec::new()) {
        panic!("{e}");
    }
    (sim, p)
}

fn run(sim: &mut Sim, ticks: u32) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        out.extend(sim.tick());
    }
    out
}

fn board(events: &[Event]) -> Option<combat_core::BoardView> {
    events.iter().rev().find_map(|e| match e {
        Event::DailyBoard { board, .. } => Some(board.clone()),
        _ => None,
    })
}

#[test]
fn the_rotation_is_fixed_per_date_and_varied_across_dates() {
    let c = content();
    assert!(
        c.live.dailies.len() >= 20,
        "{} dailies",
        c.live.dailies.len()
    );
    let d = day_index(THURSDAY);
    let today = c.daily_rotation(d);
    assert_eq!(today, c.daily_rotation(d));
    assert_eq!(today.len(), 3);
    let mut sorted = today.clone();
    sorted.dedup();
    assert_eq!(sorted.len(), 3, "{today:?}");
    let mut seen = std::collections::BTreeSet::new();
    let mut changes = 0;
    for k in 0..60 {
        let r = c.daily_rotation(d + k);
        if r != c.daily_rotation(d + k + 1) {
            changes += 1;
        }
        seen.extend(r);
    }
    assert!(changes >= 55, "the board barely changes: {changes}/60");
    assert_eq!(
        seen.len(),
        c.live.dailies.len(),
        "some dailies never show up"
    );
    // One Sprint Goal per ISO week, the same all week.
    let w = week_index(THURSDAY);
    for k in 0..4 {
        assert_eq!(
            c.weekly_rotation(week_index(THURSDAY + k * DAY)),
            c.weekly_rotation(w),
            "Thursday to Sunday share a week"
        );
    }
    assert_eq!(
        week_index(THURSDAY + 4 * DAY),
        w + 1,
        "Monday starts a week"
    );
}

#[test]
fn goals_and_rewards_scale_with_the_level() {
    let (mut low, p) = world(1);
    low.set_clock(THURSDAY);
    let (mut high, q) = world(9);
    high.set_clock(THURSDAY);
    let (a, b) = (low.task_board(p), high.task_board(q));
    assert_eq!(a.tasks.len(), 4, "three dailies and a weekly");
    let keys =
        |v: &combat_core::BoardView| v.tasks.iter().map(|t| t.key.clone()).collect::<Vec<_>>();
    assert_eq!(
        keys(&a),
        keys(&b),
        "the date decides the tasks, not the player"
    );
    for (x, y) in a.tasks.iter().zip(&b.tasks) {
        assert!(
            y.xp > x.xp && y.sp > x.sp,
            "{}: {} -> {} XP",
            x.key,
            x.xp,
            y.xp
        );
    }
    assert!(a.tasks.iter().all(|t| t.state == TaskState::Open));
    // Kill goals go where the level fits among the open zones: the campus at 1; at 9 in a
    // fresh session (only the campus and Open Space open) nothing fits, so the board's zone.
    assert_eq!(low.work_zone(1), "campus");
    assert_eq!(high.work_zone(9), "campus");
    // Totals scale too: throughput is per level.
    let need = |sim: &Sim, who| {
        sim.content()
            .live
            .dailies
            .iter()
            .position(|d| d.key == "velocity_metric")
            .and_then(|_| {
                sim.task_board(who)
                    .tasks
                    .iter()
                    .flat_map(|t| t.goals.iter())
                    .find(|g| g.goal == GoalSpec::Output)
                    .map(|g| g.need)
            })
    };
    if let (Some(a), Some(b)) = (need(&low, p), need(&high, q)) {
        assert!(b > a * 5, "{a} -> {b}");
    }
}

#[test]
fn work_zones_stay_inside_the_level_bands() {
    // With everything opened, the zone picked for a level always has that level in its band
    // (or one above its top), so dailies never send anyone far over or under their level.
    let (mut sim, p) = world(1);
    let cleared = [
        "tower/1",
        "tower/2",
        "tower/3",
        "tower/4",
        "tower/5",
        "datacenter/1",
    ];
    let save = SaveData {
        class: "fullstack".into(),
        level: 1,
        cleared: cleared.iter().map(|z| z.to_string()).collect(),
        bosses: vec!["ceo".into()],
        ..SaveData::default()
    };
    assert!(sim.import_save(p, &save));
    let c = content();
    for level in 1..=c.max_level() {
        let z = sim.work_zone(level);
        let Some(def) = c.zone(&z) else {
            panic!("{level}: unknown zone {z}");
        };
        let (lo, hi) = def.level_range;
        assert!(
            lo <= level && level <= hi + 1,
            "level {level} works in {z} ({lo}-{hi})"
        );
    }
}

#[test]
fn joining_progress_and_payout() {
    let (mut sim, p) = world(4);
    sim.set_clock(THURSDAY + 12 * 3600);
    // Away from the board: nothing happens.
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos: Vec3::new(30.0, 0.0, 30.0),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    sim.apply_as(p, Command::JoinStandup);
    let ev = run(&mut sim, SECOND);
    assert!(board(&ev).is_none());
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos: Vec3::new(0.0, 0.0, 0.0),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    sim.apply_as(p, Command::JoinStandup);
    let ev = run(&mut sim, 2);
    let Some(b) = board(&ev) else {
        panic!("no board after joining: {ev:?}");
    };
    assert!(b.tasks.iter().all(|t| t.state == TaskState::Active));
    let daily = b.tasks[0].clone();
    let sp_before = sim.unit(p).map_or(0, |u| u.sp);
    let counts: Vec<u32> = daily.goals.iter().map(|g| g.need).collect();
    assert!(sim.debug_task_counts(p, &daily.key, &counts));
    let ev = run(&mut sim, SECOND);
    assert!(
        ev.iter().any(|e| matches!(e, Event::TaskCompleted { key, weekly: false, xp, .. } if *key == daily.key && *xp == daily.xp)),
        "{ev:?}"
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::XpGained { amount, .. } if *amount == daily.xp))
    );
    assert_eq!(sim.unit(p).map_or(0, |u| u.sp), sp_before + daily.sp);
    let after = sim.task_board(p);
    assert_eq!(after.tasks[0].state, TaskState::Done);
    // Done stays done: joining again changes nothing, nothing pays twice.
    sim.apply_as(p, Command::JoinStandup);
    let ev = run(&mut sim, SECOND);
    assert!(!ev.iter().any(|e| matches!(e, Event::TaskCompleted { .. })));
}

#[test]
fn kills_talks_and_idling_count() {
    let (mut sim, p) = world(2);
    sim.set_clock(THURSDAY);
    assert!(sim.debug_join_standup(p));
    let has = |sim: &Sim, f: &dyn Fn(&GoalSpec) -> bool| {
        sim.task_board(p)
            .tasks
            .iter()
            .flat_map(|t| t.goals.iter())
            .filter(|g| f(&g.goal))
            .map(|g| g.count)
            .sum::<u32>()
    };
    // Whatever today's board holds, a campus kill moves zone kill goals and Talk goals move on
    // talking. Use the weekly (always several goals) to cover kills here.
    let wild = sim.spawn_unit("delivery_robot", Vec3::new(3.0, 0.0, 0.0), 0.0);
    let Ok(wild) = wild else { panic!("spawn") };
    let before = has(&sim, &|g| matches!(g, GoalSpec::Kill { .. }));
    sim.debug_kill(wild);
    run(&mut sim, 2);
    let after = has(&sim, &|g| matches!(g, GoalSpec::Kill { .. }));
    let kill_goals = sim
        .task_board(p)
        .tasks
        .iter()
        .flat_map(|t| t.goals.iter())
        .filter(|g| matches!(&g.goal, GoalSpec::Kill { zone: Some(z), unit } if z == "campus" && unit.as_deref().is_none_or(|u| u == "delivery_robot")))
        .count() as u32;
    assert_eq!(
        after - before,
        kill_goals,
        "one credit per matching kill goal"
    );
    // Idle: standing still out of combat in the right zone counts up; moving resets it.
    let idle_here = sim.task_board(p).tasks.iter().any(|t| {
        t.goals
            .iter()
            .any(|g| matches!(&g.goal, GoalSpec::Idle { zone } if zone == "campus"))
    });
    if idle_here {
        run(&mut sim, 5 * SECOND);
        assert!(has(&sim, &|g| matches!(g, GoalSpec::Idle { .. })) >= 3);
    }
}

#[test]
fn a_finished_weekly_is_claimed_at_the_board_with_gear() {
    let (mut sim, p) = world(6);
    sim.set_clock(THURSDAY);
    assert!(sim.debug_join_standup(p));
    let weekly = sim
        .task_board(p)
        .tasks
        .into_iter()
        .find(|t| t.weekly)
        .unwrap_or_else(|| panic!("no weekly"));
    let full: Vec<u32> = weekly.goals.iter().map(|g| g.need).collect();
    assert!(sim.debug_task_counts(p, &weekly.key, &full));
    let ev = run(&mut sim, SECOND);
    assert!(
        !ev.iter().any(|e| matches!(e, Event::TaskCompleted { .. })),
        "weeklies wait for the claim"
    );
    let ready = sim.task_board(p).tasks.into_iter().find(|t| t.weekly);
    assert_eq!(ready.map(|t| t.state), Some(TaskState::Ready));
    let role = sim.class_of(p).map(|c| c.role);
    let gear = sim.content().sprint_goal_gear(6, role);
    assert!(gear.len() >= 2, "{gear:?}");
    let bag = sim.unit(p).map_or(0, |u| u.bag.len());
    sim.apply_as(p, Command::ClaimSprintGoal { choice: 1 });
    let ev = run(&mut sim, 2);
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::TaskCompleted { weekly: true, .. }))
    );
    let u = sim.unit(p).unwrap_or_else(|| panic!("player"));
    assert_eq!(u.bag.len(), bag + 1);
    assert_eq!(u.bag.last(), gear.get(1));
    if sim
        .content()
        .task_def(&weekly.key, true)
        .is_some_and(|d| d.title)
    {
        assert!(!u.achievements.is_empty(), "a titled goal grants its title");
    }
}

#[test]
fn tasks_persist_in_the_save_and_reset_with_the_date() {
    let (mut sim, p) = world(5);
    sim.set_clock(THURSDAY + 3600);
    assert!(sim.debug_join_standup(p));
    let b = sim.task_board(p);
    let d0 = b.tasks[0].clone();
    let partial: Vec<u32> = d0.goals.iter().map(|g| g.need / 2).collect();
    assert!(sim.debug_task_counts(p, &d0.key, &partial));
    run(&mut sim, SECOND);
    let Some(save) = sim.export_save(p) else {
        panic!("no save")
    };
    assert_eq!(save.tasks.len(), 4);
    let text = save.encode();
    let Some(back) = SaveData::decode(&text) else {
        panic!("save does not decode:\n{text}")
    };
    assert_eq!(back.tasks, save.tasks);
    assert_eq!(
        (back.tasks_day, back.tasks_week),
        (save.tasks_day, save.tasks_week)
    );

    // Same day, new session: progress is back.
    let (mut again, q) = world(5);
    again.set_clock(THURSDAY + 7200);
    assert!(again.import_save(q, &back));
    let restored = again.task_board(q);
    assert_eq!(restored.tasks[0].goals, sim.task_board(p).tasks[0].goals);
    assert_eq!(restored.tasks[0].state, TaskState::Active);

    // Next day: dailies are gone, the weekly stays. Next Monday: the weekly goes too.
    let (mut friday, f) = world(5);
    friday.set_clock(THURSDAY + DAY + 60);
    assert!(friday.import_save(f, &back));
    let ev = run(&mut friday, 2 * SECOND);
    let b = board(&ev).unwrap_or_else(|| panic!("rollover sends the board"));
    let joined: Vec<_> = b
        .tasks
        .iter()
        .filter(|t| t.state != TaskState::Open)
        .collect();
    assert_eq!(joined.len(), 1);
    assert!(joined[0].weekly);
    let (mut monday, m) = world(5);
    monday.set_clock(THURSDAY + 4 * DAY + 60);
    assert!(monday.import_save(m, &back));
    run(&mut monday, 2 * SECOND);
    assert!(
        monday
            .task_board(m)
            .tasks
            .iter()
            .all(|t| t.state == TaskState::Open)
    );
    // The live clock also rolls over in a running session.
    let (mut live, l) = world(5);
    live.set_clock(THURSDAY + DAY - 3);
    assert!(live.debug_join_standup(l));
    run(&mut live, 5 * SECOND);
    let b = live.task_board(l);
    assert_eq!(b.day, day_index(THURSDAY + DAY));
    assert_eq!(
        b.tasks
            .iter()
            .filter(|t| !t.weekly && t.state != TaskState::Open)
            .count(),
        0
    );
}

#[test]
fn the_world_boss_warns_spawns_and_cancels_on_its_timers() {
    let (mut sim, p) = world(10);
    let _ = p;
    let def = sim
        .content()
        .world_boss()
        .cloned()
        .unwrap_or_else(|| panic!("no world boss"));
    sim.set_clock(THURSDAY + 12 * 3600);
    // No warning before every - warn.
    let early = (def.every_ms - def.warn_ms) / 1000 - 1;
    let ev = run(&mut sim, early * SECOND);
    assert!(!ev.iter().any(|e| matches!(e, Event::WorldBoss { .. })));
    let ev = run(&mut sim, 2 * SECOND);
    assert!(
        ev.iter().any(|e| matches!(e, Event::WorldBoss { state: WorldBossState::Warning, secs, .. } if *secs == def.warn_ms / 1000)),
        "{:?}",
        ev.iter().filter(|e| matches!(e, Event::WorldBoss { .. })).collect::<Vec<_>>()
    );
    assert_eq!(sim.world_boss_in(), Some(def.warn_ms / 1000 - 1));
    // Leaving the campus pauses the countdown.
    let anchors_any = |n: &str| anchors(n).or(Some((Vec3::new(500.0, 0.0, 500.0), 0.0)));
    if let Err(e) = sim.load_zone("tower/1", &anchors_any, Vec::new()) {
        panic!("{e}");
    }
    run(&mut sim, 20 * SECOND);
    assert_eq!(sim.world_boss_in(), Some(def.warn_ms / 1000 - 1));
    if let Err(e) = sim.load_zone("campus", &anchors, Vec::new()) {
        panic!("{e}");
    }
    let ev = run(&mut sim, def.warn_ms / 1000 * SECOND);
    let spawned = ev.iter().find_map(|e| match e {
        Event::WorldBoss {
            state: WorldBossState::Spawned,
            unit,
            ..
        } => Some(*unit),
        _ => None,
    });
    let Some(boss) = spawned else {
        panic!("no spawn")
    };
    assert_eq!(sim.world_boss_unit(), Some(boss));
    assert_eq!(sim.unit(boss).map(|u| u.key.as_str()), Some("the_board"));
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Yell { unit, .. } if *unit == boss))
    );
    // Nobody engages: the meeting is cancelled after idle_ms.
    let ev = run(&mut sim, (def.idle_ms / 1000 + 2) * SECOND);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::WorldBoss {
            state: WorldBossState::Cancelled,
            ..
        }
    )));
    assert!(ev.iter().any(
        |e| matches!(e, Event::Yell { text, .. } if text.contains("could have been an email"))
    ));
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Despawned { unit } if *unit == boss))
    );
    assert!(sim.unit(boss).is_none());
    let left = sim.world_boss_in().unwrap_or(0);
    assert!(
        left + 5 >= def.every_ms / 1000,
        "countdown restarted: {left}"
    );
}

#[test]
fn the_world_boss_is_off_without_a_clock_and_its_kill_counts() {
    let (mut sim, p) = world(10);
    assert!(sim.debug_world_boss_in(3));
    let ev = run(&mut sim, 10 * SECOND);
    assert!(sim.world_boss_unit().is_none(), "no clock, no world events");
    assert!(!ev.iter().any(|e| matches!(
        e,
        Event::WorldBoss {
            state: WorldBossState::Spawned,
            ..
        }
    )));
    sim.set_clock(THURSDAY);
    assert!(sim.debug_join_standup(p));
    assert!(sim.debug_world_boss_in(3));
    run(&mut sim, 4 * SECOND);
    let Some(boss) = sim.world_boss_unit() else {
        panic!("no boss")
    };
    // Group scaling: a lone player meets a smaller board than a group would.
    let solo_hp = sim.unit(boss).map_or(0, |u| u.max_hp);
    assert!(solo_hp >= 5000, "{solo_hp}");
    let q = sim.spawn_player("devops", Vec3::new(1.0, 0.0, 0.0), 0.0);
    sim.spawn_player("backend", Vec3::new(2.0, 0.0, 0.0), 0.0);
    let group_hp = sim.unit(boss).map_or(0, |u| u.max_hp);
    assert!(group_hp > solo_hp * 17 / 10, "{solo_hp} -> {group_hp}");
    sim.debug_kill(boss);
    let ev = run(&mut sim, 2);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::WorldBoss {
            state: WorldBossState::Defeated,
            ..
        }
    )));
    for who in [p, q] {
        assert!(
            sim.unit(who)
                .is_some_and(|u| u.achievements.iter().any(|a| a == "board_meeting")),
            "{who} has no achievement"
        );
    }
    // A killer-less debug kill shares no quest credit; the countdown restarted.
    assert_eq!(
        sim.world_boss_in(),
        Some(sim.content().world_boss().map_or(0, |b| b.every_ms / 1000))
    );
    // Its loot table: legendary named pieces.
    let c: &Content = sim.content();
    let named = c
        .economy
        .gear
        .iter()
        .filter(|g| matches!(&g.source, combat_core::GearSource::Boss(k) if k == "the_board"))
        .count();
    assert!(named >= 2);
}

#[test]
fn nights_spawn_the_night_shift_and_dawn_sends_it_home() {
    let (mut sim, p) = world(4);
    // Two game minutes before 18:00.
    let day0 = THURSDAY - THURSDAY % GAME_DAY_S;
    sim.set_clock(day0 + u64::from(DUSK_MIN) - 2);
    assert!(!sim.is_night());
    let night_units = |sim: &Sim| {
        sim.units()
            .filter(|u| u.alive && sim.content().is_night_unit(&u.key))
            .count()
    };
    run(&mut sim, SECOND);
    assert_eq!(night_units(&sim), 0);
    run(&mut sim, 2 * SECOND);
    assert!(sim.is_night());
    assert_eq!(night_units(&sim), sim.content().live.night.spawns.len());
    // Spawned at the group's level, for XP worth the trip.
    let lvl = sim
        .units()
        .find(|u| sim.content().is_night_unit(&u.key))
        .map(|u| u.level);
    assert_eq!(lvl, Some(4));
    // 12 game hours later it is dawn and the shift clocks out.
    run(&mut sim, (12 * 60 + 2) * SECOND);
    assert!(!sim.is_night());
    assert_eq!(night_units(&sim), 0);
    let _ = p;
}

#[test]
fn going_home_at_six_sharp_is_an_achievement() {
    let day0 = THURSDAY - THURSDAY % GAME_DAY_S;
    for (minute, earned) in [
        (DUSK_MIN - 1, false),
        (DUSK_MIN + 2, true),
        (DUSK_MIN + 30, false),
    ] {
        let (mut sim, p) = world(3);
        sim.set_clock(day0 + u64::from(minute));
        sim.apply_as(p, Command::GoHome);
        run(&mut sim, 2);
        let has = sim
            .unit(p)
            .is_some_and(|u| u.achievements.iter().any(|a| a == "go_home"));
        assert_eq!(has, earned, "minute {minute}");
    }
}

#[test]
fn the_clock_reaches_joiners() {
    let (mut sim, _) = world(1);
    sim.set_clock(THURSDAY);
    run(&mut sim, 10 * SECOND);
    let q = sim.spawn_player("devops", Vec3::new(1.0, 0.0, 0.0), 0.0);
    let ev = sim.tick_routed();
    let to_q = ev.iter().find_map(|(a, e)| match (a, e) {
        (combat_core::Audience::Player(x), Event::Clock { unix_s }) if *x == q => Some(*unix_s),
        _ => None,
    });
    assert_eq!(to_q, Some(THURSDAY + 10));
    assert_eq!(sim.unix_s(), THURSDAY + 10);
}
