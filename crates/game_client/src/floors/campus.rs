//! The outdoor hub: a generic corporate campus, 140 x 100 m, with the arrival parking lot,
//! a cafeteria pavilion, a smoking corner, the reception plaza and three building facades
//! (tower, data center, innovation lab). Buildings are single large blockers; the entrances
//! are the door anchors. Everything is primitives so it stays light on WebGL2.

use super::kit::*;
use super::layout::{FloorKind, FloorLayout, Theme, assemble, yaw_towards};
use super::props::bench;
use super::shape::rounded_rect;
use super::themes::{pt, route_point_near, runner, v};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

pub const HALF_X: f32 = 70.0;
pub const HALF_Z: f32 = 50.0;

const SPAWN: (f32, f32) = (-23.0, 22.0);
const GRAVEYARD: (f32, f32) = (-15.0, 28.5);
const PACKS: [(f32, f32); 8] = [
    (-57.0, 8.0),
    (-45.0, 41.0),
    (-31.0, -37.0),
    (30.0, -39.0),
    (24.0, 31.0),
    (56.0, 28.0),
    (60.0, -39.0),
    (-8.0, 43.0),
];
/// intern, facilities_guy, barista, smoking_corner_dev, two ambient.
const NPCS: [(f32, f32); 6] = [
    (-19.0, 19.0),
    (36.5, -2.0),
    (-34.0, 6.2),
    (46.0, 22.5),
    (8.0, 10.0),
    (-9.0, 15.0),
];
const PLAZA: (f32, f32) = (0.0, 2.0);

const TOWER: (Vec2, Vec3) = (Vec2::new(0.0, -36.0), Vec3::new(36.0, 26.0, 22.0));
const DATA_CENTER: (Vec2, Vec3) = (Vec2::new(54.0, -8.0), Vec3::new(24.0, 11.0, 30.0));
const LAB: (Vec2, Vec3) = (Vec2::new(-50.0, -36.0), Vec3::new(26.0, 9.0, 22.0));

fn theme() -> Theme {
    let c = Color::srgb;
    Theme {
        wall: c(0.7, 0.7, 0.72),
        trim: c(0.2, 0.22, 0.26),
        accent: LinearRgba::rgb(0.3, 1.2, 1.6),
        fabric: [c(0.25, 0.4, 0.55), c(0.35, 0.33, 0.5), c(0.22, 0.45, 0.42)],
        floor: FloorKind::Grass,
        // Horizon haze: also the clear colour, so the far ground melts into the sky.
        fog: c(0.62, 0.77, 0.93),
        ambient: c(0.8, 0.88, 1.0),
        ambient_brightness: 900.0,
        sun: c(1.0, 0.96, 0.86),
        sun_illuminance: 16000.0,
        fog_range: (55.0, 170.0),
    }
}

pub fn build() -> FloorLayout {
    let mut p = Plan::default();
    p.fp.pod(
        rounded_rect(Vec2::ZERO, HALF_X, HALF_Z, 1.0, 0.0),
        Mat::Wall,
    );
    ground(&mut p);
    perimeter(&mut p);
    let doors = [
        (
            "door:tower/1",
            facade_door(&mut p, TOWER, Face::PosZ, Sign::TowerName),
        ),
        (
            "door:datacenter/1",
            facade_door(&mut p, DATA_CENTER, Face::NegX, Sign::DataCenterName),
        ),
        (
            "door:lab/1",
            facade_door(&mut p, LAB, Face::PosZ, Sign::InnovationNeon),
        ),
    ];
    tower_shell(&mut p);
    data_center_shell(&mut p);
    lab_shell(&mut p);
    parking_lot(&mut p);
    pavilion(&mut p);
    smoking_corner(&mut p);
    plaza(&mut p);
    graveyard(&mut p);
    lawn(&mut p);
    ambient_props(&mut p);

    let door = |n: usize| doors[n].1;
    p.route = vec![
        pt(SPAWN.0, SPAWN.1),
        pt(PLAZA.0, PLAZA.1),
        pt(door(2).0.x, door(2).0.z),
        pt(PLAZA.0, PLAZA.1),
        pt(door(0).0.x, door(0).0.z),
        pt(PLAZA.0, PLAZA.1),
        pt(door(1).0.x, door(1).0.z),
    ];
    trees(&mut p, &doors);

    let spawn = v(SPAWN.0, 0.0, SPAWN.1);
    p.anchor(
        "player_spawn",
        spawn,
        yaw_towards(spawn, v(PLAZA.0, 0.0, PLAZA.1)),
    );
    let grave = v(GRAVEYARD.0, 0.0, GRAVEYARD.1);
    p.anchor("graveyard", grave, yaw_towards(grave, spawn));
    let names = [
        "pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "pack_6", "pack_7", "pack_8",
    ];
    for (name, (x, z)) in names.iter().zip(PACKS) {
        let at = v(x, 0.0, z);
        let to = route_point_near(&p.route, pt(x, z));
        p.anchor(name, at, yaw_towards(at, v(to.x, 0.0, to.y)));
    }
    let npc_names = ["npc_1", "npc_2", "npc_3", "npc_4", "npc_5", "npc_6"];
    for (name, (x, z)) in npc_names.iter().zip(NPCS) {
        let at = v(x, 0.0, z);
        // Friendly NPCs look at the arrival spot, or across the counter for the barista.
        let target = if name == &"npc_3" {
            v(x, 0.0, z + 4.0)
        } else {
            spawn
        };
        p.anchor(name, at, yaw_towards(at, target));
    }
    for (name, (pos, yaw)) in doors {
        p.anchor(name, pos, yaw);
    }
    assemble("campus", 0, "Synergy Campus", true, p, theme())
}

