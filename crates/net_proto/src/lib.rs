//! Wire protocol shared by the co-op server and the game client.
//!
//! A frame is `PROTOCOL_VERSION` (u16, little endian) followed by one postcard-encoded
//! `ClientMsg` or `ServerMsg`, at most `MAX_FRAME_BYTES` in total. Frames travel as binary
//! WebSocket messages, so no length prefix is needed. `decode` checks size, version and
//! trailing bytes, then validates the message: a peer's bytes are untrusted.

use combat_core::{
    AiState, AuraView, Command, Content, CooldownView, EntityId, Event, MAX_PLAYERS, QuestLog,
    SpellId, TelegraphShape, UnitState, Vec3,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Bumped on any incompatible change to the messages or to the combat_core types they carry.
pub const PROTOCOL_VERSION: u16 = 29;
/// Upper bound for one frame in either direction; a full zone snapshot is a few KiB.
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
/// Quest ids are short content keys; anything longer is not a real quest.
pub const MAX_QUEST_ID_BYTES: usize = 64;
/// Raw display names longer than this are rejected before sanitising.
pub const MAX_RAW_NAME_BYTES: usize = 64;
/// Class keys (`Content::classes`) are short; anything longer is not a real class.
pub const MAX_CLASS_BYTES: usize = 32;
/// The server drops a connection whose first message is not `Hello` or that sends none in time.
pub const HELLO_DEADLINE_MS: u64 = 5_000;
/// Collection caps checked by `decode`, well above what one room produces.
pub const MAX_EVENTS_PER_FRAME: usize = 2_048;
pub const MAX_SNAPSHOT_UNITS: usize = 1_024;
/// Items of one loot roll, quests of one offer.
pub const MAX_LIST_LEN: usize = 64;
/// Item names (gear, junk, consumables) are short content keys too.
pub const MAX_ITEM_NAME_BYTES: usize = 64;
/// Error, yell and other free-form server strings (yells are content lines, not user text).
pub const MAX_TEXT_BYTES: usize = 512;
/// Public rooms in one `RoomList`; the server sends the most populated, then newest.
pub const MAX_ROOM_LIST: usize = 50;
/// A sanitised room title (`sanitize_title`) is at most this many ASCII characters.
pub const MAX_TITLE_CHARS: usize = 24;
/// Raw titles longer than this are rejected before sanitising.
pub const MAX_RAW_TITLE_BYTES: usize = 64;
/// Sanitised display names (`sanitize_name`) are at most this long.
pub const MAX_NAME_CHARS: usize = 16;

const HEADER: usize = 2;

// ---- messages ----

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// Must be the first message, within `HELLO_DEADLINE_MS`; `name` is raw and goes
    /// through `sanitize_name`. `class` is a `Content::classes` key the server checks.
    Hello {
        version: u16,
        name: String,
        class: String,
    },
    /// `public` rooms appear in `RoomList`; private ones are reachable only by code. `title`
    /// is raw and goes through `sanitize_title`; empty or unusable gets a generated one.
    CreateRoom {
        public: bool,
        title: String,
    },
    /// Before a room request only: answered with `RoomList` (rate limited per connection
    /// and per IP). The connection stays open for `JoinPublic`, `CreateRoom` or `JoinRoom`.
    ListRooms,
    /// Joins a listed public room by its `RoomInfo::id`, which is not the room code.
    JoinPublic {
        id: u64,
    },
    /// In a room, leader only: list the room publicly or hide it again.
    SetPublic {
        public: bool,
    },
    /// `code` is normalised by `decode` (`parse_room_code`); a bad code fails the decode,
    /// which the server must answer with `ErrorKind::CannotJoin` like any other join failure.
    JoinRoom {
        code: String,
    },
    LeaveRoom,
    /// Player-legal commands only (`is_player_command`); the server applies them with
    /// `Sim::apply_as`, which also rebinds the actor id to the sender.
    Cmd(Command),
    /// Client-predicted movement; `seq` increases per report and is echoed by `MoveCorrection`.
    Move {
        pos: Vec3,
        facing: f32,
        moving: bool,
        jumping: bool,
        seq: u32,
    },
    /// `target` is the player the emote is aimed at; the server keeps it only when it names
    /// another member of the sender's room.
    Emote {
        emote: Emote,
        target: Option<EntityId>,
    },
    /// Echoed as `Pong { t }`; `t` is the client's own clock.
    Ping {
        t: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Followed in the same tick by a full `Snapshot` (units, HP, auras with their timers).
    /// Carries the per-player and per-room state that snapshots do not: the joiner's own quest
    /// log and the zones the group has cleared or unlocked.
    Welcome {
        you: EntityId,
        room: String,
        zone: Option<String>,
        seed: u64,
        players: Vec<PlayerInfo>,
        quests: QuestSnap,
        cleared: Vec<String>,
        unlocked: Vec<String>,
    },
    RoomCreated {
        code: String,
    },
    Error {
        kind: ErrorKind,
        text: String,
    },
    PlayerJoined(PlayerInfo),
    PlayerLeft {
        id: EntityId,
    },
    /// The events of one sim tick that are meant for this client (see `Sim::tick_routed`).
    Events {
        tick: u64,
        events: Vec<Event>,
    },
    /// Late join and periodic drift correction.
    Snapshot {
        tick: u64,
        units: Vec<UnitSnap>,
        players: Vec<PlayerSnap>,
    },
    /// Units that moved or turned since the last `Positions` (10 Hz, between snapshots):
    /// id, position in centimetres, yaw in 1/256 turns (see `UnitSnap`).
    Positions {
        tick: u64,
        units: Vec<(EntityId, [i32; 3], u8)>,
    },
    /// Full aura lists with timers for units that just gained an aura (`AuraApplied` carries
    /// no duration). Snapshots carry the same per unit.
    Auras {
        units: Vec<(EntityId, Vec<AuraSnap>)>,
    },
    /// The caster's own cooldowns after one of its casts, from the sim: the GCD and every
    /// action-bar spell still cooling down, as (spell, remaining ms, total ms).
    Cooldowns {
        tick: u64,
        gcd: (u32, u32),
        spells: Vec<(SpellId, u32, u32)>,
    },
    /// The server rejected the move with this `seq`; the client snaps to `pos` and replays
    /// its later inputs.
    MoveCorrection {
        seq: u32,
        pos: Vec3,
    },
    Emoted {
        player: EntityId,
        emote: Emote,
        target: Option<EntityId>,
    },
    Pong {
        t: u64,
    },
    /// Once a second: p99 of this room's own recent tick durations, in microseconds (the
    /// client's `~/perf` window shows it). Nothing about other rooms or the host.
    TickStat {
        p99_us: u32,
    },
    RoomList {
        rooms: Vec<RoomInfo>,
    },
    /// On join and whenever visibility or the leader changes. `leader` (the first player)
    /// may toggle `public` and start sprints.
    RoomSettings {
        public: bool,
        title: String,
        leader: EntityId,
    },
}

/// One public room in `RoomList`. No code, no address: `id` is a separate random token that
/// only works while the room is public (making it public again draws a new one).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: u64,
    /// `sanitize_title` output.
    pub title: String,
    /// Display name of the leader, `sanitize_name` output.
    pub host: String,
    pub players: u8,
    pub max: u8,
    /// Zone id (`Content::zones`), e.g. `tower/3`.
    pub zone: String,
    pub avg_level: u8,
    /// Distinct class keys of the members, sorted.
    pub roles: Vec<String>,
    pub in_sprint: bool,
    /// Seconds since the room was created.
    pub age_s: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: EntityId,
    /// Already sanitised by the server.
    pub name: String,
    /// `Content::classes` key, validated by the server at `Hello`.
    pub class: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    BadVersion,
    BadMessage,
    /// Unknown room, full room and malformed code all look the same, so the error is no
    /// oracle for guessing codes.
    CannotJoin,
    ServerFull,
    NotInRoom,
    RateLimited,
    /// Something other than `Hello` came first, or `Hello` missed `HELLO_DEADLINE_MS`.
    HelloRequired,
    /// `Hello` named a class the server's content does not have.
    UnknownClass,
    /// A leader-only action (room visibility) from another member.
    NotLeader,
}

impl ErrorKind {
    /// Canonical player-facing text; servers send this rather than details.
    pub fn message(self) -> &'static str {
        match self {
            ErrorKind::BadVersion => "Client and server versions differ. Refresh the page.",
            ErrorKind::BadMessage => "The server did not understand that.",
            ErrorKind::CannotJoin => "Cannot join that room.",
            ErrorKind::ServerFull => "The server is full. Try again later.",
            ErrorKind::NotInRoom => "You are not in a room.",
            ErrorKind::RateLimited => "Slow down.",
            ErrorKind::HelloRequired => "Handshake missing.",
            ErrorKind::UnknownClass => "Unknown role. Pick another one.",
            ErrorKind::NotLeader => "Only the host can change that.",
        }
    }
}

// ---- emotes ----

/// Predefined emotes and quick phrases. There is no free text on the wire, so there is
/// nothing to moderate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Emote {
    Sync,
    CircleBack,
    PerMyLastEmail,
    Lgtm,
    PlusOne,
    CouldHaveBeenAnEmail,
    TakeOffline,
    NoBandwidth,
    Blocked,
    ShipIt,
    /// Animated emotes: the speaker sits down, dances or facepalms (see the client's poses).
    Sit,
    Dance,
    Facepalm,
}

impl Emote {
    pub const ALL: [Emote; 13] = [
        Emote::Sync,
        Emote::CircleBack,
        Emote::PerMyLastEmail,
        Emote::Lgtm,
        Emote::PlusOne,
        Emote::CouldHaveBeenAnEmail,
        Emote::TakeOffline,
        Emote::NoBandwidth,
        Emote::Blocked,
        Emote::ShipIt,
        Emote::Sit,
        Emote::Dance,
        Emote::Facepalm,
    ];

    /// The slash command a player types or picks.
    pub fn command(self) -> &'static str {
        self.entry().0
    }

    /// What the character says.
    pub fn text(self) -> &'static str {
        self.entry().1
    }

    /// The phrase aimed at a colleague ("Let's sync on this, Alice.").
    pub fn text_at(self, name: &str) -> String {
        let template = match self {
            Emote::Sync => "Let's sync on this, {n}.",
            Emote::CircleBack => "Let's circle back on that, {n}.",
            Emote::PerMyLastEmail => "{n}, per my last email...",
            Emote::Lgtm => "LGTM, {n}. Did not read it.",
            Emote::PlusOne => "+1 to whatever {n} said.",
            Emote::CouldHaveBeenAnEmail => "{n}, this could have been an email.",
            Emote::TakeOffline => "{n}, let's take this offline.",
            Emote::NoBandwidth => "{n}, I don't have the bandwidth for you.",
            Emote::Blocked => "I'm blocked on {n}.",
            Emote::ShipIt => "Ship it, {n}. We'll fix it in prod.",
            Emote::Sit => "*sits down next to {n}* Is this seat taken by an agent?",
            Emote::Dance => "*dances with {n}* HR has been notified.",
            Emote::Facepalm => "*facepalms at {n}*",
        };
        template.replace("{n}", name)
    }

    pub fn from_command(cmd: &str) -> Option<Emote> {
        let cmd = cmd.trim();
        Emote::ALL
            .into_iter()
            .find(|e| e.command().eq_ignore_ascii_case(cmd))
    }

    fn entry(self) -> (&'static str, &'static str) {
        match self {
            Emote::Sync => ("/sync", "Let's sync on this."),
            Emote::CircleBack => ("/circle back", "Let's circle back on that."),
            Emote::PerMyLastEmail => ("/per my last email", "Per my last email..."),
            Emote::Lgtm => ("/lgtm", "LGTM. Did not read it."),
            Emote::PlusOne => ("/+1", "+1"),
            Emote::CouldHaveBeenAnEmail => (
                "/this could have been an email",
                "This could have been an email.",
            ),
            Emote::TakeOffline => ("/take this offline", "Let's take this offline."),
            Emote::NoBandwidth => ("/no bandwidth", "I don't have the bandwidth for this."),
            Emote::Blocked => ("/blocked", "I'm blocked."),
            Emote::ShipIt => ("/ship it", "Ship it. We'll fix it in prod."),
            Emote::Sit => ("/sit", "*sits down* Finally, a meeting with no agenda."),
            Emote::Dance => ("/dance", "*dances like nobody is tracking engagement*"),
            Emote::Facepalm => ("/facepalm", "*facepalms* The AI wrote it again."),
        }
    }
}

