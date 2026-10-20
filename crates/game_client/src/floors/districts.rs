//! The open-world districts around the core campus (`campus.rs`): a ring road, Food Truck
//! Plaza (south), Wellness Park with the lake (west), the New HQ construction site (north-west),
//! Garage & Station (north-east), Incubator Street with the coworking cafe (east), the
//! Corporate Wellness gym (south-east) and the overflow lot (south-west). Primitives only, built
//! from a handful of materials so the batches stay few; anything small is decor and culls with
//! distance (`floors::DECOR_RANGE_OUTDOOR`).

use super::campus::{car, scooter};
use super::kit::*;
use super::layout::yaw_towards;
use super::props::bench;
use super::themes::{pt, runner, v};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// A named part of the campus for the map, the radar and the "you are here" line.
pub struct District {
    pub name: &'static str,
    pub centre: (f32, f32),
    pub half: (f32, f32),
}

/// The core comes last so the districts win where the rectangles touch.
pub const DISTRICTS: [District; 8] = [
    District {
        name: "Food Truck Plaza",
        centre: (8.0, 66.0),
        half: (38.0, 13.0),
    },
    District {
        name: "Wellness Park",
        centre: (-91.0, 0.0),
        half: (17.0, 48.0),
    },
    District {
        name: "New HQ (Opening 2019)",
        centre: (-75.0, -67.0),
        half: (33.0, 13.0),
    },
    District {
        name: "Garage & Station",
        centre: (75.0, -66.0),
        half: (35.0, 14.0),
    },
    District {
        name: "Incubator Street",
        centre: (91.0, 0.0),
        half: (17.0, 48.0),
    },
    District {
        name: "Wellness Gym",
        centre: (78.0, 66.0),
        half: (30.0, 13.0),
    },
    District {
        name: "Overflow Lot",
        centre: (-68.0, 66.0),
        half: (38.0, 13.0),
    },
    District {
        name: "HQ Plaza",
        centre: (0.0, 0.0),
        half: (74.0, 54.0),
    },
];

/// The district a campus point lies in, if any.
pub fn district_at(x: f32, z: f32) -> Option<&'static str> {
    DISTRICTS
        .iter()
        .find(|d| (x - d.centre.0).abs() <= d.half.0 && (z - d.centre.1).abs() <= d.half.1)
        .map(|d| d.name)
}

/// Trash packs `pack_9`.. in content order (`content/game.ron`, campus zone).
const PACKS: [(f32, f32); 24] = [
    // Food Truck Plaza
    (-14.0, 60.0),
    (26.0, 58.0),
    (40.0, 74.0),
    (-24.0, 74.0),
    // Wellness Park
    (-82.0, -30.0),
    (-104.0, -14.0),
    (-84.0, 10.0),
    (-102.0, 6.0),
    (-100.0, 40.0),
    // Incubator Street
    (84.0, -30.0),
    (95.0, -14.0),
    (85.0, 12.0),
    (95.0, 38.0),
    // Wellness Gym
    (84.0, 62.0),
    (98.0, 70.0),
    (76.0, 76.0),
    // Garage & Station
    (74.0, -58.0),
    (96.0, -58.0),
    (52.0, -62.0),
    (60.0, -70.0),
    // New HQ construction site
    (-60.0, -64.0),
    (-86.0, -64.0),
    (-98.0, -74.0),
    (-70.0, -74.0),
];

