//! Ilmari Wing 4, The Cloud (docs/ILMARI_10.md C.5): the Data Center's roof and what Ilmari rents
//! above it. ilmari4/1 is the Cooling Deck (the roof: fans the size of doors, a garden of racks,
//! chillers, a water tower, a ladder policy), ilmari4/2 the Autoscale Floor (a data hall that
//! keeps adding racks to itself, the Autoscaler's boss pit with its Reserved Instance racks and
//! a bill hanging from the ceiling) and ilmari4/3 the Server Cloud (a hub, five regions floating
//! on a ring of cloud and the bridges between them; the Region Outage's platforms are exactly
//! these discs, `ZoneDef::platforms` in `game.ron`, and a test keeps the two in step). No point
//! lights (the 0.9 rule): light is emissive strips, neon and the status screens.

use super::datacenter::dress::*;
use super::ilmari1::{CELLS, HALL};
use super::ilmari3::{arrival_kit, casing, cast, ingest_stack};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::models::{self, BARRIER, BOX_LARGE, BOX_OPEN, CONVEYOR_LONG, ROBOT_ARM_A, SIGN_WARNING};
use super::props::{sad_plant, server_rack, water_cooler};
use super::shape::{regular, rounded_rect};
use super::themes::{pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// The Autoscaler's pit: room for four replicas to stand apart and the burst circles to land.
const POD2: (f32, f32, f32) = (46.0, 24.0, 21.0);
/// The hub, where the Region Outage hovers: the origin of `ZoneDef::platforms`.
pub const HUB: (f32, f32) = (46.0, 0.0);
pub const HUB_R: f32 = 9.0;
pub const REGION_R: f32 = 8.0;
/// Distance from the hub to the middle of each region.
const RING_M: f32 = 28.0;
/// Bridges: half length and half width, centred halfway along the ring spoke.
const BRIDGE: (f32, f32) = (6.5, 1.8);
const BRIDGE_AT_M: f32 = 14.5;
/// The regions in the order of `game.ron`: id, direction from the hub in degrees (x toward z).
pub const REGIONS: [(&str, f32); 5] = [
    ("us-east-1", 180.0),
    ("eu-west-1", 108.0),
    ("ap-south-1", 36.0),
    ("sa-east-1", -36.0),
    ("af-south-1", -108.0),
];

fn polar(deg: f32, r: f32) -> (f32, f32) {
    let a = deg.to_radians();
    (r * a.cos(), r * a.sin())
}

/// Region `k` of `REGIONS` as an offset from the hub.
pub fn region_offset(k: usize) -> (f32, f32) {
    polar(REGIONS[k].1, RING_M)
}

/// Bridge `k` as (offset from the hub, direction in degrees).
pub fn bridge_offset(k: usize) -> ((f32, f32), f32) {
    (polar(REGIONS[k].1, BRIDGE_AT_M), REGIONS[k].1)
}

/// The four status pylons round the hub: offset from the hub and the yaw their screen faces
/// (outward). The screens themselves are drawn by `wing4_fx` (they change with the sequence).
pub fn status_pylons() -> [(f32, f32, f32); 4] {
    [45.0_f32, 135.0, 225.0, 315.0].map(|d| {
        let (x, z) = polar(d, HUB_R - 1.2);
        (x, z, FRAC_PI_2 - d.to_radians())
    })
}

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme, outdoor) = match level {
        1 => (
            "ilmari4/1",
            "The Cloud: The Cooling Deck",
            cooling_deck(&mut p),
            true,
        ),
        2 => (
            "ilmari4/2",
            "The Cloud: The Autoscale Floor",
            autoscale_floor(&mut p),
            false,
        ),
        _ => (
            "ilmari4/3",
            "The Cloud: The Server Cloud",
            server_cloud(&mut p),
            true,
        ),
    };
    assemble(zone, level, name, outdoor, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, fog, ambient, floor, accent, fog_range, sun) = match level {
        // Roof: a bright, windy afternoon above a city of data centers.
        1 => (
            c(0.62, 0.64, 0.68),
            c(0.52, 0.62, 0.76),
            c(0.92, 0.96, 1.0),
            [0.58, 0.6, 0.64],
            LinearRgba::rgb(0.5, 1.8, 3.2),
            (50.0, 190.0),
            3600.0,
        ),
        // Autoscale floor: a cold hall in cloud-console blue, amber where the bill glows.
        2 => (
            c(0.5, 0.56, 0.68),
            c(0.1, 0.14, 0.24),
            c(0.8, 0.9, 1.0),
            [0.55, 0.62, 0.74],
            LinearRgba::rgb(0.6, 2.0, 4.0),
            (28.0, 100.0),
            2400.0,
        ),
        // Server cloud: pale sky, white floor, the status lights everywhere.
        _ => (
            c(0.8, 0.86, 0.94),
            c(0.62, 0.76, 0.94),
            c(0.95, 0.98, 1.0),
            [0.9, 0.94, 1.0],
            LinearRgba::rgb(0.4, 2.2, 3.0),
            (60.0, 230.0),
            4200.0,
        ),
    };
    Theme {
        wall,
        trim: c(0.12, 0.14, 0.2),
        accent,
        fabric: [c(0.3, 0.45, 0.65), c(0.55, 0.55, 0.65), c(0.4, 0.5, 0.5)],
        floor: if level == 3 {
            FloorKind::Gloss(floor)
        } else {
            FloorKind::Tile(floor)
        },
        fog,
        ambient,
        ambient_brightness: if level == 2 { 760.0 } else { 1100.0 },
        sun: c(1.0, 0.97, 0.92),
        sun_illuminance: sun,
        fog_range,
    }
}

