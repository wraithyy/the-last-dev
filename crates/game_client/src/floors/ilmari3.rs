//! Ilmari Wing 3, Training Data (docs/ILMARI_10.md C.4): the Innovation Lab's data warehouse
//! after the occupation, as three rooms on the plan of Wings 1-2 (corridor, hall, corridor, pit
//! and three cells, one per pack) with a bigger pit for each boss. ilmari3/1 is the Scrape Pit (the
//! Scraper Queen: scraped content in the shelving, a labelling sweatshop in the hall, a robots.txt
//! sign everybody ignores, five drafts on desks in the pit and three proxy alcoves), ilmari3/2 the
//! Validation Wing (the Hallucination: a floor with impossible-geometry hints, a 3 x 3 grid of
//! plausible tiles under the boss and two Ground Truth boards) and ilmari3/3 the Fine-Tuning
//! Chamber (Model Collapse: photocopiers, a hall of mirrors that copy each other, a wall of
//! "generations" fading from person to beige). No point lights (the 0.9 rule): light is emissive
//! strips and neon, so the rooms cost what a Wing 2 room costs.

use super::datacenter::dress::*;
use super::ilmari1::{CELLS, HALL, arrival};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::models::{
    self, BARRIER, BOX_LARGE, BOX_OPEN, BOX_SMALL, CONVEYOR_LONG, HOPPER_SQUARE, MACHINE_WINDOW,
    ROBOT_ARM_A, SIGN_WARNING,
};
use super::offsite::{cove, wall_dress};
use super::props::{doc_shelf, printer, sad_plant, server_rack, water_cooler};
use super::shape::{rounded_rect, stroke};
use super::themes::{pt, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// The boss pods, `(centre x, half x, half z)`: bigger than Wings 1-2, because the Queen's swarm,
/// the Hallucination's 3 x 3 grid of 8 m tiles and the echoes all need floor to move on.
const POD1: (f32, f32, f32) = (46.0, 24.0, 21.0);
const POD2: (f32, f32, f32) = (46.0, 20.0, 18.0);
const POD3: (f32, f32, f32) = (46.0, 20.0, 18.0);
const ARRIVAL_X: f32 = -44.0;

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => (
            "ilmari3/1",
            "Training Data: The Scrape Pit",
            scrape_pit(&mut p),
        ),
        2 => (
            "ilmari3/2",
            "Training Data: The Validation Wing",
            validation_wing(&mut p),
        ),
        _ => (
            "ilmari3/3",
            "Training Data: The Fine-Tuning Chamber",
            tuning_chamber(&mut p),
        ),
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, fog, ambient, floor, accent) = match level {
        // Scrape pit: lab-white warehouse, scraper green on every edge.
        1 => (
            c(0.62, 0.7, 0.68),
            c(0.12, 0.2, 0.18),
            c(0.85, 1.0, 0.92),
            [0.78, 0.9, 0.86],
            LinearRgba::rgb(0.6, 2.8, 1.6),
        ),
        // Validation wing: pale, confident, slightly too bright to trust.
        2 => (
            c(0.74, 0.72, 0.8),
            c(0.2, 0.18, 0.28),
            c(0.95, 0.92, 1.0),
            [0.78, 0.76, 0.84],
            LinearRgba::rgb(1.2, 1.8, 3.2),
        ),
        // Fine-tuning chamber: beige on beige, a copy of a copy of a lab.
        _ => (
            c(0.72, 0.66, 0.56),
            c(0.24, 0.2, 0.15),
            c(1.0, 0.92, 0.8),
            [0.74, 0.68, 0.58],
            LinearRgba::rgb(2.4, 1.6, 0.8),
        ),
    };
    Theme {
        wall,
        trim: c(0.1, 0.12, 0.12),
        accent,
        fabric: [c(0.3, 0.5, 0.45), c(0.45, 0.4, 0.55), c(0.55, 0.5, 0.4)],
        floor: FloorKind::Tile(floor),
        fog,
        ambient,
        ambient_brightness: 1000.0,
        sun: c(0.9, 1.0, 0.95),
        sun_illuminance: 2600.0,
        fog_range: (26.0, 95.0),
    }
}

