//! The Opening after the credits (docs/ILMARI_09.md B.1): the player's own clock on every screen,
//! the all-staff email, Ill Marie's "I'm feeling much better now" in the plaza and the badge
//! prompt. Plays once, on the first campus entry after the AGI Prototype is down (so a save that
//! finished the game long ago gets it too), skippable with Esc. Offline only: the sim has no wall
//! clock, so this reads the device clock and tells the sim once (`Command::StampIlmari`); online
//! the server never takes either command.
//!
//! QA hook: `WOW_ILMARI=1` plays it from the start on the campus, `WOW_ILMARI=mail|reveal|badge`
//! starts at that beat (screenshots).
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::camera::OrbitCamera;
use crate::combat::{CombatSim, PendingCommands, SimReady};
use crate::copy;
use crate::game::GameFlow;
use crate::hud::{ACCENT, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, label, set_text, vgrad};
use crate::net::GameMode;
use crate::player::{Player, PlayerBody};
use bevy::input::InputSystems;
use bevy::prelude::*;
use combat_core::Command;

/// Screens only: the stamp alone on a flickering black screen.
const SCREENS_SECS: f32 = 6.5;
/// The email waits for the player; this only saves an away-from-keyboard run from sitting there.
const MAIL_AUTO_SECS: f32 = 90.0;
const FIRST_LINE_SECS: f32 = 3.2;
const SECOND_LINE_SECS: f32 = 4.8;
const REFUSED_SECS: f32 = 3.4;
const SIREN_SECS: f32 = 3.6;
const FADE_SECS: f32 = 0.9;
const TIP_SECS: f32 = 2.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Phase {
    #[default]
    Idle,
    Screens,
    Mail,
    /// Marie's two lines, then the badge prompt.
    Reveal,
    Badge,
    Refused,
    Sirens,
}

#[derive(Resource, Default)]
struct Opening {
    phase: Phase,
    /// Seconds in the current phase.
    t: f32,
    /// The "terms" tooltip of the greyed-out button stays up this long.
    tip: f32,
    /// QA: the beat to start at.
    start_at: Option<Phase>,
    /// Esc or the skip button was pressed this frame.
    skip: bool,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Part {
    Overlay,
    Stamp,
    Mail,
    Continue,
    Subtitle,
    Speaker,
    Speech,
    BadgeBox,
    Return,
    Keep,
    Tip,
    Skip,
}

pub struct IlmariOpenPlugin;

impl Plugin for IlmariOpenPlugin {
    fn build(&self, app: &mut App) {
        let start_at = std::env::var("WOW_ILMARI")
            .ok()
            .and_then(|v| match v.trim() {
                "mail" => Some(Phase::Mail),
                "reveal" => Some(Phase::Reveal),
                "badge" => Some(Phase::Badge),
                _ => None,
            });
        app.insert_resource(Opening {
            start_at,
            ..default()
        })
        .add_systems(Startup, spawn_ui)
        .add_systems(PreUpdate, swallow_escape.after(InputSystems))
        .add_systems(Update, (begin, advance, draw).chain().in_set(SimReady));
    }
}

/// The device clock: unix seconds, offset east of UTC in minutes and the short zone name.
fn device_clock() -> (u64, i16, String) {
    #[cfg(target_arch = "wasm32")]
    {
        let now = js_sys::Date::new_0();
        let secs = (js_sys::Date::now() / 1000.0).max(0.0) as u64;
        // `getTimezoneOffset` counts minutes west of UTC; the sim stores east. No abbreviation
        // from here: the sim falls back to `UTC+hh:mm`.
        (secs, (-now.get_timezone_offset()) as i16, String::new())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        // ponytail: shells out to `date` (no libc/chrono, `unsafe` is forbidden here); Windows
        // has no such `date` and reads as UTC. Upgrade path: a tz crate if that matters.
        let out = std::process::Command::new("date")
            .arg("+%z %Z")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();
        let (off, name) = parse_date_output(&out);
        (secs, off, name)
    }
}

/// `+0530 IST` to (330, "IST"); anything unreadable is UTC.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn parse_date_output(out: &str) -> (i16, String) {
    let mut it = out.split_whitespace();
    let off = it.next().and_then(|z| {
        let sign = match z.as_bytes().first()? {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        let d = z.get(1..)?;
        (d.len() == 4 && d.bytes().all(|b| b.is_ascii_digit())).then_some(())?;
        let (h, m): (i16, i16) = (d[..2].parse().ok()?, d[2..].parse().ok()?);
        Some(sign * (h * 60 + m))
    });
    match off {
        Some(o) => (o, it.next().unwrap_or_default().to_string()),
        None => (0, String::new()),
    }
}

/// The Opening starts the first time the campus is up and idle with the AGI down and nothing
/// else (victory screen, credits, sprint) holding the screen.
fn begin(
    mut opening: ResMut<Opening>,
    mut flow: ResMut<GameFlow>,
    sim: Res<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    mode: Res<GameMode>,
    cinema: Res<crate::cutscene::Cinema>,
    sprint: Res<crate::sprint::SprintHud>,
    mut body: Single<&mut PlayerBody, With<Player>>,
    mut cam: Single<&mut OrbitCamera>,
) {
    if opening.phase != Phase::Idle
        || mode.is_online()
        || flow.zone != "campus"
        || flow.input_locked()
        || sprint.active
        || cinema.holds_victory()
        || !sim.0.ilmari_pending()
    {
        return;
    }
    if sim.0.ilmari_stamp().is_none() {
        let (unix_secs, utc_offset_min, tz) = device_clock();
        pending.0.push(Command::StampIlmari {
            unix_secs,
            utc_offset_min,
            tz,
        });
    }
    // "Lands at the campus spawn": the reveal Marie stands 8 m up the path, facing it.
    if let Some(&(_, pos, yaw)) = flow.anchors.iter().find(|a| a.0 == "player_spawn") {
        body.teleport_facing(pos, yaw);
        cam.set_yaw(yaw);
    }
    flow.opening = true;
    opening.phase = opening.start_at.take().unwrap_or(Phase::Screens);
    opening.t = 0.0;
    opening.tip = 0.0;
}

fn swallow_escape(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut opening: ResMut<Opening>,
    button: Query<(&Part, &Interaction), Changed<Interaction>>,
) {
    opening.skip = false;
    if opening.phase == Phase::Idle {
        return;
    }
    // The key is taken here so it does not also open the pause menu.
    if keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        opening.skip = true;
    }
    if button
        .iter()
        .any(|(p, i)| *p == Part::Skip && *i == Interaction::Pressed)
    {
        opening.skip = true;
    }
}

