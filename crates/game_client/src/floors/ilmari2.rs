//! Ilmari Wing 2, Human Resources (docs/ILMARI_09.md B.5): the Tower's HR floor after the
//! occupation, as three rooms on the plan of Wing 1 (`ilmari1::pods`) so packs and route stay
//! testable. ilmari2/1 is the Open Plan (the HR Business Partner: hot desks, culture posters,
//! beanbags nobody has used and a wellbeing wall, then a glass suite with a wall of identical
//! desks), ilmari2/2 the Culture Hub (the Committee: a pizza table, values banners, a dais with
//! three chairs, vote boards and three ballot terminals far enough apart to be a run) and
//! ilmari2/3 the Review Stage (Performance Review: rows of audience chairs, a stage with a nine-box
//! grid on the floor and the wall, grade boards and two HR Portals). The satire is in the copy:
//! every wall is warm, every sentence is a policy.

use super::datacenter::dress::*;
use super::ilmari1::{CELLS, arrival, lamps, pods};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::models::{self, BOX_LARGE, CAKE, COOKIE, CUP_COFFEE, DONUT, MUG, PIZZA, PIZZA_BOX};
use super::offsite::{cove, wall_dress};
use super::props::{
    beanbag, cubicle, doc_shelf, gong, long_table, mini_fridge, printer, sad_plant, trophy_shelf,
    water_cooler,
};
use super::themes::{pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => (
            "ilmari2/1",
            "Human Resources: The Open Plan",
            open_plan(&mut p),
        ),
        2 => (
            "ilmari2/2",
            "Human Resources: The Culture Hub",
            culture_hub(&mut p),
        ),
        _ => (
            "ilmari2/3",
            "Human Resources: The Review Stage",
            review_stage(&mut p),
        ),
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, fog, ambient, floor, accent) = match level {
        // Open plan: carpet-tile teal under warm "wellness" light.
        1 => (
            c(0.62, 0.7, 0.72),
            c(0.2, 0.2, 0.26),
            c(1.0, 0.86, 0.8),
            [1.15, 1.3, 1.38],
            LinearRgba::rgb(2.4, 1.1, 0.6),
        ),
        // Culture hub: boardroom plum, pizza-box orange.
        2 => (
            c(0.6, 0.52, 0.62),
            c(0.2, 0.14, 0.22),
            c(1.0, 0.8, 0.82),
            [0.7, 0.6, 0.72],
            LinearRgba::rgb(2.8, 0.8, 1.6),
        ),
        // Review stage: a theatre going cold, spotlights on a grid.
        _ => (
            c(0.5, 0.52, 0.62),
            c(0.12, 0.14, 0.24),
            c(0.85, 0.85, 1.0),
            [0.55, 0.58, 0.72],
            LinearRgba::rgb(1.2, 1.6, 3.2),
        ),
    };
    Theme {
        wall,
        trim: c(0.12, 0.12, 0.18),
        accent,
        fabric: [c(0.25, 0.45, 0.5), c(0.55, 0.3, 0.4), c(0.3, 0.3, 0.5)],
        floor: FloorKind::Carpet(floor),
        fog,
        ambient,
        ambient_brightness: 1000.0,
        sun: c(1.0, 0.8, 0.7),
        sun_illuminance: 2600.0,
        fog_range: (26.0, 90.0),
    }
}

fn casing(p: &mut Plan) {
    pods(p);
    p.build_walls();
    wall_dress(p, Mat::Beige, Mat::Trim, Mat::Dark);
    cove(p, Mat::NeonCyan);
}

fn warm_lamps(p: &mut Plan, list: &[(f32, f32)], intensity: f32) {
    lamps(p, list, intensity);
}

/// The "culture" on a wall: a poster and the plant that was bought to go under it.
fn poster_plant(p: &mut Plan, x: f32, z: f32, sign: Sign) {
    p.sign_near(x, z, 2.4, 2.6, sign);
    let face = if z < 0.0 { 1.0 } else { -1.0 };
    sad_plant(p, x + 2.0, z + face * 1.0);
}

