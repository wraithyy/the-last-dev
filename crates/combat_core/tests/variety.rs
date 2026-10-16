//! The variety pass: new quest objectives (Defend, KillClean), optional rares, the new units'
//! mechanics, Legacy twists and the night shift's marching deadline.

mod common;

use combat_core::{
    Command, Content, Effect, EntityId, Event, Objective, QuestStatus, Sim, SpellId,
    TelegraphShape, Vec3, ms_to_ticks,
};
use common::content;

const PACK_SPACING: f32 = 70.0;
const PROD: Vec3 = Vec3::new(100.0, 0.0, 100.0);

/// Packs down a corridor, numbered NPCs off to the side, the Prod Incident pair on its own,
/// everything else named far away; `rare_1` only when `rares` is set.
fn anchors_with(rares: bool) -> impl Fn(&str) -> Option<(Vec3, f32)> {
    move |name: &str| {
        let num = |p: &str| name.strip_prefix(p).and_then(|n| n.parse::<f32>().ok());
        if let Some(n) = num("pack_") {
            return Some((Vec3::new(0.0, 0.0, -PACK_SPACING * n), 0.0));
        }
        if let Some(n) = num("npc_") {
            return Some((Vec3::new(40.0, 0.0, -15.0 * n), 0.0));
        }
        match name {
            "npc_prod" => Some((PROD, 0.0)),
            "npc_oncall" => Some((Vec3::new(PROD.x + 4.0, 0.0, PROD.z), 0.0)),
            "boss" => Some((Vec3::new(0.0, 0.0, -PACK_SPACING * 7.0), 0.0)),
            "rare_1" if rares => Some((Vec3::new(-200.0, 0.0, 0.0), 0.0)),
            n if n.starts_with("npc_") => Some((Vec3::new(-400.0, 0.0, 400.0), 0.0)),
            _ => None,
        }
    }
}

fn world(zone: &str, level: u8, players: usize) -> (Sim, Vec<EntityId>) {
    world_with(zone, level, players, content())
}

fn world_with(zone: &str, level: u8, players: usize, c: Content) -> (Sim, Vec<EntityId>) {
    let mut sim = Sim::new(c, 11);
    sim.set_wandering(false);
    let ids: Vec<EntityId> = (0..players)
        .map(|k| sim.spawn_player("fullstack", Vec3::new(k as f32 * 2.0, 0.0, 20.0), 0.0))
        .collect();
    for &p in &ids {
        sim.set_level_of(p, level);
    }
    if let Err(e) = sim.load_zone(zone, &anchors_with(false), Vec::new()) {
        panic!("{zone}: {e}");
    }
    (sim, ids)
}

fn run(sim: &mut Sim, ticks: u64) -> Vec<Event> {
    (0..ticks).flat_map(|_| sim.tick()).collect()
}

fn secs(s: u64) -> u64 {
    ms_to_ticks(1000) * s
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
    run(sim, 1);
}

fn alive(sim: &Sim, key: &str) -> Vec<EntityId> {
    sim.units()
        .filter(|u| u.key == key && u.alive)
        .map(|u| u.id)
        .collect()
}

fn spawn(sim: &mut Sim, key: &str, pos: Vec3) -> EntityId {
    match sim.spawn_unit(key, pos, 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{key}: {e}"),
    }
}

/// Keeps the players topped up so long observations never end in a wipe.
fn heal_all(sim: &mut Sim, ids: &[EntityId]) {
    for &p in ids {
        sim.debug_set_hp(p, u32::MAX);
    }
}

/// Runs up to `max_s` seconds (healing every second) until `hit` matches an event.
fn watch(
    sim: &mut Sim,
    ids: &[EntityId],
    max_s: u64,
    hit: impl Fn(&Event) -> bool,
) -> Option<Vec<Event>> {
    for _ in 0..max_s {
        let ev = run(sim, secs(1));
        heal_all(sim, ids);
        if ev.iter().any(&hit) {
            return Some(ev);
        }
    }
    None
}

