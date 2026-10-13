//! Pure, deterministic floor layouts: blockers, anchors, dressing pieces and
//! the palette. `build_floor` has no ECS or asset dependencies.

use super::kit::{Anchor, Piece, Plan};
use super::themes;
use crate::collision::aabb;
use bevy::prelude::*;
use combat_core::Aabb;
use std::f32::consts::FRAC_PI_2;

pub const FLOOR_COUNT: u8 = 5;

pub const FLOOR_NAMES: [&str; FLOOR_COUNT as usize] = [
    "Open Space",
    "Meeting Rooms",
    "Sales & Marketing",
    "PMO",
    "C-Level Penthouse",
];

pub fn floor_name(id: u8) -> &'static str {
    FLOOR_NAMES[usize::from(id.clamp(1, FLOOR_COUNT)) - 1]
}

#[derive(Clone, Copy, Debug)]
pub enum FloorKind {
    /// Carpet tiles multiplied by this colour.
    Carpet([f32; 3]),
    Marble,
}

/// Per-floor look: surface colours, lighting and fog.
#[derive(Clone, Debug)]
pub struct Theme {
    pub wall: Color,
    pub trim: Color,
    pub accent: LinearRgba,
    pub fabric: [Color; 3],
    pub floor: FloorKind,
    pub fog: Color,
    pub ambient: Color,
    pub ambient_brightness: f32,
    pub sun: Color,
    pub sun_illuminance: f32,
}

pub struct FloorLayout {
    pub id: u8,
    pub name: &'static str,
    /// Movement and line-of-sight blockers, same semantics as the old arena colliders.
    pub blockers: Vec<Aabb>,
    /// Named anchors: world position on the floor plane and unit yaw (the yaw a
    /// unit root needs to face `(-sin, -cos)`, same convention as the player body).
    pub anchors: Vec<(&'static str, Vec3, f32)>,
    pub pieces: Vec<Piece>,
    pub theme: Theme,
}

impl FloorLayout {
    pub fn anchor(&self, name: &str) -> Option<(Vec3, f32)> {
        self.anchors
            .iter()
            .find(|a| a.0 == name)
            .map(|a| (a.1, a.2))
    }

    /// Anchor position; the contract guarantees the names, so a miss falls back to the origin.
    #[cfg(test)]
    pub fn pos(&self, name: &str) -> Vec3 {
        self.anchor(name).map_or(Vec3::ZERO, |a| a.0)
    }
}

/// Yaw that makes a unit at `from` face `to`.
pub fn yaw_towards(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    (-d.x).atan2(-d.z)
}

/// Floor 1..=5 (clamped). Same id always yields the same layout.
pub fn build_floor(id: u8) -> FloorLayout {
    let id = id.clamp(1, FLOOR_COUNT);
    let mut plan = Plan::default();
    let theme = themes::build(id, &mut plan);
    let Plan {
        pieces, anchors, ..
    } = plan;
    let blockers = pieces
        .iter()
        .filter_map(|p| match p {
            Piece::Box {
                center,
                size,
                solid: true,
                ..
            } => Some(aabb(*center, *size)),
            Piece::Prop {
                model,
                pos,
                yaw,
                solid: true,
                scale,
            } => {
                let size = Vec3::from(model.size) * super::kit::FURNITURE_SCALE * *scale;
                Some(aabb(
                    *pos + Vec3::Y * size.y / 2.0,
                    super::kit::rotated_footprint(size, *yaw),
                ))
            }
            Piece::Block(b) => Some(*b),
            _ => None,
        })
        .collect();
    FloorLayout {
        id,
        name: floor_name(id),
        blockers,
        anchors: anchors
            .into_iter()
            .map(|Anchor { name, pos, yaw }| (name, pos, yaw))
            .collect(),
        pieces,
        theme,
    }
}

/// Box in front of the exit doors; the bridge unlocks the next floor when the
/// player's feet are inside it.
pub fn elevator_exit_trigger(layout: &FloorLayout) -> Aabb {
    let (pos, yaw) = layout.anchor("elevator_exit").unwrap_or((Vec3::ZERO, 0.0));
    let facing = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    // The anchor sits 1.8 m in front of the doors; shift towards the wall so the box covers the door apron.
    aabb(
        pos - facing * 0.6 + Vec3::Y * 1.25,
        Vec3::new(2.4, 2.5, 2.4),
    )
}

/// Player yaw facing along +X (towards the boss at the far end of every floor).
pub const FACE_EAST: f32 = -FRAC_PI_2;
/// Unit yaw facing -X (towards the player spawn).
pub const FACE_WEST: f32 = FRAC_PI_2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::blocks_at;
    use crate::config::{FLOOR_HALF_X, FLOOR_HALF_Z};
    use std::collections::VecDeque;

    const CELL: f32 = 0.5;

    fn free(layout: &FloorLayout, x: f32, z: f32) -> bool {
        let p = Vec3::new(x, 0.0, z);
        !layout.blockers.iter().any(|b| blocks_at(p, b))
    }

