//! Public value types shared with the client: commands in, events out.

use crate::content::{IdlePose, School, SpellId};
use crate::geom::Vec3;
use crate::targeting::{EntityId, TargetError};
use crate::telegraph::TelegraphShape;
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
    InCombat,
    TargetNotDead,
    /// Mounting in a zone without `ZoneDef::scooters`.
    NoScooters,
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
            CastError::InCombat => "Can't do that while in combat.",
            CastError::TargetNotDead => "Your target is not dead. Yet.",
            CastError::NoScooters => "No scooters in the office. (Facilities)",
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
    /// Hand in a ready quest at its giver, or its `turn_in` NPC (talk range): XP, SP, trophy
    /// and, for a gear reward, `Content::quest_gear_choices` entry `choice` are granted.
    TurnInQuest {
        quest: String,
        choice: u8,
    },
    /// Drop an active quest and its progress; it can be accepted again.
    AbandonQuest {
        quest: String,
    },
    /// Equip a piece of gear from the bag (the old piece goes back to the bag).
    Equip {
        item: String,
    },
    /// Move the piece in `slot` (`Slot::index`) to the bag.
    Unequip {
        slot: u8,
    },
    /// Buy from a vendor within `vendor_range_m`.
    Buy {
        vendor: EntityId,
        item: String,
    },
    Sell {
        vendor: EntityId,
        item: String,
    },
    /// Buy back something sold this session at its sell price.
    Buyback {
        vendor: EntityId,
        item: String,
    },
    /// Drink a consumable from the bag or trigger an equipped piece's on-use perk.
    UseItem {
        item: String,
    },
    /// Use an interactive prop (`WorldDef::props` key) within its reach in the loaded zone.
    Interact {
        prop: String,
    },
    /// Wear the title of an unlocked achievement under the name; "" takes it off.
    SetTitle {
        achievement: String,
    },
}

/// Why an item command did nothing; routed to the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemError {
    TooFar,
    NotEnoughSp,
    BagFull,
    NotForSale,
    NotInBag,
    CannotEquip,
    CannotUse,
    NotReady,
    Dead,
}

impl ItemError {
    pub fn message(self) -> &'static str {
        match self {
            ItemError::TooFar => "Procurement only talks to you in person. Walk closer.",
            ItemError::NotEnoughSp => "Not enough Story Points. Have you tried estimating higher?",
            ItemError::BagFull => "Your bag is full. So is your calendar.",
            ItemError::NotForSale => "That is not in stock. Expected delivery: 3-6 weeks.",
            ItemError::NotInBag => "You don't have that. Check the other laptop.",
            ItemError::CannotEquip => "That's not something you can wear to the office.",
            ItemError::CannotUse => "Nothing happens. Like most initiatives.",
            ItemError::NotReady => "Not ready yet. It's still in code review.",
            ItemError::Dead => "You are dead. Equipment is the least of your problems.",
        }
    }
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
    /// One member's roll on a kill: items (trophies, gear, junk) and Story Points.
    LootDropped {
        corpse: EntityId,
        items: Vec<String>,
        sp: u32,
    },
    Looted {
        looter: EntityId,
        corpse: EntityId,
        items: Vec<String>,
        sp: u32,
    },
    /// Boss shout; line choice is round-robin per trigger and never touches the rng.
    Yell {
        unit: EntityId,
        text: String,
    },
    Respawned {
        unit: EntityId,
    },
    /// A group member brought `unit` back where it fell, with this much HP and mana.
    Resurrected {
        unit: EntityId,
        by: EntityId,
        hp: u32,
        mana: u32,
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
    /// A ground telegraph of `spell` will land `resolve_ms` from now on whoever of the other
    /// side is inside `shape` at `pos` (cones and lines open along `facing`, a yaw). `cover`
    /// lists the safe spots (radius in the spell's `TelegraphDef`), usually empty.
    TelegraphStarted {
        id: u32,
        source: EntityId,
        spell: SpellId,
        shape: TelegraphShape,
        pos: Vec3,
        facing: f32,
        resolve_ms: u32,
        cover: Vec<Vec3>,
    },
    /// The telegraph landed (its damage events follow) or, with `landed: false`, was dropped
    /// because its source reset.
    TelegraphResolved {
        id: u32,
        landed: bool,
    },
    /// The owner's full economy state after any change (routed to that player only).
    Bag {
        unit: EntityId,
        sp: u32,
        bag: Vec<String>,
        equipped: Vec<String>,
        buyback: Vec<String>,
    },
    /// Equipped gear in slot order ("" = empty), for everyone: it is visible on the model.
    GearChanged {
        unit: EntityId,
        equipped: Vec<String>,
    },
    /// A gear perk, set bonus or consumable fired.
    Proc {
        unit: EntityId,
        item: String,
    },
    ItemError {
        reason: ItemError,
    },
    /// An on-use item started its cooldown (routed to the owner).
    ItemCooldown {
        item: String,
        ms: u32,
    },
    /// Someone used a prop. `outcome` per kind (see `world.rs`): 0 a failed roll, k the k-th
    /// reward of the prop, or a random line number for purely cosmetic props.
    Interacted {
        unit: EntityId,
        prop: String,
        outcome: u32,
    },
    /// A player unlocked an achievement (everyone sees it, as in WoW).
    Achievement {
        unit: EntityId,
        key: String,
    },
    /// The achievement whose title the player now wears ("" for none).
    TitleChanged {
        unit: EntityId,
        title: String,
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
    /// Player Story Points.
    pub sp: u32,
    /// Player bag (gear, junk, consumables), one entry per item.
    pub bag: Vec<String>,
    /// Player gear by `Slot::index`, "" for an empty slot; empty for non-players.
    pub equipped: Vec<String>,
    /// Items sold this session, oldest first.
    pub buyback: Vec<String>,
    /// Player: unlocked achievement keys, oldest first.
    pub achievements: Vec<String>,
    /// Player: key of the achievement whose title is worn, "" for none.
    pub title: String,
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
