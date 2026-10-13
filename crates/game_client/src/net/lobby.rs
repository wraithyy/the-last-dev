//! "Co-op (beta)" entry in the Esc menu and the `~/coop` panel: address, display name, room code.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use super::address::{self, Field};
use super::session::LeaveRequest;
use super::store::{self, Saved};
use super::transport::{NetClient, NetState, WAKING};
use super::world::NetWorld;
use crate::hud::{
    ACCENT, ACCENT_WARM, DIM, HP_GREEN, KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, drop_shadow,
    label, panel_bg, set_text, vgrad,
};
use crate::input::{Paused, TextFocus};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

const DEFAULT_NAME: &str = "Anonymous Dev";
const PLACEHOLDER: Color = Color::srgb(0.38, 0.42, 0.5);
const FAIL_RED: Color = Color::srgb(0.95, 0.35, 0.3);
/// Characters a field box shows (JetBrains Mono at 16 px is about 9.6 px per glyph); the
/// address box is narrower when the "This server" button sits next to it.
const FIELD_CHARS: usize = 34;
const FIELD_CHARS_BESIDE_BUTTON: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intent {
    Create,
    Join,
}

/// Lobby form state. Wave 2 reads `intent`, `name` and `room` once `NetClient` reports Connected.
#[derive(Resource, Default)]
pub struct Lobby {
    pub open: bool,
    pub focus: Option<Field>,
    pub address: String,
    pub name: String,
    pub room: String,
    /// What the player asked for with the last connect attempt.
    pub intent: Option<Intent>,
    /// Local validation message; connection problems come from `NetState`.
    notice: Option<String>,
    loaded: bool,
}

impl Lobby {
    pub fn text(&self, f: Field) -> &str {
        match f {
            Field::Address => &self.address,
            Field::Name => &self.name,
            Field::Room => &self.room,
        }
    }

    fn text_mut(&mut self, f: Field) -> &mut String {
        match f {
            Field::Address => &mut self.address,
            Field::Name => &mut self.name,
            Field::Room => &mut self.room,
        }
    }

    /// A message for the status line (server errors, connection lost).
    pub fn set_notice(&mut self, text: &str) {
        self.notice = Some(text.to_string());
    }

    pub fn display_name(&self) -> &str {
        let n = self.name.trim();
        if n.is_empty() { DEFAULT_NAME } else { n }
    }

    /// The websocket URL to dial, or the message to show instead.
    fn validate(&self, intent: Intent, page_is_https: bool) -> Result<String, String> {
        let url = address::normalize(&self.address, page_is_https)
            .map_err(|e| e.message().to_string())?;
        if intent == Intent::Join && self.room.is_empty() {
            return Err("Enter the room code your colleague shared.".into());
        }
        Ok(url)
    }

    fn fill_from_saved(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let saved = store::load();
        let page = store::page();
        let this_server = page
            .as_ref()
            .and_then(|p| address::this_server_url(&p.protocol, &p.host));
        if self.address.is_empty() {
            self.address = initial_address(this_server, saved.address, DEFAULT_SERVER);
        }
        if self.name.is_empty() {
            self.name = saved.name;
        }
    }
}

/// Build-time default server (`TLD_DEFAULT_SERVER`, e.g. a GitHub repo variable); CI passes
/// an empty string when the variable is unset.
const DEFAULT_SERVER: Option<&str> = option_env!("TLD_DEFAULT_SERVER");

/// The server this page came from wins (LAN play), then the player's last address, then the
/// build's default.
fn initial_address(this_server: Option<String>, saved: String, default: Option<&str>) -> String {
    let default = default.map(str::trim).filter(|d| !d.is_empty());
    match (this_server, default) {
        (Some(s), _) => s,
        (None, Some(d)) if saved.trim().is_empty() => d.to_string(),
        (None, _) => saved,
    }
}