/// Far lawn under the fog, plus the lighter inner lawn borders.
fn ground(p: &mut Plan) {
    p.slab(v(0.0, -0.06, 0.0), 0.0, v(900.0, 0.1, 900.0), Mat::LeafDark);
}

fn perimeter(p: &mut Plan) {
    for (x, z, sx, sz) in [
        (0.0, -HALF_Z - 0.5, 2.0 * HALF_X + 4.0, 1.0),
        (0.0, HALF_Z + 0.5, 2.0 * HALF_X + 4.0, 1.0),
        (-HALF_X - 0.5, 0.0, 1.0, 2.0 * HALF_Z),
        (HALF_X + 0.5, 0.0, 1.0, 2.0 * HALF_Z),
    ] {
        p.boxed(v(x, 1.2, z), v(sx, 2.4, sz), Mat::LeafDark, true);
    }
}

/// Entrance anchor in front of the facade centre: 1.8 m out, facing away from the wall.
fn facade_door(p: &mut Plan, (c, size): (Vec2, Vec3), face: Face, sign: Sign) -> (Vec3, f32) {
    let d = face.dir();
    let dir = Vec2::new(d.x, d.z);
    let half = if d.x.abs() > 0.5 { size.x } else { size.z } / 2.0;
    let wall = c + dir * half;
    p.framed(v(wall.x, 0.0, wall.y), face.yaw(), |p| {
        p.boxed(v(0.0, 1.5, 0.12), v(4.0, 3.0, 0.24), Mat::Dark, false);
        for l in [-0.9, 0.9] {
            p.boxed(v(l, 1.15, 0.26), v(1.6, 2.3, 0.08), Mat::Glass, false);
        }
        // Canopy.
        p.boxed(v(0.0, 3.3, 1.6), v(5.2, 0.25, 3.4), Mat::Trim, false);
        p.boxed(v(-2.4, 1.6, 3.0), v(0.2, 3.2, 0.2), Mat::Metal, false);
        p.boxed(v(2.4, 1.6, 3.0), v(0.2, 3.2, 0.2), Mat::Metal, false);
        p.boxed(v(0.0, 0.02, 1.8), v(5.0, 0.05, 3.6), Mat::Concrete, false);
        match sign {
            Sign::InnovationNeon => neon_sign(p),
            _ => p.sign(v(0.0, 4.4, 0.2), 0.0, 6.0, sign),
        }
    });
    let at = wall + dir * 1.8;
    (Vec3::new(at.x, 0.0, at.y), (-dir.x).atan2(-dir.y))
}

/// The lab's neon sign: a big lit panel in a tube frame, in the local frame of the entrance.
fn neon_sign(p: &mut Plan) {
    p.sign(v(0.0, 6.0, 0.25), 0.0, 12.0, Sign::InnovationNeon);
    for (x, y, sx, sy) in [
        (0.0, 7.65, 12.2, 0.1),
        (0.0, 4.35, 12.2, 0.1),
        (-6.05, 6.0, 0.1, 3.3),
        (6.05, 6.0, 0.1, 3.3),
    ] {
        p.boxed(v(x, y, 0.3), v(sx, sy, 0.1), Mat::NeonPink, false);
    }
}

fn building(p: &mut Plan, (c, size): (Vec2, Vec3), mat: Mat) {
    p.boxed(v(c.x, size.y / 2.0, c.y), size, mat, true);
}

/// Horizontal bands and vertical mullions so a blank block reads as a facade.
fn bands(p: &mut Plan, (c, size): (Vec2, Vec3), step: f32, mat: Mat) {
    let n = (size.y / step).floor() as i32;
    for i in 1..=n {
        let y = i as f32 * step;
        p.boxed(
            v(c.x, y, c.y),
            v(size.x + 0.3, 0.25, size.z + 0.3),
            mat,
            false,
        );
    }
    for k in 1..(size.x / 4.0) as i32 {
        let x = c.x - size.x / 2.0 + k as f32 * 4.0;
        for s in [-1.0, 1.0] {
            p.boxed(
                v(x, size.y / 2.0, c.y + s * (size.z / 2.0 + 0.1)),
                v(0.25, size.y, 0.2),
                mat,
                false,
            );
        }
    }
}

