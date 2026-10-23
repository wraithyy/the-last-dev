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
    /// A stealth opener pressed outside stealth.
    NeedsStealth,
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
            CastError::NeedsStealth => "You must be in stealth. Try Tailgating.",
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
    /// Sell every junk item in the bag to a vendor in range.
    SellJunk {
        vendor: EntityId,
    },
    /// Destroy the bag item at this index. Trophies and active quest items are refused.
    Discard {
        slot: u16,
    },
    /// Destroy every junk item in the bag.
    DiscardJunk,
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
    /// Wear the title of an unlocked achievement (or your picked row-5 talent) under the name;
    /// "" takes it off.
    SetTitle {
        achievement: String,
    },
    /// Career Ladder: take option `choice` (1..=3) of talent row `row` (0..5), out of combat,
    /// once the level allows and the row is still free.
    PickTalent {
        row: u8,
        choice: u8,
    },
    /// Performance Review: clear every talent pick for Story Points, out of combat.
    ResetTalents,
    /// Sprint mode: the group leader commits the group to a timed run of `building` (a
    /// `SprintDef::buildings` key) at `level`, from the campus with everyone alive and calm.
    StartSprint {
        building: String,
        level: u8,
    },
    /// Group lead: this ISO week's Weekly Sprint (building, level and affixes fixed by the week).
    StartWeeklySprint,
    /// Daily Standup (`dailies.rs`): at the Scrum board, join today's dailies and the week's
    /// Sprint Goal.
    JoinStandup,
    /// Claim the finished Sprint Goal at the board; `choice` picks the gear
    /// (`Content::sprint_goal_gear`).
    ClaimSprintGoal {
        choice: u8,
    },
    /// The client is quitting: clocking out at 18:00 sharp earns "Go Home" (`live.rs`).
    GoHome,
    /// Companions (`pets.rs`): bring out an owned pet (`PetDef::key`), or "" to dismiss it.
    SummonPet {
        pet: String,
    },
    /// Code Review Duel (`duel.rs`): ask another group member for a 1v1.
    DuelRequest {
        target: EntityId,
    },
    /// Accept or decline the duel request aimed at you.
    DuelRespond {
        accept: bool,
    },
    /// Group lead: Normal (false) or Legacy Mode (true) for the next zone loads (`legacy.rs`).
    SetLegacy {
        on: bool,
    },
    /// Group lead, on the campus: take the group to minigame `key` (`MinigameDef::key`). The
    /// invite board and the Company Calendar both send it.
    StartMinigame {
        key: String,
    },
    /// End your own minigame run now, without a medal (the client asks first).
    LeaveMinigame,
    /// Answer the choice card of your run (`MinigameView::card`): option `option` of card
    /// `seq`; a pick for an older card does nothing.
    MinigamePick {
        seq: u16,
        option: u8,
    },
    /// Offline only: the device clock when the post-credits chain unlocks, told once by the
    /// client because the sim has no wall clock (`ilmari.rs`). Ignored before the AGI is down
    /// and once a stamp exists; a server never accepts it from the wire.
    StampIlmari {
        unix_secs: u64,
        utc_offset_min: i16,
        tz: String,
    },
    /// Offline only: the Opening has played or was skipped (needs a stamp), which retires the
    /// post-credits Marie beats and completes the opening quest.
    IlmariReveal,
    /// Group lead: Story (true) or Full (false) for the next Ilmari wing loads (`raid.rs`).
    /// Without a pick a group of three or more gets Full, a smaller one Story.
    SetIlmariStory {
        story: bool,
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
    /// Talent picks and resets wait until the fight is over.
    InCombat,
    /// Talent row locked, already taken or nothing to reset.
    NotEligible,
    /// The vendor wants a higher level for that piece (`GearDef::required_level`).
    LevelTooLow,
    /// A trophy or an active quest item cannot be deleted.
    Protected,
    /// The department behind that item does not like you enough yet (`RepGoods::tier`).
    RepTooLow,
    /// A token vendor wants more of its currency (`VendorDef::token`); offline Ilmari shops only.
    NotEnoughTokens,
}

