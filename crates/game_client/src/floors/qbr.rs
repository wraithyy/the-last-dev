//! The Quarterly Business Review dungeon (docs/DUNGEONS.md): the boardroom floor on quarter-end.
//! qbr/1 is the executive lobby and catering (a boss-less trash gauntlet), qbr/2 the presentation
//! theatre (Slide Deck Golem), qbr/3 the analytics war room (Dana) and qbr/4 the boardroom (the
//! Board). Every room is the corridor-hall-pod plan of the Migration so packs and route stay
//! testable; what differs is the dressing. The satire is in the props: pastries nobody touches,
//! a giant screen with a hockey-stick chart, a SYNERGY wall, trolleys of untouched fruit, a
//! whiteboard of buzzwords, glass meeting pods, an AI-FIRST banner and printouts piled to the
//! ceiling. Tall things hug the walls: the orbit camera follows the route at head height, and a
//! prop along it blacks out the view (the Migration learned that).

use super::basement::{Shape, whiteboard};
use super::datacenter::dress::{WallSpot, floor_line, point_lights, rnd, strips};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble, yaw_towards};
use super::models;
use super::offsite::{cove, floor_patch, pilaster, plant_pair, round_patch, wall_dress};
use super::props::{beanbag, lounge, sad_plant, vending, water_cooler};
use super::themes::{pt, runner, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => ("qbr/1", "QBR: Executive Lobby and Catering", lobby(&mut p)),
        2 => ("qbr/2", "QBR: Presentation Theatre", theatre(&mut p)),
        3 => ("qbr/3", "QBR: Analytics War Room", war_room(&mut p)),
        _ => ("qbr/4", "QBR: The Boardroom", boardroom(&mut p)),
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, trim, accent, floor, fog, ambient, bright, range) = match level {
        // Lobby: a hotel conference lobby at 7am, cream and brass, lit like a showroom.
        1 => (
            c(0.93, 0.9, 0.82),
            c(0.35, 0.27, 0.16),
            LinearRgba::rgb(3.0, 2.2, 0.8),
            FloorKind::Marble,
            c(0.7, 0.66, 0.58),
            c(1.0, 0.94, 0.84),
            860.0,
            (30.0, 95.0),
        ),
        // Theatre: velvet and a screen the size of the wall; the house lights are down.
        2 => (
            c(0.5, 0.2, 0.24),
            c(0.14, 0.07, 0.08),
            LinearRgba::rgb(3.0, 0.5, 0.6),
            FloorKind::Carpet([0.55, 0.2, 0.26]),
            c(0.22, 0.1, 0.14),
            c(0.9, 0.6, 0.65),
            700.0,
            (22.0, 80.0),
        ),
        // War room: cool blue office light and the glow of forty dashboards, all of them up.
        3 => (
            c(0.62, 0.7, 0.8),
            c(0.12, 0.16, 0.24),
            LinearRgba::rgb(0.3, 1.6, 2.8),
            FloorKind::Tile([0.7, 0.8, 0.95]),
            c(0.3, 0.38, 0.5),
            c(0.7, 0.82, 1.0),
            820.0,
            (26.0, 85.0),
        ),
        // Boardroom: walnut, leather and a polished floor, lit low and expensively.
        _ => (
            c(0.46, 0.34, 0.26),
            c(0.14, 0.09, 0.06),
            LinearRgba::rgb(3.2, 1.8, 0.5),
            FloorKind::Gloss([0.5, 0.36, 0.28]),
            c(0.2, 0.14, 0.1),
            c(1.0, 0.82, 0.62),
            780.0,
            (26.0, 85.0),
        ),
    };
    Theme {
        wall,
        trim,
        accent,
        fabric: [c(0.5, 0.18, 0.22), c(0.2, 0.28, 0.42), c(0.3, 0.3, 0.3)],
        floor,
        fog,
        ambient,
        ambient_brightness: bright,
        sun: c(1.0, 0.95, 0.85),
        sun_illuminance: 2500.0,
        fog_range: range,
    }
}

// ------------------------------------------------------------------ shared pieces

/// A swivel chair in dark leather: star base, high back, arms. `alive` gives it two small
/// blinking amber eyes, because the boardroom chairs look at you.
fn leather_chair(p: &mut Plan, x: f32, z: f32, yaw: f32, alive: bool) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prim(
            PrimKind::Cylinder,
            v(0.0, 0.05, 0.0),
            v(0.7, 0.1, 0.7),
            Mat::Chrome,
        );
        p.prim(
            PrimKind::Cylinder,
            v(0.0, 0.3, 0.0),
            v(0.08, 0.5, 0.08),
            Mat::Chrome,
        );
        p.boxed(v(0.0, 0.58, 0.0), v(0.66, 0.16, 0.62), Mat::Dark, false);
        p.boxed(v(0.0, 1.0, -0.3), v(0.62, 0.9, 0.14), Mat::Dark, false);
        p.boxed(v(0.0, 1.55, -0.3), v(0.4, 0.22, 0.16), Mat::Dark, false);
        for s in [-1.0, 1.0] {
            p.boxed(
                v(s * 0.36, 0.82, -0.02),
                v(0.08, 0.08, 0.5),
                Mat::Dark,
                false,
            );
            if alive {
                p.boxed(
                    v(s * 0.12, 1.28, -0.22),
                    v(0.07, 0.07, 0.03),
                    Mat::BlinkAmber,
                    false,
                );
            }
        }
    });
}

/// A glass of water, one per seat.
fn glass_of_water(p: &mut Plan, x: f32, y: f32, z: f32) {
    models::put(p, &models::GLASS, v(x, y, z), 0.0, 0.5, false);
    p.prim(
        PrimKind::Cylinder,
        v(x, y + 0.06, z),
        v(0.07, 0.1, 0.07),
        Mat::Water,
    );
}

/// A table the length of a bowling lane: dark marble with a gold inlay, a leather chair, a pad
/// and a glass of water per seat. Along X; `alive` seeds which chairs blink.
fn boardroom_table(p: &mut Plan, x0: f32, x1: f32, z: f32, alive: i32) {
    let (len, cx) = (x1 - x0, (x0 + x1) / 2.0);
    p.boxed(v(cx, 0.76, z), v(len, 0.1, 2.2), Mat::MarbleDark, true);
    p.boxed(v(cx, 0.35, z), v(len - 1.4, 0.7, 0.9), Mat::Dark, false);
    p.boxed(v(cx, 0.82, z), v(len - 0.3, 0.02, 0.08), Mat::Gold, false);
    for s in [-1.0, 1.0] {
        p.boxed(
            v(cx, 0.82, z + s * 1.05),
            v(len, 0.02, 0.05),
            Mat::Gold,
            false,
        );
    }
    let n = ((len - 1.0) / 1.5).floor() as i32;
    for k in 0..n {
        let x = x0 + 0.9 + k as f32 * 1.5;
        for (si, s) in [-1.0f32, 1.0].into_iter().enumerate() {
            let yaw = if s < 0.0 { 0.0 } else { PI };
            let seed = alive + k * 2 + si as i32;
            leather_chair(p, x, z + s * 1.75, yaw, rnd(seed, 5) < 0.3);
            glass_of_water(p, x + 0.3, 0.81, z + s * 0.7);
            p.boxed(
                v(x - 0.1, 0.815, z + s * 0.62),
                v(0.34, 0.012, 0.24),
                Mat::Paper,
                false,
            );
        }
    }
}

/// Tilted heat lamp over a food table: a post, an arm and an orange shade.
fn heat_lamp(p: &mut Plan, x: f32, z: f32, light: bool) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 1.0, z),
        v(0.05, 2.0, 0.05),
        Mat::Chrome,
    );
    p.boxed(v(x, 2.0, z), v(0.6, 0.04, 0.05), Mat::Chrome, false);
    for s in [-0.26, 0.26] {
        p.prim(
            PrimKind::Cone,
            v(x + s, 1.85, z),
            v(0.3, 0.22, 0.3),
            Mat::Hot,
        );
    }
    if light {
        p.light(v(x, 1.7, z), Color::srgb(1.0, 0.55, 0.2), 90_000.0);
    }
}

/// A tray of croissants (Kenney food kit), each with its own turn; the tray is steel and cold.
fn croissants(p: &mut Plan, x: f32, y: f32, z: f32, n: usize, seed: i32) {
    p.boxed(v(x, y + 0.015, z), v(1.0, 0.03, 0.56), Mat::Chrome, false);
    for k in 0..n {
        let i = seed * 17 + k as i32;
        let (dx, dz) = ((k % 4) as f32 * 0.24 - 0.36, (k / 4) as f32 * 0.24 - 0.12);
        models::put(
            p,
            &models::CROISSANT,
            v(x + dx, y + 0.03, z + dz),
            rnd(i, 1) * TAU,
            0.55,
            false,
        );
    }
}

