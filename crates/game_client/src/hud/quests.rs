//! Quests in Jira clothes: the giver dialog (a ticket with Accept / Complete and a "choose one"
//! gear reward), the WoW-style objective tracker on the right ("MeatProxies terminated 5/10"),
//! progress toasts, the "Ticket closed" banner and the `~/jira` log (J) with track toggles.
//! Everything is read from the sim (or the online mirror) each frame; widgets are rebuilt only
//! when the shown state changes.

use super::{
    ACCENT, BAR_BG, Banner, DIM, IconCache, PANEL_EDGE, asset_key, drop_shadow, label, panel_bg,
    set_text,
};
use crate::combat::{CombatEvent, PendingCommands, SimRead, SimReady, SimView, Talked};
use crate::copy;
use crate::economy::goods;
use crate::game::{GameFlow, zone_label};
use crate::input::{Paused, QuestWindows, TrophiesOpen};
use crate::player::Player;
use crate::touch::TouchMode;
use bevy::prelude::*;
use combat_core::{Command, Content, Event, Objective, QuestDef, QuestStatus};

pub const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);
pub const GREY_MARK: Color = Color::srgb(0.62, 0.64, 0.7);
const DONE_GREEN: Color = Color::srgb(0.4, 1.0, 0.5);
const DONE_GREY: Color = Color::srgb(0.5, 0.53, 0.58);
const WINDOW_W: f32 = 460.0;
const TRACKER_W: f32 = 290.0;
/// Quests the tracker shows at most; touch screens have less room under the minimap.
const TRACKER_MAX: usize = 6;
const TRACKER_MAX_TOUCH: usize = 3;
const TOAST_SECS: f32 = 2.5;
const TRACKER_TOP: f32 = super::BELOW_MINIMAP + 34.0;
/// Height kept free for the chat / combat log / meter windows docked bottom right.
const DOCK_RESERVE: f32 = 230.0;
/// Touch layout: under the player frame on the left, above the move stick.
const TOUCH_TOP: f32 = 120.0;
const TOUCH_RESERVE: f32 = 340.0;
/// Rough height of one tracker text line, for the fit check.
const LINE_PX: f32 = 16.0;

/// "SPRINT-101": the quest's position in content, so ids are stable across runs.
pub fn ticket_id(content: &Content, id: &str) -> String {
    let n = content.quests.iter().position(|q| q.id == id).unwrap_or(0);
    format!("SPRINT-{}", 101 + n)
}

pub fn unit_name(content: &Content, key: &str) -> String {
    content
        .unit_def(key)
        .map_or_else(|| key.to_string(), |u| u.name.clone())
}

/// English plural of a display name: "MeatProxy" -> "MeatProxies", "Agile Coach" -> "Agile
/// Coaches". Names ending in a bracket or an "s" stay as they are.
pub fn plural(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let consonant_y = lower.ends_with('y')
        && !lower.ends_with("ay")
        && !lower.ends_with("ey")
        && !lower.ends_with("oy");
    if name.ends_with(')') || lower.ends_with('s') {
        name.to_string()
    } else if consonant_y {
        format!("{}ies", &name[..name.len() - 1])
    } else if lower.ends_with("ch") || lower.ends_with("sh") || lower.ends_with('x') {
        format!("{name}es")
    } else {
        format!("{name}s")
    }
}

/// What an objective asks for, without the count.
pub fn objective_label(content: &Content, o: &Objective) -> String {
    match o {
        Objective::Kill(k, n) => {
            let name = unit_name(content, k);
            let who = if *n > 1 { plural(&name) } else { name };
            format!("{who} terminated")
        }
        Objective::Loot(item, n) if *n > 1 => plural(item),
        Objective::Loot(item, _) => item.clone(),
        Objective::Talk(k) => format!("Talk to {}", unit_name(content, k)),
        Objective::ClearZone(z) => format!("Clear {}", zone_label(content, z)),
    }
}

/// One tracker/toast line: "MeatProxies terminated 5/10", "Access Badges 2/5"; talk and clear
/// objectives read as a plain task ("Talk to Facilities Guy") until done.
pub fn objective_line(content: &Content, o: &Objective, have: u32, need: u32) -> String {
    let what = objective_label(content, o);
    match o {
        Objective::Kill(..) | Objective::Loot(..) => format!("{what} {have}/{need}"),
        Objective::Talk(_) | Objective::ClearZone(_) => what,
    }
}

/// Kept for the toast and tests: the same text as the tracker line.
pub fn progress_line(content: &Content, o: &Objective, count: u32, need: u32) -> String {
    objective_line(content, o, count, need)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    Pending,
    Done,
    /// "Ready to close" in gold.
    Ready,
}

/// The tracker body of one quest: every objective with its state, then the turn-in line once
/// all are done.
pub fn tracker_lines(content: &Content, def: &QuestDef, counts: &[u32]) -> Vec<(String, Line)> {
    let mut out: Vec<(String, Line)> = def
        .objectives
        .iter()
        .enumerate()
        .map(|(k, o)| {
            let need = o.need();
            let have = counts.get(k).copied().unwrap_or(0).min(need);
            let state = if have >= need {
                Line::Done
            } else {
                Line::Pending
            };
            (objective_line(content, o, have, need), state)
        })
        .collect();
    if out.iter().all(|(_, l)| *l == Line::Done) {
        out.push((
            format!(
                "Ready to close \u{2014} return to {}",
                unit_name(content, &def.ender().npc)
            ),
            Line::Ready,
        ));
    }
    out
}

