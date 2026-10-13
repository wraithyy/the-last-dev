//! Public value types shared with the client: commands in, events out.

use crate::content::{School, SpellId};
use crate::geom::Vec3;
use crate::targeting::{EntityId, TargetError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiState {
    Idle,
    Chase,
    Attack,
    Casting,
    Evade,
    Dead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuraEnd {
    Expired,
    Consumed,
    Depleted,
    Death,
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
}

#[derive(Clone, Debug, PartialEq)]
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