/// The plan of Wings 1-2 with a pod of this room's size.
pub(super) fn pods3(p: &mut Plan, pod: (f32, f32, f32)) {
    let w = Mat::Wall;
    let ((hx, hhx, hhz), (bx, bhx, bhz), a) = (HALL, pod, ARRIVAL_X);
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

pub(super) fn casing(p: &mut Plan, pod: (f32, f32, f32), glow: Mat) {
    pods3(p, pod);
    p.build_walls();
    wall_dress(p, Mat::Beige, Mat::Trim, Mat::Dark);
    cove(p, glow);
}

pub(super) fn cast(p: &mut Plan, pod: (f32, f32, f32), doors: [(&'static str, Sign); 2]) {
    let packs = CELLS.map(|(x, z)| (x, z - z.signum()));
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                (doors[0].0, (-50.0, 0.0), doors[0].1),
                (doors[1].0, (pod.0 + pod.1, pod.2 * 0.4), doors[1].1),
            ],
            packs: &packs,
            boss: (pod.0, 0.0),
            npc: None,
        },
    );
}

// ------------------------------------------------------------------ shared pieces

/// A labeller's station: a desk, a stool and a screen on the question of the hour, with a
/// pile of the day's quota and a timer sticky. Facing `yaw` (the chair is behind the desk).
fn labeller(p: &mut Plan, x: f32, z: f32, yaw: f32, screen: Mat) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
        p.boxed(v(0.0, 1.0, -0.1), v(0.9, 0.5, 0.06), screen, false);
        p.boxed(v(0.0, 0.8, -0.1), v(0.25, 0.05, 0.2), Mat::Dark, false);
        p.boxed(v(0.7, 0.82, 0.15), v(0.3, 0.1, 0.4), Mat::Paper, false);
        p.boxed(v(-0.7, 0.8, 0.1), v(0.14, 0.04, 0.14), Mat::Yellow, false);
        p.boxed(v(0.0, 0.34, 1.0), v(0.5, 0.06, 0.5), Mat::Fabric2, false);
    });
}

/// A run of identical desks along a wall, all facing `yaw`.
fn labeller_row(p: &mut Plan, x0: f32, x1: f32, z: f32, yaw: f32, n: usize) {
    for k in 0..n {
        let x = x0 + (x1 - x0) * k as f32 / (n.max(2) - 1) as f32;
        let screen = if k % 3 == 1 {
            Mat::LedRed
        } else {
            Mat::ScreenGlow
        };
        labeller(p, x, z, yaw, screen);
    }
}

/// A shelving unit stuffed with what the machine scraped: books, folders and a few boxes.
fn scraped_shelf(p: &mut Plan, x: f32, z: f32, face: Face) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.prop(&BOOKCASE_OPEN, v(-0.9, 0.0, 0.0), 0.0, true);
        p.prop(&BOOKCASE_OPEN, v(0.9, 0.0, 0.0), 0.0, true);
        for (k, y) in [0.45, 0.95, 1.45].into_iter().enumerate() {
            p.boxed(
                v(0.0, y, 0.06),
                v(1.7, 0.2, 0.3),
                [Mat::Paper, Mat::Cardboard, Mat::Beige][k],
                false,
            );
        }
        p.panel(
            v(0.0, 2.35, 0.3),
            0.0,
            Vec2::new(1.8, 0.3),
            Tex::Documentation,
        );
    });
}

/// A stack of cartons with a sheet stuck to the top: somebody's life's work, pending ingestion.
pub(super) fn ingest_stack(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    models::put(p, &BOX_LARGE, v(x, 0.0, z), yaw, 1.2, true);
    models::put(p, &BOX_OPEN, v(x + 0.1, 0.62, z), yaw + 0.4, 1.0, false);
    p.boxed(v(x, 1.3, z), v(0.5, 0.02, 0.4), Mat::Paper, false);
}

/// A caution sign on a barrier: the room's only honest piece of signage.
pub(super) fn caution(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    models::put(p, &BARRIER, v(x, 0.0, z), yaw, 2.4, true);
    models::put(p, &SIGN_WARNING, v(x, 0.0, z + 0.3), yaw, 1.3, false);
}

pub(super) fn arrival_kit(p: &mut Plan, back: Sign) {
    arrival(p);
    // A sign-in tablet that has been "improved" mid-sentence.
    p.boxed(v(-46.2, 0.55, 5.6), v(0.1, 1.1, 0.1), Mat::Metal, false);
    p.boxed(
        v(-46.2, 1.15, 5.6),
        v(0.12, 0.4, 0.6),
        Mat::ScreenGlow,
        false,
    );
    p.sign_near(-49.9, 4.5, 2.4, 2.2, back);
}