// ---- snapshots ----

/// One unit in a `Snapshot`. Compact because a snapshot carries every unit of the zone:
/// positions are integer centimetres and the facing is one byte (1.4 degree steps), which
/// postcard's varints shrink to 1-3 bytes each, against 4 for every f32. That precision is
/// plenty for drift correction; the full `UnitState` (auras, cast bar) is rebuilt by the
/// client from `Events`. Names and models come from `key_idx` into the shared content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitSnap {
    pub id: EntityId,
    /// Index into `Content::units`; client and server embed the same content file.
    pub key_idx: u16,
    pub level: u8,
    /// x, y, z in centimetres.
    pub pos_cm: [i32; 3],
    /// Yaw in 1/256 turns.
    pub facing: u8,
    pub hp: u32,
    pub max_hp: u32,
    pub target: Option<EntityId>,
    /// `UnitSnap::ALIVE` etc.
    pub flags: u8,
    pub auras: Vec<AuraSnap>,
}

/// One aura with its timer; the name and icon come from `key_idx` into `Content::auras`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuraSnap {
    pub key_idx: u16,
    pub remaining_ms: u32,
    pub total_ms: u32,
    pub stacks: u8,
    pub absorb: Option<u32>,
}

impl AuraSnap {
    pub fn from_view(a: &AuraView, content: &Content) -> Option<AuraSnap> {
        let key_idx = content.auras.iter().position(|d| d.key == a.key)?;
        Some(AuraSnap {
            key_idx: u16::try_from(key_idx).ok()?,
            remaining_ms: a.remaining_ms,
            total_ms: a.total_ms,
            stacks: a.stacks,
            absorb: a.absorb_remaining,
        })
    }

    pub fn to_view(&self, content: &Content) -> Option<AuraView> {
        let d = content.auras.get(usize::from(self.key_idx))?;
        Some(AuraView {
            key: d.key.clone(),
            name: d.name.clone(),
            icon: d.icon.clone(),
            harmful: d.harmful,
            remaining_ms: self.remaining_ms,
            total_ms: self.total_ms,
            absorb_remaining: self.absorb,
            stacks: self.stacks,
        })
    }
}

/// A player's quest log on the wire: (quest id, objective counts) in accept order, then the
/// completed ids in turn-in order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestSnap {
    pub active: Vec<(String, Vec<u32>)>,
    pub completed: Vec<String>,
}

impl QuestSnap {
    pub fn from_log(log: &QuestLog) -> QuestSnap {
        QuestSnap {
            active: log
                .active
                .iter()
                .map(|q| (q.id.clone(), q.counts.clone()))
                .collect(),
            completed: log.completed.clone(),
        }
    }

    pub fn to_log(&self) -> QuestLog {
        QuestLog {
            active: self
                .active
                .iter()
                .map(|(id, counts)| combat_core::ActiveQuest {
                    id: id.clone(),
                    counts: counts.clone(),
                })
                .collect(),
            completed: self.completed.clone(),
            kills: Default::default(),
        }
    }
}

/// The `Cooldowns` payload for `spells` as `cooldown` reports them.
pub fn cooldowns_msg(
    tick: u64,
    spells: impl IntoIterator<Item = SpellId>,
    cooldown: impl Fn(SpellId) -> CooldownView,
) -> ServerMsg {
    let mut gcd = (0, 0);
    let mut list = Vec::new();
    for spell in spells {
        let v = cooldown(spell);
        if v.gcd_remaining_ms > 0 {
            gcd = (v.gcd_remaining_ms, v.gcd_total_ms);
        }
        if v.cd_remaining_ms > 0 && list.len() < MAX_LIST_LEN {
            list.push((spell, v.cd_remaining_ms, v.cd_total_ms));
        }
    }
    ServerMsg::Cooldowns {
        tick,
        gcd,
        spells: list,
    }
}

impl UnitSnap {
    pub const ALIVE: u8 = 1;
    pub const IN_COMBAT: u8 = 1 << 1;
    pub const CASTING: u8 = 1 << 2;
    pub const STUNNED: u8 = 1 << 3;
    pub const LOOTABLE: u8 = 1 << 4;
    pub const EVADING: u8 = 1 << 5;
    pub const AUTO_ATTACK: u8 = 1 << 6;

    /// `None` for a unit whose key is not in `content` (cannot happen for sim units).
    pub fn from_state(u: &UnitState, content: &Content) -> Option<UnitSnap> {
        let key_idx = content.units.iter().position(|d| d.key == u.key)?;
        let flag = |on: bool, f: u8| if on { f } else { 0 };
        Some(UnitSnap {
            id: u.id,
            key_idx: u16::try_from(key_idx).ok()?,
            level: u.level,
            pos_cm: pos_cm(u.pos),
            facing: quantize_yaw(u.yaw),
            hp: u.hp,
            max_hp: u.max_hp,
            target: u.target,
            flags: flag(u.alive, Self::ALIVE)
                | flag(u.in_combat, Self::IN_COMBAT)
                | flag(u.cast.is_some(), Self::CASTING)
                | flag(u.stunned, Self::STUNNED)
                | flag(u.lootable, Self::LOOTABLE)
                | flag(u.ai == Some(AiState::Evade), Self::EVADING)
                | flag(u.auto_attack, Self::AUTO_ATTACK),
            auras: u
                .auras
                .iter()
                .filter_map(|a| AuraSnap::from_view(a, content))
                .take(MAX_LIST_LEN)
                .collect(),
        })
    }

    pub fn pos(&self) -> Vec3 {
        from_cm(self.pos_cm)
    }

    /// Yaw in radians, in `[0, 2*pi)`.
    pub fn yaw(&self) -> f32 {
        yaw_from_byte(self.facing)
    }

    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

/// Per-player numbers a remote party frame needs on top of its `UnitSnap`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSnap {
    pub id: EntityId,
    /// `Content::classes` key of the player's class (empty if its unit has none).
    pub class: String,
    pub mana: u32,
    pub max_mana: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    /// Equipped gear by slot as indices into `Content::economy.gear` (`NO_GEAR` = empty), so
    /// late joiners see everyone's outfit.
    pub gear: Vec<u16>,
    /// Key of the achievement (or capstone talent) whose title the player wears ("" for none).
    pub title: String,
    /// Career Ladder pick per row (0 = none), so others see the promotions.
    pub talents: combat_core::TalentPicks,
    /// The companion following the player (`PetDef::key`, "" for none).
    pub pet: String,
    /// Burnout per mille: peers' hair thins with it too (late joiners included).
    pub burnout: u16,
}

impl PlayerSnap {
    pub const NO_GEAR: u16 = u16::MAX;

    /// The gear names by slot ("" for empty or unknown indices).
    pub fn equipped(&self, content: &Content) -> Vec<String> {
        self.gear
            .iter()
            .map(|&k| {
                content
                    .economy
                    .gear
                    .get(usize::from(k))
                    .map_or_else(String::new, |g| g.name.clone())
            })
            .collect()
    }

    pub fn from_state(u: &UnitState, content: &Content) -> PlayerSnap {
        PlayerSnap {
            id: u.id,
            class: content
                .class_of_unit(&u.key)
                .map(|c| c.key.clone())
                .unwrap_or_default(),
            mana: u.mana,
            max_mana: u.max_mana,
            xp: u.xp,
            xp_to_next: u.xp_to_next,
            gear: u
                .equipped
                .iter()
                .map(|n| {
                    content
                        .economy
                        .gear
                        .iter()
                        .position(|g| g.name == *n)
                        .and_then(|k| u16::try_from(k).ok())
                        .unwrap_or(Self::NO_GEAR)
                })
                .collect(),
            title: u.title.clone(),
            talents: u.talents,
            pet: u.pet.clone(),
            burnout: u.career.burnout,
        }
    }
}

fn cm(m: f32) -> i32 {
    // `as` saturates and maps NaN to 0, which is what a corrupt position deserves.
    (m * 100.0).round() as i32
}

/// A position as the integer centimetres snapshots and `Positions` carry.
pub fn pos_cm(p: Vec3) -> [i32; 3] {
    [cm(p.x), cm(p.y), cm(p.z)]
}

pub fn from_cm([x, y, z]: [i32; 3]) -> Vec3 {
    Vec3::new(x as f32 / 100.0, y as f32 / 100.0, z as f32 / 100.0)
}

/// Yaw in radians, in `[0, 2*pi)`, from 1/256 turns.
pub fn yaw_from_byte(facing: u8) -> f32 {
    f32::from(facing) * std::f32::consts::TAU / 256.0
}

/// Yaw as 1/256 turns.
pub fn quantize_yaw(yaw: f32) -> u8 {
    let turns = (yaw / std::f32::consts::TAU).rem_euclid(1.0);
    ((turns * 256.0).round() as u32 % 256) as u8
}

// ---- command filter ----

/// The commands a client may send through `ClientMsg::Cmd`. Movement has its own message
/// (with a sequence number for corrections). Debug and admin hooks are `Sim` methods, not
/// commands, so they cannot be reached from the wire; the match has no wildcard, so a new
/// `Command` variant must be classified here before it compiles.
pub fn is_player_command(cmd: &Command) -> bool {
    match cmd {
        Command::PlayerMoved { .. } => false,
        Command::AcceptQuest { quest }
        | Command::TurnInQuest { quest, .. }
        | Command::AbandonQuest { quest } => quest.len() <= MAX_QUEST_ID_BYTES,
        Command::Equip { item }
        | Command::UseItem { item }
        | Command::Buy { item, .. }
        | Command::Sell { item, .. }
        | Command::Buyback { item, .. } => item.len() <= MAX_ITEM_NAME_BYTES,
        Command::Unequip { slot } => usize::from(*slot) < combat_core::SLOTS,
        // The bag is cut to `MAX_LIST_LEN` on the wire too, so a larger index cannot be real.
        Command::Discard { slot } => usize::from(*slot) < MAX_LIST_LEN,
        Command::DiscardJunk | Command::SellJunk { .. } => true,
        Command::SetTarget { .. }
        | Command::Cast { .. }
        | Command::StopCasting { .. }
        | Command::Loot { .. }
        | Command::StartAutoAttack { .. }
        | Command::StopAutoAttack { .. }
        | Command::RespawnPlayer
        | Command::Talk { .. } => true,
        Command::Interact { prop } => prop.len() <= combat_core::world::WORLD_KEY_MAX,
        Command::SetTitle { achievement } => achievement.len() <= combat_core::world::WORLD_KEY_MAX,
        Command::PickTalent { row, choice } => {
            usize::from(*row) < combat_core::TALENT_ROWS && *choice <= combat_core::TALENT_CHOICES
        }
        Command::ResetTalents => true,
        Command::StartSprint { building, .. } => building.len() <= MAX_QUEST_ID_BYTES,
        Command::JoinStandup | Command::ClaimSprintGoal { .. } | Command::GoHome => true,
        Command::SummonPet { pet } => pet.len() <= combat_core::world::WORLD_KEY_MAX,
        Command::DuelRequest { .. } | Command::DuelRespond { .. } => true,
        Command::SetLegacy { .. } | Command::StartWeeklySprint => true,
        Command::StartMinigame { key } => key.len() <= combat_core::minigame::MINIGAME_KEY_MAX,
        Command::LeaveMinigame | Command::MinigamePick { .. } => true,
    }
}

