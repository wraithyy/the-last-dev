//! The two log windows: a chat pane (bottom-left: speech, emotes, joins, progress notices) and a compact
//! combat log (bottom-right of the action bar). Both are fixed-size terminal panes fed from a bounded row
//! buffer; lines are word-wrapped into rows up front (the HUD font is monospaced), so scrolling is an
//! integer row offset and only the visible rows exist as UI entities.
//!
//! Window visibility (chat, combat log, `~/metrics`) lives here too: toggled by H / L / K or the dock
//! buttons, remembered per viewer, and replaced by a single tap strip on touch layouts.

use super::{ACCENT, DIM, KEY_RIM, KEY_TOP, PANEL_EDGE, drop_shadow, label};
use crate::config::CAM_ZOOM_PIXELS_PER_LINE;
use crate::input::{Paused, TextFocus};
use crate::net::{load_hud, save_hud};
use crate::touch::TouchMode;
use bevy::input::InputSystems;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::UiSystems;
use std::collections::VecDeque;

/// Visual rows kept per window (wrapped lines count each row).
pub const LOG_CAP: usize = 200;
/// JetBrains Mono advances 0.6 em; the extra margin keeps a full row from overflowing the clip.
const CHAR_W: f32 = 0.62;
const WHEEL_ROWS: f32 = 3.0;
const PAD_X: f32 = 8.0;
const PAD_Y: f32 = 5.0;

const CHAT_W: f32 = 420.0;
const CHAT_FONT: f32 = 12.0;
const CHAT_ROW_H: f32 = 15.0;
const CHAT_ROWS: usize = 11;
const COMBAT_W: f32 = 270.0;
const COMBAT_FONT: f32 = 11.0;
const COMBAT_ROW_H: f32 = 14.0;
const COMBAT_ROWS: usize = 9;
/// Bottom edge of the left and right columns: level with the action bar.
const COLUMN_BOTTOM: f32 = 20.0;
const DOCK_BOTTOM: f32 = 17.0;
/// Above the dock; the combat window sits here so the dock never overlaps it.
const COMBAT_BOTTOM: f32 = 38.0;
const COMBAT_H: f32 = 20.0 + 2.0 * PAD_Y + COMBAT_ROWS as f32 * COMBAT_ROW_H + 2.0;
const METER_GAP: f32 = 8.0;
/// Touch layout: the panel floats above the tap strip, between the stick and the button cluster.
const TOUCH_STRIP_BOTTOM: f32 = 100.0;
const TOUCH_STRIP_H: f32 = 24.0;
const TOUCH_PANEL_BOTTOM: f32 = TOUCH_STRIP_BOTTOM + TOUCH_STRIP_H + 4.0;

/// Idle seconds before the chat starts to fade, and the seconds the fade takes.
const FADE_DELAY: f32 = 6.0;
const FADE_SECS: f32 = 2.0;
const FADE_FLOOR: f32 = 0.35;

pub const PARTY_BLUE: Color = Color::srgb(0.45, 0.7, 1.0);
const CHAT_WHITE: Color = Color::srgb(0.93, 0.95, 1.0);
const CHAT_RED: Color = Color::srgb(1.0, 0.36, 0.3);
const CHAT_ORANGE: Color = Color::srgb(1.0, 0.62, 0.25);

/// Who is speaking, which decides the colours of a chat line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatKind {
    /// A group member's emote.
    Emote,
    /// Friendly NPC talk.
    Say,
    /// Hostile NPC shout.
    Yell,
}

impl ChatKind {
    /// (suffix after the speaker's name, name colour, text colour).
    pub fn style(self) -> (&'static str, Color, Color) {
        match self {
            Self::Emote => (": ", PARTY_BLUE, CHAT_ORANGE),
            Self::Say => (": ", CHAT_WHITE, CHAT_WHITE),
            Self::Yell => (" yells: ", CHAT_RED, CHAT_RED),
        }
    }
}

/// Group members only emote (net_proto has no free text); everything else is an NPC.
pub fn classify_yell(group_member: bool, hostile: bool) -> ChatKind {
    if group_member {
        ChatKind::Emote
    } else if hostile {
        ChatKind::Yell
    } else {
        ChatKind::Say
    }
}

/// One visual row: an optional coloured speaker head followed by a body.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub head: String,
    pub head_color: Color,
    pub body: String,
    pub body_color: Color,
}

