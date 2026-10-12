//! Deterministic combat rules for World of Workcraft.
//!
//! `combat_core` owns the rules that must behave identically on client and
//! server, and deliberately has no engine (Bevy) dependency.

pub mod geom;
pub mod targeting;
pub mod time;

pub use geom::{Aabb, Vec3, distance, forward, in_range, is_facing, line_of_sight};
pub use targeting::{
    EntityId, TAB_TARGET_RANGE, TargetCandidate, TargetError, tab_target, validate_target,
};
pub use time::{FixedStepper, TICK_HZ, Tick, ms_to_ticks, ticks_to_ms};
