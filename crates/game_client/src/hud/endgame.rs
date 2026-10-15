//! `~/endgame` (` by default): Promotion rank and its progress, the six department standings,
//! Legacy Mode with this week's boss lockouts (one chip per Legacy floor boss) and the weekly
//! Sprint seed. The weekly ladder belongs to a later wave, so that row says what is coming.
//! Opened by its key, the Esc menu entry or the touch Menu drawer.
//! QA: `WOW_ENDGAME=1` opens it; `WOW_CAREER_DEMO` (see `qa_career`) sets the numbers once.

use super::{ACCENT, BAR_BG, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, drop_shadow, label, panel_bg};
use super::{set_text, vgrad};
use crate::combat::{CombatSim, SimReady, SimView};
use crate::input::{EndgameOpen, Paused, TextFocus};
use crate::keybinds::{Action, Keybinds};
use crate::live::LiveState;
use crate::net::{Lobby, NetWorld};
use crate::touch::TouchMode;
use bevy::prelude::*;
use combat_core::DEPARTMENTS;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);
const OPEN: Color = Color::srgb(0.45, 0.95, 0.55);
/// Lockout chips spawned up front; content has one per Legacy floor boss.
const LOCK_SLOTS: usize = 12;
const REP_BLUE: Color = Color::srgb(0.35, 0.7, 0.95);
const WIDTH: f32 = 600.0;
const NAME_W: f32 = 96.0;
const TIER_W: f32 = 232.0;
const COUNT_W: f32 = 124.0;

#[derive(Component)]
struct EndgameWindow;
#[derive(Component)]
struct CloseButton;
#[derive(Component)]
struct MenuEntry;
#[derive(Component)]
struct MenuButton;
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Line {
    Header,
    Rank,
    RankXp,
    Blurb,
    Lockouts,
    Seed,
}
#[derive(Component)]
struct RankFill;
#[derive(Component, Clone, Copy)]
enum RepPart {
    Tier(usize),
    Count(usize),
}
#[derive(Component)]
struct RepFill(usize);
#[derive(Component)]
struct LockChip(usize);

pub struct EndgamePlugin;

impl Plugin for EndgamePlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("WOW_ENDGAME").is_some() {
            app.insert_resource(EndgameOpen(true));
        }
        app.add_systems(Startup, spawn_ui).add_systems(
            Update,
            (qa_career, toggle, menu_entry, update_window)
                .chain()
                .in_set(SimReady),
        );
    }
}

fn section(title: &str) -> impl Bundle {
    (label(title, 11.0), TextColor(ACCENT))
}