/// Named spots: (anchor, position, the point it faces).
type Spot = (&'static str, (f32, f32), (f32, f32));
const SPOTS: [Spot; 17] = [
    ("npc_food_truck", (-4.0, 69.5), (-4.0, 60.0)),
    ("npc_espresso", (30.0, 71.0), (30.0, 60.0)),
    ("npc_ranger", (-78.0, 36.0), (-70.0, 36.0)),
    ("npc_marshal", (-84.0, 30.0), (-88.0, 24.0)),
    ("npc_incubator", (84.0, -4.0), (90.0, -4.0)),
    ("npc_cowork", (95.0, 24.0), (90.0, 24.0)),
    ("npc_gym", (62.0, 58.0), (62.0, 52.0)),
    ("npc_parking", (68.0, -57.0), (68.0, -50.0)),
    ("npc_site", (-80.0, -54.5), (-80.0, -48.0)),
    ("rare_unicorn", (92.0, -40.0), (90.0, -30.0)),
    ("rare_goose", (-92.0, -24.0), (-92.0, -8.0)),
    ("rare_architect", (-84.0, -72.0), (-84.0, -60.0)),
    ("event:free_pizza", (8.0, 64.0), (8.0, 54.0)),
    ("event:fire_drill", (-88.0, 24.0), (-80.0, 24.0)),
    ("night_8", (-100.0, -36.0), (-92.0, -8.0)),
    ("night_9", (88.0, 20.0), (88.0, 0.0)),
    ("night_10", (-60.0, 66.0), (-60.0, 52.0)),
];

/// Shuttle stops outside the core (fast travel, `world.props` `shuttle_*`): prop centre and
/// facing; the plaza stop is the fifth.
const SHUTTLES: [(&str, (f32, f32), Face); 4] = [
    ("prop:shuttle_park", (-76.0, 26.0), Face::PosZ),
    ("prop:shuttle_site", (-90.0, -55.0), Face::PosZ),
    ("prop:shuttle_station", (50.0, -58.0), Face::PosZ),
    ("prop:shuttle_incubator", (76.5, 42.0), Face::NegZ),
];

/// Where packs, NPCs and doors stand, for the tree scatter in `campus.rs`.
pub fn keep_clear() -> impl Iterator<Item = (f32, f32)> {
    PACKS
        .iter()
        .copied()
        .chain(SPOTS.iter().map(|s| s.1))
        .chain(SHUTTLES.iter().map(|s| s.1))
}

/// Tree clumps of the districts (x, z, pine).
pub const TREES: [(f32, f32, bool); 22] = [
    (-108.0, -44.0, true),
    (-80.0, -44.0, false),
    (-106.0, 30.0, true),
    (-76.0, 46.0, false),
    (-96.0, 46.0, true),
    (-106.0, -26.0, false),
    (-108.0, 10.0, true),
    (-30.0, -76.0, false),
    (-20.0, -70.0, true),
    (0.0, -76.0, false),
    (20.0, -72.0, true),
    (34.0, -76.0, false),
    (108.0, -44.0, true),
    (108.0, 10.0, false),
    (78.0, 26.0, true),
    (108.0, 44.0, false),
    (-100.0, 76.0, true),
    (-40.0, 76.0, false),
    (46.0, 62.0, true),
    (108.0, 76.0, false),
    (-30.0, 62.0, true),
    (-108.0, 60.0, false),
];

pub(super) fn build(p: &mut Plan) {
    roads(p);
    food_trucks(p);
    park(p);
    construction(p);
    garage_station(p);
    incubator(p);
    gym(p);
    overflow_lot(p);
    shuttles(p);
    for (k, (x, z)) in PACKS.into_iter().enumerate() {
        let at = v(x, 0.0, z);
        p.anchor(PACK_NAMES[k], at, yaw_towards(at, v(x * 0.8, 0.0, z * 0.8)));
    }
    for (name, (x, z), (lx, lz)) in SPOTS {
        let at = v(x, 0.0, z);
        p.anchor(name, at, yaw_towards(at, v(lx, 0.0, lz)));
    }
}

const PACK_NAMES: [&str; 24] = [
    "pack_9", "pack_10", "pack_11", "pack_12", "pack_13", "pack_14", "pack_15", "pack_16",
    "pack_17", "pack_18", "pack_19", "pack_20", "pack_21", "pack_22", "pack_23", "pack_24",
    "pack_25", "pack_26", "pack_27", "pack_28", "pack_29", "pack_30", "pack_31", "pack_32",
];

fn spot(p: &mut Plan, anchor: &'static str, center: (f32, f32, f32), stand: (f32, f32)) {
    p.interactable(
        anchor,
        v(center.0, center.1, center.2),
        v(stand.0, 0.0, stand.1),
    );
}

/// Axis-aligned road with a dashed centre line: scooter-friendly, it says so on the tin.
fn road(p: &mut Plan, (x0, z0): (f32, f32), (x1, z1): (f32, f32), w: f32) {
    let along_x = (x1 - x0).abs() > (z1 - z0).abs();
    let len = if along_x { x1 - x0 } else { z1 - z0 }.abs();
    let mid = v((x0 + x1) / 2.0, 0.012, (z0 + z1) / 2.0);
    let size = if along_x {
        v(len, 0.024, w)
    } else {
        v(w, 0.024, len)
    };
    p.boxed(mid, size, Mat::Asphalt, false);
    let n = (len / 9.0) as i32;
    for k in 0..n {
        let t = (k as f32 + 0.5) / n as f32;
        let (x, z) = (x0 + (x1 - x0) * t, z0 + (z1 - z0) * t);
        let dash = if along_x {
            v(3.0, 0.02, 0.18)
        } else {
            v(0.18, 0.02, 3.0)
        };
        p.boxed(v(x, 0.03, z), dash, Mat::White, false);
    }
}

fn lamp(p: &mut Plan, x: f32, z: f32) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 2.0, z),
        v(0.14, 4.0, 0.14),
        Mat::Metal,
    );
    p.prim(
        PrimKind::Sphere,
        v(x, 4.1, z),
        v(0.45, 0.35, 0.45),
        Mat::LightPanel,
    );
}

/// The ring road around the core plus Incubator Street; lamps along the long runs.
fn roads(p: &mut Plan) {
    const W: f32 = 6.0;
    road(p, (-104.0, 52.0), (104.0, 52.0), W);
    road(p, (-104.0, -50.0), (104.0, -50.0), W);
    road(p, (-71.0, -47.0), (-71.0, 49.0), W);
    road(p, (71.0, -47.0), (71.0, 49.0), W);
    road(p, (90.0, -47.0), (90.0, 49.0), 7.0);
    for k in 0..9 {
        let t = -96.0 + k as f32 * 24.0;
        lamp(p, t, 55.6);
        lamp(p, t + 12.0, -53.6);
    }
    for z in [-30.0, -6.0, 18.0, 42.0] {
        lamp(p, -74.6, z);
        lamp(p, 74.6, z);
    }
}

