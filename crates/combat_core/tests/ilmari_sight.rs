//! Sight cones and squads (docs/ILMARI_1_0.md D.3, D.9 items 3-4, task D2): a hostile sensor's
//! cone fills a suspicion meter and raises an incident; stealth, tall blockers and a sneaking pace
//! beat it, a low prop does not, running behind it is heard; companions are never checked; squads
//! follow the roster; two runs agree. Wing 1's boss stands in as the sensor (no AI of its own, so
//! it never aggroes: only its eyes matter).
#![allow(clippy::expect_used)]

mod common;

use combat_core::companion::DALE;
use combat_core::sight::{SightDef, SquadRoles, SquadSide, squad_size};
use combat_core::{Aabb, Command, Content, Effect, EntityId, Event, Role, Sim, SpellId, Vec3};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;
const SENSOR: &str = "il_patch_tuesday";
const SQUAD_SPELL: u32 = 9901;
/// Where the sensor stands: 20 m south of the start, facing the players (yaw 0 looks along -z).
const EYE_AT: (f32, f32) = (0.0, 20.0);

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(EYE_AT.0, EYE_AT.1),
        n if n.starts_with("prop:") => at(1.0, 0.0),
        _ => at(0.0, 0.0),
    }
}

fn eyes(patrol: Vec<(f32, f32)>) -> SightDef {
    SightDef {
        range_m: 30.0,
        half_angle_deg: 60.0,
        notice_ms: 1000,
        patrol,
        patrol_mps: 1.6,
        pause_ms: 500,
        detected_aura: "il_detected".into(),
        detected_ms: 10_000,
        noisy_mps: 5.0,
        noise_m: 7.0,
        sneak_mps: 3.0,
    }
}

fn sight_content(patrol: Vec<(f32, f32)>) -> Content {
    let mut c = content();
    let d = def_mut(&mut c, SENSOR);
    d.ai = None;
    d.sight = Some(eyes(patrol));
    for k in [
        "senior_engineer",
        "backend_dev",
        "devops_eng",
        "frontend_dev",
        "security_engineer",
    ] {
        def_mut(&mut c, k).max_hp = 10_000_000;
    }
    // A world spell that fields a squad (the Consultant's sibling boss casts the real one).
    let mut s = c.spell(SpellId(950)).expect("scooter spell").clone();
    s.id = SpellId(SQUAD_SPELL);
    s.cast_ms = 0;
    s.gcd = false;
    s.yells.clear();
    s.effect = Effect::SummonSquad {
        per_role: SquadRoles {
            tank: "il_android_guard".into(),
            healer: "il_android_clerk".into(),
            melee: "il_prompt_manager".into(),
            ranged: "il_delivery_drone".into(),
        },
        filler: "il_lobby_marshal".into(),
        side: SquadSide::Red,
        min_bodies: 3,
        idle: false,
    };
    c.world.spells.push(s.id);
    c.spells.push(s);
    c
}

fn room(classes: &[&str], c: Content, blockers: Vec<Aabb>) -> (Sim, Vec<EntityId>) {
    room_with(classes, c, blockers, &[])
}

/// `picks`: the first player's companions, chosen before the wing loads (a lone player is on Story).
fn room_with(
    classes: &[&str],
    c: Content,
    blockers: Vec<Aabb>,
    picks: &[&str],
) -> (Sim, Vec<EntityId>) {
    let mut sim = Sim::new(c, 21);
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
    if !picks.is_empty() {
        let keys: Vec<String> = picks.iter().map(|k| k.to_string()).collect();
        sim.set_companion_picks(ids[0], &keys);
    }
    if let Err(e) = sim.load_zone("ilmari1/1", &anchors, blockers) {
        panic!("{e}");
    }
    (sim, ids)
}

fn sensor(sim: &Sim) -> EntityId {
    sim.units()
        .find(|u| u.key == SENSOR)
        .map(|u| u.id)
        .expect("sensor")
}

