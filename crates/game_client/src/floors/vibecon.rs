//! VibeCon, the second raid: a tech conference in five rooms. vibecon/1 is registration and the
//! expo hall with booth rows and swag tables, vibecon/2 the workshop corridor with the prompt
//! masterclass, vibecon/3 the hackathon corner with the demo-day stage, vibecon/4 the afterparty
//! lounge with the fireside-chat panel, vibecon/5 the main keynote stage. Boxes and primitives in
//! the offsite's budget; the look is black carpet, neon lanyard colours and screens.

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::props::{beanbag, lounge};
use super::shape::{blob, rounded_rect, stroke};
use super::themes::{ceiling_lights, pt, runner, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// The four lanyard colours (exhibitor, volunteer, speaker, hacker), reused everywhere.
const NEON: [Mat; 4] = [Mat::NeonYellow, Mat::LedGreen, Mat::NeonCyan, Mat::NeonPink];

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => ("vibecon/1", "VibeCon: Expo Hall", expo(&mut p)),
        2 => ("vibecon/2", "VibeCon: Workshop Rooms", workshops(&mut p)),
        3 => ("vibecon/3", "VibeCon: Hackathon Corner", hackathon(&mut p)),
        4 => ("vibecon/4", "VibeCon: Afterparty", afterparty(&mut p)),
        _ => ("vibecon/5", "VibeCon: Main Stage", main_stage(&mut p)),
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let base = Theme {
        wall: c(0.3, 0.28, 0.4),
        trim: c(0.08, 0.08, 0.12),
        accent: LinearRgba::rgb(0.4, 2.6, 3.2),
        fabric: [c(0.95, 0.3, 0.75), c(0.2, 0.8, 0.95), c(0.98, 0.85, 0.2)],
        floor: FloorKind::Carpet([0.55, 0.5, 0.7]),
        fog: c(0.08, 0.07, 0.13),
        ambient: c(0.9, 0.9, 1.0),
        ambient_brightness: 780.0,
        sun: c(0.95, 0.95, 1.0),
        sun_illuminance: 7000.0,
        fog_range: (45.0, 130.0),
    };
    match level {
        1 => base,
        2 => Theme {
            wall: c(0.86, 0.86, 0.9),
            trim: c(0.3, 0.24, 0.45),
            accent: LinearRgba::rgb(1.6, 0.9, 3.2),
            floor: FloorKind::Carpet([0.55, 0.5, 0.72]),
            fog: c(0.35, 0.32, 0.45),
            ambient_brightness: 680.0,
            sun_illuminance: 7000.0,
            ..base
        },
        3 => Theme {
            wall: c(0.3, 0.3, 0.32),
            trim: c(0.12, 0.12, 0.14),
            accent: LinearRgba::rgb(3.2, 2.6, 0.4),
            floor: FloorKind::Tile([0.55, 0.55, 0.6]),
            fog: c(0.1, 0.1, 0.12),
            ambient_brightness: 560.0,
            ..base
        },
        4 => Theme {
            wall: c(0.12, 0.05, 0.16),
            trim: c(0.04, 0.02, 0.06),
            accent: LinearRgba::rgb(3.2, 0.6, 2.6),
            floor: FloorKind::Gloss([0.22, 0.14, 0.3]),
            fog: c(0.06, 0.02, 0.08),
            ambient: c(1.0, 0.75, 1.0),
            ambient_brightness: 480.0,
            sun_illuminance: 3500.0,
            ..base
        },
        _ => Theme {
            wall: c(0.08, 0.08, 0.12),
            trim: c(0.02, 0.02, 0.04),
            accent: LinearRgba::rgb(0.6, 2.8, 3.4),
            floor: FloorKind::Carpet([0.2, 0.2, 0.3]),
            fog: c(0.02, 0.02, 0.05),
            ambient_brightness: 520.0,
            sun_illuminance: 3500.0,
            ..base
        },
    }
}

// ------------------------------------------------------------------ shared props

/// Exhibitor booth facing `face`: back wall with a sign, a counter, two screens and a roll-up.
fn booth(p: &mut Plan, x: f32, z: f32, face: Face, neon: Mat, sign: Sign) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 1.4, -1.6), v(4.4, 2.8, 0.2), Mat::Dark, true);
        p.boxed(v(0.0, 2.85, -1.6), v(4.5, 0.1, 0.25), neon, false);
        p.panel(
            v(0.0, 2.1, -1.48),
            0.0,
            Vec2::new(3.8, 1.4),
            Tex::Sign(sign),
        );
        p.boxed(v(0.0, 0.55, 0.2), v(2.6, 1.1, 0.8), Mat::White, true);
        p.boxed(v(0.0, 1.12, 0.2), v(2.7, 0.06, 0.9), neon, false);
        for sx in [-0.7, 0.7] {
            p.boxed(v(sx, 1.4, 0.2), v(0.6, 0.4, 0.05), Mat::Dark, false);
            p.boxed(
                v(sx, 1.4, 0.23),
                v(0.52, 0.32, 0.02),
                Mat::ScreenGlow,
                false,
            );
        }
        // Roll-up banner beside the counter, logo side out.
        p.boxed(v(1.9, 1.0, 0.1), v(0.8, 2.0, 0.04), neon, false);
        p.boxed(v(1.9, 0.04, 0.1), v(0.9, 0.08, 0.3), Mat::Metal, false);
    });
}