// ---- encoding ----

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtoError {
    TooLarge(usize),
    BadVersion(u16),
    /// Truncated, trailing bytes or not a valid message.
    Malformed,
    IllegalCommand,
    BadName,
    BadRoomCode,
    BadNumber,
    /// A collection or text over its cap.
    TooMany,
}

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtoError::TooLarge(n) => write!(f, "frame of {n} bytes exceeds {MAX_FRAME_BYTES}"),
            ProtoError::BadVersion(v) => {
                write!(f, "protocol version {v}, expected {PROTOCOL_VERSION}")
            }
            ProtoError::Malformed => f.write_str("malformed frame"),
            ProtoError::IllegalCommand => f.write_str("command not allowed from a client"),
            ProtoError::BadName => f.write_str("display name too long"),
            ProtoError::BadRoomCode => f.write_str("not a room code"),
            ProtoError::BadNumber => f.write_str("non-finite number"),
            ProtoError::TooMany => f.write_str("collection or text over its cap"),
        }
    }
}

impl std::error::Error for ProtoError {}

/// A top-level message. Sealed: only `ClientMsg` and `ServerMsg` travel.
pub trait Msg: Serialize + DeserializeOwned + sealed::Sealed {
    /// Checks a decoded message and normalises it (room code case).
    fn validate(self) -> Result<Self, ProtoError>;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::ClientMsg {}
    impl Sealed for super::ServerMsg {}
}

impl Msg for ClientMsg {
    fn validate(self) -> Result<Self, ProtoError> {
        match self {
            ClientMsg::Hello { name, .. } if name.len() > MAX_RAW_NAME_BYTES => {
                Err(ProtoError::BadName)
            }
            ClientMsg::Hello { class, .. } if class.len() > MAX_CLASS_BYTES => {
                Err(ProtoError::BadName)
            }
            ClientMsg::CreateRoom { title, .. } if title.len() > MAX_RAW_TITLE_BYTES => {
                Err(ProtoError::TooMany)
            }
            ClientMsg::JoinRoom { code } => Ok(ClientMsg::JoinRoom {
                code: parse_room_code(&code)?,
            }),
            ClientMsg::Cmd(c) if !is_player_command(&c) => Err(ProtoError::IllegalCommand),
            ClientMsg::Move { pos, facing, .. }
                if ![pos.x, pos.y, pos.z, facing].iter().all(|v| v.is_finite()) =>
            {
                Err(ProtoError::BadNumber)
            }
            m => Ok(m),
        }
    }
}

impl Msg for ServerMsg {
    fn validate(self) -> Result<Self, ProtoError> {
        let short = |t: &str| t.len() <= MAX_TEXT_BYTES;
        let ok = match &self {
            ServerMsg::Welcome {
                room,
                zone,
                players,
                quests,
                cleared,
                unlocked,
                ..
            } => {
                let ids = |v: &[String]| {
                    v.len() <= MAX_LIST_LEN && v.iter().all(|q| q.len() <= MAX_QUEST_ID_BYTES)
                };
                short(room)
                    && zone.as_deref().is_none_or(short)
                    && players.len() <= MAX_PLAYERS
                    && players
                        .iter()
                        .all(|p| short(&p.name) && p.class.len() <= MAX_CLASS_BYTES)
                    && quests.active.len() <= MAX_LIST_LEN
                    && quests
                        .active
                        .iter()
                        .all(|(q, c)| q.len() <= MAX_QUEST_ID_BYTES && c.len() <= MAX_LIST_LEN)
                    && ids(&quests.completed)
                    && ids(cleared)
                    && ids(unlocked)
            }
            ServerMsg::Positions { units, .. } => units.len() <= MAX_SNAPSHOT_UNITS,
            ServerMsg::Auras { units } => {
                units.len() <= MAX_SNAPSHOT_UNITS
                    && units.iter().all(|(_, a)| a.len() <= MAX_LIST_LEN)
            }
            ServerMsg::Cooldowns { spells, .. } => spells.len() <= MAX_LIST_LEN,
            ServerMsg::RoomCreated { code } => short(code),
            ServerMsg::Error { text, .. } => short(text),
            ServerMsg::PlayerJoined(p) => short(&p.name) && p.class.len() <= MAX_CLASS_BYTES,
            ServerMsg::Events { events, .. } => {
                if events.len() > MAX_EVENTS_PER_FRAME {
                    return Err(ProtoError::TooMany);
                }
                return events.iter().try_for_each(check_event).map(|_| self);
            }
            ServerMsg::Snapshot { units, players, .. } => {
                units.len() <= MAX_SNAPSHOT_UNITS
                    && players.len() <= MAX_PLAYERS
                    && players.iter().all(|p| {
                        p.class.len() <= MAX_CLASS_BYTES
                            && p.gear.len() <= combat_core::SLOTS
                            && p.title.len() <= combat_core::world::WORLD_KEY_MAX
                            && p.pet.len() <= combat_core::world::WORLD_KEY_MAX
                            && talents_ok(&p.talents)
                    })
                    && units.iter().all(|u| u.auras.len() <= MAX_LIST_LEN)
            }
            ServerMsg::MoveCorrection { pos, .. } => {
                return finite(&[pos.x, pos.y, pos.z]).map(|_| self);
            }
            ServerMsg::RoomList { rooms } => {
                rooms.len() <= MAX_ROOM_LIST
                    && rooms.iter().all(|r| {
                        title_ok(&r.title)
                            && r.host.len() <= MAX_NAME_CHARS
                            && clean(&r.host)
                            && r.zone.len() <= MAX_QUEST_ID_BYTES
                            && r.roles.len() <= MAX_PLAYERS
                            && r.roles.iter().all(|c| c.len() <= MAX_CLASS_BYTES)
                    })
            }
            ServerMsg::RoomSettings { title, .. } => title_ok(title),
            ServerMsg::PlayerLeft { .. }
            | ServerMsg::Emoted { .. }
            | ServerMsg::Pong { .. }
            | ServerMsg::TickStat { .. } => true,
        };
        if ok {
            Ok(self)
        } else {
            Err(ProtoError::TooMany)
        }
    }
}

fn talents_ok(t: &combat_core::TalentPicks) -> bool {
    t.iter().all(|k| *k <= combat_core::TALENT_CHOICES)
}

fn finite(v: &[f32]) -> Result<(), ProtoError> {
    if v.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(ProtoError::BadNumber)
    }
}

/// Floats must be finite (a NaN position would poison the client's transforms) and lists
/// and texts bounded.
fn check_event(e: &Event) -> Result<(), ProtoError> {
    let list = |n: usize| {
        if n <= MAX_LIST_LEN {
            Ok(())
        } else {
            Err(ProtoError::TooMany)
        }
    };
    let names = |v: &[String]| {
        list(v.len())?;
        if v.iter().all(|n| n.len() <= MAX_ITEM_NAME_BYTES) {
            Ok(())
        } else {
            Err(ProtoError::TooMany)
        }
    };
    match e {
        Event::Knockback { dir, strength, .. } => finite(&[dir.x, dir.y, dir.z, *strength]),
        Event::Teleported { pos, .. } => finite(&[pos.x, pos.y, pos.z]),
        Event::TelegraphStarted {
            shape,
            pos,
            facing,
            cover,
            ..
        } => {
            list(cover.len())?;
            let dims = match *shape {
                TelegraphShape::Circle { radius_m } => [radius_m, 0.0],
                TelegraphShape::Cone {
                    radius_m,
                    angle_deg,
                } => [radius_m, angle_deg],
                TelegraphShape::Line { length_m, width_m } => [length_m, width_m],
            };
            finite(&[pos.x, pos.y, pos.z, *facing, dims[0], dims[1]])?;
            cover.iter().try_for_each(|c| finite(&[c.x, c.y, c.z]))
        }
        Event::LootDropped { items, .. }
        | Event::Looted { items, .. }
        | Event::PetLooted { items, .. } => names(items),
        Event::DuelCountdown {
            center, radius_m, ..
        } => finite(&[center.x, center.y, center.z, *radius_m]),
        Event::Bag {
            bag,
            equipped,
            buyback,
            ..
        } => names(bag).and(names(equipped)).and(names(buyback)),
        Event::GearChanged { equipped, .. } => names(equipped),
        Event::Proc { item, .. } | Event::ItemCooldown { item, .. } => {
            names(std::slice::from_ref(item))
        }
        Event::QuestOffered { quests, .. } => list(quests.len()),
        Event::Interacted { prop: k, .. }
        | Event::Achievement { key: k, .. }
        | Event::TitleChanged { title: k, .. }
        | Event::PetChanged { pet: k, .. }
        | Event::PetLearned { pet: k, .. } => {
            if k.len() <= combat_core::world::WORLD_KEY_MAX {
                Ok(())
            } else {
                Err(ProtoError::TooMany)
            }
        }
        Event::Yell { text, .. } if text.len() > MAX_TEXT_BYTES => Err(ProtoError::TooMany),
        Event::TalentsChanged { talents, .. } if !talents_ok(talents) => Err(ProtoError::TooMany),
        Event::SprintStarted { status, zones } => check_sprint(status).and(names(zones)),
        Event::SprintProgress { status } => check_sprint(status),
        Event::SprintEnded { summary, .. } => {
            check_sprint(&summary.status)?;
            if summary.meter.len() > MAX_PLAYERS
                || summary.rewards.len() > MAX_PLAYERS
                || summary
                    .meter
                    .iter()
                    .any(|m| m.class.len() > MAX_CLASS_BYTES)
            {
                return Err(ProtoError::TooMany);
            }
            summary.rewards.iter().try_for_each(|r| names(&r.items))
        }
        Event::SprintAffix { affix, text, .. }
            if affix.len() > combat_core::world::WORLD_KEY_MAX || text.len() > MAX_TEXT_BYTES =>
        {
            Err(ProtoError::TooMany)
        }
        Event::DailyBoard { board, .. } => check_board(board),
        Event::TaskCompleted { key, .. } | Event::WorldEvent { key, .. }
            if key.len() > combat_core::world::WORLD_KEY_MAX =>
        {
            Err(ProtoError::TooMany)
        }
        Event::Reputation { dept, .. } if usize::from(*dept) >= combat_core::DEPARTMENTS => {
            Err(ProtoError::BadNumber)
        }
        Event::Burnout { value, .. } if *value > combat_core::BURNOUT_MAX => {
            Err(ProtoError::BadNumber)
        }
        Event::Legacy { view } => {
            names(&view.locked)?;
            names(std::slice::from_ref(&view.zone))
        }
        Event::Raid { view } => {
            names(&view.locked)?;
            list(view.tethers.len())?;
            names(std::slice::from_ref(&view.zone))
        }
        Event::Ladder { view } => {
            list(view.entries.len())?;
            for e in &view.entries {
                names(&e.crew)?;
            }
            Ok(())
        }
        Event::MinigameStarted { key, .. }
            if key.len() > combat_core::minigame::MINIGAME_KEY_MAX =>
        {
            Err(ProtoError::TooMany)
        }
        Event::Minigame { view, .. } => check_minigame_view(view),
        Event::MinigameCaught { line: text, .. } | Event::MinigameNote { text, .. }
            if text.len() > MAX_TEXT_BYTES =>
        {
            Err(ProtoError::TooMany)
        }
        Event::MinigameEnded { result } => {
            names(&result.items)?;
            if result.key.len() > combat_core::minigame::MINIGAME_KEY_MAX
                || result.summary.len() > MAX_TEXT_BYTES
            {
                return Err(ProtoError::TooMany);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// A minigame HUD: a key, a few short lines and bars, one card of a few options, a badge per
/// unit at most.
fn check_minigame_view(v: &combat_core::MinigameView) -> Result<(), ProtoError> {
    use combat_core::minigame::{CARD_OPTIONS_MAX, MINIGAME_KEY_MAX, VIEW_LINES_MAX};
    let text = |t: &str| t.len() <= MAX_TEXT_BYTES;
    let card_ok = v.card.as_ref().is_none_or(|c| {
        text(&c.who)
            && text(&c.text)
            && c.options.len() <= CARD_OPTIONS_MAX
            && c.options.iter().all(|o| text(o))
    });
    let ok = v.key.len() <= MINIGAME_KEY_MAX
        && v.lines.len() <= VIEW_LINES_MAX
        && v.lines.iter().all(|l| text(l))
        && v.gauges.len() <= VIEW_LINES_MAX
        && v.gauges.iter().all(|g| text(&g.label))
        && card_ok
        && v.badges.len() <= MAX_LIST_LEN
        && v.badges.iter().all(|(_, b)| b.len() <= MAX_ITEM_NAME_BYTES);
    if ok { Ok(()) } else { Err(ProtoError::TooMany) }
}

/// A board is a handful of tasks with a few goals each; keys and names are short content keys.
fn check_board(b: &combat_core::BoardView) -> Result<(), ProtoError> {
    use combat_core::GoalSpec;
    let key = |k: &str| k.len() <= MAX_ITEM_NAME_BYTES;
    let goal_ok = |g: &GoalSpec| match g {
        GoalSpec::Kill { zone, unit } => {
            zone.as_deref().is_none_or(key) && unit.as_deref().is_none_or(key)
        }
        GoalSpec::Idle { zone } => key(zone),
        GoalSpec::Talk { unit } => key(unit),
        _ => true,
    };
    let ok = b.tasks.len() <= MAX_LIST_LEN
        && b.tasks.iter().all(|t| {
            key(&t.key) && t.goals.len() <= MAX_LIST_LEN && t.goals.iter().all(|g| goal_ok(&g.goal))
        });
    if ok { Ok(()) } else { Err(ProtoError::TooMany) }
}

/// Building and affix keys are short content keys, at most one per affix level.
fn check_sprint(s: &combat_core::SprintStatus) -> Result<(), ProtoError> {
    let short = |k: &String| k.len() <= MAX_ITEM_NAME_BYTES;
    if s.affixes.len() <= MAX_LIST_LEN && short(&s.building) && s.affixes.iter().all(short) {
        Ok(())
    } else {
        Err(ProtoError::TooMany)
    }
}

/// Header plus postcard body. Fails only on a frame over `MAX_FRAME_BYTES` (the caller
/// should split or drop it) or a serializer error.
pub fn encode<M: Msg>(msg: &M) -> Result<Vec<u8>, ProtoError> {
    let out = postcard::to_extend(msg, PROTOCOL_VERSION.to_le_bytes().to_vec())
        .map_err(|_| ProtoError::Malformed)?;
    if out.len() > MAX_FRAME_BYTES {
        return Err(ProtoError::TooLarge(out.len()));
    }
    Ok(out)
}

/// Size and version first, so an oversize or foreign frame costs no parsing.
pub fn decode<M: Msg>(bytes: &[u8]) -> Result<M, ProtoError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ProtoError::TooLarge(bytes.len()));
    }
    let (Some(head), Some(body)) = (bytes.get(..HEADER), bytes.get(HEADER..)) else {
        return Err(ProtoError::Malformed);
    };
    let version = u16::from_le_bytes([head[0], head[1]]);
    if version != PROTOCOL_VERSION {
        return Err(ProtoError::BadVersion(version));
    }
    match postcard::take_from_bytes::<M>(body) {
        Ok((msg, [])) => msg.validate(),
        _ => Err(ProtoError::Malformed),
    }
}

// ---- display names ----

/// Trimmed, 2-16 characters of ASCII letters, digits, space, `-` or `_`; anything else
/// becomes `Intern-<n>` (the server passes a per-room counter).
pub fn sanitize_name(raw: &str, n: u32) -> String {
    let name = raw.trim();
    let ok = (2..=MAX_NAME_CHARS).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'-' | b'_'));
    if ok {
        name.to_string()
    } else {
        format!("Intern-{n}")
    }
}

