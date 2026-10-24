//! The Ilmari core (docs/ILMARI_1_0.md D.5): the lift's top floor, one skeleton room on the plan of
//! the wings (corridor, hall, corridor, pod) with no trash and no friendly face. The hall is the
//! server floor, rows of racks that never stop; the pod is the core, six monitors on its wall
//! (all of them off for now) and a ring of cable on the floor. D10-D13 build the fight on this
//! floor; until then it is a room that says nothing and listens. No point lights (the 0.9 rule).

use super::datacenter::dress::*;
use super::ilmari3::{arrival_kit, casing};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::props::server_rack;
use super::themes::{pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// The core: round enough for six monitors on the wall and four phases of floor.
const POD: (f32, f32, f32) = (46.0, 26.0, 24.0);

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
    // The core: six monitors on the north wall, all of them off, a floor ring that goes round
    // three times, cable heaps and one rack standing alone in the middle.
    let (cx, cz) = (POD.0, 0.0);
    for k in 0..6 {
        let x = cx - 15.0 + k as f32 * 6.0;
        p.boxed(v(x, 2.6, cz - 23.7), v(4.6, 2.6, 0.2), Mat::Dark, false);
        p.sign(v(x, 2.6, cz - 23.55), 0.0, 4.2, Sign::IlCoreMonitor);
    }
    floor_ring(p, cx, cz, 5.0, 28, Mat::NeonCyan, 0.12);
    floor_ring(p, cx, cz, 11.0, 44, Mat::Cold, 0.1);
    floor_ring(p, cx, cz, 18.0, 60, Mat::NeonCyan, 0.1);
    cable_bundle(
        p,
        &[(cx - 20.0, -6.0), (cx - 14.0, -2.0), (cx - 8.0, -5.0)],
        Mat::Cold,
    );
    cable_bundle(
        p,
        &[(cx + 8.0, 6.0), (cx + 14.0, 2.0), (cx + 20.0, 5.0)],
        Mat::Cold,
    );
    server_rack(p, cx + 18.0, cz - 16.0, FRAC_PI_2);
    server_rack(p, cx + 18.0, cz + 16.0, FRAC_PI_2);
    p.sign(v(cx - 25.7, 3.2, 0.0), FRAC_PI_2, 4.6, Sign::IlCoreWelcome);
    p.sign(v(cx + 25.7, 3.2, -6.0), -FRAC_PI_2, 3.4, Sign::IlCoreRack);
    p.sign(v(cx + 25.7, 3.2, 6.0), -FRAC_PI_2, 3.4, Sign::IlCoreWelcome);
    p.boxed(v(cx, 4.0, 0.0), v(0.3, 0.12, 44.0), Mat::Dark, false);
    strips(p, Mat::LightPanel, Mat::NeonCyan, 0.0, 0.1);
    // One door, the lift back down: the arrival. The room has no trash and no packs.
    cast_zone(
        p,
        &ZoneCast {
            doors: &[("door:ilmari6/3", (-50.0, 0.0), Sign::IlCoreDoor)],
            packs: &[],
            boss: (POD.0, 0.0),
            npc: None,
        },
    );
    theme()
}