/// Cakes and muffins on plates: the dessert end of a buffet, equally untouched.
fn desserts(p: &mut Plan, x: f32, y: f32, z: f32, seed: i32) {
    let all = [
        &models::CAKE,
        &models::CUPCAKE,
        &models::MUFFIN,
        &models::DONUT,
        &models::PIE,
    ];
    for k in 0..3 {
        let m = all[(seed as usize + k * 2) % all.len()];
        let at = v(x + (k as f32 - 1.0) * 0.6, y, z);
        models::put(p, &models::PLATE, at, 0.0, 0.5, false);
        models::put(
            p,
            m,
            at + v(0.0, 0.04, 0.0),
            rnd(seed, k as i32),
            0.5,
            false,
        );
    }
}

/// A white-clothed buffet (two cloth tables end to end) with croissants and cakes under heat
/// lamps nobody queues at, and a glass dome over the last croissant.
fn buffet(p: &mut Plan, x: f32, z: f32, seed: i32) {
    for s in [-1.0, 1.0] {
        p.prop(&TABLE_CLOTH, v(x + s * 1.0, 0.0, z), 0.0, true);
    }
    let top = TABLE_CLOTH.size[1] * FURNITURE_SCALE + 0.01;
    croissants(p, x - 1.0, top, z, 8, seed);
    desserts(p, x + 1.0, top, z, seed);
    p.prim(
        PrimKind::Sphere,
        v(x, top + 0.2, z),
        v(0.55, 0.4, 0.55),
        Mat::Glass,
    );
    models::put(p, &models::CROISSANT, v(x, top, z), 0.4, 0.55, false);
    // Sandwiches and a loaf for the savoury stakeholder, equally untouched.
    models::put(
        p,
        &models::SANDWICH,
        v(x - 0.2, top, z + 0.3),
        0.3,
        0.6,
        false,
    );
    models::put(
        p,
        &models::SANDWICH,
        v(x + 0.2, top, z - 0.3),
        -0.4,
        0.6,
        false,
    );
    models::put(
        p,
        &models::LOAF,
        v(x + 0.55, top, z + 0.25),
        0.8,
        0.6,
        false,
    );
    heat_lamp(p, x - 1.9, z, rnd(seed, 3) < 0.5);
    heat_lamp(p, x + 1.9, z, false);
}

/// A catering trolley of fruit nobody has touched since 07:00: three shelves, four posts, domes.
fn fruit_trolley(p: &mut Plan, x: f32, z: f32, yaw: f32, seed: i32) {
    let fruit = [
        &models::APPLE,
        &models::ORANGE,
        &models::BANANA,
        &models::PEAR,
        &models::GRAPES,
        &models::STRAWBERRY,
    ];
    p.framed(v(x, 0.0, z), yaw, |p| {
        for sx in [-0.55, 0.55] {
            for sz in [-0.3, 0.3] {
                p.prim(
                    PrimKind::Cylinder,
                    v(sx, 0.7, sz),
                    v(0.04, 1.4, 0.04),
                    Mat::Chrome,
                );
                p.prim(
                    PrimKind::Cylinder,
                    v(sx, 0.07, sz),
                    v(0.12, 0.1, 0.12),
                    Mat::Tire,
                );
            }
        }
        for (k, y) in [0.25, 0.75, 1.25].into_iter().enumerate() {
            p.boxed(v(0.0, y, 0.0), v(1.3, 0.04, 0.7), Mat::White, false);
            if k == 0 {
                models::put(
                    p,
                    &models::WATERMELON,
                    v(0.0, y + 0.02, 0.0),
                    0.3,
                    1.0,
                    false,
                );
            }
            if k == 1 {
                models::put(p, &models::BOWL, v(0.0, y + 0.02, 0.0), 0.0, 0.8, false);
            }
            for j in 0..5 {
                let i = seed * 13 + k as i32 * 7 + j;
                let m = fruit[(i as usize) % fruit.len()];
                models::put(
                    p,
                    m,
                    v(-0.48 + j as f32 * 0.24, y + 0.02, (rnd(i, 2) - 0.5) * 0.3),
                    rnd(i, 4) * TAU,
                    0.9,
                    false,
                );
            }
        }
        p.prim(
            PrimKind::Sphere,
            v(0.0, 1.5, 0.0),
            v(0.7, 0.3, 0.5),
            Mat::Glass,
        );
        p.block(v(0.0, 0.7, 0.0), v(1.3, 1.4, 0.7));
    });
}

/// Printed slides piled up and spilling over: sheets with a random turn each, a few with a bar chart.
fn slide_pile(p: &mut Plan, x: f32, z: f32, sheets: usize, seed: i32) {
    for k in 0..sheets {
        let i = seed * 31 + k as i32;
        let (dx, dz) = ((rnd(i, 1) - 0.5) * 0.26, (rnd(i, 2) - 0.5) * 0.2);
        p.slab(
            v(x + dx, 0.014 + k as f32 * 0.024, z + dz),
            (rnd(i, 3) - 0.5) * 0.7,
            v(1.0, 0.02, 0.72),
            Mat::Paper,
        );
        if k % 5 == 0 {
            let y = 0.028 + k as f32 * 0.024;
            for b in 0..4 {
                p.slab(
                    v(x + dx - 0.24 + b as f32 * 0.14, y, z + dz),
                    0.0,
                    v(0.07, 0.01, 0.1 + b as f32 * 0.07),
                    [Mat::Scr1, Mat::Red][b % 2],
                );
            }
        }
    }
    // The pile is knee-high and solid: somebody tried to recycle it and was stopped.
    p.block(
        v(x, sheets as f32 * 0.012, z),
        v(1.0, sheets as f32 * 0.024, 0.8),
    );
}

/// The pre-read: 94 pages in shrink-wrap, with a red "READ BEFORE" band. The prop anchor is a
/// step in front of it.
fn preread(p: &mut Plan, key: &'static str, x: f32, z: f32) {
    p.boxed(v(x, 0.6, z - 0.5), v(0.7, 0.2, 0.9), Mat::Paper, false);
    p.boxed(v(x, 0.72, z - 0.5), v(0.72, 0.04, 0.2), Mat::Red, false);
    p.boxed(v(x, 0.64, z - 0.5), v(0.76, 0.28, 0.96), Mat::Glass, false);
    p.boxed(v(x, 0.3, z - 0.5), v(0.8, 0.6, 1.0), Mat::Wood, true);
    p.sign(v(x, 0.5, z + 0.01), 0.0, 0.7, Sign::QbrPreRead);
    p.interactable(key, v(x, 0.8, z - 0.5), v(x, 0.0, z + 0.7));
}

/// A wall-mounted screen on a bezel at height `y`, with a lit backdrop.
pub(super) fn wall_screen(
    p: &mut Plan,
    x: f32,
    z: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Option<WallSpot> {
    let wl = WallSpot::at(p, x, z)?;
    wl.put(p, 0.0, y, 0.05, v(w + 0.4, h + 0.4, 0.08), Mat::Dark);
    wl.put(p, 0.0, y, 0.1, v(w, h, 0.03), Mat::ScreenGlow);
    Some(wl)
}

/// The giant screen with a hockey-stick chart: flat for five quarters, then straight up, with a
/// grid, a dotted trend line and an arrow, because the chart is the strategy.
fn hockey_screen(p: &mut Plan, x: f32, z: f32, y: f32, w: f32, h: f32) {
    let Some(wl) = WallSpot::at(p, x, z) else {
        return;
    };
    wl.put(p, 0.0, y, 0.05, v(w + 0.5, h + 0.5, 0.1), Mat::Dark);
    wl.put(p, 0.0, y, 0.11, v(w, h, 0.03), Mat::Rack);
    let bottom = y - h / 2.0 + 0.35;
    for k in 1..5 {
        wl.put(
            p,
            0.0,
            bottom + (h - 0.7) * k as f32 / 5.0,
            0.13,
            v(w - 0.5, 0.02, 0.02),
            Mat::Trim,
        );
    }
    let n = 14;
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let bar = 0.12 + (h - 1.0) * t.powi(5);
        let l = -w / 2.0 + 0.7 + t * (w - 1.4);
        wl.put(
            p,
            l,
            bottom + bar / 2.0,
            0.15,
            v(w / n as f32 * 0.6, bar, 0.04),
            if i < 10 { Mat::LedBlue } else { Mat::LedGreen },
        );
        // Dotted stick on top of every bar.
        wl.put(
            p,
            l,
            bottom + bar + 0.12,
            0.17,
            v(0.1, 0.1, 0.05),
            Mat::NeonYellow,
        );
    }
    // The arrow head at the top right of the stick.
    wl.put(
        p,
        w / 2.0 - 0.35,
        bottom + h - 0.55,
        0.18,
        v(0.5, 0.12, 0.05),
        Mat::NeonYellow,
    );
    wl.put(
        p,
        w / 2.0 - 0.15,
        bottom + h - 0.8,
        0.18,
        v(0.12, 0.5, 0.05),
        Mat::NeonYellow,
    );
    wl.sign(
        p,
        0.0,
        y - h / 2.0 - 0.6,
        (w * 0.5).min(5.0),
        Sign::QbrHockey,
    );
}

