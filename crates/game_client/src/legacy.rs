//! Legacy Mode presentation. The choice lives where the trip starts: at the cap, walking up to
//! a door or elevator whose floor has a Legacy variant shows a small "Normal | Legacy" switch
//! above the action bar (click or tap; the group lead decides, everyone else sees the call).
//! Walking through then loads the floor in the chosen mode. A Legacy floor announces its affix
//! with a banner and the radar footer keeps it in view (`hud::minimap`); refusals become
//! notices. Rules live in `combat_core::legacy`; this side only sends `Command::SetLegacy`.
//!
//! QA: `WOW_LEGACY=1` switches the offline group to Legacy before the first zone loads (pair
//! with `WOW_LEVEL=10` and `WOW_ZONE`); `WOW_LEGACY_LOCKS=boss,boss` sets this week's lockout.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, CombatSim, PendingCommands, SimReady, SimView, UiNotice};
use crate::game::{GameFlow, zone_label};
use crate::hud::{ACCENT, Banner, DIM, KEY_RIM, drop_shadow, label, panel_bg, set_text};
use crate::player::{Player, PlayerBody};
use crate::sprint::SprintHud;
use crate::touch::TouchMode;
use bevy::prelude::*;
use combat_core::{Command, Content, Event};

/// Orange of the Sprint affixes: Legacy reuses them.
pub const LEGACY: Color = Color::srgb(1.0, 0.62, 0.2);
/// The switch shows within this many metres of a qualifying exit trigger.
const NEAR_M: f32 = 9.0;

#[derive(Component)]
struct ChoiceRoot;
#[derive(Component)]
struct ChoiceTitle;
#[derive(Component)]
struct ChoiceCaption;
/// The two segments: false = Normal, true = Legacy.
#[derive(Component)]
struct ChoiceButton(bool);
#[derive(Component)]
struct ChoiceLabel(bool);

pub struct LegacyPlugin;

impl Plugin for LegacyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui)
            .add_systems(
                PostStartup,
                qa_legacy.after(crate::game::start_game).in_set(SimReady),
            )
            .add_systems(Update, (qa_locks, events, choice).in_set(SimReady));
    }
}

/// The affix name of a zone's Legacy variant, if it has one.
pub fn affix_name<'a>(content: &'a Content, zone: &str) -> Option<&'a str> {
    let z = content.legacy.zone(zone)?;
    content.sprint.affix(&z.affix).map(|a| a.name.as_str())
}

fn qa_legacy(mut sim: ResMut<CombatSim>) {
    if std::env::var_os("WOW_LEGACY").is_some() {
        sim.0.debug_set_legacy(true);
    }
}

/// The lockout belongs to an ISO week, so it waits for the office clock.
fn qa_locks(mut sim: ResMut<CombatSim>, mut done: Local<bool>) {
    if *done || !sim.0.clock_on() {
        return;
    }
    *done = true;
    if let Ok(v) = std::env::var("WOW_LEGACY_LOCKS") {
        let bosses: Vec<String> = v.split(',').map(|b| b.trim().to_string()).collect();
        sim.0.debug_set_legacy_lockout(&bosses);
    }
}

