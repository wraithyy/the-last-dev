//! `~/calendar`: the Company Calendar window, the one entry point for side activities. A
//! week-view list in two groups, Minigames and Quest chains; each row has a name and a joke,
//! a length estimate, a status (new, in progress, best medal), its reward icons and one big
//! button. Opened by using the board on the campus plaza, its key (`.`), the touch Menu
//! drawer or the `~/endgame` panel. Starting a minigame sends `Command::StartMinigame` (the
//! group follows the lead); starting a chain accepts its first quest, which the tracker picks
//! up. Esc closes it. Also here: the one-time tracker hint at level 2, the "!" news flag the
//! minimap and the map show on the board, and the log line for closed Jira tickets.
//! Data: `content.calendar` (`combat_core::calendar`); a minigame row turns from "Coming soon"
//! into a playable one as soon as its `MinigameDef` exists.
//! QA: `WOW_CALENDAR=1` opens it at start; `WOW_SCROLL=end` (see `hud/scroll.rs`) scrolls the list down.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, CombatSim, PendingCommands, SimReady, SimView};
use crate::game::GameFlow;
use crate::hud::quests::QuestTrack;
use crate::hud::scroll::{ScrollHint, ScrollList};
use crate::hud::{
    ACCENT, ACCENT_WARM, DIM, GOLD, IconCache, KEY_RIM, LogLineMsg, PANEL_EDGE, drop_shadow, label,
};
use crate::input::{EndgameOpen, MinigameWindows, Paused, TextFocus};
use crate::keybinds::{Action, Keybinds};
use crate::net::NetWorld;
use crate::touch::TouchMode;
use bevy::prelude::*;
use combat_core::{CalendarReward, ChainStatus, Command, Event, Medal};
use std::collections::HashMap;

const WIDTH: f32 = 860.0;
const GREEN: Color = Color::srgb(0.4, 1.0, 0.5);
/// How long the level-2 hint stays in the tracker unless the calendar is opened first.
const HINT_SECS: f32 = 240.0;
const LINE_PX: f32 = 30.0;
/// Week-view slots the rows are pinned to; pure decoration, like most calendars.
const GAME_SLOTS: [(&str, &str); 6] = [
    ("MON", "09:30"),
    ("TUE", "14:00"),
    ("WED", "10:00"),
    ("THU", "16:00"),
    ("FRI", "11:00"),
    ("FRI", "16:30"),
];
const CHAIN_SLOTS: [(&str, &str); 5] = [
    ("MON", "11:00"),
    ("TUE", "09:00"),
    ("WED", "15:00"),
    ("THU", "08:30"),
    ("FRI", "13:00"),
];

/// Calendar state other HUD parts read: the tracker hint and the board's "!" flag.
#[derive(Resource, Default)]
pub struct CalendarState {
    /// One tracker line, shown once after reaching level 2.
    pub hint: Option<String>,
    hint_until: f32,
    /// Something on the calendar was never played: the board gets a "!".
    pub news: bool,
    /// Best medals seen this session (online the save lives elsewhere).
    best: HashMap<String, Medal>,
    /// UTC day of the world clock, for `rotation` (0 while the clock is off, like the sim).
    day: u32,
}

/// Opens the calendar when pressed (the `~/endgame` panel carries one).
#[derive(Component)]
pub struct OpenCalendar;

#[derive(Component)]
struct CalendarWindow;
#[derive(Component)]
struct CalendarBody;

#[derive(Component, Clone)]
enum CalButton {
    Game(String),
    Chain(String),
    Track(String),
    Close,
}

pub struct CalendarPlugin;

impl Plugin for CalendarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CalendarState>()
            .add_systems(Startup, spawn_window)
            .add_systems(
                Update,
                (track_events, toggle, buttons, rebuild, news)
                    .chain()
                    .in_set(SimReady),
            );
        if std::env::var_os("WOW_CALENDAR").is_some() {
            app.insert_resource(MinigameWindows {
                calendar: true,
                ..default()
            });
        }
    }
}

