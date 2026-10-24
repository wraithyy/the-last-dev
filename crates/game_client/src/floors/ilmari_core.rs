//! The Ilmari core (docs/ILMARI_1_0.md D.5): the lift's top floor, one skeleton room on the plan of
//! the wings (corridor, hall, corridor, pod) with no trash and no friendly face. The hall is the
//! server floor, rows of racks that never stop; the pod is the core, six monitors on its wall
//! (all of them off for now) and a ring of cable on the floor. D10-D13 build the fight on this
//! floor; until then it is a room that says nothing and listens. No point lights (the 0.9 rule).

use super::datacenter::dress::*;
use super::ilmari1::label_printer;
use super::ilmari2::{ballot, portal};
use super::ilmari3::{arrival_kit, casing};
use super::ilmari5::kiosk;
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::models::{self, MUG};
use super::props::server_rack;
use super::themes::{pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The core: round enough for six monitors on the wall and four phases of floor.
const POD: (f32, f32, f32) = (46.0, 26.0, 24.0);
/// The ring of screens: the radius of the seats around the middle of the pod (`ring_m` in `game.ron`).
const RING: f32 = 17.0;

pub fn build() -> FloorLayout {
    let mut p = Plan::default();
    let theme = core_room(&mut p);
    assemble(
        "ilmari_core/1",
        1,
        "The Core: The Server Room At The Top",
        false,
        p,
        theme,
    )
}

fn theme() -> Theme {
    let c = Color::srgb;
    Theme {
        wall: c(0.28, 0.3, 0.38),
        trim: c(0.06, 0.08, 0.12),
        accent: LinearRgba::rgb(0.4, 2.2, 3.6),
        fabric: [c(0.25, 0.3, 0.4), c(0.3, 0.3, 0.35), c(0.2, 0.25, 0.3)],
        floor: FloorKind::Tile([0.34, 0.36, 0.44]),
        fog: c(0.04, 0.06, 0.12),
        ambient: c(0.7, 0.8, 1.0),
        ambient_brightness: 700.0,
        sun: c(0.7, 0.8, 1.0),
        sun_illuminance: 2200.0,
        fog_range: (22.0, 90.0),
    }
}

fn core_room(p: &mut Plan) -> Theme {
    casing(p, POD, Mat::NeonCyan);
    // The server floor: two long rows of racks with the aisle of the route between, every LED
    // green, every fan the same pitch.
    for x in [-30.0, -22.0, -14.0, -6.0, 2.0, 10.0] {
        for (z, yaw) in [(-8.2, 0.0), (8.2, PI)] {
            server_rack(p, x, z, yaw);
            server_rack(p, x + 1.6, z, yaw);
        }
    }
    for x in [-30.5, -22.5, -14.5, -6.5, 1.5, 9.5] {
        vent_tile(p, x, -3.0, Mat::Cold);
        vent_tile(p, x, 3.0, Mat::Cold);
    }
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(-32.0, z), pt(16.0, z), false);
    }
    floor_line(p, pt(-33.0, -2.2), pt(17.0, -2.2), 0.1, Mat::NeonCyan);
    floor_line(p, pt(-33.0, 2.2), pt(17.0, 2.2), 0.1, Mat::NeonCyan);
    p.sign_near(-33.9, 0.0, 2.7, 4.2, Sign::IlCoreWelcome);
    p.sign_near(-22.0, -10.9, 2.7, 3.2, Sign::IlCoreRack);
    p.sign_near(-6.0, -10.9, 2.7, 3.2, Sign::IlCoreMonitor);
    p.sign_near(10.0, -10.9, 2.7, 3.2, Sign::IlCoreRack);
    p.sign_near(-22.0, 10.9, 2.7, 3.2, Sign::IlCoreMonitor);
    p.sign_near(-6.0, 10.9, 2.7, 3.2, Sign::IlCoreRack);
    p.sign_near(10.0, 10.9, 2.7, 3.2, Sign::IlCoreMonitor);
    for x in [-26.0, -10.0, 6.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 20.0), Mat::Dark, false);
    }
    arrival_kit(p, Sign::IlCoreDoor);
    // Cells: cold storage (racks of backups of racks), a patch bay and the power room.
    for k in 0..4 {
        server_rack(p, -26.0 + k as f32 * 2.4, -32.4, 0.0);
    }
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlCoreRack);
    for k in 0..3 {
        server_rack(p, 0.0 + k as f32 * 2.6, 32.4, PI);
    }
    p.sign_near(6.0, 19.1, 2.6, 2.4, Sign::IlCoreRack);
    cable_bundle(p, &[(4.0, -29.0), (8.0, -31.0), (12.0, -29.0)], Mat::Cold);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlCoreMonitor);
    // The core: six wall monitors on the north wall (the wall display of D17 will light them), a
    // floor ring that goes round three times, cable heaps, and below the ring the whole fight.
    let (cx, cz) = (POD.0, 0.0);
    for k in 0..6 {
        let x = cx - 15.0 + k as f32 * 6.0;
        p.boxed(v(x, 2.6, cz - 23.7), v(4.6, 2.6, 0.2), Mat::Dark, false);
        p.sign(v(x, 2.6, cz - 23.55), 0.0, 4.2, Sign::IlCoreMonitor);
    }
    floor_ring(p, cx, cz, 5.0, 28, Mat::NeonCyan, 0.12);
    floor_ring(p, cx, cz, 11.0, 44, Mat::Cold, 0.1);
    floor_ring(p, cx, cz, RING, 60, Mat::NeonCyan, 0.1);
    cable_bundle(
        p,
        &[(cx - 22.0, -6.0), (cx - 14.0, -2.0), (cx - 8.0, -5.0)],
        Mat::Cold,
    );
    cable_bundle(
        p,
        &[(cx + 8.0, 6.0), (cx + 14.0, 2.0), (cx + 22.0, 5.0)],
        Mat::Cold,
    );
    server_rack(p, cx + 21.0, cz - 19.0, FRAC_PI_2);
    server_rack(p, cx + 21.0, cz + 19.0, FRAC_PI_2);
    server_rack(p, cx - 22.0, cz - 19.0, -FRAC_PI_2);
    server_rack(p, cx - 22.0, cz + 19.0, -FRAC_PI_2);
    p.sign(v(cx - 25.7, 3.2, 0.0), FRAC_PI_2, 4.6, Sign::IlCoreWelcome);
    p.sign(v(cx + 25.7, 3.2, -9.0), -FRAC_PI_2, 3.4, Sign::IlCoreRack);
    p.sign(v(cx + 25.7, 3.2, 9.0), -FRAC_PI_2, 3.4, Sign::IlCoreRerun);
    p.boxed(v(cx, 4.0, 0.0), v(0.3, 0.12, 44.0), Mat::Dark, false);
    // The reruns: three old television sets against the east wall, all tuned to the same face.
    for z in [-6.0, 0.0, 6.0] {
        p.prop(&TV_VINTAGE, v(cx + 24.6, 0.0, z), -FRAC_PI_2, true);
    }
    p.prop(&SPEAKER, v(cx + 24.6, 0.0, -9.5), -FRAC_PI_2, false);
    p.prop(&SPEAKER, v(cx + 24.6, 0.0, 9.5), -FRAC_PI_2, false);
    // Ill Marie's corner, west of the middle: a round rug, an empty chair with a blanket on it,
    // a side table with the thermos and a tower of tissue boxes. She stands, very still, on the rug.
    p.prop(&RUG_ROUND, v(cx - 3.5, 0.0, 0.0), 0.0, false);
    p.prop(&CHAIR_DESK, v(cx - 6.2, 0.0, 1.4), -FRAC_PI_2, false);
    p.boxed(
        v(cx - 6.0, 0.62, 1.4),
        v(0.7, 0.1, 0.8),
        Mat::Fabric2,
        false,
    );
    p.prop(&SIDE_TABLE, v(cx - 6.4, 0.0, -1.4), 0.0, true);
    models::put(p, &MUG, v(cx - 6.4, 0.5, -1.4), 0.0, 0.2, false);
    p.prop(&LAMP_TABLE, v(cx - 6.7, 0.5, -1.0), 0.0, false);
    for k in 0..4 {
        p.boxed(
            v(cx - 5.6, 0.14 + k as f32 * 0.26, -2.6),
            v(0.5, 0.24, 0.3),
            if k % 2 == 0 { Mat::Paper } else { Mat::Fabric1 },
            k == 0,
        );
    }
    p.prop(&TRASHCAN, v(cx - 4.8, 0.0, -3.4), 0.0, false);
    // The hatch in the middle of the floor, where the mainframe will come up.
    p.boxed(v(cx, 0.06, 0.0), v(5.6, 0.1, 5.6), Mat::Dark, false);
    for (dx, dz, w, d) in [
        (0.0, -2.9, 6.0, 0.2),
        (0.0, 2.9, 6.0, 0.2),
        (-2.9, 0.0, 0.2, 5.6),
        (2.9, 0.0, 0.2, 5.6),
    ] {
        p.boxed(v(cx + dx, 0.09, dz), v(w, 0.08, d), Mat::Hazard, false);
    }
    p.sign(v(cx + 3.4, 1.0, 0.0), -FRAC_PI_2, 2.2, Sign::IlCoreHatch);
    // The draft on its table, where the swarm's cluster comes for it.
    p.prop(&TABLE_ROUND, v(cx, 0.0, -9.0), 0.0, true);
    p.boxed(v(cx, 0.8, -9.0), v(0.5, 0.03, 0.36), Mat::Paper, false);
    p.sign(v(cx, 1.9, -9.0), 0.0, 1.5, Sign::IlCoreDraft);
    // The four ballot terminals in an arc south of the middle (the Board's first three in P1, Ill
    // Marie's rounds in P2), the Hydra's printer and the HR portals along the walls.
    for (key, x, z, sign) in [
        ("prop:il_core_vote_1", cx - 7.0, 8.4, Sign::IlVoteAgree),
        ("prop:il_core_vote_2", cx - 2.5, 9.4, Sign::IlVoteRefuse),
        ("prop:il_core_vote_3", cx + 2.5, 9.4, Sign::IlVoteAsk),
        ("prop:il_core_vote_4", cx + 7.0, 8.4, Sign::IlVoteRemember),
    ] {
        ballot(p, key, x, z, -1.0, sign);
    }
    // P3's Opt-Out Kiosk (a Helpful Suggestion is cancelled here), P4's three punch cards beside the
    // hatch and the three terminals of the revert on the east side of the ring.
    kiosk(p, "prop:il_core_optout", cx + 12.0, 11.0, 1.0);
    for (key, z) in [
        ("prop:il_punch_1", -7.2),
        ("prop:il_punch_2", -3.6),
        ("prop:il_punch_3", -0.4),
    ] {
        punch_card_reader(p, key, cx + 5.2, z);
    }
    for (key, z) in [
        ("prop:il_git_1", -13.0),
        ("prop:il_git_2", -9.5),
        ("prop:il_git_3", -6.0),
    ] {
        revert_terminal(p, key, cx + 11.5, z);
    }
    label_printer(p, "prop:il_core_printer", cx - 12.0, -22.0, 1.0);
    portal(p, "prop:il_core_portal_1", cx + 12.0, -22.0, 1.0);
    portal(p, "prop:il_core_portal_2", cx, 22.2, -1.0);
    strips(p, Mat::LightPanel, Mat::NeonCyan, 0.0, 0.1);
    // One door, the lift back down: the arrival. The room has no trash and no packs. The boss's
    // anchor is the first seat of the ring of screens, a ring's radius east of the middle.
    cast_zone(
        p,
        &ZoneCast {
            doors: &[("door:ilmari6/3", (-50.0, 0.0), Sign::IlCoreDoor)],
            packs: &[],
            boss: (POD.0 + RING, 0.0),
            npc: None,
        },
    );
    let boss = p
        .anchors
        .iter()
        .find(|a| a.name == "boss")
        .map_or((Vec3::new(cx + RING, 0.0, 0.0), FRAC_PI_2), |a| {
            (a.pos, a.yaw)
        });
    p.anchor("npc_marie", v(cx - 3.5, 0.0, 0.0), -FRAC_PI_2);
    seats(p, boss.0, boss.1);
    theme()
}

