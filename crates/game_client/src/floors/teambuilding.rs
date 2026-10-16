//! "Mandatory Fun: Teambuilding": the hotel garden party of the stealth minigame
//! (`combat_core::teambuilding`). The hotel terrace (start) on the west, the taxi rank (exit) on
//! the east gate, and between them the buffet, the CEO's VIP tent, the open bar, a quiet corner
//! behind the hedges, the trust-fall lawn with the icebreaker circle, the group photo backdrop,
//! the toilet queue, the swag table and the karaoke stage. Anchors: `tb_start`, `tb_exit`,
//! `tb_icebreaker`, `obj_<objective>`, `hide_1..8`, `snack_1..10` and the watchers' loops
//! `route:<route>:<n>`; the rules and timings live in `game.ron`.

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble, yaw_towards};
use super::offsite::{balloons, bunting, chair_to, floor_patch, round_patch, wall_band};
use super::shape::rounded_rect;
use super::themes::{pt, runner, v};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const ZONE: &str = "teambuilding/1";

pub const START: (f32, f32) = (-40.0, 0.0);
pub const EXIT: (f32, f32) = (42.0, 0.0);
const ICEBREAKER: (f32, f32) = (-8.0, 6.0);

/// Objective anchors with their spots.
const OBJECTIVES: [(&str, (f32, f32)); 5] = [
    ("obj_canape", (-12.0, -24.0)),
    ("obj_drink", (18.0, -23.0)),
    ("obj_swag", (0.0, 21.2)),
    ("obj_photo", (-23.0, 22.6)),
    ("obj_incident", (40.0, -29.0)),
];

/// Hiding spots and the prop that hides you there.
pub const HIDES: [(&str, (f32, f32)); 8] = [
    ("hide_1", (-24.0, -22.6)),
    ("hide_2", (27.6, -21.3)),
    ("hide_3", (-38.0, 25.5)),
    ("hide_4", (-9.0, 17.9)),
    ("hide_5", (13.0, 10.1)),
    ("hide_6", (-12.0, -4.0)),
    ("hide_7", (20.0, 16.2)),
    ("hide_8", (-33.0, 9.6)),
];

pub const SNACKS: [(&str, (f32, f32)); 10] = [
    ("snack_1", (-30.0, -4.0)),
    ("snack_2", (-18.0, -12.0)),
    ("snack_3", (-4.0, -15.0)),
    ("snack_4", (8.0, -10.0)),
    ("snack_5", (24.0, -8.0)),
    ("snack_6", (26.0, 6.0)),
    ("snack_7", (6.0, 8.0)),
    ("snack_8", (-16.0, 10.0)),
    ("snack_9", (-32.0, 16.0)),
    ("snack_10", (14.0, 28.0)),
];

/// A watcher's loop: anchor names (static, for the layout) and waypoints.
type Route = (&'static [&'static str], &'static [(f32, f32)]);

const ROUTES: [Route; 6] = [
    (
        &[
            "route:mgr_a:0",
            "route:mgr_a:1",
            "route:mgr_a:2",
            "route:mgr_a:3",
        ],
        &[(-34.0, -16.0), (-8.0, -16.0), (-8.0, -9.0), (-34.0, -9.0)],
    ),
    (
        &[
            "route:mgr_b:0",
            "route:mgr_b:1",
            "route:mgr_b:2",
            "route:mgr_b:3",
        ],
        &[(8.0, -14.0), (30.0, -14.0), (30.0, -4.0), (8.0, -4.0)],
    ),
    (
        &[
            "route:ai_a:0",
            "route:ai_a:1",
            "route:ai_a:2",
            "route:ai_a:3",
        ],
        &[(16.0, 12.0), (34.0, 12.0), (34.0, 18.0), (16.0, 18.0)],
    ),
    (
        &["route:ai_b:0", "route:ai_b:1"],
        &[(-28.0, 12.0), (6.0, 12.0)],
    ),
    (
        &[
            "route:hr:0",
            "route:hr:1",
            "route:hr:2",
            "route:hr:3",
            "route:hr:4",
            "route:hr:5",
        ],
        &[
            (-16.0, 3.0),
            (18.0, 3.0),
            (30.0, -10.0),
            (31.0, -24.0),
            (30.0, -10.0),
            (18.0, 3.0),
        ],
    ),
    (
        &[
            "route:ceo:0",
            "route:ceo:1",
            "route:ceo:2",
            "route:ceo:3",
            "route:ceo:4",
            "route:ceo:5",
        ],
        &[
            (0.0, -30.0),
            (0.0, -18.0),
            (-24.0, -2.0),
            (0.0, 8.0),
            (24.0, -2.0),
            (0.0, -18.0),
        ],
    ),
];

