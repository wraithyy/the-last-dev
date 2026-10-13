//! Emote menu (G): net_proto's predefined phrases, never free text. Online the server echoes
//! `Emoted` to the room; offline the phrase is said locally. Either way it shows as a yell:
//! a speech bubble over the speaker and a combat-log line.
//!
//! Touch UIs open the menu with `ToggleEmoteMenu` or send a phrase with `EmoteRequest`.
//! While the menu is open, 1-0 pick a phrase and Esc or G close it; those key presses are
//! consumed in `PreUpdate`, so the action bar and the pause menu never see them.

use super::session::send;
use super::transport::NetClient;
use super::world::NetWorld;
use crate::combat::{CAST_KEYS, CombatEvent, SimUnit};
use crate::hud::{ACCENT, KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, label, panel_bg, vgrad};
use crate::input::{Paused, TextFocus};
use crate::player::Player;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::ui::UiSystems;
use combat_core::Event;
use net_proto::{ClientMsg, Emote};

/// Keeps a held button or a double tap from tripping the server's rate limit.
const EMOTE_COOLDOWN_SECS: f32 = 1.0;

#[derive(Message, Clone, Copy)]
pub struct EmoteRequest(pub Emote);

#[derive(Message)]
pub struct ToggleEmoteMenu;

#[derive(Component)]
struct EmoteMenu;

#[derive(Component, Clone, Copy)]
struct EmoteButton(Emote);

pub struct EmotePlugin;

impl Plugin for EmotePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EmoteRequest>()
            .add_message::<ToggleEmoteMenu>()
            .add_systems(Startup, spawn_menu)
            .add_systems(
                PreUpdate,
                menu_input.after(InputSystems).after(UiSystems::Focus),
            )
            .add_systems(Update, send_emotes);
    }
}

fn spawn_menu(mut commands: Commands) {
    commands
        .spawn((
            EmoteMenu,
            Node {
                position_type: PositionType::Absolute,
                right: px(16),
                top: percent(30),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                padding: UiRect::all(px(10)),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(6)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            panel_bg(),
            GlobalZIndex(40),
        ))
        .with_children(|p| {
            p.spawn((label("~/emotes", 15.0), TextColor(ACCENT)));
            for (i, e) in Emote::ALL.into_iter().enumerate() {
                p.spawn((
                    Button,
                    EmoteButton(e),
                    Node {
                        padding: UiRect::axes(px(10), px(3)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BorderColor::all(KEY_RIM),
                    vgrad(KEY_TOP_HI, KEY_TOP),
                    children![label(format!("{} {}", slot_key(i), e.command()), 13.0)],
                ));
            }
        });
}

/// Key label of emote slot `i`: the number row, 0 last.
fn slot_key(i: usize) -> usize {
    (i + 1) % 10
}

/// What one frame's keys do to the menu: whether it is open afterwards and which emote
/// slot was picked. `digit` is the slot of a just-pressed number key.
pub fn menu_keys(
    open: bool,
    toggle: bool,
    esc: bool,
    digit: Option<usize>,
) -> (bool, Option<usize>) {
    if !open {
        return (toggle, None);
    }
    match digit {
        Some(slot) if slot < Emote::ALL.len() => (false, Some(slot)),
        _ if toggle || esc => (false, None),
        _ => (true, None),
    }
}

fn menu_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    paused: Res<Paused>,
    focus: Res<TextFocus>,
    mut toggles: MessageReader<ToggleEmoteMenu>,
    buttons: Query<(&Interaction, &EmoteButton), Changed<Interaction>>,
    mut requests: MessageWriter<EmoteRequest>,
    mut menu: Single<&mut Node, With<EmoteMenu>>,
) {
    let mut open = menu.display == Display::Flex;
    if toggles.read().count() % 2 == 1 {
        open = !open;
    }
    if !paused.0 && !focus.0 {
        let digit = CAST_KEYS.iter().position(|k| keys.any_just_pressed(*k));
        let was_open = open;
        let (now_open, pick) = menu_keys(
            open,
            keys.just_pressed(KeyCode::KeyG),
            keys.just_pressed(KeyCode::Escape),
            digit,
        );
        if was_open {
            // Consumed: the action bar must not cast and Esc must not open the pause menu.
            for k in CAST_KEYS.iter().flatten() {
                keys.clear_just_pressed(*k);
            }
            keys.clear_just_pressed(KeyCode::Escape);
        }
        if let Some(e) = pick.and_then(|i| Emote::ALL.get(i)) {
            requests.write(EmoteRequest(*e));
        }
        open = now_open;
    }
    for (i, b) in &buttons {
        if *i == Interaction::Pressed {
            requests.write(EmoteRequest(b.0));
            open = false;
        }
    }
    if paused.0 {
        open = false;
    }
    let want = if open { Display::Flex } else { Display::None };
    if menu.display != want {
        menu.display = want;
    }
}

fn send_emotes(
    time: Res<Time>,
    mut requests: MessageReader<EmoteRequest>,
    client: Res<NetClient>,
    net: Option<Res<NetWorld>>,
    player: Single<&SimUnit, With<Player>>,
    mut events: MessageWriter<CombatEvent>,
    mut last: Local<Option<f32>>,
) {
    for EmoteRequest(e) in requests.read() {
        let now = time.elapsed_secs();
        if last.is_some_and(|t| now - t < EMOTE_COOLDOWN_SECS) {
            continue;
        }
        *last = Some(now);
        if net.is_some() {
            send(&client, &ClientMsg::Emote(*e));
        } else {
            events.write(CombatEvent(Event::Yell {
                unit: player.0,
                text: e.text().into(),
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_menu_swallows_number_keys_and_closes_on_esc_or_g() {
        // Closed: digits are the action bar's, G opens.
        assert_eq!(menu_keys(false, false, false, Some(0)), (false, None));
        assert_eq!(menu_keys(false, true, false, None), (true, None));
        assert_eq!(menu_keys(false, false, true, None), (false, None));
        // Open: a digit picks that phrase and closes; 0 is the tenth slot.
        assert_eq!(menu_keys(true, false, false, Some(0)), (false, Some(0)));
        assert_eq!(menu_keys(true, false, false, Some(9)), (false, Some(9)));
        assert_eq!(menu_keys(true, true, false, None), (false, None));
        assert_eq!(menu_keys(true, false, true, None), (false, None));
        assert_eq!(menu_keys(true, false, false, None), (true, None));
        assert_eq!(slot_key(0), 1);
        assert_eq!(slot_key(9), 0);
    }
}