/// Text and colour for the status line.
fn status_line(state: &NetState, notice: Option<&str>, in_room: bool) -> (String, Color) {
    match state {
        NetState::Connecting { .. } => ("Connecting...".into(), ACCENT_WARM),
        NetState::Waking { .. } => (WAKING.into(), ACCENT_WARM),
        NetState::Connected if in_room => (
            "In the room. Share the code with your colleagues.".into(),
            HP_GREEN,
        ),
        NetState::Connected => ("Connected. Shaking hands...".into(), ACCENT_WARM),
        NetState::Failed { reason } => (notice.unwrap_or(reason).to_string(), FAIL_RED),
        NetState::Disconnected => match notice {
            Some(n) => (n.to_string(), FAIL_RED),
            None => (
                "Offline. The solo game keeps running underneath.".into(),
                DIM,
            ),
        },
    }
}

fn next_field(f: Field) -> Field {
    match f {
        Field::Address => Field::Name,
        Field::Name => Field::Room,
        Field::Room => Field::Address,
    }
}

/// The end of `text` that fits in `max` characters, so the caret stays in view: long URLs
/// scroll left instead of wrapping out of the box.
fn tail(text: &str, max: usize) -> String {
    let n = text.chars().count();
    if n <= max {
        return text.to_string();
    }
    let keep: String = text.chars().skip(n + 1 - max).collect();
    format!("<{keep}")
}

/// Applies one editing key to a field's text.
fn edit(field: Field, text: &mut String, key: &Key, typed: Option<&str>) {
    match key {
        Key::Backspace => {
            text.pop();
        }
        Key::Character(_) | Key::Space => {
            if let Some(t) = typed {
                field.push_str(text, t);
            }
        }
        _ => {}
    }
}

/// Native: winit is the only text source. Web: the hidden input is the single source of truth, so
/// keys that reached winit (canvas had DOM focus) are applied to the input and focus moves back to
/// it; keys the input already received are ignored here to avoid applying them twice.
#[cfg(not(target_arch = "wasm32"))]
fn edit_field(lobby: &mut Lobby, field: Field, key: &Key, typed: Option<&str>) {
    edit(field, lobby.text_mut(field), key, typed);
}

#[cfg(target_arch = "wasm32")]
fn edit_field(lobby: &mut Lobby, field: Field, key: &Key, typed: Option<&str>) {
    use super::soft_keyboard as kb;
    if kb::has_focus() {
        return;
    }
    let mut text = kb::value().unwrap_or_else(|| lobby.text(field).to_string());
    edit(field, &mut text, key, typed);
    kb::show(&text);
}

#[derive(Component)]
struct CoopMenu;
#[derive(Component)]
struct LobbyRoot;
#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct FieldBox(Field);
#[derive(Component)]
struct FieldText(Field);
#[derive(Component)]
struct RoomCodeText;
#[derive(Component)]
struct PlayersText;
#[derive(Component)]
struct ThisServerBtn;
/// Shown only while in a room (true) or only outside one (false).
#[derive(Component)]
struct ShowWhenOnline(bool);
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Btn {
    Open,
    Create,
    Join,
    Back,
    ThisServer,
    Leave,
    Copy,
}

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        let mut lobby = Lobby::default();
        if std::env::var_os("WOW_COOP").is_some() {
            lobby.open = true;
            app.insert_resource(Paused(true));
        }
        app.insert_resource(lobby)
            .init_resource::<TextFocus>()
            .add_systems(Startup, spawn_ui)
            .add_systems(Update, (lobby_logic, lobby_room, lobby_visuals).chain());
    }
}

