//! Ilmari Wing 5, the Alignment Lab (docs/ILMARI_1_0.md D.3): the Innovation Lab's upper floor
//! after the occupation, on the plan of Wings 1-3 (corridor, hall, corridor, pod and three cells)
//! with a pod of its own for each room. ilmari5/1 is the Mezzanine (an Ethics Wall of plaques that
//! contradict each other, the Responsible AI department of one, a lanyard store, a training room
//! and a concerns archive), ilmari5/2 the Policy Floor (the Alignment Consultant's rotunda: banners
//! that say "aligned", three pillars to hide behind and four Opt-Out Kiosks) and ilmari5/3 the
//! Red-Blue Range (a glass-walled exercise floor, racks as cover, a gallery of dark monitors and
//! three honeypots; Red takes the north side, Blue the south). No point lights (the 0.9 rule):
//! light is emissive strips and neon, so the rooms cost what a Wing 3 room costs.

use super::datacenter::dress::*;
use super::ilmari3::{arrival_kit, casing, cast};
use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::models::{self, BARRIER, BOX_LARGE, BOX_OPEN, BOX_SMALL};
use super::props::{sad_plant, server_rack, water_cooler};
use super::themes::{pt, v};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// The Mezzanine's atrium and the two boss pods, `(centre x, half x, half z)`: the Consultant's
/// rotunda has to hold four kiosks and three pillars, the range a lane of racks per sensor.
const POD1: (f32, f32, f32) = (46.0, 18.0, 16.0);
const POD2: (f32, f32, f32) = (46.0, 22.0, 20.0);
const POD3: (f32, f32, f32) = (46.0, 24.0, 21.0);
/// Where the Consultant's three pillars stand, as offsets from his spawn point (x, z): the cover
/// of `Principles` is `(right, forward)` = `(-9, 10)`, `(9, 10)` and `(0, 17)` and he faces west.
pub const PILLARS: [(f32, f32); 3] = [(-10.0, 9.0), (-10.0, -9.0), (-17.0, 0.0)];
/// The four Opt-Out Kiosks of the rotunda: offsets from the pod centre.
pub const KIOSKS: [(f32, f32); 4] = [(-4.0, -18.0), (-4.0, 18.0), (14.0, -18.0), (14.0, 18.0)];
/// The three honeypots of the range: offsets from the pod centre.
pub const HONEYPOTS: [(f32, f32); 3] = [(-14.0, -17.0), (-2.0, 17.0), (18.0, -2.0)];

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let (zone, name, theme) = match level {
        1 => (
            "ilmari5/1",
            "The Alignment Lab: The Mezzanine",
            mezzanine(&mut p),
        ),
        2 => (
            "ilmari5/2",
            "The Alignment Lab: The Policy Floor",
            policy_floor(&mut p),
        ),
        _ => (
            "ilmari5/3",
            "The Alignment Lab: The Red-Blue Range",
            red_blue_range(&mut p),
        ),
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(level: u8) -> Theme {
    let c = Color::srgb;
    let (wall, fog, ambient, floor, accent) = match level {
        // Mezzanine: responsible blue-white, glass and a very calm hum.
        1 => (
            c(0.7, 0.74, 0.82),
            c(0.12, 0.16, 0.26),
            c(0.92, 0.96, 1.0),
            [0.78, 0.82, 0.9],
            LinearRgba::rgb(0.7, 1.9, 3.4),
        ),
        // Policy floor: banner blue, the colour of a terms page you did not read.
        2 => (
            c(0.62, 0.68, 0.84),
            c(0.1, 0.12, 0.3),
            c(0.88, 0.92, 1.0),
            [0.7, 0.74, 0.9],
            LinearRgba::rgb(0.8, 1.4, 4.0),
        ),
        // Range: a dim exercise floor, red on the north side, blue on the south.
        _ => (
            c(0.5, 0.52, 0.62),
            c(0.16, 0.1, 0.22),
            c(0.82, 0.8, 1.0),
            [0.56, 0.56, 0.66],
            LinearRgba::rgb(2.8, 0.6, 3.0),
        ),
    };
    Theme {
        wall,
        trim: c(0.1, 0.12, 0.2),
        accent,
        fabric: [c(0.25, 0.4, 0.7), c(0.7, 0.3, 0.32), c(0.4, 0.4, 0.55)],
        floor: FloorKind::Tile(floor),
        fog,
        ambient,
        ambient_brightness: 820.0,
        sun: c(0.92, 0.96, 1.0),
        sun_illuminance: 2600.0,
        fog_range: (26.0, 95.0),
    }
}

// ------------------------------------------------------------------ shared pieces

/// A workstation: a desk, a screen, a keyboard and a chair, facing `yaw`.
pub(super) fn workstation(p: &mut Plan, x: f32, z: f32, yaw: f32, screen: Mat) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.prop(&DESK, v(0.0, 0.0, 0.0), 0.0, true);
        p.prop(&SCREEN, v(0.0, 0.8, -0.1), 0.0, false);
        p.prop(&KEYBOARD, v(0.0, 0.8, 0.2), 0.0, false);
        p.boxed(v(0.0, 1.05, -0.2), v(0.8, 0.4, 0.05), screen, false);
        p.prop(&CHAIR_DESK, v(0.0, 0.0, 1.0), PI, false);
    });
}