fn spawn_window(mut commands: Commands) {
    commands
        .spawn((
            CalendarWindow,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: px(48),
                width: px(WIDTH),
                max_width: percent(96),
                max_height: percent(90),
                margin: UiRect::left(px(-WIDTH / 2.0)),
                padding: UiRect::all(px(10)),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgb(0.85, 0.3, 0.28).with_alpha(0.85)),
            // Opaque: a long list over a busy scene must stay readable.
            BackgroundColor(Color::srgb(0.045, 0.05, 0.075)),
            drop_shadow(),
            GlobalZIndex(33),
            // A press on the panel belongs to the window, not to the camera or the touch stick.
            Button,
        ))
        .with_child((
            CalendarBody,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                min_height: px(0),
                ..default()
            },
        ));
}

/// Medals, chain turns and the level-2 hint; ticket turn-ins go to the log.
fn track_events(
    time: Res<Time>,
    sim: SimView,
    touch: Res<TouchMode>,
    binds: Res<Keybinds>,
    mut st: ResMut<CalendarState>,
    mut events: MessageReader<CombatEvent>,
    mut log: MessageWriter<LogLineMsg>,
) {
    let me = sim.player();
    let now = time.elapsed_secs();
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::MinigameEnded { result } if Some(result.unit) == me => {
                if let Some(m) = result.medal {
                    let best = st.best.entry(result.key.clone()).or_insert(m);
                    *best = (*best).max(m);
                }
            }
            Event::LevelUp { unit, level: 2 } if Some(*unit) == me => {
                let how = if touch.active {
                    "Menu > Calendar".to_string()
                } else {
                    format!("key {}", binds.label(Action::Calendar))
                };
                st.hint = Some(format!(
                    "New: Company Calendar by the plaza arch ({how}): side quests, minigames"
                ));
                st.hint_until = now + HINT_SECS;
            }
            Event::TicketsClosed { unit, count, sp } if Some(*unit) == me => {
                let s = if *count == 1 { "" } else { "s" };
                log.write(LogLineMsg(
                    format!("[jira] Closed {count} ticket{s} without reading them. +{sp} Kudos."),
                    GOLD,
                ));
            }
            _ => {}
        }
    }
    if st.hint.is_some() && now > st.hint_until {
        st.hint = None;
    }
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<Paused>,
    focus: Res<TextFocus>,
    mut windows: ResMut<MinigameWindows>,
    mut endgame: ResMut<EndgameOpen>,
    mut st: ResMut<CalendarState>,
    links: Query<&Interaction, (With<OpenCalendar>, Changed<Interaction>)>,
) {
    if !paused.0 && !focus.0 && binds.just(&keys, Action::Calendar) {
        windows.calendar = !windows.calendar;
    }
    if links.iter().any(|i| *i == Interaction::Pressed) {
        windows.calendar = true;
        endgame.0 = false;
    }
    if windows.calendar {
        st.hint = None;
    }
}

fn buttons(
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    mut windows: ResMut<MinigameWindows>,
    mut track: ResMut<QuestTrack>,
    mut log: MessageWriter<LogLineMsg>,
    pressed: Query<(&Interaction, &CalButton), Changed<Interaction>>,
) {
    let content = sim.content();
    for (i, b) in &pressed {
        if *i != Interaction::Pressed {
            continue;
        }
        match b {
            CalButton::Game(key) => {
                pending.0.push(Command::StartMinigame { key: key.clone() });
                windows.calendar = false;
            }
            CalButton::Chain(key) => {
                let Some(ch) = content.calendar.chains.iter().find(|c| c.key == *key) else {
                    continue;
                };
                if let Some(first) = ch.quests.first() {
                    pending.0.push(Command::AcceptQuest {
                        quest: first.clone(),
                    });
                    track.untracked.retain(|u| u != first);
                    log.write(LogLineMsg(
                        format!("[calendar] {} added to your tracker.", ch.name),
                        ACCENT,
                    ));
                }
                windows.calendar = false;
            }
            CalButton::Track(quest) => {
                track.untracked.retain(|u| u != quest);
                windows.calendar = false;
            }
            CalButton::Close => windows.calendar = false,
        }
    }
}

fn reward_icon(r: CalendarReward) -> (&'static str, &'static str) {
    match r {
        CalendarReward::Kudos => ("coins", "Kudos"),
        CalendarReward::Rep => ("chart_up", "Standing"),
        CalendarReward::Title => ("badge", "Title"),
        CalendarReward::Trophy => ("gear_trophy", "Trophy"),
        CalendarReward::Pet => ("rubber_duck", "Pet"),
        CalendarReward::Gear => ("gear_hoodie", "Gear"),
        CalendarReward::Relief => ("wellness", "Less burnout"),
        CalendarReward::Burnout => ("battery_low", "Costs burnout"),
    }
}