/// The SYNERGY wall: a teal panel, three overlapping logo discs that agree on nothing, the word
/// itself three times and a mascot poster to either side.
fn synergy_wall(p: &mut Plan, x: f32, z: f32, w: f32) {
    let Some(wl) = WallSpot::at(p, x, z) else {
        return;
    };
    wl.put(p, 0.0, 2.4, 0.05, v(w, 3.8, 0.08), Mat::Accent);
    wl.put(p, 0.0, 2.4, 0.1, v(w - 0.4, 3.4, 0.03), Mat::Dark);
    for (k, m) in [Mat::NeonCyan, Mat::NeonPink, Mat::NeonYellow]
        .into_iter()
        .enumerate()
    {
        let c = wl.world((k as f32 - 1.0) * 0.9, 0.16);
        p.prim_yawed(
            PrimKind::Sphere,
            v(c.x, 3.4 + (k % 2) as f32 * -0.5, c.y),
            v(1.7, 1.7, 0.06),
            wl.yaw(),
            m,
        );
    }
    wl.sign(p, 0.0, 1.6, w - 1.0, Sign::QbrSynergy);
    for s in [-1.0, 1.0] {
        let c = wl.world(s * (w / 2.0 + 1.0), 0.1);
        p.panel(
            v(c.x, 2.4, c.y),
            wl.yaw(),
            Vec2::new(1.6, 2.0),
            Tex::Synergy,
        );
    }
}

/// A CEO portrait that watches you: gold frame, dark canvas, a head, a suit, a tie and two
/// amber eyes that blink out of sync.
fn ceo_portrait(p: &mut Plan, x: f32, z: f32, seed: i32) {
    let Some(wl) = WallSpot::at(p, x, z) else {
        return;
    };
    let y = 2.35;
    wl.put(p, 0.0, y, 0.05, v(1.5, 1.95, 0.06), Mat::Gold);
    wl.put(p, 0.0, y, 0.09, v(1.3, 1.75, 0.04), Mat::Dark);
    let suits = [Mat::Fabric1, Mat::Fabric2, Mat::Rack, Mat::Wood];
    wl.put(
        p,
        0.0,
        y - 0.62,
        0.12,
        v(1.0, 0.5, 0.05),
        suits[seed as usize % suits.len()],
    );
    wl.put(p, 0.0, y - 0.55, 0.16, v(0.12, 0.45, 0.03), Mat::Red);
    let c = wl.world(0.0, 0.15);
    p.prim_yawed(
        PrimKind::Sphere,
        v(c.x, y + 0.12, c.y),
        v(0.62, 0.72, 0.28),
        wl.yaw(),
        Mat::Beige,
    );
    for s in [-0.13, 0.13] {
        wl.put(p, s, y + 0.2, 0.3, v(0.1, 0.05, 0.03), Mat::BlinkAmber);
    }
    wl.put(p, 0.0, y - 0.97, 0.1, v(0.9, 0.12, 0.03), Mat::Gold);
    wl.sign(p, 0.0, y - 0.97, 0.8, Sign::QbrCeo);
}

/// A glass meeting pod: frosted band, steel posts, a rug, a glass table with chairs, a screen on a
/// stand and a plant. The door is the gap on the south side (+Z).
fn glass_pod(p: &mut Plan, cx: f32, cz: f32, w: f32, d: f32) {
    let (hx, hz) = (w / 2.0, d / 2.0);
    for (px, pz) in [(-hx, -hz), (hx, -hz), (-hx, hz), (hx, hz)] {
        p.boxed(
            v(cx + px, 1.45, cz + pz),
            v(0.12, 2.9, 0.12),
            Mat::Metal,
            true,
        );
    }
    p.boxed(v(cx, 2.92, cz), v(w + 0.2, 0.1, d + 0.2), Mat::Metal, false);
    // North and the two sides are glass; the south side has a 1.6 m door gap.
    p.boxed(v(cx, 1.4, cz - hz), v(w - 0.2, 2.6, 0.06), Mat::Glass, true);
    for s in [-1.0, 1.0] {
        p.boxed(
            v(cx + s * hx, 1.4, cz),
            v(0.06, 2.6, d - 0.2),
            Mat::Glass,
            true,
        );
    }
    for s in [-1.0, 1.0] {
        let seg = (w - 1.6) / 2.0;
        p.boxed(
            v(cx + s * (0.8 + seg / 2.0), 1.4, cz + hz),
            v(seg, 2.6, 0.06),
            Mat::Glass,
            true,
        );
    }
    p.boxed(
        v(cx, 1.15, cz - hz),
        v(w - 0.2, 0.45, 0.07),
        Mat::Frosted,
        false,
    );
    for s in [-1.0, 1.0] {
        p.boxed(
            v(cx + s * hx, 1.15, cz),
            v(0.07, 0.45, d - 0.2),
            Mat::Frosted,
            false,
        );
    }
    p.prop(&RUG_ROUND, v(cx, 0.0, cz), 0.0, false);
    p.prop(&TABLE_GLASS, v(cx, 0.0, cz - 0.3), 0.0, true);
    p.prop(&CHAIR_MODERN, v(cx - 0.7, 0.0, cz + 0.5), PI, false);
    p.prop(&CHAIR_MODERN, v(cx + 0.7, 0.0, cz + 0.5), PI, false);
    p.prop(&CHAIR_MODERN, v(cx - 0.7, 0.0, cz - 1.1), 0.0, false);
    p.prop(&CHAIR_MODERN, v(cx + 0.7, 0.0, cz - 1.1), 0.0, false);
    p.boxed(
        v(cx - hx + 0.5, 1.0, cz - hz + 0.4),
        v(0.1, 2.0, 0.1),
        Mat::Metal,
        false,
    );
    p.prop(&SCREEN, v(cx - hx + 0.5, 1.05, cz - hz + 0.45), 0.0, false);
    p.prop(
        &POTTED_PLANT,
        v(cx + hx - 0.5, 0.0, cz - hz + 0.5),
        0.0,
        false,
    );
    p.sign(v(cx, 3.4, cz + hz), 0.0, 1.8, Sign::QbrPod);
}

/// A theatre seat in red velvet facing local -Z at yaw PI: base, back, two arms.
fn cinema_seat(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 0.25, 0.0), v(0.58, 0.5, 0.52), Mat::Red, false);
        p.boxed(v(0.0, 0.75, -0.24), v(0.58, 0.8, 0.1), Mat::Red, false);
        for s in [-1.0, 1.0] {
            p.boxed(v(s * 0.31, 0.5, 0.0), v(0.06, 0.1, 0.5), Mat::Dark, false);
        }
    });
}

/// A copier that has been printing charts nobody asked for since lunch: output tray, a drift of
/// sheets on the floor and a status light that is red and proud of it.
fn chart_printer(p: &mut Plan, x: f32, z: f32, face: Face, seed: i32) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 0.6, 0.0), v(1.5, 1.2, 0.9), Mat::Metal, true);
        p.boxed(v(0.0, 1.25, 0.0), v(1.4, 0.12, 0.8), Mat::Dark, false);
        p.boxed(v(0.0, 0.8, 0.5), v(1.0, 0.04, 0.4), Mat::Paper, false);
        p.boxed(v(0.5, 1.0, 0.46), v(0.1, 0.06, 0.02), Mat::LedRed, false);
        p.boxed(
            v(-0.2, 1.0, 0.46),
            v(0.5, 0.2, 0.02),
            Mat::ScreenGlow,
            false,
        );
    });
    let d = face.dir();
    for k in 0..16 {
        let i = seed * 19 + k;
        let (a, b) = (0.9 + rnd(i, 1) * 2.4, (rnd(i, 2) - 0.5) * 1.6);
        let side = Vec3::new(-d.z, 0.0, d.x);
        let at = v(x, 0.0, z) + d * a + side * b;
        p.slab(
            v(at.x, 0.012 + (k % 3) as f32 * 0.012, at.z),
            rnd(i, 3) * TAU,
            v(0.4, 0.01, 0.56),
            Mat::Paper,
        );
        if k % 2 == 0 {
            p.slab(
                v(at.x, 0.03 + (k % 3) as f32 * 0.012, at.z),
                rnd(i, 3) * TAU,
                v(0.3, 0.008, 0.08),
                [Mat::Scr1, Mat::Red, Mat::LedGreen][k as usize % 3],
            );
        }
    }
}

/// A sign over a scatter of a few of the same props: the "SINGLE SOURCE OF TRUTH" spreadsheet
/// triptych, three screens with three totals.
fn spreadsheet_trio(p: &mut Plan, x: f32, z: f32) {
    let Some(wl) = WallSpot::at(p, x, z) else {
        return;
    };
    wl.sign(p, 0.0, 3.55, 5.6, Sign::QbrSsot);
    for (k, s) in [Sign::QbrSheetA, Sign::QbrSheetB, Sign::QbrSheetC]
        .into_iter()
        .enumerate()
    {
        let l = (k as f32 - 1.0) * 2.4;
        wl.put(p, l, 2.0, 0.05, v(2.2, 1.5, 0.08), Mat::Dark);
        wl.sign(p, l, 2.0, 2.0, s);
    }
}