// ------------------------------------------------------------------ shared pieces

/// A cloud: a few white balls.
fn cloud(p: &mut Plan, x: f32, y: f32, z: f32, s: f32) {
    for (dx, dy, dz, k) in [
        (0.0, 0.0, 0.0, 1.0),
        (0.7, -0.1, 0.2, 0.75),
        (-0.8, -0.15, -0.1, 0.8),
        (0.1, 0.35, -0.3, 0.7),
    ] {
        let d = 10.0 * s * k;
        p.prim(
            PrimKind::Sphere,
            v(x + dx * 6.0 * s, y + dy * 4.0 * s, z + dz * 6.0 * s),
            v(d, d * 0.62, d),
            Mat::LightPanel,
        );
    }
}

/// A big cooling fan: a housing, a dark drum with four blades and a hub, and a notice.
fn fan_unit(p: &mut Plan, x: f32, z: f32, r: f32) {
    p.boxed(
        v(x, 0.7, z),
        v(2.0 * r + 0.8, 1.4, 2.0 * r + 0.8),
        Mat::Metal,
        true,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x, 1.46, z),
        v(2.0 * r, 0.12, 2.0 * r),
        Mat::Dark,
    );
    for k in 0..3 {
        let a = k as f32 * PI / 3.0;
        p.slab(
            v(x, 1.58, z),
            a,
            v(2.0 * r - 0.3, 0.05, 0.55),
            if k == 1 { Mat::Cold } else { Mat::Chrome },
        );
    }
    p.prim(
        PrimKind::Cylinder,
        v(x, 1.62, z),
        v(0.7, 0.18, 0.7),
        Mat::Gold,
    );
    for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        p.boxed(
            v(x + dx * (r + 0.2), 1.1, z + dz * (r + 0.2)),
            v(0.14, 1.5, 0.14),
            Mat::Hazard,
            false,
        );
    }
}

/// A sign on a pole, `h` m up, turned `yaw` (the pole stands where the sign hangs).
fn board(p: &mut Plan, x: f32, z: f32, h: f32, yaw: f32, width: f32, sign: Sign) {
    p.boxed(v(x, h / 2.0, z), v(0.16, h, 0.16), Mat::Metal, false);
    p.sign(v(x, h + 0.2, z), yaw, width, sign);
}

// ------------------------------------------------------------------ ilmari4/1: the cooling deck

/// Bulkhead hut on the roof edge: a concrete box room the roof opens into, with a door in its
/// outer wall. `side` is -1 for the west one (the ladder hatch) and 1 for the east (the stairwell).
fn bulkhead(p: &mut Plan, side: f32) {
    p.fp.pod(
        rounded_rect(pt(side * 51.0, 0.0), 6.0, 5.0, 1.5, 0.0),
        Mat::Wall,
    );
}

