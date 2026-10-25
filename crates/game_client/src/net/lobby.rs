//! "Co-op (beta)" entry in the Esc menu and the `~/coop` panel: address, display name, room code,
//! the public room browser ("Open standups") and the host's public/private switch.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use super::address::{self, Field};
use super::session::{self, LeaveRequest, Phase, Session, request_frame};
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
use combat_core::{Content, EntityId, Role};
use net_proto::{ClientMsg, RoomInfo};

const DEFAULT_NAME: &str = "Anonymous Dev";
const PLACEHOLDER: Color = Color::srgb(0.38, 0.42, 0.5);
const FAIL_RED: Color = Color::srgb(0.95, 0.35, 0.3);
/// Characters a field box shows (JetBrains Mono at 16 px is about 9.6 px per glyph); the
/// address box is narrower when the "This server" button sits next to it.
const FIELD_CHARS: usize = 31;
const FIELD_CHARS_BESIDE_BUTTON: usize = 17;
/// Fallback glyph advance until the first measurement of a rendered field.
const GLYPH_W: f32 = 9.6;
/// Box border + horizontal padding: where the first glyph starts inside a field box.
const TEXT_INSET: f32 = 10.0;
/// Height of the room list before it scrolls: about eight rows, so the panel keeps its shape.
const ROOM_LIST_H: f32 = 260.0;
/// The server answers one list per 2 s per connection.
const REFRESH_SECS: f64 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intent {
    Create,
    Join,
    /// Fetch the public room list; the connection stays open for the next request.
    Browse,
    /// Join a listed public room by its `RoomInfo::id`.
    JoinPublic(u64),
}

/// Lobby form state. Wave 2 reads `intent`, `name` and `room` once `NetClient` reports Connected.
#[derive(Resource, Default)]
pub struct Lobby {
    pub open: bool,
    pub focus: Option<Field>,
    pub address: String,
    pub name: String,
    pub room: String,
    /// Optional title for a room this player creates.
    pub title: String,
    /// Create the room public (listed) instead of code-only.
    pub public: bool,
    /// The room's (public, title, leader) from the last `RoomSettings`.
    pub settings: Option<(bool, String, EntityId)>,
    /// Public rooms from the last `RoomList`, sorted for display.
    rooms: Vec<RoomInfo>,
    /// Bumped on every new list, so the table is rebuilt only then.
    rooms_rev: u32,
    /// The room browser is shown.
    browsing: bool,
    /// `session::clock()` of the last list request.
    listed_at: Option<f64>,
    /// What the player asked for with the last connect attempt.
    pub intent: Option<Intent>,
    /// Local validation message; connection problems come from `NetState`.
    notice: Option<String>,
    loaded: bool,
    /// Caret position (in chars) inside the focused field's text.
    caret: usize,
    /// First char of the focused field shown in the box; keeps the caret in view.
    scroll: usize,
    /// Set by clicks: DOM focus went to the canvas and must be pulled back to the hidden input.
    refocus: bool,
    /// Measured glyph advance in logical px (0 until a field has been laid out).
    glyph_w: f32,
}

impl Lobby {
    pub fn text(&self, f: Field) -> &str {
        match f {
            Field::Address => &self.address,
            Field::Name => &self.name,
            Field::Room => &self.room,
            Field::Title => &self.title,
        }
    }

    fn text_mut(&mut self, f: Field) -> &mut String {
        match f {
            Field::Address => &mut self.address,
            Field::Name => &mut self.name,
            Field::Room => &mut self.room,
            Field::Title => &mut self.title,
        }
    }

    pub fn set_rooms(&mut self, mut rooms: Vec<RoomInfo>) {
        sort_rooms(&mut rooms);
        self.rooms = rooms;
        self.rooms_rev = self.rooms_rev.wrapping_add(1);
    }

    pub fn open_browser(&mut self) {
        self.browsing = true;
    }

