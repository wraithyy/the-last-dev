//! Entrance dressing for the campus dungeons and raids: each door is built so that it reads from
//! 30 m away as what it leads to (a conference hall, a coach stop, a loading dock, an executive
//! entrance, a hacker loft, a new-starter queue). Everything here is decoration in the frame of
//! the entrance it belongs to, built from the Kenney kits already in `assets/models/` and plain
//! boxes; only a few pieces are solid, and none sits in a door's trigger.
//!
//! Frames: inside `Plan::framed` only `boxed`, `prim`, `prim_yawed`, `sign`, `block` and `put`
//! follow the frame (slabs and merged parts do not), so those are all this file uses.

use super::kit::*;
use super::models::*;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// A rope stanchion: chrome post with a ball on top.
fn stanchion(p: &mut Plan, x: f32, z: f32, mat: Mat) {
    p.prim(PrimKind::Cylinder, v(x, 0.5, z), v(0.1, 1.0, 0.1), mat);
    p.prim(PrimKind::Sphere, v(x, 1.05, z), v(0.17, 0.17, 0.17), mat);
}

/// A bike standing on its wheels, long along local x (or z): two wheel discs, frame bars, bars.
fn bike(p: &mut Plan, x: f32, z: f32, along_x: bool, frame: Mat) {
    let (lx, lz) = if along_x { (1.0, 0.0) } else { (0.0, 1.0) };
    for s in [-0.55, 0.55] {
        let size = if along_x {
            v(0.7, 0.7, 0.05)
        } else {
            v(0.05, 0.7, 0.7)
        };
        p.prim(
            PrimKind::Sphere,
            v(x + s * lx, 0.35, z + s * lz),
            size,
            Mat::Tire,
        );
    }
    let bar = |len: f32| {
        if along_x {
            v(len, 0.05, 0.05)
        } else {
            v(0.05, 0.05, len)
        }
    };
    p.boxed(v(x, 0.62, z), bar(1.0), frame, false);
    p.boxed(v(x - 0.2 * lx, 0.4, z - 0.2 * lz), bar(0.6), frame, false);
    // Saddle post and handlebar stem, with the bars across the frame.
    p.boxed(
        v(x - 0.45 * lx, 0.8, z - 0.45 * lz),
        v(0.05, 0.4, 0.05),
        frame,
        false,
    );
    p.boxed(
        v(x + 0.5 * lx, 0.85, z + 0.5 * lz),
        v(0.05, 0.5, 0.05),
        frame,
        false,
    );
    let across = if along_x {
        v(0.05, 0.05, 0.5)
    } else {
        v(0.5, 0.05, 0.05)
    };
    p.boxed(v(x + 0.5 * lx, 1.1, z + 0.5 * lz), across, Mat::Dark, false);
}

/// A tall banner pole with a hanging pennon, local +z side.
fn banner_pole(p: &mut Plan, x: f32, z: f32, mat: Mat, stripe: Mat) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 3.5, z),
        v(0.12, 7.0, 0.12),
        Mat::Chrome,
    );
    p.boxed(v(x, 6.9, z + 0.6), v(0.06, 0.06, 1.2), Mat::Chrome, false);
    p.boxed(v(x, 5.0, z + 0.65), v(0.05, 3.6, 1.1), mat, false);
    p.boxed(
        v(x + 0.04, 6.2, z + 0.65),
        v(0.05, 0.25, 1.1),
        stripe,
        false,
    );
    p.boxed(
        v(x + 0.04, 4.0, z + 0.65),
        v(0.05, 0.25, 1.1),
        stripe,
        false,
    );
}