fn cooling_deck(p: &mut Plan) -> Theme {
    // Glass rails all round, three metres high: the sky stays in the picture and nobody falls
    // off by walking; the two bulkheads are the only opaque walls, and hold the doors.
    p.ceiling = 3.2;
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 46.0, 24.0, 4.0, 0.0), Mat::Glass);
    bulkhead(p, -1.0);
    bulkhead(p, 1.0);
    p.route = vec![pt(-52.0, 0.0), pt(-30.0, 0.0), pt(0.0, 0.0), pt(52.0, 0.0)];
    p.build_walls();
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:campus_occupied", (-57.0, 0.0), Sign::IlW4Back),
                ("door:ilmari4/2", (57.0, 0.0), Sign::IlW4ToAutoscale),
            ],
            packs: &[(-20.0, 15.0), (4.0, -15.0), (28.0, 14.0)],
            boss: (36.0, -13.0),
            npc: None,
        },
    );
    // Fans: the roof's whole personality. Each is 8-9 m clear of a pack's slots.
    for (x, z, r) in [
        (-34.0, -13.0, 2.4),
        (-10.0, -17.0, 2.0),
        (-8.0, 16.0, 2.2),
        (16.0, 14.0, 2.4),
        (40.0, 8.0, 2.6),
        (20.0, -14.0, 2.0),
    ] {
        fan_unit(p, x, z, r);
    }
    board(p, -34.0, -9.0, 2.4, 0.0, 2.6, Sign::IlFan);
    board(p, 40.0, 4.2, 2.4, PI, 2.6, Sign::IlCoolingNote);
    // The rack garden: raised beds of racks with something green growing on top.
    for k in 0..6 {
        let x = -12.0 + k as f32 * 2.2;
        p.boxed(v(x, 0.25, 20.6), v(1.9, 0.5, 2.2), Mat::Wood, true);
        p.boxed(v(x, 0.52, 20.6), v(1.7, 0.06, 2.0), Mat::Plant, false);
        server_rack(p, x, 20.9, PI);
    }
    board(p, -14.0, 19.0, 2.2, PI, 2.8, Sign::IlRackGarden);
    // Chillers along the north rail, a row of solar panels over the south one.
    for x in [-26.0, -6.0, 8.0, 28.0] {
        cooling_unit(p, x, -22.4, Face::PosZ, false);
    }
    for k in 0..5 {
        models::put(
            p,
            &models::SOLAR_PANELS,
            v(-30.0 + k as f32 * 4.6, 0.0, 22.0),
            0.0,
            3.0,
            false,
        );
    }
    // Corner kit: a water tower, a chimney that vents nothing, tanks and a container stack.
    models::put(p, &models::WATER_TOWER, v(-41.0, 0.0, 17.0), 0.4, 4.2, true);
    models::put(
        p,
        &models::CHIMNEY_LARGE,
        v(43.0, 0.0, -19.0),
        0.0,
        4.6,
        true,
    );
    models::put(p, &models::TANK_LARGE, v(-42.0, 0.0, -18.0), 0.5, 3.0, true);
    models::put(
        p,
        &models::TANK_LARGE,
        v(-38.0, 0.0, -20.0),
        -0.3,
        2.6,
        true,
    );
    models::put(
        p,
        &models::SHIPPING_CONTAINER_A,
        v(31.0, 0.0, -21.0),
        FRAC_PI_2,
        3.4,
        true,
    );
    models::put(
        p,
        &models::SHIPPING_CONTAINER_B,
        v(31.0, 2.4, -21.0),
        FRAC_PI_2,
        3.4,
        false,
    );
    // Walkway stripes and the sign of the roof's one honest policy.
    floor_line(p, pt(-45.0, 1.8), pt(44.0, 1.8), 0.22, Mat::Yellow);
    floor_line(p, pt(-45.0, -1.8), pt(44.0, -1.8), 0.22, Mat::Yellow);
    board(p, -46.0, 4.5, 3.0, FRAC_PI_2, 3.4, Sign::IlLadder);
    board(p, -46.0, -4.5, 3.0, FRAC_PI_2, 3.4, Sign::IlRoofSla);
    // Bulkhead dressing: the hatch's ladder rails and a hazard frame.
    for side in [-1.0_f32, 1.0] {
        for l in [-0.4, 0.4] {
            p.boxed(
                v(side * 55.0 + l, 1.6, -3.6),
                v(0.06, 3.2, 0.06),
                Mat::Metal,
                false,
            );
        }
        for k in 0..8 {
            p.boxed(
                v(side * 55.0, 0.3 + k as f32 * 0.4, -3.6),
                v(0.8, 0.05, 0.05),
                Mat::Metal,
                false,
            );
        }
        p.sign(v(side * 51.0, 2.4, -4.6), 0.0, 3.0, Sign::IlLadder);
    }
    // Pigeons, promoted: grey ball, smaller ball, orange beak, on the rail. Senior ones.
    for (x, z) in [(-28.0, 23.5), (6.0, 23.5), (34.0, -23.5), (-4.0, -23.5)] {
        p.prim(
            PrimKind::Sphere,
            v(x, 3.35, z),
            v(0.5, 0.36, 0.34),
            Mat::Metal,
        );
        p.prim(
            PrimKind::Sphere,
            v(x + 0.28, 3.55, z),
            v(0.22, 0.22, 0.22),
            Mat::Chrome,
        );
        p.boxed(v(x + 0.43, 3.54, z), v(0.1, 0.05, 0.05), Mat::Hot, false);
    }
    // The skyline: other people's data centers, towers of rented floors, and the haze.
    skyline(p, 1);
    theme(1)
}

/// Towers beyond the rails or below the cloud line. `level` 1 stands them round the roof,
/// 3 sinks them into the clouds under the platforms.
fn skyline(p: &mut Plan, level: u8) {
    let base = if level == 1 { -4.0 } else { -44.0 };
    for k in 0..26 {
        let a = (k as f32 * 13.7 + 7.0).to_radians();
        let r = 70.0 + 22.0 * rnd(k, 3);
        let (cx, cz) = if level == 1 { (0.0, 0.0) } else { HUB };
        let (x, z) = (cx + r * a.cos() * 1.35, cz + r * a.sin());
        let tall = rnd(k, 5);
        let (m, unit) = if tall > 0.5 {
            (&models::TOWER_D, 9.0 + 6.0 * tall)
        } else if tall > 0.25 {
            (&models::TOWER_B, 10.0 + 8.0 * tall)
        } else {
            (&models::BUILDING_N, 14.0 + 10.0 * tall)
        };
        models::put(p, m, v(x, base, z), a, unit, false);
    }
}

