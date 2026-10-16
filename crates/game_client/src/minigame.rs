//! Minigame presentation: the frame every minigame shares (invite card, compact header in the
//! quest tracker's slot with the run's lines and bars, the choice card, leave confirm, medal
//! screen) and the per-game pieces in the world (Teambuilding's vision cones, canapes and
//! objective markers, the AI take you are cornered with, the sneak toggle; the Company
//! Calendar games' snacks, terminal marker, reorg furniture and name badges). Rules live in
//! `combat_core::minigame`, `combat_core::teambuilding` and `combat_core::corporate`; this side
//! sends `Command::StartMinigame` / `LeaveMinigame` / `MinigamePick` and follows the events,
//! offline and online alike. Offline the client hosts the travel (`sprint::follow_travel` also
//! follows minigames).
//!
//! Entry: the Company Calendar (`calendar.rs`, the board on the plaza) pushes
//! `Command::StartMinigame`; the invite card stays reachable through `MinigameWindows::invite`
//! with `MinigameHud::offer`.
//!
//! QA hooks: `WOW_MINIGAME=<key>` starts it two seconds in (offline, on the campus);
//! `WOW_MINIGAME_INVITE=<key>` opens the invite card at start; `WOW_MINIGAME_DEMO=talker|medal`
//! shows the take card or a sample medal screen without playing (`medal:<key>` for another
//! minigame than the first). Card options also take the action slot keys 1-4, so
//! `WOW_KEYS=t:Digit1` answers in a scripted shot. In the venue,
//! `WOW_MINIGAME_AT=x,z,yaw_deg[,cam_deg]` places the player once the run starts, and
//! `WOW_MINIGAME_AUTOPLAY=1` hops through the objectives (with their waits) and out.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::arena::Colliders;
use crate::combat::{CombatEvent, PendingCommands, SimReady, SimUnit, SimView};
use crate::game::GameFlow;
use crate::hud::quests::{TOUCH_TOP, TRACKER_TOP, TRACKER_W};
use crate::hud::{
    ACCENT, Banner, DIM, LogLineMsg, PANEL_EDGE, drop_shadow, label, panel_bg, set_text,
};
use crate::input::{MinigameWindows, Paused};
use crate::net::NetWorld;
use crate::player::Sneak;
use crate::touch::TouchMode;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use combat_core::{
    Card, Command, EntityId, Event, Medal, MinigameDef, MinigameResult, MinigameView, PropKind,
    Rules, WatcherKind,
};
use std::collections::HashMap;

const GOOD: Color = Color::srgb(0.4, 1.0, 0.5);
const BAD: Color = Color::srgb(1.0, 0.38, 0.32);
const WARN: Color = Color::srgb(1.0, 0.75, 0.3);
const FLASH_SECS: f32 = 3.5;
/// Rays per vision cone; enough for a smooth edge where walls cut it.
const CONE_RAYS: usize = 18;
/// Cones lie just above the grass so they read on every surface.
const CONE_Y: f32 = 0.06;
/// Sight lines are cut by anything taller than a chest (`teambuilding::CHEST_Y`).
const CONE_CUT_Y: f32 = 1.1;
/// The choice card's width (touch narrows it to fit between the stick and the buttons).
const CARD_W: f32 = 540.0;

pub fn medal_color(m: Medal) -> Color {
    match m {
        Medal::Bronze => Color::srgb(0.85, 0.55, 0.3),
        Medal::Silver => Color::srgb(0.82, 0.86, 0.92),
        Medal::Gold => Color::srgb(1.0, 0.82, 0.3),
    }
}

fn kind_color(k: WatcherKind) -> Color {
    match k {
        WatcherKind::Manager => Color::srgb(1.0, 0.62, 0.2),
        WatcherKind::Talker => Color::srgb(0.35, 0.65, 1.0),
        WatcherKind::Hr => Color::srgb(1.0, 0.4, 0.75),
        WatcherKind::Ceo => Color::srgb(1.0, 0.18, 0.15),
    }
}

/// What the HUD knows about the outing, from the minigame events.
#[derive(Resource, Default)]
pub struct MinigameHud {
    /// My run is going on: the header replaces the quest tracker, Esc asks to leave.
    pub active: bool,
    /// The minigame the invite card shows (also the one being played).
    pub offer: String,
    view: Option<MinigameView>,
    got_at: f32,
    /// This outing's results, everyone's, in arrival order.
    results: Vec<MinigameResult>,
    results_dirty: bool,
    /// The AI take I am cornered with: (who, take), hidden early by "Nod politely".
    take: Option<(String, String)>,
    flash: Option<(String, Color, f32)>,
}

impl MinigameHud {
    /// A card with options is up: the action slot keys answer it instead of casting.
    pub fn card_keys(&self) -> bool {
        self.active
            && self
                .view
                .as_ref()
                .and_then(|v| v.card.as_ref())
                .is_some_and(|c| !c.options.is_empty())
    }
}

/// Name badges the current run puts over units (Reorg); the plates show them as the title.
#[derive(Resource, Default)]
pub struct PlateBadges(pub HashMap<EntityId, String>);

pub struct MinigamePlugin;

impl Plugin for MinigamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MinigameHud>()
            .init_resource::<PlateBadges>()
            .add_systems(Startup, spawn_ui)
            .add_systems(
                Update,
                (
                    track_events,
                    qa_hooks,
                    qa_play,
                    buttons,
                    card_keys,
                    update_header,
                    card_panel,
                    invite_window,
                    leave_window,
                    result_window,
                    take_card,
                    venue_props,
                    reorg_furniture,
                    cones,
                    badges,
                )
                    .chain()
                    .in_set(SimReady),
            );
        if let Ok(key) = std::env::var("WOW_MINIGAME_INVITE") {
            app.insert_resource(MinigameWindows {
                invite: true,
                ..default()
            });
            app.insert_resource(MinigameHud {
                offer: key,
                ..default()
            });
        }
    }
}

fn mmss(ms: u32) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

fn def<'a>(content: &'a combat_core::Content, key: &str) -> Option<&'a MinigameDef> {
    content.minigame(key)
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum MgButton {
    Join,
    NotNow,
    AskLeave,
    Leave,
    Stay,
    /// Esc means "leave" in a run, so the confirm also offers the pause it replaced.
    Pause,
    CloseResult,
    Nod,
    /// Option `n` of the run's card.
    Option(u8),
}

#[derive(Component)]
struct Header;
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HeaderText {
    Title,
    /// A Teambuilding objective, or a line of the other games.
    Objective(usize),
    Info,
    State,
}
/// Header bar `n`: its row, label, fill and tick mark.
#[derive(Component)]
struct GaugeRow(usize);
#[derive(Component)]
struct GaugeLabel(usize);
#[derive(Component)]
struct GaugeFill(usize);
#[derive(Component)]
struct GaugeMark(usize);
#[derive(Component)]
struct CardPanel;
#[derive(Component)]
struct CardWho;
#[derive(Component)]
struct CardText;
#[derive(Component)]
struct CardOptions;
#[derive(Component)]
struct LeaveTitle;
#[derive(Component)]
struct LeaveBody;
#[derive(Component)]
struct StayLabel;
#[derive(Component)]
struct InviteWindow;
#[derive(Component)]
struct InviteBody;
#[derive(Component)]
struct LeaveWindow;
#[derive(Component)]
struct ResultWindow;
#[derive(Component)]
struct ResultBody;
#[derive(Component)]
struct TakeCard;
#[derive(Component)]
struct TakeText;
#[derive(Component)]
struct FlashText;

fn button(kind: MgButton, text: impl Into<String>, accent: Color) -> impl Bundle {
    (
        kind,
        Button,
        Node {
            padding: UiRect::axes(px(14), px(6)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BorderColor::all(accent),
        BackgroundColor(accent.with_alpha(0.16)),
        children![(label(text, 15.0), TextColor(accent))],
    )
}

fn window(width: f32, top: Val, border: Color, z: i32) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top,
            width: px(width),
            max_width: percent(94),
            margin: UiRect::left(px(-width / 2.0)),
            padding: UiRect::all(px(12)),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(5)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(border),
        // Opaque under the gradient: world-space bubbles must not read through a modal.
        BackgroundColor(Color::srgb(0.05, 0.055, 0.08)),
        panel_bg(),
        drop_shadow(),
        GlobalZIndex(z),
    )
}