/// VibeCon: the hall behind the backdrop wall, in the venue's frame (the wall stands at x 33, the
/// door in it at z 16, the queue runs in from the west). Banners hang on the hall's glass front,
/// flag poles line the street side, a red carpet leads in along the queue and a lanyard stand
/// waits at the start of it.
pub(super) fn vibecon_hall(p: &mut Plan) {
    const WALL_X: f32 = 33.0;
    const Z: f32 = 16.0;
    let neon = [Mat::NeonYellow, Mat::LedGreen, Mat::NeonCyan, Mat::NeonPink];
    // The hall itself: 16 x 9 x 24 m behind the wall, a glass ribbon over the wall's top, a roof
    // with a neon front edge.
    p.boxed(v(WALL_X + 8.2, 4.5, Z), v(16.0, 9.0, 24.0), Mat::Wall, true);
    p.boxed(
        v(WALL_X + 0.17, 6.9, Z),
        v(0.05, 1.8, 23.0),
        Mat::Window,
        false,
    );
    p.boxed(
        v(WALL_X + 8.2, 9.15, Z),
        v(16.6, 0.3, 24.6),
        Mat::Trim,
        false,
    );
    p.boxed(
        v(WALL_X + 0.08, 9.0, Z),
        v(0.12, 0.14, 24.6),
        Mat::NeonPink,
        false,
    );
    p.boxed(
        v(WALL_X + 0.08, 4.45, Z),
        v(0.12, 0.1, 24.0),
        Mat::NeonCyan,
        false,
    );
    // Three big banners over the wall, readable from the street.
    for dz in [-8.0, 0.0, 8.0] {
        p.sign(
            v(WALL_X + 0.1, 6.9, Z + dz),
            Face::NegX.yaw(),
            4.6,
            Sign::VibeConBanner,
        );
    }
    // Flag poles in front of the hall, four banners in event colours.
    for (k, dz) in [-10.5, -6.5, 6.5, 10.5].into_iter().enumerate() {
        let mat = [Mat::CarRed, Mat::CarBlue, Mat::CarTeal, Mat::CarRed][k];
        banner_pole(p, 30.6, Z + dz - 0.65, mat, neon[k]);
    }
    // Red carpet from the queue to the door, under the neon ropes.
    p.boxed(v(27.5, 0.075, Z), v(11.0, 0.03, 2.4), Mat::Red, false);
    // Lanyard stand at the start of the queue: a post with a rail of lanyards and a stack of
    // badge boxes on the floor.
    let (sx, sz) = (24.8, Z + 5.2);
    p.boxed(v(sx, 0.95, sz), v(0.07, 1.9, 0.07), Mat::Metal, false);
    p.boxed(v(sx, 1.85, sz), v(1.1, 0.05, 0.05), Mat::Metal, false);
    p.boxed(v(sx, 0.04, sz), v(0.6, 0.08, 0.6), Mat::Dark, false);
    for k in 0..9 {
        p.boxed(
            v(sx - 0.5 + k as f32 * 0.125, 1.5, sz),
            v(0.035, 0.7, 0.02),
            neon[k % 4],
            false,
        );
        p.boxed(
            v(sx - 0.5 + k as f32 * 0.125, 1.12, sz),
            v(0.09, 0.12, 0.02),
            Mat::White,
            false,
        );
    }
    put(p, &BOX_SMALL, v(sx + 1.0, 0.0, sz + 0.2), 0.3, 0.9, false);
    put(p, &BOX_SMALL, v(sx + 1.0, 0.25, sz + 0.2), 0.7, 0.9, false);
    // Sponsor standees along the street side of the queue.
    for dz in [-1.0, 1.0] {
        p.boxed(
            v(19.6, 1.1, Z + 8.5 + dz * 2.5),
            v(0.1, 2.2, 1.4),
            Mat::Dark,
            false,
        );
        p.boxed(
            v(19.52, 1.4, Z + 8.5 + dz * 2.5),
            v(0.04, 1.2, 1.1),
            neon[(dz > 0.0) as usize + 1],
            false,
        );
    }
}

