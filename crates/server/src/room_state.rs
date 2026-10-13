//! Authoritative game state of one room: a `combat_core::Sim` plus the per-player wire state.
//!
//! `rooms.rs` drives it through `new`, `on_join`, `on_leave`, `on_message` and `tick` (30 Hz)
//! and delivers the returned frames; nothing here touches sockets or awaits.

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher, RandomState};
use std::sync::Arc;

use axum::body::Bytes;
use combat_core::{
    Aabb, Audience as SimAudience, Command, EntityId, Event, Sim, Vec3, pack_offsets,
};
use net_proto::{
    ClientMsg, ErrorKind, MAX_EVENTS_PER_FRAME, MAX_SNAPSHOT_UNITS, PlayerInfo, PlayerSnap,
    ProtoError, ServerMsg, UnitSnap, encode, sanitize_name,
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

struct Member {
    eid: EntityId,
    name: String,
    /// Last accepted position; the sim only sees it at its next tick.
    pos: Vec3,
    /// Metres this player may still move before more time passes (token bucket).
    budget: f32,
    last_move: Instant,
    /// Set once the player stood outside every exit trigger: arriving (or being placed by a
    /// pack formation) inside one must not bounce the group straight back.
    armed: bool,
}

impl Member {
    fn resync(&mut self, pos: Vec3) {
        self.pos = pos;
        self.budget = JITTER_M;
        self.last_move = Instant::now();
        self.armed = false;
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
            })
            .collect()
    }

    /// False when the sim has no room for another player; the caller drops the connection.
    pub fn on_join(&mut self, player: PlayerId, raw_name: &str) -> bool {
        self.joins += 1;
        let zone = self.sim.zone().unwrap_or(START_ZONE).to_string();
        let geom = self.world.zone(&zone);
        let (base, yaw) = geom
            .and_then(|g| g.anchor("player_spawn").or_else(|| g.anchor("graveyard")))
            .unwrap_or_default();
        let k = self.members.len();
        let (dx, dz) = pack_offsets(k + 1).get(k).copied().unwrap_or_default();
        let pos = Vec3::new(base.x + dx, base.y, base.z + dz);
        let eid = self.sim.spawn_player(pos, yaw);
        if eid == 0 {
            return false;
        }
        // Same respawn rule load_zone applies to everyone already in the group.
        if let Some((g, gy)) = geom.and_then(|g| g.anchor("graveyard")) {
            self.sim.set_respawn_point(g, gy);
        }
        let name = sanitize_name(raw_name, self.joins);
        let info = PlayerInfo {
            id: eid,
            name: name.clone(),
        };
        self.members.insert(
            player,
            Member {
                eid,
                name,
                pos,
                budget: JITTER_M,
                last_move: Instant::now(),
                armed: false,
            },
        );
        self.broadcast(Some(player), &ServerMsg::PlayerJoined(info));
        let welcome = ServerMsg::Welcome {
            you: eid,
            room: self.code.clone(),
            zone: self.sim.zone().map(str::to_string),
            seed: self.seed,
            players: self.infos(),
        };
        self.push(Audience::Only(player), &welcome);
        if let Some(s) = self.snapshot() {
            self.pending.push((Audience::Only(player), s));
        }
        true
    }

    pub fn on_leave(&mut self, player: PlayerId) {
        let Some(m) = self.members.remove(&player) else {
            return;
        };
        self.sim.remove_player(m.eid);
        self.broadcast(None, &ServerMsg::PlayerLeft { id: m.eid });
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
            ClientMsg::Emote(emote) => {
                self.broadcast(None, &ServerMsg::Emoted { player: eid, emote });
            }
            ClientMsg::Ping { t } => self.push(Audience::Only(player), &ServerMsg::Pong { t }),
            // The connection handles the handshake and leaving; these never reach a room.
            ClientMsg::Hello { .. }
            | ClientMsg::CreateRoom
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
        let alive = self.sim.unit(m.eid).is_some_and(|u| u.alive);
        let now = Instant::now();
        let rate = RUN_SPEED_MPS * SPEED_SLACK;
        let cap = rate * MAX_BURST_S + JITTER_M;
        let dt = now.duration_since(m.last_move).as_secs_f32();
        // A knockback bonus may sit above the cap; refill never pushes it higher.
        m.budget = (m.budget + rate * dt).min(cap.max(m.budget));
        m.last_move = now;
        let d = dist_xz(pos, m.pos);
        let ok = alive
            && facing.is_finite()
            && d <= m.budget
            && geom.is_some_and(|g| g.in_bounds(pos) && !g.blocked(pos));
        if ok {
            m.budget -= d;
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
        if self.check_exits() {
            // ZoneLoaded and the despawns go out with this tick, before the snapshot.
            routed.extend(self.sim.drain_events_routed());
        }
        self.route(&routed);
        if self.snapshot_due || self.sim.now().0.is_multiple_of(SNAPSHOT_EVERY_TICKS) {
            self.snapshot_due = false;
            if let Some(s) = self.snapshot() {
                self.pending.push((Audience::All, s));
            }
        }
        std::mem::take(&mut self.pending)
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
                    Some(s) if s.is_finite() => m.budget += s.max(0.0),
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

    /// Any living member standing in the trigger of an open exit takes the whole group along.
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
        let mut to = None;
        for m in self.members.values_mut() {
            let alive = self.sim.unit(m.eid).is_some_and(|u| u.alive);
            match exits.iter().find(|(t, _, _)| in_xz(t, m.pos)) {
                None => m.armed = true,
                Some((_, true, dest)) if m.armed && alive => to = Some(dest.clone()),
                Some(_) => {}
            }
        }
        to.is_some_and(|to| self.travel(&zone, &to))
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
            .map(PlayerSnap::from_state)
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
        assert!(s.on_join(0, "Dana"));
        assert!(s.on_join(1, "<bad>"));
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

    #[tokio::test(start_paused = true)]
    async fn sixth_player_is_refused_by_the_sim() {
        let mut s = room();
        for p in 0..5 {
            assert!(s.on_join(p, "x"));
        }
        assert!(!s.on_join(5, "x"));
    }

    #[tokio::test(start_paused = true)]
    async fn moves_are_checked() {
        let mut s = room();
        s.on_join(0, "a");
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

    #[tokio::test(start_paused = true)]
    async fn walking_into_an_open_door_moves_the_group() {
        let mut s = room();
        s.on_join(0, "a");
        s.on_join(1, "b");
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
        // Test shortcut: place the player as if it had walked there.
        s.members.get_mut(&0).unwrap().pos = centre;
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
    async fn locked_door_does_nothing() {
        let mut s = room();
        s.on_join(0, "a");
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
        s.on_join(0, "a");
        s.on_join(1, "b");
        s.tick();
        s.on_message(1, ClientMsg::Emote(Emote::Lgtm));
        s.on_message(0, ClientMsg::Ping { t: 7 });
        let eid1 = eid_of(&s, 1);
        s.on_leave(1);
        assert!(s.sim.unit(eid1).is_none());
        let a = msgs(&s.tick(), 0);
        assert!(a.contains(&ServerMsg::Emoted {
            player: eid1,
            emote: Emote::Lgtm
        }));
        assert!(a.contains(&ServerMsg::Pong { t: 7 }));
        assert!(a.contains(&ServerMsg::PlayerLeft { id: eid1 }));
    }
}