struct Repeat {
    key: (String, String),
    count: u32,
    rows: usize,
}

/// Bounded scrollback with the viewport offset and the idle clock that drives the fade.
pub struct LogBuf {
    rows: VecDeque<Row>,
    /// Rows between the newest row and the viewport's bottom edge.
    back: usize,
    last: Option<Repeat>,
    idle: f32,
    cols: usize,
    visible: usize,
}

impl LogBuf {
    pub fn new(cols: usize, visible: usize) -> Self {
        Self {
            rows: VecDeque::new(),
            back: 0,
            last: None,
            idle: 0.0,
            cols,
            visible,
        }
    }

    fn max_back(&self) -> usize {
        self.rows.len().saturating_sub(self.visible)
    }

    /// Appends a line; an identical consecutive line collapses into the previous one with a repeat count.
    pub fn push(&mut self, head: &str, head_color: Color, body: &str, body_color: Color) {
        let key = (head.to_owned(), body.to_owned());
        let (count, replaced) = match &self.last {
            Some(l) if l.key == key => (l.count + 1, l.rows),
            _ => (1, 0),
        };
        for _ in 0..replaced {
            self.rows.pop_back();
        }
        let shown = if count > 1 {
            format!("{body} x{count}")
        } else {
            body.to_owned()
        };
        let wrapped = wrap(head, &shown, self.cols);
        let added = wrapped.len();
        for (h, b) in wrapped {
            self.rows.push_back(Row {
                head: h,
                head_color,
                body: b,
                body_color,
            });
        }
        while self.rows.len() > LOG_CAP {
            self.rows.pop_front();
        }
        self.last = Some(Repeat {
            key,
            count,
            rows: added,
        });
        // A reader who scrolled up keeps their place while new lines arrive below.
        if self.back > 0 && replaced == 0 {
            self.back += added;
        }
        self.back = self.back.min(self.max_back());
        self.idle = 0.0;
    }

    /// Positive moves toward older rows.
    pub fn scroll(&mut self, rows: i32) {
        let max = i64::try_from(self.max_back()).unwrap_or(i64::MAX);
        let next = (i64::try_from(self.back).unwrap_or(0) + i64::from(rows)).clamp(0, max);
        self.back = usize::try_from(next).unwrap_or(0);
        self.idle = 0.0;
    }

    /// The viewport, oldest row first; `None` pads the top while the log is short.
    pub fn view(&self) -> Vec<Option<&Row>> {
        let end = self.rows.len() - self.back.min(self.rows.len());
        let start = end.saturating_sub(self.visible);
        let mut out: Vec<Option<&Row>> = vec![None; self.visible - (end - start)];
        out.extend(self.rows.range(start..end).map(Some));
        out
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[cfg(test)]
    pub fn back(&self) -> usize {
        self.back
    }

    /// Newest row as plain text, for the collapsed touch strip.
    pub fn last_line(&self) -> Option<String> {
        self.rows
            .back()
            .map(|r| format!("{}{}", r.head, r.body.trim_start()))
    }
}

/// Greedy word wrap into rows of at most `cols` characters. The first row also carries `head`;
/// continuation rows are indented two spaces. Words longer than a row are split.
pub fn wrap(head: &str, body: &str, cols: usize) -> Vec<(String, String)> {
    const INDENT: usize = 2;
    let cols = cols.max(INDENT + 4);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut used = 0usize;
    let width_of = |first: bool| {
        if first {
            cols.saturating_sub(head.chars().count()).max(1)
        } else {
            cols - INDENT
        }
    };
    for word in body.split_whitespace() {
        let mut rest = word;
        while !rest.is_empty() {
            let width = width_of(lines.is_empty());
            let w = rest.chars().count();
            let sep = usize::from(used > 0);
            if used + sep + w <= width {
                if sep == 1 {
                    cur.push(' ');
                }
                cur.push_str(rest);
                used += sep + w;
                break;
            }
            if used > 0 {
                lines.push(std::mem::take(&mut cur));
                used = 0;
                continue;
            }
            let cut = rest
                .char_indices()
                .nth(width)
                .map_or(rest.len(), |(i, _)| i);
            lines.push(rest[..cut].to_owned());
            rest = &rest[cut..];
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                (head.to_owned(), l)
            } else {
                (String::new(), format!("{:INDENT$}{l}", ""))
            }
        })
        .collect()
}