fn count(sim: &Sim, p: EntityId, quest: &str) -> u32 {
    sim.quests_of(p)
        .active
        .iter()
        .find(|q| q.id == quest)
        .and_then(|q| q.counts.first().copied())
        .unwrap_or(0)
}

fn pull(sim: &mut Sim, p: EntityId, target: EntityId) {
    sim.apply_as(
        p,
        Command::SetTarget {
            unit: p,
            target: Some(target),
        },
    );
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SpellId(4),
        },
    );
    run(sim, 2);
}

// ---- Defend ----

#[test]
fn defending_prod_counts_seconds_near_it_and_its_death_starts_over() {
    let (mut sim, ids) = world("campus", 6, 1);
    let p = ids[0];
    assert!(sim.debug_set_quest("pi_hold_the_line", QuestStatus::Active));
    let server = alive(&sim, "prod_server")[0];

    // Far away: no waves, no seconds.
    run(&mut sim, secs(3));
    assert!(alive(&sim, "pager_alert").is_empty());
    assert_eq!(count(&sim, p, "pi_hold_the_line"), 0);

    move_to(&mut sim, p, Vec3::new(PROD.x, 0.0, PROD.z + 6.0));
    run(&mut sim, secs(3));
    let waves = alive(&sim, "pager_alert");
    assert_eq!(waves.len(), 1, "one alert per 7 s");
    let w = sim.unit(waves[0]).map(|u| u.target);
    assert_eq!(
        w,
        Some(Some(server)),
        "the alert goes for prod, not for you"
    );
    assert!(count(&sim, p, "pi_hold_the_line") >= 2);

    // Prod goes down: everyone starts over, the alerts leave, prod is back on its feet.
    sim.debug_set_hp(server, 1);
    let ev = watch(
        &mut sim,
        &ids,
        15,
        |e| matches!(e, Event::Death { unit, .. } if *unit == server),
    );
    let mut ev = ev.unwrap_or_default();
    assert!(!ev.is_empty(), "the alert takes prod down");
    ev.extend(run(&mut sim, secs(1)));
    let reset = ev.iter().any(|e| {
        matches!(e, Event::QuestProgress { quest, count: 0, .. } if quest == "pi_hold_the_line")
    });
    assert!(reset, "the clock starts over");
    assert!(count(&sim, p, "pi_hold_the_line") <= 1);
    let fresh = alive(&sim, "prod_server");
    assert_eq!(fresh.len(), 1);
    assert_ne!(fresh[0], server);
    assert!(sim.unit(fresh[0]).is_some_and(|u| u.hp == u.max_hp));

    // A full minute with the alerts swatted: done.
    for _ in 0..70 {
        run(&mut sim, secs(1));
        for w in alive(&sim, "pager_alert") {
            sim.debug_kill(w);
        }
        heal_all(&mut sim, &ids);
    }
    assert_eq!(sim.quest_status("pi_hold_the_line"), QuestStatus::Ready);
    run(&mut sim, secs(10));
    assert!(
        alive(&sim, "pager_alert").is_empty(),
        "no waves once the objective is done"
    );
}

// ---- KillClean ----

#[test]
fn a_clean_kill_counts_and_a_telegraph_hit_voids_that_fight() {
    let (mut sim, ids) = world("campus", 5, 1);
    let p = ids[0];
    assert!(sim.debug_set_quest("ft_flaky_robots", QuestStatus::Active));
    let here = Vec3::new(300.0, 0.0, 300.0);
    move_to(&mut sim, p, here);

    let a = spawn(
        &mut sim,
        "delivery_robot",
        Vec3::new(here.x, 0.0, here.z - 4.0),
    );
    pull(&mut sim, p, a);
    assert!(sim.debug_kill(a));
    run(&mut sim, 1);
    assert_eq!(count(&sim, p, "ft_flaky_robots"), 1, "a clean kill counts");

    // Stand still until a Package Drop Zone lands on us, then finish it: no credit.
    let b = spawn(
        &mut sim,
        "delivery_robot",
        Vec3::new(here.x, 0.0, here.z - 4.0),
    );
    pull(&mut sim, p, b);
    let landed = watch(&mut sim, &ids, 40, |e| {
        matches!(e, Event::TelegraphResolved { landed: true, .. })
    });
    assert!(landed.is_some(), "the robot drops a package on us");
    if sim.unit(b).is_some_and(|u| u.alive) {
        assert!(sim.debug_kill(b));
    }
    run(&mut sim, 1);
    assert_eq!(
        count(&sim, p, "ft_flaky_robots"),
        1,
        "a dirty pass is a fail"
    );

    // The next fight starts clean again.
    let c = spawn(
        &mut sim,
        "delivery_robot",
        Vec3::new(here.x, 0.0, here.z - 4.0),
    );
    run(&mut sim, secs(1));
    pull(&mut sim, p, c);
    assert!(sim.debug_kill(c));
    run(&mut sim, 1);
    assert_eq!(count(&sim, p, "ft_flaky_robots"), 2);
}