// ------------------------------------------------------------------ ilmari4/2: the autoscale floor

/// A rack that is being added: a bare frame going up in stages with a crate beside it.
fn rack_stack(p: &mut Plan, x: f32, z: f32, yaw: f32, tiers: usize) {
    for t in 0..tiers {
        p.framed(v(x, 2.2 * t as f32, z), yaw, |p| {
            p.boxed(v(0.0, 1.1, 0.0), v(0.9, 2.2, 1.0), Mat::Rack, t == 0);
            p.boxed(v(0.0, 1.1, 0.505), v(0.8, 2.0, 0.02), Mat::RackPanel, false);
            for k in 0..3 {
                p.boxed(
                    v(-0.25 + 0.25 * k as f32, 0.4 + 0.5 * k as f32, 0.52),
                    v(0.06, 0.05, 0.02),
                    if (k + t) % 2 == 0 {
                        Mat::BlinkGreenA
                    } else {
                        Mat::BlinkAmber
                    },
                    false,
                );
            }
        });
    }
}

/// A Reserved Instance rack: half of the bill's rate for half a minute, on a clock face.
fn reserved_rack(p: &mut Plan, key: &'static str, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 1.2, 0.0), v(1.3, 2.4, 1.0), Mat::Rack, true);
        p.boxed(
            v(0.0, 1.45, 0.51),
            v(1.1, 0.9, 0.02),
            Mat::ScreenGlow,
            false,
        );
        p.boxed(v(0.0, 0.7, 0.51), v(1.1, 0.5, 0.02), Mat::LedBlue, false);
        p.boxed(v(0.0, 2.55, 0.0), v(1.5, 0.12, 1.2), Mat::Gold, false);
        p.sign(v(0.0, 3.3, 0.6), 0.0, 2.6, Sign::IlReservedRack);
    });
    let (fx, fz) = (yaw.sin(), yaw.cos());
    p.interactable(key, v(x, 1.2, z), v(x + fx * 2.4, 0.0, z + fz * 2.4));
}

