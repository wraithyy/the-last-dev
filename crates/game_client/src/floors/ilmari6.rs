//! Ilmari Wing 6, the Boardroom in the Sky (docs/ILMARI_1_0.md D.4): the New HQ crane that never
//! moved, now reaching the Tower's roof beam, and the penthouse it hoists you to, on the plan of
//! Wings 1-5 (corridor, hall, corridor, pod and three cells). ilmari6/1 is the Jib Walk (a
//! hundred metres of boom over a night view of the campus in its red dusk, rails, hard hat racks,
//! a pigeon with a lanyard), ilmari6/2 the Executive Floor (open plan for ten, eleven people, a
//! masseuse chair, a "wellness" room used for one-to-ones with a closed door) and ilmari6/3 the
//! Boardroom (a table the length of a runway, eight chairs, a fruit bowl, a glass wall with the
//! whole campus on it, five desks round the edge, four inbox terminals, the floor grid of
//! Right-Sizing and the private lift behind the chair). No point lights (the 0.9 rule).

use super::datacenter::dress::*;
use super::ilmari3::{arrival_kit, casing};
use super::ilmari5::{archive, auditorium, banner, queue, row, workstation};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::models::{
    self, APPLE, BANANA, BARRIER, BOX_LARGE, CONE_ROAD, ORANGE, PEAR, SIGN_WARNING,
};
use super::props::{sad_plant, water_cooler};
use super::themes::{pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

const POD1: (f32, f32, f32) = (46.0, 18.0, 16.0);
const POD2: (f32, f32, f32) = (46.0, 20.0, 18.0);
/// The Boardroom: room for the table, five desks and a fight with a CIO.
pub const POD3: (f32, f32, f32) = (46.0, 24.0, 21.0);
/// The five desks of the Boardroom, offsets from the pod centre (the CIO's `Earnings Call` cover, D9).
pub const DESKS: [(f32, f32); 5] = [
    (-14.0, -15.0),
    (-14.0, 15.0),
    (2.0, -17.5),
    (2.0, 17.5),
    (-20.0, 0.0),
];
/// The four inbox terminals of the Boardroom, offsets from the pod centre and the way each faces
/// (the sign of its side, as the Lab's kiosks: -1 on the north wall, its screen looks toward +Z). The CIO's `New Message` is cleared at any one of them.
pub const INBOXES: [(f32, f32, f32); 4] = [
    (-20.0, -19.2, -1.0),
    (-20.0, 19.2, 1.0),
    (20.0, 19.2, 1.0),
    (20.0, -19.2, -1.0),
];
/// The Penthouse Lift: offset from the pod centre.
pub const LIFT: (f32, f32) = (21.0, -3.0);

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => (
            "ilmari6/1",
            "The Boardroom in the Sky: The Jib Walk",
            jib_walk(&mut p),
        ),
        2 => (
            "ilmari6/2",
            "The Boardroom in the Sky: The Executive Floor",
            executive_floor(&mut p),
        ),
        _ => (
            "ilmari6/3",
            "The Boardroom in the Sky: The Boardroom",
            boardroom(&mut p),
        ),
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, fog, ambient, floor, accent) = match level {
        // Jib walk: a steel boom at red dusk, a hazard-yellow railing and a city that is a screen.
        1 => (
            c(0.55, 0.5, 0.46),
            c(0.34, 0.16, 0.14),
            c(1.0, 0.78, 0.66),
            [0.52, 0.5, 0.48],
            LinearRgba::rgb(3.4, 1.6, 0.3),
        ),
        // Executive floor: open-plan beige under a masseuse's lamp.
        2 => (
            c(0.72, 0.68, 0.62),
            c(0.28, 0.16, 0.14),
            c(1.0, 0.86, 0.74),
            [0.76, 0.72, 0.66],
            LinearRgba::rgb(3.0, 1.8, 0.6),
        ),
        // Boardroom: oak, glass and the colour of a very old promise.
        _ => (
            c(0.6, 0.46, 0.34),
            c(0.3, 0.14, 0.12),
            c(1.0, 0.8, 0.66),
            [0.5, 0.36, 0.26],
            LinearRgba::rgb(3.2, 1.2, 0.4),
        ),
    };
    Theme {
        wall,
        trim: c(0.1, 0.07, 0.06),
        accent,
        fabric: [c(0.5, 0.3, 0.22), c(0.3, 0.3, 0.4), c(0.4, 0.34, 0.28)],
        floor: FloorKind::Tile(floor),
        fog,
        ambient,
        ambient_brightness: 820.0,
        sun: c(1.0, 0.66, 0.5),
        sun_illuminance: 2600.0,
        fog_range: (26.0, 100.0),
    }
}

