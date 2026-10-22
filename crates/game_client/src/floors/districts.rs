//! The open-world campus around the HQ core (`campus.rs`), 464 x 332 m, laid out like a corporate
//! town. Synergy Boulevard runs north from the South Gate (the road in from Car Park P3) past the
//! Food Truck Plaza and the Lunch Lawn to the HQ plaza; the ring road around the core carries
//! everything else. Food and leisure sit next to the offices (food trucks, the gym); parking and
//! transit hold the edges (the Overflow Lot by the gate, Garage & Station along the railway in the
//! north-east). Liquidity Creek comes down from the north, feeds the lake in Wellness Park and
//! leaves past the lot; the jogging loop circles the lake over two footbridges. The New HQ
//! construction site is the north-western frontier, the Old Campus (the 1998 buildings and the
//! Legacy Systems Depot) the industrial north, the Co-Living Commons the commuter strip in the
//! east, and Executive Hill with the board's villa looks down on all of it from the south-east.
//!
//! Primitives only, from a handful of materials so batches stay few; small things are decor and
//! cull with distance (`floors::DECOR_RANGE_OUTDOOR`). Everything is placed as if the ground were
//! flat: `layout::drape` sets it down on the terrain from `terrain()`.

use super::campus::{car, scooter};
use super::kit::*;
use super::layout::yaw_towards;
use super::props::bench;
use super::themes::{pt, runner, v};
use bevy::prelude::*;
use combat_core::{Terrain, TerrainFeature as F};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// A named part of the campus for the map, the radar and the "you are here" line.
pub struct District {
    pub name: &'static str,
    pub centre: (f32, f32),
    pub half: (f32, f32),
}

/// Most specific first; the HQ core comes last so the districts win where rectangles touch.
pub const DISTRICTS: [District; 12] = [
    District {
        name: "South Gate",
        centre: (0.0, 150.0),
        half: (22.0, 16.0),
    },
    District {
        name: "Food Truck Plaza",
        centre: (8.0, 82.0),
        half: (46.0, 24.0),
    },
    District {
        name: "Overflow Lot",
        centre: (-96.0, 122.0),
        half: (46.0, 44.0),
    },
    District {
        name: "Wellness Park",
        centre: (-171.5, 54.0),
        half: (60.5, 112.0),
    },
    District {
        name: "New HQ (Opening 2019)",
        centre: (-171.5, -114.0),
        half: (60.5, 52.0),
    },
    District {
        name: "Old Campus",
        centre: (-12.5, -112.0),
        half: (84.5, 54.0),
    },
    District {
        name: "Garage & Station",
        centre: (155.0, -112.0),
        half: (77.0, 54.0),
    },
    District {
        name: "Incubator Street",
        centre: (119.0, 0.0),
        half: (35.0, 56.0),
    },
    District {
        name: "Co-Living Commons",
        centre: (196.0, 25.0),
        half: (36.0, 81.0),
    },
    District {
        name: "Executive Hill",
        centre: (191.0, 136.0),
        half: (41.0, 30.0),
    },
    District {
        name: "Wellness Gym",
        centre: (103.0, 112.0),
        half: (47.0, 54.0),
    },
    District {
        name: "HQ Plaza",
        centre: (-13.5, 0.0),
        half: (97.5, 58.0),
    },
];

/// The district a campus point lies in, if any.
pub fn district_at(x: f32, z: f32) -> Option<&'static str> {
    DISTRICTS
        .iter()
        .find(|d| (x - d.centre.0).abs() <= d.half.0 && (z - d.centre.1).abs() <= d.half.1)
        .map(|d| d.name)
}

// ---- terrain ----

pub const LAKE: (f32, f32) = (-150.0, -5.0);
pub const LAKE_R: f32 = 16.0;
/// Liquidity Creek: down from the north edge, west into the lake, out south past the lot.
pub const CREEK: [(f32, f32); 4] = [(-104.0, -166.0), (-104.0, -5.0), LAKE, (-150.0, 166.0)];
const CREEK_HALF_W: f32 = 3.5;
const CREEK_BANK: f32 = 3.0;
const CREEK_DEPTH: f32 = 1.3;
/// The water surface, above the bed and below the banks.
const WATER_Y: f32 = -0.45;
/// Bridges: (x0, z0, x1, z1, road): road bridges carry asphalt, the rest are footbridges.
pub const BRIDGES: [(f32, f32, f32, f32, bool); 6] = [
    (-157.0, 49.0, -143.0, 55.0, true),
    (-111.0, -53.0, -97.0, -47.0, true),
    (-111.0, -115.0, -97.0, -109.0, true),
    (-157.0, 18.0, -143.0, 24.0, false),
    (-127.0, -12.0, -121.0, 2.0, false),
    (-157.0, 137.0, -143.0, 143.0, false),
];
const DECK_H: f32 = 0.15;
/// The water runs on past where it meets the ground, half a metre beyond the end of the bank
/// ramp, so it tucks under the bank whatever the coarse ground mesh does between its samples.
const CREEK_WATER_HALF_W: f32 = CREEK_HALF_W + CREEK_BANK + 0.5;
const LAKE_WATER_R: f32 = LAKE_R + 4.0 + 0.5;
/// Where a creek run meets the lake: inside the rim, so the lake's foam never crosses the mouth.
const CREEK_MOUTH: f32 = LAKE_WATER_R - 1.5;
/// Executive Hill: the flat top the villa stands on, and the ramp all round it.
pub const HILLTOP: (f32, f32, f32, f32) = (180.0, 124.0, 226.0, 160.0);
pub const HILL_H: f32 = 7.0;
const HILL_RAMP: f32 = 18.0;
/// The Founders' Terrace on the Old Campus: a raised plaza with stairs on its east side.
pub const TERRACE: (f32, f32, f32, f32) = (-45.0, -100.0, -15.0, -80.0);
pub const TERRACE_H: f32 = 2.4;
const TERRACE_RAMP: f32 = 7.0;

/// The campus ground. Hills keep clear of the creek, so the river bed only ever cuts flat land.
pub fn terrain() -> Terrain {
    let hill = |x: f32, z: f32, r: f32, h: f32| F::Hill { x, z, r, h };
    let mesa = |(x0, z0, x1, z1): (f32, f32, f32, f32), h: f32, ramp: f32| F::Mesa {
        x0,
        z0,
        x1,
        z1,
        h,
        ramp,
    };
    let mut features = vec![
        // Mindfulness Mound and the Riverside Knoll in the park.
        hill(-200.0, -22.0, 22.0, 6.0),
        hill(-200.0, 110.0, 22.0, 4.5),
        // Gradient Descent Hill behind Building 1, and the spoil heaps of the New HQ.
        hill(-80.0, -152.0, 14.0, 4.0),
        hill(-200.0, -142.0, 10.0, 2.5),
        hill(-150.0, -150.0, 8.0, 2.0),
        // The knoll between the sports field and the hill.
        hill(156.0, 156.0, 12.0, 3.0),
        mesa(HILLTOP, HILL_H, HILL_RAMP),
        mesa(TERRACE, TERRACE_H, TERRACE_RAMP),
        F::River {
            pts: CREEK.to_vec(),
            half_w: CREEK_HALF_W,
            bank: CREEK_BANK,
            depth: CREEK_DEPTH,
        },
        F::River {
            pts: vec![LAKE, (LAKE.0 + 0.01, LAKE.1)],
            half_w: LAKE_R,
            bank: 4.0,
            depth: 1.6,
        },
    ];
    for (x0, z0, x1, z1, _) in BRIDGES {
        features.push(F::Bridge {
            x0,
            z0,
            x1,
            z1,
            h: DECK_H,
        });
    }
    Terrain { features }
}

