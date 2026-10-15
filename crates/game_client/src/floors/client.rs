//! "Very Large Insurance & Banking Group": the grey client office of the Body Leasing easter
//! egg. Three rooms (lobby, an open-plan hall of empty desks, one meeting room), no enemies and
//! no boss on purpose. The only door is the one you came in by; its sign says the badge is pending.

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble, yaw_towards};
use super::props::{long_table, printer};
use super::shape::{rounded_rect, stroke};
use super::themes::{ceiling_lights, pt, v};
use bevy::prelude::*;
use std::f32::consts::PI;

pub const ZONE: &str = "client/1";

fn theme() -> Theme {
    let c = Color::srgb;
    Theme {
        wall: c(0.78, 0.74, 0.66),
        trim: c(0.42, 0.43, 0.45),
        accent: LinearRgba::rgb(0.5, 0.5, 0.45),
        fabric: [c(0.5, 0.5, 0.52), c(0.6, 0.57, 0.5), c(0.45, 0.47, 0.5)],
        floor: FloorKind::Carpet([1.9, 1.8, 1.55]),
        fog: c(0.55, 0.54, 0.5),
        ambient: c(0.92, 0.9, 0.84),
        ambient_brightness: 720.0,
        sun: c(0.85, 0.85, 0.82),
        sun_illuminance: 3500.0,
        fog_range: (30.0, 90.0),
    }
}

/// A desk nobody sits at: monitor off, chair pushed back at a slight angle.
fn empty_desk(p: &mut Plan, i: usize, x: f32, z: f32) {
    let top = DESK.size[1] * FURNITURE_SCALE;
    let back = z - DESK.size[2] * FURNITURE_SCALE / 2.0;
    p.prop(&DESK, v(x, 0.0, z), 0.0, true);
    p.prop(&SCREEN, v(x, top, back + 0.2), 0.0, false);
    p.prop(&KEYBOARD, v(x, top, back + 0.62), 0.0, false);
    let skew = [0.35, -0.25, 0.1, -0.4][i % 4];
    p.prop(&CHAIR_MODERN, v(x + 0.1, 0.0, z + 0.95), PI + skew, false);
}

/// A plant that was watered once, in 2021: bare brown stalks, drooping leaves, dry soil.
fn dead_plant(p: &mut Plan, x: f32, z: f32) {
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.25, z),
        v(0.55, 0.5, 0.55),
        Mat::Concrete,
    );
    p.block(v(x, 0.3, z), v(0.55, 0.6, 0.55));
    p.prim(
        PrimKind::Cylinder,
        v(x, 0.5, z),
        v(0.48, 0.04, 0.48),
        Mat::Bark,
    );
    for (k, (dx, dz, h)) in [
        (0.0, 0.0, 1.0),
        (0.12, 0.06, 0.8),
        (-0.1, 0.08, 0.7),
        (0.04, -0.12, 0.6),
    ]
    .into_iter()
    .enumerate()
    {
        p.boxed(
            v(x + dx, 0.5 + h / 2.0, z + dz),
            v(0.03, h, 0.03),
            Mat::Bark,
            false,
        );
        // Brown leaves hang from the tips, and a few lie on the carpet.
        p.boxed(
            v(x + dx + 0.07, 0.5 + h - 0.1 - k as f32 * 0.04, z + dz),
            v(0.14, 0.02, 0.07),
            Mat::Dead,
            false,
        );
        p.boxed(
            v(x + 0.35 * (k as f32 - 1.5), 0.01, z + 0.4),
            v(0.14, 0.02, 0.08),
            Mat::Dead,
            false,
        );
    }
}

