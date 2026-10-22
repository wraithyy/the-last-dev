//! The Hackathon dungeon (docs/DUNGEONS.md), heroic: an overnight 24-hour hackathon in a converted
//! warehouse. hack24/1 is registration and the snack wall (Pizza Overlord), hack24/2 the hack floor
//! (a boss-less trash floor), hack24/3 the demo stage (Demo Day Gremlin) and hack24/4 the glass
//! pitch room (Pitch Judge VC). Every room is the corridor-hall-pod plan of the Migration and the
//! QBR so packs and route stay testable; what differs is the night: neon strings, pizza box towers,
//! energy drink pyramids, sleeping bags under the desks and a countdown stuck at 23:59:59. Tall
//! things hug the walls (the orbit camera follows the route at head height).

use super::basement::{Shape, whiteboard};
use super::datacenter::dress::{WallSpot, floor_line, rnd, strips};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::models;
use super::offsite::{balloons, bunting, cove, floor_patch, pilaster, round_patch, wall_dress};
use super::props::{beanbag, lounge, sad_plant, vending};
use super::qbr::{banner, curtains, floor_ring, greenery, lamps, tags, wall_screen};
use super::themes::{pt, runner, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => (
            "hack24/1",
            "HACK 24: Registration and Snack Wall",
            registration(&mut p),
        ),
        2 => ("hack24/2", "HACK 24: The Hack Floor", hack_floor(&mut p)),
        3 => ("hack24/3", "HACK 24: Demo Stage", demo_stage(&mut p)),
        _ => ("hack24/4", "HACK 24: The Pitch Room", pitch_room(&mut p)),
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, trim, accent, floor, fog, ambient, bright, range) = match level {
        // Registration: a warehouse at night, pink neon over concrete and pizza.
        1 => (
            c(0.62, 0.5, 0.82),
            c(0.14, 0.1, 0.24),
            LinearRgba::rgb(3.4, 0.5, 2.8),
            FloorKind::Carpet([0.95, 0.8, 1.1]),
            c(0.2, 0.14, 0.32),
            c(0.95, 0.85, 1.0),
            1500.0,
            (28.0, 90.0),
        ),
        // Hack floor: cyan monitor light, forty laptops and nobody awake.
        2 => (
            c(0.42, 0.58, 0.78),
            c(0.1, 0.14, 0.22),
            LinearRgba::rgb(0.3, 2.8, 3.2),
            FloorKind::Tile([1.1, 1.2, 1.4]),
            c(0.12, 0.2, 0.34),
            c(0.75, 0.9, 1.0),
            1400.0,
            (26.0, 90.0),
        ),
        // Demo stage: a black box with a magenta wash and one very bright screen.
        3 => (
            c(0.66, 0.3, 0.5),
            c(0.12, 0.05, 0.1),
            LinearRgba::rgb(3.6, 0.4, 1.8),
            FloorKind::Carpet([0.9, 0.4, 0.62]),
            c(0.24, 0.1, 0.2),
            c(1.0, 0.8, 0.92),
            1400.0,
            (24.0, 85.0),
        ),
        // Pitch room: glass, a cold blue night outside it and money-green lamps inside.
        _ => (
            c(0.62, 0.72, 0.86),
            c(0.14, 0.18, 0.28),
            LinearRgba::rgb(0.4, 2.4, 3.0),
            FloorKind::Gloss([1.0, 1.05, 1.2]),
            c(0.12, 0.2, 0.36),
            c(0.85, 0.95, 1.0),
            1500.0,
            (28.0, 95.0),
        ),
    };
    Theme {
        wall,
        trim,
        accent,
        fabric: [c(0.55, 0.2, 0.5), c(0.15, 0.4, 0.55), c(0.3, 0.3, 0.36)],
        floor,
        fog,
        ambient,
        ambient_brightness: bright,
        sun: c(0.7, 0.75, 1.0),
        sun_illuminance: 2200.0,
        fog_range: range,
    }
}

// ------------------------------------------------------------------ shared pieces

/// A tower of closed pizza boxes (cardboard slabs, each a little crooked, every fifth one a
/// white box with a red lid band), with an open box and a pizza on top for the optimist.
fn pizza_tower(p: &mut Plan, x: f32, z: f32, n: usize, seed: i32) {
    let h = 0.06;
    for k in 0..n {
        let i = seed * 31 + k as i32;
        let wob = 0.015 + k as f32 * 0.0012;
        let at = v(
            x + (rnd(i, 1) - 0.5) * wob * 2.0,
            h / 2.0 + k as f32 * h,
            z + (rnd(i, 2) - 0.5) * wob * 2.0,
        );
        let yaw = (rnd(i, 3) - 0.5) * 0.5;
        let white = k % 5 == 2;
        p.slab(
            at,
            yaw,
            v(0.58, h * 0.92, 0.58),
            if white { Mat::Paper } else { Mat::Cardboard },
        );
        if white {
            p.slab(
                at + v(0.0, h * 0.47, 0.0),
                yaw,
                v(0.58, 0.004, 0.14),
                Mat::Red,
            );
        }
    }
    let top = n as f32 * h;
    if seed % 2 == 0 {
        models::put(
            p,
            &models::PIZZA_BOX,
            v(x, top, z),
            rnd(seed, 4) * TAU,
            0.5,
            false,
        );
        models::put(
            p,
            &models::PIZZA,
            v(x, top + 0.02, z),
            rnd(seed, 4) * TAU,
            0.45,
            false,
        );
    }
    p.block(v(x, top / 2.0, z), v(0.62, top, 0.62));
}

/// 47 cans of energy drink: a 5x5 base, 4x4, 2x2, one, and a crushed one on top.
fn can_pyramid(p: &mut Plan, x: f32, z: f32) {
    let d = 0.31;
    let h = 0.32;
    for (k, n) in [5usize, 4, 2, 1].into_iter().enumerate() {
        for i in 0..n {
            for j in 0..n {
                let o = (n as f32 - 1.0) / 2.0;
                models::put(
                    p,
                    &models::ENERGY_CAN,
                    v(x + (i as f32 - o) * d, k as f32 * h, z + (j as f32 - o) * d),
                    rnd((k * 25 + i * 5 + j) as i32, 3) * TAU,
                    1.0,
                    false,
                );
            }
        }
    }
    models::put(p, &models::CAN_CRUSHED, v(x, 4.0 * h, z), 0.4, 1.0, false);
    p.block(v(x, 0.7, z), v(1.6, 1.4, 1.6));
}

/// A table with two laptops, cans, a pizza box and a mug; `sleeper` puts a sleeping bag under it
/// with a pair of sneakers sticking out.
fn hack_desk(p: &mut Plan, x: f32, z: f32, yaw: f32, seed: i32, sleeper: bool) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        let top = TABLE.size[1] * FURNITURE_SCALE;
        p.prop(&TABLE, v(0.0, 0.0, 0.0), 0.0, true);
        p.prop(
            &LAPTOP,
            v(-0.5, top, 0.15),
            0.3 * (rnd(seed, 1) - 0.5),
            false,
        );
        p.prop(
            &LAPTOP,
            v(0.5, top, -0.1),
            PI + 0.3 * (rnd(seed, 2) - 0.5),
            false,
        );
        for k in 0..3 {
            let m = if k == 1 {
                &models::CAN_CRUSHED
            } else {
                &models::ENERGY_CAN
            };
            models::put(
                p,
                m,
                v(-0.1 + k as f32 * 0.2, top, -0.25 + rnd(seed, 5 + k) * 0.1),
                rnd(seed, k) * TAU,
                0.8,
                false,
            );
        }
        models::put(p, &models::PIZZA_BOX, v(0.15, top, 0.1), 0.2, 0.5, false);
        models::put(p, &models::MUG, v(0.75, top, 0.3), 1.2, 0.5, false);
        p.prop(
            &CHAIR_DESK,
            v(0.2, 0.0, 1.0),
            PI + 0.6 * (rnd(seed, 9) - 0.5),
            false,
        );
        p.block(v(0.2, 0.5, 1.0), v(0.6, 1.0, 0.6));
        if sleeper {
            let bag =
                [Mat::Fabric0, Mat::Fabric1, Mat::Fabric2][(seed.unsigned_abs() as usize) % 3];
            p.slab(v(-0.05, 0.07, 0.0), 0.0, v(1.7, 0.13, 0.62), bag);
            p.prim(
                PrimKind::Sphere,
                v(0.8, 0.07, 0.0),
                v(0.62, 0.13, 0.62),
                bag,
            );
            models::put(
                p,
                &models::PILLOW_BLUE,
                v(-0.7, 0.14, 0.0),
                1.57,
                1.4,
                false,
            );
            p.boxed(v(-0.98, 0.1, 0.1), v(0.3, 0.14, 0.14), Mat::Dark, false);
            p.boxed(v(-0.98, 0.1, -0.1), v(0.3, 0.14, 0.14), Mat::Dark, false);
        }
    });
}

