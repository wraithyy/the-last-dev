//! The Innovation Lab: lab/1 is the bright white open space full of buzzwords, lab/2 the dark
//! server sanctum with the circular AGI arena (pillars for line-of-sight play).

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::props::{beanbag, foosball, water_cooler};
use super::shape::{blob, pillar, rounded_rect, stroke};
use super::themes::{ceiling_lights, pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
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
