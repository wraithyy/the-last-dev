//! The Migration dungeon (docs/DUNGEONS.md): the basement behind the Data Center, a lift-and-shift
//! gone wrong. basement/1 is the loading dock (a boss-less trash gauntlet), basement/2 the cold aisle
//! (Rollback Wraith), basement/3 the mainframe cage (the Consultants) and basement/4 the cutover war
//! room (Big Bang Cutover). Every room is the same corridor-hall-pod plan as the Onboarding so the
//! packs and the route stay testable; what differs is how much is in it: rack rows, cable trays,
//! shrink-wrapped legacy, a cage of beige mainframes, desks under a wall of energy drink cans. The
//! data center's dressing kit does the heavy lifting; no real company's floor plan is used.

use super::datacenter::dress::*;
use super::datacenter::mainframe;
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::models::{
    self, BOX_LARGE, CONE, ROBOT_ARM_A, SHIPPING_CONTAINER_A, SHIPPING_CONTAINER_B,
};
use super::offsite::{cove, floor_patch, wall_dress};
use super::props::{it_clutter, sad_plant, vending, water_cooler};
use super::shape::{rounded_rect, stroke};
use super::themes::{pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => ("basement/1", "Basement B: Loading Dock", dock(&mut p)),
        2 => ("basement/2", "Basement B: Cold Aisle", aisle(&mut p)),
        3 => ("basement/3", "Basement B: Mainframe Cage", cage(&mut p)),
        _ => (
            "basement/4",
            "Basement B: Cutover War Room",
            war_room(&mut p),
        ),
    };
    assemble(zone, level, name, false, p, theme)
}

/// Corridor from the arrival, a hall, a corridor to the far pod, and one cell either side of the
/// hall for packs. `hall` is (centre x, half x, half z), `cells` the two cell centres and `pod`
/// the far pod's centre x with its half sizes.
pub(super) struct Shape {
    pub(super) arrival_x: f32,
    pub(super) hall: (f32, f32, f32),
    pub(super) cells: [(f32, f32); 2],
    pub(super) pod: (f32, f32, f32),
    /// Wall material of the far pod (the Consultants sit in a glass meeting pod).
    pub(super) pod_wall: Mat,
}

impl Shape {
    pub(super) fn pods(&self, p: &mut Plan) {
        let w = Mat::Wall;
        let (hx, hhx, hhz) = self.hall;
        let (bx, bhx, bhz) = self.pod;
        let a = self.arrival_x;
        p.fp.pod(rounded_rect(pt(a, 0.0), 6.0, 6.0, 3.0, 0.0), w);
        p.fp.link(stroke(&[pt(a + 6.0, 0.0), pt(hx - hhx + 2.0, 0.0)], 4.0), w);
        p.fp.pod(rounded_rect(pt(hx, 0.0), hhx, hhz, 6.0, 0.0), w);
        p.fp.link(
            stroke(&[pt(hx + hhx - 2.0, 0.0), pt(bx - bhx + 2.0, 0.0)], 4.0),
            w,
        );
        p.fp.pod(rounded_rect(pt(bx, 0.0), bhx, bhz, 6.0, 0.0), self.pod_wall);
        for (cx, cz) in self.cells {
            p.fp.pod(rounded_rect(pt(cx, cz), 8.0, 7.0, 3.0, 0.0), w);
            let edge = hhz * cz.signum();
            p.fp.link(
                stroke(&[pt(cx, edge), pt(cx, cz - 4.0 * cz.signum())], 4.0),
                w,
            );
        }
        p.route = vec![
            pt(a - 4.0, 0.0),
            pt(a + 6.0, 0.0),
            pt(hx, 0.0),
            pt(bx - bhx + 2.0, 0.0),
            pt(bx, 0.0),
        ];
    }

    pub(super) fn cell_packs(&self) -> [(f32, f32); 2] {
        [0, 1].map(|k| (self.cells[k].0, self.cells[k].1 - self.cells[k].1.signum()))
    }
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, trim, accent, floor, fog, ambient, bright, range) = match level {
        // Loading dock: dusty concrete under sodium lamps.
        1 => (
            c(0.6, 0.62, 0.64),
            c(0.12, 0.13, 0.15),
            LinearRgba::rgb(2.6, 1.9, 0.3),
            [0.95, 0.9, 0.82],
            c(0.2, 0.2, 0.19),
            c(0.85, 0.85, 0.95),
            620.0,
            (28.0, 90.0),
        ),
        // Cold aisle: icy light and a mist that never clears.
        2 => (
            c(0.42, 0.52, 0.66),
            c(0.07, 0.09, 0.13),
            LinearRgba::rgb(0.2, 1.7, 2.6),
            [0.7, 0.88, 1.1],
            c(0.46, 0.58, 0.7),
            c(0.55, 0.7, 1.0),
            640.0,
            (10.0, 58.0),
        ),
        // Mainframe cage: beige gear, amber lamps, a smell of warm dust.
        3 => (
            c(0.56, 0.5, 0.42),
            c(0.14, 0.11, 0.08),
            LinearRgba::rgb(3.0, 1.9, 0.4),
            [0.95, 0.88, 0.78],
            c(0.22, 0.18, 0.13),
            c(1.0, 0.82, 0.6),
            800.0,
            (24.0, 80.0),
        ),
        // War room: night shift, the clock the only thing that is awake.
        _ => (
            c(0.4, 0.33, 0.34),
            c(0.1, 0.06, 0.06),
            LinearRgba::rgb(4.0, 0.5, 0.4),
            [0.82, 0.7, 0.7],
            c(0.2, 0.09, 0.09),
            c(1.0, 0.62, 0.52),
            880.0,
            (24.0, 80.0),
        ),
    };
    Theme {
        wall,
        trim,
        accent,
        fabric: [c(0.3, 0.38, 0.48), c(0.4, 0.34, 0.3), c(0.3, 0.3, 0.34)],
        floor: FloorKind::Tile(floor),
        fog,
        ambient,
        ambient_brightness: bright,
        sun: c(0.8, 0.85, 1.0),
        sun_illuminance: 2500.0,
        fog_range: range,
    }
}

// ------------------------------------------------------------------ shared pieces

/// Wooden pallet with two boxes of "legacy" under shrink wrap (a glass shell with a label).
fn pallet(p: &mut Plan, x: f32, z: f32, yaw: f32, label: bool) {
    p.boxed(v(x, 0.08, z), v(1.9, 0.16, 1.8), Mat::Wood, false);
    models::put(p, &BOX_LARGE, v(x, 0.16, z), yaw, 1.6, true);
    models::put(p, &BOX_LARGE, v(x, 1.04, z), yaw + 0.1, 1.5, false);
    p.boxed(v(x, 1.1, z), v(2.0, 2.0, 1.9), Mat::Glass, false);
    if label {
        p.framed(v(x, 0.0, z), yaw, |p| {
            p.sign(v(0.0, 1.2, 1.0), 0.0, 0.9, Sign::MigLegacy);
        });
    }
}

