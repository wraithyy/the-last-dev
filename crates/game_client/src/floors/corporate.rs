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
//! - `home/1`: a small flat for Stay Green: the bedroom desk (`desk`, and `cat_desk` on its
//!   keyboard), the laundry by the washer (`laundry`), the front door (`door`), a kitchen and
//!   the cat's bed (`cat_bed`).

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble, yaw_towards};
use super::offsite::{
    balloons, beams, bunting, chair_to, cove, floor_patch, plant_pair, round_patch, wall_dress,
    wall_prop,
};
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
pub const HOME: &str = "home/1";
pub const ZONES: [&str; 6] = [REVIEW, REORG, MIGRATION, HACKATHON, OPENSPACE, HOME];

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
        HOME => home(),
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

/// Wainscot, crown and a cove strip: what separates a meeting room from a box with a ceiling.
fn room_dress(p: &mut Plan, dado: Mat, rail: Mat, cove_mat: Mat) {
    wall_dress(p, dado, rail, Mat::Trim);
    cove(p, cove_mat);
}

/// A coloured pool of light, for rooms whose mood is one colour (the lights() helper is white).
fn glow(p: &mut Plan, x: f32, z: f32, color: Color) {
    p.light(v(x, 3.3, z), color, 300_000.0);
}

fn review_dress(p: &mut Plan) {
    room_dress(p, Mat::Wood, Mat::Gold, Mat::Candle);
    beams(p, -6.0, 6.0, -7.0, 7.0, 4.0, Mat::Wood);
    // Rug under the table; no pendant lamp, it would hide the rating scale from the player.
    p.prop_scaled(&RUG, v(0.0, 0.0, -1.0), 0.0, false, 1.6);
    // Waiting chairs along the wall, and a water jug on the credenza for the moral support.
    for (x, z) in [(-5.5, -4.8), (-4.0, -5.4), (5.2, -4.4), (6.2, 1.5)] {
        chair_to(p, x, z, 0.0, -1.0);
    }
    wall_prop(p, &CABINET_TV, -4.0, 6.9, true);
    p.prop(
        &TV,
        v(-4.0, CABINET_TV.size[1] * FURNITURE_SCALE, 6.45),
        PI,
        false,
    );
    p.prim(
        PrimKind::Cylinder,
        v(-5.2, 1.3, 6.4),
        v(0.3, 0.5, 0.3),
        Mat::Water,
    );
    // Bookshelves of binders, a window onto the skyline, motivation in frames.
    wall_prop(p, &BOOKCASE_CLOSED_WIDE, 6.0, -6.9, true);
    p.window_near(-5.5, -7.0, Vec2::new(3.0, 1.5), Tex::Skyline, Mat::Trim);
    p.poster_near(8.0, 4.5, 2.2, Vec2::new(1.2, 1.5), Tex::Synergy);
    p.poster_near(-8.0, 3.0, 2.3, Vec2::new(1.3, 1.5), Tex::PosterPie);
    p.poster_near(8.0, -4.5, 2.2, Vec2::new(1.2, 1.5), Tex::Wellness);
    p.prop(&LAMP_FLOOR, v(7.0, 0.0, 6.0), 0.0, true);
}

