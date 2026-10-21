//! Deterministic combat rules for The Last Dev.
//!
//! `combat_core` owns the rules that must behave identically on client and
//! server, and deliberately has no engine (Bevy) dependency.

mod ai;
mod auto_attack;
pub mod burnout;
pub mod calendar;
pub mod content;
pub mod corporate;
pub mod dailies;
pub mod duel;
pub mod economy;
mod effects;
pub mod endgame;
mod floors;
pub mod geom;
pub mod legacy;
pub mod live;
pub mod minigame;
mod nav;
pub mod pets;
mod progression;
mod quests;
pub mod raid;
mod rng;
mod save;
mod sim;
pub mod sprint;
pub mod talents;
pub mod targeting;
pub mod teambuilding;
mod telegraph;
pub mod terrain;
pub mod time;
pub mod types;
mod wander;
pub mod world;
pub mod world_events;
mod zones;

pub use burnout::{BURNOUT_MAX, BurnoutCause, BurnoutDef, BurnoutTier, QuipKind};
pub use calendar::{
    CalendarChain, CalendarDef, CalendarGame, CalendarReward, ChainStatus, TICKET_BOARD_MULT,
};
pub use content::{
    AiAbility, AiDef, AiTarget, ApplyAura, Area, AuraDef, AutoAttackDef, ClassDef, Config,
    ConsumeAura, Content, ContentError, DispelType, Effect, EnrageDef, Execute, FleeDef,
    FrontalShield, ITEM_DESC_MAX, ITEM_STACK_CAP, IdlePose, ItemBonus, ItemDef, ItemTotals,
    LevelDef, LootEntry, MAX_ACTION_BAR, MobScaling, Objective, OpenerDef, PackDef, PeriodicDef,
    PhaseDef, QuestDef, QuestGiver, QuestReward, Role, School, SpellDef, SpellId, SpellTarget,
    TelegraphAt, TelegraphDef, UnitDef, Unlock, XpRules, YellDef, ZoneDef, ZoneKind, ZoneLink,
    ZoneSpawn,
};
pub use dailies::{BoardView, GoalSpec, GoalView, SavedTask, TaskState, TaskView};
pub use duel::{DuelError, DuelOutcome};
pub use economy::{
    ActivePerk, ConsumableDef, Economy, GearDef, GearSource, GearStats, GearVisual, Goods, JunkDef,
    Perk, PerkView, Rarity, SLOTS, SetDef, Slot, VendorDef, role_package,
};
pub use endgame::{DEPARTMENTS, DepartmentDef, EndgameDef, RankDef, RepTier};
pub use floors::{CORPSE_EMPTY_MS, CORPSE_LOOT_MS, pack_offsets};
pub use geom::{Aabb, Vec3, distance, forward, in_range, is_facing, line_of_sight};
pub use legacy::{LegacyDef, LegacyError, LegacyTwist, LegacyView, LegacyZone};
pub use live::WorldBossState;
pub use minigame::{
    Card, Gauge, Medal, MedalDef, MinigameDef, MinigameResult, MinigameView, Rules,
};
pub use pets::{PetDef, PetDrop, duck_tip};
pub use progression::GROUP_SHARE_RANGE_M;
pub use quests::{
    ActiveQuest, DEFEND_RANGE_M, ESCORT_DONE_M, GIVER_REACH_SLACK_M, QUEST_PROP_COUNTED,
    QUEST_PROP_IDLE, QUEST_PROP_TAKEN, QUEST_PROP_WRONG_ORDER, QuestLog, QuestStatus,
};
pub use raid::{RaidDef, RaidView, TwinsDef};
pub use save::{SAVE_VERSION, SaveData};
pub use sim::{MAX_PLAYERS, Sim};
pub use sprint::{
    Affix, AffixDef, LadderEntry, LadderView, MeterLine, RewardLine, SprintBuilding, SprintDef,
    SprintError, SprintPreview, SprintRecord, SprintResult, SprintStatus, SprintSummary,
    WeeklySprint, affixes_for, preview as sprint_preview, retro_lines, weekly_sprint,
};
pub use talents::{
    ClassTalents, TALENT_CHOICES, TALENT_ROWS, TalentDef, TalentPicks, TalentRow, Talents,
};
pub use targeting::{
    EntityId, TAB_TARGET_RANGE, TargetCandidate, TargetError, tab_target, validate_target,
};
pub use teambuilding::{
    ObjectiveDef, Patrol, PatrolPose, ScoreDef, TeambuildingDef, WatcherDef, WatcherKind,
};
pub use telegraph::{TelegraphShape, TelegraphView};
pub use terrain::{Feature as TerrainFeature, Terrain};
pub use time::{FixedStepper, TICK_HZ, Tick, ms_to_ticks, ticks_to_ms};
pub use types::{
    AiState, Audience, AuraEnd, AuraView, Career, CastError, CastView, Command, CooldownView,
    Event, ItemError, UnitState,
};
pub use world::{
    AchievementDef, FULLSTACK_FAIL_PCT, PROP_DONE, PROP_FULLSTACK, PROP_OK, PROP_REFUSED, PropDef,
    PropKind, SecretNpc, Trigger, WorldDef,
};
pub use world_events::{WorldEventDef, WorldEventPhase};
pub use zones::{TRASH_RESPAWN_MS, ZoneLinkView};
