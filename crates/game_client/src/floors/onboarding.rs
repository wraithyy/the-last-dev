//! The Onboarding dungeon (docs/DUNGEONS.md): the first week at a generic office in three
//! pastel rooms. onboarding/1 is reception and the badge desk, onboarding/2 the equipment cage,
//! onboarding/3 "your desk". Same boxes-and-primitives budget as the offsite, and no real
//! company's floor plan: every room is a corridor into a hall with two side cells for the packs
//! and a pod for the boss.

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::offsite::{cove, floor_patch, wall_dress};
use super::props::{cubicle, lounge, printer, sad_plant};
use super::shape::{rounded_rect, stroke};
use super::themes::{ceiling_lights, pt, runner, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::PI;

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
        p.boxed(v(-20.0, 1.15, z), v(0.3, 0.08, 0.5), Mat::LedRed, false);
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

/// Floor-to-ceiling shelving along x: a steel frame and boxes, one laptop per ten of them.
fn shelving(p: &mut Plan, x0: f32, z: f32, bays: usize, laptop_bay: usize) {
    for k in 0..bays {
        let x = x0 + k as f32 * 3.2;
        p.boxed(v(x, 1.2, z), v(3.0, 2.4, 0.9), Mat::Metal, true);
        for (row, y) in [0.45, 1.05, 1.65, 2.2].into_iter().enumerate() {
            let m = if (k + row) % 3 == 0 {
                Mat::Paper
            } else {
                Mat::Cardboard
            };
            p.boxed(v(x - 0.7, y, z), v(0.8, 0.45, 0.8), m, false);
            p.boxed(v(x + 0.7, y, z), v(0.8, 0.45, 0.8), m, false);
        }
        if k == laptop_bay {
            p.boxed(v(x, 1.1, z + 0.05), v(0.7, 0.05, 0.5), Mat::Dark, false);
            p.boxed(
                v(x, 1.14, z + 0.05),
                v(0.6, 0.02, 0.4),
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
    shelving(p, -22.0, -9.0, 9, 5);
    shelving(p, -22.0, 9.0, 9, 2);
    shelving(p, -22.0, -15.5, 9, 7);
    // The wall calendar: "Week 6" circled, crossed out, circled again.
    p.sign_near(-6.0, -17.9, 2.5, 4.2, Sign::ObWeek);
    p.slab(v(-4.0, 2.4, -17.8), 0.0, v(1.6, 0.02, 0.02), Mat::Red);
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
    plant(p, 12.0, -16.0);
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
    shelving(p, -21.0, -35.0, 3, 9);
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
    // A monitor taped to the wall above the empty desk, and the welcome basket.
    p.boxed(v(-4.0, 2.2, -16.7), v(1.2, 0.8, 0.05), Mat::Dark, false);
    p.boxed(
        v(-4.0, 2.2, -16.66),
        v(1.1, 0.7, 0.02),
        Mat::ScreenGlow,
        false,
    );
    for dx in [-0.6, 0.6] {
        p.slab(
            v(-4.0 + dx, 2.2, -16.64),
            0.5,
            v(0.35, 0.1, 0.02),
            Mat::Yellow,
        );
    }
    p.boxed(v(-4.0, 0.95, -6.9), v(0.7, 0.3, 0.5), Mat::Cardboard, false);
    p.sign(v(-3.7, 1.25, -6.62), 0.0, 0.5, Sign::ObCulture);
    p.boxed(v(-4.3, 1.15, -6.9), v(0.04, 0.02, 0.3), Mat::Red, false);
    // The chat widget in the corner: it blinks when you look away.
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
    p.sign_near(40.0, 11.9, 2.6, 3.4, Sign::ObToDesk);
    plant(p, 24.0, -10.0);
    plant(p, 24.0, 10.0);
    // Side cells: a kitchen nobody can find in the north one, a meeting pod in the south.
    counter(p, -18.0, -36.0, 5.0);
    lounge(p, 10.0, 36.0, PI, true);
    p.poster_near(-6.0, 17.9, 2.3, Vec2::new(1.6, 1.4), Tex::Standup);

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
