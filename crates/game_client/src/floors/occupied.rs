//! The occupied campus (docs/ILMARI_09.md B.2): the campus builder's own output plus an overlay,
//! so the base campus keeps every prop, anchor and door it has and a phase costs one more zone id.
//! The overlay is cheap on purpose: a red-dusk theme (no lights, `docs/PERF.md`), ILMARI screens
//! on the three landmarks, barricaded checkpoints on four roads, propaganda screens, Dale's
//! tunnel shed with its two doors, the Resistance's field post and the spots the Prologue quests
//! work from.

use super::campus::{building, facade_door};
use super::districts::signpost;
use super::kit::*;
use super::layout::{FloorKind, Theme, yaw_towards};
use super::models::{BARRIER, BOX_LARGE, CONE_ROAD, put};
use super::themes::v;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// The arrival lawn, where the checkpoints' guards look for newcomers.
const SPAWN: (f32, f32) = (-12.0, 32.0);

/// Dale's tunnel: a concrete shed on the lawn east of the boulevard, with one door down to the
/// Resistance base (south face) and one back to the campus as it was (north face).
const SHED: (Vec2, Vec3) = (Vec2::new(14.0, 57.5), Vec3::new(8.0, 6.0, 5.0));

/// Red dusk, darker than the campus's morning: the sky is the fog, so the whole map turns.
pub fn theme() -> Theme {
    let c = Color::srgb;
    Theme {
        wall: c(0.55, 0.5, 0.52),
        trim: c(0.15, 0.12, 0.14),
        accent: LinearRgba::rgb(1.6, 0.25, 0.2),
        fabric: [c(0.4, 0.3, 0.32), c(0.35, 0.3, 0.4), c(0.3, 0.38, 0.36)],
        floor: FloorKind::Grass,
        fog: c(0.4, 0.19, 0.19),
        ambient: c(0.85, 0.55, 0.5),
        ambient_brightness: 640.0,
        sun: c(1.0, 0.5, 0.3),
        sun_illuminance: 7500.0,
        fog_range: (40.0, 130.0),
    }
}

/// Everything the occupation adds. Called after the campus's own anchors exist.
pub fn overlay(p: &mut Plan) {
    // The occupation switched the office off: no coffee, shuttles or boards. Their content
    // belongs to the campus zone, and a prop without content is not clickable.
    p.props.clear();
    p.anchors.retain(|a| !a.name.starts_with("prop:"));
    landmarks(p);
    tunnel(p);
    field_post(p);
    checkpoints(p);
    quest_spots(p);
    gate(p);
}

/// "ILMARI" on the Tower, the Data Center and the Lab, and a closed sign at each door. Signs
/// stand clear of the entrance dressing (the Lab's neon takes its front face, so its splash is
/// on the east face).
fn landmarks(p: &mut Plan) {
    p.sign(v(0.0, 20.5, -24.8), 0.0, 16.0, Sign::IlSplash);
    p.sign(v(85.85, 8.0, -84.0), -FRAC_PI_2, 8.0, Sign::IlSplash);
    p.sign(v(-58.85, 6.0, -80.0), FRAC_PI_2, 10.0, Sign::IlSplash);
    for (x, z, along_x) in [
        (-7.5, -19.5, true),
        (79.0, -92.0, false),
        (-80.0, -63.0, true),
    ] {
        signpost(p, x, z, along_x, 2.4, Sign::IlClosed);
    }
}

fn tunnel(p: &mut Plan) {
    building(p, SHED, Mat::Concrete);
    let south = facade_door(p, SHED, Face::PosZ, Sign::DalesTunnel);
    let north = facade_door(p, SHED, Face::NegZ, Sign::TunnelHome);
    p.anchor("door:resistance/1", south.0, south.1);
    p.anchor("door:campus", north.0, north.1);
}

/// The Server Room Gate (docs/ILMARI_09.md B.4): a second door in the Data Center's west wall,
/// 8.5 m north of the one the occupation closed. Opens with the golem's quest (`il_04`).
fn gate(p: &mut Plan) {
    let (at, yaw) = facade_door(
        p,
        (Vec2::new(98.0, -75.5), Vec3::new(24.0, 11.0, 30.0)),
        Face::NegX,
        Sign::IlW1Door,
    );
    p.anchor("door:ilmari1/1", at, yaw);
}