fn tower_shell(p: &mut Plan) {
    building(p, TOWER, Mat::Facade);
    bands(p, TOWER, 3.5, Mat::White);
    let (c, s) = TOWER;
    p.boxed(
        v(c.x, s.y + 0.4, c.y),
        v(s.x + 0.6, 0.8, s.z + 0.6),
        Mat::Trim,
        false,
    );
    p.boxed(
        v(c.x + 8.0, s.y + 1.4, c.y),
        v(5.0, 1.2, 5.0),
        Mat::Metal,
        false,
    );
    p.prim(
        PrimKind::Cylinder,
        v(c.x - 6.0, s.y + 3.0, c.y),
        v(0.3, 5.0, 0.3),
        Mat::Chrome,
    );
    p.prim(
        PrimKind::Sphere,
        v(c.x - 6.0, s.y + 5.6, c.y),
        v(0.7, 0.7, 0.7),
        Mat::LedRed,
    );
}

fn data_center_shell(p: &mut Plan) {
    building(p, DATA_CENTER, Mat::Metal);
    let (c, s) = DATA_CENTER;
    // Louvres on the long faces.
    for i in 0..9 {
        let y = 2.0 + i as f32 * 1.0;
        for sx in [-1.0, 1.0] {
            p.boxed(
                v(c.x + sx * (s.x / 2.0 + 0.05), y, c.y + 5.0),
                v(0.1, 0.35, 16.0),
                Mat::Dark,
                false,
            );
        }
    }
    // Cooling units on the roof: boxes with a fan disc each.
    for (i, (dx, dz)) in [
        (-6.0, -9.0),
        (0.0, -9.0),
        (6.0, -9.0),
        (-6.0, 0.0),
        (0.0, 0.0),
        (6.0, 0.0),
        (-6.0, 9.0),
        (6.0, 9.0),
    ]
    .into_iter()
    .enumerate()
    {
        let (x, z) = (c.x + dx, c.y + dz);
        p.boxed(v(x, s.y + 1.0, z), v(4.2, 2.0, 4.2), Mat::Concrete, false);
        p.prim(
            PrimKind::Cylinder,
            v(x, s.y + 2.05, z),
            v(3.4, 0.12, 3.4),
            Mat::Dark,
        );
        p.prim(
            PrimKind::Cylinder,
            v(x, s.y + 2.12, z),
            v(0.7, 0.2, 0.7),
            Mat::Chrome,
        );
        for k in 0..3 {
            let a = k as f32 * PI / 3.0 + i as f32 * 0.4;
            p.slab(v(x, s.y + 2.14, z), a, v(3.0, 0.05, 0.35), Mat::Chrome);
        }
        if i % 3 == 0 {
            p.boxed(
                v(x + 1.5, s.y + 2.3, z + 1.5),
                v(0.3, 0.3, 0.3),
                Mat::LedRed,
                false,
            );
        }
    }
    // Loading bay shutter and hazard stripes on the west face.
    p.boxed(
        v(c.x - s.x / 2.0 - 0.05, 2.0, c.y + 8.0),
        v(0.2, 4.0, 6.0),
        Mat::Dark,
        false,
    );
    for i in 0..6 {
        let z = c.y + 5.6 + i as f32 * 1.0;
        p.boxed(
            v(c.x - s.x / 2.0 - 0.12, 4.15, z),
            v(0.1, 0.3, 0.5),
            Mat::Hazard,
            false,
        );
    }
    p.sign(
        v(c.x - s.x / 2.0 - 0.1, 8.0, c.y - 8.0),
        -FRAC_PI_2,
        7.0,
        Sign::Compute,
    );
}

fn lab_shell(p: &mut Plan) {
    building(p, LAB, Mat::White);
    let (c, s) = LAB;
    // Ribbon window and roof trim.
    for sz in [-1.0, 1.0] {
        p.boxed(
            v(c.x, 3.2, c.y + sz * (s.z / 2.0 + 0.05)),
            v(s.x - 2.0, 2.0, 0.1),
            Mat::Glass,
            false,
        );
    }
    p.boxed(
        v(c.x, s.y + 0.15, c.y),
        v(s.x + 0.4, 0.3, s.z + 0.4),
        Mat::Trim,
        false,
    );
    // Glass dome over a glowing core.
    p.prim(
        PrimKind::Sphere,
        v(c.x, s.y + 0.2, c.y),
        v(15.0, 15.0, 15.0),
        Mat::Glass,
    );
    p.prim(
        PrimKind::Sphere,
        v(c.x, s.y + 2.2, c.y),
        v(3.0, 3.0, 3.0),
        Mat::NeonPink,
    );
    p.prim(
        PrimKind::Cylinder,
        v(c.x, s.y + 1.0, c.y),
        v(0.4, 2.0, 0.4),
        Mat::Chrome,
    );
    for k in 0..8 {
        let a = k as f32 * PI / 4.0;
        p.slab(v(c.x, s.y + 0.4, c.y), a, v(15.0, 0.06, 0.08), Mat::Chrome);
    }
    p.sign(
        v(c.x + s.x / 2.0 + 0.1, 5.0, c.y),
        FRAC_PI_2,
        6.0,
        Sign::InnovationHere,
    );
}

