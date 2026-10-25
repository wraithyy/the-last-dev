//! VibeCon, the second raid: a tech conference in five rooms. vibecon/1 is registration and the
//! expo hall with booth rows and swag tables, vibecon/2 the workshop corridor with the prompt
//! masterclass, vibecon/3 the hackathon corner with the demo-day stage, vibecon/4 the afterparty
//! lounge with the fireside-chat panel, vibecon/5 the main keynote stage. Boxes and primitives in
//! the offsite's budget; the look is black carpet, neon lanyard colours and screens.

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::offsite::{
    balloons, beams, bunting, chair_to, cove, floor_patch, plant_pair, round_patch, wall_dress,
    wall_prop,
};
use super::props::{beanbag, lounge, mini_fridge, vending};
use super::shape::{blob, rounded_rect, stroke};
use super::themes::{ceiling_lights, pt, runner, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The four lanyard colours (exhibitor, volunteer, speaker, hacker), reused everywhere.
const NEON: [Mat; 4] = [Mat::NeonYellow, Mat::LedGreen, Mat::NeonCyan, Mat::NeonPink];

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    if matches!(level, 1 | 5) {
        p.ceiling = HALL_H;
    }
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
        wall: c(0.46, 0.42, 0.6),
        trim: c(0.08, 0.08, 0.12),
        accent: LinearRgba::rgb(0.4, 2.6, 3.2),
        fabric: [c(0.95, 0.3, 0.75), c(0.2, 0.8, 0.95), c(0.98, 0.85, 0.2)],
        floor: FloorKind::Carpet([0.78, 0.72, 0.98]),
        fog: c(0.12, 0.1, 0.2),
        ambient: c(0.95, 0.93, 1.0),
        ambient_brightness: 1150.0,
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
            wall: c(0.24, 0.12, 0.3),
            trim: c(0.04, 0.02, 0.06),
            accent: LinearRgba::rgb(3.2, 0.6, 2.6),
            floor: FloorKind::Gloss([0.34, 0.24, 0.44]),
            fog: c(0.06, 0.02, 0.08),
            ambient: c(1.0, 0.75, 1.0),
            ambient_brightness: 480.0,
            sun_illuminance: 3500.0,
            ..base
        },
        _ => Theme {
            wall: c(0.2, 0.22, 0.32),
            trim: c(0.02, 0.02, 0.04),
            accent: LinearRgba::rgb(0.6, 2.8, 3.4),
            floor: FloorKind::Carpet([0.34, 0.34, 0.5]),
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
        // Booth carpet, side wings, a demo stool, a lighting ring and a plant for authenticity.
        floor_patch(p, 0.0, -0.1, 0.0, 5.2, 4.2, Mat::MarbleDark);
        for sx in [-2.35, 2.35] {
            p.boxed(v(sx, 1.0, -0.9), v(0.1, 2.0, 1.5), Mat::Dark, false);
            p.boxed(v(sx, 2.05, -0.9), v(0.12, 0.06, 1.5), neon, false);
        }
        p.prop(&STOOL_BAR, v(-1.4, 0.0, 1.2), 0.0, false);
        p.prop(&POTTED_PLANT, v(-2.9, 0.0, -1.2), 0.0, false);
        p.boxed(v(0.0, 3.5, -0.3), v(4.6, 0.08, 0.08), Mat::Metal, false);
        for sx in [-1.6, 0.0, 1.6] {
            p.boxed(v(sx, 3.4, -0.3), v(0.4, 0.2, 0.4), Mat::LightPanel, false);
        }
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
                v(sx, 4.0, d / 2.0 - 0.5),
                v(0.4, 0.25, 0.4),
                Mat::LightPanel,
                false,
            );
        }
        // Truss bar, speaker stacks, front steps and footlights.
        p.boxed(
            v(0.0, 4.2, d / 2.0 - 0.5),
            v(w, 0.16, 0.16),
            Mat::Metal,
            false,
        );
        for s in [-1.0, 1.0] {
            p.boxed(
                v(s * (w / 2.0 + 0.7), 1.3, 0.0),
                v(1.0, 2.6, 1.0),
                Mat::Dark,
                false,
            );
            p.boxed(
                v(s * (w / 2.0 + 0.7), 1.0, 0.52),
                v(0.8, 0.9, 0.04),
                Mat::Metal,
                false,
            );
        }
        p.boxed(
            v(0.0, 0.07, d / 2.0 + 0.5),
            v(3.0, 0.14, 1.0),
            Mat::MarbleDark,
            false,
        );
        for k in 0..((w / 1.5) as usize) {
            p.prim(
                PrimKind::Sphere,
                v(-w / 2.0 + 0.75 + k as f32 * 1.5, 0.34, d / 2.0 - 0.1),
                v(0.14, 0.1, 0.14),
                NEON[k % 4],
            );
        }
    });
}