fn picnic(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.75, z), v(2.0, 0.08, 0.9), Mat::Wood, true);
    for dz in [-0.8, 0.8] {
        p.boxed(v(x, 0.45, z + dz), v(2.0, 0.07, 0.35), Mat::Wood, false);
    }
    for dx in [-0.8, 0.8] {
        p.boxed(v(x + dx, 0.38, z), v(0.1, 0.76, 1.8), Mat::Metal, false);
    }
}

/// A food truck parked along x, serving hatch facing north (-z).
fn food_truck(p: &mut Plan, x: f32, z: f32, body: Mat) {
    p.boxed(v(x, 1.6, z), v(7.0, 2.6, 2.6), body, true);
    p.boxed(v(x + 3.9, 1.2, z), v(0.9, 1.8, 2.4), Mat::White, false);
    p.boxed(
        v(x - 0.5, 1.9, z - 1.32),
        v(4.0, 1.0, 0.04),
        Mat::Dark,
        false,
    );
    p.slab(v(x - 0.5, 2.7, z - 1.9), 0.0, v(4.4, 0.06, 1.3), Mat::Red);
    p.boxed(
        v(x - 0.5, 1.25, z - 1.45),
        v(4.0, 0.08, 0.3),
        Mat::Metal,
        false,
    );
    for dx in [-2.4, 2.4] {
        p.slab(v(x + dx, 0.38, z), FRAC_PI_2, v(2.7, 0.75, 0.3), Mat::Tire);
    }
}

/// Food Truck Plaza: four trucks, picnic tables, string lights, the menu board.
fn food_trucks(p: &mut Plan) {
    p.boxed(
        v(7.0, 0.02, 67.0),
        v(70.0, 0.04, 22.0),
        Mat::Concrete,
        false,
    );
    for (k, x) in [-18.0, -4.0, 10.0, 24.0].into_iter().enumerate() {
        let body = [Mat::CarTeal, Mat::Yellow, Mat::CarRed, Mat::CarBlue][k];
        food_truck(p, x, 75.0, body);
    }
    for x in [-14.0, -6.0, 2.0, 10.0, 18.0, 26.0] {
        for z in [61.5, 66.5] {
            if (x, z) != (10.0, 66.5) {
                picnic(p, x, z);
            }
        }
    }
    // String lights between poles over the tables.
    for x in [-20.0, 0.0, 20.0, 34.0] {
        p.boxed(v(x, 2.2, 58.0), v(0.14, 4.4, 0.14), Mat::Metal, false);
        p.boxed(v(x, 2.2, 70.0), v(0.14, 4.4, 0.14), Mat::Metal, false);
    }
    for z in [58.0, 70.0] {
        p.boxed(v(7.0, 4.3, z), v(54.0, 0.04, 0.04), Mat::Dark, false);
        for k in 0..14 {
            let m = [Mat::NeonYellow, Mat::NeonPink, Mat::NeonCyan][k % 3];
            p.boxed(
                v(-19.0 + k as f32 * 4.0, 4.15, z),
                v(0.2, 0.2, 0.2),
                m,
                false,
            );
        }
    }
    // The plaza sign stands at the west entrance, out of the line of sight to the pizza table.
    let (sx, sz) = (-22.0, 57.0);
    p.sign(v(sx, 3.6, sz), 0.0, 6.0, Sign::FoodTrucks);
    p.sign(v(sx, 3.6, sz - 0.1), PI, 6.0, Sign::FoodTrucks);
    p.boxed(v(sx - 2.4, 1.8, sz), v(0.15, 3.6, 0.15), Mat::Metal, false);
    p.boxed(v(sx + 2.4, 1.8, sz), v(0.15, 3.6, 0.15), Mat::Metal, false);
    // Menu board on an easel by the second truck.
    let (mx, mz) = (3.0, 72.0);
    p.boxed(v(mx, 1.1, mz), v(1.8, 1.3, 0.08), Mat::Dark, true);
    p.sign(v(mx, 1.15, mz - 0.05), PI, 1.7, Sign::TruckMenu);
    spot(p, "prop:food_truck_menu", (mx, 1.1, mz), (mx, mz - 1.8));
    // The Free Pizza event brings its own table to the gap in the middle row (`live.events`).
    for (x, z) in [(-26.0, 60.0), (40.0, 62.0)] {
        p.boxed(v(x, 0.3, z), v(1.6, 0.6, 1.6), Mat::Concrete, true);
        p.prim(PrimKind::Sphere, v(x, 0.9, z), v(1.3, 1.1, 1.3), Mat::Plant);
    }
}

/// Lake blockers: an inscribed square and four caps, close enough to a circle for walking.
fn lake(p: &mut Plan, (x, z): (f32, f32), r: f32) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.05, z),
        v(2.0 * r + 1.6, 0.1, 2.0 * r + 1.6),
        Mat::Sand,
    );
    // Tinted glass reads as deep water from the orbit camera; the light pond blue washes
    // out to white at this size.
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.1, z),
        v(2.0 * r, 0.06, 2.0 * r),
        Mat::Facade,
    );
    let s = r * 0.72;
    p.block(v(x, 0.3, z), v(2.0 * s, 0.6, 2.0 * s));
    let cap = r - s;
    for (dx, dz, sx, sz) in [
        (s + cap / 2.0, 0.0, cap, r),
        (-(s + cap / 2.0), 0.0, cap, r),
        (0.0, s + cap / 2.0, r, cap),
        (0.0, -(s + cap / 2.0), r, cap),
    ] {
        p.block(v(x + dx, 0.3, z + dz), v(sx, 0.6, sz));
    }
}

