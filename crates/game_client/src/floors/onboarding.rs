//! The Onboarding dungeon (docs/DUNGEONS.md): the first week at a generic office in three
//! pastel rooms. onboarding/1 is reception and the badge desk, onboarding/2 the equipment cage,
//! onboarding/3 "your desk". Same boxes-and-primitives budget as the offsite, and no real
//! company's floor plan: every room is a corridor into a hall with two side cells for the packs
//! and a pod for the boss.

use super::datacenter::dress::{WallSpot, floor_line, rnd};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::offsite::{bunting, cove, floor_patch, round_patch, wall_dress};
use super::props::{
    cubicle, doc_shelf, lounge, mini_fridge, printer, sad_plant, vending, water_cooler,
};
use super::shape::{rounded_rect, stroke};
use super::themes::{ceiling_lights, pt, runner, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => (
            "onboarding/1",
            "The Onboarding: Reception and Badge Desk",
            reception(&mut p),
        ),
        2 => (
            "onboarding/2",
            "The Onboarding: Equipment Cage",
            cage(&mut p),
        ),
        _ => (
            "onboarding/3",
            "The Onboarding: Your Desk",
            desk_room(&mut p),
        ),
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, trim, accent, floor, fog) = match level {
        1 => (
            c(0.86, 0.93, 0.9),
            c(0.35, 0.5, 0.52),
            LinearRgba::rgb(0.5, 2.2, 1.8),
            [0.78, 0.92, 0.88],
            c(0.55, 0.65, 0.62),
        ),
        2 => (
            c(0.87, 0.9, 0.95),
            c(0.36, 0.42, 0.56),
            LinearRgba::rgb(1.0, 1.8, 3.0),
            [0.78, 0.84, 0.95],
            c(0.55, 0.6, 0.7),
        ),
        _ => (
            c(0.96, 0.9, 0.86),
            c(0.55, 0.4, 0.36),
            LinearRgba::rgb(3.0, 1.7, 0.9),
            [0.95, 0.86, 0.8],
            c(0.7, 0.6, 0.55),
        ),
    };
    Theme {
        wall,
        trim,
        accent,
        fabric: [c(0.55, 0.78, 0.74), c(0.95, 0.72, 0.6), c(0.7, 0.72, 0.92)],
        floor: FloorKind::Carpet(floor),
        fog,
        ambient: c(1.0, 0.98, 0.94),
        // A soft pad under the muzak: the detune is the lighting, a little greyer each room.
        ambient_brightness: 640.0 - 30.0 * f32::from(level),
        sun: c(1.0, 0.96, 0.9),
        sun_illuminance: 6500.0,
        fog_range: (45.0, 120.0),
    }
}

/// Corridor from the arrival, a hall, a corridor to the boss pod, and one cell either side of
/// the hall for the two packs. `hall` is (centre x, half x, half z), `cells` the two cell centres
/// (x, z) and `boss` the pod centre x with its half sizes.
struct Shape {
    arrival_x: f32,
    hall: (f32, f32, f32),
    cells: [(f32, f32); 2],
    boss: (f32, f32, f32),
}

impl Shape {
    fn pods(&self, p: &mut Plan) {
        let w = Mat::Wall;
        let (hx, hhx, hhz) = self.hall;
        let (bx, bhx, bhz) = self.boss;
        let a = self.arrival_x;
        p.fp.pod(rounded_rect(pt(a, 0.0), 6.0, 6.0, 3.0, 0.0), w);
        p.fp.link(stroke(&[pt(a + 6.0, 0.0), pt(hx - hhx + 2.0, 0.0)], 4.0), w);
        p.fp.pod(rounded_rect(pt(hx, 0.0), hhx, hhz, 6.0, 0.0), w);
        p.fp.link(
            stroke(&[pt(hx + hhx - 2.0, 0.0), pt(bx - bhx + 2.0, 0.0)], 4.0),
            w,
        );
        p.fp.pod(rounded_rect(pt(bx, 0.0), bhx, bhz, 6.0, 0.0), w);
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

    fn packs(&self) -> [(f32, f32); 2] {
        [
            (
                self.cells[0].0,
                self.cells[0].1 - 1.0 * self.cells[0].1.signum(),
            ),
            (
                self.cells[1].0,
                self.cells[1].1 - 1.0 * self.cells[1].1.signum(),
            ),
        ]
    }
}

fn plant(p: &mut Plan, x: f32, z: f32) {
    p.prop(&POTTED_PLANT, v(x, 0.0, z), 0.0, true);
}

/// Reception desk, front towards +Z.
fn counter(p: &mut Plan, x: f32, z: f32, len: f32) {
    p.boxed(v(x, 0.55, z), v(len, 1.1, 1.0), Mat::Wood, true);
    p.boxed(v(x, 1.12, z), v(len + 0.2, 0.06, 1.2), Mat::White, false);
}

/// Cardboard stacked `n` high and a little off true, a strip of tape across each lid: the filler
/// for every storage corner. Only the bottom box is solid; the rest sits on it.
fn crates(p: &mut Plan, x: f32, z: f32, yaw: f32, n: usize) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        for k in 0..n {
            let (w, d) = (1.3 - 0.12 * k as f32, 0.95 - 0.08 * k as f32);
            let y = 0.35 + k as f32 * 0.7;
            let m = if k % 2 == 0 {
                Mat::Cardboard
            } else {
                Mat::Paper
            };
            p.boxed(v(0.07 * k as f32, y, 0.0), v(w, 0.7, d), m, k == 0);
            p.boxed(
                v(0.07 * k as f32, y + 0.351, 0.0),
                v(0.2, 0.01, d + 0.01),
                Mat::Beige,
                false,
            );
        }
    });
}