// ---- room titles ----

/// Only what `sanitize_title` and `sanitize_name` emit: ASCII letters, digits, space, `-`, `_`.
fn clean(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'-' | b'_'))
}

fn title_ok(t: &str) -> bool {
    t.len() <= MAX_TITLE_CHARS && clean(t)
}

/// Meetings a generated title is named after; `default_title` adds a number.
pub const TITLE_TEMPLATES: [&str; 8] = [
    "Daily Standup",
    "Sprint Planning",
    "Retro",
    "Backlog Grooming",
    "All Hands",
    "Quick Sync",
    "Incident Bridge",
    "Arch Review",
];

/// "Daily Standup 42", picked by `n` (the server passes the room's random id).
pub fn default_title(n: u64) -> String {
    let t = TITLE_TEMPLATES[(n % TITLE_TEMPLATES.len() as u64) as usize];
    format!("{t} {}", (n >> 8) % 100 + 1)
}

/// Keeps ASCII letters, digits, `-`, `_` and single spaces, at most `MAX_TITLE_CHARS`.
/// Everything else (control, bidi, other Unicode, markup) is dropped. `None` when fewer than
/// three characters survive: the caller uses `default_title`.
pub fn sanitize_title(raw: &str) -> Option<String> {
    let mut out = String::new();
    for c in raw.chars() {
        if out.len() >= MAX_TITLE_CHARS {
            break;
        }
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else if c == ' ' && !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    }
    let out = out.trim_end().to_string();
    (out.len() >= 3).then_some(out)
}

// ---- room codes ----

/// Flavour halves of a room code (`FIRST-SECOND-XXXXXXXX`). Neither list has a word that
/// is a prefix of another concatenation, so a code typed without dashes still parses
/// unambiguously. The words carry 8 bits; the secret is the suffix.
pub const ROOM_FIRST_WORDS: [&str; 16] = [
    "MERGE",
    "SCOPE",
    "TECH",
    "LEGACY",
    "SPRINT",
    "STANDUP",
    "SYNERGY",
    "PIVOT",
    "BLOCKER",
    "DEADLINE",
    "RUBBER",
    "YAML",
    "NULL",
    "CACHE",
    "BURNDOWN",
    "QUARTERLY",
];
pub const ROOM_SECOND_WORDS: [&str; 16] = [
    "CONFLICT", "CREEP", "DEBT", "CODE", "ZERO", "OVERRUN", "PARADIGM", "TABLE", "BUG", "PANIC",
    "DUCK", "HELL", "POINTER", "MISS", "CHART", "REVIEW",
];
/// Crockford base32: no I, L, O or U, so codes survive being read aloud.
pub const ROOM_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Suffix length: 8 symbols of 5 bits, 40 bits of entropy that cannot be guessed online.
pub const ROOM_SUFFIX_LEN: usize = 8;
/// Longest input `parse_room_code` looks at.
pub const MAX_ROOM_CODE_BYTES: usize = 64;

/// A code like `MERGE-CONFLICT-7K3M9QZA` from caller-supplied randomness: the server passes
/// 10 bytes from the OS rng (`getrandom`), never a seeded generator. Bytes 0-1 pick the
/// words, bytes 2-9 the suffix (256 is a multiple of 32, so `% 32` is unbiased).
pub fn room_code_from_bytes(bytes: &[u8; 10]) -> String {
    let first = ROOM_FIRST_WORDS[usize::from(bytes[0]) % ROOM_FIRST_WORDS.len()];
    let second = ROOM_SECOND_WORDS[usize::from(bytes[1]) % ROOM_SECOND_WORDS.len()];
    let suffix: String = bytes[2..]
        .iter()
        .map(|b| char::from(ROOM_ALPHABET[usize::from(*b) % ROOM_ALPHABET.len()]))
        .collect();
    format!("{first}-{second}-{suffix}")
}