/// Opacity of an idle chat window: full while active, easing down to the floor once quiet.
pub fn fade_alpha(idle: f32) -> f32 {
    let t = ((idle - FADE_DELAY) / FADE_SECS).clamp(0.0, 1.0);
    1.0 - t * (1.0 - FADE_FLOOR)
}

const fn cols_for(width: f32, font: f32) -> usize {
    ((width - 2.0 - 2.0 * PAD_X) / (font * CHAR_W)) as usize
}

#[derive(Resource)]
pub struct ChatLog(pub LogBuf);

impl Default for ChatLog {
    fn default() -> Self {
        Self(LogBuf::new(cols_for(CHAT_W, CHAT_FONT), CHAT_ROWS))
    }
}

#[derive(Resource)]
pub struct CombatLog(pub LogBuf);

impl Default for CombatLog {
    fn default() -> Self {
        Self(LogBuf::new(cols_for(COMBAT_W, COMBAT_FONT), COMBAT_ROWS))
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Win {
    Chat,
    Combat,
    Meter,
}

/// Which windows are open; persisted per viewer.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct HudWindows {
    pub chat: bool,
    pub combat: bool,
    pub meter: bool,
}

impl Default for HudWindows {
    fn default() -> Self {
        Self::from_bits(3)
    }
}

impl HudWindows {
    pub fn from_bits(b: u8) -> Self {
        Self {
            chat: b & 1 != 0,
            combat: b & 2 != 0,
            meter: b & 4 != 0,
        }
    }

    pub fn bits(self) -> u8 {
        u8::from(self.chat) | u8::from(self.combat) << 1 | u8::from(self.meter) << 2
    }

    fn slot(&mut self, w: Win) -> &mut bool {
        match w {
            Win::Chat => &mut self.chat,
            Win::Combat => &mut self.combat,
            Win::Meter => &mut self.meter,
        }
    }

    pub fn is_open(self, w: Win) -> bool {
        self.bits() >> (w as u8) & 1 != 0
    }

