//! Ilmari Wing 1, Datacenter Uprising (docs/ILMARI_09.md B.4): the Data Center after the
//! occupation, as three rooms of the same corridor-hall-pod plan as the Migration so packs and
//! route stay testable. ilmari1/1 is the Cold Aisle (Patch Tuesday: angry racks with red status
//! LEDs and the changelog wall screens), ilmari1/2 the Hiring Floor (the Recruiter: open-plan
//! desks, a waiting row of chairs and a glass interview pod) and ilmari1/3 the Patch Panel (the
//! Cable Hydra: a rack wall that has grown tentacles, cable heaps and four label printers).
//! The satire is in the props: every screen is polite, every rack is cross.

use super::datacenter::dress::*;
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::models::{self, BOX_LARGE, MACHINE_WINDOW, MUG, ROBOT_ARM_A};
use super::offsite::{cove, wall_dress};
use super::props::{sad_plant, water_cooler};
use super::shape::{rounded_rect, stroke};
use super::themes::{pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => (
            "ilmari1/1",
            "Datacenter Uprising: The Cold Aisle",
            cold_aisle(&mut p),
        ),
        2 => (
            "ilmari1/2",
            "Datacenter Uprising: The Hiring Floor",
            hiring_floor(&mut p),
        ),
        _ => (
            "ilmari1/3",
            "Datacenter Uprising: The Patch Panel",
            patch_panel(&mut p),
        ),
    };
    assemble(zone, level, name, false, p, theme)
}

/// Corridor from the arrival, a hall, a corridor to the far pod and three cells off the hall
/// (one per pack): the Migration's plan with a cell more, because every room here has three
/// packs and a pack never stands within 8 m of the route.
pub(super) const ARRIVAL_X: f32 = -44.0;
pub(super) const HALL: (f32, f32, f32) = (-8.0, 26.0, 11.0);
pub(super) const CELLS: [(f32, f32); 3] = [(-20.0, -26.0), (6.0, 26.0), (10.0, -26.0)];
pub(super) const POD: (f32, f32, f32) = (42.0, 13.0, 12.0);