fn spawn_ui(mut commands: Commands) {
    // The header takes the quest tracker's slot (the tracker hides while a run is on).
    commands
        .spawn((
            Header,
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(TRACKER_TOP),
                width: px(TRACKER_W),
                padding: UiRect::axes(px(10), px(6)),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.55)),
            panel_bg(),
            GlobalZIndex(12),
        ))
        .with_children(|c| {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|r| {
                r.spawn((HeaderText::Title, label("", 14.0), TextColor(ACCENT)));
                r.spawn((
                    MgButton::AskLeave,
                    Button,
                    Node {
                        padding: UiRect::axes(px(6), px(1)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BorderColor::all(PANEL_EDGE),
                    children![(label("Leave", 12.0), TextColor(DIM))],
                ));
            });
            for k in 0..8 {
                c.spawn((HeaderText::Objective(k), label("", 12.0), TextColor(DIM)));
            }
            c.spawn((HeaderText::Info, label("", 12.0), TextColor(DIM)));
            c.spawn((HeaderText::State, label("", 12.0), TextColor(GOOD)));
            for k in 0..2 {
                c.spawn((
                    GaugeRow(k),
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(1),
                        margin: UiRect::top(px(2)),
                        ..default()
                    },
                ))
                .with_children(|g| {
                    g.spawn((GaugeLabel(k), label("", 11.0), TextColor(DIM)));
                    g.spawn((
                        Node {
                            height: px(6),
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BorderColor::all(PANEL_EDGE),
                        BackgroundColor(Color::srgb(0.07, 0.07, 0.1)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            GaugeFill(k),
                            Node {
                                width: percent(0),
                                height: percent(100),
                                ..default()
                            },
                            BackgroundColor(WARN),
                        ));
                        b.spawn((
                            GaugeMark(k),
                            Node {
                                position_type: PositionType::Absolute,
                                left: percent(50),
                                top: px(-3),
                                width: px(2),
                                height: px(10),
                                display: Display::None,
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                        ));
                    });
                });
            }
        });
    commands
        .spawn((
            InviteWindow,
            window(520.0, px(80), ACCENT.with_alpha(0.7), 32),
        ))
        .with_child((
            InviteBody,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            },
        ));
    commands
        .spawn((
            LeaveWindow,
            window(400.0, percent(32), BAD.with_alpha(0.7), 34),
        ))
        .with_children(|c| {
            c.spawn((LeaveTitle, label("Leave?", 18.0), TextColor(BAD)));
            c.spawn((
                LeaveBody,
                label(
                    "No medal, no score. HR will note that you 'stepped out for a call'.",
                    13.0,
                ),
                TextColor(DIM),
            ));
            c.spawn(Node {
                column_gap: px(10),
                margin: UiRect::top(px(6)),
                ..default()
            })
            .with_children(|r| {
                r.spawn(button(MgButton::Leave, "Leave", BAD));
                r.spawn(button(MgButton::Stay, "Keep going", GOOD))
                    .insert(StayLabel);
                r.spawn(button(MgButton::Pause, "Pause", DIM));
            });
        });
    commands
        .spawn((
            ResultWindow,
            // Below the achievement toast that a Gold title pops at the same moment.
            window(560.0, percent(30), medal_color(Medal::Gold), 33),
        ))
        .with_child((
            ResultBody,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                ..default()
            },
        ));
    // The take: a speech card low in the centre, above the action bar.
    commands
        .spawn((
            TakeCard,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                bottom: px(150),
                width: px(460),
                max_width: percent(80),
                margin: UiRect::left(px(-230)),
                padding: UiRect::all(px(10)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(kind_color(WatcherKind::Talker)),
            BackgroundColor(Color::srgb(0.06, 0.07, 0.11)),
            drop_shadow(),
            GlobalZIndex(30),
        ))
        .with_children(|c| {
            c.spawn((TakeText, label("", 14.0), TextColor(Color::WHITE)));
            c.spawn(button(
                MgButton::Nod,
                "Nod politely",
                kind_color(WatcherKind::Talker),
            ));
        });
    // The choice card: low in the centre, above the action bar (on touch, between the stick
    // and the buttons).
    commands
        .spawn((
            CardPanel,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                bottom: px(150),
                width: px(CARD_W),
                max_width: percent(70),
                margin: UiRect::left(px(-CARD_W / 2.0)),
                padding: UiRect::all(px(10)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.7)),
            BackgroundColor(Color::srgb(0.05, 0.06, 0.09).with_alpha(0.94)),
            drop_shadow(),
            GlobalZIndex(29),
        ))
        .with_children(|c| {
            c.spawn((CardWho, label("", 13.0), TextColor(ACCENT)));
            c.spawn((CardText, label("", 15.0), TextColor(Color::WHITE)));
            c.spawn((
                CardOptions,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(5),
                    ..default()
                },
            ));
        });
    commands.spawn((
        FlashText,
        Node {
            position_type: PositionType::Absolute,
            // Under the zone banner, so a catch right after arrival does not overlap it.
            top: percent(36),
            width: percent(100),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        GlobalZIndex(13),
        children![(label("", 22.0), TextColor(WARN))],
    ));
}

/// Minigame events into the HUD state, banners, log and windows.
fn track_events(
    time: Res<Time>,
    sim: SimView,
    net: Option<Res<NetWorld>>,
    flow: Res<GameFlow>,
    mut hud: ResMut<MinigameHud>,
    mut windows: ResMut<MinigameWindows>,
    mut sneak: ResMut<Sneak>,
    mut events: MessageReader<CombatEvent>,
    mut banners: MessageWriter<Banner>,
    mut log: MessageWriter<LogLineMsg>,
) {
    let now = time.elapsed_secs();
    let me = sim.player();
    let content = sim.content();
    let name = |id: EntityId| -> String {
        if Some(id) == me {
            return "You".into();
        }
        net.as_deref()
            .and_then(|n| n.names.get(&id).cloned())
            .or_else(|| sim.unit(id).map(|u| u.name.clone()))
            .unwrap_or_else(|| "A colleague".into())
    };
    for CombatEvent(ev) in events.read() {
        match ev {
            // The board on the plaza is the Company Calendar now; it lists this minigame.
            Event::Interacted { unit, prop, .. } if Some(*unit) == me => {
                if content
                    .prop(prop)
                    .is_some_and(|p| p.kind == PropKind::Calendar)
                {
                    windows.calendar = true;
                }
            }
            Event::MinigameStarted { key, .. } => {
                hud.active = true;
                hud.offer = key.clone();
                hud.view = None;
                hud.results.clear();
                hud.results_dirty = true;
                hud.take = None;
                windows.invite = false;
                windows.result = false;
                if let Some(d) = def(content, key) {
                    sneak.on = d.teambuilding.is_some();
                    banners.write(Banner {
                        title: d.name.clone(),
                        subtitle: if d.hint.is_empty() {
                            d.blurb.clone()
                        } else {
                            d.hint.clone()
                        },
                        color: ACCENT,
                    });
                    log.write(LogLineMsg(
                        format!("[fun] {} has begun. {}", d.name, d.blurb),
                        ACCENT,
                    ));
                }
            }
            Event::Minigame { unit, view } if Some(*unit) == me => {
                let fresh = !hud.active;
                hud.active = true;
                hud.offer = view.key.clone();
                if fresh && view.elapsed_ms < 2000 {
                    sneak.on = def(content, &view.key).is_some_and(|d| d.teambuilding.is_some());
                }
                let before = hud.view.as_ref().map_or(0, |v| v.objectives);
                if let Some(d) = def(content, &view.key).and_then(|d| d.teambuilding.as_ref()) {
                    for (k, o) in d.objectives.iter().enumerate() {
                        if view.objectives & (1 << k) != 0 && before & (1 << k) == 0 {
                            log.write(LogLineMsg(format!("[fun] Done: {}.", o.name), GOOD));
                        }
                    }
                    let all = (1u16 << d.objectives.len()) - 1;
                    if view.objectives == all && before != all {
                        hud.flash = Some((
                            "All done. Now leave by the taxi rank (east gate).".into(),
                            GOOD,
                            now,
                        ));
                    }
                }
                hud.view = Some(view.clone());
                hud.got_at = now;
            }
            Event::MinigameCaught {
                unit,
                by,
                kind,
                line,
            } => {
                let who = name(*by);
                if Some(*unit) == me {
                    let flash: Option<String> = match kind {
                        WatcherKind::Manager => {
                            Some(format!("{who}: \"{line}\" Back to the start."))
                        }
                        // The take card is the message.
                        WatcherKind::Talker => {
                            hud.take = Some((who.clone(), line.clone()));
                            None
                        }
                        WatcherKind::Hr => Some(format!("{who}: \"{line}\"")),
                        WatcherKind::Ceo => Some(format!("The CEO: \"{line}\"")),
                    };
                    if let Some(f) = flash {
                        hud.flash = Some((f, kind_color(*kind), now));
                    }
                    log.write(LogLineMsg(format!("[fun] Caught by the {who}."), BAD));
                } else {
                    log.write(LogLineMsg(
                        format!("[fun] {} got caught by the {who}.", name(*unit)),
                        DIM,
                    ));
                }
            }
            Event::MinigameNote { unit, text, good } if Some(*unit) == me => {
                let color = if *good { GOOD } else { BAD };
                hud.flash = Some((text.clone(), color, now));
                log.write(LogLineMsg(format!("[fun] {text}"), color));
            }
            Event::MinigameEnded { result } => {
                hud.results.retain(|r| r.unit != result.unit);
                hud.results.push(result.clone());
                hud.results_dirty = true;
                if Some(result.unit) == me {
                    hud.active = false;
                    hud.take = None;
                    windows.leave = false;
                    windows.result = true;
                } else {
                    let what = match result.medal {
                        Some(m) => format!("{m:?} ({})", result.score),
                        None => "no medal".into(),
                    };
                    log.write(LogLineMsg(
                        format!("[fun] {} is out: {what}.", name(result.unit)),
                        DIM,
                    ));
                }
            }
            Event::ZoneLoaded { zone } => {
                let venue = def(content, &hud.offer).is_some_and(|d| d.zone == *zone);
                if !venue {
                    hud.active = false;
                    hud.view = None;
                    hud.take = None;
                    windows.leave = false;
                }
            }
            _ => {}
        }
    }
    sneak.venue = hud.active
        && def(content, &hud.offer)
            .is_some_and(|d| d.zone == flow.zone && d.teambuilding.is_some());
}