/// One tick with the player reported at `pos`; the incidents it produced.
fn step(sim: &mut Sim, p: EntityId, pos: Vec3) -> Vec<(EntityId, EntityId)> {
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos,
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    sim.tick()
        .into_iter()
        .filter_map(|e| match e {
            Event::Incident { player, by } => Some((player, by)),
            _ => None,
        })
        .collect()
}

/// Paces back and forth along x within +-4 m of (0, z) at `mps`; the first tick of an incident.
fn first_incident(sim: &mut Sim, p: EntityId, z: f32, mps: f32, secs: u32) -> Option<u32> {
    let (mut x, mut dir) = (0.0_f32, 1.0_f32);
    for t in 0..secs * SECOND {
        x += dir * mps / SECOND as f32;
        if x.abs() > 4.0 {
            dir = -dir;
        }
        if !step(sim, p, Vec3::new(x, 0.0, z)).is_empty() {
            return Some(t);
        }
    }
    None
}

fn has_aura(sim: &Sim, id: EntityId, key: &str) -> bool {
    sim.unit(id)
        .is_some_and(|u| u.auras.iter().any(|a| a.key == key))
}

#[test]
fn standing_in_plain_sight_raises_an_incident() {
    let (mut sim, ids) = room(&["fullstack"], sight_content(vec![]), Vec::new());
    let mut got = Vec::new();
    let mut peak = 0;
    for _ in 0..5 * SECOND {
        got.extend(step(&mut sim, ids[0], Vec3::new(0.0, 0.0, 0.0)));
        peak = peak.max(sim.sight_suspicion(ids[0]));
    }
    assert_eq!(got.first(), Some(&(ids[0], sensor(&sim))), "{got:?}");
    assert!(peak > 50, "the meter was visible on the way: {peak}");
    assert!(
        has_aura(&sim, ids[0], "il_detected"),
        "Detected is on the player"
    );
}

#[test]
fn a_player_in_stealth_is_never_seen() {
    let (mut sim, ids) = room(&["security"], sight_content(vec![]), Vec::new());
    assert!(sim.debug_apply_aura(ids[0], "tailgating", 600_000));
    for _ in 0..12 * SECOND {
        assert!(step(&mut sim, ids[0], Vec3::new(0.0, 0.0, 0.0)).is_empty());
    }
    assert_eq!(sim.sight_suspicion(ids[0]), 0);
    assert!(!has_aura(&sim, ids[0], "il_detected"));
}

#[test]
fn tall_blockers_hide_you_and_low_ones_do_not() {
    let wall_at = |top: f32| Aabb {
        min: Vec3::new(-6.0, 0.0, 9.5),
        max: Vec3::new(6.0, top, 10.5),
    };
    let tall = room(&["fullstack"], sight_content(vec![]), vec![wall_at(3.0)]);
    let low = room(&["fullstack"], sight_content(vec![]), vec![wall_at(0.8)]);
    let (mut a, mut b) = (tall, low);
    let (mut behind_wall, mut behind_table) = (None, None);
    for t in 0..5 * SECOND {
        let pos = Vec3::new(0.0, 0.0, 0.0);
        if !step(&mut a.0, a.1[0], pos).is_empty() {
            behind_wall.get_or_insert(t);
        }
        if !step(&mut b.0, b.1[0], pos).is_empty() {
            behind_table.get_or_insert(t);
        }
    }
    assert_eq!(behind_wall, None, "a wall breaks the line");
    assert!(behind_table.is_some(), "a table is below the sensor's eyes");
}

#[test]
fn any_class_can_sneak_with_care() {
    let run = {
        let (mut sim, ids) = room(&["backend"], sight_content(vec![]), Vec::new());
        first_incident(&mut sim, ids[0], 0.0, 6.0, 30)
    };
    let sneak = {
        let (mut sim, ids) = room(&["backend"], sight_content(vec![]), Vec::new());
        first_incident(&mut sim, ids[0], 0.0, 2.0, 30)
    };
    let (run, sneak) = (
        run.expect("running is seen"),
        sneak.expect("sneaking is slower, not safe"),
    );
    assert!(sneak > run, "sneak {sneak} must outlast run {run}");
}

