//! The live world on screen: the office clock on the radar, the Daily Standup window
//! (`board.rs`), daily and weekly lines in the objective tracker, Board Meeting warnings in chat,
//! on the map and the radar, and the day/night lighting (`daynight.rs`). The rules live in
//! `combat_core::{live, dailies}`; this side follows `Event::Clock`, `DailyBoard`,
//! `TaskCompleted` and `WorldBoss`, so offline and online read the same way.
//!
//! QA hooks: `WOW_GAME_TIME=HH:MM` starts the offline office clock at that game time (today's
//! date); `WOW_WORLD_BOSS_IN=<secs>` sets the Board Meeting countdown (offline, campus);
//! `WOW_STANDUP=1` joins today's standup at start with a little progress; `WOW_BOARD=<secs>`
//! opens the Scrum board window at that time; `WOW_FREEZE_DAY=1` keeps it day (`DayNight`);
//! `WOW_WORLD_EVENT=<key>@<secs>` starts that campus world event in `secs` (offline).
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod ambient;
mod board;
mod daynight;

pub use daynight::DayNight;

use crate::combat::{CombatEvent, CombatSim, SimRead, SimReady, SimView};
use crate::game::GameFlow;
use crate::hud::{
    ACCENT, ACCENT_WARM, Banner, DIM, Line, LogLineMsg, label, plural, set_text, unit_name,
};
use crate::net::NetWorld;
use bevy::prelude::*;
use combat_core::live::{
    DUSK_MIN, GAME_DAY_S, game_minute, is_night_minute, secs_to_day_reset, secs_to_week_reset,
};
use combat_core::{
    BoardView, Content, EntityId, Event, GoalSpec, GoalView, TaskState, WorldBossState,
    WorldEventPhase,
};

const NIGHT_BLUE: Color = Color::srgb(0.55, 0.65, 1.0);
const BOSS_RED: Color = Color::srgb(1.0, 0.42, 0.36);
const DONE_GREEN: Color = Color::srgb(0.4, 1.0, 0.5);

/// What the HUD knows about the live world, from the events (offline and online alike).
#[derive(Resource, Default)]
pub struct LiveState {
    /// Online: (unix seconds, sim tick) of the last `Event::Clock`.
    clock: Option<(u64, u64)>,
    /// The player's Scrum board as last sent.
    pub board: Option<BoardView>,
    /// Board Meeting countdown: (app seconds when announced, seconds left then).
    boss_warn: Option<(f32, u32)>,
    boss_up: Option<EntityId>,
    night: Option<bool>,
    /// Campus world events warned or running, as last announced.
    pub events: Vec<WorldEventState>,
}

/// A world event as the HUD tracks it: the phase and its countdown from the last event.
#[derive(Clone, Debug)]
pub struct WorldEventState {
    pub key: String,
    pub phase: WorldEventPhase,
    /// App seconds when `secs` was announced.
    at: f32,
    secs: u32,
    pub count: u32,
    pub need: u32,
}

impl WorldEventState {
    /// Seconds left of the warning or the run, counted down locally.
    pub fn left(&self, now: f32) -> u32 {
        self.secs.saturating_sub((now - self.at).max(0.0) as u32)
    }
}

impl LiveState {
    /// Seconds until the Board meets, counted down locally from the last warning.
    pub fn boss_in(&self, now: f32) -> Option<u32> {
        let (at, secs) = self.boss_warn?;
        Some(secs.saturating_sub((now - at).max(0.0) as u32))
    }

    fn joined(&self) -> impl Iterator<Item = &combat_core::TaskView> {
        self.board
            .iter()
            .flat_map(|b| b.tasks.iter())
            .filter(|t| t.state != TaskState::Open)
    }
}

/// Live lines the objective tracker (`hud/quests.rs`) shows above the quests.
#[derive(Resource, Default)]
pub struct LiveTracker {
    /// (title, title colour, goal lines).
    pub entries: Vec<(String, Color, Vec<(String, Line)>)>,
    pub footer: Vec<(String, Color)>,
    /// Changes whenever anything above does; the tracker rebuilds on it.
    pub sig: String,
}

