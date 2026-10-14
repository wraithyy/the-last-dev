//! `~/keybindings`: Esc menu entry and the rebind panel. Click an action, press a key.

// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use super::{Action, Group, KeyCapture, Keybinds, Rebind, install, key_label};
use crate::hud::{
    ACCENT, ACCENT_WARM, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, label, panel_bg, vgrad,
};
use crate::input::Paused;
use crate::net::Lobby;
use crate::touch::TouchMode;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

pub struct KeybindsPlugin;

impl Plugin for KeybindsPlugin {
    fn build(&self, app: &mut App) {
        install(app);
        app.init_resource::<PanelState>()
            .add_systems(Startup, demo.after(spawn_ui))
            .add_systems(Startup, spawn_ui)
            // After InputSystems so the captured key can still be hidden from every gameplay system.
            .add_systems(PreUpdate, capture_key.after(InputSystems))
            .add_systems(Update, (panel_logic, panel_visuals).chain());
    }
}

/// QA hook `WOW_KEYBIND_DEMO=1|2|3`: rebinds slot 1 to X and coffee to R in memory only (never
/// saved) so screenshots show the bindings on the keycaps; 1 also opens the panel.
fn demo(mut binds: ResMut<Keybinds>, mut state: ResMut<PanelState>, mut paused: ResMut<Paused>) {
    let Ok(mode) = std::env::var("WOW_KEYBIND_DEMO") else {
        return;
    };
    binds.rebind(Action::Slot(0), KeyCode::KeyX);
    binds.rebind(Action::Coffee, KeyCode::KeyR);
    // 3: paused with the panel closed, to shoot the Esc menu entry.
    paused.0 = mode != "2";
    state.open = mode == "1";
    if state.open {
        state.msg = format!("slot 1 = {}", key_label(KeyCode::KeyX));
    }
}

#[derive(Resource, Default)]
struct PanelState {
    open: bool,
    msg: String,
}

#[derive(Component)]
struct OpenButton;
#[derive(Component)]
struct OpenMenu;
#[derive(Component)]
struct PanelRoot;
#[derive(Component)]
struct Row(Action);
#[derive(Component)]
struct RowKey(Action);
#[derive(Component)]
struct ResetButton;
#[derive(Component)]
struct CloseButton;
#[derive(Component)]
struct MsgText;

const ROW_W: f32 = 212.0;

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        OpenMenu,
        Node {
            position_type: PositionType::Absolute,
            // Same row as "Co-op (beta)", to its right: lower rows collide with the action bar.
            top: percent(74),
            width: percent(100),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        GlobalZIndex(51),
        children![(
            Button,
            OpenButton,
            Node {
                margin: UiRect::left(px(320)),
                padding: UiRect::axes(px(14), px(5)),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BorderColor::all(KEY_RIM),
            vgrad(KEY_TOP_HI, KEY_TOP),
            children![label("~/keybindings", 16.0)],
        )],
    ));
    commands
        .spawn((
            PanelRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(62),
            FocusPolicy::Block,
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(10),
                    padding: UiRect::all(px(16)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                BackgroundColor(Color::srgb(0.05, 0.055, 0.08)),
                panel_bg(),
            ))
            .with_children(|panel| {
                panel.spawn((label("~/keybindings", 18.0), TextColor(ACCENT)));
                panel.spawn((
                    label(
                        "click an action, then press a key. A key in use is swapped. Esc cancels.",
                        12.0,
                    ),
                    TextColor(DIM),
                ));
                panel
                    .spawn(Node {
                        column_gap: px(18),
                        align_items: AlignItems::FlexStart,
                        ..default()
                    })
                    .with_children(|cols| {
                        spawn_columns(cols);
                    });
                panel.spawn((MsgText, label("", 13.0), TextColor(ACCENT_WARM)));
                panel
                    .spawn(Node {
                        column_gap: px(10),
                        ..default()
                    })
                    .with_children(|r| {
                        r.spawn(button(ResetButton, "Reset to defaults"));
                        r.spawn(button(CloseButton, "Close"));
                    });
            });
        });
}

fn button(marker: impl Component, text: &str) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            padding: UiRect::axes(px(12), px(4)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        vgrad(KEY_TOP_HI, KEY_TOP),
        children![label(text, 14.0)],
    )
}

/// Four columns: movement / targeting / combat, windows, action bar, numpad.
fn spawn_columns(cols: &mut ChildSpawnerCommands) {
    let layout: [&[Group]; 4] = [
        &[Group::Movement, Group::Targeting, Group::Combat],
        &[Group::Windows],
        &[Group::Bar],
        &[Group::Numpad],
    ];
    for groups in layout {
        cols.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            ..default()
        })
        .with_children(|col| {
            for g in groups {
                col.spawn((
                    label(format!("// {}", g.title()), 12.0),
                    TextColor(DIM),
                    Node {
                        margin: UiRect::top(px(4)),
                        ..default()
                    },
                ));
                for a in Action::all().into_iter().filter(|a| a.group() == *g) {
                    col.spawn((
                        Button,
                        Row(a),
                        Node {
                            width: px(ROW_W),
                            height: px(22),
                            padding: UiRect::axes(px(6), px(0)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(3)),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(Color::NONE),
                    ))
                    .with_children(|r| {
                        r.spawn(label(a.label(), 12.0));
                        r.spawn((
                            Node {
                                min_width: px(44),
                                padding: UiRect::axes(px(4), px(0)),
                                border: UiRect::all(px(1)),
                                border_radius: BorderRadius::all(px(3)),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BorderColor::all(KEY_RIM),
                            BackgroundColor(KEY_TOP),
                            children![(RowKey(a), label("", 12.0))],
                        ));
                    });
                }
            }
        });
    }
}

