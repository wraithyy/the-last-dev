//! The Company Calendar's venues (`combat_core::corporate`): five small indoor rooms from the
//! office kit, one per minigame, with no doors (the minigame frame moves the group in and out).
//! - `review/1`: meeting room 4B, one table, your manager across it.
//! - `reorg/1`: an open-plan floor with eight landmarked desk slots (`desk_<n>`) and six
//!   partition spots (`wall_<n>`); the desks and partitions themselves are placed at run time
//!   from the view (`minigame.rs`), since they move every reorg.
//! - `migration/1`: the IT back office, the old terminal (`term_old`) west, the new (`term_new`)
//!   east.
//! - `hackathon/1`: the innovation space, tables in the middle, snack spots (`pizza_<n>`,
//!   `drink_<n>`) around, the demo stage (`stage_<n>`) north.
//! - `openspace/1`: an open-plan floor around one desk pod (`pod`), with the disturbers' loops
//!   (`route:<route>:<n>`) from the edges to it.

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble, yaw_towards};
use super::props::{
    cubicle, long_table, mini_fridge, printer, sad_plant, server_rack, water_cooler,
};
use super::shape::rounded_rect;
use super::themes::{ceiling_lights, pt, v};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub const REVIEW: &str = "review/1";
pub const REORG: &str = "reorg/1";
pub const MIGRATION: &str = "migration/1";
pub const HACKATHON: &str = "hackathon/1";
pub const OPENSPACE: &str = "openspace/1";
pub const ZONES: [&str; 5] = [REVIEW, REORG, MIGRATION, HACKATHON, OPENSPACE];

/// Reorg desk slots: where a colleague stands, and the point their desk lies towards. The
/// order matches `ReorgDef::slots` in `game.ron` ("by the window", "next to the printer", ...).
pub const REORG_SLOTS: [((f32, f32), (f32, f32)); 8] = [
    ((-7.5, -6.0), (-7.5, -10.0)),
    ((7.0, -6.2), (7.0, -10.0)),
    ((10.6, 1.0), (14.0, 1.0)),
    ((7.5, 6.2), (7.5, 10.0)),
    ((-1.5, 6.2), (-1.5, 10.0)),
    ((-10.4, 5.0), (-14.0, 5.0)),
    ((1.0, -6.2), (1.0, -10.0)),
    ((-10.4, -1.8), (-14.0, -1.8)),
];

/// Reorg partition spots: centre and yaw (a 2.4 m screen along its local X).
pub const REORG_WALLS: [((f32, f32), f32); 6] = [
    ((-4.0, -2.2), 0.0),
    ((4.0, -2.2), 0.0),
    ((-4.5, 2.6), FRAC_PI_2),
    ((4.5, 2.6), FRAC_PI_2),
    ((0.0, 3.0), 0.0),
    ((-6.2, -3.6), FRAC_PI_2),
];

pub fn build(id: &str) -> Option<FloorLayout> {
    Some(match id {
        REVIEW => review(),
        REORG => reorg(),
        MIGRATION => migration(),
        HACKATHON => hackathon(),
        OPENSPACE => openspace(),
        _ => return None,
    })
}

fn theme(wall: Color, carpet: [f32; 3], fog: Color) -> Theme {
    let c = Color::srgb;
    Theme {
        wall,
        trim: c(0.3, 0.32, 0.36),
        accent: LinearRgba::rgb(0.6, 0.7, 1.0),
        fabric: [c(0.35, 0.45, 0.6), c(0.55, 0.5, 0.45), c(0.4, 0.55, 0.45)],
        floor: FloorKind::Carpet(carpet),
        fog,
        ambient: c(0.95, 0.94, 0.9),
        ambient_brightness: 760.0,
        sun: c(0.9, 0.9, 0.86),
        sun_illuminance: 3800.0,
        fog_range: (30.0, 90.0),
    }
}

fn anchor_at(p: &mut Plan, name: &'static str, x: f32, z: f32, look: (f32, f32)) {
    let a = v(x, 0.0, z);
    p.anchor(name, a, yaw_towards(a, v(look.0, 0.0, look.1)));
}

fn lights(p: &mut Plan, spots: &[(f32, f32)]) {
    ceiling_lights(p);
    for (x, z) in spots {
        p.light(v(*x, 3.6, *z), Color::srgb(0.98, 0.95, 0.88), 400_000.0);
    }
}

