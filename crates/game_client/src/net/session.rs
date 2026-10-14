//! Protocol session on top of the transport: handshake, keep-alive, server messages into
//! `NetWorld`, and the switches between offline and online play.
//!
//! Handshake: connect to `/ws`, send `Hello`, then `CreateRoom` or `JoinRoom`. The server
//! answers `RoomCreated` (create only) and `Welcome`, or `Error` and closes.
//!
//! Going online freezes the offline sim (it is not stepped and not shown); dropping out of
//! the room relinks the player to it and reloads its zone, so solo progress survives a
//! co-op session untouched. Online progress is the server's and is not kept.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use super::lobby::{Intent, Lobby};
use super::transport::{NetClient, NetEvent, NetState};
use super::world::NetWorld;
use crate::collision::to_core;
use crate::combat::{CombatSim, PendingCommands, SimUnit, UiNotice};
use crate::game::{EnterZone, GameRestarted};
use crate::hud::LogLineMsg;
use crate::player::{Player, PlayerBody};
use crate::targeting::Target;
use bevy::ecs::system::SystemParam;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use combat_core::{Command, EntityId, Event};
use net_proto::{
    ClientMsg, Emote, ErrorKind, MAX_RAW_NAME_BYTES, PROTOCOL_VERSION, ServerMsg, decode, encode,
    is_player_command, parse_room_code,
};
use std::sync::OnceLock;

pub const PING_SECS: f32 = 5.0;
pub const LOST_NOTICE: &str = "Connection lost. You are now working offline. As usual.";
pub const LEFT_NOTICE: &str = "You left the room. Back to working offline.";
/// Movement reports go out every this many 30 Hz ticks (15 Hz).
const MOVE_EVERY_TICKS: u32 = 2;
const LOG_GREY: Color = Color::srgb(0.6, 0.66, 0.76);

#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub enum GameMode {
    #[default]
    Offline,
    Online {
        you: EntityId,
        room: String,
    },
}

impl GameMode {
    pub fn is_online(&self) -> bool {
        matches!(self, GameMode::Online { .. })
    }
}

/// The player asked to leave the room (lobby button).
#[derive(Message)]
pub struct LeaveRequest;

// ---- pure state machine ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Handshake,
    InRoom,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Opened,
    Welcome,
    Error(ErrorKind),
    Closed,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    SendHello,
    EnterRoom,
    ShowError(&'static str),
    /// The room connection dropped: back to offline with a notice.
    Lost,
    /// The handshake never completed; the lobby already shows why.
    Failed,
}

/// A room code the server sent, accepted only if it parses.
fn checked_code(code: &str) -> Option<String> {
    parse_room_code(code).ok()
}

pub fn advance(phase: Phase, step: &Step) -> (Phase, Outcome) {
    match (phase, step) {
        (Phase::Idle, Step::Opened) => (Phase::Handshake, Outcome::SendHello),
        (Phase::Handshake, Step::Welcome) => (Phase::InRoom, Outcome::EnterRoom),
        (p, Step::Error(k)) => (p, Outcome::ShowError(k.message())),
        (Phase::InRoom, Step::Closed) => (Phase::Idle, Outcome::Lost),
        (Phase::Handshake, Step::Closed) => (Phase::Idle, Outcome::Failed),
        (p, _) => (p, Outcome::Nothing),
    }
}

/// `Hello` plus the room request. The name is cut to what `decode` accepts; the server
/// sanitises the rest and checks the class.
pub fn hello_frames(intent: Intent, name: &str, room: &str, class: &str) -> Vec<ClientMsg> {
    let mut name = name.trim().to_string();
    while name.len() > MAX_RAW_NAME_BYTES {
        name.pop();
    }
    let request = match intent {
        Intent::Create => ClientMsg::CreateRoom,
        Intent::Join => ClientMsg::JoinRoom {
            code: room.to_string(),
        },
    };
    vec![
        ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            name,
            class: class.to_string(),
        },
        request,
    ]
}

/// Seconds since the first call; one clock for snapshot stamps and interpolation.
pub fn clock() -> f64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_secs_f64()
}

pub fn send(client: &NetClient, msg: &ClientMsg) -> bool {
    match encode(msg) {
        Ok(frame) => client.send(frame),
        Err(err) => {
            warn!("cannot encode {msg:?}: {err}");
            false
        }
    }
}

// ---- online tick ----

pub struct MoveInput {
    pub pos: Vec3,
    pub yaw: f32,
    pub moving: bool,
    pub jumping: bool,
}

