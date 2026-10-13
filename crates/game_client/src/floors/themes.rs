//! The five floors: palettes, outlines (big pods joined by wide openings), anchors and
//! per-floor dressing. Outlines are polygons (see `shape.rs`); decor snaps to the derived walls.

use super::kit::*;
use super::layout::{FloorKind, Theme, yaw_towards};
use super::props::*;
use super::shape::{arc, blob, diamond, pillar, rounded_rect, smooth, stroke};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// Ground-plane point.
fn pt(x: f32, z: f32) -> Vec2 {
    Vec2::new(x, z)
}

fn route_point_near(route: &[Vec2], q: Vec2) -> Vec2 {
    route
        .windows(2)
        .map(|w| {
            let d = w[1] - w[0];
            let t = ((q - w[0]).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
            w[0] + d * t
        })
        .min_by(|a, b| a.distance(q).total_cmp(&b.distance(q)))
        .unwrap_or(q)
}

/// Where everything stands on a floor.
struct Cast {
    spawn: (f32, f32),
    npc: (f32, f32),
    packs: [(f32, f32); 6],
    boss: (f32, f32),
    /// Wall the arrival elevator is set into (nearest wall to this point).
    elevator: (f32, f32),
    exit: (f32, f32),
    door: Mat,
}

/// Elevators and the anchors every floor shares; call after `build_walls` and the route.
fn cast(p: &mut Plan, c: &Cast) {
    let (arrive, yaw) = p.elevator_near(c.elevator.0, c.elevator.1, c.door);
    p.anchor("elevator", arrive, yaw);
    let (exit, yaw) = p.elevator_near(c.exit.0, c.exit.1, c.door);
    p.anchor("elevator_exit", exit, yaw);

    let spawn = v(c.spawn.0, 0.0, c.spawn.1);
    let first = p.route.get(1).copied().unwrap_or(Vec2::ZERO);
    p.anchor(
        "player_spawn",
        spawn,
        yaw_towards(spawn, v(first.x, 0.0, first.y)),
    );
    let names = ["pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "pack_6"];
    for (name, (x, z)) in names.iter().zip(c.packs) {
        let at = v(x, 0.0, z);
        let to = route_point_near(&p.route, pt(x, z));
        p.anchor(name, at, yaw_towards(at, v(to.x, 0.0, to.y)));
    }
    let boss = v(c.boss.0, 0.0, c.boss.1);
    let to = p.route.last().copied().unwrap_or(Vec2::ZERO);
    p.anchor("boss", boss, yaw_towards(boss, v(to.x, 0.0, to.y)));
    let npc = v(c.npc.0, 0.0, c.npc.1);
    p.anchor("npc_1", npc, yaw_towards(npc, spawn));
}

/// Ceiling light panels on an 8 m grid wherever there is open floor.
fn ceiling_lights(p: &mut Plan) {
    for xi in -7..=7 {
        for zi in -5..=5 {
            let (x, z) = (xi as f32 * 8.0, zi as f32 * 8.0);
            if p.fp.contains(pt(x, z)) {
                p.boxed(v(x, H - 0.02, z), v(1.2, 0.04, 0.6), Mat::LightPanel, false);
            }
        }
    }
}

/// Flat carpet strip following `ctrl`, with glowing edges.
fn runner(p: &mut Plan, ctrl: &[Vec2], width: f32, mat: Mat, edge: Mat) {
    let path = smooth(ctrl);
    let pts: Vec<Vec2> = path.iter().step_by(2).copied().collect();
    for w in pts.windows(2) {
        let d = w[1] - w[0];
        let len = d.length();
        if len < 0.1 {
            continue;
        }
        let (mid, yaw) = ((w[0] + w[1]) / 2.0, (-d.y).atan2(d.x));
        let nrm = Vec2::new(-d.y, d.x) / len;
        p.slab(v(mid.x, 0.01, mid.y), yaw, v(len + 0.2, 0.02, width), mat);
        for s in [-1.0, 1.0] {
            let e = mid + nrm * s * width / 2.0;
            p.slab(v(e.x, 0.02, e.y), yaw, v(len + 0.2, 0.03, 0.08), edge);
        }
    }
}

/// Free-standing pillar: solid hole in the outline plus gold collars.
fn column(p: &mut Plan, x: f32, z: f32) {
    p.fp.hole(pillar(pt(x, z), 0.55), Mat::MarbleDark);
    p.boxed(v(x, 0.15, z), v(1.3, 0.3, 1.3), Mat::Gold, false);
    p.boxed(v(x, H - 0.15, z), v(1.3, 0.3, 1.3), Mat::Gold, false);
}

fn cool() -> Color {
    Color::srgb(0.4, 0.7, 1.0)
}

fn lights(p: &mut Plan, list: &[(f32, f32, Color)]) {
    for &(x, z, c) in list {
        p.light(v(x, 3.4, z), c, 500_000.0);
    }
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

/// Fills `plan` for floor `id` and returns the palette.
pub fn build(id: u8, plan: &mut Plan) -> Theme {
    match id {
        1 => floor1(plan),
        2 => floor2(plan),
        3 => floor3(plan),
        4 => floor4(plan),
        _ => floor5(plan),
    }
    ceiling_lights(plan);
    theme(id)
}

// ---------------------------------------------------------------- floor 1

fn floor1(p: &mut Plan) {
    let w = Mat::Wall;
    p.fp.pod(blob(pt(-36.0, 0.0), 11.0, 13.0, 0.0, 3, 0.08), w);
    p.fp.link(
        stroke(
            &[
                pt(-30.0, 0.0),
                pt(-18.0, 2.0),
                pt(0.0, 0.0),
                pt(18.0, -4.0),
                pt(30.0, -6.0),
            ],
            5.5,
        ),
        w,
    );
    p.fp.pod(blob(pt(-12.0, 18.0), 12.0, 11.0, 20.0, 3, 0.1), w);
    p.fp.link(stroke(&[pt(-14.0, 3.0), pt(-13.0, 12.0)], 5.0), w);
    p.fp.link(stroke(&[pt(-33.0, 7.0), pt(-19.0, 19.0)], 5.0), w);
    p.fp.pod(blob(pt(8.0, -14.0), 12.0, 11.0, -15.0, 3, 0.1), w);
    p.fp.pod(rounded_rect(pt(41.0, -6.0), 9.0, 12.0, 5.0, 0.0), w);
    p.fp.link(
        stroke(
            &[
                pt(-30.0, -8.0),
                pt(-24.0, -18.0),
                pt(-10.0, -24.0),
                pt(0.0, -24.0),
            ],
            3.5,
        ),
        w,
    );
    p.fp.pod(blob(pt(14.0, 21.0), 8.0, 9.0, 0.0, 3, 0.1), w);
    p.fp.link(stroke(&[pt(14.0, 0.0), pt(14.0, 16.0)], 4.0), w);
    p.fp.pod(blob(pt(30.0, 20.0), 7.0, 7.0, 40.0, 3, 0.1), w);
    p.fp.link(stroke(&[pt(19.0, 21.0), pt(26.0, 20.0)], 4.0), w);
    p.fp.link(stroke(&[pt(33.0, 15.0), pt(37.0, 3.0)], 4.0), w);
    p.fp.link(stroke(&[pt(27.0, -4.0), pt(29.0, 14.0)], 4.0), w);
    p.fp.pod(blob(pt(26.0, -26.0), 8.0, 7.0, 0.0, 3, 0.1), w);
    p.fp.link(stroke(&[pt(26.0, -10.0), pt(26.0, -20.0)], 4.0), w);
    for (x, z) in [
        (-22.0, 4.0),
        (-10.0, -3.0),
        (2.0, 4.5),
        (10.0, -6.0),
        (22.0, -8.0),
    ] {
        column(p, x, z);
    }
    p.route = vec![
        pt(-42.0, 1.0),
        pt(-30.0, 1.0),
        pt(-18.0, 2.0),
        pt(0.0, 0.0),
        pt(18.0, -4.0),
        pt(30.0, -6.0),
        pt(32.0, -6.0),
    ];
    p.build_walls();
    decor1(p);
    cast(
        p,
        &Cast {
            spawn: (-43.0, 3.0),
            npc: (-40.0, 7.0),
            packs: [
                (-32.0, -9.0),
                (-15.0, 25.0),
                (9.0, -19.5),
                (15.5, 24.0),
                (30.0, 20.0),
                (26.0, -27.0),
            ],
            boss: (44.0, -8.0),
            elevator: (-47.0, 2.0),
            exit: (50.0, -6.0),
            door: Mat::Door,
        },
    );
}

// ---------------------------------------------------------------- floor 2

fn floor2(p: &mut Plan) {
    let w = Mat::Wall;
    p.fp.pod(blob(pt(-41.0, 2.0), 9.0, 12.0, 0.0, 3, 0.08), w);
    p.fp.pod(blob(pt(-8.0, 0.0), 28.0, 23.0, 0.0, 3, 0.06), w);
    p.fp.link(stroke(&[pt(-36.0, 2.0), pt(-26.0, 2.0)], 5.0), w);
    p.fp.pod(rounded_rect(pt(41.0, -4.0), 9.5, 12.0, 6.0, 0.0), w);
    p.fp.link(stroke(&[pt(12.0, -2.0), pt(32.0, -4.0)], 5.5), w);
    p.fp.pod(blob(pt(8.0, 29.0), 10.0, 8.0, 0.0, 3, 0.08), Mat::Beige);
    p.fp.link(stroke(&[pt(6.0, 12.0), pt(8.0, 24.0)], 5.0), w);
    p.fp.pod(blob(pt(-6.0, -31.0), 9.0, 7.0, 0.0, 3, 0.08), w);
    p.fp.link(stroke(&[pt(-6.0, -14.0), pt(-6.0, -26.0)], 4.5), w);
    for (x, z) in [
        (-18.0, -6.0),
        (-26.0, 10.0),
        (-8.0, 3.0),
        (-6.0, 14.0),
        (11.0, 3.0),
        (13.0, -10.0),
    ] {
        p.fp.hole(diamond(pt(x, z), 3.4, 0.0), Mat::Glass);
    }
    p.route = vec![
        pt(-45.0, 2.0),
        pt(-30.0, 2.0),
        pt(-17.0, 6.0),
        pt(-6.0, -5.0),
        pt(6.0, 6.0),
        pt(20.0, -3.0),
        pt(30.0, -4.0),
    ];
    p.build_walls();
    decor2(p);
    cast(
        p,
        &Cast {
            spawn: (-45.0, 2.0),
            npc: (-42.0, 8.0),
            packs: [
                (-24.0, -13.0),
                (-16.0, 15.0),
                (4.0, 15.0),
                (6.0, -15.0),
                (-6.0, -33.0),
                (8.0, 30.0),
            ],
            boss: (44.0, -4.0),
            elevator: (-50.0, 4.0),
            exit: (50.0, -4.0),
            door: Mat::Door,
        },
    );
}

// ---------------------------------------------------------------- floor 3

fn floor3(p: &mut Plan) {
    let w = Mat::Wall;
    let c0 = Vec2::ZERO;
    p.fp.link(stroke(&arc(c0, 18.0, -50.0, -340.0), 8.0), w);
    // The demo stage: a solid drum the hall wraps around.
    p.fp.hole(shape_circle(c0, 11.5), w);
    p.fp.pod(blob(pt(24.0, -20.0), 9.0, 8.0, 0.0, 3, 0.08), w);
    p.fp.link(stroke(&[pt(11.0, -14.0), pt(22.0, -18.0)], 5.0), w);
    p.fp.pod(rounded_rect(pt(37.0, 8.0), 9.5, 11.0, 6.0, 0.0), w);
    p.fp.link(stroke(&[pt(16.0, 6.0), pt(30.0, 8.0)], 5.0), w);
    p.fp.pod(blob(pt(-10.0, 31.0), 10.0, 8.0, 0.0, 3, 0.08), w);
    p.fp.link(stroke(&[pt(-6.0, 22.0), pt(-10.0, 28.0)], 5.0), w);
    p.route = smooth(&arc(c0, 14.0, -50.0, -340.0));
    p.route.insert(0, pt(24.0, -20.0));
    p.route.push(pt(26.0, 8.0));
    p.build_walls();
    decor3(p);
    let at = |deg: f32| {
        let t = deg.to_radians();
        (22.5 * t.cos(), 22.5 * t.sin())
    };
    cast(
        p,
        &Cast {
            spawn: (24.0, -20.0),
            npc: (27.0, -16.0),
            packs: [
                at(-110.0),
                at(-165.0),
                at(140.0),
                at(90.0),
                at(50.0),
                (-12.0, 34.0),
            ],
            boss: (40.0, 8.0),
            elevator: (30.0, -26.0),
            exit: (46.0, 8.0),
            door: Mat::Door,
        },
    );
}

fn shape_circle(c: Vec2, r: f32) -> Vec<Vec2> {
    super::shape::regular(c, r, 36, 0.0)
}

// ---------------------------------------------------------------- floor 4

fn polar(r: f32, deg: f32) -> Vec2 {
    let t = deg.to_radians();
    pt(r * t.cos(), r * t.sin())
}

fn floor4(p: &mut Plan) {
    let w = Mat::Wall;
    let c0 = Vec2::ZERO;
    p.fp.pod(blob(c0, 28.0, 27.0, 0.0, 5, 0.02), w);
    p.fp.pod(blob(pt(-45.0, 0.0), 10.0, 11.0, 0.0, 3, 0.08), w);
    p.fp.link(stroke(&[pt(-38.0, 0.0), pt(-24.0, 0.0)], 5.5), w);
    p.fp.pod(rounded_rect(pt(46.0, 0.0), 9.0, 11.0, 5.0, 0.0), w);
    p.fp.link(stroke(&[pt(24.0, 0.0), pt(40.0, 0.0)], 5.5), w);
    // SOC offshoot in the south-east, reached round the quiet side of the ring.
    p.fp.pod(blob(pt(30.0, 28.0), 9.0, 8.0, 0.0, 3, 0.08), w);
    p.fp.link(stroke(&[pt(20.0, 18.0), pt(28.0, 26.0)], 5.0), w);
    // One bubble per pack along the north rim.
    for deg in [205.0, 236.0, 268.0, 300.0, 338.0] {
        let c = polar(32.5, deg);
        p.fp.pod(blob(c, 8.0, 7.5, deg, 0, 0.0), w);
    }
    // War room shell with doors west and east, plus loose bars that break sight lines.
    p.fp.partition(&smooth(&arc(c0, 11.0, 20.0, 160.0)), 1.2, w);
    p.fp.partition(&smooth(&arc(c0, 11.0, 200.0, 340.0)), 1.2, w);
    for deg in [221.0, 252.0, 284.0, 319.0, 40.0, 75.0, 105.0, 140.0] {
        p.fp.partition(&[polar(15.0, deg), polar(22.0, deg)], 1.0, w);
    }
    p.route = smooth(&[
        pt(-45.0, 1.0),
        pt(-30.0, 0.0),
        pt(-23.0, -8.0),
        pt(-16.0, -16.0),
        pt(-3.0, -22.0),
        pt(10.0, -19.0),
        pt(19.0, -11.0),
        pt(25.0, -3.0),
        pt(30.0, 0.0),
        pt(33.0, 0.0),
    ]);
    p.build_walls();
    decor4(p);
    let rim = |deg: f32| {
        let c = polar(32.5, deg);
        (c.x, c.y)
    };
    cast(
        p,
        &Cast {
            spawn: (-46.0, 3.0),
            npc: (-42.0, 8.0),
            packs: [
                rim(205.0),
                rim(236.0),
                rim(268.0),
                rim(338.0),
                rim(300.0),
                (31.0, 29.0),
            ],
            boss: (47.0, 0.0),
            elevator: (-55.0, 0.0),
            exit: (55.0, 4.0),
            door: Mat::Door,
        },
    );
}

// ---------------------------------------------------------------- floor 5

fn floor5(p: &mut Plan) {
    let w = Mat::Wall;
    // Grand elliptical hall tapering into a triangular executive pod.
    p.fp.pod(blob(pt(-12.0, 0.0), 38.0, 26.0, 0.0, 0, 0.0), w);
    p.fp.pod(
        super::shape::rounded_poly(
            &[
                pt(22.0, -16.0),
                pt(22.0, 16.0),
                pt(55.0, 5.0),
                pt(55.0, -5.0),
            ],
            3.0,
        ),
        w,
    );
    for (x, z) in [(-30.0, 27.0), (-10.0, 28.0), (-26.0, -26.0), (-2.0, -28.0)] {
        p.fp.pod(blob(pt(x, z), 6.0, 5.0, 0.0, 0, 0.0), w);
    }
    p.fp.pod(blob(pt(20.0, -32.0), 8.0, 7.0, 0.0, 3, 0.08), w);
    p.fp.link(stroke(&[pt(18.0, -18.0), pt(20.0, -28.0)], 5.0), w);
    for (x, z) in [
        (-30.0, -8.0),
        (-30.0, 8.0),
        (-14.0, -9.0),
        (-14.0, 9.0),
        (2.0, -8.0),
        (2.0, 8.0),
    ] {
        column(p, x, z);
    }
    p.route = vec![
        pt(-44.0, 0.0),
        pt(-20.0, 0.0),
        pt(10.0, 0.0),
        pt(24.0, 0.0),
        pt(30.0, 0.0),
    ];
    p.build_walls();
    decor5(p);
    cast(
        p,
        &Cast {
            spawn: (-44.0, 0.0),
            npc: (-41.0, 5.0),
            packs: [
                (-34.0, -14.0),
                (-22.0, 16.0),
                (-10.0, -17.0),
                (-4.0, 17.0),
                (6.0, -17.0),
                (20.0, -33.0),
            ],
            boss: (42.0, 0.0),
            elevator: (-50.0, 0.0),
            exit: (40.0, -10.0),
            door: Mat::Gold,
        },
    );
}

// ---------------------------------------------------------------- decor

/// Quiet corner for the lone plant that has given up.
fn plants(p: &mut Plan, list: &[(f32, f32)]) {
    for &(x, z) in list {
        sad_plant(p, x, z);
    }
}

fn decor1(p: &mut Plan) {
    // Cubicle islands, back to back, with a free lane beside each so packs have a clear pull.
    for (i, x) in [-6.0, -2.5].into_iter().enumerate() {
        cubicle(p, i, x, 18.5, 0.0);
        cubicle(p, i + 1, x, 15.5, PI);
    }
    cubicle(p, 0, -0.5, -9.0, 0.0);
    cubicle(p, 1, -0.5, -12.0, PI);
    lounge(p, 16.0, -12.0, FRAC_PI_2, true);

    // The middle of the floor: games nobody has time for.
    runner(
        p,
        &[
            pt(-30.0, 0.0),
            pt(-18.0, 2.0),
            pt(0.0, 0.0),
            pt(18.0, -4.0),
            pt(30.0, -6.0),
        ],
        3.5,
        Mat::Runner,
        Mat::RunnerEdge,
    );
    ping_pong(p, -17.0, -1.5);
    foosball(p, -4.0, -0.5);
    beanbag(p, -14.0, 5.5, Mat::BeanA);
    beanbag(p, -12.4, 7.2, Mat::BeanB);
    beanbag(p, -15.5, 7.4, Mat::BeanC);
    lounge(p, 7.5, 5.5, FRAC_PI_2, true);
    plants(p, &[(24.0, -1.0), (1.0, -5.0), (-28.0, 5.0), (-4.0, 9.0)]);

    // West pod: IT corner (npc_2 asleep, npc_3 chatting) and a kitchenette.
    p.framed(v(-39.0, 0.0, 9.0), PI, |p| {
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
        p.boxed(v(1.5, 0.95, -0.7), v(0.2, 0.05, 0.12), Mat::Dark, false);
    });
    it_clutter(p, -44.0, 5.5);
    mini_fridge(p, -34.5, 11.0, Face::NegZ);
    let sleeper = v(-38.6, 0.0, 7.6);
    p.anchor("npc_2", sleeper, PI);
    let talker = v(-36.4, 0.0, 6.0);
    p.anchor("npc_3", talker, yaw_towards(talker, sleeper));
    vending(p, -31.0, 11.4, Face::NegZ);
    water_cooler(p, -29.0, 10.5);
    p.prop(&FRIDGE, v(-27.5, 0.0, 9.2), FRAC_PI_2, true);
    p.prop(&SIDE_TABLE, v(-27.6, 0.0, 6.6), FRAC_PI_2, true);
    p.prop(
        &COFFEE_MACHINE,
        v(-27.6, SIDE_TABLE.size[1] * FURNITURE_SCALE, 6.6),
        0.0,
        false,
    );
    p.poster_near(-47.0, -4.0, 2.4, v2(1.9, 0.95), Tex::TurnItOff);
    p.poster_near(-45.0, 11.0, 2.5, v2(1.9, 0.63), Tex::NextAgent);

    // Satire on the walls.
    p.poster_near(-24.0, 20.0, 2.4, v2(2.0, 1.33), Tex::AiFirst);
    p.poster_near(1.0, 22.0, 2.4, v2(2.4, 1.15), Tex::AiTransformation);
    p.poster_near(-46.0, -9.0, 2.4, v2(1.2, 1.5), Tex::Synergy);
    p.poster_near(-4.0, -24.0, 2.4, v2(1.2, 1.5), Tex::PosterBurn);
    p.poster_near(19.0, -12.0, 2.4, v2(1.2, 1.5), Tex::PosterHang);
    p.poster_near(-20.0, 29.0, 2.4, v2(1.2, 1.5), Tex::PosterPie);
    p.poster_near(37.0, 29.0, 2.4, v2(1.2, 1.5), Tex::PosterBars);
    p.panel_near(-9.0, -5.5, 3.6, v2(6.0, 1.1), Tex::MandatoryFun);
    p.poster_near(-20.0, -23.0, 2.2, v2(3.0, 1.5), Tex::Roadmap);
    p.poster_near(-12.0, 4.0, 2.2, v2(3.0, 1.5), Tex::Flowchart);
    // Above the boss hall entrance, seen along the hub.
    p.panel_near(32.0, -1.0, 3.4, v2(2.4, 0.96), Tex::DaysCounter);

    // Boss hall: server racks and a GPU shrine, like every start-up eventually.
    for x in [36.0, 39.5, 43.0] {
        server_rack(p, x, -15.5, 0.0);
        server_rack(p, x, 3.5, PI);
    }
    gpu_shrine(p, 46.5, 3.4, Face::NegZ);
    p.poster_near(41.0, 6.0, 3.1, v2(2.2, 1.05), Tex::AiTransformation);
    p.boxed(v(41.0, 0.01, -6.0), v(12.0, 0.02, 3.0), Mat::Runner, false);

    lights(
        p,
        &[
            (-36.0, 0.0, cool()),
            (-18.0, 2.0, cool()),
            (0.0, 0.0, cool()),
            (18.0, -4.0, cool()),
            (-12.0, 19.0, cool()),
            (8.0, -14.0, cool()),
            (41.0, -6.0, Color::srgb(0.8, 0.4, 1.0)),
            (-40.0, 9.0, Color::srgb(1.0, 0.85, 0.6)),
        ],
    );
}

fn decor2(p: &mut Plan) {
    // Meeting boxes: a table and a board inside every glass diamond.
    for (i, (x, z)) in [
        (-18.0, -6.0),
        (-26.0, 10.0),
        (-8.0, 3.0),
        (-6.0, 14.0),
        (11.0, 3.0),
        (13.0, -10.0),
    ]
    .into_iter()
    .enumerate()
    {
        let tilt = FRAC_PI_2 / 2.0;
        p.slab(v(x, 0.4, z), tilt, v(2.4, 0.8, 1.0), Mat::Wood);
        p.slab(
            v(x, 0.82, z),
            tilt,
            v(2.5, 0.04, 1.1),
            if i % 2 == 0 { Mat::Board } else { Mat::Beige },
        );
        for k in [-1.0, 1.0] {
            let off = Quat::from_rotation_y(tilt) * v(0.0, 0.0, 1.0 * k);
            p.slab(
                v(x + off.x, 0.25, z + off.z),
                tilt,
                v(0.45, 0.5, 0.45),
                Mat::Dark,
            );
        }
        // Whiteboard against the box's far corner.
        p.slab(
            v(x - 1.6, 1.4, z - 1.6),
            tilt,
            v(2.0, 1.1, 0.05),
            Mat::Board,
        );
        p.light(v(x, 3.0, z), Color::srgb(0.7, 1.0, 0.9), 40_000.0);
    }
    plants(
        p,
        &[
            (-34.0, -14.0),
            (-2.0, -16.0),
            (12.0, 14.0),
            (-12.0, 17.0),
            (-44.0, 12.0),
        ],
    );
    p.prop(&LAMP_ROUND_FLOOR, v(-14.0, 0.0, 2.0), 0.0, false);
    p.prop(&LAMP_ROUND_FLOOR, v(1.0, 0.0, -4.0), 0.0, false);
    water_cooler(p, -30.0, -6.0);

    // Stand-up area in the arrival pod: yellow tape rectangle and a sign.
    for (cx, cz, w, d) in [
        (-40.0, -8.0, 6.0, 0.08),
        (-40.0, -3.0, 6.0, 0.08),
        (-43.0, -5.5, 0.08, 5.0),
        (-37.0, -5.5, 0.08, 5.0),
    ] {
        p.boxed(v(cx, 0.015, cz), v(w, 0.03, d), Mat::NeonYellow, false);
    }
    p.panel_near(-50.0, -6.0, 2.6, v2(2.8, 0.93), Tex::Standup);
    p.poster_near(-44.0, 13.0, 2.2, v2(3.0, 1.5), Tex::Roadmap);
    p.poster_near(-20.0, 22.0, 2.4, v2(2.0, 1.33), Tex::AiFirst);
    p.poster_near(2.0, 23.0, 2.4, v2(1.2, 1.5), Tex::Synergy);
    p.poster_near(-20.0, -22.0, 2.4, v2(2.4, 1.2), Tex::Flowchart);
    p.poster_near(8.0, -22.0, 2.4, v2(2.4, 1.2), Tex::Stickies);
    p.panel_near(26.0, -1.0, 3.4, v2(2.4, 0.96), Tex::DaysCounter);
    runner(
        p,
        &[
            pt(-38.0, 2.0),
            pt(-20.0, 2.0),
            pt(0.0, 0.0),
            pt(20.0, -2.0),
            pt(36.0, -4.0),
        ],
        3.0,
        Mat::Runner,
        Mat::RunnerEdge,
    );

    // HR pod (pack_6): beige walls, wellness corner, pizza box, chatbot screen.
    p.panel_near(-1.0, 25.0, 2.9, v2(2.4, 0.4), Tex::HrDoor);
    p.boxed(v(1.5, 0.02, 33.0), v(0.7, 0.04, 1.9), Mat::Yoga, false);
    p.panel_near(-2.0, 31.0, 2.4, v2(2.4, 0.4), Tex::Wellness);
    p.prop(&SIDE_TABLE, v(16.0, 0.0, 33.5), 0.0, true);
    p.boxed(
        v(16.0, 0.95, 33.5),
        v(0.7, 0.08, 0.7),
        Mat::Cardboard,
        false,
    );
    p.panel_near(17.0, 28.0, 2.3, v2(1.8, 1.2), Tex::PizzaBox);
    p.panel_near(8.0, 38.0, 2.3, v2(3.0, 2.25), Tex::ChatbotFace);
    p.panel_near(-2.0, 28.0, 2.3, v2(1.0, 1.25), Tex::Family);
    p.boxed(v(3.0, 0.5, 24.5), v(0.3, 1.0, 0.3), Mat::Dark, true);
    p.boxed(v(3.0, 1.2, 24.5), v(0.6, 0.5, 0.45), Mat::Metal, false);
    p.panel(v(3.0, 1.2, 24.74), 0.0, v2(0.5, 0.11), Tex::Suggestions);
    p.boxed(v(3.0, 1.35, 24.76), v(0.08, 0.1, 0.04), Mat::Gold, false);
    p.light(v(8.0, 3.2, 29.0), Color::srgb(1.0, 0.85, 0.65), 300_000.0);

    // Boss pod: the Analyst's war table, written by Claude.
    long_table(p, 39.0, -11.0, 2, true);
    p.panel_near(41.0, 8.0, 2.4, v2(3.0, 1.5), Tex::Kpi);
    p.poster_near(48.0, 6.0, 2.4, v2(1.2, 1.5), Tex::PosterPie);
    p.boxed(v(40.0, 0.01, -4.0), v(12.0, 0.02, 3.0), Mat::Runner, false);

    let mint = Color::srgb(0.6, 1.0, 0.85);
    lights(
        p,
        &[
            (-41.0, 2.0, mint),
            (-24.0, -8.0, mint),
            (-16.0, 12.0, mint),
            (0.0, -6.0, mint),
            (6.0, 8.0, mint),
            (-6.0, -31.0, Color::srgb(1.0, 0.85, 0.6)),
            (40.0, -4.0, mint),
        ],
    );
}

fn decor3(p: &mut Plan) {
    // The demo stage drum: screens, neon and podiums all the way round.
    for deg in [-60.0, -110.0, -160.0, 160.0, 110.0, 60.0] {
        let c = polar(11.7, deg);
        p.panel_near(c.x, c.y, 2.3, v2(4.5, 2.5), Tex::DemoScreen);
    }
    for deg in [-135.0, 135.0, 85.0, -85.0] {
        let c = polar(11.7, deg);
        p.panel_near(c.x, c.y, 4.0, v2(4.8, 1.2), Tex::CloseDeals);
    }
    for deg in [-10.0, 10.0] {
        let c = polar(11.7, deg);
        p.panel_near(c.x, c.y, 3.0, v2(2.2, 1.7), Tex::Leaderboard);
    }
    demo_stand(p, -16.0, 0.0, Face::PosX);
    demo_stand(p, 0.0, -16.0, Face::PosZ);
    demo_stand(p, 6.0, 16.5, Face::NegZ);
    // Showroom lounges between the pack positions, and the gong.
    lounge(p, -13.9, -12.9, 0.0, false);
    lounge(p, -8.0, 17.2, PI, false);
    p.prop(&LOUNGE_SOFA_LONG, v(-20.0, 0.0, 6.0), FRAC_PI_2, true);
    p.prop(&LOUNGE_DESIGN_SOFA, v(10.0, 0.0, 16.0), PI, true);
    p.prop(&COFFEE_TABLE_GLASS, v(10.0, 0.0, 14.0), 0.0, true);
    gong(p, 9.0, -18.0);
    trophy_shelf(p, 5.0, 11.0, Face::NegZ);
    plants(
        p,
        &[
            (-23.0, -2.0),
            (-18.0, 12.0),
            (14.0, -9.0),
            (16.0, 14.0),
            (-5.0, -18.0),
        ],
    );
    p.poster_near(-24.0, -12.0, 2.4, v2(1.2, 1.5), Tex::PosterBars);
    p.poster_near(-22.0, 14.0, 2.4, v2(2.4, 1.15), Tex::AiTransformation);
    p.poster_near(2.0, 25.0, 2.4, v2(2.0, 1.33), Tex::AiFirst);
    p.poster_near(8.0, -25.0, 2.4, v2(1.2, 1.5), Tex::Synergy);
    p.poster_near(24.0, -28.0, 2.4, v2(1.2, 1.5), Tex::PosterPie);

    // Marketing pod (pack_6, the Head of Marketing).
    p.panel_near(-10.0, 39.0, 3.5, v2(3.9, 1.3), Tex::Brand);
    p.panel_near(-18.0, 36.0, 1.9, v2(3.0, 2.0), Tex::MoodBoards);
    p.panel_near(-2.0, 36.0, 1.9, v2(3.0, 2.0), Tex::MoodBoards);
    p.panel_near(-19.5, 30.0, 2.2, v2(3.0, 1.7), Tex::Campaign);
    p.framed(v(-4.0, 0.0, 35.0), PI, |p| {
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
    });
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
            v(-17.5 + dx, 0.15 + (i / 2) as f32 * 0.32, 35.0 + dz),
            v(w, 0.3, 0.12),
            [Mat::Fabric0, Mat::Fabric1, Mat::Yellow, Mat::NeonPink][i],
            false,
        );
    }
    p.boxed(v(-17.0, 0.35, 33.5), v(0.7, 0.7, 0.5), Mat::Cardboard, true);
    beanbag(p, -2.5, 28.0, Mat::BeanC);
    p.panel(v(-2.5, 0.9, 28.6), 0.0, v2(0.9, 0.9), Tex::Cobweb);

    // Boss pod: the Customer, with a pile of unsigned contracts.
    long_table(p, 36.0, 15.5, 2, false);
    p.panel_near(37.0, 19.0, 2.4, v2(3.0, 1.5), Tex::Roadmap);
    p.boxed(v(40.0, 0.01, 8.0), v(12.0, 0.02, 3.0), Mat::Runner, false);
    p.panel_near(28.0, 8.0, 3.4, v2(2.4, 0.96), Tex::DaysCounter);
    runner(
        p,
        &smooth(&arc(Vec2::ZERO, 14.0, -50.0, -340.0)),
        3.0,
        Mat::Runner,
        Mat::RunnerEdge,
    );

    let warm = Color::srgb(1.0, 0.7, 0.45);
    lights(
        p,
        &[
            (24.0, -20.0, warm),
            (-4.0, -18.0, warm),
            (-18.0, -4.0, warm),
            (-12.0, 14.0, warm),
            (6.0, 18.0, warm),
            (-10.0, 31.0, Color::srgb(1.0, 0.4, 0.9)),
            (38.0, 8.0, warm),
            (0.0, 0.0, warm),
        ],
    );
}

fn decor4(p: &mut Plan) {
    // War room: glass table, Gantt walls on both faces of the shell.
    long_table(p, 0.0, 0.0, 4, true);
    p.panel_near(0.0, 10.3, 2.4, v2(3.5, 1.75), Tex::Gantt);
    p.panel_near(0.0, -10.3, 2.4, v2(3.5, 1.75), Tex::Gantt);
    p.panel_near(0.0, 11.7, 2.4, v2(3.5, 1.75), Tex::Gantt);
    p.panel_near(0.0, -11.7, 2.4, v2(3.5, 1.75), Tex::Gantt);
    p.panel_near(-11.7, 6.0, 3.2, v2(2.8, 0.47), Tex::WarRoom);
    p.light(v(0.0, 3.4, 0.0), Color::srgb(1.0, 0.9, 0.6), 400_000.0);

    // Gantt walls: every bar red.
    for deg in [75.0, 105.0, 140.0, 165.0, 15.0] {
        let c = polar(27.7, deg);
        p.poster_near(c.x, c.y, 2.3, v2(4.5, 2.25), Tex::Gantt);
    }
    p.poster_near(-49.0, 10.0, 2.4, v2(3.0, 1.5), Tex::Roadmap);
    p.panel_near(33.0, 0.5, 3.4, v2(2.4, 0.96), Tex::DaysCounter);

    // South lane: printers, all broken, and a consultant lounge.
    for (i, x) in [-16.0, -13.8, -11.6].into_iter().enumerate() {
        printer(p, x, 22.5, Face::NegZ);
        if i == 1 {
            p.boxed(v(x, 1.2, 22.2), v(0.3, 0.04, 0.3), Mat::Paper, false);
        }
    }
    lounge(p, 12.5, 18.0, PI, true);
    p.prop(&RUG_ROUND, v(-2.0, 0.0, 19.0), 0.0, false);
    p.prop(&LOUNGE_CHAIR_RELAX, v(-1.0, 0.0, 22.0), -FRAC_PI_2, true);
    water_cooler(p, -20.0, 12.5);
    p.prop(
        &COFFEE_MACHINE,
        v(-22.0, SIDE_TABLE.size[1] * FURNITURE_SCALE, 14.0),
        0.0,
        false,
    );
    p.prop(&SIDE_TABLE, v(-22.0, 0.0, 14.0), 0.0, true);
    doc_shelf(p, -22.0, 19.0, Face::NegZ);
    plants(p, &[(-42.0, -7.0), (22.0, 8.0), (-24.0, -7.0), (5.0, 10.0)]);
    p.poster_near(-49.0, -8.0, 2.4, v2(2.0, 1.33), Tex::AiFirst);

    // SOC offshoot (pack_6, the CISO): red "!" monitors, a password on a monitor, a shredder
    // next to unshredded paper, an open padlock, a fishing rod.
    for x in [24.0, 27.0, 30.0, 33.0, 36.0] {
        p.poster_near(x, 35.8, 2.4, v2(1.45, 1.45), Tex::SocAlert);
    }
    p.panel_near(30.0, 35.8, 3.5, v2(2.4, 0.4), Tex::WarRoom);
    p.panel(v(27.8, 1.7, 35.4), 0.0, v2(0.3, 0.3), Tex::StickyPass);
    p.boxed(v(30.0, 0.4, 34.3), v(8.0, 0.8, 0.9), Mat::Dark, true);
    p.boxed(v(23.5, 0.45, 25.0), v(0.6, 0.9, 0.4), Mat::Metal, true);
    p.boxed(v(23.5, 0.92, 25.0), v(0.5, 0.04, 0.05), Mat::Dark, false);
    for (dx, dz, h) in [(0.0, 0.0, 0.5), (0.35, 0.2, 0.35), (-0.3, 0.15, 0.25)] {
        p.boxed(
            v(24.8 + dx, h / 2.0, 25.3 + dz),
            v(0.6, h, 0.45),
            Mat::Paper,
            true,
        );
    }
    p.panel_near(38.5, 27.0, 2.3, v2(1.6, 1.28), Tex::Phishing);
    p.boxed(v(22.5, 1.1, 30.5), v(0.04, 2.2, 0.04), Mat::Wood, false);
    p.boxed(v(22.7, 2.15, 30.5), v(0.4, 0.03, 0.03), Mat::Wood, false);
    p.boxed(v(22.88, 1.7, 30.5), v(0.01, 0.9, 0.01), Mat::White, false);
    p.boxed(v(22.88, 1.2, 30.5), v(0.05, 0.06, 0.02), Mat::Metal, false);
    p.boxed(v(25.5, 1.3, 33.0), v(0.22, 0.2, 0.08), Mat::Gold, false);
    p.boxed(v(25.5, 1.5, 33.0), v(0.14, 0.12, 0.05), Mat::Metal, false);
    p.light(v(30.0, 3.2, 29.0), Color::srgb(1.0, 0.25, 0.25), 300_000.0);
    // Friendly IT support waits near the SOC, hoping for a ticket.
    let it = v(21.5, 0.0, 22.0);
    p.anchor("npc_2", it, yaw_towards(it, v(28.0, 0.0, 28.0)));

    // Boss hall: the VP's synergy dashboard.
    long_table(p, 45.0, -7.0, 2, true);
    p.panel_near(46.0, -11.5, 2.4, v2(3.5, 1.75), Tex::Gantt);
    p.boxed(v(46.0, 0.01, 0.0), v(11.0, 0.02, 3.0), Mat::Runner, false);

    let amber = Color::srgb(1.0, 0.9, 0.6);
    lights(
        p,
        &[
            (-45.0, 0.0, amber),
            (-22.0, -14.0, amber),
            (0.0, -23.0, amber),
            (20.0, -14.0, amber),
            (-14.0, 18.0, amber),
            (14.0, 14.0, amber),
            (30.0, 28.0, Color::srgb(1.0, 0.3, 0.3)),
            (46.0, 0.0, amber),
        ],
    );
}

fn decor5(p: &mut Plan) {
    // Skyline windows on the curved north and south walls.
    for (x, z) in [
        (-44.0, 14.0),
        (-20.0, 25.4),
        (4.0, 24.0),
        (-44.0, -14.0),
        (-14.0, -26.0),
        (10.0, -23.5),
    ] {
        p.window_near(x, z, v2(6.0, 3.2), Tex::Skyline, Mat::Gold);
    }
    // Executive desks in the alcoves.
    exec_desk(p, -30.0, 29.5, Face::NegZ);
    exec_desk(p, -10.0, 30.5, Face::NegZ);
    exec_desk(p, -26.0, -28.5, Face::PosZ);
    exec_desk(p, -2.0, -30.5, Face::PosZ);
    runner(
        p,
        &[pt(-44.0, 0.0), pt(-20.0, 0.0), pt(10.0, 0.0), pt(30.0, 0.0)],
        5.0,
        Mat::Marble,
        Mat::Gold,
    );
    p.prop(&LOUNGE_DESIGN_SOFA, v(-37.0, 0.0, 7.5), -FRAC_PI_2, true);
    p.prop(&LOUNGE_DESIGN_SOFA, v(-37.0, 0.0, -8.5), -FRAC_PI_2, true);
    p.prop(&COFFEE_TABLE_GLASS, v(-35.0, 0.0, 7.5), FRAC_PI_2, true);
    sad_plant(p, -46.0, 14.0);
    sad_plant(p, 12.0, 10.0);
    p.panel_near(-50.0, 8.0, 2.6, v2(2.0, 1.33), Tex::AiFirst);
    p.panel_near(-50.0, -9.0, 2.6, v2(1.2, 1.5), Tex::Synergy);
    p.panel_near(26.0, 0.0, 3.4, v2(2.4, 0.96), Tex::DaysCounter);

    // The AI shrine at the tip: a glowing monolith on a dais, GPUs and candles.
    for (i, (w, d)) in [(4.4, 7.4), (3.6, 6.2), (2.8, 5.0)].into_iter().enumerate() {
        p.boxed(
            v(49.5 + i as f32 * 0.5, 0.15 + i as f32 * 0.3, 0.0),
            v(d * 0.5, 0.3, w * 1.4),
            Mat::MarbleDark,
            true,
        );
    }
    p.boxed(v(51.0, 2.6, 0.0), v(1.0, 3.4, 2.2), Mat::Dark, true);
    for dz in [-0.7, 0.0, 0.7] {
        p.boxed(v(50.48, 2.6, dz), v(0.04, 3.0, 0.1), Mat::Monolith, false);
    }
    p.boxed(v(50.48, 4.15, 0.0), v(0.04, 0.1, 1.9), Mat::Monolith, false);
    p.panel_near(46.0, 7.0, 3.4, v2(4.0, 0.96), Tex::PraiseModel);
    gpu_shrine(p, 37.0, 9.0, Face::NegZ);
    gpu_shrine(p, 37.0, -9.0, Face::PosZ);
    p.light(v(49.0, 3.5, 0.0), Color::srgb(0.7, 0.3, 1.0), 800_000.0);
    p.boxed(
        v(40.0, 0.012, 0.0),
        v(22.0, 0.02, 6.0),
        Mat::MarbleDark,
        false,
    );

    // Mini-boss pod: a lounge nobody is allowed to sit in.
    p.prop(&LOUNGE_DESIGN_SOFA, v(20.0, 0.0, -36.0), 0.0, true);
    p.prop(&COFFEE_TABLE_GLASS, v(23.0, 0.0, -37.0), 0.0, true);

    let gold = Color::srgb(1.0, 0.85, 0.55);
    lights(
        p,
        &[
            (-40.0, 0.0, gold),
            (-24.0, 12.0, gold),
            (-12.0, -10.0, gold),
            (0.0, 10.0, gold),
            (14.0, -8.0, gold),
            (28.0, 0.0, gold),
            (20.0, -32.0, gold),
        ],
    );
}