/// What a row shows on the right: a pressable button, or a dim note in its place.
enum Act {
    Button(CalButton, String),
    Note(String),
}

struct Row {
    slot: (&'static str, &'static str),
    name: String,
    blurb: String,
    length: String,
    status: (String, Color),
    rewards: Vec<CalendarReward>,
    act: Act,
}

fn rows(
    sim: &SimView,
    offline: &CombatSim,
    online: bool,
    zone: &str,
    st: &CalendarState,
) -> (Vec<Row>, Vec<Row>) {
    let content = sim.content();
    let me = sim.player();
    let lead = sim.players().first() == me.as_ref();
    let solo = sim.players().len() <= 1;
    let (games, chains) = content.calendar.listing(st.day);
    let best = |key: &str| -> Option<Medal> {
        let saved = match (online, me) {
            (false, Some(p)) => offline.0.minigame_best(p, key),
            _ => None,
        };
        saved.max(st.best.get(key).copied())
    };
    let games = games
        .into_iter()
        .enumerate()
        .map(|(k, g)| {
            let playable = content.minigame(&g.key).is_some();
            let status = match (playable, best(&g.key)) {
                (false, _) => ("COMING SOON".to_string(), DIM),
                (true, None) => ("NEW".to_string(), ACCENT),
                (true, Some(m)) => {
                    let name = content
                        .minigame(&g.key)
                        .and_then(|d| d.medal(m))
                        .map_or_else(|| format!("{m:?}"), |d| d.name.clone());
                    (
                        format!("BEST: {}", name.to_uppercase()),
                        crate::minigame::medal_color(m),
                    )
                }
            };
            let act = if !playable {
                Act::Note("Soon".into())
            } else if zone != "campus" {
                Act::Note("From campus".into())
            } else if !lead {
                Act::Note("Lead starts".into())
            } else {
                let text = match (best(&g.key).is_some(), solo) {
                    (_, false) => "Start (group)",
                    (true, true) => "Play again",
                    (false, true) => "Start",
                };
                Act::Button(CalButton::Game(g.key.clone()), text.into())
            };
            Row {
                slot: GAME_SLOTS[k % GAME_SLOTS.len()],
                name: g.name.clone(),
                blurb: g.blurb.clone(),
                length: g.length.clone(),
                status,
                rewards: g.rewards.clone(),
                act,
            }
        })
        .collect();
    let log = sim.quests();
    let chains = chains
        .into_iter()
        .enumerate()
        .map(|(k, ch)| {
            let steps = ch.quests.len();
            let done = ch
                .quests
                .iter()
                .filter(|q| log.completed.contains(q))
                .count();
            let (status, act) = match ch.status(log) {
                ChainStatus::New => (
                    ("NEW".to_string(), ACCENT),
                    Act::Button(CalButton::Chain(ch.key.clone()), "Start".into()),
                ),
                ChainStatus::Done => (("DONE".to_string(), GREEN), Act::Note("Closed".into())),
                ChainStatus::InProgress => {
                    let cur = ch.current(log).unwrap_or_default();
                    let act = if log.active.iter().any(|a| a.id == cur) {
                        Act::Button(CalButton::Track(cur.to_string()), "Track".into())
                    } else {
                        let giver = content
                            .quest(cur)
                            .map(|q| crate::hud::unit_name(content, &q.giver.npc))
                            .unwrap_or_default();
                        Act::Note(format!("See the {giver}"))
                    };
                    (
                        (
                            format!("IN PROGRESS {}/{steps}", (done + 1).min(steps)),
                            ACCENT_WARM,
                        ),
                        act,
                    )
                }
            };
            Row {
                slot: CHAIN_SLOTS[k % CHAIN_SLOTS.len()],
                name: ch.name.clone(),
                blurb: ch.blurb.clone(),
                length: ch.length.clone(),
                status,
                rewards: ch.rewards.clone(),
                act,
            }
        })
        .collect();
    (games, chains)
}

