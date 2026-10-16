//! The data center: two indoor zones. Level 1 is a cold/hot-aisle server hall with a GPU
//! shrine and a NOC; level 2 is the same building after the cooling failed (orange light,
//! a round arena with pillars for the boss).

mod dress;

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble};
use super::props::{gpu_shrine, sad_plant, vending};
use super::shape::{blob, pillar, rounded_rect, stroke};
use super::themes::{pt, runner, v};
use super::zones::{ZoneCast, cast_zone};
use bevy::prelude::*;
use dress::*;
use std::f32::consts::PI;

pub fn build(level: u8) -> FloorLayout {
    let mut p = Plan::default();
    let theme = if level == 1 { dc1(&mut p) } else { dc2(&mut p) };
    let (zone, name) = if level == 1 {
        ("datacenter/1", "Data Center: Sublevel 1")
    } else {
        ("datacenter/2", "Data Center: Sublevel 2")
    };
    assemble(zone, level, name, false, p, theme)
}

fn theme(hot: bool) -> Theme {
    let c = Color::srgb;
    if hot {
        Theme {
            wall: c(0.55, 0.4, 0.36),
            trim: c(0.14, 0.06, 0.04),
            accent: LinearRgba::rgb(4.0, 1.2, 0.2),
            fabric: [c(0.5, 0.2, 0.15), c(0.4, 0.3, 0.2), c(0.3, 0.2, 0.2)],
            floor: FloorKind::Tile([1.0, 0.7, 0.55]),
            fog: c(0.07, 0.025, 0.015),
            ambient: c(1.0, 0.55, 0.35),
            ambient_brightness: 560.0,
            sun: c(1.0, 0.5, 0.3),
            sun_illuminance: 2500.0,
            fog_range: (20.0, 70.0),
        }
    } else {
        Theme {
            wall: c(0.45, 0.52, 0.62),
            trim: c(0.07, 0.09, 0.12),
            accent: LinearRgba::rgb(0.2, 1.6, 2.4),
            fabric: [c(0.2, 0.35, 0.5), c(0.25, 0.3, 0.45), c(0.2, 0.4, 0.4)],
            floor: FloorKind::Tile([0.8, 0.92, 1.05]),
            fog: c(0.02, 0.03, 0.05),
            ambient: c(0.5, 0.66, 1.0),
            ambient_brightness: 560.0,
            sun: c(0.7, 0.8, 1.0),
            sun_illuminance: 2500.0,
            fog_range: (25.0, 75.0),
        }
    }
}

/// The three consoles of the NOC: desks stay as they were (they are colliders); the screens
/// face the room, because a wall of red is the joke and it has to be seen.
fn noc_consoles(p: &mut Plan) {
    let tex = [Tex::NocGraph, Tex::NocAlerts, Tex::NocRegions];
    for (i, x) in [-14.0, -8.0, -2.0].into_iter().enumerate() {
        p.boxed(v(x, 0.4, 36.5), v(2.4, 0.8, 0.9), Mat::Dark, true);
        p.boxed(v(x, 0.82, 36.5), v(2.5, 0.04, 1.0), Mat::RackPanel, false);
        for (k, (dx, yaw)) in [(-0.8, -0.4), (0.0, 0.0), (0.8, 0.4)]
            .into_iter()
            .enumerate()
        {
            let (sx, sz) = (x + dx, 36.78 - (dx * dx) * 0.2);
            let face = PI + yaw;
            p.slab(v(sx, 1.2, sz + 0.04), face, v(0.74, 0.44, 0.05), Mat::Dark);
            p.panel(
                v(sx, 1.2, sz),
                face,
                Vec2::new(0.7, 0.394),
                tex[(i + k) % 3],
            );
            p.slab(v(sx, 0.9, sz + 0.03), face, v(0.1, 0.1, 0.1), Mat::Metal);
        }
        p.prop(&KEYBOARD, v(x, 0.84, 36.2), PI, false);
        p.prim(
            PrimKind::Cylinder,
            v(x + 1.0, 0.89, 36.3),
            v(0.08, 0.1, 0.08),
            if i == 1 { Mat::Red } else { Mat::White },
        );
        p.prop(&CHAIR_DESK, v(x, 0.0, 35.1), 0.0, false);
    }
}