/// Queue posts with a red rope along z, from `z0` to `z1`: the line that leads to a wall.
fn stanchions(p: &mut Plan, x: f32, z0: f32, z1: f32) {
    let n = ((z1 - z0).abs() / 2.0).ceil() as usize;
    let step = (z1 - z0) / n as f32;
    for k in 0..=n {
        let z = z0 + k as f32 * step;
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.5, z),
            v(0.07, 1.0, 0.07),
            Mat::Chrome,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x, 0.03, z),
            v(0.36, 0.06, 0.36),
            Mat::Metal,
        );
        p.prim(
            PrimKind::Sphere,
            v(x, 1.03, z),
            v(0.14, 0.14, 0.14),
            Mat::Gold,
        );
        if k < n {
            p.slab(
                v(x, 0.88, z + step / 2.0),
                0.0,
                v(0.04, 0.06, step.abs()),
                Mat::Red,
            );
        }
    }
}

/// A balloon on a string, tied to whatever is below it.
fn balloon(p: &mut Plan, x: f32, z: f32, y: f32, m: Mat) {
    p.prim(
        PrimKind::Cylinder,
        v(x, y / 2.0, z),
        v(0.012, y, 0.012),
        Mat::Paper,
    );
    p.prim(PrimKind::Sphere, v(x, y + 0.3, z), v(0.5, 0.62, 0.5), m);
}

/// Standing sign-in kiosk: a pedestal and a tilted screen that greets the previous hire.
fn kiosk(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 0.55, 0.0), v(0.5, 1.1, 0.4), Mat::White, true);
        p.boxed(v(0.0, 1.45, 0.0), v(0.9, 0.55, 0.08), Mat::Dark, false);
        p.boxed(
            v(0.0, 1.45, 0.05),
            v(0.8, 0.46, 0.02),
            Mat::ScreenGlow,
            false,
        );
        p.boxed(v(0.0, 1.12, 0.22), v(0.3, 0.04, 0.12), Mat::Wood, false);
    });
}

// ------------------------------------------------------------------ onboarding/1

const RECEPTION: Shape = Shape {
    arrival_x: -40.0,
    hall: (-6.0, 20.0, 16.0),
    cells: [(-14.0, -30.0), (-14.0, 30.0)],
    boss: (36.0, 12.0, 12.0),
};

