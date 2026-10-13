//! Contract between the combat bridge (writer) and the animation system (reader).
//! One-shot actions use increasing sequence numbers so a reader can detect each
//! new trigger without the writer having to clear flags.

use bevy::prelude::*;

#[derive(Component, Default, Debug, Clone)]
pub struct CombatAnim {
    /// Currently casting a spell with a cast time (loop the cast pose).
    pub casting: bool,
    /// Spell id of the cast in progress (0 = none); picks the cast-loop pose.
    pub cast_spell: u32,
    /// Spell id behind the latest release; 0 is the auto attack.
    pub release_spell: u32,
    /// Channeling Coffee Break: the arm sips on top of the idle legs, so `casting` stays false.
    pub drinking: bool,
    pub dead: bool,
    /// Incremented when a spell is released (cast finished or instant cast).
    pub cast_release_seq: u32,
    /// Incremented on each melee swing.
    pub attack_seq: u32,
    /// Incremented when this unit takes damage.
    pub hit_seq: u32,
}