pub(super) fn tags(p: &mut Plan, list: &[(&'static str, f32, f32)]) {
    for &(name, x, z) in list {
        p.interactable(name, v(x, 0.5, z - 0.4), v(x, 0.0, z));
    }
}

pub(super) fn lamps(p: &mut Plan, color: Color, list: &[(f32, f32)], intensity: f32) {
    point_lights(p, list, color, intensity);
}

/// A reception desk: marble front, a glowing calendar behind, a bell and a laptop.
fn reception_desk(p: &mut Plan, x: f32, z: f32) {
    p.boxed(v(x, 0.55, z), v(1.0, 1.1, 3.2), Mat::Marble, true);
    p.boxed(v(x, 1.12, z), v(1.1, 0.05, 3.3), Mat::Gold, false);
    p.prop(&LAPTOP, v(x, 1.15, z + 0.6), -FRAC_PI_2, false);
    p.prim(
        PrimKind::Sphere,
        v(x, 1.2, z - 0.7),
        v(0.16, 0.12, 0.16),
        Mat::Gold,
    );
    p.prop(&CHAIR_DESK, v(x - 1.4, 0.0, z), FRAC_PI_2, false);
}

/// Wall-hung flat banner on its own rail: width `w` along the wall.
pub(super) fn banner(p: &mut Plan, x: f32, z: f32, w: f32, y: f32, tex: Tex, aspect: f32) {
    let h = w / aspect;
    p.poster_near(x, z, y, Vec2::new(w, h), tex);
}

/// Brass stanchions and a velvet rope along a line: the carpet is for guests, and guests queue.
fn rope_line(p: &mut Plan, x0: f32, x1: f32, z: f32, step: f32) {
    let n = ((x1 - x0) / step).floor() as i32;
    for k in 0..=n {
        let x = x0 + k as f32 * step;
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.45, z),
            v(0.09, 0.9, 0.09),
            Mat::Gold,
        );
        p.prim(
            PrimKind::Sphere,
            v(x, 0.93, z),
            v(0.16, 0.16, 0.16),
            Mat::Gold,
        );
        if k < n {
            p.boxed(
                v(x + step / 2.0, 0.78, z),
                v(step - 0.1, 0.04, 0.04),
                Mat::Red,
                false,
            );
        }
    }
}

/// Red stage curtains hung in folds against a wall at `l` along it.
pub(super) fn curtains(p: &mut Plan, x: f32, z: f32, folds: usize) {
    let Some(wl) = WallSpot::at(p, x, z) else {
        return;
    };
    for k in 0..folds {
        wl.put(
            p,
            k as f32 * 0.5,
            2.0,
            0.1 + (k % 2) as f32 * 0.12,
            v(0.48, 4.0, 0.12),
            if k % 2 == 0 { Mat::Red } else { Mat::Fabric0 },
        );
    }
    wl.put(
        p,
        folds as f32 * 0.25 - 0.25,
        4.1,
        0.15,
        v(folds as f32 * 0.5, 0.3, 0.3),
        Mat::Gold,
    );
}

/// Pot plants either side of a point, big and well watered: this floor has a facilities budget.
pub(super) fn greenery(p: &mut Plan, x: f32, z: f32) {
    plant_pair(p, x, z);
}

// ------------------------------------------------------------------ qbr/1: executive lobby

const LOBBY: Shape = Shape {
    arrival_x: -44.0,
    hall: (-6.0, 22.0, 17.0),
    cells: [(-14.0, -30.0), (-14.0, 30.0)],
    pod: (36.0, 12.0, 10.0),
    pod_wall: Mat::Wall,
};

fn lobby(p: &mut Plan) -> Theme {
    LOBBY.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Marble, Mat::Gold, Mat::Trim);
    cove(p, Mat::Candle);

    // The red carpet, rolled out for guests who are not coming.
    runner(
        p,
        &[pt(-46.0, 0.0), pt(-30.0, 0.0), pt(-6.0, 0.0), pt(36.0, 0.0)],
        2.6,
        Mat::Red,
        Mat::Gold,
    );
    for x in [-26.0, -9.5, 4.0, 12.0] {
        pilaster(p, x, -16.9, Mat::Marble);
    }
    for x in [-26.0, -9.5, 0.0, 12.0] {
        pilaster(p, x, 16.9, Mat::Marble);
    }
    // North wall: the scrolling welcome screen, the AI-FIRST banner and the man in the beanbag.
    if let Some(wl) = wall_screen(p, -22.0, -16.9, 2.7, 5.6, 2.1) {
        wl.sign(p, 0.0, 2.7, 5.4, Sign::QbrWelcome);
        for k in 0..12 {
            wl.put(
                p,
                -2.6 + k as f32 * 0.47,
                1.5,
                0.13,
                v(0.3, 0.07, 0.03),
                [Mat::NeonYellow, Mat::NeonCyan][k % 2],
            );
        }
    }
    banner(p, 2.0, -16.9, 5.1, 2.5, Tex::AiFirst, 1.5);
    lounge(p, -21.0, -11.0, 0.0, true);
    lounge(p, 4.0, -11.0, 0.0, false);
    beanbag(p, 11.5, -13.8, Mat::BeanC);
    p.boxed(v(11.4, 0.7, -13.8), v(0.95, 0.24, 0.42), Mat::Dark, false);
    p.boxed(v(11.9, 0.62, -13.8), v(0.4, 0.2, 0.38), Mat::Dark, false);
    p.prim(
        PrimKind::Sphere,
        v(10.7, 0.72, -13.8),
        v(0.3, 0.32, 0.3),
        Mat::Beige,
    );
    p.sign_near(11.5, -16.9, 2.3, 2.4, Sign::QbrVisionary);
    synergy_wall(p, 15.9, -8.0, 6.4);
    slide_pile(p, -5.0, -15.0, 36, 1);
    slide_pile(p, 8.0, -15.4, 24, 2);
    preread(p, "prop:preread_1", -9.0, -11.3);

    // South wall: two buffets, fruit trolleys, the coffee station of four brands and no coffee.
    buffet(p, -22.0, 14.3, 1);
    buffet(p, -7.0, 14.3, 4);
    p.sign_near(-22.0, 16.9, 3.3, 3.6, Sign::QbrCatering);
    p.sign_near(-7.0, 16.9, 3.3, 3.6, Sign::QbrCatering);
    fruit_trolley(p, -14.0, 9.5, 0.0, 1);
    fruit_trolley(p, 0.0, 13.3, 0.0, 2);
    fruit_trolley(p, 13.0, 12.5, FRAC_PI_2, 3);
    p.sign_near(0.0, 16.9, 2.6, 2.6, Sign::QbrFruit);
    p.boxed(v(8.0, 0.45, 16.2), v(4.8, 0.9, 0.9), Mat::Wood, true);
    p.boxed(
        v(8.0, 0.92, 16.2),
        v(4.9, 0.04, 1.0),
        Mat::MarbleDark,
        false,
    );
    for (k, m) in [Mat::Red, Mat::NeonCyan, Mat::Gold, Mat::Plant]
        .into_iter()
        .enumerate()
    {
        let x = 6.3 + k as f32 * 1.1;
        p.prop(&COFFEE_MACHINE, v(x, 0.94, 16.2), PI, false);
        p.boxed(v(x, 1.75, 16.7), v(0.5, 0.3, 0.05), m, false);
        p.prim(
            PrimKind::Cylinder,
            v(x, 1.05, 15.8),
            v(0.14, 0.2, 0.14),
            Mat::Glass,
        );
    }
    p.sign_near(8.0, 16.9, 2.9, 3.6, Sign::QbrCoffee);
    // The prize car, because nothing says quarterly review like a car in the lobby.
    models::put(
        p,
        &models::SUV_LUXURY,
        v(-0.5, 0.0, 6.5),
        -FRAC_PI_2,
        1.15,
        true,
    );
    p.sign(v(-0.5, 2.0, 8.5), 0.0, 2.2, Sign::QbrPrize);
    p.prim(
        PrimKind::Cylinder,
        v(-0.5, 0.05, 6.5),
        v(4.6, 0.1, 4.6),
        Mat::Gold,
    );
    greenery(p, -27.0, -15.0);
    greenery(p, 14.5, 15.0);
    greenery(p, -27.0, 15.5);
    for z in [-3.2, 3.2] {
        floor_line(p, pt(-26.0, z), pt(12.0, z), 0.1, Mat::Gold);
    }

    // Arrival: the Executive Assistant's desk, the calendar, a coat rack and the first pre-read.
    reception_desk(p, -48.0, 5.0);
    p.sign_near(-44.0, 5.9, 2.7, 3.0, Sign::Calendar);
    p.prop(&COAT_RACK, v(-47.5, 0.0, -5.0), 0.0, false);
    p.sign_near(-49.9, -4.5, 2.4, 2.4, Sign::QbrDoor);
    preread(p, "prop:preread_2", -42.0, -4.4);
    greenery(p, -39.0, 5.0);

    // North cell: catering prep. Steel counters, fridges, trolleys waiting to be wheeled out.
    for k in 0..4 {
        p.prop(
            &KITCHEN_CABINET,
            v(-21.0 + k as f32 * 2.1, 0.0, -35.9),
            0.0,
            true,
        );
    }
    for k in 0..2 {
        p.prop(
            &FRIDGE,
            v(-9.0, 0.0, -34.5 + k as f32 * 1.4),
            -FRAC_PI_2,
            true,
        );
    }
    fruit_trolley(p, -20.0, -25.0, FRAC_PI_2, 4);
    fruit_trolley(p, -8.5, -26.0, FRAC_PI_2, 5);
    fruit_trolley(p, -9.5, -30.5, 0.0, 6);
    p.sign_near(-14.0, -36.9, 2.8, 2.6, Sign::QbrFruit);
    // South cell: the stakeholder lounge, with a pod nobody has booked and a vending machine.
    glass_pod(p, -19.0, 34.5, 6.0, 4.0);
    lounge(p, -9.0, 33.2, PI, true);
    vending(p, -6.7, 27.5, Face::NegX);
    water_cooler(p, -6.7, 29.8);
    p.sign_near(-14.0, 36.9, 2.8, 2.6, Sign::QbrToDay);

    // The exit pod: brass lift, a lounge and a chandelier of bulbs, off the route.
    for s in [-1.0, 1.0] {
        p.boxed(
            v(47.8, 1.3, -4.5 + s * 0.75),
            v(0.1, 2.6, 1.3),
            Mat::Gold,
            false,
        );
    }
    p.boxed(v(47.9, 3.0, -4.5), v(0.12, 0.3, 3.4), Mat::Dark, false);
    p.sign(v(47.7, 3.3, -4.5), -FRAC_PI_2, 2.2, Sign::QbrLift);
    p.boxed(v(47.8, 1.3, -6.3), v(0.08, 0.5, 0.2), Mat::Gold, false);
    p.boxed(
        v(47.78, 1.35, -6.3),
        v(0.06, 0.12, 0.12),
        Mat::LedRed,
        false,
    );
    lounge(p, 30.0, -6.2, 0.0, true);
    lounge(p, 31.0, 6.2, PI, false);
    p.prim(
        PrimKind::Cylinder,
        v(36.0, 4.2, -6.5),
        v(0.1, 0.6, 0.1),
        Mat::Gold,
    );
    p.prim(
        PrimKind::Sphere,
        v(36.0, 3.8, -6.5),
        v(1.6, 0.5, 1.6),
        Mat::Candle,
    );
    p.prim(
        PrimKind::Cylinder,
        v(36.0, 4.2, 6.5),
        v(0.1, 0.6, 0.1),
        Mat::Gold,
    );
    p.prim(
        PrimKind::Sphere,
        v(36.0, 3.8, 6.5),
        v(1.6, 0.5, 1.6),
        Mat::Candle,
    );
    p.sign_near(36.0, -9.9, 2.6, 3.0, Sign::QbrToTheatre);
    wall_cabinet_plant(p);

    // Velvet ropes along the carpet, and a pair of palms every few metres outside them.
    rope_line(p, -40.0, 30.0, -1.8, 3.5);
    rope_line(p, -40.0, 30.0, 1.8, 3.5);
    for x in [-18.0, -2.0, 14.0, 26.0] {
        for z in [-3.9, 3.9] {
            p.prop(&POTTED_PLANT, v(x, 0.0, z), 0.0, false);
        }
    }
    // The exit pod is the stakeholders' own: two more buffets, trolleys, a Q3 photo wall.
    buffet(p, 38.0, -8.4, 9);
    buffet(p, 38.0, 8.4, 10);
    fruit_trolley(p, 42.0, -6.5, 0.0, 19);
    fruit_trolley(p, 42.0, 6.5, 0.0, 20);
    banner(p, 36.0, -9.9, 4.0, 2.7, Tex::AiTransformation, 2.17);
    banner(p, 30.0, 9.9, 3.0, 2.7, Tex::Kpi, 1.3);
    p.prop(&LOUNGE_SOFA_CORNER, v(27.0, 0.0, -2.0), FRAC_PI_2, true);
    p.prop(&COFFEE_TABLE_SQUARE, v(29.5, 0.0, -1.6), 0.0, true);
    p.prop(&TV_VINTAGE, v(32.0, 0.0, -9.6), 0.0, true);
    p.prop(&SPEAKER, v(46.0, 0.0, -9.4), 0.0, true);
    p.prop(&SPEAKER, v(46.0, 0.0, 9.4), PI, true);
    p.sign_near(24.5, 0.0, 3.2, 2.4, Sign::QbrWelcome);

    strips(p, Mat::LightPanel, Mat::Candle, 6.0, 0.0);
    lamps(
        p,
        Color::srgb(1.0, 0.86, 0.62),
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
                ("door:campus", (-50.0, 0.0), Sign::QbrToDay),
                ("door:qbr/2", (48.0, 6.0), Sign::QbrToTheatre),
            ],
            packs: &[LOBBY.cell_packs()[0], LOBBY.cell_packs()[1], (10.0, 8.5)],
            // The contract wants a boss anchor even in a boss-less room: the exit pod's centre.
            boss: (36.0, 0.0),
            npc: None,
        },
    );
    theme(1)
}