/// A desk facing local -Z with a screen and a keyboard, placed at (x, z) turned by `yaw`.
fn desk_with_screen(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        let top = DESK.size[1] * FURNITURE_SCALE;
        let back = -DESK.size[2] * FURNITURE_SCALE / 2.0;
        p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
        p.prop(&SCREEN, v(0.0, top, back + 0.2), 0.0, false);
        p.prop(&KEYBOARD, v(0.0, top, back + 0.62), 0.0, false);
    });
}

/// Meeting room 4B: a table, two chairs, the rating scale on the screen, a wall clock. Roomy
/// enough for the camera to sit behind you and still show the manager over your shoulder.
fn review() -> FloorLayout {
    let mut p = Plan::default();
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 8.0, 7.0, 0.6, 0.0), Mat::Wall);
    p.route = vec![pt(0.0, 4.0), pt(0.0, -4.0)];
    p.build_walls();
    p.prop(&TABLE, v(0.0, 0.0, -1.0), 0.0, true);
    p.prop(&CHAIR_MODERN, v(0.0, 0.0, -3.3), 0.0, false);
    p.prop(&CHAIR_MODERN, v(0.7, 0.0, 0.9), PI + 0.4, false);
    p.prop(&LAPTOP, v(-0.3, 0.79, -1.2), PI, false);
    p.prop(&POTTED_PLANT, v(-7.2, 0.0, -6.2), 0.0, true);
    p.prop(&POTTED_PLANT, v(7.2, 0.0, -6.2), 0.0, true);
    p.prop(&LOUNGE_SOFA, v(-6.6, 0.0, 3.0), FRAC_PI_2, true);
    p.sign_near(0.0, -7.0, 2.3, 3.0, Sign::ReviewScale);
    p.sign_near(-8.0, -2.0, 2.2, 2.2, Sign::ReviewRoom);
    p.poster_near(8.0, -1.0, 2.0, Vec2::new(1.8, 1.2), Tex::Kpi);
    // The wall clock the manager keeps glancing at.
    p.prim_yawed(
        PrimKind::Cylinder,
        v(3.4, 2.7, -6.93),
        v(0.6, 0.05, 0.6),
        FRAC_PI_2,
        Mat::White,
    );
    lights(&mut p, &[(0.0, -1.0), (-4.0, 3.0), (4.0, 3.0)]);
    anchor_at(&mut p, "npc_manager", 0.0, -2.6, (0.0, 2.0));
    anchor_at(&mut p, "player_spawn", 0.8, 3.4, (0.0, -2.6));
    anchor_at(&mut p, "graveyard", -3.0, 4.5, (0.0, 0.0));
    assemble(REVIEW, 1, "Meeting Room 4B", false, p, {
        theme(
            Color::srgb(0.82, 0.82, 0.86),
            [1.4, 1.5, 1.8],
            Color::srgb(0.6, 0.62, 0.66),
        )
    })
}

/// Open plan, third floor: landmarks around the walls (windows, printer, coffee, the Synergy
/// poster, a dead plant, the fire exit, standing desks, the whiteboard), the desks in between.
fn reorg() -> FloorLayout {
    let mut p = Plan::default();
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 14.0, 10.0, 1.0, 0.0), Mat::Wall);
    p.route = vec![pt(-10.0, 0.0), pt(10.0, 0.0)];
    p.build_walls();
    for x in [-10.0, -6.0] {
        p.window_near(x, -10.0, Vec2::new(3.2, 1.6), Tex::Skyline, Mat::Trim);
    }
    printer(&mut p, 10.0, -9.2, Face::PosZ);
    p.boxed(v(13.4, 0.5, 1.0), v(1.0, 1.0, 2.6), Mat::Wood, true);
    p.prop(&COFFEE_MACHINE, v(13.4, 1.0, 1.0), -FRAC_PI_2, false);
    p.poster_near(7.5, 10.0, 2.0, Vec2::new(2.2, 1.5), Tex::Synergy);
    sad_plant(&mut p, -3.5, 9.0);
    p.sign_near(-14.0, 5.0, 2.4, 2.0, Sign::ReorgExit);
    p.boxed(v(-13.9, 1.1, 5.0), v(0.1, 2.2, 1.4), Mat::Door, false);
    // The standing desks: tall and empty, because everyone sits at them.
    for x in [-0.6, 2.6] {
        p.boxed(v(x, 0.55, -8.8), v(1.4, 1.1, 0.7), Mat::Wood, true);
        p.prop(&LAPTOP, v(x, 1.1, -8.8), 0.0, false);
    }
    p.sign_near(-14.0, -1.8, 1.9, 2.4, Sign::WhiteboardB);
    p.sign_near(14.0, -5.0, 2.2, 2.4, Sign::ReorgChart);
    water_cooler(&mut p, 12.8, 8.6);
    lights(
        &mut p,
        &[
            (-7.0, -4.0),
            (7.0, -4.0),
            (-7.0, 5.0),
            (7.0, 5.0),
            (0.0, 0.0),
        ],
    );
    for (k, ((x, z), (tx, tz))) in REORG_SLOTS.iter().enumerate() {
        let name: &'static str = [
            "desk_1", "desk_2", "desk_3", "desk_4", "desk_5", "desk_6", "desk_7", "desk_8",
        ][k];
        anchor_at(&mut p, name, *x, *z, (*tx, *tz));
    }
    for (k, ((x, z), yaw)) in REORG_WALLS.iter().enumerate() {
        let name: &'static str = ["wall_1", "wall_2", "wall_3", "wall_4", "wall_5", "wall_6"][k];
        p.anchor(name, v(*x, 0.0, *z), *yaw);
    }
    anchor_at(&mut p, "player_spawn", 0.0, -0.5, (0.0, -5.0));
    anchor_at(&mut p, "graveyard", 0.0, 0.5, (0.0, 5.0));
    assemble(REORG, 1, "Open Plan, Third Floor", false, p, {
        theme(
            Color::srgb(0.86, 0.84, 0.8),
            [1.7, 1.6, 1.4],
            Color::srgb(0.62, 0.6, 0.56),
        )
    })
}