/// Half-raised roller shutter in a dock bay on the nearest wall.
fn dock_bay(p: &mut Plan, x: f32, z: f32) {
    let Some(w) = WallSpot::at(p, x, z) else {
        return;
    };
    w.put(p, 0.0, 1.7, 0.06, v(3.6, 3.4, 0.08), Mat::Dark);
    for k in 0..7 {
        w.put(
            p,
            0.0,
            3.15 - k as f32 * 0.13,
            0.12,
            v(3.4, 0.1, 0.05),
            Mat::Metal,
        );
    }
    for sx in [-1.95, 1.95] {
        for k in 0..6 {
            let m = if k % 2 == 0 { Mat::Hazard } else { Mat::Dark };
            w.put(p, sx, 0.25 + k as f32 * 0.55, 0.08, v(0.3, 0.55, 0.1), m);
        }
    }
    w.put(p, 0.0, 3.5, 0.1, v(4.2, 0.2, 0.14), Mat::Hazard);
    w.put(p, 0.0, 3.75, 0.1, v(0.3, 0.12, 0.1), Mat::LedRed);
}

/// A whiteboard on the nearest wall: aluminium frame, the sign as its content, a marker tray.
pub(super) fn whiteboard(p: &mut Plan, x: f32, z: f32, y: f32, width: f32, sign: Sign) {
    let Some(w) = WallSpot::at(p, x, z) else {
        return;
    };
    let (cw, ch) = sign_canvas(sign);
    let h = width * ch as f32 / cw as f32;
    w.put(p, 0.0, y, 0.04, v(width + 0.2, h + 0.2, 0.05), Mat::Metal);
    w.sign(p, 0.0, y, width, sign);
    w.put(
        p,
        0.0,
        y - h / 2.0 - 0.14,
        0.12,
        v(width * 0.7, 0.05, 0.1),
        Mat::Metal,
    );
    for (k, m) in [Mat::Red, Mat::NeonCyan, Mat::Dark].into_iter().enumerate() {
        w.put(
            p,
            -0.4 + k as f32 * 0.3,
            y - h / 2.0 - 0.1,
            0.14,
            v(0.2, 0.04, 0.04),
            m,
        );
    }
}

/// A wall of tickets: sticky notes in rows, each one blocked by the next.
fn ticket_wall(p: &mut Plan, x: f32, z: f32, y: f32) {
    let Some(w) = WallSpot::at(p, x, z) else {
        return;
    };
    w.put(p, 0.0, y, 0.03, v(6.4, 2.6, 0.04), Mat::Cardboard);
    let notes = [Mat::Yellow, Mat::Paper, Mat::Red, Mat::Beige, Mat::Yellow];
    for row in 0..6 {
        for col in 0..16 {
            let k = (row * 16 + col) as i32;
            let (jx, jy) = (rnd(k, 3) * 0.06, rnd(k, 4) * 0.05);
            w.put(
                p,
                -3.0 + col as f32 * 0.4 + jx,
                y - 1.05 + row as f32 * 0.42 + jy,
                0.07,
                v(0.3, 0.3, 0.02),
                notes[(k as usize + row) % notes.len()],
            );
        }
    }
    // String between the tickets: the dependency graph.
    for k in 0..5 {
        w.put(
            p,
            -2.4 + k as f32 * 1.2,
            y - 0.4 + (k % 2) as f32 * 0.6,
            0.1,
            v(1.5, 0.02, 0.01),
            Mat::Red,
        );
    }
    w.sign(p, 0.0, y + 1.75, 3.0, Sign::MigTickets);
}

/// A tight cluster of empty energy drink cans on a surface at height `y`.
fn cans(p: &mut Plan, x: f32, y: f32, z: f32, n: usize, seed: i32) {
    let mats = [
        Mat::NeonYellow,
        Mat::Scr2,
        Mat::Red,
        Mat::Chrome,
        Mat::NeonCyan,
    ];
    for k in 0..n {
        let i = k as i32 + seed * 31;
        let (dx, dz) = (
            (k % 7) as f32 * 0.115 - 0.34 + (rnd(i, 1) - 0.5) * 0.04,
            (k / 7) as f32 * 0.14 + (rnd(i, 2) - 0.5) * 0.04,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x + dx, y + 0.07, z + dz),
            v(0.07, 0.14, 0.07),
            mats[(i as usize) % mats.len()],
        );
    }
}

/// A desk with a screen, a keyboard, a chair pushed back and the 14 cans of the night.
fn war_desk(p: &mut Plan, x: f32, z: f32, yaw: f32, seed: i32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        let top = DESK.size[1] * FURNITURE_SCALE;
        p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
        p.prop(&SCREEN, v(0.0, top, -0.25), 0.0, false);
        p.prop(&KEYBOARD, v(0.0, top, 0.1), 0.0, false);
        p.prop(
            &CHAIR_DESK,
            v(0.1, 0.0, 0.95),
            PI + 0.4 * (rnd(seed, 9) - 0.5),
            false,
        );
        p.block(v(0.1, 0.52, 0.95), v(0.6, 0.8, 0.6));
        cans(p, -0.55, top, -0.3, 14, seed);
    });
}

/// Hazard cones in a loose ring: somebody wanted to keep people out of this.
fn cones(p: &mut Plan, x: f32, z: f32, r: f32, n: usize) {
    for k in 0..n {
        let a = k as f32 * 2.0 * PI / n as f32 + 0.3;
        models::put(
            p,
            &CONE,
            v(x + r * a.cos(), 0.0, z + r * a.sin()),
            a,
            0.9,
            false,
        );
    }
}

/// A translucent outline of the previous release hovering above a rack slot.
fn ghost_release(p: &mut Plan, x: f32, z: f32, scale: f32, y: f32) {
    let (w, h, d) = (0.92 * scale, 1.15 * scale, 0.96 * scale);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        p.slab(
            v(x + sx * w / 2.0, y + h / 2.0, z + sz * d / 2.0),
            0.0,
            v(0.05 * scale, h, 0.05 * scale),
            Mat::Cold,
        );
    }
    for sy in [0.0, h] {
        for s in [-1.0, 1.0] {
            p.slab(
                v(x, y + sy, z + s * d / 2.0),
                0.0,
                v(w, 0.05 * scale, 0.05 * scale),
                Mat::Cold,
            );
            p.slab(
                v(x + s * w / 2.0, y + sy, z),
                0.0,
                v(0.05 * scale, 0.05 * scale, d),
                Mat::Cold,
            );
        }
    }
    p.boxed(
        v(x, y + h / 2.0, z),
        v(w * 0.96, h * 0.96, d * 0.96),
        Mat::Glass,
        false,
    );
}

