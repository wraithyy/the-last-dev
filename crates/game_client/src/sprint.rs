//! Sprint mode presentation: the Sprint Planning window (board and terminal props in the campus),
//! the in-run HUD (timer against par, story points burned, bosses, affix icons), affix flashes,
//! the Sprint Review screen with its retrospective and the local leaderboard. Rules live in
//! `combat_core::sprint`; this side only sends `Command::StartSprint` and follows the events.
//! Offline the client is the host: it fades to whatever `Sim::sprint_travel` names.
//!
//! QA hooks: `WOW_SPRINT_UI=<level>` opens the planner at start on that level; `WOW_SPRINT=<building>:<level>`
//! starts a run two seconds in (offline, needs the unlock, e.g. `WOW_LEVEL=10`);
//! `WOW_SPRINT_END=<secs>` delivers the running sprint at that time (review screen shots);
//! `WOW_SPRINT_BOARD=<secs>` opens the planner (with its leaderboard) at that time;
//! `WOW_SPRINT_WEEKLY=1` with `WOW_SPRINT_UI` opens it on the Weekly Sprint tab. The weekly tab
//! sends `Command::StartWeeklySprint` and shows the week's ladder (`Event::Ladder`).
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, CombatSim, PendingCommands, SimReady, SimView, UiNotice};
use crate::game::GameFlow;
use crate::hud::quests::{TOUCH_TOP, TRACKER_TOP, TRACKER_W};
use crate::hud::{
    ACCENT, Banner, DIM, IconCache, LogLineMsg, PANEL_EDGE, drop_shadow, label, panel_bg, set_text,
};
use crate::input::{Paused, SprintWindows};
use crate::net::NetWorld;
use crate::touch::TouchMode;
use bevy::prelude::*;
use combat_core::{
    Command, Event, PropKind, SprintPreview, SprintRecord, SprintResult, SprintStatus,
    SprintSummary, retro_lines,
};

const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);
const GOOD: Color = Color::srgb(0.4, 1.0, 0.5);
const BAD: Color = Color::srgb(1.0, 0.38, 0.32);
const AFFIX: Color = Color::srgb(1.0, 0.62, 0.2);
const STICKY: Color = Color::srgb(0.98, 0.9, 0.45);
const FLASH_SECS: f32 = 4.0;
const LEADERBOARD_ROWS: usize = 5;

/// What the HUD knows about the current run, from the sprint events (offline and online alike).
#[derive(Resource, Default)]
pub struct SprintHud {
    /// A run is going on: game flow keeps its campaign banners and victory screen out of it.
    pub active: bool,
    status: Option<SprintStatus>,
    /// Seconds (app time) when `status` arrived, for the timer between updates.
    got_at: f32,
    flash: Option<(String, f32)>,
    review: Option<Review>,
    /// Online: this session's recommended level per building and records.
    keys: Vec<(String, u8)>,
    records: Vec<SprintRecord>,
}

struct Review {
    result: SprintResult,
    upgrade: i8,
    summary: SprintSummary,
}

#[derive(Resource)]
struct Planner {
    building: usize,
    level: u8,
    /// The Weekly Sprint tab is selected instead of a building.
    weekly: bool,
    dirty: bool,
}

impl Default for Planner {
    fn default() -> Self {
        Planner {
            building: 0,
            level: 1,
            weekly: false,
            dirty: true,
        }
    }
}

/// The current ISO week as the sim (offline) or the server's views (online) know it.
pub fn current_week(sim: &SimView) -> u32 {
    sim.legacy_view().week.max(sim.raid_view().week)
}

#[derive(Component)]
struct PlannerWindow;
#[derive(Component)]
struct PlannerBody;
#[derive(Component, Clone, Copy)]
enum PlannerButton {
    Building(usize),
    Weekly,
    Less,
    More,
    Start,
    Close,
}
#[derive(Component)]
struct RunHud;
#[derive(Component)]
enum RunText {
    Title,
    Timer,
    Forces,
    Info,
}
#[derive(Component)]
struct ForcesFill;
#[derive(Component)]
struct AffixRow;
#[derive(Component)]
struct AffixIcon(String);
#[derive(Component)]
struct AffixTip;
#[derive(Component)]
struct FlashText;
#[derive(Component)]
struct ReviewWindow;
#[derive(Component)]
struct ReviewBody;
#[derive(Component)]
struct ReviewClose;

pub struct SprintPlugin;

impl Plugin for SprintPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SprintHud>()
            .init_resource::<Planner>()
            .add_systems(Startup, spawn_ui)
            .add_systems(
                Update,
                (
                    track_events,
                    follow_travel,
                    qa_hooks,
                    planner_input,
                    rebuild_planner,
                    update_run_hud,
                    affix_tips,
                    review_window,
                )
                    .chain()
                    .in_set(SimReady),
            );
        if std::env::var_os("WOW_SPRINT_UI").is_some() {
            app.insert_resource(SprintWindows {
                planner: true,
                ..default()
            });
        }
    }
}

fn mmss(ms: u32) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

fn class_name(key: &str) -> &'static str {
    match key {
        "fullstack" => "Fullstack",
        "frontend" => "Frontend",
        "backend" => "Backend",
        "devops" => "DevOps",
        "qa" => "QA",
        "data_scientist" => "Data Scientist",
        "security" => "Security",
        "cobol" => "COBOL",
        _ => "Contractor",
    }
}