fn advance(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut opening: ResMut<Opening>,
    mut flow: ResMut<GameFlow>,
    mut pending: ResMut<PendingCommands>,
    buttons: Query<(&Part, &Interaction)>,
) {
    if opening.phase == Phase::Idle {
        return;
    }
    let dt = time.delta_secs();
    opening.t += dt;
    opening.tip = (opening.tip - dt).max(0.0);
    let pressed = |which: Part| {
        buttons
            .iter()
            .any(|(p, i)| *p == which && *i == Interaction::Pressed)
    };
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
    let skip = opening.skip;
    let (t, phase) = (opening.t, opening.phase);
    let next = match phase {
        Phase::Idle => None,
        Phase::Screens => (t >= SCREENS_SECS).then_some(Phase::Mail),
        Phase::Mail => {
            (enter || pressed(Part::Continue) || t >= MAIL_AUTO_SECS).then_some(Phase::Reveal)
        }
        Phase::Reveal => (t >= FIRST_LINE_SECS + SECOND_LINE_SECS).then_some(Phase::Badge),
        Phase::Badge => {
            if pressed(Part::Return) {
                opening.tip = TIP_SECS;
            }
            (enter || pressed(Part::Keep)).then_some(Phase::Refused)
        }
        Phase::Refused => (t >= REFUSED_SECS).then_some(Phase::Sirens),
        Phase::Sirens => (t >= SIREN_SECS).then_some(Phase::Idle),
    };
    let next = if skip { Some(Phase::Idle) } else { next };
    if let Some(next) = next {
        opening.phase = next;
        opening.t = 0.0;
        if next == Phase::Idle {
            // Played or skipped, it never plays again; the sim retires the pre-credits Maries.
            pending.0.push(Command::IlmariReveal);
            flow.opening = false;
        }
    }
}