/// A hot desk: booked, numbered, and with a mug on the ones that have a person behind them.
fn hot_desk(p: &mut Plan, x: f32, z: f32, yaw: f32, mug: bool) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
        p.prop(&SCREEN, v(0.0, 0.76, -0.1), 0.0, false);
        p.prop(&KEYBOARD, v(0.0, 0.76, 0.2), 0.0, false);
        p.prop(&CHAIR_DESK, v(0.0, 0.0, 0.95), PI, false);
        if mug {
            models::put(p, &MUG, v(-0.6, 0.76, 0.15), 0.0, 0.35, false);
        }
        // A sticky note with the booking: it is the only thing on the desk that is current.
        p.boxed(v(0.5, 0.78, 0.05), v(0.1, 0.01, 0.1), Mat::Yellow, false);
    });
}

fn arrival_kit(p: &mut Plan) {
    arrival(p);
    // A sign-in tablet that has crashed on "Please sign in".
    p.boxed(v(-46.2, 0.55, 5.6), v(0.1, 1.1, 0.1), Mat::Metal, false);
    p.boxed(
        v(-46.2, 1.15, 5.6),
        v(0.12, 0.4, 0.6),
        Mat::ScreenGlow,
        false,
    );
}

// ------------------------------------------------------------------ ilmari2/1: the open plan

fn glass_wall(p: &mut Plan, x: f32, z: f32, len_x: f32, len_z: f32) {
    p.boxed(v(x, 1.2, z), v(len_x, 2.4, len_z), Mat::Glass, false);
    p.boxed(
        v(x, 1.0, z),
        v(len_x + 0.02, 0.5, len_z + 0.02),
        Mat::Frosted,
        false,
    );
}