/// A sleeping bag on the open floor with a pillow, a hoodie for a blanket and a laptop still open.
fn sleeping_bag(p: &mut Plan, x: f32, z: f32, yaw: f32, seed: i32) {
    let bag =
        [Mat::Fabric0, Mat::Fabric1, Mat::Fabric2, Mat::Yoga][(seed.unsigned_abs() as usize) % 4];
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.slab(v(0.0, 0.08, 0.0), 0.0, v(0.8, 0.15, 1.8), bag);
        p.prim(PrimKind::Sphere, v(0.0, 0.08, 0.9), v(0.8, 0.15, 0.8), bag);
        p.slab(v(0.0, 0.17, 0.1), 0.0, v(0.82, 0.03, 0.05), Mat::Metal);
        models::put(
            p,
            &models::PILLOW_BLUE,
            v(0.0, 0.15, -0.72),
            0.0,
            1.5,
            false,
        );
        p.prop(
            &LAPTOP,
            v(0.75, 0.0, -0.4),
            0.5 * (rnd(seed, 1) - 0.5),
            false,
        );
    });
}

/// Sticky notes on a cardboard panel: five colours, every row a little crooked.
fn sticky_wall(p: &mut Plan, x: f32, z: f32, y: f32, cols: usize, rows: usize, seed: i32) {
    let Some(w) = WallSpot::at(p, x, z) else {
        return;
    };
    let (cw, ch) = (cols as f32 * 0.4 + 0.3, rows as f32 * 0.42 + 0.3);
    w.put(p, 0.0, y, 0.03, v(cw, ch, 0.04), Mat::Cardboard);
    let notes = [
        Mat::Yellow,
        Mat::NeonYellow,
        Mat::NeonPink,
        Mat::Paper,
        Mat::NeonCyan,
    ];
    for row in 0..rows {
        for col in 0..cols {
            let k = (row * cols + col) as i32 + seed * 97;
            w.put(
                p,
                -cw / 2.0 + 0.35 + col as f32 * 0.4 + rnd(k, 3) * 0.06,
                y - ch / 2.0 + 0.35 + row as f32 * 0.42 + rnd(k, 4) * 0.05,
                0.07,
                v(0.3, 0.3, 0.02),
                notes[(k.unsigned_abs() as usize + row) % notes.len()],
            );
        }
    }
    w.sign(p, 0.0, y + ch / 2.0 + 0.5, 3.0, Sign::H24Sticky);
}

/// A sign standing proud of a screen on the wall (the screen's own glow plane would hide it).
fn screen_sign(p: &mut Plan, wl: &WallSpot, l: f32, y: f32, w: f32, sign: Sign) {
    let c = wl.world(l, 0.22);
    p.sign(v(c.x, y, c.y), wl.yaw(), w, sign);
}

/// Coloured point lights for the neon: pink, cyan and yellow in turn.
fn neon_lights(p: &mut Plan, list: &[(f32, f32)], power: f32) {
    let colors = [
        Color::srgb(1.0, 0.3, 0.8),
        Color::srgb(0.2, 0.9, 1.0),
        Color::srgb(1.0, 0.9, 0.3),
    ];
    for (k, &(x, z)) in list.iter().enumerate() {
        p.light(v(x, 3.3, z), colors[k % 3], power);
    }
}

/// A glowing "AI" on a wall in a neon frame, because the product is always AI.
fn neon_ai(p: &mut Plan, x: f32, z: f32, y: f32, w: f32) {
    let Some(wl) = WallSpot::at(p, x, z) else {
        return;
    };
    let h = w * 0.5;
    wl.put(p, 0.0, y, 0.03, v(w + 0.3, h + 0.3, 0.02), Mat::Dark);
    wl.sign(p, 0.0, y, w, Sign::H24Ai);
    for (l, yy, sx, sy) in [
        (0.0, y + h / 2.0 + 0.1, w + 0.3, 0.07),
        (0.0, y - h / 2.0 - 0.1, w + 0.3, 0.07),
        (-w / 2.0 - 0.1, y, 0.07, h + 0.3),
        (w / 2.0 + 0.1, y, 0.07, h + 0.3),
    ] {
        wl.put(p, l, yy, 0.1, v(sx, sy, 0.05), Mat::NeonCyan);
    }
}

/// A ping-pong table, net, two paddles and a ball: tournament delayed, nobody has the ball.
fn ping_pong(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 0.74, 0.0), v(2.74, 0.06, 1.52), Mat::Felt, true);
        p.boxed(v(0.0, 0.6, 0.0), v(2.5, 0.2, 1.3), Mat::Dark, false);
        for (sx, sz) in [(-1.2, -0.65), (1.2, -0.65), (-1.2, 0.65), (1.2, 0.65)] {
            p.boxed(v(sx, 0.35, sz), v(0.08, 0.7, 0.08), Mat::Metal, false);
        }
        p.boxed(v(0.0, 0.77, 0.0), v(1.5, 0.01, 0.01), Mat::White, false);
        p.boxed(v(0.0, 0.9, 0.0), v(0.02, 0.2, 1.62), Mat::Paper, false);
        p.boxed(v(0.0, 0.8, 0.0), v(0.02, 0.01, 1.5), Mat::White, false);
        p.boxed(v(-0.7, 0.8, 0.3), v(0.18, 0.02, 0.18), Mat::Red, false);
        p.boxed(
            v(0.9, 0.8, -0.35),
            v(0.18, 0.02, 0.18),
            Mat::NeonCyan,
            false,
        );
    });
}

/// The wall clock: a big red digital countdown, forever at 23:59:59.
fn countdown(p: &mut Plan, x: f32, z: f32, y: f32, w: f32) {
    let Some(wl) = WallSpot::at(p, x, z) else {
        return;
    };
    let h = w * 96.0 / 256.0;
    wl.put(p, 0.0, y, 0.03, v(w + 0.3, h + 0.3, 0.02), Mat::Dark);
    wl.sign(p, 0.0, y, w, Sign::H24Clock);
    wl.put(
        p,
        0.0,
        y - h / 2.0 - 0.1,
        0.1,
        v(w + 0.3, 0.05, 0.05),
        Mat::LedRed,
    );
}

/// A cardboard-and-tape hardware project: a breadboard, a robot built from toy bricks, a car.
fn mvp_corner(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prop(&TABLE, v(0.0, 0.0, 0.0), 0.0, true);
        let top = TABLE.size[1] * FURNITURE_SCALE;
        // The MVP: bricks, taped.
        for (k, (dx, dy, m)) in [
            (0.0, 0.0, &models::BRICK_2X4),
            (0.02, 1.0, &models::BRICK_2X4),
            (-0.03, 2.0, &models::BRICK_2X2),
            (0.0, 3.0, &models::BRICK_2X2),
        ]
        .into_iter()
        .enumerate()
        {
            models::put(
                p,
                m,
                v(dx, top + dy * 0.12, 0.0),
                0.2 * k as f32,
                3.4,
                false,
            );
        }
        p.sign(v(0.0, top + 0.95, -0.4), 0.0, 0.8, Sign::H24Mvp);
        p.boxed(
            v(0.6, top + 0.02, 0.2),
            v(0.5, 0.03, 0.3),
            Mat::Cardboard,
            false,
        );
        p.boxed(
            v(0.6, top + 0.06, 0.2),
            v(0.4, 0.04, 0.2),
            Mat::LedGreen,
            false,
        );
        models::put(p, &models::SODA_CAN, v(-0.7, top, 0.2), 0.0, 0.8, false);
    });
}

/// A speaker stack: two big speakers and a small one on top, off the route.
fn speaker_stack(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prop(&SPEAKER, v(-0.4, 0.0, 0.0), 0.0, true);
        p.prop(&SPEAKER, v(0.4, 0.0, 0.0), 0.0, true);
        models::put(p, &models::SPEAKER_SMALL, v(0.0, 1.5, 0.0), 0.0, 2.4, false);
    });
}

/// Two end points on the floor plan.
type Line2 = ((f32, f32), (f32, f32));

/// Neon pennant strings across a room, three colours, hooked up under the ceiling.
fn neon_strings(p: &mut Plan, lines: &[Line2]) {
    for &(a, b) in lines {
        bunting(
            p,
            a,
            b,
            3.9,
            [Mat::NeonPink, Mat::NeonCyan, Mat::NeonYellow],
        );
    }
}

/// A stack of boxes of leftovers: pizza, a donut, fries, tacos and a bottle, on a wide surface.
fn leftovers(p: &mut Plan, x: f32, y: f32, z: f32, seed: i32) {
    let all = [
        &models::DONUT_CHOC,
        &models::COOKIE,
        &models::FRIES,
        &models::TACO,
        &models::CANDY_BAR,
        &models::STYROFOAM,
        &models::FRAPPE,
        &models::SODA_BOTTLE,
    ];
    for k in 0..5 {
        let m = all[(seed.unsigned_abs() as usize + k * 3) % all.len()];
        let u = if std::ptr::eq(m, &models::STYROFOAM) {
            0.45
        } else {
            0.9
        };
        models::put(
            p,
            m,
            v(
                x + (k as f32 - 2.0) * 0.28,
                y,
                z + (rnd(seed, k as i32) - 0.5) * 0.3,
            ),
            rnd(seed, k as i32 + 7) * TAU,
            u,
            false,
        );
    }
}