pub(super) fn pods(p: &mut Plan) {
    let w = Mat::Wall;
    let ((hx, hhx, hhz), (bx, bhx, bhz), a) = (HALL, POD, ARRIVAL_X);
    p.fp.pod(rounded_rect(pt(a, 0.0), 6.0, 6.0, 3.0, 0.0), w);
    p.fp.link(stroke(&[pt(a + 6.0, 0.0), pt(hx - hhx + 2.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(hx, 0.0), hhx, hhz, 6.0, 0.0), w);
    p.fp.link(
        stroke(&[pt(hx + hhx - 2.0, 0.0), pt(bx - bhx + 2.0, 0.0)], 4.0),
        w,
    );
    p.fp.pod(rounded_rect(pt(bx, 0.0), bhx, bhz, 6.0, 0.0), w);
    for (cx, cz) in CELLS {
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

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, fog, ambient, floor, accent) = match level {
        // Cold aisle: icy racks gone red, the mist lit from below by status LEDs.
        1 => (
            c(0.36, 0.34, 0.4),
            c(0.26, 0.1, 0.12),
            c(1.0, 0.62, 0.62),
            [0.8, 0.66, 0.7],
            LinearRgba::rgb(4.0, 0.4, 0.4),
        ),
        // Hiring floor: corporate beige under a red recruitment banner.
        2 => (
            c(0.5, 0.44, 0.44),
            c(0.24, 0.1, 0.12),
            c(1.0, 0.74, 0.7),
            [0.9, 0.78, 0.76],
            LinearRgba::rgb(3.6, 0.5, 0.4),
        ),
        // Patch panel: black rack wall, orange and red cable light.
        _ => (
            c(0.3, 0.28, 0.32),
            c(0.22, 0.07, 0.09),
            c(1.0, 0.55, 0.5),
            [0.7, 0.58, 0.62],
            LinearRgba::rgb(4.0, 0.8, 0.3),
        ),
    };
    Theme {
        wall,
        trim: c(0.1, 0.06, 0.08),
        accent,
        fabric: [c(0.5, 0.2, 0.22), c(0.25, 0.28, 0.36), c(0.3, 0.3, 0.32)],
        floor: FloorKind::Tile(floor),
        fog,
        ambient,
        ambient_brightness: 700.0,
        sun: c(1.0, 0.6, 0.5),
        sun_illuminance: 2500.0,
        fog_range: (24.0, 85.0),
    }
}

// ------------------------------------------------------------------ shared pieces

/// A cable that left its rack and went looking for something: a chain of tilted slabs that
/// rises, curls over and ends in a plug with a red LED. `yaw` is the direction it reaches.
fn tentacle(p: &mut Plan, x: f32, z: f32, yaw: f32, reach: f32, base_y: f32) {
    let (fx, fz) = (yaw.cos(), -yaw.sin());
    let (mut along, mut y) = (0.0_f32, base_y);
    let seg = 0.75_f32;
    let steps = 6;
    for k in 0..steps {
        let a = 1.25 - 0.62 * k as f32 * (2.0 / reach.max(1.0)).min(1.0);
        let (dx, dy) = (a.cos() * seg, a.sin() * seg);
        let (cx, cy) = (along + dx / 2.0, y + dy / 2.0);
        let mat = if k % 3 == 1 { Mat::Red } else { Mat::Dark };
        p.slab_pitched(
            v(x + fx * cx, cy.max(0.07), z + fz * cx),
            yaw,
            a,
            v(seg + 0.06, 0.15 - 0.012 * k as f32, 0.15 - 0.012 * k as f32),
            mat,
        );
        along += dx;
        y = (y + dy).max(0.12);
    }
    p.boxed(
        v(x + fx * along, y, z + fz * along),
        v(0.24, 0.16, 0.16),
        Mat::Metal,
        false,
    );
    p.boxed(
        v(x + fx * (along + 0.1), y, z + fz * (along + 0.1)),
        v(0.05, 0.06, 0.06),
        Mat::BlinkRed,
        false,
    );
}

/// Cross rack row: fronts to `+Z` for yaw 0, red LEDs everywhere, a few tentacles escaping.
fn angry_row(p: &mut Plan, x0: f32, x1: f32, z: f32, yaw: f32, tentacles: usize) {
    rack_row(p, x0, x1, z, yaw, true, true);
    let face = if yaw == 0.0 { 1.0 } else { -1.0 };
    let n = (x1 - x0) as usize;
    for k in 0..tentacles {
        let x = x0 + 1.5 + (k as f32 + 0.5) * (n as f32 - 3.0) / tentacles.max(1) as f32;
        let to = if k % 2 == 0 { 0.5 } else { -0.5 };
        tentacle(
            p,
            x,
            z + face * 0.6,
            if face > 0.0 {
                -FRAC_PI_2 + to
            } else {
                FRAC_PI_2 + to
            },
            3.0,
            0.3,
        );
    }
}

/// A red status board: rows of LEDs, mostly red, a brave green one.
fn led_board(p: &mut Plan, x: f32, z: f32, y: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, y, 0.0), v(2.4, 1.2, 0.08), Mat::Dark, false);
        for row in 0..5 {
            for col in 0..9 {
                let m = if (row * 3 + col * 5) % 11 == 0 {
                    Mat::LedGreen
                } else {
                    Mat::BlinkRed
                };
                p.boxed(
                    v(-1.0 + col as f32 * 0.25, y - 0.45 + row as f32 * 0.22, 0.06),
                    v(0.1, 0.1, 0.03),
                    m,
                    false,
                );
            }
        }
    });
}

/// The wall screens of Patch Tuesday: the same shared texture, so the changelog swaps on all.
fn patch_screens(p: &mut Plan, spots: &[(f32, f32, f32)]) {
    for &(x, z, y) in spots {
        p.sign_near(x, z, y, 3.4, Sign::IlPatchIdle);
    }
}

pub(super) fn lamps(p: &mut Plan, list: &[(f32, f32)], intensity: f32) {
    point_lights(p, list, Color::srgb(1.0, 0.42, 0.38), intensity);
}

/// Arrival nook shared by the rooms: a coat rack and a plant that has given up.
pub(super) fn arrival(p: &mut Plan) {
    p.prop(&COAT_RACK, v(-47.0, 0.0, -5.0), 0.0, false);
    sad_plant(p, -48.5, 4.8);
}

fn room_casing(p: &mut Plan) {
    pods(p);
    p.build_walls();
    wall_bands(p, true);
    wall_dress(p, Mat::Concrete, Mat::Metal, Mat::Trim);
    cove(p, Mat::Hot);
}

// ------------------------------------------------------------------ ilmari1/1: the cold aisle