/// Two plants either side of the lift doors.
fn wall_cabinet_plant(p: &mut Plan) {
    sad_plant(p, 46.5, -9.0);
    sad_plant(p, 46.5, 9.0);
}

// ------------------------------------------------------------------ qbr/2: presentation theatre

const THEATRE: Shape = Shape {
    arrival_x: -44.0,
    hall: (-8.0, 26.0, 11.0),
    cells: [(-20.0, -26.0), (6.0, 26.0)],
    pod: (42.0, 13.0, 12.0),
    pod_wall: Mat::Wall,
};

/// The boss stands at this spot of the pod, facing the arrival (west).
const GOLEM_AT: (f32, f32) = (44.0, 0.0);

/// World position of a telegraph cover offset: `(right, forward)` from the boss's spawn point
/// in its spawn facing (west), the same rule as combat_core's telegraph.
fn cover_spot((r, f): (f32, f32)) -> (f32, f32) {
    let yaw = yaw_towards(
        v(GOLEM_AT.0, 0.0, GOLEM_AT.1),
        v(THEATRE_ROUTE_START.0, 0.0, THEATRE_ROUTE_START.1),
    );
    let (right, fwd) = (
        Vec2::new(yaw.cos(), -yaw.sin()),
        Vec2::new(-yaw.sin(), -yaw.cos()),
    );
    let c = Vec2::new(GOLEM_AT.0, GOLEM_AT.1) + right * r + fwd * f;
    (c.x, c.y)
}

/// First point of the theatre route, which the boss faces (see `cast_zone`).
const THEATRE_ROUTE_START: (f32, f32) = (-48.0, 0.0);

/// The Animated Pie Chart painted on the pod floor: three coloured wedges, a rim and a lit pad
/// in each, exactly where the telegraph's cover sits. The lit pad is the safe slice.
fn pie_floor(p: &mut Plan, pads: &[(f32, f32)]) {
    let (cx, cz) = GOLEM_AT;
    let wedges = [
        (-60.0f32, 60.0f32, Mat::Red),
        (60.0, 180.0, Mat::Scr1),
        (180.0, 300.0, Mat::Yellow),
    ];
    for (a0, a1, m) in wedges {
        let n = 28;
        for k in 0..n {
            let a = (a0 + (a1 - a0) * (k as f32 + 0.5) / n as f32).to_radians();
            let len = 10.5;
            // Radial strips, each a slim slab from the centre outwards.
            p.slab(
                v(cx + a.cos() * len / 2.0, 0.006, cz + a.sin() * len / 2.0),
                -a,
                v(len, 0.008, 0.85),
                m,
            );
        }
    }
    floor_ring(p, cx, cz, 10.6, 40, Mat::White, 0.14);
    for a in [-60.0f32, 60.0, 180.0] {
        let r = a.to_radians();
        floor_line(
            p,
            pt(cx, cz),
            pt(cx + r.cos() * 10.6, cz + r.sin() * 10.6),
            0.14,
            Mat::White,
        );
    }
    for &(x, z) in pads {
        round_patch(p, x, z, 5.4, Mat::Dark);
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.014, z),
            v(5.0, 0.02, 5.0),
            Mat::NeonYellow,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.02, z),
            v(3.6, 0.02, 3.6),
            Mat::Candle,
        );
        p.light(v(x, 1.6, z), Color::srgb(1.0, 0.9, 0.4), 90_000.0);
    }
}

/// Painted ring of `n` slab segments around (cx, cz).
pub(super) fn floor_ring(p: &mut Plan, cx: f32, cz: f32, radius: f32, n: usize, mat: Mat, w: f32) {
    for k in 0..n {
        let (a, b) = (k as f32 * TAU / n as f32, (k as f32 + 1.0) * TAU / n as f32);
        floor_line(
            p,
            pt(cx + radius * a.cos(), cz + radius * a.sin()),
            pt(cx + radius * b.cos(), cz + radius * b.sin()),
            w,
            mat,
        );
    }
}