/// Wellness Park: the lake with a jogging loop, a pier, the yoga lawn, the fire assembly point.
fn park(p: &mut Plan) {
    const LAKE: (f32, f32) = (-92.0, -8.0);
    lake(p, LAKE, 11.0);
    let ring: Vec<Vec2> = (0..=16)
        .map(|k| {
            let a = k as f32 * TAU / 16.0;
            pt(LAKE.0 + 15.5 * a.cos(), LAKE.1 + 15.5 * a.sin())
        })
        .collect();
    runner(p, &ring, 2.4, Mat::Sand, Mat::White);
    // Pier from the east shore.
    p.boxed(v(-83.5, 0.2, -8.0), v(6.0, 0.12, 2.0), Mat::Wood, false);
    for (dx, dz) in [(-2.5, -0.9), (-2.5, 0.9), (0.5, -0.9), (0.5, 0.9)] {
        p.boxed(
            v(-83.5 + dx, 0.5, -8.0 + dz),
            v(0.12, 0.7, 0.12),
            Mat::Wood,
            false,
        );
    }
    // Hydration stations (quest props): a slim kiosk with a screen.
    for (key, x, z, sx, sz) in [
        ("prop:hydration_1", -80.0, -27.0, -80.0, -25.4),
        ("prop:hydration_2", -102.0, 20.0, -100.4, 20.0),
        ("prop:hydration_3", -79.5, -10.5, -79.5, -8.9),
    ] {
        p.boxed(v(x, 0.7, z), v(0.6, 1.4, 0.5), Mat::White, true);
        p.boxed(v(x, 1.15, z), v(0.62, 0.3, 0.52), Mat::NeonCyan, false);
        spot(p, key, (x, 1.0, z), (sx, sz));
    }
    // Yoga lawn: mats in rows, a mindfulness gong frame.
    for k in 0..8 {
        let (x, z) = (-104.0 + (k % 4) as f32 * 2.2, 26.0 + (k / 4) as f32 * 3.2);
        p.slab(v(x, 0.03, z), 0.0, v(0.8, 0.02, 1.9), Mat::Yoga);
    }
    // Benches around the lake, one of them a prop.
    for (x, z, yaw) in [
        (-92.0, 9.5, PI),
        (-108.0, -8.0, FRAC_PI_2),
        (-92.0, -26.5, 0.0),
    ] {
        bench(p, x, z, yaw);
    }
    spot(p, "prop:lake_bench", (-92.0, 0.5, 9.5), (-92.0, 8.2));
    // Fire assembly point: a green sign on a post, a painted circle.
    let (ax, az) = (-88.0, 24.0);
    p.prim(
        PrimKind::Cylinder,
        v(ax, 0.02, az),
        v(10.0, 0.02, 10.0),
        Mat::LedGreen,
    );
    p.prim(
        PrimKind::Cylinder,
        v(ax, 0.03, az),
        v(9.2, 0.02, 9.2),
        Mat::Concrete,
    );
    p.boxed(
        v(ax + 6.0, 1.4, az + 4.0),
        v(0.15, 2.8, 0.15),
        Mat::Metal,
        true,
    );
    p.sign(v(ax + 6.0, 2.6, az + 4.1), 0.0, 2.6, Sign::AssemblyPoint);
    // Park sign at the west road entrance.
    p.boxed(v(-76.0, 1.4, 40.0), v(0.2, 2.8, 0.2), Mat::Wood, true);
    p.sign(v(-76.2, 2.4, 40.0), -FRAC_PI_2, 3.4, Sign::ParkName);
    p.sign(v(-75.8, 2.4, 40.0), FRAC_PI_2, 3.4, Sign::ParkName);
}

/// Chain-link fence: posts and a frosted mesh panel per span; `gaps` along the run stay open.
fn fence(p: &mut Plan, (x0, z0): (f32, f32), (x1, z1): (f32, f32), gaps: &[(f32, f32)]) {
    let along_x = (x1 - x0).abs() > (z1 - z0).abs();
    let (a0, a1) = if along_x { (x0, x1) } else { (z0, z1) };
    let mut a = a0;
    while a < a1 - 0.1 {
        let b = (a + 6.0).min(a1);
        let open = gaps.iter().any(|g| a < g.1 && b > g.0);
        let mid = (a + b) / 2.0;
        let (pos, size, post) = if along_x {
            (v(mid, 1.1, z0), v(b - a, 2.2, 0.08), v(a, 1.2, z0))
        } else {
            (v(x0, 1.1, mid), v(0.08, 2.2, b - a), v(x0, 1.2, a))
        };
        if !open {
            p.boxed(pos, size, Mat::Frosted, true);
        }
        p.boxed(post, v(0.12, 2.4, 0.12), Mat::Metal, false);
        a = b;
    }
}

