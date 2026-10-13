//! Quests in Jira clothes: the giver dialog (a ticket with Accept / Complete), the
//! "Sprint backlog" tracker on the right, progress toasts, the "Ticket closed" banner and the
//! `~/jira` log (J). Everything is read from the sim each frame; widgets are rebuilt only when
//! the shown state changes.

use super::{
    ACCENT, BAR_BG, Banner, DIM, IconCache, PANEL_EDGE, asset_key, drop_shadow, label, panel_bg,
    set_text,
};
use crate::combat::{CombatEvent, CombatSim, PendingCommands, SimReady, Talked};
use crate::copy;
use crate::game::{GameFlow, zone_label};
use crate::input::{Paused, QuestWindows, TrophiesOpen};
use crate::player::Player;
use bevy::prelude::*;
use combat_core::{Command, Content, Event, Objective, QuestStatus, Sim};

pub const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);
pub const GREY_MARK: Color = Color::srgb(0.62, 0.64, 0.7);
const DONE_GREEN: Color = Color::srgb(0.4, 1.0, 0.5);
const WINDOW_W: f32 = 460.0;
const TRACKER_W: f32 = 300.0;
const TRACKER_MAX: usize = 5;
const TOAST_SECS: f32 = 2.5;

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

/// What an objective asks for, without the count.
pub fn objective_label(content: &Content, o: &Objective) -> String {
    match o {
        Objective::Kill(k, _) => format!("{} neutralised", unit_name(content, k)),
        Objective::Loot(item, _) => item.clone(),
        Objective::Talk(k) => format!("Sync with {} (in person)", unit_name(content, k)),
        Objective::ClearZone(z) => format!("Clear {}", zone_label(content, z)),
    }
}

/// "3/5 Access Badges" for the progress toast.
pub fn progress_line(content: &Content, o: &Objective, count: u32, need: u32) -> String {
    let mut what = objective_label(content, o);
    if matches!(o, Objective::Loot(..))
        && need > 1
        && what.ends_with(|c: char| c.is_ascii_lowercase() && c != 's')
    {
        what.push('s');
    }
    format!("{count}/{need} {what}")
}

/// The giver's quests the player can act on in `zone`, content order.
pub fn giver_quests(sim: &Sim, npc_key: &str, zone: &str) -> Vec<(String, QuestStatus)> {
    sim.content()
        .quests
        .iter()
        .filter(|q| q.giver.npc == npc_key && q.giver.zone == zone)
        .map(|q| (q.id.clone(), sim.quest_status(&q.id)))
        .filter(|(_, s)| {
            matches!(
                s,
                QuestStatus::Available | QuestStatus::Active | QuestStatus::Ready
            )
        })
        .collect()
}