/// A position at absolute height `y` above the datum, wherever the ground is: `drape` adds the
/// terrain height later, so this subtracts it now (water surfaces, decks over a bank).
pub(super) fn abs(p: &Plan, x: f32, y: f32, z: f32) -> Vec3 {
    v(x, y - p.terrain.height(x, z), z)
}

// ---- cast ----

/// Trash packs `pack_9`.. in content order (`content/game.ron`, campus zone).
pub(super) const PACKS: [(f32, f32); 38] = [
    // Food Truck Plaza and the Lunch Lawn (1-2)
    (-20.0, 70.0),
    (-26.0, 94.0),
    (50.0, 106.0),
    (34.0, 104.0),
    // Wellness Park (2-3)
    (-128.0, -40.0),
    (-180.0, -16.0),
    (-186.0, 22.0),
    (-200.0, 76.0),
    (-174.0, 132.0),
    // Incubator Street (3-4)
    (111.0, -30.0),
    (124.0, -14.0),
    (111.0, 12.0),
    (124.0, 36.0),
    // Wellness Gym (3-4)
    (108.0, 64.0),
    (125.0, 106.0),
    (78.0, 104.0),
    // Garage & Station (4-5)
    (120.0, -86.0),
    (176.0, -80.0),
    (165.0, -132.0),
    (108.0, -134.0),
    // New HQ construction site (5)
    (-150.0, -98.0),
    (-190.0, -96.0),
    (-212.0, -122.0),
    (-168.0, -146.0),
    // Old Campus (4-5)
    (-35.0, -126.0),
    (22.0, -92.0),
    (-62.0, -98.0),
    (68.0, -150.0),
    // Co-Living Commons (4)
    (206.0, -34.0),
    (184.0, 42.0),
    (207.0, 82.0),
    // Executive Hill (5)
    (188.0, 136.0),
    (216.0, 130.0),
    (192.0, 157.0),
    // Along the boulevard, just in from the gate (1)
    (-30.0, 140.0),
    (17.0, 136.0),
    // Overflow Lot (2)
    (-112.0, 128.0),
    (-76.0, 146.0),
];

const PACK_NAMES: [&str; 38] = [
    "pack_9", "pack_10", "pack_11", "pack_12", "pack_13", "pack_14", "pack_15", "pack_16",
    "pack_17", "pack_18", "pack_19", "pack_20", "pack_21", "pack_22", "pack_23", "pack_24",
    "pack_25", "pack_26", "pack_27", "pack_28", "pack_29", "pack_30", "pack_31", "pack_32",
    "pack_33", "pack_34", "pack_35", "pack_36", "pack_37", "pack_38", "pack_39", "pack_40",
    "pack_41", "pack_42", "pack_43", "pack_44", "pack_45", "pack_46",
];