// ------------------------------------------------------------------ ilmari3/1: the scrape pit

/// One of the Queen's drafts: a desk with the unpublished page on it, a lamp, a tag and a
/// stand spot 3 m toward the middle (clear of the desk's collider) where Mark Confidential is used.
fn draft_desk(p: &mut Plan, key: &'static str, x: f32, z: f32, centre: (f32, f32)) {
    let (dx, dz) = (centre.0 - x, centre.1 - z);
    let yaw = (-dx).atan2(-dz) + PI;
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
        p.boxed(v(0.0, 0.8, 0.0), v(0.55, 0.02, 0.4), Mat::Paper, false);
        p.boxed(v(0.1, 0.82, 0.05), v(0.4, 0.01, 0.02), Mat::Dark, false);
        p.boxed(v(0.2, 0.83, -0.05), v(0.4, 0.01, 0.02), Mat::Dark, false);
        p.boxed(v(-0.5, 0.78, 0.12), v(0.12, 0.2, 0.12), Mat::Yellow, false);
        p.sign(v(0.0, 1.55, -0.45), PI, 0.9, Sign::IlDraftTag);
        p.boxed(v(0.0, 1.0, -0.43), v(0.04, 1.0, 0.04), Mat::Metal, false);
    });
    let len = (dx * dx + dz * dz).sqrt().max(0.1);
    let stand = v(x + dx / len * 3.0, 0.0, z + dz / len * 3.0);
    p.interactable(key, v(x, 0.8, z), stand);
}

/// An alcove the Queen's proxies jump to: a rack pair against the wall behind a ring of light
/// on the floor where she lands, a cable bundle between. `ring` is where she lands; the racks
/// stand 3 m behind it, away from `centre`, facing it.
fn proxy_alcove(p: &mut Plan, ring: (f32, f32), centre: (f32, f32)) {
    let (dx, dz) = (centre.0 - ring.0, centre.1 - ring.1);
    let len = (dx * dx + dz * dz).sqrt().max(0.1);
    let (fx, fz) = (dx / len, dz / len);
    let yaw = fx.atan2(fz);
    let (bx, bz) = (ring.0 - fx * 3.2, ring.1 - fz * 3.2);
    for side in [-1.4_f32, 1.4] {
        server_rack(p, bx - fz * side, bz + fx * side, yaw);
    }
    floor_ring(p, ring.0, ring.1, 2.0, 14, Mat::NeonCyan, 0.1);
    p.boxed(
        v(ring.0 - fx * 1.6, 0.05, ring.1 - fz * 1.6),
        v(2.4, 0.06, 0.12),
        Mat::Cable,
        false,
    );
}

/// A class console of the pit (a rack, a terminal, a dashboard): a lectern with a lit screen and
/// a stand spot 2.2 m toward the middle. Bonus props: no class is needed to clear the wing.
fn class_console(
    p: &mut Plan,
    key: &'static str,
    (x, z): (f32, f32),
    centre: (f32, f32),
    glow: Mat,
) {
    let (dx, dz) = (centre.0 - x, centre.1 - z);
    let len = (dx * dx + dz * dz).sqrt().max(0.1);
    let (fx, fz) = (dx / len, dz / len);
    let yaw = fx.atan2(fz);
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 0.55, 0.0), v(1.4, 1.1, 0.7), Mat::Dark, true);
        p.boxed(v(0.0, 1.45, 0.1), v(1.2, 0.7, 0.05), glow, false);
        p.boxed(v(0.0, 1.9, 0.1), v(0.14, 0.4, 0.14), Mat::Metal, false);
    });
    p.interactable(key, v(x, 1.2, z), v(x + fx * 2.2, 0.0, z + fz * 2.2));
}