fn autoscale_floor(p: &mut Plan) -> Theme {
    casing(p, POD2, Mat::NeonCyan);
    // The hall: racks both sides, thin where the floor has not scaled yet, and gaps for the
    // three cells. The numbers on the signs go up.
    rack_row(p, -30.0, -23.0, -8.6, 0.0, false, false);
    rack_row(p, -17.0, 7.0, -8.6, 0.0, false, false);
    rack_row(p, 13.0, 15.0, -8.6, 0.0, false, false);
    rack_row(p, -30.0, 3.0, 8.6, PI, false, false);
    rack_row(p, 9.0, 15.0, 8.6, PI, false, false);
    for (x, z, yaw, n) in [
        (-28.0, -8.6, 0.0, 2),
        (-10.0, 8.6, PI, 3),
        (6.0, -8.6, 0.0, 2),
    ] {
        rack_stack(p, x, z + yaw.cos() * -1.1, yaw, n);
    }
    for x in [-24.0, -16.0, -6.0, 6.0, 12.0] {
        p.boxed(v(x, 3.5, 0.0), v(0.1, 0.5, 18.0), Mat::NeonCyan, false);
    }
    p.sign_near(-33.9, 0.0, 2.7, 4.2, Sign::IlScaleUp);
    p.sign_near(-22.0, -10.9, 2.7, 3.2, Sign::IlEgress);
    p.sign_near(-4.0, -10.9, 2.7, 3.2, Sign::IlFreeTier);
    p.sign_near(10.0, -10.9, 2.7, 3.2, Sign::IlSurprise);
    p.sign_near(-22.0, 10.9, 2.7, 3.2, Sign::IlSomeoneElse);
    p.sign_near(-4.0, 10.9, 2.7, 3.2, Sign::IlScaleUp);
    p.sign_near(10.0, 10.9, 2.7, 3.2, Sign::IlEgress);
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), false);
    arrival_kit(p, Sign::IlW4Back);
    // Cells: the provisioning bay (a conveyor of racks, a robot arm that is never idle), the
    // egress toll (a barrier, a gate, a sign about leaving) and the billing office.
    for k in 0..3 {
        models::put(
            p,
            &CONVEYOR_LONG,
            v(-26.0 + 3.6 * k as f32, 0.0, -31.2),
            0.0,
            1.8,
            true,
        );
    }
    models::put(p, &ROBOT_ARM_A, v(-14.4, 0.0, -29.0), PI, 2.6, true);
    models::put(p, &BOX_LARGE, v(-26.0, 0.45, -28.0), 0.3, 1.2, true);
    models::put(p, &BOX_OPEN, v(-23.0, 0.0, -28.6), -0.2, 1.4, true);
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlScaleUp);
    models::put(p, &BARRIER, v(5.0, 0.0, 25.0), FRAC_PI_2, 2.6, true);
    models::put(p, &BARRIER, v(7.0, 0.0, 22.5), 0.0, 2.6, true);
    models::put(p, &SIGN_WARNING, v(8.6, 0.0, 22.2), 0.0, 1.4, false);
    p.sign_near(6.0, 32.5, 2.6, 3.4, Sign::IlEgress);
    p.sign_near(6.0, 19.1, 2.7, 2.6, Sign::IlSurprise);
    for k in 0..4 {
        let x = 6.0 + k as f32 * 2.0 - 4.0;
        for j in 0..3 {
            models::put(
                p,
                &models::COIN_GOLD,
                v(x, 0.1 + 0.11 * j as f32, 28.6),
                0.4 * j as f32,
                1.6,
                false,
            );
        }
    }
    ingest_stack(p, 12.0, 29.0, 0.3);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlInvoice);
    for x in [6.0, 9.0, 12.0] {
        p.prop(&DESK, v(x, 0.0, -31.0), 0.0, true);
        p.boxed(v(x, 1.1, -31.3), v(0.9, 0.5, 0.05), Mat::ScreenGlow, false);
    }
    p.sign_near(10.0, -32.6, 2.8, 3.4, Sign::IlDashboard);
    water_cooler(p, 14.6, -32.2);
    sad_plant(p, 13.5, 32.0);
    // The pit: the bill over the door, four plinths where the replicas will stand, racks
    // stacked up the walls (the floor has been scaling for a while) and the Reserved racks.
    let (cx, cz) = (POD2.0, 0.0);
    floor_ring(p, cx, cz, 4.2, 20, Mat::NeonCyan, 0.1);
    floor_ring(p, cx, cz, 15.0, 44, Mat::NeonYellow, 0.08);
    for k in 0..4 {
        let (dx, dz) = polar(45.0 + 90.0 * k as f32, 9.5);
        floor_ring(p, cx + dx, cz + dz, 1.8, 14, Mat::NeonPink, 0.08);
        p.boxed(
            v(cx + dx, 0.02, cz + dz),
            v(2.4, 0.03, 2.4),
            Mat::LedBlue,
            false,
        );
    }
    for k in 0..7 {
        let z = cz - 15.0 + k as f32 * 5.0;
        // The west wall keeps its doorway (the corridor comes in at z = 0).
        if k != 3 {
            rack_stack(p, cx - 22.6, z, FRAC_PI_2, 1 + k % 3);
        }
        rack_stack(p, cx + 22.6, z, -FRAC_PI_2, 1 + (k + 1) % 3);
    }
    for k in 0..6 {
        let x = cx - 12.5 + k as f32 * 5.0;
        if k != 2 && k != 3 {
            rack_stack(p, x, cz - 19.8, 0.0, 1 + (k + 2) % 3);
        }
        rack_stack(p, x, cz + 19.8, PI, 1 + k % 3);
    }
    p.sign(v(cx - 23.7, 3.6, 0.0), FRAC_PI_2, 6.0, Sign::IlBillBoard);
    p.sign(v(cx + 23.7, 3.6, 0.0), -FRAC_PI_2, 6.0, Sign::IlDashboard);
    p.sign(v(cx - 23.7, 3.4, -8.0), FRAC_PI_2, 3.4, Sign::IlScaleUp);
    p.sign(v(cx - 23.7, 3.4, 8.0), FRAC_PI_2, 3.4, Sign::IlSurprise);
    reserved_rack(p, "prop:il_rack_1", cx - 3.0, cz - 18.4, 0.0);
    reserved_rack(p, "prop:il_rack_2", cx + 3.0, cz + 18.4, PI);
    // The invoice: a scroll that hangs from the ceiling the length of the hall, folded in
    // places, with the line items on boards down its length.
    for k in 0..12 {
        let x = -30.0 + k as f32 * 6.2;
        let z = if k % 2 == 0 { 1.6 } else { -1.6 };
        p.boxed(
            v(x + 3.1, 3.05 - 0.02 * k as f32, z),
            v(6.4, 0.02, 1.4),
            Mat::Paper,
            false,
        );
    }
    for k in 0..5 {
        p.sign(
            v(-26.0 + k as f32 * 15.0, 2.2, 0.0),
            FRAC_PI_2,
            2.4,
            Sign::IlInvoice,
        );
    }
    strips(p, Mat::LightPanel, Mat::NeonCyan, 0.0, 0.1);
    cast(
        p,
        POD2,
        [
            ("door:ilmari4/1", Sign::IlW4Back),
            ("door:ilmari4/3", Sign::IlW4ToCloud),
        ],
    );
    theme(2)
}