#[test]
fn new_objectives_validate_and_name_their_zones() {
    let c = content();
    let defend = c
        .quest("pi_hold_the_line")
        .and_then(|q| q.objectives.first().cloned());
    assert!(matches!(defend, Some(Objective::Defend { secs: 60, .. })));
    let o = defend.unwrap_or(Objective::Talk(String::new()));
    assert_eq!(c.objective_zones(&o), vec!["campus"]);
    let clean = Objective::KillClean("oom_killer".into(), 1);
    assert_eq!(c.objective_zones(&clean), vec!["datacenter/2"]);
    // A wave that is not a hostile with AI, or a defended unit not standing in the giver's
    // zone, is rejected.
    let src = common::SRC;
    let broken = src.replace("wave: \"pager_alert\"", "wave: \"qa_lead\"");
    assert!(Content::from_ron(&broken).is_err());
    let broken = src.replace(
        "unit: \"prod_server\", wave",
        "unit: \"noc_operator\", wave",
    );
    assert!(Content::from_ron(&broken).is_err());
}

// ---- Rares ----

#[test]
fn rares_spawn_only_where_the_layout_has_them_and_stay_dead() {
    let (mut sim, _) = world("tower/1", 2, 1);
    assert!(
        alive(&sim, "jira_automation").is_empty(),
        "no anchor, no rare"
    );
    if let Err(e) = sim.load_zone("tower/1", &anchors_with(true), Vec::new()) {
        panic!("{e}");
    }
    let r = alive(&sim, "jira_automation");
    assert_eq!(r.len(), 1);
    assert!(sim.debug_kill(r[0]));
    run(&mut sim, 1);
    assert!(!sim.zone_cleared("tower/1"), "a rare is not the floor boss");
    for z in ["campus", "tower/1"] {
        if let Err(e) = sim.load_zone(z, &anchors_with(true), Vec::new()) {
            panic!("{e}");
        }
    }
    assert!(
        alive(&sim, "jira_automation").is_empty(),
        "killed rares stay dead like trash"
    );
    assert!(
        content()
            .unit_def("backup_job")
            .is_some_and(|u| u.elite && !u.loot.is_empty()),
        "the Hall B rare is an elite with its own drop"
    );
}

// ---- New units ----

#[test]
fn the_oom_killer_marks_every_player_near_it() {
    let (mut sim, ids) = world("datacenter/2", 8, 2);
    let here = Vec3::new(300.0, 0.0, 300.0);
    move_to(&mut sim, ids[0], here);
    move_to(&mut sim, ids[1], Vec3::new(here.x + 8.0, 0.0, here.z));
    let oom = spawn(
        &mut sim,
        "oom_killer",
        Vec3::new(here.x + 4.0, 0.0, here.z - 3.0),
    );
    pull(&mut sim, ids[0], oom);
    let ev = watch(&mut sim, &ids, 20, |e| {
        matches!(
            e,
            Event::TelegraphStarted {
                spell: SpellId(189),
                ..
            }
        )
    });
    let ev = ev.unwrap_or_default();
    let marks: Vec<Vec3> = ev
        .iter()
        .filter_map(|e| match e {
            Event::TelegraphStarted {
                spell: SpellId(189),
                pos,
                ..
            } => Some(*pos),
            _ => None,
        })
        .collect();
    assert_eq!(marks.len(), 2, "one SIGKILL mark per player");
    assert!(marks.iter().any(|m| combat_core::distance(*m, here) < 0.5));
}