fn reception(p: &mut Plan) -> Theme {
    RECEPTION.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Fabric0, Mat::White, Mat::Trim);
    cove(p, Mat::Candle);

    runner(
        p,
        &[pt(-44.0, 0.0), pt(-6.0, 0.0), pt(24.0, 0.0), pt(33.0, 0.0)],
        2.4,
        Mat::Fabric1,
        Mat::RunnerEdge,
    );
    // Turnstiles that beep at everything; the wide gate in the middle is "out of service".
    for z in [-13.0, -10.0, -7.0, 7.0, 10.0, 13.0] {
        p.boxed(v(-20.0, 0.55, z), v(0.5, 1.1, 0.9), Mat::Metal, true);
        // Every second one is the one that beeps.
        let led = if z.abs() > 8.0 {
            Mat::BlinkRed
        } else {
            Mat::BlinkAmber
        };
        p.boxed(v(-20.0, 1.15, z), v(0.3, 0.08, 0.5), led, false);
        p.boxed(v(-20.6, 0.8, z), v(0.04, 0.5, 0.9), Mat::Glass, false);
    }
    p.boxed(v(-20.0, 1.6, 0.0), v(0.2, 0.06, 7.0), Mat::LedRed, false);
    // Reception: "Welcome, [NAME]" shows the previous hire.
    counter(p, -6.0, -13.0, 7.0);
    for x in [-8.0, -4.0] {
        p.boxed(v(x, 1.3, -13.3), v(0.5, 0.35, 0.05), Mat::Dark, false);
        p.boxed(
            v(x, 1.3, -13.27),
            v(0.42, 0.27, 0.02),
            Mat::ScreenGlow,
            false,
        );
    }
    p.sign_near(-6.0, -15.9, 2.7, 4.2, Sign::ObWelcome);
    // Slow-motion arrows labelled "Please queue here" that lead to a wall.
    for k in 0..6 {
        let z = -2.0 - k as f32 * 1.7;
        for yaw in [0.6, -0.6] {
            p.slab(v(-12.0, 0.012, z), yaw, v(1.5, 0.01, 0.22), Mat::Yellow);
        }
    }
    p.sign_near(-12.0, -15.9, 2.4, 3.2, Sign::ObQueue);
    // Visitor sofa and the fern that has been dead since 2021.
    lounge(p, 2.0, 11.0, PI, false);
    sad_plant(p, 8.5, 13.5);
    p.prim(
        PrimKind::Cone,
        v(8.5, 1.0, 13.5),
        v(0.9, 1.5, 0.9),
        Mat::Dead,
    );
    p.poster_near(-6.0, 15.9, 2.3, Vec2::new(1.5, 1.8), Tex::Wellness);
    p.poster_near(8.0, 15.9, 2.3, Vec2::new(1.8, 1.4), Tex::MandatoryFun);
    for (x, z) in [(-24.0, -14.5), (-24.0, 14.5), (12.0, -14.5)] {
        plant(p, x, z);
    }
    // Boss pod: a training screen and the four answer tiles.
    p.boxed(v(36.0, 2.2, -11.6), v(7.0, 3.0, 0.1), Mat::White, false);
    p.panel(
        v(36.0, 2.2, -11.5),
        0.0,
        Vec2::new(6.6, 2.7),
        Tex::Documentation,
    );
    for (k, m) in [Mat::NeonCyan, Mat::NeonYellow, Mat::NeonPink, Mat::LedGreen]
        .into_iter()
        .enumerate()
    {
        floor_patch(p, 29.0 + k as f32 * 2.6, 6.5, 0.0, 2.2, 2.2, m);
    }
    p.boxed(v(42.0, 0.55, 0.0), v(0.8, 1.1, 1.2), Mat::Wood, true);
    // The queue itself: rope posts along the arrows, ending at the wall, and a welcome mat nobody
    // has been allowed to step on yet.
    stanchions(p, -9.8, -2.8, -14.6);
    stanchions(p, -14.2, -2.8, -13.0);
    floor_patch(p, -23.0, 0.0, 0.0, 2.4, 4.4, Mat::Fabric2);
    floor_patch(p, -23.0, 0.0, 0.0, 1.8, 3.8, Mat::Fabric0);
    // Sign-in kiosks beside the gates, both greeting the previous hire.
    kiosk(p, -17.0, -5.5, -FRAC_PI_2);
    kiosk(p, -17.0, 5.5, -FRAC_PI_2);
    // Visitor badges on the counter, and the reception phone that rings in another building.
    for (k, m) in [Mat::NeonCyan, Mat::NeonYellow, Mat::NeonPink, Mat::LedGreen]
        .into_iter()
        .cycle()
        .take(8)
        .enumerate()
    {
        p.slab(
            v(-8.6 + k as f32 * 0.4, 1.16, -13.1),
            0.1 * k as f32,
            v(0.18, 0.012, 0.28),
            m,
        );
    }
    p.boxed(v(-2.9, 1.2, -13.0), v(0.3, 0.1, 0.25), Mat::Dark, false);
    // Lanyard pegboard on the north wall: every colour of lanyard, none of them with a name.
    if let Some(w) = WallSpot::at(p, 4.0, -16.4) {
        w.put(p, 0.0, 1.9, 0.04, v(4.0, 1.7, 0.05), Mat::Wood);
        for k in 0..28 {
            let (col, row) = (k % 14, k / 14);
            let m = [
                Mat::NeonCyan,
                Mat::NeonPink,
                Mat::NeonYellow,
                Mat::LedGreen,
                Mat::Red,
            ][k % 5];
            w.put(
                p,
                -1.8 + col as f32 * 0.28,
                2.25 - row as f32 * 0.62 - rnd(k as i32, 1) * 0.15,
                0.09,
                v(0.07, 0.5, 0.02),
                m,
            );
            w.put(
                p,
                -1.8 + col as f32 * 0.28,
                1.92 - row as f32 * 0.62,
                0.1,
                v(0.12, 0.16, 0.02),
                Mat::White,
            );
        }
        w.sign(p, 0.0, 3.1, 3.0, Sign::ObNewStarters);
    }
    // Documentation nobody opened, a vending machine with the previous hire's snack still in it.
    doc_shelf(p, -19.0, 15.0, Face::NegZ);
    vending(p, 10.5, -15.2, Face::PosZ);
    water_cooler(p, 12.4, -15.4);
    p.prop(&COAT_RACK, v(-4.0, 0.0, 15.2), 0.0, true);
    p.prop(&COAT_RACK, v(-2.6, 0.0, 15.2), 0.0, true);
    p.prop(&TRASHCAN, v(4.6, 0.0, 15.3), 0.0, true);
    // Bunting from the "welcome week" nobody took down, along the hall's sides, not over the route.
    for z in [-12.0, 12.0] {
        bunting(
            p,
            (-24.0, z),
            (12.0, z),
            3.6,
            [Mat::NeonPink, Mat::NeonCyan, Mat::NeonYellow],
        );
    }
    // Pod: folding chairs for the training, facing the screen, and a projector table.
    for row in 0..2 {
        for col in 0..4 {
            p.prop(
                &CHAIR_MODERN,
                v(27.0 + col as f32 * 1.6, 0.0, 8.0 + row as f32 * 1.4),
                PI,
                false,
            );
        }
    }
    p.boxed(v(31.0, 0.5, 4.5), v(1.2, 0.05, 0.7), Mat::Wood, false);
    p.boxed(v(31.0, 0.7, 4.5), v(0.4, 0.3, 0.35), Mat::Dark, false);
    p.boxed(v(31.0, 0.25, 4.5), v(1.1, 0.5, 0.6), Mat::Beige, true);
    crates(p, 46.0, -9.0, 0.2, 3);
    crates(p, 46.0, 9.0, -0.2, 2);
    // Annex dressing, more of it: the north cell's lanyard stock, the south cell's break area.
    crates(p, -18.5, -27.0, 0.0, 2);
    crates(p, -9.0, -26.5, 0.3, 3);
    crates(p, -8.0, -35.0, 0.0, 2);
    mini_fridge(p, -19.0, 36.5, Face::NegZ);
    water_cooler(p, -8.5, 36.5);
    p.poster_near(-14.0, -37.0, 2.3, Vec2::new(1.6, 1.2), Tex::StickyPass);
    p.poster_near(8.0, -15.9, 2.3, Vec2::new(1.6, 1.2), Tex::Standup);
    p.poster_near(-36.0, 1.9, 2.3, Vec2::new(1.2, 1.5), Tex::HrDoor);
    p.poster_near(-30.0, -1.9, 2.3, Vec2::new(1.4, 1.1), Tex::Suggestions);

    // Annex dressing: stacked lanyards in the north cell, the coffee corner in the south one.
    for (x, z) in [(-17.0, -35.0), (-11.0, -35.0)] {
        p.boxed(v(x, 0.4, z), v(1.4, 0.8, 1.0), Mat::Cardboard, true);
    }
    lounge(p, -14.0, 36.0, PI, true);
    p.sign_near(-14.0, 36.9, 2.6, 3.0, Sign::ObToReception);

    ceiling_lights(p);
    for (x, z) in [
        (-40.0, 0.0),
        (-14.0, -8.0),
        (-14.0, 8.0),
        (6.0, 0.0),
        (36.0, 0.0),
        (-14.0, -30.0),
        (-14.0, 30.0),
    ] {
        p.light(v(x, 3.6, z), Color::srgb(1.0, 0.97, 0.9), 400_000.0);
    }
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:campus", (-46.0, 0.0), Sign::ToCampus),
                ("door:onboarding/2", (48.0, 6.0), Sign::ObToCage),
            ],
            packs: &RECEPTION.packs(),
            boss: (36.0, 0.0),
            npc: None,
        },
    );
    theme(1)
}