fn button(text: &str) -> impl Bundle {
    (
        Button,
        Node {
            padding: UiRect::axes(px(14), px(5)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        vgrad(KEY_TOP_HI, KEY_TOP),
        children![label(text, 16.0)],
    )
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        CoopMenu,
        Node {
            position_type: PositionType::Absolute,
            top: percent(74),
            width: percent(100),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        GlobalZIndex(51),
        children![(Btn::Open, button("Co-op (beta)"))],
    ));

    let this_server =
        store::page().is_some_and(|p| address::this_server_url(&p.protocol, &p.host).is_some());
    commands
        .spawn((
            LobbyRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(60),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(560),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(10),
                    padding: UiRect::all(px(18)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                // Opaque under the gradient so the pause text and volume rows do not show through.
                BackgroundColor(Color::srgb(0.05, 0.055, 0.08)),
                panel_bg(),
                drop_shadow(),
            ))
            .with_children(|p| {
                p.spawn((label("~/coop", 24.0), TextColor(ACCENT)));
                p.spawn((
                    label("// optional. the solo game keeps running underneath.", 13.0),
                    TextColor(DIM),
                ));
                for (field, name) in [
                    (Field::Address, "Server address"),
                    (Field::Name, "Display name"),
                    (Field::Room, "Room code"),
                ] {
                    p.spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: px(10),
                        ..default()
                    })
                    .with_children(|r| {
                        r.spawn((
                            label(name, 16.0),
                            TextColor(DIM),
                            Node {
                                width: px(150),
                                ..default()
                            },
                        ));
                        r.spawn((
                            Button,
                            FieldBox(field),
                            Node {
                                flex_grow: 1.0,
                                height: px(32),
                                padding: UiRect::axes(px(8), px(4)),
                                border: UiRect::all(px(2)),
                                border_radius: BorderRadius::all(px(4)),
                                align_items: AlignItems::Center,
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BorderColor::all(PANEL_EDGE),
                            BackgroundColor(Color::srgb(0.03, 0.035, 0.055)),
                            children![(FieldText(field), label("", 16.0), TextLayout::no_wrap())],
                        ));
                        if field == Field::Address && this_server {
                            r.spawn((Btn::ThisServer, ThisServerBtn, button("This server")));
                        }
                    });
                }
                p.spawn((
                    StatusText,
                    label("", 15.0),
                    Node {
                        min_height: px(40),
                        ..default()
                    },
                ));
                p.spawn((
                    RoomCodeText,
                    ShowWhenOnline(true),
                    label("", 30.0),
                    TextColor(ACCENT_WARM),
                ));
                p.spawn((
                    PlayersText,
                    ShowWhenOnline(true),
                    label("", 14.0),
                    TextColor(DIM),
                ));
                p.spawn(Node {
                    column_gap: px(10),
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                })
                .with_children(|r| {
                    r.spawn((Btn::Create, ShowWhenOnline(false), button("Create room")));
                    r.spawn((Btn::Join, ShowWhenOnline(false), button("Join room")));
                    if cfg!(target_arch = "wasm32") {
                        r.spawn((Btn::Copy, ShowWhenOnline(true), button("Copy code")));
                    }
                    r.spawn((Btn::Leave, ShowWhenOnline(true), button("Leave room")));
                    r.spawn((Btn::Back, button("Back")));
                });
            });
        });
}

