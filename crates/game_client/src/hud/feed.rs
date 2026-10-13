//! Event-driven HUD: combat log, error banner, death prompt, loot window, XP bar.

use super::{BAR_BG, BRONZE, GOLD, IconCache, asset_key, label, set_text};
use crate::combat::{CombatEvent, CombatSim, PendingCommands, SimReady, UiNotice};
use crate::config::*;
use crate::copy;
use bevy::prelude::*;
use combat_core::{AuraEnd, Command, EntityId, Event, School};
use std::collections::VecDeque;

const XP_PURPLE: Color = Color::srgb(0.6, 0.25, 0.85);

#[derive(Resource, Default)]
struct CombatLog(VecDeque<(String, Color)>);
#[derive(Component)]
struct LogLine(usize);

#[derive(Resource, Default)]
struct ErrorBanner {
    text: String,
    left: f32,
}
#[derive(Component)]
struct ErrorText;

#[derive(Component)]
struct DeathPanel;
#[derive(Component)]
struct RespawnButton;

#[derive(Resource, Default)]
struct LootState {
    items: Vec<String>,
    dirty: bool,
}
#[derive(Component)]
struct LootWindow;
#[derive(Component)]
struct LootRow;
#[derive(Component)]
struct LootClose;

#[derive(Component)]
struct XpFill;

pub struct FeedPlugin;

impl Plugin for FeedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatLog>()
            .init_resource::<ErrorBanner>()
            .init_resource::<LootState>()
            .add_systems(Startup, spawn_feed)
            .add_systems(
                Update,
                (
                    collect_events,
                    update_log,
                    update_error,
                    update_death,
                    update_loot,
                    update_xp,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn spawn_feed(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(12),
                bottom: px(20),
                width: px(COMBAT_LOG_W),
                padding: UiRect::all(px(6)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(BRONZE),
            BackgroundColor(Color::srgba(0.02, 0.02, 0.05, 0.6)),
        ))
        .with_children(|c| {
            for i in 0..COMBAT_LOG_LINES {
                c.spawn((LogLine(i), label("", 13.0)));
            }
        });
    commands.spawn((
        ErrorText,
        Node {
            position_type: PositionType::Absolute,
            top: percent(16),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        },
        label("", 22.0),
        TextLayout::justify(Justify::Center),
        TextColor(Color::srgba(1.0, 0.15, 0.1, 0.0)),
    ));
    commands
        .spawn((
            DeathPanel,
            Node {
                position_type: PositionType::Absolute,
                top: percent(30),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(12),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(30),
        ))
        .with_children(|c| {
            c.spawn((
                label("You are dead.", 30.0),
                TextColor(Color::srgb(0.9, 0.2, 0.2)),
            ));
            c.spawn((
                label("Your role has been backfilled by an AI agent.", 16.0),
                TextColor(Color::srgb(0.75, 0.7, 0.7)),
            ));
            c.spawn((
                RespawnButton,
                Button,
                Node {
                    padding: UiRect::axes(px(22), px(10)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BorderColor::all(GOLD),
                BackgroundColor(Color::srgb(0.25, 0.1, 0.1)),
                children![label("Release / Re-onboard", 20.0)],
            ));
        });
    commands
        .spawn((
            LootWindow,
            Node {
                position_type: PositionType::Absolute,
                right: percent(24),
                top: percent(30),
                width: px(230),
                padding: UiRect::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(8)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(GOLD),
            BackgroundColor(super::PANEL),
            GlobalZIndex(30),
        ))
        .with_children(|c| {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn(label("Loot", 16.0));
                h.spawn((LootClose, Button, label("X", 16.0)));
            });
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0),
                left: px(0),
                width: percent(100),
                height: px(6),
                ..default()
            },
            BackgroundColor(BAR_BG),
        ))
        .with_children(|c| {
            c.spawn((
                XpFill,
                Node {
                    height: percent(100),
                    width: percent(0),
                    ..default()
                },
                BackgroundColor(XP_PURPLE),
            ));
        });
}

fn who(sim: &CombatSim, id: EntityId) -> String {
    if sim.0.player() == Some(id) {
        "you".into()
    } else {
        sim.0
            .unit(id)
            .map_or_else(|| "something".into(), |u| u.name.clone())
    }
}

/// "You" / "CEO" at the start of a sentence.
fn subject(sim: &CombatSim, id: EntityId) -> String {
    let w = who(sim, id);
    if w == "you" { "You".into() } else { w }
}

fn possessive(sim: &CombatSim, id: EntityId) -> String {
    if sim.0.player() == Some(id) {
        "Your".into()
    } else {
        format!("{}'s", who(sim, id))
    }
}

fn spell_name(sim: &CombatSim, id: combat_core::SpellId) -> String {
    sim.0
        .content()
        .spell(id)
        .map_or_else(|| "a spell".into(), |d| d.name.clone())
}

/// "Nitpick" for the player; sim data is the source of truth for the name.
fn auto_attack_name(sim: &CombatSim, id: EntityId) -> String {
    sim.0
        .unit(id)
        .and_then(|u| sim.0.content().unit_def(&u.key))
        .and_then(|d| d.auto_attack.as_ref())
        .map_or_else(|| "attack".into(), |a| a.name.clone())
}

fn school_name(s: School) -> &'static str {
    match s {
        School::Physical => "Physical",
        School::Arcane => "Arcane",
    }
}