/// A row of identical workstations along z.
pub(super) fn row(p: &mut Plan, x0: f32, x1: f32, z: f32, yaw: f32, n: usize) {
    for k in 0..n {
        let x = x0 + (x1 - x0) * k as f32 / (n.max(2) - 1) as f32;
        let screen = if k % 3 == 1 {
            Mat::LedBlue
        } else {
            Mat::ScreenGlow
        };
        workstation(p, x, z, yaw, screen);
    }
}

/// A banner hanging from the ceiling, `w` wide, facing both ways.
pub(super) fn banner(p: &mut Plan, x: f32, z: f32, yaw: f32, w: f32, sign: Sign) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 3.55, 0.0), v(w + 0.2, 1.3, 0.06), Mat::Trim, false);
        p.sign(v(0.0, 3.55, 0.05), 0.0, w, sign);
        p.sign(v(0.0, 3.55, -0.05), PI, w, sign);
        for s in [-1.0_f32, 1.0] {
            p.boxed(
                v(s * w * 0.48, 4.05, 0.0),
                v(0.04, 0.5, 0.04),
                Mat::Metal,
                false,
            );
        }
    });
}

/// A row of chairs facing a wall of screens: a training or a hearing, as the sign says.
pub(super) fn auditorium(p: &mut Plan, x: f32, z: f32, yaw: f32, rows: usize, cols: usize) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        for r in 0..rows {
            for c in 0..cols {
                p.prop(
                    &CHAIR_MODERN,
                    v(
                        (c as f32 - (cols as f32 - 1.0) / 2.0) * 0.9,
                        0.0,
                        r as f32 * 1.2,
                    ),
                    PI,
                    false,
                );
            }
        }
        p.prop(&TV, v(-1.0, 0.8, -1.2), 0.0, true);
        p.prop(&CABINET_TV, v(-1.0, 0.0, -1.2), 0.0, true);
        p.boxed(v(1.2, 1.1, -1.4), v(1.4, 0.8, 0.05), Mat::ScreenGlow, false);
    });
}

/// A filing wall: bookcases full of folders, with the label that says what the folders are.
pub(super) fn archive(p: &mut Plan, x: f32, z: f32, face: Face, n: usize) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        for k in 0..n {
            p.prop(
                &BOOKCASE_CLOSED_WIDE,
                v(k as f32 * 1.9 - (n as f32 - 1.0), 0.0, 0.0),
                0.0,
                true,
            );
        }
        p.boxed(
            v(0.0, 2.3, 0.2),
            v(n as f32 * 1.9, 0.3, 0.04),
            Mat::Paper,
            false,
        );
    });
}

/// The Ethics Wall: a run of plaques, each one a principle, no two of them in agreement.
fn ethics_wall(p: &mut Plan, x0: f32, x1: f32, z: f32) {
    let signs = [
        Sign::IlPrincipleA,
        Sign::IlPrincipleB,
        Sign::IlPrincipleC,
        Sign::IlPrincipleD,
    ];
    let n = 6;
    for k in 0..n {
        let x = x0 + (x1 - x0) * k as f32 / (n - 1) as f32;
        p.sign_near(x, z, 2.5, 2.4, signs[k % 4]);
    }
}