/// The previous release's heartbeat: LED bars along a wall, tall at the spike, low between.
fn heartbeat(p: &mut Plan, x: f32, z: f32, y: f32, beats: usize) {
    let Some(w) = WallSpot::at(p, x, z) else {
        return;
    };
    const PROFILE: [f32; 12] = [
        0.08, 0.08, 0.12, 0.3, 0.1, 0.08, 0.7, 1.4, 0.2, 0.08, 0.16, 0.08,
    ];
    let n = beats * PROFILE.len();
    for k in 0..n {
        let h = PROFILE[k % PROFILE.len()];
        let l = (k as f32 - n as f32 / 2.0) * 0.2;
        let m = if h > 0.5 {
            Mat::BlinkRed
        } else {
            Mat::BlinkGreenA
        };
        w.put(p, l, y + h / 2.0, 0.08, v(0.14, h, 0.04), m);
    }
    w.put(
        p,
        0.0,
        y - 0.03,
        0.06,
        v(n as f32 * 0.2 + 0.3, 0.03, 0.03),
        Mat::Metal,
    );
    w.sign(p, 0.0, y + 2.0, 2.2, Sign::MigHeart);
}

/// Chain-link cage along a line: posts every 0.6 m, a top and a bottom rail, an invisible collider.
fn cage_fence(p: &mut Plan, a: Vec2, b: Vec2, solid: bool) {
    let (mid, d) = ((a + b) / 2.0, b - a);
    let len = d.length();
    let yaw = (-d.y).atan2(d.x);
    let n = (len / 0.6).floor() as i32;
    for i in 0..=n {
        let c = a + d * (i as f32 / n.max(1) as f32);
        p.prim(
            PrimKind::Cylinder,
            v(c.x, 1.4, c.y),
            v(0.05, 2.8, 0.05),
            Mat::Metal,
        );
    }
    for y in [0.15, 1.4, 2.8] {
        p.slab(v(mid.x, y, mid.y), yaw, v(len, 0.06, 0.06), Mat::Metal);
    }
    p.slab(v(mid.x, 1.4, mid.y), yaw, v(len, 2.6, 0.02), Mat::Glass);
    if solid {
        p.block(v(mid.x, 1.4, mid.y), v(len.max(0.3), 2.8, 0.3));
    }
}

/// A cable run to the ceiling with a label somebody typed by hand, wrongly.
fn labelled_cable(p: &mut Plan, x: f32, z: f32, yaw: f32, b: bool) {
    hanging_cable(p, x, z, 0.0, false);
    p.prim(
        PrimKind::Cylinder,
        v(x + 0.12, 2.2, z),
        v(0.06, 4.4, 0.06),
        Mat::Red,
    );
    p.sign(
        v(x, 1.5, z + 0.1),
        yaw,
        0.7,
        if b { Sign::MigLabelB } else { Sign::MigLabelA },
    );
}

