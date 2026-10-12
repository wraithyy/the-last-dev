//! Contract between the combat bridge (writer) and the animation system (reader).
//! One-shot actions use increasing sequence numbers so a reader can detect each
//! new trigger without the writer having to clear flags.

use bevy::prelude::*;

#[derive(Component, Default, Debug, Clone)]
pub struct CombatAnim {
    /// Currently casting a spell with a cast time (loop the cast pose).
    pub casting: bool,
    pub dead: bool,
    /// Incremented when a spell is released (cast finished or instant cast).
    pub cast_release_seq: u32,
    /// Incremented on each melee swing.
    pub attack_seq: u32,
    /// Incremented when this unit takes damage.
    pub hit_seq: u32,
}