/// The Offsite coach stop, in the pad's frame (pad 15 x 9 m around (7, 34.5), the coach along
/// its south edge at z 37, the boarding door on its north side). Adds a welcome gantry with
/// bunting over the way in, luggage waiting to be loaded, bay markings, cones and the coach's
/// own details (roof unit, luggage bays, destination board).
pub(super) fn coach_stop(p: &mut Plan) {
    const COACH_Z: f32 = 37.0;
    let side = COACH_Z - 1.3;
    // Coach details.
    p.boxed(
        v(8.5, 2.95, COACH_Z),
        v(3.2, 0.3, 1.5),
        Mat::CarWhite,
        false,
    );
    for x in [3.6, 7.0, 10.2] {
        p.boxed(v(x, 0.95, side - 0.04), v(1.8, 0.8, 0.03), Mat::Dark, false);
    }
    p.sign(
        v(12.54, 2.5, COACH_Z),
        Face::PosX.yaw(),
        2.0,
        Sign::OffsiteShuttle,
    );
    for dz in [-0.8, 0.8] {
        p.boxed(
            v(12.52, 0.9, COACH_Z + dz),
            v(0.06, 0.25, 0.4),
            Mat::NeonYellow,
            false,
        );
    }
    // Gantry over the walk in from the north, with a string of pennants.
    for x in [2.2, 11.8] {
        p.boxed(v(x, 3.4, 29.2), v(0.25, 6.8, 0.25), Mat::Metal, false);
        p.block(v(x, 1.4, 29.2), v(0.4, 2.8, 0.4));
    }
    // Sign above the beam, so the coach stays visible under it.
    p.boxed(v(7.0, 6.7, 29.2), v(10.2, 0.3, 0.3), Mat::Metal, false);
    p.boxed(v(7.0, 8.0, 29.2), v(6.4, 2.2, 0.15), Mat::Dark, false);
    for x in [4.4, 9.6] {
        p.boxed(v(x, 7.35, 29.2), v(0.15, 1.1, 0.15), Mat::Metal, false);
    }
    for dz in [-0.1, 0.1] {
        p.sign(
            v(7.0, 8.0, 29.2 + dz),
            if dz < 0.0 { PI } else { 0.0 },
            6.0,
            Sign::OffsiteWelcome,
        );
    }
    let colors = [Mat::NeonYellow, Mat::NeonCyan, Mat::NeonPink, Mat::LedGreen];
    for k in 0..12 {
        p.prim(
            PrimKind::Pennant,
            v(2.8 + k as f32 * 0.75, 6.35, 29.2),
            v(0.5, 0.6, 1.0),
            colors[k % 4],
        );
    }
    // Luggage waiting by the coach door: suitcases and a trolley.
    for (x, z, c, h) in [
        (9.2, 33.6, Mat::CarRed, 0.7),
        (9.9, 33.9, Mat::CarBlue, 0.6),
        (10.6, 33.5, Mat::Dark, 0.8),
        (10.1, 33.7, Mat::Yellow, 0.35),
    ] {
        let y = if c == Mat::Yellow { 0.8 } else { 0.0 };
        p.boxed(v(x, y + h / 2.0 + 0.04, z), v(0.5, h, 0.28), c, false);
    }
    p.boxed(v(10.0, 0.06, 33.7), v(1.9, 0.08, 0.9), Mat::Metal, false);
    p.boxed(v(10.9, 0.9, 33.7), v(0.05, 1.7, 0.7), Mat::Metal, false);
    // Bay markings: a hazard line along the kerb and bay stripes for the coach.
    for k in 0..8 {
        p.boxed(
            v(0.5 + k as f32 * 1.8, 0.075, 39.0),
            v(0.9, 0.02, 0.18),
            Mat::Yellow,
            false,
        );
    }
    for x in [0.0, 14.0] {
        p.boxed(v(x, 0.075, 37.0), v(0.18, 0.02, 4.0), Mat::White, false);
    }
    for (x, z) in [(-0.3, 30.4), (14.3, 30.4)] {
        put(p, &CONE, v(x, 0.0, z), 0.0, 1.1, false);
    }
}