fn building_name(content: &combat_core::Content, key: &str) -> String {
    content
        .sprint
        .building(key)
        .map_or_else(|| key.to_string(), |b| b.name.clone())
}

/// One horizontal row.
fn row() -> Node {
    Node {
        column_gap: px(8),
        align_items: AlignItems::Center,
        ..default()
    }
}

fn button(kind: PlannerButton, text: impl Into<String>, on: bool) -> impl Bundle {
    (
        kind,
        Button,
        Node {
            padding: UiRect::axes(px(10), px(3)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BorderColor::all(if on { ACCENT } else { PANEL_EDGE }),
        BackgroundColor(if on {
            ACCENT.with_alpha(0.18)
        } else {
            Color::NONE
        }),
        children![(label(text, 14.0), TextColor(if on { ACCENT } else { DIM }))],
    )
}

fn window_node(width: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: percent(50),
        top: px(70),
        width: px(width),
        margin: UiRect::left(px(-width / 2.0)),
        padding: UiRect::all(px(12)),
        flex_direction: FlexDirection::Column,
        row_gap: px(7),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(5)),
        overflow: Overflow::scroll_y(),
        display: Display::None,
        ..default()
    }
}

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn((
            PlannerWindow,
            window_node(640.0),
            crate::hud::scroll::ScrollList::fit(12.0),
            BorderColor::all(ACCENT.with_alpha(0.7)),
            // Opaque under the gradient: world-space bubbles must not read through a modal.
            BackgroundColor(Color::srgb(0.05, 0.055, 0.08)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(32),
        ))
        .with_child((
            PlannerBody,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(7),
                ..default()
            },
        ));
    commands
        .spawn((
            ReviewWindow,
            window_node(680.0),
            crate::hud::scroll::ScrollList::fit(12.0),
            BorderColor::all(GOLD.with_alpha(0.8)),
            BackgroundColor(Color::srgb(0.05, 0.055, 0.08)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(33),
        ))
        .with_child((
            ReviewBody,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            },
        ));
    // The run HUD takes the quest tracker's slot (the tracker hides during a run), like WoW's
    // challenge timer; `update_run_hud` moves it with the touch layout.
    commands
        .spawn((
            RunHud,
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(TRACKER_TOP),
                width: px(TRACKER_W),
                padding: UiRect::axes(px(10), px(6)),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.55)),
            panel_bg(),
            GlobalZIndex(12),
        ))
        .with_children(|c| {
            c.spawn((RunText::Title, label("", 13.0), TextColor(ACCENT)));
            c.spawn((RunText::Timer, label("", 26.0), TextColor(GOOD)));
            c.spawn((RunText::Forces, label("", 12.0), TextColor(DIM)));
            c.spawn((
                Node {
                    height: px(7),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                BackgroundColor(Color::srgb(0.07, 0.07, 0.1)),
            ))
            .with_child((
                ForcesFill,
                Node {
                    width: percent(0),
                    height: percent(100),
                    ..default()
                },
                BackgroundColor(AFFIX),
            ));
            c.spawn((RunText::Info, label("", 12.0), TextColor(DIM)));
            c.spawn((AffixRow, row()));
            c.spawn((
                AffixTip,
                Node {
                    display: Display::None,
                    max_width: px(TRACKER_W - 22.0),
                    ..default()
                },
                label("", 12.0),
                TextColor(STICKY),
            ));
        });
    commands.spawn((
        FlashText,
        Node {
            position_type: PositionType::Absolute,
            top: percent(30),
            width: percent(100),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        GlobalZIndex(13),
        children![(label("", 20.0), TextColor(AFFIX))],
    ));
}

/// Sprint events into the HUD state, banners, the combat log and the windows.
fn track_events(
    time: Res<Time>,
    sim: SimView,
    net: Option<Res<NetWorld>>,
    mut hud: ResMut<SprintHud>,
    mut windows: ResMut<SprintWindows>,
    mut planner: ResMut<Planner>,
    mut events: MessageReader<CombatEvent>,
    mut banners: MessageWriter<Banner>,
    mut log: MessageWriter<LogLineMsg>,
    mut notices: MessageWriter<UiNotice>,
) {
    let now = time.elapsed_secs();
    let me = sim.player();
    let content = sim.content();
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::Interacted { unit, prop, .. } if Some(*unit) == me => {
                if content
                    .prop(prop)
                    .is_some_and(|p| p.kind == PropKind::SprintBoard)
                {
                    windows.planner = true;
                    planner.dirty = true;
                }
            }
            Event::SprintStarted { status, .. } => {
                hud.active = true;
                hud.review = None;
                windows.review = false;
                windows.planner = false;
                let names: Vec<String> = status
                    .affixes
                    .iter()
                    .filter_map(|a| content.sprint.affix(a).map(|d| d.name.clone()))
                    .collect();
                banners.write(Banner {
                    title: format!(
                        "Sprint +{}: {}",
                        status.level,
                        building_name(content, &status.building)
                    ),
                    subtitle: if names.is_empty() {
                        format!(
                            "Par {}. No affixes. Suspiciously easy.",
                            mmss(status.par_ms)
                        )
                    } else {
                        format!("Par {}. Affixes: {}", mmss(status.par_ms), names.join(", "))
                    },
                    color: AFFIX,
                });
                log.write(LogLineMsg(
                    format!(
                        "[sprint] Committed to +{} {}. The clock starts on arrival.",
                        status.level,
                        building_name(content, &status.building)
                    ),
                    AFFIX,
                ));
                hud.status = Some(status.clone());
                hud.got_at = now;
            }
            Event::SprintProgress { status } => {
                let before = hud.status.as_ref().map(|s| s.bosses_left);
                if before.is_some_and(|b| status.bosses_left < b) && status.bosses_left > 0 {
                    banners.write(Banner {
                        title: "Floor delivered".into(),
                        subtitle: "Next one in a few seconds. Do not go anywhere.".into(),
                        color: GOOD,
                    });
                }
                hud.active = true;
                hud.status = Some(status.clone());
                hud.got_at = now;
            }
            Event::SprintEnded {
                result,
                time_ms,
                upgrade,
                summary,
            } => {
                hud.active = false;
                hud.status = Some(summary.status.clone());
                let (title, color) = match result {
                    SprintResult::Delivered => (format!("Delivered on time (+{upgrade})"), GOOD),
                    SprintResult::CarriedOver => ("Carried over to next sprint".to_string(), BAD),
                    SprintResult::Abandoned => ("Sprint abandoned".to_string(), BAD),
                };
                banners.write(Banner {
                    title,
                    subtitle: format!(
                        "{} against par {}",
                        mmss(*time_ms),
                        mmss(summary.status.par_ms)
                    ),
                    color,
                });
                // Online the session keeps its own board; offline the sim already has it.
                if let Some(net) = net.as_deref() {
                    let building = summary.status.building.clone();
                    hud.keys.retain(|(b, _)| *b != building);
                    hud.keys.push((building.clone(), summary.next_level));
                    let class = me
                        .and_then(|p| sim.unit(p))
                        .and_then(|u| content.class_of_unit(&u.key))
                        .map(|c| c.key.clone())
                        .unwrap_or_default();
                    let record = SprintRecord {
                        building,
                        level: summary.status.level,
                        time_ms: *time_ms,
                        class: class.clone(),
                        name: me
                            .and_then(|p| net.names.get(&p).cloned())
                            .unwrap_or_default(),
                    };
                    if *result == SprintResult::Delivered {
                        hud.records.push(record.clone());
                    }
                    crate::economy::save::record_online_sprint(
                        &class,
                        (*result == SprintResult::Delivered).then_some(record),
                        (summary.status.building.clone(), summary.next_level),
                    );
                }
                if *result != SprintResult::Abandoned {
                    windows.review = true;
                }
                // The planner reopens on the building just played (its leaderboard).
                if let Some(k) = content
                    .sprint
                    .buildings
                    .iter()
                    .position(|b| b.key == summary.status.building)
                {
                    planner.building = k;
                    planner.dirty = true;
                }
                hud.review = Some(Review {
                    result: *result,
                    upgrade: *upgrade,
                    summary: summary.clone(),
                });
            }
            Event::SprintAffix { affix, text, .. } => {
                let name = content
                    .sprint
                    .affix(affix)
                    .map_or_else(|| affix.clone(), |a| a.name.clone());
                log.write(LogLineMsg(format!("[{name}] {text}"), AFFIX));
                hud.flash = Some((text.clone(), now));
            }
            Event::SprintRefused { reason } => {
                notices.write(UiNotice(reason.message().to_string()));
            }
            _ => {}
        }
    }
}