/// Swag table: white cloth, piles of folded T-shirts in neon, sticker sheets, tote bags.
fn swag_table(p: &mut Plan, x: f32, z: f32, along_x: bool) {
    let (w, d) = if along_x { (3.2, 1.0) } else { (1.0, 3.2) };
    p.boxed(v(x, 0.4, z), v(w, 0.8, d), Mat::White, true);
    for k in 0..4 {
        let t = -1.2 + k as f32 * 0.8;
        let (dx, dz) = if along_x { (t, 0.0) } else { (0.0, t) };
        let h = 0.12 + (k % 3) as f32 * 0.08;
        p.boxed(
            v(x + dx, 0.8 + h / 2.0, z + dz),
            v(0.5, h, 0.4),
            NEON[k % 4],
            false,
        );
    }
    let (sx, sz) = if along_x {
        (0.0, d / 2.0)
    } else {
        (w / 2.0, 0.0)
    };
    p.boxed(
        v(x + sx, 0.95, z + sz),
        v(0.3, 0.3, 0.3),
        Mat::Cardboard,
        false,
    );
}

/// Rail of hanging lanyards in the four neon colours.
fn lanyard_rack(p: &mut Plan, x: f32, z: f32) {
    for sx in [-1.2, 1.2] {
        p.boxed(v(x + sx, 0.9, z), v(0.08, 1.8, 0.08), Mat::Metal, false);
    }
    p.boxed(v(x, 1.8, z), v(2.5, 0.05, 0.05), Mat::Metal, false);
    for k in 0..10 {
        p.boxed(
            v(x - 1.05 + k as f32 * 0.23, 1.45, z),
            v(0.04, 0.7, 0.02),
            NEON[k % 4],
            false,
        );
    }
    p.block(v(x, 0.9, z), v(2.6, 1.8, 0.3));
}

/// Queue posts with neon ropes between them along `pts`.
fn queue_line(p: &mut Plan, pts: &[(f32, f32)]) {
    for (k, &(x, z)) in pts.iter().enumerate() {
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.5, z),
            v(0.12, 1.0, 0.12),
            Mat::Chrome,
        );
        if let Some(&(nx, nz)) = pts.get(k + 1) {
            let (dx, dz) = (nx - x, nz - z);
            let len = (dx * dx + dz * dz).sqrt();
            p.slab(
                v((x + nx) / 2.0, 0.85, (z + nz) / 2.0),
                dx.atan2(dz) + FRAC_PI_2,
                v(len, 0.04, 0.04),
                NEON[k % 4],
            );
        }
    }
}

/// Two truss towers and a crossbar carrying a sign that faces `yaw`.
fn truss_banner(p: &mut Plan, x: f32, z: f32, yaw: f32, width: f32, sign: Sign) {
    let side = Vec2::new(yaw.cos(), -yaw.sin()) * (width / 2.0 + 0.3);
    for s in [-1.0, 1.0] {
        let (tx, tz) = (x + side.x * s, z + side.y * s);
        p.boxed(v(tx, 2.0, tz), v(0.3, 4.0, 0.3), Mat::Metal, false);
        p.block(v(tx, 1.0, tz), v(0.4, 2.0, 0.4));
    }
    p.slab(v(x, 4.0, z), yaw, v(width + 0.8, 0.2, 0.2), Mat::Metal);
    let (cw, ch) = sign_canvas(sign);
    let height = width * ch as f32 / cw as f32;
    p.sign(v(x, 3.85 - height / 2.0, z), yaw, width, sign);
}

/// Stage riser with a neon lip, an LED wall behind it showing `sign`, facing `face`.
fn stage(p: &mut Plan, x: f32, z: f32, face: Face, w: f32, d: f32, sign: Sign) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 0.15, 0.0), v(w, 0.3, d), Mat::MarbleDark, false);
        p.boxed(
            v(0.0, 0.31, d / 2.0),
            v(w, 0.03, 0.08),
            Mat::NeonCyan,
            false,
        );
        p.boxed(
            v(0.0, 2.6, -d / 2.0 - 0.2),
            v(w - 1.0, 4.4, 0.3),
            Mat::Dark,
            false,
        );
        p.panel(
            v(0.0, 2.8, -d / 2.0 - 0.04),
            0.0,
            Vec2::new(w - 1.6, (w - 1.6) * 96.0 / 256.0),
            Tex::Sign(sign),
        );
        for k in 0..5 {
            let sx = -w / 2.0 + 1.0 + k as f32 * (w - 2.0) / 4.0;
            p.boxed(
                v(sx, 4.6, d / 2.0 - 0.5),
                v(0.4, 0.25, 0.4),
                Mat::LightPanel,
                false,
            );
        }
    });
}