/// The wall of dashboards: two columns either side of the (muted, cheerful) board.
fn noc_wall(p: &mut Plan) {
    let tex = [Tex::NocGraph, Tex::NocAlerts, Tex::NocRegions];
    for (i, x) in [-14.0, -12.0, -4.0, -2.0].into_iter().enumerate() {
        for (j, y) in [1.7, 2.9].into_iter().enumerate() {
            p.poster_near(x, 41.0, y, Vec2::new(1.9, 1.07), tex[(i + j * 2) % 3]);
        }
    }
    p.light(v(-8.0, 3.0, 36.0), Color::srgb(1.0, 0.25, 0.2), 150_000.0);
}

/// Beige mainframe cabinets against a wall, tape reels on top and a sign that says it all.
fn mainframe(p: &mut Plan, x: f32, z: f32) {
    let Some(w) = WallSpot::at(p, x, z) else {
        return;
    };
    for (i, l) in [-0.55, 0.55].into_iter().enumerate() {
        w.put(p, l, 1.05, 0.38, v(1.0, 2.1, 0.72), Mat::Beige);
        w.put(p, l, 1.3, 0.745, v(0.84, 0.8, 0.02), Mat::Dark);
        w.put(p, l, 0.45, 0.745, v(0.84, 0.5, 0.02), Mat::Wood);
        for k in 0..6 {
            let m = [Mat::BlinkAmber, Mat::BlinkRed, Mat::BlinkAmber][(k + i) % 3];
            w.put(
                p,
                l - 0.33 + k as f32 * 0.13,
                1.55,
                0.76,
                v(0.07, 0.07, 0.02),
                m,
            );
        }
        let c = w.world(l, 0.38);
        p.prim(
            PrimKind::Cylinder,
            v(c.x, 2.14, c.y),
            v(0.5, 0.04, 0.5),
            Mat::Dark,
        );
    }
    w.put(p, 0.0, 1.0, 0.77, v(0.14, 0.14, 0.03), Mat::LedRed);
    w.sign(p, 0.0, 3.1, 2.6, Sign::Mainframe);
}

/// Three power strips daisy-chained along a wall foot, one of them taped with a warning.
fn power_strip(p: &mut Plan, x: f32, z: f32) {
    let Some(w) = WallSpot::at(p, x, z) else {
        return;
    };
    for k in 0..3 {
        let l = -0.55 + k as f32 * 0.55;
        w.put(p, l, 0.05, 0.12, v(0.42, 0.06, 0.1), Mat::Dark);
        w.put(p, l, 0.085, 0.12, v(0.06, 0.02, 0.04), Mat::LedRed);
        if k < 2 {
            w.put(p, l + 0.28, 0.03, 0.15, v(0.16, 0.025, 0.03), Mat::Yellow);
        }
    }
    w.put(p, 0.7, 0.9, 0.04, v(0.04, 1.8, 0.03), Mat::Dark);
    w.sign(p, 0.0, 0.75, 0.9, Sign::PowerStrip);
}