fn open_plan(p: &mut Plan) -> Theme {
    casing(p);
    // The hall: hot desks in rows, the mug on every second one (the rest are copies).
    for (k, x) in [-28.0, -21.0, -14.0, -7.0, 0.0, 7.0, 14.0]
        .into_iter()
        .enumerate()
    {
        hot_desk(p, x, -5.4, PI, k % 2 == 0);
        hot_desk(p, x, 5.4, 0.0, k % 2 == 1);
    }
    // Culture on the long walls: values nobody chose, over plants nobody waters.
    poster_plant(p, -26.0, -10.9, Sign::IlCulture1);
    poster_plant(p, -8.0, -10.9, Sign::IlCulture2);
    poster_plant(p, 8.0, -10.9, Sign::IlCulture3);
    p.sign_near(-17.0, 10.9, 2.7, 3.2, Sign::IlOkr);
    p.sign_near(2.0, 10.9, 2.7, 3.2, Sign::IlOpenPlan);
    p.sign_near(-34.9, 5.5, 2.6, 2.8, Sign::IlReorg);
    // The wellbeing wall: a big board, a rack of leaflets and the stress ball bowl.
    p.sign_near(-33.9, 0.0, 2.4, 4.4, Sign::IlWellbeing);
    p.boxed(v(-33.2, 0.5, -2.4), v(0.6, 1.0, 1.4), Mat::Wood, true);
    p.boxed(
        v(-33.1, 1.05, -2.4),
        v(0.4, 0.12, 1.2),
        Mat::NeonPink,
        false,
    );
    models::put(p, &CUP_COFFEE, v(-33.1, 1.0, 2.3), 0.0, 0.45, false);
    models::put(p, &DONUT, v(-33.1, 1.0, 2.7), 0.0, 0.45, false);
    // Beanbags nobody has used, in a nook with their own sign.
    for (x, z, m) in [
        (-30.0, 8.2, Mat::BeanA),
        (-27.8, 9.0, Mat::BeanB),
        (-29.2, 6.6, Mat::BeanC),
    ] {
        beanbag(p, x, z, m);
    }
    p.sign_near(-26.0, 10.9, 2.1, 2.0, Sign::IlBeanbag);
    p.prop(&RUG_ROUND, v(-29.0, 0.0, 7.8), 0.0, false);
    // A ceiling of banners: the open plan has no ceiling, only slogans.
    for x in [-24.0, -8.0, 8.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 22.0), Mat::Accent, false);
    }
    cable_tray(p, pt(-32.0, 0.0), pt(16.0, 0.0), false);
    arrival_kit(p);
    p.sign_near(-49.9, 4.5, 2.4, 2.2, Sign::IlW2Back);
    // Cells: the yoga room that is booked for eleven months, the copier cell and the mentors' corner.
    for k in 0..4 {
        p.boxed(
            v(-26.0 + k as f32 * 2.2, 0.03, -30.0),
            v(1.7, 0.05, 0.7),
            Mat::Yoga,
            false,
        );
    }
    gong(p, -22.0, -33.0);
    mini_fridge(p, -14.8, -32.2, Face::PosZ);
    p.sign_near(-20.0, -32.9, 2.6, 2.4, Sign::IlWellbeing);
    p.sign_near(-26.0, -19.1, 2.7, 2.0, Sign::IlCopies);
    printer(p, 12.0, 32.8, Face::NegZ);
    printer(p, 4.0, 32.8, Face::NegZ);
    for k in 0..3 {
        models::put(
            p,
            &BOX_LARGE,
            v(10.0 + k as f32 * 2.6, 0.0, 31.4),
            0.3 * k as f32,
            1.3,
            true,
        );
    }
    water_cooler(p, 0.5, 32.8);
    doc_shelf(p, 4.0, -32.4, Face::PosZ);
    trophy_shelf(p, 14.0, -32.4, Face::PosZ);
    p.sign_near(8.0, -33.0, 3.4, 2.6, Sign::IlCulture3);
    // Boss pod: the Partnership Suite. Glass partitions down both sides, a mirror wall behind
    // the boss and five identical desks in front of it, a floor ring in cold blue.
    for z in [-10.4, 10.4] {
        glass_wall(p, 42.0, z, 22.0, 0.1);
    }
    p.boxed(v(54.7, 1.7, 0.0), v(0.1, 3.4, 22.0), Mat::Chrome, false);
    for z in [-8.0, -4.0, 0.0, 4.0, 8.0] {
        p.framed(v(52.0, 0.0, z), -FRAC_PI_2, |p| {
            p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
            p.prop(&SCREEN, v(0.0, 0.76, -0.1), 0.0, false);
            p.prop(&CHAIR_DESK, v(0.0, 0.0, 0.95), PI, false);
        });
    }
    floor_ring(p, 42.0, 0.0, 6.4, 22, Mat::BlinkBlue, 0.1);
    floor_ring(p, 42.0, 0.0, 7.0, 24, Mat::NeonCyan, 0.08);
    p.sign(v(54.5, 3.3, -6.0), -FRAC_PI_2, 3.4, Sign::IlCopies);
    p.sign(v(54.5, 3.3, 6.0), -FRAC_PI_2, 3.4, Sign::IlOpenPlan);
    p.sign_near(29.1, 6.0, 3.0, 2.8, Sign::IlCulture1);
    p.sign_near(29.1, -6.0, 3.0, 2.8, Sign::IlOkr);
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(31.0, z), pt(50.0, z), false);
    }
    sad_plant(p, 31.0, -9.2);
    sad_plant(p, 31.0, 9.2);
    strips(p, Mat::LightPanel, Mat::NeonCyan, 0.0, 0.1);
    warm_lamps(
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
            ("door:campus_occupied", Sign::IlW2Back),
            ("door:ilmari2/2", Sign::IlW2ToCommittee),
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

// ------------------------------------------------------------------ ilmari2/2: the culture hub

/// A ballot terminal: a pedestal with a glowing screen, its letter above and the prop anchor
/// 1.8 m in front. `face` is +1 for a terminal on the north wall (stand to its south).
pub(super) fn ballot(p: &mut Plan, key: &'static str, x: f32, z: f32, face: f32, sign: Sign) {
    p.boxed(v(x, 0.55, z), v(0.8, 1.1, 0.6), Mat::Dark, true);
    p.boxed(
        v(x, 1.2, z + face * 0.1),
        v(0.7, 0.45, 0.06),
        Mat::ScreenGlow,
        false,
    );
    p.boxed(v(x, 1.12, z), v(0.86, 0.06, 0.66), Mat::Metal, false);
    p.sign(v(x, 2.0, z), if face > 0.0 { 0.0 } else { PI }, 1.1, sign);
    p.interactable(key, v(x, 0.9, z), v(x, 0.0, z + face * 1.8));
}

/// A tall executive chair for a committee member: they are androids, they never sit in it.
fn dais_chair(p: &mut Plan, x: f32, z: f32) {
    p.prop(&LOUNGE_DESIGN_CHAIR, v(x, 0.0, z), FRAC_PI_2, true);
    p.boxed(v(x + 0.55, 1.5, z), v(0.1, 1.8, 0.9), Mat::Fabric1, false);
}

fn culture_hub(p: &mut Plan) -> Theme {
    casing(p);
    // The town hall: rows of chairs facing a wall of values, a pizza table, a suggestion box.
    for row in 0..3 {
        for k in 0..6 {
            let (x, z) = (-30.0 + k as f32 * 2.0, -5.8 + row as f32 * 5.8);
            p.prop(&CHAIR_ROUNDED, v(x, 0.0, z), -FRAC_PI_2, false);
        }
    }
    for x in [-24.0, -8.0, 8.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 22.0), Mat::NeonPink, false);
    }
    p.sign_near(-33.9, 0.0, 2.8, 4.0, Sign::IlCulture2);
    p.sign_near(-17.0, -10.9, 2.7, 3.2, Sign::IlCulture1);
    p.sign_near(-3.0, -10.9, 2.7, 3.2, Sign::IlCulture3);
    p.sign_near(11.0, -10.9, 2.7, 3.2, Sign::IlOkr);
    p.sign_near(-17.0, 10.9, 2.7, 3.2, Sign::IlWellbeing);
    p.sign_near(-3.0, 10.9, 2.7, 3.2, Sign::IlQuorum);
    p.sign_near(11.0, 10.9, 2.7, 3.2, Sign::IlReorg);
    // Pizza Friday, laid out: boxes, a cake with a number on it, cookies in a bowl, one slice taken.
    p.prop(&TABLE, v(-2.0, 0.0, 8.4), 0.0, true);
    p.prop(&TABLE, v(0.0, 0.0, 8.4), 0.0, true);
    for k in 0..4 {
        models::put(
            p,
            &PIZZA_BOX,
            v(-2.6 + k as f32 * 1.0, 0.76, 8.4),
            0.2 * k as f32,
            0.9,
            false,
        );
    }
    models::put(p, &PIZZA, v(-1.6, 0.78, 8.1), 0.0, 0.9, false);
    models::put(p, &CAKE, v(1.0, 0.76, 8.5), 0.0, 0.9, false);
    models::put(p, &COOKIE, v(0.4, 0.76, 8.0), 0.0, 0.5, false);
    p.sign_near(0.0, 10.9, 3.5, 2.0, Sign::IlCulture2);
    // Where the opinions go: a box with a slot and a lock, bolted to the floor.
    p.boxed(v(-34.0 + 0.6, 0.6, -8.6), v(0.8, 1.2, 0.8), Mat::Red, true);
    p.boxed(v(-33.4, 1.1, -8.6), v(0.4, 0.04, 0.04), Mat::Dark, false);
    arrival_kit(p);
    p.sign_near(-49.9, 4.5, 2.4, 2.2, Sign::IlW2Back);
    // Cells: the break room nobody takes breaks in, the values archive and a quiet room with a gong.
    p.prop(&LOUNGE_SOFA, v(-20.0, 0.0, -31.8), 0.0, true);
    p.prop(&COFFEE_TABLE, v(-20.0, 0.0, -29.3), 0.0, true);
    models::put(p, &MUG, v(-20.0, 0.5, -29.3), 0.0, 0.4, false);
    mini_fridge(p, -14.4, -32.2, Face::PosZ);
    doc_shelf(p, -26.0, -32.4, Face::PosZ);
    p.sign_near(-20.0, -33.0, 3.2, 2.4, Sign::IlWellbeing);
    long_table(p, 6.0, 30.0, 3, true);
    doc_shelf(p, 6.0, 32.6, Face::NegZ);
    p.sign_near(0.0, 33.0, 3.2, 2.4, Sign::IlOkr);
    gong(p, 10.0, -33.0);
    trophy_shelf(p, 14.0, -32.4, Face::PosZ);
    p.sign_near(10.0, -19.1, 2.8, 2.2, Sign::IlQuorum);
    // Boss pod: the committee's dais at the back (north half; the exit door is in the south one),
    // three chairs for three members who vote as one, vote boards on the walls and over the dais,
    // and the terminals spread round the pod so that swinging a vote is a run.
    p.boxed(
        v(52.6, 0.3, -5.0),
        v(3.0, 0.6, 11.0),
        Mat::MarbleDark,
        false,
    );
    p.boxed(v(51.6, 1.0, -5.0), v(0.8, 0.8, 10.0), Mat::Wood, false);
    for z in [-8.0, -5.0, -2.0] {
        dais_chair(p, 53.4, z);
        p.boxed(v(51.2, 1.42, z), v(0.04, 0.2, 0.9), Mat::Gold, false);
    }
    p.sign(v(54.8, 3.4, -5.0), -FRAC_PI_2, 4.0, Sign::IlQuorum);
    p.sign_near(42.0, -11.9, 3.0, 4.4, Sign::IlVoteBoard);
    p.sign_near(42.0, 11.9, 3.0, 4.4, Sign::IlVoteBoard);
    p.sign_near(29.1, 0.0, 3.0, 4.4, Sign::IlVoteBoard);
    ballot(p, "prop:il_ballot_a", 32.5, -11.2, 1.0, Sign::IlBallotA);
    ballot(p, "prop:il_ballot_b", 51.5, -11.2, 1.0, Sign::IlBallotB);
    ballot(p, "prop:il_ballot_c", 42.0, 11.2, -1.0, Sign::IlBallotC);
    floor_ring(p, 42.0, 0.0, 6.4, 22, Mat::NeonPink, 0.1);
    floor_ring(p, 42.0, 0.0, 7.0, 24, Mat::NeonYellow, 0.08);
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(31.0, z), pt(50.0, z), false);
    }
    strips(p, Mat::LightPanel, Mat::NeonPink, 0.0, 0.1);
    warm_lamps(
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
            ("door:ilmari2/1", Sign::IlW2Back),
            ("door:ilmari2/3", Sign::IlW2ToReview),
        ],
    );
    theme(2)
}