// ------------------------------------------------------------------ onboarding/2

const CAGE: Shape = Shape {
    arrival_x: -44.0,
    hall: (-6.0, 22.0, 18.0),
    cells: [(-18.0, -32.0), (10.0, 32.0)],
    boss: (40.0, 14.0, 12.0),
};

/// Floor-to-ceiling shelving along x: an open steel frame, four boards a bay and a box on most
/// of them, both faces visible. Exactly one laptop per ten of them, in `laptop_bay`; the rest
/// are empty boxes that rattle. `row` only seeds which slots are bare.
fn shelving(p: &mut Plan, x0: f32, z: f32, bays: usize, laptop_bay: usize, row: i32) {
    for k in 0..bays {
        let x = x0 + k as f32 * 3.2;
        // The collider is one block per bay; the frame itself is thin and see-through.
        p.block(v(x, 1.45, z), v(3.0, 2.9, 0.9));
        for sx in [-1.5, 1.5] {
            p.boxed(v(x + sx, 1.45, z), v(0.08, 2.9, 0.9), Mat::Metal, false);
        }
        for (lvl, y) in [0.05, 0.78, 1.51, 2.24, 2.88].into_iter().enumerate() {
            p.boxed(v(x, y, z), v(3.0, 0.05, 0.9), Mat::Metal, false);
            if lvl == 4 {
                continue;
            }
            for slot in 0..3 {
                let seed = row * 997 + k as i32 * 31 + lvl as i32 * 7 + slot;
                if rnd(seed, 5) > 0.82 {
                    continue;
                }
                let h = 0.42 + rnd(seed, 6) * 0.2;
                let m = if rnd(seed, 7) > 0.55 {
                    Mat::Cardboard
                } else {
                    Mat::Paper
                };
                let bx = x - 1.0 + slot as f32 * 1.0 + (rnd(seed, 8) - 0.5) * 0.08;
                p.boxed(v(bx, y + 0.025 + h / 2.0, z), v(0.82, h, 0.74), m, false);
                // A shipping label on each face, so the boxes read as inventory from either aisle.
                for side in [-1.0, 1.0] {
                    p.boxed(
                        v(bx + 0.18, y + 0.025 + h * 0.6, z + side * 0.375),
                        v(0.26, 0.16, 0.01),
                        Mat::White,
                        false,
                    );
                }
            }
        }
        if k == laptop_bay {
            // The one laptop: a box open on the middle board with a lid that still glows.
            p.boxed(v(x, 0.88, z), v(0.6, 0.06, 0.45), Mat::Dark, false);
            p.boxed(
                v(x, 1.05, z - 0.2),
                v(0.58, 0.32, 0.02),
                Mat::ScreenGlow,
                false,
            );
        }
    }
}

