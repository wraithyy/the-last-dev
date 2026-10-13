//! Room registry and per-room actor task.
//!
//! Each room is one tokio task that owns its `RoomState`; everything else talks to it through a
//! bounded command channel. The registry only keeps counts so that limits are enforced atomically
//! under one lock, without awaiting while holding it.

use std::collections::{HashMap, VecDeque};
use std::hash::{BuildHasher, Hasher, RandomState};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::body::Bytes;
use combat_core::MAX_PLAYERS;
use net_proto::ClientMsg;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, interval};

use crate::room_state::{Audience, PlayerId, RoomState, cannot_join_frame};
use crate::world::World;

/// Produces a candidate room code (`random_room_code`; tests may inject their own).
/// `None` means the randomness source failed; the caller reports the room as unavailable.
pub type CodeGen = fn() -> Option<String>;

pub const CANNOT_JOIN: &str = "cannot join";
/// Recent tick durations kept for the p99 in `/healthz`.
const TICK_SAMPLES: usize = 4096;

pub const TICK_HZ: u64 = 30;
const CMD_QUEUE: usize = 1024;

/// Short, per-process-keyed hash of a room code for logs. Codes are join credentials, so they
/// must not appear in logs; the key stops anyone brute-forcing a code back out of a log line.
pub fn code_tag(code: &str) -> String {
    static KEY: OnceLock<RandomState> = OnceLock::new();
    let mut h = KEY.get_or_init(RandomState::new).build_hasher();
    h.write(code.as_bytes());
    format!("{:08x}", h.finish() >> 32)
}

/// Result of queueing a client message to its room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendResult {
    Queued,
    /// The room's command queue is saturated: the sender is flooding.
    Full,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinError {
    NoSuchRoom,
    RoomFull,
    TooManyRooms,
    /// Room task is shutting down or its command queue is saturated.
    Unavailable,
}

impl JoinError {
    /// What the client is told. Unknown, full and unavailable rooms look identical so the
    /// response cannot be used to probe which codes exist.
    pub fn reason(self) -> &'static str {
        match self {
            Self::NoSuchRoom | Self::RoomFull | Self::Unavailable => CANNOT_JOIN,
            Self::TooManyRooms => "server is at its room limit",
        }
    }
}

enum RoomCmd {
    Join {
        player: PlayerId,
        name: String,
        out: mpsc::Sender<Bytes>,
    },
    Leave(PlayerId),
    Msg(PlayerId, ClientMsg),
}

struct Entry {
    players: usize,
    next_player: PlayerId,
    tx: mpsc::Sender<RoomCmd>,
}

struct Inner {
    max_rooms: usize,
    max_players: usize,
    outbound_queue: usize,
    empty_ttl: Duration,
    unjoined_ttl: Duration,
    code_gen: CodeGen,
    world: Arc<World>,
    rooms: Mutex<HashMap<String, Entry>>,
    shutdown: watch::Sender<bool>,
    /// Ring of tick durations in microseconds, across all rooms.
    ticks_us: Mutex<VecDeque<u32>>,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Entry>> {
        // The map holds plain counters; a panic elsewhere cannot leave it inconsistent.
        self.rooms.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn record_tick(&self, us: u32) {
        let mut t = self.ticks_us.lock().unwrap_or_else(PoisonError::into_inner);
        if t.len() == TICK_SAMPLES {
            t.pop_front();
        }
        t.push_back(us);
    }
}

#[derive(Clone)]
pub struct RoomRegistry(Arc<Inner>);

/// Everything a connection needs after a successful join.
pub struct Joined {
    pub code: String,
    pub player: PlayerId,
    pub outbound: mpsc::Receiver<Bytes>,
    tx: mpsc::Sender<RoomCmd>,
}

impl Joined {
    /// Queues a client message without waiting: a flooding client must not stall its own
    /// connection task at the room's pace, it gets kicked instead.
    pub fn send(&self, msg: ClientMsg) -> SendResult {
        match self.tx.try_send(RoomCmd::Msg(self.player, msg)) {
            Ok(()) => SendResult::Queued,
            Err(mpsc::error::TrySendError::Full(_)) => SendResult::Full,
            Err(mpsc::error::TrySendError::Closed(_)) => SendResult::Closed,
        }
    }
}

impl RoomRegistry {
    pub fn new(cfg: &crate::config::Config, code_gen: CodeGen, world: Arc<World>) -> Self {
        Self(Arc::new(Inner {
            max_rooms: cfg.max_rooms,
            // The sim's group cap: past it a join would be admitted and then refused.
            max_players: cfg.max_players_per_room.min(MAX_PLAYERS),
            outbound_queue: cfg.outbound_queue,
            empty_ttl: cfg.empty_room_ttl,
            unjoined_ttl: cfg.unjoined_room_ttl,
            code_gen,
            world,
            rooms: Mutex::new(HashMap::new()),
            shutdown: watch::channel(false).0,
            ticks_us: Mutex::new(VecDeque::with_capacity(TICK_SAMPLES)),
        }))
    }

