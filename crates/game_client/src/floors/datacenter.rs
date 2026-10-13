//! The data center: two indoor zones. Level 1 is a cold/hot-aisle server hall with a GPU
//! shrine and a NOC; level 2 is the same building after the cooling failed (orange light,
//! a round arena with pillars for the boss).

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::props::{gpu_shrine, sad_plant, vending};
use super::shape::{blob, pillar, rounded_rect, stroke};
use super::themes::{pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let theme = if level == 1 { dc1(&mut p) } else { dc2(&mut p) };
    let (zone, name) = if level == 1 {
        ("datacenter/1", "Data Center: Sublevel 1")
    } else {
        ("datacenter/2", "Data Center: Sublevel 2")
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(hot: bool) -> Theme {
    let c = Color::srgb;
    if hot {
        Theme {
            wall: c(0.55, 0.4, 0.36),
            trim: c(0.14, 0.06, 0.04),
            accent: LinearRgba::rgb(4.0, 1.2, 0.2),
            fabric: [c(0.5, 0.2, 0.15), c(0.4, 0.3, 0.2), c(0.3, 0.2, 0.2)],
            floor: FloorKind::Tile([1.0, 0.7, 0.55]),
            fog: c(0.07, 0.025, 0.015),
            ambient: c(1.0, 0.55, 0.35),
            ambient_brightness: 560.0,
            sun: c(1.0, 0.5, 0.3),
            sun_illuminance: 2500.0,
            fog_range: (20.0, 70.0),
        }
    } else {
        Theme {
            wall: c(0.45, 0.52, 0.62),
            trim: c(0.07, 0.09, 0.12),
            accent: LinearRgba::rgb(0.2, 1.6, 2.4),
            fabric: [c(0.2, 0.35, 0.5), c(0.25, 0.3, 0.45), c(0.2, 0.4, 0.4)],
            floor: FloorKind::Tile([0.8, 0.92, 1.05]),
            fog: c(0.02, 0.03, 0.05),
            ambient: c(0.5, 0.66, 1.0),
            ambient_brightness: 560.0,
            sun: c(0.7, 0.8, 1.0),
            sun_illuminance: 2500.0,
            fog_range: (25.0, 75.0),
        }
    }
}

/// One rack front: panel plus a 2 x 3 block of blinking LEDs; the body is the row's shared box.
fn rack_front(p: &mut Plan, x: f32, z: f32, yaw: f32, hot: bool, seed: usize) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 1.1, 0.505), v(0.8, 2.0, 0.01), Mat::RackPanel, false);
        for row in 0..3 {
            for col in 0..2 {
                let k = seed + row * 3 + col * 5;
                let mat = if hot {
                    [
                        Mat::BlinkRed,
                        Mat::BlinkAmber,
                        Mat::BlinkRed,
                        Mat::BlinkAmber,
                        Mat::LedRed,
                    ][k % 5]
                } else {
                    [
                        Mat::BlinkGreenA,
                        Mat::BlinkGreenB,
                        Mat::BlinkBlue,
                        Mat::BlinkGreenA,
                        Mat::BlinkAmber,
                    ][k % 5]
                };
                p.boxed(
                    v(-0.15 + col as f32 * 0.3, 0.7 + row as f32 * 0.55, 0.52),
                    v(0.06, 0.05, 0.02),
                    mat,
                    false,
                );
            }
        }
    });
}

/// Row of racks along X from `x0` to `x1` at `z`, fronts towards `+Z` for yaw 0 and `-Z` for PI.
fn rack_row(p: &mut Plan, x0: f32, x1: f32, z: f32, yaw: f32, hot: bool) {
    let n = (x1 - x0).round() as usize;
    p.boxed(
        v((x0 + x1) / 2.0, 1.1, z),
        v(x1 - x0, 2.2, 1.0),
        Mat::Rack,
        true,
    );
    for i in 0..n {
        rack_front(
            p,
            x0 + 0.5 + i as f32,
            z,
            yaw,
            hot,
            i * 7 + z.abs() as usize,
        );
    }
}

/// Overhead cable tray along X or Z, 3.6 m up (above the camera's reach in the aisles).
fn cable_tray(p: &mut Plan, a: Vec2, b: Vec2, hot: bool) {
    let (mid, d) = ((a + b) / 2.0, b - a);
    let yaw = (-d.y).atan2(d.x);
    p.slab(
        v(mid.x, 3.7, mid.y),
        yaw,
        v(d.length(), 0.08, 0.6),
        Mat::Dark,
    );
    p.slab(
        v(mid.x, 3.78, mid.y),
        yaw,
        v(d.length(), 0.05, 0.12),
        if hot { Mat::Hot } else { Mat::Cable },
    );
}