    pub fn toggle(&mut self, w: Win) {
        let s = self.slot(w);
        *s = !*s;
    }
}

/// Touch layout: which single panel is expanded above the strip.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
struct TouchPanel(Option<Win>);

impl TouchPanel {
    /// Tap cycle: collapsed, chat, combat log, collapsed.
    fn next(self) -> Option<Win> {
        match self.0 {
            None => Some(Win::Chat),
            Some(Win::Chat) => Some(Win::Combat),
            Some(_) => None,
        }
    }
}

#[derive(Component)]
struct LogBody(Win);
#[derive(Component)]
struct LogRow(Win, usize);
#[derive(Component)]
struct LogSpan(Win, usize);
/// Idle-fade target: this node's colour at full opacity.
#[derive(Component)]
struct Fade(Color);
#[derive(Component)]
pub struct WinToggle(pub Win);
#[derive(Component)]
struct DockRoot;
#[derive(Component)]
struct TouchStrip;
#[derive(Component)]
struct TouchStripText;

pub struct LogsPlugin;

impl Plugin for LogsPlugin {
    fn build(&self, app: &mut App) {
        // Screenshot runs must not overwrite a player's remembered layout.
        let saved = if crate::debug::shooting() {
            None
        } else {
            load_hud()
        };
        app.init_resource::<ChatLog>()
            .init_resource::<CombatLog>()
            // QA hook: `WOW_TOUCH_PANEL=chat|log` starts the touch layout with that panel expanded.
            .insert_resource(TouchPanel(
                match std::env::var("WOW_TOUCH_PANEL").as_deref() {
                    Ok("chat") => Some(Win::Chat),
                    Ok("log") => Some(Win::Combat),
                    _ => None,
                },
            ))
            .insert_resource(saved.map_or_else(HudWindows::default, HudWindows::from_bits))
            .add_systems(Startup, (spawn_windows, welcome))
            .add_systems(PreUpdate, wheel.after(InputSystems).after(UiSystems::Focus))
            .add_systems(
                Update,
                (
                    toggle_keys,
                    press_toggles,
                    press_strip,
                    apply_visibility,
                    refresh,
                    persist,
                )
                    .chain(),
            );
    }
}

fn welcome(mut chat: ResMut<ChatLog>) {
    chat.0.push(
        "",
        DIM,
        "Connected to #war-room. Messages may be AI-generated.",
        DIM,
    );
    // The greeting is not activity: the window may start faded.
    chat.0.idle = FADE_DELAY;
}

fn spawn_windows(mut commands: Commands) {
    spawn_log_window(
        &mut commands,
        Win::Chat,
        "~/slack #war-room",
        CHAT_W,
        (CHAT_FONT, CHAT_ROW_H, CHAT_ROWS),
        true,
    );
    spawn_log_window(
        &mut commands,
        Win::Combat,
        "~/prod $ tail -f combat.log",
        COMBAT_W,
        (COMBAT_FONT, COMBAT_ROW_H, COMBAT_ROWS),
        false,
    );
    spawn_dock(&mut commands);
    spawn_strip(&mut commands);
}

fn spawn_log_window(
    commands: &mut Commands,
    win: Win,
    title: &str,
    width: f32,
    (font, row_h, rows): (f32, f32, usize),
    fades: bool,
) {
    let edge = PANEL_EDGE;
    let bar_bg = Color::srgba(0.14, 0.15, 0.2, 0.96);
    let body_bg = Color::srgba(0.03, 0.04, 0.06, 0.84);
    let mut root = commands.spawn((
        win,
        // Above the loot window, which a touch panel would otherwise be hidden behind.
        GlobalZIndex(31),
        Node {
            position_type: PositionType::Absolute,
            width: px(width),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
            overflow: Overflow::clip(),
            ..default()
        },
        BorderColor::all(edge),
        drop_shadow(),
    ));
    if fades {
        root.insert(Fade(edge));
    }
    root.with_children(|root| {
        let mut bar = root.spawn((
            Node {
                height: px(20),
                padding: UiRect::horizontal(px(8)),
                column_gap: px(5),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bar_bg),
        ));
        if fades {
            bar.insert(Fade(bar_bg));
        }
        bar.with_children(|bar| {
            for c in [
                Color::srgb(1.0, 0.37, 0.34),
                Color::srgb(1.0, 0.75, 0.2),
                Color::srgb(0.3, 0.8, 0.35),
            ] {
                bar.spawn((
                    Node {
                        width: px(8),
                        height: px(8),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(c),
                ));
            }
            bar.spawn((
                Node {
                    margin: UiRect::left(px(6)),
                    flex_grow: 1.0,
                    ..default()
                },
                label(title, 10.0),
                TextLayout::no_wrap(),
                TextColor(DIM),
            ));
            close_button(bar, win);
        });
        let mut body = root.spawn((
            LogBody(win),
            Interaction::default(),
            Node {
                padding: UiRect::axes(px(PAD_X), px(PAD_Y)),
                flex_direction: FlexDirection::Column,
                height: px(rows as f32 * row_h + 2.0 * PAD_Y),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(body_bg),
        ));
        if fades {
            body.insert(Fade(body_bg));
        }
        body.with_children(|body| {
            for i in 0..rows {
                body.spawn((
                    LogRow(win, i),
                    Node {
                        height: px(row_h),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    label("", font),
                    TextLayout::no_wrap(),
                    children![(
                        LogSpan(win, i),
                        TextSpan::new(""),
                        TextFont::from_font_size(font),
                        TextColor(CHAT_WHITE),
                    )],
                ));
            }
        });
    });
}

pub fn close_button(parent: &mut ChildSpawnerCommands, win: Win) {
    parent.spawn((
        WinToggle(win),
        Button,
        Node {
            padding: UiRect::horizontal(px(3)),
            ..default()
        },
        children![(label("[x]", 11.0), TextColor(DIM))],
    ));
}

fn spawn_dock(commands: &mut Commands) {
    commands
        .spawn((
            DockRoot,
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                bottom: px(DOCK_BOTTOM),
                column_gap: px(4),
                ..default()
            },
        ))
        .with_children(|dock| {
            for (win, text) in [
                (Win::Chat, "H chat"),
                (Win::Combat, "L log"),
                (Win::Meter, "K dps"),
            ] {
                dock.spawn((
                    WinToggle(win),
                    Button,
                    Node {
                        height: px(16),
                        padding: UiRect::horizontal(px(5)),
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BorderColor::all(KEY_RIM),
                    BackgroundColor(KEY_TOP),
                    children![(label(text, 10.0), TextColor(CHAT_WHITE))],
                ));
            }
        });
}

fn spawn_strip(commands: &mut Commands) {
    commands.spawn((
        TouchStrip,
        GlobalZIndex(31),
        Button,
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            margin: UiRect::left(px(-CHAT_W / 2.0)),
            bottom: px(TOUCH_STRIP_BOTTOM),
            width: px(CHAT_W),
            height: px(TOUCH_STRIP_H),
            padding: UiRect::horizontal(px(8)),
            align_items: AlignItems::Center,
            overflow: Overflow::clip(),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(KEY_RIM),
        BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.8)),
        children![(
            TouchStripText,
            label("", 12.0),
            TextLayout::no_wrap(),
            TextColor(CHAT_WHITE)
        )],
    ));
}

fn toggle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    paused: Res<Paused>,
    focus: Res<TextFocus>,
    mut windows: ResMut<HudWindows>,
) {
    if paused.0 || focus.0 {
        return;
    }
    for (key, win) in [
        (KeyCode::KeyH, Win::Chat),
        (KeyCode::KeyL, Win::Combat),
        (KeyCode::KeyK, Win::Meter),
    ] {
        if keys.just_pressed(key) {
            windows.toggle(win);
        }
    }
}

fn press_toggles(
    buttons: Query<(&Interaction, &WinToggle), Changed<Interaction>>,
    mut windows: ResMut<HudWindows>,
) {
    for (i, WinToggle(win)) in &buttons {
        if *i == Interaction::Pressed {
            windows.toggle(*win);
        }
    }
}

fn press_strip(
    taps: Query<&Interaction, (With<TouchStrip>, Changed<Interaction>)>,
    mut panel: ResMut<TouchPanel>,
) {
    if taps.iter().any(|i| *i == Interaction::Pressed) {
        panel.0 = panel.next();
    }
}

/// Wheel over a log scrolls it and is taken away from the camera zoom.
fn wheel(
    mut scroll: ResMut<AccumulatedMouseScroll>,
    bodies: Query<(&LogBody, &Interaction)>,
    mut chat: ResMut<ChatLog>,
    mut combat: ResMut<CombatLog>,
) {
    if scroll.delta.y == 0.0 {
        return;
    }
    for (LogBody(win), i) in &bodies {
        if *i == Interaction::None {
            continue;
        }
        let notches = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / CAM_ZOOM_PIXELS_PER_LINE,
        };
        let rows = (notches * WHEEL_ROWS).round() as i32;
        let rows = if rows == 0 {
            notches.signum() as i32
        } else {
            rows
        };
        match win {
            Win::Chat => chat.0.scroll(rows),
            Win::Combat => combat.0.scroll(rows),
            Win::Meter => continue,
        }
        scroll.delta = Vec2::ZERO;
        return;
    }
}