/// The night view: skyline windows along both long walls of the hall.
fn windows(p: &mut Plan, z: f32, xs: &[f32]) {
    for &x in xs {
        p.window_near(x, z, Vec2::new(3.4, 1.8), Tex::Skyline, Mat::Trim);
    }
}

/// A hard hat on a peg: a hemisphere, a brim and the sticker with the head it is aligned with.
fn hard_hat(p: &mut Plan, x: f32, y: f32, z: f32) {
    p.prim(
        PrimKind::Sphere,
        v(x, y + 0.08, z),
        v(0.34, 0.22, 0.34),
        Mat::Yellow,
    );
    p.boxed(v(x, y, z), v(0.42, 0.03, 0.42), Mat::Yellow, false);
}

/// A rack of hard hats along a wall, `n` pegs at 0.5 m.
fn hat_rack(p: &mut Plan, x: f32, z: f32, face: Face, n: usize) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(
            v(0.0, 1.5, 0.05),
            v(n as f32 * 0.5 + 0.2, 0.2, 0.06),
            Mat::Wood,
            false,
        );
        for k in 0..n {
            hard_hat(p, (k as f32 - (n as f32 - 1.0) / 2.0) * 0.5, 1.55, 0.25);
        }
    });
}

/// An inbox terminal: a lectern with a screen that never reaches zero, a glowing envelope and the
/// sign. Any class may use it (`PropKind::Inbox`).
fn inbox_terminal(p: &mut Plan, key: &'static str, x: f32, z: f32, face: f32) {
    p.boxed(v(x, 0.55, z), v(1.2, 1.1, 0.7), Mat::Dark, true);
    p.boxed(
        v(x, 1.3, z - face * 0.2),
        v(1.0, 0.6, 0.05),
        Mat::ScreenGlow,
        false,
    );
    // The envelope: a flat slab with a red flap, hovering over the lectern.
    p.boxed(
        v(x, 1.95, z - face * 0.1),
        v(0.5, 0.32, 0.04),
        Mat::Gold,
        false,
    );
    p.boxed(
        v(x, 2.04, z - face * 0.12),
        v(0.5, 0.05, 0.05),
        Mat::LedRed,
        false,
    );
    p.boxed(v(x, 2.8, z), v(0.14, 1.6, 0.14), Mat::Metal, false);
    p.sign(
        v(x, 3.3, z - face * 0.1),
        if face > 0.0 { PI } else { 0.0 },
        2.2,
        Sign::IlInbox,
    );
    p.interactable(key, v(x, 1.2, z), v(x, 0.0, z - face * 2.2));
}

/// The nine tiles of Right-Sizing, drawn as thin lines on the floor under the CIO: the floor plan
/// the CIO is about to optimise. Tiles are 6.5 m, centred on the boss.
fn tile_grid(p: &mut Plan, cx: f32, cz: f32) {
    for k in [-9.75_f32, -3.25, 3.25, 9.75] {
        p.boxed(
            v(cx + k, 0.05, cz),
            v(0.08, 0.04, 19.5),
            Mat::NeonYellow,
            false,
        );
        p.boxed(
            v(cx, 0.055, cz + k),
            v(19.5, 0.04, 0.08),
            Mat::NeonYellow,
            false,
        );
    }
}

/// A length of crane lattice standing up the wall: two legs, braces, a warning light on top.
fn lattice(p: &mut Plan, x: f32, z: f32, h: f32) {
    for dx in [-0.5_f32, 0.5] {
        p.boxed(v(x + dx, h / 2.0, z), v(0.12, h, 0.12), Mat::Hazard, false);
    }
    for k in 0..(h as usize) {
        p.boxed(
            v(x, 0.5 + k as f32, z),
            v(1.0, 0.07, 0.07),
            Mat::Hazard,
            false,
        );
    }
    p.boxed(v(x, h + 0.1, z), v(0.3, 0.2, 0.3), Mat::BlinkRed, false);
    p.block(v(x, 1.0, z), v(1.2, 2.0, 0.4));
}