/// Hackathon table: laptops with glowing lids, pizza boxes and an energy-drink pyramid.
fn hack_table(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.38, z), v(5.0, 0.76, 1.2), Mat::Wood, true);
    for k in 0..4 {
        let lx = x - 1.8 + k as f32 * 1.2;
        for s in [-1.0, 1.0] {
            p.boxed(
                v(lx, 0.8, z + 0.3 * s),
                v(0.5, 0.03, 0.35),
                Mat::Dark,
                false,
            );
            p.boxed(
                v(lx, 0.95, z + 0.46 * s),
                v(0.5, 0.3, 0.02),
                Mat::ScreenGlow,
                false,
            );
        }
    }
    p.boxed(
        v(x + 2.0, 0.84, z),
        v(0.6, 0.08, 0.6),
        Mat::Cardboard,
        false,
    );
    p.boxed(
        v(x + 2.05, 0.92, z + 0.05),
        v(0.6, 0.08, 0.6),
        Mat::Cardboard,
        false,
    );
    for (k, (dx, dy)) in [(-0.1, 0.0), (0.1, 0.0), (0.0, 0.16)]
        .into_iter()
        .enumerate()
    {
        p.prim(
            PrimKind::Cylinder,
            v(x - 2.1 + dx, 0.84 + dy, z),
            v(0.09, 0.16, 0.09),
            NEON[k + 1],
        );
    }
}

/// Armchair facing `yaw` (panel seats, lounge corners).
fn armchair(p: &mut Plan, x: f32, z: f32, yaw: f32, mat: Mat) {
    p.prop(&LOUNGE_CHAIR, v(x, 0.0, z), yaw, true);
    let back = Vec2::new(-yaw.sin(), -yaw.cos()) * 0.45;
    p.slab(v(x + back.x, 1.0, z + back.y), yaw, v(0.9, 0.08, 0.08), mat);
}

/// Strips of neon along the floor between two points (the lanyard-colour carpet trim).
fn floor_neon(p: &mut Plan, a: (f32, f32), b: (f32, f32), mat: Mat) {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dz * dz).sqrt();
    p.slab(
        v((a.0 + b.0) / 2.0, 0.02, (a.1 + b.1) / 2.0),
        dx.atan2(dz) + FRAC_PI_2,
        v(len, 0.03, 0.18),
        mat,
    );
}

fn plant(p: &mut Plan, x: f32, z: f32) {
    p.prop(&POTTED_PLANT, v(x, 0.0, z), 0.0, true);
}

fn lights(p: &mut Plan, spots: &[(f32, f32)], color: Color, intensity: f32) {
    ceiling_lights(p);
    for &(x, z) in spots {
        p.light(v(x, 3.6, z), color, intensity);
    }
}

// ------------------------------------------------------------------ vibecon/1: expo hall