fn tags(p: &mut Plan, list: &[(&'static str, f32, f32)]) {
    for &(name, x, z) in list {
        p.interactable(name, v(x, 0.5, z - 0.4), v(x, 0.0, z));
    }
}

/// Warm, patchy light for the whole room: a few real lights over many glowing strips.
fn lamps(p: &mut Plan, color: Color, list: &[(f32, f32)], intensity: f32) {
    point_lights(p, list, color, intensity);
}

// ------------------------------------------------------------------ basement/1: loading dock

const DOCK: Shape = Shape {
    arrival_x: -44.0,
    hall: (-6.0, 22.0, 17.0),
    cells: [(-14.0, -30.0), (-14.0, 30.0)],
    pod: (36.0, 12.0, 10.0),
    pod_wall: Mat::Wall,
};

fn dock(p: &mut Plan) -> Theme {
    DOCK.pods(p);
    p.build_walls();
    wall_bands(p, false);
    wall_dress(p, Mat::Concrete, Mat::Hazard, Mat::Trim);
    cove(p, Mat::Candle);

    // Painted bays and a walkway: yellow lines either side of the route.
    for z in [-4.6, 4.6] {
        floor_line(p, pt(-27.0, z), pt(14.0, z), 0.16, Mat::Hazard);
    }
    for x in [-24.0, -10.0, 4.0] {
        floor_line(p, pt(x, -16.0), pt(x, -9.0), 0.12, Mat::Yellow);
        floor_line(p, pt(x + 7.0, -16.0), pt(x + 7.0, -9.0), 0.12, Mat::Yellow);
    }
    // Three dock bays on the north wall, one with the leak that nobody reports.
    for x in [-20.0, -6.0, 8.0] {
        dock_bay(p, x, -16.9);
    }
    p.sign_near(-6.0, -16.9, 3.95, 3.0, Sign::MigDock);
    // Pallets of legacy in shrink wrap, three rows deep along the south side and by the bays.
    for (x, z, yaw) in [
        (-25.0, 14.0, 0.0),
        (-22.5, 11.0, 0.15),
        (-24.5, 8.4, -0.1),
        (-20.0, 14.5, 0.0),
        (-17.5, 12.0, 0.2),
        (-25.0, -13.0, 0.0),
        (-17.0, -13.5, 0.1),
        (-11.0, -13.0, 0.0),
        (-1.0, -13.5, -0.1),
        (13.0, -13.0, 0.0),
    ] {
        pallet(p, x, z, yaw, true);
    }
    // A second row of wrapped pallets just outside the walkway lines, so the aisle reads as a warehouse.
    for (x, z, yaw) in [
        (-22.0, -8.2, 0.1),
        (-17.0, 8.2, -0.1),
        (-12.0, -8.4, 0.0),
        (-8.0, 8.2, 0.15),
        (-3.0, -8.2, -0.1),
        (4.0, -8.4, 0.1),
    ] {
        pallet(p, x, z, yaw, true);
    }
    // The mainframe corner of the dock: the one thing every pallet is on its way to replace.
    mainframe(p, -27.9, -13.0);
    mainframe(p, -27.9, 11.5);
    ticket_wall(p, 15.9, -9.5, 2.3);
    // The forklift, stuck mid-turn with its forks in a pallet, and the beeping you can almost hear.
    models::put(
        p,
        &super::models::TRACTOR_SHOVEL,
        v(2.0, 0.0, -9.6),
        0.9,
        1.5,
        true,
    );
    models::put(p, &BOX_LARGE, v(4.4, 0.7, -10.9), 0.6, 1.3, false);
    p.boxed(v(2.0, 2.9, -9.6), v(0.2, 0.2, 0.2), Mat::BlinkAmber, false);
    cones(p, 2.4, -9.4, 3.0, 6);
    // Old monitors, a MISC box and the cable spaghetti, plus the second crate of the same.
    it_clutter(p, -2.0, 13.0);
    it_clutter(p, 10.5, 14.0);
    // The banner, the bucket and the leak: WE ARE GOING TO THE CLOUD, at 3 l per hour.
    if let Some(w) = WallSpot::at(p, 2.0, 16.9) {
        w.put(p, 0.0, 3.5, 0.05, v(5.6, 1.6, 0.04), Mat::Dark);
        w.sign(p, 0.0, 3.5, 5.2, Sign::MigCloud);
        for sx in [-2.6, 2.6] {
            w.put(p, sx, 2.6, 0.05, v(0.04, 0.6, 0.02), Mat::Metal);
        }
    }
    p.prim(
        PrimKind::Cylinder,
        v(5.0, 2.25, 13.2),
        v(0.05, 4.5, 0.05),
        Mat::Water,
    );
    p.prim(
        PrimKind::Cylinder,
        v(5.0, 0.2, 13.2),
        v(0.55, 0.4, 0.55),
        Mat::Metal,
    );
    p.slab(v(5.0, 0.012, 13.2), 0.0, v(2.4, 0.012, 2.0), Mat::Water);
    p.slab(v(5.0, 0.012, 13.2), 0.7, v(1.6, 0.013, 1.4), Mat::Glass);
    p.sign(v(6.6, 0.45, 12.2), -0.4, 0.8, Sign::MigLeak);
    cones(p, 5.0, 13.2, 1.7, 4);
    // Whiteboard with a migration plan of 14 phases whose last is just "???".
    whiteboard(p, -30.0, -2.0, 2.2, 3.2, Sign::MigPlan);
    whiteboard(p, -30.0, 3.0, 2.2, 1.8, Sign::MigNoRollback);
    // Overhead: tray on the route, pipes, a dead light, a dangling cable.
    // Trays hang beside the route, never over it: the orbit camera sits at tray height behind the player.
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(-28.0, z), pt(14.0, z), false);
    }
    cable_tray(p, pt(-14.0, 17.0), pt(-14.0, 38.0), false);
    cable_tray(p, pt(-14.0, -17.0), pt(-14.0, -38.0), false);
    pipe_run(p, pt(-28.0, 3.0), pt(14.0, 3.0), 4.22, Mat::Scr1);
    pipe_run(p, pt(-28.0, -3.0), pt(14.0, -3.0), 4.22, Mat::Red);
    hanging_cable(p, -8.0, 6.0, 1.1, true);
    hanging_cable(p, -7.6, 6.2, 1.5, false);
    floor_hole(p, 0.0, 6.0, 0.3);
    cable_bundle(p, &[(-18.0, 3.0), (-12.0, 6.0), (-6.0, 5.0)], Mat::Yellow);
    cable_bundle(p, &[(4.0, -3.0), (8.0, -6.0), (12.0, -5.0)], Mat::Red);
    wall_cabinet(p, -27.9, 8.0, false);
    wall_cabinet(p, -27.9, -8.0, false);
    extinguisher(p, 15.0, -9.0);
    extinguisher(p, -27.9, 14.0);
    // Arrival: a desk where a project manager is, in theory, managing the project.
    p.boxed(v(-48.0, 0.55, 5.0), v(1.0, 1.1, 2.4), Mat::Wood, true);
    p.boxed(v(-48.0, 1.12, 5.0), v(1.1, 0.05, 2.5), Mat::Beige, false);
    p.prop(&SCREEN, v(-48.0, 1.15, 5.6), -FRAC_PI_2, false);
    p.prop(&SCREEN, v(-48.0, 1.15, 4.4), -FRAC_PI_2, false);
    p.prop(&CHAIR_DESK, v(-46.6, 0.0, 5.0), PI + FRAC_PI_2, false);
    ticket_wall(p, -44.0, 6.9, 2.3);
    p.sign_near(-49.9, -4.5, 2.4, 2.2, Sign::MigDoor);
    // North cell: more pallets and a sad rack in wrap; south cell: legacy servers waiting for a lift.
    for (x, z) in [(-19.0, -34.5), (-9.5, -34.5), (-14.5, -36.0)] {
        pallet(p, x, z, 0.0, true);
    }
    rack_row(p, -20.0, -17.0, 35.8, PI, false, false);
    p.boxed(v(-18.5, 1.1, 35.8), v(3.2, 2.3, 1.2), Mat::Glass, false);
    sad_plant(p, -8.5, 35.8);
    p.sign_near(-14.0, -36.9, 2.6, 2.4, Sign::MigLegacy);
    // The exit pod: the LIFT AND SHIFT stack, and the ticket wall that says why.
    models::put(
        p,
        &SHIPPING_CONTAINER_A,
        v(37.5, 0.0, -6.8),
        FRAC_PI_2,
        7.3,
        true,
    );
    models::put(
        p,
        &SHIPPING_CONTAINER_B,
        v(37.0, 2.54, -6.6),
        FRAC_PI_2 + 0.12,
        7.3,
        false,
    );
    models::put(p, &BOX_LARGE, v(35.0, 5.1, -6.6), 0.4, 1.6, false);
    p.sign(v(37.5, 1.6, -5.4), 0.0, 3.0, Sign::MigLift);
    models::put(
        p,
        &SHIPPING_CONTAINER_B,
        v(30.0, 0.0, 7.6),
        FRAC_PI_2,
        7.3,
        true,
    );
    p.sign(v(30.0, 1.6, 6.2), 0.0, 3.0, Sign::MigLift);
    ticket_wall(p, 36.0, -10.4, 2.3);
    tags(
        p,
        &[
            ("prop:basement_tag_1", -19.0, -6.4),
            ("prop:basement_tag_2", 12.0, 9.5),
            ("prop:basement_tag_3", -9.0, -33.0),
        ],
    );
    p.sign_near(26.0, 0.0, 3.0, 2.4, Sign::MigToAisle);

    strips(p, Mat::LightPanel, Mat::Candle, 6.0, 0.12);
    lamps(
        p,
        Color::srgb(1.0, 0.78, 0.45),
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
        420_000.0,
    );
    lamps(
        p,
        Color::srgb(0.6, 0.75, 1.0),
        &[(-18.0, -12.0), (4.0, -12.0)],
        260_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:campus", (-50.0, 0.0), Sign::MigToDay),
                ("door:basement/2", (48.0, 6.0), Sign::MigToAisle),
            ],
            // Two cells and one pack in the hall, away from the leak.
            packs: &[DOCK.cell_packs()[0], DOCK.cell_packs()[1], (10.0, 8.5)],
            // The contract wants a boss anchor even in a boss-less room: the exit pod's centre.
            boss: (36.0, 0.0),
            npc: None,
        },
    );
    theme(1)
}

// ------------------------------------------------------------------ basement/2: cold aisle