fn theme() -> Theme {
    let c = Color::srgb;
    Theme {
        wall: c(0.2, 0.4, 0.22),
        trim: c(0.16, 0.3, 0.16),
        accent: LinearRgba::rgb(2.4, 1.6, 0.6),
        fabric: [c(0.95, 0.35, 0.55), c(0.2, 0.5, 0.85), c(0.98, 0.78, 0.25)],
        floor: FloorKind::Grass,
        fog: c(0.95, 0.78, 0.66),
        ambient: c(1.0, 0.9, 0.82),
        ambient_brightness: 880.0,
        sun: c(1.0, 0.86, 0.66),
        sun_illuminance: 14000.0,
        fog_range: (60.0, 180.0),
    }
}

/// A hedge block: solid, taller than anyone, so it breaks sight lines.
fn hedge(p: &mut Plan, x: f32, z: f32, sx: f32, sz: f32) {
    p.boxed(v(x, 1.1, z), v(sx, 2.2, sz), Mat::LeafDark, true);
    p.boxed(v(x, 2.25, z), v(sx + 0.1, 0.12, sz + 0.1), Mat::Leaf, false);
}

fn plant(p: &mut Plan, x: f32, z: f32) {
    p.prop(&POTTED_PLANT, v(x, 0.0, z), 0.0, true);
}

/// A faint ring on the grass marks every hiding spot, so players learn to read them.
fn hide_ring(p: &mut Plan, x: f32, z: f32) {
    for k in 0..12 {
        let a = k as f32 * TAU / 12.0;
        p.slab(
            v(x + 1.0 * a.cos(), 0.03, z + 1.0 * a.sin()),
            -a + FRAC_PI_2,
            v(0.38, 0.02, 0.07),
            Mat::Leaf,
        );
    }
}

/// Wooden posts with a sagging line of warm bulbs (along x).
fn string_lights(p: &mut Plan, x0: f32, x1: f32, z: f32) {
    let n = ((x1 - x0) / 9.0).round().max(1.0) as usize;
    for k in 0..=n {
        let x = x0 + (x1 - x0) * k as f32 / n as f32;
        p.boxed(v(x, 1.8, z), v(0.14, 3.6, 0.14), Mat::Wood, false);
    }
    let bulbs = ((x1 - x0) / 1.6) as usize;
    for k in 0..=bulbs {
        let t = k as f32 / bulbs as f32;
        let seg = (t * n as f32).fract();
        let y = 3.5 - 0.5 * (seg * PI).sin();
        p.prim(
            PrimKind::Sphere,
            v(x0 + (x1 - x0) * t, y, z),
            v(0.13, 0.13, 0.13),
            Mat::Candle,
        );
    }
}

/// A sign on two thin posts, facing `yaw`.
fn post_sign(p: &mut Plan, x: f32, z: f32, y: f32, width: f32, yaw: f32, sign: Sign) {
    let side = Vec3::new(yaw.cos(), 0.0, -yaw.sin()) * (width / 2.0 - 0.1);
    for s in [-1.0, 1.0] {
        let at = v(x, 0.0, z) + side * s;
        p.boxed(v(at.x, y / 2.0, at.z), v(0.08, y, 0.08), Mat::Metal, false);
    }
    p.sign(v(x, y, z), yaw, width, sign);
}

/// Hotel facade outside the west hedge: the backdrop of the terrace.
fn hotel(p: &mut Plan) {
    p.boxed(v(-60.0, 7.0, 0.0), v(22.0, 14.0, 60.0), Mat::Facade, false);
    for z in [-24.0, -12.0, 0.0, 12.0, 24.0] {
        for y in [3.5, 7.0, 10.5] {
            p.boxed(v(-48.9, y, z), v(0.1, 1.6, 3.2), Mat::Window, false);
        }
    }
    // Terrace paving with a few lounge chairs.
    p.slab(v(-40.0, 0.01, 0.0), 0.0, v(11.0, 0.02, 30.0), Mat::Concrete);
    for z in [-11.0, 11.0] {
        p.prop(&LOUNGE_CHAIR, v(-44.0, 0.0, z), FRAC_PI_2, true);
    }
    post_sign(p, -43.5, -4.5, 2.6, 3.4, FRAC_PI_2, Sign::TbWelcome);
}