/// Dale's folding table, a laminated sign, crates and a sandbag wall: the Resistance's forward
/// post. The cast stands in an arc around the table, 8 m or more from the tunnel doors.
fn field_post(p: &mut Plan) {
    let spawn = v(SPAWN.0, 0.0, SPAWN.1);
    for (name, x, z) in [
        ("npc_il_dale", 23.0, 66.0),
        ("npc_il_priya", 28.0, 62.0),
        ("npc_il_rupert", 24.0, 58.5),
        ("npc_il_brenda", 28.0, 55.0),
    ] {
        let at = v(x, 0.0, z);
        p.anchor(name, at, yaw_towards(at, spawn));
    }
    put(p, &BOX_LARGE, v(26.6, 0.0, 66.4), 0.3, 1.6, true);
    put(p, &BOX_LARGE, v(26.5, 0.55 * 1.6, 66.4), 0.6, 1.6, false);
    put(p, &BOX_LARGE, v(20.4, 0.0, 67.0), -0.2, 1.6, true);
    p.boxed(v(24.4, 0.45, 67.8), v(1.9, 0.9, 0.9), Mat::Wood, true);
    p.sign(v(24.4, 1.2, 67.34), 0.0, 1.8, Sign::ResDale);
    // The sandbag line on the road side: low, grey, readable from the road.
    for (x, z, len, along_x) in [(33.0, 59.0, 5.0, false), (31.0, 70.0, 6.0, true)] {
        let size = if along_x {
            v(len, 0.9, 0.9)
        } else {
            v(0.9, 0.9, len)
        };
        p.boxed(v(x, 0.45, z), size, Mat::Sand, true);
    }
    let vending = v(32.0, 0.0, 63.8);
    p.anchor("il_vending_spot", vending, yaw_towards(vending, spawn));
}

/// One checkpoint on a road: a funnel of barriers, a guard hut, a boom gate and a screen post.
/// Local frame: x across the road, z along it; the pack stands in the middle of the lane.
fn checkpoint(p: &mut Plan, name: &'static str, x: f32, z: f32, along_x: bool) {
    let yaw = if along_x { FRAC_PI_2 } else { 0.0 };
    p.framed(v(x, 0.0, z), yaw, |p| {
        for s in [-1.0, 1.0] {
            put(p, &BARRIER, v(s * 3.6, 0.0, -3.4), FRAC_PI_2, 17.0, true);
            put(p, &BARRIER, v(s * 3.6, 0.0, 3.4), FRAC_PI_2, 17.0, true);
            put(p, &CONE_ROAD, v(s * 2.6, 0.0, -5.6), 0.0, 9.0, false);
            put(p, &CONE_ROAD, v(s * 2.6, 0.0, 5.6), 0.0, 9.0, false);
        }
        // Guard hut with a lit window, off the lane.
        p.boxed(v(6.2, 1.3, 0.0), v(3.0, 2.6, 3.0), Mat::Metal, true);
        p.boxed(v(4.66, 1.5, 0.0), v(0.06, 0.9, 2.0), Mat::ScreenGlow, false);
        p.boxed(v(6.2, 2.7, 0.0), v(3.4, 0.2, 3.4), Mat::Dark, false);
        // Boom gate, raised: a red-and-white pole on a post, pointing at the hut.
        p.boxed(v(-4.4, 0.6, 0.0), v(0.3, 1.2, 0.3), Mat::Dark, false);
        p.boxed(v(-3.2, 1.55, -0.0), v(2.4, 0.1, 0.1), Mat::Red, false);
        p.boxed(v(-2.1, 1.55, 0.0), v(0.9, 0.11, 0.11), Mat::White, false);
        signpost(p, -6.5, 0.0, false, 2.2, Sign::IlCheckpoint);
    });
    let at = v(x, 0.0, z);
    p.anchor(name, at, yaw_towards(at, v(SPAWN.0, 0.0, SPAWN.1)));
}

