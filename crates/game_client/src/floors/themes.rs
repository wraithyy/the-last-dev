//! The five floors: palettes, partitions, anchors and per-floor dressing.

use super::kit::*;
use super::layout::{FACE_EAST, FACE_WEST, FloorKind, Theme, yaw_towards};
use super::props::*;
use crate::config::{FLOOR_HALF_X as HX, FLOOR_HALF_Z as HZ, WALL_THICKNESS};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Where things stand; every floor starts at the west end and ends in the boss room in the east.
struct Spec {
    packs: [(f32, f32); 6],
    boss: (f32, f32),
    elevator_z: f32,
    exit_z: f32,
    door: Mat,
}

fn theme(id: u8) -> Theme {
    let c = Color::srgb;
    match id {
        1 => Theme {
            wall: c(0.56, 0.55, 0.6),
            trim: c(0.12, 0.13, 0.17),
            accent: LinearRgba::rgb(0.1, 0.9, 1.4),
            fabric: [c(0.25, 0.4, 0.55), c(0.35, 0.33, 0.5), c(0.22, 0.45, 0.42)],
            floor: FloorKind::Carpet([0.9, 0.95, 1.15]),
            fog: c(0.03, 0.035, 0.06),
            ambient: c(0.7, 0.78, 1.0),
            ambient_brightness: 450.0,
            sun: c(1.0, 0.95, 0.88),
            sun_illuminance: 8000.0,
        },
        2 => Theme {
            wall: c(0.72, 0.75, 0.76),
            trim: c(0.08, 0.2, 0.22),
            accent: LinearRgba::rgb(0.2, 1.4, 0.9),
            fabric: [c(0.2, 0.5, 0.5), c(0.3, 0.45, 0.4), c(0.25, 0.4, 0.55)],
            floor: FloorKind::Carpet([0.75, 1.0, 1.0]),
            fog: c(0.03, 0.05, 0.055),
            ambient: c(0.78, 0.92, 0.95),
            ambient_brightness: 480.0,
            sun: c(0.95, 1.0, 0.95),
            sun_illuminance: 7500.0,
        },
        3 => Theme {
            wall: c(0.72, 0.52, 0.46),
            trim: c(0.22, 0.07, 0.08),
            accent: LinearRgba::rgb(1.8, 0.5, 0.12),
            fabric: [c(0.7, 0.25, 0.22), c(0.75, 0.55, 0.2), c(0.5, 0.2, 0.3)],
            floor: FloorKind::Carpet([1.25, 0.8, 0.75]),
            fog: c(0.06, 0.03, 0.03),
            ambient: c(1.0, 0.85, 0.75),
            ambient_brightness: 470.0,
            sun: c(1.0, 0.9, 0.75),
            sun_illuminance: 8000.0,
        },
        4 => Theme {
            wall: c(0.55, 0.6, 0.55),
            trim: c(0.14, 0.17, 0.14),
            accent: LinearRgba::rgb(1.5, 1.0, 0.15),
            fabric: [c(0.4, 0.45, 0.35), c(0.5, 0.5, 0.4), c(0.35, 0.4, 0.45)],
            floor: FloorKind::Carpet([0.95, 1.0, 0.8]),
            fog: c(0.04, 0.05, 0.045),
            ambient: c(0.85, 0.9, 0.8),
            ambient_brightness: 430.0,
            sun: c(0.95, 1.0, 0.85),
            sun_illuminance: 7500.0,
        },
        _ => Theme {
            wall: c(0.3, 0.27, 0.25),
            trim: c(0.5, 0.38, 0.13),
            accent: LinearRgba::rgb(2.2, 1.5, 0.3),
            fabric: [c(0.3, 0.1, 0.15), c(0.15, 0.1, 0.3), c(0.1, 0.25, 0.2)],
            floor: FloorKind::Marble,
            fog: c(0.04, 0.03, 0.05),
            ambient: c(1.0, 0.9, 0.8),
            ambient_brightness: 380.0,
            sun: c(1.0, 0.9, 0.78),
            sun_illuminance: 7000.0,
        },
    }
}

fn spec(id: u8) -> Spec {
    match id {
        1 => Spec {
            packs: [
                (-22.0, -9.0),
                (-16.0, 9.0),
                (-6.0, -9.0),
                (2.0, 9.0),
                (12.0, -9.0),
                (20.0, 9.0),
            ],
            boss: (34.0, 2.0),
            elevator_z: -6.0,
            exit_z: -8.0,
            door: Mat::Door,
        },
        2 => Spec {
            packs: [
                (-20.0, 0.0),
                (-14.0, -13.5),
                (-2.0, 13.5),
                (6.0, -13.5),
                (20.0, 13.5),
                (23.0, -14.0),
            ],
            boss: (34.0, 2.0),
            elevator_z: -6.0,
            exit_z: -8.0,
            door: Mat::Door,
        },
        3 => Spec {
            packs: [
                (-24.0, -11.0),
                (-12.0, 9.0),
                (2.0, 13.0),
                (8.0, -12.0),
                (24.0, -12.0),
                (21.0, 14.0),
            ],
            boss: (34.0, 2.0),
            elevator_z: -6.0,
            exit_z: -8.0,
            door: Mat::Door,
        },
        4 => Spec {
            packs: [
                (-22.0, -10.0),
                (-16.0, 11.0),
                (-3.0, -14.0),
                (8.0, 13.0),
                (16.0, -12.0),
                (24.0, 14.0),
            ],
            boss: (34.0, 2.0),
            elevator_z: -6.0,
            exit_z: -8.0,
            door: Mat::Door,
        },
        _ => Spec {
            packs: [
                (-23.0, -10.0),
                (-18.0, 10.0),
                (-8.0, -10.0),
                (0.0, 10.0),
                (9.0, -10.0),
                (18.0, 10.0),
            ],
            boss: (33.0, 0.0),
            elevator_z: -6.0,
            exit_z: -9.0,
            door: Mat::Gold,
        },
    }
}