/// The Migration loading dock, in the dock's frame (local +z points away from the Data Center's
/// east wall). Two more shuttered bays beside the open one, a truck backed up to the left bay,
/// pallets and boxes by the right one, a container, lamps over the bays and an asphalt apron.
pub(super) fn dock_yard(p: &mut Plan) {
    // Apron and lane markings.
    p.boxed(
        v(0.0, 0.012, 7.0),
        v(26.0, 0.024, 14.0),
        Mat::Asphalt,
        false,
    );
    for x in [-3.6, 3.6] {
        p.boxed(v(x, 0.03, 6.5), v(0.15, 0.02, 11.0), Mat::Yellow, false);
    }
    // Two more bays, shutters down, numbered by the hazard frames.
    for (x, lamp) in [(-8.0, Mat::LedRed), (8.0, Mat::LedGreen)] {
        p.boxed(v(x, 1.7, 0.14), v(4.2, 3.4, 0.28), Mat::Dark, false);
        for k in 0..8 {
            p.boxed(
                v(x, 0.25 + k as f32 * 0.4, 0.3),
                v(3.8, 0.34, 0.06),
                Mat::Metal,
                false,
            );
        }
        for sx in [-2.25, 2.25] {
            p.boxed(v(x + sx, 1.7, 0.2), v(0.3, 3.4, 0.14), Mat::Hazard, false);
        }
        p.boxed(v(x, 3.55, 0.3), v(4.8, 0.2, 0.2), Mat::Hazard, false);
        p.boxed(v(x, 3.82, 0.3), v(0.3, 0.16, 0.12), lamp, false);
    }
    // Lamps over all three bays.
    for x in [-8.0, 0.0, 8.0] {
        p.boxed(v(x, 4.3, 0.55), v(1.0, 0.15, 0.4), Mat::LightPanel, false);
        p.boxed(v(x, 4.1, 0.3), v(0.08, 0.4, 0.3), Mat::Metal, false);
    }
    // A hazard band across the face so the dock reads from the avenue.
    for k in 0..16 {
        let m = if k % 2 == 0 { Mat::Hazard } else { Mat::Dark };
        p.boxed(
            v(-9.0 + k as f32 * 1.2, 6.2, 0.12),
            v(1.2, 0.5, 0.24),
            m,
            false,
        );
    }
    // Delivery truck, rear to the left bay.
    put(p, &DELIVERY, v(-8.0, 0.0, 5.2), PI, 1.8, false);
    p.block(v(-8.0, 1.4, 5.2), v(2.8, 2.8, 6.0));
    // Pallets and stacked boxes by the right bay.
    for (x, z) in [(6.0, 3.0), (8.4, 3.2), (10.8, 3.0)] {
        p.boxed(v(x, 0.08, z), v(1.3, 0.16, 1.1), Mat::Wood, false);
    }
    for (x, z, n, yaw) in [
        (6.0, 3.0, 3, 0.1),
        (8.4, 3.2, 2, -0.15),
        (10.8, 3.0, 1, 0.2),
    ] {
        for k in 0..n {
            put(
                p,
                &BOX_LARGE,
                v(x, 0.16 + k as f32 * 0.55, z),
                yaw,
                1.0,
                false,
            );
        }
    }
    p.block(v(8.4, 0.9, 3.1), v(6.0, 1.8, 1.3));
    // A container parked on the apron's far edge, and cones along the lane.
    put(
        p,
        &SHIPPING_CONTAINER_A,
        v(11.5, 0.0, 10.5),
        FRAC_PI_2,
        7.0,
        true,
    );
    for x in [-3.6, 3.6] {
        put(p, &CONE, v(x, 0.0, 12.4), 0.0, 1.0, false);
    }
}

