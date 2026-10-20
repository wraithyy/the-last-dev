//! Kenney kit dressing over the campus districts (`models.rs`): the lake shore with its pier,
//! boats and reeds, the train at the station, shops along Synergy Boulevard, the coworking
//! towers, the Old Campus water tower and tanks, the excavator on the New HQ site, the commuter
//! houses and the food on the picnic tables. Repeated models share one mesh and material each,
//! so the shore stones and reeds batch.

use super::districts::{LAKE, LAKE_R, abs};
use super::kit::Plan;
use super::models::*;
use super::themes::v;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The water surface (`districts::WATER_Y`).
const WATER_Y: f32 = -0.45;

pub(super) fn build(p: &mut Plan) {
    creek_banks(p);
    station_train(p);
    boulevard_shops(p);
    incubator_towers(p);
    old_campus(p);
    construction(p);
    commuter_houses(p);
    picnic_food(p);
    park_rocks(p);
    // Solar panels on the villa roof: the board's ESG report has a photo of them.
    put(p, &SOLAR_PANELS, v(213.0, 6.95, 144.0), 0.0, 5.0, false);
}

/// The lake: shore stones round the edge (gaps where the creek comes in and leaves), reeds in
/// clumps, lilies on the water, a plank pier from the north shore with a rowing boat tied to
/// it, a sailing boat nobody sails, buoys and a canoe pulled up on the bank.
pub(super) fn lake(p: &mut Plan) {
    let (lx, lz) = LAKE;
    for k in 0..36 {
        let a = k as f32 * TAU / 36.0;
        // East is the creek inflow, south the outflow.
        let near = |t: f32| (a - t).abs() < 0.22 || (a - t - TAU).abs() < 0.22;
        if near(0.0) || near(FRAC_PI_2) {
            continue;
        }
        let r = LAKE_R + 0.6 + (pick(a, 1.0, 3) as f32) * 0.5;
        let (x, z) = (lx + r * a.cos(), lz + r * a.sin());
        let (m, unit) = [
            (&SHORE_STONE, 2.2),
            (&ROCK_SMALL, 5.0),
            (&ROCK_FLAT, 2.4),
            (&ROCK_SMALL, 6.0),
        ][pick(x, z, 4)];
        // Sat on the bank: big enough to break the waterline, small enough to read as a shore.
        put(p, m, v(x, 0.0, z), a * 3.0, unit, false);
    }
    for (a0, n) in [(2.3, 5), (3.6, 4), (4.4, 6), (5.6, 4)] {
        for i in 0..n {
            let a = a0 + i as f32 * 0.07;
            let r = LAKE_R - 0.4 + (i % 2) as f32 * 0.8;
            let (x, z) = (lx + r * a.cos(), lz + r * a.sin());
            put(p, &REEDS, abs(p, x, WATER_Y - 0.2, z), a * 5.0, 2.4, false);
        }
    }
    for (dx, dz) in [
        (-6.0, 4.0),
        (-9.0, 8.0),
        (5.0, 9.0),
        (8.0, -6.0),
        (-3.0, -10.0),
        (10.0, 3.0),
        (-11.0, -3.0),
    ] {
        put(
            p,
            &LILY_LARGE,
            abs(p, lx + dx, WATER_Y + 0.02, lz + dz),
            dx,
            2.6,
            false,
        );
    }
    // The pier: planks from the bank out over the water, posts at the corners.
    for k in 0..7 {
        let z = lz - 21.5 + k as f32 * 1.0;
        put(p, &PATH_WOOD, abs(p, lx, 0.2, z), 0.0, 2.2, false);
    }
    for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let at = abs(p, lx + dx, -0.6, lz - 17.0 + dz);
        p.boxed(at, v(0.16, 1.6, 0.16), super::kit::Mat::Wood, false);
    }
    put(
        p,
        &BOAT_ROW_SMALL,
        abs(p, lx + 2.4, WATER_Y - 0.25, lz - 15.0),
        0.3,
        1.0,
        false,
    );
    put(
        p,
        &BOAT_SAIL_A,
        abs(p, lx - 5.0, WATER_Y - 0.3, lz + 3.0),
        2.2,
        1.0,
        false,
    );
    for (dx, dz) in [(7.0, 7.0), (-8.0, -7.0), (2.0, 12.0)] {
        put(
            p,
            &BUOY,
            abs(p, lx + dx, WATER_Y - 0.6, lz + dz),
            0.0,
            0.7,
            false,
        );
    }
    put(p, &CANOE, v(lx - 21.0, 0.05, lz + 7.0), 0.4, 3.4, false);
    for k in 0..14 {
        let a = 0.3 + k as f32 * 0.43;
        let (x, z) = (lx + 21.0 * a.cos(), lz + 21.0 * a.sin());
        put(p, &GRASS_LARGE, v(x, 0.0, z), a, 5.0, false);
    }
    put(p, &LOG_LARGE, v(lx - 19.0, 0.0, lz - 12.0), 1.1, 2.4, false);
}

