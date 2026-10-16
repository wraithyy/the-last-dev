//! The All-Hands Offsite raid: a hotel conference resort in four rooms. offsite/1 is the lobby
//! with the icebreaker stage, offsite/2 the breakout rooms around a central hall, offsite/3 an
//! open lakeside terrace with the keynote stage, offsite/4 the grand ballroom. Everything is boxes
//! and primitives (same budget as the lab) so it stays light on WebGL2.

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::props::{beanbag, long_table, lounge};
use super::shape::{Seg, blob, rounded_rect, stroke};
use super::themes::{ceiling_lights, pt, runner, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    if matches!(level, 1 | 4) {
        p.ceiling = HALL_H;
    }
    let (zone, name, outdoor, theme) = match level {
        1 => (
            "offsite/1",
            "All-Hands Offsite: Lobby",
            false,
            lobby(&mut p),
        ),
        2 => (
            "offsite/2",
            "All-Hands Offsite: Breakout Rooms",
            false,
            breakout(&mut p),
        ),
        3 => (
            "offsite/3",
            "All-Hands Offsite: Terrace",
            true,
            terrace(&mut p),
        ),
        _ => (
            "offsite/4",
            "All-Hands Offsite: Grand Ballroom",
            false,
            ballroom(&mut p),
        ),
    };
    assemble(zone, level, name, outdoor, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    match level {
        1 => Theme {
            wall: c(0.93, 0.87, 0.76),
            trim: c(0.3, 0.2, 0.14),
            accent: LinearRgba::rgb(3.0, 1.7, 0.5),
            fabric: [c(0.55, 0.12, 0.2), c(0.12, 0.42, 0.45), c(0.85, 0.62, 0.2)],
            floor: FloorKind::Carpet([1.7, 1.3, 1.0]),
            fog: c(0.2, 0.15, 0.12),
            ambient: c(1.0, 0.92, 0.8),
            ambient_brightness: 620.0,
            sun: c(1.0, 0.92, 0.8),
            sun_illuminance: 7000.0,
            fog_range: (40.0, 110.0),
        },
        2 => Theme {
            wall: c(0.9, 0.94, 0.95),
            trim: c(0.25, 0.4, 0.42),
            accent: LinearRgba::rgb(0.3, 2.4, 1.8),
            fabric: [c(0.15, 0.6, 0.55), c(0.95, 0.5, 0.25), c(0.55, 0.4, 0.8)],
            floor: FloorKind::Carpet([0.8, 0.95, 0.95]),
            fog: c(0.5, 0.6, 0.62),
            ambient: c(0.92, 0.98, 1.0),
            ambient_brightness: 650.0,
            sun: c(1.0, 0.98, 0.95),
            sun_illuminance: 8000.0,
            fog_range: (45.0, 120.0),
        },
        3 => Theme {
            wall: c(0.9, 0.86, 0.78),
            trim: c(0.3, 0.2, 0.13),
            accent: LinearRgba::rgb(0.3, 2.2, 3.0),
            fabric: [c(0.12, 0.35, 0.5), c(0.85, 0.55, 0.25), c(0.9, 0.85, 0.7)],
            floor: FloorKind::Gloss([0.92, 0.86, 0.74]),
            fog: c(0.74, 0.84, 0.93),
            ambient: c(0.9, 0.92, 1.0),
            ambient_brightness: 800.0,
            sun: c(1.0, 0.84, 0.62),
            sun_illuminance: 13000.0,
            fog_range: (70.0, 210.0),
        },
        _ => Theme {
            wall: c(0.36, 0.14, 0.2),
            trim: c(0.78, 0.6, 0.25),
            accent: LinearRgba::rgb(3.2, 2.2, 0.6),
            fabric: [c(0.5, 0.1, 0.18), c(0.12, 0.14, 0.3), c(0.8, 0.6, 0.2)],
            floor: FloorKind::Marble,
            fog: c(0.07, 0.04, 0.06),
            ambient: c(1.0, 0.85, 0.7),
            ambient_brightness: 560.0,
            sun: c(1.0, 0.85, 0.7),
            sun_illuminance: 3500.0,
            fog_range: (40.0, 110.0),
        },
    }
}

// ------------------------------------------------------------------ raid dressing kit
// Shared by the other raid and minigame venues (vibecon, teambuilding, corporate), so every
// room gets the same wall, light and furniture vocabulary as the tower instead of bare boxes.

/// Band along every opaque wall (dado, rail, crown, cove light). Insets above 0.03 stay clear of
/// the trim and accent runs `build_walls` already lays at 0.02.
pub(super) fn wall_band(p: &mut Plan, mat: Mat, y0: f32, y1: f32, inset: f32) {
    let segs: Vec<Seg> = p
        .walls
        .iter()
        .filter(|s| s.mat != Mat::Glass)
        .copied()
        .collect();
    if !segs.is_empty() {
        p.pieces.push(Piece::Run {
            segs,
            mat,
            y0,
            y1,
            inset,
        });
    }
}

/// Dado panelling, a rail on top and a crown moulding: the walls stop being one flat colour.
pub(super) fn wall_dress(p: &mut Plan, dado: Mat, rail: Mat, crown: Mat) {
    wall_band(p, dado, 0.2, 1.05, 0.04);
    wall_band(p, rail, 1.05, 1.13, 0.06);
    let top = p.top();
    wall_band(p, crown, top - 0.32, top, 0.06);
}

/// Glowing cove strip under the crown: cheap mood lighting that costs no real light.
pub(super) fn cove(p: &mut Plan, mat: Mat) {
    let top = p.top();
    wall_band(p, mat, top - 0.42, top - 0.36, 0.09);
}

/// Flat pilaster against the nearest wall, gold base and capital.
pub(super) fn pilaster(p: &mut Plan, x: f32, z: f32, mat: Mat) {
    let Some((at, n)) = p.wall_near(x, z) else {
        return;
    };
    let yaw = n.x.atan2(n.y);
    let c = at + n * 0.17;
    let top = p.top();
    p.framed(v(c.x, 0.0, c.y), yaw, |p| {
        p.boxed(v(0.0, top / 2.0, 0.0), v(0.7, top, 0.34), mat, false);
        p.boxed(v(0.0, 0.18, 0.04), v(0.9, 0.36, 0.42), Mat::Gold, false);
        p.boxed(v(0.0, top - 0.4, 0.04), v(0.9, 0.3, 0.42), Mat::Gold, false);
    });
}

/// Furniture flush against the nearest wall, front facing into the room.
pub(super) fn wall_prop(p: &mut Plan, model: &'static Model, x: f32, z: f32, solid: bool) {
    let Some((at, n)) = p.wall_near(x, z) else {
        return;
    };
    let c = at + n * (model.size[2] * FURNITURE_SCALE / 2.0 + 0.06);
    p.prop(model, v(c.x, 0.0, c.y), n.x.atan2(n.y), solid);
}

/// Chair at (x, z) turned to face (tx, tz). Visual and seat only, never a blocker.
pub(super) fn chair_to(p: &mut Plan, x: f32, z: f32, tx: f32, tz: f32) {
    p.prop(&CHAIR_MODERN, v(x, 0.0, z), (tx - x).atan2(tz - z), false);
}

/// `n` chairs on a circle round (cx, cz), all facing the middle.
pub(super) fn ring_chairs(p: &mut Plan, cx: f32, cz: f32, r: f32, n: usize, phase: f32) {
    for k in 0..n {
        let a = phase + k as f32 * TAU / n as f32;
        chair_to(p, cx + r * a.cos(), cz + r * a.sin(), cx, cz);
    }
}

/// Flat colour patch on the carpet (rug, dance floor, lawn path); below the runners' 0.02.
pub(super) fn floor_patch(p: &mut Plan, x: f32, z: f32, yaw: f32, w: f32, d: f32, mat: Mat) {
    p.slab(v(x, 0.005, z), yaw, v(w, 0.01, d), mat);
}

pub(super) fn round_patch(p: &mut Plan, x: f32, z: f32, dia: f32, mat: Mat) {
    p.prim(PrimKind::Cylinder, v(x, 0.005, z), v(dia, 0.01, dia), mat);
}

/// Bunting between two points: a visible string sagging between ceiling hooks, with small
/// pennants hanging off it. `y` is the string height at the hooks.
pub(super) fn bunting(p: &mut Plan, a: (f32, f32), b: (f32, f32), y: f32, flags: [Mat; 3]) {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dz * dz).sqrt();
    let yaw = (-dz).atan2(dx);
    let sag_at = |t: f32| 0.35 * (t * PI).sin();
    // Short level segments approximate the curve (slabs cannot pitch); the slope stays tiny.
    let segs = ((len / 0.4) as usize).max(1);
    for k in 0..segs {
        let t = (k as f32 + 0.5) / segs as f32;
        p.slab(
            v(a.0 + dx * t, y - sag_at(t), a.1 + dz * t),
            yaw,
            v(len / segs as f32 + 0.02, 0.03, 0.03),
            Mat::Dark,
        );
    }
    let top = p.top();
    // Hooks reach the ceiling when the string hangs under one; outdoors it ties to a pole.
    let (lo, hi) = if y >= top - 0.7 { (y, top) } else { (0.0, y) };
    for (x, z) in [a, b] {
        p.boxed(
            v(x, (lo + hi) / 2.0, z),
            v(0.04, hi - lo, 0.04),
            Mat::Dark,
            false,
        );
    }
    let n = ((len / 0.7) as usize).max(1);
    for k in 0..=n {
        let t = k as f32 / n as f32;
        p.prim_yawed(
            PrimKind::Pennant,
            v(a.0 + dx * t, y - sag_at(t) - 0.22, a.1 + dz * t),
            v(0.3, 0.42, 1.0),
            yaw,
            flags[k % 3],
        );
    }
}