fn parking_lot(p: &mut Plan) {
    p.slab(
        v(-45.0, 0.015, 34.0),
        0.0,
        v(42.0, 0.03, 28.0),
        Mat::Asphalt,
    );
    // Bay lines: three rows, bays 3 m wide.
    for (z0, z1) in [(21.25, 26.75), (33.25, 38.75), (38.75, 44.25)] {
        for i in 0..=13 {
            let x = -64.5 + i as f32 * 3.0;
            p.slab(
                v(x, 0.04, (z0 + z1) / 2.0),
                0.0,
                v(0.1, 0.02, z1 - z0),
                Mat::White,
            );
        }
        p.slab(v(-45.0, 0.04, z0), 0.0, v(39.0, 0.02, 0.1), Mat::White);
    }
    p.slab(v(-45.0, 0.04, 44.25), 0.0, v(39.0, 0.02, 0.1), Mat::White);
    // Kerb along the north edge of the lot.
    p.boxed(v(-45.0, 0.1, 19.7), v(42.0, 0.2, 0.4), Mat::Concrete, false);
    let cars = [
        (-61.5, 24.0, Mat::CarRed),
        (-52.5, 24.0, Mat::CarBlue),
        (-43.5, 24.0, Mat::CarWhite),
        (-34.5, 24.0, Mat::CarTeal),
        (-58.5, 36.0, Mat::CarWhite),
        (-46.5, 36.0, Mat::CarRed),
        (-31.5, 36.0, Mat::CarBlue),
        (-64.5 + 1.5, 41.5, Mat::CarTeal),
        (-58.5, 41.5, Mat::CarWhite),
        (-31.5, 41.5, Mat::CarRed),
    ];
    for (x, z, m) in cars {
        car(p, x, z, false, m);
    }
    ai_car(p, -39.0, 36.0);
    p.sign(v(-45.0, 1.6, 19.75), 0.0, 4.0, Sign::ParkingAi);
    p.sign(v(-45.0, 1.6, 19.45), PI, 4.0, Sign::ParkingAi);
    p.boxed(v(-47.0, 0.8, 19.5), v(0.15, 1.6, 0.15), Mat::Metal, false);
    p.boxed(v(-43.0, 0.8, 19.5), v(0.15, 1.6, 0.15), Mat::Metal, false);
}

/// Simple car: body and cabin slabs, box wheels, one collider.
fn car(p: &mut Plan, x: f32, z: f32, along_x: bool, mat: Mat) {
    let yaw = if along_x { 0.0 } else { FRAC_PI_2 };
    let rot = |lx: f32, lz: f32| {
        let (s, c) = yaw.sin_cos();
        (x + lx * c + lz * s, z - lx * s + lz * c)
    };
    p.slab(v(x, 0.62, z), yaw, v(4.4, 0.7, 1.8), mat);
    let (cx, cz) = rot(-0.2, 0.0);
    p.slab(v(cx, 1.2, cz), yaw, v(2.4, 0.55, 1.6), Mat::Facade);
    for (lx, lz) in [(1.4, 0.85), (1.4, -0.85), (-1.4, 0.85), (-1.4, -0.85)] {
        let (wx, wz) = rot(lx, lz);
        p.slab(v(wx, 0.3, wz), yaw, v(0.75, 0.6, 0.25), Mat::Tire);
    }
    for lz in [-0.6, 0.6] {
        let (hx, hz) = rot(2.2, lz);
        p.slab(v(hx, 0.7, hz), yaw, v(0.06, 0.15, 0.3), Mat::NeonYellow);
    }
    let size = if along_x {
        v(4.4, 1.5, 1.8)
    } else {
        v(1.8, 1.5, 4.4)
    };
    p.block(v(x, 0.75, z), size);
}

/// The "AI-powered" car: parked across three bays, with a lidar dome and its own opinions.
fn ai_car(p: &mut Plan, x: f32, z: f32) {
    car(p, x, z, true, Mat::CarWhite);
    p.prim(
        PrimKind::Cylinder,
        v(x - 0.2, 1.65, z),
        v(0.45, 0.3, 0.45),
        Mat::NeonCyan,
    );
    p.slab(v(x, 0.72, z - 0.91), 0.0, v(1.5, 0.3, 0.02), Mat::NeonCyan);
    p.sign(v(x, 1.0, z - 0.93), PI, 2.4, Sign::AiCar);
    p.sign(v(x, 1.0, z + 0.93), 0.0, 2.4, Sign::AiCar);
}