fn cage(p: &mut Plan) -> Theme {
    CAGE.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Fabric1, Mat::White, Mat::Trim);
    cove(p, Mat::NeonCyan);

    runner(
        p,
        &[pt(-48.0, 0.0), pt(-6.0, 0.0), pt(26.0, 0.0), pt(37.0, 0.0)],
        2.4,
        Mat::Fabric0,
        Mat::RunnerEdge,
    );
    shelving(p, -22.0, -9.0, 9, 5, 0);
    shelving(p, -22.0, 9.0, 9, 2, 1);
    shelving(p, -22.0, -15.5, 9, 7, 2);
    shelving(p, -4.0, 15.4, 5, 3, 3);
    // The wall calendar: "Week 6" circled, crossed out, circled again.
    // East of the shelving rows, which would otherwise hide it.
    p.sign_near(10.0, -17.9, 2.5, 4.2, Sign::ObWeek);
    p.slab(v(11.9, 2.4, -17.8), 0.0, v(1.6, 0.02, 0.02), Mat::Red);
    p.sign_near(-30.0, 0.0, 2.7, 3.4, Sign::ObCage);
    // Counter along the south wall: a kettle bolted down and a ticket printer that prints
    // tickets for itself (the quest prop).
    counter(p, -22.0, 16.0, 4.0);
    p.prim(
        PrimKind::Cylinder,
        v(-22.6, 1.35, 16.0),
        v(0.3, 0.45, 0.3),
        Mat::Chrome,
    );
    for dx in [-0.2, 0.2] {
        p.boxed(
            v(-22.6 + dx, 1.17, 16.2),
            v(0.06, 0.04, 0.06),
            Mat::Metal,
            false,
        );
    }
    printer(p, -10.0, 16.2, Face::NegZ);
    p.interactable(
        "prop:onb_ticket_printer",
        v(-10.0, 0.9, 16.2),
        v(-10.0, 0.0, 13.6),
    );
    for k in 0..6 {
        p.slab(
            v(-10.0 + 0.4 * (k as f32 - 2.5), 0.015, 14.8 - 0.3 * k as f32),
            0.3 * k as f32,
            v(0.3, 0.01, 0.2),
            Mat::Paper,
        );
    }
    plant(p, 12.0, 16.0);
    // The counter's other half: a coffee machine, mugs nobody collects, and a fridge for the
    // lunches that were promised.
    p.prop(&COFFEE_MACHINE, v(-23.4, 1.12, 16.0), 0.0, false);
    for dx in [-0.4, 0.0, 0.4] {
        p.prim(
            PrimKind::Cylinder,
            v(-20.9 + dx, 1.2, 16.2),
            v(0.09, 0.12, 0.09),
            Mat::White,
        );
    }
    p.prop(&FRIDGE, v(-25.8, 0.0, 16.3), 0.0, true);
    p.sign_near(-25.8, 17.9, 2.9, 1.0, Sign::ObCulture);
    // Aisle markings along the route's sides, and a trolley left mid-delivery.
    for z in [-4.8, 4.8] {
        floor_line(p, pt(-26.0, z), pt(12.0, z), 0.14, Mat::Yellow);
    }
    for k in 0..3 {
        floor_line(
            p,
            pt(-24.0 + k as f32 * 14.0, -6.0),
            pt(-24.0 + k as f32 * 14.0, 6.0),
            0.1,
            Mat::Hazard,
        );
    }
    crates(p, -2.0, 6.4, 0.2, 2);
    p.boxed(v(0.2, 0.5, 6.4), v(0.08, 1.0, 0.7), Mat::Metal, false);
    p.boxed(v(-0.2, 0.08, 6.4), v(1.0, 0.04, 0.7), Mat::Metal, false);
    // Flat-packed boxes against the walls and the returns pile by the van.
    crates(p, 14.5, -14.5, 0.1, 3);
    crates(p, 14.2, 13.5, -0.1, 2);
    crates(p, 28.5, -10.2, 0.0, 3);
    crates(p, 37.0, 10.2, 0.2, 2);
    crates(p, 47.0, -9.5, 0.0, 2);
    crates(p, 47.5, 9.5, 0.3, 3);
    water_cooler(p, 26.0, 9.0);
    // Wall paper: an inventory that does not match, a notice that the printer is for tickets only.
    p.poster_near(-14.0, 17.9, 2.4, Vec2::new(1.6, 1.2), Tex::Misc);
    p.poster_near(2.0, 17.9, 2.4, Vec2::new(1.6, 1.2), Tex::OutOfOrder);
    p.poster_near(-40.0, 1.9, 2.3, Vec2::new(1.4, 1.1), Tex::Stickies);
    p.poster_near(-31.0, -1.9, 2.3, Vec2::new(1.4, 1.1), Tex::Gantt);
    for z in [-12.0, 12.0] {
        bunting(
            p,
            (-24.0, z),
            (12.0, z),
            3.8,
            [Mat::NeonCyan, Mat::White, Mat::NeonYellow],
        );
    }
    // North cell: pallets of returns; south cell: a break table.
    crates(p, -11.5, -28.5, 0.1, 3);
    crates(p, -24.5, -28.0, 0.0, 2);
    p.prop(&TABLE_ROUND, v(14.0, 0.0, 34.0), 0.0, true);
    p.prop(&STOOL_BAR, v(12.4, 0.0, 34.0), 0.0, false);
    p.prop(&STOOL_BAR, v(15.6, 0.0, 34.0), 0.0, false);
    // Boss pod: the delivery van's loading bay and a ramp of boxes.
    p.boxed(v(51.0, 1.3, 0.0), v(3.4, 2.6, 7.0), Mat::CarWhite, true);
    p.boxed(v(51.0, 0.7, 0.0), v(3.44, 0.6, 7.04), Mat::Yellow, false);
    p.boxed(v(49.2, 1.6, 0.0), v(0.04, 1.0, 5.0), Mat::Window, false);
    for z in [-3.0, 3.0] {
        p.prim(
            PrimKind::Cylinder,
            v(50.0, 0.4, z),
            v(0.8, 0.8, 0.8),
            Mat::Tire,
        );
    }
    for (x, z) in [(32.0, -9.0), (34.0, -9.0), (33.0, 9.0)] {
        p.boxed(v(x, 0.4, z), v(1.4, 0.8, 1.0), Mat::Cardboard, true);
    }
    p.sign_near(40.0, -11.9, 2.6, 3.4, Sign::ObCage);
    // Side cells: more shelves in the north one, a break area in the south.
    shelving(p, -21.0, -35.0, 3, 9, 4);
    lounge(p, 10.0, 36.0, PI, false);

    ceiling_lights(p);
    for (x, z) in [
        (-44.0, 0.0),
        (-18.0, 0.0),
        (-6.0, 8.0),
        (6.0, -8.0),
        (40.0, 0.0),
        (-18.0, -32.0),
        (10.0, 32.0),
    ] {
        p.light(v(x, 3.6, z), Color::srgb(0.95, 0.97, 1.0), 400_000.0);
    }
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:onboarding/1", (-50.0, 0.0), Sign::ObToReception),
                ("door:onboarding/3", (54.0, 6.0), Sign::ObToDesk),
            ],
            packs: &CAGE.packs(),
            boss: (40.0, 0.0),
            npc: None,
        },
    );
    theme(2)
}