/// The IT back office: TICKETRON 2009 on a beige desk in the west, FlowSphere Cloud on a glass
/// desk in the east, moving boxes and a deadline in between.
fn migration() -> FloorLayout {
    let mut p = Plan::default();
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 9.0, 6.0, 0.8, 0.0), Mat::Wall);
    p.route = vec![pt(-6.0, 0.0), pt(6.0, 0.0)];
    p.build_walls();
    // The old terminal: a beige tower, a CRT with a green screen, a sticky note on top.
    p.boxed(v(-7.7, 0.4, 0.0), v(1.2, 0.8, 2.2), Mat::Beige, true);
    p.boxed(v(-7.8, 1.15, 0.0), v(0.7, 0.6, 0.7), Mat::Beige, false);
    p.boxed(
        v(-7.43, 1.17, 0.0),
        v(0.02, 0.42, 0.52),
        Mat::LedGreen,
        false,
    );
    p.boxed(v(-7.6, 0.82, 0.0), v(0.3, 0.03, 0.9), Mat::Dark, false);
    p.sign_near(-9.0, 0.0, 2.4, 2.6, Sign::MigrationOld);
    // The new one: glass desk, a laptop and a very large screen.
    p.prop(&TABLE_GLASS, v(7.6, 0.0, 0.0), FRAC_PI_2, true);
    p.prop(&LAPTOP, v(7.3, 0.79, 0.4), -FRAC_PI_2, false);
    p.prop(&TV, v(7.95, 0.79, -0.2), -FRAC_PI_2, false);
    p.sign_near(9.0, 0.0, 2.4, 2.6, Sign::MigrationNew);
    p.sign_near(0.0, -6.0, 2.2, 2.6, Sign::MigrationDeadline);
    for (x, z) in [(-2.0, 4.6), (-1.2, 4.9), (1.6, 4.7), (2.3, -4.8)] {
        p.prop(&BOX_CLOSED, v(x, 0.0, z), 0.3, true);
    }
    server_rack(&mut p, -4.0, -5.0, 0.0);
    server_rack(&mut p, 4.0, -5.0, 0.0);
    lights(&mut p, &[(-5.0, 0.0), (5.0, 0.0), (0.0, 0.0)]);
    anchor_at(&mut p, "term_old", -6.2, 0.0, (-9.0, 0.0));
    anchor_at(&mut p, "term_new", 6.2, 0.0, (9.0, 0.0));
    anchor_at(&mut p, "npc_champion", 0.0, -3.4, (0.0, 0.0));
    anchor_at(&mut p, "player_spawn", 0.0, 2.6, (-6.0, 0.0));
    anchor_at(&mut p, "graveyard", 0.0, 3.6, (0.0, 0.0));
    assemble(MIGRATION, 1, "IT Back Office", false, p, {
        theme(
            Color::srgb(0.72, 0.74, 0.78),
            [1.2, 1.25, 1.35],
            Color::srgb(0.5, 0.52, 0.56),
        )
    })
}

pub const PIZZAS: [(f32, f32); 6] = [
    (-9.0, -3.0),
    (-9.0, 4.0),
    (9.0, -3.0),
    (9.0, 4.0),
    (-3.5, 7.6),
    (3.5, 7.6),
];
pub const DRINKS: [(f32, f32); 4] = [(-5.5, -4.6), (5.5, -4.6), (-10.0, 8.0), (10.0, 8.0)];