fn pavilion(p: &mut Plan) {
    let (cx, cz) = (-34.0, 10.0);
    p.boxed(v(cx, 0.06, cz), v(14.0, 0.12, 10.0), Mat::Concrete, false);
    for (dx, dz) in [
        (-6.5, -4.5),
        (0.0, -4.5),
        (6.5, -4.5),
        (-6.5, 4.5),
        (0.0, 4.5),
        (6.5, 4.5),
    ] {
        p.boxed(v(cx + dx, 1.8, cz + dz), v(0.3, 3.6, 0.3), Mat::Metal, true);
    }
    // Roof: slab plus a lighter underside lip.
    p.boxed(v(cx, 3.75, cz), v(15.0, 0.3, 11.0), Mat::Trim, false);
    p.boxed(v(cx, 3.95, cz), v(15.4, 0.12, 11.4), Mat::Wood, false);
    p.sign(v(cx, 3.0, cz - 5.05), 0.0, 5.0, Sign::Cafe);
    // Counter along the north side; the barista stands behind it.
    p.boxed(v(cx, 0.55, cz - 2.2), v(9.0, 1.1, 0.9), Mat::Wood, true);
    p.boxed(v(cx, 1.12, cz - 2.2), v(9.2, 0.06, 1.0), Mat::Marble, false);
    p.boxed(
        v(cx - 3.0, 1.45, cz - 2.2),
        v(0.6, 0.6, 0.5),
        Mat::Chrome,
        false,
    );
    p.boxed(
        v(cx + 3.0, 1.35, cz - 2.2),
        v(0.5, 0.4, 0.4),
        Mat::LedGreen,
        false,
    );
    p.boxed(v(cx, 1.0, cz - 3.4), v(9.0, 2.0, 0.5), Mat::Rack, false);
    for k in 0..6 {
        p.boxed(
            v(cx - 3.5 + k as f32 * 1.4, 1.6, cz - 3.1),
            v(0.35, 0.5, 0.2),
            Mat::Paper,
            false,
        );
    }
    // Tables with stools.
    for (dx, dz) in [(-4.5, 2.2), (-1.5, 2.2), (1.5, 2.2), (4.5, 2.2)] {
        p.prim(
            PrimKind::Cylinder,
            v(cx + dx, 0.75, cz + dz),
            v(1.1, 0.06, 1.1),
            Mat::White,
        );
        p.prim(
            PrimKind::Cylinder,
            v(cx + dx, 0.37, cz + dz),
            v(0.15, 0.74, 0.15),
            Mat::Metal,
        );
        p.block(v(cx + dx, 0.4, cz + dz), v(1.1, 0.8, 1.1));
        for (sx, sz) in [(-0.9, 0.0), (0.9, 0.0)] {
            p.prim(
                PrimKind::Cylinder,
                v(cx + dx + sx, 0.25, cz + dz + sz),
                v(0.4, 0.5, 0.4),
                Mat::BeanB,
            );
        }
    }
    p.sign(v(cx - 7.05, 1.8, cz + 3.0), FRAC_PI_2, 2.2, Sign::NoFeed);
}

fn smoking_corner(p: &mut Plan) {
    let (cx, cz) = (46.0, 24.0);
    p.boxed(v(cx, 0.04, cz), v(9.0, 0.08, 7.0), Mat::Concrete, false);
    // One sad bench facing the wall, one ashtray, a bin.
    bench(p, cx, cz + 1.8, PI);
    p.boxed(
        v(cx - 3.0, 0.5, cz - 0.5),
        v(0.35, 1.0, 0.35),
        Mat::Metal,
        true,
    );
    p.prim(
        PrimKind::Cylinder,
        v(cx - 3.0, 1.03, cz - 0.5),
        v(0.5, 0.08, 0.5),
        Mat::Chrome,
    );
    for k in 0..5 {
        p.boxed(
            v(cx - 3.0 + k as f32 * 0.07, 1.1, cz - 0.55),
            v(0.04, 0.03, 0.12),
            Mat::Paper,
            false,
        );
    }
    p.boxed(v(cx, 1.4, cz - 3.6), v(9.0, 2.8, 0.3), Mat::Brick, true);
    p.sign(v(cx, 1.9, cz - 3.4), 0.0, 3.6, Sign::SmokingCorner);
    p.sign(v(cx + 3.6, 1.7, cz - 3.4), 0.0, 1.6, Sign::ThisIsFine);
    p.prim(
        PrimKind::Cylinder,
        v(cx + 3.0, 0.4, cz - 1.0),
        v(0.6, 0.8, 0.6),
        Mat::Dark,
    );
    p.block(v(cx + 3.0, 0.4, cz - 1.0), v(0.6, 0.8, 0.6));
}