/// The jib's own machinery in the counterweight deck: a hook block hanging on its falls over the
/// hub (above head height, so nobody walks into it), the hoist drum it winds on, a sheave wheel
/// in the ceiling run and the operator's cab with the one chair and the one lever. Visual only
/// except for the cab and the drum (blocks).
fn crane_props(p: &mut Plan, cx: f32, cz: f32) {
    let top = p.top();
    let hook = v(cx + 2.0, 2.45, cz - 2.0);
    for dx in [-0.22_f32, 0.22] {
        p.boxed(
            v(hook.x + dx, (hook.y + top) / 2.0, hook.z),
            v(0.04, top - hook.y, 0.04),
            Mat::Metal,
            false,
        );
    }
    p.boxed(
        v(hook.x, top - 0.12, hook.z),
        v(0.9, 0.24, 0.4),
        Mat::Dark,
        false,
    );
    p.prim(
        PrimKind::Cylinder,
        v(hook.x, top - 0.2, hook.z),
        v(0.5, 0.12, 0.5),
        Mat::Metal,
    );
    p.boxed(hook, v(0.7, 0.3, 0.4), Mat::Yellow, false);
    p.boxed(
        v(hook.x, hook.y - 0.3, hook.z),
        v(0.1, 0.3, 0.1),
        Mat::Metal,
        false,
    );
    p.prim(
        PrimKind::Cone,
        v(hook.x, hook.y - 0.55, hook.z),
        v(0.3, 0.3, 0.3),
        Mat::Hot,
    );
    // The hoist drum on its stand, wound with rope (a darker ring between two flanges).
    let drum = v(cx + 12.0, 0.0, cz + 10.0);
    p.boxed(
        v(drum.x, 0.25, drum.z),
        v(3.2, 0.5, 1.6),
        Mat::Concrete,
        true,
    );
    p.prim(
        PrimKind::Cylinder,
        v(drum.x, 1.1, drum.z),
        v(1.8, 1.4, 1.8),
        Mat::Dark,
    );
    for dy in [0.4_f32, 1.8] {
        p.prim(
            PrimKind::Cylinder,
            v(drum.x, dy, drum.z),
            v(2.3, 0.12, 2.3),
            Mat::Hazard,
        );
    }
    // The cab: a glass-fronted box on the far corner, a chair and a lever behind the glass.
    let cab = v(cx + 14.0, 0.0, cz - 12.0);
    p.boxed(
        v(cab.x, 1.1, cab.z - 1.1),
        v(2.6, 2.2, 0.1),
        Mat::Dark,
        true,
    );
    p.boxed(
        v(cab.x - 1.3, 1.1, cab.z),
        v(0.1, 2.2, 2.2),
        Mat::Dark,
        true,
    );
    p.boxed(
        v(cab.x + 1.3, 1.1, cab.z),
        v(0.1, 2.2, 2.2),
        Mat::Dark,
        true,
    );
    p.boxed(v(cab.x, 2.25, cab.z), v(2.8, 0.1, 2.4), Mat::Yellow, false);
    p.boxed(
        v(cab.x, 1.55, cab.z + 1.1),
        v(2.5, 1.2, 0.05),
        Mat::Glass,
        true,
    );
    p.boxed(
        v(cab.x, 0.35, cab.z + 1.1),
        v(2.5, 0.7, 0.08),
        Mat::Yellow,
        true,
    );
    p.prop(&CHAIR_DESK, v(cab.x, 0.0, cab.z - 0.2), PI, false);
    p.boxed(
        v(cab.x + 0.7, 0.7, cab.z + 0.4),
        v(0.06, 0.5, 0.06),
        Mat::Metal,
        false,
    );
    p.boxed(
        v(cab.x + 0.7, 0.98, cab.z + 0.4),
        v(0.12, 0.12, 0.12),
        Mat::LedRed,
        false,
    );
    p.sign(v(cab.x, 2.65, cab.z + 1.2), 0.0, 1.6, Sign::IlCraneNote);
}

fn casing6(p: &mut Plan, pod: (f32, f32, f32), glow: Mat) {
    casing(p, pod, glow);
}

// ------------------------------------------------------------------ ilmari6/1: the jib walk

