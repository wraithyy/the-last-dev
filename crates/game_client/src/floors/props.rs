//! Reusable dressing and satirical gags. All pure: they only append pieces.

use super::kit::*;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Open towards +Z (local): back and side partitions, desk with a computer, chair.
pub fn cubicle(p: &mut Plan, i: usize, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        const SIZE: f32 = 3.0;
        const PART_H: f32 = 1.6;
        const PART_T: f32 = 0.1;
        let half = SIZE / 2.0;
        let fabric = [Mat::Fabric0, Mat::Fabric1, Mat::Fabric2][i % 3];
        let trim_h = 0.06;
        p.boxed(
            v(0.0, PART_H / 2.0, -half),
            v(SIZE, PART_H, PART_T),
            fabric,
            true,
        );
        p.boxed(
            v(0.0, PART_H + trim_h / 2.0, -half),
            v(SIZE, trim_h, PART_T + 0.04),
            Mat::Trim,
            false,
        );
        for side in [-1.0, 1.0] {
            p.boxed(
                v(side * half, PART_H / 2.0, 0.0),
                v(PART_T, PART_H, SIZE),
                fabric,
                true,
            );
            p.boxed(
                v(side * half, PART_H + trim_h / 2.0, 0.0),
                v(PART_T + 0.04, trim_h, SIZE),
                Mat::Trim,
                false,
            );
        }
        let top = DESK.size[1] * FURNITURE_SCALE;
        let desk_d = DESK.size[2] * FURNITURE_SCALE;
        let desk_z = -half + PART_T / 2.0 + desk_d / 2.0;
        p.prop(&DESK, v(0.0, 0.0, desk_z), 0.0, true);
        let back = desk_z - desk_d / 2.0;
        if i.is_multiple_of(2) {
            p.prop(&SCREEN, v(0.0, top, back + 0.2), 0.0, false);
            p.prop(&KEYBOARD, v(0.0, top, back + 0.62), 0.0, false);
            p.prop(&MOUSE, v(0.55, top, back + 0.62), 0.0, false);
        } else {
            p.prop(&LAPTOP, v(0.0, top, back + 0.45), 0.0, false);
            p.prop(&MOUSE, v(0.55, top, back + 0.5), 0.0, false);
            p.prop(&BOOKS, v(0.72, top, back + 0.2), 0.0, false);
        }
        p.prop(&PLANTS_SMALL[i % 3], v(-0.72, top, back + 0.25), 0.0, false);
        if i.is_multiple_of(3) {
            p.prop(&LAMP_TABLE, v(0.72, top, back + 0.25), 0.0, false);
        }
        let chair_yaw = PI + [0.35, -0.2, 0.1][i % 3];
        let chair_pos = v(0.1, 0.0, -0.1);
        p.prop(&CHAIR_DESK, chair_pos, chair_yaw, false);
        // The collider floats 0.12 m above the floor: combat LoS rays run at
        // ground level and the same list feeds them, so chairs must not block
        // sight, yet still stop a body-height walk.
        p.block(chair_pos + Vec3::Y * (0.12 + 0.4), v(0.6, 0.8, 0.6));
    });
}

pub fn server_rack(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 1.1, 0.0), v(0.9, 2.2, 1.0), Mat::Rack, true);
        let front = 0.5;
        for i in 0..8 {
            let y = 0.25 + i as f32 * 0.25;
            p.boxed(
                v(0.0, y, front + 0.005),
                v(0.78, 0.2, 0.01),
                Mat::RackPanel,
                false,
            );
            for k in 0..3 {
                let led = match (i + k + x.abs() as usize) % 4 {
                    0 => Mat::LedRed,
                    1 | 2 => Mat::LedGreen,
                    _ => Mat::LedBlue,
                };
                p.boxed(
                    v(0.22 + k as f32 * 0.1, y, front + 0.015),
                    v(0.04, 0.04, 0.01),
                    led,
                    false,
                );
            }
        }
    });
}

/// Sofa on a rug with a coffee table, an armchair and a lamp. Open towards +Z.
pub fn lounge(p: &mut Plan, x: f32, z: f32, yaw: f32, plant: bool) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prop(&RUG, v(0.0, 0.0, 0.0), 0.0, false);
        p.prop(&LOUNGE_SOFA, v(0.0, 0.0, -2.5), 0.0, true);
        p.prop(&COFFEE_TABLE, v(0.0, 0.0, 0.0), 0.0, true);
        p.prop(&LOUNGE_CHAIR, v(2.7, 0.0, 0.0), -FRAC_PI_2, true);
        p.prop(&LAMP_FLOOR, v(2.7, 0.0, -2.5), 0.0, true);
        if plant {
            p.prop(&POTTED_PLANT, v(-2.7, 0.0, -2.5), 0.0, true);
        } else {
            p.prop(&COAT_RACK, v(-2.7, 0.0, -2.5), 0.0, true);
        }
    });
}

