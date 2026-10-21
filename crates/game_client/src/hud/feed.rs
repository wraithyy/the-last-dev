//! Event-driven HUD: combat log, error banner, death prompt, XP bar (the loot window is `loot.rs`).

use super::logs::{ChatKind, ChatLog, CombatLog, LogBuf, classify_yell};
use super::loot::Shown;
use super::{KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, label, put, set_text, vgrad};
use crate::combat::{CombatEvent, PendingCommands, SimRead, SimReady, SimView, UiNotice};
use crate::config::*;
use crate::copy;
use crate::economy::goods;
use bevy::prelude::*;
use combat_core::{
    AuraEnd, BurnoutCause, Command, EntityId, Event, ITEM_STACK_CAP, ItemError, QuipKind, School,
};

const XP_BAR_H: f32 = 14.0;
const XP_TICKS: usize = 10;
const XP_PURPLE: Color = Color::srgb(0.6, 0.25, 0.85);

/// A client-side system notice for the chat window (no sim event behind it).
#[derive(Message)]
pub struct LogLineMsg(pub String, pub Color);

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
#[derive(Component)]
struct DeathRoastText;

#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct XpText;

pub struct FeedPlugin;

impl Plugin for FeedPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(super::loot::LootPlugin)
            .add_message::<LogLineMsg>()
            .init_resource::<ErrorBanner>()
            .add_systems(Startup, spawn_feed)
            .add_systems(
                Update,
                (collect_events, update_error, update_death, update_xp)
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn spawn_feed(mut commands: Commands) {
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
                DeathRoastText,
                label("", 14.0),
                TextColor(Color::srgb(0.9, 0.62, 0.3)),
                TextLayout::justify(Justify::Center),
                Node {
                    max_width: px(560),
                    ..default()
                },
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

/// Where a sim event is reported: damage, heals, auras and casts are combat; progress and speech are chat.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sink {
    Chat,
    Combat,
}

pub fn event_sink(ev: &Event) -> Sink {
    match ev {
        Event::Yell { .. }
        | Event::XpGained { .. }
        | Event::LevelUp { .. }
        | Event::SpellLearned { .. }
        | Event::Resurrected { .. }
        | Event::ZoneCleared { .. }
        | Event::Unlocked { .. }
        | Event::QuestAccepted { .. }
        | Event::QuestReady { .. }
        | Event::QuestCompleted { .. }
        | Event::QuestFailed { .. }
        | Event::Looted { .. }
        | Event::GearChanged { .. }
        | Event::RankProgress { .. }
        | Event::RankUp { .. }
        | Event::BurnoutQuip { .. } => Sink::Chat,
        _ => Sink::Combat,
    }
}

/// Routes formatted lines of one event into the chat or the combat buffer.
struct Out<'a> {
    chat: &'a mut LogBuf,
    combat: &'a mut LogBuf,
    sink: Sink,
}

impl Out<'_> {
    fn push(&mut self, text: String, color: Color) {
        match self.sink {
            Sink::Chat => self.chat.push("", color, &text, color),
            Sink::Combat => self.combat.push("", color, &text, color),
        }
    }

    fn say(&mut self, kind: ChatKind, name: &str, text: &str) {
        let (head, head_color, body_color) = kind.style();
        let head = format!("{name}{head}");
        self.chat.push(&head, head_color, text, body_color);
    }
}