fn draw(
    opening: Res<Opening>,
    sim: Res<CombatSim>,
    time: Res<Time>,
    mut nodes: Query<(&Part, &mut Node)>,
    mut texts: Query<(&Part, &mut Text, &mut TextColor)>,
    mut overlay: Single<&mut BackgroundColor, With<OverlayMark>>,
) {
    let (phase, t) = (opening.phase, opening.t);
    // Black with a flicker as the screens come on, then solid under the mail.
    let flicker = ((time.elapsed_secs() * 23.0).sin() * 43_758.545)
        .fract()
        .abs();
    let (alpha, tint) = match phase {
        Phase::Screens => {
            let ramp = (t / 0.8).min(1.0);
            let k = if t < 1.4 { 0.55 + 0.4 * flicker } else { 0.96 };
            (ramp * k, (0.0, 0.0, 0.0))
        }
        Phase::Mail => (0.94, (0.02, 0.03, 0.06)),
        Phase::Reveal => ((0.94 * (1.0 - t / FADE_SECS)).max(0.0), (0.02, 0.03, 0.06)),
        Phase::Sirens => {
            let pulse = 0.5 + 0.5 * (t * 7.0).sin();
            (0.35 + 0.25 * pulse, (0.5, 0.02, 0.03))
        }
        _ => (0.0, (0.0, 0.0, 0.0)),
    };
    overlay.0 = Color::srgba(tint.0, tint.1, tint.2, alpha);

    let (met, line) = match sim.0.player() {
        Some(p) => (sim.0.marie_met(p), copy::ilmari_stamp(&sim.0)),
        None => (0, None),
    };
    let (speaker, speech) = match phase {
        Phase::Reveal if t < FIRST_LINE_SECS => (copy::ILMARI_BADGE, copy::ILMARI_FIRST_LINE),
        Phase::Reveal => (copy::ILMARI_BADGE, copy::ilmari_second_line(met)),
        Phase::Refused => (copy::ILMARI_BADGE, copy::ILMARI_REFUSED),
        _ => ("", ""),
    };
    let stamp_text = match phase {
        // The line types itself out of the flicker: nothing until the screens are fully on.
        Phase::Screens if t >= 1.0 => line.unwrap_or_default(),
        Phase::Sirens => copy::ILMARI_SIRENS.to_string(),
        _ => String::new(),
    };
    for (part, mut text, mut colour) in &mut texts {
        match part {
            Part::Stamp => {
                set_text(&mut text, &stamp_text);
                colour.0 = if phase == Phase::Sirens {
                    Color::srgb(1.0, 0.8, 0.8)
                } else {
                    Color::WHITE
                };
            }
            Part::Speaker => set_text(&mut text, speaker),
            Part::Speech => set_text(&mut text, speech),
            Part::Tip => set_text(&mut text, copy::ILMARI_RETURN_TIP),
            _ => {}
        }
    }
    for (part, mut node) in &mut nodes {
        let show = match part {
            Part::Overlay => alpha > 0.0,
            Part::Stamp => !stamp_text.is_empty(),
            Part::Mail | Part::Continue => phase == Phase::Mail,
            Part::Subtitle | Part::Speaker | Part::Speech => !speech.is_empty(),
            Part::BadgeBox | Part::Return | Part::Keep => phase == Phase::Badge,
            Part::Tip => phase == Phase::Badge && opening.tip > 0.0,
            Part::Skip => phase != Phase::Idle,
        };
        let disp = if show { Display::Flex } else { Display::None };
        if node.display != disp {
            node.display = disp;
        }
    }
}

#[derive(Component)]
struct OverlayMark;