/// Named spots: (anchor, position, the point it faces).
type Spot = (&'static str, (f32, f32), (f32, f32));
pub(super) const SPOTS: [Spot; 29] = [
    ("npc_food_truck", (22.0, 82.5), (22.0, 74.0)),
    ("npc_espresso", (48.0, 82.5), (48.0, 74.0)),
    ("npc_ranger", (-132.0, 40.0), (-125.0, 40.0)),
    ("npc_marshal", (-174.0, 66.0), (-180.0, 60.0)),
    ("npc_incubator", (111.0, -4.0), (117.0, -4.0)),
    ("npc_cowork", (135.0, 30.0), (117.0, 30.0)),
    ("npc_gym", (86.0, 66.0), (86.0, 60.0)),
    ("npc_parking", (176.0, -104.0), (176.0, -96.0)),
    ("npc_site", (-146.0, -60.0), (-154.0, -60.0)),
    ("npc_archivist", (40.0, -119.5), (40.0, -112.0)),
    ("npc_resident", (189.0, 10.0), (196.0, 10.0)),
    ("npc_steward", (204.0, 103.0), (196.0, 103.0)),
    ("rare_unicorn", (124.0, -40.0), (117.0, -30.0)),
    ("rare_goose", (-172.0, -24.0), (-150.0, -5.0)),
    ("rare_architect", (-178.0, -140.0), (-178.0, -120.0)),
    ("rare_parachute", (210.0, 158.0), (188.0, 148.0)),
    ("event:free_pizza", (30.0, 74.5), (30.0, 66.0)),
    ("event:fire_drill", (-180.0, 60.0), (-170.0, 60.0)),
    ("night_8", (-210.0, -8.0), (-150.0, -5.0)),
    ("night_9", (128.0, 22.0), (117.0, 22.0)),
    ("night_10", (-96.0, 140.0), (-96.0, 120.0)),
    ("night_11", (-30.0, -150.0), (0.0, -112.0)),
    ("night_12", (170.0, 40.0), (196.0, 40.0)),
    // The campus HR presence: the bot kiosk at reception, the colleague slumped on the plaza.
    ("npc_hr", (13.5, -6.0), (0.0, 2.0)),
    ("npc_colleague", (-10.0, 9.5), (0.0, 2.0)),
    // The Old Campus raised plaza and the gate guard of the villa (ambient regulars).
    ("npc_founder_guide", (-22.0, -90.0), (-8.0, -90.0)),
    ("npc_villa_guard", (202.0, 130.0), (196.0, 124.0)),
    ("npc_bus_driver", (191.0, -6.0), (196.0, -6.0)),
    ("npc_gate_guard", (9.0, 147.0), (0.0, 147.0)),
];

/// Shuttle stops (fast travel, `world.props` `shuttle_*`): prop centre and facing. The plaza stop
/// is the old sign by the arrival spot.
pub(super) const SHUTTLES: [(&str, (f32, f32), Face); 8] = [
    ("prop:shuttle_park", (-126.0, 58.0), Face::PosZ),
    ("prop:shuttle_site", (-140.0, -56.0), Face::PosZ),
    ("prop:shuttle_oldcampus", (8.0, -100.0), Face::PosX),
    ("prop:shuttle_station", (140.0, -142.0), Face::PosZ),
    ("prop:shuttle_commuter", (204.0, -4.0), Face::NegX),
    ("prop:shuttle_incubator", (100.0, 44.0), Face::NegZ),
    ("prop:shuttle_villa", (188.0, 100.0), Face::PosX),
    ("prop:shuttle_gate", (12.0, 138.0), Face::NegX),
];

/// Borrowable e-scooters out in the districts (the core four are `office::CAMPUS_SCOOTERS`).
pub const SCOOTERS: [(&str, f32, f32); 8] = [
    ("prop:scooter_5", -124.0, 62.0),
    ("prop:scooter_6", 134.0, -140.0),
    ("prop:scooter_7", 98.0, 38.0),
    ("prop:scooter_8", 15.0, 132.0),
    ("prop:scooter_9", 12.0, -96.0),
    ("prop:scooter_10", 207.0, 2.0),
    ("prop:scooter_11", -134.0, -60.0),
    ("prop:scooter_12", 184.0, 104.0),
];

/// Where packs, NPCs and doors stand, for the tree scatter in `campus.rs`.
pub fn keep_clear() -> impl Iterator<Item = (f32, f32)> {
    PACKS
        .iter()
        .copied()
        .chain(SPOTS.iter().map(|s| s.1))
        .chain(SHUTTLES.iter().map(|s| s.1))
        .chain(SCOOTERS.iter().map(|s| (s.1, s.2)))
}

/// Tree clumps of the districts (x, z, pine).
pub const TREES: [(f32, f32, bool); 54] = [
    // Wellness Park
    (-212.0, -50.0, true),
    (-186.0, -52.0, false),
    (-218.0, -22.0, true),
    (-204.0, 4.0, false),
    (-222.0, 30.0, true),
    (-168.0, 40.0, false),
    (-208.0, 52.0, true),
    (-186.0, 96.0, false),
    (-216.0, 132.0, true),
    (-190.0, 150.0, false),
    (-166.0, 112.0, true),
    (-122.0, -46.0, false),
    (-126.0, 20.0, true),
    (-170.0, -40.0, false),
    // Lunch Lawn and the boulevard
    (-34.0, 64.0, false),
    (-12.0, 82.0, true),
    (-36.0, 100.0, false),
    (-9.0, 112.0, false),
    (9.0, 112.0, false),
    (-9.0, 124.0, false),
    (9.0, 124.0, false),
    // Old Campus
    (-84.0, -70.0, true),
    (-20.0, -70.0, false),
    (30.0, -70.0, true),
    (-92.0, -120.0, false),
    (-60.0, -160.0, true),
    // New HQ outskirts
    (-128.0, -70.0, false),
    (-224.0, -70.0, true),
    // Garage & Station
    (92.0, -70.0, false),
    (226.0, -70.0, true),
    (100.0, -110.0, true),
    // Incubator Street and Co-Living Commons
    (90.0, -48.0, true),
    (148.0, -48.0, false),
    (150.0, 12.0, true),
    (226.0, -20.0, false),
    (168.0, -30.0, true),
    (228.0, 28.0, true),
    (168.0, 70.0, false),
    (226.0, 100.0, false),
    // Executive Hill: specimen trees on the top and its slopes
    (184.0, 128.0, true),
    (224.0, 162.0, false),
    (170.0, 140.0, false),
    (172.0, 116.0, true),
    // Wellness Gym and the sports field
    (60.0, 62.0, false),
    (146.0, 64.0, true),
    (62.0, 160.0, false),
    (140.0, 160.0, true),
    // Overflow Lot edges
    (-140.0, 96.0, false),
    (-54.0, 96.0, true),
    (-140.0, 162.0, true),
    (-54.0, 162.0, false),
    // HQ outskirts west of the ring road
    (-90.0, -30.0, true),
    (-86.0, 20.0, false),
    (-92.0, 60.0, true),
];

pub(super) fn build(p: &mut Plan) {
    roads(p);
    creek(p);
    food_trucks(p);
    park(p);
    construction(p);
    garage_station(p);
    incubator(p);
    gym(p);
    overflow_lot(p);
    super::frontier::build(p);
    super::dressing::build(p);
    shuttles(p);
    ill_marie(p);
    picnic_dedication(p);
    for (k, (x, z)) in PACKS.into_iter().enumerate() {
        let at = v(x, 0.0, z);
        p.anchor(PACK_NAMES[k], at, yaw_towards(at, v(x * 0.8, 0.0, z * 0.8)));
    }
    for (name, (x, z), (lx, lz)) in SPOTS {
        let at = v(x, 0.0, z);
        p.anchor(name, at, yaw_towards(at, v(lx, 0.0, lz)));
    }
    // The scooters themselves are drawn by `office` (they vanish while borrowed).
    for (anchor, x, z) in SCOOTERS {
        p.interactable(anchor, v(x, 0.5, z), v(x + 1.3, 0.0, z));
    }
}

/// Ill Marie's pavement "desk" on the boulevard every new hire walks up from the South Gate:
/// loud enough to be seen from the road (tissue tower, steaming humidifier, a bin of used
/// tissues), off the walking line so she never gets in the way.
/// A dedication, tucked away on the lawn east of the lake: the author's wife
/// and daughter on a picnic, on a bench facing the water, with a chequered blanket, a
/// basket and two flowers, a big one and a small one.
fn picnic_dedication(p: &mut Plan) {
    // Far from every enemy pack and patrol, facing the lake.
    let (x, z, yaw) = (-114.0, 10.0, -FRAC_PI_2);
    p.framed(v(x, 0.0, z), yaw, |p| {
        // Local frame: the bench faces +z, the blanket lies in front of it.
        bench(p, 0.0, 0.0, 0.0);
        p.boxed(v(0.2, 0.17, 2.0), v(0.38, 0.3, 0.55), Mat::Wood, false);
        p.boxed(v(0.2, 0.42, 2.0), v(0.36, 0.2, 0.05), Mat::Wood, false);
        super::models::put(
            p,
            &super::models::FLOWER_YELLOW,
            v(-1.3, 0.0, -0.4),
            1.0,
            7.0,
            false,
        );
        super::models::put(
            p,
            &super::models::FLOWER_RED,
            v(1.3, 0.0, -0.4),
            2.0,
            4.5,
            false,
        );
    });
    // Slabs take world positions, so the chequered blanket is turned by hand.
    let turn = Quat::from_rotation_y(yaw);
    for i in 0..4 {
        for j in 0..4 {
            let mat = if (i + j) % 2 == 0 {
                Mat::Red
            } else {
                Mat::White
            };
            let at = turn * Vec3::new(-0.75 + i as f32 * 0.5, 0.0, 1.2 + j as f32 * 0.5);
            p.slab(v(x + at.x, 0.02, z + at.z), yaw, v(0.5, 0.01, 0.5), mat);
        }
    }
    // Seats are matched in world space: the bench's two slots sit 0.45 m either side; the
    // free one is for the player.
    let side = turn * Vec3::X * 0.45;
    p.occupy("npc_picnic_wife", v(x, 0.0, z) - side);
    // The daughter stands by the blanket and runs about from there (friendly wander).
    let play = turn * Vec3::new(1.4, 0.0, 2.6);
    p.anchor(
        "npc_picnic_daughter",
        v(x + play.x, 0.0, z + play.z),
        yaw + PI,
    );
}

fn ill_marie(p: &mut Plan) {
    let (x, z) = (7.5, 118.0);
    p.prop(&DESK, v(x, 0.0, z), -FRAC_PI_2, true);
    p.prop(&CHAIR_DESK, v(x + 1.3, 0.0, z), FRAC_PI_2, false);
    p.occupy("npc_ill_marie", v(x + 1.3, 0.0, z));
    // The tissue tower: boxes stacked higher than her monitor.
    for k in 0..4 {
        let y = 0.82 + k as f32 * 0.13;
        p.boxed(
            v(x - 0.15, y, z - 0.45),
            v(0.24, 0.12, 0.13),
            Mat::White,
            false,
        );
    }
    p.boxed(
        v(x - 0.15, 1.38, z - 0.45),
        v(0.06, 0.08, 0.04),
        Mat::Paper,
        false,
    );
    // Humidifier and its steam, a sad thermos, the bin of used tissues by her chair.
    p.boxed(
        v(x + 0.1, 0.9, z + 0.4),
        v(0.18, 0.24, 0.18),
        Mat::Frosted,
        false,
    );
    p.boxed(
        v(x + 0.1, 1.15, z + 0.4),
        v(0.08, 0.26, 0.08),
        Mat::Glass,
        false,
    );
    p.boxed(
        v(x - 0.3, 0.9, z + 0.15),
        v(0.08, 0.22, 0.08),
        Mat::Metal,
        false,
    );
    p.boxed(
        v(x + 1.9, 0.25, z - 0.6),
        v(0.35, 0.5, 0.35),
        Mat::Dark,
        true,
    );
    p.boxed(
        v(x + 1.9, 0.55, z - 0.6),
        v(0.3, 0.12, 0.3),
        Mat::White,
        false,
    );
}

pub(super) fn spot(p: &mut Plan, anchor: &'static str, center: (f32, f32, f32), stand: (f32, f32)) {
    p.interactable(
        anchor,
        v(center.0, center.1, center.2),
        v(stand.0, 0.0, stand.1),
    );
}

/// A sign on two posts, readable from both sides.
pub(super) fn signpost(p: &mut Plan, x: f32, z: f32, along_x: bool, width: f32, sign: Sign) {
    let yaw = if along_x { 0.0 } else { FRAC_PI_2 };
    p.framed(v(x, 0.0, z), yaw, |p| {
        for sx in [-width / 2.0 - 0.1, width / 2.0 + 0.1] {
            p.boxed(v(sx, 1.4, 0.0), v(0.14, 2.8, 0.14), Mat::Metal, false);
        }
        p.block(v(0.0, 1.0, 0.0), v(width, 2.0, 0.3));
        p.sign(v(0.0, 2.2, 0.06), 0.0, width, sign);
        p.sign(v(0.0, 2.2, -0.06), PI, width, sign);
    });
}

/// Axis-aligned road with a dashed centre line: scooter-friendly, it says so on the tin.
pub(super) fn road(p: &mut Plan, (x0, z0): (f32, f32), (x1, z1): (f32, f32), w: f32) {
    let along_x = (x1 - x0).abs() > (z1 - z0).abs();
    let len = if along_x { x1 - x0 } else { z1 - z0 }.abs();
    let mid = v((x0 + x1) / 2.0, 0.012, (z0 + z1) / 2.0);
    let size = if along_x {
        v(len, 0.024, w)
    } else {
        v(w, 0.024, len)
    };
    p.boxed(mid, size, Mat::Asphalt, false);
    let n = (len / 12.0) as i32;
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

/// A road up a slope: short sections, each tilted to the ground's rise along it so the
/// surface follows the ramp's curve instead of stepping (`drape` lifts each centre).
pub(super) fn ramp_road(p: &mut Plan, (x0, z0): (f32, f32), (x1, z1): (f32, f32), w: f32) {
    let d = Vec2::new(x1 - x0, z1 - z0);
    let n = (d.length() / 2.0).ceil().max(1.0) as i32;
    let yaw = (-d.y).atan2(d.x);
    let step = d.length() / n as f32;
    let ground = p.terrain.clone();
    let h = |t: f32| {
        let q = Vec2::new(x0, z0) + d * t;
        ground.height(q.x, q.y)
    };
    for k in 0..n {
        let (t0, t1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
        let (h0, h1, hc) = (h(t0), h(t1), h((t0 + t1) / 2.0));
        let c = Vec2::new(x0, z0) + d * ((t0 + t1) / 2.0);
        let rise = h1 - h0;
        p.slab_pitched(
            v(c.x, 0.05 + (h0 + h1) / 2.0 - hc, c.y),
            yaw,
            rise.atan2(step),
            v((step * step + rise * rise).sqrt() + 0.1, 0.12, w),
            Mat::Asphalt,
        );
    }
}

pub(super) fn lamp(p: &mut Plan, x: f32, z: f32) {
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
        Mat::StreetLamp,
    );
    // The pool of light it throws after dark: a flat disc, free compared to a real light.
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.04, z),
        v(LAMP_POOL_M, 0.01, LAMP_POOL_M),
        Mat::LampPool,
    );
}

