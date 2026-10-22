//! The Innovation Lab: lab/1 is the bright white open space full of buzzwords, lab/2 the dark
//! server sanctum with the circular AGI arena (pillars for line-of-sight play).

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::props::{beanbag, foosball, lounge, sad_plant, water_cooler};
use super::shape::{blob, pillar, rounded_rect, stroke};
use super::themes::{ceiling_lights, pt, runner, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    if level == 1 {
        p.ceiling = HALL_H;
    }
    let theme = if level == 1 {
        lab1(&mut p)
    } else {
        lab2(&mut p)
    };
    let (zone, name) = if level == 1 {
        ("lab/1", "Innovation Lab: Open Space")
    } else {
        ("lab/2", "Innovation Lab: The Sanctum")
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(dark: bool) -> Theme {
    let c = Color::srgb;
    if dark {
        Theme {
            wall: c(0.28, 0.22, 0.42),
            trim: c(0.05, 0.04, 0.09),
            accent: LinearRgba::rgb(2.6, 0.5, 3.0),
            fabric: [c(0.3, 0.1, 0.5), c(0.1, 0.3, 0.5), c(0.2, 0.1, 0.3)],
            floor: FloorKind::Tile([0.8, 0.7, 1.15]),
            fog: c(0.015, 0.01, 0.03),
            ambient: c(0.55, 0.45, 1.0),
            ambient_brightness: 480.0,
            sun: c(0.6, 0.5, 1.0),
            sun_illuminance: 1500.0,
            fog_range: (18.0, 65.0),
        }
    } else {
        Theme {
            wall: c(0.93, 0.94, 0.96),
            trim: c(0.55, 0.58, 0.65),
            accent: LinearRgba::rgb(0.3, 1.6, 2.4),
            fabric: [c(0.9, 0.4, 0.3), c(0.3, 0.6, 0.9), c(0.95, 0.8, 0.3)],
            floor: FloorKind::Gloss([1.0, 1.0, 1.02]),
            fog: c(0.55, 0.6, 0.68),
            ambient: c(0.95, 0.97, 1.0),
            ambient_brightness: 650.0,
            sun: c(1.0, 0.98, 0.95),
            sun_illuminance: 9000.0,
            fog_range: (45.0, 120.0),
        }
    }
}

fn lab1(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-52.0, 0.0), 9.0, 10.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-46.0, 0.0), pt(-36.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(-12.0, 0.0), 28.0, 24.0, 10.0, 0.0), w);
    p.fp.link(stroke(&[pt(12.0, 0.0), pt(30.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(38.0, 0.0), 12.0, 14.0, 6.0, 0.0), w);
    p.fp.pod(rounded_rect(pt(-12.0, -34.0), 10.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-12.0, -20.0), pt(-12.0, -30.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(-12.0, 34.0), 10.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-12.0, 20.0), pt(-12.0, 30.0)], 4.0), w);
    p.route = vec![
        pt(-56.0, 0.0),
        pt(-36.0, 0.0),
        pt(-12.0, 0.0),
        pt(30.0, 0.0),
        pt(38.0, 0.0),
    ];
    p.build_walls();

    // Whiteboards full of buzzwords on every long wall.
    for (x, z, s) in [
        (-38.0, -12.0, Sign::WhiteboardA),
        (-38.0, 12.0, Sign::WhiteboardB),
        (-24.0, -24.0, Sign::WhiteboardC),
        (-2.0, -24.0, Sign::WhiteboardA),
        (-24.0, 24.0, Sign::WhiteboardB),
        (-2.0, 24.0, Sign::WhiteboardC),
        (14.0, -12.0, Sign::WhiteboardB),
        (14.0, 12.0, Sign::WhiteboardA),
    ] {
        p.sign_near(x, z, 1.9, 4.6, s);
    }
    p.sign_near(-12.0, -41.5, 2.8, 4.0, Sign::InnovationHere);
    p.sign_near(-12.0, 41.5, 2.8, 3.4, Sign::FailFast);
    p.sign_near(-56.0, 6.0, 2.4, 2.4, Sign::Compute);
    p.sign_near(-56.0, -6.0, 2.4, 2.4, Sign::SeriesB);
    p.sign_near(46.0, -10.0, 2.6, 3.0, Sign::InnovationHere);

    // Beanbag islands, a foosball nobody beats, a water cooler.
    for (x, z, m) in [
        (-30.0, -4.0, Mat::BeanA),
        (-28.5, 4.5, Mat::BeanB),
        (-19.0, 6.0, Mat::BeanC),
        (-5.0, -5.5, Mat::BeanB),
        (3.0, 6.5, Mat::BeanA),
        (-20.0, -5.5, Mat::BeanC),
    ] {
        beanbag(p, x, z, m);
    }
    foosball(p, 6.0, -6.0);
    water_cooler(p, -33.0, 14.0);

    // Printing nothing: a 3D printer on a bench.
    p.boxed(v(-8.0, 0.45, -19.0), v(2.6, 0.9, 1.2), Mat::White, true);
    p.boxed(v(-8.0, 1.5, -19.4), v(1.4, 1.2, 0.08), Mat::Glass, false);
    p.boxed(v(-8.0, 2.15, -19.2), v(1.6, 0.1, 0.5), Mat::Metal, false);
    p.boxed(v(-8.0, 0.95, -18.7), v(0.6, 0.05, 0.4), Mat::Dark, false);
    p.sign(v(-8.0, 1.9, -18.55), 0.0, 1.2, Sign::Printing);

    // VR headsets on a table, with a sign about it.
    p.boxed(v(-14.0, 0.75, 19.0), v(3.6, 0.1, 1.2), Mat::White, false);
    p.boxed(v(-14.0, 0.37, 19.0), v(3.4, 0.74, 1.0), Mat::Metal, true);
    for k in 0..4 {
        let x = -15.2 + k as f32 * 0.8;
        p.boxed(v(x, 0.9, 19.0), v(0.35, 0.2, 0.22), Mat::Dark, false);
        p.boxed(v(x, 0.92, 18.88), v(0.28, 0.1, 0.02), Mat::NeonCyan, false);
    }
    p.sign(v(-14.0, 2.2, 21.4), PI, 2.0, Sign::VrDemo);
    p.boxed(v(-14.0, 1.1, 21.45), v(0.1, 2.2, 0.1), Mat::Metal, false);

    // Boss pod: a demo stage.
    p.boxed(v(40.0, 0.15, 0.0), v(8.0, 0.3, 8.0), Mat::Gold, false);
    p.sign_near(50.0, -6.0, 2.8, 4.0, Sign::SeriesB);
    // Brainstorm and VR nooks.
    p.boxed(v(-12.0, 0.05, -34.0), v(8.0, 0.1, 6.0), Mat::BeanC, false);
    p.boxed(v(-12.0, 0.05, 34.0), v(8.0, 0.1, 6.0), Mat::BeanB, false);
    for x in [-16.0, -8.0] {
        beanbag(p, x, 38.5, Mat::BeanA);
    }

    lab1_dressing(p);
    ceiling_lights(p);
    for (x, z) in [
        (-52.0, 0.0),
        (-24.0, -8.0),
        (-24.0, 8.0),
        (0.0, -8.0),
        (0.0, 8.0),
        (38.0, 0.0),
        (-12.0, -34.0),
        (-12.0, 34.0),
    ] {
        p.light(v(x, 3.6, z), Color::srgb(0.95, 0.97, 1.0), 420_000.0);
    }
    // Colour spill from the neon, the kombucha bar and the stage.
    for (x, y, z, c) in [
        (-50.0, 3.4, -8.0, Color::srgb(1.0, 0.45, 0.85)),
        (3.0, 3.0, 21.0, Color::srgb(0.5, 1.0, 0.6)),
        (-37.0, 2.8, -7.0, Color::srgb(1.0, 0.65, 0.35)),
        (40.0, 3.8, 0.0, Color::srgb(1.0, 0.5, 0.8)),
    ] {
        p.light(v(x, y, z), c, 260_000.0);
    }
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:campus", (-61.0, 0.0), Sign::ToCampus),
                ("door:lab/2", (50.0, 8.0), Sign::ToLab2),
            ],
            packs: &[
                (-27.0, -13.0),
                (-27.0, 13.0),
                (0.0, -14.0),
                (0.0, 14.0),
                (-12.0, -35.0),
            ],
            boss: (41.0, 0.0),
            npc: Some((-12.0, 33.0)),
        },
    );
    theme(false)
}