impl ItemError {
    pub fn message(self) -> &'static str {
        match self {
            ItemError::TooFar => "Procurement only talks to you in person. Walk closer.",
            ItemError::NotEnoughSp => "Not enough Kudos. Have you tried being visible in meetings?",
            ItemError::BagFull => "Your bag is full. So is your calendar.",
            ItemError::NotForSale => "That is not in stock. Expected delivery: 3-6 weeks.",
            ItemError::NotInBag => "You don't have that. Check the other laptop.",
            ItemError::CannotEquip => "That's not something you can wear to the office.",
            ItemError::CannotUse => "Nothing happens. Like most initiatives.",
            ItemError::NotReady => "Not ready yet. It's still in code review.",
            ItemError::Dead => "You are dead. Equipment is the least of your problems.",
            ItemError::InCombat => "Performance reviews are not scheduled during incidents.",
            ItemError::NotEligible => "Not eligible for that promotion. Try again next cycle.",
            ItemError::LevelTooLow => {
                "Procurement says that is above your pay grade. Come back after a promotion."
            }
            ItemError::Protected => "That one is load-bearing. Compliance would notice.",
            ItemError::RepTooLow => {
                "That department has not added you to the right mailing list yet. Earn more reputation."
            }
            ItemError::NotEnoughTokens => {
                "Brenda needs three quotes and you have one. Come back with more tokens."
            }
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
    /// A timed objective ran out: the quest left the log and can be accepted again.
    QuestFailed {
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
    /// A player's Career Ladder picks changed (everyone sees the promotions).
    TalentsChanged {
        unit: EntityId,
        talents: crate::talents::TalentPicks,
    },
    /// Sprint mode: a run started; the host loads `zones[0]` when `Sim::sprint_travel` says so.
    SprintStarted {
        status: crate::sprint::SprintStatus,
        zones: Vec<String>,
    },
    /// Once a second during a run (timer, story points burned, bosses left).
    SprintProgress {
        status: crate::sprint::SprintStatus,
    },
    /// The run is over: the review, keystone change and the chest (`upgrade` is 0 unless
    /// delivered).
    SprintEnded {
        result: crate::sprint::SprintResult,
        time_ms: u32,
        upgrade: i8,
        summary: crate::sprint::SprintSummary,
    },
    /// An affix did something visible (`unit` is the enemy or player involved, 0 for none).
    SprintAffix {
        affix: String,
        unit: EntityId,
        text: String,
    },
    /// The Weekly Sprint ladder of the room (offline: the local one); after a weekly delivery
    /// and to a joiner.
    Ladder {
        view: crate::sprint::LadderView,
    },
    /// A minigame outing began in the loaded zone: every member has a run (`minigame.rs`).
    MinigameStarted {
        key: String,
        limit_ms: u32,
    },
    /// A run's HUD (routed to its player): on change and once a second.
    Minigame {
        unit: EntityId,
        view: crate::minigame::MinigameView,
    },
    /// Teambuilding: watcher `by` caught `unit`; `line` is what the player gets told (a talker's
    /// take, or the watcher's line).
    MinigameCaught {
        unit: EntityId,
        by: EntityId,
        kind: crate::teambuilding::WatcherKind,
        line: String,
    },
    /// Something happened in your run worth a line on screen (routed): a validation error,
    /// a deflected colleague, a reorg. `good` picks the colour.
    MinigameNote {
        unit: EntityId,
        text: String,
        good: bool,
    },
    /// A run is over (finished with a medal, left early or out of time).
    MinigameEnded {
        result: crate::minigame::MinigameResult,
    },
    /// `StartSprint` was refused (routed to the asker).
    SprintRefused {
        reason: crate::sprint::SprintError,
    },
    /// Legacy Mode: the difficulty, the zone loaded in it and the room's weekly lockout; on
    /// every change, every zone load and to a joiner.
    Legacy {
        view: crate::legacy::LegacyView,
    },
    /// The raid: zone, difficulty, weekly lockout and tethers; on every change, every raid
    /// zone load (and leaving the raid) and to a joiner.
    Raid {
        view: crate::raid::RaidView,
    },
    /// `SetLegacy` was refused (routed to the asker).
    LegacyRefused {
        reason: crate::legacy::LegacyError,
    },
    /// The world clock: unix seconds at this tick (`Sim::set_clock`; sent to a joiner too).
    /// Day/night and the task reset timers follow from it.
    Clock {
        unix_s: u64,
    },
    /// The player's Scrum board (routed): `open` when they walked up to it, else an update.
    DailyBoard {
        board: crate::dailies::BoardView,
        open: bool,
    },
    /// A daily paid out, or the Sprint Goal was claimed (routed).
    TaskCompleted {
        key: String,
        weekly: bool,
        xp: u32,
        sp: u32,
    },
    /// The campus world boss: a warning `secs` before it spawns, the spawn (`unit`, leaves after
    /// `secs` unless engaged), a cancelled meeting or the kill.
    WorldBoss {
        state: crate::live::WorldBossState,
        secs: u32,
        unit: EntityId,
    },
    /// A public world event (`world_events.rs`): warned `secs` ahead, started for `secs`, kill
    /// progress `count`/`need` (`need` 0 = hold out), won or failed.
    WorldEvent {
        key: String,
        phase: crate::world_events::WorldEventPhase,
        secs: u32,
        count: u32,
        need: u32,
    },
    /// A player's active pet changed ("" = none); everyone sees it follow them.
    PetChanged {
        unit: EntityId,
        pet: String,
    },
    /// A pet joined the player's collection (routed to the owner).
    PetLearned {
        unit: EntityId,
        pet: String,
    },
    /// The Roomba swept junk off a nearby corpse into the owner's bag (routed to the owner);
    /// `emptied` when nothing of the owner's roll is left there.
    PetLooted {
        unit: EntityId,
        corpse: EntityId,
        items: Vec<String>,
        emptied: bool,
    },
    /// `from` asked `to` for a code review duel (routed to both).
    DuelRequested {
        from: EntityId,
        to: EntityId,
    },
    /// Accepted: the arena is a circle at `center`; the fight starts in `ms`.
    DuelCountdown {
        a: EntityId,
        b: EntityId,
        center: Vec3,
        radius_m: f32,
        ms: u32,
    },
    /// The countdown is over: `a` and `b` may now hurt each other (and only each other).
    DuelBegan {
        a: EntityId,
        b: EntityId,
    },
    /// The request or duel between `a` (asker) and `b` is over. `comments` counts the hits
    /// that landed; `streak` is the winner's run of wins (0 without a winner).
    DuelEnded {
        a: EntityId,
        b: EntityId,
        winner: Option<EntityId>,
        outcome: crate::duel::DuelOutcome,
        comments: u32,
        streak: u32,
    },
    /// `DuelRequest` was refused (routed to the asker).
    DuelRefused {
        reason: crate::duel::DuelError,
    },
    /// Promotion progress after the level cap (routed): `xp` towards the next rank, `gained`
    /// by this grant.
    RankProgress {
        unit: EntityId,
        rank: u8,
        xp: u32,
        gained: u32,
    },
    /// A player reached a Promotion rank (everyone sees it).
    RankUp {
        unit: EntityId,
        rank: u8,
    },
    /// Department standing changed (routed): `dept` indexes `EndgameDef::departments`.
    Reputation {
        unit: EntityId,
        dept: u8,
        total: u32,
        gained: u32,
    },
    /// Burnout moved to `value` (per mille) by `delta` (everyone sees it: hair follows it).
    Burnout {
        unit: EntityId,
        value: u16,
        delta: i16,
        cause: crate::burnout::BurnoutCause,
    },
    /// A burnt-out player yawned, muttered or replied to all (`line` indexes the content list).
    BurnoutQuip {
        unit: EntityId,
        kind: crate::burnout::QuipKind,
        line: u16,
    },
    /// The Scrum board or the Company Calendar closed `count` Jira tickets from the bag for
    /// `sp` Kudos (routed).
    TicketsClosed {
        unit: EntityId,
        count: u32,
        sp: u32,
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
    /// Player: key of the achievement (or capstone talent) whose title is worn, "" for none.
    pub title: String,
    /// Player: Career Ladder pick per row (0 = none, else option 1..=3); zeros for others.
    pub talents: crate::talents::TalentPicks,
    /// Player: the companion following them (`PetDef::key`), "" for none.
    pub pet: String,
    /// Player: owned pet keys, oldest first.
    pub pets: Vec<String>,
    /// Player: the opponent while a code review duel is being fought.
    pub duel_foe: Option<EntityId>,
    /// Player: duels won in a row (cosmetic).
    pub duel_streak: u32,
    /// Player: Performance Reviews taken so far (each makes the next one dearer).
    pub respecs: u32,
    /// Player: endgame progress (Promotion rank, department reputation, burnout).
    pub career: Career,
}

/// Endgame state of one player; all zero for everyone else.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Career {
    /// Promotion ranks reached after the level cap (0 = none yet).
    pub rank: u8,
    /// XP towards the next rank.
    pub rank_xp: u32,
    /// Standing per department, in `EndgameDef::departments` order.
    pub rep: [u32; crate::endgame::DEPARTMENTS],
    /// Per mille (0..=1000).
    pub burnout: u16,
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