/// Bunch of balloons on strings tied to a weight.
pub(super) fn balloons(p: &mut Plan, x: f32, z: f32, mats: [Mat; 3]) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.06, z),
        v(0.3, 0.12, 0.3),
        Mat::Dark,
    );
    for (k, (dx, dz, h)) in [(0.0, 0.0, 2.3), (0.3, 0.1, 2.0), (-0.25, 0.2, 2.15)]
        .into_iter()
        .enumerate()
    {
        p.boxed(
            v(x + dx / 2.0, h / 2.0, z + dz / 2.0),
            v(0.012, h, 0.012),
            Mat::Paper,
            false,
        );
        p.prim(
            PrimKind::Sphere,
            v(x + dx, h + 0.25, z + dz),
            v(0.5, 0.6, 0.5),
            mats[k % 3],
        );
    }
}

/// Dark crossbeams under the ceiling, only where the footprint holds them, so a flat tile lid
/// reads as an architectural ceiling. Visual only.
pub(super) fn beams(p: &mut Plan, x0: f32, x1: f32, z0: f32, z1: f32, step: f32, mat: Mat) {
    let top = p.top();
    let mut x = x0;
    while x <= x1 {
        let mut z = z0;
        while z < z1 {
            let (a, b) = (pt(x, z), pt(x, z + 2.0));
            if p.fp.contains(a) && p.fp.contains(b) && p.fp.contains(pt(x, z + 1.0)) {
                p.boxed(v(x, top - 0.16, z + 1.0), v(0.28, 0.28, 2.0), mat, false);
            }
            z += 2.0;
        }
        x += step;
    }
}

/// Stepped round dais: marble body, gold ring, optional inlay.
pub(super) fn dais(p: &mut Plan, x: f32, z: f32, dia: f32, inlay: Mat) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.07, z),
        v(dia + 0.5, 0.14, dia + 0.5),
        Mat::Gold,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.09, z),
        v(dia, 0.18, dia),
        Mat::MarbleDark,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.185, z),
        v(dia * 0.72, 0.01, dia * 0.72),
        inlay,
    );
}

/// Tall pot plant with a bushier cousin beside it.
pub(super) fn plant_pair(p: &mut Plan, x: f32, z: f32) {
    p.prop(&POTTED_PLANT, v(x, 0.0, z), 0.0, true);
    p.prop_scaled(&POTTED_PLANT, v(x + 0.55, 0.0, z + 0.2), 1.3, false, 0.8);
}

/// Piano-style black upright: body, lid, bench. Solid.
pub(super) fn grand_piano(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 0.95, 0.0), v(2.8, 0.22, 1.7), Mat::Dark, false);
        for (sx, sz) in [(-1.2, -0.65), (1.2, -0.65), (-1.2, 0.65)] {
            p.boxed(v(sx, 0.42, sz), v(0.12, 0.84, 0.12), Mat::Dark, false);
        }
        p.boxed(v(-0.2, 1.1, 0.0), v(2.4, 0.04, 1.5), Mat::Chrome, false);
        p.boxed(v(0.0, 1.02, 0.85), v(1.6, 0.06, 0.3), Mat::White, false);
        p.boxed(v(0.0, 0.5, 1.7), v(1.3, 0.1, 0.5), Mat::Wood, false);
    });
    p.block(v(x, 0.75, z), v(2.8, 1.5, 1.8));
}

// ------------------------------------------------------------------ shared props

/// Banquet table: cloth drum, centrepiece candle and six chairs.
fn round_table(p: &mut Plan, x: f32, z: f32, cloth: Mat, chair: Mat) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.38, z),
        v(2.6, 0.76, 2.6),
        Mat::White,
    );
    p.prim(PrimKind::Cylinder, v(x, 0.78, z), v(2.8, 0.05, 2.8), cloth);
    p.prim(
        PrimKind::Sphere,
        v(x, 1.0, z),
        v(0.3, 0.3, 0.3),
        Mat::Candle,
    );
    p.block(v(x, 0.5, z), v(2.6, 1.0, 2.6));
    ring_chairs(p, x, z, 2.0, 6, 0.3);
    // Place settings: plate, napkin fan in the table's cloth colour.
    for k in 0..6 {
        let a = k as f32 * TAU / 6.0 + 0.3;
        p.prim(
            PrimKind::Cylinder,
            v(x + 1.0 * a.cos(), 0.81, z + 1.0 * a.sin()),
            v(0.32, 0.015, 0.32),
            Mat::White,
        );
        p.slab(
            v(x + 1.0 * a.cos(), 0.84, z + 1.0 * a.sin()),
            -a,
            v(0.1, 0.02, 0.2),
            chair,
        );
    }
}

