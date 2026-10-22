//! Ill Marie's nine appearances (docs/ILMARI_09.md Part A): each zone gets her anchor and the
//! few props that tell the joke. Flavour only: nothing here is solid across a walking line, and
//! the anchors sit well off the routes and away from every pack (`zones::marie_tests`).
//!
//! The sim spawns her from `game.ron` (`npc_ill_marie_*` anchors); the gates (level, boss
//! killed) live there, so the props stay put when she is not around. They are inert dressing.

use super::districts::spot;
use super::kit::*;
use super::layout::yaw_towards;
use super::props::{bench, vending};
use super::themes::v;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// Where a unit of height `PLAYER_HEIGHT` sits on a box-built seat: the same trick `occupy` uses.
fn seat_at(p: &mut Plan, name: &'static str, centre: Vec3, yaw: f32) {
    p.seat(centre, yaw, 0.25);
    p.occupy(name, centre);
}

/// Car Park P3: feeding the pay-and-display machine at (18, 34), 1.8 m beside the spot where
/// players use it, with the stack of unpaid tickets and a thermos she put down to do it.
pub(super) fn p3(p: &mut Plan) {
    let at = v(19.8, 0.0, 35.8);
    p.anchor("npc_ill_marie_p3", at, yaw_towards(at, v(18.0, 0.0, 34.0)));
    // The machine's verdict, over its own screen (it says this whether or not it took the coin).
    p.sign(v(18.0, 1.25, 34.29), 0.0, 0.46, Sign::PaymentAccepted);
    // Travel pack of tissues on top of the machine, the unpaid tickets at its foot, the kerb thermos.
    p.boxed(v(18.15, 1.64, 34.0), v(0.14, 0.08, 0.1), Mat::White, false);
    p.boxed(v(19.1, 0.04, 34.7), v(0.2, 0.08, 0.14), Mat::Paper, false);
    p.prim(
        PrimKind::Cylinder,
        v(21.0, 0.15, 36.7),
        v(0.13, 0.3, 0.13),
        Mat::Metal,
    );
}

/// The South Gate desk: her monitor shows the first line of the autonomy log (A.4), the lowest
/// value of the whole trail. A tiny screen on the desk, facing her chair.
pub(super) fn desk_monitor(p: &mut Plan, x: f32, z: f32) {
    p.boxed(
        v(x + 0.12, 1.0, z - 0.05),
        v(0.05, 0.36, 0.6),
        Mat::Dark,
        false,
    );
    p.sign(
        v(x + 0.15, 1.0, z - 0.05),
        FRAC_PI_2,
        0.55,
        Sign::AutonomyDesk,
    );
}

/// The Onboarding, "Your Desk": the cubicle beside the player's own (x = 4, row z = -8) is hers
/// once the Buddy is dead: a "BACKUP BUDDY" tent card, a box of tissues in the company colours
/// and the laminated checklist with one tick, next to a pot that has seen better quarters.
pub(super) fn onboarding(p: &mut Plan) {
    p.occupy("npc_ill_marie_onb", v(4.0, 0.0, -8.0));
    let top = DESK.size[1] * FURNITURE_SCALE;
    p.sign(v(3.2, top + 0.14, -8.85), 0.0, 0.34, Sign::BackupBuddy);
    p.boxed(
        v(3.2, top + 0.07, -8.88),
        v(0.3, 0.14, 0.02),
        Mat::White,
        false,
    );
    p.boxed(
        v(4.7, top + 0.07, -9.1),
        v(0.26, 0.14, 0.14),
        Mat::White,
        false,
    );
    p.boxed(
        v(4.7, top + 0.15, -9.1),
        v(0.1, 0.02, 0.1),
        Mat::CarBlue,
        false,
    );
    p.boxed(
        v(4.4, top + 0.015, -8.9),
        v(0.22, 0.02, 0.3),
        Mat::Paper,
        false,
    );
    // The dead plant, on the desk corner.
    p.prim(
        PrimKind::Cylinder,
        v(3.55, top + 0.08, -9.3),
        v(0.16, 0.16, 0.16),
        Mat::Brick,
    );
    p.boxed(
        v(3.55, top + 0.22, -9.3),
        v(0.22, 0.12, 0.04),
        Mat::Dead,
        false,
    );
}

/// Tower, Meeting Rooms: she is "dialled in from bed". A camp bed in the north-west corner of the
/// big room, a call wall in front of it that shows nothing but "Ill Marie joined" three times.
pub(super) fn tower(p: &mut Plan) {
    let (bx, bz) = (-30.0, 9.0);
    p.boxed(v(bx, 0.18, bz), v(1.1, 0.36, 2.0), Mat::Wood, true);
    p.boxed(v(bx, 0.41, bz), v(1.0, 0.12, 1.9), Mat::White, false);
    p.boxed(
        v(bx, 0.52, bz + 0.45),
        v(1.04, 0.1, 1.0),
        Mat::Fabric0,
        false,
    );
    p.boxed(v(bx, 0.55, bz - 0.78), v(0.7, 0.12, 0.3), Mat::White, false);
    p.boxed(v(bx, 0.7, bz - 1.04), v(1.1, 0.8, 0.06), Mat::Wood, false);
    seat_at(
        p,
        "npc_ill_marie_screen",
        v(bx, crate::seat::SEAT_TOP, bz - 0.35),
        0.0,
    );
    // The call wall, 3.5 m in front of the foot of the bed, and the speaker under it.
    let wz = bz + 5.4;
    p.boxed(v(bx, 1.75, wz), v(3.2, 1.8, 0.12), Mat::Dark, false);
    p.sign(v(bx, 1.75, wz - 0.07), PI, 3.0, Sign::MarieCall);
    p.boxed(v(bx, 0.45, wz - 0.1), v(1.4, 0.18, 0.2), Mat::Dark, false);
    // The tower floor's log, on her laptop at the foot of the bed.
    p.boxed(
        v(bx + 0.9, 0.6, bz + 0.2),
        v(0.04, 0.5, 0.7),
        Mat::Dark,
        false,
    );
    p.sign(
        v(bx + 0.875, 0.6, bz + 0.2),
        -FRAC_PI_2,
        0.62,
        Sign::AutonomyTower,
    );
}