/// Online replacement for `Sim::tick`: reports this tick's movement, target and commands to
/// the server and returns the server events received since the last tick.
pub fn online_step(
    world: &mut NetWorld,
    client: &NetClient,
    input: MoveInput,
    wanted_target: Option<EntityId>,
    pending: &mut Vec<Command>,
) -> Vec<Event> {
    world.advance(clock());
    world.move_ticks = world.move_ticks.wrapping_add(1);
    let pos = to_core(input.pos);
    let report = (pos, input.yaw, input.moving, input.jumping);
    let changed = world.last_move.is_none_or(|last| last != report);
    if changed && world.move_ticks.is_multiple_of(MOVE_EVERY_TICKS) {
        let seq = world.moves.record(pos);
        world.last_move = Some(report);
        send(
            client,
            &ClientMsg::Move {
                pos,
                facing: input.yaw,
                moving: input.moving,
                jumping: input.jumping,
                seq,
            },
        );
    }
    if world.target_sent != Some(wanted_target) {
        world.target_sent = Some(wanted_target);
        send(
            client,
            &ClientMsg::Cmd(Command::SetTarget {
                unit: world.you,
                target: wanted_target,
            }),
        );
    }
    for cmd in pending.drain(..) {
        if let Command::Cast { spell, .. } = cmd {
            world.predict_cast(spell, input.moving || input.jumping, wanted_target, clock());
        }
        if is_player_command(&cmd) {
            send(client, &ClientMsg::Cmd(cmd));
        }
    }
    world.take_inbox()
}

// ---- Bevy side ----

#[derive(Resource, Default)]
pub struct Session {
    pub phase: Phase,
    ping: f32,
}

/// Everything a mode switch touches.
#[derive(SystemParam)]
pub struct Switch<'w, 's> {
    commands: Commands<'w, 's>,
    mode: ResMut<'w, GameMode>,
    target: ResMut<'w, Target>,
    pending: ResMut<'w, PendingCommands>,
    enter: MessageWriter<'w, EnterZone>,
    restarted: MessageWriter<'w, GameRestarted>,
    notices: MessageWriter<'w, UiNotice>,
    player: Single<'w, 's, (Entity, &'static PlayerBody), With<Player>>,
    visuals: Query<'w, 's, Entity, (With<SimUnit>, Without<Player>)>,
    class: Res<'w, crate::classes::PlayerClass>,
}

impl Switch<'_, '_> {
    /// Ids of the old world must not leak into the new one: visuals, target and HUD state go.
    fn reset(&mut self, player_id: EntityId, zone: String) {
        let (entity, _) = *self.player;
        self.commands.entity(entity).insert(SimUnit(player_id));
        for e in &self.visuals {
            self.commands.entity(e).try_despawn();
        }
        self.target.0 = None;
        self.pending.0.clear();
        self.restarted.write(GameRestarted);
        self.enter.write(EnterZone { zone, from: None });
    }

    fn go_online(&mut self, you: EntityId, room: String, zone: Option<String>) {
        *self.mode = GameMode::Online { you, room };
        self.reset(you, zone.unwrap_or_else(|| "campus".into()));
    }