/// Floor-standing cooling unit (CRAC): grille front, glowing status strip. Faces `face`.
fn cooling_unit(p: &mut Plan, x: f32, z: f32, face: Face, hot: bool) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 1.1, 0.0), v(2.2, 2.2, 1.0), Mat::Metal, true);
        p.boxed(v(0.0, 1.0, 0.51), v(1.8, 1.4, 0.02), Mat::Dark, false);
        for k in 0..5 {
            p.boxed(
                v(0.0, 0.5 + k as f32 * 0.25, 0.53),
                v(1.7, 0.04, 0.02),
                Mat::Metal,
                false,
            );
        }
        p.boxed(
            v(0.0, 2.0, 0.52),
            v(1.6, 0.06, 0.02),
            if hot { Mat::Hot } else { Mat::Cold },
            false,
        );
    });
}

/// Ceiling strip lights on an 8 m grid, in the zone's own colour.
fn strips(p: &mut Plan, mat: Mat) {
    for xi in -8..=8 {
        for zi in -6..=6 {
            let (x, z) = (xi as f32 * 8.0, zi as f32 * 8.0);
            if p.fp.contains(pt(x, z)) {
                p.boxed(v(x, H - 0.03, z), v(2.4, 0.05, 0.18), mat, false);
            }
        }
    }
}

fn point_lights(p: &mut Plan, list: &[(f32, f32)], color: Color, intensity: f32) {
    for &(x, z) in list {
        p.light(v(x, 3.4, z), color, intensity);
    }
}