/// Fills `plan` for floor `id` and returns the palette.
pub fn build(id: u8, plan: &mut Plan) -> Theme {
    let s = spec(id);
    shell(plan, &s);
    match id {
        1 => floor1(plan),
        2 => floor2(plan),
        3 => floor3(plan),
        4 => floor4(plan),
        _ => floor5(plan),
    }
    theme(id)
}

/// Outer walls, trim, ceiling lights, elevators and the anchors every floor shares.
fn shell(p: &mut Plan, s: &Spec) {
    let t = WALL_THICKNESS;
    let (span_x, span_z) = (HX * 2.0, HZ * 2.0);
    for sgn in [-1.0, 1.0] {
        p.boxed(
            v(0.0, H / 2.0, sgn * (HZ + t / 2.0)),
            v(span_x + t * 2.0, H, t),
            Mat::Wall,
            true,
        );
        p.boxed(
            v(sgn * (HX + t / 2.0), H / 2.0, 0.0),
            v(t, H, span_z + t * 2.0),
            Mat::Wall,
            true,
        );
        // Dark baseboards and a glowing accent band round the room.
        for (h, y, mat) in [(0.2, 0.1, Mat::Trim), (0.06, 2.9, Mat::Accent)] {
            p.boxed(v(0.0, y, sgn * (HZ - 0.04)), v(span_x, h, 0.06), mat, false);
            p.boxed(v(sgn * (HX - 0.04), y, 0.0), v(0.06, h, span_z), mat, false);
        }
    }
    for xi in -4..=4 {
        for z in [-16.0, -8.0, 0.0, 8.0, 16.0] {
            p.boxed(
                v(xi as f32 * 8.0, H - 0.02, z),
                v(1.2, 0.04, 0.6),
                Mat::LightPanel,
                false,
            );
        }
    }

    let arrive = p.elevator(-HX, s.elevator_z, Face::PosX, s.door);
    p.anchor("elevator", v(arrive.x, 0.0, arrive.z), FACE_EAST);
    let exit = p.elevator(HX, s.exit_z, Face::NegX, s.door);
    p.anchor("elevator_exit", v(exit.x, 0.0, exit.z), FACE_WEST);

    let spawn = v(-35.0, 0.0, 0.0);
    p.anchor("player_spawn", spawn, FACE_EAST);
    let names = ["pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "pack_6"];
    for (name, (x, z)) in names.iter().zip(s.packs) {
        p.anchor(name, v(x, 0.0, z), FACE_WEST);
    }
    p.anchor("boss", v(s.boss.0, 0.0, s.boss.1), FACE_WEST);
    let npc = v(-32.0, 0.0, 5.0);
    p.anchor("npc_1", npc, yaw_towards(npc, spawn));
}

/// Boss room at the far end: a wall with a door gap and side walls.
fn boss_room(p: &mut Plan, x0: f32, hz: f32, door: f32, mat: Mat) {
    p.wall_x(x0, -hz, hz, &[(-door, door)], mat, H);
    p.wall_z(-hz, x0, HX, &[], mat, H);
    p.wall_z(hz, x0, HX, &[], mat, H);
}

fn cool() -> Color {
    Color::srgb(0.4, 0.7, 1.0)
}

fn lights(p: &mut Plan, list: &[(f32, f32, Color)]) {
    for &(x, z, c) in list {
        p.light(v(x, 3.4, z), c, 500_000.0);
    }
}

// ---------------------------------------------------------------- floor 1

fn floor1(p: &mut Plan) {
    boss_room(p, 28.0, 12.0, 4.0, Mat::Wall);

    // Cubicle banks along both long walls, open to the middle.
    let south = [-27.0, -23.5, -20.0, -8.0, -4.5, -1.0, 12.0, 15.5, 19.0];
    let north = [-24.0, -20.5, -17.0, -4.0, -0.5, 3.0, 14.0, 17.5, 21.0];
    for (i, x) in south.into_iter().enumerate() {
        cubicle(p, i, x, -16.5, 0.0);
    }
    for (i, x) in north.into_iter().enumerate() {
        cubicle(p, i + 1, x, 16.5, PI);
    }

    // Corridor runner with glowing edges, plus decorative cables.
    p.boxed(v(-6.0, 0.01, 0.0), v(60.0, 0.02, 4.0), Mat::Runner, false);
    for s in [-1.0, 1.0] {
        p.boxed(
            v(-6.0, 0.02, s * 2.0),
            v(60.0, 0.03, 0.08),
            Mat::RunnerEdge,
            false,
        );
    }
    p.boxed(v(0.0, 0.035, 1.0), v(40.0, 0.03, 0.08), Mat::Cable, false);

    // The middle of the floor: games nobody has time for.
    ping_pong(p, -18.0, 0.0);
    foosball(p, -2.0, -0.2);
    beanbag(p, -10.0, 3.0, Mat::BeanA);
    beanbag(p, -8.4, 0.6, Mat::BeanB);
    beanbag(p, -11.2, -1.6, Mat::BeanC);
    lounge(p, 10.0, 3.2, FRAC_PI_2, true);
    sad_plant(p, 24.0, 3.0);
    sad_plant(p, -28.5, -4.5);
    sad_plant(p, 1.0, -4.5);

    vending(p, -36.0, -17.5, Face::PosZ);
    water_cooler(p, -30.0, -18.8);
    p.prop(&FRIDGE, v(-33.0, 0.0, -19.0), 0.0, true);
    p.prop(
        &COFFEE_MACHINE,
        v(-31.4, SIDE_TABLE.size[1] * FURNITURE_SCALE, -19.0),
        0.0,
        false,
    );
    p.prop(&SIDE_TABLE, v(-31.4, 0.0, -19.2), 0.0, true);

    // Satire on the walls: north wall looks -Z, south wall +Z.
    let wall_n = HZ - 0.03;
    p.poster(-12.0, wall_n, 2.4, Face::NegZ, v2(2.0, 1.33), Tex::AiFirst);
    p.poster(
        8.0,
        wall_n,
        2.4,
        Face::NegZ,
        v2(2.4, 1.15),
        Tex::AiTransformation,
    );
    p.poster(-30.0, wall_n, 2.4, Face::NegZ, v2(1.2, 1.5), Tex::Synergy);
    p.poster(27.0, wall_n, 2.4, Face::NegZ, v2(1.2, 1.5), Tex::PosterBars);
    p.poster(
        -12.0,
        -wall_n,
        2.4,
        Face::PosZ,
        v2(1.2, 1.5),
        Tex::PosterBurn,
    );
    p.poster(0.0, -wall_n, 2.4, Face::PosZ, v2(1.2, 1.5), Tex::PosterHang);
    p.poster(10.0, -wall_n, 2.4, Face::PosZ, v2(1.2, 1.5), Tex::PosterPie);
    p.poster(24.0, -wall_n, 2.4, Face::PosZ, v2(1.2, 1.5), Tex::Synergy);
    p.wall_panel(
        v(-4.0, 3.6, -wall_n),
        Face::PosZ,
        v2(6.0, 1.1),
        Tex::MandatoryFun,
    );
    whiteboard(p, -39.8, 12.0, Face::PosX, Tex::Roadmap);
    whiteboard(p, -39.8, -12.0, Face::PosX, Tex::Flowchart);
    // Above the boss door, seen from the corridor.
    p.wall_panel(
        v(27.85, 3.4, 0.0),
        Face::NegX,
        v2(2.4, 0.96),
        Tex::DaysCounter,
    );

    it_corner(p);

    // The boss room: server racks and a GPU shrine, like every start-up eventually.
    for x in [30.5, 33.5, 36.5] {
        server_rack(p, x, -11.1, 0.0);
        server_rack(p, x, 11.1, PI);
    }
    gpu_shrine(p, 37.5, 5.5, Face::NegX);
    p.poster(
        33.5,
        11.7,
        3.1,
        Face::NegZ,
        v2(2.2, 1.05),
        Tex::AiTransformation,
    );
    p.boxed(v(34.0, 0.01, 0.0), v(11.0, 0.02, 3.0), Mat::Runner, false);
    gpu_shrine(p, 33.0, -16.0, Face::PosX);

    lights(
        p,
        &[
            (-30.0, -8.0, cool()),
            (-30.0, 8.0, cool()),
            (-16.0, 0.0, cool()),
            (-2.0, -8.0, cool()),
            (-2.0, 8.0, cool()),
            (12.0, -8.0, cool()),
            (12.0, 8.0, cool()),
            (24.0, 0.0, cool()),
            (34.0, 0.0, Color::srgb(0.8, 0.4, 1.0)),
            (-34.0, 15.0, Color::srgb(1.0, 0.85, 0.6)),
        ],
    );
}

fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// IT corner in the north-west: two desks pushed together, cable spaghetti, a
/// tower of old monitors and a sign promising help in 6-8 weeks.
fn it_corner(p: &mut Plan) {
    p.framed(v(-34.0, 0.0, 18.0), PI, |p| {
        for dx in [-0.9, 0.9] {
            p.prop(&DESK, v(dx, 0.0, -1.0), 0.0, true);
        }
        p.prop(
            &LAPTOP,
            v(-0.6, DESK.size[1] * FURNITURE_SCALE, -1.1),
            0.0,
            false,
        );
        p.prop(&CHAIR_DESK, v(-0.4, 0.0, 0.1), PI + 0.5, false);
        p.prop(
            &KEYBOARD,
            v(0.9, DESK.size[1] * FURNITURE_SCALE, -0.75),
            0.0,
            false,
        );
        // Headset left on the desk.
        p.boxed(v(1.5, 0.95, -0.7), v(0.2, 0.05, 0.12), Mat::Dark, false);
    });
    it_clutter(p, -38.2, 13.0);
    mini_fridge(p, -30.5, 19.0, Face::NegZ);
    // Posters on the north wall and the sign above the desks.
    p.poster(
        -33.0,
        HZ - 0.03,
        2.5,
        Face::NegZ,
        v2(1.9, 0.95),
        Tex::TurnItOff,
    );
    p.poster(
        -36.5,
        HZ - 0.03,
        2.2,
        Face::NegZ,
        v2(1.9, 0.63),
        Tex::NextAgent,
    );
    // Sleeping IT support (npc_2) at the desk, a colleague (npc_3) chatting to them.
    let sleeper = v(-34.4, 0.0, 17.4);
    p.anchor("npc_2", sleeper, PI);
    let talker = v(-32.2, 0.0, 15.2);
    p.anchor("npc_3", talker, yaw_towards(talker, sleeper));
}

// ---------------------------------------------------------------- floor 2

fn floor2(p: &mut Plan) {
    boss_room(p, 28.0, 12.0, 4.0, Mat::Wall);

    // Glass boxes along the corridor; HR gets opaque beige walls (placed after).
    let rooms_s = [(-20.0, -8.0), (0.0, 12.0)];
    let rooms_n = [(-8.0, 4.0), (14.0, 26.0)];
    for (i, (x0, x1)) in rooms_s.into_iter().enumerate() {
        let cx = (x0 + x1) / 2.0;
        room_walls(p, x0, x1, -1.0, (cx - 1.5, cx + 1.5), Mat::Glass, H);
        meeting_room(p, cx, -1.0, i);
    }
    for (i, (x0, x1)) in rooms_n.into_iter().enumerate() {
        let cx = (x0 + x1) / 2.0;
        room_walls(p, x0, x1, 1.0, (cx - 1.5, cx + 1.5), Mat::Glass, H);
        meeting_room(p, cx, 1.0, i + 2);
    }
    hr_room(p);

    // Stand-up area by the lobby: yellow tape rectangle, sign, plants.
    for (cx, cz, w, d) in [
        (-30.5, 0.0, 6.0, 0.08),
        (-30.5, 5.0, 6.0, 0.08),
        (-33.5, 2.5, 0.08, 5.0),
        (-27.5, 2.5, 0.08, 5.0),
    ] {
        p.boxed(v(cx, 0.015, cz), v(w, 0.03, d), Mat::NeonYellow, false);
    }
    p.wall_panel(v(-39.85, 2.6, 4.0), Face::PosX, v2(2.8, 0.93), Tex::Standup);
    whiteboard(p, -39.8, -12.0, Face::PosX, Tex::Roadmap);
    sad_plant(p, -37.5, 14.0);
    water_cooler(p, -22.0, 3.0);
    p.prop(&POTTED_PLANT, v(-37.0, 0.0, -18.0), 0.0, true);

    // Corridor posters.
    p.wall_panel(
        v(27.85, 3.4, 0.0),
        Face::NegX,
        v2(2.4, 0.96),
        Tex::DaysCounter,
    );
    p.poster(8.0, HZ - 0.03, 2.4, Face::NegZ, v2(2.0, 1.33), Tex::AiFirst);
    p.poster(
        30.0,
        -HZ + 0.03,
        2.4,
        Face::PosZ,
        v2(1.2, 1.5),
        Tex::Synergy,
    );

    // Boss room: the Analyst's war table, written by Claude.
    long_table(p, 34.0, -7.0, 2, true);
    p.wall_panel(v(34.0, 2.4, 11.85), Face::NegZ, v2(3.0, 1.5), Tex::Kpi);
    p.poster(36.5, -11.85, 2.4, Face::PosZ, v2(1.2, 1.5), Tex::PosterPie);
    p.boxed(v(34.0, 0.01, 0.0), v(11.0, 0.02, 3.0), Mat::Runner, false);

    let mint = Color::srgb(0.6, 1.0, 0.85);
    lights(
        p,
        &[
            (-30.0, 0.0, mint),
            (-14.0, -13.5, mint),
            (-2.0, 13.5, mint),
            (6.0, -13.5, mint),
            (12.0, 0.0, mint),
            (20.0, 13.5, mint),
            (23.0, -14.0, Color::srgb(1.0, 0.85, 0.6)),
            (34.0, 0.0, mint),
        ],
    );
}

/// Glass meeting room contents: long table at the back wall, a board, chairs.
fn meeting_room(p: &mut Plan, cx: f32, side: f32, i: usize) {
    let (table_z, face) = if side > 0.0 {
        (17.6, Face::NegZ)
    } else {
        (-17.6, Face::PosZ)
    };
    long_table(p, cx - 1.0, table_z, 3, i.is_multiple_of(2));
    let wall = side * (HZ - 0.03);
    let tex = if i.is_multiple_of(2) {
        Tex::Flowchart
    } else {
        Tex::Stickies
    };
    p.poster(cx + 3.5, wall, 2.2, face, v2(2.4, 1.2), tex);
    sad_plant(p, cx - 5.0, side * 18.8);
    p.prop(&LAMP_ROUND_FLOOR, v(cx + 5.2, 0.0, side * 18.8), 0.0, false);
}

/// HR room (pack_6): beige walls, wellness corner, pizza box, chatbot screen.
fn hr_room(p: &mut Plan) {
    let (x0, x1) = (17.0, 27.5);
    room_walls(p, x0, x1, -1.0, (21.0, 24.0), Mat::Beige, H);
    p.wall_panel(v(22.5, 2.9, -7.85), Face::PosZ, v2(2.4, 0.4), Tex::HrDoor);
    sad_plant(p, 18.4, -18.6);
    // Wellness corner: one yoga mat and a sign.
    p.boxed(v(19.6, 0.02, -15.4), v(0.7, 0.04, 1.9), Mat::Yoga, false);
    p.wall_panel(
        v(17.15, 2.4, -15.0),
        Face::PosX,
        v2(2.4, 0.4),
        Tex::Wellness,
    );
    // Pizza Friday.
    p.prop(&SIDE_TABLE, v(26.4, 0.0, -18.8), 0.0, true);
    p.boxed(
        v(26.4, 0.95, -18.8),
        v(0.7, 0.08, 0.7),
        Mat::Cardboard,
        false,
    );
    p.wall_panel(
        v(27.35, 2.3, -15.5),
        Face::NegX,
        v2(1.8, 1.2),
        Tex::PizzaBox,
    );
    // Big chatbot screen on the back wall and a family poster.
    p.wall_panel(
        v(22.5, 2.3, -19.97),
        Face::PosZ,
        v2(3.0, 2.25),
        Tex::ChatbotFace,
    );
    p.wall_panel(v(17.15, 2.3, -18.5), Face::PosX, v2(1.0, 1.25), Tex::Family);
    // Suggestion box, chained shut.
    p.boxed(v(19.0, 0.5, -9.6), v(0.3, 1.0, 0.3), Mat::Dark, true);
    p.boxed(v(19.0, 1.2, -9.6), v(0.6, 0.5, 0.45), Mat::Metal, false);
    p.wall_panel(
        v(19.0, 1.2, -9.36),
        Face::PosZ,
        v2(0.5, 0.11),
        Tex::Suggestions,
    );
    for dy in [0.0, 0.08] {
        p.boxed(
            v(19.0, 1.2 + dy, -9.35),
            v(0.7, 0.03, 0.03),
            Mat::Dark,
            false,
        );
    }
    p.boxed(v(19.0, 1.35, -9.34), v(0.08, 0.1, 0.04), Mat::Gold, false);
    p.light(v(22.5, 3.2, -14.0), Color::srgb(1.0, 0.85, 0.65), 300_000.0);
}

// ---------------------------------------------------------------- floor 3

fn floor3(p: &mut Plan) {
    boss_room(p, 28.0, 12.0, 4.0, Mat::Wall);
    // Serpentine: the first wall opens in the north, the second in the south.
    p.wall_x(-6.0, -20.0, 10.0, &[], Mat::Wall, H);
    p.wall_x(14.0, -10.0, 20.0, &[], Mat::Wall, H);

    // Lane A: demo stands, trophies, couches.
    demo_stand(p, -30.0, -6.0, Face::NegX);
    demo_stand(p, -30.0, 12.0, Face::NegX);
    demo_stand(p, -18.0, -2.0, Face::NegX);
    trophy_shelf(p, -22.0, 19.3, Face::NegZ);
    lounge(p, -22.0, -17.0, 0.0, false);
    p.prop(&LOUNGE_SOFA_LONG, v(-12.0, 0.0, -17.5), 0.0, true);
    p.prop(&COFFEE_TABLE_GLASS, v(-12.0, 0.0, -15.0), 0.0, true);
    p.prop(&LOUNGE_SOFA_LONG, v(-36.0, 0.0, 15.0), FRAC_PI_2, true);
    neon(
        p,
        -6.15,
        3.3,
        -2.0,
        Face::NegX,
        v2(4.8, 1.2),
        Tex::CloseDeals,
    );
    p.wall_panel(
        v(-39.85, 2.4, 10.0),
        Face::PosX,
        v2(1.8, 2.4),
        Tex::Leaderboard,
    );
    p.poster(
        -12.0,
        -HZ + 0.03,
        2.4,
        Face::PosZ,
        v2(1.2, 1.5),
        Tex::PosterBars,
    );
    p.poster(
        -30.0,
        HZ - 0.03,
        2.4,
        Face::NegZ,
        v2(2.4, 1.15),
        Tex::AiTransformation,
    );
    sad_plant(p, -38.0, -17.0);

    // Lane B: gong, couches and more charts going up.
    gong(p, 4.0, 0.0);
    p.prop(&LOUNGE_DESIGN_SOFA, v(10.0, 0.0, 4.0), -FRAC_PI_2, true);
    p.prop(&LOUNGE_DESIGN_SOFA, v(-2.0, 0.0, -6.0), FRAC_PI_2, true);
    p.wall_panel(
        v(13.85, 2.4, 6.0),
        Face::NegX,
        v2(2.2, 1.7),
        Tex::Leaderboard,
    );
    p.wall_panel(
        v(-5.85, 3.0, 14.0),
        Face::PosX,
        v2(3.0, 0.97),
        Tex::DaysCounter,
    );
    p.poster(0.0, HZ - 0.03, 2.4, Face::NegZ, v2(2.0, 1.33), Tex::AiFirst);
    p.poster(4.0, -HZ + 0.03, 2.4, Face::PosZ, v2(1.2, 1.5), Tex::Synergy);
    demo_stand(p, 8.0, 5.0, Face::NegX);

    // Lane C and the marketing corner at its north end.
    demo_stand(p, 22.0, -2.0, Face::NegX);
    p.poster(
        20.0,
        -HZ + 0.03,
        2.4,
        Face::PosZ,
        v2(1.2, 1.5),
        Tex::PosterPie,
    );
    p.wall_panel(
        v(27.85, 3.4, 0.0),
        Face::NegX,
        v2(2.4, 0.96),
        Tex::DaysCounter,
    );
    marketing_corner(p);

    // Boss room: the Customer, with a pile of unsigned contracts.
    long_table(p, 34.0, -8.0, 2, false);
    p.wall_panel(v(34.0, 2.4, 11.85), Face::NegZ, v2(3.0, 1.5), Tex::Roadmap);
    p.boxed(v(34.0, 0.01, 0.0), v(11.0, 0.02, 3.0), Mat::Runner, false);

    let warm = Color::srgb(1.0, 0.7, 0.45);
    lights(
        p,
        &[
            (-30.0, 0.0, warm),
            (-20.0, 10.0, warm),
            (-20.0, -10.0, warm),
            (-0.0, 8.0, warm),
            (6.0, -8.0, warm),
            (22.0, -8.0, warm),
            (21.0, 14.0, Color::srgb(1.0, 0.4, 0.9)),
            (34.0, 0.0, warm),
        ],
    );
}

/// Marketing corner (pack_6, the Head of Marketing): neon BRAND, mood boards, ring light, tote bags.
fn marketing_corner(p: &mut Plan) {
    neon(
        p,
        21.0,
        3.5,
        HZ - 0.03,
        Face::NegZ,
        v2(3.9, 1.3),
        Tex::Brand,
    );
    p.wall_panel(
        v(25.6, 1.9, HZ - 0.03),
        Face::NegZ,
        v2(3.0, 2.0),
        Tex::MoodBoards,
    );
    p.wall_panel(
        v(17.0, 1.9, HZ - 0.03),
        Face::NegZ,
        v2(3.0, 2.0),
        Tex::MoodBoards,
    );
    p.wall_panel(v(14.15, 2.2, 16.0), Face::PosX, v2(3.0, 1.7), Tex::Campaign);
    // Ring light on a tripod.
    p.framed(v(26.0, 0.0, 17.5), PI, |p| {
        for (dx, dz) in [(-0.4, 0.3), (0.4, 0.3), (0.0, -0.4)] {
            p.boxed(
                v(dx * 0.6, 0.55, dz * 0.6),
                v(0.05, 1.1, 0.05),
                Mat::Dark,
                false,
            );
        }
        p.boxed(v(0.0, 1.4, 0.0), v(0.06, 0.6, 0.06), Mat::Dark, false);
        for (y, w, h) in [(1.95, 0.7, 0.07), (1.35, 0.7, 0.07), (1.65, 0.07, 0.7)] {
            p.boxed(v(0.0, y, 0.0), v(w, h, 0.05), Mat::LightPanel, false);
        }
        p.boxed(
            v(-0.31, 1.65, 0.0),
            v(0.07, 0.7, 0.05),
            Mat::LightPanel,
            false,
        );
        p.boxed(
            v(0.31, 1.65, 0.0),
            v(0.07, 0.7, 0.05),
            Mat::LightPanel,
            false,
        );
    });
    // Branded tote bags, stacked.
    for (i, (dx, dz, w)) in [
        (0.0, 0.0, 0.55),
        (0.35, 0.1, 0.5),
        (-0.1, 0.45, 0.5),
        (0.2, 0.05, 0.45),
    ]
    .into_iter()
    .enumerate()
    {
        p.boxed(
            v(15.7 + dx, 0.15 + (i / 2) as f32 * 0.32, 17.0 + dz),
            v(w, 0.3, 0.12),
            [Mat::Fabric0, Mat::Fabric1, Mat::Yellow, Mat::NeonPink][i],
            false,
        );
    }
    p.boxed(v(15.9, 0.35, 17.2), v(0.7, 0.7, 0.5), Mat::Cardboard, true);
    // The beanbag nobody sits on, now collecting dust.
    beanbag(p, 26.2, 12.4, Mat::BeanC);
    p.panel(v(25.6, 0.9, 13.0), 0.0, v2(0.9, 0.9), Tex::Cobweb);
}

// ---------------------------------------------------------------- floor 4

fn floor4(p: &mut Plan) {
    boss_room(p, 28.0, 12.0, 4.0, Mat::Wall);

    // War room in the middle of the floor, doors west and east.
    p.wall_x(-8.0, -7.0, 7.0, &[(-1.5, 1.5)], Mat::Wall, H);
    p.wall_x(8.0, -7.0, 7.0, &[(-1.5, 1.5)], Mat::Wall, H);
    p.wall_z(-7.0, -8.0, 8.0, &[], Mat::Wall, H);
    p.wall_z(7.0, -8.0, 8.0, &[], Mat::Wall, H);
    p.wall_panel(v(-8.15, 3.2, 0.0), Face::NegX, v2(2.8, 0.47), Tex::WarRoom);
    long_table(p, 0.0, 0.0, 4, true);
    p.wall_panel(v(0.0, 2.4, 6.85), Face::NegZ, v2(3.5, 1.75), Tex::Gantt);
    p.wall_panel(v(0.0, 2.4, -6.85), Face::PosZ, v2(3.5, 1.75), Tex::Gantt);
    p.light(v(0.0, 3.4, 0.0), Color::srgb(1.0, 0.9, 0.6), 400_000.0);

    // Gantt walls: every bar red.
    for x in [-30.0, -18.0, -6.0, 6.0, 18.0, 30.0] {
        p.poster(x, HZ - 0.03, 2.3, Face::NegZ, v2(4.5, 2.25), Tex::Gantt);
        p.poster(
            x + 3.0,
            -HZ + 0.03,
            2.3,
            Face::PosZ,
            v2(4.5, 2.25),
            Tex::Gantt,
        );
    }
    p.wall_panel(v(-39.85, 2.4, 9.0), Face::PosX, v2(3.0, 1.5), Tex::Roadmap);
    p.wall_panel(
        v(27.85, 3.4, 0.0),
        Face::NegX,
        v2(2.4, 0.96),
        Tex::DaysCounter,
    );

    // Printers, all broken, and a consultant lounge.
    for (i, x) in [-26.0, -23.8, -21.6].into_iter().enumerate() {
        printer(p, x, 19.3, Face::NegZ);
        if i == 1 {
            p.boxed(v(x, 1.2, 19.0), v(0.3, 0.04, 0.3), Mat::Paper, false);
        }
    }
    lounge(p, -31.0, -17.0, 0.0, true);
    p.prop(&RUG_ROUND, v(-24.0, 0.0, -17.0), 0.0, false);
    p.prop(&LOUNGE_CHAIR_RELAX, v(-27.0, 0.0, -17.5), -FRAC_PI_2, true);
    sad_plant(p, -38.0, 17.0);
    water_cooler(p, 12.0, 19.0);
    p.prop(
        &COFFEE_MACHINE,
        v(14.0, SIDE_TABLE.size[1] * FURNITURE_SCALE, 19.2),
        0.0,
        false,
    );
    p.prop(&SIDE_TABLE, v(14.0, 0.0, 19.3), 0.0, true);
    doc_shelf(p, 4.0, -19.3, Face::PosZ);
    soc_room(p);
    // Friendly IT support stands near the SOC door, waiting for a ticket.
    let it = v(22.0, 0.0, 4.5);
    p.anchor("npc_2", it, yaw_towards(it, v(22.5, 0.0, 9.0)));

    // Boss room: the VP's synergy dashboard.
    long_table(p, 34.0, -7.0, 2, true);
    p.wall_panel(v(34.0, 2.4, 11.85), Face::NegZ, v2(3.5, 1.75), Tex::Gantt);
    p.boxed(v(34.0, 0.01, 0.0), v(11.0, 0.02, 3.0), Mat::Runner, false);

    let amber = Color::srgb(1.0, 0.9, 0.6);
    lights(
        p,
        &[
            (-30.0, 0.0, amber),
            (-16.0, 11.0, amber),
            (-3.0, -14.0, amber),
            (8.0, 13.0, amber),
            (16.0, -12.0, amber),
            (24.0, 6.0, amber),
            (24.0, 14.0, Color::srgb(1.0, 0.3, 0.3)),
            (34.0, 0.0, amber),
        ],
    );
}

/// Security Operations Center (pack_6, the CISO): red "!" monitors, a password
/// on a monitor, a shredder next to unshredded paper, an open padlock, a fishing rod.
fn soc_room(p: &mut Plan) {
    let (x0, x1) = (17.0, 27.5);
    p.wall_z(9.0, x0, x1, &[(21.0, 24.0)], Mat::Wall, H);
    p.wall_x(x0, 9.0, 20.0, &[], Mat::Wall, H);
    p.wall_x(x1, 9.0, 20.0, &[], Mat::Wall, H);
    p.wall_panel(v(22.5, 3.0, 8.85), Face::NegZ, v2(2.4, 0.4), Tex::WarRoom);
    // Padlock, open, next to the door.
    p.boxed(v(20.7, 1.3, 8.8), v(0.22, 0.2, 0.08), Mat::Gold, false);
    p.boxed(v(20.7, 1.5, 8.8), v(0.14, 0.12, 0.05), Mat::Metal, false);
    p.boxed(v(20.78, 1.58, 8.8), v(0.03, 0.12, 0.05), Mat::Metal, false);
    // Monitor wall: every screen shows the same alert.
    for i in 0..6 {
        let x = 18.4 + i as f32 * 1.7;
        p.boxed(v(x, 2.4, HZ - 0.08), v(1.6, 1.6, 0.1), Mat::Dark, false);
        p.wall_panel(
            v(x, 2.4, HZ - 0.12),
            Face::NegZ,
            v2(1.45, 1.45),
            Tex::SocAlert,
        );
    }
    p.panel(v(21.8, 1.7, HZ - 0.2), 0.0, v2(0.3, 0.3), Tex::StickyPass);
    // Console desk along the monitors.
    p.boxed(v(22.5, 0.4, 18.9), v(9.0, 0.8, 0.9), Mat::Dark, true);
    // Shredder next to a pile of unshredded paper.
    p.boxed(v(19.0, 0.45, 12.0), v(0.6, 0.9, 0.4), Mat::Metal, true);
    p.boxed(v(19.0, 0.92, 12.0), v(0.5, 0.04, 0.05), Mat::Dark, false);
    for (dx, dz, h) in [(0.0, 0.0, 0.5), (0.35, 0.2, 0.35), (-0.3, 0.15, 0.25)] {
        p.boxed(
            v(20.2 + dx, h / 2.0, 12.2 + dz),
            v(0.6, h, 0.45),
            Mat::Paper,
            true,
        );
    }
    // Phishing results poster, and a fishing rod in the corner (a visual pun).
    p.wall_panel(
        v(27.35, 2.3, 15.0),
        Face::NegX,
        v2(1.6, 1.28),
        Tex::Phishing,
    );
    p.boxed(v(17.4, 1.1, 10.0), v(0.04, 2.2, 0.04), Mat::Wood, false);
    p.boxed(v(17.6, 2.15, 10.0), v(0.4, 0.03, 0.03), Mat::Wood, false);
    p.boxed(v(17.78, 1.7, 10.0), v(0.01, 0.9, 0.01), Mat::White, false);
    p.boxed(v(17.78, 1.2, 10.0), v(0.05, 0.06, 0.02), Mat::Metal, false);
    p.light(v(22.5, 3.2, 14.0), Color::srgb(1.0, 0.25, 0.25), 300_000.0);
}

// ---------------------------------------------------------------- floor 5

fn floor5(p: &mut Plan) {
    boss_room(p, 24.0, 14.0, 4.0, Mat::Wall);

    // Colonnade down both sides of the hall, executive desks in the alcoves.
    for i in 0..8 {
        let x = -30.0 + i as f32 * 8.0;
        for z in [-14.0, 14.0] {
            p.boxed(v(x, H / 2.0, z), v(0.9, H, 0.9), Mat::MarbleDark, true);
            p.boxed(v(x, 0.15, z), v(1.2, 0.3, 1.2), Mat::Gold, false);
            p.boxed(v(x, H - 0.15, z), v(1.2, 0.3, 1.2), Mat::Gold, false);
        }
    }
    for i in 0..4 {
        let x = -26.0 + i as f32 * 12.0;
        exec_desk(p, x, 17.5, Face::NegZ);
        exec_desk(p, x + 6.0, -17.5, Face::PosZ);
    }
    // Skyline windows on both long walls.
    let xs = [-33.0, -22.0, -11.0, 0.0, 11.0];
    window_wall(p, HZ - 0.03, Face::NegZ, &xs, Mat::Gold);
    window_wall(p, -HZ + 0.03, Face::PosZ, &xs, Mat::Gold);

    // Marble runner with gold edges, vases, a few sad plants even here.
    p.boxed(v(-8.0, 0.012, 0.0), v(44.0, 0.02, 5.0), Mat::Marble, false);
    for s in [-1.0, 1.0] {
        p.boxed(
            v(-8.0, 0.025, s * 2.5),
            v(44.0, 0.03, 0.1),
            Mat::Gold,
            false,
        );
    }
    p.prop(&LOUNGE_DESIGN_SOFA, v(-24.0, 0.0, 4.0), -FRAC_PI_2, true);
    p.prop(&LOUNGE_DESIGN_SOFA, v(-24.0, 0.0, -6.0), -FRAC_PI_2, true);
    sad_plant(p, -37.5, 17.0);
    p.wall_panel(
        v(-39.85, 2.6, 12.0),
        Face::PosX,
        v2(2.0, 1.33),
        Tex::AiFirst,
    );
    p.wall_panel(
        v(-39.85, 2.6, -14.0),
        Face::PosX,
        v2(1.2, 1.5),
        Tex::Synergy,
    );
    p.wall_panel(
        v(23.85, 3.4, 0.0),
        Face::NegX,
        v2(2.4, 0.96),
        Tex::DaysCounter,
    );

    // The AI shrine in the boss room: a glowing monolith on a dais, GPUs and candles.
    for (i, (w, d)) in [(4.4, 7.4), (3.6, 6.2), (2.8, 5.0)].into_iter().enumerate() {
        p.boxed(
            v(37.6, 0.15 + i as f32 * 0.3, 6.0),
            v(d * 0.5, 0.3, w * 1.4),
            Mat::MarbleDark,
            true,
        );
    }
    p.boxed(v(38.6, 2.6, 6.0), v(1.0, 3.4, 2.2), Mat::Dark, true);
    for dz in [-0.7, 0.0, 0.7] {
        p.boxed(
            v(38.08, 2.6, 6.0 + dz),
            v(0.04, 3.0, 0.1),
            Mat::Monolith,
            false,
        );
    }
    p.boxed(v(38.08, 4.15, 6.0), v(0.04, 0.1, 1.9), Mat::Monolith, false);
    p.wall_panel(
        v(33.0, 3.4, 13.85),
        Face::NegZ,
        v2(4.0, 0.96),
        Tex::PraiseModel,
    );
    gpu_shrine(p, 36.0, 10.5, Face::NegZ);
    gpu_shrine(p, 36.0, -4.0, Face::NegX);
    p.light(v(37.0, 3.5, 6.0), Color::srgb(0.7, 0.3, 1.0), 800_000.0);
    p.boxed(
        v(33.0, 0.012, 0.0),
        v(13.0, 0.02, 6.0),
        Mat::MarbleDark,
        false,
    );

    let gold = Color::srgb(1.0, 0.85, 0.55);
    lights(
        p,
        &[
            (-30.0, 0.0, gold),
            (-18.0, 8.0, gold),
            (-9.0, -8.0, gold),
            (0.0, 8.0, gold),
            (9.0, -8.0, gold),
            (18.0, 8.0, gold),
            (31.0, 0.0, gold),
        ],
    );
}