/// A hacker nook: a rug, three beanbags in a ring, an open pizza on the floor, cans, a laptop on
/// the carpet and a pillow, for people who gave up on chairs at midnight.
fn hack_nook(p: &mut Plan, x: f32, z: f32, seed: i32) {
    p.prop(
        &models::RUG_SQUARE,
        v(x, 0.0, z - 1.1),
        rnd(seed, 1) * 0.4,
        false,
    );
    let beans = [Mat::BeanA, Mat::BeanB, Mat::BeanC];
    for k in 0..3 {
        let a = rnd(seed, 3) * TAU + k as f32 * TAU / 3.0;
        beanbag(
            p,
            x + a.cos() * 1.6,
            z + a.sin() * 1.4,
            beans[(k + seed.unsigned_abs() as usize) % 3],
        );
    }
    models::put(
        p,
        &models::PIZZA_BOX,
        v(x, 0.01, z),
        rnd(seed, 4) * TAU,
        0.6,
        false,
    );
    models::put(
        p,
        &models::PIZZA,
        v(x, 0.065, z),
        rnd(seed, 5) * TAU,
        0.52,
        false,
    );
    for k in 0..3 {
        models::put(
            p,
            if k == 2 {
                &models::CAN_CRUSHED
            } else {
                &models::ENERGY_CAN
            },
            v(
                x + 0.5 + k as f32 * 0.25,
                0.0,
                z - 0.4 + rnd(seed, 6 + k) * 0.5,
            ),
            rnd(seed, k) * TAU,
            0.9,
            false,
        );
    }
    p.prop(&LAPTOP, v(x - 0.5, 0.0, z + 0.3), rnd(seed, 9) * TAU, false);
    models::put(
        p,
        &models::PILLOW_LONG,
        v(x - 0.9, 0.0, z - 0.6),
        rnd(seed, 2) * TAU,
        1.6,
        false,
    );
}

/// Cables snaking over the carpet between two points, a little crooked.
fn floor_cables(p: &mut Plan, pts: &[(f32, f32)]) {
    for w in pts.windows(2) {
        floor_line(p, pt(w[0].0, w[0].1), pt(w[1].0, w[1].1), 0.05, Mat::Cable);
    }
}

/// A pitch-black cardboard prize: a toy-car item box on a plinth with a hand-written label.
fn prize_box(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.35, z), v(0.8, 0.7, 0.8), Mat::Dark, true);
    models::put(p, &models::ITEM_BOX, v(x, 0.7, z), 0.4, 1.5, false);
    p.light(v(x, 1.6, z), Color::srgb(1.0, 0.8, 0.3), 40_000.0);
}

fn desk_row(p: &mut Plan, x0: f32, step: f32, n: usize, z: f32, yaw: f32, seed: i32) {
    for k in 0..n {
        let i = seed + k as i32;
        hack_desk(p, x0 + k as f32 * step, z, yaw, i, k % 2 == 0);
    }
}

// ------------------------------------------------------------------ hack24/1: registration

const REG: Shape = Shape {
    arrival_x: -44.0,
    hall: (-6.0, 22.0, 17.0),
    cells: [(-14.0, -30.0), (-14.0, 30.0)],
    pod: (36.0, 12.0, 10.0),
    pod_wall: Mat::Wall,
};

/// The boss stands here, facing the arrival; the one cold wedge is 8 m in front of it.
const PIZZA_AT: (f32, f32) = (36.0, 0.0);

/// The pizza floor of the boss pod: eight wedges of cheese and sauce round the Overlord and one
/// pineapple wedge, lit, where the Hot Slice telegraph's cover sits.
fn pizza_floor(p: &mut Plan) {
    let (cx, cz) = PIZZA_AT;
    for w in 0..8 {
        let (a0, a1) = (w as f32 * 45.0, (w + 1) as f32 * 45.0);
        let mat = if w % 2 == 0 { Mat::Candle } else { Mat::Yellow };
        for k in 0..16 {
            let a = (a0 + (a1 - a0) * (k as f32 + 0.5) / 16.0).to_radians();
            let len = 9.2;
            p.slab(
                v(cx + a.cos() * len / 2.0, 0.006, cz + a.sin() * len / 2.0),
                -a,
                v(len, 0.008, 0.62),
                mat,
            );
        }
        let r = a0.to_radians();
        floor_line(
            p,
            pt(cx, cz),
            pt(cx + r.cos() * 9.4, cz + r.sin() * 9.4),
            0.16,
            Mat::Hot,
        );
    }
    floor_ring(p, cx, cz, 9.4, 40, Mat::Cardboard, 0.35);
    // Pepperoni: red discs scattered over the cheese.
    for k in 0..14 {
        let a = rnd(k, 1) * TAU;
        let r = 2.0 + rnd(k, 2) * 6.5;
        p.prim(
            PrimKind::Cylinder,
            v(cx + a.cos() * r, 0.012, cz + a.sin() * r),
            v(0.9, 0.012, 0.9),
            Mat::Red,
        );
    }
    // The pineapple wedge: a lit pad, 8 m in front of the boss.
    let (x, z) = (cx - 8.0, cz);
    round_patch(p, x, z, 7.6, Mat::Metal);
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.014, z),
        v(7.0, 0.02, 7.0),
        Mat::NeonYellow,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.02, z),
        v(5.2, 0.02, 5.2),
        Mat::Candle,
    );
    p.light(v(x, 1.8, z), Color::srgb(1.0, 0.9, 0.4), 110_000.0);
    // A small pineapple at the pad's edge, so nobody doubts which wedge it is.
    p.prim(
        PrimKind::Sphere,
        v(x, 0.4, z - 3.9),
        v(0.55, 0.7, 0.55),
        Mat::NeonYellow,
    );
    for k in 0..5 {
        let a = k as f32 * TAU / 5.0;
        p.prim(
            PrimKind::Cone,
            v(x + a.cos() * 0.15, 0.95, z - 3.9 + a.sin() * 0.15),
            v(0.18, 0.5, 0.18),
            Mat::Plant,
        );
    }
}

