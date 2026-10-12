use crate::geom::{Aabb, Vec3, distance, in_range, is_facing, line_of_sight};
use std::f32::consts::FRAC_PI_2;

pub type EntityId = u64;

#[derive(Clone, Copy, Debug)]
pub struct TargetCandidate {
    pub id: EntityId,
    pub pos: Vec3,
    pub hostile: bool,
    pub alive: bool,
}

pub const TAB_TARGET_RANGE: f32 = 40.0;

/// WotLK-like Tab: live hostiles within `max_range`, front arc preferred,
/// ordered by distance; cycles to the next after `current`, wrapping. Behind
/// candidates are used only when nothing is in front.
pub fn tab_target(
    origin: Vec3,
    yaw: f32,
    current: Option<EntityId>,
    candidates: &[TargetCandidate],
    max_range: f32,
) -> Option<EntityId> {
    let mut front: Vec<(f32, EntityId)> = Vec::new();
    let mut behind: Vec<(f32, EntityId)> = Vec::new();
    for c in candidates
        .iter()
        .filter(|c| c.alive && c.hostile && in_range(origin, c.pos, max_range))
    {
        let entry = (distance(origin, c.pos), c.id);
        if is_facing(origin, yaw, c.pos, FRAC_PI_2) {
            front.push(entry);
        } else {
            behind.push(entry);
        }
    }
    let mut list = if front.is_empty() { behind } else { front };
    // Id tiebreak keeps the cycle order deterministic for equal distances.
    list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let next = match current.and_then(|cur| list.iter().position(|e| e.1 == cur)) {
        Some(i) => (i + 1) % list.len(),
        None => 0,
    };
    list.get(next).map(|e| e.1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetError {
    NoTarget,
    Dead,
    OutOfRange,
    NotFacing,
    NoLineOfSight,
}

/// Check order: dead, range, facing (only if `require_facing`), LoS.
pub fn validate_target(
    origin: Vec3,
    yaw: f32,
    target: Option<&TargetCandidate>,
    range: f32,
    require_facing: bool,
    blockers: &[Aabb],
) -> Result<(), TargetError> {
    let t = target.ok_or(TargetError::NoTarget)?;
    if !t.alive {
        return Err(TargetError::Dead);
    }
    if !in_range(origin, t.pos, range) {
        return Err(TargetError::OutOfRange);
    }
    if require_facing && !is_facing(origin, yaw, t.pos, FRAC_PI_2) {
        return Err(TargetError::NotFacing);
    }
    if !line_of_sight(origin, t.pos, blockers) {
        return Err(TargetError::NoLineOfSight);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const O: Vec3 = Vec3::new(0.0, 0.0, 0.0);

    fn mob(id: EntityId, z: f32) -> TargetCandidate {
        TargetCandidate {
            id,
            pos: Vec3::new(0.0, 0.0, z),
            hostile: true,
            alive: true,
        }
    }

    fn tab(cur: Option<EntityId>, c: &[TargetCandidate]) -> Option<EntityId> {
        tab_target(O, 0.0, cur, c, TAB_TARGET_RANGE)
    }

    #[test]
    fn tab_picks_nearest_first() {
        let c = [mob(1, -20.0), mob(2, -5.0), mob(3, -10.0)];
        assert_eq!(tab(None, &c), Some(2));
    }

    #[test]
    fn tab_cycles_by_distance_and_wraps() {
        let c = [mob(1, -20.0), mob(2, -5.0), mob(3, -10.0)];
        assert_eq!(tab(Some(2), &c), Some(3));
        assert_eq!(tab(Some(3), &c), Some(1));
        assert_eq!(tab(Some(1), &c), Some(2));
    }

    #[test]
    fn tab_unknown_current_starts_over() {
        let c = [mob(1, -20.0), mob(2, -5.0)];
        assert_eq!(tab(Some(99), &c), Some(2));
    }

    #[test]
    fn tab_skips_dead_friendly_and_out_of_range() {
        let mut dead = mob(1, -2.0);
        dead.alive = false;
        let mut friend = mob(2, -3.0);
        friend.hostile = false;
        let far = mob(3, -41.0);
        let ok = mob(4, -30.0);
        assert_eq!(tab(None, &[dead, friend, far, ok]), Some(4));
        assert_eq!(tab(None, &[dead, friend, far]), None);
    }

    #[test]
    fn tab_empty_is_none() {
        assert_eq!(tab(None, &[]), None);
    }

    #[test]
    fn tab_prefers_front_over_closer_behind() {
        let c = [mob(1, 2.0), mob(2, -30.0)];
        assert_eq!(tab(None, &c), Some(2));
        // Cycling stays within the front group.
        assert_eq!(tab(Some(2), &c), Some(2));
    }

    #[test]
    fn tab_falls_back_to_behind() {
        let c = [mob(1, 8.0), mob(2, 3.0)];
        assert_eq!(tab(None, &c), Some(2));
        assert_eq!(tab(Some(2), &c), Some(1));
    }

    #[test]
    fn validate_each_error() {
        let wall = Aabb {
            min: Vec3::new(-1.0, -1.0, -6.0),
            max: Vec3::new(1.0, 5.0, -4.0),
        };
        let v = |t: Option<&TargetCandidate>, range, facing, b: &[Aabb]| {
            validate_target(O, 0.0, t, range, facing, b)
        };
        let near = mob(1, -10.0);
        let mut dead = near;
        dead.alive = false;
        let behind = mob(2, 5.0);

        assert_eq!(v(None, 30.0, true, &[]), Err(TargetError::NoTarget));
        assert_eq!(v(Some(&dead), 30.0, true, &[]), Err(TargetError::Dead));
        assert_eq!(v(Some(&near), 5.0, true, &[]), Err(TargetError::OutOfRange));
        assert_eq!(
            v(Some(&behind), 30.0, true, &[]),
            Err(TargetError::NotFacing)
        );
        assert_eq!(v(Some(&behind), 30.0, false, &[]), Ok(()));
        assert_eq!(
            v(Some(&near), 30.0, true, &[wall]),
            Err(TargetError::NoLineOfSight)
        );
        assert_eq!(v(Some(&near), 30.0, true, &[]), Ok(()));
    }

    #[test]
    fn validate_dead_checked_before_range() {
        let mut far_dead = mob(1, -100.0);
        far_dead.alive = false;
        assert_eq!(
            validate_target(O, 0.0, Some(&far_dead), 5.0, true, &[]),
            Err(TargetError::Dead)
        );
    }
}