const AISLE: Shape = Shape {
    arrival_x: -44.0,
    hall: (-8.0, 26.0, 11.0),
    cells: [(-20.0, -26.0), (6.0, 26.0)],
    pod: (42.0, 13.0, 12.0),
    pod_wall: Mat::Wall,
};

fn aisle(p: &mut Plan) -> Theme {
    AISLE.pods(p);
    p.build_walls();
    wall_bands(p, false);
    wall_dress(p, Mat::Concrete, Mat::Metal, Mat::Trim);
    cove(p, Mat::Cold);

    // The long aisle: two rows of racks with fronts inwards, a cross aisle at -12..-4.
    for (x0, x1) in [(-31.0, -12.0), (-4.0, 15.0)] {
        rack_row(p, x0, x1, -6.0, 0.0, false, true);
        rack_row(p, x0, x1, 6.0, PI, false, true);
        for z in [-5.1, 5.1] {
            floor_line(p, pt(x0, z), pt(x1, z), 0.08, Mat::Cold);
        }
        // The previous release, outlined above every third rack.
        let mut x = x0 + 1.5;
        while x < x1 {
            ghost_release(p, x, -6.0, 1.0, 2.55);
            ghost_release(p, x + 1.5, 6.0, 1.0, 2.55);
            x += 3.0;
        }
    }
    for x in [
        -29.5, -26.5, -23.5, -20.5, -17.5, -14.5, -2.5, 0.5, 3.5, 6.5, 9.5, 12.5,
    ] {
        vent_tile(p, x, -3.5, Mat::Cold);
        vent_tile(p, x, 3.5, Mat::Cold);
    }
    // Mist: low puffs along the aisle that the vents keep feeding.
    for (x, z, n) in [
        (-28.5, -3.5, 3),
        (-22.5, 3.5, 3),
        (-16.5, -3.5, 2),
        (-8.0, 0.0, 3),
        (-2.5, 3.5, 3),
        (3.5, -3.5, 3),
        (9.5, 3.5, 2),
        (14.0, -3.5, 3),
    ] {
        steam(p, x, z, n);
    }
    // Hot side behind the racks: cable trays, CRAC units at both ends, floor holes, labels.
    for z in [-9.0, 9.0] {
        for (x0, x1) in [(-31.0, -12.0), (-4.0, 15.0)] {
            cable_tray(p, pt(x0, z), pt(x1, z), false);
        }
    }
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(-31.0, z), pt(16.0, z), false);
    }
    cable_tray(p, pt(-8.0, -10.5), pt(-8.0, -4.5), false);
    cable_tray(p, pt(-8.0, 4.5), pt(-8.0, 10.5), false);
    pipe_run(p, pt(-32.0, 2.6), pt(16.0, 2.6), 4.22, Mat::Scr1);
    pipe_run(p, pt(-32.0, -2.6), pt(16.0, -2.6), 4.22, Mat::Red);
    for x in [-29.0, -22.0, 7.0, 13.0] {
        cooling_unit(p, x, -9.9, Face::PosZ, false);
        cooling_unit(p, x + 0.0, 9.9, Face::NegZ, false);
    }
    floor_hole(p, -8.0, 4.0, 0.4);
    floor_hole(p, -8.0, -4.0, -0.4);
    cable_bundle(p, &[(-26.0, -4.0), (-24.0, -1.0), (-21.0, 2.0)], Mat::Scr1);
    cable_bundle(p, &[(4.0, 4.0), (6.0, 1.0), (9.0, -2.0)], Mat::Yellow);
    // Heartbeats on both end walls and a giant ghost release over the aisle's far end.
    heartbeat(p, -33.8, 0.0, 1.2, 3);
    p.sign_near(-33.9, 6.5, 2.7, 2.2, Sign::ColdAisle);
    p.sign_near(-33.9, -6.5, 2.7, 2.4, Sign::DoNotUnplug);
    p.sign_near(15.9, -9.0, 2.9, 2.4, Sign::Uptime);
    p.sign_near(15.9, 9.0, 2.9, 2.4, Sign::Rack42);
    p.sign_near(0.0, -10.9, 3.0, 2.4, Sign::HotAisle);
    p.sign_near(-18.0, 10.9, 3.0, 2.4, Sign::NoFood);
    wall_cabinet(p, 17.8, 8.0, false);
    extinguisher(p, 17.8, -8.0);
    tags(
        p,
        &[
            ("prop:basement_tag_4", -22.0, 8.8),
            ("prop:basement_tag_5", 1.0, -8.2),
            ("prop:basement_tag_6", 12.0, 8.8),
        ],
    );
    // Arrival: the dock's last pallet and a jacket rack ("bring a jacket").
    p.prop(&COAT_RACK, v(-47.0, 0.0, -5.0), 0.0, false);
    p.prop(&COAT_RACK, v(-45.5, 0.0, -5.0), 0.0, false);
    p.sign_near(-49.9, 4.5, 2.4, 2.2, Sign::MigToDock);
    sad_plant(p, -48.5, 4.8);
    // North cell: the DevOps corner, a snapshot rack; south cell: a vending machine of cold coffee.
    rack_row(p, -26.0, -22.0, -31.8, PI, false, false);
    ghost_release(p, -24.0, -31.8, 1.4, 2.6);
    p.sign_near(-20.0, -32.9, 2.8, 2.4, Sign::Kubernetes);
    vending(p, 2.0, 32.8, Face::NegZ);
    water_cooler(p, 10.0, 32.8);
    p.sign_near(6.0, 32.9, 2.8, 2.4, Sign::ThisIsFine);
    // Boss pod: a shrine of racks along both sides, the floor rings, a huge ghost at the back wall.
    for z in [-10.0, 10.0] {
        rack_row(
            p,
            33.0,
            51.0,
            z,
            if z < 0.0 { 0.0 } else { PI },
            false,
            true,
        );
    }
    for x in [35.0, 41.0, 47.0] {
        ghost_release(p, x, -10.0, 1.2, 2.55);
        ghost_release(p, x, 10.0, 1.2, 2.55);
    }
    floor_ring(p, 42.0, 0.0, 6.2, 22, Mat::Cold, 0.1);
    floor_ring(p, 42.0, 0.0, 6.8, 22, Mat::Hazard, 0.08);
    ghost_release(p, 53.4, 0.0, 3.0, 0.6);
    p.sign(v(53.6, 4.0, 0.0), -FRAC_PI_2, 2.4, Sign::MigGhost);
    heartbeat(p, 54.9, -7.0, 1.0, 2);
    heartbeat(p, 54.9, 7.0, 1.0, 2);
    for dz in [-5.0, 5.0] {
        for dx in [-0.25, 0.0, 0.25] {
            hanging_cable(p, 36.0 + dx, dz, 1.25, dx == 0.0);
        }
    }
    steam(p, 42.0, -8.0, 3);
    steam(p, 42.0, 8.0, 3);
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(31.0, z), pt(53.0, z), false);
    }
    p.sign_near(30.0, 0.0, 3.0, 2.4, Sign::TempWarning);

    strips(p, Mat::Cold, Mat::Cold, 0.0, 0.1);
    lamps(
        p,
        Color::srgb(0.55, 0.8, 1.0),
        &[
            (-44.0, 0.0),
            (-26.0, -3.0),
            (-26.0, 3.0),
            (-10.0, 0.0),
            (2.0, -3.0),
            (2.0, 3.0),
            (12.0, 0.0),
            (-20.0, -26.0),
            (6.0, 26.0),
            (42.0, -5.0),
            (42.0, 5.0),
            (52.0, 0.0),
        ],
        360_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:basement/1", (-50.0, 0.0), Sign::MigToDock),
                ("door:basement/3", (55.0, 6.0), Sign::MigToCage),
            ],
            packs: &AISLE.cell_packs(),
            boss: (42.0, 0.0),
            npc: None,
        },
    );
    theme(2)
}