pub fn build() -> FloorLayout {
    let mut p = Plan::default();
    let w = Mat::Wall;
    // Lobby, hall and meeting room in a row; the corridors are short so the walk is dull.
    p.fp.pod(rounded_rect(pt(-34.0, 0.0), 8.0, 9.0, 1.0, 0.0), w);
    p.fp.link(stroke(&[pt(-30.0, 0.0), pt(-22.0, 0.0)], 3.0), w);
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 20.0, 16.0, 1.0, 0.0), w);
    p.fp.link(stroke(&[pt(18.0, 0.0), pt(28.0, 0.0)], 3.0), w);
    p.fp.pod(rounded_rect(pt(34.0, 0.0), 8.0, 10.0, 1.0, 0.0), w);
    p.route = vec![pt(-38.0, 0.0), pt(-22.0, 0.0), pt(0.0, 0.0), pt(28.0, 0.0)];
    p.build_walls();

    // Lobby: reception desk, a row of waiting chairs, a dead plant, the printer from 2009.
    p.boxed(v(-34.0, 0.55, -5.5), v(5.0, 1.1, 1.0), Mat::Wood, true);
    p.boxed(v(-34.0, 1.12, -5.5), v(5.2, 0.06, 1.1), Mat::Trim, false);
    p.sign_near(-34.0, -9.5, 2.9, 5.2, Sign::ClientName);
    p.sign_near(-37.5, 9.5, 2.0, 2.2, Sign::ClientQuiet);
    for k in 0..4 {
        p.prop(
            &CHAIR_MODERN,
            v(-37.5 + k as f32 * 1.1, 0.0, 7.6),
            PI,
            false,
        );
    }
    dead_plant(&mut p, -29.5, 7.5);
    dead_plant(&mut p, -39.5, -7.5);
    printer(&mut p, -28.5, -7.6, Face::PosZ);
    p.sign_near(-28.5, -9.5, 2.3, 2.0, Sign::ClientPrinter);

    // Open-plan hall: rows of desks, all free, none used.
    let mut i = 0;
    for z in [-12.0, -7.5, 7.5, 12.0] {
        for x in [-14.4, -10.8, -7.2, -3.6, 3.6, 7.2, 10.8, 14.4] {
            empty_desk(&mut p, i, x, z);
            i += 1;
        }
    }
    dead_plant(&mut p, 17.8, 14.0);
    dead_plant(&mut p, -17.8, -14.0);
    p.sign_near(0.0, -16.5, 2.8, 4.0, Sign::ClientHotDesk);
    p.sign_near(-10.0, 16.5, 2.8, 3.4, Sign::ClientStatus);
    p.sign_near(10.0, 16.5, 2.8, 3.4, Sign::ClientTimesheet);

    // Meeting room: a long table with every chair empty but one, and a screen of nobody.
    long_table(&mut p, 34.0, -2.0, 3, false);
    p.sign_near(42.5, -2.0, 2.4, 3.6, Sign::ClientRoom);
    dead_plant(&mut p, 40.0, 8.0);

    ceiling_lights(&mut p);
    for (x, z) in [
        (-34.0, -3.0),
        (-34.0, 4.0),
        (-12.0, 0.0),
        (-12.0, 11.0),
        (12.0, 0.0),
        (12.0, 11.0),
        (0.0, -9.0),
        (34.0, 0.0),
    ] {
        p.light(v(x, 3.6, z), Color::srgb(0.95, 0.93, 0.85), 380_000.0);
    }

    // The way in is also the way out; it opens once the timesheet is approved.
    let (arrive, yaw) = p.door_near(-42.0, 0.0, Sign::BadgePending);
    p.anchor("door:campus", arrive, yaw);
    let facing = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    // Clear of the door trigger, which reaches 2.4 m from the wall.
    let spawn = arrive + facing * 2.4;
    p.anchor(
        "player_spawn",
        spawn,
        yaw_towards(spawn, v(-22.0, 0.0, 0.0)),
    );
    p.anchor("graveyard", arrive + facing * 4.0, yaw);
    let at = |name: &'static str, p: &mut Plan, x: f32, z: f32, look: (f32, f32)| {
        let a = v(x, 0.0, z);
        p.anchor(name, a, yaw_towards(a, v(look.0, 0.0, look.1)));
    };
    // The receptionist sits behind her desk, the delivery lead at the head of the table, and
    // the three clerks stand in the aisle with nothing to do.
    at("npc_1", &mut p, -34.0, -7.0, (-34.0, 0.0));
    at("npc_2", &mut p, 34.0, 3.2, (34.0, -2.0));
    at("npc_3", &mut p, -11.0, 0.0, (-11.0, 5.0));
    at("npc_4", &mut p, 2.0, 1.5, (-6.0, 1.5));
    at("npc_5", &mut p, 12.0, -1.0, (0.0, 1.0));
    assemble(
        ZONE,
        1,
        "Very Large Insurance & Banking Group",
        false,
        p,
        theme(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floors::testgrid::Grid;
    use crate::floors::zones::{build_zone, door_trigger};

    fn layout() -> FloorLayout {
        build_zone(ZONE).unwrap_or_else(|| panic!("zone {ZONE} missing"))
    }

    #[test]
    fn every_anchor_is_free_inside_the_outline_and_reachable_from_the_spawn() {
        let l = layout();
        let spawn = l.pos("player_spawn");
        let g = Grid::from(&l, spawn);
        for n in [
            "door:campus",
            "graveyard",
            "npc_1",
            "npc_2",
            "npc_3",
            "npc_4",
            "npc_5",
        ] {
            assert!(l.anchor(n).is_some(), "missing {n}");
        }
        for (name, p, _) in &l.anchors {
            assert!(l.footprint.contains(Vec2::new(p.x, p.z)), "{name} outside");
            assert!(g.free_anchor(&l, *p), "{name} inside a blocker");
            assert!(g.at_anchor(&l, *p).is_finite(), "{name} unreachable");
        }
    }

    #[test]
    fn nothing_to_fight_and_the_exit_trigger_leaves_the_spawn_alone() {
        let l = layout();
        assert!(!l.outdoor);
        assert!(
            l.anchors
                .iter()
                .all(|a| !a.0.starts_with("pack_") && a.0 != "boss")
        );
        let t = door_trigger(&l, "door:campus");
        let s = l.pos("player_spawn");
        assert!(!(s.x > t.min.x && s.x < t.max.x && s.z > t.min.z && s.z < t.max.z));
        let g = Grid::from(&l, s);
        let centre = Vec3::new((t.min.x + t.max.x) / 2.0, 0.0, (t.min.z + t.max.z) / 2.0);
        assert!(g.at(centre).is_finite());
    }

    #[test]
    fn the_hall_is_full_of_empty_desks() {
        let desks = layout()
            .pieces
            .iter()
            .filter(|p| matches!(p, Piece::Prop { model, .. } if model.file == DESK.file))
            .count();
        assert_eq!(desks, 32);
    }
}