/// New HQ (Opening 2019): a fenced site with a half-built frame, a crane and the portacabin.
fn construction(p: &mut Plan) {
    p.boxed(v(-75.0, 0.02, -68.0), v(62.0, 0.04, 22.0), Mat::Sand, false);
    fence(p, (-106.0, -57.0), (-44.0, -57.0), &[(-78.0, -70.0)]);
    fence(p, (-106.0, -79.0), (-106.0, -57.0), &[]);
    fence(p, (-44.0, -79.0), (-44.0, -57.0), &[]);
    // Foundation slab and the frame: columns, some beams, never finished.
    p.boxed(
        v(-78.0, 0.15, -68.0),
        v(26.0, 0.3, 14.0),
        Mat::Concrete,
        false,
    );
    for (i, x) in [-90.0, -82.0, -74.0, -66.0].into_iter().enumerate() {
        for (j, z) in [-74.0, -62.0].into_iter().enumerate() {
            let h = if (i + j) % 3 == 0 { 3.4 } else { 7.0 };
            p.boxed(v(x, h / 2.0, z), v(0.6, h, 0.6), Mat::Concrete, true);
        }
    }
    for z in [-74.0, -62.0] {
        p.boxed(v(-74.0, 7.0, z), v(17.0, 0.5, 0.5), Mat::Concrete, false);
    }
    p.boxed(
        v(-66.0, 7.0, -68.0),
        v(0.5, 0.5, 12.5),
        Mat::Concrete,
        false,
    );
    // Rebar sticking out of the short columns.
    for (x, z) in [(-90.0, -74.0), (-82.0, -62.0), (-66.0, -74.0)] {
        for d in [-0.15, 0.15] {
            p.boxed(v(x + d, 4.2, z + d), v(0.05, 1.6, 0.05), Mat::Bark, false);
        }
    }
    // Tower crane: mast, jib, counterweight, hook.
    let (cx, cz) = (-54.0, -73.0);
    p.boxed(v(cx, 11.0, cz), v(1.4, 22.0, 1.4), Mat::Hazard, true);
    p.boxed(v(cx - 7.0, 22.4, cz), v(24.0, 0.8, 0.8), Mat::Hazard, false);
    p.boxed(
        v(cx + 4.0, 21.6, cz),
        v(3.0, 1.6, 1.6),
        Mat::Concrete,
        false,
    );
    p.boxed(
        v(cx - 14.0, 17.0, cz),
        v(0.05, 10.0, 0.05),
        Mat::Dark,
        false,
    );
    p.boxed(v(cx - 14.0, 11.8, cz), v(0.6, 0.5, 0.6), Mat::Yellow, false);
    p.boxed(v(cx, 0.8, cz + 2.5), v(1.0, 1.6, 0.6), Mat::Dark, true);
    p.boxed(
        v(cx, 1.3, cz + 2.82),
        v(0.3, 0.3, 0.05),
        Mat::NeonRed,
        false,
    );
    spot(p, "prop:crane_button", (cx, 1.3, cz + 2.8), (cx, cz + 4.2));
    // Portacabin site office; the handbook reader sits behind it.
    p.boxed(v(-100.0, 1.4, -60.5), v(6.0, 2.8, 2.6), Mat::Beige, true);
    p.boxed(v(-100.0, 2.9, -60.5), v(6.3, 0.2, 2.9), Mat::Trim, false);
    p.boxed(v(-98.0, 1.5, -59.18), v(1.4, 0.8, 0.04), Mat::Window, false);
    p.boxed(v(-101.8, 1.05, -59.18), v(0.9, 2.1, 0.04), Mat::Door, false);
    p.prop(&CHAIR_DESK, v(-100.0, 0.0, -63.5), 0.0 - PI, false);
    p.occupy("secret:handbook_reader", v(-100.0, 0.0, -63.5));
    // Gate sign, hard hat sign, the countdown.
    p.boxed(v(-79.0, 2.5, -57.2), v(0.25, 5.0, 0.25), Mat::Metal, true);
    p.boxed(v(-69.0, 2.5, -57.2), v(0.25, 5.0, 0.25), Mat::Metal, true);
    p.sign(v(-74.0, 4.4, -57.0), 0.0, 9.0, Sign::NewHq);
    p.sign(v(-60.0, 1.6, -56.9), 0.0, 2.6, Sign::HardHat);
    p.sign(v(-88.0, 1.6, -56.9), 0.0, 2.6, Sign::HardHat);
    let (dx, dz) = (-64.0, -58.5);
    p.boxed(v(dx, 1.0, dz), v(0.15, 2.0, 0.15), Mat::Metal, false);
    p.sign(v(dx, 2.2, dz + 0.1), 0.0, 1.8, Sign::DaysAi);
    spot(p, "prop:days_sign_site", (dx, 2.2, dz), (dx, dz + 2.2));
    // Materials: pallets and pipes.
    for (x, z) in [(-96.0, -70.0), (-60.0, -61.0), (-50.0, -64.0)] {
        p.boxed(v(x, 0.4, z), v(1.6, 0.8, 1.2), Mat::Cardboard, true);
    }
    for k in 0..3 {
        p.prim_yawed(
            PrimKind::Cylinder,
            v(-90.0, 0.4, -77.0 + k as f32 * 0.9),
            v(0.8, 5.0, 0.8),
            FRAC_PI_2,
            Mat::Concrete,
        );
    }
}