/// Case-insensitive; dashes and spaces anywhere are optional; Crockford look-alikes
/// (`O` -> `0`, `I`/`L` -> `1`) are folded. Returns the canonical `FIRST-SECOND-XXXXXXXX`.
pub fn parse_room_code(s: &str) -> Result<String, ProtoError> {
    if s.len() > MAX_ROOM_CODE_BYTES {
        return Err(ProtoError::BadRoomCode);
    }
    let compact: String = s
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '\t'))
        .map(|c| c.to_ascii_uppercase())
        .collect();
    for first in ROOM_FIRST_WORDS {
        let Some(rest) = compact.strip_prefix(first) else {
            continue;
        };
        for second in ROOM_SECOND_WORDS {
            let Some(tail) = rest.strip_prefix(second) else {
                continue;
            };
            let suffix: Option<String> = tail
                .chars()
                .map(|c| match c {
                    'O' => Some('0'),
                    'I' | 'L' => Some('1'),
                    c if c.is_ascii() && ROOM_ALPHABET.contains(&(c as u8)) => Some(c),
                    _ => None,
                })
                .collect();
            if let Some(suffix) = suffix.filter(|x| x.len() == ROOM_SUFFIX_LEN) {
                return Ok(format!("{first}-{second}-{suffix}"));
            }
        }
    }
    Err(ProtoError::BadRoomCode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat_core::{AuraEnd, CastError, School, SpellId};

    fn client_msgs() -> Vec<ClientMsg> {
        let mut v = vec![
            ClientMsg::Hello {
                version: PROTOCOL_VERSION,
                name: "Dana".into(),
                class: "backend".into(),
            },
            ClientMsg::CreateRoom {
                public: true,
                title: "Mob Programming".into(),
            },
            ClientMsg::CreateRoom {
                public: false,
                title: String::new(),
            },
            ClientMsg::ListRooms,
            ClientMsg::JoinPublic { id: u64::MAX },
            ClientMsg::SetPublic { public: true },
            ClientMsg::JoinRoom {
                code: "SCOPE-CREEP-7K3M9QZA".into(),
            },
            ClientMsg::LeaveRoom,
            ClientMsg::Move {
                pos: Vec3::new(1.5, 0.0, -3.25),
                facing: 1.0,
                moving: true,
                jumping: false,
                seq: 9,
            },
            ClientMsg::Ping { t: 123_456 },
        ];
        v.extend(Emote::ALL.into_iter().map(|emote| ClientMsg::Emote {
            emote,
            target: None,
        }));
        v.push(ClientMsg::Emote {
            emote: Emote::Sync,
            target: Some(7),
        });
        v.extend(
            [
                Command::SetTarget {
                    unit: 1,
                    target: Some(7),
                },
                Command::SetTarget {
                    unit: 1,
                    target: None,
                },
                Command::Cast {
                    caster: 1,
                    spell: SpellId(2),
                },
                Command::StopCasting { caster: 1 },
                Command::Loot {
                    looter: 1,
                    corpse: 3,
                },
                Command::StartAutoAttack { unit: 1 },
                Command::StopAutoAttack { unit: 1 },
                Command::RespawnPlayer,
                Command::Talk { unit: 4 },
                Command::AcceptQuest {
                    quest: "q_intro".into(),
                },
                Command::TurnInQuest {
                    quest: "q_intro".into(),
                    choice: 2,
                },
                Command::AbandonQuest {
                    quest: "q_intro".into(),
                },
                Command::Equip {
                    item: "The Monorepo".into(),
                },
                Command::Unequip { slot: 4 },
                Command::Buy {
                    vendor: 12,
                    item: "Ibuprofen".into(),
                },
                Command::Sell {
                    vendor: 12,
                    item: "Expired Free Pizza Coupon".into(),
                },
                Command::SellJunk { vendor: 12 },
                Command::Discard { slot: 3 },
                Command::DiscardJunk,
                Command::Buyback {
                    vendor: 12,
                    item: "Expired Free Pizza Coupon".into(),
                },
                Command::UseItem {
                    item: "Cold Brew".into(),
                },
                Command::Interact {
                    prop: "printer_pmo".into(),
                },
                Command::SetTitle {
                    achievement: "pc_load_letter".into(),
                },
                Command::PickTalent { row: 4, choice: 3 },
                Command::ResetTalents,
            ]
            .into_iter()
            .map(ClientMsg::Cmd),
        );
        v
    }

    fn server_msgs() -> Vec<ServerMsg> {
        let p = PlayerInfo {
            id: 1,
            name: "Dana".into(),
            class: "devops".into(),
        };
        let unit = UnitSnap {
            id: 5,
            key_idx: 3,
            level: 4,
            pos_cm: [150, 0, -32_500],
            facing: 200,
            hp: 900,
            max_hp: 1440,
            target: Some(1),
            flags: UnitSnap::ALIVE | UnitSnap::IN_COMBAT,
            auras: vec![AuraSnap {
                key_idx: 2,
                remaining_ms: 4_500,
                total_ms: 8_000,
                stacks: 2,
                absorb: Some(30),
            }],
        };
        vec![
            ServerMsg::Welcome {
                you: 1,
                room: "MERGE-CONFLICT-42".into(),
                zone: Some("campus".into()),
                seed: 42,
                players: vec![p.clone()],
                quests: QuestSnap {
                    active: vec![("q_intro".into(), vec![1, 0])],
                    completed: vec!["q_first".into()],
                },
                cleared: vec!["tower/3".into()],
                unlocked: vec!["datacenter/1".into()],
            },
            ServerMsg::Positions {
                tick: 101,
                units: vec![(5, [150, 0, -32_400], 201), (1, [-20, 35, 0], 0)],
            },
            ServerMsg::Auras {
                units: vec![(
                    5,
                    vec![AuraSnap {
                        key_idx: 0,
                        remaining_ms: 900,
                        total_ms: 1_000,
                        stacks: 1,
                        absorb: None,
                    }],
                )],
            },
            ServerMsg::Cooldowns {
                tick: 102,
                gcd: (1_200, 1_500),
                spells: vec![(SpellId(3), 5_000, 8_000)],
            },
            ServerMsg::RoomCreated {
                code: "MERGE-CONFLICT-42".into(),
            },
            ServerMsg::Error {
                kind: ErrorKind::CannotJoin,
                text: "Room is full.".into(),
            },
            ServerMsg::PlayerJoined(p),
            ServerMsg::PlayerLeft { id: 2 },
            ServerMsg::Events {
                tick: 99,
                events: vec![
                    Event::Resurrected {
                        unit: 2,
                        by: 1,
                        hp: 90,
                        mana: 40,
                    },
                    Event::CastFailed {
                        caster: 1,
                        spell: SpellId(13),
                        reason: CastError::InCombat,
                    },
                    Event::Damage {
                        source: 1,
                        target: 5,
                        spell: Some(SpellId(1)),
                        amount: 80,
                        absorbed: 0,
                        crit: true,
                        combo: false,
                        school: School::Arcane,
                    },
                    Event::CastFailed {
                        caster: 1,
                        spell: SpellId(3),
                        reason: CastError::OutOfRange,
                    },
                    Event::AuraRemoved {
                        target: 5,
                        key: "code_smell".into(),
                        reason: AuraEnd::Expired,
                    },
                    Event::AiState {
                        unit: 5,
                        state: AiState::Evade,
                    },
                    Event::Knockback {
                        unit: 1,
                        dir: Vec3::new(0.0, 0.0, -1.0),
                        strength: 4.0,
                    },
                    Event::QuestOffered {
                        npc: 4,
                        quests: vec!["q_intro".into()],
                    },
                    Event::QuestFailed {
                        quest: "coffee_for_the_cto".into(),
                    },
                    Event::TelegraphStarted {
                        id: 7,
                        source: 5,
                        spell: SpellId(635),
                        shape: TelegraphShape::Circle { radius_m: 45.0 },
                        pos: Vec3::new(1.0, 0.0, -3.0),
                        facing: 0.5,
                        resolve_ms: 5000,
                        cover: vec![Vec3::new(-6.0, 0.0, 2.0), Vec3::new(6.0, 0.0, 2.0)],
                    },
                    Event::TelegraphStarted {
                        id: 8,
                        source: 5,
                        spell: SpellId(630),
                        shape: TelegraphShape::Line {
                            length_m: 34.0,
                            width_m: 3.5,
                        },
                        pos: Vec3::new(0.0, 0.0, 0.0),
                        facing: -1.2,
                        resolve_ms: 1800,
                        cover: vec![],
                    },
                    Event::TelegraphStarted {
                        id: 9,
                        source: 6,
                        spell: SpellId(608),
                        shape: TelegraphShape::Cone {
                            radius_m: 8.0,
                            angle_deg: 80.0,
                        },
                        pos: Vec3::new(2.0, 0.0, 2.0),
                        facing: 3.0,
                        resolve_ms: 1800,
                        cover: vec![],
                    },
                    Event::TelegraphResolved {
                        id: 7,
                        landed: true,
                    },
                    Event::TelegraphResolved {
                        id: 8,
                        landed: false,
                    },
                    Event::LootDropped {
                        corpse: 9,
                        items: vec![
                            "Prompt Fragment".into(),
                            "Mechanical Keyboard of Clacking".into(),
                        ],
                        sp: 14,
                    },
                    Event::Looted {
                        looter: 1,
                        corpse: 9,
                        items: vec!["Mechanical Keyboard of Clacking".into()],
                        sp: 14,
                    },
                    Event::Bag {
                        unit: 1,
                        sp: 120,
                        bag: vec!["Ibuprofen".into(), "Cold Brew".into()],
                        equipped: vec![
                            "".into(),
                            "".into(),
                            "Mechanical Keyboard of Clacking".into(),
                            "".into(),
                            "".into(),
                        ],
                        buyback: vec!["Expired Free Pizza Coupon".into()],
                    },
                    Event::GearChanged {
                        unit: 1,
                        equipped: vec![
                            "".into(),
                            "".into(),
                            "Mechanical Keyboard of Clacking".into(),
                            "".into(),
                            "".into(),
                        ],
                    },
                    Event::Proc {
                        unit: 1,
                        item: "Mechanical Keyboard of Clacking".into(),
                    },
                    Event::ItemError {
                        reason: combat_core::ItemError::NotEnoughSp,
                    },
                    Event::ItemCooldown {
                        item: "Escalation Hotline Badge".into(),
                        ms: 60_000,
                    },
                    Event::TalentsChanged {
                        unit: 1,
                        talents: [2, 2, 1, 0, 0],
                    },
                    Event::ItemError {
                        reason: combat_core::ItemError::InCombat,
                    },
                ],
            },
            ServerMsg::Snapshot {
                tick: 100,
                units: vec![unit],
                players: vec![PlayerSnap {
                    id: 1,
                    class: "frontend".into(),
                    mana: 50,
                    max_mana: 200,
                    xp: 10,
                    xp_to_next: 400,
                    gear: vec![3, PlayerSnap::NO_GEAR, 0, PlayerSnap::NO_GEAR, 7],
                    title: "pc_load_letter".into(),
                    talents: [1, 3, 0, 2, 0],
                    pet: "rubber_duck".into(),
                    burnout: 720,
                }],
            },
            ServerMsg::MoveCorrection {
                seq: 9,
                pos: Vec3::new(1.0, 0.0, 2.0),
            },
            ServerMsg::Emoted {
                player: 2,
                emote: Emote::Lgtm,
                target: None,
            },
            ServerMsg::Emoted {
                player: 2,
                emote: Emote::Sync,
                target: Some(3),
            },
            ServerMsg::Pong { t: 123_456 },
            ServerMsg::TickStat { p99_us: 1234 },
            ServerMsg::Events {
                tick: 9,
                events: sprint_events(),
            },
            ServerMsg::RoomList {
                rooms: vec![room_info(), room_info()],
            },
            ServerMsg::RoomList { rooms: Vec::new() },
            ServerMsg::RoomSettings {
                public: true,
                title: "Daily Standup 42".into(),
                leader: 1,
            },
        ]
    }

    fn room_info() -> RoomInfo {
        RoomInfo {
            id: 0xdead_beef,
            title: "Daily Standup 42".into(),
            host: "Dana".into(),
            players: 2,
            max: 5,
            zone: "tower/3".into(),
            avg_level: 7,
            roles: vec!["backend".into(), "devops".into()],
            in_sprint: true,
            age_s: 90,
        }
    }

    #[test]
    fn room_lists_and_titles_are_capped() {
        let list = |rooms: Vec<RoomInfo>| roundtrip(&ServerMsg::RoomList { rooms });
        assert!(list(vec![room_info(); MAX_ROOM_LIST]).is_ok());
        assert_eq!(
            list(vec![room_info(); MAX_ROOM_LIST + 1]),
            Err(ProtoError::TooMany)
        );
        let bad = [
            RoomInfo {
                title: "x".repeat(MAX_TITLE_CHARS + 1),
                ..room_info()
            },
            RoomInfo {
                title: "evil\u{202e}gnp".into(),
                ..room_info()
            },
            RoomInfo {
                host: "<b>".into(),
                ..room_info()
            },
            RoomInfo {
                host: "h".repeat(MAX_NAME_CHARS + 1),
                ..room_info()
            },
            RoomInfo {
                roles: vec!["devops".into(); MAX_PLAYERS + 1],
                ..room_info()
            },
            RoomInfo {
                zone: "z".repeat(MAX_QUEST_ID_BYTES + 1),
                ..room_info()
            },
        ];
        for r in bad {
            assert_eq!(list(vec![r]), Err(ProtoError::TooMany));
        }
        let settings = ServerMsg::RoomSettings {
            public: false,
            title: "a\nb".into(),
            leader: 1,
        };
        assert_eq!(roundtrip(&settings), Err(ProtoError::TooMany));
        let long = ClientMsg::CreateRoom {
            public: true,
            title: "t".repeat(MAX_RAW_TITLE_BYTES + 1),
        };
        assert_eq!(roundtrip(&long), Err(ProtoError::TooMany));
    }

    #[test]
    fn titles_are_sanitised_or_generated() {
        assert_eq!(
            sanitize_title("  Mob   Programming! ").as_deref(),
            Some("Mob Programming")
        );
        assert_eq!(
            sanitize_title("abc\u{202e}def\u{0007}<script>").as_deref(),
            Some("abcdefscript")
        );
        assert_eq!(sanitize_title("Zo\u{eb} ok").as_deref(), Some("Zo ok"));
        assert_eq!(
            sanitize_title(&"w".repeat(100)).map(|t| t.len()),
            Some(MAX_TITLE_CHARS)
        );
        assert_eq!(sanitize_title("a        b").as_deref(), Some("a b"));
        for none in ["", "  ", "ab", "!!!", "\u{202e}\u{202e}\u{202e}"] {
            assert_eq!(sanitize_title(none), None, "{none:?}");
        }
        for n in [0, 1, 7, 255, 1 << 20, u64::MAX] {
            let t = default_title(n);
            assert!(title_ok(&t) && t.len() >= 3, "{t}");
            assert_eq!(sanitize_title(&t).as_deref(), Some(t.as_str()));
        }
    }

    fn sprint_events() -> Vec<Event> {
        let status = combat_core::SprintStatus {
            building: "tower".into(),
            level: 7,
            affixes: vec!["reorg".into(), "burnout".into(), "mandatory_fun".into()],
            par_ms: 1_200_000,
            elapsed_ms: 61_000,
            penalty_ms: 5_000,
            forces_pct: 63,
            bosses_left: 2,
            bosses_total: 3,
            stage: 1,
            stages: 3,
            deaths: 1,
            tech_debt: 0,
        };
        vec![
            Event::SprintStarted {
                status: status.clone(),
                zones: vec!["tower/1".into(), "tower/3".into(), "tower/4".into()],
            },
            Event::SprintProgress {
                status: status.clone(),
            },
            Event::SprintAffix {
                affix: "reorg".into(),
                unit: 0,
                text: "Reorg!".into(),
            },
            Event::SprintRefused {
                reason: combat_core::SprintError::NotLeader,
            },
            Event::SprintEnded {
                result: combat_core::SprintResult::Delivered,
                time_ms: 700_000,
                upgrade: 2,
                summary: combat_core::SprintSummary {
                    status,
                    meter: vec![combat_core::MeterLine {
                        unit: 1,
                        class: "devops".into(),
                        damage: 123_456,
                        healing: 7_890,
                    }],
                    rewards: vec![combat_core::RewardLine {
                        unit: 1,
                        items: vec!["Hoodie".into()],
                        sp: 280,
                    }],
                    next_level: 9,
                },
            },
        ]
    }

    #[test]
    fn sprint_events_are_capped() {
        let mut ev = sprint_events();
        if let Event::SprintProgress { status } = &mut ev[1] {
            status.building = "x".repeat(MAX_ITEM_NAME_BYTES + 1);
        }
        let msg = ServerMsg::Events {
            tick: 1,
            events: vec![ev[1].clone()],
        };
        assert_eq!(roundtrip(&msg), Err(ProtoError::TooMany));
        let start = ClientMsg::Cmd(Command::StartSprint {
            building: "lab".into(),
            level: 3,
        });
        assert_eq!(roundtrip(&start), Ok(start.clone()));
        let long = ClientMsg::Cmd(Command::StartSprint {
            building: "x".repeat(MAX_QUEST_ID_BYTES + 1),
            level: 3,
        });
        assert_eq!(roundtrip(&long), Err(ProtoError::IllegalCommand));
    }

    fn live_events() -> Vec<Event> {
        use combat_core::{BoardView, GoalSpec, GoalView, TaskState, TaskView, WorldBossState};
        let goals = vec![
            GoalView {
                goal: GoalSpec::Kill {
                    zone: Some("tower/2".into()),
                    unit: Some("scrum_master".into()),
                },
                count: 3,
                need: 8,
            },
            GoalView {
                goal: GoalSpec::Idle {
                    zone: "tower/2".into(),
                },
                count: 12,
                need: 60,
            },
            GoalView {
                goal: GoalSpec::Sprint { level: 2 },
                count: 0,
                need: 1,
            },
        ];
        vec![
            Event::Clock {
                unix_s: 1_792_065_600,
            },
            Event::DailyBoard {
                board: BoardView {
                    day: 20_741,
                    week: 2_963,
                    tasks: vec![TaskView {
                        key: "do_nothing".into(),
                        weekly: false,
                        state: TaskState::Active,
                        goals,
                        xp: 900,
                        sp: 30,
                        level: 5,
                    }],
                },
                open: true,
            },
            Event::TaskCompleted {
                key: "do_nothing".into(),
                weekly: false,
                xp: 900,
                sp: 30,
            },
            Event::WorldBoss {
                state: WorldBossState::Warning,
                secs: 300,
                unit: 0,
            },
        ]
    }

    #[test]
    fn endgame_events_cross_the_wire_and_are_checked() {
        use combat_core::{BurnoutCause, QuipKind};
        let msg = ServerMsg::Events {
            tick: 12,
            events: vec![
                Event::RankProgress {
                    unit: 1,
                    rank: 2,
                    xp: 1_200,
                    gained: 300,
                },
                Event::RankUp { unit: 1, rank: 3 },
                Event::Reputation {
                    unit: 1,
                    dept: 5,
                    total: 8_400,
                    gained: 75,
                },
                Event::Burnout {
                    unit: 2,
                    value: 650,
                    delta: -50,
                    cause: BurnoutCause::Kill,
                },
                Event::BurnoutQuip {
                    unit: 2,
                    kind: QuipKind::ReplyAll,
                    line: 4,
                },
                Event::ItemError {
                    reason: combat_core::ItemError::RepTooLow,
                },
            ],
        };
        assert_eq!(roundtrip(&msg), Ok(msg.clone()));
        let bad_dept = ServerMsg::Events {
            tick: 12,
            events: vec![Event::Reputation {
                unit: 1,
                dept: combat_core::DEPARTMENTS as u8,
                total: 1,
                gained: 1,
            }],
        };
        assert_eq!(roundtrip(&bad_dept), Err(ProtoError::BadNumber));
        let bad_burnout = ServerMsg::Events {
            tick: 12,
            events: vec![Event::Burnout {
                unit: 1,
                value: combat_core::BURNOUT_MAX + 1,
                delta: 1,
                cause: BurnoutCause::Debug,
            }],
        };
        assert_eq!(roundtrip(&bad_burnout), Err(ProtoError::BadNumber));
    }

    #[test]
    fn calendar_events_cross_the_wire() {
        let msg = ServerMsg::Events {
            tick: 30,
            events: vec![
                Event::TicketsClosed {
                    unit: 4,
                    count: 3,
                    sp: 18,
                },
                Event::Burnout {
                    unit: 4,
                    value: 380,
                    delta: 80,
                    cause: combat_core::BurnoutCause::Quest,
                },
            ],
        };
        assert_eq!(roundtrip(&msg), Ok(msg.clone()));
        // The calendar starts a chain with the plain quest accept.
        let c = ClientMsg::Cmd(Command::AcceptQuest {
            quest: "er_receipt".into(),
        });
        assert_eq!(roundtrip(&c), Ok(c.clone()));
    }

    #[test]
    fn the_raid_and_the_weekly_ladder_cross_the_wire_and_are_capped() {
        use combat_core::{LadderEntry, LadderView, RaidView};
        let raid = RaidView {
            zone: "offsite/2".into(),
            async_mode: true,
            week: 2_960,
            locked: vec!["async:icebreaker_facilitator".into()],
            tethers: vec![(4, 7), (9, 12)],
            tether_m: 8,
        };
        let ladder = LadderView {
            week: 2_960,
            entries: vec![LadderEntry {
                time_ms: 754_000,
                deaths: 2,
                crew: vec!["Ada (Fullstack Engineer)".into(), "Backend Engineer".into()],
            }],
        };
        let msg = ServerMsg::Events {
            tick: 41,
            events: vec![
                Event::Raid { view: raid.clone() },
                Event::Ladder {
                    view: ladder.clone(),
                },
            ],
        };
        assert_eq!(roundtrip(&msg), Ok(msg.clone()));
        let cmd = ClientMsg::Cmd(Command::StartWeeklySprint);
        assert_eq!(roundtrip(&cmd), Ok(cmd.clone()));
        let mut many_ties = raid.clone();
        many_ties.tethers = vec![(1, 2); MAX_LIST_LEN + 1];
        let mut long_lock = raid;
        long_lock.locked = vec!["x".repeat(MAX_ITEM_NAME_BYTES + 1)];
        let mut crowd = ladder.clone();
        crowd.entries = vec![LadderEntry::default(); MAX_LIST_LEN + 1];
        let mut long_name = ladder;
        long_name.entries[0].crew = vec!["y".repeat(MAX_ITEM_NAME_BYTES + 1)];
        for e in [
            Event::Raid { view: many_ties },
            Event::Raid { view: long_lock },
            Event::Ladder { view: crowd },
            Event::Ladder { view: long_name },
        ] {
            let bad = ServerMsg::Events {
                tick: 41,
                events: vec![e],
            };
            assert_eq!(roundtrip(&bad), Err(ProtoError::TooMany));
        }
    }

    #[test]
    fn minigames_cross_the_wire_and_are_capped() {
        use combat_core::{Card, Gauge, Medal, MinigameResult, MinigameView, WatcherKind};
        let view = MinigameView {
            key: "teambuilding".into(),
            elapsed_ms: 61_000,
            limit_ms: 900_000,
            objectives: 0b10101,
            dwell: Some((4, 55)),
            collected: 3,
            collectibles: 10,
            taken: 0b1011,
            catches: 1,
            suspicion: 45,
            hidden: true,
            noisy: false,
            score: 2010,
            ..Default::default()
        };
        let corporate = MinigameView {
            key: "performance_review".into(),
            elapsed_ms: 30_000,
            limit_ms: 420_000,
            lines: vec!["Question 2/9".into(), "Calendar burned: 8 min".into()],
            gauges: vec![Gauge {
                label: "Meets Expectations: 3.4 of 5".into(),
                value: 60,
                mark: Some(50),
            }],
            card: Some(Card {
                seq: 7,
                who: "Your Manager".into(),
                text: "Where do you see yourself in five years?".into(),
                options: vec!["Here.".into(), "Owning the vision.".into()],
            }),
            badges: vec![(12, "Sam, VP of Vibes".into())],
            layout: 0b11_0000_0000_0000_0000_1011,
            score: 80,
            ..Default::default()
        };
        let result = MinigameResult {
            unit: 3,
            key: "teambuilding".into(),
            medal: Some(Medal::Silver),
            score: 1450,
            time_ms: 312_000,
            catches: 1,
            collected: 6,
            quit: false,
            group_score: 3100,
            items: vec!["Party Leftovers Container (Sealed)".into()],
            summary: String::new(),
        };
        let msg = ServerMsg::Events {
            tick: 9,
            events: vec![
                Event::MinigameStarted {
                    key: "teambuilding".into(),
                    limit_ms: 900_000,
                },
                Event::Minigame {
                    unit: 3,
                    view: view.clone(),
                },
                Event::MinigameCaught {
                    unit: 3,
                    by: 12,
                    kind: WatcherKind::Talker,
                    line: "Prompting is a love language.".into(),
                },
                Event::Minigame {
                    unit: 3,
                    view: corporate.clone(),
                },
                Event::MinigameNote {
                    unit: 3,
                    text: "Validation error: that value does not match the ticket.".into(),
                    good: false,
                },
                Event::MinigameEnded {
                    result: result.clone(),
                },
            ],
        };
        assert_eq!(roundtrip(&msg), Ok(msg.clone()));
        for cmd in [
            Command::StartMinigame {
                key: "teambuilding".into(),
            },
            Command::LeaveMinigame,
            Command::MinigamePick { seq: 7, option: 3 },
        ] {
            let c = ClientMsg::Cmd(cmd);
            assert_eq!(roundtrip(&c), Ok(c.clone()));
        }
        let long_key = ClientMsg::Cmd(Command::StartMinigame {
            key: "k".repeat(combat_core::minigame::MINIGAME_KEY_MAX + 1),
        });
        assert_eq!(roundtrip(&long_key), Err(ProtoError::IllegalCommand));
        let mut long_view = view;
        long_view.key = "k".repeat(combat_core::minigame::MINIGAME_KEY_MAX + 1);
        let mut many_items = result.clone();
        many_items.items = vec!["x".into(); MAX_LIST_LEN + 1];
        let mut long_summary = result;
        long_summary.summary = "s".repeat(MAX_TEXT_BYTES + 1);
        let mut many_options = corporate.clone();
        if let Some(c) = many_options.card.as_mut() {
            c.options = vec!["o".into(); combat_core::minigame::CARD_OPTIONS_MAX + 1];
        }
        let mut long_card = corporate.clone();
        if let Some(c) = long_card.card.as_mut() {
            c.text = "t".repeat(MAX_TEXT_BYTES + 1);
        }
        let mut many_lines = corporate;
        many_lines.lines = vec!["l".into(); combat_core::minigame::VIEW_LINES_MAX + 1];
        for e in [
            Event::Minigame {
                unit: 3,
                view: long_view,
            },
            Event::MinigameCaught {
                unit: 3,
                by: 12,
                kind: WatcherKind::Ceo,
                line: "z".repeat(MAX_TEXT_BYTES + 1),
            },
            Event::MinigameEnded { result: many_items },
            Event::MinigameEnded {
                result: long_summary,
            },
            Event::Minigame {
                unit: 3,
                view: many_options,
            },
            Event::Minigame {
                unit: 3,
                view: long_card,
            },
            Event::Minigame {
                unit: 3,
                view: many_lines,
            },
            Event::MinigameNote {
                unit: 3,
                text: "n".repeat(MAX_TEXT_BYTES + 1),
                good: true,
            },
        ] {
            let bad = ServerMsg::Events {
                tick: 9,
                events: vec![e],
            };
            assert_eq!(roundtrip(&bad), Err(ProtoError::TooMany));
        }
    }

    #[test]
    fn legacy_mode_crosses_the_wire_and_is_capped() {
        use combat_core::{LegacyError, LegacyView};
        let view = LegacyView {
            on: true,
            zone: "tower/3".into(),
            week: 2_960,
            locked: vec!["vibe_coder".into(), "customer".into()],
        };
        let msg = ServerMsg::Events {
            tick: 40,
            events: vec![
                Event::Legacy { view: view.clone() },
                Event::LegacyRefused {
                    reason: LegacyError::LevelTooLow,
                },
            ],
        };
        assert_eq!(roundtrip(&msg), Ok(msg.clone()));
        for on in [true, false] {
            let cmd = ClientMsg::Cmd(Command::SetLegacy { on });
            assert_eq!(roundtrip(&cmd), Ok(cmd.clone()));
        }
        let mut long = view.clone();
        long.zone = "x".repeat(MAX_ITEM_NAME_BYTES + 1);
        let mut many = view;
        many.locked = vec!["boss".into(); MAX_LIST_LEN + 1];
        for v in [long, many] {
            let bad = ServerMsg::Events {
                tick: 40,
                events: vec![Event::Legacy { view: v }],
            };
            assert_eq!(roundtrip(&bad), Err(ProtoError::TooMany));
        }
    }

    #[test]
    fn live_world_crosses_the_wire_and_is_capped() {
        let msg = ServerMsg::Events {
            tick: 9,
            events: live_events(),
        };
        assert_eq!(roundtrip(&msg), Ok(msg.clone()));
        for cmd in [
            Command::JoinStandup,
            Command::ClaimSprintGoal { choice: 2 },
            Command::GoHome,
        ] {
            let m = ClientMsg::Cmd(cmd);
            assert_eq!(roundtrip(&m), Ok(m.clone()));
        }
        let mut big = live_events();
        if let Event::DailyBoard { board, .. } = &mut big[1] {
            board.tasks[0].goals[0].goal = combat_core::GoalSpec::Talk {
                unit: "x".repeat(MAX_ITEM_NAME_BYTES + 1),
            };
        }
        let msg = ServerMsg::Events {
            tick: 9,
            events: vec![big[1].clone()],
        };
        assert_eq!(roundtrip(&msg), Err(ProtoError::TooMany));
    }

    fn roundtrip<M: Msg + PartialEq + fmt::Debug>(m: &M) -> Result<M, ProtoError> {
        decode::<M>(&encode(m)?)
    }

    #[test]
    fn every_message_roundtrips() {
        for m in client_msgs() {
            assert_eq!(roundtrip(&m), Ok(m.clone()));
        }
        for m in server_msgs() {
            assert_eq!(roundtrip(&m), Ok(m.clone()));
        }
    }

    #[test]
    fn oversize_frames_are_rejected_both_ways() {
        let huge = ServerMsg::Error {
            kind: ErrorKind::BadMessage,
            text: "x".repeat(MAX_FRAME_BYTES),
        };
        assert!(matches!(encode(&huge), Err(ProtoError::TooLarge(_))));
        let mut bytes = PROTOCOL_VERSION.to_le_bytes().to_vec();
        bytes.resize(MAX_FRAME_BYTES + 1, 0);
        assert!(matches!(
            decode::<ClientMsg>(&bytes),
            Err(ProtoError::TooLarge(_))
        ));
    }

    #[test]
    fn wrong_version_truncation_and_trailing_bytes_are_rejected() {
        let Ok(mut bytes) = encode(&ClientMsg::Ping { t: 1 }) else {
            panic!("encode")
        };
        let mut old = bytes.clone();
        old[0] = old[0].wrapping_add(1);
        assert!(matches!(
            decode::<ClientMsg>(&old),
            Err(ProtoError::BadVersion(_))
        ));
        assert_eq!(decode::<ClientMsg>(&bytes[..1]), Err(ProtoError::Malformed));
        assert_eq!(decode::<ClientMsg>(&[]), Err(ProtoError::Malformed));
        bytes.push(0);
        assert_eq!(decode::<ClientMsg>(&bytes), Err(ProtoError::Malformed));
    }

    #[test]
    fn client_validation_blocks_illegal_input() {
        let moved = ClientMsg::Cmd(Command::PlayerMoved {
            pos: Vec3::default(),
            yaw: 0.0,
            moving: false,
            jumping: false,
        });
        assert_eq!(roundtrip(&moved), Err(ProtoError::IllegalCommand));
        let long_quest = ClientMsg::Cmd(Command::AcceptQuest {
            quest: "q".repeat(MAX_QUEST_ID_BYTES + 1),
        });
        assert_eq!(roundtrip(&long_quest), Err(ProtoError::IllegalCommand));
        let long_item = ClientMsg::Cmd(Command::Buy {
            vendor: 3,
            item: "i".repeat(MAX_ITEM_NAME_BYTES + 1),
        });
        assert_eq!(roundtrip(&long_item), Err(ProtoError::IllegalCommand));
        let bad_slot = ClientMsg::Cmd(Command::Unequip {
            slot: combat_core::SLOTS as u8,
        });
        assert_eq!(roundtrip(&bad_slot), Err(ProtoError::IllegalCommand));
        let bad_discard = ClientMsg::Cmd(Command::Discard {
            slot: MAX_LIST_LEN as u16,
        });
        assert_eq!(roundtrip(&bad_discard), Err(ProtoError::IllegalCommand));
        for (row, choice) in [
            (combat_core::TALENT_ROWS as u8, 1),
            (0, combat_core::TALENT_CHOICES + 1),
        ] {
            let bad = ClientMsg::Cmd(Command::PickTalent { row, choice });
            assert_eq!(roundtrip(&bad), Err(ProtoError::IllegalCommand));
        }
        let nan = ClientMsg::Move {
            pos: Vec3::new(f32::NAN, 0.0, 0.0),
            facing: 0.0,
            moving: false,
            jumping: false,
            seq: 1,
        };
        assert_eq!(roundtrip(&nan), Err(ProtoError::BadNumber));
        let bad_room = ClientMsg::JoinRoom {
            code: "DROP-TABLE-7K3M9QZA".into(),
        };
        assert_eq!(roundtrip(&bad_room), Err(ProtoError::BadRoomCode));
        let lower = ClientMsg::JoinRoom {
            code: " tech debt 7k3m9qza ".into(),
        };
        assert_eq!(
            roundtrip(&lower),
            Ok(ClientMsg::JoinRoom {
                code: "TECH-DEBT-7K3M9QZA".into()
            })
        );
        let long_name = ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            name: "n".repeat(MAX_RAW_NAME_BYTES + 1),
            class: "fullstack".into(),
        };
        assert_eq!(roundtrip(&long_name), Err(ProtoError::BadName));
        let long_class = ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            name: "Dana".into(),
            class: "c".repeat(MAX_CLASS_BYTES + 1),
        };
        assert_eq!(roundtrip(&long_class), Err(ProtoError::BadName));
    }

    #[test]
    fn random_bytes_never_panic() {
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as u32
        };
        let valid: Vec<Vec<u8>> = client_msgs()
            .iter()
            .filter_map(|m| encode(m).ok())
            .chain(server_msgs().iter().filter_map(|m| encode(m).ok()))
            .collect();
        for round in 0..20_000 {
            let mut bytes: Vec<u8> = match round % 3 {
                // Pure noise, mostly rejected by the version check.
                0 => (0..next() % 64).map(|_| next() as u8).collect(),
                // Valid header, noise body: exercises the postcard decoder.
                1 => {
                    let mut b = PROTOCOL_VERSION.to_le_bytes().to_vec();
                    b.extend((0..next() % 96).map(|_| next() as u8));
                    b
                }
                // A real frame with a few flipped bytes.
                _ => valid[next() as usize % valid.len()].clone(),
            };
            if round % 3 == 2 {
                for _ in 0..1 + next() % 3 {
                    let k = HEADER + next() as usize % (bytes.len() - HEADER).max(1);
                    if let Some(b) = bytes.get_mut(k) {
                        *b ^= next() as u8;
                    }
                }
            }
            let _ = decode::<ClientMsg>(&bytes);
            let _ = decode::<ServerMsg>(&bytes);
        }
    }

    #[test]
    fn names_are_sanitised() {
        assert_eq!(sanitize_name("  Dana_K-9 ", 1), "Dana_K-9");
        assert_eq!(sanitize_name("Al", 1), "Al");
        assert_eq!(sanitize_name("A", 3), "Intern-3");
        assert_eq!(sanitize_name("Seventeen chars!!", 4), "Intern-4");
        assert_eq!(sanitize_name("abcdefghijklmnopq", 5), "Intern-5");
        assert_eq!(sanitize_name("<script>", 6), "Intern-6");
        assert_eq!(sanitize_name("Zoë", 7), "Intern-7");
        assert_eq!(sanitize_name("   ", 8), "Intern-8");
    }

    #[test]
    fn room_codes_generate_from_bytes_and_parse_leniently() {
        let mut b = [0u8; 10];
        for k in 0..=255u8 {
            b.iter_mut()
                .enumerate()
                .for_each(|(i, x)| *x = k.wrapping_mul(31).wrapping_add(i as u8 * 7));
            let code = room_code_from_bytes(&b);
            assert_eq!(code.rsplit('-').next().map(str::len), Some(ROOM_SUFFIX_LEN));
            assert_eq!(parse_room_code(&code), Ok(code.clone()));
            assert_eq!(parse_room_code(&code.to_lowercase()), Ok(code.clone()));
            assert_eq!(parse_room_code(&code.replace('-', "")), Ok(code.clone()));
        }
        assert_eq!(
            room_code_from_bytes(&[0, 0, 0, 1, 2, 31, 32, 255, 10, 20]),
            "MERGE-CONFLICT-012Z0ZAM"
        );
        let canon = Ok("MERGE-CONFLICT-7K3M9QZA".to_string());
        for ok in [
            "merge-conflict-7k3m9qza",
            " MERGE CONFLICT 7K3M9QZA ",
            "MERGE-CONFLICT7K3M9QZA",
            "mergeconflict7k3m9qza",
        ] {
            assert_eq!(parse_room_code(ok), canon, "{ok}");
        }
        // Crockford look-alikes fold to digits.
        assert_eq!(
            parse_room_code("TECH-DEBT-OIL00000"),
            Ok("TECH-DEBT-01100000".into())
        );
        for bad in [
            "",
            "MERGE-CONFLICT",
            "MERGE-CONFLICT-7K3M9QZ",
            "MERGE-CONFLICT-7K3M9QZAB",
            "MERGE-CONFLICT-7K3M9QZU",
            "MERGE-CONFLICT-7K3M9QZ!",
            "MERGE-7K3M9QZA",
            "FOO-BAR-7K3M9QZA",
            "MERGE-CONFLICT-42",
        ] {
            assert_eq!(parse_room_code(bad), Err(ProtoError::BadRoomCode), "{bad}");
        }
        assert_eq!(
            parse_room_code(&"A".repeat(MAX_ROOM_CODE_BYTES + 1)),
            Err(ProtoError::BadRoomCode)
        );
    }

    #[test]
    fn room_words_concatenate_without_prefix_ambiguity() {
        let mut all = Vec::new();
        for f in ROOM_FIRST_WORDS {
            for s in ROOM_SECOND_WORDS {
                all.push(format!("{f}{s}"));
            }
        }
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                assert!(i == j || !b.starts_with(a.as_str()), "{a} / {b}");
            }
        }
        let letters = ROOM_FIRST_WORDS.iter().chain(&ROOM_SECOND_WORDS);
        assert!(
            letters
                .flat_map(|w| w.bytes())
                .all(|b| b.is_ascii_uppercase())
        );
    }

    #[test]
    fn server_validation_rejects_bad_floats_and_oversized_lists() {
        let knock = ServerMsg::Events {
            tick: 1,
            events: vec![Event::Knockback {
                unit: 1,
                dir: Vec3::new(f32::INFINITY, 0.0, 0.0),
                strength: 1.0,
            }],
        };
        assert_eq!(roundtrip(&knock), Err(ProtoError::BadNumber));
        let promo = ServerMsg::Events {
            tick: 1,
            events: vec![Event::TalentsChanged {
                unit: 1,
                talents: [1, 9, 0, 0, 0],
            }],
        };
        assert_eq!(roundtrip(&promo), Err(ProtoError::TooMany));
        let corr = ServerMsg::MoveCorrection {
            seq: 1,
            pos: Vec3::new(0.0, f32::NAN, 0.0),
        };
        assert_eq!(roundtrip(&corr), Err(ProtoError::BadNumber));
        let tele = |shape, cover: Vec<Vec3>| ServerMsg::Events {
            tick: 1,
            events: vec![Event::TelegraphStarted {
                id: 1,
                source: 2,
                spell: SpellId(600),
                shape,
                pos: Vec3::new(0.0, 0.0, 0.0),
                facing: 0.0,
                resolve_ms: 1000,
                cover,
            }],
        };
        let circle = TelegraphShape::Circle { radius_m: 3.0 };
        let nan_cone = TelegraphShape::Cone {
            radius_m: f32::NAN,
            angle_deg: 60.0,
        };
        assert_eq!(
            roundtrip(&tele(nan_cone, vec![])),
            Err(ProtoError::BadNumber)
        );
        let bad_cover = vec![Vec3::new(0.0, f32::INFINITY, 0.0)];
        assert_eq!(
            roundtrip(&tele(circle, bad_cover)),
            Err(ProtoError::BadNumber)
        );
        let crowd = vec![Vec3::new(0.0, 0.0, 0.0); MAX_LIST_LEN + 1];
        assert_eq!(roundtrip(&tele(circle, crowd)), Err(ProtoError::TooMany));
        let many = ServerMsg::Events {
            tick: 1,
            events: vec![Event::Despawned { unit: 1 }; MAX_EVENTS_PER_FRAME + 1],
        };
        assert_eq!(roundtrip(&many), Err(ProtoError::TooMany));
        let loot = ServerMsg::Events {
            tick: 1,
            events: vec![Event::LootDropped {
                corpse: 1,
                items: vec!["x".into(); MAX_LIST_LEN + 1],
                sp: 3,
            }],
        };
        assert_eq!(roundtrip(&loot), Err(ProtoError::TooMany));
        let long_name = ServerMsg::Events {
            tick: 1,
            events: vec![Event::GearChanged {
                unit: 1,
                equipped: vec!["x".repeat(MAX_ITEM_NAME_BYTES + 1)],
            }],
        };
        assert_eq!(roundtrip(&long_name), Err(ProtoError::TooMany));
        let big_bag = ServerMsg::Events {
            tick: 1,
            events: vec![Event::Bag {
                unit: 1,
                sp: 0,
                bag: vec!["junk".into(); MAX_LIST_LEN + 1],
                equipped: vec![],
                buyback: vec![],
            }],
        };
        assert_eq!(roundtrip(&big_bag), Err(ProtoError::TooMany));
        let welcome = |players: usize, cleared: usize| ServerMsg::Welcome {
            you: 1,
            room: "r".into(),
            zone: None,
            seed: 1,
            players: vec![
                PlayerInfo {
                    id: 1,
                    name: "a".into(),
                    class: "fullstack".into(),
                };
                players
            ],
            quests: QuestSnap::default(),
            cleared: vec!["z".into(); cleared],
            unlocked: Vec::new(),
        };
        assert!(roundtrip(&welcome(1, 1)).is_ok());
        assert_eq!(
            roundtrip(&welcome(MAX_PLAYERS + 1, 0)),
            Err(ProtoError::TooMany)
        );
        assert_eq!(
            roundtrip(&welcome(1, MAX_LIST_LEN + 1)),
            Err(ProtoError::TooMany)
        );
        let flood = ServerMsg::Positions {
            tick: 1,
            units: vec![(1, [0, 0, 0], 0); MAX_SNAPSHOT_UNITS + 1],
        };
        assert_eq!(roundtrip(&flood), Err(ProtoError::TooMany));
        assert_eq!(ErrorKind::CannotJoin.message(), "Cannot join that room.");
    }

    #[test]
    fn emotes_have_unique_commands_and_parse_back() {
        for e in Emote::ALL {
            assert_eq!(Emote::from_command(e.command()), Some(e));
            assert!(e.command().starts_with('/') && !e.text().is_empty());
        }
        assert_eq!(Emote::from_command(" /LGTM "), Some(Emote::Lgtm));
        assert_eq!(Emote::from_command("/rm -rf"), None);
        for e in Emote::ALL {
            let t = e.text_at("Alice");
            assert!(t.contains("Alice") && !t.contains("{n}"), "{t}");
        }
        assert_eq!(Emote::Sync.text_at("Alice"), "Let's sync on this, Alice.");
    }

    #[test]
    fn unit_snap_quantises_position_and_facing() {
        let Ok(content) = Content::from_ron(include_str!("../../../content/game.ron")) else {
            panic!("content")
        };
        let mut sim = combat_core::Sim::new(content, 1);
        let p = sim.spawn_player("fullstack", Vec3::new(12.345, 0.0, -630.004), 1.0);
        let Some(u) = sim.unit(p).cloned() else {
            panic!("no player")
        };
        let Some(s) = UnitSnap::from_state(&u, sim.content()) else {
            panic!("no snap")
        };
        assert_eq!(s.pos_cm, [1235, 0, -63000]);
        assert!((s.pos().x - 12.35).abs() < 1e-4);
        assert!((s.yaw() - 1.0).abs() < std::f32::consts::TAU / 256.0);
        assert!(s.has(UnitSnap::ALIVE) && !s.has(UnitSnap::IN_COMBAT));
        assert_eq!(
            sim.content().units[usize::from(s.key_idx)].key,
            sim.content().classes[0].unit
        );
        let ps = PlayerSnap::from_state(&u, sim.content());
        assert_eq!(ps.class, "fullstack");
        assert_eq!(quantize_yaw(-std::f32::consts::FRAC_PI_2), 192);
        assert_eq!(from_cm(pos_cm(u.pos)), s.pos());
        assert!((yaw_from_byte(s.facing) - s.yaw()).abs() < 1e-6);
    }

    #[test]
    fn player_gear_travels_as_content_indices() {
        let Ok(content) = Content::from_ron(include_str!("../../../content/game.ron")) else {
            panic!("content")
        };
        let mut sim = combat_core::Sim::new(content.clone(), 1);
        let p = sim.spawn_player("backend", Vec3::default(), 0.0);
        assert!(sim.debug_equip(p, "Incident Commander Headset"));
        assert!(sim.debug_equip(p, "The Monorepo"));
        let Some(u) = sim.unit(p) else {
            panic!("no player")
        };
        let snap = PlayerSnap::from_state(u, &content);
        assert_eq!(snap.gear.len(), combat_core::SLOTS);
        assert_eq!(snap.equipped(&content), u.equipped);
        assert_eq!(
            roundtrip(&ServerMsg::Snapshot {
                tick: 1,
                units: vec![],
                players: vec![snap.clone()]
            }),
            Ok(ServerMsg::Snapshot {
                tick: 1,
                units: vec![],
                players: vec![snap]
            })
        );
    }

    #[test]
    fn auras_quests_and_cooldowns_convert_through_content() {
        let Ok(content) = Content::from_ron(include_str!("../../../content/game.ron")) else {
            panic!("content")
        };
        let Some(def) = content.auras.get(1) else {
            panic!("no auras")
        };
        let view = AuraView {
            key: def.key.clone(),
            name: def.name.clone(),
            icon: def.icon.clone(),
            harmful: def.harmful,
            remaining_ms: 3_000,
            total_ms: 6_000,
            absorb_remaining: None,
            stacks: 1,
        };
        let Some(snap) = AuraSnap::from_view(&view, &content) else {
            panic!("aura not in content")
        };
        assert_eq!(snap.key_idx, 1);
        assert_eq!(snap.to_view(&content), Some(view));
        let log = QuestLog {
            active: vec![combat_core::ActiveQuest {
                id: "q".into(),
                counts: vec![2],
            }],
            completed: vec!["done".into()],
            kills: Default::default(),
        };
        assert_eq!(QuestSnap::from_log(&log).to_log(), log);
        let msg = cooldowns_msg(7, [SpellId(1), SpellId(2)], |s| CooldownView {
            gcd_remaining_ms: 900,
            gcd_total_ms: 1_500,
            cd_remaining_ms: if s == SpellId(2) { 4_000 } else { 0 },
            cd_total_ms: 6_000,
            usable: Ok(()),
        });
        assert_eq!(
            msg,
            ServerMsg::Cooldowns {
                tick: 7,
                gcd: (900, 1_500),
                spells: vec![(SpellId(2), 4_000, 6_000)],
            }
        );
    }

    #[test]
    fn pet_and_duel_commands_pass_and_long_pet_keys_do_not() {
        for cmd in [
            Command::SummonPet {
                pet: "roomba".into(),
            },
            Command::SummonPet { pet: String::new() },
            Command::DuelRequest { target: 2 },
            Command::DuelRespond { accept: false },
        ] {
            let bytes = encode(&ClientMsg::Cmd(cmd.clone())).unwrap_or_default();
            assert_eq!(decode::<ClientMsg>(&bytes), Ok(ClientMsg::Cmd(cmd)));
        }
        let long = ClientMsg::Cmd(Command::SummonPet {
            pet: "p".repeat(combat_core::world::WORLD_KEY_MAX + 1),
        });
        let bytes = encode(&long).unwrap_or_default();
        assert!(decode::<ClientMsg>(&bytes).is_err());
        let ev = ServerMsg::Events {
            tick: 1,
            events: vec![Event::DuelCountdown {
                a: 1,
                b: 2,
                center: Vec3::new(f32::NAN, 0.0, 0.0),
                radius_m: 20.0,
                ms: 3000,
            }],
        };
        let bytes = encode(&ev).unwrap_or_default();
        assert_eq!(decode::<ServerMsg>(&bytes), Err(ProtoError::BadNumber));
    }
}
