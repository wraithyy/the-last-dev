//! Deterministic combat rules for The Last Dev.
//!
//! `combat_core` owns the rules that must behave identically on client and
//! server, and deliberately has no engine (Bevy) dependency.

mod ai;
mod auto_attack;
pub mod content;
pub mod economy;
mod effects;
mod floors;
pub mod geom;
mod nav;
mod progression;
mod quests;
mod rng;
mod save;
mod sim;
pub mod sprint;
pub mod talents;
pub mod targeting;
mod telegraph;
pub mod time;
pub mod types;
mod wander;
pub mod world;
mod zones;

pub use content::{
    AiAbility, AiDef, AiTarget, ApplyAura, Area, AuraDef, AutoAttackDef, ClassDef, Config,
    ConsumeAura, Content, ContentError, DispelType, Effect, EnrageDef, Execute, FleeDef,
    FrontalShield, ITEM_DESC_MAX, ITEM_STACK_CAP, IdlePose, ItemBonus, ItemDef, ItemTotals,
    LevelDef, LootEntry, MAX_ACTION_BAR, MobScaling, Objective, PackDef, PeriodicDef, PhaseDef,
    QuestDef, QuestGiver, QuestReward, Role, School, SpellDef, SpellId, SpellTarget, TelegraphAt,
    TelegraphDef, UnitDef, Unlock, XpRules, YellDef, ZoneDef, ZoneKind, ZoneLink, ZoneSpawn,
};
pub use economy::{
    ActivePerk, ConsumableDef, Economy, GearDef, GearSource, GearStats, GearVisual, Goods, JunkDef,
    Perk, PerkView, Rarity, SLOTS, SetDef, Slot, VendorDef,
};
pub use floors::{CORPSE_EMPTY_MS, CORPSE_LOOT_MS, pack_offsets};
pub use geom::{Aabb, Vec3, distance, forward, in_range, is_facing, line_of_sight};
pub use progression::GROUP_SHARE_RANGE_M;
pub use quests::{ActiveQuest, GIVER_REACH_SLACK_M, QuestLog, QuestStatus};
pub use save::{SAVE_VERSION, SaveData};
pub use sim::{MAX_PLAYERS, Sim};
pub use sprint::{
    Affix, AffixDef, MeterLine, RewardLine, SprintBuilding, SprintDef, SprintError, SprintPreview,
    SprintRecord, SprintResult, SprintStatus, SprintSummary, affixes_for,
    preview as sprint_preview, retro_lines,
};
pub use talents::{
    ClassTalents, TALENT_CHOICES, TALENT_ROWS, TalentDef, TalentPicks, TalentRow, Talents,
};
pub use targeting::{
    EntityId, TAB_TARGET_RANGE, TargetCandidate, TargetError, tab_target, validate_target,
};
pub use telegraph::{TelegraphShape, TelegraphView};
pub use time::{FixedStepper, TICK_HZ, Tick, ms_to_ticks, ticks_to_ms};
pub use types::{
    AiState, Audience, AuraEnd, AuraView, CastError, CastView, Command, CooldownView, Event,
    ItemError, UnitState,
};
pub use world::{AchievementDef, PropDef, PropKind, SecretNpc, Trigger, WorldDef};
pub use zones::{TRASH_RESPAWN_MS, ZoneLinkView};