    /// Creates a fresh room and joins it as its first player.
    pub fn create_and_join(&self, name: &str) -> Result<Joined, JoinError> {
        let inner = &self.0;
        let mut rooms = inner.lock();
        if rooms.len() >= inner.max_rooms {
            return Err(JoinError::TooManyRooms);
        }
        // A handful of retries makes a collision (or a hostile generator) a non-event.
        let code = (0..10)
            .filter_map(|_| (inner.code_gen)())
            .find(|c| !rooms.contains_key(c))
            .ok_or(JoinError::Unavailable)?;
        let (tx, rx) = mpsc::channel(CMD_QUEUE);
        rooms.insert(
            code.clone(),
            Entry {
                players: 0,
                next_player: 0,
                tx,
            },
        );
        let joined = join_locked(inner, &mut rooms, &code, name);
        match joined {
            Ok(j) => {
                tokio::spawn(run_room(code.clone(), inner.clone(), rx));
                tracing::info!(room = %code_tag(&code), rooms = rooms.len(), "room created");
                Ok(j)
            }
            Err(e) => {
                rooms.remove(&code);
                Err(e)
            }
        }
    }

    pub fn join(&self, code: &str, name: &str) -> Result<Joined, JoinError> {
        let mut rooms = self.0.lock();
        join_locked(&self.0, &mut rooms, code, name)
    }

    pub async fn leave(&self, joined: &Joined) {
        {
            let mut rooms = self.0.lock();
            if let Some(e) = rooms.get_mut(&joined.code) {
                e.players = e.players.saturating_sub(1);
            }
        }
        // Err means the room already ended, which is fine.
        let _ = joined.tx.send(RoomCmd::Leave(joined.player)).await;
    }

    /// (rooms, players)
    pub fn counts(&self) -> (usize, usize) {
        let rooms = self.0.lock();
        (rooms.len(), rooms.values().map(|e| e.players).sum())
    }

    /// 99th percentile of recent room tick durations, in microseconds.
    pub fn tick_p99_us(&self) -> u32 {
        let t = self
            .0
            .ticks_us
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut v: Vec<u32> = t.iter().copied().collect();
        drop(t);
        v.sort_unstable();
        v.get(v.len() * 99 / 100).copied().unwrap_or(0)
    }