/// Garden-party dressing, all visual: the stone path, flower beds, a gazebo, a rose arch, patio
/// sets, picnic blankets, lanterns and a fountain. Nothing here blocks: the stealth rules own
/// the solid cover (hedges, tents, banners) and the tests check the anchors against it.
fn garden(p: &mut Plan) {
    // Lighter hedge crown so the outline reads as hedge, not wall.
    wall_band(p, Mat::Leaf, 3.6, H, 0.05);
    runner(
        p,
        &[
            pt(-38.0, 0.0),
            pt(-20.0, -2.0),
            pt(0.0, 0.0),
            pt(20.0, 2.0),
            pt(40.0, 0.0),
        ],
        3.0,
        Mat::Sand,
        Mat::Concrete,
    );
    // Flower beds: a soil ring with a dome of blooms.
    for (k, (x, z)) in [
        (-36.0, -14.0),
        (36.0, 12.0),
        (10.0, 21.0),
        (-4.0, 26.0),
        (-44.0, 18.0),
        (44.0, -12.0),
    ]
    .into_iter()
    .enumerate()
    {
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.12, z),
            v(2.6, 0.24, 2.6),
            Mat::Bark,
        );
        p.prim(PrimKind::Sphere, v(x, 0.3, z), v(2.2, 0.8, 2.2), Mat::Leaf);
        for j in 0..7 {
            let a = j as f32 * 0.9 + k as f32;
            let r = 0.2 + (j % 3) as f32 * 0.35;
            p.prim(
                PrimKind::Sphere,
                v(x + r * a.cos(), 0.62 - r * 0.25, z + r * a.sin()),
                v(0.3, 0.3, 0.3),
                [Mat::Fabric0, Mat::Fabric1, Mat::Fabric2, Mat::Yellow][(j + k) % 4],
            );
        }
    }
    // Gazebo on the east lawn with a little table.
    let (gx, gz) = (40.0, 10.0);
    p.prim(
        PrimKind::Cylinder,
        v(gx, 0.1, gz),
        v(6.4, 0.2, 6.4),
        Mat::Wood,
    );
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0;
        p.boxed(
            v(gx + 2.8 * a.cos(), 1.6, gz + 2.8 * a.sin()),
            v(0.18, 3.2, 0.18),
            Mat::White,
            false,
        );
    }
    p.prim(
        PrimKind::Cone,
        v(gx, 3.8, gz),
        v(7.0, 1.4, 7.0),
        Mat::Fabric0,
    );
    p.prim(
        PrimKind::Cylinder,
        v(gx, 0.55, gz),
        v(1.2, 0.7, 1.2),
        Mat::White,
    );
    for k in 0..3 {
        let a = k as f32 * TAU / 3.0 + 0.4;
        chair_to(p, gx + 1.7 * a.cos(), gz + 1.7 * a.sin(), gx, gz);
    }
    // Rose arch over the path near the start.
    for z in [-1.8, 1.8] {
        p.boxed(v(-33.0, 1.3, z), v(0.14, 2.6, 0.14), Mat::White, false);
        for j in 0..4 {
            p.prim(
                PrimKind::Sphere,
                v(-33.0, 0.5 + j as f32 * 0.6, z),
                v(0.3, 0.3, 0.3),
                Mat::Fabric0,
            );
        }
    }
    p.boxed(v(-33.0, 2.65, 0.0), v(0.16, 0.16, 3.9), Mat::White, false);
    for j in 0..7 {
        p.prim(
            PrimKind::Sphere,
            v(-33.0, 2.7, -1.8 + j as f32 * 0.6),
            v(0.32, 0.3, 0.32),
            [Mat::Fabric0, Mat::Leaf][j % 2],
        );
    }
    // Patio sets on the hotel terrace.
    for (x, z) in [(-43.0, -6.0), (-43.0, 6.0)] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.45, z),
            v(1.1, 0.06, 1.1),
            Mat::White,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.22, z),
            v(0.1, 0.44, 0.1),
            Mat::Metal,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x, 1.6, z),
            v(0.05, 3.2, 0.05),
            Mat::Metal,
        );
        p.prim(PrimKind::Cone, v(x, 3.1, z), v(2.6, 0.6, 2.6), Mat::Fabric1);
        for k in 0..3 {
            let a = k as f32 * TAU / 3.0 + 1.0;
            chair_to(p, x + 1.0 * a.cos(), z + 1.0 * a.sin(), x, z);
        }
    }
    // Picnic blankets with baskets, on the lawn away from the stealth spots.
    for (x, z, yaw, m) in [
        (-8.0, 28.0, 0.2, Mat::Fabric0),
        (22.0, -14.5, -0.3, Mat::Fabric1),
        (-28.0, 28.0, 0.5, Mat::Fabric2),
    ] {
        floor_patch(p, x, z, yaw, 2.6, 2.2, m);
        floor_patch(p, x, z, yaw, 1.8, 1.4, Mat::White);
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.18, z),
            v(0.6, 0.3, 0.6),
            Mat::Wood,
        );
    }
    // Hay bales along the south hedge.
    for x in [10.0, 12.2, 11.1] {
        let up = if x > 12.0 { 0.9 } else { 0.0 };
        p.boxed(v(x, 0.4 + up, 32.0), v(1.4, 0.8, 0.9), Mat::Sand, false);
    }
    // Fountain in the north-west corner.
    let (fx, fz) = (-38.0, -28.0);
    p.prim(
        PrimKind::Cylinder,
        v(fx, 0.3, fz),
        v(4.0, 0.6, 4.0),
        Mat::Concrete,
    );
    p.prim(
        PrimKind::Cylinder,
        v(fx, 0.61, fz),
        v(3.5, 0.03, 3.5),
        Mat::Water,
    );
    p.prim(
        PrimKind::Cylinder,
        v(fx, 1.0, fz),
        v(0.5, 1.4, 0.5),
        Mat::Concrete,
    );
    p.prim(
        PrimKind::Cylinder,
        v(fx, 1.7, fz),
        v(1.6, 0.12, 1.6),
        Mat::Concrete,
    );
    p.prim(PrimKind::Cone, v(fx, 2.2, fz), v(0.6, 1.0, 0.6), Mat::Glass);
    // Topiary: a ball on a stem by the tents, the gate and the bar.
    for (x, z) in [
        (-7.5, -23.0),
        (7.5, -23.0),
        (43.0, 4.2),
        (43.0, -4.2),
        (28.0, -23.5),
    ] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.5, z),
            v(0.15, 1.0, 0.15),
            Mat::Bark,
        );
        p.prim(PrimKind::Sphere, v(x, 1.4, z), v(1.1, 1.1, 1.1), Mat::Leaf);
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.2, z),
            v(0.7, 0.4, 0.7),
            Mat::Concrete,
        );
    }
    // Lanterns along the path and warm bunting between the tents.
    for x in [-26.0, -10.0, 10.0, 26.0] {
        for z in [-3.2, 3.2] {
            p.boxed(v(x, 0.9, z), v(0.06, 1.8, 0.06), Mat::Metal, false);
            p.prim(
                PrimKind::Sphere,
                v(x, 1.9, z),
                v(0.3, 0.38, 0.3),
                Mat::Candle,
            );
        }
    }
    bunting(
        p,
        (-6.0, -24.8),
        (6.0, -24.8),
        3.3,
        [Mat::Fabric0, Mat::Fabric1, Mat::Fabric2],
    );
    bunting(
        p,
        (11.5, -25.0),
        (24.5, -25.0),
        3.5,
        [Mat::Fabric2, Mat::Fabric0, Mat::Fabric1],
    );
    bunting(
        p,
        (-6.0, 22.0),
        (6.0, 22.0),
        3.4,
        [Mat::Fabric1, Mat::Fabric2, Mat::Fabric0],
    );
    balloons(p, 24.0, 22.0, [Mat::Fabric0, Mat::Yellow, Mat::Fabric1]);
    balloons(p, 33.0, 22.5, [Mat::Fabric1, Mat::Fabric2, Mat::Fabric0]);
    round_patch(p, ICEBREAKER.0, ICEBREAKER.1, 7.0, Mat::Fabric2);
    // Warm light pools: the sun is low and the tents are shaded.
    for (x, z) in [
        (-21.0, -22.0),
        (0.0, -27.0),
        (18.0, -27.0),
        (0.0, 20.0),
        (28.0, 22.0),
        (-40.0, 0.0),
    ] {
        p.light(v(x, 3.2, z), Color::srgb(1.0, 0.8, 0.55), 380_000.0);
    }
    // The hotel is the venue; it says so, loudly.
    p.sign(
        v(-48.85, 12.6, 0.0),
        Face::PosX.yaw(),
        11.0,
        Sign::OffsiteWelcome,
    );
}