fn scrape_pit(p: &mut Plan) -> Theme {
    casing(p, POD1, Mat::NeonCyan);
    // The warehouse of the hall: the labelling sweatshop, two long rows facing each other.
    labeller_row(p, -30.0, 14.0, -7.4, 0.0, 7);
    labeller_row(p, -30.0, 14.0, 7.4, PI, 7);
    p.sign_near(-33.9, 0.0, 2.6, 4.2, Sign::IlLabelThis);
    p.sign_near(-22.0, -10.9, 2.7, 3.2, Sign::IlPieceRate);
    p.sign_near(-4.0, -10.9, 2.7, 3.2, Sign::IlLabelThis);
    p.sign_near(10.0, -10.9, 2.7, 3.2, Sign::IlPieceRate);
    p.sign_near(-22.0, 10.9, 2.7, 3.2, Sign::IlOptOut);
    p.sign_near(-4.0, 10.9, 2.7, 3.2, Sign::IlTos);
    p.sign_near(10.0, 10.9, 2.7, 3.2, Sign::IlCorpus);
    // Banners overhead: the hall has no ceiling art, only a quota.
    for x in [-26.0, -10.0, 6.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 20.0), Mat::Accent, false);
    }
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), false);
    arrival_kit(p, Sign::IlW3Back);
    // robots.txt, enormous and ignored: a printed sheet on a stand at the arrival, a barrier
    // across it that every drone has stepped over.
    p.sign(v(-38.2, 2.5, -4.6), FRAC_PI_2, 2.4, Sign::IlRobots);
    caution(p, -37.0, -3.4, FRAC_PI_2);
    // Cells: the break room (four minutes, timed), the shelving the machine has read and the
    // loading dock that feeds it.
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlBreak);
    p.prop(&TABLE_ROUND, v(-20.0, 0.0, -29.5), 0.0, true);
    for k in 0..3 {
        let a = k as f32 * 2.0 * PI / 3.0;
        p.prop(
            &STOOL_BAR,
            v(-20.0 + 1.5 * a.cos(), 0.0, -29.5 + 1.5 * a.sin()),
            a,
            false,
        );
    }
    water_cooler(p, -14.6, -32.4);
    p.boxed(v(-26.0, 0.6, -32.4), v(1.8, 1.2, 0.5), Mat::White, true);
    p.boxed(
        v(-26.0, 1.0, -32.1),
        v(1.4, 0.4, 0.04),
        Mat::ScreenGlow,
        false,
    );
    for x in [-2.0, 4.0, 10.0] {
        scraped_shelf(p, x, 32.4, Face::NegZ);
    }
    p.sign_near(6.0, 19.1, 2.7, 2.6, Sign::IlCorpus);
    ingest_stack(p, 1.0, 29.0, 0.2);
    ingest_stack(p, 11.0, 29.0, -0.3);
    for x in [6.0, 8.5] {
        scraped_shelf(p, x, -32.4, Face::PosZ);
    }
    ingest_stack(p, 12.5, -29.0, 0.5);
    ingest_stack(p, 3.5, -29.0, -0.2);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlDraftTag);
    // The pit: the five drafts on desks in a ring round the Queen, vents round the walls and a
    // conveyor feeding a hopper at the back that has been eating since 2019.
    let (cx, cz) = (POD1.0, 0.0);
    for (k, key) in [
        "prop:il_draft_1",
        "prop:il_draft_2",
        "prop:il_draft_3",
        "prop:il_draft_4",
        "prop:il_draft_5",
    ]
    .into_iter()
    .enumerate()
    {
        let a = (54.0 + 72.0 * k as f32).to_radians();
        draft_desk(p, key, cx + 10.5 * a.cos(), cz + 10.5 * a.sin(), (cx, cz));
    }
    class_console(
        p,
        "prop:il_devops_rate",
        (cx - 13.4, cz + 13.4),
        (cx, cz),
        Mat::LedGreen,
    );
    class_console(
        p,
        "prop:il_sec_robots",
        (cx - 13.4, cz - 13.4),
        (cx, cz),
        Mat::ScreenGlow,
    );
    class_console(
        p,
        "prop:il_ds_dashboard",
        (cx + 13.4, cz + 13.4),
        (cx, cz),
        Mat::NeonCyan,
    );
    floor_ring(p, cx, cz, 4.4, 20, Mat::NeonCyan, 0.1);
    floor_ring(p, cx, cz, 15.2, 40, Mat::Accent, 0.08);
    // The vents: eight grilles round the pit, where the waves come out.
    for k in 0..8 {
        let a = (22.5 + 45.0 * k as f32).to_radians();
        let (vx, vz) = (cx + 18.5 * a.cos(), cz + 18.5 * a.sin());
        vent_tile(p, vx, vz, Mat::Dark);
        p.boxed(v(vx, 0.02, vz), v(2.0, 0.02, 2.0), Mat::Hazard, false);
    }
    // The three proxy alcoves of Rotating Proxies. The Queen faces the door (west): `right` is the
    // north wall, so the spots of the spell, (right, forward) from her spawn, are (14.8, 0),
    // (-14.8, 0) and (0, 11.8), and these rings sit exactly there.
    proxy_alcove(p, (cx, cz - 14.8), (cx, cz));
    proxy_alcove(p, (cx, cz + 14.8), (cx, cz));
    proxy_alcove(p, (cx - 11.8, cz), (cx, cz));
    // The hopper line along the east wall.
    for z in [-13.0, -7.0, -1.0] {
        models::put(
            p,
            &CONVEYOR_LONG,
            v(cx + 21.0, 0.0, z),
            FRAC_PI_2,
            2.4,
            true,
        );
    }
    models::put(p, &HOPPER_SQUARE, v(cx + 21.0, 0.0, -17.5), 0.0, 3.2, true);
    models::put(p, &ROBOT_ARM_A, v(cx + 20.0, 0.0, 3.5), PI, 2.6, true);
    // Shelving of scraped content all round the walls, five metres up the arc of the pit.
    for z in [-17.0, -9.0, 9.0, 17.0] {
        scraped_shelf(p, cx - 22.4, z, Face::PosX);
    }
    for x in [-12.0, -4.0, 4.0, 12.0] {
        scraped_shelf(p, cx + x, -20.2, Face::PosZ);
        scraped_shelf(p, cx + x, 20.2, Face::NegZ);
    }
    p.sign(v(cx - 23.7, 3.4, 0.0), FRAC_PI_2, 4.6, Sign::IlRobots);
    p.sign(v(cx + 23.7, 3.4, -6.0), -FRAC_PI_2, 3.4, Sign::IlOptOut);
    p.sign(v(cx + 23.7, 3.4, 6.0), -FRAC_PI_2, 3.4, Sign::IlTos);
    p.sign_near(cx - 23.9, 12.0, 2.8, 3.0, Sign::IlCorpus);
    p.sign_near(cx - 23.9, -12.0, 2.8, 3.0, Sign::IlCorpus);
    strips(p, Mat::LightPanel, Mat::NeonCyan, 0.0, 0.1);
    cast(
        p,
        POD1,
        [
            ("door:campus_occupied", Sign::IlW3Back),
            ("door:ilmari3/2", Sign::IlW3ToValidation),
        ],
    );
    theme(1)
}

