//! Event-driven HUD: combat log, error banner, death prompt, loot window, XP bar.

use super::{
    ACCENT, DIM, IconCache, KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, drop_shadow, label, panel_bg,
    set_text, vgrad,
};
use crate::combat::{CombatEvent, PendingCommands, SimRead, SimReady, SimView, UiNotice};
use crate::config::*;
use crate::copy;
use bevy::prelude::*;
use combat_core::{AuraEnd, Command, EntityId, Event, ITEM_STACK_CAP, School};
use std::collections::VecDeque;

const XP_BAR_H: f32 = 14.0;
const XP_TICKS: usize = 10;
const XP_PURPLE: Color = Color::srgb(0.6, 0.25, 0.85);

#[derive(Resource, Default)]
struct CombatLog(VecDeque<(String, Color)>);

/// A client-side line for the combat log (no sim event behind it).
#[derive(Message)]
pub struct LogLineMsg(pub String, pub Color);
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
#[derive(Component)]
struct XpText;

pub struct FeedPlugin;

impl Plugin for FeedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatLog>()
            .add_message::<LogLineMsg>()
            .init_resource::<ErrorBanner>()
            .init_resource::<LootState>()
            .add_systems(Startup, spawn_feed)
            .add_systems(Update, close_loot_on_restart)
            .add_systems(
                Update,
                loot_demo
                    .in_set(SimReady)
                    .run_if(|| std::env::var_os("WOW_LOOT_DEMO").is_some()),
            )
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

/// Hidden on touch layouts, where the action bar needs the width.
#[derive(Component)]
pub struct CombatLogRoot;

fn spawn_feed(mut commands: Commands) {
    commands
        .spawn((
            CombatLogRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(12),
                bottom: px(20),
                width: px(COMBAT_LOG_W),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            drop_shadow(),
        ))
        .with_children(|root| {
            // Terminal title bar: window dots plus the command that "produces" the log.
            root.spawn((
                Node {
                    height: px(20),
                    padding: UiRect::horizontal(px(8)),
                    column_gap: px(5),
                    align_items: AlignItems::Center,
                    ..default()
                },
                vgrad(Color::srgb(0.17, 0.18, 0.23), Color::srgb(0.12, 0.13, 0.17)),
            ))
            .with_children(|bar| {
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
                        ..default()
                    },
                    label("~/prod $ tail -f combat.log", 11.0),
                    TextColor(DIM),
                ));
            });
            root.spawn((
                Node {
                    padding: UiRect::axes(px(8), px(6)),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                },
                vgrad(
                    Color::srgba(0.03, 0.04, 0.06, 0.82),
                    Color::srgba(0.02, 0.025, 0.04, 0.78),
                ),
            ))
            .with_children(|c| {
                for i in 0..COMBAT_LOG_LINES {
                    c.spawn((LogLine(i), label("", 12.0)));
                }
            });
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
                label("process exited with code 137", 24.0),
                TextColor(Color::srgb(0.9, 0.2, 0.2)),
            ));
            c.spawn((
                label("Your role has been backfilled by an AI agent.", 14.0),
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
                BorderColor::all(KEY_RIM),
                vgrad(KEY_TOP_HI, KEY_TOP),
                children![label("[ Release / Re-onboard ]", 16.0)],
            ));
        });
    commands
        .spawn((
            LootWindow,
            Node {
                position_type: PositionType::Absolute,
                right: percent(24),
                top: percent(30),
                width: px(300),
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
            GlobalZIndex(30),
        ))
        .with_children(|c| {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn((label("// loot", 14.0), TextColor(DIM)));
                h.spawn((LootClose, Button, label("[x]", 14.0)));
            });
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0),
                left: px(0),
                width: percent(100),
                height: px(XP_BAR_H),
                border: UiRect::vertical(px(1)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            vgrad(Color::srgb(0.02, 0.02, 0.04), Color::srgb(0.1, 0.09, 0.12)),
        ))
        .with_children(|c| {
            c.spawn((
                XpFill,
                Node {
                    height: percent(100),
                    width: percent(0),
                    ..default()
                },
                vgrad(
                    XP_PURPLE.mix(&Color::WHITE, 0.35),
                    XP_PURPLE.mix(&Color::BLACK, 0.45),
                ),
            ));
            // Tick marks divide the bar into tenths, like the WoW experience bar.
            for k in 1..XP_TICKS {
                c.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(100.0 * k as f32 / XP_TICKS as f32),
                        width: px(1),
                        top: px(0),
                        bottom: px(0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                ));
            }
            c.spawn(super::gloss(0.2));
            c.spawn((
                XpText,
                label("", 9.0),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                TextLayout::justify(Justify::Center),
                TextColor(Color::srgb(0.9, 0.92, 1.0)),
            ));
        });
}