#[test]
fn the_closed_model_and_raid_door_staff_have_shields() {
    let c = content();
    for k in ["closed_model", "lanyard_enforcer", "vc_booth_staff"] {
        assert!(
            c.unit_def(k).is_some_and(|u| u.frontal_shield.is_some()),
            "{k}"
        );
    }
}

#[test]
fn tower_trash_kicks_and_telegraphs() {
    let c = content();
    let spells = |k: &str| -> Vec<SpellId> {
        c.unit_def(k)
            .and_then(|u| u.ai.as_ref())
            .map(|a| a.abilities.iter().map(|x| x.spell).collect())
            .unwrap_or_default()
    };
    let has = |k: &str, f: &dyn Fn(&combat_core::SpellDef) -> bool| {
        spells(k).iter().any(|s| c.spell(*s).is_some_and(f))
    };
    assert!(has("project_manager", &|s| s.telegraph.is_some()));
    assert!(has("consultant", &|s| s.kick));
    assert!(has("ai_ethicist", &|s| s.telegraph.is_some()));

    // The Ethicist actually throws the trolley now.
    let (mut sim, ids) = world("tower/5", 10, 1);
    let here = Vec3::new(300.0, 0.0, 300.0);
    move_to(&mut sim, ids[0], here);
    let e = spawn(
        &mut sim,
        "ai_ethicist",
        Vec3::new(here.x, 0.0, here.z - 6.0),
    );
    pull(&mut sim, ids[0], e);
    let ev = watch(&mut sim, &ids, 15, |e| {
        matches!(
            e,
            Event::TelegraphStarted {
                spell: SpellId(617),
                ..
            }
        )
    });
    assert!(ev.is_some(), "Trolley Problem");
}

/// One readable mechanic per raid trash type, and no two types of a room share one.
#[test]
fn raid_trash_rooms_never_repeat_a_mechanic() {
    let c = content();
    let mechanic = |key: &str| -> Option<&'static str> {
        let u = c.unit_def(key)?;
        if u.frontal_shield.is_some() {
            return Some("shield");
        }
        let first = u.ai.as_ref()?.abilities.first()?;
        let s = c.spell(first.spell)?;
        if matches!(s.effect, Effect::Summon { .. }) {
            return Some("add");
        }
        if s.kick {
            return Some("kick");
        }
        if let Some(t) = &s.telegraph {
            return Some(match t.shape {
                TelegraphShape::Circle { .. } => "circle",
                TelegraphShape::Cone { .. } => "cone",
                TelegraphShape::Line { .. } => "line",
            });
        }
        let dispellable = |aura: &str| {
            c.aura(aura)
                .is_some_and(|a| a.harmful && a.dispel == combat_core::DispelType::Magic)
        };
        let dispel = match &s.effect {
            Effect::Aura { aura, .. } => dispellable(aura),
            Effect::Damage {
                apply_aura: Some(a),
                ..
            } => dispellable(&a.aura),
            _ => false,
        };
        dispel.then_some("dispel")
    };
    let rooms = c
        .zones
        .iter()
        .filter(|z| z.id.starts_with("offsite/") || z.id.starts_with("vibecon/"));
    for z in rooms {
        let mut kinds: Vec<&str> = z
            .packs
            .iter()
            .flat_map(|p| p.units.iter())
            .map(String::as_str)
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        let mut seen: Vec<(&str, &str)> = Vec::new();
        for k in kinds {
            let Some(m) = mechanic(k) else {
                panic!("{}: {k} has no readable mechanic", z.id);
            };
            if let Some((other, _)) = seen.iter().find(|(_, o)| *o == m) {
                panic!("{}: {k} and {other} both use {m}", z.id);
            }
            seen.push((k, m));
        }
    }
}