/// A rope-and-post queue line toward `to`: the shortest way to nothing.
pub(super) fn queue(p: &mut Plan, x: f32, z: f32, len: usize, along_x: bool) {
    for k in 0..=len {
        let (px, pz) = if along_x {
            (x + k as f32 * 1.6, z)
        } else {
            (x, z + k as f32 * 1.6)
        };
        p.prim(
            PrimKind::Cylinder,
            v(px, 0.45, pz),
            v(0.12, 0.9, 0.12),
            Mat::Gold,
        );
        if k < len {
            let size = if along_x {
                v(1.6, 0.04, 0.04)
            } else {
                v(0.04, 0.04, 1.6)
            };
            let at = if along_x {
                v(px + 0.8, 0.8, pz)
            } else {
                v(px, 0.8, pz + 0.8)
            };
            p.boxed(at, size, Mat::Red, false);
        }
    }
}

// ------------------------------------------------------------------ ilmari5/1: the mezzanine

fn mezzanine(p: &mut Plan) -> Theme {
    casing(p, POD1, Mat::NeonCyan);
    // The hall: the Ethics Wall on the north side, the Responsible AI department (one desk) in
    // the middle, a hiring banner nobody has ever answered.
    ethics_wall(p, -30.0, 14.0, -10.9);
    p.sign_near(-33.9, 0.0, 2.7, 4.2, Sign::IlW5Door);
    p.sign_near(-26.0, 10.9, 2.7, 3.2, Sign::IlHiringOne);
    p.sign_near(-8.0, 10.9, 2.7, 3.2, Sign::IlConcerns);
    p.sign_near(10.0, 10.9, 2.7, 3.2, Sign::IlSafeDefault);
    for x in [-28.0, -20.0, -12.0, -4.0, 4.0, 12.0] {
        archive(p, x, -9.9, Face::PosZ, 1);
    }
    row(p, -26.0, 10.0, 6.5, PI, 4);
    workstation(p, -14.0, -6.2, 0.0, Mat::LedGreen);
    for x in [-30.0, -18.0, -6.0, 8.0, 15.0] {
        p.prop(&POTTED_PLANT, v(x, 0.0, 9.6), 0.0, false);
    }
    water_cooler(p, 16.5, -9.2);
    for x in [-26.0, -8.0, 8.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 20.0), Mat::Accent, false);
    }
    banner(p, -2.0, 0.0, FRAC_PI_2, 6.0, Sign::IlHiringOne);
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), false);
    arrival_kit(p, Sign::IlW5Back);
    // Cells: the lanyard store (every conference since 2009), the training room (a mandatory
    // hour on responsible use, taken at the pace of the slowest slide) and the concerns archive.
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlConcerns);
    for k in 0..4 {
        p.prop(
            &COAT_RACK,
            v(-26.0 + k as f32 * 2.4, 0.0, -31.0),
            0.0,
            false,
        );
    }
    p.prop(&TABLE_ROUND, v(-18.0, 0.0, -30.0), 0.0, true);
    p.boxed(v(-18.0, 0.9, -30.0), v(0.5, 0.1, 0.4), Mat::Paper, false);
    p.sign_near(6.0, 19.1, 2.6, 2.4, Sign::IlTerms);
    auditorium(p, 6.0, 31.0, PI, 2, 4);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlConcerns);
    archive(p, 7.0, -32.2, Face::PosZ, 2);
    archive(p, 13.0, -32.2, Face::PosZ, 1);
    sad_plant(p, 15.5, -30.0);
    // The atrium: a glass stair that climbs to a sign and stops, the department's banner and a
    // reception desk with a bell nobody has rung.
    let (cx, cz) = (POD1.0, 0.0);
    floor_ring(p, cx, cz, 5.0, 24, Mat::NeonCyan, 0.1);
    floor_ring(p, cx, cz, 9.5, 36, Mat::Accent, 0.08);
    for k in 0..8 {
        let h = 0.3 + k as f32 * 0.4;
        p.boxed(
            v(cx + 4.0 + k as f32 * 0.6, h / 2.0, cz - 14.0),
            v(0.6, h, 2.4),
            if k % 2 == 0 { Mat::Glass } else { Mat::Frosted },
            k < 2,
        );
    }
    p.sign(
        v(cx + 8.6, 3.3, cz - 14.0),
        -FRAC_PI_2,
        3.0,
        Sign::IlHiringOne,
    );
    p.prop(&DESK, v(cx - 6.0, 0.0, cz + 13.0), 0.0, true);
    p.prop(&CHAIR_DESK, v(cx - 6.0, 0.0, cz + 14.2), PI, false);
    p.boxed(
        v(cx - 6.0, 0.9, cz + 13.0),
        v(0.2, 0.1, 0.2),
        Mat::Gold,
        false,
    );
    queue(p, cx - 14.0, cz + 9.0, 4, true);
    for z in [-12.0, 12.0] {
        banner(p, cx + 8.0, cz + z, 0.0, 4.0, Sign::IlAlignedBanner);
    }
    p.sign(v(cx - 17.7, 3.0, 0.0), FRAC_PI_2, 4.0, Sign::IlW5Door);
    p.sign(v(cx + 17.7, 3.0, -4.0), -FRAC_PI_2, 3.2, Sign::IlPrincipleB);
    p.sign(v(cx + 17.7, 3.0, 4.0), -FRAC_PI_2, 3.2, Sign::IlPrincipleA);
    for (x, z) in [(-12.0, -12.0), (12.0, 12.0), (-12.0, 12.0)] {
        models::put(p, &BOX_SMALL, v(cx + x, 0.0, cz + z), x * 0.1, 1.0, true);
    }
    strips(p, Mat::LightPanel, Mat::NeonCyan, 0.0, 0.1);
    cast(
        p,
        POD1,
        [
            ("door:campus_occupied", Sign::IlW5Back),
            ("door:ilmari5/2", Sign::IlW5ToPolicy),
        ],
    );
    theme(1)
}