// ------------------------------------------------------------------ ilmari2/3: the review stage

/// An HR Portal: a kiosk with a green button, its sign above and the prop anchor 1.8 m in front.
pub(super) fn portal(p: &mut Plan, key: &'static str, x: f32, z: f32, face: f32) {
    p.boxed(v(x, 0.7, z), v(0.9, 1.4, 0.6), Mat::White, true);
    p.boxed(
        v(x, 1.3, z + face * 0.31),
        v(0.7, 0.4, 0.03),
        Mat::ScreenGlow,
        false,
    );
    p.boxed(
        v(x, 0.9, z + face * 0.33),
        v(0.2, 0.12, 0.05),
        Mat::LedGreen,
        false,
    );
    p.sign(
        v(x, 2.0, z),
        if face > 0.0 { 0.0 } else { PI },
        1.5,
        Sign::IlPortal,
    );
    p.interactable(key, v(x, 0.9, z), v(x, 0.0, z + face * 1.8));
}

/// A floating rating card on a pole: the stage is full of them, every one a three.
fn card_pole(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.boxed(v(x, 1.1, z), v(0.05, 2.2, 0.05), Mat::Metal, false);
    p.sign(v(x, 2.5, z), yaw, 0.9, Sign::IlNineBox);
}

fn review_stage(p: &mut Plan) -> Theme {
    casing(p);
    // The hall is an auditorium that has been told to stay: rows of chairs facing the pod,
    // an aisle down the middle, feedback boxes at the end of each row.
    for row in 0..4 {
        for side in [-1.0_f32, 1.0] {
            let (x, z) = (-28.0 + row as f32 * 2.6, side * 7.4);
            p.boxed(v(x, 0.3, z), v(0.7, 0.6, 5.6), Mat::Fabric1, false);
            p.boxed(v(x - 0.4, 0.8, z), v(0.12, 0.8, 5.6), Mat::Fabric1, false);
            for k in 0..3 {
                p.prop(
                    &CHAIR_ROUNDED,
                    v(x, 0.0, z + (k as f32 - 1.0) * 1.9),
                    FRAC_PI_2,
                    false,
                );
            }
        }
    }
    for z in [-10.4, 10.4] {
        p.boxed(v(-30.0, 0.5, z), v(0.5, 1.0, 0.5), Mat::Red, true);
    }
    p.sign_near(-33.9, 0.0, 2.8, 4.0, Sign::IlGradeBoard);
    p.sign_near(-17.0, -10.9, 2.8, 3.2, Sign::IlGradeBoard);
    p.sign_near(-17.0, 10.9, 2.8, 3.2, Sign::IlGradeBoard);
    p.sign_near(-4.0, -10.9, 2.7, 3.2, Sign::IlCulture3);
    p.sign_near(-4.0, 10.9, 2.7, 3.2, Sign::IlOkr);
    p.sign_near(9.0, -10.9, 2.7, 3.2, Sign::IlReorg);
    p.sign_near(9.0, 10.9, 2.7, 3.2, Sign::IlCulture1);
    // Cards on poles in the aisle, a spotlight rail over it.
    for x in [-26.0, -18.0, -10.0, -2.0, 6.0, 14.0] {
        card_pole(p, x, 0.0, PI);
    }
    p.boxed(v(-8.0, 3.9, 0.0), v(46.0, 0.12, 0.3), Mat::Dark, false);
    arrival_kit(p);
    p.sign_near(-49.9, 4.5, 2.4, 2.2, Sign::IlW2Back);
    // Cells: the calibration room (a flip chart and a very long table), the HR bot charging bay
    // and the room where the feedback is filed.
    long_table(p, -20.0, -30.0, 2, false);
    p.sign_near(-20.0, -33.0, 3.0, 2.6, Sign::IlGradeBoard);
    for k in 0..2 {
        cubicle(p, k, 3.0 + k as f32 * 3.4, 31.0, PI);
    }
    p.sign_near(6.0, 33.0, 3.2, 2.4, Sign::IlCopies);
    doc_shelf(p, 10.0, -32.4, Face::PosZ);
    trophy_shelf(p, 16.0, -32.4, Face::PosZ);
    p.sign_near(13.0, -19.1, 2.8, 2.2, Sign::IlNineBox);
    // Boss pod: the stage. A raised floor in light grey, a nine-box laid out in the floor with
    // the boss in the middle cell, and the same grid ten times too big on the back wall.
    p.boxed(v(42.0, 0.04, 0.0), v(25.0, 0.06, 22.0), Mat::Marble, false);
    let cell = 4.2;
    for k in -1..=2 {
        let o = (k as f32 - 0.5) * cell;
        floor_line(
            p,
            pt(42.0 - 1.5 * cell, o),
            pt(42.0 + 1.5 * cell, o),
            0.12,
            Mat::White,
        );
        floor_line(
            p,
            pt(42.0 + o, -1.5 * cell),
            pt(42.0 + o, 1.5 * cell),
            0.12,
            Mat::White,
        );
    }
    floor_ring(p, 42.0, 0.0, 9.4, 28, Mat::NeonCyan, 0.08);
    p.sign(v(54.8, 2.6, 0.0), -FRAC_PI_2, 4.2, Sign::IlNineBox);
    p.sign(v(54.8, 2.4, -7.4), -FRAC_PI_2, 3.0, Sign::IlGradeBoard);
    p.sign(v(54.8, 2.4, 7.4), -FRAC_PI_2, 3.0, Sign::IlGradeBoard);
    p.sign_near(42.0, -11.9, 3.0, 4.4, Sign::IlGradeBoard);
    p.sign_near(42.0, 11.9, 3.0, 4.4, Sign::IlGradeBoard);
    p.sign_near(29.1, 0.0, 3.0, 3.6, Sign::IlReorg);
    portal(p, "prop:il_hr_portal_1", 31.5, -11.2, 1.0);
    portal(p, "prop:il_hr_portal_2", 52.5, 11.2, -1.0);
    for (x, z) in [(32.0, -8.8), (32.0, 8.8), (52.0, -8.8), (52.0, 3.0)] {
        card_pole(p, x, z, if x < 40.0 { FRAC_PI_2 } else { -FRAC_PI_2 });
    }
    p.boxed(v(42.0, 4.0, 0.0), v(0.3, 0.12, 22.0), Mat::Dark, false);
    p.boxed(v(40.0, 4.0, 0.0), v(0.3, 0.12, 22.0), Mat::Dark, false);
    for z in [-3.6, 3.6] {
        cable_tray(p, pt(31.0, z), pt(50.0, z), false);
    }
    p.boxed(v(33.0, 0.7, -10.2), v(1.5, 1.4, 0.7), Mat::Wood, true);
    strips(p, Mat::LightPanel, Mat::NeonCyan, 0.0, 0.1);
    warm_lamps(
        p,
        &[
            (-44.0, 0.0),
            (-26.0, 0.0),
            (-12.0, 0.0),
            (2.0, 0.0),
            (-20.0, -26.0),
            (6.0, 26.0),
            (36.0, 0.0),
            (48.0, -6.0),
            (48.0, 6.0),
        ],
        340_000.0,
    );
    cast(
        p,
        [
            ("door:ilmari2/2", Sign::IlW2Back),
            ("door:campus_occupied", Sign::IlW2Out),
        ],
    );
    theme(3)
}
