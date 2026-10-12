//! Player-vs-static-AABB collision, kept free of ECS so it is unit-testable.

use crate::config::{ARENA_HALF, PLAYER_HEIGHT, PLAYER_RADIUS, STEP_HEIGHT};
use bevy::math::Vec3;
use combat_core::{Aabb, Vec3 as CVec3};

pub fn to_core(v: Vec3) -> CVec3 {
    CVec3::new(v.x, v.y, v.z)
}

pub fn aabb(center: Vec3, size: Vec3) -> Aabb {
    let half = size / 2.0;
    Aabb {
        min: to_core(center - half),
        max: to_core(center + half),
    }
}

fn overlaps_xz(pos: Vec3, b: &Aabb) -> bool {
    pos.x + PLAYER_RADIUS > b.min.x
        && pos.x - PLAYER_RADIUS < b.max.x
        && pos.z + PLAYER_RADIUS > b.min.z
        && pos.z - PLAYER_RADIUS < b.max.z
}

/// A box blocks horizontal movement unless it is low enough to step onto.
fn blocks(pos: Vec3, b: &Aabb) -> bool {
    overlaps_xz(pos, b) && b.max.y > pos.y + STEP_HEIGHT && b.min.y < pos.y + PLAYER_HEIGHT
}

/// Height of the walkable surface under the feet (floor or a prop top).
pub fn ground_height(pos: Vec3, colliders: &[Aabb]) -> f32 {
    colliders
        .iter()
        .filter(|b| overlaps_xz(pos, b) && b.max.y <= pos.y + STEP_HEIGHT)
        .map(|b| b.max.y)
        .fold(0.0, f32::max)
}

/// Axis-separated sliding: each axis is committed only if it stays unblocked,
/// so the player slides along walls instead of sticking.
pub fn slide(mut pos: Vec3, delta: Vec3, colliders: &[Aabb]) -> Vec3 {
    let limit = ARENA_HALF - PLAYER_RADIUS;
    let x = (pos.x + delta.x).clamp(-limit, limit);
    let tried = Vec3::new(x, pos.y, pos.z);
    if !colliders.iter().any(|b| blocks(tried, b)) {
        pos = tried;
    }
    let z = (pos.z + delta.z).clamp(-limit, limit);
    let tried = Vec3::new(pos.x, pos.y, z);
    if !colliders.iter().any(|b| blocks(tried, b)) {
        pos = tried;
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall() -> Vec<Aabb> {
        vec![aabb(Vec3::new(0.0, 1.0, -5.0), Vec3::new(10.0, 2.0, 1.0))]
    }

    #[test]
    fn slides_along_wall_and_never_enters() {
        let p = slide(
            Vec3::new(0.0, 0.0, -4.0),
            Vec3::new(0.2, 0.0, -0.2),
            &wall(),
        );
        assert_eq!(p.x, 0.2);
        assert_eq!(p.z, -4.0);
    }

    #[test]
    fn low_prop_is_steppable_high_prop_is_ground_when_above() {
        let desk = vec![aabb(Vec3::new(0.0, 0.375, 0.0), Vec3::new(2.0, 0.75, 2.0))];
        assert_eq!(ground_height(Vec3::new(0.0, 0.9, 0.0), &desk), 0.75);
        assert_eq!(ground_height(Vec3::new(0.0, 0.0, 0.0), &desk), 0.0);
        assert!(blocks(Vec3::new(0.0, 0.0, 0.0), &desk[0]));
    }
}