fn jib_walk(p: &mut Plan) -> Theme {
    casing6(p, POD1, Mat::NeonYellow);
    windows(p, -10.9, &[-30.0, -20.0, -10.0, 0.0, 10.0]);
    windows(p, 10.9, &[-30.0, -20.0, -10.0, 0.0, 10.0]);
    p.sign_near(-33.9, 0.0, 2.7, 4.2, Sign::IlW6Door);
    p.sign_near(-25.0, -10.9, 2.7, 2.8, Sign::IlJibRules);
    p.sign_near(-5.0, -10.9, 2.7, 2.8, Sign::IlCraneNote);
    p.sign_near(-15.0, 10.9, 2.7, 2.8, Sign::IlPigeon);
    p.sign_near(5.0, 10.9, 2.7, 2.8, Sign::IlJibRules);
    // The boom: lattice legs every few metres, a double handrail along the walkway and cones
    // that mark the places where the walkway is, officially, absent.
    for x in [-28.0, -18.0, -8.0, 2.0, 12.0] {
        lattice(p, x, -9.4, 5.0);
        lattice(p, x + 5.0, 9.4, 5.0);
    }
    for z in [-3.2, 3.2] {
        p.boxed(v(-8.0, 0.55, z), v(52.0, 0.06, 0.06), Mat::Yellow, false);
        p.boxed(v(-8.0, 1.05, z), v(52.0, 0.06, 0.06), Mat::Yellow, false);
        for k in 0..14 {
            p.boxed(
                v(-33.0 + k as f32 * 4.0, 0.55, z),
                v(0.06, 1.1, 0.06),
                Mat::Metal,
                false,
            );
        }
    }
    for x in [-24.0, -4.0, 10.0] {
        models::put(p, &CONE_ROAD, v(x, 0.0, -5.4), 0.0, 9.0, false);
        models::put(p, &CONE_ROAD, v(x + 1.4, 0.0, 5.4), 0.0, 9.0, false);
    }
    for x in [-26.0, -12.0, 4.0] {
        hat_rack(p, x, -10.2, Face::PosZ, 4);
    }
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), true);
    arrival_kit(p, Sign::IlW6Back);
    // A pigeon on the rail, promoted: grey ball, smaller ball, orange beak.
    for (x, z) in [(-14.0, 3.2), (6.0, -3.2)] {
        p.prim(
            PrimKind::Sphere,
            v(x, 1.25, z),
            v(0.46, 0.34, 0.32),
            Mat::Metal,
        );
        p.prim(
            PrimKind::Sphere,
            v(x + 0.26, 1.45, z),
            v(0.2, 0.2, 0.2),
            Mat::Chrome,
        );
        p.boxed(v(x + 0.4, 1.44, z), v(0.1, 0.05, 0.05), Mat::Hot, false);
    }
    // Cells: the portacabin (a site office with a calendar), the materials yard and the canteen.
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlJibRules);
    p.prop(&DESK, v(-20.0, 0.0, -31.4), 0.0, true);
    p.prop(&CHAIR_DESK, v(-20.0, 0.0, -30.2), PI, false);
    models::put(p, &BOX_LARGE, v(-25.0, 0.0, -30.0), 0.2, 1.4, true);
    models::put(p, &BARRIER, v(-14.5, 0.0, -28.5), FRAC_PI_2, 2.4, true);
    p.sign_near(6.0, 19.1, 2.6, 2.4, Sign::IlCraneNote);
    p.prop(&TABLE_ROUND, v(6.0, 0.0, 29.0), 0.0, true);
    for k in 0..3 {
        let a = k as f32 * 2.0 * PI / 3.0;
        p.prop(
            &STOOL_BAR,
            v(6.0 + 1.5 * a.cos(), 0.0, 29.0 + 1.5 * a.sin()),
            a,
            false,
        );
    }
    water_cooler(p, 12.0, 32.0);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlPigeon);
    models::put(p, &BOX_LARGE, v(6.0, 0.0, -30.0), 0.4, 1.4, true);
    models::put(p, &BOX_LARGE, v(9.0, 0.0, -31.0), -0.2, 1.4, true);
    models::put(p, &SIGN_WARNING, v(12.5, 0.0, -29.0), 0.3, 1.3, false);
    // The foot of the jib, the hub of the pod: the counterweight deck. A big concrete block on
    // a plinth, a winch drum and the wall of the whole campus in its dusk.
    let (cx, cz) = (POD1.0, 0.0);
    for k in 0..3 {
        p.boxed(
            v(cx + 8.0, 0.7 + k as f32 * 1.4, cz - 11.0),
            v(3.4, 1.3, 3.4),
            Mat::Concrete,
            k == 0,
        );
    }
    p.prim(
        PrimKind::Cylinder,
        v(cx - 8.0, 1.0, cz + 11.0),
        v(3.0, 2.0, 3.0),
        Mat::Metal,
    );
    p.prim(
        PrimKind::Cylinder,
        v(cx - 8.0, 2.1, cz + 11.0),
        v(3.4, 0.2, 3.4),
        Mat::Hazard,
    );
    p.block(v(cx - 8.0, 1.0, cz + 11.0), v(3.0, 2.0, 3.0));
    crane_props(p, cx, cz);
    floor_ring(p, cx, cz, 5.0, 24, Mat::NeonYellow, 0.1);
    floor_ring(p, cx, cz, 9.5, 36, Mat::Hazard, 0.1);
    for x in [-14.0, 0.0, 14.0] {
        p.window_near(cx + x, -16.2, Vec2::new(4.0, 2.0), Tex::Skyline, Mat::Trim);
        p.window_near(cx + x, 16.2, Vec2::new(4.0, 2.0), Tex::Skyline, Mat::Trim);
    }
    p.sign(v(cx - 17.7, 3.0, 0.0), FRAC_PI_2, 4.0, Sign::IlCampusView);
    p.sign(v(cx + 17.7, 3.0, -4.0), -FRAC_PI_2, 3.2, Sign::IlCraneNote);
    p.sign(v(cx + 17.7, 3.0, 4.0), -FRAC_PI_2, 3.2, Sign::IlJibRules);
    strips(p, Mat::LightPanel, Mat::NeonYellow, 0.0, 0.1);
    jib_cast(p);
    theme(1)
}

