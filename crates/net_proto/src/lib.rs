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
pub const PROTOCOL_VERSION: u16 = 8;
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
    CreateRoom,
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
    /// Key of the achievement whose title the player wears ("" for none).
    pub title: String,
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
                    })
                    && units.iter().all(|u| u.auras.len() <= MAX_LIST_LEN)
            }
            ServerMsg::MoveCorrection { pos, .. } => {
                return finite(&[pos.x, pos.y, pos.z]).map(|_| self);
            }
            ServerMsg::PlayerLeft { .. } | ServerMsg::Emoted { .. } | ServerMsg::Pong { .. } => {
                true
            }
        };
        if ok {
            Ok(self)
        } else {
            Err(ProtoError::TooMany)
        }
    }
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
        Event::LootDropped { items, .. } | Event::Looted { items, .. } => names(items),
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
        | Event::TitleChanged { title: k, .. } => {
            if k.len() <= combat_core::world::WORLD_KEY_MAX {
                Ok(())
            } else {
                Err(ProtoError::TooMany)
            }
        }
        Event::Yell { text, .. } if text.len() > MAX_TEXT_BYTES => Err(ProtoError::TooMany),
        _ => Ok(()),
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
    let ok = (2..=16).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'-' | b'_'));
    if ok {
        name.to_string()
    } else {
        format!("Intern-{n}")
    }
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
            ClientMsg::CreateRoom,
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
        ]
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
}