fn qa_hooks(
    time: Res<Time>,
    flow: Res<GameFlow>,
    net: Option<Res<NetWorld>>,
    mut hud: ResMut<MinigameHud>,
    mut windows: ResMut<MinigameWindows>,
    mut pending: ResMut<PendingCommands>,
    sim: SimView,
    mut done: Local<bool>,
) {
    if *done || net.is_some() || time.elapsed_secs() < 2.0 {
        return;
    }
    *done = true;
    if let Ok(key) = std::env::var("WOW_MINIGAME")
        && flow.zone == "campus"
    {
        pending.0.push(Command::StartMinigame { key });
    }
    match std::env::var("WOW_MINIGAME_DEMO").as_deref() {
        Ok("talker") => {
            hud.take = sim
                .content()
                .minigames
                .first()
                .and_then(|m| m.teambuilding.as_ref())
                .and_then(|t| t.ai_takes.first().cloned())
                .map(|t| ("AI Enthusiast".into(), t));
        }
        Ok(demo) if demo.starts_with("medal") => {
            let me = sim.player().unwrap_or_default();
            let content = sim.content();
            let chosen = demo
                .strip_prefix("medal:")
                .and_then(|k| content.minigame(k))
                .or(content.minigames.first());
            if let Some(m) = chosen {
                let (summary, gold) = demo_summary(m);
                let silver = m
                    .medal(Medal::Silver)
                    .map_or(gold * 4 / 5, |d| d.min_score + 10);
                hud.offer = m.key.clone();
                hud.results = vec![
                    MinigameResult {
                        unit: me,
                        key: m.key.clone(),
                        medal: Some(Medal::Gold),
                        score: gold,
                        time_ms: 296_000,
                        catches: 0,
                        collected: 7,
                        quit: false,
                        group_score: gold + silver,
                        items: m.medals.get(2).map(|d| d.items.clone()).unwrap_or_default(),
                        summary: summary.clone(),
                    },
                    MinigameResult {
                        unit: me + 1,
                        key: m.key.clone(),
                        medal: Some(Medal::Silver),
                        score: silver,
                        time_ms: 344_000,
                        catches: 1,
                        collected: 5,
                        quit: false,
                        group_score: gold + silver,
                        items: Vec::new(),
                        summary,
                    },
                ];
                hud.results_dirty = true;
                windows.result = true;
            }
        }
        _ => {}
    }
}

/// A believable medal-screen line and Gold score for the sample (`WOW_MINIGAME_DEMO=medal`).
fn demo_summary(m: &MinigameDef) -> (String, u32) {
    let (line, score) = match m.rules() {
        Some(Rules::Review(_)) => (
            "Burned 46 min of your manager's calendar (hard stop: 30). Rating: 3 out of 5.",
            460,
        ),
        Some(Rules::Reorg(_)) => (
            "Found your manager after 5 of 5 reorgs, 0 wrong guesses. You now report to a dotted line.",
            1310,
        ),
        Some(Rules::Migration(_)) => (
            "Migrated 20 of 20 tickets with 0 validation errors. They are migrating back on Monday.",
            1112,
        ),
        Some(Rules::Hackathon(_)) => (
            "Demo 100% built, 8 pizza, 8 energy drinks. It crashed on stage, as is tradition.",
            1080,
        ),
        Some(Rules::Focus(_)) => (
            "60 s of deep work. Interrupted 0 times, deflected 9. You wrote one line of code.",
            1347,
        ),
        _ => ("", 1792),
    };
    (line.to_string(), score)
}

/// Autoplay: (anchor, seconds to stand there).
const AUTOPLAY: [(&str, f32); 9] = [
    ("snack_2", 0.6),
    ("obj_canape", 2.2),
    ("obj_drink", 2.7),
    ("snack_6", 0.6),
    ("obj_swag", 2.2),
    ("obj_photo", 1.5),
    ("snack_9", 0.6),
    ("obj_incident", 21.0),
    ("tb_exit", 2.0),
];

fn qa_play(
    time: Res<Time>,
    flow: Res<GameFlow>,
    hud: Res<MinigameHud>,
    mut body: Single<&mut crate::player::PlayerBody>,
    mut cam: Single<&mut crate::camera::OrbitCamera>,
    mut placed: Local<bool>,
    mut step: Local<Option<(usize, f32)>>,
) {
    if !hud.active || hud.view.is_none() {
        return;
    }
    let now = time.elapsed_secs();
    if !*placed {
        *placed = true;
        let at: Vec<f32> = std::env::var("WOW_MINIGAME_AT")
            .unwrap_or_default()
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        if let [x, z, yaw, ref rest @ ..] = at[..] {
            body.teleport_facing(Vec3::new(x, 0.0, z), yaw.to_radians());
            cam.set_yaw(yaw.to_radians() + rest.first().copied().unwrap_or(0.0).to_radians());
        }
        if std::env::var_os("WOW_MINIGAME_AUTOPLAY").is_some() {
            *step = Some((0, now));
        }
    }
    let Some((k, since)) = *step else { return };
    let Some((anchor, secs)) = AUTOPLAY.get(k) else {
        *step = None;
        return;
    };
    // Held on the spot for the whole step, like a player standing still.
    if let Some(a) = flow.anchors.iter().find(|a| a.0 == *anchor) {
        body.teleport_facing(a.1, a.2);
    }
    if now - since >= *secs {
        *step = Some((k + 1, now));
    }
}

fn buttons(
    sim: SimView,
    pressed: Query<(&MgButton, &Interaction), Changed<Interaction>>,
    mut hud: ResMut<MinigameHud>,
    mut windows: ResMut<MinigameWindows>,
    mut pending: ResMut<PendingCommands>,
    mut paused: ResMut<Paused>,
) {
    for (b, i) in &pressed {
        if *i != Interaction::Pressed {
            continue;
        }
        match b {
            MgButton::Join => {
                if sim.players().first() == sim.player().as_ref() {
                    pending.0.push(Command::StartMinigame {
                        key: hud.offer.clone(),
                    });
                }
                windows.invite = false;
            }
            MgButton::NotNow => windows.invite = false,
            MgButton::AskLeave => windows.leave = true,
            MgButton::Leave => {
                pending.0.push(Command::LeaveMinigame);
                windows.leave = false;
            }
            MgButton::Stay => windows.leave = false,
            MgButton::Pause => {
                windows.leave = false;
                paused.0 = true;
            }
            MgButton::CloseResult => windows.result = false,
            MgButton::Nod => hud.take = None,
            MgButton::Option(n) => {
                if let Some(c) = hud.view.as_ref().and_then(|v| v.card.as_ref()) {
                    pending.0.push(Command::MinigamePick {
                        seq: c.seq,
                        option: *n,
                    });
                }
            }
        }
    }
}