    /// Empties a field and focuses it with the caret at the start.
    fn clear_field(&mut self, f: Field) {
        self.text_mut(f).clear();
        self.caret = 0;
        self.scroll = 0;
        self.focus = Some(f);
        self.refocus = true;
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

/// Joinable rooms first, then the fuller ones (a group forming), then the newest.
fn sort_rooms(rooms: &mut [RoomInfo]) {
    rooms.sort_by(|a, b| {
        let open = |r: &RoomInfo| r.players < r.max;
        open(b)
            .cmp(&open(a))
            .then(b.players.cmp(&a.players))
            .then(a.age_s.cmp(&b.age_s))
    });
}

fn role_tag(role: Role) -> &'static str {
    match role {
        Role::Tank => "TNK",
        Role::Healer => "HEAL",
        Role::Melee => "MEL",
        Role::Ranged => "RNG",
    }
}

/// "<1m", "12m", "3h".
fn ago(secs: u32) -> String {
    match secs {
        0..60 => "<1m".into(),
        60..3600 => format!("{}m", secs / 60),
        _ => format!("{}h", secs / 3600),
    }
}

/// Table cells of one room: title, host and age, players, zone, level, role tags and sprint.
fn row_cells(r: &RoomInfo, content: &Content) -> [String; 6] {
    let zone = content
        .zones
        .iter()
        .find(|z| z.id == r.zone)
        .map_or(r.zone.as_str(), |z| z.name.as_str());
    let mut tags: Vec<&str> = r
        .roles
        .iter()
        .filter_map(|k| content.class(k).map(|c| role_tag(c.role)))
        .collect();
    if r.in_sprint {
        tags.push("SPRINT");
    }
    let players = if r.players >= r.max {
        "FULL".to_string()
    } else {
        format!("{}/{}", r.players, r.max)
    };
    [
        r.title.clone(),
        format!("{} {}", r.host, ago(r.age_s)),
        players,
        zone.to_string(),
        format!("Lv {}", r.avg_level),
        tags.join(" "),
    ]
}

/// Text nodes do not clip their own glyphs, so long cells are cut to the column width
/// (JetBrains Mono at 13 px is about 7.8 px per glyph).
fn fit_cell(cell: &str, width_px: f32) -> String {
    let max = (width_px / 7.8).floor() as usize;
    if cell.chars().count() <= max {
        return cell.to_string();
    }
    let mut out: String = cell.chars().take(max.saturating_sub(1)).collect();
    out.push('~');
    out
}

fn list_footer(shown: usize, total: usize) -> String {
    match total {
        0 => "No open standups. Create a room and make it public.".into(),
        n if n > shown => format!("{shown} of {n} public rooms. Refresh for others."),
        n => format!("{n} public room(s). Click one to join."),
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
        Field::Room => Field::Title,
        Field::Title => Field::Address,
    }
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

fn byte_at(text: &str, idx: usize) -> usize {
    text.char_indices().nth(idx).map_or(text.len(), |(b, _)| b)
}

/// Inserts the accepted characters of `s` at the caret, up to the field limit.
fn insert_at(field: Field, text: &mut String, caret: &mut usize, s: &str) {
    let mut at = (*caret).min(char_len(text));
    for c in s.chars().filter_map(|c| field.filter(c)) {
        if char_len(text) >= field.max_len() {
            break;
        }
        text.insert(byte_at(text, at), c);
        at += 1;
    }
    *caret = at;
}

/// Applies one editing key at the caret.
fn edit(field: Field, text: &mut String, caret: &mut usize, key: &Key, typed: Option<&str>) {
    let len = char_len(text);
    *caret = (*caret).min(len);
    match key {
        Key::Backspace if *caret > 0 => {
            *caret -= 1;
            text.remove(byte_at(text, *caret));
        }
        Key::Delete if *caret < len => {
            text.remove(byte_at(text, *caret));
        }
        Key::ArrowLeft => *caret = caret.saturating_sub(1),
        Key::ArrowRight => *caret = (*caret + 1).min(len),
        Key::Home => *caret = 0,
        Key::End => *caret = len,
        Key::Character(_) | Key::Space => {
            if let Some(t) = typed {
                insert_at(field, text, caret, t);
            }
        }
        _ => {}
    }
}

/// New first visible char so the caret stays inside a `max`-column window. While scrolled, column
/// 0 shows the `<` marker, so the caret may not sit on it.
fn scroll_to(scroll: usize, caret: usize, len: usize, max: usize) -> usize {
    if len <= max {
        return 0;
    }
    let mut s = scroll;
    if caret < s + usize::from(s > 0) {
        s = caret.saturating_sub(1);
    } else if caret > s + max {
        s = caret - max;
    }
    s.min(len - max)
}

/// The visible slice for a window starting at `scroll`, with `<` marking hidden text on the left.
fn window(text: &str, scroll: usize, max: usize) -> String {
    let mut out: String = text.chars().skip(scroll).take(max).collect();
    if scroll > 0 {
        out.replace_range(..byte_at(&out, 1), "<");
    }
    out
}

/// Caret for a click `x` px right of the first glyph: nearest gap, never on the `<` marker.
fn caret_at_click(x: f32, glyph_w: f32, scroll: usize, len: usize, max: usize) -> usize {
    let col = (x / glyph_w).round().max(0.0) as usize;
    let lo = usize::from(scroll > 0);
    (scroll + col.clamp(lo, max)).min(len)
}

/// Native: winit is the only text source. Web: the hidden input is the single source of truth, so
/// keys that reached winit (canvas had DOM focus) are applied to the input and focus moves back to
/// it; keys the input already received are ignored here to avoid applying them twice.
#[cfg(not(target_arch = "wasm32"))]
fn edit_field(lobby: &mut Lobby, field: Field, key: &Key, typed: Option<&str>) {
    let mut caret = lobby.caret;
    edit(field, lobby.text_mut(field), &mut caret, key, typed);
    lobby.caret = caret;
}

#[cfg(target_arch = "wasm32")]
fn edit_field(lobby: &mut Lobby, field: Field, key: &Key, typed: Option<&str>) {
    use super::soft_keyboard as kb;
    if kb::has_focus() {
        return;
    }
    let mut text = kb::value().unwrap_or_else(|| lobby.text(field).to_string());
    let mut caret = lobby.caret;
    edit(field, &mut text, &mut caret, key, typed);
    kb::show(&text);
    kb::set_caret(caret);
    lobby.caret = caret;
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
/// Thin bar drawn at the caret; absolute so the text never shifts.
#[derive(Component)]
struct CaretBar(Field);
/// "x" inside a field's right edge; blocks the field box beneath it from also seeing the click.
#[derive(Component)]
struct ClearBtn(Field);
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
    Browse,
    Refresh,
    /// Public/Private choice for a room to create.
    Public,
    /// Host only, in a room: list it or hide it.
    RoomVis,
}

/// Texts the browser system rewrites.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum BrowseText {
    PublicChoice,
    RoomVis,
    Visibility,
    Footer,
}

/// Nodes the browser system shows or hides.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum BrowsePart {
    Table,
    HostOnly,
}