fn registration(p: &mut Plan) -> Theme {
    REG.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Trim, Mat::NeonPink, Mat::Dark);
    cove(p, Mat::NeonPink);

    runner(
        p,
        &[pt(-46.0, 0.0), pt(-30.0, 0.0), pt(-6.0, 0.0), pt(36.0, 0.0)],
        2.4,
        Mat::Dark,
        Mat::NeonCyan,
    );
    for x in [-26.0, -9.5, 4.0, 12.0] {
        pilaster(p, x, -16.9, Mat::Trim);
        pilaster(p, x, 16.9, Mat::Trim);
    }
    // Arrival: the registration desk, the Wi-Fi sticky note and a rack of lanyards.
    for z in [3.6, 6.6] {
        p.prop(&TABLE_CLOTH, v(-48.0, 0.0, z + 1.0), FRAC_PI_2, true);
    }
    p.prop(&LAPTOP, v(-48.0, 0.8, 4.6), -FRAC_PI_2, false);
    p.prop(&LAPTOP, v(-48.0, 0.8, 7.2), -FRAC_PI_2, false);
    leftovers(p, -48.0, 0.8, 5.8, 3);
    p.sign_near(-49.9, 6.0, 2.6, 2.6, Sign::H24Welcome);
    p.sign_near(-49.9, -4.5, 2.2, 2.4, Sign::H24Wifi);
    p.prop(&COAT_RACK, v(-47.5, 0.0, -5.0), 0.0, false);
    pizza_tower(p, -47.5, -8.5, 22, 1);
    beanbag(p, -47.0, 11.0, Mat::BeanA);
    beanbag(p, -45.5, 12.8, Mat::BeanB);
    sleeping_bag(p, -41.0, 11.6, 1.2, 2);

    // North wall: the AI-FIRST banner, the sticky-note wall, pizza box towers and a swag table.
    if let Some(wl) = wall_screen(p, -24.0, -16.9, 2.7, 5.6, 2.1) {
        screen_sign(p, &wl, 0.0, 2.7, 5.4, Sign::H24Welcome);
    }
    sticky_wall(p, -14.0, -16.9, 2.2, 14, 6, 1);
    banner(p, -1.0, -16.9, 5.1, 2.5, Tex::AiFirst, 1.5);
    neon_ai(p, 7.0, -16.9, 2.7, 2.6);
    p.sign_near(12.5, -16.9, 2.6, 2.6, Sign::H24Leader);
    for (k, x) in [-27.0, -24.5, -9.5, -7.0, 14.5].into_iter().enumerate() {
        pizza_tower(p, x, -15.0, 30 + (k * 5) % 12, k as i32 + 4);
    }
    for k in 0..2 {
        p.prop(&TABLE_CLOTH, v(1.0 + k as f32 * 2.0, 0.0, -14.4), 0.0, true);
    }
    // The last XXXL t-shirt, folded, with a hopeful sign.
    p.boxed(v(2.0, 0.83, -14.4), v(0.7, 0.07, 0.5), Mat::Red, false);
    p.boxed(v(2.0, 0.88, -14.4), v(0.5, 0.05, 0.3), Mat::Paper, false);
    p.sign_near(2.0, -16.9, 2.1, 2.2, Sign::H24Swag);
    tags(p, &[("prop:hack_tee", 2.0, -13.3)]);
    lounge(p, -19.0, -11.0, 0.0, true);
    beanbag(p, 6.0, -12.0, Mat::BeanC);
    beanbag(p, 8.4, -12.6, Mat::BeanA);
    models::put(
        p,
        &models::PILLOW_BLUE_LONG,
        v(7.0, 0.3, -12.0),
        0.5,
        1.6,
        false,
    );

    // South wall: the drinks fridge with no cold drinks, the can pyramid, the sign-up sheet.
    for k in 0..3 {
        p.prop(
            &models::FRIDGE_LARGE,
            v(-25.0 + k as f32 * 1.1, 0.0, 16.3),
            PI,
            true,
        );
    }
    p.sign_near(-24.0, 16.9, 3.0, 2.8, Sign::H24Fridge);
    tags(p, &[("prop:hack_fridge", -23.8, 13.9)]);
    can_pyramid(p, -17.0, 14.6);
    p.sign_near(-17.0, 16.9, 3.0, 2.2, Sign::H24Cans);
    can_pyramid(p, -9.8, 15.0);
    for k in 0..2 {
        p.prop(&TABLE_CLOTH, v(-3.0 + k as f32 * 2.0, 0.0, 14.8), PI, true);
    }
    p.prop(&LAPTOP, v(-3.0, 0.8, 14.8), PI, false);
    p.boxed(v(-2.0, 0.83, 14.5), v(0.5, 0.01, 0.7), Mat::Paper, false);
    p.sign_near(-2.0, 16.9, 2.6, 3.2, Sign::H24Team);
    tags(p, &[("prop:hack_signup", -2.0, 13.4)]);
    leftovers(p, -3.0, 0.8, 15.2, 9);
    for (k, x) in [6.0, 9.0, 12.5].into_iter().enumerate() {
        pizza_tower(p, x, 15.2, 24 + k * 8, 20 + k as i32);
    }
    p.prop(&KITCHEN_BAR, v(14.2, 0.0, 15.2), PI, true);
    for k in 0..2 {
        p.prop(
            &models::MICROWAVE,
            v(13.8 + k as f32 * 0.9, 1.0, 15.2),
            PI,
            false,
        );
    }
    models::put(p, &models::BLENDER, v(15.2, 1.0, 15.2), PI, 2.4, false);

    // The middle of the hall: hackers who gave up on the chairs. Desks with sleeping bags under
    // them (the doc's rule: under, not on), beanbags, neon strings and a ball of pizza boxes.
    desk_row(p, -20.0, 4.8, 3, 7.4, 0.0, 10);
    desk_row(p, -20.0, 4.8, 2, -7.4, PI, 20);
    sleeping_bag(p, 6.0, 6.6, 0.3, 3);
    sleeping_bag(p, 8.0, 7.6, -0.2, 4);
    sleeping_bag(p, -2.0, -6.8, 1.4, 5);
    desk_row(p, 1.0, 4.8, 2, 8.4, 0.0, 30);
    neon_strings(
        p,
        &[
            ((-27.0, -14.0), (15.0, -14.0)),
            ((-27.0, 14.0), (15.0, 14.0)),
            ((-27.0, -7.5), (15.0, -7.5)),
            ((-27.0, 7.5), (15.0, 7.5)),
        ],
    );
    balloons(
        p,
        -29.0,
        -5.0,
        [Mat::NeonPink, Mat::NeonCyan, Mat::NeonYellow],
    );
    balloons(
        p,
        -29.0,
        6.0,
        [Mat::NeonCyan, Mat::NeonYellow, Mat::NeonPink],
    );
    greenery(p, 15.0, -9.0);
    sad_plant(p, 15.0, 8.0);
    for (k, (x, z)) in [
        (-24.0, 10.5),
        (-6.5, 9.5),
        (-9.0, -10.5),
        (6.5, -9.0),
        (-31.0, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        if x > -30.0 {
            hack_nook(p, x, z, 200 + k as i32);
        }
    }
    floor_cables(
        p,
        &[
            (-20.0, 6.2),
            (-14.0, 4.6),
            (-6.0, 5.4),
            (4.0, 4.4),
            (12.0, 6.0),
        ],
    );
    floor_cables(
        p,
        &[(-22.0, -6.0), (-12.0, -4.8), (-2.0, -5.6), (8.0, -4.2)],
    );
    mvp_corner(p, 12.0, -9.5, 0.0);
    for (k, (x, z)) in [(-12.0, 12.5), (-22.0, -13.0), (2.0, -9.2)]
        .into_iter()
        .enumerate()
    {
        pizza_tower(p, x, z, 18 + 6 * k, 120 + k as i32);
    }

    // North cell: swag storage. Boxes of shirts, an XXXL pile (one shirt), pizza overflow.
    for k in 0..4 {
        p.prop(
            &BOOKCASE_CLOSED_WIDE,
            v(-21.0 + k as f32 * 1.7, 0.0, -36.8),
            0.0,
            true,
        );
    }
    for k in 0..5 {
        models::put(
            p,
            &models::BOX_OPEN,
            v(-9.5, 0.0, -34.5 + k as f32 * 1.2),
            rnd(k, 1) * 0.6,
            2.2,
            true,
        );
    }
    pizza_tower(p, -19.0, -26.0, 40, 31);
    pizza_tower(p, -17.4, -26.4, 34, 32);
    pizza_tower(p, -15.8, -26.0, 28, 33);
    mvp_corner(p, -12.0, -33.0, 0.0);
    p.sign_near(-14.0, -36.9, 2.9, 2.6, Sign::H24Pizza);
    // South cell: the snack wall. Every drink, every bag, a vending machine out of order.
    vending(p, -7.0, 27.5, Face::NegX);
    vending(p, -7.0, 29.0, Face::NegX);
    for k in 0..4 {
        p.prop(
            &KITCHEN_CABINET,
            v(-21.0 + k as f32 * 2.1, 0.0, 36.0),
            PI,
            true,
        );
    }
    can_pyramid(p, -18.0, 28.5);
    can_pyramid(p, -11.0, 28.0);
    leftovers(p, -20.0, 1.0, 35.8, 14);
    leftovers(p, -16.0, 1.0, 35.8, 15);
    p.sign_near(-14.0, 36.9, 2.9, 2.6, Sign::H24Cans);

    // The boss pod: a pizza kitchen. Conveyor with boxes, the oven, flour, a wall of boxes.
    pizza_floor(p);
    for k in 0..6 {
        let x = 26.0 + k as f32 * 2.0;
        models::put(p, &models::CONVEYOR_LONG, v(x, 0.0, -8.6), 0.0, 1.0, true);
        if k % 2 == 0 {
            models::put(p, &models::PIZZA_BOX, v(x, 0.4, -8.6), 0.3, 0.6, false);
        }
    }
    models::put(
        p,
        &models::LEVER_SINGLE,
        v(43.6, 0.0, -7.0),
        -FRAC_PI_2,
        1.8,
        false,
    );
    for k in 0..3 {
        models::put(
            p,
            &models::STOOL_SQUARE,
            v(12.8 + k as f32 * 1.0, 0.0, 14.0),
            0.0,
            2.4,
            false,
        );
    }
    models::put(
        p,
        &models::MACHINE_WINDOW,
        v(46.2, 0.0, -8.2),
        -FRAC_PI_2,
        1.5,
        true,
    );
    p.light(v(45.0, 1.6, -8.2), Color::srgb(1.0, 0.45, 0.15), 160_000.0);
    p.sign_near(46.0, -9.9, 3.4, 2.4, Sign::H24Pizza);
    models::put(p, &models::HOPPER_SQUARE, v(46.5, 0.0, 7.5), 0.0, 1.3, true);
    pizza_tower(p, 27.0, 8.6, 36, 41);
    pizza_tower(p, 28.7, 8.9, 30, 42);
    pizza_tower(p, 41.0, 8.8, 40, 43);
    p.prop(&TABLE_CLOTH, v(34.0, 0.0, 9.0), PI, true);
    models::put(p, &models::PIZZA_CUTTER, v(34.0, 0.8, 9.0), 0.4, 1.4, false);
    models::put(p, &models::PIZZA, v(33.0, 0.8, 9.0), 0.0, 0.5, false);
    banner(p, 36.0, 9.9, 4.4, 2.6, Tex::PizzaBox, 1.0);
    neon_strings(
        p,
        &[((26.0, -9.0), (46.0, -9.0)), ((26.0, 9.0), (46.0, 9.0))],
    );
    p.sign(v(47.7, 3.6, -3.0), -FRAC_PI_2, 2.2, Sign::H24Door);
    speaker_stack(p, 46.5, 3.0, -FRAC_PI_2);

    neon_lights(
        p,
        &[
            (-36.0, -8.0),
            (-20.0, 10.0),
            (-4.0, -10.0),
            (10.0, 10.0),
            (28.0, -6.0),
            (44.0, 6.0),
        ],
        260_000.0,
    );
    strips(p, Mat::LightPanel, Mat::NeonPink, 6.0, 0.35);
    lamps(
        p,
        Color::srgb(0.95, 0.7, 1.0),
        &[
            (-44.0, 0.0),
            (-26.0, 0.0),
            (-6.0, -9.0),
            (-6.0, 9.0),
            (10.0, 0.0),
            (-14.0, -30.0),
            (-14.0, 30.0),
            (36.0, 0.0),
        ],
        430_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:campus", (-50.0, 0.0), Sign::H24ToDay),
                ("door:hack24/2", (48.0, 6.0), Sign::H24ToFloor),
            ],
            packs: &REG.cell_packs(),
            boss: PIZZA_AT,
            npc: None,
        },
    );
    theme(1)
}