/// Wall-mounted whiteboard with a procedural drawing.
pub fn whiteboard(p: &mut Plan, x: f32, z: f32, face: Face, tex: Tex) {
    p.poster(x, z, 2.2, face, Vec2::new(3.0, 1.5), tex);
}

pub fn water_cooler(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.45, z), v(0.4, 0.9, 0.4), Mat::Board, true);
    p.boxed(v(x, 1.12, z), v(0.3, 0.45, 0.3), Mat::Water, false);
}

/// Vending machine facing `face`.
pub fn vending(p: &mut Plan, x: f32, z: f32, face: Face) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 0.95, 0.0), v(1.0, 1.9, 0.8), Mat::Red, true);
        p.panel(v(0.0, 1.1, 0.41), 0.0, Vec2::new(0.8, 1.2), Tex::Vending);
    });
}

/// Potted plant that has given up, with fallen leaves.
pub fn sad_plant(p: &mut Plan, x: f32, z: f32) {
    p.prop(&POTTED_PLANT, v(x, 0.0, z), 0.0, true);
    for (dx, dz) in [(0.3, 0.2), (-0.35, 0.1), (0.1, -0.35), (-0.2, 0.4)] {
        p.boxed(
            v(x + dx, 0.01, z + dz),
            v(0.16, 0.02, 0.1),
            Mat::Dead,
            false,
        );
    }
    p.boxed(v(x, 1.15, z + 0.05), v(0.5, 0.06, 0.16), Mat::Dead, false);
}

/// Ping-pong table nobody uses: dusty, with a cobweb on the net.
pub fn ping_pong(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.76, z), v(2.7, 0.08, 1.5), Mat::Scr1, false);
    p.boxed(v(x, 0.4, z), v(2.5, 0.8, 1.3), Mat::Dark, true);
    p.boxed(v(x, 0.9, z), v(0.03, 0.2, 1.6), Mat::White, false);
    p.boxed(v(x, 0.77, z), v(0.03, 0.01, 1.5), Mat::White, false);
    p.panel(
        v(x - 0.1, 1.15, z + 0.2),
        0.0,
        Vec2::new(0.8, 0.8),
        Tex::Cobweb,
    );
    p.panel(
        v(x + 0.1, 1.15, z + 0.2),
        PI,
        Vec2::new(0.8, 0.8),
        Tex::Cobweb,
    );
}

pub fn foosball(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.85, z), v(2.4, 0.1, 1.2), Mat::Felt, false);
    p.boxed(v(x, 0.5, z), v(2.2, 1.0, 1.0), Mat::Wood, true);
    for k in [-0.8, -0.27, 0.27, 0.8] {
        p.boxed(v(x + k, 1.0, z), v(0.04, 0.04, 1.6), Mat::Metal, false);
        for dz in [-0.3, 0.3] {
            p.boxed(v(x + k, 0.95, z + dz), v(0.1, 0.18, 0.08), Mat::Red, false);
        }
    }
}

pub fn beanbag(p: &mut Plan, x: f32, z: f32, mat: Mat) {
    p.boxed(v(x, 0.28, z), v(1.1, 0.56, 1.0), mat, true);
    p.boxed(v(x, 0.6, z - 0.05), v(0.8, 0.14, 0.7), mat, false);
}

/// Dusty bookshelf row with a "Documentation" sign and cobwebs. Open towards +Z.
pub fn doc_shelf(p: &mut Plan, x: f32, z: f32, face: Face) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        for k in 0..3 {
            p.prop(
                &BOOKCASE_CLOSED_WIDE,
                v(-1.7 + k as f32 * 1.9 + 0.95, 0.0, 0.0),
                0.0,
                true,
            );
        }
        p.panel(
            v(0.0, 3.05, 0.2),
            0.0,
            Vec2::new(2.2, 0.35),
            Tex::Documentation,
        );
        p.panel(v(-2.5, 2.1, 0.3), 0.0, Vec2::new(0.9, 0.9), Tex::Cobweb);
        p.panel(v(2.5, 2.1, 0.3), 0.0, Vec2::new(0.9, 0.9), Tex::Cobweb);
    });
}