/// Offline the client hosts the run: fade to the zone the sim asks for.
fn follow_travel(sim: Res<CombatSim>, net: Option<Res<NetWorld>>, mut flow: ResMut<GameFlow>) {
    if net.is_some() {
        return;
    }
    // Minigames ride the same hook: the venue, and the campus once every run is over.
    if let Some(z) = sim.0.sprint_travel().or(sim.0.minigame_travel()) {
        flow.travel_to(z);
    }
}

fn env_secs(key: &str) -> Option<f32> {
    std::env::var(key).ok()?.trim().parse().ok()
}

fn qa_hooks(
    time: Res<Time>,
    flow: Res<GameFlow>,
    mut sim: ResMut<CombatSim>,
    net: Option<Res<NetWorld>>,
    mut pending: ResMut<PendingCommands>,
    mut windows: ResMut<SprintWindows>,
    mut next_try: Local<f32>,
    mut ended: Local<bool>,
    mut board: Local<bool>,
) {
    if net.is_some() {
        return;
    }
    let t = time.elapsed_secs();
    // Retried every 2 s until a run exists (a fight on the way in refuses the start).
    if t > 2.0f32.max(*next_try)
        && flow.zone == "campus"
        && sim.0.sprint_status().is_none()
        && sim.0.sprint_last_result().is_none()
        && let Ok(v) = std::env::var("WOW_SPRINT")
    {
        *next_try = t + 2.0;
        let (building, level) = v.split_once(':').unwrap_or((v.as_str(), "1"));
        pending.0.push(Command::StartSprint {
            building: building.to_string(),
            level: level.trim().parse().unwrap_or(1),
        });
    }
    if !*ended && env_secs("WOW_SPRINT_END").is_some_and(|s| t >= s) {
        *ended = sim.0.debug_end_sprint(true);
    }
    if !*board && env_secs("WOW_SPRINT_BOARD").is_some_and(|s| t >= s) {
        *board = true;
        windows.review = false;
        windows.planner = true;
    }
}

/// The buildings, the level and the recommended level of the local player.
fn recommended(sim: &SimView, offline: &CombatSim, online: bool, hud: &SprintHud, b: &str) -> u8 {
    if online {
        hud.keys.iter().find(|(k, _)| k == b).map_or(1, |(_, l)| *l)
    } else {
        sim.player().map_or(1, |p| offline.0.sprint_key(p, b))
    }
}

