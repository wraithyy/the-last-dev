//! Public value types shared with the client: commands in, events out.

use crate::content::{IdlePose, School, SpellId};
use crate::geom::Vec3;
use crate::targeting::{EntityId, TargetError};
use serde::{Deserialize, Serialize};

/// Who an event is for, see `Sim::drain_events_routed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Audience {
    All,
    Player(EntityId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CastError {
    NoTarget,
    InvalidTarget,
    Dead,
    OutOfRange,
    NotFacing,
    NoLineOfSight,
    NotEnoughMana,
    OnCooldown,
    OnGcd,
    AlreadyCasting,
    SchoolLocked,
    Moving,
    NotInterruptible,
    NothingToInterrupt,
    Stunned,
    Silenced,
    NotLearned,
    FullMana,
}

impl CastError {
    /// WoW-style UI error text.
    pub fn message(self) -> &'static str {
        match self {
            CastError::NoTarget => "You have no target.",
            CastError::InvalidTarget => "Invalid target.",
            CastError::Dead => "Target is dead.",
            CastError::OutOfRange => "Out of range.",
            CastError::NotFacing => "You must be facing your target.",
            CastError::NoLineOfSight => "Target not in line of sight.",
            CastError::NotEnoughMana => "Not enough mana.",
            CastError::OnCooldown => "Ability is not ready yet.",
            CastError::OnGcd => "Another action is in progress.",
            CastError::AlreadyCasting => "Another action is in progress.",
            CastError::SchoolLocked => "That school is locked out.",
            CastError::Moving => "Can't do that while moving.",
            CastError::NotInterruptible => "That spell can't be interrupted.",
            CastError::NothingToInterrupt => "Nothing to interrupt.",
            CastError::Stunned => "Can't do that while stunned.",
            CastError::Silenced => "Can't do that while silenced.",
            CastError::NotLearned => "You haven't learned that spell yet.",
            CastError::FullMana => "You are already at full mana.",
        }
    }
}