fn theatre(p: &mut Plan) -> Theme {
    THEATRE.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Wood, Mat::Gold, Mat::Trim);
    cove(p, Mat::NeonRed);

    runner(
        p,
        &[pt(-46.0, 0.0), pt(-30.0, 0.0), pt(0.0, 0.0), pt(42.0, 0.0)],
        2.4,
        Mat::Dark,
        Mat::Gold,
    );
    for x in [-30.0, -12.0, 0.0, 12.0] {
        pilaster(p, x, 10.9, Mat::Wood);
    }
    // The cinema: a screen as wide as the wall that says "Agenda" and then "Agenda".
    if let Some(wl) = wall_screen(p, -3.0, -10.9, 2.7, 20.0, 3.2) {
        wl.sign(p, 0.0, 2.7, 5.6, Sign::QbrAgenda);
        wl.put(p, -8.4, 2.7, 0.14, v(0.4, 2.4, 0.04), Mat::Candle);
        wl.put(p, 8.4, 2.7, 0.14, v(0.4, 2.4, 0.04), Mat::Candle);
    }
    for (k, x) in (0..22usize).map(|k| (k, -12.5 + k as f32 * 0.78)) {
        if (-4.6..-3.2).contains(&x) {
            continue;
        }
        for (r, z) in [-4.5, -5.8, -7.1, -8.4].into_iter().enumerate() {
            if (k + r) % 11 != 5 {
                cinema_seat(p, x, z, PI);
            }
        }
    }
    // The laser pointer on its stand, the red dot on the screen and the clicker with no batteries.
    p.prim(
        PrimKind::Cylinder,
        v(-10.0, 0.6, -9.6),
        v(0.06, 1.2, 0.06),
        Mat::Metal,
    );
    p.boxed(v(-10.0, 1.25, -9.6), v(0.3, 0.1, 0.1), Mat::Red, false);
    p.prim(
        PrimKind::Sphere,
        v(-9.2, 2.7, -10.78),
        v(0.2, 0.2, 0.05),
        Mat::NeonRed,
    );
    p.sign_near(-10.0, -10.9, 0.9, 1.3, Sign::QbrLaser);
    p.prop(&SIDE_TABLE_DRAWERS, v(8.0, 0.0, -9.9), 0.0, true);
    p.boxed(v(8.0, 0.95, -9.9), v(0.28, 0.05, 0.08), Mat::Dark, false);
    p.sign_near(8.0, -10.9, 1.9, 1.5, Sign::QbrClicker);
    for x in [-15.5, 9.5] {
        p.prop(&SPEAKER, v(x, 0.0, -10.3), 0.0, true);
        p.prop(&SPEAKER, v(x + 0.6, 0.0, -10.3), 0.0, true);
    }
    preread(p, "prop:preread_3", -16.0, 7.6);
    preread(p, "prop:preread_4", 12.0, -8.3);
    // South side: the lectern, the printed slides, fruit and an AI-FIRST banner.
    p.boxed(v(-3.0, 0.55, 8.6), v(0.9, 1.1, 0.7), Mat::Wood, true);
    p.boxed(v(-3.0, 1.15, 8.5), v(0.8, 0.06, 0.6), Mat::Gold, false);
    p.prop(&LAPTOP, v(-3.0, 1.18, 8.6), PI, false);
    for (k, x) in [-27.0, -22.0, -14.0, -9.0, 4.0, 10.0, 15.0]
        .into_iter()
        .enumerate()
    {
        slide_pile(
            p,
            x,
            9.0 + (k % 2) as f32 * 0.6,
            20 + (k * 7) % 30,
            10 + k as i32,
        );
    }
    p.sign_near(-18.0, 10.9, 2.6, 2.8, Sign::QbrSlides);
    banner(p, -25.0, 10.9, 4.8, 2.5, Tex::AiFirst, 1.5);
    banner(p, 8.0, 10.9, 3.6, 2.5, Tex::PosterPie, 0.8);
    fruit_trolley(p, -26.5, -9.0, FRAC_PI_2, 7);
    fruit_trolley(p, 14.5, 7.5, FRAC_PI_2, 8);
    greenery(p, -32.0, -9.0);
    greenery(p, 16.0, -9.0);
    greenery(p, -32.0, 9.0);
    lounge(p, -26.0, 5.5, PI, true);

    // North cell: the slide archive. Shelves of printouts, a copier and a very tired chair.
    for k in 0..5 {
        p.prop(
            &BOOKCASE_CLOSED_WIDE,
            v(-26.0 + k as f32 * 1.7, 0.0, -32.8),
            0.0,
            true,
        );
    }
    chart_printer(p, -14.0, -32.0, Face::PosZ, 3);
    slide_pile(p, -22.0, -21.0, 40, 31);
    slide_pile(p, -16.5, -21.0, 28, 32);
    p.sign_near(-20.0, -32.9, 3.4, 2.8, Sign::QbrSlides);
    // South cell: the green room. A mirror of bulbs, a sofa, a fruit trolley and a lot of water.
    lounge(p, 6.0, 31.0, PI, true);
    for k in 0..5 {
        p.boxed(
            v(-1.2 + k as f32 * 0.7, 2.4, 32.9),
            v(0.2, 0.2, 0.05),
            Mat::Candle,
            false,
        );
    }
    fruit_trolley(p, 11.0, 22.0, 0.0, 9);
    vending(p, 12.0, 32.4, Face::NegZ);
    p.sign_near(6.0, 32.9, 3.0, 2.6, Sign::QbrToDay);

    // The pod: the hockey-stick screen on the far wall, the pie chart on the floor.
    hockey_screen(p, 54.9, 0.0, 2.7, 21.0, 3.4);
    let pads = [
        cover_spot((-7.0, 4.0)),
        cover_spot((7.0, 4.0)),
        cover_spot((0.0, -6.5)),
    ];
    pie_floor(p, &pads);
    banner(p, 36.0, 11.9, 4.8, 2.5, Tex::AiFirst, 1.5);
    banner(p, 36.0, -11.9, 3.6, 2.5, Tex::PosterBars, 1.0);
    p.sign_near(44.0, -11.9, 3.4, 3.0, Sign::QbrPie);
    for (x, z) in [(52.0, 8.5), (52.0, -8.5), (31.5, 9.0), (31.5, -9.0)] {
        slide_pile(p, x, z, 44, (x + z) as i32);
    }
    for z in [-10.0, 10.0] {
        p.prop(&SPEAKER, v(53.0, 0.0, z), -FRAC_PI_2, true);
    }
    p.boxed(v(31.0, 0.45, -5.0), v(0.7, 0.9, 0.9), Mat::Metal, true);
    p.boxed(v(31.0, 1.0, -5.0), v(0.5, 0.2, 0.4), Mat::Dark, false);
    p.boxed(
        v(31.0, 1.0, -5.22),
        v(0.18, 0.18, 0.06),
        Mat::ScreenGlow,
        false,
    );

    // Stage dressing: red curtains either side of the screen, footlights along the front, rope.
    curtains(p, 54.9, -10.5, 5);
    curtains(p, 54.9, 8.2, 5);
    for k in 0..16 {
        p.boxed(
            v(33.0, 0.06, -9.0 + k as f32 * 1.2),
            v(0.2, 0.12, 0.3),
            Mat::NeonRed,
            false,
        );
    }
    rope_line(p, -40.0, 28.0, -1.7, 3.5);
    rope_line(p, -40.0, 28.0, 1.7, 3.5);
    for z in [-4.0, 4.0] {
        p.light(v(40.0, 3.6, z * 2.0), Color::srgb(1.0, 0.9, 0.7), 260_000.0);
    }
    p.prop(&LOUNGE_SOFA_OTTOMAN, v(30.0, 0.0, 2.8), 0.0, false);
    p.prop(&TV_VINTAGE, v(30.5, 0.0, 10.0), PI, true);
    p.prop(&TABLE_CROSS_CLOTH, v(50.5, 0.0, 10.4), PI, true);
    p.prop(&TABLE_CROSS_CLOTH, v(50.5, 0.0, -10.4), 0.0, true);
    glass_of_water(p, 50.0, 0.83, 10.4);
    glass_of_water(p, 51.0, 0.83, -10.4);

    strips(p, Mat::LightPanel, Mat::Candle, 6.0, 0.2);
    lamps(
        p,
        Color::srgb(1.0, 0.6, 0.55),
        &[
            (-44.0, 0.0),
            (-28.0, 0.0),
            (-12.0, 6.0),
            (0.0, 6.0),
            (12.0, 0.0),
            (-20.0, -26.0),
            (6.0, 26.0),
            (36.0, -6.0),
            (36.0, 6.0),
            (50.0, 0.0),
        ],
        400_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:qbr/1", (-50.0, 0.0), Sign::QbrToLobby),
                ("door:qbr/3", (55.0, 6.0), Sign::QbrToWar),
            ],
            packs: &THEATRE.cell_packs(),
            boss: GOLEM_AT,
            npc: None,
        },
    );
    theme(2)
}

// ------------------------------------------------------------------ qbr/3: analytics war room