fn expo(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-60.0, 0.0), 6.0, 7.0, 3.0, 0.0), w);
    p.fp.link(stroke(&[pt(-55.0, 0.0), pt(-44.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(-34.0, 0.0), 10.0, 13.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-25.0, 0.0), pt(-20.0, 0.0)], 6.0), w);
    p.fp.pod(rounded_rect(pt(8.0, 0.0), 30.0, 24.0, 8.0, 0.0), w);
    p.route = vec![
        pt(-60.0, 0.0),
        pt(-34.0, 0.0),
        pt(-12.0, 0.0),
        pt(10.0, 0.0),
        pt(22.0, 0.0),
    ];
    p.build_walls();

    runner(
        p,
        &[
            pt(-62.0, 0.0),
            pt(-34.0, 0.0),
            pt(-12.0, 0.0),
            pt(24.0, 0.0),
        ],
        2.6,
        Mat::Runner,
        Mat::NeonPink,
    );
    // Registration: the desk, the lanyard rack and the queue snaking towards it.
    p.sign_near(-34.0, -12.8, 2.9, 6.0, Sign::VibeConRegistration);
    p.boxed(v(-34.0, 0.55, -9.4), v(7.0, 1.1, 1.0), Mat::Dark, true);
    p.boxed(
        v(-34.0, 1.12, -9.4),
        v(7.2, 0.06, 1.2),
        Mat::NeonCyan,
        false,
    );
    for k in 0..3 {
        let x = -36.5 + k as f32 * 2.5;
        p.boxed(v(x, 1.3, -9.6), v(0.45, 0.3, 0.05), Mat::Dark, false);
        p.boxed(
            v(x, 1.3, -9.57),
            v(0.38, 0.22, 0.02),
            Mat::ScreenGlow,
            false,
        );
    }
    lanyard_rack(p, -40.0, -11.6);
    lanyard_rack(p, -28.0, -11.6);
    queue_line(
        p,
        &[(-30.0, -6.5), (-38.0, -6.5), (-38.0, -4.0), (-30.0, -4.0)],
    );
    p.sign_near(-43.8, -6.0, 2.4, 3.0, Sign::VibeConLanyards);
    p.sign_near(-34.0, 12.8, 2.8, 5.0, Sign::VibeConWifi);
    for (x, z) in [(-42.0, 10.0), (-26.0, 10.0), (-56.0, -5.0), (-56.0, 5.0)] {
        plant(p, x, z);
    }
    truss_banner(p, -21.0, 0.0, Face::NegX.yaw(), 6.0, Sign::VibeConBanner);

    // The hall: booths against both long walls, swag islands in between, the Ship Live stage
    // at the far end. Neon trim marks the aisles.
    let signs = [Sign::VibeConBooth, Sign::VibeConSwag, Sign::VibeConBooth];
    for (k, x) in [-12.0, 4.0, 20.0].into_iter().enumerate() {
        booth(p, x, -21.0, Face::PosZ, NEON[k % 4], signs[k]);
        booth(
            p,
            x - 4.0,
            21.0,
            Face::NegZ,
            NEON[(k + 2) % 4],
            signs[(k + 1) % 3],
        );
    }
    for (x, z, along) in [
        (-14.0, 5.0, true),
        (-14.0, -5.0, true),
        (2.0, 5.5, true),
        (2.0, -5.5, true),
    ] {
        swag_table(p, x, z, along);
    }
    for (a, b, m) in [
        ((-20.0, 3.0), (26.0, 3.0), Mat::NeonPink),
        ((-20.0, -3.0), (26.0, -3.0), Mat::NeonCyan),
    ] {
        floor_neon(p, a, b, m);
    }
    for (x, z) in [(-20.0, -18.0), (-20.0, 18.0), (34.0, -16.0), (34.0, 16.0)] {
        plant(p, x, z);
    }
    stage(p, 33.0, 0.0, Face::NegX, 12.0, 5.0, Sign::VibeConShipLive);
    p.sign_near(8.0, 23.8, 3.0, 6.0, Sign::VibeConBanner);

    lights(
        p,
        &[
            (-60.0, 0.0),
            (-34.0, -6.0),
            (-34.0, 6.0),
            (-12.0, -12.0),
            (-12.0, 12.0),
            (8.0, 0.0),
            (20.0, -12.0),
            (20.0, 12.0),
            (30.0, 0.0),
        ],
        Color::srgb(0.85, 0.85, 1.0),
        420_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:campus", (-66.0, 0.0), Sign::ToCampus),
                ("door:vibecon/2", (38.0, 14.0), Sign::ToWorkshops),
            ],
            packs: &[(-34.0, 8.5), (-6.0, -13.0), (-6.0, 13.0), (16.0, 13.0)],
            boss: (28.0, 0.0),
            npc: Some((-34.0, -7.8)),
        },
    );
    theme(1)
}

// ------------------------------------------------------------------ vibecon/2: workshops