/// Where each window sits; the meter stacks above the combat log (or the dock when that is closed).
fn place(node: &mut Node, win: Win, touch: bool, combat_open: bool) {
    let (left, right, bottom, margin) = match (win, touch) {
        (Win::Chat, false) => (px(12), Val::Auto, px(COLUMN_BOTTOM), Val::Px(0.0)),
        (Win::Combat, false) => (Val::Auto, px(12), px(COMBAT_BOTTOM), Val::Px(0.0)),
        (Win::Meter, false) => {
            let bottom = if combat_open {
                COMBAT_BOTTOM + COMBAT_H + METER_GAP
            } else {
                COMBAT_BOTTOM
            };
            (Val::Auto, px(12), px(bottom), Val::Px(0.0))
        }
        (_, true) => (
            percent(50),
            Val::Auto,
            px(TOUCH_PANEL_BOTTOM),
            Val::Px(-node_width(win) / 2.0),
        ),
    };
    let margin = UiRect::left(margin);
    if node.left != left {
        node.left = left;
    }
    if node.right != right {
        node.right = right;
    }
    if node.bottom != bottom {
        node.bottom = bottom;
    }
    if node.margin != margin {
        node.margin = margin;
    }
}

fn node_width(win: Win) -> f32 {
    match win {
        Win::Chat => CHAT_W,
        _ => COMBAT_W,
    }
}