const WAR: Shape = Shape {
    arrival_x: -44.0,
    hall: (-8.0, 22.0, 17.0),
    cells: [(-18.0, -30.0), (8.0, 30.0)],
    pod: (40.0, 14.0, 12.0),
    pod_wall: Mat::Wall,
};

/// An analyst's desk: two screens with a dashboard each, all lines going up and to the right, a
/// mug, a stack of printouts and a chair pushed back from a deadline.
fn analyst_desk(p: &mut Plan, x: f32, z: f32, face: Face, seed: i32) {
    let texs = [
        Tex::NocGraph,
        Tex::Kpi,
        Tex::Roadmap,
        Tex::PosterBars,
        Tex::Leaderboard,
        Tex::NocRegions,
    ];
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        let top = DESK.size[1] * FURNITURE_SCALE;
        p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
        for (k, s) in [-0.5, 0.5].into_iter().enumerate() {
            p.prop(&SCREEN, v(s, top, -0.2), 0.0, false);
            p.panel(
                v(s, top + 0.38, -0.12),
                0.0,
                Vec2::new(0.7, 0.44),
                texs[(seed as usize + k * 3) % texs.len()],
            );
        }
        p.prop(&KEYBOARD, v(0.0, top, 0.25), 0.0, false);
        p.prop(
            &CHAIR_DESK,
            v(0.2, 0.0, 1.0),
            PI + 0.5 * (rnd(seed, 9) - 0.5),
            false,
        );
        p.block(v(0.2, 0.52, 1.0), v(0.6, 0.8, 0.6));
        p.prim(
            PrimKind::Cylinder,
            v(-0.95, top + 0.06, 0.1),
            v(0.1, 0.12, 0.1),
            Mat::White,
        );
        p.boxed(
            v(0.95, top + 0.07, 0.15),
            v(0.4, 0.12, 0.3),
            Mat::Paper,
            false,
        );
    });
}

fn war_room(p: &mut Plan) -> Theme {
    WAR.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Dark, Mat::Metal, Mat::Trim);
    cove(p, Mat::Cold);

    // Rows of analysts. Every desk faces the wall of dashboards and has two of its own.
    for (k, x) in [-26.0, -19.0, -12.0, -5.0, 2.0, 9.0]
        .into_iter()
        .enumerate()
    {
        analyst_desk(p, x, -10.0, Face::NegZ, k as i32);
        analyst_desk(p, x + 1.0, 10.0, Face::PosZ, k as i32 + 8);
    }
    for (k, x) in [-22.0, -15.0, -8.0, -1.0, 6.0].into_iter().enumerate() {
        analyst_desk(p, x, -6.4, Face::NegZ, k as i32 + 20);
        analyst_desk(p, x + 1.0, 6.4, Face::PosZ, k as i32 + 30);
    }
    // North wall: dashboards all the way, every line up and to the right, the SSOT triptych.
    let walls = [
        Tex::NocGraph,
        Tex::Kpi,
        Tex::Roadmap,
        Tex::PosterBars,
        Tex::Leaderboard,
        Tex::WarRoom,
        Tex::NocRegions,
        Tex::Gantt,
    ];
    for (k, x) in [-27.0, -12.5, 6.0, 12.0].into_iter().enumerate() {
        p.poster_near(x, -16.9, 2.5, Vec2::new(3.0, 1.9), walls[k % walls.len()]);
    }
    spreadsheet_trio(p, -2.0, -16.9);
    for (k, x) in [-27.0, -20.0, -14.0, -8.0, -2.0, 4.0, 12.0]
        .into_iter()
        .enumerate()
    {
        p.poster_near(
            x,
            16.9,
            2.5,
            Vec2::new(3.0, 1.9),
            walls[(k + 3) % walls.len()],
        );
        if k % 2 == 0 {
            p.sign_near(x, 16.9, 3.85, 2.2, Sign::QbrDash);
        }
    }
    // The printers that print charts nobody asked for, and the paper they leave.
    for (k, x) in [-27.0, -24.0, -21.0].into_iter().enumerate() {
        chart_printer(p, x, 15.6, Face::NegZ, k as i32);
    }
    p.sign_near(-24.0, 16.9, 3.0, 3.0, Sign::QbrPrinter);
    // Whiteboards of buzzwords at the west wall, glass meeting pods in the corners.
    whiteboard(p, -29.9, -8.0, 2.3, 3.4, Sign::QbrBuzz);
    whiteboard(p, -29.9, 9.0, 2.3, 3.4, Sign::QbrBuzz);
    whiteboard(p, 13.9, -9.0, 2.3, 3.0, Sign::QbrBuzz);
    glass_pod(p, 10.0, 12.8, 5.0, 3.6);
    glass_pod(p, -27.0, -13.0, 5.0, 3.6);
    greenery(p, 13.0, -14.5);
    greenery(p, -28.0, 14.0);
    slide_pile(p, -3.0, -14.0, 38, 41);
    slide_pile(p, 3.5, 14.5, 30, 42);
    fruit_trolley(p, 12.5, 4.5, FRAC_PI_2, 12);
    tags(
        p,
        &[
            ("prop:source_receipt_1", -14.0, 3.6),
            ("prop:source_receipt_2", 4.0, -3.6),
            ("prop:source_receipt_3", 8.0, 24.5),
        ],
    );
    // Overhead cabling on the sides only: the camera rides at tray height behind the player.
    p.sign_near(-29.9, 0.5, 3.2, 3.0, Sign::QbrToTheatre);
    // Arrival: a desk with a printout of the last quarter, upside down.
    p.boxed(v(-48.0, 0.55, 5.0), v(1.0, 1.1, 2.4), Mat::Wood, true);
    p.prop(&LAPTOP, v(-48.0, 1.13, 5.0), -FRAC_PI_2, false);
    slide_pile(p, -46.0, -4.5, 22, 43);
    p.sign_near(-49.9, -4.5, 2.4, 2.4, Sign::QbrToTheatre);
    // Cells: a coffee corner with a laptop wall, and the paper store.
    for k in 0..4 {
        p.prop(
            &KITCHEN_CABINET,
            v(-24.0 + k as f32 * 2.1, 0.0, -36.0),
            0.0,
            true,
        );
    }
    p.prop(&COFFEE_MACHINE, v(-22.0, 0.94, -36.1), 0.0, false);
    chart_printer(p, -10.0, -35.0, Face::PosZ, 5);
    slide_pile(p, -12.0, -24.0, 40, 44);
    for k in 0..4 {
        p.prop(
            &BOOKCASE_CLOSED_WIDE,
            v(2.0 + k as f32 * 1.7, 0.0, 36.0),
            PI,
            true,
        );
    }
    vending(p, 14.0, 36.0, Face::NegZ);
    p.sign_near(8.0, 36.9, 2.8, 2.8, Sign::QbrTrust);
    // The pod: a wall of every dashboard there is, glass pods either side, Dana's lectern.
    for row in 0..2 {
        for col in 0..6 {
            let t = walls[(row * 3 + col) % walls.len()];
            p.poster_near(
                54.0,
                -9.0 + col as f32 * 3.6,
                1.65 + row as f32 * 1.75,
                Vec2::new(3.2, 1.6),
                t,
            );
        }
    }
    p.sign_near(54.0, 0.0, 4.0, 5.0, Sign::QbrDash);
    glass_pod(p, 30.5, -9.0, 5.0, 4.0);
    glass_pod(p, 30.5, 9.0, 5.0, 4.0);
    p.sign_near(40.0, 11.9, 3.4, 3.0, Sign::QbrTrust);
    p.sign_near(40.0, -11.9, 3.4, 3.0, Sign::QbrSsot);
    for z in [-8.5, 8.5] {
        analyst_desk(p, 47.0, z, Face::PosX, 50 + z as i32);
    }
    slide_pile(p, 52.0, -10.0, 30, 46);

    // The pod is a data floor, too: desk islands along both walls and the paper that follows them.
    for (k, x) in [37.0, 43.0, 49.0].into_iter().enumerate() {
        analyst_desk(p, x, -10.3, Face::PosZ, 60 + k as i32);
        analyst_desk(p, x, 10.3, Face::NegZ, 70 + k as i32);
    }
    chart_printer(p, 52.4, -5.0, Face::NegX, 8);
    chart_printer(p, 52.4, 9.6, Face::NegX, 9);
    for z in [-6.5, 6.5] {
        floor_line(p, pt(28.0, z), pt(52.0, z), 0.12, Mat::LedBlue);
    }
    rope_line(p, 27.0, 38.0, -3.2, 2.9);
    rope_line(p, 27.0, 38.0, 3.2, 2.9);

    strips(p, Mat::Cold, Mat::LightPanel, 6.0, 0.1);
    lamps(
        p,
        Color::srgb(0.65, 0.8, 1.0),
        &[
            (-44.0, 0.0),
            (-22.0, 0.0),
            (-6.0, -9.0),
            (-6.0, 9.0),
            (8.0, 0.0),
            (-18.0, -30.0),
            (8.0, 30.0),
            (32.0, 0.0),
            (46.0, 0.0),
        ],
        400_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:qbr/2", (-50.0, 0.0), Sign::QbrToTheatre),
                ("door:qbr/4", (54.0, 6.0), Sign::QbrToBoard),
            ],
            packs: &WAR.cell_packs(),
            boss: (40.0, 0.0),
            npc: None,
        },
    );
    theme(3)
}