fn lobby_logic(
    time: Res<Time>,
    paused: Res<Paused>,
    mut lobby: ResMut<Lobby>,
    mut net: ResMut<NetClient>,
    mut text_focus: ResMut<TextFocus>,
    mut keys: MessageReader<KeyboardInput>,
    buttons: Query<(&Interaction, &Btn), Changed<Interaction>>,
    fields: Query<(&Interaction, &FieldBox), Changed<Interaction>>,
    mut coop_menu: Single<&mut Node, (With<CoopMenu>, Without<LobbyRoot>)>,
    mut root: Single<&mut Node, (With<LobbyRoot>, Without<CoopMenu>)>,
    mut field_texts: Query<(&FieldText, &mut Text, &mut TextColor), Without<StatusText>>,
    mut status: Single<(&mut Text, &mut TextColor), With<StatusText>>,
    mut shown_focus: Local<Option<Field>>,
    mut kb_seen: Local<String>,
    world: Option<Res<NetWorld>>,
    this_server_btn: Query<(), With<ThisServerBtn>>,
) {
    // Esc unpauses (see targeting.rs), which also closes the lobby.
    if !paused.0 {
        lobby.open = false;
    }
    let pressed = |b: Btn| {
        buttons
            .iter()
            .any(|(i, k)| *i == Interaction::Pressed && *k == b)
    };
    if paused.0 && !lobby.open && pressed(Btn::Open) {
        lobby.open = true;
    }
    coop_menu.display = if paused.0 && !lobby.open {
        Display::Flex
    } else {
        Display::None
    };
    root.display = if lobby.open {
        Display::Flex
    } else {
        Display::None
    };
    if lobby.open {
        lobby.fill_from_saved();
    } else {
        lobby.focus = None;
    }

    if lobby.open {
        for (i, FieldBox(f)) in &fields {
            if *i == Interaction::Pressed {
                lobby.focus = Some(*f);
                // The click moved DOM focus to the canvas; re-show pulls it back to the hidden input.
                *shown_focus = None;
            }
        }
        // Enter/Tab come from winit; text edits go through edit_field (see there for the web).
        for e in keys.read() {
            let Some(field) = lobby.focus else { continue };
            if e.state != ButtonState::Pressed {
                continue;
            }
            match &e.logical_key {
                Key::Enter => lobby.focus = None,
                Key::Tab => lobby.focus = Some(next_field(field)),
                key => edit_field(&mut lobby, field, key, e.text.as_deref()),
            }
        }
        if pressed(Btn::ThisServer)
            && let Some(p) = store::page()
            && let Some(url) = address::this_server_url(&p.protocol, &p.host)
        {
            lobby.address = url;
        }
        if pressed(Btn::Back) {
            if matches!(
                net.state(),
                NetState::Connecting { .. } | NetState::Waking { .. }
            ) {
                net.disconnect();
            }
            lobby.open = false;
            lobby.focus = None;
        }
        for (btn, intent) in [(Btn::Create, Intent::Create), (Btn::Join, Intent::Join)] {
            if !pressed(btn) || net.state().busy() {
                continue;
            }
            let https = store::page().is_some_and(|p| p.https());
            match lobby.validate(intent, https) {
                Ok(url) => {
                    lobby.notice = None;
                    lobby.intent = Some(intent);
                    store::save(&Saved {
                        address: lobby.address.clone(),
                        name: lobby.name.clone(),
                    });
                    net.connect(&url);
                }
                Err(msg) => lobby.notice = Some(msg),
            }
        }
    }

    sync_soft_keyboard(&mut lobby, &mut shown_focus, &mut kb_seen);
    text_focus.0 = lobby.open && lobby.focus.is_some();

    let caret = ((time.elapsed_secs() * 2.0) as u32).is_multiple_of(2);
    for (FieldText(f), mut text, mut color) in &mut field_texts {
        let value = lobby.text(*f);
        let focused = lobby.focus == Some(*f);
        let max = if *f == Field::Address && !this_server_btn.is_empty() {
            FIELD_CHARS_BESIDE_BUTTON
        } else {
            FIELD_CHARS
        };
        let (shown, tint) = match (value.is_empty(), focused) {
            (true, false) => (placeholder(*f).to_string(), PLACEHOLDER),
            (_, true) => (
                format!("{}{}", tail(value, max - 1), if caret { "_" } else { " " }),
                ACCENT,
            ),
            _ => (tail(value, max), Color::WHITE),
        };
        set_text(&mut text, &shown);
        color.0 = tint;
    }
    let (line, tint) = status_line(net.state(), lobby.notice.as_deref(), world.is_some());
    set_text(&mut status.0, &line);
    status.1.0 = tint;
}