    /// Stops all room tasks; their players' outbound channels close.
    pub fn shutdown(&self) {
        self.0.shutdown.send_replace(true);
    }
}

fn join_locked(
    inner: &Inner,
    rooms: &mut HashMap<String, Entry>,
    code: &str,
    name: &str,
) -> Result<Joined, JoinError> {
    let e = rooms.get_mut(code).ok_or(JoinError::NoSuchRoom)?;
    if e.players >= inner.max_players {
        return Err(JoinError::RoomFull);
    }
    let (out, outbound) = mpsc::channel(inner.outbound_queue);
    let player = e.next_player;
    let name = name.to_string();
    e.tx.try_send(RoomCmd::Join { player, name, out })
        .map_err(|_| JoinError::Unavailable)?;
    e.next_player += 1;
    e.players += 1;
    tracing::info!(room = %code_tag(code), player, players = e.players, "player joined");
    Ok(Joined {
        code: code.to_string(),
        player,
        outbound,
        tx: e.tx.clone(),
    })
}

async fn run_room(code: String, inner: Arc<Inner>, mut rx: mpsc::Receiver<RoomCmd>) {
    let tag = code_tag(&code);
    let mut state = RoomState::new(code.clone(), inner.world.clone());
    let mut joins = 0u32;
    let mut players: HashMap<PlayerId, mpsc::Sender<Bytes>> = HashMap::new();
    let mut empty_since = Some(Instant::now());
    let mut shutdown = inner.shutdown.subscribe();
    let mut ticker = interval(Duration::from_micros(1_000_000 / TICK_HZ));
    // After a stall, skip missed ticks instead of bursting to catch up.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            cmd = rx.recv() => match cmd {
                None => break,
                Some(RoomCmd::Join { player, name, out }) => {
                    if !state.on_join(player, &name) {
                        // Dropping `out` closes the connection after the error frame.
                        if let Some(b) = cannot_join_frame() {
                            let _ = out.try_send(b);
                        }
                        continue;
                    }
                    players.insert(player, out);
                    joins += 1;
                    empty_since = None;
                }
                Some(RoomCmd::Leave(player)) => {
                    if players.remove(&player).is_some() {
                        state.on_leave(player);
                    }
                    tracing::info!(room = %tag, player, "player left");
                    if players.is_empty() {
                        empty_since = Some(Instant::now());
                    }
                }
                Some(RoomCmd::Msg(player, bytes)) => state.on_message(player, bytes),
            },
            _ = ticker.tick() => {
                let started = std::time::Instant::now();
                let out = state.tick();
                if !players.is_empty() {
                    inner.record_tick(u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX));
                }
                dispatch(&tag, &mut players, &mut state, out);
                // A room nobody ever joined is most likely creation spam; reap it quickly.
                let ttl = if joins > 1 { inner.empty_ttl } else { inner.unjoined_ttl };
                if empty_since.is_some_and(|t| t.elapsed() >= ttl) && remove_if_empty(&inner, &code) {
                    tracing::info!(room = %tag, "room destroyed (empty)");
                    break;
                }
            }
            _ = shutdown.wait_for(|v| *v) => {
                tracing::info!(room = %tag, "room closed (shutdown)");
                break;
            }
        }
    }
    // Dropping `players` closes every outbound channel, which makes connections close.
}

/// Decided under the registry lock so a concurrent join cannot land in a dying room.
fn remove_if_empty(inner: &Inner, code: &str) -> bool {
    let mut rooms = inner.lock();
    if rooms.get(code).is_some_and(|e| e.players == 0) {
        rooms.remove(code);
        true
    } else {
        false
    }
}

fn dispatch(
    tag: &str,
    players: &mut HashMap<PlayerId, mpsc::Sender<Bytes>>,
    state: &mut RoomState,
    out: Vec<(Audience, Bytes)>,
) {
    let mut dead = Vec::new();
    for (aud, bytes) in out {
        for (&id, tx) in players.iter() {
            let wanted = match aud {
                Audience::All => true,
                Audience::Only(p) => p == id,
            };
            if wanted && tx.try_send(bytes.clone()).is_err() {
                dead.push(id);
            }
        }
    }
    for id in dead {
        // Full queue means a slow client; closed means it is already leaving.
        if players.remove(&id).is_some() {
            // The later Leave command finds the player gone, so on_leave must run here or the
            // game state would keep a ghost.
            state.on_leave(id);
            tracing::warn!(room = %tag, player = id, "dropping client with full outbound queue");
        }
    }
}