fn segment(on: bool, text: &str) -> impl Bundle {
    (
        Button,
        ChoiceButton(on),
        Node {
            padding: UiRect::axes(px(14), px(5)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(KEY_RIM),
        BackgroundColor(Color::NONE),
        children![(ChoiceLabel(on), label(text, 14.0))],
    )
}

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn((
            ChoiceRoot,
            Node {
                position_type: PositionType::Absolute,
                top: px(crate::hud::CHOOSER_TOP),
                left: percent(50),
                width: px(420),
                margin: UiRect::left(px(-210)),
                padding: UiRect::axes(px(10), px(6)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(LEGACY.with_alpha(0.6)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(14),
        ))
        .with_children(|c| {
            c.spawn((ChoiceTitle, label("", 12.0), TextColor(DIM)));
            c.spawn(Node {
                column_gap: px(8),
                ..default()
            })
            .with_children(|row| {
                row.spawn(segment(false, "Normal"));
                row.spawn(segment(true, "Legacy"));
            });
            c.spawn((
                ChoiceCaption,
                label("", 11.0),
                TextColor(DIM),
                TextLayout::justify(Justify::Center),
            ));
        });
}

/// Legacy events into banners and notices.
fn events(
    sim: SimView,
    mut seen: Local<String>,
    mut reader: MessageReader<CombatEvent>,
    mut banners: MessageWriter<Banner>,
    mut notices: MessageWriter<UiNotice>,
) {
    for CombatEvent(e) in reader.read() {
        match e {
            Event::Legacy { view } => {
                // Once per floor entered in Legacy Mode, after the zone's own banner.
                if !view.zone.is_empty() && *seen != view.zone {
                    let content = sim.content();
                    if let Some(a) = content
                        .legacy
                        .zone(&view.zone)
                        .and_then(|z| content.sprint.affix(&z.affix))
                    {
                        banners.write(Banner {
                            title: format!("LEGACY MODE: {}", a.name),
                            subtitle: a.text.clone(),
                            color: LEGACY,
                        });
                    }
                }
                seen.clone_from(&view.zone);
            }
            Event::LegacyRefused { reason } => {
                notices.write(UiNotice(reason.message().to_string()));
            }
            _ => {}
        }
    }
}

/// Distance in the ground plane from `p` to the box (0 inside it).
fn dist_to(b: &combat_core::Aabb, p: Vec3) -> f32 {
    let dx = (b.min.x - p.x).max(p.x - b.max.x).max(0.0);
    let dz = (b.min.z - p.z).max(p.z - b.max.z).max(0.0);
    (dx * dx + dz * dz).sqrt()
}

fn choice(
    sim: SimView,
    flow: Res<GameFlow>,
    sprint: Res<SprintHud>,
    touch: Res<TouchMode>,
    body: Single<&PlayerBody, With<Player>>,
    mut pending: ResMut<PendingCommands>,
    buttons: Query<(&Interaction, &ChoiceButton), Changed<Interaction>>,
    mut root: Single<&mut Node, (With<ChoiceRoot>, Without<ChoiceButton>)>,
    mut segs: Query<
        (
            &ChoiceButton,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Node,
        ),
        Without<ChoiceRoot>,
    >,
    mut labels: Query<
        (&ChoiceLabel, &mut Text, &mut TextColor),
        (Without<ChoiceTitle>, Without<ChoiceCaption>),
    >,
    mut title: Single<
        &mut Text,
        (
            With<ChoiceTitle>,
            Without<ChoiceLabel>,
            Without<ChoiceCaption>,
        ),
    >,
    mut caption: Single<
        &mut Text,
        (
            With<ChoiceCaption>,
            Without<ChoiceLabel>,
            Without<ChoiceTitle>,
        ),
    >,
) {
    let content = sim.content();
    let me = sim.player().and_then(|p| sim.unit(p));
    let capped = me.is_some_and(|u| u.alive && u.level >= content.max_level());
    let near = flow
        .exits
        .iter()
        .filter(|e| content.legacy.zone(&e.to).is_some())
        .map(|e| (e, dist_to(&e.trigger, body.pos)))
        .filter(|(_, d)| *d <= NEAR_M)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e.to.clone());
    let show = capped && !sprint.active && !flow.input_locked() && near.is_some();
    let want = if show { Display::Flex } else { Display::None };
    if root.display != want {
        root.display = want;
    }
    let Some(to) = near.filter(|_| show) else {
        return;
    };
    let view = sim.legacy_view();
    let lead = sim.players().first().copied() == sim.player();
    if lead {
        for (i, b) in &buttons {
            if *i == Interaction::Pressed && b.0 != view.on {
                pending.0.push(Command::SetLegacy { on: b.0 });
            }
        }
    }
    let affix = affix_name(content, &to).unwrap_or("one affix");
    set_text(&mut title, &format!("Next: {}", zone_label(content, &to)));
    let boss = content
        .zone(&to)
        .and_then(|z| z.boss.as_ref())
        .map(|b| b.unit.clone());
    let lock = match boss {
        Some(b) if view.locked.contains(&b) => "boss bonus taken this week",
        Some(_) => "boss bonus open this week",
        None => "no boss on this floor",
    };
    let caption_text = match (view.on, lead) {
        (true, _) => format!(
            "Level {}-{} enemies, {affix}, Legacy loot and reputation; {lock}.",
            content.legacy.mob_level, content.legacy.boss_level
        ),
        (false, true) => "The campaign floor as you know it. Legacy is the heroic tier.".into(),
        (false, false) => "The group lead picks the difficulty.".into(),
    };
    let caption_text = if !lead && view.on {
        format!("{caption_text} The group lead picks.")
    } else {
        caption_text
    };
    set_text(&mut caption, &caption_text);
    // Touch: just above the action bar, so the door itself stays in view.
    let (top, bottom) = if touch.active {
        (Val::Auto, px(94))
    } else {
        (px(crate::hud::CHOOSER_TOP), Val::Auto)
    };
    if root.bottom != bottom {
        root.top = top;
        root.bottom = bottom;
    }
    let pad = if touch.active { px(12) } else { px(5) };
    for (b, mut bg, mut border, mut node) in &mut segs {
        let on = b.0 == view.on;
        let tint = if b.0 { LEGACY } else { ACCENT };
        bg.0 = if on {
            tint.with_alpha(0.85)
        } else {
            Color::NONE
        };
        *border = BorderColor::all(if on { tint } else { KEY_RIM });
        if node.padding.top != pad {
            node.padding = UiRect::axes(px(16), pad);
        }
    }
    for (l, mut text, mut color) in &mut labels {
        let on = l.0 == view.on;
        let s = if l.0 {
            format!("Legacy: {affix}")
        } else {
            "Normal".to_string()
        };
        set_text(&mut text, &s);
        color.0 = if on {
            Color::srgb(0.05, 0.05, 0.08)
        } else if lead {
            Color::WHITE
        } else {
            DIM
        };
    }
}