/// Hackathon table: laptops with glowing lids, pizza boxes and an energy-drink pyramid.
fn hack_table(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.38, z), v(5.0, 0.76, 1.2), Mat::Wood, true);
    for (sx, sz) in [(-1.2, 1.3), (1.2, -1.3), (-1.2, -1.3), (1.2, 1.3)] {
        chair_to(p, x + sx, z + sz, x + sx, z);
    }
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

/// Row of Kenney chairs along z centred on (x, z), all facing +x (the stage or screen).
fn seat_row(p: &mut Plan, x: f32, z: f32, len: f32) {
    let n = (len / 0.85) as usize;
    for k in 0..n {
        let cz = z + (k as f32 + 0.5 - n as f32 / 2.0) * (len / n as f32);
        p.prop(&CHAIR_MODERN, v(x, 0.0, cz), FRAC_PI_2, false);
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

// ------------------------------------------------------------------ room dressing

/// Neon vertical LED column against the nearest wall: the hall's ribs.
fn led_column(p: &mut Plan, x: f32, z: f32, neon: Mat) {
    let Some((at, n)) = p.wall_near(x, z) else {
        return;
    };
    let yaw = n.x.atan2(n.y);
    let c = at + n * 0.12;
    p.framed(v(c.x, 0.0, c.y), yaw, |p| {
        p.boxed(v(0.0, 2.2, 0.0), v(0.5, 4.4, 0.2), Mat::Dark, false);
        p.boxed(v(0.0, 2.2, 0.11), v(0.12, 4.0, 0.02), neon, false);
    });
}

/// The conference mascot: a chrome robot with a chatbot face on its chest.
fn mascot(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prim(
            PrimKind::Cylinder,
            v(0.0, 0.1, 0.0),
            v(1.6, 0.2, 1.6),
            Mat::MarbleDark,
        );
        p.prim(
            PrimKind::Cylinder,
            v(0.0, 1.0, 0.0),
            v(1.1, 1.6, 0.9),
            Mat::Chrome,
        );
        p.prim(
            PrimKind::Sphere,
            v(0.0, 2.1, 0.0),
            v(0.9, 0.8, 0.8),
            Mat::Chrome,
        );
        for sx in [-0.18, 0.18] {
            p.boxed(v(sx, 2.15, 0.36), v(0.14, 0.14, 0.04), Mat::NeonCyan, false);
        }
        p.boxed(v(0.0, 2.7, 0.0), v(0.04, 0.5, 0.04), Mat::Metal, false);
        p.prim(
            PrimKind::Sphere,
            v(0.0, 3.0, 0.0),
            v(0.2, 0.2, 0.2),
            Mat::NeonPink,
        );
        for sx in [-0.75, 0.75] {
            p.prim(
                PrimKind::Cylinder,
                v(sx, 1.1, 0.0),
                v(0.28, 1.1, 0.28),
                Mat::Chrome,
            );
        }
        p.panel(
            v(0.0, 1.2, 0.46),
            0.0,
            Vec2::new(0.8, 0.8),
            Tex::ChatbotFace,
        );
    });
    p.block(v(x, 1.0, z), v(1.7, 2.0, 1.7));
}

/// Neon-lit wall base shared by the VibeCon halls.
fn hall_walls(p: &mut Plan, rail: Mat, cove_mat: Mat) {
    wall_dress(p, Mat::Dark, rail, Mat::Trim);
    cove(p, cove_mat);
}