fn buffet(p: &mut Plan) {
    // Long table with a skirt to the grass; the cloth is lifted at one spot: hiding spot 1.
    p.boxed(v(-21.0, 0.48, -24.0), v(16.0, 0.96, 1.4), Mat::White, true);
    p.boxed(
        v(-21.0, 0.98, -24.0),
        v(16.2, 0.04, 1.6),
        Mat::Fabric0,
        false,
    );
    p.boxed(v(-24.0, 0.3, -23.25), v(1.4, 0.6, 0.06), Mat::Dark, false);
    for k in 0..7 {
        let x = -27.5 + k as f32 * 2.0;
        p.prim(
            PrimKind::Cylinder,
            v(x, 1.03, -24.0),
            v(1.0, 0.04, 0.7),
            Mat::Chrome,
        );
        p.prim(
            PrimKind::Sphere,
            v(x, 1.1, -24.0),
            v(0.35, 0.12, 0.35),
            if k % 2 == 0 { Mat::Yellow } else { Mat::Red },
        );
    }
    // The last good canape, under a silver cloche at the east end.
    p.prim(
        PrimKind::Cylinder,
        v(-13.6, 1.03, -24.0),
        v(0.9, 0.04, 0.9),
        Mat::Chrome,
    );
    p.prim(
        PrimKind::Sphere,
        v(-13.6, 1.1, -24.0),
        v(0.8, 0.55, 0.8),
        Mat::Chrome,
    );
    post_sign(p, -21.0, -25.6, 2.7, 4.0, 0.0, Sign::TbBuffet);
}