// ------------------------------------------------------------------ ilmari5/2: the policy floor

/// An Opt-Out Kiosk: a lit cabinet with a screen and a sign, `face` toward the room (+1 for the
/// south wall), and the prop's stand spot 2.2 m in front of it.
pub(super) fn kiosk(p: &mut Plan, key: &'static str, x: f32, z: f32, face: f32) {
    p.boxed(v(x, 0.7, z), v(1.4, 1.4, 0.6), Mat::Dark, true);
    p.boxed(
        v(x, 1.25, z - face * 0.32),
        v(1.1, 0.7, 0.04),
        Mat::LedGreen,
        false,
    );
    p.boxed(
        v(x, 0.25, z - face * 0.32),
        v(0.9, 0.2, 0.04),
        Mat::NeonCyan,
        false,
    );
    p.boxed(v(x, 2.1, z), v(0.16, 1.4, 0.16), Mat::Metal, false);
    p.sign(
        v(x, 2.9, z - face * 0.1),
        if face > 0.0 { PI } else { 0.0 },
        2.2,
        Sign::IlKiosk,
    );
    p.interactable(key, v(x, 1.2, z), v(x, 0.0, z - face * 2.2));
}

/// A pillar of the rotunda: a fluted column with a principle on it (the cover of `Principles`).
fn pillar(p: &mut Plan, x: f32, z: f32, sign: Sign) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 1.8, z),
        v(1.6, 3.6, 1.6),
        Mat::Marble,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.12, z),
        v(2.1, 0.24, 2.1),
        Mat::MarbleDark,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x, 3.5, z),
        v(2.0, 0.2, 2.0),
        Mat::Gold,
    );
    p.block(v(x, 1.8, z), v(1.6, 3.6, 1.6));
    p.sign(v(x - 0.82, 2.0, z), -FRAC_PI_2, 1.1, sign);
    p.sign(v(x + 0.82, 2.0, z), FRAC_PI_2, 1.1, sign);
}