fn who(sim: &dyn SimRead, id: EntityId) -> String {
    if sim.player() == Some(id) {
        "you".into()
    } else {
        sim.unit(id)
            .map_or_else(|| "something".into(), |u| u.name.clone())
    }
}

/// "You" / "CEO" at the start of a sentence.
fn subject(sim: &dyn SimRead, id: EntityId) -> String {
    let w = who(sim, id);
    if w == "you" { "You".into() } else { w }
}

fn possessive(sim: &dyn SimRead, id: EntityId) -> String {
    if sim.player() == Some(id) {
        "Your".into()
    } else {
        format!("{}'s", who(sim, id))
    }
}

const QUEST_YELLOW: Color = Color::srgb(1.0, 0.85, 0.35);

fn quest_name(sim: &dyn SimRead, id: &str) -> String {
    sim.content()
        .quest(id)
        .map_or_else(|| id.to_string(), |q| q.name.clone())
}

fn spell_name(sim: &dyn SimRead, id: combat_core::SpellId) -> String {
    sim.content()
        .spell(id)
        .map_or_else(|| "a spell".into(), |d| d.name.clone())
}

/// "Nitpick" for the player; sim data is the source of truth for the name.
fn auto_attack_name(sim: &dyn SimRead, id: EntityId) -> String {
    sim.unit(id)
        .and_then(|u| sim.content().unit_def(&u.key))
        .and_then(|d| d.auto_attack.as_ref())
        .map_or_else(|| "attack".into(), |a| a.name.clone())
}

fn school_name(s: School) -> &'static str {
    match s {
        School::Physical => "Physical",
        School::Arcane => "Arcane",
    }
}