/// Diameter of the light pool under a street lamp.
const LAMP_POOL_M: f32 = 9.0;

type Road = ((f32, f32), (f32, f32), f32);

/// Road spans between the bridges (x0, z0, x1, z1, width): the network the map draws.
const ROADS: [Road; 17] = [
    // The ring road around the core and its two long east-west avenues.
    ((-225.0, 52.0), (-157.0, 52.0), 6.0),
    ((-143.0, 52.0), (225.0, 52.0), 6.0),
    ((-225.0, -50.0), (-111.0, -50.0), 6.0),
    ((-97.0, -50.0), (225.0, -50.0), 6.0),
    ((-71.0, -47.0), (-71.0, 49.0), 6.0),
    ((71.0, -47.0), (71.0, 49.0), 6.0),
    // Synergy Boulevard, gate to plaza.
    ((0.0, 55.0), (0.0, 163.0), 10.0),
    // The north road into the Old Campus, and the railway road to the station.
    ((0.0, -53.0), (0.0, -150.0), 7.0),
    ((-97.0, -112.0), (225.0, -112.0), 6.0),
    ((-200.0, -112.0), (-111.0, -112.0), 6.0),
    // Incubator Street and the Co-Living lane up to the foot of Executive Hill.
    ((117.0, -47.0), (117.0, 49.0), 7.0),
    ((196.0, -47.0), (196.0, 106.0), 6.0),
    // The south avenue past the lot, the trucks and the gym.
    ((-143.0, 96.0), (-5.0, 96.0), 6.0),
    ((5.0, 96.0), (193.0, 96.0), 6.0),
    // Lot and site access.
    ((-96.0, 99.0), (-96.0, 102.0), 6.0),
    ((-154.0, -53.0), (-154.0, -66.0), 6.0),
    ((150.0, -109.0), (150.0, -103.0), 6.0),
];

fn roads(p: &mut Plan) {
    for (a, b, w) in ROADS {
        road(p, a, b, w);
    }
    // The private road up Executive Hill.
    ramp_road(p, (196.0, 106.0), (196.0, 126.0), 6.0);
    for k in 0..19 {
        let t = -216.0 + k as f32 * 24.0;
        lamp(p, t, 55.6);
        lamp(p, t + 12.0, -53.6);
    }
    for z in [-30.0, -6.0, 18.0, 42.0] {
        lamp(p, -74.6, z);
        lamp(p, 74.6, z);
        lamp(p, 121.2, z - 6.0);
    }
    for z in [64.0, 88.0, 112.0, 136.0] {
        lamp(p, -6.0, z);
        lamp(p, 6.0, z);
    }
}

/// Liquidity Creek: water on the river bed, invisible walls along it (gaps at the bridges),
/// the bridges themselves, and the lake.
fn creek(p: &mut Plan) {
    let crosses = |x: f32, z: f32, r: f32| {
        BRIDGES
            .iter()
            .any(|b| x + r > b.0 && x - r < b.2 && z + r > b.1 && z - r < b.3)
    };
    for (run, w) in CREEK.windows(2).enumerate() {
        let (a, b) = (Vec2::new(w[0].0, w[0].1), Vec2::new(w[1].0, w[1].1));
        creek_water(
            p,
            run,
            shorten(a, b, CREEK_MOUTH),
            shorten(b, a, CREEK_MOUTH),
        );
        // The lake has its own water and walls.
        let (a, b) = (shorten(a, b, LAKE_R), shorten(b, a, LAKE_R));
        let d = b - a;
        let len = d.length();
        if len < 1.0 {
            continue;
        }
        let along_x = d.x.abs() > d.y.abs();
        // Walls along the bed in 3 m cells, merged into runs between the bridges.
        let n = (len / 3.0).ceil() as i32;
        let cell = |k: i32| a + d * ((k as f32 + 0.5) / n as f32);
        let mut k = 0;
        while k < n {
            if crosses(cell(k).x, cell(k).y, 1.5) {
                k += 1;
                continue;
            }
            let start = k;
            while k < n && !crosses(cell(k).x, cell(k).y, 1.5) {
                k += 1;
            }
            let (s0, s1) = (
                a + d * (start as f32 / n as f32),
                a + d * (k as f32 / n as f32),
            );
            creek_wall(p, (s0 + s1) / 2.0, (s1 - s0).length() + 0.1, along_x);
        }
    }
    // The bend: both runs stop short of the corner, and the water round the outside of it is a
    // disc, below the runs so its foam rim only shows where they do not cover it.
    let bend = CREEK[1];
    let at = abs(p, bend.0, WATER_Y, bend.1);
    p.prim(
        PrimKind::Cylinder,
        at,
        v(2.0 * CREEK_WATER_HALF_W, 0.08, 2.0 * CREEK_WATER_HALF_W),
        Mat::Lake,
    );
    creek_wall(
        p,
        Vec2::new(CREEK[1].0, CREEK[1].1),
        2.0 * (CREEK_HALF_W + 1.0),
        true,
    );
    for (x0, z0, x1, z1, road) in BRIDGES {
        bridge(p, (x0, z0, x1, z1), road);
    }
    lake(p);
    // Creek signs at the bridge heads.
    signpost(p, -160.0, 46.0, true, 3.0, Sign::Creek);
    signpost(p, -94.0, -56.0, true, 3.0, Sign::Creek);
}