fn jib_cast(p: &mut Plan) {
    super::ilmari3::cast(
        p,
        POD1,
        [
            ("door:campus_occupied", Sign::IlW6Back),
            ("door:ilmari6/2", Sign::IlW6ToExec),
        ],
    );
}

// ------------------------------------------------------------------ ilmari6/2: the executive floor

/// A glass room: three glass walls on a frame, a closed door and a sign over it.
fn glass_room(p: &mut Plan, x: f32, z: f32, w: f32, d: f32, sign: Sign) {
    p.boxed(v(x, 1.4, z - d / 2.0), v(w, 2.8, 0.08), Mat::Frosted, true);
    p.boxed(v(x - w / 2.0, 1.4, z), v(0.08, 2.8, d), Mat::Glass, true);
    p.boxed(v(x + w / 2.0, 1.4, z), v(0.08, 2.8, d), Mat::Glass, true);
    p.boxed(v(x, 2.84, z), v(w + 0.1, 0.08, d + 0.1), Mat::Trim, false);
    p.sign(v(x, 3.3, z + d / 2.0), 0.0, 2.4, sign);
}

fn executive_floor(p: &mut Plan) -> Theme {
    casing6(p, POD2, Mat::NeonYellow);
    windows(p, -10.9, &[-30.0, -22.0, -14.0, -6.0, 2.0, 10.0]);
    p.sign_near(-33.9, 0.0, 2.7, 4.0, Sign::IlW6ToExec);
    // The hall: open plan for ten, eleven people, a masseuse chair booked until 2031 and a
    // wellness room with the door shut.
    row(p, -28.0, 8.0, -6.6, 0.0, 5);
    row(p, -28.0, 8.0, 6.6, PI, 5);
    workstation(p, 12.0, -6.6, 0.0, Mat::LedRed);
    p.sign_near(-22.0, 10.9, 2.7, 3.2, Sign::IlOpenPlanTen);
    p.sign_near(-4.0, 10.9, 2.7, 3.2, Sign::IlMassage);
    p.sign_near(10.0, 10.9, 2.7, 3.2, Sign::IlWellnessRoom);
    p.prop(&LOUNGE_CHAIR_RELAX, v(-8.0, 0.0, 9.2), PI, false);
    p.prop(&SIDE_TABLE, v(-6.4, 0.0, 9.4), 0.0, false);
    p.prop(&POTTED_PLANT, v(-12.0, 0.0, 9.6), 0.0, false);
    p.prop(&POTTED_PLANT, v(14.0, 0.0, 9.6), 0.0, false);
    glass_room(p, 4.0, 5.0, 4.6, 4.0, Sign::IlWellnessRoom);
    for x in [-26.0, -10.0, 6.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 20.0), Mat::Accent, false);
    }
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), false);
    arrival_kit(p, Sign::IlW6Back);
    // Cells: the fruit kitchen, a layoff room (one table, two chairs, a box of tissues) and an
    // archive of reorganisation charts.
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlFruitBowl);
    p.prop(&KITCHEN_CABINET, v(-24.0, 0.0, -32.4), 0.0, true);
    p.prop(&KITCHEN_BAR, v(-21.4, 0.0, -32.4), 0.0, true);
    p.prop(&FRIDGE, v(-18.6, 0.0, -32.4), 0.0, true);
    models::put(p, &APPLE, v(-21.4, 0.9, -32.2), 0.0, 0.5, false);
    models::put(p, &PEAR, v(-21.0, 0.9, -32.2), 0.0, 0.5, false);
    p.sign_near(6.0, 19.1, 2.6, 2.4, Sign::IlWellnessRoom);
    p.prop(&TABLE, v(6.0, 0.0, 29.0), 0.0, true);
    p.prop(&CHAIR_MODERN, v(5.0, 0.0, 30.2), PI, false);
    p.prop(&CHAIR_MODERN, v(7.0, 0.0, 27.8), 0.0, false);
    p.boxed(v(6.4, 0.9, 29.0), v(0.3, 0.14, 0.2), Mat::White, false);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlRoadmapWall);
    archive(p, 7.0, -32.2, Face::PosZ, 2);
    archive(p, 13.0, -32.2, Face::PosZ, 1);
    sad_plant(p, 15.5, -30.0);
    // The elevator lobby: a long sofa row, a rope queue to the lift that is "being serviced" and
    // a screen of the roadmap that is the same as last quarter's, reversed.
    let (cx, cz) = (POD2.0, 0.0);
    for k in 0..3 {
        p.prop(
            &LOUNGE_SOFA_LONG,
            v(cx - 8.0 + k as f32 * 3.4, 0.0, cz + 14.0),
            PI,
            true,
        );
    }
    p.prop(&COFFEE_TABLE, v(cx - 5.0, 0.0, cz + 11.0), 0.0, true);
    queue(p, cx + 6.0, cz - 12.0, 4, true);
    auditorium(p, cx - 8.0, cz - 10.0, 0.0, 2, 3);
    floor_ring(p, cx, cz, 6.0, 28, Mat::NeonYellow, 0.1);
    floor_ring(p, cx, cz, 12.0, 44, Mat::Gold, 0.08);
    for z in [-12.0, 12.0] {
        banner(p, cx + 12.0, cz + z, 0.0, 4.0, Sign::IlRoadmapWall);
    }
    p.sign(v(cx - 19.7, 3.2, 0.0), FRAC_PI_2, 4.4, Sign::IlOpenPlanTen);
    p.sign(
        v(cx + 19.7, 3.2, -4.0),
        -FRAC_PI_2,
        3.2,
        Sign::IlRoadmapWall,
    );
    p.sign(v(cx + 19.7, 3.2, 4.0), -FRAC_PI_2, 3.2, Sign::IlMassage);
    for x in [-14.0, 0.0, 14.0] {
        p.window_near(cx + x, -17.2, Vec2::new(4.0, 2.0), Tex::Skyline, Mat::Trim);
    }
    strips(p, Mat::LightPanel, Mat::NeonYellow, 0.0, 0.1);
    super::ilmari3::cast(
        p,
        POD2,
        [
            ("door:ilmari6/1", Sign::IlW6Back),
            ("door:ilmari6/3", Sign::IlW6ToBoardroom),
        ],
    );
    theme(2)
}