// ------------------------------------------------------------------ hack24/2: the hack floor

const FLOOR: Shape = Shape {
    arrival_x: -44.0,
    hall: (-4.0, 26.0, 18.0),
    cells: [(-18.0, -31.0), (10.0, 31.0)],
    pod: (40.0, 12.0, 10.0),
    pod_wall: Mat::Wall,
};

fn hack_floor(p: &mut Plan) -> Theme {
    FLOOR.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Trim, Mat::NeonCyan, Mat::Dark);
    cove(p, Mat::NeonCyan);

    runner(
        p,
        &[pt(-46.0, 0.0), pt(-30.0, 0.0), pt(-4.0, 0.0), pt(40.0, 0.0)],
        2.4,
        Mat::Dark,
        Mat::NeonPink,
    );
    for x in [-26.0, -9.5, 7.0, 18.0] {
        pilaster(p, x, -17.9, Mat::Trim);
        pilaster(p, x, 17.9, Mat::Trim);
    }
    // North wall: the architecture diagram (a box labelled AI), a countdown that never moves,
    // the sticky-note wall and the whiteboard of ideas, which is also a quest.
    whiteboard(p, -23.0, -17.9, 2.6, 4.6, Sign::H24Whiteboard);
    countdown(p, -12.0, -17.9, 3.1, 5.6);
    p.sign_near(-12.0, -17.9, 1.6, 2.6, Sign::H24ClockNote);
    whiteboard(p, 0.0, -17.9, 2.4, 5.0, Sign::H24Ideas);
    tags(p, &[("prop:hack_whiteboard", 0.0, -16.0)]);
    sticky_wall(p, 10.0, -17.9, 2.3, 14, 6, 2);
    banner(p, 20.0, -17.9, 4.6, 2.6, Tex::AiFirst, 1.5);
    // South wall: the mentor corner with a bored mentor's shirt, more sticky notes, a poster.
    sticky_wall(p, -20.0, 17.9, 2.3, 14, 6, 3);
    neon_ai(p, -9.0, 17.9, 2.8, 2.8);
    p.sign_near(0.0, 17.9, 2.7, 2.4, Sign::H24Mentor);
    p.prop(&TABLE_CLOTH, v(1.0, 0.0, 16.4), PI, true);
    p.prop(&STOOL_BAR, v(0.0, 0.0, 14.8), 0.0, false);
    p.prop(&STOOL_BAR, v(2.0, 0.0, 14.8), 0.0, false);
    p.prop(&COFFEE_MACHINE, v(1.0, 0.8, 16.4), PI, false);
    models::put(p, &models::RADIO, v(-0.3, 0.8, 16.3), PI, 1.6, false);
    models::put(p, &models::MUG, v(2.0, 0.8, 16.2), 0.5, 0.5, false);
    p.sign_near(6.5, 17.9, 2.5, 2.4, Sign::H24Quiet);
    banner(p, 14.0, 17.9, 4.0, 2.6, Tex::Suggestions, 1.3);
    banner(p, 22.0, 17.9, 3.6, 2.6, Tex::Standup, 1.3);

    // The floor itself: islands of desks facing each other, sleeping bags under them, beanbags
    // in the aisles and open laptops on the carpet.
    for (row, z) in [-9.0, 9.0].into_iter().enumerate() {
        for k in 0..5 {
            let x = -22.0 + k as f32 * 5.2 + row as f32 * 2.0;
            hack_desk(
                p,
                x,
                z,
                if row == 0 { 0.0 } else { PI },
                40 + k as i32 + row as i32 * 9,
                (k + row) % 2 == 0,
            );
        }
    }
    for (k, (x, z)) in [
        (6.0, 6.2),
        (8.5, -6.0),
        (-6.0, -5.4),
        (-3.0, 6.4),
        (14.0, 5.8),
        (16.0, -6.4),
    ]
    .into_iter()
    .enumerate()
    {
        sleeping_bag(p, x, z, 0.4 + k as f32, 60 + k as i32);
    }
    for (x, z, m) in [
        (22.0, 5.0, Mat::BeanA),
        (23.5, 6.8, Mat::BeanB),
        (22.4, -5.6, Mat::BeanC),
        (24.0, -7.2, Mat::BeanA),
        (-27.0, 6.0, Mat::BeanB),
        (-28.0, -6.5, Mat::BeanC),
    ] {
        beanbag(p, x, z, m);
    }
    neon_strings(
        p,
        &[
            ((-28.0, -15.0), (24.0, -15.0)),
            ((-28.0, 15.0), (24.0, 15.0)),
            ((-28.0, -5.5), (24.0, -5.5)),
            ((-28.0, 5.5), (24.0, 5.5)),
        ],
    );
    pizza_tower(p, 24.5, 13.0, 30, 70);
    pizza_tower(p, 24.5, -13.0, 36, 71);
    balloons(
        p,
        -31.0,
        -4.0,
        [Mat::NeonYellow, Mat::NeonPink, Mat::NeonCyan],
    );
    for (row, z) in [-14.2, 14.2].into_iter().enumerate() {
        for k in 0..8 {
            let x = -26.0 + k as f32 * 5.0;
            hack_desk(
                p,
                x,
                z,
                if row == 0 { 0.0 } else { PI },
                140 + k as i32 + row as i32 * 11,
                (k + row) % 3 == 0,
            );
        }
    }
    for (k, (x, z)) in [(-14.0, 5.8), (2.0, -5.8), (12.0, 5.8), (-30.0, -2.0)]
        .into_iter()
        .enumerate()
    {
        if x > -29.0 {
            hack_nook(p, x, z, 300 + k as i32);
        }
    }
    floor_cables(
        p,
        &[
            (-24.0, -5.0),
            (-12.0, -3.8),
            (0.0, -4.6),
            (14.0, -3.6),
            (22.0, -5.0),
        ],
    );
    floor_cables(
        p,
        &[
            (-24.0, 5.0),
            (-10.0, 3.8),
            (4.0, 4.6),
            (16.0, 3.6),
            (22.0, 5.0),
        ],
    );
    greenery(p, 25.5, 15.5);
    sad_plant(p, -31.0, 8.0);

    // North cell: the nap room. Lights off, bags in rows, one beanbag per sleeper.
    for k in 0..6 {
        sleeping_bag(
            p,
            -25.0 + (k % 3) as f32 * 2.4,
            -33.0 + (k / 3) as f32 * 2.6,
            FRAC_PI_2 * (k % 2) as f32,
            80 + k,
        );
    }
    for k in 0..3 {
        beanbag(
            p,
            -12.0,
            -34.0 + k as f32 * 2.0,
            [Mat::BeanA, Mat::BeanB, Mat::BeanC][k],
        );
    }
    p.sign_near(-18.0, -37.9, 2.8, 2.6, Sign::H24Quiet);
    p.prop(&COAT_RACK, v(-23.0, 0.0, -36.5), 0.0, false);
    // South cell: the kitchen. Microwave, blender, 47 cans and a fridge of cold things for nobody.
    for k in 0..3 {
        p.prop(
            &KITCHEN_CABINET,
            v(4.0 + k as f32 * 2.1, 0.0, 37.0),
            PI,
            true,
        );
    }
    for k in 0..2 {
        p.prop(
            &models::MICROWAVE,
            v(4.0 + k as f32 * 1.0, 1.0, 37.0),
            PI,
            false,
        );
    }
    models::put(p, &models::BLENDER, v(8.6, 1.0, 37.0), PI, 2.4, false);
    p.prop(&models::FRIDGE_LARGE, v(14.0, 0.0, 36.9), PI, true);
    can_pyramid(p, 6.0, 33.5);
    can_pyramid(p, 13.0, 33.0);
    leftovers(p, 4.5, 1.0, 36.8, 6);
    p.sign_near(10.0, 37.9, 2.9, 2.6, Sign::H24Cans);

    // The pod: the break area. A ping-pong table, a foosball of laptops, a caution sign.
    for (x, z, yaw) in [(34.0, -8.4, 0.0), (46.0, 8.4, PI)] {
        p.prop(&models::BENCH, v(x, 0.0, z), yaw, true);
    }
    ping_pong(p, 40.0, -6.5, 0.0);
    p.sign_near(40.0, -9.9, 2.6, 2.6, Sign::H24PingPong);
    ping_pong(p, 40.0, 6.5, 0.0);
    p.sign_near(44.0, 9.9, 2.7, 2.2, Sign::H24Caution);
    lounge(p, 33.5, -6.8, 0.0, false);
    mvp_corner(p, 48.0, -6.0, -FRAC_PI_2);
    mvp_corner(p, 48.0, 6.0, -FRAC_PI_2);
    p.sign(v(51.7, 3.4, 0.0), -FRAC_PI_2, 2.4, Sign::H24Door);
    for z in [-4.5, 4.5] {
        models::put(p, &models::BARRIER, v(30.0, 0.0, z), 0.0, 5.0, false);
    }
    models::put(p, &models::SIGN_WARNING, v(30.0, 0.0, 9.0), 0.0, 5.0, false);
    models::put(p, &models::CONE_ROAD, v(33.0, 0.0, -9.0), 0.0, 8.0, false);
    neon_strings(
        p,
        &[((30.0, -9.0), (50.0, -9.0)), ((30.0, 9.0), (50.0, 9.0))],
    );

    neon_lights(
        p,
        &[
            (-36.0, 8.0),
            (-20.0, -12.0),
            (-4.0, 12.0),
            (14.0, -12.0),
            (32.0, -6.0),
            (46.0, 6.0),
        ],
        260_000.0,
    );
    strips(p, Mat::LightPanel, Mat::NeonCyan, 6.0, 0.45);
    lamps(
        p,
        Color::srgb(0.6, 0.85, 1.0),
        &[
            (-44.0, 0.0),
            (-26.0, 0.0),
            (-8.0, -10.0),
            (-8.0, 10.0),
            (10.0, 0.0),
            (-18.0, -31.0),
            (10.0, 31.0),
            (40.0, 0.0),
        ],
        410_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:hack24/1", (-50.0, 0.0), Sign::H24ToReg),
                ("door:hack24/3", (52.0, 6.0), Sign::H24ToStage),
            ],
            packs: &[FLOOR.cell_packs()[0], FLOOR.cell_packs()[1], (14.0, 11.0)],
            // The contract wants a boss anchor even in a boss-less room: the break pod's centre.
            boss: (40.0, 0.0),
            npc: None,
        },
    );
    theme(2)
}