/// Printer with an out-of-order notice. Faces `face`.
pub fn printer(p: &mut Plan, x: f32, z: f32, face: Face) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 0.5, 0.0), v(0.9, 1.0, 0.7), Mat::Metal, true);
        p.boxed(v(0.0, 1.05, 0.0), v(0.8, 0.1, 0.6), Mat::Dark, false);
        p.boxed(v(0.0, 0.7, 0.36), v(0.6, 0.04, 0.2), Mat::Paper, false);
        p.boxed(v(0.25, 0.9, 0.36), v(0.08, 0.03, 0.02), Mat::LedRed, false);
        p.panel(
            v(0.0, 0.6, 0.37),
            0.0,
            Vec2::new(0.5, 0.33),
            Tex::OutOfOrder,
        );
    });
}

/// Altar of GPUs ringed with candles. Faces +Z (local).
pub fn gpu_shrine(p: &mut Plan, x: f32, z: f32, face: Face) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 0.15, 0.0), v(3.4, 0.3, 2.4), Mat::Dark, true);
        p.boxed(v(0.0, 0.45, -0.1), v(2.6, 0.3, 1.8), Mat::RackPanel, false);
        for k in 0..4 {
            let gx = -1.05 + k as f32 * 0.7;
            p.boxed(v(gx, 1.0, -0.2), v(0.55, 0.9, 0.12), Mat::Dark, false);
            p.boxed(v(gx, 1.0, -0.13), v(0.4, 0.4, 0.02), Mat::RackPanel, false);
            p.boxed(v(gx, 0.62, -0.13), v(0.5, 0.04, 0.02), Mat::LedGreen, false);
        }
        for (cx, cz) in [
            (-1.5, 0.9),
            (-0.8, 1.05),
            (0.8, 1.05),
            (1.5, 0.9),
            (-1.6, -0.9),
            (1.6, -0.9),
        ] {
            p.boxed(v(cx, 0.4, cz), v(0.1, 0.2, 0.1), Mat::Paper, false);
            p.boxed(v(cx, 0.54, cz), v(0.04, 0.06, 0.04), Mat::Candle, false);
        }
        p.light(v(0.0, 1.4, 1.2), Color::srgb(1.0, 0.6, 0.3), 120_000.0);
    });
}

/// Gong on a wooden frame.
pub fn gong(p: &mut Plan, x: f32, z: f32) {
    for dx in [-0.8, 0.8] {
        p.boxed(v(x + dx, 1.2, z), v(0.14, 2.4, 0.2), Mat::Wood, true);
    }
    p.boxed(v(x, 2.35, z), v(1.8, 0.14, 0.14), Mat::Wood, false);
    p.boxed(v(x, 1.45, z), v(1.1, 1.1, 0.06), Mat::Gold, false);
    p.boxed(v(x, 1.45, z + 0.04), v(0.3, 0.3, 0.04), Mat::Yellow, false);
    p.boxed(v(x + 1.3, 0.5, z), v(0.12, 1.0, 0.12), Mat::Wood, false);
}

/// Shelf of trophies against a wall. Open towards +Z (local).
pub fn trophy_shelf(p: &mut Plan, x: f32, z: f32, face: Face) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        for k in 0..3 {
            p.prop(
                &BOOKCASE_CLOSED_WIDE,
                v(-1.9 + k as f32 * 1.9, 0.0, 0.0),
                0.0,
                true,
            );
        }
        for k in 0..6 {
            let tx = -2.6 + k as f32 * 1.0;
            p.boxed(v(tx, 1.95, 0.0), v(0.12, 0.24, 0.12), Mat::Gold, false);
            p.boxed(v(tx, 2.12, 0.0), v(0.26, 0.12, 0.2), Mat::Gold, false);
        }
    });
}

/// Demo stand: podium and a big screen facing +Z (local).
pub fn demo_stand(p: &mut Plan, x: f32, z: f32, face: Face) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 0.5, 0.0), v(1.4, 1.0, 0.8), Mat::Dark, true);
        p.boxed(v(0.0, 1.02, 0.0), v(1.5, 0.04, 0.9), Mat::Gold, false);
        p.boxed(v(0.0, 1.5, -0.45), v(0.1, 1.0, 0.1), Mat::Dark, false);
        p.boxed(v(0.0, 2.3, -0.5), v(2.6, 1.6, 0.12), Mat::Dark, true);
        p.panel(
            v(0.0, 2.3, -0.43),
            0.0,
            Vec2::new(2.4, 1.4),
            Tex::DemoScreen,
        );
    });
}

/// Long glass-top conference table made of kit tables, with chairs. Along X (local),
/// chairs on both sides, `n` segments.
pub fn long_table(p: &mut Plan, x: f32, z: f32, n: usize, mat_glass: bool) {
    let seg = TABLE.size[0] * FURNITURE_SCALE;
    let x0 = x - seg * n as f32 / 2.0 + seg / 2.0;
    let model = if mat_glass { &TABLE_GLASS } else { &TABLE };
    for i in 0..n {
        let tx = x0 + i as f32 * seg;
        p.prop(model, v(tx, 0.0, z), 0.0, true);
        p.prop(&CHAIR_MODERN, v(tx, 0.0, z + 0.95), PI, false);
        p.prop(&CHAIR_MODERN, v(tx, 0.0, z - 0.95), 0.0, false);
    }
}