/// The QBR executive entrance, in the door's frame (local +z points west, away from the tower's
/// wing). A porte-cochere with gold columns over a long red carpet, topiary and planters along
/// it, brass lamps and a parked luxury car at the kerb.
pub(super) fn executive_forecourt(p: &mut Plan) {
    // Carpet out to the kerb and a marble apron under the canopy.
    p.boxed(v(0.0, 0.03, 6.5), v(2.4, 0.05, 5.4), Mat::Red, false);
    p.boxed(v(0.0, 0.02, 5.0), v(7.4, 0.04, 8.0), Mat::Marble, false);
    for sx in [-1.25, 1.25] {
        p.boxed(v(sx, 0.07, 6.5), v(0.08, 0.02, 5.4), Mat::Gold, false);
    }
    // Outer canopy on four columns, with a gold fascia.
    for (x, z) in [(-3.4, 8.2), (3.4, 8.2)] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 2.1, z),
            v(0.3, 4.2, 0.3),
            Mat::Gold,
        );
        p.block(v(x, 2.0, z), v(0.4, 4.0, 0.4));
    }
    p.boxed(v(0.0, 4.3, 5.8), v(7.8, 0.22, 5.0), Mat::Trim, false);
    p.boxed(v(0.0, 4.15, 8.3), v(7.9, 0.3, 0.14), Mat::Gold, false);
    // Topiary in tubs and long planters either side of the carpet.
    for sx in [-2.9, 2.9] {
        for z in [3.2, 5.6] {
            p.boxed(v(sx, 0.3, z), v(0.8, 0.6, 0.8), Mat::Marble, false);
            p.prim(
                PrimKind::Sphere,
                v(sx, 1.1, z),
                v(1.0, 1.0, 1.0),
                Mat::Plant,
            );
        }
        put(p, &PLANTER, v(sx * 1.35, 0.0, 1.4), 0.0, 3.2, false);
    }
    // Brass lamp posts at the kerb end of the carpet.
    for sx in [-1.7, 1.7] {
        p.prim(
            PrimKind::Cylinder,
            v(sx, 1.4, 9.2),
            v(0.1, 2.8, 0.1),
            Mat::Gold,
        );
        p.prim(
            PrimKind::Sphere,
            v(sx, 2.9, 9.2),
            v(0.4, 0.4, 0.4),
            Mat::LightPanel,
        );
    }
    // The luxury car waits at the kerb, nose out, chauffeur not included.
    put(p, &SUV_LUXURY, v(-7.4, 0.0, 6.5), 0.0, 1.55, false);
    p.block(v(-7.4, 0.75, 6.5), v(1.9, 1.5, 4.4));
}