fn cold_aisle(p: &mut Plan) -> Theme {
    room_casing(p);
    // The hall: two long walls of cross racks, a mist-filled aisle between them.
    angry_row(p, -32.0, -14.0, -7.0, 0.0, 5);
    angry_row(p, -32.0, -14.0, 7.0, PI, 5);
    angry_row(p, -6.0, 2.0, -7.0, 0.0, 2);
    angry_row(p, -6.0, 2.0, 7.0, PI, 2);
    for x in [
        -30.5, -26.5, -22.5, -18.5, -14.5, -10.5, -6.5, 6.5, 10.5, 14.5,
    ] {
        vent_tile(p, x, -3.0, Mat::Hot);
        vent_tile(p, x, 3.0, Mat::Hot);
    }
    for (x, z, n) in [
        (-26.0, 0.0, 3),
        (-14.0, 3.0, 3),
        (-4.0, -3.0, 2),
        (12.0, 0.0, 3),
    ] {
        steam(p, x, z, n);
    }
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(-32.0, z), pt(16.0, z), true);
    }
    floor_line(p, pt(-33.0, -2.2), pt(17.0, -2.2), 0.1, Mat::Red);
    floor_line(p, pt(-33.0, 2.2), pt(17.0, 2.2), 0.1, Mat::Red);
    cable_bundle(p, &[(-24.0, -1.0), (-20.0, 1.5), (-15.0, -0.5)], Mat::Red);
    cable_bundle(p, &[(2.0, 1.0), (6.0, -1.5), (11.0, 0.5)], Mat::Yellow);
    // Propaganda for the machines, and the first of the changelog screens.
    patch_screens(
        p,
        &[(-33.9, 5.0, 2.8), (-33.9, -5.0, 2.8), (17.9, 4.0, 2.8)],
    );
    led_board(p, -17.0, -10.8, 2.7, 0.0);
    led_board(p, -17.0, 10.8, 2.7, PI);
    p.sign_near(0.0, -10.9, 3.1, 2.6, Sign::IlRackVote);
    p.sign_near(-8.0, 10.9, 3.1, 2.4, Sign::IlUptime);
    p.sign_near(-33.9, 0.0, 3.2, 2.6, Sign::IlW1Door);
    arrival(p);
    // North cell: the rack that got the first email. South: a vending machine that sells nothing.
    angry_row(p, -26.0, -14.0, -31.8, PI, 4);
    models::put(p, &MACHINE_WINDOW, v(6.0, 0.0, 32.0), PI, 1.6, true);
    models::put(p, &BOX_LARGE, v(0.0, 0.0, 31.5), 0.3, 1.3, true);
    water_cooler(p, 12.0, 32.8);
    angry_row(p, 4.0, 16.0, -31.8, PI, 3);
    p.sign_near(-20.0, -32.9, 2.8, 2.4, Sign::DoNotUnplug);
    // Boss pod: Patch Tuesday's shrine. Racks down both sides, the screens all facing the
    // fight (the changelog must be readable from the middle of it), a floor ring of red.
    for z in [-10.0, 10.0] {
        angry_row(p, 33.0, 51.0, z, if z < 0.0 { 0.0 } else { PI }, 3);
    }
    floor_ring(p, 42.0, 0.0, 6.2, 22, Mat::BlinkRed, 0.1);
    floor_ring(p, 42.0, 0.0, 6.8, 22, Mat::Hazard, 0.08);
    patch_screens(
        p,
        &[
            (54.9, -6.0, 2.9),
            (54.9, 6.0, 2.9),
            (42.0, -11.9, 3.2),
            (42.0, 11.9, 3.2),
            (29.1, -5.0, 2.9),
            (29.1, 5.0, 2.9),
        ],
    );
    led_board(p, 54.0, 0.0, 2.0, -FRAC_PI_2);
    steam(p, 42.0, -8.0, 3);
    steam(p, 42.0, 8.0, 3);
    for dz in [-5.0, 5.0] {
        for dx in [-0.25, 0.0, 0.25] {
            hanging_cable(p, 36.0 + dx, dz, 1.25, dx == 0.0);
        }
    }
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(31.0, z), pt(53.0, z), true);
    }
    p.sign_near(30.0, 0.0, 3.0, 2.4, Sign::TempWarning);
    strips(p, Mat::Hot, Mat::Hot, 0.0, 0.2);
    lamps(
        p,
        &[
            (-44.0, 0.0),
            (-26.0, -3.0),
            (-26.0, 3.0),
            (-10.0, 0.0),
            (6.0, 0.0),
            (14.0, 0.0),
            (-20.0, -26.0),
            (6.0, 26.0),
            (36.0, 0.0),
            (48.0, 0.0),
        ],
        340_000.0,
    );
    cast(
        p,
        [
            ("door:campus_occupied", Sign::IlW1Back),
            ("door:ilmari1/2", Sign::IlW1ToHiring),
        ],
    );
    theme(1)
}