fn shrine_extras(p: &mut Plan) {
    floor_ring(p, -8.0, -36.5, 4.6, 20, Mat::LedGreen, 0.1);
    for (k, (dx, dz)) in [(-2.0, -33.8), (2.0, -33.8), (-2.0, -32.2), (2.0, -32.2)]
        .into_iter()
        .enumerate()
    {
        p.slab(
            v(-8.0 + dx, 0.011, dz),
            0.12 * (k as f32 - 1.5),
            v(0.7, 0.02, 1.2),
            Mat::Yoga,
        );
    }
    // Offerings at the foot of the altar: energy drinks and a pizza nobody ordered.
    for (k, m) in [Mat::Red, Mat::Yellow, Mat::Scr2, Mat::Red, Mat::Scr1]
        .into_iter()
        .enumerate()
    {
        p.prim(
            PrimKind::Cylinder,
            v(-9.4 + k as f32 * 0.32, 0.08, -37.9),
            v(0.07, 0.16, 0.07),
            m,
        );
    }
    p.slab(v(-6.5, 0.02, -37.7), 0.3, v(0.5, 0.04, 0.5), Mat::Cardboard);
    for x in [-16.0, 0.0] {
        for dx in [-0.25, 0.0, 0.25] {
            hanging_cable(p, x + dx, -38.0, 1.25, false);
        }
    }
    p.light(v(-8.0, 2.4, -36.0), Color::srgb(0.4, 1.0, 0.5), 180_000.0);
    p.sign_near(2.9, -33.0, 2.6, 3.0, Sign::AiTraining);
}

fn dc1_lobby(p: &mut Plan) {
    runner(
        p,
        &[pt(-56.0, 0.0), pt(-48.0, 1.5), pt(-40.0, 0.0)],
        2.4,
        Mat::Runner,
        Mat::RunnerEdge,
    );
    // Reception: the desk is a collider; a monitor, keyboard, chair and a mug make it a job.
    p.boxed(v(-52.5, 0.55, -6.0), v(2.5, 1.1, 0.9), Mat::Wood, true);
    p.boxed(v(-52.5, 1.12, -6.0), v(2.6, 0.05, 1.0), Mat::Marble, false);
    p.prop(&SCREEN, v(-52.9, 1.145, -6.1), PI, false);
    p.prop(&KEYBOARD, v(-52.9, 1.145, -5.65), PI, false);
    p.prim(
        PrimKind::Cylinder,
        v(-51.8, 1.19, -5.9),
        v(0.09, 0.1, 0.09),
        Mat::White,
    );
    p.prop(&CHAIR_DESK, v(-52.5, 0.0, -7.5), 0.0, false);
    p.poster_near(-51.0, -11.0, 1.7, Vec2::new(0.5, 0.5), Tex::StickyPass);
    // Waiting nook in the south wall's bay.
    p.prop(&LOUNGE_SOFA, v(-49.0, 0.0, 10.1), PI, false);
    p.prop(&COFFEE_TABLE, v(-49.0, 0.0, 8.2), 0.0, false);
    p.prop(&POTTED_PLANT, v(-46.6, 0.0, 10.2), 0.0, false);
    p.sign_near(-49.0, 11.0, 2.6, 2.4, Sign::Escort);
    extinguisher(p, -44.5, 11.0);
    wall_cabinet(p, -39.1, 5.5, false);
}