/// The Hackathon loft, in the back wall's frame (local +z points east into the back lot, local x
/// runs along the cafe's brick wall). Neon along the wall top, a tall HACK24 sign on a pole,
/// bikes against the wall, pizza boxes and cans, beanbags and a string of lights.
pub(super) fn hacker_loft(p: &mut Plan) {
    let neon = [Mat::NeonPink, Mat::NeonCyan, Mat::NeonYellow, Mat::LedGreen];
    p.boxed(
        v(0.0, 3.55, 0.12),
        v(15.6, 0.07, 0.07),
        Mat::NeonCyan,
        false,
    );
    p.boxed(v(0.0, 3.8, 0.12), v(15.6, 0.07, 0.07), Mat::NeonPink, false);
    // The street-side landmark: a pole sign that reads from the avenue.
    for dx in [-0.1, 0.1] {
        p.sign(
            v(7.5, 7.0, 3.2 + dx),
            if dx < 0.0 { PI } else { 0.0 },
            5.0,
            Sign::H24Door,
        );
    }
    p.boxed(v(7.5, 3.5, 3.2), v(0.25, 7.0, 0.25), Mat::Metal, false);
    p.boxed(v(7.5, 7.0, 3.2), v(5.3, 3.5, 0.12), Mat::Dark, false);
    for (ox, oy, sx, sy) in [(0.0, 1.75, 5.4, 0.1), (0.0, -1.75, 5.4, 0.1)] {
        p.boxed(
            v(7.5 + ox, 7.0 + oy, 3.2),
            v(sx, sy, 0.16),
            Mat::NeonPink,
            false,
        );
    }
    // Bikes leaning on the brick and a skateboard on the ground.
    bike(p, -5.6, 1.1, true, Mat::CarRed);
    bike(p, -4.2, 1.9, true, Mat::CarTeal);
    bike(p, 5.2, 1.0, true, Mat::Yellow);
    p.boxed(v(-6.8, 0.06, 3.2), v(0.2, 0.05, 0.8), Mat::Wood, false);
    // Pizza boxes and energy cans on a crate, empties on the floor.
    p.boxed(v(3.4, 0.3, 1.5), v(1.2, 0.6, 0.9), Mat::Wood, true);
    for k in 0..3 {
        put(
            p,
            &PIZZA_BOX,
            v(3.4, 0.62 + k as f32 * 0.06, 1.5),
            0.4 * k as f32,
            0.5,
            false,
        );
    }
    for (x, z) in [(2.9, 1.2), (3.9, 1.8), (4.4, 3.0), (2.0, 3.4)] {
        put(
            p,
            &ENERGY_CAN,
            v(x, if z < 2.0 { 0.6 } else { 0.0 }, z),
            x,
            0.8,
            false,
        );
    }
    for (x, z) in [(1.0, 4.0), (4.9, 4.2), (-1.0, 3.6)] {
        put(p, &PIZZA_BOX, v(x, 0.0, z), x * 2.0, 0.5, false);
    }
    // Beanbags for the all-nighters.
    for (x, z, m) in [
        (-2.4, 3.6, Mat::BeanA),
        (-0.6, 4.6, Mat::BeanB),
        (6.0, 3.0, Mat::BeanC),
    ] {
        p.prim(PrimKind::Sphere, v(x, 0.35, z), v(1.1, 0.7, 1.1), m);
    }
    // A string of party lights over the lot, between the pole sign and the wall.
    for k in 0..14 {
        let x = -7.0 + k as f32 * 1.05;
        let sag = 3.0 - 0.35 * (1.0 - ((k as f32 - 6.5) / 6.5).powi(2));
        p.boxed(v(x, sag, 6.0), v(0.14, 0.14, 0.14), neon[k % 4], false);
    }
    p.boxed(v(0.0, 3.1, 6.0), v(15.0, 0.02, 0.02), Mat::Dark, false);
    for x in [-7.5, 7.5] {
        p.boxed(v(x, 1.8, 6.0), v(0.12, 3.6, 0.12), Mat::Metal, false);
    }
}

/// The new-starter queue, in the starters' wall frame (local +z points south onto the arrival
/// lawn). Welcome balloons tethered either side of the door, a lane of rope stanchions with
/// painted arrows leading to it, a welcome mat and a hanging banner over the lane.
pub(super) fn starters_queue(p: &mut Plan) {
    let balloons = [
        Mat::NeonPink,
        Mat::NeonCyan,
        Mat::NeonYellow,
        Mat::LedGreen,
        Mat::CarBlue,
    ];
    for (sx, k0) in [(-4.2, 0), (4.2, 2)] {
        p.prim(
            PrimKind::Cylinder,
            v(sx, 0.15, 1.0),
            v(0.5, 0.3, 0.5),
            Mat::Dark,
        );
        for k in 0..5usize {
            let (dx, dz) = [
                (0.0, 0.0),
                (0.5, 0.2),
                (-0.45, 0.3),
                (0.25, -0.35),
                (-0.2, -0.1),
            ][k];
            let h = 3.3 + (k % 3) as f32 * 0.55;
            p.boxed(
                v(sx + dx * 0.4, h / 2.0, 1.0 + dz * 0.4),
                v(0.02, h, 0.02),
                Mat::White,
                false,
            );
            p.prim(
                PrimKind::Sphere,
                v(sx + dx, h + 0.4, 1.0 + dz),
                v(0.75, 0.9, 0.75),
                balloons[(k + k0) % 5],
            );
        }
    }
    // Welcome mat and the queue lane: ropes between stanchions, arrows painted on the ground.
    p.boxed(v(0.0, 0.05, 1.3), v(2.4, 0.04, 1.4), Mat::LedGreen, false);
    p.boxed(v(0.0, 0.03, 7.2), v(3.4, 0.03, 10.0), Mat::Concrete, false);
    for z in [3.4, 5.4, 7.4, 9.4] {
        for sx in [-1.7, 1.7] {
            stanchion(p, sx, z, Mat::Gold);
            if z < 9.0 {
                p.boxed(
                    v(sx, 0.85, z + 1.0),
                    v(0.04, 0.04, 2.0),
                    Mat::LedGreen,
                    false,
                );
            }
        }
    }
    // Arrows pointing at the door (local -z): a stem and three widening bars for the head.
    for z in [4.6, 7.0, 9.2] {
        p.boxed(v(0.0, 0.06, z + 0.5), v(0.3, 0.02, 0.9), Mat::Yellow, false);
        for (k, w) in [0.4, 0.9, 1.4].into_iter().enumerate() {
            p.boxed(
                v(0.0, 0.06, z - 0.1 + k as f32 * 0.2),
                v(w, 0.02, 0.18),
                Mat::Yellow,
                false,
            );
        }
    }
    // Hanging banner over the lane.
    for sx in [-2.3, 2.3] {
        p.boxed(v(sx, 2.0, 3.0), v(0.14, 4.0, 0.14), Mat::Metal, false);
    }
    p.boxed(v(0.0, 4.15, 3.0), v(4.8, 0.12, 0.12), Mat::Metal, false);
    p.boxed(v(0.0, 3.6, 3.0), v(4.0, 1.1, 0.06), Mat::Dark, false);
    for (dz, yaw) in [(0.05, 0.0), (-0.05, PI)] {
        p.sign(v(0.0, 3.6, 3.0 + dz), yaw, 3.8, Sign::ObWelcome);
    }
}