impl LiveTracker {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.footer.is_empty()
    }
}

#[derive(Component)]
struct ClockText;

pub struct LivePlugin;

impl Plugin for LivePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ambient::plugin)
            .init_resource::<LiveState>()
            .init_resource::<LiveTracker>()
            .init_resource::<DayNight>()
            .add_systems(Startup, (spawn_clock, board::spawn_window))
            .add_systems(
                Update,
                (
                    qa_hooks,
                    track_events,
                    clock_text,
                    tracker_lines,
                    board::board_input,
                    board::rebuild_board,
                )
                    .chain()
                    .in_set(SimReady),
            )
            .add_systems(
                Update,
                daynight::light_the_world
                    .after(crate::game::FloorSwap)
                    .in_set(SimReady),
            );
    }
}

/// The device clock (seconds since the epoch, UTC).
fn device_unix_s() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0).max(0.0) as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }
}

/// The offline sim's clock anchor: the device clock, or `WOW_GAME_TIME=HH:MM` moved to that
/// game time on the same date (screenshots of dusk, night and day).
pub fn start_clock() -> u64 {
    let now = device_unix_s();
    let wanted = std::env::var("WOW_GAME_TIME").ok().and_then(|v| {
        let (h, m) = v.trim().split_once(':')?;
        let (h, m): (u64, u64) = (h.parse().ok()?, m.parse().ok()?);
        (h < 24 && m < 60).then_some(h * 60 + m)
    });
    match wanted {
        Some(minute) => {
            let start = now - now % GAME_DAY_S;
            // Stay on today's date: go back a game day rather than forward over midnight.
            let t = start + minute;
            if t / 86_400 == now / 86_400 {
                t
            } else {
                t.saturating_sub(GAME_DAY_S)
            }
        }
        None => now,
    }
}

/// Unix seconds now, with the fraction of the second: the offline sim's clock, or online the
/// last `Event::Clock` advanced by the server tick.
pub fn unix_now(
    sim: &dyn SimRead,
    offline: &CombatSim,
    online: bool,
    st: &LiveState,
) -> Option<f64> {
    if !online {
        return offline.0.clock_on().then(|| offline.0.unix_secs_f64());
    }
    let (at, tick) = st.clock?;
    Some(at as f64 + sim.now().0.saturating_sub(tick) as f64 / f64::from(combat_core::TICK_HZ))
}

fn hhmm(minute: u32) -> String {
    format!("{:02}:{:02}", minute / 60 % 24, minute % 60)
}

fn mmss(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// "7h 12m", "3d 4h", "12m".
pub fn duration(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{}m", m.max(1))
    }
}

fn zone_name(content: &Content, id: &str) -> String {
    crate::game::zone_label(content, id)
}

/// "Delivery Robots defeated in Campus", without the numbers.
pub fn goal_label(content: &Content, g: &GoalSpec) -> String {
    match g {
        GoalSpec::Kill { zone, unit } => {
            let who = unit
                .as_deref()
                .map_or_else(|| "Enemies".to_string(), |u| plural(&unit_name(content, u)));
            match zone {
                Some(z) => format!("{who} defeated in {}", zone_name(content, z)),
                None => format!("{who} defeated"),
            }
        }
        GoalSpec::Elite => "Elites or bosses defeated".into(),
        GoalSpec::Loot => "Things looted from the fallen".into(),
        GoalSpec::Props => "Office equipment used".into(),
        GoalSpec::Output => "Throughput (damage + healing)".into(),
        GoalSpec::Sprint { level } => format!("Sprint finished at level {level}+"),
        GoalSpec::Idle { zone } => {
            format!(
                "Did absolutely nothing in {} (seconds)",
                zone_name(content, zone)
            )
        }
        GoalSpec::Talk { unit } => format!("Synced with {}", unit_name(content, unit)),
        GoalSpec::Night => "Night shift defeated (after 18:00)".into(),
        GoalSpec::WorldBoss => "Survived the Board Meeting".into(),
    }
}