fn lab2(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-52.0, 0.0), 8.0, 9.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-46.0, 0.0), pt(-30.0, 0.0)], 3.5), w);
    p.fp.pod(blob(pt(-22.0, 0.0), 14.0, 15.0, 0.0, 0, 0.0), w);
    p.fp.link(stroke(&[pt(-12.0, 0.0), pt(6.0, 0.0)], 4.0), w);
    p.fp.pod(blob(pt(25.0, 0.0), 22.0, 22.0, 0.0, 0, 0.0), w);
    p.fp.pod(rounded_rect(pt(0.0, -31.0), 10.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-8.0, -2.0), pt(-4.0, -27.0)], 3.5), w);
    p.fp.pod(rounded_rect(pt(0.0, 31.0), 10.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-8.0, 2.0), pt(-4.0, 27.0)], 3.5), w);
    // Ring of pillars for line-of-sight play around the AGI.
    for k in 0..6 {
        let a = (20.0 + k as f32 * 60.0).to_radians();
        p.fp.hole(
            pillar(pt(25.0 + 11.0 * a.cos(), 11.0 * a.sin()), 1.3),
            Mat::Dark,
        );
    }
    p.route = vec![
        pt(-56.0, 0.0),
        pt(-30.0, 0.0),
        pt(-10.0, 0.0),
        pt(6.0, 0.0),
        pt(25.0, 0.0),
    ];
    p.build_walls();

    // Arena floor: neon rings and a dais under the prototype.
    for k in 0..24 {
        let a = k as f32 * PI / 12.0;
        let (c, s) = (a.cos(), a.sin());
        p.slab(
            v(25.0 + 17.5 * c, 0.03, 17.5 * s),
            -a + FRAC_PI_2,
            v(4.6, 0.05, 0.3),
            Mat::NeonPink,
        );
        p.slab(
            v(25.0 + 6.5 * c, 0.03, 6.5 * s),
            -a + FRAC_PI_2,
            v(1.8, 0.05, 0.2),
            Mat::NeonCyan,
        );
    }
    p.prim(
        PrimKind::Cylinder,
        v(33.0, 0.08, 0.0),
        v(7.0, 0.16, 7.0),
        Mat::Dark,
    );
    p.prim(
        PrimKind::Cylinder,
        v(33.0, 0.17, 0.0),
        v(6.2, 0.02, 6.2),
        Mat::NeonCyan,
    );
    for k in 0..6 {
        let a = (20.0 + k as f32 * 60.0).to_radians();
        let (x, z) = (25.0 + 11.0 * a.cos(), 11.0 * a.sin());
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.12, z),
            v(3.0, 0.24, 3.0),
            Mat::NeonPink,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x, 2.5, z),
            v(2.7, 0.14, 2.7),
            Mat::NeonPink,
        );
    }
    p.sign_near(25.0, -23.0, 3.0, 5.0, Sign::AgiArena);
    p.sign_near(25.0, 23.0, 3.0, 5.0, Sign::Compute);

    // Server monoliths along the approach, glowing violet.
    for x in [-28.0, -24.0, -20.0, -16.0] {
        for z in [-5.0, 5.0] {
            p.boxed(v(x, 1.4, z), v(1.2, 2.8, 0.8), Mat::Rack, true);
            p.boxed(
                v(x, 1.4, z + if z < 0.0 { 0.42 } else { -0.42 }),
                v(0.9, 2.5, 0.03),
                Mat::Monolith,
                false,
            );
        }
    }
    p.sign_near(-44.0, 8.0, 2.6, 2.4, Sign::ToLab2);
    p.sign_near(-60.0, 6.0, 2.4, 2.4, Sign::ToLab1);
    p.sign_near(-35.0, -14.0, 2.6, 2.6, Sign::Uptime);
    p.sign_near(-8.0, 38.0, 2.8, 3.0, Sign::DoNotUnplug);
    p.sign_near(-8.0, -38.0, 2.8, 3.0, Sign::SeriesB);
    // Side nooks: dark alcoves with a lone glowing rack.
    for z in [-37.6, 37.6] {
        p.boxed(v(0.0, 1.4, z), v(1.2, 2.8, 0.8), Mat::Rack, true);
        p.boxed(
            v(0.0, 1.4, z + if z < 0.0 { 0.42 } else { -0.42 }),
            v(0.9, 2.5, 0.03),
            Mat::Monolith,
            false,
        );
    }
    lab2_dressing(p);
    // Neon edge along the corridor floor.
    for x in [-44.0, -38.0, -32.0] {
        p.slab(v(x, 0.03, -2.4), 0.0, v(4.0, 0.04, 0.1), Mat::NeonCyan);
        p.slab(v(x, 0.03, 2.4), 0.0, v(4.0, 0.04, 0.1), Mat::NeonCyan);
    }

    for (x, z, c) in [
        (-52.0, 0.0, Color::srgb(0.7, 0.5, 1.0)),
        (-30.0, 0.0, Color::srgb(0.5, 0.6, 1.0)),
        (-22.0, -8.0, Color::srgb(0.8, 0.4, 1.0)),
        (-22.0, 8.0, Color::srgb(0.8, 0.4, 1.0)),
        (0.0, 0.0, Color::srgb(0.5, 0.7, 1.0)),
        (15.0, 0.0, Color::srgb(1.0, 0.4, 0.9)),
        (25.0, 12.0, Color::srgb(0.4, 0.9, 1.0)),
        (25.0, -12.0, Color::srgb(1.0, 0.4, 0.9)),
        (33.0, 0.0, Color::srgb(0.5, 1.0, 1.0)),
        (0.0, -31.0, Color::srgb(0.7, 0.5, 1.0)),
        (0.0, 31.0, Color::srgb(0.7, 0.5, 1.0)),
    ] {
        p.light(v(x, 3.4, z), c, 420_000.0);
    }
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:lab/1", (-60.0, 0.0), Sign::ToLab1),
                ("door:campus", (-52.0, -9.0), Sign::ToCampus),
            ],
            packs: &[
                (-24.0, -11.0),
                (-24.0, 11.0),
                (0.0, -32.0),
                (0.0, 32.0),
                (10.0, 10.0),
            ],
            boss: (33.0, 0.0),
            npc: Some((-52.0, 6.0)),
        },
    );
    theme(true)
}