impl From<TargetError> for CastError {
    fn from(e: TargetError) -> Self {
        match e {
            TargetError::NoTarget => CastError::NoTarget,
            TargetError::Dead => CastError::Dead,
            TargetError::OutOfRange => CastError::OutOfRange,
            TargetError::NotFacing => CastError::NotFacing,
            TargetError::NoLineOfSight => CastError::NoLineOfSight,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiState {
    Idle,
    Chase,
    Attack,
    Casting,
    Evade,
    Dead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuraEnd {
    Expired,
    Consumed,
    Depleted,
    Death,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// The client owns player movement; it reports the result every tick before `tick()`.
    PlayerMoved {
        pos: Vec3,
        yaw: f32,
        moving: bool,
        jumping: bool,
    },
    SetTarget {
        unit: EntityId,
        target: Option<EntityId>,
    },
    Cast {
        caster: EntityId,
        spell: SpellId,
    },
    StopCasting {
        caster: EntityId,
    },
    Loot {
        looter: EntityId,
        corpse: EntityId,
    },
    /// Enables the unit's auto attack on its current target; ignored (no event) if
    /// there is no valid hostile living target or the unit has no auto attack.
    StartAutoAttack {
        unit: EntityId,
    },
    StopAutoAttack {
        unit: EntityId,
    },
    RespawnPlayer,
    /// Talk to a friendly unit within `loot_range_m`; it answers with a `Yell` from its `talk` lines.
    Talk {
        unit: EntityId,
    },
    /// Accept an available quest; the player must be within talk range of a living giver.
    AcceptQuest {
        quest: String,
    },
    /// Hand in a ready quest at its giver (talk range): XP and trophy are granted.
    TurnInQuest {
        quest: String,
    },
    /// Drop an active quest and its progress; it can be accepted again.
    AbandonQuest {
        quest: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    CastStarted {
        caster: EntityId,
        spell: SpellId,
        total_ms: u32,
    },
    CastSucceeded {
        caster: EntityId,
        spell: SpellId,
        target: Option<EntityId>,
    },
    CastFailed {
        caster: EntityId,
        spell: SpellId,
        reason: CastError,
    },
    CastInterrupted {
        caster: EntityId,
        spell: SpellId,
        by: Option<EntityId>,
        lockout_ms: u32,
    },
    CastQueued {
        caster: EntityId,
        spell: SpellId,
    },
    Damage {
        source: EntityId,
        target: EntityId,
        spell: Option<SpellId>,
        amount: u32,
        absorbed: u32,
        crit: bool,
        combo: bool,
        school: School,
    },
    MeleeSwing {
        attacker: EntityId,
        target: EntityId,
    },
    /// A white attack (`spell: None`) or damaging spell failed its hit roll.
    Miss {
        source: EntityId,
        target: EntityId,
        spell: Option<SpellId>,
    },
    AutoAttackToggled {
        unit: EntityId,
        on: bool,
    },
    AuraApplied {
        target: EntityId,
        key: String,
    },
    AuraRemoved {
        target: EntityId,
        key: String,
        reason: AuraEnd,
    },
    EnterCombat {
        unit: EntityId,
    },
    LeaveCombat {
        unit: EntityId,
    },
    AiState {
        unit: EntityId,
        state: AiState,
    },
    Death {
        unit: EntityId,
        killer: Option<EntityId>,
    },
    XpGained {
        unit: EntityId,
        amount: u32,
    },
    LootDropped {
        corpse: EntityId,
        items: Vec<String>,
    },
    Looted {
        looter: EntityId,
        corpse: EntityId,
        items: Vec<String>,
    },
    /// Boss shout; line choice is round-robin per trigger and never touches the rng.
    Yell {
        unit: EntityId,
        text: String,
    },
    Respawned {
        unit: EntityId,
    },
    Heal {
        source: EntityId,
        target: EntityId,
        spell: Option<SpellId>,
        amount: u32,
    },
    /// Effective mana restored by a channel tick (clamped to max mana).
    ManaGain {
        unit: EntityId,
        spell: SpellId,
        amount: u32,
    },
    /// Impulse away from the caster. The sim moved sim-owned units already; the
    /// client applies it to the player body.
    Knockback {
        unit: EntityId,
        dir: Vec3,
        strength: f32,
    },
    /// The unit was moved to `pos` (the client must move the player body there).
    Teleported {
        unit: EntityId,
        pos: Vec3,
    },
    LevelUp {
        unit: EntityId,
        level: u8,
    },
    SpellLearned {
        unit: EntityId,
        spell: SpellId,
    },
    PhaseChanged {
        unit: EntityId,
        phase: u8,
    },
    Summoned {
        unit: EntityId,
        summoner: EntityId,
    },
    /// The unit is gone for good (floor change, or a summoned add that finished evading).
    Despawned {
        unit: EntityId,
    },
    /// Deprecated: also emitted for `tower/<n>` zones until the client switches to zone events.
    FloorLoaded {
        floor: u8,
    },
    /// Deprecated twin of `ZoneCleared` for `tower/<n>` zones.
    FloorCleared {
        floor: u8,
    },
    ZoneLoaded {
        zone: String,
    },
    /// The zone's boss died (once per session; campus has no boss).
    ZoneCleared {
        zone: String,
    },
    /// The first gated link into `zone` just opened.
    Unlocked {
        zone: String,
    },
    /// Talking to a quest giver that has quests to offer or to take back (ids in content order).
    QuestOffered {
        npc: EntityId,
        quests: Vec<String>,
    },
    QuestAccepted {
        quest: String,
    },
    /// `objective` indexes `QuestDef::objectives`.
    QuestProgress {
        quest: String,
        objective: usize,
        count: u32,
        need: u32,
    },
    /// Every objective is complete; turn it in at the giver.
    QuestReady {
        quest: String,
    },
    QuestCompleted {
        quest: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CastView {
    pub spell: SpellId,
    pub elapsed_ms: u32,
    pub total_ms: u32,
    pub interruptible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuraView {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub harmful: bool,
    pub remaining_ms: u32,
    pub total_ms: u32,
    pub absorb_remaining: Option<u32>,
    pub stacks: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnitState {
    pub id: EntityId,
    pub key: String,
    pub name: String,
    pub level: u8,
    pub hostile: bool,
    /// The sim owns positions of non-player units.
    pub pos: Vec3,
    pub yaw: f32,
    pub hp: u32,
    pub max_hp: u32,
    pub mana: u32,
    pub max_mana: u32,
    pub alive: bool,
    pub in_combat: bool,
    pub target: Option<EntityId>,
    pub cast: Option<CastView>,
    pub auras: Vec<AuraView>,
    pub ai: Option<AiState>,
    pub lootable: bool,
    pub respawn_in_ms: Option<u32>,
    /// Hostile units: XP awarded on kill. Player: accumulated XP.
    pub xp: u32,
    pub auto_attack: bool,
    /// Time until the next auto swing; 0 when off or ready and waiting.
    pub swing_remaining_ms: u32,
    /// Player: XP needed for the next level (0 at the cap); others 0.
    pub xp_to_next: u32,
    /// The client must freeze the player while stunned.
    pub stunned: bool,
    pub silenced: bool,
    pub boss: bool,
    /// Social aggro group: when one member is pulled the whole pack joins.
    pub pack: Option<u32>,
    /// Boss phase, starting at 1.
    pub phase: u8,
    pub idle_pose: Option<IdlePose>,
    /// Player trophies as (item name, copies) in first-pickup order; empty for other units.
    pub inventory: Vec<(String, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CooldownView {
    pub gcd_remaining_ms: u32,
    pub gcd_total_ms: u32,
    pub cd_remaining_ms: u32,
    pub cd_total_ms: u32,
    /// Full legality now, including the current target.
    pub usable: Result<(), CastError>,
}
