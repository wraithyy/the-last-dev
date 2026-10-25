//! Esc menu > Start over: wipes the current class's offline save and restarts the run from the
//! car park with the tutorial. The player has to type a passive-aggressive HR phrase to confirm,
//! so a stray click never costs a character. Offline only; online runs never touch the save.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::classes::PlayerClass;
use crate::economy::save::WipeSave;
use crate::game::RestartRun;
use crate::hud::Banner;
use crate::hud::{
    ACCENT, ACCENT_WARM, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, label, vgrad,
};
use crate::input::{Paused, TextFocus};
use crate::net::GameMode;
use bevy::input::ButtonState;
use bevy::input::InputSystems;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

const PHRASES: [&str; 8] = [
    "I understand my contributions were valued",
    "per my last save please delete me",
    "I am excited about this new chapter",
    "no one will notice I am gone",
    "I have backed nothing up and I am at peace",
    "I consent to being restructured",
    "please circle back to level one",
    "this aligns with my personal growth goals",
];
const MAX_CHARS: usize = 64;

#[derive(Resource, Default)]
struct Wipe {
    open: bool,
    phrase: usize,
    typed: String,
}

/// Case, extra spaces and a trailing full stop do not count; the words do.
fn normalise(s: &str) -> String {
    s.trim()
        .trim_end_matches('.')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn matches(typed: &str, phrase: &str) -> bool {
    normalise(typed) == normalise(phrase)
}

#[derive(Component)]
pub struct StartOverButton;
#[derive(Component)]
struct DialogRoot;
#[derive(Component)]
struct PhraseText;
#[derive(Component)]
struct TypedText;
#[derive(Component)]
struct WipeButton;
#[derive(Component)]
struct CancelButton;

pub struct WipePlugin;

impl Plugin for WipePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wipe>()
            .add_systems(Startup, spawn_dialog)
            .add_systems(PreUpdate, esc_closes.after(InputSystems))
            .add_systems(Update, (open, typing, confirm, visuals).chain());
    }
}