fn aura_name(sim: &CombatSim, target: EntityId, key: &str) -> String {
    sim.0
        .unit(target)
        .and_then(|u| {
            u.auras
                .iter()
                .find(|a| a.key == key)
                .map(|a| a.name.clone())
        })
        .unwrap_or_else(|| {
            let mut s = key.replace('_', " ");
            if let Some(c) = s.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            s
        })
}

fn collect_events(
    mut events: MessageReader<CombatEvent>,
    mut notices: MessageReader<UiNotice>,
    sim: Res<CombatSim>,
    mut log: ResMut<CombatLog>,
    mut banner: ResMut<ErrorBanner>,
    mut loot: ResMut<LootState>,
    mut xp_gains: Local<usize>,
) {
    let me = sim.0.player();
    let mut push = |text: String, color: Color| {
        log.0.push_back((text, color));
        while log.0.len() > COMBAT_LOG_LINES {
            log.0.pop_front();
        }
    };
    for n in notices.read() {
        banner.text.clone_from(&n.0);
        banner.left = ERROR_TEXT_SECS;
    }
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::Damage {
                source,
                target,
                spell,
                amount,
                absorbed,
                crit,
                combo,
                school,
            } => {
                let taken = Some(*target) == me;
                let verb = if *crit { "crits" } else { "hits" };
                let victim = if taken {
                    "you".to_string()
                } else {
                    who(&sim, *target)
                };
                let mut line = match spell {
                    Some(s) => format!(
                        "{} {} {verb} {victim} for {amount} {}.",
                        possessive(&sim, *source),
                        spell_name(&sim, *s),
                        school_name(*school),
                    ),
                    None if Some(*source) == me => format!(
                        "Your {} {verb} {victim} for {amount}.",
                        auto_attack_name(&sim, *source)
                    ),
                    None => format!("{} {verb} {victim} for {amount}.", subject(&sim, *source)),
                };
                if *absorbed > 0 {
                    line.push_str(&format!(" ({absorbed} absorbed)"));
                }
                if *combo {
                    line.push_str(" Combo!");
                }
                let color = if taken {
                    Color::srgb(1.0, 0.4, 0.35)
                } else if *crit {
                    Color::srgb(1.0, 0.65, 0.2)
                } else {
                    Color::srgb(1.0, 0.95, 0.7)
                };
                push(line, color);
            }
            Event::Miss {
                source,
                target,
                spell,
            } => {
                let line = match spell {
                    Some(s) if Some(*target) == me => format!(
                        "You resist {} {}.",
                        possessive(&sim, *source),
                        spell_name(&sim, *s)
                    ),
                    Some(s) if Some(*source) == me => format!(
                        "{} resists your {}.",
                        who(&sim, *target),
                        spell_name(&sim, *s)
                    ),
                    Some(s) => format!(
                        "{} resists {} {}.",
                        who(&sim, *target),
                        possessive(&sim, *source),
                        spell_name(&sim, *s)
                    ),
                    None if Some(*source) == me => format!(
                        "Your {} misses {}.",
                        auto_attack_name(&sim, *source),
                        who(&sim, *target)
                    ),
                    None => format!(
                        "{} attack misses {}.",
                        possessive(&sim, *source),
                        who(&sim, *target)
                    ),
                };
                push(line, Color::srgb(0.8, 0.8, 0.85));
            }
            Event::CastStarted { caster, spell, .. } if Some(*caster) != me => push(
                format!(
                    "{} begins to cast {}.",
                    subject(&sim, *caster),
                    spell_name(&sim, *spell)
                ),
                Color::srgb(0.85, 0.7, 1.0),
            ),
            Event::CastInterrupted {
                caster, spell, by, ..
            } => {
                let by = by.map_or(String::new(), |b| format!(" by {}", who(&sim, b)));
                push(
                    format!(
                        "{} {} is interrupted{by}.",
                        possessive(&sim, *caster),
                        spell_name(&sim, *spell)
                    ),
                    Color::srgb(1.0, 0.85, 0.2),
                );
            }
            Event::CastFailed { caster, reason, .. } if Some(*caster) == me => {
                banner.text = reason.message().to_owned();
                banner.left = ERROR_TEXT_SECS;
            }
            Event::AuraApplied { target, key } => {
                let name = aura_name(&sim, *target, key);
                let line = if Some(*target) == me {
                    format!("You gain {name}.")
                } else {
                    format!("{} is afflicted by {name}.", who(&sim, *target))
                };
                push(line, Color::srgb(0.6, 0.85, 1.0));
            }
            Event::AuraRemoved {
                target,
                key,
                reason,
            } => {
                let name = aura_name(&sim, *target, key);
                let how = match reason {
                    AuraEnd::Expired => "fades from",
                    AuraEnd::Consumed => "is consumed on",
                    AuraEnd::Depleted => "is depleted on",
                    AuraEnd::Death => "ends on",
                };
                push(
                    format!("{name} {how} {}.", who(&sim, *target)),
                    Color::srgb(0.6, 0.7, 0.8),
                );
            }
            Event::Death { unit, .. } => push(
                if Some(*unit) == me {
                    "You die.".to_string()
                } else {
                    format!("{} dies.", who(&sim, *unit))
                },
                Color::srgb(0.9, 0.9, 0.9),
            ),
            Event::XpGained { unit, amount } if Some(*unit) == me => {
                push(
                    format!("You gain {amount} experience."),
                    Color::srgb(0.75, 0.45, 1.0),
                );
                push(copy::xp_quip(*xp_gains).into(), Color::srgb(0.5, 0.4, 0.6));
                *xp_gains += 1;
            }
            Event::Yell { unit, text } => push(
                format!("{} yells: {text}", who(&sim, *unit)),
                copy::YELL_RED,
            ),
            Event::Looted { looter, items, .. } if Some(*looter) == me => {
                for item in items {
                    push(format!("You loot [{item}]."), Color::srgb(0.3, 0.9, 0.4));
                }
                if items.is_empty() {
                    push(
                        "You find nothing to loot. The AI already expensed it.".into(),
                        Color::srgb(0.7, 0.7, 0.7),
                    );
                } else {
                    loot.items.clone_from(items);
                    loot.dirty = true;
                }
            }
            _ => {}
        }
    }
}