/// The action slot keys 1-4 answer the card while it has options (`combat::cast_keys` holds
/// back the spells meanwhile).
fn card_keys(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    windows: Res<MinigameWindows>,
    hud: Res<MinigameHud>,
    mut pending: ResMut<PendingCommands>,
) {
    if paused.0 || windows.leave || !hud.card_keys() {
        return;
    }
    let Some(c) = hud.view.as_ref().and_then(|v| v.card.as_ref()) else {
        return;
    };
    for i in 0..c.options.len().min(combat_core::minigame::CARD_OPTIONS_MAX) {
        if keys.any_just_pressed(binds.slot_keys(i)) {
            pending.0.push(Command::MinigamePick {
                seq: c.seq,
                option: i as u8,
            });
        }
    }
}

fn update_header(
    time: Res<Time>,
    paused: Res<Paused>,
    net: Option<Res<NetWorld>>,
    sim: SimView,
    hud: Res<MinigameHud>,
    sneak: Res<Sneak>,
    touch: Res<TouchMode>,
    mut root: Single<
        &mut Node,
        (
            With<Header>,
            Without<FlashText>,
            Without<HeaderText>,
            Without<GaugeRow>,
            Without<GaugeFill>,
            Without<GaugeMark>,
        ),
    >,
    mut texts: Query<
        (&HeaderText, &mut Text, &mut TextColor, &mut Node),
        (
            Without<Header>,
            Without<GaugeRow>,
            Without<GaugeFill>,
            Without<GaugeMark>,
            Without<FlashText>,
        ),
    >,
    mut rows: Query<
        (&GaugeRow, &mut Node),
        (
            Without<Header>,
            Without<HeaderText>,
            Without<GaugeFill>,
            Without<GaugeMark>,
            Without<FlashText>,
        ),
    >,
    mut labels: Query<(&GaugeLabel, &mut Text), Without<HeaderText>>,
    mut fills: Query<
        (&GaugeFill, &mut Node, &mut BackgroundColor),
        (
            Without<Header>,
            Without<HeaderText>,
            Without<GaugeRow>,
            Without<GaugeMark>,
            Without<FlashText>,
        ),
    >,
    mut marks: Query<
        (&GaugeMark, &mut Node),
        (
            Without<Header>,
            Without<HeaderText>,
            Without<GaugeRow>,
            Without<GaugeFill>,
            Without<FlashText>,
        ),
    >,
    mut flash: Single<
        (&mut Node, &Children),
        (
            With<FlashText>,
            Without<Header>,
            Without<HeaderText>,
            Without<GaugeRow>,
            Without<GaugeFill>,
            Without<GaugeMark>,
        ),
    >,
    mut flash_text: Query<
        (&mut Text, &mut TextColor),
        (Without<HeaderText>, Without<TakeText>, Without<GaugeLabel>),
    >,
) {
    let now = time.elapsed_secs();
    let (fnode, kids) = &mut *flash;
    let flashing = hud.flash.as_ref().filter(|(_, _, t)| now - t < FLASH_SECS);
    fnode.display = if flashing.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if let (Some((line, color, _)), Some(k)) = (flashing, kids.first())
        && let Ok((mut t, mut c)) = flash_text.get_mut(*k)
    {
        set_text(&mut t, line);
        c.0 = *color;
    }
    let content = sim.content();
    let (Some(view), Some(d)) = (
        hud.view.as_ref().filter(|_| hud.active),
        def(content, &hud.offer),
    ) else {
        root.display = Display::None;
        return;
    };
    root.display = Display::Flex;
    // Touch keeps the right side for buttons: the panel sits under the player frame instead.
    let (left, right, top) = if touch.active {
        (px(12), Val::Auto, px(TOUCH_TOP))
    } else {
        (Val::Auto, px(12), px(TRACKER_TOP))
    };
    if root.top != top || root.left != left {
        root.left = left;
        root.right = right;
        root.top = top;
    }
    let frozen = paused.0 && net.is_none();
    let extra = if frozen {
        0.0
    } else {
        (now - hud.got_at).clamp(0.0, 1.5)
    };
    let elapsed = (view.elapsed_ms + (extra * 1000.0) as u32).min(view.limit_ms);
    let tb = d.teambuilding.as_ref();
    let all = tb.map_or(0, |r| (1u16 << r.objectives.len()) - 1);
    let my_stun = sim
        .player()
        .and_then(|p| sim.unit(p))
        .is_some_and(|u| u.stunned);
    for (kind, mut text, mut color, mut node) in &mut texts {
        let shown = match (*kind, tb) {
            (HeaderText::Title, _) => Some((
                format!(
                    "{}  {}/{}",
                    d.short_name(),
                    mmss(elapsed),
                    mmss(view.limit_ms)
                ),
                if view.limit_ms.saturating_sub(elapsed) < 60_000 {
                    BAD
                } else {
                    ACCENT
                },
            )),
            (HeaderText::Objective(k), Some(rules)) => rules.objectives.get(k).map(|o| {
                let done = view.objectives & (1 << k) != 0;
                let progress = view
                    .dwell
                    .filter(|(i, _)| usize::from(*i) == k)
                    .map(|(_, p)| format!("  {p}%"))
                    .unwrap_or_default();
                if done {
                    (format!("[x] {}", o.name), DIM.with_alpha(0.6))
                } else {
                    (format!("[ ] {}{progress}", o.name), Color::WHITE)
                }
            }),
            (HeaderText::Objective(k), None) => view
                .lines
                .get(k)
                .map(|l| (l.clone(), if k == 0 { Color::WHITE } else { DIM })),
            (HeaderText::Info, Some(_)) => Some((
                if view.objectives == all {
                    format!(
                        "Leave by the taxi rank!  Canapes {}/{}  Score {}",
                        view.collected, view.collectibles, view.score
                    )
                } else {
                    format!(
                        "Canapes {}/{}   Caught {}   Score {}",
                        view.collected, view.collectibles, view.catches, view.score
                    )
                },
                if view.objectives == all { GOOD } else { DIM },
            )),
            (HeaderText::Info, None) => Some((format!("Score {}", view.score), ACCENT)),
            (HeaderText::State, Some(_)) => Some(if my_stun {
                ("HELD UP".to_string(), BAD)
            } else if view.hidden {
                ("HIDDEN".to_string(), GOOD)
            } else if view.noisy {
                (
                    "RUNNING: they can hear you (jump = sneak)".to_string(),
                    WARN,
                )
            } else if sneak.on {
                ("Sneaking (jump = run)".to_string(), GOOD)
            } else {
                ("Walking tall (jump = sneak)".to_string(), WARN)
            }),
            (HeaderText::State, None) => None,
        };
        let Some((s, c)) = shown else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        set_text(&mut text, &s);
        color.0 = c;
    }
    // Teambuilding's one bar is suspicion; the other games bring their own.
    let bars: Vec<(String, u8, Option<u8>, Color)> = match tb {
        Some(_) => vec![(
            "Being noticed".into(),
            view.suspicion.min(100),
            None,
            if view.suspicion >= 70 { BAD } else { WARN },
        )],
        None => view
            .gauges
            .iter()
            .map(|g| (g.label.clone(), g.value.min(100), g.mark, ACCENT))
            .collect(),
    };
    for (GaugeRow(k), mut node) in &mut rows {
        node.display = if *k < bars.len() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (GaugeLabel(k), mut text) in &mut labels {
        if let Some(b) = bars.get(*k) {
            set_text(&mut text, &b.0);
        }
    }
    for (GaugeFill(k), mut node, mut bg) in &mut fills {
        if let Some(b) = bars.get(*k) {
            node.width = percent(f32::from(b.1));
            bg.0 = b.3;
        }
    }
    for (GaugeMark(k), mut node) in &mut marks {
        match bars.get(*k).and_then(|b| b.2) {
            Some(m) => {
                node.display = Display::Flex;
                node.left = percent(f32::from(m));
            }
            None => node.display = Display::None,
        }
    }
}

#[derive(Component)]
struct OptionText(u8);

fn option_button(i: u8, text: String, touch: bool) -> impl Bundle {
    (
        MgButton::Option(i),
        Button,
        Node {
            padding: UiRect::axes(px(12), px(if touch { 7 } else { 5 })),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BorderColor::all(ACCENT.with_alpha(0.6)),
        BackgroundColor(ACCENT.with_alpha(0.12)),
        children![(OptionText(i), label(text, 14.0), TextColor(Color::WHITE))],
    )
}

/// The run's card: who, what, and a button per option (keys 1-4 on a keyboard). Rebuilt when
/// the decision changes; option texts that tick (a cooldown) update in place.
fn card_panel(
    mut commands: Commands,
    hud: Res<MinigameHud>,
    windows: Res<MinigameWindows>,
    touch: Res<TouchMode>,
    mut panel: Single<&mut Node, With<CardPanel>>,
    mut who: Single<&mut Text, (With<CardWho>, Without<CardText>, Without<OptionText>)>,
    mut text: Single<&mut Text, (With<CardText>, Without<CardWho>, Without<OptionText>)>,
    mut option_texts: Query<(&OptionText, &mut Text), (Without<CardWho>, Without<CardText>)>,
    list: Single<Entity, With<CardOptions>>,
    mut built: Local<Option<(u16, usize, bool)>>,
) {
    let card: Option<&Card> = hud
        .view
        .as_ref()
        .filter(|_| hud.active && !windows.result)
        .and_then(|v| v.card.as_ref());
    let Some(c) = card else {
        panel.display = Display::None;
        *built = None;
        return;
    };
    panel.display = Display::Flex;
    // Touch: between the header (left) and the jump and action buttons (right), at the bottom.
    let (left, margin, width, bottom) = if touch.active {
        (percent(30), px(0), percent(48), px(6))
    } else {
        (percent(50), px(-CARD_W / 2.0), px(CARD_W), px(150))
    };
    if panel.bottom != bottom {
        panel.left = left;
        panel.margin.left = margin;
        panel.width = width;
        panel.bottom = bottom;
    }
    set_text(&mut who, &c.who);
    set_text(&mut text, &c.text);
    let label_of = |i: usize, o: &str| {
        if touch.active {
            o.to_string()
        } else {
            format!("{}   {o}", i + 1)
        }
    };
    let sig = (c.seq, c.options.len(), touch.active);
    if *built == Some(sig) {
        for (OptionText(i), mut t) in &mut option_texts {
            if let Some(o) = c.options.get(usize::from(*i)) {
                set_text(&mut t, &label_of(usize::from(*i), o));
            }
        }
        return;
    }
    *built = Some(sig);
    commands.entity(*list).despawn_related::<Children>();
    commands.entity(*list).with_children(|l| {
        for (i, o) in c.options.iter().enumerate() {
            l.spawn(option_button(i as u8, label_of(i, o), touch.active));
        }
    });
}

fn invite_window(
    mut commands: Commands,
    sim: SimView,
    offline: Res<crate::combat::CombatSim>,
    net: Option<Res<NetWorld>>,
    flow: Res<GameFlow>,
    hud: Res<MinigameHud>,
    windows: Res<MinigameWindows>,
    mut window: Single<&mut Node, With<InviteWindow>>,
    body: Single<Entity, With<InviteBody>>,
    mut built: Local<bool>,
    mut seen: Local<(String, usize)>,
) {
    // Where the group is and how big it is decide the RSVP line.
    let now = (flow.zone.clone(), sim.players().len());
    if *seen != now {
        *seen = now;
        *built = false;
    }
    window.display = if windows.invite {
        Display::Flex
    } else {
        Display::None
    };
    if !windows.invite {
        *built = false;
        return;
    }
    if *built {
        return;
    }
    let content = sim.content();
    let Some(d) = def(content, &hud.offer) else {
        return;
    };
    *built = true;
    let me = sim.player();
    let lead = sim.players().first() == me.as_ref();
    let best = match (net.as_deref(), me) {
        (None, Some(p)) => offline.0.minigame_best(p, &d.key),
        _ => None,
    };
    commands.entity(*body).despawn_related::<Children>();
    commands.entity(*body).with_children(|c| {
        c.spawn((
            label(
                "~/calendar/invite.ics  // attendance: optional (mandatory)",
                12.0,
            ),
            TextColor(DIM),
        ));
        c.spawn((label(d.name.clone(), 22.0), TextColor(ACCENT)));
        c.spawn((
            label(format!("{}   {}   solo or co-op", d.blurb, d.length), 13.0),
            TextColor(Color::WHITE),
        ));
        c.spawn((label(d.how.clone(), 13.0), TextColor(DIM)));
        for m in Medal::ALL {
            let Some(md) = d.medal(m) else { continue };
            let rule = match (md.min_score, md.flawless) {
                (0, _) => "finish".to_string(),
                (s, false) => format!("score {s}+"),
                (s, true) => format!("score {s}+, {}", flawless_rule(d)),
            };
            let mut gives = vec![format!("+{} {} rep", md.rep, d.department)];
            if md.relief > 0 {
                gives.push(format!("burnout -{}%", md.relief / 10));
            }
            gives.extend(
                md.items
                    .iter()
                    .skip(match m {
                        Medal::Bronze => 0,
                        // Higher medals repeat the lower ones' items; show only what is new.
                        Medal::Silver => d.medal(Medal::Bronze).map_or(0, |b| b.items.len()),
                        Medal::Gold => d.medal(Medal::Silver).map_or(0, |b| b.items.len()),
                    })
                    .cloned(),
            );
            if md.event.is_some() {
                gives.push("a title".into());
            }
            c.spawn((
                label(
                    format!("{:?} \"{}\" ({rule}): {}", m, md.name, gives.join(", ")),
                    13.0,
                ),
                TextColor(medal_color(m)),
            ));
        }
        if let Some(b) = best {
            c.spawn((
                label(format!("Your best: {b:?}"), 13.0),
                TextColor(medal_color(b)),
            ));
        }
        let note = if !lead {
            Some("Only the group lead can RSVP for everyone.")
        } else if flow.zone != "campus" {
            Some("RSVP from the campus.")
        } else {
            None
        };
        if let Some(n) = note {
            c.spawn((label(n, 12.0), TextColor(WARN)));
        }
        c.spawn(Node {
            column_gap: px(10),
            margin: UiRect::top(px(4)),
            ..default()
        })
        .with_children(|r| {
            if note.is_none() {
                r.spawn(button(MgButton::Join, "Join (RSVP yes)", GOOD));
            }
            r.spawn(button(MgButton::NotNow, "Not now", DIM));
        });
    });
}

fn leave_window(
    sim: SimView,
    windows: Res<MinigameWindows>,
    hud: Res<MinigameHud>,
    mut window: Single<&mut Node, With<LeaveWindow>>,
    mut title: Single<&mut Text, (With<LeaveTitle>, Without<LeaveBody>)>,
    mut body: Single<&mut Text, (With<LeaveBody>, Without<LeaveTitle>)>,
    stay: Single<&Children, With<StayLabel>>,
    mut stay_text: Query<&mut Text, (Without<LeaveTitle>, Without<LeaveBody>)>,
) {
    let open = windows.leave && hud.active;
    window.display = if open { Display::Flex } else { Display::None };
    if !open {
        return;
    }
    let Some(d) = def(sim.content(), &hud.offer) else {
        return;
    };
    let tb = d.teambuilding.is_some();
    set_text(&mut title, &format!("Leave {}?", d.name));
    set_text(
        &mut body,
        if tb {
            "No medal, no score. HR will note that you 'stepped out for a call'."
        } else {
            "No medal, no score. It will come up in your next 1:1."
        },
    );
    for k in stay.iter() {
        if let Ok(mut t) = stay_text.get_mut(k) {
            set_text(&mut t, if tb { "Keep sneaking" } else { "Keep going" });
        }
    }
}

fn result_window(
    mut commands: Commands,
    sim: SimView,
    net: Option<Res<NetWorld>>,
    mut hud: ResMut<MinigameHud>,
    windows: Res<MinigameWindows>,
    mut window: Single<(&mut Node, &mut BorderColor), With<ResultWindow>>,
    body: Single<Entity, With<ResultBody>>,
) {
    let (node, border) = &mut *window;
    node.display = if windows.result {
        Display::Flex
    } else {
        Display::None
    };
    if !windows.result || !hud.results_dirty {
        return;
    }
    hud.results_dirty = false;
    let content = sim.content();
    let me = sim.player();
    let Some(mine) = hud.results.iter().find(|r| Some(r.unit) == me).cloned() else {
        return;
    };
    let Some(d) = def(content, &mine.key) else {
        return;
    };
    let color = mine.medal.map_or(BAD, medal_color);
    **border = BorderColor::all(color);
    let name = |id: EntityId| -> String {
        if Some(id) == me {
            return "You".into();
        }
        net.as_deref()
            .and_then(|n| n.names.get(&id).cloned())
            .unwrap_or_else(|| "Teammate".into())
    };
    let group: u32 = hud
        .results
        .iter()
        .filter(|r| !r.quit)
        .map(|r| r.score)
        .sum();
    let total = sim.players().len().max(hud.results.len());
    commands.entity(*body).despawn_related::<Children>();
    commands.entity(*body).with_children(|c| {
        c.spawn((
            label(format!("~/{}/results.csv  // recorded", d.key), 12.0),
            TextColor(DIM),
        ));
        let headline = match mine.medal {
            Some(m) => format!(
                "{} MEDAL: \"{}\"",
                format!("{m:?}").to_uppercase(),
                d.medal(m).map_or("", |x| x.name.as_str())
            ),
            None if mine.quit && mine.time_ms + 1000 >= d.limit_s * 1000 => {
                "OUT OF TIME. It ended without you.".into()
            }
            None if !mine.quit => "NO MEDAL. Attendance noted.".into(),
            None => "LEFT EARLY. No medal. HR noted it.".into(),
        };
        c.spawn((label(headline, 24.0), TextColor(color)));
        if !mine.quit && !mine.summary.is_empty() {
            c.spawn((
                label(format!("{}   Score {}", mine.summary, mine.score), 15.0),
                TextColor(Color::WHITE),
            ));
        }
        if let (Some(rules), false) = (d.teambuilding.as_ref(), mine.quit) {
            let s = &rules.score;
            c.spawn((
                label(
                    format!(
                        "Time {}   Canapes {}   Caught {}   Score {}",
                        mmss(mine.time_ms),
                        mine.collected,
                        mine.catches,
                        mine.score
                    ),
                    15.0,
                ),
                TextColor(Color::WHITE),
            ));
            c.spawn((
                label(
                    format!(
                        "= {} base + {} canapes - {} caught - {} time ({} per second)",
                        s.base,
                        s.per_snack * u32::from(mine.collected),
                        s.per_catch * u32::from(mine.catches),
                        s.per_second * (mine.time_ms / 1000),
                        s.per_second
                    ),
                    12.0,
                ),
                TextColor(DIM),
            ));
        }
        if let Some(m) = mine.medal
            && let Some(md) = d.medal(m)
        {
            let mut gives = vec![format!("+{} {} reputation", md.rep, d.department)];
            if md.relief > 0 {
                gives.push(format!("burnout -{}%", md.relief / 10));
            }
            gives.extend(mine.items.iter().cloned());
            if let Some(title) = md.event.as_ref().and_then(|e| title_for(content, e)) {
                gives.push(format!("title \"{title}\" (if new)"));
            }
            c.spawn((
                label(format!("Rewards: {}", gives.join(", ")), 13.0),
                TextColor(GOOD),
            ));
        }
        c.spawn((
            label(
                format!(
                    "Group score {group}  ({} of {total} back)",
                    hud.results.len()
                ),
                15.0,
            ),
            TextColor(ACCENT),
        ));
        for r in &hud.results {
            let what = match r.medal {
                Some(m) => format!("{m:?}  {}", r.score),
                None => "no medal".into(),
            };
            c.spawn((
                label(format!("  {:<14} {what}", name(r.unit)), 13.0),
                TextColor(r.medal.map_or(DIM, medal_color)),
            ));
        }
        c.spawn((
            label(
                if hud.results.len() < total {
                    "The others are still at it. The shuttle leaves when everyone is done."
                } else {
                    "Everyone is done: the shuttle takes you back to the campus."
                },
                12.0,
            ),
            TextColor(DIM),
        ));
        c.spawn(button(MgButton::CloseResult, "Close", ACCENT));
    });
}

/// What a clean run means in each game, for the invite's medal lines.
fn flawless_rule(d: &MinigameDef) -> &'static str {
    match d.rules() {
        Some(Rules::Review(_)) => "the manager stays to the end",
        Some(Rules::Reorg(_)) => "no wrong guesses",
        Some(Rules::Migration(_)) => "no validation errors",
        Some(Rules::Hackathon(_)) => "focus never runs out",
        Some(Rules::Focus(_)) => "never interrupted",
        _ => "never caught",
    }
}

/// The title an achievement event unlocks, for the reward line.
fn title_for(content: &combat_core::Content, event: &str) -> Option<String> {
    content
        .world
        .achievements
        .iter()
        .find(|a| a.trigger == combat_core::Trigger::Event(event.to_string()))
        .map(|a| a.title.clone())
}

fn take_card(
    sim: SimView,
    mut hud: ResMut<MinigameHud>,
    mut card: Single<&mut Node, With<TakeCard>>,
    mut text: Single<&mut Text, With<TakeText>>,
) {
    // The card goes when the corner ends, even if nobody nodded.
    let stunned = sim
        .player()
        .and_then(|p| sim.unit(p))
        .is_some_and(|u| u.stunned);
    let demo = std::env::var_os("WOW_MINIGAME_DEMO").is_some();
    if hud.take.is_some() && !stunned && !demo && hud.active {
        hud.take = None;
    }
    card.display = match &hud.take {
        Some((who, take)) => {
            set_text(&mut text, &format!("{who}:\n{take}"));
            Display::Flex
        }
        None => Display::None,
    };
}

#[derive(Component)]
struct VenueProp;
#[derive(Component)]
struct Snack(usize);
#[derive(Component)]
struct ObjectiveMarker(usize);
#[derive(Component)]
struct ExitMarker;

#[derive(Default)]
struct VenueMeshes {
    built_for: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SnackLook {
    Canape,
    Pizza,
    Drink,
}

/// A venue's collectibles (anchor, look) in the view's `taken` order, its markers (anchor
/// names) and its exit beacon.
fn venue_spots(d: &MinigameDef) -> (Vec<(String, SnackLook)>, Vec<String>, Option<&'static str>) {
    match d.rules() {
        Some(Rules::Teambuilding(t)) => (
            (1..=t.snacks)
                .map(|k| (format!("snack_{k}"), SnackLook::Canape))
                .collect(),
            t.objectives.iter().map(|o| o.anchor.clone()).collect(),
            Some("tb_exit"),
        ),
        Some(Rules::Hackathon(h)) => (
            (1..=h.pizzas)
                .map(|k| (format!("pizza_{k}"), SnackLook::Pizza))
                .chain((1..=h.drinks).map(|k| (format!("drink_{k}"), SnackLook::Drink)))
                .collect(),
            Vec::new(),
            None,
        ),
        // Where the next ticket goes: the old terminal, or the new one while carrying.
        Some(Rules::Migration(_)) => (Vec::new(), vec!["term_old".into(), "term_new".into()], None),
        _ => (Vec::new(), Vec::new(), None),
    }
}

/// Snacks, objective markers and the exit beacon, spawned from the layout anchors when the
/// venue loads; hidden as the run's view says.
fn venue_props(
    mut commands: Commands,
    time: Res<Time>,
    sim: SimView,
    flow: Res<GameFlow>,
    hud: Res<MinigameHud>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<VenueProp>>,
    mut snacks: Query<(&Snack, &mut Visibility), (Without<ObjectiveMarker>, Without<ExitMarker>)>,
    mut markers: Query<
        (&ObjectiveMarker, &mut Visibility, &mut Transform),
        (Without<Snack>, Without<ExitMarker>),
    >,
    mut exit: Query<
        (&mut Visibility, &mut Transform),
        (With<ExitMarker>, Without<Snack>, Without<ObjectiveMarker>),
    >,
    mut state: Local<VenueMeshes>,
) {
    let content = sim.content();
    let venue = content.minigames.iter().find(|m| m.zone == flow.zone);
    if state.built_for != flow.zone {
        state.built_for.clone_from(&flow.zone);
        for e in &existing {
            commands.entity(e).despawn();
        }
        let Some(d) = venue else {
            return;
        };
        let (spots, objectives, exit_anchor) = venue_spots(d);
        let at = |name: &str| flow.anchors.iter().find(|a| a.0 == name).map(|a| a.1);
        let plate = meshes.add(Cylinder::new(0.28, 0.04));
        let bite = meshes.add(Sphere::new(0.12));
        let stand = meshes.add(Cylinder::new(0.03, 1.0));
        let pizza_box = meshes.add(Cuboid::new(0.5, 0.07, 0.5));
        let can = meshes.add(Cylinder::new(0.07, 0.2));
        let unlit = |c: Color| StandardMaterial {
            base_color: c,
            emissive: c.to_linear() * 1.2,
            ..default()
        };
        let white = materials.add(unlit(Color::srgb(0.95, 0.95, 0.95)));
        let food = materials.add(unlit(Color::srgb(1.0, 0.75, 0.25)));
        let cardboard = materials.add(unlit(Color::srgb(0.9, 0.62, 0.32)));
        let neon = materials.add(unlit(Color::srgb(0.35, 1.0, 0.45)));
        let metal = materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.62, 0.66),
            metallic: 0.8,
            ..default()
        });
        for (k, (anchor, look)) in spots.iter().enumerate() {
            let Some(p) = at(anchor) else {
                continue;
            };
            let mut e = commands.spawn((
                VenueProp,
                Snack(k),
                Transform::from_translation(p),
                Visibility::default(),
            ));
            // Pizza and cans sit on the side table the layout put there.
            let table_top = 0.93;
            e.with_children(|c| match look {
                SnackLook::Canape => {
                    c.spawn((
                        Mesh3d(stand.clone()),
                        MeshMaterial3d(metal.clone()),
                        Transform::from_xyz(0.0, 0.5, 0.0),
                    ));
                    c.spawn((
                        Mesh3d(plate.clone()),
                        MeshMaterial3d(white.clone()),
                        Transform::from_xyz(0.0, 1.0, 0.0),
                    ));
                    c.spawn((
                        Mesh3d(bite.clone()),
                        MeshMaterial3d(food.clone()),
                        Transform::from_xyz(0.0, 1.1, 0.0),
                    ));
                }
                SnackLook::Pizza => {
                    c.spawn((
                        Mesh3d(pizza_box.clone()),
                        MeshMaterial3d(cardboard.clone()),
                        Transform::from_xyz(0.0, table_top, 0.0),
                    ));
                    c.spawn((
                        Mesh3d(pizza_box.clone()),
                        MeshMaterial3d(cardboard.clone()),
                        Transform::from_xyz(0.03, table_top + 0.075, -0.02)
                            .with_rotation(Quat::from_rotation_y(0.3)),
                    ));
                }
                SnackLook::Drink => {
                    for (dx, dz) in [(-0.1, 0.0), (0.1, 0.05), (0.0, -0.1)] {
                        c.spawn((
                            Mesh3d(can.clone()),
                            MeshMaterial3d(neon.clone()),
                            Transform::from_xyz(dx, table_top + 0.1, dz),
                        ));
                    }
                }
            });
        }
        let gem = meshes.add(Cone {
            radius: 0.35,
            height: 0.7,
        });
        let gold = materials.add(unlit(Color::srgb(1.0, 0.78, 0.2)));
        let green = materials.add(unlit(Color::srgb(0.3, 1.0, 0.45)));
        for (k, anchor) in objectives.iter().enumerate() {
            let Some(p) = at(anchor) else { continue };
            commands.spawn((
                VenueProp,
                ObjectiveMarker(k),
                Mesh3d(gem.clone()),
                MeshMaterial3d(gold.clone()),
                Transform::from_translation(p + Vec3::Y * 2.6)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
                Visibility::default(),
            ));
        }
        if let Some(p) = exit_anchor.and_then(at) {
            commands.spawn((
                VenueProp,
                ExitMarker,
                Mesh3d(gem.clone()),
                MeshMaterial3d(green),
                Transform::from_translation(p + Vec3::Y * 3.0)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::PI))
                    .with_scale(Vec3::splat(1.6)),
                Visibility::Hidden,
            ));
        }
        return;
    }
    let Some(d) = venue else { return };
    let view = hud.view.as_ref().filter(|_| hud.active);
    let taken = view.map_or(0, |v| v.taken);
    for (s, mut vis) in &mut snacks {
        let want = if taken & (1 << s.0) != 0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        vis.set_if_neq(want);
    }
    let t = time.elapsed_secs();
    let tb = d.teambuilding.as_ref();
    let done = view.map_or(0, |v| v.objectives);
    let carrying = view.is_some_and(|v| v.layout & 1 != 0);
    for (m, mut vis, mut tf) in &mut markers {
        let hidden = match tb {
            Some(_) => done & (1 << m.0) != 0,
            // Migration: only the terminal the ticket goes to next.
            None => (m.0 == 1) != carrying,
        };
        let want = if hidden || view.is_none() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        vis.set_if_neq(want);
        tf.translation.y = 2.6 + (t * 2.0 + m.0 as f32).sin() * 0.15;
        tf.rotation = Quat::from_rotation_y(t * 1.5) * Quat::from_rotation_x(std::f32::consts::PI);
    }
    let all = tb.map_or(0, |r| (1u16 << r.objectives.len()) - 1);
    for (mut vis, mut tf) in &mut exit {
        let want = if view.is_some_and(|_| done == all) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        vis.set_if_neq(want);
        tf.translation.y = 3.0 + (t * 3.0).sin() * 0.2;
    }
}