// ------------------------------------------------------------------ ilmari6/3: the boardroom

/// A fruit bowl: a plate and the last of the fruit.
fn fruit_bowl(p: &mut Plan, x: f32, z: f32) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.82, z),
        v(0.8, 0.08, 0.8),
        Mat::White,
    );
    for (dx, dz, m) in [
        (-0.15, 0.1, &APPLE),
        (0.2, 0.0, &ORANGE),
        (0.0, -0.2, &BANANA),
    ] {
        models::put(p, m, v(x + dx, 0.9, z + dz), dx * 6.0, 0.55, false);
    }
}

/// The contents of cleared desks: boxes with a plant and an apple on top, standing on the tiles
/// Right-Sizing takes first, and a fruit bowl at every desk (the last apple has opinions).
fn cleared_desks(p: &mut Plan, cx: f32, cz: f32) {
    for (k, (dx, dz)) in [
        (-9.0_f32, -9.0_f32),
        (9.0, 9.0),
        (-9.0, 9.0),
        (9.0, -9.0),
        (0.0, -9.0),
        (0.0, 9.0),
        (-9.0, 0.0),
        (9.0, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        let (x, z) = (cx + dx, cz + dz);
        models::put(p, &BOX_LARGE, v(x, 0.0, z), k as f32 * 0.7, 1.1, false);
        models::put(p, &APPLE, v(x + 0.1, 0.62, z), k as f32, 0.5, false);
        if k % 2 == 0 {
            models::put(
                p,
                &BOX_LARGE,
                v(x + 1.1, 0.0, z + 0.4),
                1.2 + k as f32,
                0.8,
                false,
            );
        }
    }
}

fn boardroom(p: &mut Plan) -> Theme {
    casing6(p, POD3, Mat::NeonYellow);
    windows(p, -10.9, &[-30.0, -20.0, -10.0, 0.0, 10.0]);
    windows(p, 10.9, &[-30.0, -20.0, -10.0, 0.0, 10.0]);
    p.sign_near(-33.9, 0.0, 2.7, 4.0, Sign::IlW6ToBoardroom);
    // The hall: the executive corridor, portraits of former directors (all of them the same
    // director), a fruit station and a lounge for people waiting to be told.
    for x in [-28.0, -16.0, -4.0, 8.0] {
        p.prop(&LOUNGE_SOFA, v(x, 0.0, 9.6), PI, true);
        p.prop(&SIDE_TABLE, v(x + 1.6, 0.0, 9.8), 0.0, false);
    }
    for x in [-24.0, -10.0, 4.0, 14.0] {
        p.sign_near(x, -10.9, 2.5, 2.2, Sign::IlRoadmapWall);
    }
    p.sign_near(-22.0, 10.9, 2.7, 3.0, Sign::IlRunway);
    p.sign_near(0.0, 10.9, 2.7, 3.0, Sign::IlFruitBowl);
    p.prop(&LAMP_FLOOR, v(-34.0, 0.0, 9.6), 0.0, false);
    p.prop(&LAMP_FLOOR, v(16.0, 0.0, -9.6), 0.0, false);
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), false);
    arrival_kit(p, Sign::IlW6Back);
    // Cells: the minute-takers' room, the catering pantry and the director's wardrobe.
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlRunway);
    p.prop(&TABLE, v(-22.0, 0.0, -30.0), 0.0, true);
    for k in 0..3 {
        p.prop(
            &CHAIR_MODERN,
            v(-23.0 + k as f32 * 1.0, 0.0, -28.6),
            0.0,
            false,
        );
    }
    fruit_bowl(p, -22.0, -30.0);
    p.sign_near(6.0, 19.1, 2.6, 2.4, Sign::IlFruitBowl);
    p.prop(&KITCHEN_CABINET, v(2.0, 0.0, 32.4), 0.0, true);
    p.prop(&KITCHEN_BAR, v(4.6, 0.0, 32.4), 0.0, true);
    p.prop(&FRIDGE, v(7.4, 0.0, 32.4), 0.0, true);
    p.prop(&COFFEE_MACHINE, v(4.6, 0.9, 32.4), 0.0, false);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlCampusView);
    p.prop(&COAT_RACK, v(6.0, 0.0, -32.0), 0.0, false);
    p.prop(&COAT_RACK, v(8.0, 0.0, -32.0), 0.0, false);
    p.prop(&COAT_RACK, v(10.0, 0.0, -32.0), 0.0, false);
    // The Boardroom: a table the length of a runway down the middle (north of the route's end so
    // the boss has the floor), eight chairs, the fruit bowl, the glass wall and five desks.
    let (cx, cz) = (POD3.0, 0.0);
    for k in 0..6 {
        p.prop(
            &TABLE_GLASS,
            v(cx - 1.0 + k as f32 * 1.8, 0.0, cz - 17.0),
            FRAC_PI_2,
            true,
        );
    }
    for k in 0..4 {
        let x = cx + k as f32 * 3.0;
        p.prop(&CHAIR_ROUNDED, v(x, 0.0, cz - 18.4), 0.0, false);
        p.prop(&CHAIR_ROUNDED, v(x, 0.0, cz - 15.6), PI, false);
    }
    fruit_bowl(p, cx + 4.0, cz - 17.0);
    for (k, (dx, dz)) in DESKS.into_iter().enumerate() {
        let yaw = if dz.abs() > 1.0 {
            if dz < 0.0 { PI } else { 0.0 }
        } else {
            FRAC_PI_2
        };
        p.framed(v(cx + dx, 0.0, cz + dz), yaw, |p| {
            p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
            p.prop(&SCREEN, v(0.0, 0.8, -0.1), 0.0, false);
            p.boxed(
                v(0.0, 1.05, -0.2),
                v(0.8, 0.4, 0.05),
                if k % 2 == 0 {
                    Mat::LedRed
                } else {
                    Mat::ScreenGlow
                },
                false,
            );
            p.prop(&CHAIR_DESK, v(0.0, 0.0, 1.0), PI, false);
        });
    }
    // The glass wall: the whole campus in its red dusk, floor to ceiling on the north side.
    for k in 0..7 {
        p.window_near(
            cx - 18.0 + k as f32 * 6.0,
            -20.2,
            Vec2::new(5.0, 2.6),
            Tex::Skyline,
            Mat::Trim,
        );
    }
    // The Penthouse Lift behind the head chair: a door frame, a lit button and the sign.
    let (lx, lz) = (cx + LIFT.0, cz + LIFT.1);
    p.boxed(v(lx + 1.2, 1.4, lz), v(0.4, 2.8, 3.0), Mat::Dark, true);
    p.boxed(
        v(lx + 0.95, 1.25, lz),
        v(0.06, 2.4, 1.8),
        Mat::Chrome,
        false,
    );
    p.boxed(
        v(lx + 0.9, 1.3, lz + 1.3),
        v(0.06, 0.3, 0.16),
        Mat::NeonYellow,
        false,
    );
    p.sign(v(lx + 0.88, 2.8, lz), -FRAC_PI_2, 2.4, Sign::IlLiftPlate);
    p.interactable(
        "prop:il_penthouse_lift",
        v(lx + 0.9, 1.0, lz),
        v(lx - 2.0, 0.0, lz),
    );
    floor_ring(p, cx, cz, 6.0, 28, Mat::NeonYellow, 0.12);
    floor_ring(p, cx, cz, 14.0, 48, Mat::Gold, 0.1);
    tile_grid(p, cx, cz);
    cleared_desks(p, cx, cz);
    for (k, (dx, dz, face)) in INBOXES.into_iter().enumerate() {
        let key = [
            "prop:il_inbox_1",
            "prop:il_inbox_2",
            "prop:il_inbox_3",
            "prop:il_inbox_4",
        ][k];
        inbox_terminal(p, key, cx + dx, cz + dz, face);
    }
    p.sign(v(cx - 23.7, 3.4, 0.0), FRAC_PI_2, 4.6, Sign::IlRunway);
    p.sign(v(cx + 23.7, 3.4, 6.0), -FRAC_PI_2, 3.2, Sign::IlRoadmapWall);
    p.sign(v(cx + 23.7, 3.4, 12.0), -FRAC_PI_2, 3.2, Sign::IlFruitBowl);
    p.boxed(v(cx, 4.0, 0.0), v(0.3, 0.12, 40.0), Mat::Dark, false);
    strips(p, Mat::LightPanel, Mat::NeonYellow, 0.0, 0.1);
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:ilmari6/2", (-50.0, 0.0), Sign::IlW6Back),
                (
                    "door:ilmari_core/1",
                    (POD3.0 + POD3.1, -POD3.2 * 0.5),
                    Sign::IlCoreDoor,
                ),
                (
                    "door:campus_occupied",
                    (POD3.0 + POD3.1, POD3.2 * 0.6),
                    Sign::IlW6Out,
                ),
            ],
            packs: &super::ilmari1::CELLS.map(|(x, z)| (x, z - z.signum())),
            boss: (POD3.0, 0.0),
            npc: None,
        },
    );
    theme(3)
}
