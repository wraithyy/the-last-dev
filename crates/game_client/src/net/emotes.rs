//! Emote menu (G): net_proto's predefined phrases, never free text. Online the server echoes
//! `Emoted` to the room; offline the phrase is said locally. Either way it shows as a yell:
//! a speech bubble over the speaker and a combat-log line.
//!
//! Touch UIs open the menu with `ToggleEmoteMenu` or send a phrase with `EmoteRequest`.

use super::session::send;
use super::transport::NetClient;
use super::world::NetWorld;
use crate::combat::{CombatEvent, SimUnit};
use crate::hud::{ACCENT, KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, label, panel_bg, vgrad};
use crate::input::{Paused, TextFocus};
use crate::player::Player;
use bevy::prelude::*;
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
            .add_systems(Update, (menu_input, send_emotes).chain());
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
            for e in Emote::ALL {
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
                    children![label(e.command(), 13.0)],
                ));
            }
        });
}

fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
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
    if !paused.0 && !focus.0 && keys.just_pressed(KeyCode::KeyG) {
        open = !open;
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