fn planner_input(
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    hud: Res<SprintHud>,
    mut windows: ResMut<SprintWindows>,
    mut planner: ResMut<Planner>,
    mut pending: ResMut<PendingCommands>,
    buttons: Query<(&Interaction, &PlannerButton), Changed<Interaction>>,
    mut qa_level: Local<Option<u8>>,
) {
    let def = &sim.content().sprint;
    if def.buildings.is_empty() {
        return;
    }
    if windows.is_changed() && windows.planner {
        planner.dirty = true;
        let b = def.buildings[planner.building.min(def.buildings.len() - 1)]
            .key
            .clone();
        planner.level = recommended(&sim, &offline, net.is_some(), &hud, &b);
        let qa = std::env::var("WOW_SPRINT_UI")
            .ok()
            .and_then(|v| v.parse().ok());
        if let Some(level) = qa.filter(|_| qa_level.is_none()) {
            *qa_level = Some(level);
            planner.level = level;
            planner.weekly = std::env::var_os("WOW_SPRINT_WEEKLY").is_some();
        }
    }
    for (i, b) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        planner.dirty = true;
        match *b {
            PlannerButton::Weekly => planner.weekly = true,
            PlannerButton::Building(k) => {
                planner.weekly = false;
                planner.building = k.min(def.buildings.len() - 1);
                let key = def.buildings[planner.building].key.clone();
                planner.level = recommended(&sim, &offline, net.is_some(), &hud, &key);
            }
            PlannerButton::Less => planner.level = planner.level.saturating_sub(1).max(1),
            PlannerButton::More => planner.level = (planner.level + 1).min(def.max_level.max(1)),
            PlannerButton::Close => windows.planner = false,
            PlannerButton::Start if planner.weekly => {
                pending.0.push(Command::StartWeeklySprint);
                windows.planner = false;
            }
            PlannerButton::Start => {
                pending.0.push(Command::StartSprint {
                    building: def.buildings[planner.building].key.clone(),
                    level: planner.level,
                });
                windows.planner = false;
            }
        }
    }
}

/// Leaderboard lines of `building`, best level first then fastest.
fn board(records: &[SprintRecord], building: &str) -> Vec<SprintRecord> {
    let mut v: Vec<SprintRecord> = records
        .iter()
        .filter(|r| r.building == building)
        .cloned()
        .collect();
    v.sort_by(|a, b| b.level.cmp(&a.level).then(a.time_ms.cmp(&b.time_ms)));
    v.truncate(LEADERBOARD_ROWS);
    v
}

fn records_for(
    sim: &SimView,
    offline: &CombatSim,
    net: Option<&NetWorld>,
    hud: &SprintHud,
) -> Vec<SprintRecord> {
    match net {
        Some(_) => {
            let class = sim
                .player()
                .and_then(|p| sim.unit(p))
                .and_then(|u| sim.content().class_of_unit(&u.key))
                .map(|c| c.key.clone())
                .unwrap_or_default();
            let mut v = crate::economy::save::local_sprint_records(&class);
            v.extend(hud.records.iter().cloned());
            v
        }
        None => sim
            .player()
            .map(|p| offline.0.sprint_records(p).to_vec())
            .unwrap_or_default(),
    }
}

fn spawn_board(c: &mut ChildSpawnerCommands, rows: &[SprintRecord]) {
    if rows.is_empty() {
        c.spawn((
            label(
                "  Nothing delivered yet. The board is as empty as the roadmap.",
                12.0,
            ),
            TextColor(DIM),
        ));
    }
    for (k, r) in rows.iter().enumerate() {
        let who = if r.name.is_empty() {
            "you"
        } else {
            r.name.as_str()
        };
        c.spawn((
            label(
                format!(
                    "  {}. +{:<3} {:>6}   {:<10} {who}",
                    k + 1,
                    r.level,
                    mmss(r.time_ms),
                    class_name(&r.class)
                ),
                13.0,
            ),
            TextColor(if k == 0 { GOLD } else { Color::WHITE }),
        ));
    }
}

/// The Ilmari row of the Weekly tab and `~/endgame` (D16): the week's Directive and the best row of
/// each tab of the core's boards. Nothing until the room has a ranked kill, so the core stays a
/// surprise for a campaign that has not reached it.
pub struct IlmariRow {
    pub week: u32,
    pub directive: String,
    /// "12:31, 1 dead (crew)" per tab: Full programme, Async attendance, Judgment Day.
    pub best: [Option<String>; 3],
}

pub fn ilmari_row(
    full: &combat_core::LadderView,
    jd: &combat_core::LadderView,
) -> Option<IlmariRow> {
    let best = |l: &combat_core::LadderView, is_async: bool| {
        l.entries.iter().find(|e| e.is_async == is_async).map(|e| {
            format!(
                "{}, {} dead ({})",
                mmss(e.time_ms),
                e.deaths,
                e.crew.join(", ")
            )
        })
    };
    let row = IlmariRow {
        week: full.week,
        directive: if full.directive.is_empty() {
            jd.directive.clone()
        } else {
            full.directive.clone()
        },
        best: [best(full, false), best(full, true), best(jd, false)],
    };
    (row.best.iter().any(Option::is_some) && !row.directive.is_empty()).then_some(row)
}