/// Small deterministic wobble in -0.5..0.5, so repeated props are not copy-pasted.
fn jit(i: usize) -> f32 {
    ((i * 37 + 11) % 17) as f32 / 17.0 - 0.5
}

/// A standing-desk workstation for the open space: screen or laptop, a mug, a sticker-covered
/// lid. `yaw` is 0 or PI so the boxes stay axis aligned.
fn startup_desk(p: &mut Plan, i: usize, x: f32, z: f32, yaw: f32) {
    let top = DESK.size[1] * FURNITURE_SCALE;
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
        if i.is_multiple_of(2) {
            p.prop(&SCREEN, v(0.0, top, -0.2), 0.0, false);
            p.prop(&KEYBOARD, v(0.0, top, 0.1), 0.0, false);
        } else {
            p.prop(&LAPTOP, v(0.1, top, 0.0), 0.0, false);
            p.boxed(
                v(0.1, top + 0.17, -0.12),
                v(0.2, 0.14, 0.01),
                [Mat::BeanA, Mat::BeanB, Mat::BeanC][i % 3],
                false,
            );
        }
        p.prim(
            PrimKind::Cylinder,
            v(0.68, top + 0.06, 0.05),
            v(0.1, 0.12, 0.1),
            [Mat::White, Mat::Red, Mat::BeanB][i % 3],
        );
        // One succulent per island corner, not one per desk.
        if i.is_multiple_of(4) {
            p.prop(&PLANTS_SMALL[i % 3], v(-0.68, top, -0.25), 0.0, false);
        }
        p.prop(
            &CHAIR_DESK,
            v(0.1 + jit(i) * 0.3, 0.0, 0.95),
            PI + jit(i + 5) * 0.8,
            false,
        );
    });
}

/// Two desks back to back.
fn desk_island(p: &mut Plan, i: usize, x: f32, z: f32) {
    for (k, dx) in [-1.8, 1.8].into_iter().enumerate() {
        startup_desk(p, i + k * 2, x + dx, z, 0.0);
        startup_desk(p, i + k * 2 + 1, x + dx, z - 1.0, PI);
    }
}

/// A free-standing wall of sticky notes: seven pivots this quarter, all of them strategic.
/// Runs along x at (x, z); the front faces +z.
fn pivot_wall(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 1.65, 0.0), v(6.0, 3.1, 0.16), Mat::Board, true);
        p.boxed(v(0.0, 0.1, 0.0), v(6.1, 0.2, 0.22), Mat::Metal, false);
        p.boxed(v(0.0, 3.2, 0.0), v(6.1, 0.1, 0.22), Mat::Metal, false);
        for sx in [-3.0, 3.0] {
            p.boxed(v(sx, 1.65, 0.0), v(0.12, 3.3, 0.24), Mat::Metal, false);
        }
        p.sign(v(0.0, 2.55, 0.11), 0.0, 2.4, Sign::HypeWall);
        p.sign(v(0.0, 1.65, -0.11), PI, 3.6, Sign::WhiteboardB);
        let notes = [
            Mat::Yellow,
            Mat::BeanA,
            Mat::BeanB,
            Mat::BeanC,
            Mat::Plant,
            Mat::Paper,
        ];
        for r in 0..3 {
            for c in 0..8 {
                if (r + c) % 4 == 3 {
                    continue;
                }
                let i = r * 8 + c;
                p.boxed(
                    v(
                        -2.4 + c as f32 * 0.69 + jit(i) * 0.1,
                        0.65 + r as f32 * 0.5 + jit(i + 3) * 0.06,
                        0.1,
                    ),
                    v(0.38, 0.38, 0.02),
                    notes[(r + c * 2) % notes.len()],
                    false,
                );
            }
        }
        // Red string between the pivots.
        for (cx, cy, w, h) in [
            (-1.0, 1.15, 1.4, 0.025),
            (0.9, 0.9, 0.025, 0.55),
            (1.6, 1.4, 1.2, 0.025),
        ] {
            p.boxed(v(cx, cy, 0.105), v(w, h, 0.01), Mat::NeonRed, false);
        }
    });
}