fn reorg_dress(p: &mut Plan) {
    room_dress(p, Mat::Trim, Mat::Metal, Mat::Cold);
    beams(p, -12.0, 12.0, -10.0, 10.0, 6.0, Mat::Metal);
    for x in [-1.0, 3.0, 8.0] {
        p.window_near(x, 10.0, Vec2::new(3.0, 1.6), Tex::Skyline, Mat::Trim);
    }
    // Filing cabinets and a plant at every corner: the only things the reorg left in place.
    for (x, z) in [(-13.8, -9.0), (-13.8, 9.0), (13.8, -9.0), (13.8, 9.0)] {
        plant_pair(p, x * 0.93, z * 0.93);
    }
    for z in [-8.0, 8.0] {
        wall_prop(p, &BOOKCASE_CLOSED, -14.0, z, true);
    }
    wall_prop(p, &BOOKCASE_OPEN, 14.0, -8.0, true);
    p.prop(&TRASHCAN, v(-12.8, 0.0, 9.0), 0.0, false);
    p.prop(&COAT_RACK, v(-12.6, 0.0, 3.0), 0.0, true);
    for (x, z, tex) in [
        (-8.0, 10.0, Tex::Roadmap),
        (-11.0, -10.0, Tex::Gantt),
        (3.0, -10.0, Tex::Standup),
        (14.0, 6.0, Tex::PosterBars),
    ] {
        p.poster_near(x, z, 2.3, Vec2::new(2.2, 1.3), tex);
    }
    // Moving boxes: half the floor is already somewhere else.
    for (x, z, yaw) in [
        (-13.0, 6.8, 0.4),
        (-12.4, 7.6, 1.1),
        (12.8, -8.2, 0.2),
        (13.2, -7.4, 0.9),
    ] {
        p.prop(&BOX_CLOSED, v(x, 0.0, z), yaw, false);
    }
}

fn migration_dress(p: &mut Plan) {
    room_dress(p, Mat::Dark, Mat::Metal, Mat::Cold);
    // Cable tray along the ceiling and cords down to both terminals.
    wall_band_cable(p);
    for (a, b) in [
        ((-4.0, -5.0), (-7.0, -0.8)),
        ((4.0, -5.0), (7.0, -0.8)),
        ((-4.0, -5.0), (4.0, -5.0)),
    ] {
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        p.slab(
            v((a.0 + b.0) / 2.0, 0.02, (a.1 + b.1) / 2.0),
            (-dz).atan2(dx),
            v((dx * dx + dz * dz).sqrt(), 0.03, 0.07),
            Mat::Cable,
        );
    }
    floor_patch(p, 0.0, -4.6, 0.0, 11.0, 2.4, Mat::Dark);
    server_rack(p, -1.4, -5.2, 0.0);
    server_rack(p, 1.4, -5.2, 0.0);
    // Old side: beige desk, a chair with a worn seat, a retro poster. New side: a plant and a TV.
    p.prop(&CHAIR_DESK, v(-5.6, 0.0, 1.8), -FRAC_PI_2, false);
    p.poster_near(-9.0, 3.5, 2.2, Vec2::new(1.2, 1.5), Tex::TurnItOff);
    p.poster_near(-9.0, -3.5, 2.0, Vec2::new(1.2, 1.5), Tex::Cobweb);
    p.poster_near(9.0, -3.5, 2.2, Vec2::new(1.2, 1.5), Tex::NextAgent);
    p.poster_near(9.0, 3.5, 2.2, Vec2::new(1.2, 1.5), Tex::StickyPass);
    plant_pair(p, 8.2, 5.0);
    round_patch(p, 7.0, 0.0, 3.2, Mat::Cold);
    round_patch(p, -7.0, 0.0, 3.2, Mat::Hot);
    glow(p, -6.5, 0.0, Color::srgb(1.0, 0.7, 0.35));
    glow(p, 6.5, 0.0, Color::srgb(0.5, 0.8, 1.0));
}

/// Cable tray high on the walls: a dark band with a bright cable on it.
fn wall_band_cable(p: &mut Plan) {
    super::offsite::wall_band(p, Mat::Dark, H - 1.0, H - 0.8, 0.1);
    super::offsite::wall_band(p, Mat::Cable, H - 0.95, H - 0.9, 0.12);
}