/// The innovation space: two long tables of laptops, snacks around the walls, the stage north.
fn hackathon() -> FloorLayout {
    let mut p = Plan::default();
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 12.0, 10.0, 1.0, 0.0), Mat::Wall);
    p.route = vec![pt(0.0, 6.0), pt(0.0, -6.0)];
    p.build_walls();
    // The stage: low enough to step onto, the demo screen behind it.
    p.boxed(v(0.0, 0.12, -8.3), v(9.0, 0.24, 3.2), Mat::Dark, true);
    p.sign_near(0.0, -10.0, 2.5, 4.4, Sign::HackStage);
    p.sign_near(-12.0, 0.0, 2.4, 3.2, Sign::HackBanner);
    p.sign_near(12.0, 0.0, 2.2, 2.4, Sign::HackPizza);
    long_table(&mut p, -2.6, 1.5, 2, false);
    long_table(&mut p, 2.6, 1.5, 2, false);
    for x in [-3.6, -1.6, 1.6, 3.6] {
        p.prop(&LAPTOP, v(x, 0.79, 1.3), 0.0, false);
    }
    // A table under every snack spot; the client puts the pizza or the can on it.
    for (x, z) in PIZZAS.iter().chain(DRINKS.iter()) {
        p.prop(&SIDE_TABLE, v(*x, 0.0, *z), 0.0, false);
    }
    mini_fridge(&mut p, -11.2, 8.8, Face::PosX);
    mini_fridge(&mut p, 11.2, 8.8, Face::NegX);
    p.poster_near(-6.0, 10.0, 2.0, Vec2::new(1.6, 1.1), Tex::PizzaBox);
    lights(&mut p, &[(0.0, -7.0), (-6.0, 2.0), (6.0, 2.0), (0.0, 6.0)]);
    let names: [&'static str; 6] = [
        "pizza_1", "pizza_2", "pizza_3", "pizza_4", "pizza_5", "pizza_6",
    ];
    for (n, (x, z)) in names.into_iter().zip(PIZZAS) {
        p.anchor(n, v(x, 0.0, z), 0.0);
    }
    let names: [&'static str; 4] = ["drink_1", "drink_2", "drink_3", "drink_4"];
    for (n, (x, z)) in names.into_iter().zip(DRINKS) {
        p.anchor(n, v(x, 0.0, z), 0.0);
    }
    for (n, x) in [
        ("stage_1", -1.2),
        ("stage_2", 1.2),
        ("stage_3", -3.0),
        ("stage_4", 3.0),
    ] {
        anchor_at(&mut p, n, x, -7.6, (x, 0.0));
    }
    anchor_at(&mut p, "npc_mc", 0.0, -9.2, (0.0, 0.0));
    anchor_at(&mut p, "npc_hacker_1", -5.6, 1.5, (-2.6, 1.5));
    anchor_at(&mut p, "npc_hacker_2", 5.6, 1.5, (2.6, 1.5));
    anchor_at(&mut p, "player_spawn", 0.0, 4.6, (0.0, 0.0));
    anchor_at(&mut p, "graveyard", 0.0, 6.0, (0.0, 0.0));
    assemble(HACKATHON, 1, "The Innovation Space", false, p, {
        let mut t = theme(
            Color::srgb(0.3, 0.3, 0.38),
            [0.9, 0.9, 1.1],
            Color::srgb(0.2, 0.2, 0.28),
        );
        t.accent = LinearRgba::rgb(2.0, 0.8, 0.3);
        t.ambient_brightness = 620.0;
        t
    })
}

/// A disturber's loop: its two anchors, the far end and the end at the pod.
type FocusRoute = (&'static str, &'static str, (f32, f32), (f32, f32));

pub const FOCUS_ROUTES: [FocusRoute; 4] = [
    (
        "route:colleague:0",
        "route:colleague:1",
        (-12.5, 4.0),
        (-3.0, 1.0),
    ),
    ("route:caller:0", "route:caller:1", (12.5, 4.0), (3.0, 1.0)),
    ("route:cake:0", "route:cake:1", (-5.0, 9.5), (0.0, 3.4)),
    ("route:warden:0", "route:warden:1", (6.0, -9.5), (0.0, -3.2)),
];