/// `WORD-WORD-XXXXXXXX` from 10 OS-random bytes; the 40-bit suffix is the join credential.
pub fn random_room_code() -> Option<String> {
    let mut b = [0u8; 10];
    getrandom::fill(&mut b).ok()?;
    Some(net_proto::room_code_from_bytes(&b))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::Config;
    use net_proto::{Emote, ServerMsg, decode, parse_room_code};

    fn cfg(rooms: usize, players: usize) -> Config {
        Config {
            max_rooms: rooms,
            max_players_per_room: players,
            ..Config::default()
        }
    }

    fn reg(c: &Config) -> RoomRegistry {
        RoomRegistry::new(c, random_room_code, World::embedded().unwrap())
    }

    async fn settle() {
        // Lets the room task process queued commands under paused time.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    fn drain(j: &mut Joined) -> Vec<ServerMsg> {
        std::iter::from_fn(|| j.outbound.try_recv().ok())
            .map(|b| decode::<ServerMsg>(&b).unwrap())
            .collect()
    }

    #[test]
    fn code_shape() {
        let c = random_room_code().unwrap();
        assert_eq!(parse_room_code(&c), Ok(c.clone()));
        assert_ne!(c, random_room_code().unwrap());
    }

    #[test]
    fn code_tag_hides_code() {
        let t = code_tag("MERGE-HOTFIX-ABCDEFGH");
        assert_eq!(t.len(), 8);
        assert!(!t.contains("MERGE"));
        assert_eq!(t, code_tag("MERGE-HOTFIX-ABCDEFGH"));
        assert_ne!(t, code_tag("MERGE-HOTFIX-ABCDEFGJ"));
    }

    #[test]
    fn join_failures_look_identical() {
        assert_eq!(JoinError::NoSuchRoom.reason(), JoinError::RoomFull.reason());
        assert_eq!(
            JoinError::NoSuchRoom.reason(),
            JoinError::Unavailable.reason()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn create_join_full_leave() {
        let reg = reg(&cfg(2, 2));
        let a = reg.create_and_join("a").unwrap();
        let b = reg.join(&a.code, "b").unwrap();
        assert_ne!(a.player, b.player);
        assert_eq!(reg.join(&a.code, "c").err(), Some(JoinError::RoomFull));
        assert_eq!(reg.join("NOPE", "c").err(), Some(JoinError::NoSuchRoom));
        assert_eq!(reg.counts(), (1, 2));
        reg.leave(&b).await;
        assert_eq!(reg.counts(), (1, 1));
        assert!(reg.join(&a.code, "c").is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn player_cap_is_the_sims() {
        let reg = reg(&cfg(2, 50));
        let a = reg.create_and_join("a").unwrap();
        let rest: Vec<_> = (0..MAX_PLAYERS - 1)
            .map(|_| reg.join(&a.code, "x").unwrap())
            .collect();
        assert_eq!(reg.join(&a.code, "x").err(), Some(JoinError::RoomFull));
        drop(rest);
    }

    #[tokio::test(start_paused = true)]
    async fn room_limit() {
        let reg = reg(&cfg(1, 5));
        let _a = reg.create_and_join("a").unwrap();
        assert_eq!(
            reg.create_and_join("b").err(),
            Some(JoinError::TooManyRooms)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn lonely_room_expires_after_a_minute() {
        let reg = reg(&cfg(5, 5));
        let a = reg.create_and_join("a").unwrap();
        settle().await;
        reg.leave(&a).await;
        tokio::time::sleep(Duration::from_secs(55)).await;
        assert_eq!(reg.counts().0, 1, "still alive before unjoined ttl");
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(reg.counts().0, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn empty_room_expires_after_ttl() {
        let reg = reg(&cfg(5, 5));
        let a = reg.create_and_join("a").unwrap();
        let b = reg.join(&a.code, "b").unwrap();
        let code = a.code.clone();
        settle().await;
        reg.leave(&a).await;
        reg.leave(&b).await;
        tokio::time::sleep(Duration::from_secs(599)).await;
        assert_eq!(reg.counts().0, 1, "still alive before ttl");
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(reg.counts().0, 0);
        assert_eq!(reg.join(&code, "c").err(), Some(JoinError::NoSuchRoom));
    }

    #[tokio::test(start_paused = true)]
    async fn rejoin_resets_expiry() {
        let reg = reg(&cfg(5, 5));
        let a = reg.create_and_join("a").unwrap();
        settle().await;
        reg.leave(&a).await;
        tokio::time::sleep(Duration::from_secs(50)).await;
        let b = reg.join(&a.code, "b").unwrap();
        tokio::time::sleep(Duration::from_secs(700)).await;
        assert_eq!(reg.counts(), (1, 1));
        drop(b);
    }

    #[tokio::test(start_paused = true)]
    async fn messages_reach_the_room() {
        let reg = reg(&cfg(5, 5));
        let mut a = reg.create_and_join("a").unwrap();
        let mut b = reg.join(&a.code, "b").unwrap();
        settle().await;
        assert!(matches!(drain(&mut a)[0], ServerMsg::Welcome { .. }));
        assert!(matches!(drain(&mut b)[0], ServerMsg::Welcome { .. }));
        assert_eq!(a.send(ClientMsg::Emote(Emote::ShipIt)), SendResult::Queued);
        settle().await;
        assert!(drain(&mut b).iter().any(|m| matches!(
            m,
            ServerMsg::Emoted {
                emote: Emote::ShipIt,
                ..
            }
        )));
        assert!(reg.tick_p99_us() > 0);
    }

    #[tokio::test(start_paused = true)]
    async fn slow_client_is_dropped() {
        let cfg = Config {
            outbound_queue: 2,
            ..cfg(5, 5)
        };
        let reg = reg(&cfg);
        let a = reg.create_and_join("a").unwrap();
        let mut b = reg.join(&a.code, "b").unwrap(); // never drained
        for _ in 0..5 {
            a.send(ClientMsg::Emote(Emote::Sync));
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
        // Two queued messages remain readable, then the channel reports closed.
        assert!(b.outbound.recv().await.is_some());
        assert!(b.outbound.recv().await.is_some());
        assert!(b.outbound.recv().await.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn dropped_slow_client_triggers_on_leave() {
        let cfg = Config {
            outbound_queue: 5,
            ..cfg(5, 5)
        };
        let reg = reg(&cfg);
        let mut a = reg.create_and_join("a").unwrap();
        let b = reg.join(&a.code, "b").unwrap(); // never drained
        let mut seen = false;
        for _ in 0..10 {
            a.send(ClientMsg::Emote(Emote::Sync));
            tokio::time::sleep(Duration::from_millis(40)).await;
            seen |= drain(&mut a)
                .iter()
                .any(|m| matches!(m, ServerMsg::PlayerLeft { .. }));
        }
        assert!(seen, "on_leave was not called for the dropped client");
        drop(b);
    }

    #[tokio::test(start_paused = true)]
    async fn flooding_sender_gets_full_instead_of_blocking() {
        let reg = reg(&cfg(5, 5));
        let a = reg.create_and_join("a").unwrap();
        // Single-threaded runtime and no await: the room task cannot drain.
        let results: Vec<_> = (0..CMD_QUEUE + 10)
            .map(|_| a.send(ClientMsg::Ping { t: 0 }))
            .collect();
        assert_eq!(results.last(), Some(&SendResult::Full));
        assert_eq!(results[0], SendResult::Queued);
    }
}