fn plaza(p: &mut Plan) {
    // Paved plaza, the paths to each door and the walk from the lot.
    p.boxed(
        v(PLAZA.0, 0.025, PLAZA.1),
        v(36.0, 0.05, 24.0),
        Mat::Concrete,
        false,
    );
    for ctrl in [
        vec![pt(0.0, -9.0), pt(0.0, -23.0)],
        vec![pt(-14.0, -4.0), pt(-32.0, -14.0), pt(-50.0, -22.0)],
        vec![pt(14.0, 0.0), pt(28.0, -6.0), pt(40.0, -8.0)],
        vec![pt(-23.0, 22.0), pt(-12.0, 11.0)],
    ] {
        runner(p, &ctrl, 5.0, Mat::Concrete, Mat::White);
    }
    // Reception: a kiosk with a sign, flagpoles, planters and benches.
    p.boxed(v(10.0, 0.55, -4.0), v(5.0, 1.1, 1.2), Mat::Wood, true);
    p.boxed(v(10.0, 1.12, -4.0), v(5.2, 0.06, 1.3), Mat::Marble, false);
    p.sign(v(10.0, 2.4, -4.55), 0.0, 3.6, Sign::Reception);
    p.boxed(v(7.7, 1.2, -4.5), v(0.12, 2.4, 0.12), Mat::Metal, false);
    p.boxed(v(12.3, 1.2, -4.5), v(0.12, 2.4, 0.12), Mat::Metal, false);
    p.sign(v(0.0, 3.4, 12.75), 0.0, 9.0, Sign::Welcome);
    p.sign(v(0.0, 3.4, 12.45), PI, 9.0, Sign::Welcome);
    p.boxed(v(-4.4, 1.7, 12.7), v(0.2, 3.4, 0.2), Mat::Metal, false);
    p.boxed(v(4.4, 1.7, 12.7), v(0.2, 3.4, 0.2), Mat::Metal, false);
    for (i, x) in [-6.0, -4.0, -2.0].into_iter().enumerate() {
        p.prim(
            PrimKind::Cylinder,
            v(x, 3.5, 7.5),
            v(0.12, 7.0, 0.12),
            Mat::Chrome,
        );
        p.block(v(x, 1.0, 7.5), v(0.3, 2.0, 0.3));
        let flag = [Mat::CarBlue, Mat::White, Mat::CarRed][i];
        p.slab(v(x + 0.6, 6.3, 7.5), 0.0, v(1.1, 0.7, 0.04), flag);
    }
    for (x, z, yaw) in [
        (-12.0, 8.0, 0.0),
        (12.0, 9.0, 0.0),
        (16.0, -3.0, PI),
        (-15.0, -3.0, PI),
        (6.0, 15.0, 0.0),
    ] {
        bench(p, x, z, yaw);
    }
    for (x, z) in [(-16.0, 12.0), (16.0, 12.0), (-16.0, -8.0), (16.0, -8.0)] {
        p.boxed(v(x, 0.3, z), v(1.6, 0.6, 1.6), Mat::Concrete, true);
        p.prim(PrimKind::Sphere, v(x, 0.9, z), v(1.3, 1.1, 1.3), Mat::Plant);
    }
    // Lamp posts along the paths.
    for (x, z) in [
        (-3.5, -14.0),
        (3.5, -14.0),
        (-24.0, -11.0),
        (22.0, -4.0),
        (-17.0, 17.0),
        (30.0, -9.5),
    ] {
        p.prim(
            PrimKind::Cylinder,
            v(x, 2.0, z),
            v(0.14, 4.0, 0.14),
            Mat::Metal,
        );
        p.prim(
            PrimKind::Sphere,
            v(x, 4.1, z),
            v(0.45, 0.35, 0.45),
            Mat::LightPanel,
        );
    }
}

