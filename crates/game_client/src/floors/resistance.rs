//! "The Booth, Sub-Level": the Resistance base under the Car Park P3 attendant's booth
//! (docs/ILMARI_09.md B.2). One hall with Dale's table, a rack Tom has not left, and a side room
//! where Brenda runs procurement. Two doors: up to the car park (the world as it was) and out to
//! the occupied campus; choosing the door is the whole phase toggle.

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble, yaw_towards};
use super::props::{beanbag, doc_shelf, it_clutter, long_table, server_rack, water_cooler};
use super::shape::{rounded_rect, stroke};
use super::themes::{ceiling_lights, pt, v};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub const ZONE: &str = combat_core::ilmari::BASE_ZONE;

fn theme() -> Theme {
    let c = Color::srgb;
    Theme {
        wall: c(0.52, 0.55, 0.5),
        trim: c(0.2, 0.24, 0.2),
        accent: LinearRgba::rgb(0.5, 1.4, 0.7),
        fabric: [c(0.3, 0.42, 0.34), c(0.45, 0.4, 0.3), c(0.3, 0.36, 0.42)],
        floor: FloorKind::Tile([0.9, 1.0, 0.9]),
        fog: c(0.2, 0.24, 0.2),
        ambient: c(0.85, 0.95, 0.85),
        ambient_brightness: 700.0,
        sun: c(0.8, 0.85, 0.8),
        sun_illuminance: 3000.0,
        fog_range: (30.0, 90.0),
    }
}

pub fn build() -> FloorLayout {
    let mut p = Plan::default();
    let w = Mat::Wall;
    // The hall and, through a short opening, the procurement annex.
    p.fp.pod(rounded_rect(pt(0.0, 0.0), 14.0, 10.0, 1.0, 0.0), w);
    p.fp.link(stroke(&[pt(12.0, 0.0), pt(18.0, 0.0)], 4.0), w);
    p.fp.pod(rounded_rect(pt(26.0, 0.0), 10.0, 7.0, 1.0, 0.0), w);
    p.route = vec![pt(-11.0, 0.0), pt(0.0, 0.0), pt(14.0, 0.0), pt(26.0, 0.0)];
    p.build_walls();
    hall(&mut p);
    annex(&mut p);
    ceiling_lights(&mut p);
    for (x, z) in [
        (-8.0, -5.0),
        (-8.0, 5.0),
        (6.0, -5.0),
        (6.0, 5.0),
        (26.0, -2.0),
        (30.0, 3.0),
    ] {
        p.light(v(x, 3.6, z), Color::srgb(0.85, 1.0, 0.85), 330_000.0);
    }

    // The two exits: the ladder up to P3 on the west wall, the tunnel on the north wall.
    let (up, up_yaw) = p.door_near(-14.0, 0.0, Sign::ResBack);
    p.anchor("door:parking_p3", up, up_yaw);
    let (out, out_yaw) = p.door_near(0.0, -10.0, Sign::ResOut);
    p.anchor("door:campus_occupied", out, out_yaw);
    let facing = Quat::from_rotation_y(up_yaw) * Vec3::NEG_Z;
    let spawn = up + facing * 2.4;
    p.anchor("player_spawn", spawn, yaw_towards(spawn, v(0.0, 0.0, 0.0)));
    p.anchor("graveyard", up + facing * 4.0, up_yaw);
    let at = |name: &'static str, p: &mut Plan, x: f32, z: f32, look: (f32, f32)| {
        let a = v(x, 0.0, z);
        p.anchor(name, a, yaw_towards(a, v(look.0, 0.0, look.1)));
    };
    // Dale at his table, Brenda behind the counter, a volunteer laminating, Tom at the rack.
    at("npc_dale", &mut p, -5.0, 3.5, (-5.0, 0.0));
    at("npc_brenda", &mut p, 26.0, -5.0, (26.0, 0.0));
    at("npc_1", &mut p, 7.0, 6.0, (5.0, 2.0));
    at("npc_2", &mut p, -10.0, -6.0, (-6.0, 0.0));
    // The sign-up sheet for the companions, on the wall side of the hall where the board hangs.
    at("npc_roster", &mut p, 9.0, -6.5, (9.0, 0.0));
    assemble(ZONE, 1, "The Booth, Sub-Level", false, p, theme())
}

/// Dale's folding table and laminator, the laminated wall, a rack and a row of cots.
fn hall(p: &mut Plan) {
    long_table(p, -5.0, 0.0, 2, false);
    p.sign_near(-8.0, -10.0, 2.7, 4.0, Sign::ResMap);
    p.sign_near(9.0, -10.0, 2.6, 3.4, Sign::ResRules);
    p.sign_near(-14.0, 6.0, 2.6, 2.6, Sign::ResDale);
    p.sign_near(14.0, -8.0, 2.4, 3.4, Sign::ResBoard);
    server_rack(p, -12.0, -8.4, 0.0);
    server_rack(p, -10.0, -8.4, 0.0);
    it_clutter(p, -12.5, 8.4);
    water_cooler(p, 12.0, -8.0);
    for (x, z) in [(2.0, 7.5), (4.4, 8.2), (6.8, 7.6), (9.6, 8.2)] {
        beanbag(p, x, z, Mat::BeanA);
    }
    put_crates(p, 11.4, 6.0);
}

/// Brenda's counter, shelves of stamped boxes and the sign that explains the quotes.
fn annex(p: &mut Plan) {
    p.boxed(v(26.0, 0.55, -2.8), v(8.0, 1.1, 1.0), Mat::Wood, true);
    p.boxed(v(26.0, 1.12, -2.8), v(8.2, 0.06, 1.1), Mat::Trim, false);
    p.sign_near(26.0, -7.0, 2.9, 5.0, Sign::ResProcurement);
    doc_shelf(p, 33.0, 6.2, Face::NegZ);
    doc_shelf(p, 20.0, 6.2, Face::NegZ);
    put_crates(p, 34.0, -4.0);
    put_crates(p, 17.8, 4.4);
    // Three identical quotes, pinned to the counter's front: the framework agreement.
    for k in 0..3 {
        p.boxed(
            v(24.0 + k as f32 * 2.0, 0.7, -2.27),
            v(0.5, 0.7, 0.02),
            Mat::Paper,
            false,
        );
    }
    p.prop(&LAMP_FLOOR, v(31.0, 0.0, -5.6), 0.0, false);
    let _ = (FRAC_PI_2, PI);
}

fn put_crates(p: &mut Plan, x: f32, z: f32) {
    use super::models::{BOX_LARGE, put};
    put(p, &BOX_LARGE, v(x, 0.0, z), 0.3, 1.6, true);
    put(p, &BOX_LARGE, v(x + 0.1, 0.88, z), 0.7, 1.6, false);
}