fn chandelier(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 4.15, z), v(0.05, 0.7, 0.05), Mat::Metal, false);
    p.prim(
        PrimKind::Cylinder,
        v(x, 3.7, z),
        v(2.4, 0.12, 2.4),
        Mat::Gold,
    );
    p.prim(
        PrimKind::Sphere,
        v(x, 3.45, z),
        v(0.8, 0.8, 0.8),
        Mat::Candle,
    );
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + 0.4;
        p.prim(
            PrimKind::Sphere,
            v(x + 1.2 * a.cos(), 3.88, z + 1.2 * a.sin()),
            v(0.2, 0.2, 0.2),
            Mat::Candle,
        );
    }
}

fn flipchart(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 1.45, 0.0), v(0.95, 1.2, 0.05), Mat::Board, false);
        p.panel(
            v(0.0, 1.45, 0.04),
            0.0,
            Vec2::new(0.85, 1.05),
            Tex::Stickies,
        );
        for sx in [-0.35, 0.35] {
            p.boxed(v(sx, 0.4, -0.1), v(0.04, 0.8, 0.04), Mat::Metal, false);
        }
        p.boxed(v(0.0, 0.8, -0.05), v(0.9, 0.04, 0.2), Mat::Metal, false);
    });
    p.block(v(x, 0.9, z), v(1.0, 1.8, 0.4));
}

fn plant(p: &mut Plan, x: f32, z: f32) {
    p.prop(&POTTED_PLANT, v(x, 0.0, z), 0.0, true);
}

fn swag_stack(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.4, z), v(1.4, 0.8, 1.0), Mat::Cardboard, true);
    p.boxed(v(x + 0.1, 1.0, z), v(1.0, 0.4, 0.8), Mat::Cardboard, false);
    p.boxed(v(x - 0.3, 1.3, z), v(0.5, 0.2, 0.5), Mat::BeanA, false);
}

/// Desk with a marble top; the front looks towards +Z.
fn desk(p: &mut Plan, x: f32, z: f32, len: f32) {
    p.boxed(v(x, 0.55, z), v(len, 1.1, 1.0), Mat::Wood, true);
    p.boxed(v(x, 1.12, z), v(len + 0.2, 0.06, 1.2), Mat::Marble, false);
    p.boxed(
        v(x, 1.4, z - 0.45),
        v(len - 0.4, 0.04, 0.04),
        Mat::Gold,
        false,
    );
}

/// Wooden posts with a sagging string of warm bulbs between each pair (along x).
fn string_lights(p: &mut Plan, x0: f32, x1: f32, z: f32) {
    let n = ((x1 - x0) / 7.0).round().max(1.0) as usize;
    for k in 0..=n {
        let x = x0 + (x1 - x0) * k as f32 / n as f32;
        p.boxed(v(x, 1.7, z), v(0.14, 3.4, 0.14), Mat::Wood, false);
    }
    let bulbs = ((x1 - x0) / 1.4) as usize;
    for k in 0..=bulbs {
        let t = k as f32 / bulbs as f32;
        let x = x0 + (x1 - x0) * t;
        // Catenary-ish sag between posts.
        let seg = (t * n as f32).fract();
        let y = 3.3 - 0.45 * (seg * PI).sin();
        p.prim(PrimKind::Sphere, v(x, y, z), v(0.2, 0.2, 0.2), Mat::Candle);
    }
    p.boxed(
        v((x0 + x1) / 2.0, 3.32, z),
        v(x1 - x0, 0.02, 0.02),
        Mat::Dark,
        false,
    );
}

// ------------------------------------------------------------------ offsite/1: lobby

fn lobby(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    // Revolving-door vestibule, the main lobby, the boss pod and two side annexes.
    p.fp.pod(rounded_rect(pt(-52.0, 0.0), 7.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-46.0, 0.0), pt(-34.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(-14.0, 0.0), 22.0, 20.0, 9.0, 0.0), w);
    p.fp.link(stroke(&[pt(6.0, 0.0), pt(24.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(36.0, 0.0), 14.0, 14.0, 7.0, 0.0), w);
    p.fp.pod(rounded_rect(pt(-24.0, -32.0), 10.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-24.0, -18.0), pt(-24.0, -28.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(-24.0, 32.0), 10.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-24.0, 18.0), pt(-24.0, 28.0)], 4.0), w);
    p.route = vec![
        pt(-56.0, 0.0),
        pt(-36.0, 0.0),
        pt(-14.0, 0.0),
        pt(24.0, 0.0),
        pt(36.0, 0.0),
    ];
    p.build_walls();
    wall_dress(p, Mat::Wood, Mat::Gold, Mat::Trim);
    cove(p, Mat::Candle);
    lobby_dress(p);

    // Red carpet from the revolving door to the icebreaker stage.
    runner(
        p,
        &[
            pt(-56.0, 0.0),
            pt(-36.0, 0.0),
            pt(-14.0, 0.0),
            pt(24.0, 0.0),
            pt(33.0, 0.0),
        ],
        2.6,
        Mat::Fabric0,
        Mat::Gold,
    );
    // The revolving door: a glass drum with four vanes around the doorway.
    p.prim(
        PrimKind::Cylinder,
        v(-57.6, 1.3, 0.0),
        v(2.6, 2.6, 2.6),
        Mat::Glass,
    );
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + 0.5;
        p.slab(
            v(-57.6 + 0.6 * a.cos(), 1.3, 0.6 * a.sin()),
            -a,
            v(1.2, 2.4, 0.04),
            Mat::Metal,
        );
    }
    p.prim(
        PrimKind::Cylinder,
        v(-57.6, 2.65, 0.0),
        v(2.8, 0.1, 2.8),
        Mat::Gold,
    );
    for z in [-6.5, 6.5] {
        plant(p, -47.5, z);
    }
    p.sign_near(-52.0, -7.8, 2.6, 3.0, Sign::OffsiteAgenda);

    // Reception: desk under the big banner, the concierge in front, name tags beside it.
    p.sign_near(-8.0, -19.8, 3.0, 7.0, Sign::OffsiteWelcome);
    desk(p, -8.0, -17.2, 7.0);
    for x in [-10.5, -5.5] {
        p.boxed(v(x, 1.3, -17.6), v(0.45, 0.3, 0.05), Mat::Dark, false);
        p.boxed(
            v(x, 1.3, -17.57),
            v(0.38, 0.22, 0.02),
            Mat::ScreenGlow,
            false,
        );
    }
    p.boxed(v(-16.0, 0.4, -17.4), v(5.0, 0.8, 0.9), Mat::White, true);
    for k in 0..8 {
        p.boxed(
            v(-17.8 + k as f32 * 0.5, 0.84, -17.3),
            v(0.3, 0.04, 0.2),
            Mat::Paper,
            false,
        );
    }
    p.sign_near(-16.0, -19.8, 2.5, 3.2, Sign::OffsiteNameTags);
    p.sign_near(0.0, -19.8, 2.4, 2.4, Sign::OffsiteAgenda);
    for x in [4.5, -33.0] {
        plant(p, x, -15.0);
    }

    // Lounge sofas on the south side, with plants.
    lounge(p, -2.0, 14.0, PI, true);
    for x in [4.5, -33.0] {
        plant(p, x, 15.0);
    }
    p.sign_near(-8.0, 19.8, 2.8, 5.0, Sign::OffsiteWelcome);

    // Swag room (north) and the coffee lounge (south).
    for (x, z) in [(-31.5, -38.2), (-17.5, -38.2), (-17.5, -26.2)] {
        swag_stack(p, x, z);
    }
    p.sign_near(-24.0, -39.8, 2.6, 3.2, Sign::OffsiteNameTags);
    lounge(p, -29.0, 38.0, PI, true);
    beanbag(p, -18.0, 38.0, Mat::BeanC);
    beanbag(p, -16.5, 35.0, Mat::BeanB);
    p.sign_near(-24.0, 39.8, 2.6, 3.0, Sign::OffsiteBar);

    // Icebreaker stage: a ring of chairs nobody wants to sit in, a gold dais, a lectern.
    dais(p, 38.0, 0.0, 8.0, Mat::Fabric0);
    ring_chairs(p, 38.0, 0.0, 6.2, 10, 0.2);
    for z in [-12.5, 12.5] {
        p.boxed(v(49.0, 2.25, z), v(1.0, 4.5, 3.0), Mat::Fabric0, false);
    }
    p.boxed(v(44.0, 4.05, 0.0), v(0.2, 0.2, 16.0), Mat::Metal, false);
    for k in 0..5 {
        p.boxed(
            v(44.0, 3.9, -6.0 + k as f32 * 3.0),
            v(0.4, 0.2, 0.4),
            Mat::LightPanel,
            false,
        );
    }
    p.boxed(v(46.0, 0.55, 0.0), v(0.8, 1.1, 1.2), Mat::Wood, true);
    p.sign_near(50.0, -6.0, 2.6, 4.0, Sign::OffsiteNameTags);
    for z in [-11.0, 11.0] {
        plant(p, 47.0, z);
    }

    ceiling_lights(p);
    for (x, z) in [
        (-52.0, 0.0),
        (-26.0, -8.0),
        (-26.0, 8.0),
        (-4.0, -8.0),
        (-4.0, 8.0),
        (38.0, 0.0),
        (-24.0, -32.0),
        (-24.0, 32.0),
    ] {
        p.light(v(x, 3.6, z), Color::srgb(1.0, 0.9, 0.74), 420_000.0);
    }
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:campus", (-60.0, 0.0), Sign::ToCampus),
                ("door:offsite/2", (50.0, 8.0), Sign::ToBreakout),
            ],
            packs: &[(-32.0, -11.0), (-32.0, 11.0), (-24.0, -34.0), (-24.0, 34.0)],
            boss: (38.0, 0.0),
            npc: Some((-8.0, -14.4)),
        },
    );
    theme(1)
}