fn workshops(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-60.0, 0.0), 6.0, 7.0, 3.0, 0.0), w);
    p.fp.link(stroke(&[pt(-56.0, 0.0), pt(-8.0, 0.0)], 3.0), w);
    for x in [-48.0, -34.0, -20.0] {
        for s in [-1.0, 1.0] {
            p.fp.pod(rounded_rect(pt(x, 14.0 * s), 6.0, 5.0, 2.0, 0.0), w);
            p.fp.link(stroke(&[pt(x, 2.0 * s), pt(x, 10.0 * s)], 2.0), w);
        }
    }
    // The masterclass auditorium.
    p.fp.pod(blob(pt(16.0, 0.0), 26.0, 22.0, 0.0, 0, 0.0), w);
    p.fp.link(stroke(&[pt(36.0, 0.0), pt(46.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(52.0, 0.0), 7.0, 8.0, 3.0, 0.0), w);
    p.route = vec![
        pt(-58.0, 0.0),
        pt(-40.0, 0.0),
        pt(-12.0, 0.0),
        pt(4.0, 0.0),
        pt(14.0, 0.0),
    ];
    p.build_walls();

    runner(
        p,
        &[
            pt(-58.0, 0.0),
            pt(-30.0, 0.0),
            pt(-10.0, 0.0),
            pt(12.0, 0.0),
        ],
        2.2,
        Mat::Runner,
        Mat::NeonCyan,
    );
    for x in [-48.0, -34.0, -20.0] {
        for z in [-3.3, 3.3] {
            p.sign_near(x + 3.0, z, 2.2, 1.7, Sign::VibeConWorkshop);
        }
    }
    p.sign_near(-52.0, -7.0, 2.4, 3.0, Sign::VibeConWifi);
    // Workshop rooms: rows of chairs facing a screen, laptops on every lap.
    for (x, s) in [(-48.0, -1.0), (-34.0, 1.0), (-20.0, -1.0), (-34.0, -1.0)] {
        let z = 14.0 * s;
        p.poster_near(x, z + 5.0 * s, 2.0, Vec2::new(3.4, 1.9), Tex::PromptTips);
        for row in 0..2 {
            for k in 0..3 {
                let cx = x - 2.0 + k as f32 * 2.0;
                let cz = z + (1.8 - row as f32 * 1.6) * s;
                p.prop(
                    &CHAIR_MODERN,
                    v(cx, 0.0, cz),
                    if s > 0.0 { 0.0 } else { PI },
                    false,
                );
            }
        }
    }
    for (x, s) in [(-48.0, 1.0), (-20.0, 1.0)] {
        let z = 14.0 * s;
        beanbag(p, x - 2.5, z + 2.0, Mat::BeanA);
        beanbag(p, x + 2.5, z + 2.0, Mat::BeanC);
        p.poster_near(x, z + 5.0, 2.0, Vec2::new(2.6, 1.8), Tex::Stickies);
    }

    // Auditorium: tiered seat rows facing the projection wall, the lectern at the front.
    for row in 0..3 {
        let x = 9.0 + row as f32 * 3.0;
        for s in [-1.0, 1.0] {
            p.boxed(v(x, 0.25, 11.0 * s), v(1.0, 0.5, 9.0), Mat::Fabric0, true);
            p.boxed(
                v(x - 0.45, 0.7, 11.0 * s),
                v(0.15, 0.6, 9.0),
                Mat::Dark,
                false,
            );
        }
    }
    p.boxed(v(36.0, 2.4, 0.0), v(0.3, 4.4, 14.0), Mat::Dark, false);
    p.sign(
        v(35.8, 2.6, 0.0),
        Face::NegX.yaw(),
        12.0,
        Sign::VibeConMasterclass,
    );
    p.boxed(v(31.0, 0.55, -5.5), v(0.8, 1.1, 1.0), Mat::Wood, true);
    p.boxed(v(30.6, 1.15, -5.5), v(0.1, 0.1, 0.6), Mat::NeonPink, false);
    for (a, b, m) in [
        ((22.0, -16.0), (34.0, -8.0), Mat::NeonPink),
        ((22.0, 16.0), (34.0, 8.0), Mat::NeonCyan),
    ] {
        floor_neon(p, a, b, m);
    }
    for (x, z) in [(32.0, -14.0), (32.0, 14.0)] {
        plant(p, x, z);
    }

    lights(
        p,
        &[
            (-58.0, 0.0),
            (-34.0, 0.0),
            (-48.0, -14.0),
            (-20.0, 14.0),
            (-6.0, 0.0),
            (12.0, -10.0),
            (12.0, 10.0),
            (26.0, 0.0),
            (52.0, 0.0),
        ],
        Color::srgb(0.95, 0.9, 1.0),
        400_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:vibecon/1", (-67.0, 0.0), Sign::ToExpo),
                ("door:vibecon/3", (60.0, 0.0), Sign::ToHackathon),
            ],
            packs: &[(-48.0, -14.0), (-20.0, 14.0), (1.0, -14.0), (1.0, 14.0)],
            boss: (24.0, 0.0),
            npc: None,
        },
    );
    theme(2)
}

// ------------------------------------------------------------------ vibecon/3: hackathon corner