fn dc1(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-48.0, 0.0), 9.0, 11.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-40.0, 0.0), pt(-30.0, 0.0)], 3.5), w);
    p.fp.pod(rounded_rect(pt(-8.0, 0.0), 24.0, 20.0, 6.0, 0.0), w);
    p.fp.link(stroke(&[pt(12.0, 0.0), pt(26.0, 0.0)], 3.5), w);
    p.fp.pod(rounded_rect(pt(35.0, 0.0), 12.0, 14.0, 6.0, 0.0), w);
    p.fp.pod(rounded_rect(pt(-8.0, -33.0), 11.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-8.0, -18.0), pt(-8.0, -29.0)], 3.0), w);
    p.fp.pod(rounded_rect(pt(-8.0, 33.0), 12.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-8.0, 18.0), pt(-8.0, 29.0)], 3.0), w);
    p.fp.hole(pillar(pt(30.0, -7.0), 0.7), Mat::Metal);
    p.fp.hole(pillar(pt(30.0, 7.0), 0.7), Mat::Metal);
    p.route = vec![
        pt(-52.0, 0.0),
        pt(-30.0, 0.0),
        pt(-8.0, 0.0),
        pt(26.0, 0.0),
        pt(36.0, 0.0),
    ];
    p.build_walls();
    wall_bands(p, false);

    // Server hall: two cold aisles (fronts facing each other) with a cross aisle at x -12..-4,
    // and a warm service corridor between the rack backs.
    for (x0, x1) in [(-30.0, -12.0), (-4.0, 14.0)] {
        rack_row(p, x0, x1, -7.0, PI, false, true);
        rack_row(p, x0, x1, -14.5, 0.0, false, false);
        rack_row(p, x0, x1, 7.0, 0.0, false, true);
        rack_row(p, x0, x1, 14.5, PI, false, false);
        for (z, dir) in [(-14.5, 1.0), (-7.0, -1.0), (7.0, 1.0), (14.5, -1.0)] {
            floor_line(
                p,
                pt(x0, z + dir * 0.7),
                pt(x1, z + dir * 0.7),
                0.07,
                Mat::Cold,
            );
        }
        for z in [-5.8, 5.8] {
            floor_line(p, pt(x0, z), pt(x1, z), 0.1, Mat::Hazard);
        }
    }
    for z in [-10.5, 10.5] {
        for x in [
            -28.5, -25.5, -22.5, -19.5, -16.5, -13.5, -2.5, 0.5, 3.5, 6.5, 9.5, 12.5,
        ] {
            vent_tile(p, x, z, Mat::Cold);
        }
    }
    for (x, z, n) in [
        (-22.5, -10.5, 3),
        (6.5, 10.5, 3),
        (-16.5, 10.5, 2),
        (0.5, -10.5, 2),
    ] {
        steam(p, x, z, n);
    }
    cable_bundle(
        p,
        &[(-26.5, -13.9), (-25.0, -10.9), (-23.2, -7.9)],
        Mat::Scr1,
    );
    cable_bundle(p, &[(2.5, -13.9), (3.6, -11.0), (1.6, -7.9)], Mat::Red);
    cable_bundle(
        p,
        &[(-19.5, 13.9), (-18.4, 10.7), (-20.5, 7.9)],
        Mat::Yellow,
    );
    cable_bundle(p, &[(8.5, 7.9), (9.3, 10.9), (7.5, 13.9)], Mat::Scr2);
    floor_hole(p, -8.0, 4.5, 0.5);
    floor_hole(p, -8.0, -6.5, -0.4);
    for x in [-26.0, -18.0, 6.0, 12.0] {
        cooling_unit(p, x, -18.7, Face::PosZ, false);
        cooling_unit(p, x, 18.7, Face::NegZ, false);
    }
    for z in [-10.8, 10.8] {
        for (x0, x1) in [(-30.0, -12.0), (-4.0, 14.0)] {
            cable_tray(p, pt(x0, z), pt(x1, z), false);
        }
        p.sign_near(-31.0, z, 2.6, 2.4, Sign::ColdAisle);
    }
    p.sign_near(0.0, -19.5, 2.8, 2.4, Sign::HotAisle);
    p.sign_near(0.0, 19.5, 2.8, 2.4, Sign::HotAisle);
    p.sign_near(14.0, 20.0, 2.8, 2.6, Sign::Compute);
    cable_tray(p, pt(-30.0, 0.0), pt(14.0, 0.0), false);
    cable_tray(p, pt(-8.0, -18.0), pt(-8.0, 18.0), false);
    pipe_run(p, pt(-31.0, 3.2), pt(15.0, 3.2), 4.22, Mat::Scr1);
    pipe_run(p, pt(-31.0, -3.2), pt(15.0, -3.2), 4.22, Mat::Red);
    p.sign_near(-31.5, 0.0, 3.0, 3.0, Sign::Uptime);
    p.sign_near(15.9, -10.8, 2.6, 2.6, Sign::Kubernetes);
    p.sign_near(15.9, 10.8, 2.6, 2.4, Sign::Rack42);
    p.sign_near(-31.7, -6.0, 2.0, 2.0, Sign::NoFood);
    wall_cabinet(p, -31.9, -3.6, false);
    wall_cabinet(p, -31.9, 3.6, false);
    power_strip(p, -31.9, 6.4);
    extinguisher(p, 15.9, -3.4);
    mainframe(p, 15.9, 6.3);

    // Lobby: reception, notices, vending, a sad plant.
    dc1_lobby(p);
    p.sign_near(-56.0, -6.0, 2.2, 2.0, Sign::DoNotUnplug);
    p.sign_near(-56.0, 6.0, 2.2, 2.0, Sign::ThisIsFine);
    vending(p, -43.0, -9.8, Face::PosZ);
    sad_plant(p, -55.5, 9.0);
    p.sign_near(-48.0, -11.0, 2.6, 3.0, Sign::Uptime);

    // GPU shrine room.
    gpu_shrine(p, -8.0, -39.5, Face::PosZ);
    p.sign_near(-8.0, -41.5, 3.4, 3.2, Sign::Shrine);
    for x in [-16.0, 0.0] {
        p.boxed(v(x, 0.6, -38.0), v(0.9, 1.2, 0.9), Mat::Rack, true);
        rack_front(p, x, -38.0, 0.0, false, 3 + x as i32, 1.1);
    }
    p.sign_near(-18.5, -33.0, 2.4, 2.0, Sign::Compute);
    shrine_extras(p);

    // NOC: three consoles facing the room, the cheerful board and the red wall.
    p.sign_near(-8.0, 40.5, 2.5, 6.0, Sign::NocBoard);
    noc_consoles(p);
    noc_wall(p);
    p.sign_near(-19.5, 33.0, 2.4, 2.0, Sign::ThisIsFine);

    // Boss hall: the job queue board over a bank of racks, a scheduler wheel on the floor.
    p.sign_near(47.0, -6.0, 2.8, 4.0, Sign::TempWarning);
    for z in [-9.0, 9.0] {
        rack_row(
            p,
            37.5,
            42.5,
            z,
            if z < 0.0 { 0.0 } else { PI },
            false,
            false,
        );
    }
    floor_ring(p, 38.0, 0.0, 6.0, 20, Mat::Cold, 0.1);
    floor_ring(p, 38.0, 0.0, 6.6, 20, Mat::Hazard, 0.08);
    wall_cabinet(p, 23.1, -5.5, false);
    extinguisher(p, 33.0, 13.9);

    // Warm light in the corridor, cold in the aisles: the contrast is the zone's identity.
    let aisles = [
        (-40.0, -9.0),
        (-44.0, 8.0),
        (-20.0, -10.8),
        (-20.0, 10.8),
        (0.0, -10.8),
        (0.0, 10.8),
        (-8.0, -33.0),
        (-8.0, 33.0),
        (20.0, 0.0),
        (34.0, 0.0),
    ];
    point_lights(p, &aisles, Color::srgb(0.55, 0.78, 1.0), 380_000.0);
    point_lights(
        p,
        &[(-52.0, 0.0), (-22.0, 0.0), (-8.0, 0.0), (8.0, 0.0)],
        Color::srgb(1.0, 0.72, 0.4),
        300_000.0,
    );
    strips(p, Mat::Cold, Mat::Candle, 2.0, 0.0);
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:campus", (-57.0, 0.0), Sign::ToCampus),
                ("door:datacenter/2", (47.5, 8.0), Sign::ToDataCenter2),
            ],
            packs: &[
                (-21.0, -10.8),
                (-21.0, 10.8),
                (7.0, 10.8),
                (7.0, -10.8),
                (-8.0, -34.0),
            ],
            boss: (38.0, 0.0),
            npc: Some((-8.0, 31.0)),
        },
    );
    theme(false)
}

