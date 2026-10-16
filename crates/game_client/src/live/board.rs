//! The Daily Standup window: today's three dailies and the week's Sprint Goal from the Scrum
//! board (talk to the Scrum Master Bot or use the board on the campus plaza). "Join standup"
//! takes them all; a finished Sprint Goal is claimed here with a gear choice; "Clock out" is
//! the honest way to leave at 18:00.

use super::{LiveState, duration, goal_line, hhmm, task_name, unix_now};
use crate::combat::{CombatSim, PendingCommands, SimView};
use crate::hud::{
    ACCENT, ACCENT_WARM, DIM, GOLD, LogLineMsg, PANEL_EDGE, drop_shadow, label, panel_bg,
};
use crate::input::SprintWindows;
use crate::net::NetWorld;
use crate::player::Player;
use bevy::prelude::*;
use combat_core::live::{game_minute, secs_to_day_reset, secs_to_week_reset};
use combat_core::{Command, TaskState};

/// Walking this far from the Scrum Master Bot closes the window.
const CLOSE_RANGE_M: f32 = 12.0;
const WEEKDAYS: [&str; 7] = [
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
];

#[derive(Component)]
pub(super) struct BoardWindow;
#[derive(Component)]
pub(super) struct BoardBody;
#[derive(Component, Clone, Copy)]
pub(super) enum BoardButton {
    Join,
    Claim(u8),
    ClockOut,
    Close,
}

pub(super) fn spawn_window(mut commands: Commands) {
    let width = 600.0;
    commands
        .spawn((
            BoardWindow,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: px(60),
                width: px(width),
                margin: UiRect::left(px(-width / 2.0)),
                padding: UiRect::all(px(12)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                overflow: Overflow::scroll_y(),
                display: Display::None,
                ..default()
            },
            crate::hud::scroll::ScrollList::fit(12.0),
            BorderColor::all(Color::srgb(0.4, 0.75, 0.45).with_alpha(0.8)),
            BackgroundColor(Color::srgb(0.05, 0.055, 0.08)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(32),
        ))
        .with_child((
            BoardBody,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                ..default()
            },
        ));
}

