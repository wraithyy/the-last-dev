//! `~/career`: the Career Ladder window (R). Five promotion rows of three cards; a row opens at
//! its level, a click on an open card takes that promotion (out of combat, the sim decides), and
//! the Performance Review button resets every pick for Kudos. A toast and a glowing dock
//! button announce a waiting promotion; anyone's promotion in the group goes to the chat log.
//! QA: `WOW_CAREER=1` opens the window, `WOW_TALENTS=1,2,3,0,0` sets picks once at start.

use super::{
    ACCENT, BAR_BG, DIM, IconCache, LogLineMsg, PANEL_EDGE, drop_shadow, label, panel_bg, set_text,
};
use crate::combat::{CombatEvent, CombatSim, PendingCommands, SimRead, SimReady, SimView};
use crate::input::{CareerOpen, Paused, TextFocus};
use crate::keybinds::{Action, Keybinds};
use bevy::prelude::*;
use combat_core::{Command, EntityId, Event, TALENT_ROWS, TalentPicks};
use std::collections::HashMap;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);
const LOCKED: Color = Color::srgb(0.42, 0.43, 0.48);
const CARD_W: f32 = 250.0;
const LABEL_W: f32 = 128.0;
const TOAST_SECS: f32 = 6.0;

#[derive(Component)]
struct CareerWindow;
#[derive(Component)]
struct CareerRows;
#[derive(Component)]
struct CareerClose;
#[derive(Component)]
struct CareerHeader;
#[derive(Component)]
struct ReviewButton;
#[derive(Component)]
struct ReviewText;
#[derive(Component)]
struct DockButton;
#[derive(Component)]
struct DockText;
#[derive(Component)]
struct PromoToast;
#[derive(Component)]
enum ToastLine {
    Row,
    Hint,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CardState {
    Picked,
    /// Another option of this row was picked.
    Passed,
    Open,
    Locked,
}

#[derive(Component)]
struct Card {
    row: u8,
    choice: u8,
    state: CardState,
}

#[derive(Resource, Default)]
struct Toast(f32);

/// What the rows were last built from; any change rebuilds them.
#[derive(Clone, PartialEq)]
struct Shown {
    class: String,
    level: u8,
    talents: TalentPicks,
    title: String,
}

pub struct CareerPlugin;

impl Plugin for CareerPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("WOW_CAREER").is_some() {
            app.insert_resource(CareerOpen(true));
        }
        app.init_resource::<Toast>()
            .add_systems(Startup, spawn_ui)
            .add_systems(
                Update,
                (
                    qa_talents,
                    toggle,
                    update_window,
                    click_cards,
                    promotion_watch,
                    promotion_log,
                    hide_dock_on_touch,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

/// Rows the level opens that are still unpicked.
fn open_rows(sim: &dyn SimRead, me: EntityId) -> Vec<usize> {
    let Some(u) = sim.unit(me) else {
        return Vec::new();
    };
    let (class, picks) = sim.talents_of(me);
    sim.content()
        .talents
        .for_class(&class)
        .map(|c| {
            c.rows
                .iter()
                .enumerate()
                .filter(|(r, row)| row.level <= u.level && picks[*r] == 0)
                .map(|(r, _)| r)
                .collect()
        })
        .unwrap_or_default()
}

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn((
            CareerWindow,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: px(64),
                width: px(LABEL_W + 3.0 * CARD_W + 40.0),
                margin: UiRect::left(px(-(LABEL_W + 3.0 * CARD_W + 40.0) / 2.0)),
                padding: UiRect::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
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
        .with_children(|c| {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn((label("~/career  (R to close)", 14.0), TextColor(DIM)));
                h.spawn((CareerClose, Button, label("[x]", 14.0)));
            });
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|h| {
                h.spawn((CareerHeader, label("", 13.0), TextColor(GOLD)));
                h.spawn((
                    ReviewButton,
                    Button,
                    Node {
                        padding: UiRect::axes(px(8), px(3)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BorderColor::all(PANEL_EDGE),
                    children![(ReviewText, label("", 12.0), TextColor(DIM))],
                ));
            });
            c.spawn((
                CareerRows,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    ..default()
                },
            ));
        });
    commands.spawn((
        DockButton,
        Button,
        Node {
            position_type: PositionType::Absolute,
            top: px(22),
            left: percent(50),
            margin: UiRect::left(px(-62)),
            padding: UiRect::axes(px(8), px(2)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BorderColor::all(PANEL_EDGE),
        BackgroundColor(super::PANEL),
        GlobalZIndex(12),
        children![(DockText, label("~/career [R]", 11.0), TextColor(DIM))],
    ));
    commands
        .spawn((
            PromoToast,
            Node {
                position_type: PositionType::Absolute,
                top: px(212),
                left: percent(50),
                width: px(420),
                margin: UiRect::left(px(-210)),
                padding: UiRect::axes(px(14), px(10)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(3),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(6)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(GOLD),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(45),
        ))
        .with_children(|t| {
            t.spawn((label("PROMOTION AVAILABLE", 11.0), TextColor(GOLD)));
            t.spawn((ToastLine::Row, label("", 20.0), TextColor(Color::WHITE)));
            t.spawn((ToastLine::Hint, label("", 12.0), TextColor(DIM)));
        });
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<Paused>,
    focus: Res<TextFocus>,
    mut open: ResMut<CareerOpen>,
    close: Query<&Interaction, (With<CareerClose>, Changed<Interaction>)>,
    dock: Query<&Interaction, (With<DockButton>, Changed<Interaction>)>,
    mut window: Single<&mut Node, With<CareerWindow>>,
) {
    if !paused.0 && !focus.0 && binds.just(&keys, Action::Career) {
        open.0 = !open.0;
    }
    if dock.iter().any(|i| *i == Interaction::Pressed) {
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters
fn update_window(
    mut commands: Commands,
    open: Res<CareerOpen>,
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    rows: Single<Entity, With<CareerRows>>,
    mut header: Single<&mut Text, (With<CareerHeader>, Without<ReviewText>)>,
    mut review: Single<(&mut Text, &mut TextColor), (With<ReviewText>, Without<CareerHeader>)>,
    mut cards: Query<(&Card, &Interaction, &mut BorderColor)>,
    mut shown: Local<Option<Shown>>,
) {
    if !open.0 {
        return;
    }
    let Some(me) = sim.player() else { return };
    let Some(u) = sim.unit(me).cloned() else {
        return;
    };
    let content = sim.content();
    let (class, picks) = sim.talents_of(me);
    let class_name = content.class(&class).map_or("", |c| c.name.as_str());
    let worn = content
        .title_text(&u.title)
        .map_or_else(|| "no title".to_string(), |t| format!("<{t}>"));
    set_text(
        &mut header,
        &format!("{class_name}, level {}   {worn}", u.level),
    );
    let cost = content.talents.respec_cost(u.level, u.respecs);
    let any = picks.iter().any(|k| *k > 0);
    let next = content.talents.respec_cost(u.level, u.respecs + 1);
    set_text(
        &mut review.0,
        &format!(
            "Performance Review: reset all for {} (you have {}; the next one costs {})",
            crate::copy::kudos(cost),
            crate::copy::kudos(u.sp),
            crate::copy::thousands(next)
        ),
    );
    review.1.0 = if any && u.sp >= cost { ACCENT } else { LOCKED };
    let now = Shown {
        class: class.clone(),
        level: u.level,
        talents: picks,
        title: u.title.clone(),
    };
    if shown.as_ref() != Some(&now) {
        commands.entity(*rows).despawn_related::<Children>();
        build_rows(&mut commands, *rows, &*sim, &now, &assets, &mut icons);
        *shown = Some(now);
    }
    for (card, interaction, mut border) in &mut cards {
        let hovered = *interaction != Interaction::None;
        let color = match card.state {
            CardState::Picked => GOLD,
            CardState::Open if hovered => Color::WHITE,
            CardState::Open => ACCENT,
            _ if hovered => PANEL_EDGE.with_alpha(1.0),
            _ => PANEL_EDGE.with_alpha(0.5),
        };
        let want = BorderColor::all(color);
        if *border != want {
            *border = want;
        }
    }
}

fn build_rows(
    commands: &mut Commands,
    parent: Entity,
    sim: &dyn SimRead,
    shown: &Shown,
    assets: &AssetServer,
    icons: &mut IconCache,
) {
    let Some(ladder) = sim.content().talents.for_class(&shown.class) else {
        commands.entity(parent).with_child((
            label("This class has no career ladder. Classic.", 12.0),
            TextColor(DIM),
        ));
        return;
    };
    for (r, row) in ladder.rows.iter().enumerate().take(TALENT_ROWS) {
        let unlocked = row.level <= shown.level;
        let pick = shown.talents[r];
        let status = if pick > 0 {
            "promoted".to_string()
        } else if unlocked {
            "pick one".to_string()
        } else {
            format!("unlocks at level {}", row.level)
        };
        let status_color = if pick > 0 {
            GOLD
        } else if unlocked {
            ACCENT
        } else {
            LOCKED
        };
        commands.entity(parent).with_children(|p| {
            p.spawn(Node {
                column_gap: px(6),
                align_items: AlignItems::Stretch,
                ..default()
            })
            .with_children(|line| {
                line.spawn(Node {
                    width: px(LABEL_W),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    row_gap: px(2),
                    ..default()
                })
                .with_children(|l| {
                    l.spawn((
                        label(format!("L{}", row.level), 16.0),
                        TextColor(if unlocked { GOLD } else { LOCKED }),
                    ));
                    l.spawn((
                        label(&row.name, 11.0),
                        TextColor(if unlocked { Color::WHITE } else { LOCKED }),
                        Node {
                            width: percent(100),
                            ..default()
                        },
                    ));
                    l.spawn((label(status, 10.0), TextColor(status_color)));
                });
                for (k, t) in row.options.iter().enumerate() {
                    let choice = k as u8 + 1;
                    let state = if pick == choice {
                        CardState::Picked
                    } else if pick > 0 {
                        CardState::Passed
                    } else if unlocked {
                        CardState::Open
                    } else {
                        CardState::Locked
                    };
                    let bright = matches!(state, CardState::Picked | CardState::Open);
                    let (name_c, title_c, text_c) = if bright {
                        (Color::WHITE, GOLD, DIM)
                    } else {
                        (LOCKED, LOCKED, LOCKED)
                    };
                    let worn =
                        state == CardState::Picked && r + 1 == TALENT_ROWS && shown.title == t.key;
                    let title = if r + 1 == TALENT_ROWS && state == CardState::Picked {
                        if worn {
                            format!("<{}>  (worn, click to take off)", t.title)
                        } else {
                            format!("<{}>  (click to wear)", t.title)
                        }
                    } else {
                        format!("Promoted to {}", t.title)
                    };
                    line.spawn((
                        Card {
                            row: r as u8,
                            choice,
                            state,
                        },
                        Button,
                        Node {
                            width: px(CARD_W),
                            padding: UiRect::all(px(6)),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(3),
                            border: UiRect::all(px(if state == CardState::Picked { 2 } else { 1 })),
                            border_radius: BorderRadius::all(px(4)),
                            ..default()
                        },
                        BorderColor::all(PANEL_EDGE),
                        BackgroundColor(if bright {
                            BAR_BG
                        } else {
                            BAR_BG.with_alpha(0.5)
                        }),
                    ))
                    .with_children(|card| {
                        card.spawn(Node {
                            column_gap: px(6),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|h| {
                            let mut icon = ImageNode::new(icons.get(assets, &t.icon));
                            if !bright {
                                icon.color = Color::srgba(0.5, 0.5, 0.55, 0.6);
                            }
                            h.spawn((
                                icon,
                                Node {
                                    width: px(30),
                                    height: px(30),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                            ));
                            h.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                ..default()
                            })
                            .with_children(|n| {
                                n.spawn((label(&t.name, 13.0), TextColor(name_c)));
                                n.spawn((label(title, 10.0), TextColor(title_c)));
                            });
                        });
                        card.spawn((
                            label(&t.text, 11.0),
                            TextColor(text_c),
                            Node {
                                width: percent(100),
                                ..default()
                            },
                        ));
                    });
                }
            });
        });
    }
}

/// Open cards take the promotion; the picked capstone toggles its title; the review resets.
fn click_cards(
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    cards: Query<(&Card, &Interaction), Changed<Interaction>>,
    review: Query<&Interaction, (With<ReviewButton>, Changed<Interaction>)>,
) {
    let Some(me) = sim.player() else { return };
    let Some(u) = sim.unit(me) else { return };
    for (card, i) in &cards {
        if *i != Interaction::Pressed {
            continue;
        }
        match card.state {
            CardState::Open => pending.0.push(Command::PickTalent {
                row: card.row,
                choice: card.choice,
            }),
            CardState::Picked if usize::from(card.row) + 1 == TALENT_ROWS => {
                let (class, _) = sim.talents_of(me);
                let key = sim
                    .content()
                    .talents
                    .for_class(&class)
                    .and_then(|c| c.rows.get(usize::from(card.row)))
                    .and_then(|r| r.options.get(usize::from(card.choice) - 1))
                    .map(|t| t.key.clone())
                    .unwrap_or_default();
                let pick = if u.title == key { String::new() } else { key };
                pending.0.push(Command::SetTitle { achievement: pick });
            }
            _ => {}
        }
    }
    if review.iter().any(|i| *i == Interaction::Pressed) {
        pending.0.push(Command::ResetTalents);
    }
}

/// The touch menu has a Career entry, and the dock label would sit in the top row's way.
fn hide_dock_on_touch(
    touch: Res<crate::touch::TouchMode>,
    mut dock: Single<&mut Node, With<DockButton>>,
) {
    let want = if touch.active {
        Display::None
    } else {
        Display::Flex
    };
    if dock.display != want {
        dock.display = want;
    }
}

/// Toast when a row opens (level up, or a save that has rows to fill) and a pulsing dock button
/// while any row waits.
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters
fn promotion_watch(
    time: Res<Time>,
    sim: SimView,
    open: Res<CareerOpen>,
    binds: Res<Keybinds>,
    mut toast: ResMut<Toast>,
    mut root: Single<&mut Node, With<PromoToast>>,
    mut lines: Query<(&ToastLine, &mut Text), Without<DockText>>,
    mut dock: Single<&mut BorderColor, With<DockButton>>,
    mut dock_text: Single<(&mut Text, &mut TextColor), With<DockText>>,
    mut last: Local<Option<(EntityId, usize)>>,
) {
    toast.0 = (toast.0 - time.delta_secs()).max(0.0);
    let Some(me) = sim.player() else { return };
    let waiting = open_rows(&*sim, me);
    let before = last.filter(|(p, _)| *p == me).map_or(0, |(_, n)| n);
    if waiting.len() > before && !open.0 {
        toast.0 = TOAST_SECS;
        let (class, _) = sim.talents_of(me);
        let row = waiting
            .last()
            .and_then(|r| sim.content().talents.for_class(&class)?.rows.get(*r));
        let name = row.map_or_else(String::new, |r| r.name.clone());
        let key = binds.label(Action::Career);
        for (line, mut text) in &mut lines {
            match line {
                ToastLine::Row => set_text(&mut text, &name),
                ToastLine::Hint => set_text(
                    &mut text,
                    &format!("Three offers, one headcount. Press {key} to pick."),
                ),
            }
        }
    }
    *last = Some((me, waiting.len()));
    if open.0 || waiting.is_empty() {
        toast.0 = 0.0;
    }
    let want = if toast.0 > 0.0 {
        Display::Flex
    } else {
        Display::None
    };
    if root.display != want {
        root.display = want;
    }
    let key = binds.label(Action::Career);
    let (text, color, border) = if waiting.is_empty() {
        (format!("~/career [{key}]"), DIM, PANEL_EDGE)
    } else {
        let pulse = (time.elapsed_secs() * 4.0).sin() * 0.5 + 0.5;
        (
            format!("~/career [{key}]  promotion!"),
            GOLD,
            GOLD.mix(&ACCENT, pulse),
        )
    };
    set_text(&mut dock_text.0, &text);
    dock_text.1.0 = color;
    **dock = BorderColor::all(border);
}

/// "Dana was promoted to Senior Prompt Engineer!" for everyone in the group.
fn promotion_log(
    sim: SimView,
    mut events: MessageReader<CombatEvent>,
    mut log: MessageWriter<LogLineMsg>,
    mut seen: Local<HashMap<EntityId, TalentPicks>>,
) {
    for CombatEvent(ev) in events.read() {
        let Event::TalentsChanged { unit, talents } = ev else {
            continue;
        };
        let old = seen.insert(*unit, *talents).unwrap_or_default();
        let name = sim.unit(*unit).map_or("Someone", |u| u.name.as_str());
        if talents.iter().all(|k| *k == 0) && old.iter().any(|k| *k > 0) {
            log.write(LogLineMsg(
                format!("{name} survived a Performance Review. All promotions revoked."),
                LOCKED,
            ));
            continue;
        }
        let (class, _) = sim.talents_of(*unit);
        let Some(ladder) = sim.content().talents.for_class(&class) else {
            continue;
        };
        for (r, row) in ladder.rows.iter().enumerate().take(TALENT_ROWS) {
            if talents[r] > 0
                && old[r] != talents[r]
                && let Some(t) = row.options.get(usize::from(talents[r]) - 1)
            {
                log.write(LogLineMsg(
                    format!("{name} was promoted to {}! ({})", t.title, t.name),
                    GOLD,
                ));
            }
        }
    }
}

/// `WOW_TALENTS=1,2,3,0,0`: picks for the local player once the start level is set.
fn qa_talents(time: Res<Time>, mut sim: ResMut<CombatSim>, mut done: Local<bool>) {
    if *done || time.elapsed_secs() < 1.0 {
        return;
    }
    *done = true;
    let Ok(v) = std::env::var("WOW_TALENTS") else {
        return;
    };
    let mut picks = [0; TALENT_ROWS];
    for (slot, n) in picks.iter_mut().zip(v.split(',')) {
        *slot = n.trim().parse().unwrap_or(0);
    }
    if let Some(p) = sim.0.player() {
        sim.0.set_talents(p, picks);
    }
}