#[test]
fn running_behind_the_sensor_is_heard_and_walking_is_not() {
    // 5 m behind the sensor: outside its cone, inside its noise radius, nothing in between.
    let z = EYE_AT.1 + 5.0;
    let ran = {
        let (mut sim, ids) = room(&["fullstack"], sight_content(vec![]), Vec::new());
        first_incident(&mut sim, ids[0], z, 6.0, 10)
    };
    let walked = {
        let (mut sim, ids) = room(&["fullstack"], sight_content(vec![]), Vec::new());
        first_incident(&mut sim, ids[0], z, 2.0, 10)
    };
    assert!(ran.is_some(), "heard");
    assert_eq!(walked, None, "silent at a walk");
}

#[test]
fn a_patrol_walks_its_loop_as_a_pure_function_of_the_clock() {
    let path = vec![(0.0, 0.0), (10.0, 0.0), (10.0, -5.0), (0.0, -5.0)];
    let trace = || {
        let (mut sim, ids) = room(&["fullstack"], sight_content(path.clone()), Vec::new());
        let s = sensor(&sim);
        (0..20 * SECOND)
            .map(|_| {
                step(&mut sim, ids[0], Vec3::new(300.0, 0.0, 300.0));
                let u = sim.unit(s).expect("sensor");
                (u.pos.x.to_bits(), u.pos.z.to_bits(), u.yaw.to_bits())
            })
            .collect::<Vec<_>>()
    };
    let (a, b) = (trace(), trace());
    assert_eq!(a, b, "two runs agree bit for bit");
    let xs: Vec<f32> = a.iter().map(|p| f32::from_bits(p.0)).collect();
    let (lo, hi) = (
        xs.iter().copied().fold(f32::MAX, f32::min),
        xs.iter().copied().fold(f32::MIN, f32::max),
    );
    assert!(hi - lo > 5.0, "it actually walks: {lo}..{hi}");
}

#[test]
fn companions_are_never_checked() {
    // The owner hides; Dale stands in plain view of the sensor. No incident may name anyone.
    let (mut sim, ids) = room_with(&["security"], sight_content(vec![]), Vec::new(), &[DALE]);
    assert!(!sim.companions_of(ids[0]).is_empty(), "Dale came along");
    assert!(sim.debug_apply_aura(ids[0], "tailgating", 600_000));
    let mut seen = Vec::new();
    for _ in 0..8 * SECOND {
        seen.extend(step(&mut sim, ids[0], Vec3::new(0.0, 0.0, 0.0)));
    }
    assert!(seen.is_empty(), "{seen:?}");
}

#[test]
fn squad_size_is_one_per_role_and_never_below_the_minimum() {
    use Role::*;
    assert_eq!(squad_size(&[Tank, Healer, Melee, Ranged, Ranged], 3), 4);
    assert_eq!(squad_size(&[Tank, Healer, Melee], 3), 3);
    assert_eq!(squad_size(&[Melee, Melee, Melee], 3), 3);
    assert_eq!(
        squad_size(&[Ranged], 3),
        3,
        "a solo player still meets three bodies"
    );
    assert_eq!(squad_size(&[Tank, Healer], 1), 2);
}

fn squad_keys(sim: &mut Sim, caster: EntityId) -> Vec<String> {
    sim.apply_as(
        caster,
        Command::Cast {
            caster,
            spell: SpellId(SQUAD_SPELL),
        },
    );
    sim.tick();
    let mut keys: Vec<String> = sim
        .squad_units(SquadSide::Red)
        .into_iter()
        .filter_map(|u| sim.unit(u).map(|s| s.key.clone()))
        .collect();
    keys.sort();
    keys
}