fn bar(fill: impl Bundle) -> impl Bundle {
    (
        Node {
            height: px(8),
            flex_grow: 1.0,
            border_radius: BorderRadius::all(px(3)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(BAR_BG),
        children![fill],
    )
}

fn fill_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(0),
        top: px(0),
        bottom: px(0),
        width: percent(0),
        ..default()
    }
}

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn((
            EndgameWindow,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: px(56),
                width: px(WIDTH),
                margin: UiRect::left(px(-WIDTH / 2.0)),
                padding: UiRect::all(px(10)),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.7)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(32),
        ))
        .with_children(|w| {
            w.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn((Line::Header, label("~/endgame", 14.0), TextColor(DIM)));
                h.spawn((CloseButton, Button, label("[x]", 14.0)));
            });
            w.spawn(section("PROMOTION"));
            w.spawn((Line::Rank, label("", 14.0), TextColor(GOLD)));
            w.spawn(Node {
                column_gap: px(8),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|r| {
                r.spawn(bar((RankFill, fill_node(), BackgroundColor(GOLD))));
                r.spawn((
                    Line::RankXp,
                    label("", 11.0),
                    TextColor(DIM),
                    Node {
                        width: px(COUNT_W),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
            });
            w.spawn((Line::Blurb, label("", 11.0), TextColor(DIM)));
            w.spawn((
                section("REPUTATION"),
                Node {
                    margin: UiRect::top(px(4)),
                    ..default()
                },
            ));
            for d in 0..DEPARTMENTS {
                w.spawn(Node {
                    column_gap: px(8),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|r| {
                    r.spawn((
                        label("", 12.0),
                        Node {
                            width: px(NAME_W),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        RepName(d),
                    ));
                    r.spawn((
                        RepPart::Tier(d),
                        label("", 11.0),
                        TextColor(REP_BLUE),
                        Node {
                            width: px(TIER_W),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                    r.spawn(bar((RepFill(d), fill_node(), BackgroundColor(REP_BLUE))));
                    r.spawn((
                        RepPart::Count(d),
                        label("", 11.0),
                        TextColor(DIM),
                        Node {
                            width: px(COUNT_W),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ));
                });
            }
            w.spawn((
                section("LOCKOUTS"),
                Node {
                    margin: UiRect::top(px(4)),
                    ..default()
                },
            ));
            w.spawn((Line::Lockouts, label("", 11.0), TextColor(DIM)));
            w.spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(5),
                row_gap: px(3),
                ..default()
            })
            .with_children(|r| {
                for k in 0..LOCK_SLOTS {
                    r.spawn((
                        LockChip(k),
                        label("", 11.0),
                        TextLayout::default().with_no_wrap(),
                        TextColor(DIM),
                        Node {
                            padding: UiRect::axes(px(5), px(1)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(3)),
                            display: Display::None,
                            ..default()
                        },
                        BorderColor::all(DIM),
                    ));
                }
            });
            w.spawn((
                section("WEEKLY SPRINT SEED"),
                Node {
                    margin: UiRect::top(px(4)),
                    ..default()
                },
            ));
            w.spawn((Line::Seed, label("", 11.0), TextColor(DIM)));
        });
    // Esc menu entry, left of "Change role" (keyboard layout only; touch has the Menu drawer).
    commands.spawn((
        MenuEntry,
        Node {
            position_type: PositionType::Absolute,
            top: percent(81),
            width: percent(100),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        GlobalZIndex(51),
        children![(
            Button,
            MenuButton,
            Node {
                margin: UiRect::right(px(390)),
                padding: UiRect::axes(px(14), px(5)),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BorderColor::all(KEY_RIM),
            vgrad(KEY_TOP_HI, KEY_TOP),
            children![label("~/endgame", 16.0)],
        )],
    ));
}

#[derive(Component)]
struct RepName(usize);

/// QA hook `WOW_CAREER_DEMO=<rank>,<rep1>,..,<rep6>[,<burnout>[,<rank xp>]]`: sets the offline
/// player's Promotion rank, standings, burnout and rank XP once, for screenshots.
fn qa_career(mut sim: ResMut<CombatSim>, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Ok(v) = std::env::var("WOW_CAREER_DEMO") else {
        *done = true;
        return;
    };
    let Some(p) = sim.0.player() else { return };
    *done = true;
    let n: Vec<u32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    let mut rep = [0; DEPARTMENTS];
    for (k, r) in rep.iter_mut().enumerate() {
        *r = n.get(k + 1).copied().unwrap_or(0);
    }
    let rank = n.first().copied().unwrap_or(0).min(u32::from(u8::MAX)) as u8;
    let cap = sim.0.content().max_level();
    sim.0.set_level_of(p, cap);
    let rank_xp = n.get(DEPARTMENTS + 2).copied().unwrap_or(0);
    sim.0.debug_set_career(p, rank, rank_xp, rep);
    if let Some(b) = n.get(DEPARTMENTS + 1) {
        sim.0
            .debug_set_burnout(p, (*b).min(u32::from(u16::MAX)) as u16);
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<Paused>,
    focus: Res<TextFocus>,
    mut open: ResMut<EndgameOpen>,
    close: Query<&Interaction, (With<CloseButton>, Changed<Interaction>)>,
    mut window: Single<&mut Node, With<EndgameWindow>>,
) {
    if !paused.0 && !focus.0 && binds.just(&keys, Action::Endgame) {
        open.0 = !open.0;
    }
    if close.iter().any(|i| *i == Interaction::Pressed) {
        open.0 = false;
    }
    let want = if open.0 { Display::Flex } else { Display::None };
    if window.display != want {
        window.display = want;
    }
}

/// The Esc menu button resumes the game and opens the window over it.
fn menu_entry(
    mut paused: ResMut<Paused>,
    mut open: ResMut<EndgameOpen>,
    lobby: Res<Lobby>,
    touch: Res<TouchMode>,
    button: Query<&Interaction, (With<MenuButton>, Changed<Interaction>)>,
    mut entry: Single<&mut Node, With<MenuEntry>>,
) {
    let show = paused.0 && !lobby.open && !touch.active;
    let want = if show { Display::Flex } else { Display::None };
    if entry.display != want {
        entry.display = want;
    }
    if show && button.iter().any(|i| *i == Interaction::Pressed) {
        paused.0 = false;
        open.0 = true;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters
fn update_window(
    open: Res<EndgameOpen>,
    binds: Res<Keybinds>,
    touch: Res<TouchMode>,
    sim: SimView,
    offline: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    live: Res<LiveState>,
    mut lines: Query<(&Line, &mut Text), (Without<RepPart>, Without<RepName>)>,
    mut parts: Query<(&RepPart, &mut Text), (Without<Line>, Without<RepName>)>,
    mut names: Query<(&RepName, &mut Text), (Without<Line>, Without<RepPart>)>,
    mut rank_fill: Query<&mut Node, (With<RankFill>, Without<RepFill>)>,
    mut rep_fill: Query<(&RepFill, &mut Node), (Without<RankFill>, Without<LockChip>)>,
    mut chips: Query<
        (
            &LockChip,
            &mut Text,
            &mut TextColor,
            &mut BorderColor,
            &mut Node,
        ),
        (
            Without<Line>,
            Without<RepPart>,
            Without<RepName>,
            Without<RankFill>,
            Without<RepFill>,
        ),
    >,
) {
    if !open.0 {
        return;
    }
    let Some(u) = sim.player().and_then(|id| sim.unit(id)) else {
        return;
    };
    let content = sim.content();
    let eg = &content.endgame;
    let c = &u.career;
    let cap = content.max_level();
    let need = eg.rank_xp_to_next(c.rank);
    let max_rank = eg.ranks.len();
    let (rank, xp, blurb, frac) = if u.level < cap {
        (
            format!("Promotions open at level {cap}. You are level {}.", u.level),
            "XP fills levels first".to_string(),
            "Until then the Career Ladder (talents) is the only ladder.".to_string(),
            0.0,
        )
    } else {
        let name = eg.rank_name(c.rank).unwrap_or("Level 10, no promotion yet");
        let paragon = eg.paragon_pct(c.rank);
        let rank = format!("{name}  (rank {} of {max_rank})", c.rank);
        let blurb = usize::from(c.rank)
            .checked_sub(1)
            .and_then(|i| eg.ranks.get(i))
            .map_or_else(
                || "Kill XP and dailies at the cap now count towards a promotion.".to_string(),
                |r| r.blurb.clone(),
            );
        let blurb = format!(
            "Paragon +{paragon}% max HP and mana (cap +{}%). {blurb}",
            eg.paragon_cap_pct
        );
        if need == 0 {
            (rank, "out of titles".to_string(), blurb, 1.0)
        } else {
            let frac = c.rank_xp as f32 / need as f32;
            let xp = format!(
                "{} / {} XP",
                crate::copy::thousands(c.rank_xp),
                crate::copy::thousands(need)
            );
            (rank, xp, blurb, frac)
        }
    };
    let unix = crate::live::unix_now(&*sim, &offline, net.is_some(), &live).map(|u| u as u64);
    let seed = match unix {
        Some(t) => format!(
            "Week {} resets in {}. A fixed weekly seed, affixes and a ladder for Sprint runs arrive in a later update; for now every week is as random as the roadmap.",
            combat_core::live::week_index(t),
            crate::live::duration(combat_core::live::secs_to_week_reset(t))
        ),
        None => "The weekly seed waits for the office clock.".to_string(),
    };
    // Legacy Mode: one chip per floor boss, green while its bonus loot is still open this week.
    let view = sim.legacy_view();
    let week = unix.map(combat_core::live::week_index);
    let locked = week.map_or(&[][..], |w| view.locked_in(w));
    let bosses: Vec<(String, bool)> = eg_bosses(content)
        .into_iter()
        .map(|(key, name)| (name, locked.contains(&key)))
        .collect();
    let taken = bosses.iter().filter(|b| b.1).count();
    let lockouts = if u.level < cap {
        format!(
            "Legacy Mode opens at level {cap}: every floor again with one affix, Legacy loot and reputation; boss bonus loot once a week."
        )
    } else {
        let mode = if view.on {
            "Legacy Mode on."
        } else {
            "Normal difficulty; pick Legacy at a floor door."
        };
        let here = crate::legacy::affix_name(content, &view.zone)
            .map(|a| format!(" Here: {a}."))
            .unwrap_or_default();
        let reset = unix.map_or_else(String::new, |t| {
            format!(
                ", resets in {}",
                crate::live::duration(combat_core::live::secs_to_week_reset(t))
            )
        });
        format!(
            "{mode}{here} Boss bonus loot taken {taken}/{}{reset}.",
            bosses.len()
        )
    };
    for (LockChip(k), mut text, mut color, mut border, mut node) in &mut chips {
        let want = match bosses.get(*k) {
            Some((name, done)) => {
                let (s, c) = if *done {
                    (format!("{name}: taken"), DIM.with_alpha(0.7))
                } else {
                    (format!("{name}: open"), OPEN)
                };
                set_text(&mut text, &s);
                color.0 = c;
                *border = BorderColor::all(c.with_alpha(0.6));
                Display::Flex
            }
            None => Display::None,
        };
        if node.display != want {
            node.display = want;
        }
    }
    for (line, mut text) in &mut lines {
        let s = match line {
            // Touch has no key to name; the [x] is right there.
            Line::Header if touch.active => "~/endgame".to_string(),
            Line::Header => format!("~/endgame  ({} to close)", binds.label(Action::Endgame)),
            Line::Rank => rank.clone(),
            Line::RankXp => xp.clone(),
            Line::Blurb => blurb.clone(),
            Line::Lockouts => lockouts.clone(),
            Line::Seed => seed.clone(),
        };
        set_text(&mut text, &s);
    }
    for mut n in &mut rank_fill {
        let w = percent(frac.clamp(0.0, 1.0) * 100.0);
        if n.width != w {
            n.width = w;
        }
    }
    for (RepName(d), mut text) in &mut names {
        let name = eg.departments.get(*d).map_or("", |x| x.name.as_str());
        set_text(&mut text, name);
    }
    // Progress within the current tier; the top tier shows a full bar.
    let span = |rep: u32| -> (f32, String) {
        let t = eg.tier_of(rep);
        let lo = eg.rep_tiers.get(t).map_or(0, |x| x.at);
        match eg.rep_tiers.get(t + 1) {
            Some(next) => (
                (rep - lo) as f32 / (next.at - lo).max(1) as f32,
                format!(
                    "{} / {}",
                    crate::copy::thousands(rep),
                    crate::copy::thousands(next.at)
                ),
            ),
            None => (1.0, crate::copy::thousands(rep)),
        }
    };
    for (part, mut text) in &mut parts {
        let s = match *part {
            RepPart::Tier(d) => {
                let t = eg.tier_of(c.rep[d]);
                eg.rep_tiers
                    .get(t)
                    .map_or_else(String::new, |x| x.name.clone())
            }
            RepPart::Count(d) => span(c.rep[d]).1,
        };
        set_text(&mut text, &s);
    }
    for (RepFill(d), mut n) in &mut rep_fill {
        let w = percent(span(c.rep[*d]).0.clamp(0.0, 1.0) * 100.0);
        if n.width != w {
            n.width = w;
        }
    }
}

/// Legacy floor bosses in content order: (unit key, display name).
fn eg_bosses(content: &combat_core::Content) -> Vec<(String, String)> {
    content
        .legacy
        .zones
        .iter()
        .filter_map(|z| content.zone(&z.zone)?.boss.as_ref())
        .map(|b| {
            let name = content
                .unit_def(&b.unit)
                .map_or_else(|| b.unit.clone(), |d| d.name.clone());
            (b.unit.clone(), name)
        })
        .collect()
}