/// The NPC's quests the player can act on in `zone`, content order: pick-ups at their giver,
/// open and finished work at whoever takes it (the giver, or a breadcrumb's target).
pub fn giver_quests(sim: &dyn SimRead, npc_key: &str, zone: &str) -> Vec<(String, QuestStatus)> {
    sim.content()
        .quests
        .iter()
        .map(|q| (q, sim.quest_status(&q.id)))
        .filter(|(q, s)| match s {
            QuestStatus::Available => q.giver.npc == npc_key && q.giver.zone == zone,
            QuestStatus::Active | QuestStatus::Ready => {
                q.ender().npc == npc_key && q.ender().zone == zone
            }
            _ => false,
        })
        .map(|(q, s)| (q.id.clone(), s))
        .collect()
}

/// Yellow "?" to turn in, yellow "!" to pick up, grey "?" while work is pending.
pub fn giver_mark(sim: &dyn SimRead, npc_key: &str, zone: &str) -> Option<(&'static str, Color)> {
    let qs = giver_quests(sim, npc_key, zone);
    let has = |s: QuestStatus| qs.iter().any(|(_, q)| *q == s);
    if has(QuestStatus::Ready) {
        Some(("?", GOLD))
    } else if has(QuestStatus::Available) {
        Some(("!", GOLD))
    } else if has(QuestStatus::Active) {
        Some(("?", GREY_MARK))
    } else {
        None
    }
}

/// Which quests the tracker follows: auto-tracked on accept, untracked from the log (J).
#[derive(Resource, Default)]
pub struct QuestTrack {
    pub untracked: Vec<String>,
    pub collapsed: bool,
    /// Quest the log (J) opens on, from a tracker title click.
    pub focus: Option<String>,
}

impl QuestTrack {
    pub fn is_tracked(&self, id: &str) -> bool {
        !self.untracked.iter().any(|u| u == id)
    }
}

/// Tracked open quests in accept order.
pub fn tracked<'a>(sim: &'a dyn SimRead, track: &QuestTrack) -> Vec<&'a combat_core::ActiveQuest> {
    sim.quests()
        .active
        .iter()
        .filter(|q| track.is_tracked(&q.id))
        .collect()
}

/// Unit keys the tracked quests still need (kill targets, droppers of wanted items, bosses of
/// zones to clear, NPCs to talk to): the minimap and map paint these in quest yellow.
pub fn objective_units(sim: &dyn SimRead, track: &QuestTrack) -> Vec<String> {
    let content = sim.content();
    let mut out: Vec<String> = Vec::new();
    let mut add = |k: &str| {
        if !out.iter().any(|o| o == k) {
            out.push(k.to_string());
        }
    };
    for q in tracked(sim, track) {
        let Some(def) = content.quest(&q.id) else {
            continue;
        };
        for (k, o) in def.objectives.iter().enumerate() {
            if q.counts.get(k).copied().unwrap_or(0) >= o.need() {
                continue;
            }
            match o {
                Objective::Kill(u, _) | Objective::Talk(u) => add(u),
                Objective::Loot(it, _) => {
                    for u in content
                        .units
                        .iter()
                        .filter(|u| u.loot.iter().any(|l| l.item == *it))
                    {
                        add(&u.key);
                    }
                }
                Objective::ClearZone(z) => {
                    if let Some(b) = content.zone(z).and_then(|z| z.boss.as_ref()) {
                        add(&b.unit);
                    }
                }
            }
        }
    }
    out
}

fn status_chip(s: QuestStatus) -> (&'static str, Color) {
    match s {
        QuestStatus::Available => ("TO DO", Color::srgb(0.7, 0.74, 0.82)),
        QuestStatus::Active => ("IN PROGRESS", Color::srgb(0.4, 0.65, 1.0)),
        QuestStatus::Ready => ("IN REVIEW", GOLD),
        QuestStatus::Done => ("DONE", DONE_GREEN),
        QuestStatus::Locked => ("BLOCKED", Color::srgb(1.0, 0.4, 0.35)),
    }
}

#[derive(Clone, PartialEq)]
enum Action {
    Select(String),
    Accept(String),
    Complete(String),
    Abandon(String),
    /// Gear reward pick in the open ticket.
    Choose(u8),
    ToggleTrack(String),
    /// Tracker title: open the log on that ticket.
    Open(String),
    Collapse,
    Back,
    Close,
    CloseLog,
}

#[derive(Component)]
struct ActionButton(Action);
#[derive(Component)]
struct DialogWindow;
#[derive(Component)]
struct LogWindow;
#[derive(Component)]
struct Tracker;
#[derive(Component)]
struct TrackerArea;
#[derive(Component)]
struct ProgressToast;
#[derive(Component)]
struct TrophyHint;
#[derive(Component)]
struct TrophyRow;

#[derive(Resource, Default)]
struct DialogState {
    selected: Option<String>,
    cursor: usize,
    choice: u8,
    shown: String,
}

#[derive(Resource, Default)]
struct Shown {
    tracker: String,
    log: String,
    toast_age: f32,
}

pub struct QuestsPlugin;