// ------------------------------------------------------------------ ilmari3/2: the validation wing

/// A staircase that climbs to the ceiling and stops: well signed, goes nowhere.
fn stairs_to_nowhere(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        for k in 0..10 {
            let h = 0.3 + k as f32 * 0.38;
            p.boxed(
                v(0.0, h / 2.0, k as f32 * 0.5),
                v(1.8, h, 0.5),
                if k % 2 == 0 { Mat::White } else { Mat::Beige },
                k < 2,
            );
        }
        p.boxed(v(0.0, 2.0, 5.2), v(1.8, 0.1, 0.2), Mat::Dark, false);
        p.sign(v(0.0, 2.9, 5.3), PI, 1.6, Sign::IlStairs);
    });
}

/// A door frame standing in the middle of nowhere, a door shut in the floor and one in the wall
/// that was never a wall: the Hallucination's idea of architecture.
fn door_to_nowhere(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(-0.55, 1.1, 0.0), v(0.12, 2.2, 0.12), Mat::Trim, false);
        p.boxed(v(0.55, 1.1, 0.0), v(0.12, 2.2, 0.12), Mat::Trim, false);
        p.boxed(v(0.0, 2.2, 0.0), v(1.22, 0.12, 0.12), Mat::Trim, false);
        p.boxed(v(0.0, 1.1, 0.0), v(0.98, 2.1, 0.03), Mat::NeonCyan, false);
    });
    p.block(v(x, 1.0, z), v(0.3, 2.0, 0.3));
}

fn floor_door(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 0.015, 0.0), v(1.1, 0.02, 2.1), Mat::Trim, false);
        p.boxed(v(0.0, 0.03, 0.0), v(0.9, 0.02, 1.9), Mat::Door, false);
        p.boxed(v(0.3, 0.05, 0.0), v(0.08, 0.04, 0.2), Mat::Gold, false);
    });
}