/// The same row as one line for the `~/endgame` window.
pub fn ilmari_row_line(row: &IlmariRow) -> String {
    let tab = |name: &str, b: &Option<String>| {
        format!(
            "{name} {}",
            b.as_deref()
                .map_or("-", |s| s.split(',').next().unwrap_or("-"))
        )
    };
    format!(
        "Ilmari, week {}: {}. {}, {}, {}.",
        row.week,
        row.directive,
        tab("Full", &row.best[0]),
        tab("Async", &row.best[1]),
        tab("Judgment", &row.best[2]),
    )
}

fn spawn_ilmari_row(c: &mut ChildSpawnerCommands, row: &IlmariRow) {
    c.spawn((
        label(
            format!(
                "Ilmari core, week {}: {} (every ranked fight wears it)",
                row.week, row.directive
            ),
            13.0,
        ),
        TextColor(ACCENT),
    ));
    for (name, best) in ["Full programme", "Async attendance", "Judgment Day"]
        .iter()
        .zip(&row.best)
    {
        c.spawn((
            label(
                format!("  {name:<17} {}", best.as_deref().unwrap_or("nobody yet")),
                13.0,
            ),
            TextColor(if best.is_some() { GOLD } else { DIM }),
        ));
    }
}

fn spawn_ladder(c: &mut ChildSpawnerCommands, view: &combat_core::LadderView) {
    if view.entries.is_empty() {
        c.spawn((
            label(
                "  Nobody delivered this week's sprint yet. First to ship sets the bar.",
                12.0,
            ),
            TextColor(DIM),
        ));
    }
    for (k, e) in view.entries.iter().take(LEADERBOARD_ROWS).enumerate() {
        c.spawn((
            label(
                format!(
                    "  {}. {:>6}  {} dead   {}",
                    k + 1,
                    mmss(e.time_ms),
                    e.deaths,
                    e.crew.join(", ")
                ),
                13.0,
            ),
            TextColor(if k == 0 { GOLD } else { Color::WHITE }),
        ));
    }
}

fn spawn_affix(
    c: &mut ChildSpawnerCommands,
    content: &combat_core::Content,
    key: &str,
    assets: &AssetServer,
    icons: &mut IconCache,
    with_text: bool,
) {
    let Some(a) = content.sprint.affix(key) else {
        return;
    };
    let icon = icons.get(assets, &a.icon);
    c.spawn(Node {
        column_gap: px(8),
        align_items: AlignItems::FlexStart,
        ..default()
    })
    .with_children(|r| {
        r.spawn((
            ImageNode::new(icon),
            Node {
                width: px(26),
                height: px(26),
                flex_shrink: 0.0,
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(AFFIX),
        ));
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            width: px(if with_text { 570.0 } else { 120.0 }),
            ..default()
        })
        .with_children(|t| {
            t.spawn((label(a.name.clone(), 14.0), TextColor(AFFIX)));
            if with_text {
                t.spawn((label(a.text.clone(), 12.0), TextColor(DIM)));
            }
        });
    });
}