/// Neon sign (emissive texture) on a wall.
pub fn neon(p: &mut Plan, x: f32, y: f32, z: f32, face: Face, size: Vec2, tex: Tex) {
    p.wall_panel(v(x, y, z), face, size, tex);
}

/// Stacked old monitors, a MISC box and cable spaghetti.
pub fn it_clutter(p: &mut Plan, x: f32, z: f32) {
    for k in 0..4 {
        p.prop(&SCREEN, v(x, k as f32 * 0.72, z), 0.15 * k as f32, false);
    }
    p.block(v(x, 1.4, z), v(1.0, 2.8, 0.8));
    p.prop_scaled(&BOX_CLOSED, v(x + 2.0, 0.0, z), 0.3, true, 1.6);
    p.panel(
        v(x + 2.0, 0.4, z + 0.45),
        0.3,
        Vec2::new(0.55, 0.28),
        Tex::Misc,
    );
    for (dx, dz, w, d) in [
        (0.8, 0.8, 2.6, 0.05),
        (1.5, 1.0, 0.05, 1.8),
        (0.2, 1.6, 2.0, 0.05),
        (2.2, 1.9, 1.4, 0.05),
        (1.0, 1.2, 0.05, 1.0),
    ] {
        p.boxed(v(x + dx, 0.02, z + dz), v(w, 0.04, d), Mat::Dark, false);
    }
    p.boxed(
        v(x + 1.0, 0.03, z + 1.3),
        v(0.9, 0.04, 0.05),
        Mat::Cable,
        false,
    );
}

/// Mini fridge stacked with energy drinks.
pub fn mini_fridge(p: &mut Plan, x: f32, z: f32, face: Face) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 0.45, 0.0), v(0.6, 0.9, 0.6), Mat::White, true);
        p.boxed(v(0.0, 0.6, 0.31), v(0.5, 0.02, 0.02), Mat::Metal, false);
        for k in 0..5 {
            let m = [Mat::NeonCyan, Mat::NeonPink, Mat::NeonYellow][k % 3];
            p.boxed(
                v(-0.24 + k as f32 * 0.12, 1.0, 0.0),
                v(0.06, 0.16, 0.06),
                m,
                false,
            );
        }
    });
}

/// Room walls with one door; `side` +1 north / -1 south of the corridor.
pub fn room_walls(p: &mut Plan, x0: f32, x1: f32, side: f32, door: (f32, f32), mat: Mat, h: f32) {
    let z_in = side * 8.0;
    p.wall_z(z_in, x0, x1, &[door], mat, h);
    let (a, b) = if side > 0.0 {
        (8.0, 20.0)
    } else {
        (-20.0, -8.0)
    };
    p.wall_x(x0, a, b, &[], mat, h);
    p.wall_x(x1, a, b, &[], mat, h);
}

/// Skyline "window" on a long wall with gold-ish mullions.
pub fn window_wall(p: &mut Plan, z: f32, face: Face, xs: &[f32], mullion: Mat) {
    for &x in xs {
        p.wall_panel(v(x, 2.4, z), face, Vec2::new(6.0, 3.2), Tex::Skyline);
        for dx in [-3.0, 0.0, 3.0] {
            let d = face.dir();
            p.boxed(
                v(x + dx, 2.4, z + d.z * 0.06),
                v(0.12, 3.4, 0.1),
                mullion,
                false,
            );
        }
        p.boxed(v(x, 4.05, z), v(6.1, 0.12, 0.12), mullion, false);
        p.boxed(v(x, 0.75, z), v(6.1, 0.12, 0.12), mullion, false);
    }
}

/// Executive desk with a chair, facing +Z (local); the chair sits at +Z.
pub fn exec_desk(p: &mut Plan, x: f32, z: f32, face: Face) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 0.75, 0.0), v(2.6, 0.1, 1.2), Mat::MarbleDark, false);
        p.boxed(v(0.0, 0.37, 0.0), v(2.4, 0.74, 1.0), Mat::Wood, true);
        p.prop(&SCREEN, v(0.0, 0.8, -0.2), 0.0, false);
        p.prop(&CHAIR_DESK, v(0.0, 0.0, 1.2), PI, false);
        p.boxed(v(0.9, 0.9, 0.2), v(0.2, 0.3, 0.2), Mat::Gold, false);
    });
}