fn collect_events(
    mut events: MessageReader<CombatEvent>,
    mut notices: MessageReader<UiNotice>,
    mut extra: MessageReader<LogLineMsg>,
    sim: SimView,
    mut chat: ResMut<ChatLog>,
    mut combat: ResMut<CombatLog>,
    mut banner: ResMut<ErrorBanner>,
    mut loot: ResMut<super::loot::LootState>,
    mut toasts: MessageWriter<super::loot::LootToast>,
    settings: Res<crate::settings::Settings>,
    mut roasts: ResMut<crate::roasts::Roasts>,
    mut best_gear: Local<Option<u64>>,
    mut xp_gains: Local<usize>,
    // Mana ticks of the running Coffee Break, reported once as a single line.
    mut coffee_mana: Local<u32>,
    mut therapy: Local<usize>,
) {
    let me = sim.player();
    let coffee = sim.content().channel_spell().map(|s| s.id);
    let mut out = Out {
        chat: &mut chat.0,
        combat: &mut combat.0,
        sink: Sink::Chat,
    };
    for LogLineMsg(text, color) in extra.read() {
        out.push(text.clone(), *color);
    }
    for n in notices.read() {
        banner.text.clone_from(&n.0);
        banner.left = ERROR_TEXT_SECS;
    }
    let batch: Vec<&Event> = events.read().map(|CombatEvent(ev)| ev).collect();
    // A dash (e.g. Hot Reload) moves you via Teleported in the same batch as your own cast;
    // only an unexplained move deserves the "reorganised" line.
    let own_cast = batch
        .iter()
        .any(|e| matches!(e, Event::CastSucceeded { caster, .. } if Some(*caster) == me));
    // Several pieces equipped in one tick brag once, for the final outfit.
    let last_gear = batch
        .iter()
        .rposition(|e| matches!(e, Event::GearChanged { unit, .. } if Some(*unit) == me));
    for (k, ev) in batch.into_iter().enumerate() {
        out.sink = event_sink(ev);
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
                out.push(line, color);
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
                out.push(line, Color::srgb(0.8, 0.8, 0.85));
            }
            Event::CastStarted { caster, spell, .. }
                if Some(*caster) == me && Some(*spell) == coffee =>
            {
                *coffee_mana = 0;
                out.push(copy::COFFEE_START.into(), copy::COFFEE_COLOR);
            }
            Event::ManaGain { unit, amount, .. } if Some(*unit) == me => *coffee_mana += amount,
            Event::CastSucceeded { caster, spell, .. }
                if Some(*caster) == me && Some(*spell) == coffee =>
            {
                out.push(copy::coffee_restored(*coffee_mana), copy::COFFEE_COLOR);
                *coffee_mana = 0;
            }
            Event::CastInterrupted { caster, spell, .. }
                if Some(*caster) == me && Some(*spell) == coffee =>
            {
                out.push(copy::COFFEE_INTERRUPTED.into(), Color::srgb(1.0, 0.85, 0.2));
                if *coffee_mana > 0 {
                    out.push(copy::coffee_restored(*coffee_mana), copy::COFFEE_COLOR);
                }
                *coffee_mana = 0;
            }
            Event::CastStarted { caster, spell, .. } if Some(*caster) != me => out.push(
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
                out.push(
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
            Event::ItemError { reason } => {
                banner.text = if *reason == ItemError::NotEnoughSp {
                    roasts.declined().to_owned()
                } else {
                    reason.message().to_owned()
                };
                banner.left = ERROR_TEXT_SECS;
            }
            Event::Proc { unit, item } => {
                let whose = if Some(*unit) == me {
                    "Your".to_string()
                } else {
                    possessive(&*sim, *unit)
                };
                out.push(format!("{whose} [{item}] kicks in."), goods::PERK_GOLD);
            }
            Event::GearChanged { unit, equipped } if Some(*unit) == me && Some(k) == last_gear => {
                let score = goods::gear_score(sim.content(), equipped);
                match *best_gear {
                    Some(best) if score > best => out.push(
                        format!(
                            "New personal best: gear score {score} (was {best}). A LinkedIn post about your growth journey has been drafted for you."
                        ),
                        goods::UPGRADE_GREEN,
                    ),
                    _ => {}
                }
                *best_gear = Some(best_gear.unwrap_or(0).max(score));
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
                out.push(line, Color::srgb(0.6, 0.85, 1.0));
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
                out.push(
                    format!("{name} {how} {}.", who(&*sim, *target)),
                    Color::srgb(0.6, 0.7, 0.8),
                );
            }
            Event::Death { unit, .. } => out.push(
                if Some(*unit) == me {
                    "You die.".to_string()
                } else {
                    format!("{} dies.", who(&*sim, *unit))
                },
                Color::srgb(0.9, 0.9, 0.9),
            ),
            Event::XpGained { unit, amount } if Some(*unit) == me => {
                out.push(
                    format!("You gain {amount} experience."),
                    Color::srgb(0.75, 0.45, 1.0),
                );
                out.push(copy::xp_quip(*xp_gains).into(), Color::srgb(0.5, 0.4, 0.6));
                *xp_gains += 1;
            }
            Event::Yell { unit, text } => {
                // Unit names, not "you": "you yells" would read oddly, and chat lines name the speaker.
                let (name, hostile) = sim.unit(*unit).map_or_else(
                    || ("Someone".to_string(), false),
                    |u| (u.name.clone(), u.hostile),
                );
                let member = me == Some(*unit) || sim.players().contains(unit);
                out.say(classify_yell(member, hostile), &name, text);
            }
            Event::Resurrected { unit, by, .. } => out.push(
                if Some(*unit) == me {
                    format!(
                        "{} turned you off and on again. You feel... rebooted.",
                        subject(&*sim, *by)
                    )
                } else {
                    format!(
                        "{} turned {} off and on again.",
                        subject(&*sim, *by),
                        who(&*sim, *unit)
                    )
                },
                Color::srgb(0.55, 0.95, 0.7),
            ),
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
                out.push(
                    format!("{by} heals {} for {amount}.", who(&*sim, *target)),
                    Color::srgb(0.4, 1.0, 0.45),
                );
            }
            Event::LevelUp { unit, level } if Some(*unit) == me => {
                out.push(copy::level_up_line(*level), Color::srgb(1.0, 0.82, 0.3));
            }
            Event::SpellLearned { unit, spell } if Some(*unit) == me => out.push(
                format!("You learned {}.", spell_name(&*sim, *spell)),
                Color::srgb(1.0, 0.9, 0.5),
            ),
            Event::PhaseChanged { unit, phase } => out.push(
                if *phase > 1 {
                    format!("{} enters phase {phase}.", subject(&*sim, *unit))
                } else {
                    format!("{} resets to phase 1.", subject(&*sim, *unit))
                },
                Color::srgb(1.0, 0.55, 0.2),
            ),
            Event::Summoned { unit, summoner } => out.push(
                format!(
                    "{} summons {}.",
                    subject(&*sim, *summoner),
                    who(&*sim, *unit)
                ),
                Color::srgb(0.85, 0.7, 1.0),
            ),
            Event::Knockback { unit, .. } if Some(*unit) == me => {
                out.push("You are knocked back.".into(), Color::srgb(1.0, 0.6, 0.4));
            }
            Event::Teleported { unit, .. } if Some(*unit) == me && !own_cast => out.push(
                "You have been reorganised to a new location.".into(),
                Color::srgb(1.0, 0.6, 0.4),
            ),
            Event::ZoneCleared { zone } => out.push(
                format!("{} cleared.", crate::game::zone_label(sim.content(), zone)),
                Color::srgb(0.4, 1.0, 0.5),
            ),
            Event::Unlocked { zone } => out.push(
                format!(
                    "Access granted: {}.",
                    crate::game::zone_label(sim.content(), zone)
                ),
                Color::srgb(1.0, 0.82, 0.3),
            ),
            Event::QuestAccepted { quest } => out.push(
                format!("Ticket assigned to you: {}.", quest_name(&*sim, quest)),
                QUEST_YELLOW,
            ),
            Event::QuestReady { quest } => out.push(
                format!("{}: ready for review.", quest_name(&*sim, quest)),
                QUEST_YELLOW,
            ),
            Event::QuestCompleted { quest } => out.push(
                format!("Ticket closed: {}.", quest_name(&*sim, quest)),
                QUEST_YELLOW,
            ),
            Event::QuestFailed { quest } => out.push(
                format!(
                    "Ticket failed: {}. It can be taken again.",
                    quest_name(&*sim, quest)
                ),
                QUEST_YELLOW,
            ),
            Event::Looted {
                looter,
                corpse,
                items,
                sp,
            } if Some(*looter) == me => {
                let content = sim.content();
                let me_state = me.and_then(|p| sim.unit(p));
                let equipped = me_state
                    .map(|u| goods::slots_of(&u.equipped))
                    .unwrap_or_default();
                let role = me_state
                    .and_then(|u| content.class_of_unit(&u.key))
                    .map(|c| c.role);
                if best_gear.is_none() {
                    *best_gear = Some(goods::gear_score(content, &equipped));
                }
                for item in items {
                    if let Some(g) = content.economy.gear(item) {
                        let tag = if goods::is_upgrade(content, item, &equipped, role) {
                            "  UPGRADE! Press I to wear it."
                        } else {
                            ""
                        };
                        out.push(
                            format!("You receive loot: [{item}] ({}){tag}", g.rarity.label()),
                            goods::rarity_color(g.rarity),
                        );
                        continue;
                    }
                    if content.economy.goods(item).is_some() {
                        let ticket = content
                            .economy
                            .junk
                            .iter()
                            .any(|j| j.ticket && j.name == *item);
                        let tip = if ticket {
                            " Close it at the Scrum board or the Company Calendar for Kudos."
                        } else {
                            ""
                        };
                        out.push(
                            format!("You receive: [{item}]{tip}"),
                            goods::name_color(content, item),
                        );
                        continue;
                    }
                    let Some(def) = content.item(item) else {
                        continue;
                    };
                    let held = sim.inventory().iter().find(|(n, _)| n == item);
                    let capped = held.is_some_and(|(_, c)| *c > ITEM_STACK_CAP);
                    let bonus = if capped {
                        format!("stack cap {ITEM_STACK_CAP} reached")
                    } else {
                        copy::bonus_text(&def.bonus.totals(1))
                    };
                    out.push(
                        format!("You receive trophy: [{item}] ({bonus})"),
                        copy::BONUS_GREEN,
                    );
                }
                if *sp > 0 {
                    out.push(format!("You receive {sp} Kudos."), goods::SP_GOLD);
                }
                let left = sim.unit(*corpse).is_some_and(|u| u.lootable);
                match loot.on_looted(*corpse, items, *sp, left, settings.auto_loot) {
                    Shown::Nothing => out.push(
                        "You find nothing to loot. The AI already expensed it.".into(),
                        Color::srgb(0.7, 0.7, 0.7),
                    ),
                    Shown::Feed => {
                        for item in items {
                            toasts.write(super::loot::LootToast(
                                format!("+ [{item}]"),
                                goods::name_color(content, item),
                            ));
                        }
                    }
                    Shown::Window => {}
                }
            }
            Event::RankProgress { unit, gained, .. } if Some(*unit) == me && *gained > 0 => {
                out.push(
                    format!("You gain {gained} promotion experience."),
                    Color::srgb(0.75, 0.45, 1.0),
                );
            }
            Event::RankUp { unit, rank } => {
                let title = sim
                    .content()
                    .endgame
                    .rank_name(*rank)
                    .unwrap_or("something")
                    .to_string();
                let line = if Some(*unit) == me {
                    format!("You have been promoted to {title}. Same desk, more meetings.")
                } else {
                    format!("{} has been promoted to {title}.", who(&*sim, *unit))
                };
                out.push(line, Color::srgb(1.0, 0.82, 0.3));
            }
            Event::Reputation {
                unit,
                dept,
                total,
                gained,
            } if Some(*unit) == me => {
                let eg = &sim.content().endgame;
                let name = eg
                    .departments
                    .get(usize::from(*dept))
                    .map_or("A department", |d| d.name.as_str());
                let tier = eg.tier_of(*total);
                let tier_name = eg.rep_tiers.get(tier).map_or("", |t| t.name.as_str());
                if tier > eg.tier_of(total - gained) {
                    out.sink = Sink::Chat;
                    out.push(
                        format!("{name} now considers you: {tier_name}."),
                        Color::srgb(0.4, 0.85, 1.0),
                    );
                } else {
                    out.push(
                        format!("{name} standing +{gained} ({tier_name})."),
                        Color::srgb(0.4, 0.75, 0.9),
                    );
                }
            }
            Event::Burnout {
                unit, delta, cause, ..
            } if Some(*unit) == me => {
                let pct = copy::burnout_pct(*delta);
                let line = match cause {
                    BurnoutCause::Kill => {
                        let lines = &sim.content().endgame.burnout.therapy;
                        let t = lines
                            .get(*therapy % lines.len().max(1))
                            .map_or("Therapeutic.", String::as_str)
                            .to_string();
                        *therapy += 1;
                        Some(format!("{t} (burnout {pct})"))
                    }
                    BurnoutCause::Holiday => {
                        out.sink = Sink::Chat;
                        Some(format!(
                            "Welcome back. Time off did its job: burnout {pct}. HR calls it 'recharging'."
                        ))
                    }
                    BurnoutCause::Death => Some(format!(
                        "Burnout {pct}. Dying at work still counts as working late."
                    )),
                    BurnoutCause::Meeting => Some(format!("Burnout {pct}: another meeting.")),
                    BurnoutCause::Minigame => {
                        Some(format!("Burnout {pct}: mandatory fun is still mandatory."))
                    }
                    BurnoutCause::CoffeeBreak => Some(format!("Burnout {pct}: coffee.")),
                    BurnoutCause::Quest if *delta > 0 => Some(format!("Burnout {pct}: paperwork.")),
                    BurnoutCause::Quest => Some(format!("Burnout {pct}: that felt honest.")),
                    _ => None,
                };
                if let Some(l) = line {
                    let color = if *delta < 0 {
                        Color::srgb(0.35, 1.0, 0.4)
                    } else {
                        Color::srgb(1.0, 0.6, 0.35)
                    };
                    out.push(l, color);
                }
            }
            Event::BurnoutQuip { unit, kind, line } => {
                let def = &sim.content().endgame.burnout;
                let Some(text) = def.quip_line(*kind, *line) else {
                    continue;
                };
                let name = sim.unit(*unit).map_or("Someone", |u| u.name.as_str());
                out.sink = Sink::Chat;
                match kind {
                    QuipKind::ReplyAll => {
                        out.chat
                            .push(&format!("[Reply All] {name}: "), REPLY_ALL, text, REPLY_ALL);
                    }
                    QuipKind::Mutter => {
                        let (_, head, body) = ChatKind::Emote.style();
                        out.chat
                            .push(&format!("{name} mutters: "), head, text, body);
                    }
                    QuipKind::Yawn => {
                        out.push(format!("{name} {text}"), Color::srgb(0.6, 0.6, 0.66))
                    }
                    QuipKind::Misfire => out.push(
                        format!("Your spell fizzles. {name}: \"{text}\""),
                        Color::srgb(1.0, 0.6, 0.35),
                    ),
                }
            }
            _ => {}
        }
    }
}