/// Glass meeting pod; the door in the west side says AGI ROOM and opens onto one chair facing
/// a locked black box.
fn glass_pod(p: &mut Plan, x0: f32, x1: f32, z0: f32, z1: f32) {
    let zc = (z0 + z1) / 2.0;
    let rail = |p: &mut Plan, c: Vec3, size: Vec3| {
        p.boxed(v(c.x, 0.05, c.z), v(size.x, 0.1, size.z), Mat::Metal, false);
        p.boxed(
            v(c.x, 2.95, c.z),
            v(size.x, 0.08, size.z),
            Mat::Metal,
            false,
        );
    };
    // Walls along x (north, south) and along z (east, west with the door gap).
    for z in [z0, z1] {
        let (c, len) = ((x0 + x1) / 2.0, x1 - x0);
        p.boxed(v(c, 1.45, z), v(len, 2.9, 0.06), Mat::Glass, true);
        rail(p, v(c, 0.0, z), v(len, 0.0, 0.12));
    }
    p.boxed(v(x1, 1.45, zc), v(0.06, 2.9, z1 - z0), Mat::Glass, true);
    rail(p, v(x1, 0.0, zc), v(0.12, 0.0, z1 - z0));
    for (a, b) in [(z0, zc - 0.8), (zc + 0.8, z1)] {
        let (c, len) = ((a + b) / 2.0, b - a);
        p.boxed(v(x0, 1.45, c), v(0.06, 2.9, len), Mat::Glass, true);
        rail(p, v(x0, 0.0, c), v(0.12, 0.0, len));
    }
    for (x, z) in [
        (x0, z0),
        (x1, z0),
        (x1, z1),
        (x0, z1),
        (x0, zc - 0.8),
        (x0, zc + 0.8),
    ] {
        p.boxed(v(x, 1.5, z), v(0.12, 3.0, 0.12), Mat::Metal, false);
    }
    // Glass alone vanishes against the white hall: a frosted privacy band with metal rails and
    // mullions gives every wall a readable outline.
    let frame = |p: &mut Plan, cx: f32, cz: f32, along_x: bool, len: f32| {
        let (sx, sz) = if along_x { (len, 0.1) } else { (0.1, len) };
        p.boxed(v(cx, 1.25, cz), v(sx, 0.7, sz), Mat::Frosted, false);
        for y in [0.9, 1.6] {
            p.boxed(
                v(cx, y, cz),
                v(sx + 0.02, 0.06, sz + 0.02),
                Mat::Metal,
                false,
            );
        }
        let n = (len / 1.6).round().max(1.0) as usize;
        for k in 1..n {
            let t = k as f32 / n as f32 - 0.5;
            let (mx, mz) = if along_x {
                (cx + t * len, cz)
            } else {
                (cx, cz + t * len)
            };
            p.boxed(v(mx, 1.45, mz), v(0.07, 2.9, 0.07), Mat::Metal, false);
        }
    };
    for z in [z0, z1] {
        frame(p, (x0 + x1) / 2.0, z, true, x1 - x0);
    }
    frame(p, x1, zc, false, z1 - z0);
    for (a, b) in [(z0, zc - 0.8), (zc + 0.8, z1)] {
        frame(p, x0, (a + b) / 2.0, false, b - a);
    }
    p.boxed(v(x0, 2.72, zc), v(0.1, 0.3, 1.7), Mat::Metal, false);
    // The sign stands on two poles above the door.
    for dz in [-0.8, 0.8] {
        p.boxed(
            v(x0 - 0.02, 3.5, zc + dz),
            v(0.06, 1.1, 0.06),
            Mat::Metal,
            false,
        );
    }
    p.sign(v(x0 - 0.08, 3.75, zc), -FRAC_PI_2, 2.0, Sign::AgiRoom);
    // Inside: a pedestal with a very large model and a chair that has been waiting.
    let px = x1 - 1.2;
    p.boxed(v(px, 0.5, zc), v(0.8, 1.0, 0.8), Mat::Rack, true);
    p.boxed(
        v(px - 0.41, 0.6, zc),
        v(0.02, 0.7, 0.6),
        Mat::Monolith,
        false,
    );
    p.boxed(
        v(px - 0.41, 0.2, zc),
        v(0.02, 0.06, 0.5),
        Mat::LedBlue,
        false,
    );
    p.prim(
        PrimKind::Cylinder,
        v(px, 0.011, zc),
        v(2.2, 0.02, 2.2),
        Mat::NeonCyan,
    );
    p.prop(&CHAIR_MODERN, v(px - 1.5, 0.0, zc), FRAC_PI_2, false);
    p.light(v(px - 0.6, 2.4, zc), Color::srgb(0.5, 0.8, 1.0), 200_000.0);
}