#[derive(Component)]
struct ReorgPiece;

/// What the Reorg's furniture was last built for: (layout bits, colliders added).
#[derive(Default)]
struct ReorgBuilt {
    layout: Option<u32>,
    added: Vec<combat_core::Aabb>,
}

/// The Reorg's desks (one per used slot, in front of its colleague) and partitions, rebuilt
/// whenever the view's layout bits change. Each piece blocks movement, except one that would
/// land on the player.
fn reorg_furniture(
    mut commands: Commands,
    flow: Res<GameFlow>,
    hud: Res<MinigameHud>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut colliders: ResMut<Colliders>,
    me: Single<&Transform, With<crate::player::Player>>,
    existing: Query<Entity, With<ReorgPiece>>,
    mut built: Local<ReorgBuilt>,
) {
    use crate::floors::furniture::{DESK, KEYBOARD, SCREEN};
    let layout = hud
        .view
        .as_ref()
        .filter(|v| hud.active && flow.zone == crate::floors::corporate::REORG && v.layout != 0)
        .map(|v| v.layout);
    if layout == built.layout {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    let added = std::mem::take(&mut built.added);
    colliders.0.retain(|b| !added.contains(b));
    built.layout = layout;
    let Some(bits) = layout else {
        return;
    };
    let at = |name: String| {
        flow.anchors
            .iter()
            .find(|a| a.0 == name)
            .map(|a| (a.1, a.2))
    };
    let player = me.translation;
    let mut solid = |center: Vec3, size: Vec3, colliders: &mut Colliders| {
        let b = crate::collision::aabb(center + Vec3::Y * size.y / 2.0, size);
        let clear = (player.x - center.x).abs() > size.x / 2.0 + 0.6
            || (player.z - center.z).abs() > size.z / 2.0 + 0.6;
        if clear {
            colliders.0.push(b);
            built.added.push(b);
        }
    };
    let desk = crate::floors::model_size(&DESK);
    for k in (0..16).filter(|k| bits & (1 << k) != 0) {
        let Some((pos, yaw)) = at(format!("desk_{}", k + 1)) else {
            continue;
        };
        let rot = Quat::from_rotation_y(yaw);
        let centre = pos + rot * Vec3::new(0.0, 0.0, -(0.5 + desk.z / 2.0));
        let top = desk.y;
        let back = -desk.z / 2.0;
        for (model, local) in [
            (&DESK, Vec3::ZERO),
            (&SCREEN, Vec3::new(0.0, top, back + 0.2)),
            (&KEYBOARD, Vec3::new(0.0, top, back + 0.62)),
        ] {
            let e = crate::floors::spawn_model(
                &mut commands,
                &assets,
                model,
                centre + rot * local,
                yaw,
            );
            commands.entity(e).insert(ReorgPiece);
        }
        let turned = if (yaw.sin()).abs() > 0.7 {
            Vec3::new(desk.z, desk.y, desk.x)
        } else {
            desk
        };
        solid(centre, turned, &mut colliders);
    }
    let fabric = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.42, 0.58),
        perceptual_roughness: 0.95,
        ..default()
    });
    let screen = meshes.add(Cuboid::new(2.4, 1.5, 0.08));
    for k in 0..16 {
        if bits & (1 << (16 + k)) == 0 {
            continue;
        }
        let Some((pos, yaw)) = at(format!("wall_{}", k + 1)) else {
            continue;
        };
        commands.spawn((
            ReorgPiece,
            Mesh3d(screen.clone()),
            MeshMaterial3d(fabric.clone()),
            Transform::from_translation(pos + Vec3::Y * 0.75)
                .with_rotation(Quat::from_rotation_y(yaw)),
        ));
        let size = if (yaw.sin()).abs() > 0.7 {
            Vec3::new(0.08, 1.5, 2.4)
        } else {
            Vec3::new(2.4, 1.5, 0.08)
        };
        solid(pos, size, &mut colliders);
    }
}