// ------------------------------------------------------------------ ilmari4/3: the server cloud

/// The unlit status page of a region platform: a pole, a stack of racks and the region's sign
/// facing the hub. `deg` is the region's direction from the hub, `at` its centre.
fn region_dress(p: &mut Plan, k: usize, at: (f32, f32)) {
    let deg = REGIONS[k].1;
    let out = polar(deg, 1.0);
    let toward_hub = out.1.atan2(out.0) + PI;
    let yaw = FRAC_PI_2 - toward_hub;
    // The ring of lights and the status pad (the sequence paints over them from `wing4_fx`).
    floor_ring(p, at.0, at.1, REGION_R - 0.8, 28, Mat::NeonCyan, 0.14);
    floor_ring(p, at.0, at.1, REGION_R - 2.4, 18, Mat::LedGreen, 0.1);
    // The island under it: a tapering stack, like a server tower hung out of the sky.
    for (j, (d, h)) in [
        (2.0 * REGION_R, 1.0),
        (1.5 * REGION_R, 2.2),
        (0.9 * REGION_R, 3.4),
    ]
    .into_iter()
    .enumerate()
    {
        let y = -0.5 - [0.0, 1.6, 4.0][j];
        p.prim(
            PrimKind::Cylinder,
            v(at.0, y, at.1),
            v(d, h, d),
            if j == 0 { Mat::Metal } else { Mat::Dark },
        );
    }
    // Racks on the outer rim, the sign on a pole beyond them facing in.
    let (ox, oz) = (at.0 + out.0 * 5.2, at.1 + out.1 * 5.2);
    for side in [-1.6_f32, 0.0, 1.6] {
        let (sx, sz) = (-out.1 * side, out.0 * side);
        server_rack(p, ox + sx, oz + sz, yaw);
    }
    let (bx, bz) = (at.0 + out.0 * 6.9, at.1 + out.1 * 6.9);
    let sign = [
        Sign::IlRegionUsEast,
        Sign::IlRegionEuWest,
        Sign::IlRegionApSouth,
        Sign::IlRegionSaEast,
        Sign::IlRegionAfSouth,
    ][k];
    board(p, bx, bz, 3.6, yaw, 5.6, sign);
    // A joke each, off to the side of the sign.
    let joke = [
        Sign::IlNotBug,
        Sign::IlMultiRegion,
        Sign::IlSomeoneElse,
        Sign::IlSla,
        Sign::IlEgress,
    ][k];
    let (jx, jz) = (bx - out.1 * 4.2, bz + out.0 * 4.2);
    board(p, jx, jz, 1.7, yaw, 2.8, joke);
}