fn hackathon_dress(p: &mut Plan) {
    room_dress(p, Mat::Dark, Mat::Hot, Mat::Hot);
    beams(p, -10.0, 10.0, -10.0, 10.0, 5.0, Mat::Metal);
    // Stage truss, speaker stacks and footlights.
    p.boxed(v(0.0, 4.0, -7.0), v(10.0, 0.18, 0.18), Mat::Metal, false);
    for x in [-4.0, -1.3, 1.3, 4.0] {
        p.boxed(v(x, 3.85, -7.0), v(0.4, 0.22, 0.4), Mat::LightPanel, false);
    }
    for s in [-1.0, 1.0] {
        p.boxed(v(s * 5.6, 1.1, -8.4), v(0.9, 2.2, 0.9), Mat::Dark, false);
        p.boxed(v(s * 5.6, 0.9, -7.93), v(0.7, 0.8, 0.04), Mat::Metal, false);
        p.boxed(v(s * 5.0, 0.4, -6.4), v(0.8, 0.8, 0.5), Mat::Dark, false);
    }
    for k in 0..8 {
        p.prim(
            PrimKind::Sphere,
            v(-3.5 + k as f32, 0.3, -6.65),
            v(0.14, 0.1, 0.14),
            Mat::NeonYellow,
        );
    }
    // Energy-drink pyramid by the south fridges, sleeping bags, cables, beanbags.
    for row in 0..3 {
        for k in 0..(4 - row) {
            p.prim(
                PrimKind::Cylinder,
                v(
                    -8.0 + k as f32 * 0.24 + row as f32 * 0.12,
                    0.1 + row as f32 * 0.2,
                    9.4,
                ),
                v(0.18, 0.2, 0.18),
                [Mat::NeonYellow, Mat::LedGreen, Mat::NeonPink][(k + row) % 3],
            );
        }
    }
    for (x, z, yaw) in [(-9.0, 1.0, 0.2), (9.0, 1.0, -0.2)] {
        p.slab(v(x, 0.08, z), yaw, v(0.9, 0.16, 2.0), Mat::Fabric1);
    }
    for (x, z) in [(-1.5, 1.5), (1.5, 1.5)] {
        floor_patch(p, x, z + 1.5, 0.0, 2.8, 0.1, Mat::Cable);
    }
    beanbag_pair(p);
    p.poster_near(-12.0, -6.0, 2.4, Vec2::new(1.8, 1.2), Tex::Leaderboard);
    p.poster_near(12.0, -6.0, 2.4, Vec2::new(1.8, 1.2), Tex::DemoScreen);
    p.poster_near(6.0, 10.0, 2.2, Vec2::new(1.6, 1.1), Tex::PizzaBox);
    balloons(
        p,
        11.0,
        -7.5,
        [Mat::NeonYellow, Mat::NeonPink, Mat::NeonCyan],
    );
    glow(p, 0.0, -8.0, Color::srgb(1.0, 0.6, 0.3));
}

fn beanbag_pair(p: &mut Plan) {
    super::props::beanbag(p, -10.6, -0.4, Mat::BeanA);
    super::props::beanbag(p, 10.6, -0.4, Mat::BeanB);
}

fn openspace_dress(p: &mut Plan) {
    room_dress(p, Mat::Trim, Mat::White, Mat::Cold);
    beams(p, -12.0, 12.0, -11.0, 11.0, 6.0, Mat::Trim);
    floor_patch(p, 0.0, 0.0, 0.0, 5.4, 4.2, Mat::Fabric1);
    for x in [-9.0, -4.0, 5.0] {
        p.window_near(x, 11.0, Vec2::new(3.0, 1.6), Tex::Skyline, Mat::Trim);
    }
    wall_prop(p, &BOOKCASE_CLOSED_WIDE, -14.0, 7.0, true);
    wall_prop(p, &BOOKCASE_OPEN, 14.0, 7.0, true);
    p.prop(
        &COFFEE_MACHINE,
        v(13.6, SIDE_TABLE.size[1] * FURNITURE_SCALE, -6.0),
        -FRAC_PI_2,
        false,
    );
    p.prop(&SIDE_TABLE, v(13.6, 0.0, -6.0), -FRAC_PI_2, false);
    plant_pair(p, 12.5, 10.0);
    plant_pair(p, -12.5, -10.0);
    for (x, z, tex) in [
        (-6.0, -11.0, Tex::Standup),
        (6.0, -11.0, Tex::Kpi),
        (-14.0, 8.0, Tex::Wellness),
        (14.0, 2.0, Tex::Roadmap),
    ] {
        p.poster_near(x, z, 2.3, Vec2::new(1.5, 1.3), tex);
    }
    p.sign_near(0.0, 11.0, 3.2, 3.0, Sign::ScrumBoard);
    bunting(
        p,
        (-6.0, -4.0),
        (6.0, -4.0),
        p.top() - 0.6,
        [Mat::Fabric0, Mat::Fabric1, Mat::Fabric2],
    );
}