pub fn goal_line(content: &Content, g: &GoalView) -> String {
    format!("{}: {}/{}", goal_label(content, &g.goal), g.count, g.need)
}

pub fn task_name(content: &Content, key: &str, weekly: bool) -> String {
    content
        .task_def(key, weekly)
        .map_or_else(|| key.to_string(), |d| d.name.clone())
}

fn spawn_clock(mut commands: Commands) {
    let (top, width) = crate::hud::MINIMAP_BOX;
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(12),
            top: px(top + 7.0),
            width: px(width),
            justify_content: JustifyContent::Center,
            ..default()
        },
        GlobalZIndex(20),
        children![(ClockText, label("", 12.0), TextColor(ACCENT))],
    ));
}

/// Live events into the HUD state, banners and chat lines; the board window opens on demand.
fn track_events(
    time: Res<Time>,
    sim: SimView,
    net: Option<Res<NetWorld>>,
    mut st: ResMut<LiveState>,
    mut windows: ResMut<crate::input::SprintWindows>,
    mut events: MessageReader<CombatEvent>,
    mut banners: MessageWriter<Banner>,
    mut log: MessageWriter<LogLineMsg>,
    mut restarted: MessageReader<crate::game::GameRestarted>,
    (mut was_online, mut saved_at): (Local<bool>, Local<f32>),
) {
    // A new run or switching between offline and online starts from what the new sim says.
    if restarted.read().count() > 0 || *was_online != net.is_some() {
        *was_online = net.is_some();
        *st = LiveState::default();
    }
    let now = time.elapsed_secs();
    let content = sim.content();
    let me = sim.player();
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::Clock { unix_s } => st.clock = Some((*unix_s, sim.now().0)),
            Event::DailyBoard { board, open } => {
                if *open {
                    windows.standup = true;
                }
                // Progress arrives once a second in a fight; the save only needs it now and then.
                let states = |b: &BoardView| b.tasks.iter().map(|t| t.state).collect::<Vec<_>>();
                let moved = st.board.as_ref().map(states) != Some(states(board));
                if net.is_some() && (*open || moved || now - *saved_at >= 5.0) {
                    *saved_at = now;
                    // Online progress also goes into the local save of the class.
                    if let Some(class) = me
                        .and_then(|p| sim.unit(p))
                        .and_then(|u| content.class_of_unit(&u.key).map(|c| c.key.clone()))
                    {
                        crate::economy::save::record_online_tasks(&class, board);
                    }
                }
                st.board = Some(board.clone());
            }
            Event::TaskCompleted {
                key,
                weekly,
                xp,
                sp,
            } => {
                let name = task_name(content, key, *weekly);
                let (title, subtitle) = if *weekly {
                    (
                        format!("Sprint Goal delivered: {name}"),
                        format!("+{xp} XP, +{sp} Kudos. Velocity has been reported upwards."),
                    )
                } else {
                    (
                        format!("Standup item closed: {name}"),
                        format!("+{xp} XP, +{sp} Kudos. Nobody noticed, which is the goal."),
                    )
                };
                log.write(LogLineMsg(format!("{title} ({subtitle})"), DONE_GREEN));
                banners.write(Banner {
                    title,
                    subtitle,
                    color: DONE_GREEN,
                });
            }
            Event::WorldBoss { state, secs, unit } => match state {
                WorldBossState::Warning => {
                    st.boss_warn = Some((now, *secs));
                    let text = if *secs >= 120 {
                        format!(
                            "[Calendar] The Board meets on the east lawn in {} minutes. Attendance is mandatory. Bring 3-5 colleagues.",
                            secs / 60
                        )
                    } else {
                        "[Calendar] Board Meeting in 1 minute. The agenda is you.".to_string()
                    };
                    log.write(LogLineMsg(text, ACCENT_WARM));
                }
                WorldBossState::Spawned => {
                    st.boss_warn = None;
                    st.boss_up = Some(*unit);
                    banners.write(Banner {
                        title: "Board Meeting".into(),
                        subtitle:
                            "The Board <Quarterly Visionaries> is in session on the east lawn."
                                .into(),
                        color: BOSS_RED,
                    });
                    log.write(LogLineMsg(
                        "[Calendar] Board Meeting started on the east lawn. It leaves in 10 minutes if nobody shows up."
                            .into(),
                        BOSS_RED,
                    ));
                }
                WorldBossState::Cancelled => {
                    st.boss_up = None;
                    log.write(LogLineMsg(
                        "[Calendar] Board Meeting cancelled. It could have been an email.".into(),
                        DIM,
                    ));
                }
                WorldBossState::Defeated => {
                    st.boss_up = None;
                    banners.write(Banner {
                        title: "Meeting adjourned".into(),
                        subtitle: "The Board has been defeated. The minutes will say otherwise."
                            .into(),
                        color: DONE_GREEN,
                    });
                }
            },
            Event::WorldEvent {
                key,
                phase,
                secs,
                count,
                need,
            } => world_event(
                &mut st,
                content,
                now,
                (key, *phase, *secs, *count, *need),
                &mut banners,
                &mut log,
            ),
            Event::ZoneLoaded { .. } => {
                st.boss_up = None;
                st.events.clear();
            }
            _ => {}
        }
    }
}