fn vip_tent(p: &mut Plan) {
    for x in [-6.0, 6.0] {
        p.boxed(v(x, 1.5, -29.5), v(0.3, 3.0, 9.0), Mat::White, true);
    }
    for x in [-4.0, 4.0] {
        p.boxed(v(x, 1.5, -25.0), v(4.0, 3.0, 0.3), Mat::White, true);
    }
    p.boxed(v(0.0, 3.05, -29.5), v(12.4, 0.1, 9.4), Mat::Fabric2, false);
    p.boxed(v(0.0, 2.75, -25.0), v(4.0, 0.5, 0.3), Mat::White, false);
    p.sign(v(0.0, 3.6, -24.8), 0.0, 3.6, Sign::TbVip);
    p.prop(&LOUNGE_SOFA, v(-3.0, 0.0, -32.5), 0.0, true);
    p.prop(&LOUNGE_SOFA, v(3.0, 0.0, -32.5), 0.0, true);
    p.boxed(v(0.0, 0.4, -31.5), v(1.2, 0.8, 0.6), Mat::Gold, true);
}

fn open_bar(p: &mut Plan) {
    p.boxed(v(18.0, 0.55, -25.0), v(12.0, 1.1, 1.2), Mat::Wood, true);
    p.boxed(v(18.0, 1.12, -25.0), v(12.2, 0.06, 1.4), Mat::Marble, false);
    p.boxed(v(18.0, 1.3, -32.6), v(14.0, 2.6, 0.6), Mat::Wood, true);
    for k in 0..12 {
        p.prim(
            PrimKind::Cylinder,
            v(12.0 + k as f32 * 1.1, 2.9, -32.4),
            v(0.18, 0.5, 0.18),
            [Mat::NeonYellow, Mat::Glass, Mat::LedGreen][k % 3],
        );
    }
    for k in 0..5 {
        p.prop(&STOOL_BAR, v(13.0 + k as f32 * 2.5, 0.0, -23.6), 0.0, false);
    }
    p.sign(v(18.0, 3.4, -32.25), 0.0, 4.0, Sign::OffsiteBar);
    plant(p, 28.8, -20.6);
}