/// Kombucha bar against the south wall: three taps, a back shelf of jars and bar stools.
fn kombucha_bar(p: &mut Plan, x: f32) {
    let z = 23.3;
    p.boxed(v(x, 0.55, z), v(4.4, 1.1, 1.1), Mat::Metal, true);
    p.boxed(v(x, 1.13, z), v(4.6, 0.06, 1.2), Mat::Wood, false);
    for (k, dx) in [-1.2, 0.0, 1.2].into_iter().enumerate() {
        p.prim(
            PrimKind::Cylinder,
            v(x + dx, 1.4, z + 0.05),
            v(0.12, 0.5, 0.12),
            Mat::Chrome,
        );
        p.boxed(
            v(x + dx, 1.62, z - 0.08),
            v(0.06, 0.06, 0.24),
            Mat::Chrome,
            false,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x + dx, 1.2, z - 0.35),
            v(0.14, 0.14, 0.14),
            [Mat::Plant, Mat::Yellow, Mat::BeanA][k],
        );
    }
    p.boxed(v(x, 1.55, 23.85), v(4.4, 0.05, 0.3), Mat::Wood, false);
    for k in 0..6 {
        p.prim(
            PrimKind::Cylinder,
            v(x - 2.0 + k as f32 * 0.8, 1.76, 23.85),
            v(0.22, 0.33, 0.22),
            [Mat::Water, Mat::Yellow, Mat::Plant][k % 3],
        );
    }
    p.sign_near(x, 24.0, 3.1, 3.4, Sign::Kombucha);
    for dx in [-1.5, 0.0, 1.5] {
        p.prop(
            &STOOL_BAR,
            v(x + dx, 0.0, 21.9),
            jit(dx as usize + 3) * 2.0,
            false,
        );
    }
}

/// The robot arm nobody gave a task, cordoned off with tape and holding a coffee.
fn robot_arm(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.15, z), v(0.9, 0.3, 0.9), Mat::Dark, true);
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.43, z),
        v(0.6, 0.26, 0.6),
        Mat::Metal,
    );
    p.boxed(v(x, 1.2, z), v(0.22, 1.3, 0.22), Mat::BeanA, false);
    p.prim(
        PrimKind::Cylinder,
        v(x, 1.9, z),
        v(0.4, 0.3, 0.4),
        Mat::Dark,
    );
    p.boxed(v(x + 0.75, 1.9, z), v(1.5, 0.2, 0.2), Mat::BeanA, false);
    p.prim(
        PrimKind::Cylinder,
        v(x + 1.5, 1.85, z),
        v(0.3, 0.22, 0.3),
        Mat::Dark,
    );
    for dz in [-0.09, 0.09] {
        p.boxed(
            v(x + 1.5, 1.62, z + dz),
            v(0.05, 0.3, 0.04),
            Mat::Metal,
            false,
        );
    }
    p.prim(
        PrimKind::Cylinder,
        v(x + 1.5, 1.58, z),
        v(0.12, 0.14, 0.12),
        Mat::White,
    );
    for (cx, cz, w, d) in [
        (0.0, -1.8, 3.6, 0.1),
        (0.0, 1.8, 3.6, 0.1),
        (-1.8, 0.0, 0.1, 3.6),
        (1.8, 0.0, 0.1, 3.6),
    ] {
        p.boxed(
            v(x + 0.8 + cx, 0.011, z + cz),
            v(w, 0.02, d),
            Mat::Hazard,
            false,
        );
    }
    p.sign_near(x - 1.9, z, 2.9, 2.2, Sign::RobotArm);
}

/// Prototype graveyard against the west wall: a bench of dead demos, candles, tombstones.
fn graveyard(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.35, z), v(0.9, 0.7, 3.4), Mat::Concrete, true);
    // A drone with one propeller missing.
    p.boxed(v(x, 0.78, z - 1.2), v(0.4, 0.08, 0.4), Mat::Dark, false);
    for (dx, dz) in [(-0.2, -0.2), (0.2, -0.2), (-0.2, 0.2)] {
        p.prim(
            PrimKind::Cylinder,
            v(x + dx, 0.84, z - 1.2 + dz),
            v(0.22, 0.02, 0.22),
            Mat::Metal,
        );
    }
    // A cardboard "robot", a headset, a wallet-sized "blockchain" box.
    p.boxed(
        v(x, 0.95, z - 0.2),
        v(0.45, 0.5, 0.35),
        Mat::Cardboard,
        false,
    );
    p.boxed(
        v(x, 1.3, z - 0.2),
        v(0.3, 0.25, 0.25),
        Mat::Cardboard,
        false,
    );
    p.boxed(v(x, 0.78, z + 0.6), v(0.3, 0.08, 0.2), Mat::Dark, false);
    p.boxed(
        v(x + 0.16, 0.78, z + 0.6),
        v(0.02, 0.05, 0.16),
        Mat::NeonCyan,
        false,
    );
    p.boxed(v(x, 0.85, z + 1.2), v(0.3, 0.22, 0.3), Mat::Gold, false);
    for dz in [-1.55, 1.55] {
        p.prim(
            PrimKind::Cylinder,
            v(x + 0.2, 0.78, z + dz),
            v(0.1, 0.16, 0.1),
            Mat::Candle,
        );
    }
    for (k, dz) in [-1.3, 0.0, 1.3].into_iter().enumerate() {
        let tx = x + 2.2;
        p.boxed(v(tx, 0.4, z + dz), v(0.14, 0.8, 0.55), Mat::Concrete, false);
        p.boxed(
            v(tx, 0.84, z + dz),
            v(0.14, 0.12, 0.4),
            Mat::Concrete,
            false,
        );
        p.boxed(
            v(tx - 0.08, 0.5 + k as f32 * 0.08, z + dz),
            v(0.02, 0.1, 0.3),
            Mat::Dark,
            false,
        );
    }
    sad_plant(p, x + 0.2, z + 2.3);
    p.sign_near(x - 0.6, z, 2.6, 2.6, Sign::ProtoGraveyard);
}

/// Folding chairs for a demo day nobody attends, knocked askew, beside the stage approach.
fn audience(p: &mut Plan, x: f32, z0: f32, step: f32) {
    let mut i = 0;
    for row in 0..2 {
        for k in 0..3 {
            p.prop(
                &CHAIR_MODERN,
                v(
                    x + row as f32 * 1.4 + jit(i) * 0.2,
                    0.0,
                    z0 + k as f32 * step,
                ),
                FRAC_PI_2 + jit(i + 7) * 0.5,
                false,
            );
            i += 1;
        }
    }
}

/// Flat area rug: a coloured field with a lighter border, below the runner's 0.02 so it never
/// fights it.
fn area_rug(p: &mut Plan, x: f32, z: f32, w: f32, d: f32, field: Mat, border: Mat) {
    p.slab(v(x, 0.005, z), 0.0, v(w, 0.01, d), border);
    p.slab(v(x, 0.006, z), 0.0, v(w - 0.5, 0.01, d - 0.5), field);
}