fn dc2_lobby(p: &mut Plan) {
    runner(
        p,
        &[pt(-56.0, 0.0), pt(-48.0, -1.0), pt(-40.0, 0.0)],
        2.4,
        Mat::Runner,
        Mat::Hot,
    );
    // The ghost's corner: webs, tape and a collapsed ceiling tile or two.
    p.panel_near(-41.0, 8.5, 2.0, Vec2::new(1.8, 1.8), Tex::Cobweb);
    p.panel_near(-41.0, 8.5, 1.0, Vec2::new(1.2, 1.2), Tex::Cobweb);
    for (y, yaw) in [(1.05, 0.05), (0.75, -0.04)] {
        p.slab(v(-42.0, y, 6.6), yaw, v(0.04, 0.06, 2.6), Mat::Hazard);
    }
    for (x, z, yaw) in [(-44.0, -5.0, 0.4), (-45.2, -4.2, -0.3), (-50.0, 4.0, 0.9)] {
        p.slab(v(x, 0.03, z), yaw, v(0.6, 0.04, 0.6), Mat::Paper);
    }
    p.prop(&CHAIR_DESK, v(-47.5, 0.0, 6.0), 2.2, false);
    p.sign_near(-48.0, 10.0, 2.4, 2.4, Sign::Escort);
    extinguisher(p, -44.5, 10.0);
    wall_cabinet(p, -39.1, -4.5, true);
}