/// Room code, member list and the in-room buttons.
fn lobby_room(
    world: Option<Res<NetWorld>>,
    buttons: Query<(&Interaction, &Btn), Changed<Interaction>>,
    mut leave: MessageWriter<LeaveRequest>,
    mut code: Single<&mut Text, With<RoomCodeText>>,
    mut members: Single<&mut Text, (With<PlayersText>, Without<RoomCodeText>)>,
    mut shown: Query<(&ShowWhenOnline, &mut Node)>,
) {
    let online = world.is_some();
    for (when, mut node) in &mut shown {
        let want = if when.0 == online {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
    let Some(world) = world else { return };
    set_text(&mut code, &world.room);
    set_text(&mut members, &member_line(&world));
    for (i, b) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        match b {
            Btn::Leave => {
                leave.write(LeaveRequest);
            }
            Btn::Copy => copy_to_clipboard(&world.room),
            _ => {}
        }
    }
}

fn member_line(world: &NetWorld) -> String {
    use crate::combat::SimRead;
    let names: Vec<String> = world
        .players()
        .iter()
        .map(|id| {
            let name = world.name_of(*id).unwrap_or("Intern");
            if *id == world.you {
                format!("{name} (you)")
            } else {
                name.to_string()
            }
        })
        .collect();
    format!("In the room: {}", names.join(", "))
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_to_clipboard(_: &str) {}

/// `navigator.clipboard.writeText`, looked up dynamically so no extra web-sys features are
/// needed; a missing clipboard (insecure context) just does nothing.
#[cfg(target_arch = "wasm32")]
fn copy_to_clipboard(text: &str) {
    use wasm_bindgen::{JsCast, JsValue};
    let get = |o: &JsValue, k: &str| js_sys::Reflect::get(o, &JsValue::from_str(k)).ok();
    let Some(clipboard) =
        get(&js_sys::global().into(), "navigator").and_then(|n| get(&n, "clipboard"))
    else {
        return;
    };
    if let Some(f) =
        get(&clipboard, "writeText").and_then(|f| f.dyn_into::<js_sys::Function>().ok())
    {
        let _ = f.call1(&clipboard, &JsValue::from_str(text));
    }
}

fn placeholder(f: Field) -> &'static str {
    match f {
        Field::Address => "192.168.1.20:7777",
        Field::Name => DEFAULT_NAME,
        Field::Room => "MERGE-CONFLICT-7K3M9QZA",
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sync_soft_keyboard(_: &mut Lobby, _: &mut Option<Field>, _: &mut String) {}

/// Keeps the hidden HTML input and the focused field in step: focus changes move DOM focus (which
/// opens the soft keyboard on phones), and typing or pasting flows back through the field filter.
#[cfg(target_arch = "wasm32")]
fn sync_soft_keyboard(lobby: &mut Lobby, shown: &mut Option<Field>, seen: &mut String) {
    use super::soft_keyboard as kb;
    if lobby.focus != *shown {
        *shown = lobby.focus;
        match lobby.focus {
            Some(f) => kb::show(lobby.text(f)),
            None => kb::hide(),
        }
        seen.clear();
        seen.push_str(lobby.focus.map_or("", |f| lobby.text(f)));
    }
    let (Some(f), Some(raw)) = (lobby.focus, kb::value()) else {
        return;
    };
    // Typing reaches either the hidden input or, while the canvas has DOM focus, winit. Adopt the
    // input only when it changed, else push the field into it so winit edits are not overwritten.
    if raw != *seen {
        let clean = f.sanitize(&raw);
        if clean != raw {
            kb::set_value(&clean);
        }
        *lobby.text_mut(f) = clean;
    } else if raw != lobby.text(f) {
        kb::set_value(lobby.text(f));
    }
    seen.clear();
    seen.push_str(lobby.text(f));
}

fn lobby_visuals(
    lobby: Res<Lobby>,
    mut fields: Query<(&Interaction, &FieldBox, &mut BorderColor), Without<Btn>>,
    mut buttons: Query<(&Interaction, &mut BorderColor), (With<Btn>, Without<FieldBox>)>,
) {
    for (i, FieldBox(f), mut border) in &mut fields {
        let on = lobby.focus == Some(*f) || *i == Interaction::Hovered;
        *border = BorderColor::all(if on { ACCENT } else { PANEL_EDGE });
    }
    for (i, mut border) in &mut buttons {
        *border = BorderColor::all(if *i == Interaction::None {
            KEY_RIM
        } else {
            ACCENT
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::platform::time::Instant;

    fn lobby(address: &str, room: &str) -> Lobby {
        Lobby {
            address: address.into(),
            room: room.into(),
            ..default()
        }
    }

    #[test]
    fn join_needs_room_but_create_does_not() {
        let l = lobby("10.0.0.2", "");
        assert_eq!(
            l.validate(Intent::Create, false).as_deref(),
            Ok("ws://10.0.0.2:7777/ws")
        );
        assert!(l.validate(Intent::Join, false).is_err());
        assert!(
            lobby("10.0.0.2", "AB12")
                .validate(Intent::Join, false)
                .is_ok()
        );
    }

    #[test]
    fn bad_address_and_mixed_content_are_reported() {
        assert!(lobby("", "A").validate(Intent::Create, false).is_err());
        let msg = lobby("10.0.0.2", "A")
            .validate(Intent::Create, true)
            .unwrap_err();
        assert!(msg.contains("Browsers block"), "{msg}");
    }

    #[test]
    fn display_name_falls_back() {
        assert_eq!(Lobby::default().display_name(), DEFAULT_NAME);
        assert_eq!(
            Lobby {
                name: " Ann ".into(),
                ..default()
            }
            .display_name(),
            "Ann"
        );
    }

    #[test]
    fn status_line_follows_state() {
        let connecting = NetState::Connecting {
            since: Instant::now(),
        };
        assert_eq!(status_line(&connecting, None, false).0, "Connecting...");
        let waking = NetState::Waking {
            retry_at: Instant::now(),
        };
        assert_eq!(status_line(&waking, None, false).0, WAKING);
        assert!(
            status_line(&NetState::Connected, None, false)
                .0
                .starts_with("Connected")
        );
        let failed = NetState::Failed {
            reason: "Server unreachable".into(),
        };
        assert_eq!(status_line(&failed, None, false).0, "Server unreachable");
        assert_eq!(
            status_line(&NetState::Disconnected, Some("oops"), false).0,
            "oops"
        );
        assert!(
            status_line(&NetState::Disconnected, None, false)
                .0
                .starts_with("Offline")
        );
    }

    #[test]
    fn editing_filters_and_backspaces() {
        let mut s = String::new();
        edit(Field::Room, &mut s, &Key::Character("a".into()), Some("a"));
        edit(Field::Room, &mut s, &Key::Character("!".into()), Some("!"));
        edit(Field::Room, &mut s, &Key::Space, Some(" "));
        edit(Field::Room, &mut s, &Key::Character("7".into()), Some("7"));
        assert_eq!(s, "A-7");
        edit(Field::Room, &mut s, &Key::Backspace, None);
        assert_eq!(s, "A-");
        edit(Field::Room, &mut s, &Key::Backspace, None);
        edit(Field::Room, &mut s, &Key::Backspace, None);
        assert_eq!(s, "");
    }

    #[test]
    fn long_values_show_their_tail_on_one_line() {
        assert_eq!(tail("abc", 5), "abc");
        assert_eq!(tail("abcde", 5), "abcde");
        let url = "wss://the-last-dev-server.onrender.com/ws";
        let shown = tail(url, 20);
        assert_eq!(shown.chars().count(), 20);
        assert!(
            shown.starts_with('<') && shown.ends_with("onrender.com/ws"),
            "{shown}"
        );
    }

    #[test]
    fn address_prefill_order() {
        let d = Some("wss://x.onrender.com");
        assert_eq!(
            initial_address(Some("ws://h/ws".into()), "a".into(), d),
            "ws://h/ws"
        );
        assert_eq!(initial_address(None, "10.0.0.2".into(), d), "10.0.0.2");
        assert_eq!(
            initial_address(None, String::new(), d),
            "wss://x.onrender.com"
        );
        assert_eq!(initial_address(None, String::new(), Some(" ")), "");
        assert_eq!(initial_address(None, String::new(), None), "");
    }

    #[test]
    fn tab_cycles_fields() {
        assert_eq!(next_field(Field::Address), Field::Name);
        assert_eq!(next_field(Field::Room), Field::Address);
    }
}