const EVENT_GOLD: Color = Color::srgb(1.0, 0.82, 0.3);

/// A world event phase into the HUD state, a banner and a chat line.
fn world_event(
    st: &mut LiveState,
    content: &Content,
    now: f32,
    (key, phase, secs, count, need): (&str, WorldEventPhase, u32, u32, u32),
    banners: &mut MessageWriter<Banner>,
    log: &mut MessageWriter<LogLineMsg>,
) {
    let def = content.live.events.iter().find(|e| e.key == key);
    let name = def.map_or(key, |d| d.name.as_str());
    let i = match st.events.iter().position(|e| e.key == key) {
        Some(i) => i,
        None => {
            st.events.push(WorldEventState {
                key: key.to_string(),
                phase,
                at: now,
                secs,
                count,
                need,
            });
            st.events.len() - 1
        }
    };
    let e = &mut st.events[i];
    match phase {
        WorldEventPhase::Progress => {
            e.count = count;
            return;
        }
        WorldEventPhase::Warning | WorldEventPhase::Started => {
            *e = WorldEventState {
                key: key.to_string(),
                phase,
                at: now,
                secs,
                count,
                need,
            };
        }
        WorldEventPhase::Won | WorldEventPhase::Failed => {
            st.events.remove(i);
        }
    }
    match phase {
        WorldEventPhase::Warning => log.write(LogLineMsg(
            format!("[Campus] {name} in {}. Everyone nearby joins.", mmss(secs)),
            EVENT_GOLD,
        )),
        WorldEventPhase::Started => {
            let goal = if need > 0 {
                format!("Stop {need} of them in {} at the marker.", mmss(secs))
            } else {
                format!("Hold out for {} at the marker.", mmss(secs))
            };
            banners.write(Banner {
                title: name.to_string(),
                subtitle: format!("World event started. {goal}"),
                color: EVENT_GOLD,
            });
            log.write(LogLineMsg(
                format!("[Campus] {name} has started. {goal}"),
                EVENT_GOLD,
            ))
        }
        WorldEventPhase::Won => {
            banners.write(Banner {
                title: format!("{name}: done"),
                subtitle: "Everyone who took part gets XP and Kudos.".into(),
                color: DONE_GREEN,
            });
            log.write(LogLineMsg(
                format!("[Campus] {name}: success. Rewards go to everyone who took part."),
                DONE_GREEN,
            ))
        }
        WorldEventPhase::Failed => log.write(LogLineMsg(
            format!("[Campus] {name}: failed. It comes back later."),
            DIM,
        )),
        WorldEventPhase::Progress => return,
    };
}