fn dc2_arena(p: &mut Plan) {
    for k in 0..6 {
        let a = (30.0 + k as f32 * 60.0).to_radians();
        let (cx, cz) = (36.0 + 9.0 * a.cos(), 9.0 * a.sin());
        for y in [0.15, H - 0.15] {
            p.prim(
                PrimKind::Cylinder,
                v(cx, y, cz),
                v(2.6, 0.3, 2.6),
                Mat::Dark,
            );
        }
        p.prim(
            PrimKind::Cylinder,
            v(cx, 1.25, cz),
            v(2.3, 0.3, 2.3),
            Mat::Hazard,
        );
        // Cables that used to feed the pillar-mounted racks now just lie between them.
        if k % 2 == 0 {
            let at = |deg: f32, r: f32| {
                let a = deg.to_radians();
                (36.0 + r * a.cos(), r * a.sin())
            };
            let deg = 30.0 + k as f32 * 60.0;
            cable_bundle(
                p,
                &[at(deg + 8.0, 7.6), at(deg + 30.0, 6.9), at(deg + 52.0, 7.6)],
                [Mat::Red, Mat::Yellow, Mat::Scr1][k / 2],
            );
        }
    }
    floor_ring(p, 36.0, 0.0, 6.5, 24, Mat::Hot, 0.12);
    floor_ring(p, 36.0, 0.0, 3.0, 12, Mat::Hazard, 0.1);
    // The ceiling fan that no longer spins.
    p.prim(
        PrimKind::Cylinder,
        v(36.0, 4.4, 0.0),
        v(3.6, 0.2, 3.6),
        Mat::Dark,
    );
    p.prim(
        PrimKind::Cylinder,
        v(36.0, 4.25, 0.0),
        v(0.5, 0.2, 0.5),
        Mat::Metal,
    );
    for k in 0..4 {
        let a = k as f32 * PI / 2.0 + 0.3;
        p.slab(
            v(36.0 + 0.95 * a.cos(), 4.25, 0.95 * a.sin()),
            -a,
            v(1.5, 0.04, 0.38),
            Mat::Metal,
        );
    }
    p.sign_near(36.0, -17.0, 3.0, 4.0, Sign::TempWarning);
    p.sign_near(52.0, 6.0, 2.4, 3.2, Sign::ThisIsFine);
    p.sign_near(48.0, -8.0, 2.8, 2.6, Sign::Kubernetes);
}