// ------------------------------------------------------------------ basement/3: mainframe cage

const CAGE: Shape = Shape {
    arrival_x: -44.0,
    hall: (-8.0, 22.0, 17.0),
    cells: [(-18.0, -30.0), (8.0, 30.0)],
    pod: (40.0, 14.0, 12.0),
    pod_wall: Mat::Glass,
};

/// A beige tape cabinet: two reels on top and a row of lamps, standing free.
fn tape_cabinet(p: &mut Plan, x: f32, z: f32, yaw: f32, seed: i32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 1.0, 0.0), v(1.8, 2.0, 0.9), Mat::Beige, true);
        p.boxed(v(0.0, 1.35, 0.46), v(1.5, 0.7, 0.02), Mat::Dark, false);
        for k in 0..2 {
            p.prim(
                PrimKind::Cylinder,
                v(-0.4 + k as f32 * 0.8, 1.4, 0.49),
                v(0.5, 0.04, 0.5),
                Mat::Chrome,
            );
        }
        for k in 0..8 {
            let m = [Mat::BlinkAmber, Mat::BlinkRed, Mat::BlinkGreenB][((k + seed) % 3) as usize];
            p.boxed(
                v(-0.7 + k as f32 * 0.2, 1.8, 0.47),
                v(0.08, 0.08, 0.02),
                m,
                false,
            );
        }
        p.boxed(v(0.0, 0.55, 0.46), v(1.5, 0.6, 0.02), Mat::Wood, false);
    });
}

fn cage(p: &mut Plan) -> Theme {
    CAGE.pods(p);
    p.build_walls();
    wall_bands(p, false);
    wall_dress(p, Mat::Beige, Mat::Metal, Mat::Trim);
    cove(p, Mat::Candle);

    // Two caged rooms flanking the route. North: tape cabinets and the robot nobody retired.
    cage_fence(p, pt(-14.0, -6.0), pt(-2.0, -6.0), true);
    cage_fence(p, pt(2.0, -6.0), pt(12.0, -6.0), true);
    cage_fence(p, pt(12.0, -6.0), pt(12.0, -15.5), true);
    cage_fence(p, pt(-14.0, -6.0), pt(-14.0, -15.5), true);
    cage_fence(p, pt(-2.0, -6.0), pt(-2.0, -7.6), false);
    cage_fence(p, pt(2.0, -6.0), pt(2.0, -7.6), false);
    for (k, x) in [-11.0, -8.0, -5.0, 5.0, 8.0].into_iter().enumerate() {
        tape_cabinet(p, x, -12.0, 0.0, k as i32);
    }
    models::put(p, &ROBOT_ARM_A, v(0.0, 0.0, -14.0), 0.0, 1.6, true);
    p.boxed(v(0.0, 0.2, -12.5), v(2.4, 0.4, 1.6), Mat::Metal, false);
    for x in [-9.0, -5.5, 5.5, 9.0] {
        mainframe(p, x, -16.9);
    }
    p.sign_near(-8.0, -6.0, 2.9, 2.6, Sign::MigCage);
    // South: the racks in rows behind a second cage, plus the labelled cables.
    cage_fence(p, pt(-14.0, 6.0), pt(-2.0, 6.0), true);
    cage_fence(p, pt(2.0, 6.0), pt(12.0, 6.0), true);
    cage_fence(p, pt(12.0, 6.0), pt(12.0, 15.5), true);
    cage_fence(p, pt(-14.0, 6.0), pt(-14.0, 15.5), true);
    rack_row(p, -12.0, -3.0, 15.0, PI, false, true);
    rack_row(p, 3.0, 11.0, 15.0, PI, false, true);
    rack_row(p, -12.0, -3.0, 10.5, 0.0, false, true);
    rack_row(p, 3.0, 11.0, 10.5, 0.0, false, true);
    for (k, (x, z)) in [(-9.0, 8.2), (-4.0, 8.2), (5.0, 8.2), (9.5, 8.2)]
        .into_iter()
        .enumerate()
    {
        labelled_cable(p, x, z, PI, k % 2 == 0);
    }
    for (k, (x, z)) in [(-8.0, -8.4), (-3.5, -8.4), (3.5, -8.4), (10.0, -8.4)]
        .into_iter()
        .enumerate()
    {
        labelled_cable(p, x, z, 0.0, k % 2 == 1);
    }
    cable_bundle(p, &[(-12.0, 3.0), (-8.0, 4.5), (-3.0, 3.0)], Mat::Yellow);
    cable_bundle(p, &[(4.0, -3.5), (8.0, -4.5), (11.0, -3.0)], Mat::Scr1);
    cable_bundle(p, &[(-6.0, 2.5), (0.0, 3.5), (6.0, 2.5)], Mat::Red);
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(-28.0, z), pt(14.0, z), false);
    }
    for x in [-8.0, -18.0] {
        cable_tray(p, pt(x, -16.0), pt(x, -4.5), false);
        cable_tray(p, pt(x, 4.5), pt(x, 16.0), false);
    }
    pipe_run(p, pt(-28.0, 3.0), pt(14.0, 3.0), 4.22, Mat::Scr1);
    pipe_run(p, pt(-28.0, -3.0), pt(14.0, -3.0), 4.22, Mat::Red);
    for x in [-26.0, 12.0] {
        cooling_unit(p, x, -16.0, Face::PosZ, false);
        cooling_unit(p, x, 16.0, Face::NegZ, false);
    }
    floor_hole(p, -22.0, 6.0, 0.3);
    floor_hole(p, -22.0, -6.0, -0.3);
    wall_cabinet(p, -29.9, 10.0, false);
    extinguisher(p, -29.9, -10.0);
    p.sign_near(-29.9, 0.0, 2.9, 3.0, Sign::MigRates);
    p.sign_near(13.9, 6.0, 2.8, 2.2, Sign::Mainframe);
    // Arrival and cells: a paper-jammed printer wall, a locked cupboard of manuals, a break nook.
    p.boxed(v(-48.0, 0.55, -5.0), v(1.0, 1.1, 2.4), Mat::Wood, true);
    p.prop(&LAPTOP, v(-48.0, 1.13, -5.0), -FRAC_PI_2, false);
    p.sign_near(-49.9, 4.5, 2.4, 2.2, Sign::MigToAisle);
    for k in 0..4 {
        p.prop(
            &BOOKCASE_CLOSED,
            v(-22.0 + k as f32 * 1.6, 0.0, -35.8),
            0.0,
            true,
        );
    }
    p.sign_near(-18.0, -36.9, 2.8, 2.4, Sign::Mainframe);
    mainframe(p, 6.0, 36.9);
    mainframe(p, 10.0, 36.9);
    water_cooler(p, 13.5, 35.5);
    // The glass meeting pod: a rug, a glass table, two designer chairs, the rate card.
    p.prop(&RUG_ROUND, v(40.0, 0.012, 0.0), 0.0, false);
    p.prop(&TABLE_GLASS, v(47.0, 0.0, -6.0), 0.0, false);
    p.prop(&CHAIR_MODERN, v(46.0, 0.0, -4.8), PI, false);
    p.prop(&CHAIR_MODERN, v(48.0, 0.0, -4.8), PI, false);
    p.prop(&LOUNGE_DESIGN_SOFA, v(47.0, 0.0, 8.6), PI, false);
    whiteboard(p, 53.9, 0.0, 2.4, 3.6, Sign::MigRates);
    whiteboard(p, 40.0, -11.9, 2.4, 3.2, Sign::MigCutover);
    for z in [-9.0, 9.0] {
        cage_fence(p, pt(27.0, z), pt(53.0, z), false);
    }
    cage_fence(p, pt(26.8, -11.0), pt(26.8, -4.5), false);
    cage_fence(p, pt(26.8, 4.5), pt(26.8, 11.0), false);
    // Tape cabinets along both long walls of the pod, fronts to the table, and a second meeting corner.
    for (k, x) in [29.0, 32.0, 35.0, 44.0, 47.0, 50.0].into_iter().enumerate() {
        tape_cabinet(p, x, -10.7, 0.0, k as i32);
        tape_cabinet(p, x + 1.0, 10.7, PI, k as i32 + 3);
    }
    p.prop(&TABLE_GLASS, v(32.0, 0.0, -6.0), 0.0, false);
    p.prop(&CHAIR_MODERN, v(31.0, 0.0, -4.8), PI, false);
    p.prop(&CHAIR_MODERN, v(33.0, 0.0, -4.8), PI, false);
    p.prop(&LOUNGE_DESIGN_SOFA, v(32.0, 0.0, 7.6), PI, false);
    sad_plant(p, 52.4, -9.8);
    sad_plant(p, 52.4, 9.8);
    mainframe(p, 53.9, -5.5);
    mainframe(p, 53.9, 5.5);
    p.light(v(40.0, 3.4, 0.0), Color::srgb(1.0, 0.92, 0.75), 520_000.0);
    p.sign_near(30.0, 0.0, 3.2, 2.4, Sign::MigCage);

    strips(p, Mat::LightPanel, Mat::Candle, 8.0, 0.0);
    lamps(
        p,
        Color::srgb(1.0, 0.82, 0.55),
        &[
            (-44.0, 0.0),
            (-22.0, 0.0),
            (-6.0, -11.0),
            (-6.0, 11.0),
            (8.0, 0.0),
            (-18.0, -30.0),
            (8.0, 30.0),
            (36.0, -5.0),
            (46.0, 5.0),
        ],
        400_000.0,
    );
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:basement/2", (-50.0, 0.0), Sign::MigToAisle),
                ("door:basement/4", (54.0, 6.0), Sign::MigToWar),
            ],
            packs: &CAGE.cell_packs(),
            boss: (40.0, 0.0),
            npc: None,
        },
    );
    theme(3)
}