impl Plugin for QuestsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogState>()
            .init_resource::<Shown>()
            .init_resource::<QuestTrack>()
            .add_systems(Startup, spawn_quest_ui)
            .add_systems(
                Update,
                (
                    open_on_talk,
                    toggle_log,
                    handle_actions,
                    update_dialog,
                    update_log,
                    update_tracker,
                    quest_events,
                    trophy_hint,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn window_node(top: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(12),
        top: px(top),
        width: px(WINDOW_W),
        max_height: percent(78),
        padding: UiRect::all(px(10)),
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        border: UiRect {
            left: px(3),
            ..UiRect::all(px(1))
        },
        border_radius: BorderRadius::all(px(5)),
        overflow: Overflow::clip(),
        display: Display::None,
        ..default()
    }
}

fn spawn_quest_ui(mut commands: Commands) {
    commands.spawn((
        DialogWindow,
        window_node(132.0),
        BorderColor {
            left: GOLD,
            ..BorderColor::all(PANEL_EDGE)
        },
        panel_bg(),
        drop_shadow(),
        GlobalZIndex(32),
    ));
    commands.spawn((
        LogWindow,
        window_node(132.0),
        BorderColor {
            left: ACCENT,
            ..BorderColor::all(PANEL_EDGE)
        },
        panel_bg(),
        drop_shadow(),
        GlobalZIndex(31),
    ));
    // The area is laid out between the minimap and the bottom-right dock; its computed height
    // decides how many whole quests fit (`update_tracker`), whatever the screen size or UI scale.
    let area = commands
        .spawn((
            TrackerArea,
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                // Below the minimap and the player's aura row (top right).
                top: px(TRACKER_TOP),
                bottom: px(DOCK_RESERVE),
                width: px(TRACKER_W),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.spawn((
        Tracker,
        ChildOf(area),
        Node {
            padding: UiRect::all(px(8)),
            flex_direction: FlexDirection::Column,
            row_gap: px(3),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(PANEL_EDGE.with_alpha(0.5)),
        BackgroundColor(Color::srgba(0.03, 0.035, 0.05, 0.8)),
        GlobalZIndex(20),
    ));
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: percent(23),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_child((ProgressToast, label("", 18.0), TextColor(Color::NONE)));
}

fn text(c: &mut ChildSpawnerCommands, s: impl Into<String>, size: f32, color: Color) {
    c.spawn((
        label(s, size),
        TextColor(color),
        Node {
            max_width: percent(100),
            ..default()
        },
    ));
}

fn section(c: &mut ChildSpawnerCommands, title: &str) {
    c.spawn((
        label(format!("// {title}"), 11.0),
        TextColor(ACCENT.with_alpha(0.85)),
        Node {
            margin: UiRect::top(px(4)),
            ..default()
        },
    ));
}

fn button(c: &mut ChildSpawnerCommands, key: &str, caption: &str, action: Action, primary: bool) {
    let color = if primary { GOLD } else { DIM };
    c.spawn((
        ActionButton(action),
        Button,
        Node {
            padding: UiRect::axes(px(10), px(4)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(3)),
            column_gap: px(6),
            ..default()
        },
        BorderColor::all(color.with_alpha(0.8)),
        BackgroundColor(BAR_BG),
    ))
    .with_children(|b| {
        if !key.is_empty() {
            b.spawn((label(format!("[{key}]"), 12.0), TextColor(DIM)));
        }
        b.spawn((label(caption, 13.0), TextColor(color)));
    });
}

fn objectives(c: &mut ChildSpawnerCommands, content: &Content, sim: &dyn SimRead, id: &str) {
    let Some(def) = content.quest(id) else { return };
    let counts = sim
        .quests()
        .active
        .iter()
        .find(|q| q.id == id)
        .map(|q| q.counts.clone());
    let done = sim.quest_status(id) == QuestStatus::Done;
    for (k, o) in def.objectives.iter().enumerate() {
        let need = o.need();
        let have = if done {
            need
        } else {
            counts.as_ref().and_then(|c| c.get(k).copied()).unwrap_or(0)
        };
        let ok = have >= need;
        text(
            c,
            format!(
                "{} {}",
                if ok { "[x]" } else { "[ ]" },
                objective_line(content, o, have.min(need), need)
            ),
            13.0,
            if ok { DONE_GREEN } else { Color::WHITE },
        );
    }
    if def.group {
        text(
            c,
            "Group recommended: elite work. Bring a friend, or a lot of potions.",
            11.0,
            Color::srgb(1.0, 0.55, 0.25),
        );
    }
}

/// Reward block: XP and SP, the trophy (hover for its joke) and the "choose one" gear row.
fn reward(
    c: &mut ChildSpawnerCommands,
    sim: &dyn SimRead,
    id: &str,
    choice: u8,
    assets: &AssetServer,
    icons: &mut IconCache,
) {
    let content = sim.content();
    let Some(def) = content.quest(id) else { return };
    section(c, "reward");
    let sp = (u64::from(def.reward.xp) * u64::from(content.economy.quest_sp_pct) / 100) as u32
        + def.reward.sp;
    let xp = if def.reward.xp > 0 {
        format!("+{} XP   +{sp} SP", content.quest_xp(def))
    } else {
        "Exposure".into()
    };
    text(c, xp, 13.0, Color::srgb(0.75, 0.6, 1.0));
    let role = sim
        .player()
        .and_then(|p| sim.unit(p))
        .and_then(|u| content.class_of_unit(&u.key))
        .map(|c| c.role);
    let choices = content.quest_gear_choices(def, role);
    if !choices.is_empty() {
        text(c, "Choose one:", 12.0, DIM);
        for (k, name) in choices.iter().enumerate() {
            let picked = usize::from(choice).min(choices.len() - 1) == k;
            let lines = goods::tooltip(content, name, &[], role);
            c.spawn((
                ActionButton(Action::Choose(k as u8)),
                Button,
                Node {
                    column_gap: px(8),
                    padding: UiRect::all(px(4)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(if picked {
                    GOLD
                } else {
                    PANEL_EDGE.with_alpha(0.4)
                }),
                BackgroundColor(BAR_BG),
            ))
            .with_children(|r| {
                r.spawn((
                    ImageNode::new(icons.get(assets, goods::icon_of(content, name))),
                    Node {
                        width: px(30),
                        height: px(30),
                        ..default()
                    },
                ));
                r.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|t| {
                    // Name, rarity/slot line and the stat lines; the rest lives in the bag tooltip.
                    for (k, (line, color)) in lines.iter().take(4).enumerate() {
                        t.spawn((
                            label(line.clone(), if k == 0 { 13.0 } else { 11.0 }),
                            TextColor(*color),
                        ));
                    }
                });
            });
        }
    }
    let Some(item) = def.reward.trophy.as_deref().and_then(|t| content.item(t)) else {
        return;
    };
    c.spawn((
        TrophyRow,
        Button,
        Node {
            column_gap: px(8),
            align_items: AlignItems::Center,
            ..default()
        },
    ))
    .with_children(|r| {
        r.spawn((
            ImageNode::new(icons.get(assets, &item.icon)),
            Node {
                width: px(32),
                height: px(32),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BorderColor::all(GOLD),
        ));
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|t| {
            t.spawn((label(item.name.clone(), 13.0), TextColor(GOLD)));
            t.spawn((
                label(copy::bonus_text(&item.bonus.totals(1)), 12.0),
                TextColor(copy::BONUS_GREEN),
            ));
        });
    });
    c.spawn((
        TrophyHint,
        label(format!("\"{}\"", item.description), 12.0),
        TextColor(DIM),
        Node {
            display: Display::None,
            max_width: percent(100),
            ..default()
        },
    ));
}

fn open_on_talk(
    mut talked: MessageReader<Talked>,
    sim: SimView,
    flow: Res<GameFlow>,
    mut windows: ResMut<QuestWindows>,
    mut state: ResMut<DialogState>,
) {
    for Talked(id) in talked.read() {
        let Some(u) = sim.unit(*id) else { continue };
        let list = giver_quests(&*sim, &u.key, &flow.zone);
        if list.is_empty() {
            continue;
        }
        windows.dialog = Some(*id);
        windows.log = false;
        state.selected = (list.len() == 1).then(|| list[0].0.clone());
        state.cursor = 0;
        state.choice = 0;
        state.shown.clear();
    }
}

fn toggle_log(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    mut windows: ResMut<QuestWindows>,
) {
    if !paused.0 && binds.just(&keys, crate::keybinds::Action::QuestLog) {
        windows.log = !windows.log;
        if windows.log {
            windows.dialog = None;
        }
    }
}

fn handle_actions(
    keys: Res<ButtonInput<KeyCode>>,
    paused: Res<Paused>,
    sim: SimView,
    flow: Res<GameFlow>,
    mut windows: ResMut<QuestWindows>,
    mut state: ResMut<DialogState>,
    mut track: ResMut<QuestTrack>,
    mut pending: ResMut<PendingCommands>,
    buttons: Query<(&Interaction, &ActionButton), Changed<Interaction>>,
) {
    if paused.0 {
        return;
    }
    let mut actions: Vec<Action> = buttons
        .iter()
        .filter(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, b)| b.0.clone())
        .collect();
    // Keyboard drives the dialog only: Up/Down pick, Enter does the primary action, Backspace goes back.
    if let Some(npc) = windows.dialog.and_then(|id| sim.unit(id)) {
        let list = giver_quests(&*sim, &npc.key, &flow.zone);
        match &state.selected {
            None => {
                if keys.just_pressed(KeyCode::ArrowDown) {
                    state.cursor = (state.cursor + 1).min(list.len().saturating_sub(1));
                }
                if keys.just_pressed(KeyCode::ArrowUp) {
                    state.cursor = state.cursor.saturating_sub(1);
                }
                if keys.just_pressed(KeyCode::Enter)
                    && let Some((id, _)) = list.get(state.cursor)
                {
                    actions.push(Action::Select(id.clone()));
                }
            }
            Some(id) => {
                if keys.just_pressed(KeyCode::Enter) {
                    match sim.quest_status(id) {
                        QuestStatus::Available => actions.push(Action::Accept(id.clone())),
                        QuestStatus::Ready => actions.push(Action::Complete(id.clone())),
                        _ => actions.push(Action::Close),
                    }
                }
                if keys.just_pressed(KeyCode::Backspace) && list.len() > 1 {
                    actions.push(Action::Back);
                }
            }
        }
    }
    for action in actions {
        match action {
            Action::Select(id) => {
                state.selected = Some(id);
                state.choice = 0;
            }
            Action::Back => state.selected = None,
            Action::Close => windows.dialog = None,
            Action::CloseLog => {
                windows.log = false;
                track.focus = None;
            }
            Action::Abandon(quest) => pending.0.push(Command::AbandonQuest { quest }),
            Action::Choose(k) => state.choice = k,
            Action::ToggleTrack(id) => {
                if track.is_tracked(&id) {
                    track.untracked.push(id);
                } else {
                    track.untracked.retain(|u| *u != id);
                }
            }
            Action::Open(id) => {
                track.focus = Some(id);
                windows.log = true;
                windows.dialog = None;
            }
            Action::Collapse => track.collapsed = !track.collapsed,
            Action::Accept(ref quest) | Action::Complete(ref quest) => {
                pending.0.push(if matches!(action, Action::Accept(_)) {
                    Command::AcceptQuest {
                        quest: quest.clone(),
                    }
                } else {
                    Command::TurnInQuest {
                        quest: quest.clone(),
                        choice: state.choice,
                    }
                });
                // Back to the list when the giver has more to hand out, else done talking.
                let more = windows
                    .dialog
                    .and_then(|id| sim.unit(id))
                    .map(|npc| giver_quests(&*sim, &npc.key, &flow.zone))
                    .is_some_and(|l| {
                        l.iter().any(|(q, s)| {
                            q != quest && matches!(s, QuestStatus::Available | QuestStatus::Ready)
                        })
                    });
                if more {
                    state.selected = None;
                    state.cursor = 0;
                } else {
                    windows.dialog = None;
                }
            }
        }
    }
}

fn update_dialog(
    mut commands: Commands,
    sim: SimView,
    flow: Res<GameFlow>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut windows: ResMut<QuestWindows>,
    mut state: ResMut<DialogState>,
    player: Single<&Transform, With<Player>>,
    mut window: Single<(Entity, &mut Node), With<DialogWindow>>,
) {
    let (root, node) = &mut *window;
    let npc = windows.dialog.and_then(|id| sim.unit(id));
    let reach = sim.content().config.loot_range_m + combat_core::GIVER_REACH_SLACK_M;
    let near = npc.is_some_and(|u| {
        u.alive
            && Vec2::new(
                u.pos.x - player.translation.x,
                u.pos.z - player.translation.z,
            )
            .length()
                <= reach
    });
    let list = npc
        .map(|u| giver_quests(&*sim, &u.key, &flow.zone))
        .unwrap_or_default();
    if !near || list.is_empty() {
        windows.dialog = None;
        node.display = Display::None;
        state.shown.clear();
        return;
    }
    let Some(npc) = npc else { return };
    // A quest that left the list (turned in) falls back to the list view.
    if let Some(sel) = &state.selected
        && !list.iter().any(|(q, _)| q == sel)
    {
        state.selected = None;
    }
    node.display = Display::Flex;
    let content = sim.content();
    let counts: Vec<u32> = sim
        .quests()
        .active
        .iter()
        .flat_map(|q| q.counts.clone())
        .collect();
    let sig = format!(
        "{}|{:?}|{}|{}|{list:?}|{counts:?}",
        npc.id, state.selected, state.cursor, state.choice
    );
    if sig == state.shown {
        return;
    }
    state.shown = sig;
    commands.entity(*root).despawn_related::<Children>();
    let npc_name = npc.name.clone();
    let selected = state.selected.clone();
    let cursor = state.cursor;
    let choice = state.choice;
    commands.entity(*root).with_children(|c| {
        let Some(id) = selected else {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn((
                    label(
                        format!("~/jira/board?reporter={}", asset_key(&npc_name)),
                        12.0,
                    ),
                    TextColor(DIM),
                ));
                button(h, "", "x", Action::Close, false);
            });
            text(
                c,
                format!("{npc_name} has tickets for you"),
                17.0,
                Color::WHITE,
            );
            for (k, (q, s)) in list.iter().enumerate() {
                let (chip, color) = status_chip(*s);
                let name = content.quest(q).map_or(q.as_str(), |d| d.name.as_str());
                let mark = match s {
                    QuestStatus::Ready => ("?", GOLD),
                    QuestStatus::Available => ("!", GOLD),
                    _ => ("?", GREY_MARK),
                };
                c.spawn((
                    ActionButton(Action::Select(q.clone())),
                    Button,
                    Node {
                        column_gap: px(8),
                        padding: UiRect::axes(px(6), px(4)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(3)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(if k == cursor {
                        ACCENT
                    } else {
                        PANEL_EDGE.with_alpha(0.4)
                    }),
                    BackgroundColor(BAR_BG),
                ))
                .with_children(|r| {
                    r.spawn((label(mark.0, 16.0), TextColor(mark.1)));
                    r.spawn((label(ticket_id(content, q), 12.0), TextColor(DIM)));
                    r.spawn((label(name, 14.0), TextColor(Color::WHITE)));
                    r.spawn((label(chip, 10.0), TextColor(color)));
                });
            }
            text(c, "Up/Down pick | Enter open | Esc close", 11.0, DIM);
            return;
        };
        let Some(def) = content.quest(&id) else {
            return;
        };
        let status = sim.quest_status(&id);
        let (chip, chip_color) = status_chip(status);
        c.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            h.spawn(Node {
                column_gap: px(8),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|l| {
                l.spawn((label(ticket_id(content, &id), 12.0), TextColor(ACCENT)));
                l.spawn((label("Story", 11.0), TextColor(DIM)));
                // Bracketed text, not a bordered box: the box mis-sized around 10 px text.
                l.spawn((label(format!("[{chip}]"), 10.0), TextColor(chip_color)));
            });
            button(h, "", "x", Action::Close, false);
        });
        text(c, def.name.clone(), 19.0, Color::WHITE);
        text(
            c,
            format!("Reporter: {npc_name}   Assignee: you   Priority: Highest (again)"),
            11.0,
            DIM,
        );
        section(c, "description");
        let body = match status {
            QuestStatus::Active => &def.progress_text,
            QuestStatus::Ready => &def.done_text,
            _ => &def.text,
        };
        text(c, body.clone(), 13.0, Color::srgb(0.86, 0.89, 0.95));
        section(c, "acceptance criteria");
        objectives(c, content, &*sim, &id);
        reward(c, &*sim, &id, choice, &assets, &mut icons);
        c.spawn(Node {
            column_gap: px(8),
            margin: UiRect::top(px(6)),
            ..default()
        })
        .with_children(|b| {
            match status {
                QuestStatus::Available => {
                    button(b, "Enter", "Accept", Action::Accept(id.clone()), true);
                }
                QuestStatus::Ready => {
                    button(b, "Enter", "Complete", Action::Complete(id.clone()), true);
                }
                _ => {}
            }
            if list.len() > 1 {
                button(b, "Bksp", "Back", Action::Back, false);
            }
            button(b, "Esc", "Close", Action::Close, false);
        });
    });
}

fn update_log(
    mut commands: Commands,
    sim: SimView,
    windows: Res<QuestWindows>,
    track: Res<QuestTrack>,
    mut shown: ResMut<Shown>,
    mut window: Single<(Entity, &mut Node), With<LogWindow>>,
) {
    let (root, node) = &mut *window;
    if !windows.log {
        node.display = Display::None;
        shown.log.clear();
        return;
    }
    node.display = Display::Flex;
    let log = sim.quests();
    let sig = format!(
        "{:?}|{:?}|{:?}|{:?}",
        log.active, log.completed, track.untracked, track.focus
    );
    if sig == shown.log {
        return;
    }
    shown.log = sig;
    let content = sim.content();
    // The ticket clicked in the tracker comes first.
    let mut order: Vec<&combat_core::ActiveQuest> = log.active.iter().collect();
    if let Some(f) = &track.focus {
        order.sort_by_key(|q| q.id != *f);
    }
    commands.entity(*root).despawn_related::<Children>();
    commands.entity(*root).with_children(|c| {
        c.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|h| {
            h.spawn((label("~/jira  (J to close)", 14.0), TextColor(DIM)));
            button(h, "", "x", Action::CloseLog, false);
        });
        let tracked_n = log.active.iter().filter(|q| track.is_tracked(&q.id)).count();
        text(
            c,
            format!(
                "{} open ({tracked_n} tracked), {} closed. Velocity: whatever the dashboard needs it to be.",
                log.active.len(),
                log.completed.len()
            ),
            12.0,
            DIM,
        );
        if log.active.is_empty() {
            text(
                c,
                "Backlog empty. Look for a yellow ! before someone assigns you a chatbot.",
                13.0,
                Color::WHITE,
            );
        }
        for q in order {
            let Some(def) = content.quest(&q.id) else {
                continue;
            };
            let status = sim.quest_status(&q.id);
            let (chip, color) = status_chip(status);
            let focused = track.focus.as_deref() == Some(q.id.as_str());
            let is_tracked = track.is_tracked(&q.id);
            c.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    padding: UiRect::all(px(6)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BorderColor::all(if focused {
                    ACCENT
                } else {
                    PANEL_EDGE.with_alpha(0.6)
                }),
                BackgroundColor(BAR_BG),
            ))
            .with_children(|t| {
                t.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(6),
                    ..default()
                })
                .with_children(|h| {
                    h.spawn(Node {
                        column_gap: px(8),
                        align_items: AlignItems::Center,
                        flex_shrink: 1.0,
                        ..default()
                    })
                    .with_children(|l| {
                        l.spawn((label(ticket_id(content, &q.id), 11.0), TextColor(ACCENT)));
                        l.spawn((label(def.name.clone(), 14.0), TextColor(Color::WHITE)));
                        l.spawn((label(chip, 10.0), TextColor(color)));
                    });
                    h.spawn(Node {
                        column_gap: px(4),
                        ..default()
                    })
                    .with_children(|b| {
                        button(
                            b,
                            "",
                            if is_tracked { "Tracked" } else { "Track" },
                            Action::ToggleTrack(q.id.clone()),
                            is_tracked,
                        );
                        button(b, "", "Abandon", Action::Abandon(q.id.clone()), false);
                    });
                });
                objectives(t, content, &*sim, &q.id);
                let hint = if status == QuestStatus::Ready {
                    format!("Turn in to {}.", unit_name(content, &def.ender().npc))
                } else {
                    def.progress_text.clone()
                };
                text(t, hint, 11.0, DIM);
            });
        }
    });
}