/// Lawn dressing so the grass between the buildings is not empty: a pond, picnic tables,
/// flower beds and a statue of the Thought Leader.
fn lawn(p: &mut Plan) {
    p.prim(
        PrimKind::Cylinder,
        v(14.0, 0.06, -31.0),
        v(11.0, 0.12, 11.0),
        Mat::Concrete,
    );
    p.prim(
        PrimKind::Cylinder,
        v(14.0, 0.12, -31.0),
        v(10.0, 0.06, 10.0),
        Mat::Water,
    );
    p.block(v(14.0, 0.3, -31.0), v(10.0, 0.6, 10.0));
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + 0.6;
        p.prim(
            PrimKind::Sphere,
            v(14.0 + 4.2 * a.cos(), 0.35, -31.0 + 4.2 * a.sin()),
            v(0.9, 0.5, 0.9),
            Mat::Plant,
        );
    }
    for (x, z) in [
        (-14.0, 30.0),
        (-4.0, 34.0),
        (32.0, 34.0),
        (60.0, 12.0),
        (-58.0, -8.0),
    ] {
        p.boxed(v(x, 0.75, z), v(2.0, 0.08, 0.9), Mat::Wood, true);
        for dz in [-0.8, 0.8] {
            p.boxed(v(x, 0.45, z + dz), v(2.0, 0.07, 0.35), Mat::Wood, false);
        }
        for dx in [-0.8, 0.8] {
            p.boxed(v(x + dx, 0.38, z), v(0.1, 0.76, 1.8), Mat::Metal, false);
        }
    }
    for (i, (x, z)) in [
        (-6.0, 20.0),
        (6.0, 20.0),
        (-20.0, 6.0),
        (20.0, 6.0),
        (-6.0, -14.0),
        (6.0, -14.0),
    ]
    .into_iter()
    .enumerate()
    {
        p.boxed(v(x, 0.15, z), v(3.0, 0.3, 1.2), Mat::Bark, false);
        for k in 0..5 {
            let m = [Mat::NeonPink, Mat::Yellow, Mat::Red, Mat::BeanB, Mat::White][(i + k) % 5];
            p.boxed(
                v(x - 1.2 + k as f32 * 0.6, 0.38, z),
                v(0.3, 0.2, 0.3),
                m,
                false,
            );
        }
    }
    // The Thought Leader: a gold figure on a plinth, one arm up.
    p.boxed(v(14.0, 0.5, 6.0), v(1.6, 1.0, 1.6), Mat::Concrete, true);
    p.boxed(v(14.0, 1.7, 6.0), v(0.5, 1.4, 0.3), Mat::Gold, false);
    p.prim(
        PrimKind::Sphere,
        v(14.0, 2.6, 6.0),
        v(0.45, 0.45, 0.45),
        Mat::Gold,
    );
    p.slab(v(14.3, 2.3, 6.0), 0.0, v(0.1, 0.9, 0.1), Mat::Gold);
    p.sign(v(14.0, 0.55, 5.15), 0.0, 1.0, Sign::AiStrategy);
}

fn graveyard(p: &mut Plan) {
    let (gx, gz) = GRAVEYARD;
    p.boxed(v(gx, 0.03, gz), v(7.0, 0.06, 4.0), Mat::Sand, false);
    for (dx, dz) in [(-2.4, 1.6), (2.4, 1.6), (0.0, 1.9)] {
        p.boxed(
            v(gx + dx, 0.5, gz + dz),
            v(0.9, 1.0, 0.25),
            Mat::Concrete,
            true,
        );
        p.boxed(
            v(gx + dx, 1.05, gz + dz),
            v(0.5, 0.2, 0.25),
            Mat::Concrete,
            false,
        );
    }
    p.sign(v(gx, 1.1, gz + 2.1), PI, 1.2, Sign::GraveSprint);
}

fn ambient_props(p: &mut Plan) {
    // Days without an AI incident: 0. On posts beside the plaza, readable from the lot.
    for x in [-9.5, -4.5] {
        p.boxed(v(x, 1.5, -9.0), v(0.2, 3.0, 0.2), Mat::Metal, false);
    }
    p.sign(v(-7.0, 3.1, -8.85), 0.0, 5.0, Sign::DaysAi);
    // AI billboard on the south edge.
    for x in [-14.0, -2.0] {
        p.boxed(v(x, 3.0, 47.5), v(0.4, 6.0, 0.4), Mat::Metal, true);
    }
    p.sign(v(-8.0, 6.5, 47.2), PI, 12.0, Sign::Billboard);
    p.sign(v(26.0, 3.2, -15.0), 0.0, 4.0, Sign::AiStrategy);
    p.boxed(v(26.0, 1.5, -15.15), v(0.2, 3.0, 0.2), Mat::Metal, false);
    p.sign(v(-26.0, 3.0, -10.0), 0.0, 4.0, Sign::Compute);
    p.boxed(v(-26.0, 1.5, -10.15), v(0.2, 3.0, 0.2), Mat::Metal, false);
    // Scooters littering the walkways; four more are borrowable (`office::CAMPUS_SCOOTERS`).
    for (i, (x, z)) in [
        (-4.0, -12.0),
        (-17.0, -7.0),
        (-28.0, -14.0),
        (30.0, -7.5),
        (-14.0, 13.0),
        (35.0, -5.0),
    ]
    .into_iter()
    .enumerate()
    {
        scooter(p, x, z, i as f32 * 1.7, i % 3 == 0);
    }
    p.sign(v(-3.0, 1.2, -11.0), 0.0, 1.4, Sign::ScooterZone);
    p.boxed(v(-3.0, 0.5, -10.85), v(0.1, 1.0, 0.1), Mat::Metal, false);
    // The lonely bike rack with its one bike.
    for k in 0..4 {
        p.boxed(
            v(14.0 + k as f32 * 0.5, 0.4, -14.0),
            v(0.05, 0.8, 0.05),
            Mat::Metal,
            false,
        );
    }
    p.boxed(v(14.75, 0.8, -14.0), v(1.7, 0.05, 0.05), Mat::Metal, false);
    p.slab(
        v(14.75, 0.45, -14.0),
        FRAC_PI_2,
        v(1.5, 0.04, 0.04),
        Mat::CarRed,
    );
    p.prim(
        PrimKind::Cylinder,
        v(14.2, 0.3, -14.0),
        v(0.55, 0.05, 0.55),
        Mat::Tire,
    );
    p.sign(v(15.0, 1.4, -14.4), 0.0, 1.6, Sign::BikeRack);
    p.block(v(14.75, 0.5, -14.0), v(2.0, 1.0, 0.4));
    // Shuttle stop that nobody has seen a shuttle at.
    p.boxed(v(-16.0, 1.2, 21.0), v(0.15, 2.4, 0.15), Mat::Metal, false);
    p.sign(v(-16.0, 2.7, 21.1), 0.0, 1.6, Sign::ShuttleBus);
}