fn dc1(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-48.0, 0.0), 9.0, 11.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-40.0, 0.0), pt(-30.0, 0.0)], 3.5), w);
    p.fp.pod(rounded_rect(pt(-8.0, 0.0), 24.0, 20.0, 6.0, 0.0), w);
    p.fp.link(stroke(&[pt(12.0, 0.0), pt(26.0, 0.0)], 3.5), w);
    p.fp.pod(rounded_rect(pt(35.0, 0.0), 12.0, 14.0, 6.0, 0.0), w);
    p.fp.pod(rounded_rect(pt(-8.0, -33.0), 11.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-8.0, -18.0), pt(-8.0, -29.0)], 3.0), w);
    p.fp.pod(rounded_rect(pt(-8.0, 33.0), 12.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-8.0, 18.0), pt(-8.0, 29.0)], 3.0), w);
    p.fp.hole(pillar(pt(30.0, -7.0), 0.7), Mat::Metal);
    p.fp.hole(pillar(pt(30.0, 7.0), 0.7), Mat::Metal);
    p.route = vec![
        pt(-52.0, 0.0),
        pt(-30.0, 0.0),
        pt(-8.0, 0.0),
        pt(26.0, 0.0),
        pt(36.0, 0.0),
    ];
    p.build_walls();

    // Server hall: two cold aisles (fronts facing each other) with a cross aisle at x -12..-4.
    for (x0, x1) in [(-30.0, -12.0), (-4.0, 14.0)] {
        rack_row(p, x0, x1, -7.0, PI, false);
        rack_row(p, x0, x1, -14.5, 0.0, false);
        rack_row(p, x0, x1, 7.0, 0.0, false);
        rack_row(p, x0, x1, 14.5, PI, false);
    }
    for x in [-26.0, -18.0, 6.0, 12.0] {
        cooling_unit(p, x, -18.7, Face::PosZ, false);
        cooling_unit(p, x, 18.7, Face::NegZ, false);
    }
    for z in [-10.8, 10.8] {
        for (x0, x1) in [(-30.0, -12.0), (-4.0, 14.0)] {
            cable_tray(p, pt(x0, z), pt(x1, z), false);
        }
        p.sign_near(-31.0, z, 2.6, 2.4, Sign::ColdAisle);
    }
    p.sign_near(0.0, -19.5, 2.8, 2.4, Sign::HotAisle);
    p.sign_near(0.0, 19.5, 2.8, 2.4, Sign::HotAisle);
    p.sign_near(14.0, 20.0, 2.8, 2.6, Sign::Compute);
    cable_tray(p, pt(-30.0, 0.0), pt(14.0, 0.0), false);
    cable_tray(p, pt(-8.0, -18.0), pt(-8.0, 18.0), false);
    p.sign_near(-31.5, 0.0, 3.0, 3.0, Sign::Uptime);

    // Lobby: reception, notices, vending, a sad plant.
    p.boxed(v(-52.5, 0.55, -6.0), v(2.5, 1.1, 0.9), Mat::Wood, true);
    p.boxed(v(-52.5, 1.12, -6.0), v(2.6, 0.05, 1.0), Mat::Marble, false);
    p.sign_near(-56.0, -6.0, 2.2, 2.0, Sign::DoNotUnplug);
    p.sign_near(-56.0, 6.0, 2.2, 2.0, Sign::ThisIsFine);
    vending(p, -43.0, -9.8, Face::PosZ);
    sad_plant(p, -55.5, 9.0);
    p.sign_near(-48.0, -11.0, 2.6, 3.0, Sign::Uptime);

    // GPU shrine room.
    gpu_shrine(p, -8.0, -39.5, Face::PosZ);
    p.sign_near(-8.0, -41.5, 3.4, 3.2, Sign::Shrine);
    for x in [-16.0, 0.0] {
        p.boxed(v(x, 0.6, -38.0), v(0.9, 1.2, 0.9), Mat::Rack, true);
        rack_front(p, x, -38.0, 0.0, false, 3);
    }
    p.sign_near(-18.5, -33.0, 2.4, 2.0, Sign::Compute);

    // NOC: three consoles facing the wall board.
    p.sign_near(-8.0, 40.5, 2.5, 6.0, Sign::NocBoard);
    for x in [-14.0, -8.0, -2.0] {
        p.boxed(v(x, 0.4, 36.5), v(2.4, 0.8, 0.9), Mat::Dark, true);
        p.boxed(v(x, 1.0, 36.5), v(1.2, 0.7, 0.06), Mat::ScreenGlow, false);
        p.boxed(v(x, 0.82, 36.0), v(0.8, 0.04, 0.3), Mat::RackPanel, false);
    }
    p.sign_near(-19.5, 33.0, 2.4, 2.0, Sign::ThisIsFine);

    // Boss hall: the job queue board over a bank of racks.
    p.sign_near(47.0, -6.0, 2.8, 4.0, Sign::TempWarning);
    for z in [-9.0, 9.0] {
        p.boxed(v(40.0, 1.1, z), v(5.0, 2.2, 1.0), Mat::Rack, true);
        for i in 0..5 {
            rack_front(
                p,
                37.5 + i as f32,
                z,
                if z < 0.0 { 0.0 } else { PI },
                false,
                i * 3,
            );
        }
    }

    let pts = [
        (-52.0, 0.0),
        (-40.0, -9.0),
        (-44.0, 8.0),
        (-20.0, -10.8),
        (-20.0, 10.8),
        (0.0, -10.8),
        (0.0, 10.8),
        (-8.0, -33.0),
        (-8.0, 33.0),
        (20.0, 0.0),
        (34.0, 0.0),
        (-8.0, 0.0),
    ];
    point_lights(p, &pts, Color::srgb(0.55, 0.78, 1.0), 380_000.0);
    strips(p, Mat::Cold);
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:campus", (-57.0, 0.0), Sign::ToCampus),
                ("door:datacenter/2", (47.5, 8.0), Sign::ToDataCenter2),
            ],
            packs: &[
                (-21.0, -10.8),
                (-21.0, 10.8),
                (7.0, 10.8),
                (7.0, -10.8),
                (-8.0, -34.0),
            ],
            boss: (38.0, 0.0),
            npc: Some((-8.0, 31.0)),
        },
    );
    theme(false)
}