fn rebuild(
    mut commands: Commands,
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    flow: Res<GameFlow>,
    st: Res<CalendarState>,
    touch: Res<TouchMode>,
    windows: Res<MinigameWindows>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut window: Single<(&mut Node, &mut GlobalZIndex), With<CalendarWindow>>,
    body: Single<Entity, With<CalendarBody>>,
    mut shown: Local<String>,
) {
    let (window, z) = &mut *window;
    let want = if windows.calendar {
        Display::Flex
    } else {
        Display::None
    };
    if window.display != want {
        window.display = want;
    }
    if !windows.calendar {
        shown.clear();
        return;
    }
    // Touch: the window starts at the top edge, above the Menu button that would cover Close.
    let (top, zi) = if touch.active {
        (px(6), 57)
    } else {
        (px(48), 33)
    };
    if window.top != top {
        window.top = top;
    }
    if z.0 != zi {
        z.0 = zi;
    }
    let (games, chains) = rows(&sim, &offline, net.is_some(), &flow.zone, &st);
    let sig = format!(
        "{:?}|{}|{}",
        games
            .iter()
            .chain(&chains)
            .map(|r| (&r.status.0, act_sig(&r.act)))
            .collect::<Vec<_>>(),
        flow.zone,
        touch.active
    );
    if *shown == sig {
        return;
    }
    *shown = sig;
    let mut used: Vec<CalendarReward> = Vec::new();
    for r in games.iter().chain(&chains).flat_map(|r| &r.rewards) {
        if !used.contains(r) {
            used.push(*r);
        }
    }
    commands.entity(*body).despawn_related::<Children>();
    commands.entity(*body).with_children(|c| {
        c.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            h.spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|t| {
                t.spawn((label("~/calendar  Company Calendar", 18.0), TextColor(GOLD)));
                t.spawn((
                    label(
                        "Everything is on, every day. Attendance optional (mandatory).",
                        12.0,
                    ),
                    TextColor(DIM),
                ));
            });
            h.spawn(button(CalButton::Close, "Close (Esc)", false));
        });
        let mut list = c.spawn((
            ScrollList::default(),
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                overflow: Overflow::scroll_y(),
                max_height: px(if touch.active { 330.0 } else { 640.0 }),
                margin: UiRect::top(px(6)),
                ..default()
            },
        ));
        let list_id = list.id();
        list.with_children(|l| {
            group(l, "MINIGAMES", "solo or co-op, the group lead starts");
            for r in &games {
                row(l, r, &assets, &mut icons);
            }
            group(l, "QUEST CHAINS", "start here, then follow the tracker");
            for r in &chains {
                row(l, r, &assets, &mut icons);
            }
        });
        c.spawn(Node {
            margin: UiRect::top(px(6)),
            column_gap: px(10),
            row_gap: px(2),
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|f| {
            f.spawn((label("Rewards:", 11.0), TextColor(DIM)));
            for r in used {
                let (icon, word) = reward_icon(r);
                f.spawn(Node {
                    column_gap: px(3),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|p| {
                    p.spawn((
                        ImageNode::new(icons.get(&assets, icon)),
                        Node {
                            width: px(14),
                            height: px(14),
                            ..default()
                        },
                    ));
                    p.spawn((label(word, 11.0), TextColor(DIM)));
                });
            }
            f.spawn((
                label(
                    "Jira tickets from loot: use this board to close them.",
                    11.0,
                ),
                TextColor(DIM),
            ));
            f.spawn((ScrollHint(list_id), label("", 12.0), TextColor(ACCENT_WARM)));
        });
    });
}

fn act_sig(a: &Act) -> String {
    match a {
        Act::Button(_, t) | Act::Note(t) => t.clone(),
    }
}

fn group(l: &mut ChildSpawnerCommands, title: &str, sub: &str) {
    l.spawn(Node {
        margin: UiRect::top(px(6)),
        padding: UiRect::bottom(px(2)),
        column_gap: px(10),
        align_items: AlignItems::Baseline,
        border: UiRect::bottom(px(1)),
        flex_shrink: 0.0,
        ..default()
    })
    .insert(BorderColor::all(PANEL_EDGE))
    .with_children(|g| {
        g.spawn((label(title, 13.0), TextColor(ACCENT)));
        g.spawn((label(sub, 11.0), TextColor(DIM)));
    });
}