/// The Esc menu button, spawned next to "Change role".
pub fn start_over_button() -> impl Bundle {
    (
        Button,
        StartOverButton,
        Node {
            margin: UiRect::top(px(4)),
            padding: UiRect::axes(px(10), px(3)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        vgrad(KEY_TOP_HI, KEY_TOP),
        children![(label("Start over (wipe this role)", 12.0), TextColor(DIM))],
    )
}

fn key_button(marker: impl Component, text: &str) -> impl Bundle {
    (
        Button,
        marker,
        Node {
            padding: UiRect::axes(px(16), px(6)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        vgrad(KEY_TOP_HI, KEY_TOP),
        children![label(text, 16.0)],
    )
}

fn spawn_dialog(mut commands: Commands) {
    commands.spawn((
        DialogRoot,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        // The Esc menu buttons underneath must not take clicks through the dialog.
        FocusPolicy::Block,
        GlobalZIndex(60),
        children![(
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(10),
                padding: UiRect::all(px(20)),
                max_width: px(560),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            BackgroundColor(Color::srgb(0.05, 0.06, 0.09)),
            children![
                (label("~/hr $ offboard --self", 18.0), TextColor(ACCENT_WARM)),
                (
                    label(
                        "This wipes this role's level, gear, Kudos and quests and starts again in the car park. Other roles keep theirs. HR needs it in writing:",
                        13.0
                    ),
                    TextColor(DIM),
                    TextLayout::justify(Justify::Center),
                ),
                (PhraseText, label("", 16.0), TextColor(ACCENT)),
                (
                    Node {
                        min_width: px(420),
                        padding: UiRect::axes(px(8), px(5)),
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BorderColor::all(KEY_RIM),
                    BackgroundColor(Color::srgb(0.02, 0.02, 0.04)),
                    children![(TypedText, label("", 15.0))],
                ),
                (
                    Node {
                        column_gap: px(12),
                        ..default()
                    },
                    children![
                        key_button(CancelButton, "Cancel"),
                        key_button(WipeButton, "Wipe"),
                    ],
                ),
            ],
        )],
    ));
}

fn esc_closes(mut keys: ResMut<ButtonInput<KeyCode>>, mut wipe: ResMut<Wipe>) {
    if wipe.open && keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        close(&mut wipe);
    }
}

fn close(wipe: &mut Wipe) {
    wipe.open = false;
    wipe.typed.clear();
    #[cfg(target_arch = "wasm32")]
    crate::net::soft_keyboard::hide();
}

fn open(
    time: Res<Time>,
    paused: Res<Paused>,
    mode: Res<GameMode>,
    mut wipe: ResMut<Wipe>,
    // A Query, not Single: a Single with Changed fails while idle and skips the whole system.
    button: Query<&Interaction, (Changed<Interaction>, With<StartOverButton>)>,
    mut focus: ResMut<TextFocus>,
) {
    if wipe.open && !paused.0 {
        close(&mut wipe);
    }
    if !wipe.open
        && paused.0
        && !mode.is_online()
        && button.iter().any(|i| *i == Interaction::Pressed)
    {
        wipe.open = true;
        wipe.phrase = (time.elapsed().as_millis() as usize) % PHRASES.len();
        wipe.typed.clear();
        #[cfg(target_arch = "wasm32")]
        {
            crate::net::soft_keyboard::show("");
            crate::net::soft_keyboard::take_enter();
        }
    }
    // Only this dialog's own transitions touch the flag, so the lobby keeps owning it otherwise.
    if wipe.is_changed() && focus.0 != wipe.open {
        focus.0 = wipe.open;
    }
}

fn typing(mut wipe: ResMut<Wipe>, mut keys: MessageReader<KeyboardInput>) {
    if !wipe.open {
        keys.clear();
        return;
    }
    // Web: the hidden input (soft keyboard, paste) is the text source while it has focus.
    #[cfg(target_arch = "wasm32")]
    {
        use crate::net::soft_keyboard as kb;
        if kb::has_focus() {
            keys.clear();
            if let Some(v) = kb::value()
                && v != wipe.typed
            {
                wipe.typed = v.chars().take(MAX_CHARS).collect();
            }
            return;
        }
    }
    let mut text = wipe.typed.clone();
    for e in keys.read() {
        if e.state != ButtonState::Pressed {
            continue;
        }
        match &e.logical_key {
            Key::Backspace => {
                text.pop();
            }
            Key::Enter | Key::Escape | Key::Tab => {}
            _ => {
                if let Some(t) = e.text.as_deref() {
                    text.extend(t.chars().filter(|c| !c.is_control()));
                }
            }
        }
    }
    let text: String = text.chars().take(MAX_CHARS).collect();
    if text != wipe.typed {
        #[cfg(target_arch = "wasm32")]
        crate::net::soft_keyboard::show(&text);
        wipe.typed = text;
    }
}

fn confirm(
    keys: Res<ButtonInput<KeyCode>>,
    mut wipe: ResMut<Wipe>,
    mut paused: ResMut<Paused>,
    class: Res<PlayerClass>,
    buttons: Query<
        (&Interaction, Has<WipeButton>),
        (
            Changed<Interaction>,
            Or<(With<WipeButton>, With<CancelButton>)>,
        ),
    >,
    mut wipes: MessageWriter<WipeSave>,
    mut restart: MessageWriter<RestartRun>,
    mut banners: MessageWriter<Banner>,
) {
    if !wipe.open {
        return;
    }
    let mut go = keys.just_pressed(KeyCode::Enter);
    #[cfg(target_arch = "wasm32")]
    {
        go |= crate::net::soft_keyboard::take_enter();
    }
    for (i, is_wipe) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        if is_wipe {
            go = true;
        } else {
            close(&mut wipe);
            return;
        }
    }
    if go && matches(&wipe.typed, PHRASES[wipe.phrase]) {
        wipes.write(WipeSave(class.0.clone()));
        restart.write(RestartRun);
        banners.write(Banner {
            title: "Access revoked".into(),
            subtitle: "Effective immediately. Please collect your new badge in the car park."
                .into(),
            color: ACCENT_WARM,
        });
        close(&mut wipe);
        paused.0 = false;
    }
}

fn visuals(
    wipe: Res<Wipe>,
    mut root: Single<&mut Node, With<DialogRoot>>,
    mut phrase: Single<&mut Text, (With<PhraseText>, Without<TypedText>)>,
    mut typed: Single<(&mut Text, &mut TextColor), (With<TypedText>, Without<PhraseText>)>,
    mut wipe_button: Single<&mut BorderColor, With<WipeButton>>,
) {
    if !wipe.is_changed() {
        return;
    }
    root.display = if wipe.open {
        Display::Flex
    } else {
        Display::None
    };
    let want = format!("\"{}\"", PHRASES[wipe.phrase]);
    if phrase.0 != want {
        phrase.0 = want;
    }
    let ok = matches(&wipe.typed, PHRASES[wipe.phrase]);
    typed.0.0 = format!("{}_", wipe.typed);
    typed.1.0 = if ok { ACCENT } else { Color::WHITE };
    wipe_button.set_all(if ok { ACCENT_WARM } else { KEY_RIM });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phrase_ignores_case_spacing_and_a_full_stop_but_not_words() {
        let p = PHRASES[0];
        assert!(matches("i understand my contributions were valued", p));
        assert!(matches(
            "  I  UNDERSTAND my contributions   were valued. ",
            p
        ));
        assert!(!matches("I understand my contributions were", p));
        assert!(!matches("", p));
        for p in PHRASES {
            assert!(p.len() <= MAX_CHARS, "{p}");
            assert!(p.is_ascii(), "{p}");
        }
    }
}