fn policy_floor(p: &mut Plan) -> Theme {
    casing(p, POD2, Mat::NeonCyan);
    // The hall: a hearing room of the policy workshop, rows of chairs before two screens of
    // terms, a rope queue to a desk that only stamps.
    auditorium(p, -24.0, -8.0, 0.0, 2, 5);
    auditorium(p, -24.0, 8.0, PI, 2, 5);
    auditorium(p, 0.0, -8.0, 0.0, 2, 4);
    auditorium(p, 0.0, 8.0, PI, 2, 4);
    queue(p, 6.0, 4.0, 4, true);
    p.sign_near(-33.9, 0.0, 2.7, 4.0, Sign::IlW5ToPolicy);
    p.sign_near(-22.0, -10.9, 2.7, 3.2, Sign::IlTerms);
    p.sign_near(-6.0, -10.9, 2.7, 3.2, Sign::IlSafeDefault);
    p.sign_near(10.0, -10.9, 2.7, 3.2, Sign::IlTerms);
    p.sign_near(-22.0, 10.9, 2.7, 3.2, Sign::IlAlignedBanner);
    p.sign_near(-6.0, 10.9, 2.7, 3.2, Sign::IlConcerns);
    p.sign_near(10.0, 10.9, 2.7, 3.2, Sign::IlSafeDefault);
    for x in [-26.0, -10.0, 6.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 20.0), Mat::Accent, false);
    }
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), false);
    arrival_kit(p, Sign::IlW5Back);
    // Cells: legal review (a wall of binders), the approvals queue and the shredder room.
    archive(p, -24.0, -32.2, Face::PosZ, 2);
    archive(p, -16.0, -32.2, Face::PosZ, 1);
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlTerms);
    for k in 0..4 {
        p.prop(&CHAIR_MODERN, v(2.0 + k as f32 * 2.2, 0.0, 32.4), PI, false);
    }
    p.sign_near(6.0, 19.1, 2.6, 2.4, Sign::IlSafeDefault);
    for k in 0..3 {
        p.prop(&TRASHCAN, v(5.0 + k as f32 * 2.4, 0.0, -32.4), 0.0, false);
    }
    models::put(p, &BOX_LARGE, v(12.0, 0.0, -30.0), 0.3, 1.2, true);
    models::put(p, &BOX_OPEN, v(12.2, 0.62, -30.0), 0.7, 1.0, false);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlConcerns);
    // The rotunda: banners that say "aligned", four kiosks round the edge, three pillars on the
    // spots the Principles hide behind and rings on the floor like the lines of a form.
    let (cx, cz) = (POD2.0, 0.0);
    for (k, (dx, dz)) in KIOSKS.into_iter().enumerate() {
        let key = [
            "prop:il_optout_1",
            "prop:il_optout_2",
            "prop:il_optout_3",
            "prop:il_optout_4",
        ][k];
        kiosk(p, key, cx + dx, cz + dz, dz.signum());
    }
    for (k, (dx, dz)) in PILLARS.into_iter().enumerate() {
        pillar(
            p,
            cx + dx,
            cz + dz,
            [Sign::IlPrincipleA, Sign::IlPrincipleB, Sign::IlPrincipleC][k],
        );
    }
    floor_ring(p, cx, cz, 5.0, 24, Mat::NeonCyan, 0.12);
    floor_ring(p, cx, cz, 9.0, 36, Mat::Gold, 0.1);
    floor_ring(p, cx, cz, 17.0, 56, Mat::Accent, 0.1);
    for k in 0..6 {
        let a = (30.0 + 60.0 * k as f32).to_radians();
        banner(
            p,
            cx + 19.0 * a.cos(),
            cz + 17.0 * a.sin(),
            -a + FRAC_PI_2,
            3.6,
            [Sign::IlAlignedBanner, Sign::IlSafeDefault][k % 2],
        );
    }
    for z in [-9.0, 9.0] {
        p.prop(&TV, v(cx + 21.4, 0.9, z), -FRAC_PI_2, false);
        p.boxed(
            v(cx + 21.8, 1.2, z),
            v(0.1, 1.0, 1.4),
            Mat::ScreenGlow,
            false,
        );
    }
    p.sign(
        v(cx - 21.7, 3.4, 0.0),
        FRAC_PI_2,
        4.6,
        Sign::IlAlignedBanner,
    );
    p.sign(v(cx + 21.7, 3.4, -5.0), -FRAC_PI_2, 3.4, Sign::IlTerms);
    p.sign(v(cx + 21.7, 3.4, 5.0), -FRAC_PI_2, 3.4, Sign::IlConcerns);
    p.boxed(v(cx, 4.0, 0.0), v(0.3, 0.12, 38.0), Mat::Dark, false);
    strips(p, Mat::LightPanel, Mat::NeonCyan, 0.0, 0.1);
    cast(
        p,
        POD2,
        [
            ("door:ilmari5/1", Sign::IlW5Back),
            ("door:ilmari5/3", Sign::IlW5ToRange),
        ],
    );
    theme(2)
}