// ------------------------------------------------------------------ onboarding/3

const DESK: Shape = Shape {
    arrival_x: -44.0,
    hall: (-6.0, 22.0, 18.0),
    cells: [(-18.0, -32.0), (10.0, 32.0)],
    boss: (40.0, 14.0, 12.0),
};

fn desk_room(p: &mut Plan) -> Theme {
    DESK.pods(p);
    p.build_walls();
    wall_dress(p, Mat::Fabric2, Mat::White, Mat::Trim);
    cove(p, Mat::Candle);

    runner(
        p,
        &[pt(-48.0, 0.0), pt(-6.0, 0.0), pt(26.0, 0.0), pt(37.0, 0.0)],
        2.4,
        Mat::Fabric2,
        Mat::RunnerEdge,
    );
    // Open-plan rows either side of the walkway: yours is the one with the marker nameplate.
    for (k, x) in [-20.0, -12.0, -4.0, 4.0, 12.0].into_iter().enumerate() {
        cubicle(p, k, x, -8.0, 0.0);
        cubicle(p, k + 1, x, 8.0, PI);
    }
    p.sign(v(-4.0, 1.75, -9.4), 0.0, 1.1, Sign::ObNameplate);
    // A second, back row against the walls (gaps left where the side cells open up), so the room
    // reads as an open plan with people missing rather than a corridor of cubicles.
    for (k, x) in [-12.0, 4.0, 12.0].into_iter().enumerate() {
        cubicle(p, k + 2, x, -15.5, 0.0);
    }
    for (k, x) in [-20.0, -12.0, -4.0, 4.0].into_iter().enumerate() {
        cubicle(p, k + 1, x, 15.5, PI);
    }
    // A monitor taped to the wall above the empty desk: silver tape on every corner, a post-it
    // that says "ask Dave", and a bare desk and chair nobody has been assigned.
    if let Some(w) = WallSpot::at(p, -4.0, -18.0) {
        w.put(p, 0.0, 2.2, 0.04, v(1.5, 1.0, 0.05), Mat::Dark);
        w.put(p, 0.0, 2.2, 0.075, v(1.38, 0.88, 0.02), Mat::ScreenGlow);
        for (sx, sy) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            w.put(
                p,
                sx * 0.72,
                2.2 + sy * 0.47,
                0.1,
                v(0.4, 0.09, 0.02),
                Mat::Chrome,
            );
        }
        w.put(p, 0.4, 2.35, 0.1, v(0.3, 0.3, 0.02), Mat::Yellow);
    }
    p.prop(&CHAIR_DESK, v(-4.0, 0.0, -16.2), PI + 0.4, false);
    p.boxed(v(-4.0, 0.75, -17.4), v(1.6, 0.06, 0.8), Mat::Wood, true);
    for sx in [-0.7, 0.7] {
        p.boxed(
            v(-4.0 + sx, 0.36, -17.4),
            v(0.06, 0.72, 0.7),
            Mat::Metal,
            false,
        );
    }
    // The welcome basket on the empty desk: cellophane, a lanyard, one pen and the Culture Deck.
    p.boxed(v(-4.0, 0.95, -6.9), v(0.7, 0.3, 0.5), Mat::Cardboard, false);
    p.prim(
        PrimKind::Sphere,
        v(-4.0, 1.25, -6.9),
        v(0.9, 0.7, 0.7),
        Mat::Glass,
    );
    p.sign(v(-3.7, 1.25, -6.62), 0.0, 0.5, Sign::ObCulture);
    p.boxed(v(-4.3, 1.15, -6.9), v(0.04, 0.02, 0.3), Mat::Red, false);
    p.boxed(
        v(-4.55, 1.12, -6.9),
        v(0.012, 0.012, 0.28),
        Mat::NeonCyan,
        false,
    );
    for (dx, m) in [
        (-1.6, Mat::NeonPink),
        (1.5, Mat::NeonCyan),
        (0.2, Mat::NeonYellow),
    ] {
        balloon(p, -4.0 + dx, -7.4, 1.0, m);
    }
    // The chat widget in the corner: a bubble on a stand, three dots that blink when you look away.
    p.boxed(v(51.0, 0.5, -9.0), v(0.9, 1.0, 0.9), Mat::White, true);
    p.prim(
        PrimKind::Sphere,
        v(51.0, 1.45, -9.0),
        v(1.1, 0.9, 0.9),
        Mat::BlinkBlue,
    );
    p.boxed(
        v(51.0, 1.45, -8.5),
        v(0.5, 0.12, 0.05),
        Mat::ScreenGlow,
        false,
    );
    p.prim(
        PrimKind::Cylinder,
        v(51.0, 2.1, -9.0),
        v(0.08, 1.4, 0.08),
        Mat::Chrome,
    );
    p.boxed(v(50.4, 3.2, -9.0), v(2.2, 1.1, 0.12), Mat::White, false);
    p.boxed(
        v(50.4, 3.2, -8.93),
        v(2.0, 0.9, 0.02),
        Mat::ScreenGlow,
        false,
    );
    for (k, m) in [Mat::BlinkBlue, Mat::BlinkGreenA, Mat::BlinkBlue]
        .into_iter()
        .enumerate()
    {
        p.prim(
            PrimKind::Sphere,
            v(50.0 + k as f32 * 0.4, 3.2, -8.9),
            v(0.22, 0.22, 0.22),
            m,
        );
    }
    p.prim(
        PrimKind::Cone,
        v(51.1, 2.45, -9.0),
        v(0.35, 0.3, 0.35),
        Mat::White,
    );
    p.sign_near(40.0, 11.9, 2.6, 3.4, Sign::ObToDesk);
    plant(p, 24.0, -10.0);
    plant(p, 24.0, 10.0);
    // Side cells: a kitchen nobody can find in the north one, a meeting pod in the south.
    counter(p, -18.0, -36.0, 5.0);
    lounge(p, 10.0, 36.0, PI, true);
    super::marie::onboarding(p);
    p.poster_near(-6.0, 17.9, 2.3, Vec2::new(1.6, 1.4), Tex::Standup);
    // The rest of the floor: kitchen things in the north cell, a fridge with somebody's name on
    // everything, kanban boards that disagree, and the far end of the open plan.
    p.prop(&FRIDGE, v(-23.5, 0.0, -37.2), 0.0, true);
    p.prop(&COFFEE_MACHINE, v(-19.5, 1.12, -36.0), 0.0, false);
    p.prop(&TABLE_ROUND, v(-14.0, 0.0, -31.0), 0.0, true);
    for dz in [-1.6, 1.6] {
        p.prop(&STOOL_BAR, v(-14.0, 0.0, -31.0 + dz), 0.0, false);
    }
    p.prop(&TRASHCAN, v(-12.0, 0.0, -37.4), 0.0, true);
    p.poster_near(-18.0, -39.0, 2.4, Vec2::new(1.6, 1.2), Tex::Roadmap);
    p.poster_near(-24.0, 17.9, 2.4, Vec2::new(1.8, 1.4), Tex::Stickies);
    p.poster_near(14.0, -17.9, 2.4, Vec2::new(1.8, 1.4), Tex::Gantt);
    p.poster_near(14.0, 17.9, 2.4, Vec2::new(1.6, 1.2), Tex::PromptTips);
    p.poster_near(-34.0, 1.9, 2.3, Vec2::new(1.2, 1.5), Tex::Wellness);
    p.poster_near(-40.0, -1.9, 2.3, Vec2::new(1.4, 1.1), Tex::MandatoryFun);
    water_cooler(p, 14.4, -16.8);
    vending(p, 12.4, 17.2, Face::NegZ);
    p.prop(&COAT_RACK, v(-26.2, 0.0, 6.0), 0.0, true);
    // Welcome rug under the boss, and the banner on the pod's far wall that has been up all week.
    p.prop(&RUG, v(34.0, 0.0, 0.0), 0.0, false);
    round_patch(p, 34.0, 0.0, 7.0, Mat::Fabric1);
    if let Some(w) = WallSpot::at(p, 53.8, 4.0) {
        w.put(p, 0.0, 2.8, 0.05, v(4.0, 1.4, 0.05), Mat::White);
        w.sign(p, 0.0, 2.8, 3.8, Sign::ObNewStarters);
    }
    for (x, z) in [(46.0, -9.5), (46.0, 9.5)] {
        crates(p, x, z, 0.2, 2);
    }
    for z in [-13.0, 13.0] {
        bunting(
            p,
            (-26.0, z),
            (14.0, z),
            3.7,
            [Mat::NeonYellow, Mat::NeonPink, Mat::NeonCyan],
        );
    }

    ceiling_lights(p);
    for (x, z) in [
        (-44.0, 0.0),
        (-18.0, 0.0),
        (-6.0, 8.0),
        (6.0, -8.0),
        (40.0, 0.0),
        (-18.0, -32.0),
        (10.0, 32.0),
    ] {
        p.light(v(x, 3.6, z), Color::srgb(1.0, 0.95, 0.88), 400_000.0);
    }
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:onboarding/2", (-50.0, 0.0), Sign::ObToCage),
                ("door:campus", (54.0, -6.0), Sign::ToCampus),
            ],
            packs: &DESK.packs(),
            boss: (34.0, 0.0),
            // 15 m from the boss and by the exit door, so the turn-in never pulls it.
            npc: Some((48.0, -8.0)),
        },
    );
    theme(3)
}