/// Low frosted partition marking a work zone; `yaw` is 0 (along x) or a quarter turn. Only the
/// white base is solid, the frosted panel above is for show.
fn partition(p: &mut Plan, x: f32, z: f32, len: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 0.4, 0.0), v(len, 0.8, 0.1), Mat::White, true);
        p.boxed(
            v(0.0, 1.05, 0.0),
            v(len - 0.1, 0.5, 0.05),
            Mat::Frosted,
            false,
        );
        p.boxed(v(0.0, 1.33, 0.0), v(len, 0.05, 0.12), Mat::Metal, false);
        for sx in [-1.0, 1.0] {
            p.boxed(
                v(sx * (len / 2.0 - 0.05), 0.67, 0.0),
                v(0.1, 1.34, 0.14),
                Mat::Metal,
                false,
            );
        }
    });
}

/// Concrete planter trough with a row of bushes, `yaw` 0 or a quarter turn.
fn planter(p: &mut Plan, x: f32, z: f32, len: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 0.3, 0.0), v(len, 0.6, 0.7), Mat::Concrete, true);
        p.boxed(v(0.0, 0.61, 0.0), v(len - 0.1, 0.02, 0.6), Mat::Dark, false);
        let n = (len / 0.55).ceil() as usize;
        for k in 0..n {
            let t = (k as f32 + 0.5) / n as f32 - 0.5;
            p.prim(
                PrimKind::Sphere,
                v(t * (len - 0.4), 0.95 + jit(k + 2) * 0.2, jit(k) * 0.12),
                v(0.62, 0.7 + jit(k + 4) * 0.2, 0.6),
                if k % 2 == 0 { Mat::Plant } else { Mat::Leaf },
            );
        }
    });
}

/// Pendant lamp on a cable from the ceiling: conical shade over a glowing disc.
fn pendant(p: &mut Plan, x: f32, z: f32, y: f32) {
    let top = p.top();
    p.boxed(
        v(x, (top + y) / 2.0, z),
        v(0.03, top - y, 0.03),
        Mat::Dark,
        false,
    );
    p.prim(
        PrimKind::Cone,
        v(x, y + 0.15, z),
        v(0.85, 0.3, 0.85),
        Mat::White,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x, y - 0.005, z),
        v(0.7, 0.03, 0.7),
        Mat::LightPanel,
    );
}

fn lab1_dressing(p: &mut Plan) {
    // A navy runner with cyan edges from the lobby to the demo stage, like the tower's hub.
    runner(
        p,
        &[
            pt(-58.0, 0.0),
            pt(-36.0, 0.0),
            pt(-12.0, 0.0),
            pt(30.0, 0.0),
            pt(35.0, 0.0),
        ],
        1.8,
        Mat::Runner,
        Mat::RunnerEdge,
    );
    // Lobby: two lounge nooks either side of the arrival, neon and the NDA at the door.
    lounge(p, -52.0, -6.2, 0.0, true);
    lounge(p, -52.0, 6.2, PI, false);
    p.sign_near(-61.0, 5.3, 2.4, 2.2, Sign::LabNda);
    p.sign_near(-61.0, -4.9, 2.6, 2.2, Sign::MoveFast);
    p.sign_near(-50.0, -10.0, 2.7, 4.0, Sign::InnovationNeon);
    p.poster_near(-45.0, 10.0, 2.4, Vec2::new(1.9, 0.95), Tex::TurnItOff);

    // Open space: three desk islands, the pivot wall and the glass pod.
    desk_island(p, 0, -31.0, -20.0);
    desk_island(p, 4, -6.0, 18.5);
    desk_island(p, 8, -32.0, 19.5);
    lounge(p, -12.0, -8.5, 0.0, true);
    lounge(p, -12.0, 8.5, PI, false);
    // Standing-meeting tables nobody sits at.
    for (x, z) in [(-20.0, -15.0), (-3.0, -9.5)] {
        p.prop(&TABLE_ROUND, v(x, 0.0, z), 0.0, true);
        p.prop(
            &LAPTOP,
            v(x, TABLE_ROUND.size[1] * FURNITURE_SCALE, z),
            0.4,
            false,
        );
    }
    pivot_wall(p, 11.0, -17.0, -FRAC_PI_2);
    glass_pod(p, 8.5, 14.0, 10.5, 17.0);
    kombucha_bar(p, 2.9);
    robot_arm(p, -38.0, -7.0);
    graveyard(p, -39.0, 6.9);

    // Rugs under the beanbag islands, so they read as zones rather than lumps.
    for (x, z) in [(-29.5, -4.0), (-5.5, -5.5), (2.5, 6.5)] {
        p.prop(&RUG_ROUND, v(x, 0.0, z), 0.0, false);
    }
    // Wall dressing: posters in the corridors, between the whiteboards and in the nooks.
    p.poster_near(-18.5, -24.0, 2.3, Vec2::new(1.2, 1.5), Tex::PosterBars);
    p.poster_near(-6.5, 24.0, 2.3, Vec2::new(2.0, 1.33), Tex::AiFirst);
    p.poster_near(18.0, -4.0, 2.4, Vec2::new(3.0, 1.5), Tex::Roadmap);
    p.poster_near(24.0, 4.0, 2.4, Vec2::new(2.4, 1.15), Tex::AiTransformation);
    p.poster_near(-44.0, 4.0, 2.4, Vec2::new(1.2, 1.5), Tex::Synergy);
    p.sign_near(-21.8, -34.0, 2.4, 2.4, Sign::WhiteboardB);
    p.sign_near(-2.2, -34.0, 2.4, 2.4, Sign::WhiteboardA);
    p.sign_near(-21.8, 34.0, 2.4, 2.4, Sign::WhiteboardC);
    p.sign_near(-2.2, 34.0, 2.4, 2.4, Sign::WhiteboardB);

    // Demo stage: neon edge, speaker stacks, a truss and the banner; chairs to the sides.
    let c = v(40.0, 0.0, 0.0);
    for dz in [-3.94, 3.94] {
        p.boxed(v(c.x, 0.32, dz), v(8.0, 0.04, 0.12), Mat::NeonPink, false);
    }
    for dx in [-3.94, 3.94] {
        p.boxed(
            v(c.x + dx, 0.32, 0.0),
            v(0.12, 0.04, 7.76),
            Mat::NeonPink,
            false,
        );
    }
    for dz in [-4.9, 4.9] {
        p.boxed(v(35.0, 0.8, dz), v(0.8, 1.6, 0.8), Mat::Dark, true);
        p.prim(
            PrimKind::Cylinder,
            v(35.4, 1.1, dz),
            v(0.5, 0.04, 0.5),
            Mat::Metal,
        );
        p.prim(
            PrimKind::Cylinder,
            v(35.4, 0.5, dz),
            v(0.6, 0.04, 0.6),
            Mat::Metal,
        );
    }
    for dz in [-4.0, 4.0] {
        p.boxed(v(44.2, 2.2, dz), v(0.2, 4.4, 0.2), Mat::Metal, false);
    }
    p.boxed(v(44.2, 4.3, 0.0), v(0.25, 0.25, 8.2), Mat::Metal, false);
    for dz in [-2.5, 0.0, 2.5] {
        p.boxed(v(43.9, 4.0, dz), v(0.3, 0.3, 0.3), Mat::Dark, false);
    }
    p.sign_near(50.0, 0.0, 3.0, 4.0, Sign::DemoDay);
    audience(p, 29.5, -10.0, 1.5);
    audience(p, 29.5, 6.0, 1.5);
    lab1_zoning(p);
}