fn server_cloud(p: &mut Plan) -> Theme {
    // The hall with its three cells is the Status Page lobby: a reception, some monitors that
    // are green, desks of people who say "can you try again".
    p.ceiling = 3.2;
    cloud_outline(p);
    p.build_walls();
    wall_dress_hall(p);
    arrival_kit(p, Sign::IlW4Back);
    // Hall dressing: rows of "support" desks with the same screen, a reception, a wall of
    // monitors that cannot go red.
    for k in 0..6 {
        let x = -28.0 + k as f32 * 8.0;
        for z in [-7.6_f32, 7.6] {
            p.framed(v(x, 0.0, z), if z < 0.0 { 0.0 } else { PI }, |p| {
                p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
                p.boxed(v(0.0, 1.0, -0.1), v(0.9, 0.5, 0.06), Mat::ScreenGlow, false);
                p.boxed(v(0.0, 0.34, 1.0), v(0.5, 0.06, 0.5), Mat::Fabric0, false);
            });
        }
    }
    for x in [-26.0, -10.0, 4.0] {
        p.boxed(v(x, 3.0, 0.0), v(0.1, 0.5, 18.0), Mat::NeonCyan, false);
    }
    p.sign_near(-33.9, 0.0, 2.7, 4.2, Sign::IlStatusGreen);
    p.sign_near(-22.0, -10.9, 2.7, 3.2, Sign::IlSomeoneElse);
    p.sign_near(-4.0, -10.9, 2.7, 3.2, Sign::IlMultiRegion);
    p.sign_near(8.0, -10.9, 2.7, 3.2, Sign::IlNotBug);
    p.sign_near(-22.0, 10.9, 2.7, 3.2, Sign::IlEgress);
    p.sign_near(-4.0, 10.9, 2.7, 3.2, Sign::IlSla);
    p.sign_near(-33.0, -10.9, 2.7, 3.0, Sign::IlStatusGreen);
    cable_tray(p, pt(-32.0, 0.0), pt(10.0, 0.0), false);
    // Cells: a break room with an egress vending machine, the status page's own closet (one
    // green wall) and a billing office.
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlEgress);
    p.prop(&TABLE_ROUND, v(-20.0, 0.0, -29.5), 0.0, true);
    water_cooler(p, -14.6, -32.4);
    p.sign_near(6.0, 19.1, 2.7, 2.6, Sign::IlStatusGreen);
    for k in 0..3 {
        p.sign(
            v(2.0 + k as f32 * 3.4, 2.1, 32.6),
            PI,
            3.0,
            Sign::IlStatusGreen,
        );
    }
    ingest_stack(p, 12.0, 29.0, 0.2);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlDashboard);
    for x in [6.0, 9.0, 12.0] {
        p.prop(&DESK, v(x, 0.0, -31.0), 0.0, true);
        p.boxed(v(x, 1.1, -31.3), v(0.9, 0.5, 0.05), Mat::ScreenGlow, false);
    }
    sad_plant(p, 13.5, -32.0);
    // The hub: a dais of rings, a tower of racks the boss hovers over, the status pylons.
    let (hx, hz) = HUB;
    floor_ring(p, hx, hz, 3.0, 18, Mat::NeonYellow, 0.14);
    floor_ring(p, hx, hz, 6.0, 30, Mat::NeonCyan, 0.14);
    floor_ring(p, hx, hz, HUB_R - 0.7, 44, Mat::LedGreen, 0.14);
    for (j, (d, h)) in [
        (2.0 * HUB_R, 1.0),
        (1.6 * HUB_R, 2.4),
        (1.0 * HUB_R, 3.8),
        (0.5 * HUB_R, 5.0),
    ]
    .into_iter()
    .enumerate()
    {
        let y = -0.5 - [0.0, 1.7, 4.4, 8.1][j];
        p.prim(
            PrimKind::Cylinder,
            v(hx, y, hz),
            v(d, h, d),
            if j == 0 { Mat::Metal } else { Mat::Dark },
        );
    }
    for (dx, dz, yaw) in status_pylons() {
        p.boxed(
            v(hx + dx, 1.5, hz + dz),
            v(0.3, 3.0, 0.3),
            Mat::Metal,
            false,
        );
        // The screen of a pylon is drawn by `wing4_fx`; this is its dead frame.
        p.framed(v(hx + dx, 0.0, hz + dz), yaw, |p| {
            p.boxed(v(0.0, 3.1, 0.05), v(3.0, 1.7, 0.12), Mat::Dark, false);
        });
    }
    // The regions and the bridges.
    for k in 0..REGIONS.len() {
        let (dx, dz) = region_offset(k);
        region_dress(p, k, (hx + dx, hz + dz));
        let ((bx, bz), deg) = bridge_offset(k);
        let yaw = -deg.to_radians();
        // Bridge: a lit deck line and a rail of cables either side.
        for side in [-1.0_f32, 1.0] {
            let (sx, sz) = polar(deg + 90.0, side * (BRIDGE.1 - 0.2));
            p.slab(
                v(hx + bx + sx, 0.012, hz + bz + sz),
                yaw,
                v(2.0 * BRIDGE.0 - 1.0, 0.012, 0.12),
                Mat::NeonCyan,
            );
        }
        floor_line(
            p,
            pt(
                hx + bx - polar(deg, BRIDGE.0).0,
                hz + bz - polar(deg, BRIDGE.0).1,
            ),
            pt(
                hx + bx + polar(deg, BRIDGE.0).0,
                hz + bz + polar(deg, BRIDGE.0).1,
            ),
            0.3,
            Mat::Frosted,
        );
    }
    // The sky: clouds under and around the platforms, towers sunk into them.
    for k in 0..34 {
        let a = (k as f32 * 37.0 + 5.0).to_radians();
        let r = 18.0 + 70.0 * rnd(k, 11);
        let (x, z) = (hx + r * a.cos() * 1.4, hz + r * a.sin());
        cloud(p, x, -5.0 - 14.0 * rnd(k, 13), z, 1.2 + 1.6 * rnd(k, 17));
    }
    skyline(p, 3);
    let (ax, az) = region_offset(2);
    let packs = CELLS.map(|(x, z)| (x, z - z.signum()));
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:ilmari4/2", (-50.0, 0.0), Sign::IlW4Back),
                (
                    "door:campus_occupied",
                    (hx + ax + 14.0, hz + az),
                    Sign::IlW4Out,
                ),
            ],
            packs: &packs,
            boss: (hx, hz),
            npc: None,
        },
    );
    theme(3)
}