fn expo_dress(p: &mut Plan) {
    hall_walls(p, Mat::NeonPink, Mat::NeonCyan);
    beams(p, -18.0, 34.0, -24.0, 24.0, 8.0, Mat::Metal);
    floor_patch(p, 4.0, 0.0, 0.0, 46.0, 5.6, Mat::MarbleDark);
    for (k, x) in [-4.0, 12.0, 28.0].into_iter().enumerate() {
        led_column(p, x, -23.9, NEON[k]);
        led_column(p, x - 4.0, 23.9, NEON[k + 1]);
    }
    mascot(p, 14.0, -8.5, FRAC_PI_2 * 0.5);
    mascot(p, 14.0, 8.5, PI - FRAC_PI_2 * 0.5);
    balloons(p, 30.5, -9.0, [Mat::Fabric0, Mat::Fabric1, Mat::Fabric2]);
    balloons(p, 30.5, 9.0, [Mat::Fabric2, Mat::Fabric0, Mat::Fabric1]);
    bunting(
        p,
        (-16.0, -11.0),
        (30.0, -11.0),
        p.top() - 0.6,
        [Mat::NeonPink, Mat::NeonCyan, Mat::NeonYellow],
    );
    bunting(
        p,
        (-16.0, 11.0),
        (30.0, 11.0),
        p.top() - 0.6,
        [Mat::NeonYellow, Mat::NeonPink, Mat::NeonCyan],
    );
    for (x, z, tex, w, h) in [
        (-20.0, -12.0, Tex::PromptTips, 1.6, 1.3),
        (-20.0, 12.0, Tex::Suggestions, 1.4, 1.5),
        (37.0, -6.0, Tex::Brand, 1.6, 1.2),
        (37.0, 8.0, Tex::ChatbotFace, 1.4, 1.4),
    ] {
        p.poster_near(x, z, 2.3, Vec2::new(w, h), tex);
    }
    // Registration corner: carpet under the queue, plants at the door.
    floor_patch(p, -34.0, -5.5, 0.0, 12.0, 6.0, Mat::MarbleDark);
    plant_pair(p, -43.0, 10.5);
    plant_pair(p, -25.0, 10.5);
    for (x, z) in [(-30.0, 9.0), (-38.0, 9.0)] {
        beanbag(p, x, z, Mat::BeanA);
    }
}

fn workshops_dress(p: &mut Plan) {
    wall_dress(p, Mat::Trim, Mat::NeonPink, Mat::Trim);
    cove(p, Mat::NeonPink);
    beams(p, -4.0, 40.0, -22.0, 22.0, 8.0, Mat::Metal);
    for (x, z) in [
        (-48.0, 14.0),
        (-48.0, -14.0),
        (-34.0, 14.0),
        (-34.0, -14.0),
        (-20.0, 14.0),
        (-20.0, -14.0),
    ] {
        floor_patch(p, x, z, 0.0, 5.6, 4.4, Mat::MarbleDark);
        wall_prop(p, &TV, x, z + z.signum() * 5.4, false);
    }
    for z in [-3.0, 3.0] {
        wall_prop(p, &BOOKCASE_OPEN, -56.0, z * 3.0, true);
    }
    p.prop(
        &COFFEE_MACHINE,
        v(-40.0, SIDE_TABLE.size[1] * FURNITURE_SCALE, -2.2),
        0.0,
        false,
    );
    p.prop(&SIDE_TABLE, v(-40.0, 0.0, -2.2), 0.0, false);
    // Masterclass: lectern light pool, LED columns beside the screen, plants.
    round_patch(p, 30.0, -5.5, 3.0, Mat::NeonPink);
    for (k, z) in [-10.0, 10.0].into_iter().enumerate() {
        p.boxed(v(35.6, 2.2, z), v(0.3, 4.2, 0.5), Mat::Dark, false);
        p.boxed(v(35.4, 2.2, z), v(0.04, 4.0, 0.1), NEON[k * 2], false);
    }
    plant_pair(p, 4.0, -20.0);
    plant_pair(p, 4.0, 20.0);
    p.poster_near(14.0, -21.8, 2.2, Vec2::new(2.6, 1.6), Tex::PromptTips);
    p.poster_near(14.0, 21.8, 2.2, Vec2::new(1.6, 1.3), Tex::ChatbotFace);
}