fn apply_visibility(
    windows: Res<HudWindows>,
    touch: Res<TouchMode>,
    panel: Res<TouchPanel>,
    mut roots: Query<(&Win, &mut Node), (Without<DockRoot>, Without<TouchStrip>)>,
    mut dock: Single<&mut Node, (With<DockRoot>, Without<Win>, Without<TouchStrip>)>,
    mut strip: Single<&mut Node, (With<TouchStrip>, Without<Win>, Without<DockRoot>)>,
    mut toggles: Query<(&WinToggle, &mut BorderColor), With<Button>>,
) {
    let show = |on: bool| if on { Display::Flex } else { Display::None };
    for (win, mut node) in &mut roots {
        let on = if touch.active {
            panel.0 == Some(*win)
        } else {
            windows.is_open(*win)
        };
        if node.display != show(on) {
            node.display = show(on);
        }
        place(&mut node, *win, touch.active, windows.combat);
    }
    let desktop = show(!touch.active);
    if dock.display != desktop {
        dock.display = desktop;
    }
    let tap = show(touch.active);
    if strip.display != tap {
        strip.display = tap;
    }
    for (WinToggle(win), mut border) in &mut toggles {
        // Only the dock buttons carry a border colour worth tinting.
        let want = if windows.is_open(*win) {
            ACCENT
        } else {
            KEY_RIM
        };
        if border.top != want {
            *border = BorderColor::all(want);
        }
    }
}

fn refresh(
    time: Res<Time>,
    mut chat: ResMut<ChatLog>,
    combat: Res<CombatLog>,
    bodies: Query<(&LogBody, &Interaction)>,
    mut rows: Query<(&LogRow, &mut Text, &mut TextColor), Without<LogSpan>>,
    mut spans: Query<(&LogSpan, &mut TextSpan, &mut TextColor), Without<LogRow>>,
    mut fades: Query<(
        &Fade,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
    )>,
    touch: Res<TouchMode>,
    mut strip: Single<&mut Text, (With<TouchStripText>, Without<LogRow>)>,
) {
    let hovered = bodies
        .iter()
        .any(|(b, i)| b.0 == Win::Chat && *i != Interaction::None);
    chat.0.idle = if hovered {
        0.0
    } else {
        chat.0.idle + time.delta_secs()
    };
    let alpha = fade_alpha(chat.0.idle);
    for (LogRow(win, i), mut text, mut color) in &mut rows {
        let (buf, a) = match win {
            Win::Chat => (&chat.0, alpha),
            _ => (&combat.0, 1.0),
        };
        let row = buf.view().get(*i).copied().flatten();
        let want = row.map_or(("", Color::NONE), |r| (r.head.as_str(), r.head_color));
        super::set_text(&mut text, want.0);
        let c = want.1.with_alpha(want.1.alpha() * a);
        if color.0 != c {
            color.0 = c;
        }
    }
    for (LogSpan(win, i), mut span, mut color) in &mut spans {
        let (buf, a) = match win {
            Win::Chat => (&chat.0, alpha),
            _ => (&combat.0, 1.0),
        };
        let row = buf.view().get(*i).copied().flatten();
        let (body, c) = row.map_or(("", Color::NONE), |r| (r.body.as_str(), r.body_color));
        if span.0 != body {
            body.clone_into(&mut span.0);
        }
        let c = c.with_alpha(c.alpha() * a);
        if color.0 != c {
            color.0 = c;
        }
    }
    for (Fade(base), bg, border) in &mut fades {
        let c = base.with_alpha(base.alpha() * alpha);
        if let Some(mut bg) = bg
            && bg.0 != c
        {
            bg.0 = c;
        }
        if let Some(mut border) = border
            && border.top != c
        {
            *border = BorderColor::all(c);
        }
    }
    if touch.active {
        let line = chat
            .0
            .last_line()
            .or_else(|| combat.0.last_line())
            .unwrap_or_default();
        super::set_text(&mut strip, &line);
    }
}