/// Reorg badges onto the name plates (`nameplates::update_titles` reads them).
fn badges(hud: Res<MinigameHud>, mut plates: ResMut<PlateBadges>) {
    let want: HashMap<EntityId, String> = hud
        .view
        .as_ref()
        .filter(|_| hud.active)
        .map(|v| v.badges.iter().cloned().collect())
        .unwrap_or_default();
    if plates.0 != want {
        plates.0 = want;
    }
}

#[derive(Component)]
struct ViewCone {
    unit: Entity,
}

/// One flat fan per watcher, cut where a wall or tall prop blocks the view, tinted by who it
/// is and brighter while it is noticing you.
fn cones(
    mut commands: Commands,
    sim: SimView,
    flow: Res<GameFlow>,
    hud: Res<MinigameHud>,
    colliders: Res<Colliders>,
    me: Single<&Transform, (With<crate::player::Player>, Without<ViewCone>)>,
    units: Query<(Entity, &SimUnit, &Transform), Without<ViewCone>>,
    mut fans: Query<(
        Entity,
        &ViewCone,
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
        &mut Transform,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut by_unit: Local<HashMap<Entity, Entity>>,
) {
    let content = sim.content();
    // Teambuilding's watchers, or Open-Space Focus's colleagues on their way to your desk.
    let walkers: Option<Vec<&combat_core::WatcherDef>> = content
        .minigames
        .iter()
        .find(|m| m.zone == flow.zone)
        .and_then(|m| match m.rules()? {
            Rules::Teambuilding(t) => Some(t.watchers.iter().collect()),
            Rules::Focus(f) => Some(f.disturbers.iter().map(|d| &d.walk).collect()),
            _ => None,
        });
    let Some(walkers) = walkers else {
        for (e, ..) in &fans {
            commands.entity(e).despawn();
        }
        by_unit.clear();
        return;
    };
    let seen = hud
        .view
        .as_ref()
        .filter(|_| hud.active)
        .map_or(0.0, |v| f32::from(v.suspicion) / 100.0);
    // Units that are watchers, with their cone.
    let mut alive = Vec::new();
    for (e, su, tf) in &units {
        let Some(u) = sim.unit(su.0) else { continue };
        let Some(w) = walkers.iter().find(|w| w.unit == u.key) else {
            continue;
        };
        alive.push(e);
        let pos = tf.translation;
        let fwd = tf.rotation * Vec3::NEG_Z;
        let yaw = (-fwd.x).atan2(-fwd.z);
        let mesh = fan_mesh(
            pos,
            yaw,
            w.range_m,
            w.half_angle_deg.to_radians(),
            // Only the cones that could be the ones noticing you light up.
            if pos.distance(me.translation) <= w.range_m + 1.0 {
                seen
            } else {
                0.0
            },
            &colliders.0,
        );
        let color = kind_color(w.kind);
        match by_unit.get(&e).and_then(|c| fans.get_mut(*c).ok()) {
            Some((_, _, m, mat, _)) => {
                if let Some(mut slot) = meshes.get_mut(&m.0) {
                    *slot = mesh;
                }
                if let Some(mut mm) = materials.get_mut(&mat.0) {
                    mm.base_color = color;
                }
            }
            None => {
                let c = commands
                    .spawn((
                        ViewCone { unit: e },
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: color,
                            unlit: true,
                            alpha_mode: AlphaMode::Blend,
                            cull_mode: None,
                            ..default()
                        })),
                        Transform::IDENTITY,
                        bevy::light::NotShadowCaster,
                    ))
                    .id();
                by_unit.insert(e, c);
            }
        }
    }
    for (e, cone, ..) in &fans {
        if !alive.contains(&cone.unit) {
            commands.entity(e).despawn();
        }
    }
    by_unit.retain(|u, _| alive.contains(u));
}