/// Meeting room 4B: a table, two chairs, the rating scale on the screen, a wall clock. Roomy
/// enough for the camera to sit behind you and still show the manager over your shoulder.
fn review() -> FloorLayout {
    let mut p = Plan::default();
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 8.0, 7.0, 0.6, 0.0), Mat::Wall);
    p.route = vec![pt(0.0, 4.0), pt(0.0, -4.0)];
    p.build_walls();
    review_dress(&mut p);
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
    reorg_dress(&mut p);
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
    migration_dress(&mut p);
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
    hackathon_dress(&mut p);
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
    openspace_dress(&mut p);
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

/// The partition between the living room and the bedroom runs along this x, open in the middle.
const HOME_WALL_X: f32 = 2.0;
/// Where you work: standing spot, and the desk in front of it against the north wall.
const HOME_DESK: (f32, f32) = (6.5, -3.4);

/// One flat, two rooms: the living room and kitchen west, the bedroom with the work desk east.
/// Far enough from the desk that every walk chore costs real time away from the laptop.
fn home() -> FloorLayout {
    let mut p = Plan::default();
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 9.0, 6.0, 0.5, 0.0), Mat::Wall);
    p.route = vec![pt(-6.0, 0.0), pt(6.0, 0.0)];
    p.build_walls();
    room_dress(&mut p, Mat::Beige, Mat::White, Mat::Candle);
    let h = p.top();
    // The partition, with a doorway between z = -1.1 and 1.1.
    for (z, len) in [(-3.55, 4.9), (3.55, 4.9)] {
        p.boxed(v(HOME_WALL_X, h / 2.0, z), v(0.16, h, len), Mat::Wall, true);
    }
    p.boxed(
        v(HOME_WALL_X, h - 0.25, 0.0),
        v(0.18, 0.5, 2.2),
        Mat::Wall,
        false,
    );
    home_bedroom(&mut p);
    home_living(&mut p);
    home_kitchen(&mut p);
    home_laundry(&mut p);
    lights(&mut p, &[(5.5, 0.0), (-3.0, -2.0), (-5.5, 3.5)]);
    glow(&mut p, HOME_DESK.0, -4.6, Color::srgb(0.5, 0.75, 1.0));
    let (dx, dz) = HOME_DESK;
    anchor_at(&mut p, "desk", dx, dz, (dx, -6.0));
    // The cat's spot on the keyboard: on top of the desk, a reference point nobody walks to.
    let keys = v(dx - 0.25, DESK.size[1] * FURNITURE_SCALE, -4.85);
    p.anchor("cat_desk", keys, yaw_towards(keys, v(dx, 0.0, 0.0)));
    anchor_at(&mut p, "cat_bed", 3.4, 4.6, (6.0, 0.0));
    anchor_at(&mut p, "laundry", -1.6, -3.7, (-1.6, -6.0));
    anchor_at(&mut p, "door", -7.6, -3.0, (-9.0, -3.0));
    anchor_at(&mut p, "player_spawn", dx, dz + 0.4, (dx, -6.0));
    anchor_at(&mut p, "graveyard", 4.6, 0.0, (dx, dz));
    assemble(HOME, 1, "Your Apartment, Third Floor", false, p, {
        let mut t = theme(
            Color::srgb(0.9, 0.86, 0.78),
            [1.7, 1.45, 1.15],
            Color::srgb(0.62, 0.58, 0.52),
        );
        t.ambient = Color::srgb(1.0, 0.93, 0.82);
        t
    })
}