/// Garage & Station: the multi-storey garage (one big block with deck bands), the surface lot
/// in front, the platform with its shelter and the tracks running off the map.
fn garage_station(p: &mut Plan) {
    let (gx, gz) = (93.0, -71.0);
    p.boxed(v(gx, 4.5, gz), v(26.0, 9.0, 16.0), Mat::Concrete, true);
    for y in [3.0, 6.0, 9.0] {
        p.boxed(v(gx, y, gz), v(26.4, 0.4, 16.4), Mat::Trim, false);
    }
    for k in 0..6 {
        let x = gx - 11.0 + k as f32 * 4.4;
        for y in [1.5, 4.5, 7.5] {
            p.boxed(v(x, y, gz + 8.02), v(3.2, 1.6, 0.04), Mat::Dark, false);
        }
    }
    p.sign(v(gx, 7.5, gz + 8.25), 0.0, 8.0, Sign::Garage);
    // Surface lot.
    p.boxed(
        v(88.0, 0.015, -58.0),
        v(36.0, 0.03, 8.0),
        Mat::Asphalt,
        false,
    );
    for k in 0..12 {
        p.slab(
            v(71.5 + k as f32 * 3.0, 0.04, -58.0),
            0.0,
            v(0.1, 0.02, 6.0),
            Mat::White,
        );
    }
    for (x, m) in [
        (80.0, Mat::CarWhite),
        (86.0, Mat::CarRed),
        (101.0, Mat::CarTeal),
    ] {
        car(p, x, -58.0, false, m);
    }
    // Platform, shelter, departure board and the tracks.
    p.boxed(
        v(54.0, 0.06, -70.0),
        v(24.0, 0.12, 8.0),
        Mat::Concrete,
        false,
    );
    p.boxed(v(54.0, 0.13, -73.8), v(24.0, 0.02, 0.4), Mat::Yellow, false);
    for dz in [-0.75, 0.75] {
        p.boxed(
            v(75.0, 0.12, -77.0 + dz),
            v(70.0, 0.12, 0.12),
            Mat::Chrome,
            false,
        );
    }
    for k in 0..24 {
        p.boxed(
            v(41.0 + k as f32 * 3.0, 0.05, -77.0),
            v(0.3, 0.08, 2.4),
            Mat::Bark,
            false,
        );
    }
    for (x, z) in [(46.0, -68.0), (62.0, -68.0), (46.0, -72.0), (62.0, -72.0)] {
        p.boxed(v(x, 1.5, z), v(0.15, 3.0, 0.15), Mat::Metal, false);
    }
    p.boxed(v(54.0, 3.05, -70.0), v(17.0, 0.12, 5.0), Mat::Dark, false);
    p.sign(v(54.0, 2.5, -67.5), 0.0, 5.0, Sign::StationName);
    // The 2019 intern, still waiting for the shuttle to the new HQ, on a desk chair they
    // brought from onboarding, at the far end of the platform.
    p.prop(&CHAIR_DESK, v(43.5, 0.0, -71.0), FRAC_PI_2 - PI, false);
    p.occupy("secret:intern_2019", v(43.5, 0.0, -71.0));
    bench(p, 54.0, -71.5, 0.0);
    let (bx, bz) = (58.0, -67.6);
    p.boxed(v(bx, 2.2, bz), v(3.4, 1.1, 0.2), Mat::Dark, false);
    p.sign(v(bx, 2.2, bz + 0.11), 0.0, 3.2, Sign::DepartureBoard);
    spot(p, "prop:departure_board", (bx, 2.2, bz), (bx, bz + 3.0));
    super::props::vending(p, 64.5, -69.0, Face::NegX);
    spot(p, "prop:station_vending", (64.5, 1.0, -69.0), (63.0, -69.0));
}

/// A shipping-container pop-up booth: open side towards the street, a neon strip and a sign.
fn booth(p: &mut Plan, x: f32, z: f32, face: Face, neon: Mat) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 1.3, -1.1), v(6.0, 2.6, 0.2), Mat::Metal, true);
        for sx in [-2.9, 2.9] {
            p.boxed(v(sx, 1.3, 0.0), v(0.2, 2.6, 2.4), Mat::Metal, true);
        }
        p.boxed(v(0.0, 2.7, 0.0), v(6.2, 0.2, 2.6), Mat::Trim, false);
        p.boxed(v(0.0, 2.55, 1.25), v(5.8, 0.08, 0.06), neon, false);
        p.boxed(v(0.0, 0.5, 0.6), v(3.0, 1.0, 0.6), Mat::White, true);
        p.sign(v(0.0, 3.2, 1.2), 0.0, 3.4, Sign::PitchBooth);
        p.prim(
            PrimKind::Sphere,
            v(2.0, 0.35, 1.6),
            v(1.0, 0.7, 1.0),
            Mat::BeanA,
        );
    });
}