/// The office clock on the radar, and a chat line when Crunch mode starts or ends.
fn clock_text(
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    flow: Res<GameFlow>,
    mut st: ResMut<LiveState>,
    mut text: Single<(&mut Text, &mut TextColor), With<ClockText>>,
    mut log: MessageWriter<LogLineMsg>,
) {
    let Some(unix) = unix_now(&*sim, &offline, net.is_some(), &st) else {
        set_text(&mut text.0, "");
        return;
    };
    let minute = game_minute(unix as u64);
    let night = is_night_minute(minute);
    let color = if night {
        NIGHT_BLUE
    } else if minute + 60 >= DUSK_MIN {
        ACCENT_WARM
    } else {
        ACCENT
    };
    set_text(&mut text.0, &hhmm(minute));
    if text.1.0 != color {
        text.1.0 = color;
    }
    if st.night != Some(night) {
        let was = st.night.replace(night);
        if was.is_some() && flow.zone == "campus" {
            let line = if night {
                "18:00. Crunch mode: the office empties, the lights dim and the night shift clocks in. Deadline Wraiths reported on the lawn."
            } else {
                "06:00. The night shift clocks out. Somebody left a pizza box on the Scrum board."
            };
            log.write(LogLineMsg(line.into(), NIGHT_BLUE));
        }
    }
}

/// Joined dailies and the Sprint Goal, reset timers and the Board Meeting countdown, as tracker
/// lines (`hud/quests.rs` renders them).
fn tracker_lines(
    time: Res<Time>,
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    flow: Res<GameFlow>,
    st: Res<LiveState>,
    calendar: Res<crate::calendar::CalendarState>,
    mut out: ResMut<LiveTracker>,
) {
    let content = sim.content();
    let mut entries = Vec::new();
    for t in st.joined() {
        if t.state == TaskState::Done {
            continue;
        }
        let name = task_name(content, &t.key, t.weekly);
        let title = if t.weekly {
            format!("Sprint Goal: {}", name.trim_start_matches("Sprint Goal: "))
        } else {
            format!("Standup: {name}")
        };
        let mut lines: Vec<(String, Line)> = t
            .goals
            .iter()
            .map(|g| {
                let state = if g.count >= g.need {
                    Line::Done
                } else {
                    Line::Pending
                };
                (goal_line(content, g), state)
            })
            .collect();
        if t.state == TaskState::Ready {
            lines.push((
                "Claim it at the Scrum board (campus plaza)".into(),
                Line::Ready,
            ));
        }
        let color = if t.weekly { ACCENT_WARM } else { ACCENT };
        entries.push((title, color, lines));
    }
    let mut footer = Vec::new();
    let unix = unix_now(&*sim, &offline, net.is_some(), &st).map(|u| u as u64);
    if let Some(u) = unix.filter(|_| st.board.is_some()) {
        let open = st.joined().any(|t| t.state != TaskState::Done);
        let text = if open {
            format!(
                "Dailies reset in {} (standup is mandatory)",
                duration(secs_to_day_reset(u))
            )
        } else {
            format!(
                "All done. New dailies in {}, Sprint Goal in {}",
                duration(secs_to_day_reset(u)),
                duration(secs_to_week_reset(u))
            )
        };
        footer.push((text, DIM));
    }
    let night_zone = content.live.night.zone.as_str();
    if let Some(m) = unix.map(game_minute).filter(|_| flow.zone == night_zone) {
        if is_night_minute(m) {
            footer.push((
                "Crunch mode until 06:00: the night shift is on the lawn (better XP)".into(),
                NIGHT_BLUE,
            ));
        } else if m + 60 >= DUSK_MIN {
            footer.push((
                format!("Crunch mode at 18:00 (in {} min)", DUSK_MIN - m),
                ACCENT_WARM,
            ));
        }
    }
    let boss_zone = content.world_boss().map(|b| b.zone.as_str());
    if let Some(secs) = st.boss_in(time.elapsed_secs()).filter(|s| *s > 0) {
        if boss_zone == Some(flow.zone.as_str()) {
            footer.push((
                format!("Board Meeting on the east lawn in {}", mmss(secs)),
                BOSS_RED,
            ));
        }
    } else if st
        .boss_up
        .is_some_and(|id| sim.unit(id).is_some_and(|u| u.alive))
    {
        footer.push((
            "Board Meeting in session (east lawn). Group up.".into(),
            BOSS_RED,
        ));
    }
    let now = time.elapsed_secs();
    for e in &st.events {
        let name = content
            .live
            .events
            .iter()
            .find(|d| d.key == e.key)
            .map_or(e.key.as_str(), |d| d.name.as_str());
        if e.phase == WorldEventPhase::Warning {
            footer.push((
                format!("World event: {name} in {}", mmss(e.left(now))),
                EVENT_GOLD,
            ));
            continue;
        }
        let goal = if e.need > 0 {
            let what = content
                .live
                .events
                .iter()
                .find(|d| d.key == e.key)
                .map_or_else(
                    || "Enemies".to_string(),
                    |d| plural(&unit_name(content, &d.wave)),
                );
            let done = if e.count >= e.need {
                Line::Done
            } else {
                Line::Pending
            };
            (format!("{what} stopped {}/{}", e.count, e.need), done)
        } else {
            ("Keep it alive at the marker".to_string(), Line::Pending)
        };
        entries.push((
            format!("World Event: {name}"),
            EVENT_GOLD,
            vec![goal, (format!("{} left", mmss(e.left(now))), Line::Pending)],
        ));
    }
    if let Some(h) = &calendar.hint {
        footer.push((h.clone(), ACCENT));
    }
    let sig = format!("{entries:?}|{footer:?}");
    if sig != out.sig {
        *out = LiveTracker {
            entries,
            footer,
            sig,
        };
    }
}