/// The bed, the work desk with a laptop and a ring light, a wardrobe, the cat's bed.
fn home_bedroom(p: &mut Plan) {
    let (dx, _) = HOME_DESK;
    p.prop(&DESK, v(dx, 0.0, -4.85), 0.0, true);
    let top = DESK.size[1] * FURNITURE_SCALE;
    p.prop(&LAPTOP, v(dx + 0.2, top, -4.9), 0.0, false);
    p.prop(&MOUSE, v(dx + 0.65, top, -4.75), 0.0, false);
    p.prop(&LAMP_TABLE, v(dx - 0.7, top, -5.1), 0.0, false);
    p.prop(&CHAIR_DESK, v(dx, 0.0, -4.05), PI, false);
    // A mug that has been "nearly finished" since the 9:00 standup.
    p.prim(
        PrimKind::Cylinder,
        v(dx + 0.85, top + 0.06, -5.0),
        v(0.09, 0.12, 0.09),
        Mat::Red,
    );
    // Bed: frame, mattress, duvet, pillows.
    let (bx, bz) = (6.6, 3.4);
    p.boxed(v(bx, 0.2, bz), v(2.0, 0.4, 2.3), Mat::Wood, true);
    p.boxed(v(bx, 0.48, bz), v(1.9, 0.16, 2.2), Mat::White, false);
    p.boxed(v(bx, 0.6, bz - 0.2), v(1.94, 0.1, 1.6), Mat::Fabric0, false);
    for x in [bx - 0.45, bx + 0.45] {
        p.boxed(v(x, 0.64, bz + 0.85), v(0.7, 0.14, 0.4), Mat::White, false);
    }
    p.boxed(v(bx, 0.7, bz + 1.17), v(2.0, 1.0, 0.08), Mat::Wood, false);
    p.boxed(v(8.5, 1.1, -0.5), v(0.6, 2.2, 1.8), Mat::Wood, true);
    // The cat's bed, which the cat has never used.
    p.prim(
        PrimKind::Cylinder,
        v(3.4, 0.08, 4.6),
        v(0.8, 0.16, 0.8),
        Mat::Fabric2,
    );
    p.prop(&RUG, v(5.2, 0.0, 0.4), FRAC_PI_2, false);
    p.window_near(5.5, 6.0, Vec2::new(2.2, 1.3), Tex::Skyline, Mat::Trim);
    p.poster_near(9.0, 2.8, 1.8, Vec2::new(1.0, 0.8), Tex::Family);
}

/// Sofa, TV, a coffee table with yesterday's takeaway.
fn home_living(p: &mut Plan) {
    p.prop(&LOUNGE_SOFA, v(-4.0, 0.0, 0.6), 0.0, true);
    p.prop(&COFFEE_TABLE, v(-4.0, 0.0, -0.9), 0.0, false);
    p.prop(&CABINET_TV, v(-4.0, 0.0, -2.9), PI, true);
    p.prop(
        &TV,
        v(-4.0, CABINET_TV.size[1] * FURNITURE_SCALE, -2.9),
        PI,
        false,
    );
    p.prop(&RUG, v(-4.0, 0.0, -0.8), 0.0, false);
    p.prop(&POTTED_PLANT, v(0.9, 0.0, 5.2), 0.0, true);
    p.prop(&COAT_RACK, v(-8.3, 0.0, -4.8), 0.0, true);
    p.window_near(-4.0, -6.0, Vec2::new(2.4, 1.3), Tex::Skyline, Mat::Trim);
    p.poster_near(-9.0, 1.5, 1.8, Vec2::new(1.1, 0.8), Tex::CorpArt);
    // The front door, a doormat and a parcel nobody ordered.
    p.boxed(v(-8.94, 1.05, -3.0), v(0.08, 2.1, 1.05), Mat::Door, false);
    p.boxed(v(-8.9, 1.0, -2.65), v(0.06, 0.06, 0.12), Mat::Chrome, false);
    p.slab(v(-8.3, 0.015, -3.0), 0.0, v(0.6, 0.02, 1.0), Mat::Felt);
    p.prop(&BOX_CLOSED, v(-8.4, 0.0, -1.6), 0.4, false);
}