    fn go_offline(&mut self, sim: &CombatSim, notice: &str) {
        self.commands.remove_resource::<NetWorld>();
        *self.mode = GameMode::Offline;
        let id = sim.0.player().unwrap_or_default();
        let zone = sim.0.zone().unwrap_or("campus").to_string();
        self.reset(id, zone);
        self.notices.write(UiNotice(notice.into()));
    }
}

pub fn run_session(
    time: Res<Time>,
    mut events: MessageReader<NetEvent>,
    mut leave: MessageReader<LeaveRequest>,
    mut client: ResMut<NetClient>,
    mut session: ResMut<Session>,
    mut lobby: ResMut<Lobby>,
    sim: Res<CombatSim>,
    mut net: Option<ResMut<NetWorld>>,
    mut log: MessageWriter<LogLineMsg>,
    mut switch: Switch,
) {
    let mut fresh: Option<NetWorld> = None;
    let body_pos = to_core(switch.player.1.pos);
    for ev in events.read() {
        let step = match ev {
            NetEvent::Opened => Step::Opened,
            NetEvent::Closed { .. } | NetEvent::Error(_) => Step::Closed,
            NetEvent::Frame(bytes) => {
                let msg = match decode::<ServerMsg>(bytes) {
                    Ok(m) => m,
                    Err(err) => {
                        warn!("dropping server frame: {err}");
                        continue;
                    }
                };
                match msg {
                    ServerMsg::Welcome {
                        you,
                        room,
                        zone,
                        players,
                        quests,
                        cleared,
                        unlocked,
                        ..
                    } => {
                        let (phase, outcome) = advance(session.phase, &Step::Welcome);
                        session.phase = phase;
                        if outcome == Outcome::EnterRoom {
                            // Ping at once: cast bars use the round trip from the first second.
                            session.ping = PING_SECS;
                            info!("joined room as {you}, {} player(s)", players.len());
                            let mut world =
                                NetWorld::new(sim.0.content().clone(), you, room.clone(), &players);
                            world.apply_welcome(&quests, &cleared, &unlocked);
                            if let Some(code) = checked_code(&room) {
                                lobby.room = code;
                            }
                            switch.go_online(you, room, zone);
                            fresh = Some(world);
                        }
                        continue;
                    }
                    ServerMsg::Error { kind, .. } => Step::Error(kind),
                    ServerMsg::RoomCreated { code } => {
                        // The code ends up in the clipboard; only a well-formed one may.
                        if let Some(code) = checked_code(&code) {
                            info!("room created: {code}");
                            lobby.room = code;
                        }
                        continue;
                    }
                    other => {
                        if let Some(world) = fresh.as_mut().or(net.as_deref_mut()) {
                            apply(world, other, body_pos, &mut log);
                        }
                        continue;
                    }
                }
            }
        };
        let (phase, outcome) = advance(session.phase, &step);
        session.phase = phase;
        match outcome {
            Outcome::SendHello => {
                let room = lobby.room.clone();
                for msg in hello_frames(
                    lobby.intent.unwrap_or(Intent::Create),
                    lobby.display_name(),
                    &room,
                    &switch.class.0,
                ) {
                    send(&client, &msg);
                }
            }
            Outcome::ShowError(text) => {
                lobby.set_notice(text);
                if phase == Phase::InRoom {
                    switch.notices.write(UiNotice(text.into()));
                }
            }
            Outcome::Lost => {
                lobby.set_notice(LOST_NOTICE);
                switch.go_offline(&sim, LOST_NOTICE);
                fresh = None;
            }
            Outcome::Nothing | Outcome::EnterRoom | Outcome::Failed => {}
        }
    }
    if leave.read().count() > 0 && session.phase != Phase::Idle {
        send(&client, &ClientMsg::LeaveRoom);
        client.disconnect();
        if session.phase == Phase::InRoom {
            switch.go_offline(&sim, LEFT_NOTICE);
            fresh = None;
        }
        lobby.set_notice(LEFT_NOTICE);
        session.phase = Phase::Idle;
    }
    // A socket that died before opening never produces Closed through the handshake path.
    if session.phase == Phase::Handshake && matches!(client.state(), NetState::Failed { .. }) {
        session.phase = Phase::Idle;
    }
    if session.phase != Phase::Idle && *client.state() == NetState::Connected {
        session.ping += time.delta_secs();
        if session.ping >= PING_SECS {
            session.ping = 0.0;
            send(
                &client,
                &ClientMsg::Ping {
                    t: (clock() * 1000.0) as u64,
                },
            );
        }
    }
    if let Some(world) = fresh {
        switch.commands.insert_resource(world);
    }
}

/// In-room messages other than the handshake.
fn apply(
    world: &mut NetWorld,
    msg: ServerMsg,
    body_pos: combat_core::Vec3,
    log: &mut MessageWriter<LogLineMsg>,
) {
    match msg {
        ServerMsg::Events { tick, events } => world.receive_events(tick, events, clock()),
        ServerMsg::Snapshot {
            tick,
            units,
            players,
        } => world.apply_snapshot(tick, &units, &players, clock()),
        ServerMsg::Positions { tick, units } => world.apply_positions(tick, &units, clock()),
        ServerMsg::Auras { units } => world.apply_auras(&units),
        ServerMsg::Cooldowns { tick, gcd, spells } => world.apply_cooldowns(tick, gcd, &spells),
        ServerMsg::MoveCorrection { seq, pos } => world.correct_move(seq, pos, body_pos),
        ServerMsg::Emoted {
            player,
            emote,
            target,
        } => {
            let text = emote_line(world, emote, target);
            world.poses.push((player, emote));
            world.push_presentation(Event::Yell { unit: player, text });
        }
        ServerMsg::PlayerJoined(p) => {
            world.add_player(&p);
            let name = world.name_of(p.id).unwrap_or("A colleague");
            log.write(LogLineMsg(format!("{name} joined the room."), LOG_GREY));
        }
        ServerMsg::PlayerLeft { id } => {
            let name = world.name_of(id).unwrap_or("A colleague").to_string();
            log.write(LogLineMsg(
                format!("{name} left the room. Probably a meeting."),
                LOG_GREY,
            ));
            world.remove_player(id);
        }
        ServerMsg::Pong { t } => world.record_pong(t, clock()),
        ServerMsg::Welcome { .. } | ServerMsg::RoomCreated { .. } | ServerMsg::Error { .. } => {}
    }
}

/// "Let's sync on this, Alice." when aimed at a known member, the plain phrase otherwise.
pub fn emote_line(world: &NetWorld, emote: Emote, target: Option<EntityId>) -> String {
    match target.and_then(|t| world.name_of(t)) {
        Some(name) => emote.text_at(name),
        None => emote.text().into(),
    }
}

/// `WOW_ONLINE=<ws url>` connects at startup and creates a room; with `WOW_JOIN=<code>` it
/// joins that room instead (default address `ws://127.0.0.1:7777/ws`). `WOW_NAME` sets the
/// display name (the server sanitises it like any other).
pub fn auto_connect(mut lobby: ResMut<Lobby>, mut client: ResMut<NetClient>) {
    let online = std::env::var("WOW_ONLINE").ok();
    let join = std::env::var("WOW_JOIN").ok();
    if online.is_none() && join.is_none() {
        return;
    }
    let address = online.unwrap_or_else(|| "ws://127.0.0.1:7777/ws".into());
    let url = match super::address::normalize(&address, false) {
        Ok(u) => u,
        Err(e) => {
            warn!("WOW_ONLINE: {}", e.message());
            return;
        }
    };
    lobby.address = address;
    if let Ok(name) = std::env::var("WOW_NAME") {
        lobby.name = name;
    }
    lobby.intent = Some(match join {
        Some(code) => {
            lobby.room = super::address::Field::Room.sanitize(&code);
            Intent::Join
        }
        None => Intent::Create,
    });
    client.connect(&url);
}

#[cfg(test)]
mod tests {
    #[test]
    fn server_room_codes_must_parse_before_use() {
        let good = net_proto::room_code_from_bytes(&[3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(super::checked_code(&good), Some(good));
        for bad in ["", "rm -rf /", "MERGE-CONFLICT-7K3M9QZ\n", &"A".repeat(500)] {
            assert_eq!(super::checked_code(bad), None, "{bad:?}");
        }
    }

    use super::*;

    #[test]
    fn handshake_then_room_then_loss() {
        let (p, o) = advance(Phase::Idle, &Step::Opened);
        assert_eq!((p, o), (Phase::Handshake, Outcome::SendHello));
        let (p, o) = advance(p, &Step::Welcome);
        assert_eq!((p, o), (Phase::InRoom, Outcome::EnterRoom));
        // A second Welcome changes nothing.
        assert_eq!(
            advance(p, &Step::Welcome),
            (Phase::InRoom, Outcome::Nothing)
        );
        assert_eq!(advance(p, &Step::Closed), (Phase::Idle, Outcome::Lost));
    }

    #[test]
    fn errors_show_canonical_text_and_failed_handshake_is_quiet() {
        let (p, o) = advance(Phase::Handshake, &Step::Error(ErrorKind::CannotJoin));
        assert_eq!(p, Phase::Handshake);
        assert_eq!(o, Outcome::ShowError("Cannot join that room."));
        assert_eq!(advance(p, &Step::Closed), (Phase::Idle, Outcome::Failed));
        assert_eq!(
            advance(Phase::Idle, &Step::Closed),
            (Phase::Idle, Outcome::Nothing)
        );
        assert_eq!(
            advance(Phase::Idle, &Step::Welcome),
            (Phase::Idle, Outcome::Nothing)
        );
    }

    #[test]
    fn hello_frames_follow_the_intent_and_cap_the_name() {
        let create = hello_frames(Intent::Create, "  Dana ", "", "devops");
        assert_eq!(
            create,
            vec![
                ClientMsg::Hello {
                    version: PROTOCOL_VERSION,
                    name: "Dana".into(),
                    class: "devops".into(),
                },
                ClientMsg::CreateRoom
            ]
        );
        let join = hello_frames(
            Intent::Join,
            &"x".repeat(100),
            "SCOPE-CREEP-7K3M9QZA",
            "fullstack",
        );
        let ClientMsg::Hello { name, .. } = &join[0] else {
            panic!("no hello")
        };
        assert_eq!(name.len(), MAX_RAW_NAME_BYTES);
        assert_eq!(
            join[1],
            ClientMsg::JoinRoom {
                code: "SCOPE-CREEP-7K3M9QZA".into()
            }
        );
        // Every frame survives the server's decode.
        for m in join.iter().chain(&create) {
            let Ok(bytes) = encode(m) else {
                panic!("encode")
            };
            assert_eq!(decode::<ClientMsg>(&bytes).as_ref(), Ok(m));
        }
    }
}