fn hackathon(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-60.0, 0.0), 6.0, 7.0, 3.0, 0.0), w);
    p.fp.link(stroke(&[pt(-55.0, 0.0), pt(-42.0, 0.0)], 4.0), w);
    p.fp.pod(blob(pt(-12.0, 0.0), 32.0, 25.0, 0.0, 3, 0.06), w);
    p.fp.link(stroke(&[pt(18.0, 0.0), pt(30.0, 0.0)], 6.0), w);
    p.fp.pod(rounded_rect(pt(40.0, 0.0), 12.0, 15.0, 5.0, 0.0), w);
    p.route = vec![
        pt(-60.0, 0.0),
        pt(-40.0, 0.0),
        pt(-12.0, 0.0),
        pt(18.0, 0.0),
        pt(32.0, 0.0),
    ];
    p.build_walls();

    runner(
        p,
        &[pt(-60.0, 0.0), pt(-30.0, 0.0), pt(0.0, 0.0), pt(32.0, 0.0)],
        2.4,
        Mat::Runner,
        Mat::NeonYellow,
    );
    // Team tables in two loose rows, the beanbag nap zone, the pizza and energy-drink wall.
    for (x, z) in [
        (-30.0, -6.5),
        (-18.0, -20.0),
        (-6.0, -6.5),
        (-30.0, 6.5),
        (-18.0, 20.0),
        (-6.0, 6.5),
        (6.0, 18.0),
    ] {
        hack_table(p, x, z);
    }
    for (x, z, m) in [
        (-26.0, 14.0, Mat::BeanA),
        (-24.0, 17.0, Mat::BeanB),
        (-28.0, 17.5, Mat::BeanC),
    ] {
        beanbag(p, x, z, m);
    }
    p.boxed(v(2.0, 0.5, -18.0), v(8.0, 1.0, 1.0), Mat::White, true);
    for k in 0..6 {
        p.boxed(
            v(-1.0 + k as f32 * 1.2, 1.1, -18.0),
            v(0.9, 0.12, 0.9),
            Mat::Cardboard,
            false,
        );
    }
    for row in 0..4 {
        for k in 0..(6 - row) {
            p.prim(
                PrimKind::Cylinder,
                v(
                    -25.0 + k as f32 * 0.22 + row as f32 * 0.11,
                    0.1 + row as f32 * 0.2,
                    -19.0,
                ),
                v(0.18, 0.2, 0.18),
                NEON[(k + row) % 4],
            );
        }
    }
    p.block(v(-24.4, 0.5, -19.0), v(1.6, 1.0, 0.4));
    p.sign_near(-12.0, -24.6, 2.9, 6.0, Sign::VibeConHackathon);
    p.sign_near(-12.0, 24.6, 2.8, 5.0, Sign::VibeConWifi);
    for (x, z) in [(-56.0, -5.0), (-56.0, 5.0), (14.0, 10.0), (28.0, -12.0)] {
        plant(p, x, z);
    }

    // Demo day: a judges' table in front of the stage, the stage laptop on its stand.
    stage(p, 47.0, 0.0, Face::NegX, 14.0, 6.0, Sign::VibeConJudging);
    p.boxed(v(33.0, 0.4, -9.0), v(1.0, 0.8, 4.0), Mat::White, true);
    for k in 0..3 {
        p.prop(
            &CHAIR_MODERN,
            v(34.2, 0.0, -10.5 + k as f32 * 1.5),
            FRAC_PI_2,
            false,
        );
    }
    p.boxed(v(42.0, 0.8, 0.0), v(0.6, 1.0, 0.6), Mat::Metal, true);
    p.boxed(v(42.0, 1.35, 0.0), v(0.5, 0.04, 0.35), Mat::Dark, false);
    p.boxed(
        v(41.8, 1.5, 0.0),
        v(0.04, 0.3, 0.35),
        Mat::ScreenGlow,
        false,
    );

    lights(
        p,
        &[
            (-60.0, 0.0),
            (-30.0, 0.0),
            (-18.0, -14.0),
            (-18.0, 14.0),
            (-4.0, 0.0),
            (10.0, 12.0),
            (24.0, 0.0),
            (40.0, 0.0),
        ],
        Color::srgb(1.0, 0.95, 0.85),
        400_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:vibecon/2", (-66.0, 0.0), Sign::ToWorkshops),
                ("door:vibecon/4", (40.0, -15.0), Sign::ToAfterparty),
            ],
            packs: &[(-30.0, -15.0), (-12.0, 13.0), (8.0, -13.0)],
            boss: (38.0, 0.0),
            npc: None,
        },
    );
    theme(3)
}

// ------------------------------------------------------------------ vibecon/4: afterparty