/// Stones and reeds along the creek, alternating banks.
fn creek_banks(p: &mut Plan) {
    for k in 0..16 {
        let z = -150.0 + k as f32 * 9.0;
        if (-60.0..-40.0).contains(&z) || (-120.0..-104.0).contains(&z) {
            continue;
        }
        let s = if k % 2 == 0 { -1.0 } else { 1.0 };
        let x = -104.0 + s * 6.8;
        put(p, &ROCK_FLAT, v(x, 0.0, z), z, 2.0, false);
        put(
            p,
            &REEDS,
            abs(p, -104.0 + s * 4.0, WATER_Y - 0.2, z + 3.0),
            z,
            2.2,
            false,
        );
    }
    for k in 0..12 {
        let z = 30.0 + k as f32 * 11.0;
        if (44.0..60.0).contains(&z) || (132.0..148.0).contains(&z) {
            continue;
        }
        let s = if k % 2 == 0 { -1.0 } else { 1.0 };
        put(p, &SHORE_STONE, v(-150.0 + s * 6.8, 0.0, z), z, 2.2, false);
        put(
            p,
            &REEDS,
            abs(p, -150.0 + s * 4.0, WATER_Y - 0.2, z - 3.0),
            z,
            2.2,
            false,
        );
    }
}

/// A city train waiting at the platform; it has been "arriving shortly" since the reorg.
fn station_train(p: &mut Plan) {
    let unit = 2.6;
    put(p, &TRAIN_HEAD, v(166.0, 0.2, -158.0), FRAC_PI_2, unit, true);
    for k in 0..3 {
        let x = 159.5 - k as f32 * 7.1;
        put(p, &TRAIN_CAR, v(x, 0.2, -158.0), FRAC_PI_2, unit, true);
    }
}

/// Retail on the boulevard between the avenue and the gate: everything a campus needs, which
/// is a juice bar, a bank branch nobody uses and a store selling the company hoodie.
fn boulevard_shops(p: &mut Plan) {
    put(p, &BUILDING_J, v(-20.0, 0.0, 108.0), FRAC_PI_2, 6.0, true);
    put(p, &BUILDING_N, v(-21.0, 0.0, 124.0), FRAC_PI_2, 5.0, true);
    put(p, &BUILDING_G, v(19.0, 0.0, 106.0), -FRAC_PI_2, 8.0, true);
    put(p, &BUILDING_A, v(19.0, 0.0, 117.0), -FRAC_PI_2, 8.0, true);
    put(p, &BUILDING_A, v(19.0, 0.0, 126.5), -FRAC_PI_2, 7.0, true);
    for z in [104.0, 133.0] {
        put(p, &PLANTER, v(-12.0, 0.0, z), 0.0, 4.0, true);
        put(p, &PLANTER, v(12.0, 0.0, z), 0.0, 4.0, true);
    }
}

/// The coworking towers behind Incubator Street: glass, logo spots for sale on every floor.
fn incubator_towers(p: &mut Plan) {
    put(p, &TOWER_D, v(147.0, 0.0, -30.0), -FRAC_PI_2, 7.0, true);
    put(p, &TOWER_B, v(147.0, 0.0, -6.0), -FRAC_PI_2, 7.0, true);
}