fn rebuild_planner(
    mut commands: Commands,
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    hud: Res<SprintHud>,
    windows: Res<SprintWindows>,
    flow: Res<GameFlow>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut planner: ResMut<Planner>,
    mut window: Single<&mut Node, With<PlannerWindow>>,
    body: Single<Entity, With<PlannerBody>>,
    mut seen: Local<(String, bool)>,
) {
    // The blocker line depends on where the group is and whether a run is going.
    if seen.0 != flow.zone || seen.1 != hud.active {
        *seen = (flow.zone.clone(), hud.active);
        planner.dirty = true;
    }
    window.display = if windows.planner {
        Display::Flex
    } else {
        Display::None
    };
    if !windows.planner || !planner.dirty {
        return;
    }
    planner.dirty = false;
    let content = sim.content();
    let def = &content.sprint;
    let Some(b) = def.buildings.get(planner.building) else {
        return;
    };
    let seed = net
        .as_deref()
        .map_or_else(|| offline.0.session_seed(), |n| n.seed);
    let week = current_week(&sim);
    let weekly = combat_core::weekly_sprint(def, week).filter(|_| planner.weekly);
    let (bkey, level, seed) = match &weekly {
        Some(w) => (w.building.clone(), w.level, seed),
        None => (b.key.clone(), planner.level, seed),
    };
    let Some(mut p): Option<SprintPreview> = combat_core::sprint_preview(def, seed, &bkey, level)
    else {
        return;
    };
    if let Some(w) = &weekly {
        p.affixes.clone_from(&w.affixes);
    }
    let rec = recommended(&sim, &offline, net.is_some(), &hud, &b.key);
    let me = sim.player();
    let unit = me.and_then(|m| sim.unit(m));
    let leader = sim.players().first().copied() == me;
    let unlocked = match net.as_deref() {
        Some(_) => unit.is_some_and(|u| u.level >= def.unlock_level),
        None => me.is_some_and(|m| offline.0.sprint_unlocked(m)),
    };
    let blocker = if hud.active {
        Some("A sprint is already running. Finish it or walk out.")
    } else if !unlocked {
        Some(combat_core::SprintError::Locked.message())
    } else if !leader {
        Some(combat_core::SprintError::NotLeader.message())
    } else if flow.zone != "campus" {
        Some(combat_core::SprintError::NotInCampus.message())
    } else {
        None
    };
    let rows = board(&records_for(&sim, &offline, net.as_deref(), &hud), &b.key);
    // The weekly building, not the tab under the cursor, decides the stage line.
    let b = def.building(&bkey).unwrap_or(b);
    let stages = if b.pick as usize == b.zones.len() {
        format!("{} zones", b.zones.len())
    } else {
        format!("{} random of {} floors", b.pick, b.zones.len())
    };
    commands.entity(*body).despawn_related::<Children>();
    commands.entity(*body).with_children(|c| {
        c.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|h| {
            h.spawn((
                label(
                    "~/sprint-planning  // estimate in story points, deliver in blood",
                    14.0,
                ),
                TextColor(DIM),
            ));
            h.spawn(button(PlannerButton::Close, "[x]", false));
        });
        c.spawn(row()).with_children(|r| {
            r.spawn((label("building", 13.0), TextColor(DIM)));
            for (k, bd) in def.buildings.iter().enumerate() {
                r.spawn(button(
                    PlannerButton::Building(k),
                    bd.name.clone(),
                    k == planner.building && weekly.is_none(),
                ));
            }
            if def.weekly_level > 0 {
                r.spawn(button(PlannerButton::Weekly, "Weekly", weekly.is_some()));
            }
        });
        if let Some(w) = &weekly {
            c.spawn((
                label(
                    format!(
                        "Weekly Sprint, week {}: {} at +{}. Same building, floors and affixes for everyone until Monday.",
                        w.week, w.name, w.level
                    ),
                    13.0,
                ),
                TextColor(GOLD),
            ));
        }
        c.spawn(Node {
            display: if weekly.is_some() {
                Display::None
            } else {
                Display::Flex
            },
            ..row()
        })
        .with_children(|r| {
            r.spawn((label("story points", 13.0), TextColor(DIM)));
            r.spawn(button(PlannerButton::Less, "-", false));
            r.spawn((label(format!("+{}", planner.level), 22.0), TextColor(GOLD)));
            r.spawn(button(PlannerButton::More, "+", false));
            r.spawn((
                label(format!("recommended +{rec} (your last sprint here)"), 12.0),
                TextColor(DIM),
            ));
        });
        c.spawn((
            label(
                format!(
                    "par {}   enemy HP +{}%  dmg +{}%   {stages}   death +{} s",
                    mmss(p.par_ms),
                    p.hp_pct - 100,
                    p.damage_pct - 100,
                    def.death_penalty_ms / 1000
                ),
                13.0,
            ),
            TextColor(Color::WHITE),
        ));
        c.spawn((
            label(
                if weekly.is_some() {
                    "the week's fixed affixes"
                } else {
                    "this week's affixes"
                },
                13.0,
            ),
            TextColor(ACCENT),
        ));
        if p.affixes.is_empty() {
            c.spawn((
                label(
                    format!(
                        "  none below +{}. Enjoy it while it lasts.",
                        def.affix_levels.first().copied().unwrap_or(2)
                    ),
                    12.0,
                ),
                TextColor(DIM),
            ));
        }
        for a in &p.affixes {
            spawn_affix(c, content, a, &assets, &mut icons, true);
        }
        let next: Vec<String> = def
            .affix_levels
            .iter()
            .filter(|l| **l > level && weekly.is_none())
            .map(|l| format!("+{l}"))
            .collect();
        if !next.is_empty() {
            c.spawn((
                label(format!("  more affixes join at {}", next.join(", ")), 12.0),
                TextColor(DIM),
            ));
        }
        if weekly.is_some() {
            let online = net.is_some();
            c.spawn((
                label(
                    if online {
                        "weekly ladder: this room"
                    } else {
                        "weekly ladder: this machine"
                    },
                    13.0,
                ),
                TextColor(ACCENT),
            ));
            spawn_ladder(c, &sim.ladder_view());
            if let Some(row) = ilmari_row(
                &sim.ilmari_ladder_view(combat_core::LadderBoard::IlmariFull),
                &sim.ilmari_ladder_view(combat_core::LadderBoard::IlmariJudgment),
            ) {
                spawn_ilmari_row(c, &row);
            }
        } else {
            c.spawn((
                label(format!("leaderboard: {}", b.name), 13.0),
                TextColor(ACCENT),
            ));
            spawn_board(c, &rows);
        }
        match blocker {
            Some(why) => {
                c.spawn((label(why, 13.0), TextColor(BAD)));
            }
            None => {
                c.spawn(row()).with_children(|r| {
                    let (go, rule) = if weekly.is_some() {
                        (
                            "> commit to the Weekly Sprint",
                            "Delivered runs go on the ladder, fastest first.",
                        )
                    } else {
                        (
                            "> commit to sprint",
                            "Beat par: +1..+3 levels. Miss it: carried over (-1).",
                        )
                    };
                    r.spawn(button(PlannerButton::Start, go, true));
                    r.spawn((label(rule, 12.0), TextColor(DIM)));
                });
            }
        }
    });
}