#[test]
fn raid_adds_and_debuffs_fire() {
    let (mut sim, ids) = world("offsite/1", 10, 1);
    let here = Vec3::new(300.0, 0.0, 300.0);
    move_to(&mut sim, ids[0], here);
    let av = spawn(
        &mut sim,
        "av_technician",
        Vec3::new(here.x, 0.0, here.z - 4.0),
    );
    pull(&mut sim, ids[0], av);
    let ev = watch(
        &mut sim,
        &ids,
        10,
        |e| matches!(e, Event::Summoned { summoner, .. } if *summoner == av),
    );
    assert!(ev.is_some(), "the AV technician wheels in a floor monitor");
    assert_eq!(alive(&sim, "av_floor_monitor").len(), 1);
    let ev = watch(&mut sim, &ids, 10, |e| {
        matches!(
            e,
            Event::TelegraphStarted {
                spell: SpellId(1117),
                ..
            }
        )
    });
    assert!(ev.is_some(), "the monitor shrieks down the aisle");

    let (mut sim, ids) = world("offsite/1", 10, 1);
    move_to(&mut sim, ids[0], here);
    let bar = spawn(
        &mut sim,
        "open_bar_rep",
        Vec3::new(here.x, 0.0, here.z - 4.0),
    );
    pull(&mut sim, ids[0], bar);
    let p = ids[0];
    let ev = watch(
        &mut sim,
        &ids,
        10,
        |e| matches!(e, Event::AuraApplied { target, key } if *target == p && key == "open_tab"),
    );
    assert!(ev.is_some(), "the open bar puts a round on your tab");

    let (mut sim, ids) = world("vibecon/1", 10, 1);
    move_to(&mut sim, ids[0], here);
    let gob = spawn(
        &mut sim,
        "vc_swag_goblin",
        Vec3::new(here.x, 0.0, here.z - 4.0),
    );
    pull(&mut sim, ids[0], gob);
    let ev = watch(
        &mut sim,
        &ids,
        10,
        |e| matches!(e, Event::Summoned { summoner, .. } if *summoner == gob),
    );
    assert!(ev.is_some(), "a tote bag joins the fight");
}

// ---- The night shift ----

#[test]
fn the_hard_deadline_marches_towards_you() {
    let (mut sim, ids) = world("campus", 6, 1);
    let here = Vec3::new(300.0, 0.0, 300.0);
    move_to(&mut sim, ids[0], here);
    // Inside its aggro radius, so it pulls itself.
    let d = spawn(
        &mut sim,
        "hard_deadline",
        Vec3::new(here.x, 0.0, here.z - 8.0),
    );
    run(&mut sim, 2);
    let ev = watch(&mut sim, &ids, 15, |e| {
        matches!(
            e,
            Event::TelegraphStarted {
                spell: SpellId(711),
                ..
            }
        )
    })
    .unwrap_or_default();
    let steps: Vec<(Vec3, u32)> = ev
        .iter()
        .filter_map(|e| match e {
            Event::TelegraphStarted {
                spell: SpellId(711),
                pos,
                resolve_ms,
                ..
            } => Some((*pos, *resolve_ms)),
            _ => None,
        })
        .collect();
    assert_eq!(steps.len(), 6, "six steps of the due date");
    let from = sim.unit(d).map_or(here, |u| u.pos);
    for (k, w) in steps.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        assert!(
            combat_core::distance(b.0, from) > combat_core::distance(a.0, from) + 2.0,
            "step {k} lands further out"
        );
        assert!(b.1 > a.1, "and later");
    }
    // One of the steps covers where the player stood at release.
    let me = sim.unit(ids[0]).map_or(here, |u| u.pos);
    assert!(steps.iter().any(|s| combat_core::distance(s.0, me) < 2.4));
}