/// Lobby furniture and architecture beyond the first pass: pilasters, a fountain with the
/// synergy ice sculpture, a piano nobody plays, luggage, brochures and festive clutter.
fn lobby_dress(p: &mut Plan) {
    for x in [-33.0, -27.0, -21.0, -2.0, 4.0] {
        pilaster(p, x, -19.9, Mat::Marble);
        pilaster(p, x, 19.9, Mat::Marble);
    }
    for z in [-10.0, 10.0] {
        pilaster(p, 49.9, z, Mat::Marble);
    }
    beams(p, -28.0, 4.0, -20.0, 20.0, 8.0, Mat::Trim);
    // Fountain with the company logo carved in ice, melting on schedule.
    let (fx, fz) = (-14.0, -9.0);
    p.prim(
        PrimKind::Cylinder,
        v(fx, 0.3, fz),
        v(4.6, 0.6, 4.6),
        Mat::Marble,
    );
    p.prim(
        PrimKind::Cylinder,
        v(fx, 0.61, fz),
        v(4.0, 0.03, 4.0),
        Mat::Water,
    );
    p.prim(
        PrimKind::Cylinder,
        v(fx, 0.9, fz),
        v(1.0, 0.6, 1.0),
        Mat::MarbleDark,
    );
    p.prim(PrimKind::Cone, v(fx, 2.0, fz), v(1.0, 2.2, 1.0), Mat::Glass);
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + 0.8;
        p.prim(
            PrimKind::Cone,
            v(fx + 0.9 * a.cos(), 1.2, fz + 0.9 * a.sin()),
            v(0.35, 1.3, 0.35),
            Mat::Glass,
        );
    }
    p.block(v(fx, 0.5, fz), v(4.6, 1.0, 4.6));
    p.boxed(v(fx, 0.35, fz + 3.0), v(0.9, 0.7, 0.3), Mat::Dark, false);
    p.panel(
        v(fx, 0.55, fz + 3.16),
        0.0,
        Vec2::new(0.7, 0.45),
        Tex::Synergy,
    );
    // Piano corner.
    floor_patch(p, -21.0, 12.0, 0.0, 6.0, 4.4, Mat::Fabric1);
    grand_piano(p, -21.0, 12.0, 0.0);
    plant_pair(p, -25.5, 15.5);
    // Reading wall.
    for x in [-30.0, -26.0] {
        wall_prop(p, &BOOKCASE_CLOSED_WIDE, x, 19.9, true);
    }
    p.poster_near(-12.0, 19.9, 2.3, Vec2::new(1.4, 1.8), Tex::Wellness);
    p.poster_near(2.0, 19.9, 2.3, Vec2::new(1.8, 1.4), Tex::Family);
    p.poster_near(-31.0, -19.9, 2.3, Vec2::new(1.6, 1.4), Tex::Standup);
    // Luggage by the revolving door: gold cart, suitcases, one lanyard-tagged cardboard box.
    p.boxed(v(-55.0, 0.15, 5.0), v(1.6, 0.1, 1.0), Mat::Gold, false);
    p.boxed(v(-55.0, 1.2, 5.45), v(1.6, 2.2, 0.05), Mat::Gold, false);
    for (k, m) in [Mat::BeanA, Mat::BeanB, Mat::Cardboard]
        .into_iter()
        .enumerate()
    {
        p.boxed(
            v(-55.6 + k as f32 * 0.6, 0.55, 5.0),
            v(0.5, 0.7, 0.35),
            m,
            false,
        );
    }
    p.block(v(-55.0, 0.6, 5.0), v(1.7, 1.2, 1.1));
    bunting(
        p,
        (-32.0, -6.0),
        (-2.0, -6.0),
        p.top() - 0.6,
        [Mat::Fabric0, Mat::Fabric1, Mat::Gold],
    );
    bunting(
        p,
        (-32.0, 6.0),
        (-2.0, 6.0),
        p.top() - 0.6,
        [Mat::Fabric1, Mat::Gold, Mat::Fabric0],
    );
    // Confetti left over from the last all-hands, round the icebreaker stage.
    for k in 0..28 {
        let (a, r) = (k as f32 * 2.399, 5.0 + (k as f32 * 1.7).sin().abs() * 5.5);
        p.slab(
            v(38.0 + r * a.cos(), 0.025, r * a.sin()),
            a,
            v(0.16, 0.01, 0.1),
            [Mat::NeonPink, Mat::NeonYellow, Mat::NeonCyan, Mat::Gold][k % 4],
        );
    }
    balloons(p, 46.0, -9.0, [Mat::Fabric0, Mat::Gold, Mat::Fabric1]);
    balloons(p, 46.0, 9.0, [Mat::Fabric1, Mat::Fabric0, Mat::Gold]);
}