/// Runs before gameplay: takes the first pressed key while an action waits for one, and
/// hides it (and Esc) from every other system so a rebind never also fires the old meaning.
fn capture_key(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut capture: ResMut<KeyCapture>,
    mut binds: ResMut<Keybinds>,
    mut state: ResMut<PanelState>,
) {
    let Some(action) = capture.0 else {
        // Esc closes the panel itself instead of unpausing the game underneath.
        if state.open && keys.just_pressed(KeyCode::Escape) {
            keys.clear_just_pressed(KeyCode::Escape);
            state.open = false;
        }
        return;
    };
    let pressed: Vec<KeyCode> = keys.get_just_pressed().copied().collect();
    let Some(&key) = pressed.first() else { return };
    for k in &pressed {
        keys.clear_just_pressed(*k);
    }
    capture.0 = None;
    if key == KeyCode::Escape {
        state.msg = "cancelled".into();
        return;
    }
    state.msg = match binds.rebind(action, key) {
        Rebind::Done => format!("{} = {}", action.label(), key_label(key)),
        Rebind::Swapped(other) => format!(
            "{} = {} (swapped: {} now {})",
            action.label(),
            key_label(key),
            other.label(),
            binds.label(other)
        ),
        Rebind::Rejected(why) => format!("{}: {why}", key_label(key)),
    };
    crate::net::save_keys(&binds.serialize());
}

fn panel_logic(
    paused: Res<Paused>,
    lobby: Res<Lobby>,
    touch: Res<TouchMode>,
    mut state: ResMut<PanelState>,
    mut capture: ResMut<KeyCapture>,
    mut binds: ResMut<Keybinds>,
    open_btn: Query<&Interaction, (Changed<Interaction>, With<OpenButton>)>,
    rows: Query<(&Interaction, &Row), Changed<Interaction>>,
    reset: Query<&Interaction, (Changed<Interaction>, With<ResetButton>)>,
    close: Query<&Interaction, (Changed<Interaction>, With<CloseButton>)>,
    mut menu: Single<&mut Node, (With<OpenMenu>, Without<PanelRoot>)>,
    mut root: Single<&mut Node, (With<PanelRoot>, Without<OpenMenu>)>,
) {
    let click = |i: &Interaction| *i == Interaction::Pressed;
    if state.open && !paused.0 {
        state.open = false;
    }
    if !state.open && paused.0 && !lobby.open && !touch.active && open_btn.iter().any(click) {
        state.open = true;
        state.msg.clear();
    }
    if state.open {
        if let Some((_, row)) = rows.iter().find(|(i, _)| click(i)) {
            capture.0 = Some(row.0);
            state.msg = format!("press a key for {} (Esc cancels)", row.0.label());
        }
        if reset.iter().any(click) {
            binds.reset();
            capture.0 = None;
            crate::net::save_keys(&binds.serialize());
            state.msg = "defaults restored".into();
        }
        if close.iter().any(click) {
            state.open = false;
        }
    }
    if !state.open {
        capture.0 = None;
    }
    let show_menu = if paused.0 && !lobby.open && !state.open && !touch.active {
        Display::Flex
    } else {
        Display::None
    };
    if menu.display != show_menu {
        menu.display = show_menu;
    }
    let show_root = if state.open {
        Display::Flex
    } else {
        Display::None
    };
    if root.display != show_root {
        root.display = show_root;
    }
}

fn panel_visuals(
    state: Res<PanelState>,
    capture: Res<KeyCapture>,
    binds: Res<Keybinds>,
    mut rows: Query<(&Row, &Interaction, &mut BorderColor, &mut BackgroundColor)>,
    mut keys: Query<(&RowKey, &mut Text)>,
    mut msg: Single<&mut Text, (With<MsgText>, Without<RowKey>)>,
) {
    if !state.open {
        return;
    }
    for (row, i, mut border, mut bg) in &mut rows {
        let waiting = capture.0 == Some(row.0);
        let (b, c) = if waiting {
            (ACCENT_WARM, Color::srgba(1.0, 0.55, 0.25, 0.15))
        } else if *i == Interaction::None {
            (Color::NONE, Color::NONE)
        } else {
            (ACCENT, Color::srgba(0.36, 0.82, 0.86, 0.12))
        };
        border.set_all(b);
        if bg.0 != c {
            bg.0 = c;
        }
    }
    for (k, mut text) in &mut keys {
        let s = if capture.0 == Some(k.0) {
            "...".to_owned()
        } else {
            binds.label(k.0)
        };
        crate::hud::set_text(&mut text, &s);
    }
    crate::hud::set_text(&mut msg, &state.msg);
}