/// Breaks the white hall into zones: area rugs, low partitions at the desk islands, planters
/// and pendant lamps. Everything stays clear of the runner lane, the packs and the doors.
fn lab1_zoning(p: &mut Plan) {
    // Desk islands: blue rugs; lounges: warm rugs; kombucha bar: a yellow strip.
    for (x, z) in [(-31.0, -20.4), (-32.0, 19.3), (-6.0, 18.4)] {
        area_rug(p, x, z, 8.6, 5.2, Mat::Fabric1, Mat::White);
    }
    for z in [-9.75, 9.75] {
        area_rug(p, -12.0, z, 8.0, 5.6, Mat::Fabric0, Mat::White);
    }
    area_rug(p, 2.9, 21.4, 6.4, 2.6, Mat::Fabric2, Mat::White);
    // Partitions on the hall side of each island, each capped by a planter.
    for (x, z, len) in [(-31.0, -17.9, 6.0), (-32.0, 16.6, 6.0), (-6.5, 15.7, 5.5)] {
        partition(p, x, z, len, 0.0);
        for sx in [-1.0, 1.0] {
            planter(p, x + sx * (len / 2.0 + 0.8), z, 1.4, 0.0);
        }
    }
    // Pendants over every zone.
    for (x, z) in [
        (-31.0, -20.4),
        (-32.0, 19.3),
        (-6.0, 18.4),
        (-12.0, -9.75),
        (-12.0, 9.75),
        (2.9, 21.4),
        (6.0, -6.0),
        (-29.0, 0.3),
    ] {
        pendant(p, x, z, 3.4);
    }
}