fn cast(p: &mut Plan, doors: [(&'static str, Sign); 2]) {
    let packs = CELLS.map(|(x, z)| (x, z - z.signum()));
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                (doors[0].0, (-50.0, 0.0), doors[0].1),
                (doors[1].0, (55.0, 6.0), doors[1].1),
            ],
            packs: &packs,
            boss: (42.0, 0.0),
            npc: None,
        },
    );
}

// ------------------------------------------------------------------ ilmari1/2: the hiring floor

/// An open-plan desk: desk, screen, laptop, a chair and the mug that proves a human sat here.
fn hiring_desk(p: &mut Plan, x: f32, z: f32, yaw: f32, mug: bool) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
        p.prop(&SCREEN, v(0.0, 0.76, -0.1), 0.0, false);
        p.prop(&LAPTOP, v(0.55, 0.76, 0.1), 0.2, false);
        p.prop(&CHAIR_DESK, v(0.0, 0.0, 0.9), PI, false);
        if mug {
            models::put(p, &MUG, v(-0.55, 0.76, 0.2), 0.0, 0.35, false);
        }
        p.sign(v(0.0, 1.5, -0.35), 0.0, 0.5, Sign::IlHiring);
    });
}

fn hiring_floor(p: &mut Plan) -> Theme {
    room_casing(p);
    // The open plan: desks in pairs, the mug on every second one (the other half are androids).
    for (k, x) in [-28.0, -20.0, -12.0, -4.0, 4.0, 12.0]
        .into_iter()
        .enumerate()
    {
        hiring_desk(p, x, -5.0, PI, k % 2 == 0);
        hiring_desk(p, x, 5.0, 0.0, k % 2 == 1);
    }
    // A waiting row along the north wall: chairs, one plant, one sad coffee.
    for k in 0..7 {
        p.prop(
            &CHAIR_ROUNDED,
            v(-30.0 + k as f32 * 3.0, 0.0, -9.6),
            0.0,
            false,
        );
    }
    p.prop(&COFFEE_TABLE_SQUARE, v(-9.0, 0.0, -9.6), 0.0, true);
    sad_plant(p, -12.0, -9.6);
    // The reception desk and the banner that says the quiet part.
    p.boxed(v(-33.0, 0.55, 0.0), v(1.0, 1.1, 5.0), Mat::Wood, true);
    p.boxed(v(-32.5, 1.2, 0.0), v(0.5, 0.2, 4.4), Mat::Dark, false);
    p.sign_near(-33.9, 6.0, 2.7, 3.0, Sign::IlHiring);
    p.sign_near(-33.9, -6.0, 2.7, 3.0, Sign::IlHiring);
    p.sign_near(17.9, 5.0, 2.7, 2.6, Sign::IlHiring);
    for x in [-24.0, -8.0, 8.0] {
        p.boxed(v(x, 3.6, 0.0), v(0.1, 0.5, 22.0), Mat::Red, false);
    }
    led_board(p, -10.0, 10.8, 2.7, PI);
    p.sign_near(0.0, -10.9, 3.1, 2.6, Sign::IlUptime);
    cable_tray(p, pt(-32.0, 0.0), pt(16.0, 0.0), true);
    arrival(p);
    p.sign_near(-49.9, 4.5, 2.4, 2.2, Sign::IlW1Back);
    // Cells: a server alcove where the androids are printed, and a break room that is not.
    angry_row(p, -26.0, -14.0, -31.8, PI, 3);
    for k in 0..3 {
        models::put(
            p,
            &BOX_LARGE,
            v(-26.0 + k as f32 * 4.5, 0.0, -30.0),
            0.2 * k as f32,
            1.4,
            true,
        );
    }
    water_cooler(p, 12.0, 32.8);
    p.prop(&LOUNGE_SOFA, v(6.0, 0.0, 32.0), PI, true);
    p.prop(&COFFEE_TABLE, v(6.0, 0.0, 29.5), 0.0, true);
    // Boss pod: the interview room, glass on three sides, one chair for you, eleven rounds of
    // chairs for them along the back.
    for k in 0..8 {
        p.prop(
            &CHAIR_ROUNDED,
            v(33.0 + k as f32 * 2.4, 0.0, -10.6),
            0.0,
            false,
        );
        p.prop(
            &CHAIR_ROUNDED,
            v(33.0 + k as f32 * 2.4, 0.0, 10.6),
            PI,
            false,
        );
    }
    p.prop(&TABLE_GLASS, v(35.0, 0.0, -6.0), 0.0, true);
    p.prop(&LAPTOP, v(35.0, 0.78, -6.0), 0.0, false);
    models::put(p, &MUG, v(34.4, 0.78, -5.6), 0.0, 0.35, false);
    floor_ring(p, 42.0, 0.0, 6.2, 22, Mat::BlinkRed, 0.1);
    floor_ring(p, 42.0, 0.0, 7.0, 24, Mat::Hazard, 0.08);
    p.sign(v(54.9, 3.2, 0.0), -FRAC_PI_2, 3.4, Sign::IlHiring);
    p.sign_near(29.1, 7.0, 2.9, 2.4, Sign::IlRackVote);
    p.sign_near(29.1, -7.0, 2.9, 2.4, Sign::IlUptime);
    led_board(p, 52.0, -11.0, 2.4, 0.0);
    led_board(p, 52.0, 11.0, 2.4, PI);
    p.sign_near(30.0, 0.0, 3.0, 2.4, Sign::TempWarning);
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(31.0, z), pt(53.0, z), true);
    }
    strips(p, Mat::LightPanel, Mat::Hot, 0.0, 0.1);
    lamps(
        p,
        &[
            (-44.0, 0.0),
            (-26.0, 0.0),
            (-12.0, 0.0),
            (2.0, 0.0),
            (-20.0, -26.0),
            (6.0, 26.0),
            (36.0, 0.0),
            (48.0, 0.0),
        ],
        300_000.0,
    );
    cast(
        p,
        [
            ("door:ilmari1/1", Sign::IlW1Back),
            ("door:ilmari1/3", Sign::IlW1ToPanel),
        ],
    );
    theme(2)
}