// ------------------------------------------------------------------ ilmari5/3: the red-blue range

/// A honeypot: a lit laptop on a pedestal, a sticker that says not to touch it, and the prop's
/// stand spot 2.2 m in front (toward the middle).
fn honeypot(p: &mut Plan, key: &'static str, x: f32, z: f32, centre: (f32, f32)) {
    let (dx, dz) = (centre.0 - x, centre.1 - z);
    let len = (dx * dx + dz * dz).sqrt().max(0.1);
    let (fx, fz) = (dx / len, dz / len);
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.5, z),
        v(0.9, 1.0, 0.9),
        Mat::Metal,
    );
    p.prop(&LAPTOP, v(x, 1.0, z), (-fx).atan2(-fz) + PI, false);
    p.boxed(v(x, 1.12, z), v(0.5, 0.02, 0.4), Mat::ScreenGlow, false);
    p.boxed(v(x, 0.12, z), v(1.3, 0.04, 1.3), Mat::Hazard, false);
    p.block(v(x, 0.5, z), v(0.9, 1.0, 0.9));
    p.sign(
        v(x - fx * 0.2, 1.9, z - fz * 0.2),
        fx.atan2(fz) + PI,
        1.1,
        Sign::IlHoneypot,
    );
    p.interactable(key, v(x, 1.0, z), v(x + fx * 2.2, 0.0, z + fz * 2.2));
}

/// A rack of cover: two cabinets back to back, tall enough to hide behind.
fn cover_rack(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        server_rack(p, -0.55, 0.0, 0.0);
        server_rack(p, 0.55, 0.0, PI);
    });
}