// ------------------------------------------------------------------ hack24/3: the demo stage

const STAGE: Shape = Shape {
    arrival_x: -44.0,
    hall: (-8.0, 24.0, 15.0),
    cells: [(-20.0, -26.0), (6.0, 26.0)],
    pod: (42.0, 13.0, 12.0),
    pod_wall: Mat::Wall,
};

const GREMLIN_AT: (f32, f32) = (44.0, 0.0);

fn demo_stage(p: &mut Plan) -> Theme {
    STAGE.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Trim, Mat::NeonPink, Mat::Dark);
    cove(p, Mat::NeonRed);

    runner(
        p,
        &[pt(-46.0, 0.0), pt(-30.0, 0.0), pt(0.0, 0.0), pt(44.0, 0.0)],
        2.4,
        Mat::Dark,
        Mat::NeonPink,
    );
    for x in [-30.0, -14.0, 0.0, 12.0] {
        pilaster(p, x, -14.9, Mat::Trim);
        pilaster(p, x, 14.9, Mat::Trim);
    }
    // The hall is the audience: rows of chairs facing the stage, a projector cart, a Wi-Fi sign.
    for (r, x) in [-2.0, 0.4, 2.8, 5.2, 7.6, 10.0].into_iter().enumerate() {
        for k in 0..4 {
            for z in [-5.0 - k as f32 * 1.35, 5.0 + k as f32 * 1.35] {
                if (r + k) % 7 != 3 {
                    p.prop(&CHAIR_MODERN, v(x, 0.0, z), FRAC_PI_2, false);
                }
            }
        }
    }
    // The demo prep desks at the back of the hall: laptops, cables, a very nervous team.
    for k in 0..4 {
        hack_desk(p, -26.0 + k as f32 * 5.2, -10.5, 0.0, 90 + k, k % 2 == 1);
        hack_desk(p, -26.0 + k as f32 * 5.2, 10.5, PI, 100 + k, k % 2 == 0);
    }
    p.sign_near(-14.0, -14.9, 3.2, 3.0, Sign::H24Wifi);
    p.sign_near(-26.0, 14.9, 3.2, 3.0, Sign::H24Recording);
    sticky_wall(p, -2.0, -14.9, 2.2, 12, 5, 4);
    banner(p, -2.0, 14.9, 4.8, 2.6, Tex::AiFirst, 1.5);
    banner(p, 10.0, 14.9, 3.6, 2.6, Tex::DemoScreen, 1.4);
    neon_ai(p, 10.0, -14.9, 2.8, 2.6);
    neon_strings(
        p,
        &[
            ((-31.0, -13.0), (15.0, -13.0)),
            ((-31.0, 13.0), (15.0, 13.0)),
            ((-31.0, -7.5), (-3.0, -7.5)),
            ((-31.0, 7.5), (-3.0, 7.5)),
        ],
    );
    for (k, (x, z)) in [(-28.0, 6.2), (-12.0, 6.0), (-24.0, -6.2), (-8.0, -6.0)]
        .into_iter()
        .enumerate()
    {
        hack_nook(p, x, z, 400 + k as i32);
    }
    floor_cables(p, &[(-30.0, 3.0), (-18.0, 4.2), (-6.0, 3.4), (8.0, 4.4)]);
    floor_cables(
        p,
        &[(-30.0, -3.0), (-18.0, -4.2), (-6.0, -3.4), (8.0, -4.4)],
    );
    pizza_tower(p, -31.0, 13.4, 34, 110);
    pizza_tower(p, -31.0, -13.4, 40, 111);
    can_pyramid(p, 15.0, -12.5);
    greenery(p, 15.0, 12.0);
    for z in [-3.2, 3.2] {
        floor_line(p, pt(-34.0, z), pt(14.0, z), 0.1, Mat::NeonPink);
    }

    // North cell: backstage. Flight cases of cables, a mirrored display that shows the wrong
    // screen, speakers that squeal, a finish gate somebody borrowed from a toy race.
    for k in 0..3 {
        speaker_stack(p, -26.0 + k as f32 * 2.0, -31.5, 0.0);
    }
    for k in 0..3 {
        p.prop(
            &models::TV_ANTENNA,
            v(-18.0 + k as f32 * 1.4, 1.0, -32.0),
            0.0,
            false,
        );
        p.prop(
            &SIDE_TABLE_DRAWERS,
            v(-18.0 + k as f32 * 1.4, 0.0, -32.0),
            0.0,
            true,
        );
    }
    p.prop(&models::TABLE_CROSS, v(-13.5, 0.0, -21.5), 0.0, true);
    p.prop(&LAPTOP, v(-14.5, 0.84, -21.5), 0.3, false);
    models::put(p, &models::RADIO, v(-12.5, 0.84, -21.5), 0.0, 1.6, false);
    models::put(p, &models::BUTTON_FLOOR, v(36.5, 0.0, 8.0), 0.0, 2.4, false);
    models::put(
        p,
        &models::GATE_FINISH,
        v(-20.0, 0.0, -20.0),
        PI,
        3.2,
        false,
    );
    p.sign_near(-20.0, -32.9, 3.3, 2.6, Sign::H24Recording);
    for k in 0..4 {
        models::put(
            p,
            &models::BOX_SMALL,
            v(-12.0, 0.0, -29.0 + k as f32 * 1.3),
            0.0,
            1.5,
            true,
        );
    }
    // South cell: the green room. Sofa, snacks, a mirror of bulbs and a single banana.
    lounge(p, 6.0, 31.0, PI, true);
    for k in 0..5 {
        p.boxed(
            v(-1.2 + k as f32 * 0.7, 2.4, 32.9),
            v(0.2, 0.2, 0.05),
            Mat::Candle,
            false,
        );
    }
    p.prop(&TABLE_CLOTH, v(12.0, 0.0, 31.5), PI, true);
    leftovers(p, 12.0, 0.8, 31.5, 2);
    p.sign_near(8.0, 32.9, 3.0, 2.6, Sign::H24Mentor);
    models::put(p, &models::ITEM_BOX, v(-4.0, 0.0, 30.0), 0.2, 2.0, true);
    sad_plant(p, 14.5, 24.5);

    // The pod: the stage. A black deck, footlights, the DEMO DAY banner above a wall of screens,
    // one of them mirrored and wrong, the projector on its truss and the judges' table.
    p.slab(v(42.0, 0.012, 0.0), 0.0, v(25.0, 0.02, 23.0), Mat::Fabric0);
    for k in 0..20 {
        p.boxed(
            v(30.4, 0.06, -11.0 + k as f32 * 1.16),
            v(0.2, 0.12, 0.3),
            Mat::NeonPink,
            false,
        );
    }
    if let Some(wl) = wall_screen(p, 54.9, 0.0, 2.4, 12.0, 3.2) {
        screen_sign(p, &wl, 0.0, 2.4, 9.0, Sign::H24DemoDay);
        wl.put(p, -5.4, 2.4, 0.14, v(0.4, 2.8, 0.04), Mat::NeonPink);
        wl.put(p, 5.4, 2.4, 0.14, v(0.4, 2.8, 0.04), Mat::NeonPink);
    }
    // The mirrored display: the demo, backwards, on a second screen.
    p.poster_near(54.9, -9.0, 2.3, Vec2::new(3.6, 2.4), Tex::DemoScreen);
    p.poster_near(54.9, 9.0, 2.3, Vec2::new(3.6, 2.4), Tex::DemoScreen);
    curtains(p, 54.9, -11.4, 4);
    curtains(p, 54.9, 9.6, 4);
    // The projector: on a truss over the front of the stage, a lens, a dust beam.
    p.boxed(v(36.0, 3.9, 0.0), v(0.3, 0.3, 9.0), Mat::Metal, false);
    p.boxed(v(36.0, 3.55, 0.0), v(0.8, 0.4, 0.9), Mat::Dark, false);
    p.prim(
        PrimKind::Cone,
        v(37.0, 3.2, 0.0),
        v(0.6, 1.0, 0.6),
        Mat::Candle,
    );
    p.sign_near(30.0, 11.9, 3.1, 3.0, Sign::H24Recording);
    // Judges' table: a bell, one banana, score cards, three chairs behind it.
    p.prop(&TABLE_CROSS_CLOTH, v(48.0, 0.0, 9.0), PI, true);
    models::put(p, &models::BANANA, v(48.4, 0.8, 9.0), 0.4, 0.7, false);
    p.prim(
        PrimKind::Sphere,
        v(47.4, 0.84, 9.0),
        v(0.18, 0.12, 0.18),
        Mat::Gold,
    );
    p.boxed(v(48.0, 0.78, 8.7), v(0.5, 0.01, 0.3), Mat::Paper, false);
    for k in 0..3 {
        p.prop(&CHAIR_DESK, v(46.8 + k as f32 * 1.2, 0.0, 10.4), PI, false);
    }
    p.sign_near(48.0, 11.9, 3.2, 2.6, Sign::H24Judges);
    tags(p, &[("prop:hack_banana", 48.0, 7.7)]);
    speaker_stack(p, 52.0, -9.5, -FRAC_PI_2);
    speaker_stack(p, 52.0, 7.0, -FRAC_PI_2);
    for z in [-8.5, 5.0] {
        p.boxed(v(33.0, 0.5, z), v(0.8, 1.0, 0.8), Mat::Metal, true);
        p.boxed(v(33.0, 1.05, z), v(0.6, 0.1, 0.6), Mat::Dark, false);
    }
    // Stage rigging: truss towers with lamps at the sides, floor monitors, confetti cannons.
    for z in [-11.3, 11.3] {
        for x in [31.0, 40.0, 50.0] {
            p.boxed(v(x, 2.1, z), v(0.3, 4.2, 0.3), Mat::Metal, false);
            p.boxed(v(x, 3.9, z * 0.92), v(0.5, 0.35, 0.5), Mat::Dark, false);
            p.boxed(
                v(x, 3.9, z * 0.92 - z.signum() * 0.28),
                v(0.3, 0.25, 0.06),
                Mat::Candle,
                false,
            );
        }
    }
    for z in [-5.0, 5.0] {
        p.boxed(v(33.5, 0.2, z), v(0.9, 0.4, 0.5), Mat::Dark, false);
        p.boxed(v(33.5, 0.42, z), v(0.8, 0.04, 0.4), Mat::Metal, false);
    }
    for (x, z) in [(32.0, -9.5), (32.0, 9.5)] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.5, z),
            v(0.3, 1.0, 0.3),
            Mat::NeonPink,
        );
        p.prim(
            PrimKind::Cone,
            v(x, 1.2, z),
            v(0.4, 0.4, 0.4),
            Mat::NeonYellow,
        );
    }
    floor_cables(p, &[(33.0, -5.0), (40.0, -6.0), (54.0, -4.0)]);
    floor_cables(p, &[(33.0, 5.0), (40.0, 6.0), (54.0, 4.0)]);
    neon_strings(
        p,
        &[((30.0, -11.0), (54.0, -11.0)), ((30.0, 11.0), (54.0, 11.0))],
    );
    p.light(v(40.0, 3.6, -6.0), Color::srgb(1.0, 0.4, 0.7), 300_000.0);
    p.light(v(40.0, 3.6, 6.0), Color::srgb(0.5, 0.7, 1.0), 300_000.0);

    neon_lights(
        p,
        &[
            (-34.0, -8.0),
            (-18.0, 10.0),
            (0.0, -10.0),
            (14.0, 10.0),
            (36.0, -4.0),
            (50.0, 4.0),
        ],
        260_000.0,
    );
    strips(p, Mat::LightPanel, Mat::NeonRed, 6.0, 0.35);
    lamps(
        p,
        Color::srgb(1.0, 0.65, 0.8),
        &[
            (-44.0, 0.0),
            (-28.0, 0.0),
            (-12.0, 6.0),
            (0.0, -6.0),
            (12.0, 0.0),
            (-20.0, -26.0),
            (6.0, 26.0),
            (36.0, -6.0),
            (50.0, 0.0),
        ],
        410_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:hack24/2", (-50.0, 0.0), Sign::H24ToFloor),
                ("door:hack24/4", (55.0, 6.0), Sign::H24ToPitch),
            ],
            packs: &STAGE.cell_packs(),
            boss: GREMLIN_AT,
            npc: None,
        },
    );
    theme(3)
}