// ------------------------------------------------------------------ offsite/2: breakout rooms

fn breakout(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-60.0, 0.0), 6.0, 7.0, 3.0, 0.0), w);
    p.fp.link(stroke(&[pt(-56.0, 0.0), pt(-8.0, 0.0)], 3.0), w);
    for x in [-48.0, -34.0, -20.0] {
        for s in [-1.0, 1.0] {
            p.fp.pod(rounded_rect(pt(x, 14.0 * s), 6.0, 5.0, 2.0, 0.0), w);
            p.fp.link(stroke(&[pt(x, 2.0 * s), pt(x, 10.0 * s)], 2.0), w);
        }
    }
    // The central breakout hall, where the twins align.
    p.fp.pod(blob(pt(16.0, 0.0), 26.0, 24.0, 0.0, 0, 0.0), w);
    p.fp.link(stroke(&[pt(36.0, 0.0), pt(46.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(52.0, 0.0), 7.0, 8.0, 3.0, 0.0), w);
    p.route = vec![
        pt(-58.0, 0.0),
        pt(-40.0, 0.0),
        pt(-12.0, 0.0),
        pt(6.0, 0.0),
        pt(14.0, 0.0),
    ];
    p.build_walls();
    wall_dress(p, Mat::Fabric0, Mat::White, Mat::Trim);
    cove(p, Mat::NeonCyan);
    breakout_dress(p);

    // Corridor: teal carpet, a sign by every room entrance.
    runner(
        p,
        &[
            pt(-58.0, 0.0),
            pt(-30.0, 0.0),
            pt(-10.0, 0.0),
            pt(10.0, 0.0),
        ],
        2.2,
        Mat::Fabric0,
        Mat::RunnerEdge,
    );
    for x in [-48.0, -34.0, -20.0] {
        for z in [-3.3, 3.3] {
            p.sign_near(x + 3.0, z, 2.2, 1.7, Sign::BreakoutRoom);
        }
    }
    p.sign_near(-52.0, -7.0, 2.4, 3.0, Sign::OffsiteAgenda);
    plant(p, -64.0, -5.5);
    plant(p, -64.0, 5.5);

    // Six small rooms: the two with packs stay open in the middle, the rest hold a meeting.
    for (x, wall) in [(-48.0, Sign::WhiteboardA), (-20.0, Sign::WhiteboardC)] {
        for s in [-1.0, 1.0] {
            long_table(p, x, 14.0 * s, 2, true);
            p.sign_near(x, 14.0 * s + 5.4 * s, 1.9, 3.4, wall);
            flipchart(p, x + 4.4, 14.0 * s + 3.6 * s, PI * (1.0 - s) / 2.0);
        }
    }
    for s in [-1.0, 1.0] {
        let z = 14.0 * s;
        flipchart(p, -38.6, z + 3.4 * s, PI * (1.0 - s) / 2.0);
        beanbag(p, -29.8, z + 3.6 * s, Mat::BeanA);
        beanbag(p, -29.6, z + 1.9 * s, Mat::BeanB);
        p.sign_near(-34.0, z + 5.4 * s, 1.9, 3.4, Sign::WhiteboardB);
    }
    // Sticky-note walls in the rooms and the hall.
    for (x, z) in [(-52.0, -14.0), (-52.0, 14.0), (-16.0, -14.0), (-16.0, 14.0)] {
        p.poster_near(x, z, 1.9, Vec2::new(2.6, 1.8), Tex::Stickies);
    }

    // Hall: an alignment ring under the twins, beanbags and flipcharts round the edge.
    for k in 0..20 {
        let a = k as f32 * TAU / 20.0;
        p.slab(
            v(24.0 + 7.5 * a.cos(), 0.03, 7.5 * a.sin()),
            -a + FRAC_PI_2,
            v(2.4, 0.04, 0.25),
            if k % 2 == 0 {
                Mat::NeonCyan
            } else {
                Mat::NeonYellow
            },
        );
    }
    runner(
        p,
        &[pt(-4.0, 0.0), pt(8.0, 0.0), pt(17.0, 0.0)],
        2.2,
        Mat::Fabric1,
        Mat::Gold,
    );
    for (x, z, m) in [
        (-1.0, -8.5, Mat::BeanA),
        (1.0, -10.0, Mat::BeanB),
        (-1.0, 8.5, Mat::BeanC),
        (1.0, 10.0, Mat::BeanA),
        (36.0, -9.0, Mat::BeanB),
        (37.0, 9.0, Mat::BeanC),
    ] {
        beanbag(p, x, z, m);
    }
    for (x, z) in [(10.0, -20.0), (22.0, -22.5), (22.0, 22.5), (10.0, 20.0)] {
        flipchart(p, x, z, if z < 0.0 { 0.0 } else { PI });
    }
    for (x, z) in [(34.0, -14.0), (34.0, 14.0), (-4.0, -12.0), (-4.0, 12.0)] {
        plant(p, x, z);
    }
    for (x, z) in [(30.0, -17.0), (30.0, 17.0), (-3.0, 0.0)] {
        p.poster_near(x, z, 2.0, Vec2::new(3.2, 2.0), Tex::Stickies);
    }
    p.sign_near(42.0, -6.0, 2.8, 4.2, Sign::InnovationHere);

    ceiling_lights(p);
    for (x, z) in [
        (-58.0, 0.0),
        (-34.0, 0.0),
        (-34.0, -14.0),
        (-34.0, 14.0),
        (-8.0, 0.0),
        (6.0, -12.0),
        (6.0, 12.0),
        (24.0, 0.0),
        (52.0, 0.0),
    ] {
        p.light(v(x, 3.5, z), Color::srgb(0.92, 0.99, 1.0), 400_000.0);
    }
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:offsite/1", (-67.0, 0.0), Sign::ToLobby),
                ("door:offsite/3", (60.0, 0.0), Sign::ToTerrace),
            ],
            packs: &[(-34.0, -14.0), (-34.0, 14.0), (4.0, -16.0), (4.0, 16.0)],
            boss: (24.0, 0.0),
            npc: None,
        },
    );
    theme(2)
}