/// Yellow "?" to turn in, yellow "!" to pick up, grey "?" while work is pending.
pub fn giver_mark(sim: &Sim, npc_key: &str, zone: &str) -> Option<(&'static str, Color)> {
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
struct ProgressToast;
#[derive(Component)]
struct TrophyHint;
#[derive(Component)]
struct TrophyRow;

#[derive(Resource, Default)]
struct DialogState {
    selected: Option<String>,
    cursor: usize,
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
    commands.spawn((
        Tracker,
        Node {
            position_type: PositionType::Absolute,
            right: px(12),
            // Below the player's aura row (top right).
            top: px(96),
            width: px(TRACKER_W),
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

fn objectives(c: &mut ChildSpawnerCommands, content: &Content, sim: &Sim, id: &str) {
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
                "{} {}  {have}/{need}",
                if ok { "[x]" } else { "[ ]" },
                objective_label(content, o)
            ),
            13.0,
            if ok { DONE_GREEN } else { Color::WHITE },
        );
    }
}

fn reward(
    c: &mut ChildSpawnerCommands,
    content: &Content,
    id: &str,
    assets: &AssetServer,
    icons: &mut IconCache,
) {
    let Some(def) = content.quest(id) else { return };
    section(c, "reward");
    let xp = if def.reward.xp > 0 {
        format!("+{} XP", def.reward.xp)
    } else {
        "Exposure".into()
    };
    text(c, xp, 13.0, Color::srgb(0.75, 0.6, 1.0));
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
    sim: Res<CombatSim>,
    flow: Res<GameFlow>,
    mut windows: ResMut<QuestWindows>,
    mut state: ResMut<DialogState>,
) {
    for Talked(id) in talked.read() {
        let Some(u) = sim.0.unit(*id) else { continue };
        let list = giver_quests(&sim.0, &u.key, &flow.zone);
        if list.is_empty() {
            continue;
        }
        windows.dialog = Some(*id);
        windows.log = false;
        state.selected = (list.len() == 1).then(|| list[0].0.clone());
        state.cursor = 0;
        state.shown.clear();
    }
}

fn toggle_log(
    keys: Res<ButtonInput<KeyCode>>,
    paused: Res<Paused>,
    mut windows: ResMut<QuestWindows>,
) {
    if !paused.0 && keys.just_pressed(KeyCode::KeyJ) {
        windows.log = !windows.log;
        if windows.log {
            windows.dialog = None;
        }
    }
}

fn handle_actions(
    keys: Res<ButtonInput<KeyCode>>,
    paused: Res<Paused>,
    sim: Res<CombatSim>,
    flow: Res<GameFlow>,
    mut windows: ResMut<QuestWindows>,
    mut state: ResMut<DialogState>,
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
    if let Some(npc) = windows.dialog.and_then(|id| sim.0.unit(id)) {
        let list = giver_quests(&sim.0, &npc.key, &flow.zone);
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
                    match sim.0.quest_status(id) {
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
            Action::Select(id) => state.selected = Some(id),
            Action::Back => state.selected = None,
            Action::Close => windows.dialog = None,
            Action::CloseLog => windows.log = false,
            Action::Abandon(quest) => pending.0.push(Command::AbandonQuest { quest }),
            Action::Accept(ref quest) | Action::Complete(ref quest) => {
                pending.0.push(if matches!(action, Action::Accept(_)) {
                    Command::AcceptQuest {
                        quest: quest.clone(),
                    }
                } else {
                    Command::TurnInQuest {
                        quest: quest.clone(),
                    }
                });
                // Back to the list when the giver has more to hand out, else done talking.
                let more = windows
                    .dialog
                    .and_then(|id| sim.0.unit(id))
                    .map(|npc| giver_quests(&sim.0, &npc.key, &flow.zone))
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
    sim: Res<CombatSim>,
    flow: Res<GameFlow>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut windows: ResMut<QuestWindows>,
    mut state: ResMut<DialogState>,
    player: Single<&Transform, With<Player>>,
    mut window: Single<(Entity, &mut Node), With<DialogWindow>>,
) {
    let (root, node) = &mut *window;
    let npc = windows.dialog.and_then(|id| sim.0.unit(id));
    let reach = sim.0.content().config.loot_range_m + combat_core::GIVER_REACH_SLACK_M;
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
        .map(|u| giver_quests(&sim.0, &u.key, &flow.zone))
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
    let content = sim.0.content();
    let counts: Vec<u32> = sim
        .0
        .quests()
        .active
        .iter()
        .flat_map(|q| q.counts.clone())
        .collect();
    let sig = format!(
        "{}|{:?}|{}|{list:?}|{counts:?}",
        npc.id, state.selected, state.cursor
    );
    if sig == state.shown {
        return;
    }
    state.shown = sig;
    commands.entity(*root).despawn_related::<Children>();
    let npc_name = npc.name.clone();
    let selected = state.selected.clone();
    let cursor = state.cursor;
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
        let status = sim.0.quest_status(&id);
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
        objectives(c, content, &sim.0, &id);
        reward(c, content, &id, &assets, &mut icons);
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
    sim: Res<CombatSim>,
    windows: Res<QuestWindows>,
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
    let log = sim.0.quests();
    let sig = format!("{:?}|{:?}", log.active, log.completed);
    if sig == shown.log {
        return;
    }
    shown.log = sig;
    let content = sim.0.content();
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
        text(
            c,
            format!(
                "{} open, {} closed. Velocity: whatever the dashboard needs it to be.",
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
        for q in &log.active {
            let Some(def) = content.quest(&q.id) else {
                continue;
            };
            let status = sim.0.quest_status(&q.id);
            let (chip, color) = status_chip(status);
            c.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    padding: UiRect::all(px(6)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE.with_alpha(0.6)),
                BackgroundColor(BAR_BG),
            ))
            .with_children(|t| {
                t.spawn(Node {
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
                        l.spawn((label(ticket_id(content, &q.id), 11.0), TextColor(ACCENT)));
                        l.spawn((label(def.name.clone(), 14.0), TextColor(Color::WHITE)));
                        l.spawn((label(chip, 10.0), TextColor(color)));
                    });
                    button(h, "", "Abandon", Action::Abandon(q.id.clone()), false);
                });
                objectives(t, content, &sim.0, &q.id);
                let hint = if status == QuestStatus::Ready {
                    format!("Turn in to {}.", unit_name(content, &def.giver.npc))
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
    sim: Res<CombatSim>,
    windows: Res<QuestWindows>,
    trophies: Res<TrophiesOpen>,
    mut shown: ResMut<Shown>,
    mut tracker: Single<(Entity, &mut Node), With<Tracker>>,
) {
    let (root, node) = &mut *tracker;
    let log = sim.0.quests();
    let visible = !log.active.is_empty() && !trophies.0 && !windows.map;
    node.display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    let sig = format!("{:?}", log.active);
    if !visible || sig == shown.tracker {
        return;
    }
    shown.tracker = sig;
    let content = sim.0.content();
    commands.entity(*root).despawn_related::<Children>();
    commands.entity(*root).with_children(|c| {
        c.spawn((label("~/jira - Sprint backlog", 12.0), TextColor(ACCENT)));
        for q in log.active.iter().take(TRACKER_MAX) {
            let Some(def) = content.quest(&q.id) else {
                continue;
            };
            let ready = sim.0.quest_status(&q.id) == QuestStatus::Ready;
            text(
                c,
                def.name.clone(),
                13.0,
                if ready { GOLD } else { Color::WHITE },
            );
            if ready {
                text(
                    c,
                    format!("  -> review with {}", unit_name(content, &def.giver.npc)),
                    11.0,
                    GOLD,
                );
                continue;
            }
            for (o, have) in def.objectives.iter().zip(&q.counts) {
                let need = o.need();
                let ok = *have >= need;
                text(
                    c,
                    format!(
                        "  {} {have}/{need} {}",
                        if ok { "[x]" } else { "[ ]" },
                        objective_label(content, o)
                    ),
                    11.0,
                    if ok {
                        DONE_GREEN
                    } else {
                        Color::srgb(0.8, 0.84, 0.9)
                    },
                );
            }
        }
        if log.active.len() > TRACKER_MAX {
            text(
                c,
                format!("  +{} more in ~/jira (J)", log.active.len() - TRACKER_MAX),
                11.0,
                DIM,
            );
        }
    });
}

fn quest_events(
    time: Res<Time>,
    sim: Res<CombatSim>,
    mut events: MessageReader<CombatEvent>,
    mut banners: MessageWriter<Banner>,
    mut shown: ResMut<Shown>,
    mut toast: Single<(&mut Text, &mut TextColor), With<ProgressToast>>,
) {
    let content = sim.0.content();
    let (toast_text, toast_color) = &mut *toast;
    for CombatEvent(ev) in events.read() {
        match ev {
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
            Event::QuestCompleted { quest } => {
                let Some(def) = content.quest(quest) else {
                    continue;
                };
                banners.write(Banner {
                    title: format!("Ticket closed: {}", def.name),
                    subtitle: format!(
                        "+{} XP. Moved to Done. The AI will present it at the all-hands.",
                        def.reward.xp
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

    fn sim() -> Sim {
        match crate::combat::new_sim() {
            Ok(s) => s,
            Err(e) => panic!("{e:?}"),
        }
    }

    #[test]
    fn markers_follow_quest_status() {
        let mut s = sim();
        s.spawn_player(combat_core::Vec3::new(0.0, 0.0, 0.0), 0.0);
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
    fn progress_lines_read_like_the_tracker() {
        let s = sim();
        let c = s.content();
        let badges = Objective::Loot("Access Badge".into(), 4);
        assert_eq!(progress_line(c, &badges, 3, 4), "3/4 Access Badges");
        let invoices = Objective::Loot("Token Invoice (Itemized)".into(), 4);
        assert_eq!(
            progress_line(c, &invoices, 1, 4),
            "1/4 Token Invoice (Itemized)"
        );
        assert_eq!(ticket_id(c, "onboarding"), "SPRINT-101");
        assert!(
            objective_label(c, &Objective::ClearZone("tower/1".into())).contains("Tower floor 1")
        );
    }
}