fn hackathon_dress(p: &mut Plan) {
    hall_walls(p, Mat::NeonYellow, Mat::NeonYellow);
    beams(p, -26.0, 4.0, -24.0, 24.0, 8.0, Mat::Metal);
    // Extension cords snaking between the tables, because there are never enough sockets.
    for (a, b) in [
        ((-30.0, -6.5), (-18.0, -13.0)),
        ((-6.0, -6.5), (-18.0, -13.0)),
        ((-30.0, 6.5), (-18.0, 13.0)),
        ((-6.0, 6.5), (6.0, 13.0)),
    ] {
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        p.slab(
            v((a.0 + b.0) / 2.0, 0.02, (a.1 + b.1) / 2.0),
            (-dz).atan2(dx),
            v((dx * dx + dz * dz).sqrt(), 0.03, 0.06),
            Mat::Cable,
        );
    }
    // Snack and charging station on the south wall.
    vending(p, -4.0, 23.4, Face::NegZ);
    vending(p, -2.4, 23.4, Face::NegZ);
    mini_fridge(p, -0.6, 23.5, Face::NegZ);
    for (x, z, tex) in [
        (-30.0, 24.5, Tex::Leaderboard),
        (-22.0, -24.5, Tex::WarRoom),
        (6.0, -24.5, Tex::SocAlert),
        (-42.0, -10.0, Tex::DemoScreen),
    ] {
        p.poster_near(x, z, 2.4, Vec2::new(2.0, 1.4), tex);
    }
    // Sleeping bags by the beanbags.
    for (x, z, yaw) in [(-22.0, 14.5, 0.3), (-30.0, 15.0, -0.2)] {
        p.slab(v(x, 0.08, z), yaw, v(0.9, 0.16, 2.0), Mat::Fabric1);
    }
    // Demo day: carpet under the judges, spotlights on the stage.
    floor_patch(p, 36.0, -8.0, 0.0, 6.0, 5.0, Mat::MarbleDark);
    for z in [-3.0, 3.0] {
        p.prim(
            PrimKind::Cylinder,
            v(41.0, 0.012, z),
            v(1.8, 0.01, 1.8),
            Mat::NeonYellow,
        );
    }
    plant_pair(p, 34.0, 12.0);
}

fn afterparty_dress(p: &mut Plan) {
    hall_walls(p, Mat::NeonPink, Mat::NeonCyan);
    beams(p, -34.0, 6.0, -20.0, 20.0, 8.0, Mat::Metal);
    // Mirror ball and coloured lamps over the dance floor, speaker stacks round it.
    p.boxed(v(-18.9, 3.9, 0.7), v(0.04, 0.5, 0.04), Mat::Metal, false);
    p.prim(
        PrimKind::Sphere,
        v(-18.9, 3.45, 0.7),
        v(0.9, 0.9, 0.9),
        Mat::Chrome,
    );
    for (k, (x, z)) in [(-24.0, -3.0), (-14.0, -3.0), (-24.0, 4.5), (-14.0, 4.5)]
        .into_iter()
        .enumerate()
    {
        p.prim(
            PrimKind::Cylinder,
            v(x, 3.9, z),
            v(0.35, 0.3, 0.35),
            NEON[k],
        );
    }
    for (x, z) in [(-27.0, -4.5), (-27.0, 5.5), (-11.0, -4.5), (-11.0, 5.5)] {
        p.boxed(v(x, 1.1, z), v(0.9, 2.2, 0.9), Mat::Dark, true);
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.8, z + 0.46),
            v(0.5, 0.04, 0.5),
            Mat::Metal,
        );
    }
    // Stools at the high tables, more couches for the quiet ones.
    for (x, z) in [(-36.0, -10.0), (-36.0, 10.0), (0.0, -17.0), (4.0, 16.0)] {
        for k in 0..3 {
            let a = k as f32 * TAU / 3.0 + 0.5;
            p.prop(
                &STOOL_BAR,
                v(x + 1.0 * a.cos(), 0.0, z + 1.0 * a.sin()),
                -a,
                false,
            );
        }
    }
    lounge(p, -46.0, -12.0, FRAC_PI_2, false);
    lounge(p, -46.0, 12.0, FRAC_PI_2, true);
    balloons(
        p,
        -28.0,
        16.0,
        [Mat::NeonPink, Mat::NeonCyan, Mat::NeonYellow],
    );
    balloons(
        p,
        -8.0,
        -16.0,
        [Mat::NeonCyan, Mat::NeonYellow, Mat::NeonPink],
    );
    for k in 0..30 {
        let (a, r) = (k as f32 * 2.399, 1.0 + (k as f32 * 1.3).sin().abs() * 9.0);
        p.slab(
            v(-19.0 + r * a.cos(), 0.03, 0.7 + r * 0.8 * a.sin()),
            a,
            v(0.16, 0.01, 0.1),
            NEON[k % 4],
        );
    }
    // Fireside: rug, bookcases, side tables, a mantel of awards.
    p.prop(&RUG, v(36.0, 0.25, 0.0), FRAC_PI_2, false);
    wall_prop(p, &BOOKCASE_OPEN, 44.0, -7.0, true);
    wall_prop(p, &BOOKCASE_CLOSED, 44.0, 7.0, true);
    for z in [-2.5, 2.5] {
        p.prop(&SIDE_TABLE, v(37.5, 0.24, z * 1.9), FRAC_PI_2, false);
    }
    p.boxed(v(44.0, 1.5, 0.0), v(0.5, 0.1, 5.0), Mat::Wood, false);
    for k in 0..4 {
        p.prim(
            PrimKind::Cylinder,
            v(43.9, 1.7, -1.5 + k as f32),
            v(0.2, 0.3, 0.2),
            Mat::Gold,
        );
    }
    plant_pair(p, 30.0, 8.0);
}