// ------------------------------------------------------------------ hack24/4: the pitch room

const PITCH: Shape = Shape {
    arrival_x: -44.0,
    hall: (-6.0, 22.0, 16.0),
    cells: [(-16.0, -28.0), (8.0, 28.0)],
    pod: (40.0, 14.0, 12.0),
    pod_wall: Mat::Glass,
};

const JUDGE_AT: (f32, f32) = (40.0, 0.0);

/// The fish tank: a glass box, water, gravel, one very large fish that has raised a round.
fn fish_tank(p: &mut Plan, x: f32, z: f32, w: f32) {
    p.boxed(v(x, 0.5, z), v(w, 1.0, 0.9), Mat::Dark, true);
    p.boxed(v(x, 1.6, z), v(w, 1.3, 0.8), Mat::Water, false);
    p.boxed(v(x, 1.6, z), v(w + 0.1, 1.4, 0.9), Mat::Glass, false);
    p.boxed(v(x, 1.04, z), v(w - 0.1, 0.08, 0.7), Mat::Sand, false);
    p.boxed(v(x, 2.35, z), v(w + 0.1, 0.12, 0.95), Mat::Dark, false);
    p.light(v(x, 2.2, z + 0.5), Color::srgb(0.5, 0.9, 1.0), 40_000.0);
    p.prim(
        PrimKind::Sphere,
        v(x - 0.4, 1.7, z),
        v(0.8, 0.45, 0.3),
        Mat::NeonYellow,
    );
    p.prim(
        PrimKind::Cone,
        v(x - 0.95, 1.7, z),
        v(0.3, 0.4, 0.2),
        Mat::Hot,
    );
    p.prim(
        PrimKind::Sphere,
        v(x + 0.6, 1.4, z),
        v(0.15, 0.1, 0.1),
        Mat::NeonCyan,
    );
    for k in 0..3 {
        p.prim(
            PrimKind::Cone,
            v(x - w / 2.0 + 0.4 + k as f32 * 0.25, 1.35, z + 0.2),
            v(0.2, 0.6, 0.2),
            Mat::Plant,
        );
    }
}

/// A clock with only the number 10 on it: glass, a thin red second hand that never moves.
fn clock_ten(p: &mut Plan, x: f32, z: f32, y: f32) {
    let Some(wl) = WallSpot::at(p, x, z) else {
        return;
    };
    wl.put(p, 0.0, y, 0.03, v(1.8, 1.8, 0.02), Mat::Dark);
    wl.sign(p, 0.0, y, 1.5, Sign::H24Ten);
    wl.put(p, 0.0, y - 1.05, 0.08, v(0.9, 0.06, 0.04), Mat::NeonYellow);
}

