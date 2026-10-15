//! One place that knows how a seated character meets a seat. Props register `SeatSpot`s from
//! their own transform (`floors::kit`), and every seated character (NPC, the player's /sit,
//! a remote player) is drawn by shifting its model root onto the nearest spot, so the pose
//! never depends on where the sim happens to have the unit standing.
//!
//! The rig constants were measured on the last frame of the Quaternius `SitDown` clip
//! (feet on the floor): hip joint 0.569 up and 0.42 behind the model origin, underside of
//! the pelvis 0.38 up, all in native model units (scale by `height / MODEL_NATIVE_HEIGHT`).

use crate::config::MODEL_NATIVE_HEIGHT;
use bevy::prelude::*;

/// Hip joint behind the model origin while seated.
const SIT_HIP_BACK: f32 = 0.42;
/// Hip joint height while seated (the pivot a slumping sleeper tips about).
const SIT_HIP_Y: f32 = 0.569;
/// Underside of the pelvis while seated; this is what rests on the seat surface.
const SIT_BUTT_Y: f32 = 0.38;
/// Seat surface height the chibi proportions want: the sit clip keeps the feet on the floor,
/// so anything higher leaves the legs dangling by the difference.
pub const SEAT_TOP: f32 = 0.47;
/// Beyond this a sit emote does not snap to a seat (it plays in place).
pub const SNAP_M: f32 = 3.0;
/// A seated character this close to a spot (xz) is drawn on it.
const ON_SEAT_M: f32 = 1.3;
/// Clearance between a solid seat and the body that walks away from it.
const BODY_GAP: f32 = 0.05;

/// One place to sit: a bench slot, a chair, half a sofa.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeatSpot {
    /// Centre of the seat surface; `y` is the surface height.
    pub pos: Vec3,
    /// Unit yaw the sitter faces (`(-sin, -cos)`), away from the backrest.
    pub yaw: f32,
    /// Half the seat's extent along the facing axis.
    pub half_depth: f32,
    /// An NPC sits here for good, so the player's /sit skips it.
    pub taken: bool,
}

/// A yaw wrapped to (-PI, PI].
pub fn wrap_yaw(a: f32) -> f32 {
    std::f32::consts::PI - (std::f32::consts::PI - a).rem_euclid(std::f32::consts::TAU)
}

pub fn model_scale(height: f32) -> f32 {
    height / MODEL_NATIVE_HEIGHT
}

impl SeatSpot {
    pub fn forward(&self) -> Vec3 {
        Quat::from_rotation_y(self.yaw) * Vec3::NEG_Z
    }

    /// Where the model origin (the feet) goes so the hips land on the seat; floor level.
    pub fn origin(&self, scale: f32) -> Vec3 {
        let p = self.pos + self.forward() * SIT_HIP_BACK * scale;
        Vec3::new(p.x, 0.0, p.z)
    }

    /// Model lift that puts the pelvis on the surface; never negative (nothing digs a hole).
    pub fn lift(&self, scale: f32) -> f32 {
        (self.pos.y - SIT_BUTT_Y * scale).max(0.0)
    }

    /// Where a body stands while seated: the feet position, pushed out in front of a solid
    /// seat so collision does not shove it away, and the same place it stands up to.
    pub fn body_pos(&self, scale: f32, radius: f32) -> Vec3 {
        let reach = (SIT_HIP_BACK * scale).max(self.half_depth + radius + BODY_GAP);
        let p = self.pos + self.forward() * reach;
        Vec3::new(p.x, 0.0, p.z)
    }

    /// The model-root offset, in the entity's own frame, plus the extra yaw that draws a
    /// character standing at `at` (facing `entity_yaw`) seated on this spot.
    pub fn root_shift(&self, scale: f32, at: Vec3, entity_yaw: f32) -> (Vec3, f32) {
        let world = self.origin(scale) - Vec3::new(at.x, 0.0, at.z);
        let local = Quat::from_rotation_y(-entity_yaw) * world;
        (
            Vec3::new(local.x, self.lift(scale), local.z),
            self.yaw - entity_yaw,
        )
    }
}