/// The six seats of the screens: the sim puts seat `k` at `forward(yaw + PI + k * 60 deg)` from
/// the middle, which is a ring's radius ahead of the boss anchor (`TwinsDef::ring_m`, `raid.rs`).
/// Each is a dark pad with a neon rim, and the six are tied by a thin cable to the hatch.
fn seats(p: &mut Plan, boss: Vec3, yaw: f32) {
    let f = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let centre = boss + f * RING;
    for k in 0..6 {
        let a = yaw + PI + k as f32 * TAU / 6.0;
        let at = centre + Vec3::new(-a.sin(), 0.0, -a.cos()) * RING;
        p.prim(
            PrimKind::Cylinder,
            v(at.x, 0.05, at.z),
            v(4.2, 0.1, 4.2),
            Mat::Dark,
        );
        p.prim(
            PrimKind::Cylinder,
            v(at.x, 0.07, at.z),
            v(4.5, 0.06, 4.5),
            Mat::NeonCyan,
        );
        p.prim(
            PrimKind::Cylinder,
            v(at.x, 0.12, at.z),
            v(3.9, 0.06, 3.9),
            Mat::Dark,
        );
    }
}

/// A card reader on a pedestal beside the hatch: a hopper of punch cards, a slot, a lamp. P4's heal
/// for the mainframe, open to every class.
fn punch_card_reader(p: &mut Plan, key: &'static str, x: f32, z: f32) {
    p.boxed(v(x, 0.5, z), v(0.7, 1.0, 0.8), Mat::Metal, true);
    p.boxed(v(x, 1.05, z), v(0.8, 0.1, 0.9), Mat::Dark, false);
    p.boxed(v(x - 0.1, 1.14, z), v(0.3, 0.08, 0.5), Mat::Paper, false);
    p.boxed(
        v(x - 0.36, 0.78, z),
        v(0.04, 0.12, 0.5),
        Mat::LedGreen,
        false,
    );
    p.interactable(key, v(x, 1.0, z), v(x - 1.6, 0.0, z));
}

/// A revert terminal: a dark console with a green screen turned toward the hatch; whoever stands
/// at it can `git revert`.
fn revert_terminal(p: &mut Plan, key: &'static str, x: f32, z: f32) {
    p.boxed(v(x, 0.55, z), v(0.8, 1.1, 0.9), Mat::Dark, true);
    p.boxed(
        v(x - 0.1, 1.3, z),
        v(0.06, 0.6, 0.8),
        Mat::ScreenGlow,
        false,
    );
    p.boxed(v(x, 1.14, z), v(0.9, 0.06, 1.0), Mat::Metal, false);
    p.sign(v(x - 0.2, 2.0, z), FRAC_PI_2, 1.1, Sign::IlCoreMonitor);
    p.interactable(key, v(x, 0.9, z), v(x - 1.8, 0.0, z));
}
