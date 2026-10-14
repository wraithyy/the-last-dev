//! Deterministic combat rules for The Last Dev.
//!
//! `combat_core` owns the rules that must behave identically on client and
//! server, and deliberately has no engine (Bevy) dependency.

mod ai;
mod auto_attack;
pub mod content;
mod effects;
mod floors;
pub mod geom;
mod nav;
mod progression;
mod quests;
mod rng;
mod sim;
pub mod targeting;
pub mod time;
pub mod types;
mod wander;
mod zones;

pub use content::{
    AiAbility, AiDef, AiTarget, ApplyAura, Area, AuraDef, AutoAttackDef, ClassDef, Config,
    ConsumeAura, Content, ContentError, Effect, Execute, ITEM_DESC_MAX, ITEM_STACK_CAP, IdlePose,
    ItemBonus, ItemDef, ItemTotals, LevelDef, LootEntry, MAX_ACTION_BAR, MobScaling, Objective,
    PackDef, PeriodicDef, PhaseDef, QuestDef, QuestGiver, QuestReward, Role, School, SpellDef,
    SpellId, SpellTarget, UnitDef, Unlock, XpRules, YellDef, ZoneDef, ZoneKind, ZoneLink,
    ZoneSpawn,
};
pub use floors::pack_offsets;
pub use geom::{Aabb, Vec3, distance, forward, in_range, is_facing, line_of_sight};
pub use progression::GROUP_SHARE_RANGE_M;
pub use quests::{ActiveQuest, GIVER_REACH_SLACK_M, QuestLog, QuestStatus};
pub use sim::{MAX_PLAYERS, Sim};
pub use targeting::{
    EntityId, TAB_TARGET_RANGE, TargetCandidate, TargetError, tab_target, validate_target,
};
pub use time::{FixedStepper, TICK_HZ, Tick, ms_to_ticks, ticks_to_ms};
pub use types::{
    AiState, Audience, AuraEnd, AuraView, CastError, CastView, Command, CooldownView, Event,
    UnitState,
};
pub use zones::{TRASH_RESPAWN_MS, ZoneLinkView};