fn pitch_room(p: &mut Plan) -> Theme {
    PITCH.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Trim, Mat::NeonCyan, Mat::Dark);
    cove(p, Mat::NeonCyan);

    runner(
        p,
        &[pt(-46.0, 0.0), pt(-30.0, 0.0), pt(-6.0, 0.0), pt(40.0, 0.0)],
        2.4,
        Mat::Dark,
        Mat::NeonCyan,
    );
    for x in [-26.0, -10.0, 6.0, 14.0] {
        pilaster(p, x, -15.9, Mat::Trim);
        pilaster(p, x, 15.9, Mat::Trim);
    }
    // The waiting area: a founder on a sofa, a Series Z banner, the wall of logos of the dead.
    banner(p, -22.0, -15.9, 5.4, 2.6, Tex::AiTransformation, 2.17);
    p.sign_near(-8.0, -15.9, 2.9, 3.6, Sign::H24Series);
    neon_ai(p, 4.0, -15.9, 2.9, 2.6);
    p.sign_near(12.0, -15.9, 2.6, 2.4, Sign::H24Leader);
    lounge(p, -17.0, -10.5, 0.0, true);
    p.prop(&LOUNGE_SOFA_CORNER, v(-4.0, 0.0, -13.0), 0.0, true);
    p.prop(
        &models::COFFEE_TABLE_GLASS_SQ,
        v(-1.8, 0.0, -12.0),
        0.0,
        true,
    );
    models::put(p, &models::COIN_GOLD, v(-1.8, 0.8, -12.0), 0.3, 2.0, false);
    prize_box(p, 8.0, -13.0);
    prize_box(p, 10.2, -13.4);
    for k in 0..3 {
        p.prop(
            &CHAIR_MODERN,
            v(-26.0 + k as f32 * 1.2, 0.0, -13.5),
            0.0,
            false,
        );
    }
    p.sign_near(-26.0, -15.9, 2.3, 2.4, Sign::H24Mvp);
    // South wall: the pitch deck printer, a vending machine of cold brew, leaflets and sofas.
    banner(p, -20.0, 15.9, 4.6, 2.6, Tex::Roadmap, 1.3);
    banner(p, -8.0, 15.9, 4.2, 2.6, Tex::CloseDeals, 1.3);
    sticky_wall(p, 6.0, 15.9, 2.3, 12, 5, 6);
    lounge(p, -26.0, 11.0, PI, true);
    p.prop(&LOUNGE_SOFA_CORNER, v(-14.0, 0.0, 13.0), PI, true);
    vending(p, 14.0, 14.8, Face::NegZ);
    for x in [-4.0, -1.5] {
        p.prop(&models::BENCH_CUSHION_LOW, v(x, 0.0, 12.8), PI, true);
    }
    p.prop(&models::BENCH, v(1.0, 0.0, 12.8), PI, true);
    p.prop(&models::FRIDGE_SMALL, v(16.0, 0.0, 14.8), PI, true);
    p.sign_near(15.0, 15.9, 3.4, 2.4, Sign::H24Fridge);
    neon_strings(
        p,
        &[
            ((-28.0, -14.5), (16.0, -14.5)),
            ((-28.0, 14.5), (16.0, 14.5)),
            ((-28.0, -7.5), (16.0, -7.5)),
            ((-28.0, 7.5), (16.0, 7.5)),
        ],
    );
    for (k, (x, z)) in [(-24.0, 6.0), (-10.0, 6.5), (4.0, 7.0), (2.0, -8.5)]
        .into_iter()
        .enumerate()
    {
        hack_nook(p, x, z, 600 + k as i32);
    }
    floor_cables(p, &[(-28.0, 4.0), (-14.0, 3.8), (0.0, 4.6), (14.0, 3.6)]);
    greenery(p, -29.0, -4.5);
    greenery(p, -29.0, 4.5);
    p.sign_near(-27.0, 15.9, 3.2, 2.6, Sign::H24Welcome);

    // North cell: the "Portfolio" room. A long table of dead start-up trophies.
    p.prop(&TABLE_CROSS_CLOTH, v(-16.0, 0.0, -33.0), 0.0, true);
    for k in 0..6 {
        models::put(
            p,
            &models::COIN_GOLD,
            v(-17.2 + k as f32 * 0.5, 0.8, -33.0),
            0.5 * k as f32,
            1.6,
            false,
        );
    }
    for k in 0..4 {
        p.prop(
            &BOOKCASE_CLOSED_WIDE,
            v(-21.0 + k as f32 * 1.7, 0.0, -34.8),
            0.0,
            true,
        );
    }
    prize_box(p, -9.5, -33.5);
    p.sign_near(-16.0, -34.9, 2.9, 2.6, Sign::H24Series);
    // South cell: the other waiting room: more founders, more sofas, more water.
    lounge(p, 8.0, 33.0, PI, true);
    beanbag(p, 14.0, 28.0, Mat::BeanB);
    beanbag(p, 14.0, 30.0, Mat::BeanC);
    sleeping_bag(p, 2.5, 28.0, 0.7, 7);
    p.sign_near(8.0, 34.9, 3.0, 2.6, Sign::H24Quiet);

    // The pod: the pitch room proper. A glass room, a fish tank, the banner, the clock with only
    // a 10, the judge's chair on a rug and the sofa where the founders sit.
    p.prop(&models::RUG_ROUNDED, v(48.0, 0.0, 0.0), FRAC_PI_2, false);
    round_patch(p, 40.0, 0.0, 9.0, Mat::Metal);
    p.prim(
        PrimKind::Cylinder,
        v(40.0, 0.02, 0.0),
        v(8.4, 0.02, 8.4),
        Mat::NeonCyan,
    );
    p.prim(
        PrimKind::Cylinder,
        v(40.0, 0.03, 0.0),
        v(8.0, 0.02, 8.0),
        Mat::Trim,
    );
    floor_ring(p, 40.0, 0.0, 7.0, 36, Mat::NeonCyan, 0.12);
    fish_tank(p, 36.0, -11.4, 5.0);
    tags(p, &[("prop:hack_fishtank", 36.0, -9.7)]);
    p.sign_near(44.0, -11.9, 3.2, 3.6, Sign::H24Series);
    clock_ten(p, 53.9, -5.0, 2.8);
    banner(p, 53.9, 5.0, 4.6, 2.6, Tex::AiFirst, 1.5);
    p.prop(&LOUNGE_DESIGN_SOFA, v(30.0, 0.0, 9.8), PI, true);
    p.prop(&LOUNGE_DESIGN_SOFA, v(33.5, 0.0, 9.8), PI, true);
    p.prop(&COFFEE_TABLE_SQUARE, v(32.0, 0.0, 7.6), 0.0, true);
    models::put(p, &models::COIN_GOLD, v(32.0, 0.8, 7.6), 0.3, 1.6, false);
    p.prop(&CHAIR_ROUNDED, v(52.0, 0.0, 9.5), PI, false);
    p.prop(&TABLE_ROUND, v(50.0, 0.0, 9.5), 0.0, true);
    models::put(p, &models::MUG, v(50.0, 0.8, 9.4), 0.0, 0.5, false);
    // The portfolio shelf, round tables for founders, a pitch easel and a few very tall plants.
    for k in 0..4 {
        p.prop(
            &BOOKCASE_CLOSED_WIDE,
            v(41.0 + k as f32 * 1.7, 0.0, -11.7),
            0.0,
            true,
        );
        models::put(
            p,
            &models::COIN_GOLD,
            v(41.0 + k as f32 * 1.7, 1.9, -11.5),
            0.4,
            1.4,
            false,
        );
    }
    prize_box(p, 52.0, -8.0);
    for (x, z) in [(46.0, -8.5), (30.0, -8.5)] {
        p.prop(&TABLE_ROUND, v(x, 0.0, z), 0.0, true);
        models::put(p, &models::PLATE, v(x, 0.9, z), 0.0, 0.5, false);
        models::put(p, &models::DONUT_CHOC, v(x, 0.95, z), 0.0, 0.8, false);
        for a in [0.6f32, 2.8, 4.9] {
            p.prop(
                &CHAIR_MODERN,
                v(x + a.cos() * 1.3, 0.0, z + a.sin() * 1.3),
                a + PI,
                false,
            );
        }
    }
    for (x, z) in [(27.0, -11.0), (27.0, 11.0), (53.0, -11.0), (53.0, 11.0)] {
        p.prop(&POTTED_PLANT, v(x, 0.0, z), 0.0, false);
    }
    p.boxed(v(45.0, 0.9, 6.5), v(0.08, 1.8, 0.08), Mat::Metal, false);
    p.boxed(v(46.2, 0.9, 6.5), v(0.08, 1.8, 0.08), Mat::Metal, false);
    p.boxed(v(45.6, 1.5, 6.4), v(1.5, 1.0, 0.05), Mat::White, false);
    p.sign(v(45.6, 1.5, 6.36), 0.0, 1.4, Sign::H24Whiteboard);
    for z in [-4.0, 4.0] {
        p.prim(
            PrimKind::Cylinder,
            v(34.5, 0.8, z),
            v(0.05, 1.6, 0.05),
            Mat::Metal,
        );
        p.prim(
            PrimKind::Sphere,
            v(34.5, 1.65, z),
            v(0.14, 0.18, 0.14),
            Mat::Dark,
        );
    }
    neon_strings(
        p,
        &[((28.0, -12.0), (52.0, -12.0)), ((28.0, 12.0), (52.0, 12.0))],
    );
    p.light(v(40.0, 3.7, 0.0), Color::srgb(0.6, 0.95, 1.0), 400_000.0);
    p.light(v(48.0, 3.7, -8.0), Color::srgb(0.4, 0.9, 0.7), 200_000.0);
    p.sign(v(25.1, 3.3, -6.0), FRAC_PI_2, 2.2, Sign::H24Series);
    floor_patch(p, 30.0, 0.0, 0.0, 3.0, 22.0, Mat::Trim);

    neon_lights(
        p,
        &[
            (-34.0, 8.0),
            (-18.0, -10.0),
            (0.0, 10.0),
            (14.0, -10.0),
            (34.0, -6.0),
            (48.0, 6.0),
        ],
        260_000.0,
    );
    strips(p, Mat::LightPanel, Mat::NeonCyan, 6.0, 0.3);
    lamps(
        p,
        Color::srgb(0.7, 0.9, 1.0),
        &[
            (-44.0, 0.0),
            (-26.0, 0.0),
            (-6.0, -9.0),
            (-6.0, 9.0),
            (10.0, 0.0),
            (-16.0, -28.0),
            (8.0, 28.0),
            (36.0, 0.0),
        ],
        420_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:hack24/3", (-50.0, 0.0), Sign::H24ToStage),
                ("door:campus", (54.0, -6.0), Sign::H24ToDay),
            ],
            packs: &PITCH.cell_packs(),
            boss: JUDGE_AT,
            // 13 m from the boss and by the exit door, so the turn-in never pulls it.
            npc: Some((50.0, -9.0)),
        },
    );
    theme(4)
}