/// Data Center, Hall A: in the lee of the west rack row's east end, where the rack backs breathe
/// out. A camping blanket on the rack ledge, a hand warmer, the thermos, and the rack-door screen.
pub(super) fn racks(p: &mut Plan) {
    let (x, z) = (-11.2, 5.2);
    let at = v(x, 0.0, z);
    p.anchor(
        "npc_ill_marie_racks",
        at,
        yaw_towards(at, v(x - 1.0, 0.0, 7.0)),
    );
    p.boxed(v(x - 1.3, 2.3, 6.1), v(0.9, 0.12, 0.8), Mat::Fabric2, false);
    p.boxed(
        v(x - 1.3, 2.05, 6.55),
        v(0.9, 0.5, 0.06),
        Mat::Fabric2,
        false,
    );
    p.boxed(
        v(x - 0.5, 0.15, z + 0.5),
        v(0.12, 0.3, 0.12),
        Mat::Metal,
        false,
    );
    p.boxed(
        v(x + 0.6, 0.06, z - 0.2),
        v(0.12, 0.12, 0.08),
        Mat::Hot,
        false,
    );
    p.boxed(
        v(x - 1.9, 1.15, 6.43),
        v(0.5, 0.06, 0.04),
        Mat::BlinkAmber,
        false,
    );
    p.sign(v(x - 1.9, 1.6, 6.45), PI, 0.7, Sign::AutonomyRack);
}

/// Basement B, Cutover War Room: she holds the pager at the war-room desk (x = 38, row z = -6.2)
/// after the Cutover: an on-call laptop, a bridge speakerphone and a runbook in pink tissue paper.
pub(super) fn oncall(p: &mut Plan) {
    p.occupy("npc_ill_marie_oncall", v(38.0, 0.0, -7.1));
    let top = DESK.size[1] * FURNITURE_SCALE;
    p.prop(&LAPTOP, v(37.3, top, -6.2), PI, false);
    p.prim(
        PrimKind::Cylinder,
        v(38.8, top + 0.04, -6.5),
        v(0.22, 0.08, 0.22),
        Mat::Dark,
    );
    p.boxed(
        v(37.7, top + 0.02, -6.55),
        v(0.3, 0.03, 0.4),
        Mat::NeonPink,
        false,
    );
}

/// Stay Green flat: she calls in from the player's bed, thermos and all. The laptop screen on the
/// desk shows the call; the real her is under the duvet (the "screen" gag in a room too small for
/// a wall of it).
pub(super) fn home(p: &mut Plan) {
    let (bx, bz) = (6.6, 3.4);
    seat_at(
        p,
        "npc_ill_marie_call",
        v(bx, crate::seat::SEAT_TOP, bz + 0.1),
        PI,
    );
    // Second laptop on the duvet, the thermos on the bedside.
    p.prop(&LAPTOP, v(bx, 0.7, bz - 0.5), PI, false);
    p.prim(
        PrimKind::Cylinder,
        v(bx + 1.25, 0.14, bz + 0.9),
        v(0.12, 0.28, 0.12),
        Mat::Metal,
    );
}

/// Campus, Services Street: the cough drop machine and the queue for it. Marie waits beside the
/// lane to the machine (not in it), 97 m from her desk so the two never share a screen.
pub(super) fn queue(p: &mut Plan) {
    vending(p, 52.0, 26.5, Face::PosZ);
    spot(p, "prop:cough_drops", (52.0, 1.1, 26.9), (52.0, 28.1));
    let at = v(49.2, 0.0, 29.4);
    p.anchor(
        "npc_ill_marie_queue",
        at,
        yaw_towards(at, v(52.0, 0.0, 26.5)),
    );
    // Queue rail: two posts and a belt, along her side of the lane.
    for (x, z) in [(50.3, 29.9), (50.3, 27.3)] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.45, z),
            v(0.07, 0.9, 0.07),
            Mat::Chrome,
        );
    }
    p.boxed(v(50.3, 0.8, 28.6), v(0.03, 0.04, 2.6), Mat::Red, false);
    // Thermos with the tissue pack taped to it.
    p.prim(
        PrimKind::Cylinder,
        v(48.5, 0.15, 30.1),
        v(0.13, 0.3, 0.13),
        Mat::Metal,
    );
    p.boxed(v(48.5, 0.18, 30.2), v(0.1, 0.14, 0.04), Mat::White, false);
}

/// Campus, the Lab forecourt, once the AGI is quiet: a bench facing the Lab door, 10 m off it.
pub(super) fn campus_bench(p: &mut Plan) {
    let (x, z) = (-63.0, -65.0);
    let yaw = (-72.0f32 - x).atan2(-67.2 - z);
    bench(p, x, z, yaw);
    p.occupy("npc_ill_marie_bench", v(x, 0.0, z));
}