#[derive(Component)]
struct RoomRows;

/// A clickable row of the room browser.
#[derive(Component)]
struct RoomRow {
    id: u64,
    open: bool,
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
            .add_systems(
                Update,
                (
                    lobby_clicks,
                    lobby_logic,
                    lobby_fields,
                    lobby_room,
                    lobby_browse,
                    lobby_visuals,
                )
                    .chain(),
            );
    }
}

fn button(text: &str) -> impl Bundle {
    (button_node(), children![label(text, 16.0)])
}

/// A button whose label the caller spawns (texts that change).
fn button_node() -> impl Bundle {
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
    )
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        CoopMenu,
        Node {
            position_type: PositionType::Absolute,
            top: percent(crate::hud::PAUSE_ROW1),
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
                    max_height: percent(94),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                crate::hud::scroll::ScrollList::default(),
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
                    (Field::Title, "New room title"),
                ] {
                    let mut row = p.spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: px(10),
                        ..default()
                    });
                    if field == Field::Title {
                        row.insert(ShowWhenOnline(false));
                    }
                    row.with_children(|r| {
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
                            children![
                                (FieldText(field), label("", 16.0), TextLayout::no_wrap()),
                                (
                                    CaretBar(field),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        top: px(5),
                                        width: px(2),
                                        height: px(18),
                                        ..default()
                                    },
                                    BackgroundColor(ACCENT),
                                    Visibility::Hidden,
                                ),
                                (
                                    Button,
                                    ClearBtn(field),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        right: px(4),
                                        top: px(3),
                                        width: px(22),
                                        height: px(20),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(px(1)),
                                        border_radius: BorderRadius::all(px(3)),
                                        display: Display::None,
                                        ..default()
                                    },
                                    BorderColor::all(KEY_RIM),
                                    children![(label("x", 14.0), TextColor(DIM))],
                                ),
                            ],
                        ));
                        if field == Field::Address && this_server {
                            r.spawn((Btn::ThisServer, ThisServerBtn, button("This server")));
                        }
                        if field == Field::Title {
                            r.spawn((Btn::Public, button_node())).with_children(|b| {
                                b.spawn((BrowseText::PublicChoice, label("Private", 16.0)));
                            });
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
                p.spawn((
                    BrowseText::Visibility,
                    ShowWhenOnline(true),
                    label("", 14.0),
                    TextColor(DIM),
                ));
                p.spawn((
                    BrowsePart::Table,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        display: Display::None,
                        ..default()
                    },
                ))
                .with_children(|t| {
                    t.spawn(Node {
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|h| {
                        h.spawn((label("Open standups", 18.0), TextColor(ACCENT)));
                        h.spawn((Btn::Refresh, button("Refresh")));
                    });
                    t.spawn((
                        RoomRows,
                        crate::hud::scroll::ScrollList::default(),
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(3),
                            max_height: px(ROOM_LIST_H),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                    ));
                    t.spawn((BrowseText::Footer, label("", 13.0), TextColor(DIM)));
                });
                p.spawn(Node {
                    column_gap: px(10),
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                })
                .with_children(|r| {
                    r.spawn((Btn::Create, ShowWhenOnline(false), button("Create room")));
                    r.spawn((Btn::Join, ShowWhenOnline(false), button("Join room")));
                    r.spawn((Btn::Browse, ShowWhenOnline(false), button("Browse rooms")));
                    r.spawn((Btn::RoomVis, BrowsePart::HostOnly, button_node()))
                        .with_children(|b| {
                            b.spawn((BrowseText::RoomVis, label("", 16.0)));
                        });
                    if cfg!(target_arch = "wasm32") {
                        r.spawn((Btn::Copy, ShowWhenOnline(true), button("Copy code")));
                    }
                    r.spawn((Btn::Leave, ShowWhenOnline(true), button("Leave room")));
                    r.spawn((Btn::Back, button("Back")));
                });
            });
        });
}