fn persist(windows: Res<HudWindows>, mut last: Local<Option<u8>>) {
    if !windows.is_changed() {
        return;
    }
    let bits = windows.bits();
    // The initial resource is either the loaded value or a screenshot run's default: never write it back.
    if windows.is_added() || crate::debug::shooting() {
        *last = Some(bits);
        return;
    }
    if *last != Some(bits) {
        *last = Some(bits);
        save_hud(bits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(cols: usize, visible: usize) -> LogBuf {
        LogBuf::new(cols, visible)
    }

    fn texts(b: &LogBuf) -> Vec<String> {
        b.view()
            .into_iter()
            .map(|r| r.map_or(String::new(), |r| format!("{}{}", r.head, r.body)))
            .collect()
    }

    #[test]
    fn speakers_are_classified_by_membership_then_hostility() {
        assert_eq!(classify_yell(true, false), ChatKind::Emote);
        assert_eq!(classify_yell(true, true), ChatKind::Emote);
        assert_eq!(classify_yell(false, true), ChatKind::Yell);
        assert_eq!(classify_yell(false, false), ChatKind::Say);
        let (suffix, name, text) = ChatKind::Emote.style();
        assert_eq!(suffix, ": ");
        assert_eq!((name, text), (PARTY_BLUE, CHAT_ORANGE));
        assert_ne!(ChatKind::Yell.style().2, ChatKind::Say.style().2);
    }

    #[test]
    fn wrapping_respects_width_head_and_long_words() {
        let rows = wrap("Bob: ", "Let's sync on this, Alice.", 40);
        assert_eq!(
            rows,
            vec![("Bob: ".to_owned(), "Let's sync on this, Alice.".to_owned())]
        );
        let rows = wrap("", "aaaa bbbb cccc", 10);
        assert_eq!(
            rows.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(),
            ["aaaa bbbb", "  cccc"]
        );
        let rows = wrap("", "abcdefghijklmnopqrstuvwxyz", 10);
        assert!(rows.len() >= 3);
        assert!(
            rows.iter()
                .all(|(h, b)| h.chars().count() + b.chars().count() <= 10)
        );
        assert_eq!(wrap("x: ", "", 10).len(), 1);
    }

    #[test]
    fn scrollback_is_bounded_and_keeps_the_readers_place() {
        let mut b = buf(30, 3);
        for i in 0..10 {
            b.push("", Color::WHITE, &format!("line {i}"), Color::WHITE);
        }
        assert_eq!(texts(&b), ["line 7", "line 8", "line 9"]);
        b.scroll(1000);
        assert_eq!(b.back(), 7);
        assert_eq!(texts(&b), ["line 0", "line 1", "line 2"]);
        b.scroll(-1000);
        assert_eq!(b.back(), 0);
        b.scroll(2);
        b.push("", Color::WHITE, "line 10", Color::WHITE);
        assert_eq!(b.back(), 3, "viewport stays on the same lines");
        assert_eq!(texts(&b), ["line 5", "line 6", "line 7"]);
    }

    #[test]
    fn buffer_is_capped_and_short_logs_pad_the_top() {
        let mut b = buf(30, 4);
        b.push("", Color::WHITE, "only", Color::WHITE);
        assert_eq!(texts(&b), ["", "", "", "only"]);
        for i in 0..(LOG_CAP + 50) {
            b.push("", Color::WHITE, &format!("n{i}"), Color::WHITE);
        }
        assert_eq!(b.len(), LOG_CAP);
        b.scroll(i32::MAX);
        assert_eq!(b.back(), LOG_CAP - 4);
    }

    #[test]
    fn identical_consecutive_lines_collapse_with_a_count() {
        let mut b = buf(30, 3);
        for _ in 0..3 {
            b.push("", Color::WHITE, "Ouch.", Color::WHITE);
        }
        assert_eq!(b.len(), 1);
        assert_eq!(texts(&b)[2], "Ouch. x3");
        b.push("", Color::WHITE, "Other.", Color::WHITE);
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn chat_fades_after_the_delay_but_never_below_the_floor() {
        assert_eq!(fade_alpha(0.0), 1.0);
        assert_eq!(fade_alpha(FADE_DELAY), 1.0);
        assert!(fade_alpha(FADE_DELAY + FADE_SECS / 2.0) < 1.0);
        assert!((fade_alpha(1000.0) - FADE_FLOOR).abs() < 1e-6);
    }

    #[test]
    fn window_flags_round_trip_through_their_bits() {
        let mut w = HudWindows::default();
        assert_eq!((w.chat, w.combat, w.meter), (true, true, false));
        w.toggle(Win::Meter);
        assert_eq!(w.bits(), 7);
        assert_eq!(HudWindows::from_bits(w.bits()), w);
        w.toggle(Win::Chat);
        assert!(!w.is_open(Win::Chat) && w.is_open(Win::Meter));
    }
}