/// World-space fan at the feet of a watcher: `CONE_RAYS` rays, each cut at the first blocker
/// taller than a chest. The body is faint and the rim strong (vertex alpha), so the edge you
/// must stay out of reads on grass and on a phone; `heat` (0..1) brightens it while noticing.
fn fan_mesh(
    pos: Vec3,
    yaw: f32,
    range: f32,
    half: f32,
    heat: f32,
    blockers: &[combat_core::Aabb],
) -> Mesh {
    let origin = Vec3::new(pos.x, CONE_Y, pos.z);
    let (body, rim) = (0.26 + 0.3 * heat, 0.95);
    let mut verts = vec![[origin.x, origin.y, origin.z]];
    let mut colors = vec![[1.0, 1.0, 1.0, body * 0.5]];
    for k in 0..=CONE_RAYS {
        let a = yaw - half + 2.0 * half * k as f32 / CONE_RAYS as f32;
        let dir = Vec3::new(-a.sin(), 0.0, -a.cos());
        let len = ray_len(origin, dir, range, blockers);
        let (inner, outer) = (origin + dir * (len - 0.5).max(0.0), origin + dir * len);
        verts.push([inner.x, inner.y, inner.z]);
        colors.push([1.0, 1.0, 1.0, body]);
        verts.push([outer.x, outer.y, outer.z]);
        colors.push([1.0, 1.0, 1.0, rim]);
    }
    let mut idx = Vec::new();
    for k in 0..CONE_RAYS as u32 {
        let (i0, o0, i1, o1) = (1 + 2 * k, 2 + 2 * k, 3 + 2 * k, 4 + 2 * k);
        idx.extend([0, i0, i1, i0, o0, o1, i0, o1, i1]);
    }
    let n = verts.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, verts)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(idx))
}