/// Breakout hall: lounge corners, a coffee wall, project boards and a ceiling grid.
fn breakout_dress(p: &mut Plan) {
    beams(p, -4.0, 40.0, -24.0, 24.0, 8.0, Mat::Trim);
    round_patch(p, 24.0, 0.0, 15.0, Mat::Fabric2);
    for (x, z, yaw) in [
        (12.0, -13.0, 0.0),
        (12.0, 13.0, PI),
        (36.0, -14.0, 0.0),
        (36.0, 14.0, PI),
    ] {
        lounge(p, x, z, yaw, true);
    }
    // Coffee wall with a fridge nobody restocks.
    wall_prop(p, &SIDE_TABLE, 20.0, -24.1, true);
    p.prop(
        &COFFEE_MACHINE,
        v(20.0, SIDE_TABLE.size[1] * FURNITURE_SCALE, -23.5),
        0.0,
        false,
    );
    wall_prop(p, &FRIDGE, 15.0, -24.1, true);
    wall_prop(p, &TV, 30.0, 23.9, false);
    for (x, z, tex, w, h) in [
        (6.0, -24.0, Tex::Roadmap, 3.0, 1.5),
        (28.0, -24.0, Tex::Gantt, 2.6, 1.6),
        (6.0, 24.0, Tex::Flowchart, 3.0, 1.5),
        (38.0, 24.0, Tex::Kpi, 1.8, 1.2),
        (41.0, 0.0, Tex::Suggestions, 1.4, 1.8),
    ] {
        p.poster_near(x, z, 2.2, Vec2::new(w, h), tex);
    }
    // Corridor ends: a plant and a notice board.
    plant_pair(p, -56.0, 5.0);
    p.poster_near(-8.0, 3.0, 2.2, Vec2::new(1.6, 1.2), Tex::Standup);
    bunting(
        p,
        (6.0, -10.0),
        (6.0, 10.0),
        p.top() - 0.6,
        [Mat::Fabric0, Mat::Fabric1, Mat::Fabric2],
    );
}

/// Terrace: pergola over the boardwalk, parasols, loungers by the water, palms, a staged keynote.
fn terrace_dress(p: &mut Plan) {
    // Pergola over the boardwalk west end.
    for x in [-58.0, -50.0] {
        for z in [-3.2, 3.2] {
            p.boxed(v(x, 1.6, z), v(0.25, 3.2, 0.25), Mat::Wood, false);
        }
        p.boxed(v(x, 3.25, 0.0), v(0.3, 0.2, 7.2), Mat::Wood, false);
    }
    for k in 0..9 {
        p.boxed(
            v(-59.0 + k as f32 * 1.1, 3.4, 0.0),
            v(0.12, 0.12, 7.6),
            Mat::Bark,
            false,
        );
    }
    // Parasols on the cocktail tables.
    for (x, z, m) in [
        (-38.0, 12.0, Mat::Fabric0),
        (-30.0, 14.5, Mat::Fabric1),
        (-20.0, 13.0, Mat::Fabric2),
        (-8.0, 13.0, Mat::Fabric0),
        (22.0, 23.5, Mat::Fabric1),
        (34.0, 23.5, Mat::Fabric2),
        (46.0, 23.5, Mat::Fabric0),
    ] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 1.6, z),
            v(0.05, 3.2, 0.05),
            Mat::Metal,
        );
        p.prim(PrimKind::Cone, v(x, 3.15, z), v(2.4, 0.55, 2.4), m);
    }
    // Sun loungers along the south balustrade.
    for k in 0..4 {
        let x = 24.0 + k as f32 * 4.0;
        p.prop(&LOUNGE_CHAIR_RELAX, v(x, 0.0, 22.2), PI, false);
    }
    // Palms: leaning trunk, a crown of fronds.
    for (x, z) in [
        (-62.0, -9.0),
        (-62.0, 9.0),
        (66.0, -22.0),
        (66.0, 22.0),
        (-4.0, 22.0),
        (-4.0, -22.0),
    ] {
        for k in 0..6 {
            let a = k as f32 * TAU / 6.0;
            p.slab(
                v(x + 0.9 * a.cos(), 4.6, z + 0.9 * a.sin()),
                -a,
                v(2.2, 0.06, 0.5),
                Mat::Leaf,
            );
        }
        p.prim(
            PrimKind::Cylinder,
            v(x, 2.2, z),
            v(0.4, 4.4, 0.4),
            Mat::Bark,
        );
        p.prim(PrimKind::Sphere, v(x, 4.5, z), v(0.8, 0.6, 0.8), Mat::Bark);
    }
    // Keynote stage: wood fascia and steps, truss with lamps, speaker stacks, curtains.
    p.boxed(v(54.9, 0.1, 0.0), v(0.1, 0.2, 20.0), Mat::Wood, false);
    p.boxed(v(54.0, 0.06, 0.0), v(1.6, 0.12, 4.0), Mat::Wood, false);
    p.boxed(v(62.0, 5.2, 0.0), v(0.3, 0.3, 20.0), Mat::Metal, false);
    for k in 0..7 {
        p.boxed(
            v(62.0, 4.95, -9.0 + k as f32 * 3.0),
            v(0.5, 0.25, 0.5),
            Mat::LightPanel,
            false,
        );
    }
    for z in [-10.0, 10.0] {
        p.boxed(v(62.0, 2.6, z), v(0.3, 5.2, 0.3), Mat::Metal, false);
        p.boxed(v(65.0, 1.3, z * 0.9), v(1.4, 2.6, 1.2), Mat::Dark, true);
        p.boxed(v(64.28, 1.0, z * 0.9), v(0.04, 1.2, 1.0), Mat::Metal, false);
    }
    for z in [-6.0, 6.0] {
        p.boxed(v(66.2, 2.8, z * 1.2), v(0.3, 5.6, 3.0), Mat::Fabric0, false);
    }
    p.poster_near(-62.0, -4.0, 2.4, Vec2::new(1.4, 1.8), Tex::Wellness);
    // Lanterns on the balustrade.
    for k in 0..8 {
        p.prim(
            PrimKind::Sphere,
            v(-30.0 + k as f32 * 10.0, 1.15, 24.9),
            v(0.3, 0.4, 0.3),
            Mat::Candle,
        );
    }
}

/// Ballroom: pilasters and drapes round the walls, a parquet floor, gold beams, the stage staged.
fn ballroom_dress(p: &mut Plan) {
    for x in [-30.0, -20.0, -10.0, 0.0, 10.0, 20.0, 28.0] {
        pilaster(p, x, -27.9, Mat::Gold);
        pilaster(p, x, 27.9, Mat::Gold);
    }
    for x in [-24.0, -12.0, 4.0, 22.0] {
        for s in [-1.0, 1.0] {
            p.boxed(v(x, 2.0, s * 27.6), v(2.6, 4.0, 0.12), Mat::Fabric1, false);
        }
    }
    beams(p, -34.0, 30.0, -28.0, 28.0, 8.0, Mat::Dark);
    floor_patch(p, 14.0, 0.0, 0.0, 12.0, 14.0, Mat::Wood);
    floor_patch(p, -12.0, 0.0, 0.0, 30.0, 10.0, Mat::Fabric2);
    // Stage: steps, footlights, speaker stacks.
    for k in 0..3 {
        p.boxed(
            v(27.6 - k as f32 * 0.5, 0.04 + k as f32 * 0.04, 0.0),
            v(0.5, 0.08 + k as f32 * 0.08, 6.0),
            Mat::MarbleDark,
            false,
        );
    }
    for k in 0..12 {
        p.prim(
            PrimKind::Sphere,
            v(28.1, 0.3, -11.0 + k as f32 * 2.0),
            v(0.2, 0.12, 0.2),
            Mat::Candle,
        );
    }
    for z in [-9.0, 9.0] {
        p.boxed(v(40.0, 1.3, z), v(1.1, 2.6, 1.0), Mat::Dark, true);
    }
    p.poster_near(-8.0, -27.9, 2.2, Vec2::new(1.6, 1.2), Tex::PraiseModel);
    p.poster_near(8.0, 27.9, 2.2, Vec2::new(1.6, 1.2), Tex::Family);
    p.poster_near(-24.0, 27.9, 2.2, Vec2::new(1.8, 1.2), Tex::CloseDeals);
    for (x, z) in [(-45.0, -6.0), (-45.0, 6.0)] {
        plant_pair(p, x, z);
    }
}