/// Water over one run of the creek. Runs sit 3 cm apart in height (`run` counts them), because
/// they overlap at the bend and the lake and equal heights would z-fight.
fn creek_water(p: &mut Plan, run: usize, a: Vec2, b: Vec2) {
    let d = b - a;
    if d.length() < 1.0 {
        return;
    }
    let mid = (a + b) / 2.0;
    let w = 2.0 * CREEK_WATER_HALF_W;
    let size = if d.x.abs() > d.y.abs() {
        v(d.length(), 0.08, w)
    } else {
        v(w, 0.08, d.length())
    };
    let at = abs(p, mid.x, WATER_Y + 0.03 * (run + 1) as f32, mid.y);
    p.boxed(at, size, Mat::Lake, false);
}

/// Invisible wall over the bed, low enough to throw spells across the water.
fn creek_wall(p: &mut Plan, c: Vec2, len: f32, along_x: bool) {
    let wall = 2.0 * (CREEK_HALF_W + 1.0);
    let s = if along_x {
        v(len, 2.7, wall)
    } else {
        v(wall, 2.7, len)
    };
    p.block(v(c.x, 0.85, c.y), s);
}

/// `a` moved towards `b` by `r` if it lies within `r` of the lake centre.
fn shorten(a: Vec2, b: Vec2, r: f32) -> Vec2 {
    let lake = Vec2::new(LAKE.0, LAKE.1);
    if a.distance(lake) < 1.0 {
        a + (b - a).normalize_or_zero() * r
    } else {
        a
    }
}

/// Deck, kerbs and railings over the creek; the railings are walls, the ends stay open.
fn bridge(p: &mut Plan, (x0, z0, x1, z1): (f32, f32, f32, f32), road: bool) {
    let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
    let (lx, lz) = (x1 - x0, z1 - z0);
    // A bridge runs along its longer side.
    let along_x = lx > lz;
    let deck = if road { Mat::Asphalt } else { Mat::Wood };
    // The deck is drawn 8 cm above the walkable height the terrain reports for the bridge: the
    // jogging loop's flat path slabs are centred on that height, and a path coplanar with the
    // deck flickers. Feet sink into the plank, which the orbit camera never shows.
    p.boxed(v(cx, 0.02, cz), v(lx, 0.12, lz), deck, false);
    // The pier's top sits inside the deck, never flush with its top face.
    p.boxed(
        v(cx, -0.25, cz),
        v(lx - 0.6, 0.5, lz - 0.6),
        Mat::Concrete,
        false,
    );
    for s in [-1.0, 1.0] {
        let (pos, size) = if along_x {
            (v(cx, 0.55, cz + s * (lz / 2.0 - 0.1)), v(lx, 1.1, 0.2))
        } else {
            (v(cx + s * (lx / 2.0 - 0.1), 0.55, cz), v(0.2, 1.1, lz))
        };
        let rail = if road { Mat::Metal } else { Mat::Wood };
        p.boxed(pos, size, rail, true);
    }
}