fn dc2_cooling_room(p: &mut Plan) {
    p.sign_near(-12.0, -41.5, 3.0, 3.2, Sign::TempWarning);
    for (x, z, yaw) in [(-18.0, -38.0, 0.0), (-6.0, -38.0, 0.4)] {
        p.slab(v(x, 1.1, z), yaw, v(2.2, 2.2, 1.0), Mat::Metal);
        p.slab(v(x, 1.0, z + 0.55), yaw, v(1.8, 1.4, 0.04), Mat::Hot);
        p.block(v(x, 1.1, z), v(2.4, 2.2, 1.6));
    }
    // Chiller tanks with burst pipes: coolant jets (see-through) and a spreading puddle.
    for x in [-14.5, -9.5] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 1.2, -39.6),
            v(1.2, 2.4, 1.2),
            Mat::Metal,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x, 2.45, -39.6),
            v(1.3, 0.1, 1.3),
            Mat::Dark,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x, (2.5 + H) / 2.0, -39.6),
            v(0.25, H - 2.5, 0.25),
            Mat::Metal,
        );
    }
    for (x, z) in [(-14.5, -38.7), (-9.5, -38.7)] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 1.6, z),
            v(0.3, 3.2, 0.3),
            Mat::Glass,
        );
        p.prim(PrimKind::Sphere, v(x, 0.2, z), v(1.4, 0.5, 1.4), Mat::Glass);
    }
    p.slab(v(-12.0, 0.01, -36.0), 0.2, v(6.0, 0.012, 3.2), Mat::Glass);
    p.slab(v(-11.2, 0.014, -35.2), -0.4, v(3.0, 0.012, 1.8), Mat::Glass);
    p.light(v(-12.0, 3.0, -36.5), Color::srgb(0.3, 0.8, 1.0), 260_000.0);
    p.sign_near(-21.9, -36.0, 2.4, 2.4, Sign::NoFood);
    wall_cabinet(p, -2.1, -36.0, true);
}