/// Open plan, second floor: one pod of four desks in the middle (yours), cubicles round the
/// edges, and the corridors everyone walks to reach you.
fn openspace() -> FloorLayout {
    let mut p = Plan::default();
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 14.0, 11.0, 1.0, 0.0), Mat::Wall);
    p.route = vec![pt(-10.0, 0.0), pt(10.0, 0.0)];
    p.build_walls();
    // The pod: two desks facing two, a low divider down the middle.
    for (x, z, yaw) in [
        (-0.9, -0.5, PI),
        (0.9, -0.5, PI),
        (-0.9, 0.5, 0.0),
        (0.9, 0.5, 0.0),
    ] {
        desk_with_screen(&mut p, x, z, yaw);
    }
    p.boxed(v(0.0, 1.15, 0.0), v(3.6, 0.5, 0.06), Mat::Fabric1, false);
    for (i, (x, z, yaw)) in [
        (-10.0, -7.5, 0.0),
        (-6.5, -7.5, 0.0),
        (9.5, -7.5, 0.0),
        (-10.0, 7.5, PI),
        (9.5, 7.5, PI),
        (6.0, 7.5, PI),
    ]
    .into_iter()
    .enumerate()
    {
        cubicle(&mut p, i, x, z, yaw);
    }
    p.sign_near(-14.0, -2.0, 2.2, 2.4, Sign::OpenQuiet);
    p.sign_near(14.0, -2.0, 2.2, 2.4, Sign::OpenCollab);
    sad_plant(&mut p, -13.0, 10.0);
    p.prop(&POTTED_PLANT, v(13.0, 0.0, 10.0), 0.0, true);
    printer(&mut p, 2.5, -10.2, Face::PosZ);
    lights(
        &mut p,
        &[(0.0, 0.0), (-8.0, 0.0), (8.0, 0.0), (0.0, 7.0), (0.0, -7.0)],
    );
    p.anchor("pod", v(0.0, 0.0, 0.0), 0.0);
    for (a, b, far, near) in FOCUS_ROUTES {
        anchor_at(&mut p, a, far.0, far.1, near);
        anchor_at(&mut p, b, near.0, near.1, (0.0, 0.0));
    }
    anchor_at(&mut p, "player_spawn", 0.0, 1.9, (0.0, 0.0));
    anchor_at(&mut p, "graveyard", 0.0, 5.0, (0.0, 0.0));
    assemble(OPENSPACE, 1, "Open Plan, Second Floor", false, p, {
        theme(
            Color::srgb(0.84, 0.86, 0.84),
            [1.5, 1.6, 1.5],
            Color::srgb(0.6, 0.62, 0.6),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floors::testgrid::Grid;
    use crate::floors::zones::build_zone;

    #[test]
    fn every_venue_anchor_is_free_inside_and_reachable_from_the_spawn() {
        for id in ZONES {
            let l = build_zone(id).unwrap_or_else(|| panic!("zone {id} missing"));
            let spawn = l.pos("player_spawn");
            let g = Grid::from(&l, spawn);
            assert!(l.anchor("graveyard").is_some(), "{id}");
            assert!(!l.outdoor, "{id}");
            for (name, p, _) in &l.anchors {
                assert!(
                    l.footprint.contains(Vec2::new(p.x, p.z)),
                    "{id}: {name} outside"
                );
                // The pod is the middle of the desk block, a reference point nobody stands on.
                if *name == "pod" {
                    continue;
                }
                assert!(g.free_anchor(&l, *p), "{id}: {name} inside a blocker");
                assert!(g.at_anchor(&l, *p).is_finite(), "{id}: {name} unreachable");
            }
        }
    }

    #[test]
    fn the_venues_carry_what_their_rules_ask_for() {
        let has = |id: &str, n: &str| build_zone(id).is_some_and(|l| l.anchor(n).is_some());
        assert!(has(REVIEW, "npc_manager"));
        for k in 1..=8 {
            assert!(has(REORG, &format!("desk_{k}")));
        }
        for k in 1..=6 {
            assert!(has(REORG, &format!("wall_{k}")));
        }
        assert!(has(MIGRATION, "term_old") && has(MIGRATION, "term_new"));
        for k in 1..=6 {
            assert!(has(HACKATHON, &format!("pizza_{k}")));
        }
        assert!(has(HACKATHON, "drink_4") && has(HACKATHON, "stage_1"));
        assert!(has(OPENSPACE, "pod") && has(OPENSPACE, "route:warden:1"));
    }

    #[test]
    fn reorg_desks_are_far_enough_apart_to_talk_to_one_at_a_time() {
        for (i, (a, _)) in REORG_SLOTS.iter().enumerate() {
            for (b, _) in &REORG_SLOTS[i + 1..] {
                let d = Vec2::new(a.0 - b.0, a.1 - b.1).length();
                assert!(d > 2.0 * 2.4, "{a:?} {b:?} {d}");
            }
        }
    }
}