#[test]
fn the_night_shift_has_a_new_trick_each() {
    let (mut sim, ids) = world("campus", 5, 1);
    let here = Vec3::new(300.0, 0.0, 300.0);
    move_to(&mut sim, ids[0], here);
    let w = spawn(
        &mut sim,
        "deadline_wraith",
        Vec3::new(here.x, 0.0, here.z - 4.0),
    );
    pull(&mut sim, ids[0], w);
    let ev = watch(&mut sim, &ids, 15, |e| {
        matches!(
            e,
            Event::TelegraphStarted {
                spell: SpellId(709),
                ..
            }
        )
    });
    assert!(ev.is_some(), "Release Window Closing");

    let (mut sim, ids) = world("campus", 5, 1);
    move_to(&mut sim, ids[0], here);
    let c = spawn(
        &mut sim,
        "sleep_deprived_contractor",
        Vec3::new(here.x, 0.0, here.z - 4.0),
    );
    pull(&mut sim, ids[0], c);
    let ev = watch(&mut sim, &ids, 15, |e| {
        matches!(
            e,
            Event::CastStarted {
                spell: SpellId(710),
                ..
            }
        )
    });
    assert!(ev.is_some(), "3 a.m. Hotfix, a must-interrupt");
    assert!(content().spell(SpellId(710)).is_some_and(|s| s.kick));
    assert!(
        content()
            .live
            .night
            .spawns
            .iter()
            .any(|s| s.unit == "hard_deadline"),
        "the mini-elite works nights"
    );
}

// ---- Legacy twists ----

fn legacy(zone: &str) -> (Sim, Vec<EntityId>) {
    let (mut sim, ids) = world("campus", 10, 1);
    sim.apply_as(ids[0], Command::SetLegacy { on: true });
    run(&mut sim, 1);
    if let Err(e) = sim.load_zone(zone, &anchors_with(false), Vec::new()) {
        panic!("{e}");
    }
    assert_eq!(sim.legacy_zone(), Some(zone));
    (sim, ids)
}

fn casts(ev: &[Event], spell: u32) -> bool {
    ev.iter().any(|e| {
        matches!(e, Event::CastStarted { spell: s, .. } | Event::TelegraphStarted { spell: s, .. } if s.0 == spell)
    })
}

#[test]
fn every_legacy_floor_has_its_own_twist() {
    let c = content();
    let mut spells = Vec::new();
    for z in &c.legacy.zones {
        let twist = z.twist.as_ref();
        let spell = match twist {
            Some(combat_core::LegacyTwist::TrashAbility(a))
            | Some(combat_core::LegacyTwist::BossAbility(a)) => a.spell,
            Some(combat_core::LegacyTwist::BossSwap(_, to)) => *to,
            None => panic!("{} has no twist", z.zone),
        };
        assert!(!spells.contains(&spell), "{} repeats a twist", z.zone);
        spells.push(spell);
    }
}

#[test]
fn legacy_trash_gains_the_floor_ability() {
    let (mut sim, ids) = legacy("tower/1");
    move_to(&mut sim, ids[0], Vec3::new(0.0, 0.0, -PACK_SPACING + 6.0));
    let ev = watch(&mut sim, &ids, 20, |e| casts(std::slice::from_ref(e), 641));
    assert!(ev.is_some(), "Merge Conflict from the trash");
    // Normal floors keep their old habits.
    let (mut sim, ids) = world("tower/1", 10, 1);
    move_to(&mut sim, ids[0], Vec3::new(0.0, 0.0, -PACK_SPACING + 6.0));
    let ev = watch(&mut sim, &ids, 20, |e| casts(std::slice::from_ref(e), 641));
    assert!(ev.is_none(), "no twist on Normal");
}

#[test]
fn legacy_bosses_swap_or_gain_an_ability() {
    let boss = Vec3::new(0.0, 0.0, -PACK_SPACING * 7.0 + 6.0);
    let (mut sim, ids) = legacy("tower/2");
    move_to(&mut sim, ids[0], boss);
    let mut all = Vec::new();
    for _ in 0..30 {
        let ev = run(&mut sim, secs(1));
        heal_all(&mut sim, &ids);
        all.extend(ev);
    }
    assert!(
        casts(&all, 642),
        "the analysis is the 190-page legacy edition"
    );
    assert!(!casts(&all, 119), "and the old one is gone");

    let (mut sim, ids) = legacy("tower/4");
    move_to(&mut sim, ids[0], boss);
    let ev = watch(&mut sim, &ids, 25, |e| casts(std::slice::from_ref(e), 644));
    assert!(ev.is_some(), "the VP announces a Matrix Reorg");
}