fn update_log(log: Res<CombatLog>, mut lines: Query<(&LogLine, &mut Text, &mut TextColor)>) {
    if !log.is_changed() {
        return;
    }
    let skip = COMBAT_LOG_LINES - log.0.len();
    for (LogLine(i), mut text, mut color) in &mut lines {
        match i.checked_sub(skip).and_then(|k| log.0.get(k)) {
            Some((s, c)) => {
                set_text(&mut text, s);
                color.0 = *c;
            }
            None => set_text(&mut text, ""),
        }
    }
}

fn update_error(
    time: Res<Time>,
    mut banner: ResMut<ErrorBanner>,
    mut text: Single<(&mut Text, &mut TextColor), With<ErrorText>>,
) {
    banner.left = (banner.left - time.delta_secs()).max(0.0);
    let (t, color) = &mut *text;
    set_text(t, &banner.text);
    color.0 = Color::srgba(1.0, 0.18, 0.12, (banner.left / ERROR_TEXT_SECS).min(1.0));
}

fn update_death(
    sim: Res<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    mut panel: Single<&mut Node, With<DeathPanel>>,
    buttons: Query<&Interaction, (With<RespawnButton>, Changed<Interaction>)>,
) {
    let dead = sim
        .0
        .player()
        .and_then(|id| sim.0.unit(id))
        .is_some_and(|u| !u.alive);
    panel.display = if dead { Display::Flex } else { Display::None };
    if dead && buttons.iter().any(|i| *i == Interaction::Pressed) {
        pending.0.push(Command::RespawnPlayer);
    }
}

fn update_loot(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut loot: ResMut<LootState>,
    mut window: Single<(Entity, &mut Node), With<LootWindow>>,
    rows: Query<Entity, With<LootRow>>,
    clicks: Query<&Interaction, (Or<(With<LootRow>, With<LootClose>)>, Changed<Interaction>)>,
) {
    let (window_entity, node) = &mut *window;
    if loot.dirty {
        loot.dirty = false;
        for e in &rows {
            commands.entity(e).despawn();
        }
        for item in &loot.items {
            let icon = icons.get(&assets, &asset_key(item));
            commands.entity(*window_entity).with_child((
                LootRow,
                Button,
                Node {
                    column_gap: px(8),
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    (
                        ImageNode::new(icon),
                        Node {
                            width: px(36),
                            height: px(36),
                            ..default()
                        }
                    ),
                    (
                        label(item.clone(), 15.0),
                        TextColor(Color::srgb(0.2, 0.85, 0.3))
                    ),
                ],
            ));
        }
        node.display = if loot.items.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
    }
    if clicks.iter().any(|i| *i == Interaction::Pressed) {
        loot.items.clear();
        node.display = Display::None;
        for e in &rows {
            commands.entity(e).despawn();
        }
    }
}

fn update_xp(sim: Res<CombatSim>, mut fill: Single<&mut Node, With<XpFill>>) {
    let xp = sim
        .0
        .player()
        .and_then(|id| sim.0.unit(id))
        .map_or(0, |u| u.xp);
    let max = sim.0.content().config.xp_per_level.max(1);
    fill.width = percent((100.0 * xp as f32 / max as f32).clamp(0.0, 100.0));
}