/// Incubator Street: container booths both sides, the street sign, the coworking cafe.
fn incubator(p: &mut Plan) {
    let neon = [Mat::NeonPink, Mat::NeonCyan, Mat::NeonYellow, Mat::LedGreen];
    for (k, z) in [-36.0, -22.0, 0.0, 14.0].into_iter().enumerate() {
        booth(p, 80.5, z, Face::PosX, neon[k % 4]);
    }
    for (k, z) in [-28.0, -6.0, 6.0].into_iter().enumerate() {
        booth(p, 99.5, z, Face::NegX, neon[(k + 2) % 4]);
    }
    for z in [-44.0, 46.0] {
        p.boxed(v(85.6, 3.4, z), v(0.2, 6.8, 0.2), Mat::Metal, false);
        p.boxed(v(94.4, 3.4, z), v(0.2, 6.8, 0.2), Mat::Metal, false);
        p.sign(v(90.0, 6.2, z), 0.0, 7.0, Sign::IncubatorStreet);
        p.sign(v(90.0, 6.2, z - 0.1), PI, 7.0, Sign::IncubatorStreet);
    }
    // Pitch whiteboard by the street.
    let (wx, wz) = (86.0, -14.0);
    p.boxed(v(wx, 1.5, wz), v(0.06, 1.3, 2.0), Mat::Board, true);
    p.sign(v(wx + 0.05, 1.5, wz), FRAC_PI_2, 1.9, Sign::WhiteboardB);
    spot(p, "prop:pitch_whiteboard", (wx, 1.5, wz), (wx + 1.6, wz));
    // Coworking cafe: a canopy on posts, open towards the street, desks and a tap counter.
    let (cx, cz) = (101.0, 24.0);
    p.boxed(v(cx, 0.03, cz), v(10.0, 0.06, 16.0), Mat::Wood, false);
    for (dx, dz) in [
        (-5.0, -8.0),
        (5.0, -8.0),
        (-5.0, 8.0),
        (5.0, 8.0),
        (5.0, 0.0),
    ] {
        p.boxed(
            v(cx + dx, 1.7, cz + dz),
            v(0.25, 3.4, 0.25),
            Mat::Metal,
            true,
        );
    }
    p.boxed(v(cx, 3.5, cz), v(10.6, 0.2, 16.6), Mat::Trim, false);
    p.boxed(v(cx + 5.0, 1.7, cz), v(0.15, 3.4, 16.0), Mat::Brick, true);
    p.sign(v(cx - 5.4, 3.0, cz), -FRAC_PI_2, 4.6, Sign::CoworkCafe);
    for (key, dz) in [("prop:hot_desk_1", -4.0), ("prop:hot_desk_2", 4.0)] {
        p.boxed(
            v(cx + 1.5, 0.74, cz + dz),
            v(1.4, 0.06, 2.4),
            Mat::White,
            false,
        );
        p.block(v(cx + 1.5, 0.4, cz + dz), v(1.4, 0.8, 2.4));
        p.boxed(
            v(cx + 1.6, 0.9, cz + dz),
            v(0.3, 0.25, 0.4),
            Mat::ScreenGlow,
            false,
        );
        bench(p, cx - 0.2, cz + dz, -FRAC_PI_2);
        spot(p, key, (cx + 1.5, 0.8, cz + dz), (cx - 2.0, cz + dz));
    }
    p.boxed(v(cx + 3.8, 0.55, cz), v(1.0, 1.1, 3.0), Mat::Wood, true);
    p.boxed(v(cx + 3.8, 1.3, cz), v(0.4, 0.5, 0.6), Mat::Chrome, false);
    spot(p, "prop:cowork_coffee", (cx + 3.8, 1.3, cz), (cx + 2.2, cz));
    lamp(p, 85.0, 30.0);
    lamp(p, 95.0, -40.0);
}