/// Reply-all mails stand out like they do in a real inbox.
const REPLY_ALL: Color = Color::srgb(1.0, 0.78, 0.35);

fn update_error(
    time: Res<Time>,
    mut banner: ResMut<ErrorBanner>,
    mut text: Single<(&mut Text, &mut TextColor), With<ErrorText>>,
) {
    banner.left = (banner.left - time.delta_secs()).max(0.0);
    let (t, color) = &mut *text;
    set_text(t, &banner.text);
    put!(
        color.0,
        Color::srgba(1.0, 0.18, 0.12, (banner.left / ERROR_TEXT_SECS).min(1.0))
    );
}

fn update_death(
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    mut panel: Single<&mut Node, With<DeathPanel>>,
    mut roast: Single<&mut Text, With<DeathRoastText>>,
    death: Res<crate::roasts::DeathRoast>,
    buttons: Query<&Interaction, (With<RespawnButton>, Changed<Interaction>)>,
) {
    let dead = sim
        .player()
        .and_then(|id| sim.unit(id))
        .is_some_and(|u| !u.alive);
    put!(
        panel.display,
        if dead { Display::Flex } else { Display::None }
    );
    set_text(&mut roast, &death.text);
    if dead && buttons.iter().any(|i| *i == Interaction::Pressed) {
        pending.0.push(Command::RespawnPlayer);
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
        // Past the cap the bar fills towards the next Promotion rank.
        let eg = &sim.content().endgame;
        let (rank, xp) = (u.career.rank, u.career.rank_xp);
        let need = eg.rank_xp_to_next(rank);
        let title = eg.rank_name(rank).unwrap_or("No promotion yet");
        let (width, line) = match (need, eg.ranks.is_empty()) {
            (_, true) => (
                100.0,
                format!("Lv {} | maxed out, still not promoted", u.level),
            ),
            (0, false) => (
                100.0,
                format!(
                    "Lv {} | {title} | out of titles. HR is drafting one.",
                    u.level
                ),
            ),
            (need, false) => {
                let pct = (100.0 * xp as f32 / need as f32).clamp(0.0, 100.0);
                (
                    pct,
                    format!(
                        "Lv {} | {title} | promotion {xp} / {need} XP ({pct:.0}%)",
                        u.level
                    ),
                )
            }
        };
        put!(fill.width, percent(width));
        set_text(&mut text, &line);
        return;
    }
    let pct = (100.0 * u.xp as f32 / u.xp_to_next as f32).clamp(0.0, 100.0);
    put!(fill.width, percent(pct));
    set_text(
        &mut text,
        &format!(
            "Lv {} | career progress {} / {} XP ({pct:.0}%)",
            u.level, u.xp, u.xp_to_next
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_and_progress_go_to_chat_while_fighting_stays_in_the_combat_log() {
        let chat = [
            Event::Yell {
                unit: 1,
                text: "hi".into(),
            },
            Event::XpGained { unit: 1, amount: 5 },
            Event::QuestCompleted { quest: "q".into() },
        ];
        for e in &chat {
            assert_eq!(event_sink(e), Sink::Chat, "{e:?}");
        }
        let combat = [
            Event::Heal {
                source: 1,
                target: 1,
                spell: None,
                amount: 3,
            },
            Event::Death {
                unit: 2,
                killer: None,
            },
        ];
        for e in &combat {
            assert_eq!(event_sink(e), Sink::Combat, "{e:?}");
        }
    }
}