/// Field and clear-button clicks: focus, caret placement, clearing.
fn lobby_clicks(
    mut lobby: ResMut<Lobby>,
    windows: Query<&Window>,
    touches: Res<Touches>,
    fields: Query<
        (&Interaction, &FieldBox, &ComputedNode, &UiGlobalTransform),
        Changed<Interaction>,
    >,
    clears: Query<(&Interaction, &ClearBtn), Changed<Interaction>>,
    this_server_btn: Query<(), With<ThisServerBtn>>,
) {
    if !lobby.open {
        return;
    }
    let pointer = windows
        .iter()
        .find_map(Window::cursor_position)
        .or_else(|| {
            touches
                .iter()
                .next()
                .map(bevy::input::touch::Touch::position)
        });
    for (i, FieldBox(f), node, tf) in &fields {
        if *i != Interaction::Pressed {
            continue;
        }
        let len = char_len(lobby.text(*f));
        let max = max_chars(*f, !this_server_btn.is_empty());
        let glyph_w = if lobby.glyph_w > 0.0 {
            lobby.glyph_w
        } else {
            GLYPH_W
        };
        // Without a pointer position (touch not reported) the caret goes to the end.
        // An unfocused field shows its text from the start.
        let scroll = if lobby.focus == Some(*f) {
            lobby.scroll
        } else {
            0
        };
        lobby.caret = pointer.map_or(len, |p| {
            let left = (tf.translation.x - node.size.x / 2.0) * node.inverse_scale_factor;
            caret_at_click(p.x - left - TEXT_INSET, glyph_w, scroll, len, max)
        });
        lobby.scroll = scroll;
        lobby.focus = Some(*f);
        lobby.refocus = true;
    }
    for (i, ClearBtn(f)) in &clears {
        if *i == Interaction::Pressed {
            lobby.clear_field(*f);
        }
    }
}

fn max_chars(f: Field, this_server_btn: bool) -> usize {
    if f == Field::Title || (f == Field::Address && this_server_btn) {
        FIELD_CHARS_BESIDE_BUTTON
    } else {
        FIELD_CHARS
    }
}