/// Distance along `dir` (flat) to the first tall blocker, at most `range` (slab test in xz).
fn ray_len(o: Vec3, dir: Vec3, range: f32, blockers: &[combat_core::Aabb]) -> f32 {
    let mut best = range;
    for b in blockers.iter().filter(|b| b.max.y > CONE_CUT_Y) {
        let (mut t0, mut t1) = (0.0f32, best);
        let mut hit = true;
        for (o, d, lo, hi) in [
            (o.x, dir.x, b.min.x, b.max.x),
            (o.z, dir.z, b.min.z, b.max.z),
        ] {
            if d.abs() < 1e-6 {
                if o < lo || o > hi {
                    hit = false;
                    break;
                }
            } else {
                let (a, c) = ((lo - o) / d, (hi - o) / d);
                t0 = t0.max(a.min(c));
                t1 = t1.min(a.max(c));
                if t0 > t1 {
                    hit = false;
                    break;
                }
            }
        }
        if hit && t0 > 0.0 {
            best = best.min(t0);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat_core::Aabb;

    #[test]
    fn a_cone_ray_stops_at_a_wall_and_ignores_a_low_table() {
        let wall = Aabb {
            min: combat_core::Vec3::new(-2.0, 0.0, -5.2),
            max: combat_core::Vec3::new(2.0, 3.0, -4.8),
        };
        let table = Aabb {
            min: combat_core::Vec3::new(-2.0, 0.0, -3.2),
            max: combat_core::Vec3::new(2.0, 0.9, -2.8),
        };
        let o = Vec3::new(0.0, CONE_Y, 0.0);
        let len = ray_len(o, Vec3::NEG_Z, 9.0, &[wall, table]);
        assert!((len - 4.8).abs() < 1e-3, "{len}");
        assert_eq!(ray_len(o, Vec3::Z, 9.0, &[wall]), 9.0);
    }

    #[test]
    fn the_fan_has_an_inner_and_an_outer_point_per_ray() {
        let m = fan_mesh(Vec3::ZERO, 0.0, 8.0, 0.6, 0.0, &[]);
        assert_eq!(m.count_vertices(), 1 + 2 * (CONE_RAYS + 1));
    }
}