fn quiet_corner(p: &mut Plan) {
    // Hedges close the north-east corner; the way in is a gap on its west side.
    hedge(p, 39.5, -21.5, 13.0, 1.0);
    hedge(p, 33.0, -30.0, 1.0, 8.0);
    p.prop(&BENCH_CUSHION, v(41.0, 0.0, -31.5), 0.0, true);
    post_sign(p, 39.0, -20.6, 2.5, 3.2, 0.0, Sign::TbQuiet);
}

fn trust_fall(p: &mut Plan) {
    p.boxed(v(0.0, 0.6, -6.0), v(3.0, 1.2, 3.0), Mat::Wood, true);
    p.boxed(v(0.0, 1.25, -6.0), v(3.2, 0.1, 3.2), Mat::Trim, false);
    p.slab(v(0.0, 0.08, -3.0), 0.0, v(4.0, 0.16, 3.0), Mat::Fabric1);
    post_sign(p, 3.4, -8.2, 2.4, 3.0, 0.0, Sign::TbTrustFall);
    // The icebreaker circle: chairs nobody wants to sit in.
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0;
        p.slab(
            v(
                ICEBREAKER.0 + 3.0 * a.cos(),
                0.45,
                ICEBREAKER.1 + 3.0 * a.sin(),
            ),
            -a,
            v(0.5, 0.9, 0.5),
            Mat::Fabric2,
        );
    }
    post_sign(
        p,
        ICEBREAKER.0,
        ICEBREAKER.1 - 4.0,
        2.2,
        3.0,
        0.0,
        Sign::TbIcebreaker,
    );
    plant(p, -12.8, -4.6);
}

fn group_photo(p: &mut Plan) {
    p.boxed(v(-23.0, 1.4, 20.0), v(10.0, 2.8, 0.3), Mat::White, true);
    p.sign(v(-23.0, 1.7, 19.8), PI, 8.0, Sign::TbPhoto);
    for k in 0..3 {
        p.slab(
            v(-23.0, 0.08 + k as f32 * 0.1, 18.8 - k as f32 * 0.7),
            0.0,
            v(9.0, 0.16 + k as f32 * 0.2, 0.7),
            Mat::Wood,
        );
    }
    // Camera on a tripod, aimed at the backdrop.
    for a in [0.0, TAU / 3.0, 2.0 * TAU / 3.0] {
        p.slab(
            v(-23.0 + 0.3 * a.cos(), 0.7, 11.0 + 0.3 * a.sin()),
            a,
            v(0.05, 1.4, 0.05),
            Mat::Dark,
        );
    }
    p.boxed(v(-23.0, 1.5, 11.0), v(0.5, 0.35, 0.3), Mat::Dark, false);
}

fn toilets(p: &mut Plan) {
    for (k, x) in [-42.0, -39.5, -37.0].into_iter().enumerate() {
        p.boxed(v(x, 1.2, 30.5), v(2.0, 2.4, 2.0), Mat::Fabric1, true);
        p.boxed(v(x, 1.1, 29.48), v(1.0, 2.0, 0.04), Mat::Door, false);
        p.boxed(
            v(x, 2.0, 29.46),
            v(0.3, 0.15, 0.02),
            if k == 1 { Mat::LedGreen } else { Mat::LedRed },
            false,
        );
    }
    // The queue: stanchions and a rope, longer than the toilets.
    for k in 0..6 {
        let x = -42.0 + k as f32 * 1.6;
        p.boxed(v(x, 0.5, 26.6), v(0.08, 1.0, 0.08), Mat::Chrome, false);
    }
    p.boxed(v(-38.0, 0.9, 26.6), v(8.0, 0.04, 0.04), Mat::Red, false);
    // A privacy screen in front of the queue: stand behind it and you are just waiting.
    p.boxed(v(-38.0, 0.95, 24.4), v(6.0, 1.9, 0.1), Mat::Fabric1, true);
    post_sign(p, -33.0, 28.0, 2.4, 3.0, PI, Sign::TbToilets);
    plant(p, -33.8, 10.4);
}

