//! One hero prop and one tinted light at each instanced room's entrance, so the first view says
//! where you are (the rooms of a dungeon share a corridor-hall-pod plan and differ mostly in
//! tint). The prop is a Kenney model blown up to about two metres, not solid, and set beside the
//! walk to the first pack, in the first free spot ahead of the spawn.

use super::kit::*;
use super::layout::blockers_of;
use super::models::*;
use super::themes::v;
use bevy::prelude::*;

/// (zone, model, size in metres of its longest side, light colour as sRGB).
type Spec = (&'static str, &'static Model, f32, [f32; 3]);

const AMBER: [f32; 3] = [1.0, 0.72, 0.3];
const CYAN: [f32; 3] = [0.45, 0.85, 1.0];
const VIOLET: [f32; 3] = [0.75, 0.5, 1.0];
const RED: [f32; 3] = [1.0, 0.3, 0.25];
const PINK: [f32; 3] = [1.0, 0.4, 0.8];
const GREEN: [f32; 3] = [0.45, 1.0, 0.6];
const TEAL: [f32; 3] = [0.4, 1.0, 0.9];
const SKY: [f32; 3] = [0.6, 0.8, 1.0];
const GOLD: [f32; 3] = [1.0, 0.85, 0.5];
const ORANGE: [f32; 3] = [1.0, 0.5, 0.25];

/// Each Ilmari wing shares one hero across its three rooms; the wing is the identity there.
fn specs() -> [Spec; 31] {
    [
        ("basement/1", &SHIPPING_CONTAINER_A, 3.0, AMBER),
        ("basement/2", &FRIDGE_LARGE, 2.6, CYAN),
        ("basement/3", &HOPPER_SQUARE, 2.4, VIOLET),
        ("basement/4", &SIGN_WARNING, 2.2, RED),
        ("hack24/1", &PIZZA_BOX, 2.4, PINK),
        ("hack24/2", &LAPTOP, 2.4, CYAN),
        ("hack24/3", &SPEAKER, 2.6, PINK),
        ("hack24/4", &COIN_GOLD, 2.2, GREEN),
        ("vibecon/1", &GATE_FINISH, 3.2, GOLD),
        ("vibecon/2", &COFFEE_MACHINE, 2.2, AMBER),
        ("vibecon/3", &ENERGY_CAN, 2.2, GREEN),
        ("vibecon/4", &CAKE, 2.2, PINK),
        ("vibecon/5", &TV, 2.8, SKY),
        ("ilmari1/1", &MACHINE_WINDOW, 2.6, RED),
        ("ilmari1/2", &MACHINE_WINDOW, 2.6, RED),
        ("ilmari1/3", &MACHINE_WINDOW, 2.6, RED),
        ("ilmari2/1", &PILLOW_BLUE, 2.2, PINK),
        ("ilmari2/2", &PILLOW_BLUE, 2.2, PINK),
        ("ilmari2/3", &PILLOW_BLUE, 2.2, PINK),
        ("ilmari3/1", &CONVEYOR_LONG, 3.0, TEAL),
        ("ilmari3/2", &CONVEYOR_LONG, 3.0, TEAL),
        ("ilmari3/3", &CONVEYOR_LONG, 3.0, TEAL),
        ("ilmari4/1", &WATER_TOWER, 3.0, SKY),
        ("ilmari4/2", &WATER_TOWER, 3.0, SKY),
        ("ilmari4/3", &WATER_TOWER, 3.0, SKY),
        ("ilmari5/1", &ITEM_BOX, 2.0, GOLD),
        ("ilmari5/2", &ITEM_BOX, 2.0, GOLD),
        ("ilmari5/3", &ITEM_BOX, 2.0, GOLD),
        ("ilmari6/1", &ROBOT_ARM_A, 3.2, ORANGE),
        ("ilmari6/2", &ROBOT_ARM_A, 3.2, ORANGE),
        ("ilmari6/3", &ROBOT_ARM_A, 3.2, ORANGE),
    ]
}

/// Distance from `p` to the polyline.
fn route_gap(route: &[Vec2], p: Vec2) -> f32 {
    route
        .windows(2)
        .map(|w| {
            let (a, ab) = (w[0], w[1] - w[0]);
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            p.distance(a + ab * t)
        })
        .fold(f32::MAX, f32::min)
}

pub fn place(zone: &str, p: &mut Plan) {
    let Some(&(_, model, size, [r, g, b])) = specs().iter().find(|s| s.0 == zone) else {
        return;
    };
    let Some(anchor) = p.anchors.iter().find(|a| a.name == "player_spawn") else {
        return;
    };
    let (spawn, yaw) = (anchor.pos, anchor.yaw);
    let forward = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    let side = forward.cross(Vec3::Y);
    let blockers = blockers_of(&p.pieces);
    let half = size / 2.0 + 0.4;
    let free = |at: Vec3| {
        let c = Vec2::new(at.x, at.z);
        [
            (1.0, 1.0),
            (1.0, -1.0),
            (-1.0, 1.0),
            (-1.0, -1.0),
            (0.0, 0.0),
        ]
        .iter()
        .all(|(dx, dz)| p.fp.contains(c + Vec2::new(dx * half, dz * half)))
            && route_gap(&p.route, c) >= half + 0.8
            && blockers.iter().all(|bx| {
                bx.min.y > 2.5
                    || bx.max.x < at.x - half
                    || bx.min.x > at.x + half
                    || bx.max.z < at.z - half
                    || bx.min.z > at.z + half
            })
    };
    // Close enough to be in the first view, off to either side of the walk.
    let spot = [7.0, 9.0, 11.0, 5.0, 13.0]
        .iter()
        .flat_map(|ahead| {
            [3.0, -3.0, 4.5, -4.5, 6.0, -6.0]
                .iter()
                .map(move |lat| (*ahead, *lat))
        })
        .map(|(ahead, lat)| spawn + forward * ahead + side * lat)
        .find(|at| free(*at));
    let Some(at) = spot else { return };
    // Facing the spawn so the model's front shows.
    let face = (-forward).x.atan2((-forward).z);
    let longest = model.size.iter().cloned().fold(0.0, f32::max);
    put(p, model, v(at.x, 0.0, at.z), face, size / longest, false);
    p.light(
        v(at.x, size.max(2.4) + 0.4, at.z),
        Color::srgb(r, g, b),
        260_000.0,
    );
}