// ------------------------------------------------------------------ offsite/3: terrace

fn terrace(p: &mut Plan) -> Theme {
    let (deck, hotel) = (Mat::Glass, Mat::Wall);
    // Service vestibule (west), the promenade, the wide keynote deck and a north exit vestibule.
    // Deck edges are glass balustrades, the vestibules are solid so the doors have a wall.
    p.fp.pod(rounded_rect(pt(-64.0, 0.0), 5.0, 7.0, 3.0, 0.0), hotel);
    p.fp.link(stroke(&[pt(-60.0, 0.0), pt(-36.0, 0.0)], 4.0), deck);
    p.fp.pod(rounded_rect(pt(-28.0, 0.0), 14.0, 16.0, 8.0, 0.0), deck);
    p.fp.link(stroke(&[pt(-16.0, 0.0), pt(16.0, 0.0)], 5.0), deck);
    p.fp.pod(rounded_rect(pt(40.0, 0.0), 27.0, 25.0, 6.0, 0.0), deck);
    p.fp.link(stroke(&[pt(50.0, -22.0), pt(50.0, -31.0)], 3.0), deck);
    p.fp.pod(rounded_rect(pt(50.0, -36.0), 6.0, 6.0, 3.0, 0.0), hotel);
    p.route = vec![
        pt(-62.0, 0.0),
        pt(-36.0, 0.0),
        pt(-28.0, 0.0),
        pt(0.0, 0.0),
        pt(46.0, 0.0),
    ];
    p.build_walls();

    // The lake south of the deck, the shore and hills behind it, the hotel to the north.
    p.slab(v(0.0, -0.12, 0.0), 0.0, v(900.0, 0.1, 900.0), Mat::LeafDark);
    p.slab(
        v(0.0, -0.045, 110.0),
        0.0,
        v(600.0, 0.05, 180.0),
        Mat::Water,
    );
    for (x, s) in [
        (-170.0, 70.0),
        (-80.0, 55.0),
        (10.0, 80.0),
        (110.0, 60.0),
        (190.0, 75.0),
    ] {
        p.prim(
            PrimKind::Cone,
            v(x, s * 0.4, 215.0),
            v(s * 1.8, s * 0.8, s * 1.8),
            Mat::LeafDark,
        );
    }
    p.boxed(v(0.0, 8.0, -52.0), v(160.0, 16.0, 10.0), Mat::Wall, false);
    for y in [4.0, 8.0, 12.0] {
        p.boxed(v(0.0, y, -46.9), v(150.0, 1.4, 0.1), Mat::Window, false);
    }
    p.sign(v(-30.0, 14.0, -46.8), 0.0, 14.0, Sign::OffsiteWelcome);

    // Boardwalk down the middle, wooden bands across the deck.
    runner(
        p,
        &[pt(-62.0, 0.0), pt(-36.0, 0.0), pt(0.0, 0.0), pt(28.0, 0.0)],
        5.0,
        Mat::Wood,
        Mat::Beige,
    );
    for x in [-20.0, 0.0, 16.0] {
        p.slab(v(x, 0.015, 0.0), 0.0, v(0.3, 0.03, 9.0), Mat::Trim);
    }
    p.sign_near(-64.0, -6.8, 2.6, 3.0, Sign::OffsiteAgenda);

    // String lights over the promenade and along the arena edges.
    string_lights(p, -42.0, -14.0, -13.0);
    string_lights(p, -42.0, -14.0, 13.0);
    string_lights(p, 16.0, 62.0, -23.0);
    string_lights(p, 16.0, 62.0, 23.0);

    // Bar counter on the north side with stools and a neon sign.
    p.boxed(v(-14.0, 0.55, -13.2), v(9.0, 1.1, 1.0), Mat::Wood, true);
    p.boxed(v(-14.0, 1.13, -13.2), v(9.2, 0.06, 1.2), Mat::Marble, false);
    p.boxed(v(-14.0, 1.3, -15.5), v(9.0, 2.6, 0.4), Mat::Dark, true);
    for k in 0..12 {
        let m = [Mat::Scr0, Mat::Scr1, Mat::Scr2, Mat::Yellow][k % 4];
        p.boxed(
            v(-17.8 + k as f32 * 0.65, 1.55, -15.25),
            v(0.18, 0.5, 0.18),
            m,
            false,
        );
    }
    for k in 0..5 {
        p.prop(&STOOL_BAR, v(-17.0 + k as f32 * 2.0, 0.0, -11.7), PI, false);
    }
    p.sign(v(-14.0, 3.6, -15.2), 0.0, 3.6, Sign::OffsiteBar);

    // Cocktail tables near the edges: pole, top and a candle.
    for (x, z) in [
        (-38.0, 12.0),
        (-30.0, 14.5),
        (-20.0, 13.0),
        (-8.0, 13.0),
        (-30.0, -14.0),
        (22.0, -23.5),
        (34.0, -23.5),
        (46.0, -23.5),
        (22.0, 23.5),
        (34.0, 23.5),
        (46.0, 23.5),
    ] {
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
        p.prim(
            PrimKind::Sphere,
            v(x, 1.15, z),
            v(0.14, 0.14, 0.14),
            Mat::Candle,
        );
        p.block(v(x, 0.6, z), v(0.9, 1.2, 0.9));
    }
    for (x, z) in [
        (-60.0, -11.0),
        (-60.0, 11.0),
        (-3.0, 15.0),
        (-3.0, -12.0),
        (14.0, -22.0),
        (14.0, 22.0),
    ] {
        plant(p, x, z);
    }

    // The keynote stage: low riser, lectern, a cyan projector and the slide wall behind.
    p.boxed(
        v(61.0, 0.1, 0.0),
        v(12.0, 0.2, 20.0),
        Mat::MarbleDark,
        false,
    );
    p.boxed(v(66.6, 3.0, 0.0), v(0.2, 5.8, 11.0), Mat::Dark, false);
    p.sign(
        v(66.4, 3.0, 0.0),
        Face::NegX.yaw(),
        10.0,
        Sign::KeynoteScreen,
    );
    p.boxed(v(63.0, 0.8, -5.0), v(0.9, 1.2, 0.7), Mat::Dark, true);
    p.boxed(v(62.7, 1.45, -5.0), v(0.7, 0.04, 0.5), Mat::NeonCyan, false);
    p.prim(
        PrimKind::Cylinder,
        v(61.5, 0.28, 0.0),
        v(2.2, 0.16, 2.2),
        Mat::Dark,
    );
    p.prim(
        PrimKind::Cylinder,
        v(61.5, 0.38, 0.0),
        v(1.8, 0.04, 1.8),
        Mat::NeonCyan,
    );
    p.prim(
        PrimKind::Cylinder,
        v(61.5, 1.6, 0.0),
        v(0.35, 2.4, 0.35),
        Mat::ScreenGlow,
    );
    for k in 0..16 {
        let a = k as f32 * TAU / 16.0;
        p.slab(
            v(58.0 + 3.2 * a.cos(), 0.23, 3.2 * a.sin()),
            -a + FRAC_PI_2,
            v(1.2, 0.03, 0.12),
            Mat::NeonCyan,
        );
    }

    terrace_dress(p);
    for (x, z, c, i) in [
        (-52.0, 0.0, Color::srgb(1.0, 0.85, 0.65), 380_000.0),
        (-30.0, 0.0, Color::srgb(1.0, 0.8, 0.6), 380_000.0),
        (-14.0, -12.0, Color::srgb(1.0, 0.7, 0.5), 380_000.0),
        (42.0, 0.0, Color::srgb(1.0, 0.85, 0.65), 380_000.0),
        (58.0, 0.0, Color::srgb(0.4, 0.9, 1.0), 500_000.0),
    ] {
        p.light(v(x, 3.4, z), c, i);
    }
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:offsite/2", (-69.0, 0.0), Sign::ToBreakout),
                ("door:offsite/4", (50.0, -43.0), Sign::ToBallroom),
            ],
            packs: &[(-34.0, -11.0), (-22.0, 11.0), (20.0, 12.0)],
            boss: (58.0, 0.0),
            npc: None,
        },
    );
    theme(3)
}