#[test]
fn squads_mirror_the_party_roster() {
    // Backend (tank), DevOps (healer), Frontend (melee), Fullstack and Security: one unit per role.
    let (mut sim, ids) = room(
        &["backend", "devops", "frontend", "fullstack", "security"],
        sight_content(vec![]),
        Vec::new(),
    );
    assert_eq!(
        squad_keys(&mut sim, ids[0]),
        [
            "il_android_clerk",
            "il_android_guard",
            "il_delivery_drone",
            "il_prompt_manager"
        ]
    );
    // Trio of tank, healer and melee: three bodies, no filler needed.
    let (mut trio, t) = room(
        &["backend", "devops", "frontend"],
        sight_content(vec![]),
        Vec::new(),
    );
    assert_eq!(squad_keys(&mut trio, t[0]).len(), 3);
    // Three ranged players: one role, padded to the minimum with the filler.
    let (mut ranged, r) = room(
        &["fullstack", "fullstack", "fullstack"],
        sight_content(vec![]),
        Vec::new(),
    );
    assert_eq!(
        squad_keys(&mut ranged, r[0]),
        ["il_delivery_drone", "il_lobby_marshal", "il_lobby_marshal"]
    );
}

#[test]
fn a_solo_story_player_with_companions_meets_exactly_three_bodies() {
    let picks = [DALE, combat_core::companion::PRIYA];
    let (mut sim, ids) = room_with(&["fullstack"], sight_content(vec![]), Vec::new(), &picks);
    assert_eq!(sim.companions_of(ids[0]).len(), 2);
    // The companions are not in the roster read: Dale (a tank) would otherwise add a role.
    let keys = squad_keys(&mut sim, ids[0]);
    assert_eq!(keys.len(), 3, "{keys:?}");
    assert!(!keys.contains(&"il_android_guard".to_string()), "{keys:?}");
}

#[test]
fn the_same_fight_twice_gives_the_same_log() {
    let log = || {
        let (mut sim, ids) = room(
            &["fullstack", "backend"],
            sight_content(vec![(0.0, 0.0), (8.0, 0.0), (8.0, -4.0)]),
            Vec::new(),
        );
        let mut out = Vec::new();
        for t in 0..15 * SECOND {
            let wob = (t % 40) as f32 / 10.0;
            sim.apply_as(
                ids[1],
                Command::PlayerMoved {
                    pos: Vec3::new(wob, 0.0, 2.0),
                    yaw: 0.0,
                    moving: true,
                    jumping: false,
                },
            );
            step(&mut sim, ids[0], Vec3::new(0.0, 0.0, 0.0));
        }
        sim.apply_as(
            ids[0],
            Command::Cast {
                caster: ids[0],
                spell: SpellId(SQUAD_SPELL),
            },
        );
        for e in sim.tick() {
            out.push(log_line(&sim, 0, &e));
        }
        out.push(format!("{:?}", sim.squad_units(SquadSide::Red)));
        out.push(format!("{}", sim.sight_suspicion(ids[0])));
        out
    };
    assert_eq!(log(), log());
}

#[test]
fn sight_content_validates() {
    let ron = include_str!("../../../content/game.ron");
    assert!(Content::from_ron(ron).is_ok());
    let eyes = |notice: u32, aura: &str, patrol: &str| {
        ron.replacen(
            "key: \"il_patch_tuesday\",",
            &format!(
                "key: \"il_patch_tuesday\", sight: Some((range_m: 30.0, half_angle_deg: 60.0, notice_ms: {notice}, detected_aura: \"{aura}\", detected_ms: 10000, patrol: {patrol})),"
            ),
            1,
        )
    };
    assert!(
        Content::from_ron(&eyes(1000, "il_detected", "[]")).is_ok(),
        "no patrol"
    );
    assert!(Content::from_ron(&eyes(1000, "il_detected", "[(0.0, 0.0), (5.0, 0.0)]")).is_ok());
    for (n, a, p) in [
        (1000, "il_detected", "[(1.0, 1.0)]"),
        (0, "il_detected", "[]"),
        (1000, "no_such_aura", "[]"),
    ] {
        assert!(Content::from_ron(&eyes(n, a, p)).is_err(), "{n} {a} {p}");
    }
}