fn button(kind: BoardButton, text: impl Into<String>, primary: bool) -> impl Bundle {
    let color = if primary { ACCENT } else { DIM };
    (
        kind,
        Button,
        Node {
            padding: UiRect::axes(px(10), px(3)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BorderColor::all(if primary { ACCENT } else { PANEL_EDGE }),
        BackgroundColor(if primary {
            ACCENT.with_alpha(0.18)
        } else {
            Color::NONE
        }),
        children![(label(text, 13.0), TextColor(color))],
    )
}

pub(super) fn board_input(
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    st: Res<LiveState>,
    mut windows: ResMut<SprintWindows>,
    mut pending: ResMut<PendingCommands>,
    mut log: MessageWriter<LogLineMsg>,
    player: Single<&Transform, With<Player>>,
    buttons: Query<(&Interaction, &BoardButton), Changed<Interaction>>,
) {
    if !windows.standup {
        return;
    }
    let npc = sim.content().live.board.as_ref().map(|b| b.npc.clone());
    let far = sim
        .units()
        .filter(|u| Some(&u.key) == npc.as_ref())
        .map(|u| {
            let d = player.translation - Vec3::new(u.pos.x, player.translation.y, u.pos.z);
            d.length()
        })
        .fold(f32::MAX, f32::min);
    // Only close on distance where the bot exists (the QA hook opens it anywhere).
    if far != f32::MAX && far > CLOSE_RANGE_M {
        windows.standup = false;
        return;
    }
    for (i, b) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        match *b {
            BoardButton::Join => pending.0.push(Command::JoinStandup),
            BoardButton::Claim(k) => pending.0.push(Command::ClaimSprintGoal { choice: k }),
            BoardButton::ClockOut => {
                pending.0.push(Command::GoHome);
                let at = unix_now(&*sim, &offline, net.is_some(), &st)
                    .map_or_else(|| "--:--".into(), |u| hhmm(game_minute(u as u64)));
                log.write(LogLineMsg(
                    format!("You clocked out at {at}. Your laptop stays open, like your tab."),
                    DIM,
                ));
                windows.standup = false;
            }
            BoardButton::Close => windows.standup = false,
        }
    }
}

pub(super) fn rebuild_board(
    mut commands: Commands,
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    st: Res<LiveState>,
    windows: Res<SprintWindows>,
    mut window: Single<&mut Node, With<BoardWindow>>,
    body: Single<Entity, With<BoardBody>>,
    mut shown: Local<String>,
) {
    let want = if windows.standup {
        Display::Flex
    } else {
        Display::None
    };
    if window.display != want {
        window.display = want;
    }
    if !windows.standup {
        shown.clear();
        return;
    }
    let Some(board) = &st.board else { return };
    let unix = unix_now(&*sim, &offline, net.is_some(), &st).map(|u| u as u64);
    let resets = unix.map(|u| {
        format!(
            "Dailies reset in {} (00:00 UTC). Sprint Goal resets in {} (Monday).",
            duration(secs_to_day_reset(u)),
            duration(secs_to_week_reset(u))
        )
    });
    let sig = format!("{board:?}|{resets:?}");
    if *shown == sig {
        return;
    }
    *shown = sig;
    let content = sim.content();
    let role = sim
        .player()
        .and_then(|p| sim.unit(p))
        .and_then(|u| content.class_of_unit(&u.key))
        .map(|c| c.role);
    let any_open = board.tasks.iter().any(|t| t.state == TaskState::Open);
    let day = WEEKDAYS[board.day as usize % 7];
    commands.entity(*body).despawn_related::<Children>();
    commands.entity(*body).with_children(|c| {
        c.spawn((
            label(format!("~/standup  Daily Standup, {day}"), 16.0),
            TextColor(ACCENT),
        ));
        c.spawn((
            label(
                "Yesterday: unclear. Today: these. Blockers: you. Join once and every task counts until the reset.",
                12.0,
            ),
            TextColor(DIM),
        ));
        for t in &board.tasks {
            let name = task_name(content, &t.key, t.weekly);
            let (chip, chip_color) = match t.state {
                TaskState::Open => ("OPEN", DIM),
                TaskState::Active => ("IN PROGRESS", ACCENT),
                TaskState::Ready => ("CLAIM", GOLD),
                TaskState::Done => ("DONE", Color::srgb(0.4, 1.0, 0.5)),
            };
            c.spawn(Node {
                margin: UiRect::top(px(6)),
                column_gap: px(8),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|h| {
                let kind = if t.weekly { "WEEKLY" } else { "DAILY" };
                h.spawn((
                    label(kind, 11.0),
                    TextColor(if t.weekly { ACCENT_WARM } else { DIM }),
                ));
                h.spawn((
                    label(name, 14.0),
                    TextColor(if t.weekly { GOLD } else { Color::WHITE }),
                ));
                h.spawn((label(format!("[{chip}]"), 11.0), TextColor(chip_color)));
            });
            if let Some(def) = content.task_def(&t.key, t.weekly) {
                c.spawn((
                    Node {
                        max_width: px(570),
                        ..default()
                    },
                    label(def.text.clone(), 12.0),
                    TextColor(Color::srgb(0.78, 0.8, 0.86)),
                ));
            }
            for g in &t.goals {
                let done = g.count >= g.need;
                let mark = if done { "  \u{2713} " } else { "  - " };
                c.spawn((
                    label(format!("{mark}{}", goal_line(content, g)), 12.0),
                    TextColor(if done {
                        Color::srgb(0.5, 0.53, 0.58)
                    } else {
                        Color::srgb(0.82, 0.86, 0.92)
                    }),
                ));
            }
            let reward = if t.weekly {
                format!("Reward: +{} XP, +{} Kudos, and one piece of gear:", t.xp, t.sp)
            } else {
                format!("Reward: +{} XP, +{} Kudos", t.xp, t.sp)
            };
            c.spawn((label(reward, 12.0), TextColor(GOLD)));
            if t.weekly {
                let gear = content.sprint_goal_gear(t.level, role);
                c.spawn(Node {
                    column_gap: px(6),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|r| {
                    for (k, g) in gear.iter().enumerate() {
                        if t.state == TaskState::Ready {
                            r.spawn(button(BoardButton::Claim(k as u8), format!("Claim: {g}"), true));
                        } else {
                            r.spawn((label(format!("[{g}]"), 11.0), TextColor(DIM)));
                        }
                    }
                });
            }
        }
        if let Some(r) = &resets {
            c.spawn((
                Node {
                    margin: UiRect::top(px(6)),
                    ..default()
                },
                label(r.clone(), 11.0),
                TextColor(DIM),
            ));
        }
        c.spawn(Node {
            margin: UiRect::top(px(4)),
            column_gap: px(8),
            ..default()
        })
        .with_children(|r| {
            if any_open {
                r.spawn(button(BoardButton::Join, "Join standup", true));
            }
            r.spawn(button(BoardButton::ClockOut, "Clock out", false));
            r.spawn(button(BoardButton::Close, "Close (Esc)", false));
        });
    });
}