/// Offline QA: the Board Meeting countdown, a joined standup with progress.
fn qa_hooks(
    time: Res<Time>,
    mut sim: ResMut<CombatSim>,
    net: Option<Res<NetWorld>>,
    mut st: ResMut<LiveState>,
    mut windows: ResMut<crate::input::SprintWindows>,
    mut done: Local<(bool, bool)>,
) {
    if net.is_some() {
        return;
    }
    let t = time.elapsed_secs();
    let env_secs = |k: &str| {
        std::env::var(k)
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
    };
    if !done.0 && t > 1.0 {
        done.0 = true;
        if let Some(secs) = env_secs("WOW_WORLD_BOSS_IN") {
            sim.0.debug_world_boss_in(secs as u32);
        }
        if let Some((key, secs)) = std::env::var("WOW_WORLD_EVENT").ok().and_then(|v| {
            let (k, s) = v.split_once('@')?;
            Some((k.to_string(), s.trim().parse::<u32>().ok()?))
        }) {
            sim.0.debug_world_event_in(&key, secs);
        }
        if std::env::var_os("WOW_STANDUP").is_some()
            && let Some(p) = sim.0.player()
        {
            sim.0.debug_join_standup(p);
            let first = sim
                .0
                .task_board(p)
                .tasks
                .iter()
                .find(|t| !t.weekly)
                .map(|t| {
                    (
                        t.key.clone(),
                        t.goals.iter().map(|g| g.need / 2 + 1).collect::<Vec<_>>(),
                    )
                });
            if let Some((key, counts)) = first {
                sim.0.debug_task_counts(p, &key, &counts);
            }
        }
    }
    if !done.1 && env_secs("WOW_BOARD").is_some_and(|at| t >= at) {
        done.1 = true;
        if let Some(p) = sim.0.player() {
            st.board = Some(sim.0.task_board(p));
            windows.standup = true;
        }
    }
}