/// A Ground Truth board: a pole with a green screen that is right about most things, its sign,
/// and the prop anchor 1.8 m in front. `face` is +1 for a board on the north wall.
fn truth_board(p: &mut Plan, key: &'static str, x: f32, z: f32, face: f32) {
    p.boxed(v(x, 0.7, z), v(1.8, 1.4, 0.5), Mat::Dark, true);
    p.boxed(
        v(x, 1.45, z + face * 0.3),
        v(1.6, 0.9, 0.04),
        Mat::ScreenGlow,
        false,
    );
    p.boxed(v(x, 2.2, z), v(0.2, 1.7, 0.2), Mat::Metal, false);
    p.sign(
        v(x, 2.9, z + face * 0.1),
        if face > 0.0 { 0.0 } else { PI },
        2.2,
        Sign::IlTruth,
    );
    p.interactable(key, v(x, 1.2, z), v(x, 0.0, z + face * 2.0));
}

fn validation_wing(p: &mut Plan) -> Theme {
    casing(p, POD2, Mat::NeonCyan);
    // The hall: two rows of validators' desks (all of them confident), a staircase to the
    // ceiling, doors that are not in walls and doors that are in floors.
    labeller_row(p, -30.0, 14.0, -7.6, 0.0, 5);
    labeller_row(p, -30.0, 14.0, 7.6, PI, 5);
    stairs_to_nowhere(p, -26.0, -3.0, FRAC_PI_2);
    stairs_to_nowhere(p, 8.0, 3.0, -FRAC_PI_2);
    door_to_nowhere(p, -12.0, 2.4, 0.3);
    door_to_nowhere(p, 2.0, -2.4, -0.4);
    floor_door(p, -18.0, -3.8, 0.0);
    floor_door(p, -2.0, 3.8, 0.0);
    floor_door(p, 12.0, -2.0, FRAC_PI_2);
    p.sign_near(-33.9, 0.0, 2.7, 4.0, Sign::IlConfident);
    p.sign_near(-22.0, -10.9, 2.7, 3.2, Sign::IlImpossible);
    p.sign_near(-6.0, -10.9, 2.7, 3.2, Sign::IlFloor404);
    p.sign_near(10.0, -10.9, 2.7, 3.2, Sign::IlCitation);
    p.sign_near(-22.0, 10.9, 2.7, 3.2, Sign::IlCitation);
    p.sign_near(-6.0, 10.9, 2.7, 3.2, Sign::IlImpossible);
    p.sign_near(10.0, 10.9, 2.7, 3.2, Sign::IlConfident);
    for x in [-26.0, -10.0, 6.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 20.0), Mat::NeonCyan, false);
    }
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), false);
    arrival_kit(p, Sign::IlW3Back);
    // Cells: a library of confident citations, a printer room that prints footnotes and a
    // quiet room whose floor is a spreadsheet.
    scraped_shelf(p, -24.0, -32.4, Face::PosZ);
    scraped_shelf(p, -16.0, -32.4, Face::PosZ);
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlCitation);
    printer(p, 2.0, 32.8, Face::NegZ);
    printer(p, 6.0, 32.8, Face::NegZ);
    printer(p, 10.0, 32.8, Face::NegZ);
    ingest_stack(p, 2.0, 29.0, 0.2);
    p.sign_near(6.0, 19.1, 2.6, 2.4, Sign::IlFloor404);
    for k in 0..3 {
        for j in 0..3 {
            p.boxed(
                v(6.0 + k as f32 * 2.2, 0.02, -29.0 + j as f32 * 2.2),
                v(2.0, 0.03, 2.0),
                if (k + j) % 2 == 0 {
                    Mat::White
                } else {
                    Mat::Beige
                },
                false,
            );
        }
    }
    sad_plant(p, 13.0, -32.0);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlConfident);
    // The pit: a 3 x 3 grid of 8 m tiles round the Hallucination, every one of them plausible.
    // The tiles are the same material on purpose; the lines between them are the only truth the
    // floor admits to. The hazard cycle is drawn by the client on top of this.
    let (cx, cz) = (POD2.0, 0.0);
    let tile = 8.0;
    for row in 0..3 {
        for col in 0..3 {
            let (x, z) = (
                cx + (col as f32 - 1.0) * tile,
                cz + (row as f32 - 1.0) * tile,
            );
            p.boxed(
                v(x, 0.02, z),
                v(tile - 0.5, 0.03, tile - 0.5),
                if (row + col) % 2 == 0 {
                    Mat::White
                } else {
                    Mat::Marble
                },
                false,
            );
        }
    }
    for k in -1..=2 {
        let o = (k as f32 - 0.5) * tile;
        floor_line(
            p,
            pt(cx - 1.5 * tile, cz + o),
            pt(cx + 1.5 * tile, cz + o),
            0.16,
            Mat::NeonCyan,
        );
        floor_line(
            p,
            pt(cx + o, cz - 1.5 * tile),
            pt(cx + o, cz + 1.5 * tile),
            0.16,
            Mat::NeonCyan,
        );
    }
    // Impossible geometry in the corners: stairs into the wall, a door in mid-air, a bridge
    // that ends over a floor and a clock with thirteen numbers.
    stairs_to_nowhere(p, cx - 16.0, cz - 15.0, 0.0);
    stairs_to_nowhere(p, cx + 16.0, cz + 15.0, PI);
    door_to_nowhere(p, cx + 16.0, cz - 14.0, 0.0);
    door_to_nowhere(p, cx - 16.0, cz + 14.0, 0.0);
    // Ground Truth boards on the two long walls, far enough from the middle to be a trip.
    truth_board(p, "prop:il_truth_board_1", cx - 8.0, cz - 16.2, 1.0);
    truth_board(p, "prop:il_truth_board_2", cx + 8.0, cz + 16.2, -1.0);
    p.sign(v(cx - 19.7, 3.2, 0.0), FRAC_PI_2, 4.4, Sign::IlConfident);
    p.sign(v(cx + 19.7, 3.2, -5.0), -FRAC_PI_2, 3.4, Sign::IlFloor404);
    p.sign(v(cx + 19.7, 3.2, 5.0), -FRAC_PI_2, 3.4, Sign::IlCitation);
    p.sign_near(cx - 6.0, -17.9, 3.0, 3.2, Sign::IlImpossible);
    p.sign_near(cx + 6.0, 17.9, 3.0, 3.2, Sign::IlImpossible);
    p.boxed(v(cx, 4.0, 0.0), v(0.3, 0.12, 36.0), Mat::Dark, false);
    strips(p, Mat::LightPanel, Mat::NeonCyan, 0.0, 0.1);
    cast(
        p,
        POD2,
        [
            ("door:ilmari3/1", Sign::IlW3Back),
            ("door:ilmari3/3", Sign::IlW3ToTuning),
        ],
    );
    theme(2)
}