fn lake(p: &mut Plan) {
    let (x, z) = LAKE;
    let r = LAKE_R;
    let at = abs(p, x, WATER_Y, z);
    p.prim(
        PrimKind::Cylinder,
        at,
        v(2.0 * LAKE_WATER_R, 0.08, 2.0 * LAKE_WATER_R),
        Mat::Lake,
    );
    // Walls: an inscribed square plus four caps, close enough to a circle for walking.
    let s = r * 0.72;
    p.block(v(x, 0.85, z), v(2.0 * s, 2.7, 2.0 * s));
    let cap = r - s;
    for (dx, dz, sx, sz) in [
        (s + cap / 2.0, 0.0, cap, r),
        (-(s + cap / 2.0), 0.0, cap, r),
        (0.0, s + cap / 2.0, r, cap),
        (0.0, -(s + cap / 2.0), r, cap),
    ] {
        p.block(v(x + dx, 0.85, z + dz), v(sx, 2.7, sz));
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

/// A food truck parked along x, serving hatch facing north (-z): a delivery van with an awning
/// and a counter. `body` picks the van's paint from the old colours.
fn food_truck(p: &mut Plan, x: f32, z: f32, body: Mat) {
    let yaw = if matches!(body, Mat::CarTeal | Mat::CarRed) {
        FRAC_PI_2
    } else {
        -FRAC_PI_2
    };
    super::models::put(p, &super::models::DELIVERY, v(x, 0.0, z), yaw, 2.1, false);
    p.block(v(x, 1.6, z), v(7.0, 3.2, 3.2));
    p.slab(v(x - 0.5, 2.9, z - 2.3), 0.0, v(4.4, 0.06, 1.3), Mat::Red);
    p.boxed(
        v(x - 0.5, 1.25, z - 1.75),
        v(4.0, 0.08, 0.4),
        Mat::Metal,
        false,
    );
}

/// Food Truck Plaza, east of the boulevard and a minute from the tower: four trucks, picnic
/// tables, string lights and the menu board.
fn food_trucks(p: &mut Plan) {
    const Z: f32 = 88.0;
    p.boxed(
        v(30.0, 0.02, 78.0),
        v(50.0, 0.04, 26.0),
        Mat::Concrete,
        false,
    );
    for (k, x) in [12.0, 24.0, 36.0, 48.0].into_iter().enumerate() {
        let body = [Mat::CarTeal, Mat::Yellow, Mat::CarRed, Mat::CarBlue][k];
        food_truck(p, x, Z, body);
    }
    for x in [12.0, 18.0, 24.0, 30.0, 36.0, 42.0, 48.0] {
        for z in [70.0, 78.0] {
            if (x, z) != (30.0, 70.0) {
                picnic(p, x, z);
            }
        }
    }
    // String lights between poles over the tables.
    for x in [8.0, 22.0, 38.0, 52.0] {
        p.boxed(v(x, 2.2, 67.0), v(0.14, 4.4, 0.14), Mat::Metal, false);
        p.boxed(v(x, 2.2, 82.0), v(0.14, 4.4, 0.14), Mat::Metal, false);
    }
    for z in [67.0, 82.0] {
        p.boxed(v(30.0, 4.3, z), v(44.0, 0.04, 0.04), Mat::Dark, false);
        for k in 0..11 {
            let m = [Mat::NeonYellow, Mat::NeonPink, Mat::NeonCyan][k % 3];
            p.boxed(
                v(10.0 + k as f32 * 4.0, 4.15, z),
                v(0.2, 0.2, 0.2),
                m,
                false,
            );
        }
    }
    // The plaza sign faces the boulevard.
    signpost(p, 8.0, 62.0, false, 5.0, Sign::FoodTrucks);
    // Menu board on an easel by the second truck.
    let (mx, mz) = (29.0, 84.0);
    p.boxed(v(mx, 1.1, mz), v(1.8, 1.3, 0.08), Mat::Dark, true);
    p.sign(v(mx, 1.15, mz - 0.05), PI, 1.7, Sign::TruckMenu);
    spot(p, "prop:food_truck_menu", (mx, 1.1, mz), (mx, mz - 1.8));
    for (x, z) in [(6.0, 66.0), (54.0, 66.0)] {
        p.boxed(v(x, 0.3, z), v(1.6, 0.6, 1.6), Mat::Concrete, true);
        p.prim(PrimKind::Sphere, v(x, 0.9, z), v(1.3, 1.1, 1.3), Mat::Plant);
    }
}

/// Wellness Park: the lake with the jogging loop over its two footbridges, the pier, the yoga
/// lawn, the fire assembly point, an observation gazebo on Mindfulness Mound.
fn park(p: &mut Plan) {
    let (lx, lz) = LAKE;
    let ring: Vec<Vec2> = (0..=20)
        .map(|k| {
            let a = k as f32 * TAU / 20.0;
            pt(lx + 26.0 * a.cos(), lz + 26.0 * a.sin())
        })
        .collect();
    runner(p, &ring, 2.4, Mat::Sand, Mat::White);
    signpost(p, -122.0, 16.0, false, 2.6, Sign::JoggingLoop);
    super::dressing::lake(p);
    // Hydration stations (quest props): a slim kiosk with a screen.
    for (key, x, z, sx, sz) in [
        ("prop:hydration_1", -128.0, -30.0, -128.0, -28.4),
        ("prop:hydration_2", -178.0, 10.0, -176.4, 10.0),
        ("prop:hydration_3", -140.0, -34.0, -140.0, -32.4),
    ] {
        p.boxed(v(x, 0.7, z), v(0.6, 1.4, 0.5), Mat::White, true);
        p.boxed(v(x, 1.15, z), v(0.62, 0.3, 0.52), Mat::NeonCyan, false);
        spot(p, key, (x, 1.0, z), (sx, sz));
    }
    // Yoga lawn: mats in rows.
    for k in 0..12 {
        let (x, z) = (-214.0 + (k % 4) as f32 * 2.4, 34.0 + (k / 4) as f32 * 3.2);
        p.slab(v(x, 0.03, z), 0.0, v(0.8, 0.02, 1.9), Mat::Yoga);
    }
    super::props::gong(p, -206.0, 30.0);
    // Benches round the lake between the shore and the loop, one of them a prop.
    for (x, z, yaw) in [
        (lx - 22.0, lz, FRAC_PI_2),
        (lx, lz - 22.0, 0.0),
        (lx - 15.0, lz + 15.0, 0.75 * PI),
    ] {
        bench(p, x, z, yaw);
    }
    spot(p, "prop:lake_bench", (lx - 22.0, 0.5, lz), (lx - 20.7, lz));
    // Fire assembly point: a green sign on a post, a painted circle.
    let (ax, az) = (-180.0, 60.0);
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
    // Observation gazebo on Mindfulness Mound: posts and a roof, nothing to observe but the
    // construction site that never finishes.
    let (gx, gz) = (-200.0, -22.0);
    for (dx, dz) in [(-2.0, -2.0), (2.0, -2.0), (-2.0, 2.0), (2.0, 2.0)] {
        p.boxed(v(gx + dx, 1.3, gz + dz), v(0.2, 2.6, 0.2), Mat::Wood, true);
    }
    p.boxed(v(gx, 2.7, gz), v(5.0, 0.2, 5.0), Mat::Trim, false);
    p.prim(PrimKind::Cone, v(gx, 3.5, gz), v(5.6, 1.4, 5.6), Mat::Wood);
    bench(p, gx, gz + 1.0, PI);
    // Park sign at the bridge head on the ring road.
    signpost(p, -136.0, 46.0, true, 3.4, Sign::ParkName);
    // A dog-park fence on the Riverside Knoll, with no dogs: dogs need a manager's approval.
    super::frontier::fence(p, (-214.0, 96.0), (-188.0, 96.0), &[(-203.0, -199.0)]);
    super::frontier::fence(p, (-214.0, 96.0), (-214.0, 122.0), &[]);
}

/// New HQ (Opening 2019): a fenced site with a half-built frame, the crane and the portacabin.
fn construction(p: &mut Plan) {
    use super::frontier::fence;
    p.boxed(
        v(-172.0, 0.02, -116.0),
        v(108.0, 0.04, 96.0),
        Mat::Sand,
        false,
    );
    fence(p, (-226.0, -66.0), (-118.0, -66.0), &[(-160.0, -148.0)]);
    fence(p, (-226.0, -162.0), (-226.0, -66.0), &[]);
    fence(p, (-118.0, -162.0), (-118.0, -66.0), &[(-116.0, -108.0)]);
    // Foundation slab and the frame: columns, some beams, never finished.
    p.boxed(
        v(-176.0, 0.15, -122.0),
        v(40.0, 0.3, 22.0),
        Mat::Concrete,
        false,
    );
    for (i, x) in [-194.0, -185.0, -176.0, -167.0, -158.0]
        .into_iter()
        .enumerate()
    {
        for (j, z) in [-131.0, -113.0].into_iter().enumerate() {
            let h = if (i + j) % 3 == 0 { 3.4 } else { 7.0 };
            p.boxed(v(x, h / 2.0, z), v(0.6, h, 0.6), Mat::Concrete, true);
        }
    }
    for z in [-131.0, -113.0] {
        p.boxed(v(-176.0, 7.0, z), v(26.0, 0.5, 0.5), Mat::Concrete, false);
    }
    p.boxed(
        v(-167.0, 7.0, -122.0),
        v(0.5, 0.5, 18.5),
        Mat::Concrete,
        false,
    );
    for (x, z) in [(-194.0, -131.0), (-185.0, -113.0), (-158.0, -131.0)] {
        for d in [-0.15, 0.15] {
            p.boxed(v(x + d, 4.2, z + d), v(0.05, 1.6, 0.05), Mat::Bark, false);
        }
    }
    // Tower crane: mast, jib, counterweight, hook.
    let (cx, cz) = (-140.0, -132.0);
    p.boxed(v(cx, 11.0, cz), v(1.4, 22.0, 1.4), Mat::Hazard, true);
    p.boxed(v(cx - 9.0, 22.4, cz), v(28.0, 0.8, 0.8), Mat::Hazard, false);
    p.boxed(
        v(cx + 4.0, 21.6, cz),
        v(3.0, 1.6, 1.6),
        Mat::Concrete,
        false,
    );
    p.boxed(
        v(cx - 18.0, 17.0, cz),
        v(0.05, 10.0, 0.05),
        Mat::Dark,
        false,
    );
    p.boxed(v(cx - 18.0, 11.8, cz), v(0.6, 0.5, 0.6), Mat::Yellow, false);
    p.boxed(v(cx, 0.8, cz + 2.5), v(1.0, 1.6, 0.6), Mat::Dark, true);
    p.boxed(
        v(cx, 1.3, cz + 2.82),
        v(0.3, 0.3, 0.05),
        Mat::NeonRed,
        false,
    );
    spot(p, "prop:crane_button", (cx, 1.3, cz + 2.8), (cx, cz + 4.2));
    // Portacabin site office; the handbook reader sits behind it.
    let (ox, oz) = (-210.0, -80.0);
    p.boxed(v(ox, 1.4, oz), v(6.0, 2.8, 2.6), Mat::Beige, true);
    p.boxed(v(ox, 2.9, oz), v(6.3, 0.2, 2.9), Mat::Trim, false);
    p.boxed(
        v(ox + 2.0, 1.5, oz + 1.32),
        v(1.4, 0.8, 0.04),
        Mat::Window,
        false,
    );
    p.boxed(
        v(ox - 1.8, 1.05, oz + 1.32),
        v(0.9, 2.1, 0.04),
        Mat::Door,
        false,
    );
    p.prop(&CHAIR_DESK, v(ox, 0.0, oz - 3.0), 0.0 - PI, false);
    p.occupy("secret:handbook_reader", v(ox, 0.0, oz - 3.0));
    // Gate posts, the hard hat signs, the countdown.
    p.boxed(v(-160.5, 2.5, -66.2), v(0.25, 5.0, 0.25), Mat::Metal, true);
    p.boxed(v(-147.5, 2.5, -66.2), v(0.25, 5.0, 0.25), Mat::Metal, true);
    p.sign(v(-154.0, 4.4, -66.0), 0.0, 9.0, Sign::NewHq);
    p.sign(v(-136.0, 1.6, -65.9), 0.0, 2.6, Sign::HardHat);
    p.sign(v(-172.0, 1.6, -65.9), 0.0, 2.6, Sign::HardHat);
    let (dx, dz) = (-142.0, -67.5);
    p.boxed(v(dx, 1.0, dz), v(0.15, 2.0, 0.15), Mat::Metal, false);
    p.sign(v(dx, 2.2, dz + 0.1), 0.0, 1.8, Sign::DaysAi);
    spot(p, "prop:days_sign_site", (dx, 2.2, dz), (dx, dz + 2.2));
    // Materials: pallets and pipes.
    for (x, z) in [(-200.0, -100.0), (-150.0, -112.0), (-130.0, -100.0)] {
        p.boxed(v(x, 0.4, z), v(1.6, 0.8, 1.2), Mat::Cardboard, true);
    }
    for k in 0..3 {
        p.prim_yawed(
            PrimKind::Cylinder,
            v(-190.0, 0.4, -152.0 + k as f32 * 0.9),
            v(0.8, 5.0, 0.8),
            FRAC_PI_2,
            Mat::Concrete,
        );
    }
}

/// Garage & Station on the transit edge: the multi-storey garage, the surface lot, the platform
/// with its shelter and the railway running off the map.
fn garage_station(p: &mut Plan) {
    let (gx, gz) = (205.0, -128.0);
    p.boxed(v(gx, 4.5, gz), v(36.0, 9.0, 22.0), Mat::Concrete, true);
    for y in [3.0, 6.0, 9.0] {
        p.boxed(v(gx, y, gz), v(36.4, 0.4, 22.4), Mat::Trim, false);
    }
    for k in 0..8 {
        let x = gx - 15.5 + k as f32 * 4.4;
        for y in [1.5, 4.5, 7.5] {
            p.boxed(v(x, y, gz + 11.02), v(3.2, 1.6, 0.04), Mat::Dark, false);
        }
    }
    p.sign(v(gx, 7.5, gz + 11.25), 0.0, 8.0, Sign::Garage);
    // Surface lot north of the station road.
    p.boxed(
        v(150.0, 0.015, -95.0),
        v(52.0, 0.03, 14.0),
        Mat::Asphalt,
        false,
    );
    for k in 0..17 {
        p.slab(
            v(126.0 + k as f32 * 3.0, 0.04, -97.0),
            0.0,
            v(0.1, 0.02, 6.0),
            Mat::White,
        );
    }
    for (x, m) in [
        (133.5, Mat::CarWhite),
        (142.5, Mat::CarRed),
        (160.5, Mat::CarTeal),
        (169.5, Mat::CarBlue),
    ] {
        car(p, x, -97.0, false, m);
    }
    // Platform, shelter, departure board and the tracks.
    let (px, pz) = (150.0, -151.0);
    p.boxed(v(px, 0.06, pz), v(40.0, 0.12, 8.0), Mat::Concrete, false);
    p.boxed(
        v(px, 0.13, pz - 3.8),
        v(40.0, 0.02, 0.4),
        Mat::Yellow,
        false,
    );
    rails(p);
    for (x, z) in [
        (px - 8.0, pz + 2.0),
        (px + 8.0, pz + 2.0),
        (px - 8.0, pz - 2.0),
        (px + 8.0, pz - 2.0),
    ] {
        p.boxed(v(x, 1.5, z), v(0.15, 3.0, 0.15), Mat::Metal, false);
    }
    p.boxed(v(px, 3.05, pz), v(17.0, 0.12, 5.0), Mat::Dark, false);
    p.sign(v(px, 2.5, pz + 2.5), 0.0, 5.0, Sign::StationName);
    // The 2019 intern, still waiting for the shuttle to the new HQ, on a desk chair they
    // brought from onboarding, at the far end of the platform.
    p.prop(&CHAIR_DESK, v(px - 18.5, 0.0, pz), FRAC_PI_2 - PI, false);
    p.occupy("secret:intern_2019", v(px - 18.5, 0.0, pz));
    bench(p, px, pz - 0.5, 0.0);
    let (bx, bz) = (px + 4.0, pz + 3.4);
    p.boxed(v(bx, 2.2, bz), v(3.4, 1.1, 0.2), Mat::Dark, false);
    p.sign(v(bx, 2.2, bz + 0.11), 0.0, 3.2, Sign::DepartureBoard);
    spot(p, "prop:departure_board", (bx, 2.2, bz), (bx, bz + 3.0));
    super::props::vending(p, px + 10.5, pz + 2.0, Face::NegX);
    spot(
        p,
        "prop:station_vending",
        (px + 10.5, 1.0, pz + 2.0),
        (px + 9.0, pz + 2.0),
    );
}

/// The railway along the north edge: ballast and two rails end to end, sleepers only by the
/// platform where anyone looks at them.
fn rails(p: &mut Plan) {
    let (x0, x1, z) = (-90.0, 232.0, -158.0);
    let (mid, len) = ((x0 + x1) / 2.0, x1 - x0);
    p.boxed(v(mid, 0.04, z), v(len, 0.08, 3.4), Mat::Concrete, false);
    for dz in [-0.75, 0.75] {
        p.boxed(v(mid, 0.14, z + dz), v(len, 0.12, 0.12), Mat::Chrome, false);
    }
    for k in 0..20 {
        p.boxed(
            v(120.0 + k as f32 * 3.0, 0.09, z),
            v(0.3, 0.06, 2.4),
            Mat::Bark,
            false,
        );
    }
    // A buffer stop where the sidings end in the Old Campus.
    p.boxed(v(x0 + 1.0, 0.6, z), v(0.6, 1.2, 2.6), Mat::Hazard, true);
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
        booth(p, 104.5, z, Face::PosX, neon[k % 4]);
    }
    for (k, z) in [-28.0, -6.0, 6.0].into_iter().enumerate() {
        booth(p, 129.5, z, Face::NegX, neon[(k + 2) % 4]);
    }
    for z in [-44.0, 46.0] {
        p.boxed(v(112.6, 3.4, z), v(0.2, 6.8, 0.2), Mat::Metal, false);
        p.boxed(v(121.4, 3.4, z), v(0.2, 6.8, 0.2), Mat::Metal, false);
        p.sign(v(117.0, 6.2, z), 0.0, 7.0, Sign::IncubatorStreet);
        p.sign(v(117.0, 6.2, z - 0.1), PI, 7.0, Sign::IncubatorStreet);
    }
    // Pitch whiteboard by the street.
    let (wx, wz) = (113.0, -14.0);
    p.boxed(v(wx, 1.5, wz), v(0.06, 1.3, 2.0), Mat::Board, true);
    p.sign(v(wx + 0.05, 1.5, wz), FRAC_PI_2, 1.9, Sign::WhiteboardB);
    spot(p, "prop:pitch_whiteboard", (wx, 1.5, wz), (wx + 1.6, wz));
    // Coworking cafe: a canopy on posts, open towards the street, desks and a tap counter.
    let (cx, cz) = (142.0, 30.0);
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
}

/// The Corporate Wellness gym next to the offices: an outdoor canopy over treadmill desks,
/// weight racks, the step leaderboard screen and a smoothie bar.
fn gym(p: &mut Plan) {
    // Laid out around this corner; the clerk's desk is at its front.
    let (ox, oz) = (24.0, 8.0);
    let at = |x: f32, y: f32, z: f32| v(x + ox, y, z + oz);
    p.boxed(at(80.0, 0.02, 67.0), v(56.0, 0.04, 22.0), Mat::Felt, false);
    for x in [56.0, 68.0, 80.0] {
        for z in [60.0, 72.0] {
            p.boxed(at(x, 1.8, z), v(0.3, 3.6, 0.3), Mat::Metal, true);
        }
    }
    p.boxed(at(68.0, 3.7, 66.0), v(25.0, 0.2, 13.0), Mat::Trim, false);
    p.sign(at(68.0, 3.0, 59.4), PI, 6.0, Sign::GymName);
    // Treadmill desks in two rows facing the leaderboard.
    for (k, x) in [60.0, 64.0, 68.0, 72.0, 76.0].into_iter().enumerate() {
        for z in [64.0, 68.5] {
            p.boxed(at(x, 0.12, z), v(0.9, 0.2, 2.0), Mat::Dark, false);
            p.boxed(at(x, 1.1, z + 0.85), v(1.0, 0.06, 0.5), Mat::White, false);
            p.boxed(at(x, 0.6, z + 0.85), v(0.08, 1.0, 0.08), Mat::Metal, false);
            p.block(at(x, 0.5, z + 0.3), v(1.0, 1.0, 2.0));
            if k % 2 == 0 {
                p.boxed(
                    at(x, 1.25, z + 0.95),
                    v(0.4, 0.25, 0.03),
                    Mat::ScreenGlow,
                    false,
                );
            }
        }
    }
    spot(
        p,
        "prop:treadmill_desk",
        (64.0 + ox, 1.1, 64.85 + oz),
        (62.4 + ox, 64.0 + oz),
    );
    // The clerk's desk.
    p.boxed(at(62.0, 0.55, 59.6), v(3.0, 1.1, 0.8), Mat::White, true);
    // Weight racks.
    for x in [86.0, 90.0, 94.0] {
        p.boxed(at(x, 0.9, 74.0), v(2.2, 1.8, 0.6), Mat::Metal, true);
        for y in [0.5, 1.0, 1.5] {
            p.prim_yawed(
                PrimKind::Cylinder,
                at(x, y, 74.0),
                v(0.5, 2.4, 0.5),
                FRAC_PI_2,
                Mat::Tire,
            );
        }
    }
    // Step leaderboard screen.
    p.boxed(at(92.0, 3.0, 78.0), v(8.0, 4.0, 0.3), Mat::Dark, true);
    p.sign(at(92.0, 3.0, 77.8), PI, 7.6, Sign::StepBoard);
    spot(
        p,
        "prop:step_leaderboard",
        (92.0 + ox, 3.0, 77.8 + oz),
        (92.0 + ox, 74.5 + oz),
    );
    // Smoothie bar.
    p.boxed(at(102.0, 0.55, 62.0), v(1.0, 1.1, 5.0), Mat::Yoga, true);
    p.boxed(at(102.6, 1.6, 62.0), v(0.2, 1.0, 5.0), Mat::LedGreen, false);
    spot(
        p,
        "prop:smoothie_bar",
        (102.0 + ox, 1.1, 62.0 + oz),
        (100.4 + ox, 62.0 + oz),
    );
}

/// The Overflow Lot by the gate: asphalt, rows of cars, the sign that sends you to P3.
fn overflow_lot(p: &mut Plan) {
    p.boxed(
        v(-96.0, 0.015, 130.0),
        v(80.0, 0.03, 56.0),
        Mat::Asphalt,
        false,
    );
    for row in [108.0, 120.0, 140.0, 152.0] {
        for k in 0..25 {
            p.slab(
                v(-132.0 + k as f32 * 3.0, 0.04, row),
                0.0,
                v(0.1, 0.02, 6.0),
                Mat::White,
            );
        }
    }
    let cars = [
        (-127.5, 108.0, Mat::CarBlue),
        (-118.5, 108.0, Mat::CarWhite),
        (-100.5, 108.0, Mat::CarRed),
        (-82.5, 108.0, Mat::CarTeal),
        (-64.5, 108.0, Mat::CarWhite),
        (-124.5, 120.0, Mat::CarRed),
        (-97.5, 120.0, Mat::CarBlue),
        (-70.5, 120.0, Mat::CarWhite),
        (-130.5, 140.0, Mat::CarTeal),
        (-91.5, 140.0, Mat::CarWhite),
        (-61.5, 140.0, Mat::CarRed),
        (-115.5, 152.0, Mat::CarWhite),
        (-88.5, 152.0, Mat::CarBlue),
    ];
    for (x, z, m) in cars {
        car(p, x, z, false, m);
    }
    for (i, (x, z)) in [(-120.0, 130.0), (-80.0, 132.0), (-60.0, 154.0)]
        .into_iter()
        .enumerate()
    {
        scooter(p, x, z, i as f32 * 2.1, i == 1);
    }
    signpost(p, -110.0, 99.0, true, 4.0, Sign::OverflowLot);
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
        assert_eq!(district_at(LAKE.0, LAKE.1), Some("Wellness Park"));
        assert_eq!(district_at(117.0, 0.0), Some("Incubator Street"));
        assert_eq!(district_at(30.0, 78.0), Some("Food Truck Plaza"));
        assert_eq!(district_at(0.0, 2.0), Some("HQ Plaza"));
        assert_eq!(district_at(0.0, 158.0), Some("South Gate"));
        assert_eq!(district_at(205.0, 145.0), Some("Executive Hill"));
        assert_eq!(district_at(40.0, -130.0), Some("Old Campus"));
        assert_eq!(district_at(196.0, 20.0), Some("Co-Living Commons"));
        assert_eq!(district_at(500.0, 0.0), None);
    }

    #[test]
    fn the_terrain_has_hills_a_creek_and_bridges_over_it() {
        let t = terrain();
        let (x0, z0, x1, z1) = HILLTOP;
        assert!((t.height((x0 + x1) / 2.0, (z0 + z1) / 2.0) - HILL_H).abs() < 1e-3);
        assert!(t.height(-104.0, -80.0) < -1.0, "the creek bed");
        for (x0, z0, x1, z1, _) in BRIDGES {
            let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
            assert!(
                (t.height(cx, cz) - DECK_H).abs() < 1e-3,
                "deck at {cx},{cz}"
            );
            assert!(t.surface(cx, cz) < -0.5, "bed under the deck at {cx},{cz}");
        }
        // The core, the boulevard and the gate are flat ground.
        for (x, z) in [(0.0, 0.0), (0.0, 120.0), (0.0, 158.0), (30.0, 78.0)] {
            assert_eq!(t.height(x, z), 0.0, "{x},{z}");
        }
        // Every road is walkable: no slope on it steeper than the hill road.
        for (a, b, _) in ROADS {
            for k in 0..=40 {
                let f = k as f32 / 40.0;
                let (x, z) = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
                assert!(t.slope(x, z) < 0.45, "road slope at {x},{z}");
            }
        }
    }
}