fn aura_name(sim: &dyn SimRead, target: EntityId, key: &str) -> String {
    sim.unit(target)
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
    mut extra: MessageReader<LogLineMsg>,
    sim: SimView,
    mut log: ResMut<CombatLog>,
    mut banner: ResMut<ErrorBanner>,
    mut loot: ResMut<LootState>,
    mut xp_gains: Local<usize>,
    mut last: Local<(String, u32)>,
    // Mana ticks of the running Coffee Break, reported once as a single line.
    mut coffee_mana: Local<u32>,
) {
    let me = sim.player();
    let coffee = sim.content().channel_spell().map(|s| s.id);
    let mut push = |text: String, color: Color| {
        // Identical consecutive lines collapse into one with a repeat count.
        if last.1 > 0
            && last.0 == text
            && let Some(back) = log.0.back_mut()
        {
            last.1 += 1;
            back.0 = format!("{text} x{}", last.1);
            return;
        }
        *last = (text.clone(), 1);
        log.0.push_back((text, color));
        while log.0.len() > COMBAT_LOG_LINES {
            log.0.pop_front();
        }
    };
    for LogLineMsg(text, color) in extra.read() {
        push(text.clone(), *color);
    }
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
                    who(&*sim, *target)
                };
                let mut line = match spell {
                    Some(s) => format!(
                        "{} {} {verb} {victim} for {amount} {}.",
                        possessive(&*sim, *source),
                        spell_name(&*sim, *s),
                        school_name(*school),
                    ),
                    None if Some(*source) == me => format!(
                        "Your {} {verb} {victim} for {amount}.",
                        auto_attack_name(&*sim, *source)
                    ),
                    None => format!("{} {verb} {victim} for {amount}.", subject(&*sim, *source)),
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
                        possessive(&*sim, *source),
                        spell_name(&*sim, *s)
                    ),
                    Some(s) if Some(*source) == me => format!(
                        "{} resists your {}.",
                        who(&*sim, *target),
                        spell_name(&*sim, *s)
                    ),
                    Some(s) => format!(
                        "{} resists {} {}.",
                        who(&*sim, *target),
                        possessive(&*sim, *source),
                        spell_name(&*sim, *s)
                    ),
                    None if Some(*source) == me => format!(
                        "Your {} misses {}.",
                        auto_attack_name(&*sim, *source),
                        who(&*sim, *target)
                    ),
                    None => format!(
                        "{} attack misses {}.",
                        possessive(&*sim, *source),
                        who(&*sim, *target)
                    ),
                };
                push(line, Color::srgb(0.8, 0.8, 0.85));
            }
            Event::CastStarted { caster, spell, .. }
                if Some(*caster) == me && Some(*spell) == coffee =>
            {
                *coffee_mana = 0;
                push(copy::COFFEE_START.into(), copy::COFFEE_COLOR);
            }
            Event::ManaGain { unit, amount, .. } if Some(*unit) == me => *coffee_mana += amount,
            Event::CastSucceeded { caster, spell, .. }
                if Some(*caster) == me && Some(*spell) == coffee =>
            {
                push(copy::coffee_restored(*coffee_mana), copy::COFFEE_COLOR);
                *coffee_mana = 0;
            }
            Event::CastInterrupted { caster, spell, .. }
                if Some(*caster) == me && Some(*spell) == coffee =>
            {
                push(copy::COFFEE_INTERRUPTED.into(), Color::srgb(1.0, 0.85, 0.2));
                if *coffee_mana > 0 {
                    push(copy::coffee_restored(*coffee_mana), copy::COFFEE_COLOR);
                }
                *coffee_mana = 0;
            }
            Event::CastStarted { caster, spell, .. } if Some(*caster) != me => push(
                format!(
                    "{} begins to cast {}.",
                    subject(&*sim, *caster),
                    spell_name(&*sim, *spell)
                ),
                Color::srgb(0.85, 0.7, 1.0),
            ),
            Event::CastInterrupted {
                caster, spell, by, ..
            } => {
                let by = by.map_or(String::new(), |b| format!(" by {}", who(&*sim, b)));
                push(
                    format!(
                        "{} {} is interrupted{by}.",
                        possessive(&*sim, *caster),
                        spell_name(&*sim, *spell)
                    ),
                    Color::srgb(1.0, 0.85, 0.2),
                );
            }
            Event::CastFailed { caster, reason, .. } if Some(*caster) == me => {
                banner.text = reason.message().to_owned();
                banner.left = ERROR_TEXT_SECS;
            }
            Event::AuraApplied { target, key } => {
                let name = aura_name(&*sim, *target, key);
                let harmful = sim.content().aura(key).is_none_or(|a| a.harmful);
                let line = match (Some(*target) == me, harmful) {
                    (true, true) => format!("You are afflicted by {name}."),
                    (true, false) => format!("You gain {name}."),
                    (false, true) => format!("{} is afflicted by {name}.", who(&*sim, *target)),
                    (false, false) => format!("{} gains {name}.", who(&*sim, *target)),
                };
                push(line, Color::srgb(0.6, 0.85, 1.0));
            }
            Event::AuraRemoved {
                target,
                key,
                reason,
            } => {
                let name = aura_name(&*sim, *target, key);
                let how = match reason {
                    AuraEnd::Expired => "fades from",
                    AuraEnd::Consumed => "is consumed on",
                    AuraEnd::Depleted => "is depleted on",
                    AuraEnd::Death => "ends on",
                };
                push(
                    format!("{name} {how} {}.", who(&*sim, *target)),
                    Color::srgb(0.6, 0.7, 0.8),
                );
            }
            Event::Death { unit, .. } => push(
                if Some(*unit) == me {
                    "You die.".to_string()
                } else {
                    format!("{} dies.", who(&*sim, *unit))
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
            Event::Yell { unit, text } => {
                // "you" would read oddly in "<name> yells", so use the unit's own name.
                let (name, hostile) = sim.unit(*unit).map_or_else(
                    || ("Someone".to_string(), false),
                    |u| (u.name.clone(), u.hostile),
                );
                push(
                    format!("{name} {}: {text}", copy::speech_verb(hostile)),
                    copy::yell_color(me == Some(*unit), hostile),
                );
            }
            Event::Heal {
                source,
                target,
                spell,
                amount,
            } if *amount > 0 => {
                let by = match spell {
                    Some(s) => format!("{} {}", possessive(&*sim, *source), spell_name(&*sim, *s)),
                    None => subject(&*sim, *source),
                };
                push(
                    format!("{by} heals {} for {amount}.", who(&*sim, *target)),
                    Color::srgb(0.4, 1.0, 0.45),
                );
            }
            Event::LevelUp { unit, level } if Some(*unit) == me => {
                push(copy::level_up_line(*level), Color::srgb(1.0, 0.82, 0.3));
            }
            Event::SpellLearned { unit, spell } if Some(*unit) == me => push(
                format!("You learned {}.", spell_name(&*sim, *spell)),
                Color::srgb(1.0, 0.9, 0.5),
            ),
            Event::PhaseChanged { unit, phase } => push(
                if *phase > 1 {
                    format!("{} enters phase {phase}.", subject(&*sim, *unit))
                } else {
                    format!("{} resets to phase 1.", subject(&*sim, *unit))
                },
                Color::srgb(1.0, 0.55, 0.2),
            ),
            Event::Summoned { unit, summoner } => push(
                format!(
                    "{} summons {}.",
                    subject(&*sim, *summoner),
                    who(&*sim, *unit)
                ),
                Color::srgb(0.85, 0.7, 1.0),
            ),
            Event::Knockback { unit, .. } if Some(*unit) == me => {
                push("You are knocked back.".into(), Color::srgb(1.0, 0.6, 0.4));
            }
            Event::Teleported { unit, .. } if Some(*unit) == me => push(
                "You have been reorganised to a new location.".into(),
                Color::srgb(1.0, 0.6, 0.4),
            ),
            Event::ZoneCleared { zone } => push(
                format!("{} cleared.", crate::game::zone_label(sim.content(), zone)),
                Color::srgb(0.4, 1.0, 0.5),
            ),
            Event::Unlocked { zone } => push(
                format!(
                    "Access granted: {}.",
                    crate::game::zone_label(sim.content(), zone)
                ),
                Color::srgb(1.0, 0.82, 0.3),
            ),
            Event::QuestAccepted { quest } => push(
                format!("Ticket assigned to you: {}.", quest_name(&*sim, quest)),
                QUEST_YELLOW,
            ),
            Event::QuestReady { quest } => push(
                format!("{}: ready for review.", quest_name(&*sim, quest)),
                QUEST_YELLOW,
            ),
            Event::QuestCompleted { quest } => push(
                format!("Ticket closed: {}.", quest_name(&*sim, quest)),
                QUEST_YELLOW,
            ),
            Event::Looted { looter, items, .. } if Some(*looter) == me => {
                for item in items {
                    let Some(def) = sim.content().item(item) else {
                        continue;
                    };
                    let held = sim.inventory().iter().find(|(n, _)| n == item);
                    let capped = held.is_some_and(|(_, c)| *c > ITEM_STACK_CAP);
                    let bonus = if capped {
                        format!("stack cap {ITEM_STACK_CAP} reached")
                    } else {
                        copy::bonus_text(&def.bonus.totals(1))
                    };
                    push(
                        format!("You receive trophy: [{item}] ({bonus})"),
                        copy::BONUS_GREEN,
                    );
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
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    mut panel: Single<&mut Node, With<DeathPanel>>,
    buttons: Query<&Interaction, (With<RespawnButton>, Changed<Interaction>)>,
) {
    let dead = sim
        .player()
        .and_then(|id| sim.unit(id))
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
    sim: SimView,
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
            let def = sim.content().item(item);
            let icon = icons.get(&assets, def.map_or("unknown", |d| d.icon.as_str()));
            let bonus = def.map_or_else(String::new, |d| copy::bonus_text(&d.bonus.totals(1)));
            let desc = def.map_or_else(String::new, |d| d.description.clone());
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
                            flex_shrink: 0.0,
                            ..default()
                        }
                    ),
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            flex_shrink: 1.0,
                            flex_basis: px(0),
                            flex_grow: 1.0,
                            ..default()
                        },
                        children![
                            (label(item.clone(), 13.0), TextColor(Color::WHITE)),
                            (label(bonus, 12.0), TextColor(copy::BONUS_GREEN)),
                            (label(desc, 11.0), TextColor(DIM)),
                        ]
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

fn update_xp(
    sim: SimView,
    mut fill: Single<&mut Node, With<XpFill>>,
    mut text: Single<&mut Text, With<XpText>>,
) {
    let Some(u) = sim.player().and_then(|id| sim.unit(id)) else {
        return;
    };
    if u.xp_to_next == 0 {
        fill.width = percent(100);
        set_text(
            &mut text,
            &format!(
                "Lv {} | sprint progress: max level, backlog infinite",
                u.level
            ),
        );
        return;
    }
    let pct = (100.0 * u.xp as f32 / u.xp_to_next as f32).clamp(0.0, 100.0);
    fill.width = percent(pct);
    set_text(
        &mut text,
        &format!(
            "Lv {} | sprint progress {} / {} ({pct:.0}%)",
            u.level, u.xp, u.xp_to_next
        ),
    );
}

/// The old sim's corpse loot must not survive a restart.
fn close_loot_on_restart(
    mut restarted: MessageReader<crate::game::GameRestarted>,
    mut loot: ResMut<LootState>,
) {
    if restarted.read().count() > 0 {
        loot.items.clear();
        loot.dirty = true;
    }
}

/// QA hook (`WOW_LOOT_DEMO`): shows the loot window and the matching log lines for three sample items
/// without a kill, so the layout can be screenshotted when no fight is available. Display only; nothing is granted.
fn loot_demo(
    sim: SimView,
    mut loot: ResMut<LootState>,
    mut lines: MessageWriter<LogLineMsg>,
    mut done: Local<bool>,
) {
    if std::mem::replace(&mut *done, true) {
        return;
    }
    for name in [
        "Prompt Fragment",
        "Expired GPU Credits",
        "Unread AI Strategy Deck",
    ] {
        let Some(def) = sim.content().item(name) else {
            continue;
        };
        lines.write(LogLineMsg(
            format!(
                "You receive trophy: [{name}] ({})",
                copy::bonus_text(&def.bonus.totals(1))
            ),
            copy::BONUS_GREEN,
        ));
        loot.items.push(name.to_string());
    }
    loot.dirty = true;
}