/// The dark sanctum: wall glow, cable trays, cooling pylons, a truss ring over the arena and
/// the arena's rim cabinets.
fn lab2_dressing(p: &mut Plan) {
    // A magenta band low on every wall, inset past the trim so it never fights it.
    p.pieces.push(Piece::Run {
        segs: p.walls.clone(),
        mat: Mat::Cable,
        y0: 0.55,
        y1: 0.6,
        inset: 0.03,
    });

    // Lobby: racks along the west wall flank the arrival; floor lamps ring the greeter.
    for z in [-5.0, 5.0] {
        p.framed(v(-58.9, 0.0, z), FRAC_PI_2, |p| {
            p.boxed(v(0.0, 1.4, 0.0), v(1.2, 2.8, 0.8), Mat::Rack, true);
            p.boxed(v(0.0, 1.4, 0.42), v(0.9, 2.5, 0.03), Mat::Monolith, false);
        });
    }
    for k in 0..8 {
        let a = k as f32 * PI / 4.0;
        p.boxed(
            v(-52.0 + 1.8 * a.cos(), 0.015, 6.0 + 1.8 * a.sin()),
            v(0.3, 0.03, 0.3),
            Mat::NeonCyan,
            false,
        );
    }
    p.sign_near(-56.0, -9.0, 2.6, 2.6, Sign::LabRules);

    // Corridor: cable bundles along both walls and a tray overhead.
    for z in [-3.2, 3.2] {
        p.boxed(v(-38.0, 0.08, z), v(15.0, 0.16, 0.2), Mat::Cable, false);
    }
    p.boxed(v(-38.0, 4.1, 0.0), v(15.0, 0.12, 0.7), Mat::Dark, false);
    for x in [-44.0, -38.0, -32.0] {
        p.boxed(v(x, 4.3, 0.0), v(0.08, 0.4, 0.08), Mat::Metal, false);
    }

    // Hub: cooling pylons with cold bands.
    for (x, z) in [(-30.0, -10.0), (-30.0, 10.0), (-14.0, -10.0), (-14.0, 10.0)] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 2.25, z),
            v(0.6, 4.5, 0.6),
            Mat::Metal,
        );
        for y in [1.0, 3.2] {
            p.prim(
                PrimKind::Cylinder,
                v(x, y, z),
                v(0.66, 0.12, 0.66),
                Mat::Cold,
            );
        }
        p.block(v(x, 1.0, z), v(0.6, 2.0, 0.6));
    }
    // Hub dressing: a lit floor emblem on the route (flat, never a blocker), overhead trays
    // tying the pylons together, and status racks and NOC screens along the rim.
    p.prim(
        PrimKind::Cylinder,
        v(-22.0, 0.012, 0.0),
        v(8.0, 0.02, 8.0),
        Mat::Dark,
    );
    p.prim(
        PrimKind::Cylinder,
        v(-22.0, 0.016, 0.0),
        v(7.4, 0.02, 7.4),
        Mat::NeonCyan,
    );
    p.prim(
        PrimKind::Cylinder,
        v(-22.0, 0.02, 0.0),
        v(7.2, 0.02, 7.2),
        Mat::Dark,
    );
    p.prim(
        PrimKind::Cylinder,
        v(-22.0, 0.024, 0.0),
        v(3.0, 0.02, 3.0),
        Mat::NeonPink,
    );
    p.prim(
        PrimKind::Cylinder,
        v(-22.0, 0.028, 0.0),
        v(2.7, 0.02, 2.7),
        Mat::Dark,
    );
    for z in [-10.0, 10.0] {
        p.boxed(v(-22.0, 4.05, z), v(16.0, 0.12, 0.5), Mat::Dark, false);
        p.boxed(v(-22.0, 3.97, z), v(16.0, 0.03, 0.08), Mat::Cable, false);
    }
    for x in [-30.0, -14.0] {
        p.boxed(v(x, 4.05, 0.0), v(0.5, 0.12, 20.0), Mat::Dark, false);
    }
    for (x, z, tex) in [
        (-22.0, -14.5, Tex::NocGraph),
        (-22.0, 14.5, Tex::NocAlerts),
        (-30.0, 13.5, Tex::NocRegions),
    ] {
        p.poster_near(x, z, 2.2, Vec2::new(3.0, 1.6), tex);
    }
    for (x, z) in [(-17.0, -14.0), (-27.0, -14.0), (-17.0, 14.0), (-27.0, 14.0)] {
        if let Some((at, n)) = p.wall_near(x, z) {
            let yaw = n.x.atan2(n.y);
            let c = at + n * 0.45;
            p.framed(v(c.x, 0.0, c.y), yaw, |p| {
                p.boxed(v(0.0, 1.2, 0.0), v(1.0, 2.4, 0.7), Mat::Rack, false);
                p.boxed(v(0.0, 1.2, 0.36), v(0.8, 2.1, 0.03), Mat::Monolith, false);
                for (k, led) in [
                    Mat::BlinkGreenA,
                    Mat::BlinkRed,
                    Mat::BlinkBlue,
                    Mat::BlinkGreenB,
                ]
                .into_iter()
                .enumerate()
                {
                    p.boxed(
                        v(0.36, 0.5 + k as f32 * 0.45, 0.38),
                        v(0.05, 0.05, 0.02),
                        led,
                        false,
                    );
                }
            });
        }
    }
    // Chevrons on the floor point towards the arena.
    for x in [-6.0, -2.0, 2.0] {
        for s in [-1.0, 1.0] {
            p.slab(
                v(x, 0.03, 0.55 * s),
                s * 0.7,
                v(1.5, 0.03, 0.18),
                Mat::NeonPink,
            );
        }
    }

    // Arena: a truss ring on cables overhead, hazard tape round the dais, rim cabinets.
    let centre = (25.0, 0.0);
    for k in 0..24 {
        let a = k as f32 * PI / 12.0;
        let (c, s) = (a.cos(), a.sin());
        p.slab(
            v(centre.0 + 13.0 * c, 4.1, 13.0 * s),
            -a + FRAC_PI_2,
            v(3.5, 0.2, 0.3),
            Mat::Dark,
        );
        p.slab(
            v(centre.0 + 13.0 * c, 3.97, 13.0 * s),
            -a + FRAC_PI_2,
            v(3.4, 0.04, 0.1),
            Mat::NeonCyan,
        );
    }
    for k in 0..6 {
        let a = k as f32 * PI / 3.0 + 0.5;
        p.boxed(
            v(centre.0 + 13.0 * a.cos(), 4.3, 13.0 * a.sin()),
            v(0.06, 0.4, 0.06),
            Mat::Metal,
            false,
        );
    }
    for k in 0..16 {
        let a = k as f32 * PI / 8.0;
        p.slab(
            v(33.0 + 4.7 * a.cos(), 0.025, 4.7 * a.sin()),
            -a + FRAC_PI_2,
            v(1.5, 0.02, 0.2),
            if k % 2 == 0 { Mat::Hazard } else { Mat::Dark },
        );
    }
    for k in 0..8 {
        let a = k as f32 * PI / 4.0 + PI / 8.0;
        let (c, s) = (a.cos(), a.sin());
        let yaw = -a + FRAC_PI_2;
        p.slab(
            v(centre.0 + 21.0 * c, 1.4, 21.0 * s),
            yaw,
            v(1.6, 2.8, 0.5),
            Mat::Rack,
        );
        p.slab(
            v(centre.0 + 20.72 * c, 1.4, 20.72 * s),
            yaw,
            v(1.3, 2.5, 0.03),
            Mat::Monolith,
        );
    }
    p.sign_near(46.0, 0.0, 2.6, 3.0, Sign::TempWarning);
    p.sign_near(8.0, 16.0, 2.6, 3.0, Sign::ThisIsFine);
    p.light(v(45.0, 2.6, 0.0), Color::srgb(1.0, 0.45, 0.3), 220_000.0);
    p.light(v(-22.0, 3.0, 0.0), Color::srgb(0.4, 0.8, 1.0), 220_000.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floors::zones::build_zone;

    /// Dressing placed by hand must not poke through a wall: every solid box and prop stands
    /// inside the outline (the hall is rounded, so corner props are the usual offenders).
    #[test]
    fn solid_dressing_stays_inside_the_outline() {
        for zone in ["lab/1", "lab/2"] {
            let Some(l) = build_zone(zone) else {
                panic!("zone {zone} missing");
            };
            for piece in &l.pieces {
                let at = match piece {
                    Piece::Box {
                        center,
                        solid: true,
                        ..
                    } => *center,
                    Piece::Prop {
                        pos, solid: true, ..
                    } => *pos,
                    _ => continue,
                };
                assert!(
                    l.footprint.contains(Vec2::new(at.x, at.z)) || at.y > 4.0,
                    "{zone}: solid piece outside the outline at {at}"
                );
            }
        }
    }
}
