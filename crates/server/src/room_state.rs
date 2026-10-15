//! Authoritative game state of one room: a `combat_core::Sim` plus the per-player wire state.
//!
//! `rooms.rs` drives it through `new`, `on_join`, `on_leave`, `on_message` and `tick` (30 Hz)
//! and delivers the returned frames; nothing here touches sockets or awaits.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{BuildHasher, Hasher, RandomState};
use std::sync::Arc;

use axum::body::Bytes;
use combat_core::{
    Aabb, Audience as SimAudience, Command, EntityId, Event, Sim, Vec3, pack_offsets,
};
use net_proto::{
    AuraSnap, ClientMsg, ErrorKind, MAX_EVENTS_PER_FRAME, MAX_SNAPSHOT_UNITS, PlayerInfo,
    PlayerSnap, ProtoError, QuestSnap, ServerMsg, UnitSnap, cooldowns_msg, encode, pos_cm,
    quantize_yaw, sanitize_name,
};
use tokio::time::Instant;

use crate::world::{World, in_xz};

pub type PlayerId = u32;

/// Who receives an outbound frame. `All` means the room's players at delivery time, so
/// event-driven broadcasts are expanded to `Only` when queued: a player joining later in the
/// same tick must not receive them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    All,
    Only(PlayerId),
}

pub const START_ZONE: &str = "campus";
/// Client run speed (game_client `config::RUN_SPEED`).
pub const RUN_SPEED_MPS: f32 = 7.0;
/// Allowed speed over the run speed, for clock drift and frame-time spikes.
const SPEED_SLACK: f32 = 1.25;
/// Movement budget that is always available: network jitter and float noise.
const JITTER_M: f32 = 1.0;
/// At most this much movement may be buffered (a lag spike delivering a burst of moves).
const MAX_BURST_S: f32 = 1.0;
const SNAPSHOT_EVERY_TICKS: u64 = 30;
/// `Positions` cadence: 10 Hz, so remote units render about 0.2 s behind instead of 1.1 s.
const POSITIONS_EVERY_TICKS: u64 = 3;
/// A knockback lets a stunned player's body slide for this long after the event.
const KNOCK_WINDOW_S: f32 = 1.0;
/// Float noise a stunned player's reports may carry (a facing-only update, a stop flag).
const STUNNED_SLACK_M: f32 = 0.05;
/// One player alone in an exit takes the group along only after this long and only out of
/// combat, so a single griefer cannot yank everyone else through a door mid-fight.
const EXIT_DWELL: std::time::Duration = std::time::Duration::from_secs(3);

struct Member {
    eid: EntityId,
    name: String,
    class: String,
    /// Last accepted position; the sim only sees it at its next tick.
    pos: Vec3,
    /// Metres this player may still move before more time passes (token bucket).
    budget: f32,
    last_move: Instant,
    /// Set once the player stood outside every exit trigger: arriving (or being placed by a
    /// pack formation) inside one must not bounce the group straight back.
    armed: bool,
    /// When this player began standing in an open exit trigger (armed and alive).
    exit_since: Option<Instant>,
    /// Metres of knockback still allowed while stunned, and when it was granted.
    knock: (f32, Instant),
}

impl Member {
    fn resync(&mut self, pos: Vec3) {
        self.pos = pos;
        self.budget = JITTER_M;
        self.last_move = Instant::now();
        self.armed = false;
        self.exit_since = None;
        self.knock = (0.0, self.last_move);
    }
}

pub struct RoomState {
    code: String,
    world: Arc<World>,
    sim: Sim,
    seed: u64,
    members: BTreeMap<PlayerId, Member>,
    /// Fallback name counter for `sanitize_name`.
    joins: u32,
    snapshot_due: bool,
    pending: Vec<(Audience, Bytes)>,
    /// Position and facing last sent per unit (`Positions` carries only changes).
    sent_pos: HashMap<EntityId, ([i32; 3], u8)>,
    /// QA: joiners' start levels in join order (`Config::dev_start_level`).
    pub dev_start_level: Vec<u8>,
    /// Listed in `RoomList`; only the leader changes it (`ClientMsg::SetPublic`).
    pub public: bool,
    /// Already sanitised (`net_proto::sanitize_title` or `default_title`).
    pub title: String,
    /// Leader named by the last `RoomSettings`, so a leader change is announced once.
    leader_sent: EntityId,
}

/// What the public room list shows about a room; refreshed by the room task.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoomSummary {
    pub public: bool,
    pub title: String,
    /// Leader's sanitised display name; empty until someone joined.
    pub host: String,
    pub zone: String,
    pub avg_level: u8,
    pub roles: Vec<String>,
    pub in_sprint: bool,
}

fn frame(msg: &ServerMsg) -> Option<Bytes> {
    match encode(msg) {
        Ok(b) => Some(Bytes::from(b)),
        Err(e) => {
            tracing::warn!(error = %e, "dropping unencodable frame");
            None
        }
    }
}

/// Sent by the room when the sim refuses a player that the registry admitted.
pub fn cannot_join_frame() -> Option<Bytes> {
    frame(&ServerMsg::Error {
        kind: ErrorKind::CannotJoin,
        text: ErrorKind::CannotJoin.message().into(),
    })
}

/// Which exit, if any, the group takes: everyone alive is in one, or a lone dweller out of combat.
fn exit_choice(
    living: usize,
    ready: usize,
    in_combat: bool,
    dest: Option<String>,
    dwelled: Option<String>,
) -> Option<String> {
    if living > 0 && ready == living {
        dest
    } else if in_combat {
        None
    } else {
        dwelled
    }
}