// ------------------------------------------------------------------ ilmari3/3: the fine-tuning chamber

/// A mirror panel and the mirror in it: `generation` fades the glass and the frame toward beige.
fn mirror(p: &mut Plan, x: f32, y: f32, z: f32, yaw: f32, generation: usize) {
    let (glass, frame) = match generation {
        0 => (Mat::Chrome, Mat::Gold),
        1 => (Mat::Glass, Mat::Metal),
        _ => (Mat::Frosted, Mat::Beige),
    };
    p.framed(v(x, y, z), yaw, |p| {
        p.boxed(v(0.0, 0.0, 0.0), v(1.9, 2.4, 0.08), frame, false);
        p.boxed(v(0.0, 0.0, 0.05), v(1.7, 2.2, 0.04), glass, false);
        // The panel reflected in itself: a smaller frame inside, then a smaller one.
        p.boxed(v(0.0, 0.0, 0.08), v(1.0, 1.3, 0.02), frame, false);
        p.boxed(v(0.0, 0.0, 0.1), v(0.55, 0.7, 0.02), glass, false);
    });
}

/// A photocopier pair mid-job: copy of a copy of a copy, stacked as it came.
fn copier(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    models::put(p, &MACHINE_WINDOW, v(x, 0.0, z), yaw, 1.6, true);
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(1.2, 0.5, 0.0), v(0.6, 1.0, 0.8), Mat::Paper, false);
        p.boxed(v(1.2, 1.05, 0.0), v(0.5, 0.1, 0.7), Mat::Beige, false);
        p.boxed(v(0.0, 1.8, 0.6), v(0.9, 0.08, 0.04), Mat::LedGreen, false);
    });
}

