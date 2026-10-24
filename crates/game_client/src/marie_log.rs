//! Ill Marie's desk on the occupied campus (docs/ILMARI_1_0.md D.2.4). Using it hands over her
//! autonomy log in the sim (`PropKind::Log`); this opens the four-screen read: the log, her
//! sticky note and the Resistance's annotation. Any use opens it again, so the log can be read
//! twice. Esc or the button closes it; Esc is taken here so it does not also open the pause menu.
//!
//! QA hook: `WOW_MARIE_LOG=1..4` opens the window on that screen at start (screenshots).
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, SimReady, SimView};
use crate::copy::MARIE_LOG_PAGES;
use crate::hud::{ACCENT, DIM, drop_shadow, label, panel_bg, set_text};
use bevy::input::InputSystems;
use bevy::prelude::*;
use combat_core::Event;

const DESK: &str = "il_marie_desk";

#[derive(Resource, Default)]
struct MarieLog {
    /// The screen on show (0-based), `None` while closed.
    page: Option<usize>,
    /// Esc or the close button was pressed this frame.
    close: bool,
    /// The next-screen button was pressed this frame.
    next: bool,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Part {
    Window,
    Title,
    Body,
    Next,
    Close,
}

pub struct MarieLogPlugin;

impl Plugin for MarieLogPlugin {
    fn build(&self, app: &mut App) {
        let start = std::env::var("WOW_MARIE_LOG")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|n| (1..=MARIE_LOG_PAGES.len()).contains(n))
            .map(|n| n - 1);
        app.insert_resource(MarieLog {
            page: start,
            ..default()
        })
        .add_systems(Startup, spawn_window)
        .add_systems(PreUpdate, take_input.after(InputSystems))
        .add_systems(Update, (open_on_use, step, draw).chain().in_set(SimReady));
    }
}

fn spawn_window(mut commands: Commands) {
    commands
        .spawn((
            Part::Window,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(18),
                margin: UiRect::left(px(-330)),
                width: px(660),
                max_width: percent(94),
                padding: UiRect::all(px(16)),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.7)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(58),
        ))
        .with_children(|w| {
            w.spawn((Part::Title, label("", 15.0), TextColor(ACCENT)));
            w.spawn((Part::Body, label("", 15.0)));
            w.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|f| {
                f.spawn((Part::Next, Button, label("[next]", 14.0), TextColor(DIM)));
                f.spawn((
                    Part::Close,
                    Button,
                    label("[close, Esc]", 14.0),
                    TextColor(DIM),
                ));
            });
        });
}

/// Esc and the buttons, read before the pause menu sees the key.
fn take_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut log: ResMut<MarieLog>,
    buttons: Query<(&Part, &Interaction), Changed<Interaction>>,
) {
    log.close = false;
    log.next = false;
    if log.page.is_none() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        log.close = true;
    }
    for (part, i) in &buttons {
        if *i == Interaction::Pressed {
            match part {
                Part::Close => log.close = true,
                Part::Next => log.next = true,
                _ => {}
            }
        }
    }
}

/// The desk was used by the local player: the log opens on its first screen.
fn open_on_use(sim: SimView, mut events: MessageReader<CombatEvent>, mut log: ResMut<MarieLog>) {
    let me = sim.player();
    for CombatEvent(ev) in events.read() {
        if let Event::Interacted { unit, prop, .. } = ev
            && Some(*unit) == me
            && prop == DESK
        {
            log.page = Some(0);
        }
    }
}

fn step(mut log: ResMut<MarieLog>) {
    let Some(page) = log.page else { return };
    if log.close {
        log.page = None;
    } else if log.next {
        log.page = (page + 1 < MARIE_LOG_PAGES.len()).then_some(page + 1);
    }
}

fn draw(log: Res<MarieLog>, mut parts: Query<(&Part, &mut Node, Option<&mut Text>)>) {
    let page = log.page;
    for (part, mut node, text) in &mut parts {
        match part {
            Part::Window => {
                node.display = if page.is_some() {
                    Display::Flex
                } else {
                    Display::None
                };
            }
            Part::Title => {
                if let (Some(p), Some(mut t)) = (page, text) {
                    set_text(&mut t, MARIE_LOG_PAGES[p].0);
                }
            }
            Part::Body => {
                if let (Some(p), Some(mut t)) = (page, text) {
                    set_text(&mut t, MARIE_LOG_PAGES[p].1);
                }
            }
            Part::Next => {
                if let (Some(p), Some(mut t)) = (page, text) {
                    let last = p + 1 == MARIE_LOG_PAGES.len();
                    set_text(&mut t, if last { "[done]" } else { "[next]" });
                }
            }
            Part::Close => {}
        }
    }
}