/// The Corporate Wellness gym: an outdoor canopy over treadmill desks, weight racks, the step
/// leaderboard screen and a smoothie bar.
fn gym(p: &mut Plan) {
    p.boxed(v(80.0, 0.02, 67.0), v(56.0, 0.04, 22.0), Mat::Felt, false);
    for x in [56.0, 68.0, 80.0] {
        for z in [60.0, 72.0] {
            p.boxed(v(x, 1.8, z), v(0.3, 3.6, 0.3), Mat::Metal, true);
        }
    }
    p.boxed(v(68.0, 3.7, 66.0), v(25.0, 0.2, 13.0), Mat::Trim, false);
    p.sign(v(68.0, 3.0, 59.4), PI, 6.0, Sign::GymName);
    // Treadmill desks in two rows facing the leaderboard.
    for (k, x) in [60.0, 64.0, 68.0, 72.0, 76.0].into_iter().enumerate() {
        for z in [64.0, 68.5] {
            p.boxed(v(x, 0.12, z), v(0.9, 0.2, 2.0), Mat::Dark, false);
            p.boxed(v(x, 1.1, z + 0.85), v(1.0, 0.06, 0.5), Mat::White, false);
            p.boxed(v(x, 0.6, z + 0.85), v(0.08, 1.0, 0.08), Mat::Metal, false);
            p.block(v(x, 0.5, z + 0.3), v(1.0, 1.0, 2.0));
            if k % 2 == 0 {
                p.boxed(
                    v(x, 1.25, z + 0.95),
                    v(0.4, 0.25, 0.03),
                    Mat::ScreenGlow,
                    false,
                );
            }
        }
    }
    spot(p, "prop:treadmill_desk", (64.0, 1.1, 64.85), (62.4, 64.0));
    // The clerk's desk.
    p.boxed(v(62.0, 0.55, 59.6), v(3.0, 1.1, 0.8), Mat::White, true);
    // Weight racks.
    for x in [86.0, 90.0, 94.0] {
        p.boxed(v(x, 0.9, 74.0), v(2.2, 1.8, 0.6), Mat::Metal, true);
        for y in [0.5, 1.0, 1.5] {
            p.prim_yawed(
                PrimKind::Cylinder,
                v(x, y, 74.0),
                v(0.5, 2.4, 0.5),
                FRAC_PI_2,
                Mat::Tire,
            );
        }
    }
    // Step leaderboard screen.
    p.boxed(v(92.0, 3.0, 78.0), v(8.0, 4.0, 0.3), Mat::Dark, true);
    p.sign(v(92.0, 3.0, 77.8), PI, 7.6, Sign::StepBoard);
    spot(p, "prop:step_leaderboard", (92.0, 3.0, 77.8), (92.0, 74.5));
    // Smoothie bar.
    p.boxed(v(102.0, 0.55, 62.0), v(1.0, 1.1, 5.0), Mat::Yoga, true);
    p.boxed(v(102.6, 1.6, 62.0), v(0.2, 1.0, 5.0), Mat::LedGreen, false);
    spot(p, "prop:smoothie_bar", (102.0, 1.1, 62.0), (100.4, 62.0));
}

/// The overflow lot in the south-west: asphalt, a few cars, the AI parking sign.
fn overflow_lot(p: &mut Plan) {
    p.boxed(
        v(-66.0, 0.015, 66.0),
        v(60.0, 0.03, 20.0),
        Mat::Asphalt,
        false,
    );
    for k in 0..19 {
        p.slab(
            v(-93.0 + k as f32 * 3.0, 0.04, 62.0),
            0.0,
            v(0.1, 0.02, 6.0),
            Mat::White,
        );
    }
    for (x, z, m) in [
        (-90.0, 62.0, Mat::CarBlue),
        (-81.0, 62.0, Mat::CarWhite),
        (-72.0, 62.0, Mat::CarRed),
        (-54.0, 62.0, Mat::CarTeal),
        (-48.0, 70.0, Mat::CarWhite),
    ] {
        car(p, x, z, false, m);
    }
    for (i, (x, z)) in [(-84.0, 70.0), (-66.0, 72.0), (-40.0, 62.0)]
        .into_iter()
        .enumerate()
    {
        scooter(p, x, z, i as f32 * 2.1, i == 1);
    }
    p.sign(v(-66.0, 1.6, 56.6), PI, 4.0, Sign::ParkingAi);
    p.boxed(v(-68.0, 0.8, 56.6), v(0.15, 1.6, 0.15), Mat::Metal, false);
    p.boxed(v(-64.0, 0.8, 56.6), v(0.15, 1.6, 0.15), Mat::Metal, false);
}

/// Shuttle stop: a post with the shuttle sign, a small roof and a bench. The plaza stop reuses
/// the old sign by the arrival spot (`campus.rs`).
fn shuttles(p: &mut Plan) {
    for (anchor, (x, z), face) in SHUTTLES {
        let d = face.dir();
        p.framed(v(x, 0.0, z), face.yaw(), |p| {
            p.boxed(v(0.0, 1.4, 0.0), v(0.15, 2.8, 0.15), Mat::Metal, true);
            p.sign(v(0.0, 2.5, 0.1), 0.0, 1.8, Sign::CampusShuttle);
            p.boxed(v(2.0, 2.6, -0.6), v(2.8, 0.1, 1.6), Mat::Dark, false);
            for sx in [0.8, 3.2] {
                p.boxed(v(sx, 1.3, -1.2), v(0.08, 2.6, 0.08), Mat::Metal, false);
            }
        });
        bench(p, x + 2.0 * d.z, z - 2.0 * d.x - 0.6 * d.z, face.yaw());
        p.interactable(anchor, v(x, 1.6, z), v(x + 1.5 * d.x, 0.0, z + 1.5 * d.z));
    }
    // The plaza stop: the old shuttle sign by the arrival spot.
    p.interactable(
        "prop:shuttle_plaza",
        v(-16.0, 1.6, 21.0),
        v(-16.0, 0.0, 22.5),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn districts_name_their_corners() {
        assert_eq!(district_at(-92.0, -8.0), Some("Wellness Park"));
        assert_eq!(district_at(90.0, 0.0), Some("Incubator Street"));
        assert_eq!(district_at(8.0, 66.0), Some("Food Truck Plaza"));
        assert_eq!(district_at(0.0, 2.0), Some("HQ Plaza"));
        assert_eq!(district_at(500.0, 0.0), None);
    }
}