fn dc2(p: &mut Plan) -> Theme {
    let w = Mat::Wall;
    p.fp.pod(rounded_rect(pt(-48.0, 0.0), 9.0, 10.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-40.0, 0.0), pt(-30.0, 0.0)], 3.5), w);
    p.fp.pod(rounded_rect(pt(-12.0, 0.0), 20.0, 18.0, 6.0, 0.0), w);
    p.fp.pod(rounded_rect(pt(-12.0, -34.0), 10.0, 8.0, 4.0, 0.0), w);
    p.fp.link(stroke(&[pt(-10.0, -16.0), pt(-10.0, -30.0)], 3.5), w);
    p.fp.link(stroke(&[pt(2.0, 0.0), pt(24.0, 0.0)], 3.5), w);
    p.fp.pod(blob(pt(36.0, 0.0), 17.0, 17.0, 0.0, 0, 0.0), w);
    for k in 0..6 {
        let a = (30.0 + k as f32 * 60.0).to_radians();
        p.fp.hole(
            pillar(pt(36.0 + 9.0 * a.cos(), 9.0 * a.sin()), 1.0),
            Mat::Metal,
        );
    }
    p.route = vec![
        pt(-52.0, 0.0),
        pt(-30.0, 0.0),
        pt(-8.0, 0.0),
        pt(24.0, 0.0),
        pt(36.0, 0.0),
    ];
    p.build_walls();
    wall_bands(p, true);

    // Gutted rows around the central aisle; fronts face it. Gaps at x -20..-14 and -6..0.
    for (x0, x1) in [(-31.0, -20.0), (-14.0, -6.0), (0.0, 6.0)] {
        rack_row(p, x0, x1, -9.0, 0.0, true, true);
        rack_row(p, x0, x1, 9.0, PI, true, true);
    }
    // Rack 17 is on fire, as documented.
    p.prim(
        PrimKind::Cone,
        v(-24.5, 2.8, -9.0),
        v(0.6, 1.1, 0.6),
        Mat::Hot,
    );
    p.prim(
        PrimKind::Cone,
        v(-24.5, 2.7, -9.0),
        v(0.34, 0.8, 0.34),
        Mat::Candle,
    );
    p.sign_near(-18.0, -17.9, 2.6, 3.0, Sign::RackFire);
    for x in [-26.0, -10.0, 2.0] {
        cooling_unit(p, x, -16.7, Face::PosZ, true);
        cooling_unit(p, x, 16.7, Face::NegZ, true);
        // A puddle under every unit: they have been crying for a while.
        p.slab(
            v(x + 0.3, 0.01, if x < 0.0 { -15.2 } else { 15.2 }),
            0.3,
            v(2.4, 0.012, 1.4),
            Mat::Glass,
        );
    }
    p.sign_near(-31.0, -4.0, 3.0, 2.6, Sign::TempWarning);
    p.sign_near(-31.0, 4.0, 2.2, 2.2, Sign::ThisIsFine);
    p.sign_near(8.0, 17.0, 2.6, 2.6, Sign::HotAisle);
    p.sign_near(8.0, -17.0, 2.6, 2.6, Sign::HotAisle);
    p.sign_near(-40.0, -10.0, 2.6, 2.4, Sign::DoNotUnplug);
    p.sign_near(-56.0, 3.8, 2.2, 2.0, Sign::ToDataCenter1);
    p.sign_near(7.9, 8.0, 2.4, 2.6, Sign::AiTraining);
    cable_tray(p, pt(-30.0, 0.0), pt(24.0, 0.0), true);
    cable_tray(p, pt(-10.0, -16.0), pt(-10.0, -30.0), true);
    pipe_run(p, pt(-31.0, 4.5), pt(7.0, 4.5), 4.22, Mat::Red);
    pipe_run(p, pt(-31.0, -4.5), pt(7.0, -4.5), 4.22, Mat::Scr1);
    // Vents venting: glowing floor tiles along the aisle, a few of them steaming.
    for i in 0..6 {
        for z in [-3.5, 3.5] {
            vent_tile(p, -25.5 + i as f32 * 6.0, z, Mat::Hot);
        }
    }
    for (x, z) in [(-19.5, -3.5), (-7.5, 3.5), (4.5, -3.5), (-13.5, 3.5)] {
        steam(p, x, z, 3);
    }
    cable_bundle(p, &[(-18.5, -8.0), (-17.0, -4.5), (-18.8, -1.5)], Mat::Red);
    cable_bundle(p, &[(3.5, 8.0), (2.2, 4.0), (4.0, 1.2)], Mat::Yellow);
    floor_hole(p, -10.0, 0.5, 0.3);
    for (x, z, tip) in [
        (-22.0, -3.0, 2.6),
        (-12.0, 3.4, 2.9),
        (-3.0, -2.2, 2.4),
        (5.0, 4.0, 2.8),
    ] {
        hanging_cable(p, x, z, tip, true);
    }
    // Emergency strobes: static glow slabs with a red light each.
    for (x, z) in [(-31.9, 0.0), (-56.9, -5.0)] {
        if let Some(wall) = WallSpot::at(p, x, z) {
            wall.put(p, 0.0, 3.2, 0.08, v(0.3, 0.2, 0.1), Mat::NeonRed);
        }
    }
    p.light(v(-31.0, 3.0, 0.0), Color::srgb(1.0, 0.1, 0.05), 250_000.0);
    p.light(v(-55.0, 3.0, -5.0), Color::srgb(1.0, 0.1, 0.05), 250_000.0);
    extinguisher(p, -31.9, -8.0);
    dc2_lobby(p);
    dc2_cooling_room(p);
    dc2_arena(p);

    let pts = [
        (-52.0, 0.0),
        (-44.0, 6.0),
        (-22.0, -13.0),
        (-22.0, 13.0),
        (-3.0, -13.0),
        (-3.0, 13.0),
        (-12.0, -35.0),
        (14.0, 0.0),
        (30.0, -8.0),
        (30.0, 8.0),
        (42.0, 0.0),
        (-12.0, 0.0),
    ];
    point_lights(p, &pts, Color::srgb(1.0, 0.45, 0.2), 520_000.0);
    strips(p, Mat::Hot, Mat::Hot, 0.0, 0.3);
    cast_zone(
        p,
        &ZoneCast {
            doors: &[
                ("door:datacenter/1", (-57.0, 0.0), Sign::ToDataCenter1),
                ("door:campus", (-48.0, -10.0), Sign::ToCampus),
            ],
            packs: &[
                (-24.0, -14.0),
                (-24.0, 14.0),
                (-2.0, -14.0),
                (-2.0, 14.0),
                (-12.0, -35.0),
            ],
            boss: (43.0, 0.0),
            npc: Some((-44.0, -7.0)),
        },
    );
    theme(true)
}