fn lobby_logic(
    paused: Res<Paused>,
    mut lobby: ResMut<Lobby>,
    mut net: ResMut<NetClient>,
    mut text_focus: ResMut<TextFocus>,
    mut keys: MessageReader<KeyboardInput>,
    buttons: Query<(&Interaction, &Btn), Changed<Interaction>>,
    mut coop_menu: Single<&mut Node, (With<CoopMenu>, Without<LobbyRoot>)>,
    mut root: Single<&mut Node, (With<LobbyRoot>, Without<CoopMenu>)>,
    mut status: Single<(&mut Text, &mut TextColor), With<StatusText>>,
    mut shown_focus: Local<Option<Field>>,
    mut kb_seen: Local<String>,
    world: Option<Res<NetWorld>>,
    session: Res<Session>,
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
        if lobby.refocus {
            lobby.refocus = false;
            // The click moved DOM focus to the canvas; re-show pulls it back to the hidden input.
            *shown_focus = None;
        }
        // Enter/Tab come from winit; text edits go through edit_field (see there for the web).
        for e in keys.read() {
            let Some(field) = lobby.focus else { continue };
            if e.state != ButtonState::Pressed {
                continue;
            }
            match &e.logical_key {
                Key::Enter => lobby.focus = None,
                Key::Tab => {
                    let next = next_field(field);
                    lobby.focus = Some(next);
                    lobby.caret = char_len(lobby.text(next));
                }
                key => edit_field(&mut lobby, field, key, e.text.as_deref()),
            }
        }
        if pressed(Btn::ThisServer)
            && let Some(p) = store::page()
            && let Some(url) = address::this_server_url(&p.protocol, &p.host)
        {
            lobby.caret = char_len(&url);
            lobby.address = url;
        }
        if pressed(Btn::Back) {
            // A browsing connection has no room to keep; one in a room stays.
            if matches!(
                net.state(),
                NetState::Connecting { .. } | NetState::Waking { .. }
            ) || (*net.state() == NetState::Connected && world.is_none())
            {
                net.disconnect();
            }
            lobby.open = false;
            lobby.focus = None;
            lobby.browsing = false;
        }
        if pressed(Btn::Browse) {
            lobby.browsing = !lobby.browsing;
            if lobby.browsing {
                request(&mut lobby, &mut net, session.phase, Intent::Browse);
            }
        }
        if pressed(Btn::Refresh) {
            request(&mut lobby, &mut net, session.phase, Intent::Browse);
        }
        for (btn, intent) in [(Btn::Create, Intent::Create), (Btn::Join, Intent::Join)] {
            if pressed(btn) {
                request(&mut lobby, &mut net, session.phase, intent);
            }
        }
    }

    sync_soft_keyboard(&mut lobby, &mut shown_focus, &mut kb_seen);
    text_focus.0 = lobby.open && lobby.focus.is_some();

    let browse_open = world.is_none()
        && *net.state() == NetState::Connected
        && lobby.intent == Some(Intent::Browse);
    let (line, tint) = if browse_open && lobby.notice.is_none() {
        ("Browsing public rooms.".to_string(), HP_GREEN)
    } else {
        status_line(net.state(), lobby.notice.as_deref(), world.is_some())
    };
    set_text(&mut status.0, &line);
    status.1.0 = tint;
}

/// Starts `intent`: on an open browsing connection the request goes out at once, otherwise a
/// new connection is dialled and the session sends it after `Hello`.
fn request(lobby: &mut Lobby, net: &mut NetClient, phase: Phase, intent: Intent) {
    let browse_open = phase == Phase::Handshake
        && *net.state() == NetState::Connected
        && lobby.intent == Some(Intent::Browse);
    if net.state().busy() && !browse_open {
        return;
    }
    if intent == Intent::Browse {
        let now = session::clock();
        if lobby.listed_at.is_some_and(|t| now - t < REFRESH_SECS) {
            return;
        }
        lobby.listed_at = Some(now);
        lobby.browsing = true;
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
            if browse_open {
                let msg = request_frame(intent, &lobby.room, (lobby.public, &lobby.title));
                session::send(net, &msg);
            } else {
                net.connect(&url);
            }
        }
        Err(msg) => lobby.notice = Some(msg),
    }
}