/// The Company Calendar hub on Services Street: a small pavilion over the calendar board, a rug
/// to stand on and seven pennants for the seven minigames. Built round the board at (x, z)
/// facing +z; the stand spot two metres in front of it stays free.
pub(super) fn calendar_pavilion(p: &mut Plan, x: f32, z: f32) {
    let (cz, depth) = (z + 2.4, 7.0);
    p.boxed(v(x, 0.08, cz), v(8.0, 0.16, depth), Mat::Concrete, false);
    p.boxed(v(x, 0.17, cz + 0.4), v(5.2, 0.02, 3.6), Mat::Felt, false);
    for (sx, sz) in [
        (-3.6, -0.9),
        (3.6, -0.9),
        (-3.6, depth - 1.5),
        (3.6, depth - 1.5),
    ] {
        p.boxed(
            v(x + sx, 1.95, z + sz),
            v(0.25, 3.8, 0.25),
            Mat::Metal,
            true,
        );
    }
    p.boxed(v(x, 3.95, cz), v(8.6, 0.22, depth + 0.6), Mat::Trim, false);
    p.boxed(v(x, 4.15, cz), v(8.6, 0.2, depth + 0.6), Mat::CarRed, false);
    let front = cz + (depth + 0.6) / 2.0;
    // The sign stands on the roof edge so it reads over the heads of a crowd.
    for sx in [-2.3, 2.3] {
        p.boxed(
            v(x + sx, 4.6, front - 0.3),
            v(0.1, 0.8, 0.1),
            Mat::Metal,
            false,
        );
    }
    for (dz, yaw) in [(-0.2, 0.0), (-0.4, PI)] {
        p.sign(v(x, 5.4, front + dz), yaw, 5.2, Sign::Calendar);
    }
    let colors = [
        Mat::NeonPink,
        Mat::NeonCyan,
        Mat::NeonYellow,
        Mat::LedGreen,
        Mat::CarBlue,
        Mat::CarRed,
        Mat::Yellow,
    ];
    for (k, c) in colors.into_iter().enumerate() {
        p.prim(
            PrimKind::Pennant,
            v(x - 3.0 + k as f32, 3.6, front - 0.1),
            v(0.55, 0.8, 1.0),
            c,
        );
    }
    for sx in [-3.6, 3.6] {
        put(p, &PLANTER, v(x + sx * 1.1, 0.0, z - 0.2), 0.0, 3.2, false);
    }
}