/// Kick scooter: deck, stem, bars and two wheels; low enough to step over.
fn scooter(p: &mut Plan, x: f32, z: f32, yaw: f32, fallen: bool) {
    let (s, c) = yaw.sin_cos();
    let at = |lx: f32, lz: f32| (x + lx * c + lz * s, z - lx * s + lz * c);
    p.slab(v(x, 0.1, z), yaw, v(0.18, 0.04, 0.7), Mat::Metal);
    for lz in [-0.38, 0.38] {
        let (wx, wz) = at(0.0, lz);
        p.slab(v(wx, 0.08, wz), yaw, v(0.05, 0.14, 0.14), Mat::Tire);
    }
    if fallen {
        let (sx, sz) = at(0.0, -0.7);
        p.slab(v(sx, 0.14, sz), yaw, v(0.04, 0.04, 0.8), Mat::Metal);
    } else {
        let (sx, sz) = at(0.0, -0.36);
        p.slab(v(sx, 0.5, sz), yaw, v(0.04, 0.8, 0.04), Mat::Metal);
        p.slab(v(sx, 0.9, sz), yaw, v(0.4, 0.04, 0.04), Mat::Dark);
    }
}

fn trees(p: &mut Plan, doors: &[(&'static str, (Vec3, f32))]) {
    let keep_clear: Vec<Vec2> = PACKS
        .iter()
        .chain(NPCS.iter())
        .chain([SPAWN, GRAVEYARD].iter())
        .map(|&(x, z)| pt(x, z))
        .chain(doors.iter().map(|(_, (pos, _))| pt(pos.x, pos.z)))
        .collect();
    let route = p.route.clone();
    let tree = |p: &mut Plan, x: f32, z: f32, pine: bool, size: f32| {
        let q = pt(x, z);
        let off_route = route_point_near(&route, q).distance(q) > 4.5;
        if !off_route || keep_clear.iter().any(|a| a.distance(q) < 6.0) {
            return;
        }
        p.prim(
            PrimKind::Cylinder,
            v(x, 1.1, z),
            v(0.5, 2.2, 0.5),
            Mat::Bark,
        );
        if pine {
            p.prim(
                PrimKind::Cone,
                v(x, 3.0 + size, z),
                v(size * 1.2, size * 2.4, size * 1.2),
                Mat::LeafDark,
            );
        } else {
            p.prim(
                PrimKind::Sphere,
                v(x, 3.1 + size * 0.5, z),
                v(size * 1.5, size * 1.3, size * 1.5),
                Mat::Leaf,
            );
        }
        p.block(v(x, 1.5, z), v(0.7, 3.0, 0.7));
    };
    // A belt around the edge plus clumps in the lawns between buildings.
    for i in 0..22 {
        let x = -66.0 + i as f32 * 6.2;
        tree(p, x, -47.0, i % 2 == 0, 1.5 + (i % 3) as f32 * 0.2);
        tree(p, x + 2.0, 47.5, i % 2 == 1, 1.6);
    }
    for i in 0..13 {
        let z = -42.0 + i as f32 * 7.0;
        tree(p, -67.0, z, i % 2 == 0, 1.6);
        tree(p, 67.0, z, i % 2 == 1, 1.7);
    }
    for (x, z, pine) in [
        (-22.0, -30.0, false),
        (22.0, -30.0, false),
        (-24.0, 2.0, true),
        (24.0, 14.0, false),
        (-8.0, 28.0, false),
        (8.0, 24.0, true),
        (40.0, 38.0, false),
        (66.0, 8.0, true),
        (36.0, 14.0, false),
        (-60.0, -8.0, true),
        (-58.0, 20.0, false),
        (-12.0, -22.0, false),
    ] {
        tree(p, x, z, pine, 1.8);
    }
}