fn dist_xz(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

impl RoomState {
    pub fn new(code: String, world: Arc<World>) -> Self {
        // Gameplay seed, not a secret: std's randomly keyed hasher is enough.
        let seed = RandomState::new().build_hasher().finish();
        let mut sim = Sim::new(world.content.clone(), seed);
        // World::new loaded every zone once already, so this cannot fail in practice.
        if let Err(e) = world.load(&mut sim, START_ZONE, None) {
            tracing::error!(error = %e, "cannot load start zone");
        }
        Self {
            code,
            world,
            sim,
            seed,
            members: BTreeMap::new(),
            joins: 0,
            snapshot_due: false,
            pending: Vec::new(),
            sent_pos: HashMap::new(),
            dev_start_level: Vec::new(),
            public: false,
            title: String::new(),
            leader_sent: 0,
        }
    }

    fn leader(&self) -> EntityId {
        self.sim.players().first().copied().unwrap_or_default()
    }

    fn settings(&mut self) -> ServerMsg {
        self.leader_sent = self.leader();
        ServerMsg::RoomSettings {
            public: self.public,
            title: self.title.clone(),
            leader: self.leader_sent,
        }
    }

    pub fn summary(&self) -> RoomSummary {
        let leader = self.leader();
        let levels: Vec<u32> = self
            .members
            .values()
            .filter_map(|m| self.sim.unit(m.eid))
            .map(|u| u32::from(u.level))
            .collect();
        let avg = levels.iter().sum::<u32>() / u32::try_from(levels.len().max(1)).unwrap_or(1);
        let roles: BTreeSet<&String> = self.members.values().map(|m| &m.class).collect();
        RoomSummary {
            public: self.public,
            title: self.title.clone(),
            host: self
                .members
                .values()
                .find(|m| m.eid == leader)
                .map(|m| m.name.clone())
                .unwrap_or_default(),
            zone: self.sim.zone().unwrap_or(START_ZONE).to_string(),
            avg_level: u8::try_from(avg).unwrap_or(u8::MAX),
            roles: roles.into_iter().cloned().collect(),
            in_sprint: self.sim.sprint_active(),
        }
    }

    fn push(&mut self, to: Audience, msg: &ServerMsg) {
        if let Some(b) = frame(msg) {
            self.pending.push((to, b));
        }
    }

    /// To every current member except `except`.
    fn broadcast(&mut self, except: Option<PlayerId>, msg: &ServerMsg) {
        let Some(b) = frame(msg) else {
            return;
        };
        for p in self.members.keys().filter(|p| Some(**p) != except) {
            self.pending.push((Audience::Only(*p), b.clone()));
        }
    }

    fn infos(&self) -> Vec<PlayerInfo> {
        self.members
            .values()
            .map(|m| PlayerInfo {
                id: m.eid,
                name: m.name.clone(),
                class: m.class.clone(),
            })
            .collect()
    }

    /// False when the sim has no room for another player (or `class` is unknown, which the
    /// handshake already rules out); the caller drops the connection.
    pub fn on_join(&mut self, player: PlayerId, raw_name: &str, class: &str) -> bool {
        self.joins += 1;
        let zone = self.sim.zone().unwrap_or(START_ZONE).to_string();
        let geom = self.world.zone(&zone);
        let (base, yaw) = geom
            .and_then(|g| g.anchor("player_spawn").or_else(|| g.anchor("graveyard")))
            .unwrap_or_default();
        let k = self.members.len();
        let (dx, dz) = pack_offsets(k + 1).get(k).copied().unwrap_or_default();
        let pos = Vec3::new(base.x + dx, base.y, base.z + dz);
        let eid = self.sim.spawn_player(class, pos, yaw);
        if eid == 0 {
            return false;
        }
        let nth = (self.joins as usize).saturating_sub(1);
        if let Some(level) = self
            .dev_start_level
            .get(nth)
            .or(self.dev_start_level.last())
        {
            self.sim.set_level_of(eid, *level);
        }
        // QA only (co-op outfit screenshots): `TLD_DEV_GEAR=a|b;c|d` equips joiner N with list N.
        if let Ok(v) = std::env::var("TLD_DEV_GEAR") {
            let lists: Vec<&str> = v.split(';').collect();
            if let Some(list) = lists.get(nth).or(lists.last()) {
                for item in list.split('|').map(str::trim).filter(|s| !s.is_empty()) {
                    self.sim.debug_equip(eid, item);
                }
            }
        }
        // Same respawn rule load_zone applies to everyone already in the group.
        if let Some((g, gy)) = geom.and_then(|g| g.anchor("graveyard")) {
            self.sim.set_respawn_point(g, gy);
        }
        let name = sanitize_name(raw_name, self.joins);
        let info = PlayerInfo {
            id: eid,
            name: name.clone(),
            class: class.to_string(),
        };
        self.members.insert(
            player,
            Member {
                eid,
                name,
                class: class.to_string(),
                pos,
                budget: JITTER_M,
                last_move: Instant::now(),
                armed: false,
                exit_since: None,
                knock: (0.0, Instant::now()),
            },
        );
        self.broadcast(Some(player), &ServerMsg::PlayerJoined(info));
        let welcome = ServerMsg::Welcome {
            you: eid,
            room: self.code.clone(),
            zone: self.sim.zone().map(str::to_string),
            seed: self.seed,
            players: self.infos(),
            quests: QuestSnap::from_log(self.sim.quests_of(eid)),
            cleared: self.cleared_zones(),
            unlocked: self.unlocked_zones(),
        };
        self.push(Audience::Only(player), &welcome);
        if let Some(s) = self.snapshot() {
            self.pending.push((Audience::Only(player), s));
        }
        let settings = self.settings();
        self.push(Audience::Only(player), &settings);
        true
    }

    fn cleared_zones(&self) -> Vec<String> {
        let content = self.sim.content();
        content
            .zones
            .iter()
            .filter(|z| self.sim.zone_cleared(&z.id))
            .map(|z| z.id.clone())
            .collect()
    }

    /// Zones behind a gated link that has opened (the client keys its door state on these).
    fn unlocked_zones(&self) -> Vec<String> {
        let content = self.sim.content();
        let gated: BTreeSet<String> = content
            .zones
            .iter()
            .flat_map(|z| z.links.iter())
            .filter(|l| l.requires.is_some())
            .map(|l| l.to.clone())
            .collect();
        content
            .zones
            .iter()
            .flat_map(|z| self.sim.zone_links(&z.id))
            .filter(|l| l.unlocked && gated.contains(&l.to))
            .map(|l| l.to)
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect()
    }

    pub fn on_leave(&mut self, player: PlayerId) {
        let Some(m) = self.members.remove(&player) else {
            return;
        };
        self.sim.remove_player(m.eid);
        self.broadcast(None, &ServerMsg::PlayerLeft { id: m.eid });
        if self.leader() != self.leader_sent && !self.members.is_empty() {
            let settings = self.settings();
            self.broadcast(None, &settings);
        }
    }

    /// `msg` is decoded and validated by `net_proto` and rate limited by the connection.
    pub fn on_message(&mut self, player: PlayerId, msg: ClientMsg) {
        let Some(eid) = self.members.get(&player).map(|m| m.eid) else {
            return;
        };
        match msg {
            // apply_as rebinds the actor to `eid`: a client can only ever act as itself.
            ClientMsg::Cmd(cmd) => self.sim.apply_as(eid, cmd),
            ClientMsg::Move {
                pos,
                facing,
                moving,
                jumping,
                seq,
            } => self.on_move(player, pos, facing, moving, jumping, seq),
            ClientMsg::Emote { emote, target } => {
                // Aimed only at another member of this room; anything else is said to nobody.
                let target =
                    target.filter(|t| *t != eid && self.members.values().any(|m| m.eid == *t));
                self.broadcast(
                    None,
                    &ServerMsg::Emoted {
                        player: eid,
                        emote,
                        target,
                    },
                );
            }
            ClientMsg::Ping { t } => self.push(Audience::Only(player), &ServerMsg::Pong { t }),
            ClientMsg::SetPublic { public } => {
                if eid != self.leader() {
                    let kind = ErrorKind::NotLeader;
                    let text = kind.message().into();
                    self.push(Audience::Only(player), &ServerMsg::Error { kind, text });
                } else if public != self.public {
                    self.public = public;
                    let settings = self.settings();
                    self.broadcast(None, &settings);
                }
            }
            // The connection handles the handshake and leaving; these never reach a room.
            ClientMsg::Hello { .. }
            | ClientMsg::CreateRoom { .. }
            | ClientMsg::ListRooms
            | ClientMsg::JoinPublic { .. }
            | ClientMsg::JoinRoom { .. }
            | ClientMsg::LeaveRoom => {}
        }
    }

    fn on_move(
        &mut self,
        player: PlayerId,
        pos: Vec3,
        facing: f32,
        moving: bool,
        jumping: bool,
        seq: u32,
    ) {
        let zone = self.sim.zone().unwrap_or(START_ZONE);
        let geom = self.world.zone(zone);
        let Some(m) = self.members.get_mut(&player) else {
            return;
        };
        let (alive, stunned) = self
            .sim
            .unit(m.eid)
            .map_or((false, false), |u| (u.alive, u.stunned));
        let now = Instant::now();
        // A ride or a sprint (`AuraDef::speed_pct`) raises the budget with the sim's own number.
        let speed = self.sim.move_speed_pct(m.eid) as f32 / 100.0;
        let rate = RUN_SPEED_MPS * SPEED_SLACK * speed;
        let cap = rate * MAX_BURST_S + JITTER_M;
        let dt = now.duration_since(m.last_move).as_secs_f32();
        // A knockback bonus may sit above the cap; refill never pushes it higher.
        m.budget = (m.budget + rate * dt).min(cap.max(m.budget));
        m.last_move = now;
        let d = dist_xz(pos, m.pos);
        // A stunned body only moves where a knockback throws it.
        let knock = if now.duration_since(m.knock.1).as_secs_f32() <= KNOCK_WINDOW_S {
            m.knock.0
        } else {
            0.0
        };
        let stun_ok = !stunned || d <= knock + STUNNED_SLACK_M;
        let ok = alive
            && stun_ok
            && facing.is_finite()
            && d <= m.budget
            && geom.is_some_and(|g| {
                g.in_bounds(pos) && !g.blocked(pos) && !g.segment_blocked(m.pos, pos)
            });
        if ok {
            m.budget -= d;
            if stunned {
                m.knock.0 = (m.knock.0 - d).max(0.0);
            }
            m.pos = pos;
            let cmd = Command::PlayerMoved {
                pos,
                yaw: facing,
                moving,
                jumping,
            };
            let eid = m.eid;
            self.sim.apply_as(eid, cmd);
        } else {
            let pos = m.pos;
            self.push(
                Audience::Only(player),
                &ServerMsg::MoveCorrection { seq, pos },
            );
        }
    }

    /// Advances the sim one tick and returns every frame to deliver.
    pub fn tick(&mut self) -> Vec<(Audience, Bytes)> {
        if self.members.is_empty() {
            // Nobody to simulate for; the world pauses until someone joins or the room expires.
            return std::mem::take(&mut self.pending);
        }
        let mut routed = self.sim.tick_routed();
        self.follow_sim_moves(&routed);
        if self.follow_sprint() || self.check_exits() {
            // ZoneLoaded and the despawns go out with this tick, before the snapshot.
            routed.extend(self.sim.drain_events_routed());
        }
        self.route(&routed);
        self.push_timers(&routed);
        let now = self.sim.now().0;
        if self.snapshot_due || now.is_multiple_of(SNAPSHOT_EVERY_TICKS) {
            self.snapshot_due = false;
            if let Some(s) = self.snapshot() {
                self.pending.push((Audience::All, s));
            }
            // The snapshot carried every position: the next `Positions` sends changes from here.
            self.sent_pos = self
                .sim
                .units()
                .map(|u| (u.id, (pos_cm(u.pos), quantize_yaw(u.yaw))))
                .collect();
        } else if now.is_multiple_of(POSITIONS_EVERY_TICKS) {
            self.push_positions();
        }
        std::mem::take(&mut self.pending)
    }

    /// Units whose position or facing changed since they were last sent.
    fn push_positions(&mut self) {
        let mut units = Vec::new();
        for u in self.sim.units() {
            let now = (pos_cm(u.pos), quantize_yaw(u.yaw));
            if self.sent_pos.get(&u.id) != Some(&now) {
                self.sent_pos.insert(u.id, now);
                units.push((u.id, now.0, now.1));
            }
        }
        if units.is_empty() {
            return;
        }
        units.truncate(MAX_SNAPSHOT_UNITS);
        let msg = ServerMsg::Positions {
            tick: self.sim.now().0,
            units,
        };
        self.push(Audience::All, &msg);
    }

    /// Timers the events cannot carry: aura durations for units that just gained an aura,
    /// and each caster's own cooldowns after its casts.
    fn push_timers(&mut self, routed: &[(SimAudience, Event)]) {
        let mut aura_targets = BTreeSet::new();
        let mut casters = BTreeSet::new();
        for (_, e) in routed {
            match e {
                Event::AuraApplied { target, .. } => {
                    aura_targets.insert(*target);
                }
                Event::CastStarted { caster, .. }
                | Event::CastSucceeded { caster, .. }
                | Event::CastInterrupted { caster, .. } => {
                    casters.insert(*caster);
                }
                _ => {}
            }
        }
        if !aura_targets.is_empty() {
            let content = self.sim.content();
            let units: Vec<(EntityId, Vec<AuraSnap>)> = aura_targets
                .iter()
                .filter_map(|id| self.sim.unit(*id))
                .map(|u| {
                    let auras = u
                        .auras
                        .iter()
                        .filter_map(|a| AuraSnap::from_view(a, content))
                        .take(net_proto::MAX_LIST_LEN)
                        .collect();
                    (u.id, auras)
                })
                .collect();
            self.broadcast(None, &ServerMsg::Auras { units });
        }
        let members: Vec<(PlayerId, EntityId)> = self
            .members
            .iter()
            .filter(|(_, m)| casters.contains(&m.eid))
            .map(|(p, m)| (*p, m.eid))
            .collect();
        for (player, eid) in members {
            let content = self.sim.content();
            let class = self.sim.class_of(eid);
            let spells = class
                .map_or(content.action_bar(), |c| &c.action_bar)
                .iter()
                .copied()
                .chain(class.map(|c| c.resurrect))
                .chain(content.channel_spell().map(|s| s.id));
            let msg = cooldowns_msg(self.sim.now().0, spells, |s| self.sim.cooldown(eid, s));
            self.push(Audience::Only(player), &msg);
        }
    }

    /// The sim moved a player itself (respawn, teleport): movement checks restart there.
    /// A knockback is applied by the client, so it only widens that player's budget.
    fn follow_sim_moves(&mut self, routed: &[(SimAudience, Event)]) {
        for (_, e) in routed {
            let (unit, knock) = match e {
                Event::Respawned { unit } | Event::Teleported { unit, .. } => (*unit, None),
                Event::Knockback { unit, strength, .. } => (*unit, Some(*strength)),
                _ => continue,
            };
            let pos = self.sim.unit(unit).map(|u| u.pos);
            if let (Some(m), Some(pos)) = (self.members.values_mut().find(|m| m.eid == unit), pos) {
                match knock {
                    Some(s) if s.is_finite() => {
                        m.budget += s.max(0.0);
                        m.knock = (m.knock.0 + s.max(0.0), Instant::now());
                    }
                    Some(_) => {}
                    None => m.resync(pos),
                }
            }
        }
    }

    /// One `Events` frame per recipient per tick, holding the events meant for them.
    fn route(&mut self, routed: &[(SimAudience, Event)]) {
        if routed.is_empty() {
            return;
        }
        let tick = self.sim.now().0;
        let recipients: Vec<(PlayerId, EntityId)> =
            self.members.iter().map(|(p, m)| (*p, m.eid)).collect();
        for (player, eid) in recipients {
            let events: Vec<Event> = routed
                .iter()
                .filter(|(a, _)| match a {
                    SimAudience::All => true,
                    SimAudience::Player(id) => *id == eid,
                })
                .map(|(_, e)| e.clone())
                .collect();
            for chunk in events.chunks(MAX_EVENTS_PER_FRAME) {
                self.push_events(player, tick, chunk);
            }
        }
    }

    /// Halves a batch until it fits in a frame.
    fn push_events(&mut self, player: PlayerId, tick: u64, events: &[Event]) {
        if events.is_empty() {
            return;
        }
        let msg = ServerMsg::Events {
            tick,
            events: events.to_vec(),
        };
        match encode(&msg) {
            Ok(b) => self.pending.push((Audience::Only(player), Bytes::from(b))),
            Err(ProtoError::TooLarge(_)) if events.len() > 1 => {
                let (a, b) = events.split_at(events.len() / 2);
                self.push_events(player, tick, a);
                self.push_events(player, tick, b);
            }
            Err(e) => tracing::warn!(error = %e, "dropping unencodable event"),
        }
    }

    /// The group travels when every living member stands in an open exit trigger, or when one
    /// has stood in it for `EXIT_DWELL` and nobody is in combat. A solo player is the first case.
    /// True when the group travelled.
    fn check_exits(&mut self) -> bool {
        let Some(zone) = self.sim.zone().map(str::to_string) else {
            return false;
        };
        let Some(geom) = self.world.zone(&zone) else {
            return false;
        };
        let exits: Vec<(Aabb, bool, String)> = self
            .sim
            .zone_links(&zone)
            .into_iter()
            .filter_map(|l| geom.trigger(&l.anchor).map(|t| (*t, l.unlocked, l.to)))
            .collect();
        let now = Instant::now();
        let (mut living, mut ready, mut in_combat) = (0usize, 0usize, false);
        let mut dest: Option<String> = None;
        let mut dwelled: Option<String> = None;
        for m in self.members.values_mut() {
            let unit = self.sim.unit(m.eid);
            let alive = unit.is_some_and(|u| u.alive);
            if alive {
                living += 1;
                in_combat |= unit.is_some_and(|u| u.in_combat);
            }
            let hit = exits.iter().find(|(t, _, _)| in_xz(t, m.pos));
            if hit.is_none() {
                m.armed = true;
            }
            match hit {
                Some((_, true, d)) if m.armed && alive => {
                    ready += 1;
                    dest = Some(d.clone());
                    let since = *m.exit_since.get_or_insert(now);
                    if now.duration_since(since) >= EXIT_DWELL {
                        dwelled = Some(d.clone());
                    }
                }
                _ => m.exit_since = None,
            }
        }
        let to = exit_choice(living, ready, in_combat, dest, dwelled);
        to.is_some_and(|to| self.travel(&zone, &to))
    }

    /// Sprint mode: the sim names the next stage (or campus after the review); the server
    /// loads it for the whole group. True when the group travelled.
    fn follow_sprint(&mut self) -> bool {
        let Some(to) = self.sim.sprint_travel().map(str::to_string) else {
            return false;
        };
        let from = self.sim.zone().unwrap_or(START_ZONE).to_string();
        self.travel(&from, &to)
    }

    fn travel(&mut self, from: &str, to: &str) -> bool {
        let arrival = self.world.zone(to).and_then(|g| g.arrival(from));
        if let Err(e) = self.world.load(&mut self.sim, to, arrival) {
            tracing::error!(error = %e, "zone travel failed");
            return false;
        }
        for m in self.members.values_mut() {
            if let Some(u) = self.sim.unit(m.eid) {
                m.resync(u.pos);
            }
        }
        // The snapshot places everyone at the arrival point.
        self.snapshot_due = true;
        true
    }

    fn snapshot(&self) -> Option<Bytes> {
        let content = self.sim.content();
        let units = self
            .sim
            .units()
            .filter_map(|u| UnitSnap::from_state(u, content))
            .take(MAX_SNAPSHOT_UNITS)
            .collect();
        let players = self
            .sim
            .players()
            .iter()
            .filter_map(|id| self.sim.unit(*id))
            .map(|u| PlayerSnap::from_state(u, content))
            .collect();
        frame(&ServerMsg::Snapshot {
            tick: self.sim.now().0,
            units,
            players,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use net_proto::{Emote, decode};

    fn room() -> RoomState {
        RoomState::new("TECH-DEBT-00000000".into(), World::embedded().unwrap())
    }

    fn msgs(out: &[(Audience, Bytes)], to: PlayerId) -> Vec<ServerMsg> {
        out.iter()
            .filter(|(a, _)| match a {
                Audience::All => true,
                Audience::Only(p) => *p == to,
            })
            .map(|(_, b)| decode::<ServerMsg>(b).unwrap())
            .collect()
    }

    fn eid_of(s: &RoomState, p: PlayerId) -> EntityId {
        s.members[&p].eid
    }

    #[tokio::test(start_paused = true)]
    async fn join_welcomes_and_announces() {
        let mut s = room();
        assert!(s.on_join(0, "Dana", "fullstack"));
        assert!(s.on_join(1, "<bad>", "fullstack"));
        let out = s.tick();
        let a = msgs(&out, 0);
        assert!(
            matches!(&a[0], ServerMsg::Welcome { zone: Some(z), players, .. } if z == "campus" && players.len() == 1)
        );
        assert!(matches!(a[1], ServerMsg::Snapshot { .. }));
        assert!(
            a.iter()
                .any(|m| matches!(m, ServerMsg::PlayerJoined(p) if p.name == "Intern-2"))
        );
        let b = msgs(&out, 1);
        assert!(
            matches!(&b[0], ServerMsg::Welcome { players, .. } if players.len() == 2 && players[1].name == "Intern-2")
        );
    }

    /// Each player keeps the class of its `Hello`: the sim spawns that class's unit, and the
    /// welcome, join notice and snapshots carry the class key for frames and nameplates.
    #[tokio::test(start_paused = true)]
    async fn players_join_as_their_own_class() {
        let mut s = room();
        assert!(s.on_join(0, "Tank", "backend"));
        assert!(s.on_join(1, "Ops", "devops"));
        let out = s.tick();
        let b = msgs(&out, 1);
        let classes =
            |players: &[PlayerInfo]| players.iter().map(|p| p.class.clone()).collect::<Vec<_>>();
        assert!(
            matches!(&b[0], ServerMsg::Welcome { players, .. } if classes(players) == ["backend", "devops"])
        );
        let snap = b.iter().find_map(|m| match m {
            ServerMsg::Snapshot { players, .. } => Some(players.clone()),
            _ => None,
        });
        let snap_classes: Vec<String> = snap
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.class)
            .collect();
        assert_eq!(snap_classes, ["backend", "devops"]);
        assert!(msgs(&out, 0).iter().any(
            |m| matches!(m, ServerMsg::PlayerJoined(p) if p.name == "Ops" && p.class == "devops")
        ));
        let keys: Vec<String> = s
            .sim
            .players()
            .iter()
            .filter_map(|id| s.sim.unit(*id).map(|u| u.key.clone()))
            .collect();
        assert_eq!(keys, ["backend_dev", "devops_eng"]);
        // The handshake rejects unknown classes; the room still refuses to spawn one.
        assert!(!s.on_join(2, "Rock", "10x_rockstar"));
    }

    #[tokio::test(start_paused = true)]
    async fn sixth_player_is_refused_by_the_sim() {
        let mut s = room();
        for p in 0..5 {
            assert!(s.on_join(p, "x", "fullstack"));
        }
        assert!(!s.on_join(5, "x", "fullstack"));
    }

    #[tokio::test(start_paused = true)]
    async fn moves_are_checked() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.tick();
        let start = s.members[&0].pos;
        let mv = |pos: Vec3, seq| ClientMsg::Move {
            pos,
            facing: 0.0,
            moving: true,
            jumping: false,
            seq,
        };
        // Within the jitter allowance right away.
        s.on_message(0, mv(Vec3::new(start.x + 0.5, 0.0, start.z), 1));
        // Teleport.
        s.on_message(0, mv(Vec3::new(start.x + 30.0, 0.0, start.z), 2));
        // Out of bounds.
        s.on_message(0, mv(Vec3::new(1e6, 0.0, 0.0), 3));
        let out = s.tick();
        let corrections: Vec<u32> = msgs(&out, 0)
            .iter()
            .filter_map(|m| match m {
                ServerMsg::MoveCorrection { seq, .. } => Some(*seq),
                _ => None,
            })
            .collect();
        assert_eq!(corrections, [2, 3]);
        let eid = eid_of(&s, 0);
        assert!((s.sim.unit(eid).unwrap().pos.x - (start.x + 0.5)).abs() < 1e-4);
        // After a second at run speed, 7 m is fine.
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        s.on_message(0, mv(Vec3::new(start.x + 7.5, 0.0, start.z), 4));
        let out = s.tick();
        assert!(
            !msgs(&out, 0)
                .iter()
                .any(|m| matches!(m, ServerMsg::MoveCorrection { .. }))
        );
    }

    fn mv(pos: Vec3, seq: u32) -> ClientMsg {
        ClientMsg::Move {
            pos,
            facing: 0.0,
            moving: true,
            jumping: false,
            seq,
        }
    }

    fn corrections(out: &[(Audience, Bytes)], to: PlayerId) -> Vec<u32> {
        msgs(out, to)
            .iter()
            .filter_map(|m| match m {
                ServerMsg::MoveCorrection { seq, .. } => Some(*seq),
                _ => None,
            })
            .collect()
    }

    #[tokio::test(start_paused = true)]
    async fn mounted_players_may_move_faster() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.tick();
        let start = s.members[&0].pos;
        // 13 m in a second: too fast on foot (8.75 m/s budget plus jitter) ...
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        s.on_message(0, mv(Vec3::new(start.x + 13.0, start.y, start.z), 1));
        assert_eq!(corrections(&s.tick(), 0), [1]);
        // ... fine on an e-scooter (+60%).
        let eid = eid_of(&s, 0);
        assert!(s.sim.debug_apply_aura(eid, "e_scooter", 60_000));
        s.tick();
        assert_eq!(s.sim.move_speed_pct(eid), 160);
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        s.on_message(0, mv(Vec3::new(start.x + 13.0, start.y, start.z), 2));
        assert!(corrections(&s.tick(), 0).is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn stunned_players_cannot_walk() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.tick();
        let eid = eid_of(&s, 0);
        assert!(s.sim.debug_apply_aura(eid, "standup", 5_000));
        s.tick();
        assert!(s.sim.unit(eid).unwrap().stunned);
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        let start = s.members[&0].pos;
        s.on_message(0, mv(Vec3::new(start.x + 0.5, start.y, start.z), 1));
        // Turning on the spot (or a stop flag) is not walking.
        s.on_message(0, mv(start, 2));
        assert_eq!(corrections(&s.tick(), 0), [1]);
        // A knockback may throw the stunned body that far.
        s.members.get_mut(&0).unwrap().knock = (2.0, Instant::now());
        s.on_message(0, mv(Vec3::new(start.x + 1.5, start.y, start.z), 3));
        assert!(corrections(&s.tick(), 0).is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn moves_through_thin_walls_are_corrected() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.tick();
        let g = s.world.zone("campus").unwrap();
        // A tall, thin wall with walkable ground on both sides of its middle.
        let found = g.blockers.iter().find_map(|b| {
            let (w, d) = (b.max.x - b.min.x, b.max.z - b.min.z);
            let c = Vec3::new((b.min.x + b.max.x) / 2.0, 0.0, (b.min.z + b.max.z) / 2.0);
            let off = w.min(d) / 2.0 + 0.5;
            let (a, z) = if w < d {
                (
                    Vec3::new(c.x - off, 0.0, c.z),
                    Vec3::new(c.x + off, 0.0, c.z),
                )
            } else {
                (
                    Vec3::new(c.x, 0.0, c.z - off),
                    Vec3::new(c.x, 0.0, c.z + off),
                )
            };
            let legal = |p: Vec3| g.in_bounds(p) && !g.blocked(p);
            (b.max.y > 2.5 && w.min(d) < 1.0 && w.max(d) > 3.0 && legal(a) && legal(z))
                .then_some((a, z))
        });
        let (a, z) = found.expect("campus has a thin wall");
        s.members.get_mut(&0).unwrap().pos = a;
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        s.on_message(0, mv(z, 1));
        assert_eq!(corrections(&s.tick(), 0), [1]);
        assert_eq!(s.members[&0].pos, a);
    }

    #[tokio::test(start_paused = true)]
    async fn claimed_height_cannot_lift_a_player_over_a_wall() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.tick();
        let g = s.world.zone("campus").unwrap();
        let found = g.blockers.iter().find_map(|b| {
            let (w, d) = (b.max.x - b.min.x, b.max.z - b.min.z);
            let c = Vec3::new((b.min.x + b.max.x) / 2.0, 0.0, (b.min.z + b.max.z) / 2.0);
            let off = w.min(d) / 2.0 + 0.5;
            let (a, z) = if w < d {
                (
                    Vec3::new(c.x - off, 0.0, c.z),
                    Vec3::new(c.x + off, 0.0, c.z),
                )
            } else {
                (
                    Vec3::new(c.x, 0.0, c.z - off),
                    Vec3::new(c.x, 0.0, c.z + off),
                )
            };
            let legal = |p: Vec3| g.in_bounds(p) && !g.blocked(p);
            (b.max.y > 2.5 && w.min(d) < 1.0 && w.max(d) > 3.0 && legal(a) && legal(z))
                .then_some((a, z))
        });
        let (a, z) = found.expect("campus has a thin wall");
        s.members.get_mut(&0).unwrap().pos = a;
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        for (seq, y) in [(1, 7.0), (2, 1.4)] {
            s.on_message(0, mv(Vec3::new(z.x, y, z.z), seq));
        }
        assert_eq!(corrections(&s.tick(), 0), [1, 2]);
        assert_eq!(s.members[&0].pos, a);
        // An ordinary jump apex on open ground is still accepted.
        let up = Vec3::new(a.x, 1.28, a.z);
        s.on_message(0, mv(up, 3));
        assert!(corrections(&s.tick(), 0).is_empty());
        assert_eq!(s.members[&0].pos, up);
    }

    #[tokio::test(start_paused = true)]
    async fn moving_units_go_out_as_positions_between_snapshots() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.sim.set_wandering(false);
        // Run past the join snapshot to a full snapshot, which resets the change tracking.
        while !s.sim.now().0.is_multiple_of(SNAPSHOT_EVERY_TICKS) || s.sim.now().0 == 0 {
            s.tick();
        }
        let eid = eid_of(&s, 0);
        let start = s.members[&0].pos;
        let to = Vec3::new(start.x + 0.5, start.y, start.z);
        s.on_message(0, mv(to, 1));
        let mut seen = Vec::new();
        for _ in 0..POSITIONS_EVERY_TICKS * 2 {
            for m in msgs(&s.tick(), 0) {
                if let ServerMsg::Positions { units, .. } = m {
                    seen.push(units);
                }
            }
        }
        // Once with the new spot, then nothing for a unit that stood still.
        let mine: Vec<_> = seen.iter().flatten().filter(|u| u.0 == eid).collect();
        assert_eq!(mine.len(), 1, "{seen:?}");
        assert_eq!(mine[0].1, pos_cm(to));
    }

    #[tokio::test(start_paused = true)]
    async fn auras_and_cooldowns_carry_their_timers() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.on_join(1, "b", "fullstack");
        s.tick();
        let mob = s
            .sim
            .units()
            .find(|u| u.hostile && u.alive && !u.boss)
            .map(|u| u.id)
            .unwrap();
        assert!(s.sim.debug_apply_aura(mob, "review_fatigue", 8_000));
        let out = s.tick();
        let auras = msgs(&out, 1).into_iter().find_map(|m| match m {
            ServerMsg::Auras { units } => Some(units),
            _ => None,
        });
        let units = auras.expect("Auras frame");
        let (id, list) = &units[0];
        assert_eq!(*id, mob);
        assert_eq!(list[0].total_ms, 8_000);
        assert!((7_900..=8_000).contains(&list[0].remaining_ms), "{list:?}");

        // The caster alone hears about its cooldowns.
        let eid = eid_of(&s, 0);
        let coffee = s.sim.content().channel_spell().unwrap().id;
        // Coffee is only allowed below full mana.
        s.sim.set_player_mana(10);
        s.on_message(
            0,
            ClientMsg::Cmd(Command::Cast {
                caster: eid,
                spell: coffee,
            }),
        );
        let out = s.tick();
        let started = msgs(&out, 0).iter().any(|m| {
            matches!(m, ServerMsg::Events { events, .. }
                if events.iter().any(|e| matches!(e, Event::CastStarted { .. } | Event::CastSucceeded { .. })))
        });
        assert!(started, "coffee break did not start");
        assert!(
            msgs(&out, 0)
                .iter()
                .any(|m| matches!(m, ServerMsg::Cooldowns { .. }))
        );
        assert!(
            !msgs(&out, 1)
                .iter()
                .any(|m| matches!(m, ServerMsg::Cooldowns { .. }))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn late_joiner_gets_its_own_log_zone_state_and_aura_timers() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.tick();
        assert!(
            s.sim
                .debug_set_quest("onboarding", combat_core::QuestStatus::Active)
        );
        let door = *s
            .world
            .zone("campus")
            .unwrap()
            .trigger("door:tower/1")
            .unwrap();
        s.members.get_mut(&0).unwrap().pos = Vec3::new(
            (door.min.x + door.max.x) / 2.0,
            0.0,
            (door.min.z + door.max.z) / 2.0,
        );
        s.tick();
        assert_eq!(s.sim.zone(), Some("tower/1"));
        let boss = s
            .sim
            .units()
            .find(|u| u.boss && u.alive)
            .map(|u| u.id)
            .unwrap();
        assert!(s.sim.debug_kill(boss));
        let mob = s
            .sim
            .units()
            .find(|u| u.hostile && u.alive && !u.boss)
            .map(|u| u.id)
            .unwrap();
        assert!(s.sim.debug_apply_aura(mob, "review_fatigue", 8_000));
        for _ in 0..30 {
            s.tick();
        }
        assert!(s.on_join(1, "late", "fullstack"));
        let out = s.tick();
        let m = msgs(&out, 1);
        let ServerMsg::Welcome {
            quests,
            cleared,
            unlocked,
            zone,
            ..
        } = &m[0]
        else {
            panic!("{:?}", m[0])
        };
        assert_eq!(zone.as_deref(), Some("tower/1"));
        assert_eq!(
            quests,
            &QuestSnap::default(),
            "the joiner's log is its own, fresh"
        );
        assert_eq!(cleared, &["tower/1".to_string()]);
        assert!(unlocked.contains(&"tower/2".to_string()), "{unlocked:?}");
        let ServerMsg::Snapshot { units, .. } = &m[1] else {
            panic!("{:?}", m[1])
        };
        let aura = &units.iter().find(|u| u.id == mob).unwrap().auras[0];
        assert_eq!(aura.total_ms, 8_000);
        assert!((6_500..8_000).contains(&aura.remaining_ms), "{aura:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn walking_into_an_open_door_moves_the_group() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.on_join(1, "b", "fullstack");
        s.tick();
        let door = *s
            .world
            .zone("campus")
            .unwrap()
            .trigger("door:tower/1")
            .unwrap();
        let centre = Vec3::new(
            (door.min.x + door.max.x) / 2.0,
            0.0,
            (door.min.z + door.max.z) / 2.0,
        );
        // Test shortcut: place the players as if they had walked there. One alone is not enough.
        s.members.get_mut(&0).unwrap().pos = centre;
        s.tick();
        assert_eq!(s.sim.zone(), Some("campus"));
        s.members.get_mut(&1).unwrap().pos = centre;
        let out = s.tick();
        assert_eq!(s.sim.zone(), Some("tower/1"));
        // Arrivals start disarmed: standing in the back door does not bounce the group.
        let back = *s
            .world
            .zone("tower/1")
            .unwrap()
            .trigger("door:campus")
            .unwrap();
        s.members.get_mut(&1).unwrap().pos = Vec3::new(
            (back.min.x + back.max.x) / 2.0,
            0.0,
            (back.min.z + back.max.z) / 2.0,
        );
        s.tick();
        assert_eq!(s.sim.zone(), Some("tower/1"));
        for p in [0, 1] {
            let m = msgs(&out, p);
            assert!(m.iter().any(|m| matches!(m, ServerMsg::Events { events, .. }
                if events.iter().any(|e| matches!(e, Event::ZoneLoaded { zone } if zone == "tower/1")))));
            assert!(m.iter().any(|m| matches!(m, ServerMsg::Snapshot { .. })));
        }
        let arrival = s
            .world
            .zone("tower/1")
            .unwrap()
            .arrival("campus")
            .unwrap()
            .0;
        assert!(dist_xz(s.members[&0].pos, arrival) < 4.0);
    }

    #[tokio::test(start_paused = true)]
    async fn the_leader_starts_a_sprint_and_the_server_moves_the_group() {
        let mut s = room();
        s.on_join(0, "lead", "fullstack");
        s.on_join(1, "member", "devops");
        for p in [0, 1] {
            let e = eid_of(&s, p);
            s.sim.set_level_of(e, 10);
        }
        s.tick();
        let start = || {
            ClientMsg::Cmd(Command::StartSprint {
                building: "lab".into(),
                level: 2,
            })
        };
        s.on_message(1, start());
        let out = s.tick();
        let refused = |out: &[(Audience, Bytes)], p| {
            msgs(out, p).iter().any(|m| {
                matches!(m, ServerMsg::Events { events, .. }
                if events.iter().any(|e| matches!(e, Event::SprintRefused { .. })))
            })
        };
        assert!(refused(&out, 1) && !refused(&out, 0));
        assert!(!s.sim.sprint_active());
        s.on_message(0, start());
        let mut out = Vec::new();
        for _ in 0..150 {
            out.extend(s.tick());
        }
        assert_eq!(s.sim.zone(), Some("lab/1"));
        for p in [0, 1] {
            let m = msgs(&out, p);
            for want in ["started", "loaded"] {
                assert!(
                    m.iter()
                        .any(|m| matches!(m, ServerMsg::Events { events, .. }
                        if events.iter().any(|e| match e {
                            Event::SprintStarted { .. } => want == "started",
                            Event::ZoneLoaded { zone } => want == "loaded" && zone == "lab/1",
                            _ => false,
                        }))),
                    "player {p} missed {want}"
                );
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn lone_dweller_moves_the_group_after_three_seconds() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.on_join(1, "b", "fullstack");
        s.tick();
        let door = *s
            .world
            .zone("campus")
            .unwrap()
            .trigger("door:tower/1")
            .unwrap();
        s.members.get_mut(&0).unwrap().pos = Vec3::new(
            (door.min.x + door.max.x) / 2.0,
            0.0,
            (door.min.z + door.max.z) / 2.0,
        );
        s.tick();
        tokio::time::advance(std::time::Duration::from_millis(2_500)).await;
        s.tick();
        assert_eq!(s.sim.zone(), Some("campus"));
        tokio::time::advance(std::time::Duration::from_millis(600)).await;
        s.tick();
        assert_eq!(s.sim.zone(), Some("tower/1"));
    }

    #[test]
    fn combat_blocks_the_lone_dweller_but_not_the_whole_group() {
        let to = || Some("tower/1".to_string());
        assert_eq!(exit_choice(2, 1, false, to(), to()), to());
        assert_eq!(exit_choice(2, 1, true, to(), to()), None);
        assert_eq!(exit_choice(2, 2, true, to(), None), to());
        assert_eq!(exit_choice(2, 1, false, to(), None), None);
        assert_eq!(exit_choice(0, 0, false, None, None), None);
    }

    #[tokio::test(start_paused = true)]
    async fn locked_door_does_nothing() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.tick();
        let door = *s
            .world
            .zone("campus")
            .unwrap()
            .trigger("door:datacenter/1")
            .unwrap();
        s.members.get_mut(&0).unwrap().pos = Vec3::new(
            (door.min.x + door.max.x) / 2.0,
            0.0,
            (door.min.z + door.max.z) / 2.0,
        );
        s.tick();
        assert_eq!(s.sim.zone(), Some("campus"));
    }

    #[tokio::test(start_paused = true)]
    async fn leave_emote_ping() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.on_join(1, "b", "fullstack");
        s.tick();
        s.on_message(
            1,
            ClientMsg::Emote {
                emote: Emote::Lgtm,
                target: None,
            },
        );
        s.on_message(0, ClientMsg::Ping { t: 7 });
        let eid1 = eid_of(&s, 1);
        s.on_leave(1);
        assert!(s.sim.unit(eid1).is_none());
        let a = msgs(&s.tick(), 0);
        assert!(a.contains(&ServerMsg::Emoted {
            player: eid1,
            emote: Emote::Lgtm,
            target: None,
        }));
        assert!(a.contains(&ServerMsg::Pong { t: 7 }));
        assert!(a.contains(&ServerMsg::PlayerLeft { id: eid1 }));
    }

    #[tokio::test(start_paused = true)]
    async fn emote_targets_must_be_other_members_of_the_room() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        s.on_join(1, "b", "fullstack");
        s.tick();
        let (a, b) = (eid_of(&s, 0), eid_of(&s, 1));
        let mob = s.sim.units().find(|u| u.hostile).map(|u| u.id).unwrap();
        for (target, kept) in [
            (Some(b), Some(b)),
            (Some(a), None),
            (Some(mob), None),
            (Some(9_999), None),
            (None, None),
        ] {
            s.on_message(
                0,
                ClientMsg::Emote {
                    emote: Emote::Sync,
                    target,
                },
            );
            let got = msgs(&s.tick(), 1);
            assert!(
                got.contains(&ServerMsg::Emoted {
                    player: a,
                    emote: Emote::Sync,
                    target: kept
                }),
                "{target:?}"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn dev_start_level_applies_to_joiners_only_when_set() {
        let mut s = room();
        s.on_join(0, "a", "fullstack");
        assert_eq!(s.sim.unit(eid_of(&s, 0)).unwrap().level, 1);
        s.dev_start_level = vec![9, 5, 2];
        s.on_join(1, "b", "fullstack");
        s.on_join(2, "c", "fullstack");
        s.on_join(3, "d", "fullstack");
        assert_eq!(s.sim.unit(eid_of(&s, 1)).unwrap().level, 5);
        assert_eq!(s.sim.unit(eid_of(&s, 2)).unwrap().level, 2);
        assert_eq!(s.sim.unit(eid_of(&s, 3)).unwrap().level, 2);
    }
}