// ------------------------------------------------------------------ ilmari1/3: the patch panel

/// A label printer: a steel box with a paper tongue, a green LED and its own sign. The prop
/// anchor sits 1.7 m in front of it.
pub(super) fn label_printer(p: &mut Plan, key: &'static str, x: f32, z: f32, face: f32) {
    p.boxed(v(x, 0.45, z), v(1.1, 0.9, 0.7), Mat::Metal, true);
    p.boxed(v(x, 0.95, z), v(0.9, 0.12, 0.6), Mat::Dark, false);
    p.boxed(
        v(x - face * 0.05, 0.6, z + face * 0.38),
        v(0.5, 0.05, 0.1),
        Mat::Paper,
        false,
    );
    p.boxed(
        v(x + 0.4, 0.8, z + face * 0.36),
        v(0.08, 0.08, 0.03),
        Mat::LedGreen,
        false,
    );
    p.sign(
        v(x, 1.55, z),
        if face > 0.0 { 0.0 } else { PI },
        1.4,
        Sign::IlLabelHere,
    );
    p.interactable(key, v(x, 0.8, z), v(x, 0.0, z + face * 1.8));
}

fn patch_panel(p: &mut Plan) -> Theme {
    room_casing(p);
    // The hall: a patch panel the size of a wall, every port a different cable, no two labelled.
    angry_row(p, -32.0, -14.0, -7.0, 0.0, 4);
    angry_row(p, -32.0, -14.0, 7.0, PI, 4);
    angry_row(p, -6.0, 2.0, -7.0, 0.0, 2);
    angry_row(p, -6.0, 2.0, 7.0, PI, 2);
    for k in 0..9 {
        let x = -30.0 + k as f32 * 3.4;
        cable_bundle(
            p,
            &[(x, -7.0), (x + 1.5, -3.0), (x - 0.5, 1.0), (x + 1.0, 5.0)],
            if k % 2 == 0 { Mat::Red } else { Mat::Yellow },
        );
    }
    for x in [-26.0, -16.0, -6.0] {
        floor_hole(p, x, 3.0, 0.4);
        floor_hole(p, x + 3.0, -3.0, -0.4);
    }
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(-32.0, z), pt(16.0, z), true);
    }
    cable_tray(p, pt(10.0, -10.0), pt(10.0, 10.0), true);
    p.sign_near(-33.9, 5.0, 2.8, 2.6, Sign::IlNoLabels);
    p.sign_near(-33.9, -5.0, 2.8, 2.6, Sign::IlNoLabels);
    p.sign_near(17.9, -4.0, 2.8, 2.6, Sign::IlUptime);
    led_board(p, -17.0, -10.8, 2.7, 0.0);
    led_board(p, -17.0, 10.8, 2.7, PI);
    arrival(p);
    p.sign_near(-49.9, 4.5, 2.4, 2.2, Sign::IlW1Back);
    // The first cables of the day are already out in the hall.
    for (x, z, yaw) in [
        (-12.0, -8.0, -FRAC_PI_2),
        (-4.0, 8.0, FRAC_PI_2),
        (12.0, -8.0, -FRAC_PI_2),
    ] {
        tentacle(p, x, z, yaw, 3.0, 0.3);
    }
    // Cells: a robot arm that was meant to swap tapes, and a stack of spare patch leads.
    models::put(p, &ROBOT_ARM_A, v(-20.0, 0.0, -31.5), 0.0, 1.2, true);
    angry_row(p, 4.0, 16.0, -31.8, PI, 3);
    for k in 0..4 {
        models::put(
            p,
            &BOX_LARGE,
            v(2.0 + k as f32 * 3.2, 0.0, 31.5),
            0.2 * k as f32,
            1.4,
            true,
        );
    }
    // Boss pod: the Hydra's rack wall at the back, a ring of cable heaps and four printers
    // spread round the walls so that nobody has to walk through the cables to reach one.
    p.boxed(v(54.6, 1.6, -4.0), v(1.0, 3.2, 14.0), Mat::Rack, true);
    for row in 0..5 {
        for col in 0..14 {
            let m = if (row + col * 3) % 7 == 0 {
                Mat::LedGreen
            } else {
                Mat::BlinkRed
            };
            p.boxed(
                v(53.95, 0.5 + row as f32 * 0.55, -10.5 + col as f32 * 0.9),
                v(0.04, 0.12, 0.3),
                m,
                false,
            );
        }
    }
    for (k, z) in [-9.0, -6.0, -3.0, 0.0, 2.0].into_iter().enumerate() {
        tentacle(
            p,
            53.0,
            z,
            PI + (k as f32 - 2.0) * 0.18,
            4.0,
            1.4 + (k % 2) as f32 * 0.5,
        );
    }
    for (x, z) in [
        (38.0, -7.0),
        (46.0, 7.0),
        (36.0, 6.0),
        (47.0, -6.0),
        (41.0, 0.0),
    ] {
        cable_bundle(
            p,
            &[(x - 2.0, z), (x, z + 1.0), (x + 2.0, z - 0.5)],
            Mat::Red,
        );
    }
    floor_ring(p, 42.0, 0.0, 6.4, 22, Mat::BlinkRed, 0.1);
    floor_ring(p, 42.0, 0.0, 7.0, 24, Mat::Hazard, 0.08);
    label_printer(p, "prop:il_label_printer_1", 31.5, -11.2, 1.0);
    label_printer(p, "prop:il_label_printer_2", 31.5, 11.2, -1.0);
    label_printer(p, "prop:il_label_printer_3", 48.5, -11.2, 1.0);
    label_printer(p, "prop:il_label_printer_4", 48.5, 11.2, -1.0);
    p.sign_near(42.0, -11.9, 3.2, 3.0, Sign::IlNoLabels);
    p.sign_near(42.0, 11.9, 3.2, 3.0, Sign::IlNoLabels);
    p.sign_near(29.1, 0.0, 3.2, 2.6, Sign::TempWarning);
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(31.0, z), pt(53.0, z), true);
    }
    steam(p, 36.0, -8.0, 3);
    steam(p, 36.0, 8.0, 3);
    strips(p, Mat::Hot, Mat::Hot, 0.0, 0.25);
    lamps(
        p,
        &[
            (-44.0, 0.0),
            (-26.0, 0.0),
            (-10.0, 0.0),
            (6.0, 0.0),
            (-20.0, -26.0),
            (6.0, 26.0),
            (36.0, 0.0),
            (48.0, -6.0),
            (48.0, 6.0),
        ],
        360_000.0,
    );
    cast(
        p,
        [
            ("door:ilmari1/2", Sign::IlW1Back),
            ("door:campus_occupied", Sign::IlW1Out),
        ],
    );
    theme(3)
}