fn swag_table(p: &mut Plan) {
    p.boxed(v(0.0, 0.45, 23.0), v(8.0, 0.9, 1.2), Mat::White, true);
    for k in 0..6 {
        let x = -3.0 + k as f32 * 1.2;
        p.boxed(
            v(x, 1.1, 23.0),
            v(0.8, 0.4, 0.6),
            [Mat::Cardboard, Mat::BeanA, Mat::BeanB][k % 3],
            false,
        );
    }
    post_sign(p, 0.0, 24.4, 2.6, 3.4, PI, Sign::TbSwag);
    // Roll-up banners: tall, thin, perfect to stand behind.
    for (x, z) in [(-9.0, 16.8), (13.0, 9.0)] {
        p.boxed(v(x, 1.05, z), v(1.8, 2.1, 0.12), Mat::Dark, true);
        p.sign(v(x, 1.15, z - 0.08), PI, 1.7, Sign::TbBanner);
    }
}

fn karaoke(p: &mut Plan) {
    p.slab(v(28.0, 0.12, 25.0), 0.0, v(10.0, 0.24, 6.0), Mat::Trim);
    p.boxed(v(28.0, 2.2, 28.5), v(5.0, 2.4, 0.2), Mat::Dark, true);
    p.sign(v(28.0, 2.2, 28.38), PI, 4.6, Sign::TbKaraoke);
    for x in [22.5, 33.5] {
        p.boxed(v(x, 1.0, 27.5), v(1.0, 2.0, 1.0), Mat::Dark, true);
        p.prim(
            PrimKind::Cylinder,
            v(x, 1.3, 26.98),
            v(0.6, 0.04, 0.6),
            Mat::Metal,
        );
    }
    p.boxed(v(28.0, 0.8, 24.0), v(0.05, 1.6, 0.05), Mat::Metal, false);
    p.prim(
        PrimKind::Sphere,
        v(28.0, 1.65, 24.0),
        v(0.14, 0.2, 0.14),
        Mat::Dark,
    );
    plant(p, 20.0, 15.2);
}

fn exit_gate(p: &mut Plan) {
    for z in [-2.5, 2.5] {
        p.boxed(v(45.0, 1.6, z), v(0.4, 3.2, 0.4), Mat::Wood, false);
    }
    p.boxed(v(45.0, 3.3, 0.0), v(0.4, 0.3, 5.4), Mat::Wood, false);
    p.sign(v(44.75, 3.9, 0.0), -FRAC_PI_2, 3.6, Sign::TbTaxi);
    // A taxi waits beyond the hedge.
    p.boxed(v(50.0, 0.7, 0.0), v(2.0, 1.0, 4.2), Mat::Yellow, false);
    p.boxed(v(50.0, 1.4, 0.3), v(1.8, 0.6, 2.2), Mat::Glass, false);
}

fn trees(p: &mut Plan) {
    for (x, z) in [
        (-30.0, -38.0),
        (-6.0, -39.0),
        (22.0, -38.5),
        (40.0, 38.0),
        (8.0, 39.0),
        (-20.0, 38.5),
        (52.0, -20.0),
        (52.0, 22.0),
    ] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 2.0, z),
            v(0.6, 4.0, 0.6),
            Mat::Bark,
        );
        p.prim(PrimKind::Sphere, v(x, 5.2, z), v(5.0, 4.4, 5.0), Mat::Leaf);
        p.prim(
            PrimKind::Sphere,
            v(x + 1.4, 4.4, z - 0.8),
            v(3.4, 3.0, 3.4),
            Mat::LeafDark,
        );
        p.prim(
            PrimKind::Sphere,
            v(x - 1.2, 6.2, z + 0.9),
            v(3.2, 2.8, 3.2),
            Mat::Leaf,
        );
    }
}