// ------------------------------------------------------------------ offsite/4: ballroom

fn ballroom(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-56.0, 0.0), 6.0, 8.0, 3.0, 0.0), w);
    p.fp.link(stroke(&[pt(-50.0, 0.0), pt(-40.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 42.0, 28.0, 10.0, 0.0), w);
    p.fp.pod(rounded_rect(pt(-24.0, 40.0), 8.0, 7.0, 3.0, 0.0), w);
    p.fp.link(stroke(&[pt(-24.0, 24.0), pt(-24.0, 36.0)], 3.5), w);
    p.route = vec![
        pt(-58.0, 0.0),
        pt(-40.0, 0.0),
        pt(-12.0, 0.0),
        pt(2.0, -9.0),
        pt(24.0, -8.0),
    ];
    p.build_walls();
    wall_dress(p, Mat::Wood, Mat::Gold, Mat::Gold);
    cove(p, Mat::Candle);
    ballroom_dress(p);

    // Gold-edged carpet through the foyer and between the tables.
    runner(
        p,
        &[
            pt(-58.0, 0.0),
            pt(-40.0, 0.0),
            pt(-12.0, 0.0),
            pt(2.0, -9.0),
            pt(22.0, -9.0),
        ],
        2.8,
        Mat::Fabric0,
        Mat::Gold,
    );
    // Foyer: plants, agenda, a coat check desk.
    for z in [-6.5, 6.5] {
        plant(p, -52.5, z);
    }
    p.sign_near(-53.0, -7.8, 2.6, 3.0, Sign::OffsiteAgenda);
    desk(p, -56.0, 6.6, 4.0);
    p.sign_near(-56.0, 7.8, 2.0, 2.4, Sign::OffsiteNameTags);

    // Round banquet tables, clear of the route and the packs.
    for (i, (x, z)) in [
        (-34.0, -19.0),
        (-34.0, 19.0),
        (-17.0, -22.0),
        (-17.0, 22.0),
        (-3.0, -21.0),
        (-3.0, 21.0),
        (12.0, -23.0),
        (14.0, 24.0),
    ]
    .into_iter()
    .enumerate()
    {
        let cloth = [Mat::Fabric0, Mat::Gold, Mat::Fabric1][i % 3];
        round_table(p, x, z, cloth, Mat::Fabric2);
    }
    for (x, z) in [
        (-24.0, -8.0),
        (-24.0, 8.0),
        (-8.0, -12.0),
        (-8.0, 12.0),
        (16.0, -14.0),
        (16.0, 14.0),
    ] {
        chandelier(p, x, z);
    }

    // The long family table in front of the stage, seats assigned by tenure.
    p.boxed(v(14.0, 0.39, 0.0), v(2.0, 0.78, 14.0), Mat::White, true);
    p.boxed(v(14.0, 0.8, 0.0), v(0.5, 0.02, 14.0), Mat::Fabric0, false);
    for k in 0..7 {
        let z = -6.0 + k as f32 * 2.0;
        p.slab(v(12.6, 0.45, z), 0.0, v(0.5, 0.9, 0.5), Mat::Fabric2);
        p.slab(v(15.4, 0.45, z), 0.0, v(0.5, 0.9, 0.5), Mat::Fabric2);
        if k % 2 == 0 {
            p.prim(
                PrimKind::Sphere,
                v(14.0, 0.95, z),
                v(0.22, 0.22, 0.22),
                Mat::Candle,
            );
        }
    }
    p.sign(v(11.4, 1.5, 0.0), Face::NegX.yaw(), 2.4, Sign::FamilyTable);
    p.boxed(v(11.4, 0.75, 0.0), v(0.08, 1.5, 0.08), Mat::Metal, false);

    // Stage: raised dais, curtains, podium, truss with lights and the giant LED screen.
    p.boxed(
        v(35.0, 0.12, 0.0),
        v(14.0, 0.24, 24.0),
        Mat::MarbleDark,
        false,
    );
    for z in [-12.5, 12.5] {
        p.boxed(v(41.0, 2.25, z), v(1.2, 4.5, 2.0), Mat::Fabric0, false);
    }
    p.boxed(v(40.0, 0.7, 0.0), v(1.0, 1.1, 1.2), Mat::Wood, true);
    p.boxed(v(39.5, 1.3, 0.0), v(0.1, 0.1, 0.5), Mat::Gold, false);
    p.boxed(v(30.0, 4.1, 0.0), v(0.2, 0.2, 20.0), Mat::Metal, false);
    for k in 0..5 {
        p.boxed(
            v(29.6, 3.95, -8.0 + k as f32 * 4.0),
            v(0.4, 0.2, 0.4),
            Mat::LightPanel,
            false,
        );
    }
    p.sign_near(42.0, 0.0, 2.7, 9.0, Sign::FamilyBanner);
    p.sign_near(0.0, -27.8, 2.8, 6.0, Sign::OffsiteWelcome);
    p.sign_near(0.0, 27.8, 2.8, 6.0, Sign::FamilyBanner);

    // Rear exit: a dignified pair of plants.
    plant(p, -29.5, 44.0);
    plant(p, -18.5, 44.0);
    p.sign_near(-24.0, 46.8, 2.4, 2.6, Sign::OffsiteNameTags);

    for (x, z) in [
        (-56.0, 0.0),
        (-34.0, 0.0),
        (-24.0, -8.0),
        (-24.0, 8.0),
        (-8.0, 0.0),
        (16.0, 0.0),
        (30.0, 0.0),
        (38.0, 0.0),
        (-24.0, 40.0),
    ] {
        p.light(v(x, 3.6, z), Color::srgb(1.0, 0.82, 0.58), 420_000.0);
    }
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:offsite/3", (-63.0, 0.0), Sign::ToTerrace),
                ("door:campus", (-24.0, 48.0), Sign::ToCampus),
            ],
            packs: &[(-24.0, -14.0), (-24.0, 14.0), (8.0, 18.0)],
            boss: (34.0, 0.0),
            npc: None,
        },
    );
    theme(4)
}