fn afterparty(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-60.0, 0.0), 6.0, 7.0, 3.0, 0.0), w);
    p.fp.link(stroke(&[pt(-55.0, 0.0), pt(-40.0, 0.0)], 4.0), w);
    p.fp.pod(blob(pt(-14.0, 0.0), 28.0, 23.0, 0.0, 5, 0.05), w);
    p.fp.link(stroke(&[pt(12.0, 0.0), pt(24.0, 0.0)], 5.0), w);
    p.fp.pod(rounded_rect(pt(34.0, 0.0), 12.0, 14.0, 5.0, 0.0), w);
    p.route = vec![
        pt(-60.0, 0.0),
        pt(-40.0, 0.0),
        pt(-14.0, 0.0),
        pt(14.0, 0.0),
        pt(26.0, 0.0),
    ];
    p.build_walls();

    // Dance floor: a grid of glowing tiles, the DJ booth south, the bar north.
    for i in 0..5 {
        for j in 0..4 {
            p.slab(
                v(-24.0 + i as f32 * 2.2, 0.02, -3.3 + j as f32 * 2.2),
                0.0,
                v(2.0, 0.03, 2.0),
                NEON[(i + j) % 4],
            );
        }
    }
    p.boxed(v(-18.0, 0.6, 19.0), v(6.0, 1.2, 1.4), Mat::Dark, true);
    p.boxed(
        v(-18.0, 1.22, 19.0),
        v(6.1, 0.05, 1.5),
        Mat::NeonPink,
        false,
    );
    for sx in [-1.6, 1.6] {
        p.prim(
            PrimKind::Cylinder,
            v(-18.0 + sx, 1.3, 19.0),
            v(0.8, 0.06, 0.8),
            Mat::Dark,
        );
    }
    for sx in [-22.5, -13.5] {
        p.boxed(v(sx, 1.0, 20.5), v(1.2, 2.0, 1.0), Mat::Dark, true);
    }
    p.sign_near(-18.0, 22.6, 2.9, 7.0, Sign::VibeConAfterparty);
    p.boxed(v(-10.0, 0.55, -18.6), v(12.0, 1.1, 1.0), Mat::Wood, true);
    p.boxed(
        v(-10.0, 1.12, -18.6),
        v(12.2, 0.06, 1.2),
        Mat::NeonCyan,
        false,
    );
    for k in 0..14 {
        p.boxed(
            v(-15.5 + k as f32 * 0.85, 1.55, -21.0),
            v(0.18, 0.5, 0.18),
            [Mat::Scr0, Mat::Scr1, Mat::Scr2, Mat::Yellow][k % 4],
            false,
        );
    }
    for k in 0..6 {
        p.prop(&STOOL_BAR, v(-15.0 + k as f32 * 2.0, 0.0, -17.0), PI, false);
    }
    p.sign_near(-10.0, -22.6, 3.0, 4.0, Sign::ToAfterparty);
    // High tables with abandoned name badges.
    for (x, z) in [(-36.0, -10.0), (-36.0, 10.0), (0.0, -17.0), (4.0, 16.0)] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.5, z),
            v(0.12, 1.0, 0.12),
            Mat::Metal,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x, 1.03, z),
            v(0.95, 0.05, 0.95),
            Mat::White,
        );
        p.boxed(v(x + 0.2, 1.07, z), v(0.15, 0.02, 0.1), Mat::Paper, false);
        p.block(v(x, 0.6, z), v(0.9, 1.2, 0.9));
    }
    lounge(p, -46.0, 0.0, FRAC_PI_2, true);

    // Fireside chat: three armchairs round a fireplace screen, the panel's name cards.
    p.boxed(
        v(34.0, 0.12, 0.0),
        v(10.0, 0.24, 18.0),
        Mat::MarbleDark,
        false,
    );
    for (z, m) in [
        (0.0, Mat::NeonCyan),
        (-5.0, Mat::NeonYellow),
        (5.0, Mat::NeonPink),
    ] {
        armchair(p, 36.5, z, -FRAC_PI_2, m);
    }
    p.boxed(v(44.5, 1.6, 0.0), v(0.3, 3.2, 8.0), Mat::Dark, false);
    p.boxed(v(44.3, 0.9, 0.0), v(0.05, 1.0, 3.0), Mat::Hot, false);
    p.sign(
        v(44.3, 2.6, 0.0),
        Face::NegX.yaw(),
        7.0,
        Sign::VibeConFireside,
    );
    for (x, z) in [(26.0, -12.0), (26.0, 12.0), (44.0, -12.0), (44.0, 12.0)] {
        plant(p, x, z);
    }

    for (x, z, c) in [
        (-60.0, 0.0, Color::srgb(1.0, 0.6, 1.0)),
        (-34.0, 0.0, Color::srgb(1.0, 0.5, 0.9)),
        (-18.0, 12.0, Color::srgb(0.5, 0.9, 1.0)),
        (-10.0, -12.0, Color::srgb(0.5, 1.0, 0.8)),
        (0.0, 0.0, Color::srgb(1.0, 0.6, 1.0)),
        (34.0, 0.0, Color::srgb(1.0, 0.7, 0.45)),
    ] {
        p.light(v(x, 3.4, z), c, 380_000.0);
    }
    ceiling_lights(p);
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:vibecon/3", (-66.0, 0.0), Sign::ToHackathon),
                ("door:vibecon/5", (34.0, -14.0), Sign::ToMainStage),
            ],
            packs: &[(-32.0, -12.0), (-6.0, 12.0), (6.0, -9.0)],
            boss: (30.0, 0.0),
            npc: None,
        },
    );
    theme(4)
}