pub fn build() -> FloorLayout {
    let mut p = Plan::default();
    p.fp.pod(
        rounded_rect(pt(0.0, 0.0), 46.0, 34.0, 3.0, 0.0),
        Mat::LeafDark,
    );
    p.route = vec![pt(START.0, START.1), pt(0.0, 0.0), pt(EXIT.0, EXIT.1)];
    p.build_walls();
    hotel(&mut p);
    buffet(&mut p);
    vip_tent(&mut p);
    open_bar(&mut p);
    quiet_corner(&mut p);
    trust_fall(&mut p);
    group_photo(&mut p);
    toilets(&mut p);
    swag_table(&mut p);
    karaoke(&mut p);
    exit_gate(&mut p);
    trees(&mut p);
    garden(&mut p);
    for z in [-17.5, 0.5, 19.5] {
        string_lights(&mut p, -34.0, 34.0, z);
    }
    for (_, (x, z)) in HIDES {
        hide_ring(&mut p, x, z);
    }
    let at = |p: &mut Plan, name: &'static str, x: f32, z: f32, look: (f32, f32)| {
        let a = v(x, 0.0, z);
        p.anchor(name, a, yaw_towards(a, v(look.0, 0.0, look.1)));
    };
    for name in ["player_spawn", "graveyard", "tb_start"] {
        at(&mut p, name, START.0, START.1, (0.0, 0.0));
    }
    at(&mut p, "tb_exit", EXIT.0, EXIT.1, (50.0, 0.0));
    at(
        &mut p,
        "tb_icebreaker",
        ICEBREAKER.0,
        ICEBREAKER.1,
        (0.0, 6.0),
    );
    for (name, (x, z)) in OBJECTIVES.into_iter().chain(HIDES).chain(SNACKS) {
        at(&mut p, name, x, z, (0.0, 0.0));
    }
    for (names, pts) in ROUTES {
        for (k, (name, (x, z))) in names.iter().zip(pts.iter()).enumerate() {
            let (nx, nz) = pts[(k + 1) % pts.len()];
            at(&mut p, name, *x, *z, (nx, nz));
        }
    }
    assemble(ZONE, 1, "Mandatory Fun: Teambuilding", true, p, theme())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floors::testgrid::Grid;
    use combat_core::{Vec3 as CVec3, line_of_sight};

    fn c(p: (f32, f32)) -> CVec3 {
        CVec3::new(p.0, 0.0, p.1)
    }

    #[test]
    fn every_anchor_is_free_inside_the_outline_and_reachable_from_the_start() {
        let l = build();
        let start = l.pos("tb_start");
        let g = Grid::from(&l, start);
        for (name, p, _) in &l.anchors {
            assert!(l.footprint.contains(Vec2::new(p.x, p.z)), "{name} outside");
            assert!(g.free_anchor(&l, *p), "{name} inside a blocker");
            assert!(g.at_anchor(&l, *p).is_finite(), "{name} unreachable");
        }
    }

    /// Watchers walk straight lines through the sim, which knows no walls: every leg must be
    /// clear of blockers a body width either side.
    #[test]
    fn every_patrol_leg_is_clear() {
        let l = build();
        for (names, pts) in ROUTES {
            for k in 0..pts.len() {
                let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
                let d = Vec2::new(b.0 - a.0, b.1 - a.1).normalize_or_zero();
                let side = Vec2::new(-d.y, d.x) * 0.45;
                for s in [-1.0, 0.0, 1.0] {
                    let o = side * s;
                    let (p0, p1) = (
                        CVec3::new(a.0 + o.x, 0.5, a.1 + o.y),
                        CVec3::new(b.0 + o.x, 0.5, b.1 + o.y),
                    );
                    assert!(line_of_sight(p0, p1, &l.blockers), "{}: leg {k}", names[0]);
                }
            }
        }
    }

    #[test]
    fn hiding_spots_sit_next_to_cover() {
        let l = build();
        for (name, (x, z)) in HIDES {
            let near = l.blockers.iter().any(|b| {
                let dx = (b.min.x - x).max(x - b.max.x).max(0.0);
                let dz = (b.min.z - z).max(z - b.max.z).max(0.0);
                (dx * dx + dz * dz).sqrt() < 1.5 && b.max.y > 0.5
            });
            assert!(near, "{name} has nothing to hide behind");
        }
        // The objectives are spread: a run crosses the party.
        let spots: Vec<CVec3> = OBJECTIVES.iter().map(|(_, p)| c(*p)).collect();
        for (i, a) in spots.iter().enumerate() {
            for b in &spots[i + 1..] {
                assert!(combat_core::distance(*a, *b) > 15.0);
            }
        }
    }
}