fn dc2(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-48.0, 0.0), 9.0, 10.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-40.0, 0.0), pt(-30.0, 0.0)], 3.5), w);
    p.fp.pod(rounded_rect(pt(-12.0, 0.0), 20.0, 18.0, 6.0, 0.0), w);
    p.fp.pod(rounded_rect(pt(-12.0, -34.0), 10.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-10.0, -16.0), pt(-10.0, -30.0)], 3.5), w);
    p.fp.link(stroke(&[pt(2.0, 0.0), pt(24.0, 0.0)], 3.5), w);
    p.fp.pod(blob(pt(36.0, 0.0), 17.0, 17.0, 0.0, 0, 0.0), w);
    for k in 0..6 {
        let a = (30.0 + k as f32 * 60.0).to_radians();
        p.fp.hole(
            pillar(pt(36.0 + 9.0 * a.cos(), 9.0 * a.sin()), 1.0),
            Mat::Metal,
        );
    }
    p.route = vec![
        pt(-52.0, 0.0),
        pt(-30.0, 0.0),
        pt(-8.0, 0.0),
        pt(24.0, 0.0),
        pt(36.0, 0.0),
    ];
    p.build_walls();

    // Gutted rows around the central aisle; fronts face it. Gaps at x -20..-14 and -6..0.
    for (x0, x1) in [(-31.0, -20.0), (-14.0, -6.0), (0.0, 6.0)] {
        rack_row(p, x0, x1, -9.0, 0.0, true);
        rack_row(p, x0, x1, 9.0, PI, true);
    }
    for x in [-26.0, -10.0, 2.0] {
        cooling_unit(p, x, -16.7, Face::PosZ, true);
        cooling_unit(p, x, 16.7, Face::NegZ, true);
    }
    p.sign_near(-31.0, -4.0, 3.0, 2.6, Sign::TempWarning);
    p.sign_near(-31.0, 4.0, 2.2, 2.2, Sign::ThisIsFine);
    p.sign_near(8.0, 17.0, 2.6, 2.6, Sign::HotAisle);
    p.sign_near(8.0, -17.0, 2.6, 2.6, Sign::HotAisle);
    p.sign_near(-40.0, -10.0, 2.6, 2.4, Sign::DoNotUnplug);
    p.sign_near(-56.0, 6.0, 2.2, 2.0, Sign::ToDataCenter1);
    cable_tray(p, pt(-30.0, 0.0), pt(24.0, 0.0), true);
    // Vents venting: glowing floor grilles along the aisle.
    for i in 0..8 {
        p.boxed(
            v(-26.0 + i as f32 * 6.0, 0.03, -3.5),
            v(1.6, 0.05, 0.5),
            Mat::Hot,
            false,
        );
        p.boxed(
            v(-26.0 + i as f32 * 6.0, 0.03, 3.5),
            v(1.6, 0.05, 0.5),
            Mat::Hot,
            false,
        );
    }
    // Broken cooling room: units off their feet.
    p.sign_near(-12.0, -41.5, 3.0, 3.2, Sign::TempWarning);
    for (x, z, yaw) in [(-18.0, -38.0, 0.0), (-6.0, -38.0, 0.4)] {
        p.slab(v(x, 1.1, z), yaw, v(2.2, 2.2, 1.0), Mat::Metal);
        p.slab(v(x, 1.0, z + 0.55), yaw, v(1.8, 1.4, 0.04), Mat::Hot);
        p.block(v(x, 1.1, z), v(2.4, 2.2, 1.6));
    }
    // Arena: a ring of embers, fans that no longer spin and the way out is behind you.
    for k in 0..16 {
        let a = k as f32 * PI / 8.0;
        p.slab(
            v(36.0 + 14.5 * a.cos(), 0.03, 14.5 * a.sin()),
            -a + FRAC_PI_2,
            v(3.4, 0.04, 0.35),
            Mat::Hot,
        );
    }
    p.sign_near(36.0, -17.0, 3.0, 4.0, Sign::TempWarning);
    p.sign_near(52.0, 6.0, 2.4, 3.2, Sign::ThisIsFine);

    let pts = [
        (-52.0, 0.0),
        (-44.0, 6.0),
        (-22.0, -13.0),
        (-22.0, 13.0),
        (-3.0, -13.0),
        (-3.0, 13.0),
        (-12.0, -35.0),
        (14.0, 0.0),
        (30.0, -8.0),
        (30.0, 8.0),
        (42.0, 0.0),
        (-12.0, 0.0),
    ];
    point_lights(p, &pts, Color::srgb(1.0, 0.45, 0.2), 520_000.0);
    strips(p, Mat::Hot);
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:datacenter/1", (-57.0, 0.0), Sign::ToDataCenter1),
                ("door:campus", (-48.0, -10.0), Sign::ToCampus),
            ],
            packs: &[
                (-24.0, -14.0),
                (-24.0, 14.0),
                (-2.0, -14.0),
                (-2.0, 14.0),
                (-12.0, -35.0),
            ],
            boss: (43.0, 0.0),
            npc: Some((-44.0, -7.0)),
        },
    );
    theme(true)
}