fn row(l: &mut ChildSpawnerCommands, r: &Row, assets: &AssetServer, icons: &mut IconCache) {
    let live = !matches!(r.status.0.as_str(), "COMING SOON" | "DONE");
    l.spawn((
        Node {
            column_gap: px(10),
            align_items: AlignItems::Center,
            padding: UiRect::axes(px(6), px(4)),
            border_radius: BorderRadius::all(px(4)),
            min_height: px(LINE_PX + 18.0),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(if live {
            Color::srgba(1.0, 1.0, 1.0, 0.035)
        } else {
            Color::NONE
        }),
    ))
    .with_children(|h| {
        // The week-view slot: day over time, like a calendar's left rail.
        h.spawn(Node {
            flex_direction: FlexDirection::Column,
            width: px(46),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|s| {
            s.spawn((label(r.slot.0, 12.0), TextColor(ACCENT_WARM)));
            s.spawn((label(r.slot.1, 11.0), TextColor(DIM)));
        });
        h.spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: px(0),
            min_width: px(0),
            ..default()
        })
        .with_children(|t| {
            let name = if live {
                Color::WHITE
            } else {
                Color::srgb(0.72, 0.75, 0.8)
            };
            t.spawn((label(r.name.clone(), 15.0), TextColor(name)));
            t.spawn((label(r.blurb.clone(), 12.0), TextColor(DIM)));
        });
        h.spawn(Node {
            flex_direction: FlexDirection::Column,
            width: px(150),
            flex_shrink: 0.0,
            row_gap: px(3),
            ..default()
        })
        .with_children(|m| {
            m.spawn((
                label(r.status.0.clone(), 11.0),
                TextColor(r.status.1),
                TextLayout::default().with_no_wrap(),
            ));
            m.spawn(Node {
                column_gap: px(6),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|q| {
                q.spawn((
                    label(r.length.clone(), 11.0),
                    TextColor(DIM),
                    TextLayout::default().with_no_wrap(),
                    Node {
                        width: px(44),
                        ..default()
                    },
                ));
                for rw in &r.rewards {
                    q.spawn((
                        ImageNode::new(icons.get(assets, reward_icon(*rw).0)),
                        Node {
                            width: px(16),
                            height: px(16),
                            ..default()
                        },
                    ));
                }
            });
        });
        match &r.act {
            Act::Button(b, text) => {
                h.spawn(button(b.clone(), text, true));
            }
            Act::Note(text) => {
                h.spawn((
                    Node {
                        width: px(112),
                        height: px(34),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(5)),
                        ..default()
                    },
                    BorderColor::all(PANEL_EDGE.with_alpha(0.5)),
                    children![(label(text.clone(), 12.0), TextColor(DIM))],
                ));
            }
        }
    });
}

fn button(kind: CalButton, text: &str, primary: bool) -> impl Bundle {
    let (w, h) = if primary { (112.0, 34.0) } else { (96.0, 28.0) };
    (
        kind,
        Button,
        Node {
            width: px(w),
            height: px(h),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(if primary { 2.0 } else { 1.0 })),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BorderColor::all(if primary { ACCENT } else { KEY_RIM }),
        BackgroundColor(if primary {
            ACCENT.with_alpha(0.22)
        } else {
            Color::NONE
        }),
        children![(
            label(text.to_string(), if primary { 14.0 } else { 12.0 }),
            TextColor(if primary { Color::WHITE } else { DIM })
        )],
    )
}

/// The board's "!": anything listed today that was never played or started.
fn news(
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    live: Res<crate::live::LiveState>,
    mut st: ResMut<CalendarState>,
) {
    let content = sim.content();
    let me = sim.player();
    let online = net.is_some();
    let day = crate::live::unix_now(&*sim, &offline, online, &live)
        .map_or(0, |u| combat_core::live::day_index(u as u64));
    if st.day != day {
        st.day = day;
    }
    let (games, chains) = content.calendar.listing(day);
    let unplayed_game = games.iter().any(|g| {
        content.minigame(&g.key).is_some()
            && !st.best.contains_key(&g.key)
            && match (online, me) {
                (false, Some(p)) => offline.0.minigame_best(p, &g.key).is_none(),
                _ => true,
            }
    });
    let new_chain = chains
        .iter()
        .any(|c| c.status(sim.quests()) == ChainStatus::New);
    let news = unplayed_game || new_chain;
    if st.news != news {
        st.news = news;
    }
}