/// The Server Cloud's outline: arrival pod, corridor, the lobby hall with its three cells, then
/// the hub, five regions and five bridges. Region and hub rails are glass; the hall is walled.
fn cloud_outline(p: &mut Plan) {
    let w = Mat::Wall;
    let g = Mat::Glass;
    let ((hx, hhx, hhz), a) = (HALL, -44.0);
    p.fp.pod(rounded_rect(pt(a, 0.0), 6.0, 6.0, 3.0, 0.0), w);
    p.fp.link(
        super::shape::stroke(&[pt(a + 6.0, 0.0), pt(hx - hhx + 2.0, 0.0)], 4.0),
        w,
    );
    p.fp.pod(rounded_rect(pt(hx, 0.0), hhx, hhz, 6.0, 0.0), w);
    for (cx, cz) in CELLS {
        p.fp.pod(rounded_rect(pt(cx, cz), 8.0, 7.0, 3.0, 0.0), w);
        let edge = hhz * cz.signum();
        p.fp.link(
            super::shape::stroke(&[pt(cx, edge), pt(cx, cz - 4.0 * cz.signum())], 4.0),
            w,
        );
    }
    p.fp.pod(regular(pt(HUB.0, HUB.1), HUB_R, 44, 0.0), g);
    for k in 0..REGIONS.len() {
        let (dx, dz) = region_offset(k);
        p.fp.pod(regular(pt(HUB.0 + dx, HUB.1 + dz), REGION_R, 40, 0.0), g);
        let ((bx, bz), deg) = bridge_offset(k);
        p.fp.link(
            rounded_rect(pt(HUB.0 + bx, HUB.1 + bz), BRIDGE.0, BRIDGE.1, 0.4, deg),
            g,
        );
    }
    // Exit annex on ap-south-1: a service lift to the ladder, in a small opaque box.
    let (ax, az) = region_offset(2);
    p.fp.pod(
        rounded_rect(pt(HUB.0 + ax + 10.0, HUB.1 + az), 3.5, 3.5, 1.0, 0.0),
        w,
    );
    p.route = vec![
        pt(a - 4.0, 0.0),
        pt(a + 6.0, 0.0),
        pt(hx, 0.0),
        pt(HUB.0 + region_offset(0).0, HUB.1),
        pt(HUB.0, HUB.1),
    ];
}

/// The lobby's wall dress on the opaque walls only (the platforms' rails are glass).
fn wall_dress_hall(p: &mut Plan) {
    super::offsite::wall_dress(p, Mat::Beige, Mat::Trim, Mat::Dark);
    super::offsite::cove(p, Mat::NeonCyan);
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat_core::{PlatformKind, PlatformShape};

    /// The drawn platforms are the ones the sim reads: same centres, sizes and kinds.
    #[test]
    fn the_layout_matches_zone_def_platforms() {
        let c = combat_core::Content::from_ron(include_str!("../../../../content/game.ron"))
            .expect("content");
        let z = c.zone("ilmari4/3").expect("zone");
        let want = |id: &str| z.platforms.iter().find(|p| p.id == id).expect(id);
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        let hub = want("hub");
        assert!(near(hub.x, 0.0) && near(hub.z, 0.0));
        assert_eq!(hub.shape, PlatformShape::Disc { r: HUB_R });
        for (k, (id, _)) in REGIONS.iter().enumerate() {
            let d = want(id);
            let (x, zz) = region_offset(k);
            assert!(
                near(d.x, x) && near(d.z, zz),
                "{id}: {} {} vs {x} {zz}",
                d.x,
                d.z
            );
            assert_eq!(d.shape, PlatformShape::Disc { r: REGION_R });
            assert_eq!(d.kind, PlatformKind::Region);
            let b = want(&format!("bridge_{}", k + 1));
            let ((bx, bz), deg) = bridge_offset(k);
            assert!(
                near(b.x, bx) && near(b.z, bz),
                "{} {} vs {bx} {bz}",
                b.x,
                b.z
            );
            match b.shape {
                PlatformShape::Rect { hx, hz, yaw_deg } => {
                    assert!(near(hx, BRIDGE.0) && near(hz, BRIDGE.1) && near(yaw_deg, deg));
                }
                other => panic!("bridge shape {other:?}"),
            }
            assert_eq!(b.kind, PlatformKind::Bridge);
        }
        assert_eq!(z.platforms.len(), 11);
    }

    /// The boss stands at the hub centre, on the platform the sim calls the hub.
    #[test]
    fn the_hub_is_under_the_boss_and_the_regions_are_on_the_footprint() {
        let l = build(3);
        let boss = l.anchor("boss").expect("boss").0;
        assert!((boss.x - HUB.0).abs() < 0.01 && (boss.z - HUB.1).abs() < 0.01);
        for k in 0..REGIONS.len() {
            let (dx, dz) = region_offset(k);
            assert!(l.footprint.contains(Vec2::new(HUB.0 + dx, HUB.1 + dz)));
            let ((bx, bz), _) = bridge_offset(k);
            assert!(l.footprint.contains(Vec2::new(HUB.0 + bx, HUB.1 + bz)));
        }
        // Between the regions there is nothing: the void is real.
        let (x, z) = polar(144.0, 25.0);
        assert!(!l.footprint.contains(Vec2::new(HUB.0 + x, HUB.1 + z)));
    }
}