fn main_dress(p: &mut Plan) {
    hall_walls(p, Mat::NeonCyan, Mat::NeonPink);
    beams(p, -20.0, 22.0, -26.0, 26.0, 8.0, Mat::Metal);
    for (k, x) in [-18.0, -6.0, 6.0, 18.0].into_iter().enumerate() {
        led_column(p, x, -25.9, NEON[k]);
        led_column(p, x, 25.9, NEON[(k + 2) % 4]);
    }
    // Moving heads on a truss over the crowd, light pools on the floor beneath.
    for (k, x) in [-2.0, 10.0, 22.0].into_iter().enumerate() {
        p.boxed(v(x, 4.05, 0.0), v(0.2, 0.2, 22.0), Mat::Metal, false);
        for (j, z) in [-8.0, -4.0, 4.0, 8.0].into_iter().enumerate() {
            p.prim(
                PrimKind::Cylinder,
                v(x, 3.8, z),
                v(0.3, 0.3, 0.3),
                NEON[(k + j) % 4],
            );
            round_patch(p, x + 1.0, z * 1.4, 2.4, NEON[(k + j) % 4]);
        }
    }
    // Front-of-house desk: three screens and a very serious mixer.
    p.boxed(v(14.0, 0.5, -9.0), v(3.0, 1.0, 1.2), Mat::Dark, true);
    for sx in [-1.0, 0.0, 1.0] {
        p.boxed(
            v(14.0 + sx, 1.35, -9.3),
            v(0.8, 0.5, 0.05),
            Mat::ScreenGlow,
            false,
        );
    }
    p.prop(&LAPTOP, v(13.0, 1.0, -8.8), PI, false);
    chair_to(p, 14.0, -7.4, 14.0, -9.0);
    plant_pair(p, -43.0, 5.5);
    p.poster_near(-38.0, -6.0, 2.4, Vec2::new(1.6, 1.3), Tex::Brand);
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
    expo_dress(p);

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
        Color::srgb(0.9, 0.88, 1.0),
        560_000.0,
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
    workshops_dress(p);

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
            // The old bench is now an invisible blocker so the walkable space stays put.
            p.block(v(x, 0.25, 11.0 * s), v(1.0, 0.5, 9.0));
            seat_row(p, x, 11.0 * s, 9.0);
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
    hackathon_dress(p);

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
    afterparty_dress(p);

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
    main_dress(p);

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
            p.block(v(x, 0.25, bz), v(0.9, 0.5, 7.0));
            seat_row(p, x, bz, 7.0);
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