fn button(part: Part, text: &str, enabled: bool) -> impl Bundle {
    (
        Button,
        part,
        Node {
            padding: UiRect::axes(px(16), px(6)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(5)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(if enabled { KEY_RIM } else { KEY_TOP }),
        vgrad(if enabled { KEY_TOP_HI } else { KEY_TOP }, KEY_TOP),
        children![(
            label(text, 16.0),
            TextColor(if enabled {
                Color::WHITE
            } else {
                DIM.with_alpha(0.45)
            }),
        )],
    )
}

fn spawn_ui(mut commands: Commands) {
    // Overlay: the stamp, then the mail window on top of the same black.
    commands
        .spawn((
            Part::Overlay,
            OverlayMark,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(59),
        ))
        .with_children(|o| {
            o.spawn((
                Part::Stamp,
                label("", 26.0),
                TextColor(Color::WHITE),
                TextLayout::justify(Justify::Center),
                Node {
                    max_width: px(860),
                    display: Display::None,
                    ..default()
                },
            ));
            o.spawn((
                Part::Mail,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    width: px(760),
                    max_width: percent(94),
                    padding: UiRect::all(px(22)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.07, 0.09, 0.13)),
                BorderColor::all(KEY_RIM),
            ))
            .with_children(|m| {
                for (k, v) in copy::ILMARI_MAIL_HEAD {
                    m.spawn(Node {
                        column_gap: px(10),
                        ..default()
                    })
                    .with_children(|r| {
                        r.spawn((label(k, 15.0), TextColor(DIM)));
                        r.spawn((label(v, 15.0), TextColor(ACCENT)));
                    });
                }
                m.spawn(Node {
                    height: px(1),
                    margin: UiRect::vertical(px(6)),
                    ..default()
                })
                .insert(BackgroundColor(KEY_RIM));
                m.spawn((
                    label(copy::ILMARI_MAIL_BODY, 16.0),
                    TextColor(Color::srgb(0.9, 0.93, 1.0)),
                ));
                m.spawn((
                    Node {
                        align_self: AlignSelf::FlexEnd,
                        margin: UiRect::top(px(10)),
                        ..default()
                    },
                    children![button(Part::Continue, "Mark as read  [Enter]", true)],
                ));
            });
        });
    // Marie's lines, like a cutscene subtitle.
    commands.spawn((
        Part::Subtitle,
        Node {
            position_type: PositionType::Absolute,
            bottom: percent(13),
            left: px(0),
            right: px(0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(58),
        children![
            (Part::Speaker, label("", 15.0), TextColor(ACCENT)),
            (
                Part::Speech,
                label("", 21.0),
                TextColor(Color::srgb(0.95, 0.96, 1.0)),
                TextLayout::justify(Justify::Center),
                Node {
                    min_width: px(520),
                    max_width: px(900),
                    padding: UiRect::axes(px(14), px(6)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            ),
        ],
    ));
    // The badge prompt: "Return badge" can be pressed and goes nowhere.
    commands
        .spawn((
            Part::BadgeBox,
            Node {
                position_type: PositionType::Absolute,
                bottom: percent(14),
                left: px(0),
                right: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(8),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(58),
        ))
        .with_children(|b| {
            b.spawn((
                label(copy::ILMARI_BADGE_PROMPT, 20.0),
                TextColor(Color::WHITE),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                TextLayout::justify(Justify::Center),
                Node {
                    min_width: px(520),
                    padding: UiRect::axes(px(14), px(6)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
            ));
            b.spawn(Node {
                column_gap: px(14),
                ..default()
            })
            .with_children(|row| {
                row.spawn(button(Part::Return, "Return badge", false));
                row.spawn(button(Part::Keep, "Keep badge  [Enter]", true));
            });
            b.spawn((Part::Tip, label("", 14.0), TextColor(DIM)));
        });
    commands.spawn((
        Button,
        Part::Skip,
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            right: px(16),
            padding: UiRect::axes(px(10), px(3)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(KEY_RIM),
        vgrad(KEY_TOP_HI, KEY_TOP),
        GlobalZIndex(60),
        children![label("skip  [Esc]", 12.0)],
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shell_clock_parses_and_falls_back_to_utc() {
        assert_eq!(parse_date_output("+0200 CEST\n"), (120, "CEST".into()));
        assert_eq!(parse_date_output("-0800 PST"), (-480, "PST".into()));
        assert_eq!(parse_date_output("+0530 +0530"), (330, "+0530".into()));
        assert_eq!(parse_date_output(""), (0, String::new()));
        assert_eq!(
            parse_date_output("date: illegal option"),
            (0, String::new())
        );
    }

    #[test]
    fn the_whole_opening_runs_about_forty_seconds_with_a_reader() {
        // Six and a half seconds of screens, a mail read in ~20 s, the lines and the prompt.
        let fixed = SCREENS_SECS + FIRST_LINE_SECS + SECOND_LINE_SECS + REFUSED_SECS + SIREN_SECS;
        assert!((20.0..=25.0).contains(&fixed), "{fixed}");
    }
}