/// The room browser table, row clicks, the Public/Private choice and the host's switch.
fn lobby_browse(
    mut commands: Commands,
    mut lobby: ResMut<Lobby>,
    mut net: ResMut<NetClient>,
    session: Res<Session>,
    world: Option<Res<NetWorld>>,
    sim: Res<crate::combat::CombatSim>,
    buttons: Query<(&Interaction, &Btn), Changed<Interaction>>,
    rows: Query<(&Interaction, &RoomRow), Changed<Interaction>>,
    list: Single<Entity, With<RoomRows>>,
    mut parts: Query<(&BrowsePart, &mut Node)>,
    mut texts: Query<(&BrowseText, &mut Text)>,
    mut built: Local<Option<u32>>,
    time: Res<Time>,
) {
    if !lobby.open {
        return;
    }
    // QA: `WOW_BROWSE_JOIN=<s>` joins the first open listed room that many seconds in.
    if world.is_none()
        && !matches!(lobby.intent, Some(Intent::JoinPublic(_)))
        && let Some(at) = std::env::var("WOW_BROWSE_JOIN")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        && time.elapsed_secs() >= at
        && let Some(id) = lobby.rooms.iter().find(|r| r.players < r.max).map(|r| r.id)
    {
        request(&mut lobby, &mut net, session.phase, Intent::JoinPublic(id));
    }
    let you_lead = match (&world, &lobby.settings) {
        (Some(w), Some((_, _, leader))) => w.you == *leader,
        _ => false,
    };
    for (i, b) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        match b {
            Btn::Public => lobby.public = !lobby.public,
            Btn::RoomVis if you_lead => {
                if let Some((public, ..)) = lobby.settings {
                    session::send(&net, &ClientMsg::SetPublic { public: !public });
                }
            }
            _ => {}
        }
    }
    for (i, row) in &rows {
        if *i == Interaction::Pressed && row.open && world.is_none() {
            request(
                &mut lobby,
                &mut net,
                session.phase,
                Intent::JoinPublic(row.id),
            );
        }
    }
    for (part, mut node) in &mut parts {
        let on = match part {
            BrowsePart::Table => lobby.browsing && world.is_none(),
            BrowsePart::HostOnly => you_lead,
        };
        let want = if on { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    let public_now = lobby.settings.as_ref().is_some_and(|s| s.0);
    for (which, mut text) in &mut texts {
        let line = match which {
            BrowseText::PublicChoice => if lobby.public { "Public" } else { "Private" }.to_string(),
            BrowseText::RoomVis => if public_now {
                "Make private"
            } else {
                "Make public"
            }
            .to_string(),
            BrowseText::Visibility => match &lobby.settings {
                Some((true, title, _)) => format!("Public: listed as \"{title}\"."),
                _ => "Private: only people with the code can join.".to_string(),
            },
            BrowseText::Footer => list_footer(lobby.rooms.len(), lobby.rooms.len()),
        };
        set_text(&mut text, &line);
    }
    if *built == Some(lobby.rooms_rev) {
        return;
    }
    *built = Some(lobby.rooms_rev);
    let content = sim.0.content();
    commands.entity(*list).despawn_related::<Children>();
    commands.entity(*list).with_children(|l| {
        for r in lobby.rooms.iter() {
            let open = r.players < r.max;
            let tint = if open { Color::WHITE } else { DIM };
            let cells = row_cells(r, content);
            l.spawn((
                Button,
                RoomRow { id: r.id, open },
                Node {
                    column_gap: px(8),
                    padding: UiRect::axes(px(6), px(5)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                BackgroundColor(if open {
                    Color::srgb(0.07, 0.08, 0.11)
                } else {
                    Color::srgb(0.04, 0.04, 0.05)
                }),
            ))
            .with_children(|row| {
                for (cell, w) in cells.iter().zip([130.0, 90.0, 36.0, 92.0, 38.0, 80.0]) {
                    row.spawn((
                        label(fit_cell(cell, w), 13.0),
                        TextColor(tint),
                        TextLayout::no_wrap(),
                        Node {
                            width: px(w),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ));
                }
            });
        }
    });
}

/// Field text, caret bar and clear buttons.
fn lobby_fields(
    time: Res<Time>,
    mut lobby: ResMut<Lobby>,
    mut field_texts: Query<(&FieldText, &mut Text, &mut TextColor, &ComputedNode)>,
    mut carets: Query<(&CaretBar, &mut Node, &mut Visibility), Without<ClearBtn>>,
    mut clears: Query<(&ClearBtn, &mut Node), Without<CaretBar>>,
    this_server_btn: Query<(), With<ThisServerBtn>>,
) {
    let beside = !this_server_btn.is_empty();
    let blink = ((time.elapsed_secs() * 2.0) as u32).is_multiple_of(2);
    for (FieldText(f), mut text, mut color, node) in &mut field_texts {
        let value = lobby.text(*f);
        let len = char_len(value);
        let focused = lobby.focus == Some(*f);
        let max = max_chars(*f, beside);
        let (shown, tint) = match (value.is_empty(), focused) {
            (true, false) => (placeholder(*f).to_string(), PLACEHOLDER),
            (_, true) => (window(value, lobby.scroll.min(len), max), ACCENT),
            _ => (window(value, 0, max), Color::WHITE),
        };
        // Only an unchanged text has a layout that matches it (one frame behind).
        if !value.is_empty() && text.0 == shown && node.size.x > 0.0 {
            let n = char_len(&shown) as f32;
            lobby.glyph_w = node.size.x * node.inverse_scale_factor / n;
        }
        set_text(&mut text, &shown);
        color.0 = tint;
    }
    if let Some(f) = lobby.focus {
        let len = char_len(lobby.text(f));
        lobby.caret = lobby.caret.min(len);
        lobby.scroll = scroll_to(lobby.scroll, lobby.caret, len, max_chars(f, beside));
    }
    let glyph_w = if lobby.glyph_w > 0.0 {
        lobby.glyph_w
    } else {
        GLYPH_W
    };
    for (CaretBar(f), mut node, mut vis) in &mut carets {
        let on = lobby.focus == Some(*f) && blink;
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        // Absolute offsets start at the padding box, i.e. inside the border.
        let col = lobby.caret.saturating_sub(lobby.scroll);
        node.left = px(TEXT_INSET - 2.0 + col as f32 * glyph_w - 1.0);
    }
    for (ClearBtn(f), mut node) in &mut clears {
        let want = if lobby.text(*f).is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if node.display != want {
            node.display = want;
        }
    }
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
        Field::Title => "optional",
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
            Some(f) => {
                kb::show(lobby.text(f));
                kb::set_caret(lobby.caret);
            }
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
        let raw_caret = kb::caret().unwrap_or_else(|| char_len(&raw));
        let prefix: String = raw.chars().take(raw_caret).collect();
        lobby.caret = char_len(&f.sanitize(&prefix)).min(char_len(&clean));
        if clean != raw {
            kb::set_value(&clean);
            kb::set_caret(lobby.caret);
        }
        *lobby.text_mut(f) = clean;
    } else if raw != lobby.text(f) {
        kb::set_value(lobby.text(f));
        kb::set_caret(lobby.caret);
    } else if kb::has_focus()
        && let Some(c) = kb::caret()
    {
        // Arrow keys and touch editing move the caret inside the input only.
        lobby.caret = c.min(char_len(&raw));
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

    fn typed(field: Field, text: &mut String, caret: &mut usize, key: Key, t: Option<&str>) {
        edit(field, text, caret, &key, t);
    }

    #[test]
    fn editing_filters_and_backspaces() {
        let (mut s, mut c) = (String::new(), 0);
        for t in ["a", "!", " ", "7"] {
            typed(
                Field::Room,
                &mut s,
                &mut c,
                Key::Character(t.into()),
                Some(t),
            );
        }
        assert_eq!(s, "A-7");
        typed(Field::Room, &mut s, &mut c, Key::Backspace, None);
        assert_eq!(s, "A-");
        for _ in 0..3 {
            typed(Field::Room, &mut s, &mut c, Key::Backspace, None);
        }
        assert_eq!((s.as_str(), c), ("", 0));
    }

    #[test]
    fn insert_and_delete_at_the_caret() {
        let mut s = "abde".to_string();
        let mut c = 2;
        typed(
            Field::Address,
            &mut s,
            &mut c,
            Key::Character("c".into()),
            Some("c"),
        );
        assert_eq!((s.as_str(), c), ("abcde", 3));
        typed(Field::Address, &mut s, &mut c, Key::Backspace, None);
        assert_eq!((s.as_str(), c), ("abde", 2));
        typed(Field::Address, &mut s, &mut c, Key::Delete, None);
        assert_eq!((s.as_str(), c), ("abe", 2));
        // Edges are no-ops.
        c = 0;
        typed(Field::Address, &mut s, &mut c, Key::Backspace, None);
        c = 3;
        typed(Field::Address, &mut s, &mut c, Key::Delete, None);
        assert_eq!((s.as_str(), c), ("abe", 3));
    }

    #[test]
    fn caret_keys_move_within_bounds() {
        let mut s = "abc".to_string();
        let mut c = 3;
        typed(Field::Name, &mut s, &mut c, Key::ArrowRight, None);
        assert_eq!(c, 3);
        typed(Field::Name, &mut s, &mut c, Key::ArrowLeft, None);
        assert_eq!(c, 2);
        typed(Field::Name, &mut s, &mut c, Key::Home, None);
        assert_eq!(c, 0);
        typed(Field::Name, &mut s, &mut c, Key::ArrowLeft, None);
        assert_eq!(c, 0);
        typed(Field::Name, &mut s, &mut c, Key::End, None);
        assert_eq!(c, 3);
    }

    #[test]
    fn insert_respects_limit_and_filter_mid_text() {
        let mut s = "a".repeat(Field::Name.max_len());
        let mut c = 1;
        insert_at(Field::Name, &mut s, &mut c, "z");
        assert_eq!(s.len(), Field::Name.max_len());
        let mut s = "ad".to_string();
        let mut c = 1;
        insert_at(Field::Name, &mut s, &mut c, "b!c");
        assert_eq!((s.as_str(), c), ("abcd", 3));
    }

    #[test]
    fn window_keeps_the_caret_in_view() {
        let len = 60;
        let max = 20;
        // Caret at the end: scrolled to the tail.
        let s = scroll_to(0, len, len, max);
        assert_eq!(s, len - max);
        // Moving left inside the window does not scroll.
        assert_eq!(scroll_to(s, len - 5, len, max), s);
        // Moving past the left edge scrolls just enough, leaving the marker column free.
        assert_eq!(scroll_to(s, s, len, max), s - 1);
        // Home returns to the start.
        assert_eq!(scroll_to(s, 0, len, max), 0);
        // Every caret position ends up visible and off the marker column.
        let mut sc = 0;
        for caret in (0..=len).chain((0..=len).rev()) {
            sc = scroll_to(sc, caret, len, max);
            let col = caret - sc;
            assert!(
                col <= max && (sc == 0 || col >= 1),
                "caret {caret} scroll {sc}"
            );
        }
        assert_eq!(scroll_to(5, 3, 10, 20), 0);
    }

    #[test]
    fn window_text_marks_hidden_head() {
        assert_eq!(window("abcdef", 0, 4), "abcd");
        assert_eq!(window("abcdef", 2, 4), "<def");
        assert_eq!(window("abc", 0, 5), "abc");
    }

    #[test]
    fn click_lands_between_glyphs() {
        assert_eq!(caret_at_click(0.0, 10.0, 0, 5, 20), 0);
        assert_eq!(caret_at_click(24.0, 10.0, 0, 5, 20), 2);
        assert_eq!(caret_at_click(26.0, 10.0, 0, 5, 20), 3);
        assert_eq!(caret_at_click(-30.0, 10.0, 0, 5, 20), 0);
        assert_eq!(caret_at_click(500.0, 10.0, 0, 5, 20), 5);
        // Scrolled: offset by the hidden head, never onto the marker.
        assert_eq!(caret_at_click(0.0, 10.0, 4, 30, 20), 5);
        assert_eq!(caret_at_click(50.0, 10.0, 4, 30, 20), 9);
    }

    #[test]
    fn clear_resets_text_and_caret() {
        let mut l = lobby("10.0.0.2", "AB");
        l.caret = 4;
        l.scroll = 1;
        l.clear_field(Field::Address);
        assert!(l.address.is_empty() && l.room == "AB");
        assert_eq!((l.caret, l.scroll, l.focus), (0, 0, Some(Field::Address)));
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
        assert_eq!(next_field(Field::Room), Field::Title);
        assert_eq!(next_field(Field::Title), Field::Address);
    }

    fn room(id: u64, players: u8, age_s: u32) -> RoomInfo {
        RoomInfo {
            id,
            title: "Retro 7".into(),
            host: "Ann".into(),
            players,
            max: 5,
            zone: "tower/2".into(),
            avg_level: 4,
            roles: vec!["backend".into(), "devops".into()],
            in_sprint: false,
            age_s,
        }
    }

    #[test]
    fn rooms_sort_joinable_then_fuller_then_newer() {
        let mut v = vec![
            room(1, 5, 10),
            room(2, 1, 50),
            room(3, 3, 900),
            room(4, 1, 5),
        ];
        sort_rooms(&mut v);
        let ids: Vec<u64> = v.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![3, 4, 2, 1]);
    }

    #[test]
    fn long_cells_are_cut_to_their_column() {
        assert_eq!(fit_cell("Anonymous Dev 1m", 90.0), "Anonymous ~");
        assert_eq!(fit_cell("Lv 3", 38.0), "Lv 3");
    }

    #[test]
    fn row_cells_name_zone_roles_and_full_rooms() {
        let sim = crate::combat::new_sim().unwrap();
        let content = sim.content();
        let cells = row_cells(&room(1, 2, 125), content);
        assert_eq!(cells[0], "Retro 7");
        assert_eq!(cells[1], "Ann 2m");
        assert_eq!(cells[2], "2/5");
        assert_eq!(cells[3], "Meeting Rooms");
        assert_eq!(cells[4], "Lv 4");
        assert_eq!(cells[5], "TNK HEAL");
        let full = RoomInfo {
            in_sprint: true,
            zone: "nowhere".into(),
            roles: vec!["no_such_class".into()],
            ..room(2, 5, 7_300)
        };
        let cells = row_cells(&full, content);
        assert_eq!((cells[1].as_str(), cells[2].as_str()), ("Ann 2h", "FULL"));
        assert_eq!(
            (cells[3].as_str(), cells[5].as_str()),
            ("nowhere", "SPRINT")
        );
        assert_eq!(ago(0), "<1m");
    }

    #[test]
    fn footer_counts_hidden_rows() {
        assert!(list_footer(0, 0).starts_with("No open standups"));
        assert_eq!(
            list_footer(8, 20),
            "8 of 20 public rooms. Refresh for others."
        );
        assert!(list_footer(3, 3).starts_with("3 public"));
    }

    #[test]
    fn browse_and_public_join_need_no_code() {
        let l = lobby("10.0.0.2", "");
        assert!(l.validate(Intent::Browse, false).is_ok());
        assert!(l.validate(Intent::JoinPublic(9), false).is_ok());
    }
}