fn tuning_chamber(p: &mut Plan) -> Theme {
    casing(p, POD3, Mat::NeonYellow);
    // The hall: a copy room. Copiers back to back, boxes of "originals" that are also copies,
    // a long table of the day's output and a TV that shows the last TV.
    for x in [-30.0, -22.0, -14.0, -6.0, 2.0, 10.0] {
        copier(p, x, -8.0, PI);
        copier(p, x + 3.0, 8.0, 0.0);
    }
    for x in [-26.0, -8.0, 6.0] {
        p.prop(&TV_VINTAGE, v(x, 0.0, -10.2), PI, true);
        p.prop(&TV_VINTAGE, v(x + 1.2, 0.0, 10.2), 0.0, true);
    }
    p.sign_near(-33.9, 0.0, 2.7, 4.0, Sign::IlGen1);
    p.sign_near(-22.0, -10.9, 2.7, 3.0, Sign::IlGen2);
    p.sign_near(-6.0, -10.9, 2.7, 3.0, Sign::IlGen3);
    p.sign_near(10.0, -10.9, 2.7, 3.0, Sign::IlBeige);
    p.sign_near(-22.0, 10.9, 2.7, 3.0, Sign::IlEpoch);
    p.sign_near(-6.0, 10.9, 2.7, 3.0, Sign::IlGen3);
    p.sign_near(10.0, 10.9, 2.7, 3.0, Sign::IlFineTune);
    for x in [-26.0, -10.0, 6.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 20.0), Mat::Beige, false);
    }
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), false);
    arrival_kit(p, Sign::IlW3Back);
    // Cells: a recycling room of paper that is all one grey, the "originals" archive and a
    // loop of monitors showing the loop.
    for k in 0..4 {
        ingest_stack(p, -26.0 + k as f32 * 2.4, -29.5, 0.2 * k as f32);
    }
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlBeige);
    doc_shelf(p, -14.0, -32.4, Face::PosZ);
    for k in 0..3 {
        p.prop(&TV, v(2.0 + k as f32 * 2.4, 0.0, 32.0), PI, true);
    }
    p.sign_near(6.0, 19.1, 2.6, 2.4, Sign::IlEpoch);
    scraped_shelf(p, 6.0, -32.4, Face::PosZ);
    scraped_shelf(p, 10.0, -32.4, Face::PosZ);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlGen2);
    // The pit: a hall of mirrors, each wall a copy of the last one a little duller. Three rings
    // on the floor in graded beige (generation 1, 2, 3) and the boss in the middle one.
    let (cx, cz) = (POD3.0, 0.0);
    for k in 0..7 {
        let z = cz - 15.0 + k as f32 * 5.0;
        mirror(p, cx - 19.6, 2.0, z, FRAC_PI_2, k % 3);
        mirror(p, cx + 19.6, 2.0, z, -FRAC_PI_2, (k + 1) % 3);
    }
    for k in 0..7 {
        let x = cx - 15.0 + k as f32 * 5.0;
        mirror(p, x, 2.0, cz - 17.6, 0.0, k % 3);
        mirror(p, x, 2.0, cz + 17.6, PI, (k + 2) % 3);
    }
    floor_ring(p, cx, cz, 5.0, 22, Mat::NeonYellow, 0.12);
    floor_ring(p, cx, cz, 9.5, 36, Mat::Gold, 0.1);
    floor_ring(p, cx, cz, 14.0, 48, Mat::Beige, 0.1);
    p.sign(v(cx, 3.9, -17.3), 0.0, 3.6, Sign::IlTrainingLoss);
    p.sign(v(cx, 3.9, 17.3), PI, 3.6, Sign::IlTrainingLoss);
    p.sign(v(cx - 19.2, 3.9, 0.0), FRAC_PI_2, 3.6, Sign::IlGen1);
    p.sign(v(cx + 19.2, 3.9, -5.0), -FRAC_PI_2, 3.2, Sign::IlGen2);
    p.sign(v(cx + 19.2, 3.9, 5.0), -FRAC_PI_2, 3.2, Sign::IlGen3);
    // A few of the dead originals: boxes of paper at the edge of the floor, beige to the last.
    for (x, z) in [(-14.0, -13.0), (14.0, 13.0), (-14.0, 13.0), (14.0, -13.0)] {
        models::put(p, &BOX_SMALL, v(cx + x, 0.0, cz + z), x * 0.1, 1.0, true);
    }
    p.boxed(v(cx, 4.0, 0.0), v(0.3, 0.12, 34.0), Mat::Dark, false);
    strips(p, Mat::LightPanel, Mat::NeonYellow, 0.0, 0.1);
    cast(
        p,
        POD3,
        [
            ("door:ilmari3/2", Sign::IlW3Back),
            ("door:campus_occupied", Sign::IlW3Out),
        ],
    );
    theme(3)
}