/// Old Campus extras: the water tower, the tanks behind the depot, a second warehouse, crates
/// and the robot arm the depot bought in 2009 and never plugged in.
fn old_campus(p: &mut Plan) {
    put(p, &WATER_TOWER, v(-88.0, 0.0, -128.0), 0.3, 6.0, true);
    put(p, &TANK_LARGE, v(70.0, 0.0, -132.0), 0.0, 5.0, true);
    put(p, &WAREHOUSE_Q, v(-62.0, 0.0, -74.0), PI, 8.0, true);
    put(p, &WAREHOUSE_L, v(-82.0, 0.0, -96.0), FRAC_PI_2, 7.0, true);
    put(p, &ROBOT_ARM_A, v(24.0, 0.0, -120.5), 0.6, 1.5, true);
    for (x, z) in [(56.0, -120.5), (58.5, -119.0), (57.0, -117.5)] {
        put(p, &BOX_LARGE, v(x, 0.0, z), x, 1.6, true);
    }
}

/// The New HQ site: an excavator parked since the groundbreaking photo, and cones.
fn construction(p: &mut Plan) {
    put(p, &TRACTOR_SHOVEL, v(-150.0, 0.0, -122.0), 2.4, 2.2, true);
    for k in 0..6 {
        put(
            p,
            &CONE,
            v(-162.0 + k as f32 * 3.0, 0.0, -63.5),
            0.0,
            1.4,
            false,
        );
    }
}

/// Houses on the far side of the commuter lane: the people who could afford a house commute
/// in from here, forty seconds further than the pods.
fn commuter_houses(p: &mut Plan) {
    put(p, &HOUSE_A, v(226.0, 0.0, -36.0), -FRAC_PI_2, 7.0, true);
    put(p, &HOUSE_T, v(226.0, 0.0, 4.0), -FRAC_PI_2, 6.0, true);
    put(p, &HOUSE_N, v(226.0, 0.0, 50.0), -FRAC_PI_2, 5.0, true);
    for z in [-28.0, 12.0, 60.0] {
        put(p, &FENCE_1X4, v(221.0, 0.0, z), FRAC_PI_2, 4.0, false);
    }
}

/// Parasols over every other picnic table at the trucks, lunch left on a few.
fn picnic_food(p: &mut Plan) {
    for x in [12.0, 24.0, 36.0, 48.0] {
        put(p, &PARASOL_A, v(x, 0.0, 78.0), x, 7.0, false);
    }
    for (x, z, m) in [
        (18.0, 70.0, &BURGER),
        (18.3, 69.8, &SODA_CAN),
        (42.0, 78.0, &CUP_COFFEE),
        (36.0, 70.0, &BURGER),
    ] {
        put(p, m, v(x, 0.79, z), x, 0.45, false);
    }
}

/// Rocks, bushes and flowers on the park hills and round the benches.
fn park_rocks(p: &mut Plan) {
    for (x, z, m, u) in [
        (-206.0, -30.0, &ROCK_TALL, 3.0),
        (-190.0, -14.0, &ROCK_BIG, 2.6),
        (-212.0, -12.0, &ROCK_SMALL, 4.0),
        (-196.0, 118.0, &ROCK_BIG, 2.4),
        (-176.0, 40.0, &ROCK_TALL, 2.2),
        (-128.0, 30.0, &ROCK_FLAT, 2.6),
    ] {
        put(p, m, v(x, 0.0, z), x + z, u, true);
    }
    for (x, z) in [
        (-174.0, -2.0),
        (-150.0, -29.0),
        (-163.0, 13.0),
        (-136.0, 44.0),
        (-184.0, 58.0),
        (-204.0, 26.0),
    ] {
        put(p, &BUSH_LARGE, v(x, 0.0, z), x, 6.0, false);
        put(p, &FLOWER_YELLOW, v(x + 1.4, 0.0, z + 0.6), z, 5.0, false);
        put(p, &FLOWER_RED, v(x - 1.2, 0.0, z - 0.8), x, 5.0, false);
    }
    for (x, z) in [(-30.0, 70.0), (-14.0, 96.0), (-36.0, 90.0)] {
        put(p, &BUSH, v(x, 0.0, z), x, 6.0, false);
    }
}