// ------------------------------------------------------------------ basement/4: war room

const WAR: Shape = Shape {
    arrival_x: -44.0,
    hall: (-8.0, 22.0, 17.0),
    cells: [(-18.0, -30.0), (8.0, 30.0)],
    pod: (38.0, 16.0, 13.0),
    pod_wall: Mat::Wall,
};

fn war_room(p: &mut Plan) -> Theme {
    WAR.pods(p);
    p.build_walls();
    wall_bands(p, true);
    wall_dress(p, Mat::Dark, Mat::Metal, Mat::Trim);
    cove(p, Mat::NeonRed);

    // Desk rows either side of the route: every desk with its 14 cans, a screen on green.
    for (k, x) in [-26.0, -18.0, -10.0, -2.0, 6.0].into_iter().enumerate() {
        war_desk(p, x, -9.0, PI, k as i32);
        war_desk(p, x + 1.0, 9.0, 0.0, k as i32 + 8);
    }
    // Inner desk rows hug the walkway: the war room is full, and has been since Thursday.
    for (k, x) in [-22.0, -14.0, -6.0, 2.0].into_iter().enumerate() {
        war_desk(p, x, -5.6, PI, 60 + k as i32);
        war_desk(p, x + 1.0, 5.8, 0.0, 70 + k as i32);
    }
    for z in [-9.0, 9.0] {
        p.boxed(
            v(-10.0, 0.012, z * 0.6),
            v(36.0, 0.012, 0.3),
            Mat::Cable,
            false,
        );
    }
    // The wall of tickets and the wall of dashboards: green, and everyone pretends not to look.
    ticket_wall(p, -8.0, -16.9, 2.3);
    for (k, x) in [-24.0, -16.0, 0.0, 8.0].into_iter().enumerate() {
        p.sign_near(x, 16.9, 2.6, 2.6, Sign::MigDash);
        if k % 2 == 0 {
            p.poster_near(
                x,
                16.9,
                1.3,
                Vec2::new(1.9, 1.07),
                [Tex::NocGraph, Tex::NocRegions][k / 2 % 2],
            );
        }
    }
    whiteboard(p, -30.0, -3.0, 2.3, 3.4, Sign::MigCutover);
    whiteboard(p, -30.0, 4.0, 2.3, 2.0, Sign::MigNoRollback);
    // Cots, pizza boxes and a coffee maker: three days in, nobody leaves.
    for (x, z) in [(-27.5, 14.0), (-23.0, 14.0), (-27.5, -14.0)] {
        p.boxed(v(x, 0.3, z), v(1.8, 0.4, 0.8), Mat::Fabric1, true);
        p.boxed(v(x - 0.65, 0.55, z), v(0.4, 0.12, 0.6), Mat::Paper, false);
    }
    for (x, z, yaw) in [(-18.0, 14.2, 0.3), (-16.4, 13.8, -0.2), (-6.0, -13.5, 0.1)] {
        p.slab(v(x, 0.08, z), yaw, v(0.5, 0.14, 0.5), Mat::Cardboard);
        p.slab(v(x, 0.24, z), yaw + 0.2, v(0.5, 0.12, 0.5), Mat::Cardboard);
    }
    p.prop(&COFFEE_MACHINE, v(12.0, 0.0, 14.5), PI, false);
    cans(p, 12.0, 0.0, 15.6, 14, 99);
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(-28.0, z), pt(14.0, z), true);
    }
    cable_tray(p, pt(-12.0, 16.5), pt(-12.0, 4.5), true);
    cable_tray(p, pt(-12.0, -4.5), pt(-12.0, -16.5), true);
    pipe_run(p, pt(-28.0, 3.0), pt(14.0, 3.0), 4.22, Mat::Red);
    cable_bundle(p, &[(-20.0, 3.0), (-14.0, 5.5), (-8.0, 4.0)], Mat::Red);
    cable_bundle(p, &[(0.0, -3.0), (6.0, -6.0), (12.0, -4.5)], Mat::Yellow);
    floor_hole(p, -4.0, 5.0, 0.4);
    wall_cabinet(p, -29.9, 12.0, true);
    extinguisher(p, 15.9, -9.0);
    p.sign_near(15.9, 9.0, 2.8, 2.4, Sign::ThisIsFine);
    p.sign_near(-29.9, -12.0, 2.8, 2.4, Sign::MigHypercare);
    // Arrival: the hand-over desk and the pager nobody will take.
    p.boxed(v(-48.0, 0.55, 5.0), v(1.0, 1.1, 2.4), Mat::Wood, true);
    p.prop(&LAPTOP, v(-48.0, 1.13, 5.0), -FRAC_PI_2, false);
    p.sign_near(-49.9, -4.5, 2.4, 2.2, Sign::MigToCage);
    // Cells: the break room (fridge of drinks) and the paper archive.
    p.prop(&FRIDGE, v(-22.0, 0.0, -36.0), 0.0, true);
    cans(p, -22.0, 0.0, -34.6, 14, 55);
    for k in 0..3 {
        p.prop(
            &BOOKCASE_CLOSED_WIDE,
            v(-16.0 + k as f32 * 2.4, 0.0, -36.0),
            0.0,
            true,
        );
    }
    vending(p, 4.0, 36.0, Face::NegZ);
    water_cooler(p, 12.0, 36.0);
    p.sign_near(8.0, 36.9, 2.8, 2.4, Sign::MigNoRollback);
    // The pod: a billboard of a clock, the GO button, desks along the north and south walls.
    p.sign_near(53.0, 0.0, 3.0, 9.5, Sign::MigClock);
    p.boxed(v(53.7, 3.0, 0.0), v(0.12, 2.9, 10.2), Mat::Dark, false);
    p.light(v(51.0, 3.4, 0.0), Color::srgb(1.0, 0.25, 0.2), 520_000.0);
    for (k, x) in [28.0, 33.0, 38.0, 43.0].into_iter().enumerate() {
        war_desk(p, x, -11.0, PI, 20 + k as i32);
        war_desk(p, x + 1.0, 11.5, 0.0, 30 + k as i32);
    }
    // The red button under a plastic cover, taped shut and un-taped (the quest prop).
    let (bx, bz) = (51.0, 9.2);
    p.prim(
        PrimKind::Cylinder,
        v(bx, 0.55, bz),
        v(0.9, 1.1, 0.9),
        Mat::Metal,
    );
    p.prim(
        PrimKind::Cylinder,
        v(bx, 1.14, bz),
        v(0.75, 0.08, 0.75),
        Mat::Hazard,
    );
    p.prim(
        PrimKind::Cylinder,
        v(bx, 1.26, bz),
        v(0.5, 0.16, 0.5),
        Mat::NeonRed,
    );
    p.prim(
        PrimKind::Cylinder,
        v(bx, 1.36, bz),
        v(0.72, 0.36, 0.72),
        Mat::Glass,
    );
    p.slab(v(bx, 1.55, bz), 0.5, v(0.8, 0.02, 0.1), Mat::Yellow);
    p.slab(v(bx, 1.56, bz), -0.4, v(0.8, 0.02, 0.1), Mat::Paper);
    p.sign(v(bx, 0.9, bz - 0.47), 0.0, 0.7, Sign::MigNoRollback);
    p.interactable("prop:cutover_button", v(bx, 1.3, bz), v(bx, 0.0, bz - 1.6));
    floor_patch(p, bx, bz - 1.6, 0.0, 2.2, 2.0, Mat::Hazard);
    p.light(v(bx, 2.4, bz), Color::srgb(1.0, 0.2, 0.15), 260_000.0);
    // A second desk row in the pod, a lane for the fight kept clear between the rows.
    for (k, x) in [28.0, 33.0, 38.0, 43.0, 48.0].into_iter().enumerate() {
        // The on-call engineer stands at the north-east end, so that desk is left out.
        if x < 47.0 {
            war_desk(p, x, -6.2, PI, 40 + k as i32);
        }
        war_desk(p, x + 1.0, 6.4, 0.0, 50 + k as i32);
    }
    for (x, z, yaw) in [
        (30.0, -3.8, 0.3),
        (31.4, 3.9, -0.2),
        (44.0, -4.0, 0.1),
        (45.5, 4.0, 0.5),
    ] {
        p.slab(v(x, 0.08, z), yaw, v(0.5, 0.14, 0.5), Mat::Cardboard);
        p.slab(v(x, 0.24, z), yaw + 0.2, v(0.5, 0.12, 0.5), Mat::Cardboard);
    }
    ticket_wall(p, 40.0, -12.9, 2.3);
    sad_plant(p, 52.4, -11.5);
    sad_plant(p, 52.4, 11.5);
    p.sign_near(30.0, -12.9, 2.9, 3.0, Sign::MigHypercare);
    p.sign_near(30.0, 12.9, 2.9, 3.0, Sign::MigDash);

    strips(p, Mat::NeonRed, Mat::NeonRed, 0.0, 0.25);
    lamps(
        p,
        Color::srgb(1.0, 0.6, 0.5),
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
                ("door:basement/3", (-50.0, 0.0), Sign::MigToCage),
                ("door:campus", (54.0, -6.0), Sign::MigToDay),
            ],
            packs: &WAR.cell_packs(),
            boss: (34.0, 0.0),
            // 15 m from the boss and by the exit door, so the turn-in never pulls it.
            npc: Some((48.0, -8.0)),
        },
    );
    theme(4)
}