fn update_run_hud(
    time: Res<Time>,
    paused: Res<Paused>,
    net: Option<Res<NetWorld>>,
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut commands: Commands,
    hud: Res<SprintHud>,
    mut root: Single<&mut Node, (With<RunHud>, Without<ForcesFill>, Without<FlashText>)>,
    mut texts: Query<(&RunText, &mut Text, &mut TextColor)>,
    mut fill: Single<&mut Node, (With<ForcesFill>, Without<RunHud>, Without<FlashText>)>,
    row: Single<Entity, With<AffixRow>>,
    mut flash: Single<
        (&mut Node, &Children),
        (With<FlashText>, Without<RunHud>, Without<ForcesFill>),
    >,
    mut flash_text: Query<&mut Text, Without<RunText>>,
    mut shown_affixes: Local<Vec<String>>,
    touch: Res<TouchMode>,
) {
    let now = time.elapsed_secs();
    let (fnode, kids) = &mut *flash;
    let flashing = hud.flash.as_ref().filter(|(_, t)| now - t < FLASH_SECS);
    fnode.display = if flashing.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if let (Some((line, _)), Some(k)) = (flashing, kids.first())
        && let Ok(mut t) = flash_text.get_mut(*k)
    {
        set_text(&mut t, line);
    }
    let Some(st) = hud.status.as_ref().filter(|_| hud.active) else {
        root.display = Display::None;
        shown_affixes.clear();
        return;
    };
    root.display = Display::Flex;
    // Touch keeps the right side for buttons: the panel sits under the player frame instead.
    let (left, right, top) = if touch.active {
        (px(12), Val::Auto, px(TOUCH_TOP))
    } else {
        (Val::Auto, px(12), px(TRACKER_TOP))
    };
    if root.top != top || root.left != left {
        root.left = left;
        root.right = right;
        root.top = top;
    }
    // Between the once-a-second updates the clock runs locally (not while paused offline).
    let frozen = paused.0 && net.is_none();
    let extra = if frozen || st.elapsed_ms == 0 {
        0.0
    } else {
        (now - hud.got_at).clamp(0.0, 1.5)
    };
    let time_ms = st.time_ms() + (extra * 1000.0) as u32;
    let content = sim.content();
    for (kind, mut text, mut color) in &mut texts {
        let s = match kind {
            RunText::Title => format!(
                "SPRINT +{}  {}  {}/{}",
                st.level,
                // The planner's "(3 random floors)" note would wrap the narrow panel.
                building_name(content, &st.building)
                    .split(" (")
                    .next()
                    .unwrap_or_default(),
                st.stage,
                st.stages
            ),
            RunText::Timer => {
                color.0 = if time_ms <= st.par_ms { GOOD } else { BAD };
                if st.elapsed_ms == 0 {
                    format!("--:-- / {}", mmss(st.par_ms))
                } else {
                    format!("{} / {}", mmss(time_ms), mmss(st.par_ms))
                }
            }
            RunText::Forces => format!("Story points burned: {}/100%", st.forces_pct),
            RunText::Info => {
                let mut s = format!(
                    "Bosses {}/{}   Deaths {} (+{} s)",
                    st.bosses_total - st.bosses_left.min(st.bosses_total),
                    st.bosses_total,
                    st.deaths,
                    st.penalty_ms / 1000
                );
                if st.tech_debt > 0 {
                    s.push_str(&format!("   Tech debt x{}", st.tech_debt));
                }
                s
            }
        };
        set_text(&mut text, &s);
    }
    fill.width = percent(f32::from(st.forces_pct.min(100)));
    if *shown_affixes != st.affixes {
        *shown_affixes = st.affixes.clone();
        commands.entity(*row).despawn_related::<Children>();
        for a in &st.affixes {
            let Some(def) = content.sprint.affix(a) else {
                continue;
            };
            let icon = icons.get(&assets, &def.icon);
            commands.entity(*row).with_child((
                AffixIcon(a.clone()),
                Button,
                ImageNode::new(icon),
                Node {
                    width: px(24),
                    height: px(24),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(AFFIX),
            ));
        }
    }
}

/// Hovering an affix icon in the run HUD shows its rules.
fn affix_tips(
    sim: SimView,
    icons: Query<(&Interaction, &AffixIcon)>,
    mut tip: Single<(&mut Node, &mut Text), With<AffixTip>>,
) {
    let hovered = icons
        .iter()
        .find(|(i, _)| **i != Interaction::None)
        .and_then(|(_, a)| sim.content().sprint.affix(&a.0));
    let (node, text) = &mut *tip;
    match hovered {
        Some(a) => {
            node.display = Display::Flex;
            set_text(text, &format!("{}: {}", a.name, a.text));
        }
        None => node.display = Display::None,
    }
}

fn review_window(
    mut commands: Commands,
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    hud: Res<SprintHud>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut windows: ResMut<SprintWindows>,
    mut window: Single<&mut Node, With<ReviewWindow>>,
    body: Single<Entity, With<ReviewBody>>,
    close: Query<&Interaction, (With<ReviewClose>, Changed<Interaction>)>,
    mut built: Local<bool>,
) {
    if close.iter().any(|i| *i == Interaction::Pressed) {
        windows.review = false;
    }
    window.display = if windows.review {
        Display::Flex
    } else {
        Display::None
    };
    if !windows.review {
        *built = false;
        return;
    }
    if *built {
        return;
    }
    let Some(r) = hud.review.as_ref() else {
        return;
    };
    *built = true;
    let content = sim.content();
    let st = &r.summary.status;
    let me = sim.player();
    let (verdict, color) = match r.result {
        SprintResult::Delivered => (format!("SPRINT DELIVERED  (+{} levels)", r.upgrade), GOOD),
        SprintResult::CarriedOver => ("CARRIED OVER TO NEXT SPRINT  (-1 level)".to_string(), BAD),
        SprintResult::Abandoned => ("ABANDONED. Nobody saw anything.".to_string(), BAD),
    };
    let names = net.as_deref().map(|n| n.names.clone()).unwrap_or_default();
    let rows = board(
        &records_for(&sim, &offline, net.as_deref(), &hud),
        &st.building,
    );
    let top = r
        .summary
        .meter
        .iter()
        .map(|m| m.damage.max(m.healing))
        .max()
        .unwrap_or(1)
        .max(1);
    commands.entity(*body).despawn_related::<Children>();
    commands.entity(*body).with_children(|c| {
        c.spawn((
            label("~/sprint-review  // attendance mandatory", 14.0),
            TextColor(DIM),
        ));
        c.spawn((label(verdict, 24.0), TextColor(color)));
        c.spawn((
            label(
                format!(
                    "+{} {}   {} / par {}   deaths {} (+{} s)   story points {}%",
                    st.level,
                    building_name(content, &st.building),
                    mmss(st.time_ms()),
                    mmss(st.par_ms),
                    st.deaths,
                    st.penalty_ms / 1000,
                    st.forces_pct
                ),
                14.0,
            ),
            TextColor(Color::WHITE),
        ));
        if !st.affixes.is_empty() {
            c.spawn(row()).with_children(|r2| {
                for a in &st.affixes {
                    spawn_affix(r2, content, a, &assets, &mut icons, false);
                }
            });
        }
        c.spawn((label("burndown (who burned what)", 13.0), TextColor(ACCENT)));
        for m in r.summary.meter.iter().take(5) {
            let who = names.get(&m.unit).cloned().unwrap_or_else(|| {
                if Some(m.unit) == me {
                    "you".into()
                } else {
                    "teammate".into()
                }
            });
            c.spawn(row()).with_children(|l| {
                l.spawn((
                    Node {
                        width: px(150),
                        ..default()
                    },
                    label(format!("{who} ({})", class_name(&m.class)), 13.0),
                    TextColor(Color::WHITE),
                ));
                l.spawn((
                    Node {
                        width: px(180 * m.damage.max(m.healing) as u32 / top as u32),
                        height: px(9),
                        ..default()
                    },
                    BackgroundColor(if m.healing > m.damage { GOOD } else { AFFIX }),
                ));
                l.spawn((
                    label(format!("{} dmg  {} heal", m.damage, m.healing), 12.0),
                    TextColor(DIM),
                ));
            });
        }
        if let Some(chest) = r.summary.rewards.iter().find(|x| Some(x.unit) == me) {
            let items = if chest.items.is_empty() {
                "nothing (bag full?)".to_string()
            } else {
                chest.items.join(", ")
            };
            c.spawn((
                label(format!("sprint chest: {items}  +{} Kudos", chest.sp), 13.0),
                TextColor(GOLD),
            ));
        }
        c.spawn((label("retrospective", 13.0), TextColor(ACCENT)));
        c.spawn(Node {
            column_gap: px(8),
            ..default()
        })
        .with_children(|notes| {
            for line in retro_lines(r.result, &r.summary) {
                notes.spawn((
                    Node {
                        width: px(208),
                        padding: UiRect::all(px(6)),
                        ..default()
                    },
                    BackgroundColor(STICKY),
                    children![(label(line, 12.0), TextColor(Color::srgb(0.12, 0.1, 0.05)))],
                ));
            }
        });
        c.spawn((
            label(
                format!("next sprint here: +{} recommended", r.summary.next_level),
                13.0,
            ),
            TextColor(Color::WHITE),
        ));
        c.spawn((
            label(
                format!("leaderboard: {}", building_name(content, &st.building)),
                13.0,
            ),
            TextColor(ACCENT),
        ));
        spawn_board(c, &rows);
        c.spawn(row()).with_children(|r2| {
            r2.spawn((
                ReviewClose,
                Button,
                Node {
                    padding: UiRect::axes(px(10), px(3)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BorderColor::all(ACCENT),
                children![(label("> close the retro", 14.0), TextColor(ACCENT))],
            ));
            r2.spawn((
                label("The group heads back to campus shortly.", 12.0),
                TextColor(DIM),
            ));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat_core::{LadderEntry, LadderView};

    fn view(directive: &str, rows: &[(u32, bool)]) -> LadderView {
        LadderView {
            week: 2964,
            directive: directive.into(),
            entries: rows
                .iter()
                .map(|&(time_ms, is_async)| LadderEntry {
                    time_ms,
                    deaths: 1,
                    crew: vec!["Ada (Backend Engineer)".into()],
                    is_async,
                })
                .collect(),
        }
    }

    /// The Ilmari row stays hidden until a ranked kill exists, then names the Directive and the
    /// best row of each tab (the Async rows never stand in for the group's).
    #[test]
    fn the_ilmari_row_waits_for_a_ranked_kill_and_splits_the_tabs() {
        assert!(ilmari_row(&view("Directive: Open Plan", &[]), &view("", &[])).is_none());
        assert!(ilmari_row(&view("", &[(60_000, false)]), &view("", &[])).is_none());
        let row = ilmari_row(
            &view("Directive: Open Plan", &[(730_000, false), (700_000, true)]),
            &view("Directive: Open Plan", &[(850_000, false)]),
        )
        .expect("a row");
        assert_eq!(row.week, 2964);
        assert_eq!(
            row.best[0].as_deref(),
            Some("12:10, 1 dead (Ada (Backend Engineer))")
        );
        assert_eq!(
            row.best[1].as_deref(),
            Some("11:40, 1 dead (Ada (Backend Engineer))")
        );
        assert_eq!(
            row.best[2].as_deref(),
            Some("14:10, 1 dead (Ada (Backend Engineer))")
        );
        assert_eq!(
            ilmari_row_line(&row),
            "Ilmari, week 2964: Directive: Open Plan. Full 12:10, Async 11:40, Judgment 14:10."
        );
    }
}