/// The seat nearest to `from` on the floor plane, within `max` metres.
pub fn nearest(seats: &[SeatSpot], from: Vec3, max: f32, free_only: bool) -> Option<&SeatSpot> {
    let d = |s: &SeatSpot| Vec2::new(s.pos.x - from.x, s.pos.z - from.z).length();
    seats
        .iter()
        .filter(|s| !(free_only && s.taken) && d(s) <= max)
        .min_by(|a, b| d(a).total_cmp(&d(b)))
}

/// The seat a seated character at `at` is on, if any.
pub fn seat_under(seats: &[SeatSpot], at: Vec3) -> Option<&SeatSpot> {
    nearest(seats, at, ON_SEAT_M, false)
}

/// Hip joint of the seated rig in model-root space (before the root's half turn), for tipping
/// a sleeper about the seat instead of about the feet.
pub fn sit_hip(scale: f32) -> Vec3 {
    Vec3::new(0.0, SIT_HIP_Y, -SIT_HIP_BACK) * scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const S: f32 = 1.8 / MODEL_NATIVE_HEIGHT;

    fn spot(yaw: f32) -> SeatSpot {
        SeatSpot {
            pos: Vec3::new(10.0, SEAT_TOP, -4.0),
            yaw,
            half_depth: 0.25,
            taken: false,
        }
    }

    /// Where the sit clip's hip joint ends up in the world for a character that stands at
    /// `at`, faces `entity_yaw` and is drawn with the shift for `s`.
    fn hip_world(s: &SeatSpot, scale: f32, at: Vec3, entity_yaw: f32) -> Vec3 {
        let (off, dyaw) = s.root_shift(scale, at, entity_yaw);
        let model = Quat::from_rotation_y(entity_yaw)
            * (off + Quat::from_rotation_y(dyaw + PI) * sit_hip(scale));
        at + model
    }

    #[test]
    fn hips_land_on_the_seat_whatever_the_entity_does() {
        for yaw in [0.0, 1.0, PI, -2.2] {
            let s = spot(yaw);
            for (at, entity_yaw) in [
                (s.body_pos(S, 0.4), yaw),
                (Vec3::new(3.0, 0.0, 9.0), 0.7),
                (s.origin(S), yaw + PI),
            ] {
                let h = hip_world(&s, S, at, entity_yaw);
                assert!(
                    Vec2::new(h.x - s.pos.x, h.z - s.pos.z).length() < 1e-4,
                    "yaw {yaw}: hip {h} vs seat {}",
                    s.pos
                );
                // The pelvis underside rests on the surface.
                assert!((h.y - SIT_HIP_Y * S + SIT_BUTT_Y * S - s.pos.y).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn origin_is_in_front_of_the_backrest_side() {
        let s = spot(0.0);
        // Yaw 0 faces -Z, so the feet are ahead (smaller z) of the seat centre.
        assert!(s.origin(S).z < s.pos.z);
        assert!(s.lift(S) > 0.0 && s.lift(S) < s.pos.y);
        // A seat lower than the pelvis lifts nothing.
        let low = SeatSpot {
            pos: Vec3::new(0.0, 0.05, 0.0),
            ..s
        };
        assert_eq!(low.lift(S), 0.0);
    }

    #[test]
    fn body_stands_clear_of_a_solid_seat() {
        let s = spot(PI);
        let b = s.body_pos(S, 0.4);
        let ahead = (b - s.pos).dot(s.forward());
        assert!(ahead >= s.half_depth + 0.4);
    }

    #[test]
    fn nearest_skips_taken_seats_for_choosing_but_not_for_drawing() {
        let mut a = spot(0.0);
        a.taken = true;
        let b = SeatSpot {
            pos: a.pos + Vec3::X * 0.9,
            taken: false,
            ..a
        };
        let seats = [a, b];
        let from = a.pos + Vec3::X * 0.1;
        assert_eq!(nearest(&seats, from, SNAP_M, true), Some(&b));
        assert_eq!(seat_under(&seats, from), Some(&a));
        assert_eq!(nearest(&seats, from + Vec3::X * 20.0, SNAP_M, true), None);
    }
}