/// The kitchen along the south wall: counter, sink, hob, fridge, coffee machine.
fn home_kitchen(p: &mut Plan) {
    let z = 5.55;
    p.boxed(v(-5.3, 0.45, z), v(4.4, 0.9, 0.7), Mat::Wood, true);
    p.boxed(v(-5.3, 0.92, z), v(4.5, 0.05, 0.74), Mat::Marble, false);
    p.boxed(v(-6.4, 0.95, z), v(0.6, 0.02, 0.45), Mat::Chrome, false);
    p.boxed(v(-4.5, 0.95, z), v(0.6, 0.02, 0.5), Mat::Dark, false);
    p.prop(&COFFEE_MACHINE, v(-3.5, 0.94, z), PI, false);
    p.prop(&FRIDGE, v(-8.4, 0.0, z - 0.05), PI, true);
    p.boxed(v(-5.3, 2.0, z + 0.1), v(4.4, 0.7, 0.4), Mat::Wood, false);
    for x in [-6.0, -4.8] {
        p.prop(&STOOL_BAR, v(x, 0.0, 4.6), 0.0, false);
    }
}

/// The washer by the partition and the drying rack waiting for a load.
fn home_laundry(p: &mut Plan) {
    let (x, z) = (-0.4, -5.55);
    p.boxed(v(x, 0.43, z), v(0.65, 0.86, 0.62), Mat::White, true);
    p.prim_yawed(
        PrimKind::Cylinder,
        v(x, 0.5, z + 0.32),
        v(0.42, 0.04, 0.42),
        FRAC_PI_2,
        Mat::Glass,
    );
    p.boxed(
        v(x + 0.18, 0.78, z + 0.32),
        v(0.12, 0.05, 0.02),
        Mat::LedGreen,
        false,
    );
    // Drying rack: two A-frames and a few rails, folded open.
    let (rx, rz) = (-2.5, -5.2);
    for dx in [-0.6, 0.6] {
        p.boxed(v(rx + dx, 0.5, rz), v(0.04, 1.0, 0.6), Mat::Metal, false);
    }
    for (dz, y) in [(-0.25, 0.95), (0.0, 1.0), (0.25, 0.95)] {
        p.boxed(v(rx, y, rz + dz), v(1.2, 0.02, 0.02), Mat::Metal, false);
    }
    p.block(v(rx, 0.5, rz), v(1.3, 1.0, 0.7));
    // The basket and the pile that will not hang itself.
    p.prim(
        PrimKind::Cylinder,
        v(-1.4, 0.2, -5.4),
        v(0.6, 0.4, 0.5),
        Mat::Wood,
    );
    for (dx, dz, m) in [
        (-0.1, 0.0, Mat::Fabric0),
        (0.12, 0.08, Mat::Fabric1),
        (0.0, -0.1, Mat::White),
        (0.1, -0.05, Mat::Fabric2),
    ] {
        p.prim(
            PrimKind::Sphere,
            v(-1.4 + dx, 0.45, -5.4 + dz),
            v(0.32, 0.18, 0.3),
            m,
        );
    }
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
                // The pod is the middle of the desk block and the cat's spot is on a keyboard:
                // reference points nobody stands on.
                if *name == "pod" || *name == "cat_desk" {
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
        for n in ["desk", "laundry", "door", "cat_bed", "cat_desk"] {
            assert!(has(HOME, n), "{n}");
        }
    }

    #[test]
    fn home_chores_take_a_walk_from_the_desk() {
        let l = build_zone(HOME).unwrap_or_else(|| panic!("no home"));
        let g = Grid::from(&l, l.pos("desk"));
        // At 7 m/s a round trip plus the dwell must cost more than the status holds unaided.
        for n in ["laundry", "door"] {
            let d = g.at_anchor(&l, l.pos(n));
            assert!(d > 8.0, "{n} only {d} m from the desk");
        }
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