    /// Coarse 0.5 m BFS (4-neighbour) from `from`; returns path length per cell.
    fn distances(layout: &FloorLayout, from: Vec3) -> (Vec<f32>, usize, usize) {
        let w = (FLOOR_HALF_X * 2.0 / CELL) as usize;
        let h = (FLOOR_HALF_Z * 2.0 / CELL) as usize;
        let cell = |x: f32, z: f32| {
            (
                ((x + FLOOR_HALF_X) / CELL) as usize,
                ((z + FLOOR_HALF_Z) / CELL) as usize,
            )
        };
        let center = |i: usize, j: usize| {
            (
                i as f32 * CELL - FLOOR_HALF_X + CELL / 2.0,
                j as f32 * CELL - FLOOR_HALF_Z + CELL / 2.0,
            )
        };
        let blocked: Vec<bool> = (0..w * h)
            .map(|k| {
                let (x, z) = center(k % w, k / w);
                !free(layout, x, z)
            })
            .collect();
        let mut dist = vec![f32::INFINITY; w * h];
        let (si, sj) = cell(from.x, from.z);
        dist[sj * w + si] = 0.0;
        let mut q = VecDeque::from([(si, sj)]);
        while let Some((i, j)) = q.pop_front() {
            let d = dist[j * w + i];
            for (di, dj) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (ni, nj) = (i as i32 + di, j as i32 + dj);
                if ni < 0 || nj < 0 || ni as usize >= w || nj as usize >= h {
                    continue;
                }
                let k = nj as usize * w + ni as usize;
                if !blocked[k] && dist[k].is_infinite() {
                    dist[k] = d + CELL;
                    q.push_back((ni as usize, nj as usize));
                }
            }
        }
        (dist, w, h)
    }

    fn at(dist: &(Vec<f32>, usize, usize), p: Vec3) -> f32 {
        let i = ((p.x + FLOOR_HALF_X) / CELL) as usize;
        let j = ((p.z + FLOOR_HALF_Z) / CELL) as usize;
        dist.0[j * dist.1 + i]
    }

    const REQUIRED: [&str; 11] = [
        "pack_1",
        "pack_2",
        "pack_3",
        "pack_4",
        "pack_5",
        "pack_6",
        "boss",
        "npc_1",
        "player_spawn",
        "elevator",
        "elevator_exit",
    ];

    #[test]
    fn deterministic() {
        for id in 1..=FLOOR_COUNT {
            let (a, b) = (build_floor(id), build_floor(id));
            assert_eq!(a.blockers.len(), b.blockers.len());
            assert_eq!(a.pieces.len(), b.pieces.len());
            assert_eq!(a.anchors.len(), b.anchors.len());
        }
    }

    #[test]
    fn names_match_contract() {
        let names: Vec<_> = (1..=5).map(|i| build_floor(i).name).collect();
        assert_eq!(
            names,
            [
                "Open Space",
                "Meeting Rooms",
                "Sales & Marketing",
                "PMO",
                "C-Level Penthouse"
            ]
        );
    }

    #[test]
    fn anchors_exist_and_are_walkable() {
        for id in 1..=FLOOR_COUNT {
            let l = build_floor(id);
            for name in REQUIRED {
                assert!(l.anchor(name).is_some(), "floor {id}: missing {name}");
            }
            for (name, pos, _) in &l.anchors {
                assert!(
                    pos.x.abs() < FLOOR_HALF_X && pos.z.abs() < FLOOR_HALF_Z,
                    "floor {id}: {name} outside the floor"
                );
                assert!(
                    free(&l, pos.x, pos.z),
                    "floor {id}: anchor {name} at {pos} is inside a blocker"
                );
            }
        }
    }

    #[test]
    fn floor_one_has_it_support_anchors() {
        let l = build_floor(1);
        assert!(l.anchor("npc_2").is_some() && l.anchor("npc_3").is_some());
    }

    #[test]
    fn packs_are_spaced_so_pulls_do_not_chain() {
        for id in 1..=FLOOR_COUNT {
            let l = build_floor(id);
            let names = [
                "pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "pack_6", "boss",
            ];
            for (i, a) in names.iter().enumerate() {
                for b in &names[i + 1..] {
                    let d = l.pos(a).distance(l.pos(b));
                    assert!(d >= 14.0, "floor {id}: {a}-{b} only {d:.1} m apart");
                }
                let d = l.pos(a).distance(l.pos("player_spawn"));
                assert!(d >= 15.0, "floor {id}: {a} only {d:.1} m from spawn");
            }
        }
    }

    #[test]
    fn route_is_connected_and_progressive() {
        for id in 1..=FLOOR_COUNT {
            let l = build_floor(id);
            let d = distances(&l, l.pos("player_spawn"));
            let route: Vec<f32> = ["pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "boss"]
                .iter()
                .map(|n| at(&d, l.pos(n)))
                .collect();
            assert!(
                route.iter().all(|v| v.is_finite()),
                "floor {id}: unreachable {route:?}"
            );
            assert!(
                route.windows(2).all(|w| w[0] < w[1]),
                "floor {id}: route not progressive {route:?}"
            );
            for n in ["pack_6", "npc_1", "elevator", "elevator_exit"] {
                assert!(at(&d, l.pos(n)).is_finite(), "floor {id}: {n} unreachable");
            }
            // Floors 1 and 5 keep pack_6 on the main route, before the boss.
            if id == 1 || id == 5 {
                assert!(at(&d, l.pos("pack_6")) > route[4] && at(&d, l.pos("pack_6")) < route[5]);
            }
        }
    }

    #[test]
    fn exit_is_near_the_boss_and_the_arrival_near_the_spawn() {
        for id in 1..=FLOOR_COUNT {
            let l = build_floor(id);
            assert!(l.pos("elevator").distance(l.pos("player_spawn")) < 10.0);
            let trig = elevator_exit_trigger(&l);
            let p = l.pos("elevator_exit");
            assert!(p.x > trig.min.x && p.x < trig.max.x && p.z > trig.min.z && p.z < trig.max.z);
            assert!(l.pos("elevator_exit").distance(l.pos("boss")) < 14.0);
        }
    }

    #[test]
    fn referenced_furniture_files_exist() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/furniture");
        for m in super::super::kit::ALL_MODELS {
            assert!(dir.join(format!("{}.glb", m.file)).exists(), "{}", m.file);
        }
    }
}