fn checkpoints(p: &mut Plan) {
    checkpoint(p, "il_checkpoint_1", -71.0, 10.0, false);
    checkpoint(p, "il_checkpoint_2", 71.0, -8.0, false);
    checkpoint(p, "il_checkpoint_3", 0.0, 108.0, false);
    checkpoint(p, "il_checkpoint_4", 0.0, -70.0, false);
}

/// A propaganda screen on a post; `prop` makes its cable bundle a quest prop (stand spot 1.8 m
/// in front of the foot of the post).
fn screen(p: &mut Plan, x: f32, z: f32, yaw: f32, sign: Sign, prop: Option<&'static str>) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 1.1, 0.0), v(0.16, 2.2, 0.16), Mat::Metal, false);
        p.boxed(v(0.0, 2.9, 0.0), v(3.2, 1.3, 0.14), Mat::Dark, false);
        p.sign(v(0.0, 2.9, 0.09), 0.0, 3.0, sign);
        p.sign(v(0.0, 2.9, -0.09), PI, 3.0, sign);
        p.block(v(0.0, 1.0, 0.0), v(0.5, 2.0, 0.5));
    });
    if let Some(key) = prop {
        let (s, c) = yaw.sin_cos();
        let stand = v(x + s * 1.8, 0.0, z + c * 1.8);
        p.interactable(key, v(x, 0.5, z), stand);
    }
}

/// The places the Prologue works from: the node, the elites' spots, the escort goal, the screens
/// whose cables are unplugged, and the passes and mug photos pinned around the map. Every spot
/// is 2 m clear of walls and 5 m from other anchors (the reachability tests check).
fn quest_spots(p: &mut Plan) {
    let spawn = v(SPAWN.0, 0.0, SPAWN.1);
    // The fibre node (a unit, so the waves can hit it) on the Data Center road; its plug is the
    // prop 3.5 m south of it.
    let node = v(80.0, 0.0, -58.0);
    p.anchor("il_node_spot", node, yaw_towards(node, spawn));
    p.interactable(
        "prop:il_node_dc",
        v(node.x, 1.0, node.z + 0.6),
        v(80.0, 0.0, -54.4),
    );
    // The elites: the gate's golem on the road to the Data Center, the marshal on the lawn
    // east of the Tower.
    let golem = v(62.0, 0.0, -66.0);
    p.anchor("rare_gate", golem, yaw_towards(golem, spawn));
    let marshal = v(30.0, 0.0, -34.0);
    p.anchor("rare_lobby", marshal, yaw_towards(marshal, spawn));
    p.anchor("il_escort_goal", v(74.0, 0.0, -72.0), 0.0);
    // Cable bundles behind three propaganda screens, and a few more that only talk.
    screen(p, 40.0, 60.0, 0.0, Sign::IlScreenA, Some("prop:il_cable_1"));
    screen(p, -50.0, 40.0, PI, Sign::IlScreenB, Some("prop:il_cable_2"));
    screen(
        p,
        -76.0,
        -24.0,
        FRAC_PI_2,
        Sign::IlScreenC,
        Some("prop:il_cable_3"),
    );
    screen(p, 20.0, 30.0, PI, Sign::IlScreenA, None);
    screen(p, 100.0, 30.0, FRAC_PI_2, Sign::IlScreenB, None);
    // HR passes and mug photos: glowing pickups (the office module draws them).
    for (key, x, z) in [
        ("prop:il_hr_pass_1", -40.0, -35.0),
        ("prop:il_hr_pass_2", 40.0, -30.0),
        ("prop:il_hr_pass_3", 100.0, 0.0),
        ("prop:il_mug_proof_1", -100.0, 70.0),
        ("prop:il_mug_proof_2", -20.0, 70.0),
        ("prop:il_mug_proof_3", 110.0, -50.0),
        ("prop:il_mug_proof_4", 90.0, 60.0),
    ] {
        p.interactable(key, v(x, 0.3, z), v(x + 1.2, 0.0, z));
    }
}