// ------------------------------------------------------------------ vibecon/5: main stage

fn main_stage(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-60.0, 0.0), 6.0, 7.0, 3.0, 0.0), w);
    p.fp.link(stroke(&[pt(-55.0, 0.0), pt(-46.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(-38.0, 0.0), 9.0, 12.5, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-30.0, 0.0), pt(-24.0, 0.0)], 6.0), w);
    p.fp.pod(rounded_rect(pt(8.0, 0.0), 32.0, 26.0, 9.0, 0.0), w);
    p.route = vec![
        pt(-60.0, 0.0),
        pt(-38.0, 0.0),
        pt(-14.0, 0.0),
        pt(10.0, 0.0),
        pt(22.0, 0.0),
    ];
    p.build_walls();

    runner(
        p,
        &[
            pt(-62.0, 0.0),
            pt(-38.0, 0.0),
            pt(-14.0, 0.0),
            pt(24.0, 0.0),
        ],
        3.0,
        Mat::Runner,
        Mat::NeonCyan,
    );
    // Foyer: the merch stand and the press wall.
    swag_table(p, -38.0, -9.0, true);
    p.sign_near(-38.0, -10.8, 2.6, 4.0, Sign::VibeConSwag);
    p.sign_near(-38.0, 10.8, 2.8, 5.0, Sign::VibeConBanner);
    lanyard_rack(p, -32.0, 9.4);

    // Audience: four seat blocks either side of the centre aisle, facing the stage.
    for (bx, bz) in [(-14.0, -12.0), (-14.0, 12.0), (2.0, -12.0), (2.0, 12.0)] {
        for row in 0..4 {
            let x = bx - 4.5 + row as f32 * 3.0;
            p.boxed(v(x, 0.25, bz), v(0.9, 0.5, 7.0), Mat::Fabric1, true);
            p.boxed(v(x - 0.4, 0.75, bz), v(0.12, 0.6, 7.0), Mat::Dark, false);
        }
    }
    for (a, b, m) in [
        ((-24.0, 2.6), (24.0, 2.6), Mat::NeonCyan),
        ((-24.0, -2.6), (24.0, -2.6), Mat::NeonCyan),
    ] {
        floor_neon(p, a, b, m);
    }
    // The stage: wide riser, a runway into the crowd, the giant LED wall.
    stage(p, 35.0, 0.0, Face::NegX, 24.0, 7.0, Sign::VibeConKeynote);
    p.boxed(v(27.0, 0.15, 0.0), v(8.0, 0.3, 3.0), Mat::MarbleDark, false);
    for z in [-12.5, 12.5] {
        p.boxed(v(36.0, 2.5, z), v(1.4, 5.0, 2.0), Mat::Dark, false);
        p.boxed(v(35.2, 2.5, z), v(0.05, 4.0, 1.6), Mat::ScreenGlow, false);
    }
    for (x, z) in [(-26.0, -20.0), (-26.0, 20.0)] {
        plant(p, x, z);
    }
    p.sign_near(8.0, 25.8, 3.0, 6.0, Sign::VibeConWifi);

    lights(
        p,
        &[
            (-60.0, 0.0),
            (-38.0, 0.0),
            (-14.0, -12.0),
            (-14.0, 12.0),
            (4.0, 0.0),
            (20.0, -12.0),
            (20.0, 12.0),
            (30.0, 0.0),
        ],
        Color::srgb(0.75, 0.9, 1.0),
        420_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:vibecon/4", (-66.0, 0.0), Sign::ToAfterparty),
                ("door:campus", (14.0, -26.0), Sign::ToCampus),
            ],
            packs: &[(-38.0, 8.5), (-6.0, -21.0), (-6.0, 21.0)],
            boss: (26.0, 0.0),
            npc: None,
        },
    );
    theme(5)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every room carries its signature dressing, readable from the walking side.
    #[test]
    fn each_room_has_its_signs() {
        let want: [&[Sign]; 5] = [
            &[
                Sign::VibeConRegistration,
                Sign::VibeConLanyards,
                Sign::VibeConBooth,
                Sign::VibeConShipLive,
                Sign::VibeConBanner,
            ],
            &[Sign::VibeConMasterclass, Sign::VibeConWorkshop],
            &[Sign::VibeConHackathon, Sign::VibeConJudging],
            &[Sign::VibeConAfterparty, Sign::VibeConFireside],
            &[Sign::VibeConKeynote],
        ];
        for (k, signs) in want.iter().enumerate() {
            let l = build(k as u8 + 1);
            for s in *signs {
                assert!(
                    l.pieces
                        .iter()
                        .any(|p| matches!(p, Piece::Panel { tex: Tex::Sign(x), .. } if x == s)),
                    "{}: lacks {s:?}",
                    l.zone
                );
            }
        }
    }
}