// ------------------------------------------------------------------ qbr/4: the boardroom

const BOARD: Shape = Shape {
    arrival_x: -44.0,
    hall: (-8.0, 22.0, 17.0),
    cells: [(-18.0, -30.0), (8.0, 30.0)],
    pod: (38.0, 16.0, 13.0),
    pod_wall: Mat::Wall,
};

fn boardroom(p: &mut Plan) -> Theme {
    BOARD.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Wood, Mat::Gold, Mat::Trim);
    cove(p, Mat::Candle);

    runner(
        p,
        &[pt(-46.0, 0.0), pt(-30.0, 0.0), pt(0.0, 0.0), pt(36.0, 0.0)],
        2.6,
        Mat::Dark,
        Mat::Gold,
    );
    // The gallery hall: former CEOs along both long walls, each one blinking out of turn.
    for (k, x) in [-27.0, -23.0, -9.5, -5.0, -0.5, 4.0, 8.5, 12.0]
        .into_iter()
        .enumerate()
    {
        ceo_portrait(p, x, -16.9, k as i32);
    }
    for (k, x) in [-27.0, -23.0, -19.0, -14.5, -10.0, -5.5, -1.0, 3.0, 12.0]
        .into_iter()
        .enumerate()
    {
        ceo_portrait(p, x, 16.9, k as i32 + 3);
    }
    for x in [-25.0, -21.0, -7.0, -2.5, 2.0, 6.0, 10.0] {
        pilaster(p, x, -16.9, Mat::Marble);
    }
    // North wall centre: the hockey stick, huge. South: the SYNERGY wall and a banner.
    hockey_screen(p, -4.5, -16.9, 2.7, 12.0, 3.2);
    synergy_wall(p, -4.0, 16.9, 7.0);
    banner(p, 8.0, 16.9, 4.5, 2.5, Tex::AiFirst, 1.5);
    // Lounge by the route, and water: a single glass per seat does not extend to the hall.
    for (k, x) in [-24.0, -12.0, 0.0, 10.0].into_iter().enumerate() {
        p.prop(&LOUNGE_DESIGN_SOFA, v(x, 0.0, -10.0), 0.0, true);
        p.prop(&COFFEE_TABLE_GLASS, v(x, 0.0, -8.0), 0.0, true);
        p.prop(
            &LOUNGE_DESIGN_CHAIR,
            v(x + 1.8, 0.0, -9.0),
            -FRAC_PI_2,
            true,
        );
        p.prop(&LAMP_SQUARE_TABLE, v(x - 1.4, 1.0, -10.5), 0.0, false);
        glass_of_water(p, x + 0.2, 0.55, -8.0);
        if k % 2 == 0 {
            p.prop(&LOUNGE_SOFA_CORNER, v(x + 4.0, 0.0, 11.0), PI, true);
        }
    }
    p.prop(&RUG, v(-12.0, 0.0, -8.5), 0.0, false);
    p.prop(&RUG, v(0.0, 0.0, -8.5), 0.0, false);
    preread_note(p);
    slide_pile(p, -26.5, -12.5, 46, 51);
    slide_pile(p, 13.0, -12.0, 34, 52);
    fruit_trolley(p, -22.0, 14.0, 0.0, 14);
    fruit_trolley(p, -17.0, 14.0, 0.0, 15);
    fruit_trolley(p, 12.5, 10.0, FRAC_PI_2, 16);
    glass_pod(p, -27.0, 12.0, 5.0, 4.0);
    glass_pod(p, 14.0, -3.0, 2.0, 2.0);
    greenery(p, -28.0, -15.0);
    greenery(p, 14.5, 15.0);
    whiteboard(p, -29.9, 0.5, 2.3, 3.6, Sign::QbrBuzz);
    p.sign_near(-29.9, 8.5, 3.4, 3.0, Sign::QbrBoard);
    // Arrival.
    p.boxed(v(-48.0, 0.55, 5.0), v(1.0, 1.1, 2.4), Mat::Wood, true);
    p.prop(&LAPTOP, v(-48.0, 1.13, 5.0), -FRAC_PI_2, false);
    p.sign_near(-49.9, -4.5, 2.4, 2.4, Sign::QbrToWar);
    greenery(p, -39.0, 5.0);
    // North cell: the minutes archive, one shelf and eleven years of "learnings".
    for k in 0..5 {
        p.prop(
            &BOOKCASE_CLOSED_WIDE,
            v(-26.0 + k as f32 * 1.7, 0.0, -36.2),
            0.0,
            true,
        );
    }
    slide_pile(p, -12.0, -24.5, 50, 53);
    slide_pile(p, -24.5, -24.0, 36, 54);
    chart_printer(p, -10.0, -35.0, Face::PosZ, 6);
    // South cell: the board's kitchenette, a bar, a fridge and a lot of leftover croissants.
    for k in 0..3 {
        p.prop(&KITCHEN_BAR, v(2.0 + k as f32 * 1.0, 0.0, 33.5), PI, true);
    }
    p.prop(&FRIDGE, v(14.0, 0.0, 36.0), PI, true);
    buffet(p, 6.0, 26.5, 7);
    vending(p, 12.0, 36.4, Face::NegZ);

    // The pod: the Board's table the length of a bowling lane, twice, with the dais between.
    boardroom_table(p, 25.0, 46.0, -9.5, 100);
    boardroom_table(p, 25.0, 46.0, 9.5, 200);
    floor_patch(p, 37.0, 0.0, 0.0, 9.0, 16.0, Mat::Gold);
    floor_patch(p, 37.0, 0.0, 0.0, 8.4, 15.4, Mat::Red);
    for z in [-5.0, 0.0, 5.0] {
        // Throne behind each head: a tall gold-trimmed leather chair facing west.
        p.framed(v(39.5, 0.0, z), -FRAC_PI_2, |p| {
            p.boxed(v(0.0, 0.6, 0.0), v(0.9, 0.2, 0.8), Mat::Dark, false);
            p.boxed(v(0.0, 1.5, -0.4), v(0.9, 1.8, 0.14), Mat::Dark, false);
            p.boxed(v(0.0, 2.5, -0.4), v(1.0, 0.16, 0.18), Mat::Gold, false);
            p.prim(
                PrimKind::Cylinder,
                v(0.0, 0.25, 0.0),
                v(0.14, 0.5, 0.14),
                Mat::Gold,
            );
            p.boxed(v(0.0, 0.05, 0.0), v(0.9, 0.1, 0.9), Mat::Gold, false);
        });
    }
    synergy_wall(p, 44.0, -12.9, 8.0);
    banner(p, 44.0, 12.9, 5.1, 2.5, Tex::AiFirst, 1.5);
    for (k, x) in [24.5, 28.5, 32.5].into_iter().enumerate() {
        ceo_portrait(p, x, -12.9, k as i32 + 6);
        ceo_portrait(p, x, 12.9, k as i32 + 9);
    }
    whiteboard(p, 50.0, 12.9, 2.4, 3.4, Sign::QbrBuzz);
    p.sign_near(54.0, 6.0, 3.4, 3.0, Sign::QbrBoard);
    fruit_trolley(p, 52.5, 3.5, FRAC_PI_2, 17);
    fruit_trolley(p, 52.5, 8.0, FRAC_PI_2, 18);
    glass_pod(p, 51.0, -9.5, 4.0, 3.0);
    for z in [-11.5, 11.5] {
        slide_pile(p, 23.5, z, 40, 55 + z as i32);
    }
    // The Board's glass of water, the one with no seat.
    p.prop(&SIDE_TABLE, v(40.5, 0.0, 9.0), 0.0, false);
    glass_of_water(p, 40.5, 0.93, 9.0);

    strips(p, Mat::LightPanel, Mat::Candle, 7.0, 0.0);
    lamps(
        p,
        Color::srgb(1.0, 0.8, 0.55),
        &[
            (-44.0, 0.0),
            (-22.0, 0.0),
            (-6.0, -9.0),
            (-6.0, 9.0),
            (8.0, 0.0),
            (-18.0, -30.0),
            (8.0, 30.0),
            (30.0, 0.0),
            (44.0, -6.0),
            (44.0, 6.0),
        ],
        400_000.0,
    );
    p.light(v(36.0, 3.6, 0.0), Color::srgb(1.0, 0.85, 0.6), 520_000.0);
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:qbr/3", (-50.0, 0.0), Sign::QbrToWar),
                ("door:campus", (54.0, -6.0), Sign::QbrToDay),
            ],
            packs: &BOARD.cell_packs(),
            boss: (36.0, 0.0),
            // 16 m from the boss and by the exit door, so the turn-in never pulls it.
            npc: Some((50.0, -9.0)),
        },
    );
    theme(4)
}

/// The note on the boardroom door: reserved for the Chair, also the Chair.
fn preread_note(p: &mut Plan) {
    p.sign_near(-29.9, -9.0, 2.2, 2.2, Sign::QbrReserved);
}