fn update_tracker(
    mut commands: Commands,
    sim: SimView,
    windows: Res<QuestWindows>,
    trophies: Res<TrophiesOpen>,
    track: Res<QuestTrack>,
    touch: Res<TouchMode>,
    mut area: Single<(&mut Node, &ComputedNode), (With<TrackerArea>, Without<Tracker>)>,
    mut shown: ResMut<Shown>,
    mut tracker: Single<(Entity, &mut Node), With<Tracker>>,
) {
    let (area_node, computed) = &mut *area;
    // Touch: the right side holds the buttons, so the tracker moves under the player frame
    // and stops above the stick.
    let (left, right, top, bottom) = if touch.active {
        (px(12), Val::Auto, px(TOUCH_TOP), px(TOUCH_RESERVE))
    } else {
        (Val::Auto, px(12), px(TRACKER_TOP), px(DOCK_RESERVE))
    };
    if area_node.left != left || area_node.top != top {
        area_node.left = left;
        area_node.right = right;
        area_node.top = top;
        area_node.bottom = bottom;
    }
    let room = computed.size().y * computed.inverse_scale_factor();
    let (root, node) = &mut *tracker;
    let list = tracked(&*sim, &track);
    let visible = !list.is_empty() && !trophies.0 && !windows.map;
    node.display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    let max = if touch.active {
        TRACKER_MAX_TOUCH
    } else {
        TRACKER_MAX
    };
    // Whole quests only, as many as fit the area; before its first layout it reads 0: show all.
    let room = if room > 0.0 { room } else { f32::MAX };
    let sig = format!("{list:?}|{}|{max}|{room:.0}", track.collapsed);
    if !visible || sig == shown.tracker {
        return;
    }
    shown.tracker = sig;
    let content = sim.content();
    let mut used = LINE_PX * 2.0;
    let entries: Vec<(String, String, Vec<(String, Line)>)> = list
        .iter()
        .take(max)
        .filter_map(|q| {
            let def = content.quest(&q.id)?;
            Some((
                q.id.clone(),
                def.name.clone(),
                tracker_lines(content, def, &q.counts),
            ))
        })
        .take_while(|(_, _, lines)| {
            used += LINE_PX * (lines.len() + 1) as f32 + 3.0;
            used <= room
        })
        .collect();
    let more = list.len() - entries.len();
    let collapsed = track.collapsed;
    commands.entity(*root).despawn_related::<Children>();
    commands.entity(*root).with_children(|c| {
        c.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            h.spawn((
                label(format!("~/jira  Objectives ({})", list.len()), 12.0),
                TextColor(ACCENT),
            ));
            h.spawn((
                ActionButton(Action::Collapse),
                Button,
                Node {
                    padding: UiRect::axes(px(6), px(0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_child((
                label(if collapsed { "[+]" } else { "[-]" }, 13.0),
                TextColor(DIM),
            ));
        });
        if collapsed {
            return;
        }
        for (id, name, lines) in entries {
            let ready = lines.last().is_some_and(|(_, l)| *l == Line::Ready);
            c.spawn((
                ActionButton(Action::Open(id)),
                Button,
                Node {
                    margin: UiRect::top(px(3)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_child((
                label(name, 13.0),
                TextColor(if ready { GOLD } else { Color::WHITE }),
            ));
            for (line, state) in lines {
                let (mark, color) = match state {
                    Line::Pending => ("  - ", Color::srgb(0.82, 0.86, 0.92)),
                    Line::Done => ("  \u{2713} ", DONE_GREY),
                    Line::Ready => ("  ", GOLD),
                };
                text(c, format!("{mark}{line}"), 11.5, color);
            }
        }
        if more > 0 {
            text(c, format!("  +{more} more in ~/jira (J)"), 11.0, DIM);
        }
    });
}

fn quest_events(
    time: Res<Time>,
    sim: SimView,
    mut events: MessageReader<CombatEvent>,
    mut banners: MessageWriter<Banner>,
    mut shown: ResMut<Shown>,
    mut track: ResMut<QuestTrack>,
    mut toast: Single<(&mut Text, &mut TextColor), With<ProgressToast>>,
) {
    let content = sim.content();
    let (toast_text, toast_color) = &mut *toast;
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::QuestAccepted { quest } => track.untracked.retain(|u| u != quest),
            Event::QuestProgress {
                quest,
                objective,
                count,
                need,
            } => {
                if let Some(o) = content
                    .quest(quest)
                    .and_then(|q| q.objectives.get(*objective))
                {
                    set_text(toast_text, &progress_line(content, o, *count, *need));
                    shown.toast_age = 0.0;
                }
            }
            Event::QuestReady { quest } => {
                if let Some(def) = content.quest(quest) {
                    set_text(
                        toast_text,
                        &format!(
                            "{}: ready to close \u{2014} return to {}",
                            def.name,
                            unit_name(content, &def.ender().npc)
                        ),
                    );
                    shown.toast_age = 0.0;
                }
            }
            Event::QuestCompleted { quest } => {
                let Some(def) = content.quest(quest) else {
                    continue;
                };
                track.untracked.retain(|u| u != quest);
                banners.write(Banner {
                    title: format!("Ticket closed: {}", def.name),
                    subtitle: format!(
                        "+{} XP. Moved to Done. The AI will present it at the all-hands.",
                        content.quest_xp(def)
                    ),
                    color: DONE_GREEN,
                });
            }
            _ => {}
        }
    }
    shown.toast_age += time.delta_secs();
    let alpha = (1.0 - (shown.toast_age - TOAST_SECS * 0.6) / (TOAST_SECS * 0.4)).clamp(0.0, 1.0);
    toast_color.0 = GOLD.with_alpha(if toast_text.0.is_empty() { 0.0 } else { alpha });
    if alpha == 0.0 && !toast_text.0.is_empty() {
        toast_text.0.clear();
    }
}

fn trophy_hint(
    rows: Query<&Interaction, With<TrophyRow>>,
    mut hints: Query<&mut Node, With<TrophyHint>>,
    mut buttons: Query<(&Interaction, &mut BackgroundColor), With<ActionButton>>,
) {
    let hovered = rows.iter().any(|i| *i != Interaction::None);
    for mut n in &mut hints {
        let d = if hovered {
            Display::Flex
        } else {
            Display::None
        };
        if n.display != d {
            n.display = d;
        }
    }
    for (i, mut bg) in &mut buttons {
        let c = match i {
            Interaction::None => BAR_BG,
            _ => Color::srgb(0.16, 0.18, 0.24),
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat_core::Sim;

    fn sim() -> Sim {
        match crate::combat::new_sim() {
            Ok(s) => s,
            Err(e) => panic!("{e:?}"),
        }
    }

    #[test]
    fn markers_follow_quest_status() {
        let mut s = sim();
        s.spawn_player("fullstack", combat_core::Vec3::new(0.0, 0.0, 0.0), 0.0);
        assert_eq!(giver_mark(&s, "intern", "campus").map(|m| m.0), Some("!"));
        // The chain behind onboarding is locked, so Facilities has nothing yet.
        assert_eq!(giver_mark(&s, "facilities_guy", "campus"), None);
        s.debug_set_quest("onboarding", QuestStatus::Active);
        assert_eq!(giver_mark(&s, "intern", "campus"), Some(("?", GREY_MARK)));
        s.debug_set_quest("onboarding", QuestStatus::Ready);
        assert_eq!(giver_mark(&s, "intern", "campus"), Some(("?", GOLD)));
        assert!(giver_mark(&s, "intern", "lab/1").is_none());
    }

    #[test]
    fn tracker_lines_read_like_wow() {
        let s = sim();
        let c = s.content();
        let badges = Objective::Loot("Access Badge".into(), 4);
        assert_eq!(progress_line(c, &badges, 3, 4), "Access Badges 3/4");
        let invoices = Objective::Loot("Token Invoice (Itemized)".into(), 4);
        assert_eq!(
            progress_line(c, &invoices, 1, 4),
            "Token Invoice (Itemized) 1/4"
        );
        let proxies = Objective::Kill("meatproxy".into(), 10);
        assert_eq!(
            objective_line(c, &proxies, 5, 10),
            "MeatProxies terminated 5/10"
        );
        let talk = Objective::Talk("facilities_guy".into());
        assert_eq!(objective_line(c, &talk, 0, 1), "Talk to Facilities Guy");
        assert_eq!(plural("Agile Coach"), "Agile Coaches");
        assert_eq!(plural("Copilot Drone"), "Copilot Drones");
        assert_eq!(
            plural("Gantt Chart (Pure Fiction)"),
            "Gantt Chart (Pure Fiction)"
        );
        assert_eq!(ticket_id(c, "onboarding"), "SPRINT-101");
        assert!(
            objective_label(c, &Objective::ClearZone("tower/1".into())).contains("Tower floor 1")
        );
        // Mid-progress: pending and done lines; all done adds the gold turn-in line.
        let Some(demo) = c.quest("demo_day") else {
            panic!("no demo_day")
        };
        let lines = tracker_lines(c, demo, &[3, 1]);
        assert_eq!(
            lines,
            [
                ("VC Partners terminated 3/3".to_string(), Line::Done),
                ("Demo Presenters terminated 1/3".to_string(), Line::Pending),
            ]
        );
        let lines = tracker_lines(c, demo, &[3, 3]);
        assert_eq!(
            lines.last(),
            Some(&(
                "Ready to close \u{2014} return to Lab Safety Officer".to_string(),
                Line::Ready
            ))
        );
        // A breadcrumb is closed at its target, not its giver.
        let Some(crumb) = c.quest("escalation_path") else {
            panic!("no escalation_path")
        };
        assert!(
            tracker_lines(c, crumb, &[1])
                .last()
                .is_some_and(|l| l.0.ends_with("IT Support Lead"))
        );
    }

    #[test]
    fn tracking_follows_accepts_and_untracks() {
        let mut s = sim();
        s.spawn_player("fullstack", combat_core::Vec3::new(0.0, 0.0, 0.0), 0.0);
        s.debug_set_quest("onboarding", QuestStatus::Active);
        s.debug_set_quest("coffee_run", QuestStatus::Active);
        let mut track = QuestTrack::default();
        assert_eq!(tracked(&s, &track).len(), 2);
        track.untracked.push("coffee_run".into());
        let ids: Vec<&str> = tracked(&s, &track).iter().map(|q| q.id.as_str()).collect();
        assert_eq!(ids, ["onboarding"]);
        // Onboarding wants Facilities: his key is a highlighted objective unit.
        assert_eq!(objective_units(&s, &track), ["facilities_guy"]);
    }
}