fn red_blue_range(p: &mut Plan) -> Theme {
    casing(p, POD3, Mat::NeonPink);
    // The hall: the exercise floor. Rows of racks as cover with a lane of floor tape between,
    // red on the north side and blue on the south, and a gallery of dark monitors on the south wall.
    for x in [-28.0, -14.0, 0.0, 12.0] {
        cover_rack(p, x, -7.5, FRAC_PI_2);
        cover_rack(p, x + 5.0, 7.5, FRAC_PI_2);
    }
    floor_line(p, pt(-34.0, -4.4), pt(18.0, -4.4), 0.14, Mat::Red);
    floor_line(p, pt(-34.0, 4.4), pt(18.0, 4.4), 0.14, Mat::LedBlue);
    for k in 0..6 {
        p.prop(&TV, v(-30.0 + k as f32 * 8.0, 1.2, 10.5), PI, false);
        p.boxed(
            v(-30.0 + k as f32 * 8.0, 1.7, 10.78),
            v(1.5, 0.9, 0.04),
            Mat::Dark,
            false,
        );
    }
    p.sign_near(-33.9, 0.0, 2.7, 4.2, Sign::IlRange);
    p.sign_near(-22.0, -10.9, 2.7, 3.2, Sign::IlRedTeam);
    p.sign_near(-6.0, -10.9, 2.7, 3.2, Sign::IlSensorLane);
    p.sign_near(10.0, -10.9, 2.7, 3.2, Sign::IlRedTeam);
    p.sign_near(-22.0, 10.9, 2.7, 3.2, Sign::IlLabDark);
    p.sign_near(-6.0, 10.9, 2.7, 3.2, Sign::IlBlueTeam);
    p.sign_near(10.0, 10.9, 2.7, 3.2, Sign::IlLabDark);
    for x in [-26.0, -10.0, 6.0] {
        p.boxed(v(x, 3.7, 0.0), v(0.1, 0.5, 20.0), Mat::NeonPink, false);
    }
    cable_tray(p, pt(-32.0, 0.0), pt(14.0, 0.0), false);
    arrival_kit(p, Sign::IlW5Back);
    // Cells: Red's armoury (racks and a barrier), Blue's operations room (a wall of screens)
    // and the findings wall (everything they found, framed).
    for k in 0..3 {
        server_rack(p, -26.0 + k as f32 * 2.6, -32.4, 0.0);
    }
    models::put(p, &BARRIER, v(-18.0, 0.0, -28.0), FRAC_PI_2, 2.4, true);
    p.sign_near(-20.0, -19.1, 2.6, 2.4, Sign::IlRedTeam);
    row(p, 0.0, 10.0, 31.0, PI, 3);
    p.sign_near(6.0, 19.1, 2.6, 2.4, Sign::IlBlueTeam);
    archive(p, 7.0, -32.2, Face::PosZ, 2);
    p.sign_near(10.0, -19.1, 2.6, 2.4, Sign::IlPurpleSoon);
    sad_plant(p, 15.5, -30.0);
    // The range: Red's floor patch north, Blue's south (the squads of `SummonSquad` stand there),
    // lanes of cover for the three sensors, three honeypots and a viewing gallery over it all.
    let (cx, cz) = (POD3.0, 0.0);
    p.boxed(v(cx, 0.02, cz - 11.0), v(30.0, 0.03, 12.0), Mat::Red, false);
    p.boxed(
        v(cx, 0.02, cz + 11.0),
        v(30.0, 0.03, 12.0),
        Mat::LedBlue,
        false,
    );
    floor_ring(p, cx, cz, 6.0, 28, Mat::NeonPink, 0.12);
    for (dx, dz, yaw) in [
        (-10.0, -6.0, 0.0),
        (-10.0, 6.0, 0.0),
        (6.0, -6.0, 0.0),
        (6.0, 6.0, 0.0),
        (-18.0, 0.0, FRAC_PI_2),
        (14.0, 12.0, FRAC_PI_2),
        (14.0, -12.0, FRAC_PI_2),
    ] {
        cover_rack(p, cx + dx, cz + dz, yaw);
    }
    for (k, (dx, dz)) in HONEYPOTS.into_iter().enumerate() {
        let key = [
            "prop:il_honeypot_1",
            "prop:il_honeypot_2",
            "prop:il_honeypot_3",
        ][k];
        honeypot(p, key, cx + dx, cz + dz, (cx, cz));
    }
    for z in [-17.0, 17.0] {
        p.boxed(
            v(cx - 4.0, 1.0, cz + z),
            v(34.0, 1.0, 0.1),
            Mat::Glass,
            false,
        );
    }
    p.sign(v(cx - 23.7, 3.4, 0.0), FRAC_PI_2, 4.6, Sign::IlRange);
    p.sign(v(cx + 23.7, 3.4, -5.0), -FRAC_PI_2, 3.4, Sign::IlRedTeam);
    p.sign(v(cx + 23.7, 3.4, 5.0), -FRAC_PI_2, 3.4, Sign::IlBlueTeam);
    p.sign_near(cx - 8.0, -20.7, 3.2, 3.2, Sign::IlSensorLane);
    p.sign_near(cx + 8.0, 20.7, 3.2, 3.2, Sign::IlSensorLane);
    p.sign_near(cx + 9.0, -20.7, 3.2, 3.2, Sign::IlAlignDone);
    p.boxed(v(cx, 4.0, 0.0), v(0.3, 0.12, 40.0), Mat::Dark, false);
    strips(p, Mat::LightPanel, Mat::NeonPink, 0.0, 0.1);
    cast(
        p,
        POD3,
        [
            ("door:ilmari5/2", Sign::IlW5Back),
            ("door:campus_occupied", Sign::IlW5Out),
        ],
    );
    theme(3)
}
