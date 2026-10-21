//! Car Park P3, where every new hire starts: 120 x 170 m of asphalt "a short walk" south of the
//! campus. Your car is in the last bay; the HR Onboarding Bot's "First Day" corner is beside it,
//! the Badge Office portacabin and the pay-and-display booth stand on the way north, and the
//! exit ramp climbs to the gate onto the campus road (`door:campus`), guarded by the Exit
//! Barrier. Level-1 clamp drones and survey ambushers work the rows.

use super::districts::{lamp, ramp_road, road, signpost, spot};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble, yaw_towards};
use super::props::bench;
use super::shape::rounded_rect;
use super::themes::{pt, route_point_near, v};
use bevy::prelude::*;
use combat_core::{Terrain, TerrainFeature};
use std::f32::consts::{FRAC_PI_2, PI};

pub const ZONE: &str = "parking_p3";
const HALF_X: f32 = 60.0;
const HALF_Z: f32 = 85.0;

const SPAWN: (f32, f32) = (0.0, 72.0);
const GRAVEYARD: (f32, f32) = (-12.0, 77.0);
/// The exit deck the gate stands on, and its ramp down to the lot.
const DECK: (f32, f32, f32, f32) = (-16.0, -85.0, 16.0, -64.0);
const DECK_H: f32 = 2.4;
const DECK_RAMP: f32 = 12.0;
/// Clamp drones in the rows, survey ambushers by the paths, one mixed pack at the ramp.
const PACKS: [(f32, f32); 6] = [
    (-34.0, 50.0),
    (30.0, 52.0),
    (-38.0, 8.0),
    (36.0, 6.0),
    (-22.0, -30.0),
    (24.0, -34.0),
];
/// Friendly cast: (anchor, position, the point it faces).
type Spot = (&'static str, (f32, f32), (f32, f32));
const CAST: [Spot; 10] = [
    // The "First Day" corner beside your car.
    ("npc_hr", (-3.5, 68.0), SPAWN),
    ("npc_dummy", (2.0, 61.5), SPAWN),
    ("npc_colleague", (-2.5, 63.5), SPAWN),
    // Your onboarding buddy, by the arrival spot, with the ticket that sends you up the hill.
    ("npc_1", (5.0, 69.0), SPAWN),
    ("npc_badge", (-30.0, 24.5), (-30.0, 34.0)),
    ("npc_attendant", (25.0, 28.0), (16.0, 28.0)),
    ("npc_vendor", (28.6, -10.5), (14.0, -10.5)),
    // Along the walk to the gate: a vibecoder at his hatchback, a consultant, a pilot passenger.
    ("npc_vibe", (9.5, 45.0), (13.0, 45.0)),
    ("npc_consultant", (-8.0, 14.0), (0.0, 14.0)),
    ("npc_pilot", (-8.0, -22.0), (0.0, -26.0)),
];
const BOSS: (f32, f32) = (9.0, -71.0);

fn theme() -> Theme {
    let c = Color::srgb;
    Theme {
        wall: c(0.62, 0.63, 0.66),
        trim: c(0.22, 0.23, 0.26),
        accent: LinearRgba::rgb(1.4, 1.1, 0.3),
        fabric: [c(0.3, 0.32, 0.36), c(0.4, 0.36, 0.3), c(0.28, 0.36, 0.4)],
        floor: FloorKind::Grass,
        // Overcast: the light in P3 is always 07:40 on a Monday in February.
        fog: c(0.66, 0.7, 0.76),
        ambient: c(0.82, 0.86, 0.94),
        ambient_brightness: 850.0,
        sun: c(0.95, 0.94, 0.9),
        sun_illuminance: 12000.0,
        fog_range: (50.0, 150.0),
    }
}

pub fn build() -> FloorLayout {
    let (x0, z0, x1, z1) = DECK;
    let mut p = Plan::default();
    p.terrain = Terrain {
        features: vec![TerrainFeature::Mesa {
            x0,
            z0,
            x1,
            z1,
            h: DECK_H,
            ramp: DECK_RAMP,
        }],
    };
    p.fp.pod(
        rounded_rect(Vec2::ZERO, HALF_X, HALF_Z, 1.0, 0.0),
        Mat::Wall,
    );
    ground(&mut p);
    lot(&mut p);
    onboarding_corner(&mut p);
    badge_office(&mut p);
    attendant_booth(&mut p);
    vendor_kiosk(&mut p);
    jokes(&mut p);
    let door = exit(&mut p);
    pickups(&mut p);

    let spawn = v(SPAWN.0, 0.0, SPAWN.1);
    p.route = vec![
        pt(SPAWN.0, SPAWN.1),
        pt(0.0, 40.0),
        pt(-12.0, 24.0),
        pt(0.0, 0.0),
        pt(0.0, -50.0),
        pt(door.0.x, door.0.z),
    ];
    p.anchor("player_spawn", spawn, 0.0);
    let grave = v(GRAVEYARD.0, 0.0, GRAVEYARD.1);
    p.anchor("graveyard", grave, yaw_towards(grave, spawn));
    for (k, (x, z)) in PACKS.into_iter().enumerate() {
        let at = v(x, 0.0, z);
        let to = route_point_near(&p.route, pt(x, z));
        let name = ["pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "pack_6"][k];
        p.anchor(name, at, yaw_towards(at, v(to.x, 0.0, to.y)));
    }
    for (name, (x, z), (lx, lz)) in CAST {
        let at = v(x, 0.0, z);
        p.anchor(name, at, yaw_towards(at, v(lx, 0.0, lz)));
    }
    // The colleague and the intern sit by content (idle_pose); without a seat they hovered in
    // mid-air. Each gets a bench with a slot under them, and the anchor moves onto the seat.
    bench(&mut p, -2.05, 63.5, 0.0);
    bench(&mut p, 5.23, 69.39, -1.03);
    p.occupy("npc_colleague", v(-2.5, 0.0, 63.5));
    p.occupy("npc_1", v(5.0, 0.0, 69.0));
    let boss = v(BOSS.0, 0.0, BOSS.1);
    p.anchor("boss", boss, yaw_towards(boss, v(0.0, 0.0, -60.0)));
    p.anchor("door:campus", door.0, door.1);
    trees(&mut p);
    assemble(ZONE, 0, "Car Park P3", true, p, theme())
}

/// Grass beyond the fence under the fog, and the fence itself.
fn ground(p: &mut Plan) {
    let (hx, hz) = (HALF_X + 1.0, HALF_Z + 1.0);
    for (x, z, sx, sz) in [
        (0.0, -hz - 200.0, 600.0, 400.0),
        (0.0, hz + 200.0, 600.0, 400.0),
        (-hx - 150.0, 0.0, 300.0, 2.0 * hz),
        (hx + 150.0, 0.0, 300.0, 2.0 * hz),
    ] {
        p.slab(v(x, -0.06, z), 0.0, v(sx, 0.1, sz), Mat::LeafDark);
    }
    for (x, z, sx, sz) in [
        (0.0, -HALF_Z - 0.5, 2.0 * HALF_X + 2.0, 1.0),
        (0.0, HALF_Z + 0.5, 2.0 * HALF_X + 2.0, 1.0),
        (-HALF_X - 0.5, 0.0, 1.0, 2.0 * HALF_Z),
        (HALF_X + 0.5, 0.0, 1.0, 2.0 * HALF_Z),
    ] {
        p.boxed(v(x, 1.1, z), v(sx, 2.2, sz), Mat::Frosted, true);
    }
}

/// Asphalt, bay lines, the cars of everyone who got here at 07:00 to get a closer bay.
fn lot(p: &mut Plan) {
    p.boxed(
        v(0.0, 0.015, 16.0),
        v(112.0, 0.03, 132.0),
        Mat::Asphalt,
        false,
    );
    road(p, (0.0, 82.0), (0.0, -50.0), 8.0);
    let rows = [72.0, 52.0, 40.0, 8.0, -4.0, -32.0];
    for z in rows {
        for k in 0..33 {
            let x = -50.0 + k as f32 * 3.0;
            if x.abs() < 6.0 {
                continue;
            }
            p.slab(v(x, 0.04, z), 0.0, v(0.1, 0.02, 5.6), Mat::White);
        }
    }
    let cars = [
        (-8.5, 72.0, Mat::CarWhite),
        (-17.5, 72.0, Mat::CarBlue),
        (-44.5, 72.0, Mat::CarTeal),
        (14.5, 72.0, Mat::CarRed),
        (41.5, 72.0, Mat::CarWhite),
        (-20.5, 52.0, Mat::CarRed),
        (-47.5, 52.0, Mat::CarWhite),
        (11.5, 52.0, Mat::CarBlue),
        (44.5, 52.0, Mat::CarTeal),
        (-11.5, 40.0, Mat::CarWhite),
        (38.5, 40.0, Mat::CarRed),
        (-47.5, 8.0, Mat::CarBlue),
        (-14.5, 8.0, Mat::CarWhite),
        (20.5, 8.0, Mat::CarTeal),
        (47.5, -4.0, Mat::CarWhite),
        (-26.5, -4.0, Mat::CarRed),
        (-41.5, -32.0, Mat::CarWhite),
        (8.5, -32.0, Mat::CarBlue),
        (41.5, -32.0, Mat::CarRed),
    ];
    for (x, z, m) in cars {
        super::campus::car(p, x, z, false, m);
    }
    // Your car, in bay 4,112, with a clamp on it already.
    let (cx, cz) = (2.5, 77.0);
    super::campus::car(p, cx, cz, true, Mat::Yellow);
    p.prim(
        PrimKind::Cylinder,
        v(cx + 1.4, 0.35, cz - 1.0),
        v(0.8, 0.25, 0.8),
        Mat::Hazard,
    );
    signpost(p, -8.0, 84.0, true, 4.0, Sign::CarParkP3);
    signpost(p, 52.0, 30.0, false, 3.6, Sign::ClampWarning);
    // The P3 shuttle stop: a shelter, a bench, a sign.
    let (sx, sz) = (-10.0, 46.0);
    for (dx, dz) in [(-2.0, -0.8), (2.0, -0.8), (-2.0, 0.8), (2.0, 0.8)] {
        p.boxed(
            v(sx + dx, 1.3, sz + dz),
            v(0.12, 2.6, 0.12),
            Mat::Metal,
            false,
        );
    }
    p.boxed(v(sx, 2.66, sz), v(4.8, 0.12, 2.0), Mat::Dark, false);
    bench(p, sx, sz, 0.0);
    p.sign(v(sx, 2.0, sz + 0.86), 0.0, 2.2, Sign::P3Shuttle);
    for z in [60.0, 20.0, -20.0] {
        lamp(p, -5.5, z);
        lamp(p, 5.5, z + 10.0);
    }
}

/// The First Day corner beside your car: a pop-up HR banner, a rope line, a sad plant.
fn onboarding_corner(p: &mut Plan) {
    let (x, z) = (-6.0, 60.0);
    p.boxed(v(x, 0.02, z), v(6.0, 0.04, 10.0), Mat::Felt, false);
    p.boxed(v(x - 3.2, 1.2, z), v(0.1, 2.4, 1.6), Mat::White, false);
    p.sign(v(x - 3.12, 1.6, z), FRAC_PI_2, 1.5, Sign::FirstDay);
    p.block(v(x - 3.2, 1.2, z), v(0.3, 2.4, 1.6));
    super::props::sad_plant(p, x - 2.4, z - 4.2);
}

/// The Badge Office: a portacabin with a hatch, a queue rail and the photo booth.
fn badge_office(p: &mut Plan) {
    let (x, z) = (-30.0, 20.0);
    p.boxed(v(x, 1.5, z), v(10.0, 3.0, 4.0), Mat::Beige, true);
    p.boxed(v(x, 3.1, z), v(10.4, 0.2, 4.4), Mat::Trim, false);
    p.boxed(v(x, 1.6, z + 2.03), v(2.0, 0.8, 0.04), Mat::Window, false);
    p.boxed(v(x, 1.1, z + 2.25), v(2.4, 0.08, 0.5), Mat::White, false);
    p.boxed(
        v(x + 3.5, 1.05, z + 2.03),
        v(0.9, 2.1, 0.04),
        Mat::Door,
        false,
    );
    p.sign(v(x, 2.6, z + 2.06), 0.0, 4.2, Sign::BadgeOffice);
    for k in 0..5 {
        p.prim(
            PrimKind::Cylinder,
            v(x - 4.0 + k as f32 * 2.0, 0.5, z + 7.5),
            v(0.1, 1.0, 0.1),
            Mat::Chrome,
        );
    }
    p.boxed(v(x, 0.9, z + 7.5), v(8.0, 0.05, 0.05), Mat::Red, false);
    // The photo booth: one photo, no retakes, used for the next eleven years.
    let (bx, bz) = (x - 8.0, z + 3.0);
    p.boxed(v(bx, 1.2, bz), v(1.6, 2.4, 1.4), Mat::CarBlue, true);
    p.boxed(v(bx, 1.4, bz + 0.71), v(0.8, 1.6, 0.04), Mat::Dark, false);
    p.boxed(
        v(bx, 2.2, bz + 0.72),
        v(1.2, 0.3, 0.04),
        Mat::NeonYellow,
        false,
    );
    spot(
        p,
        "prop:badge_photo_booth",
        (bx, 1.4, bz + 0.7),
        (bx, bz + 2.2),
    );
}

/// Pay-and-display: a booth for the attendant, the ticket machine that takes no cards.
fn attendant_booth(p: &mut Plan) {
    let (x, z) = (28.0, 28.0);
    p.boxed(v(x, 1.3, z), v(3.0, 2.6, 3.0), Mat::White, true);
    p.boxed(v(x, 2.7, z), v(3.4, 0.2, 3.4), Mat::Trim, false);
    p.boxed(v(x - 1.52, 1.5, z), v(0.04, 0.9, 2.0), Mat::Window, false);
    let (mx, mz) = (18.0, 34.0);
    p.boxed(v(mx, 0.8, mz), v(0.7, 1.6, 0.5), Mat::CarBlue, true);
    p.boxed(
        v(mx, 1.25, mz + 0.26),
        v(0.5, 0.4, 0.04),
        Mat::ScreenGlow,
        false,
    );
    p.sign(v(mx, 1.9, mz + 0.26), 0.0, 1.2, Sign::PayDisplay);
    spot(
        p,
        "prop:p3_ticket_machine",
        (mx, 1.2, mz + 0.25),
        (mx, mz + 1.8),
    );
}

/// The lanyard kiosk: the only vendor in P3, cash only, card reader decorative.
fn vendor_kiosk(p: &mut Plan) {
    let (x, z) = (27.0, -10.5);
    p.boxed(v(x, 0.55, z), v(1.2, 1.1, 4.0), Mat::Wood, true);
    p.boxed(v(x, 1.12, z), v(1.3, 0.06, 4.1), Mat::Marble, false);
    for (dz, m) in [(-1.2, Mat::Red), (0.0, Mat::CarBlue), (1.2, Mat::Yellow)] {
        p.boxed(v(x + 0.2, 1.7, z + dz), v(0.05, 0.9, 0.08), m, false);
    }
    p.boxed(v(x + 0.6, 2.6, z), v(0.1, 0.1, 4.2), Mat::Metal, false);
    let (cx, cz) = (31.0, -16.0);
    p.boxed(v(cx, 0.9, cz), v(0.9, 1.8, 0.7), Mat::Dark, true);
    p.boxed(
        v(cx, 1.3, cz + 0.36),
        v(0.6, 0.5, 0.04),
        Mat::ScreenGlow,
        false,
    );
    spot(p, "prop:p3_coffee", (cx, 1.3, cz + 0.35), (cx, cz + 1.8));
}

/// The exit ramp up to the deck, the gate with its sign, and the door anchor beyond it.
fn exit(p: &mut Plan) -> (Vec3, f32) {
    let (x0, z0, x1, z1) = DECK;
    ramp_road(p, (0.0, -50.0), (0.0, -66.0), 8.0);
    p.boxed(
        v((x0 + x1) / 2.0, 0.02, (z0 + z1) / 2.0),
        v(x1 - x0, 0.04, z1 - z0),
        Mat::Concrete,
        false,
    );
    for x in [-6.0, 6.0] {
        p.boxed(v(x, 1.6, -76.0), v(1.2, 3.2, 1.2), Mat::Concrete, true);
    }
    p.boxed(v(0.0, 3.4, -76.0), v(13.2, 0.4, 0.8), Mat::Trim, false);
    p.sign(v(0.0, 4.4, -75.6), 0.0, 6.0, Sign::ToCampusGate);
    p.sign(v(0.0, 4.4, -76.4), PI, 6.0, Sign::ToCampusGate);
    // Railings along the deck edges that are not the ramp.
    for s in [-1.0, 1.0] {
        p.boxed(
            v(s * (x1 + 0.1), 0.6, (z0 + z1) / 2.0 + 4.0),
            v(0.15, 1.2, z1 - z0 - 8.0),
            Mat::Metal,
            true,
        );
    }
    signpost(p, -9.0, -46.0, true, 3.6, Sign::ToCampusGate);
    (v(0.0, 0.0, -80.2), PI)
}

/// The hatchback's picnic table with "production" on it, and the AI-hype and corporate ads
/// along the way north.
fn jokes(p: &mut Plan) {
    let (tx, tz) = (14.0, 45.0);
    p.boxed(v(tx, 0.7, tz), v(1.6, 0.1, 1.0), Mat::Wood, true);
    for dx in [-0.6, 0.6] {
        p.boxed(v(tx + dx, 0.35, tz), v(0.1, 0.7, 0.8), Mat::Wood, false);
    }
    p.boxed(v(tx, 0.9, tz), v(0.5, 0.3, 0.35), Mat::Yellow, false);
    spot(p, "prop:p3_prod_lunchbox", (tx, 0.95, tz), (tx, tz + 1.4));
    for (key, x, z, sign) in [
        ("prop:p3_ad_agents", 13.0, 60.0, Sign::AgentsAd),
        ("prop:p3_ad_family", -16.0, 34.0, Sign::FamilyAd),
        ("prop:p3_ad_rto", 14.0, 2.0, Sign::RtoAd),
    ] {
        signpost(p, x, z, true, 3.6, sign);
        spot(p, key, (x, 1.8, z + 0.3), (x, z + 1.6));
    }
}

/// Lost lanyards (`Objective::Collect`, tag `lost_lanyard`): three, dropped by people who quit
/// before they reached the gate.
fn pickups(p: &mut Plan) {
    for (key, x, z) in [
        ("prop:p3_lanyard_1", -46.0, 30.0),
        ("prop:p3_lanyard_2", 46.0, -18.0),
        ("prop:p3_lanyard_3", -40.0, -42.0),
    ] {
        p.interactable(key, v(x, 0.3, z), v(x + 1.2, 0.0, z));
    }
}

fn trees(p: &mut Plan) {
    for k in 0..9 {
        let z = -70.0 + k as f32 * 18.0;
        for x in [-57.0, 57.0] {
            super::models::tree(p, x, z, k % 2 == 0, 1.6);
        }
    }
}
