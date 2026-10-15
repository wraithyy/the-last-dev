//! Floating combat text: UI text nodes pinned to a world-space anchor above a unit.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, SimReady, SimUnit, SimView};
use crate::config::*;
use crate::player::Player;
use crate::presentation::PresentEvent;
use bevy::prelude::*;
use combat_core::{BurnoutCause, Content, Effect, EntityId, Event, SpellId};

const DOT_TEXT_SIZE: f32 = 18.0;
const HEAL_GREEN: Color = Color::srgb(0.35, 1.0, 0.4);

/// Damage from a spell whose effects deal no direct damage came from its aura (DoT tick, Deadline).
fn is_periodic(content: &Content, spell: Option<SpellId>) -> bool {
    spell.and_then(|id| content.spell(id)).is_some_and(|d| {
        !std::iter::once(&d.effect)
            .chain(&d.extra_effects)
            .any(|e| matches!(e, Effect::Damage { .. }))
    })
}

/// An execute spell that hit a target at or below its threshold (judged from HP after the hit).
fn is_execute(
    content: &Content,
    spell: Option<SpellId>,
    hp_after: u32,
    amount: u32,
    max_hp: u32,
) -> bool {
    let threshold = spell
        .and_then(|id| content.spell(id))
        .and_then(|d| match &d.effect {
            Effect::Damage {
                execute: Some(e), ..
            } => Some(e.below_pct),
            _ => None,
        });
    threshold.is_some_and(|pct| (hp_after + amount) * 100 <= pct * max_hp.max(1))
}

const POP_SECS: f32 = 0.22;
const TEXT_BOX_W: f32 = 240.0;
/// Horizontal staggering (px) so back-to-back hits do not overprint each other.
const STAGGER: [f32; 3] = [-34.0, 0.0, 34.0];

#[derive(Component)]
struct FloatText {
    anchor: Entity,
    fallback: Vec3,
    age: f32,
    life: f32,
    offset: Vec2,
    color: Color,
    /// World height of the spawn point above the unit's feet.
    anchor_y: f32,
    pop: bool,
}

pub struct FloatingTextPlugin;

impl Plugin for FloatingTextPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_texts, place_texts).chain().in_set(SimReady));
    }
}

struct Spec {
    text: String,
    size: f32,
    color: Color,
    offset: Vec2,
    pop: bool,
}

fn entity_of(units: &Query<(Entity, &SimUnit)>, id: EntityId) -> Option<Entity> {
    units.iter().find(|(_, su)| su.0 == id).map(|(e, _)| e)
}

fn spawn_texts(
    mut commands: Commands,
    mut raw: MessageReader<CombatEvent>,
    mut present: MessageReader<PresentEvent>,
    sim: SimView,
    player: Single<&SimUnit, With<Player>>,
    units: Query<(Entity, &SimUnit)>,
    mut counter: Local<usize>,
) {
    // Interrupts show immediately; Damage/Miss arrive via the presentation queue (at impact).
    let interrupts = raw.read().map(|CombatEvent(ev)| ev).filter(|ev| {
        matches!(
            ev,
            Event::CastInterrupted { .. }
                | Event::Heal { .. }
                | Event::ManaGain { .. }
                | Event::Burnout { .. }
        )
    });
    let presented = present.read().map(|PresentEvent(ev)| ev);
    for ev in interrupts.chain(presented) {
        let (target, specs) = match ev {
            Event::Heal { target, amount, .. } if *amount > 0 => {
                let stagger = STAGGER[*counter % STAGGER.len()];
                *counter += 1;
                (
                    *target,
                    vec![Spec {
                        text: format!("+{amount}"),
                        size: FLOAT_TEXT_SIZE,
                        color: HEAL_GREEN,
                        offset: Vec2::new(stagger, 0.0),
                        pop: false,
                    }],
                )
            }
            Event::ManaGain { unit, amount, .. } => (
                *unit,
                vec![Spec {
                    text: format!("+{amount}"),
                    size: FLOAT_TEXT_SIZE,
                    color: crate::copy::COFFEE_COLOR,
                    offset: Vec2::ZERO,
                    pop: false,
                }],
            ),
            Event::Damage {
                target,
                spell,
                amount,
                absorbed,
                crit,
                combo,
                ..
            } => {
                let on_player = *target == player.0;
                let content = sim.content();
                let dot = is_periodic(content, *spell);
                let execute = sim.unit(*target).is_some_and(|u| {
                    *amount > 0 && is_execute(content, *spell, u.hp, *amount, u.max_hp)
                });
                let stagger = STAGGER[*counter % STAGGER.len()];
                *counter += 1;
                let base = if on_player {
                    Color::srgb(1.0, 0.3, 0.3)
                } else if *combo {
                    Color::srgb(1.0, 0.55, 0.15)
                } else {
                    Color::srgb(1.0, 0.95, 0.6)
                };
                let mut specs = Vec::new();
                if *amount > 0 {
                    specs.push(Spec {
                        text: if *crit {
                            format!("{amount}!")
                        } else {
                            amount.to_string()
                        },
                        size: if *crit || execute {
                            FLOAT_TEXT_CRIT_SIZE
                        } else if dot {
                            DOT_TEXT_SIZE
                        } else {
                            FLOAT_TEXT_SIZE
                        },
                        color: base,
                        offset: Vec2::new(stagger, 0.0),
                        pop: *crit || execute,
                    });
                }
                if execute && !on_player {
                    specs.push(Spec {
                        text: "EXECUTE!".into(),
                        size: 20.0,
                        color: Color::srgb(1.0, 0.3, 0.2),
                        offset: Vec2::new(stagger, -26.0),
                        pop: true,
                    });
                }
                if *absorbed > 0 {
                    let text = if *amount == 0 {
                        "Absorbed".to_string()
                    } else {
                        format!("({absorbed} absorbed)")
                    };
                    specs.push(Spec {
                        text,
                        size: 16.0,
                        color: Color::srgb(1.0, 0.85, 0.4),
                        offset: Vec2::new(stagger, 22.0),
                        pop: false,
                    });
                }
                if *combo && !on_player {
                    specs.push(Spec {
                        text: "COMBO!".into(),
                        size: 18.0,
                        color: Color::srgb(1.0, 0.4, 0.9),
                        offset: Vec2::new(stagger, -24.0),
                        pop: true,
                    });
                }
                (*target, specs)
            }
            Event::CastInterrupted {
                caster,
                by: Some(_),
                ..
            } if *caster != player.0 => (
                *caster,
                vec![Spec {
                    text: "Interrupted".into(),
                    size: 22.0,
                    color: Color::srgb(1.0, 0.85, 0.2),
                    offset: Vec2::ZERO,
                    pop: false,
                }],
            ),
            Event::Miss { target, spell, .. } => {
                let stagger = STAGGER[*counter % STAGGER.len()];
                *counter += 1;
                (
                    *target,
                    vec![Spec {
                        text: if spell.is_some() { "Resist" } else { "Miss" }.into(),
                        size: 16.0,
                        color: Color::srgb(0.68, 0.68, 0.72),
                        offset: Vec2::new(stagger, 0.0),
                        pop: false,
                    }],
                )
            }
            // Relief you earned (a manager down, a prop, a coffee) floats; slow drifts do not.
            Event::Burnout {
                unit, delta, cause, ..
            } if *unit == player.0
                && matches!(
                    cause,
                    BurnoutCause::Kill | BurnoutCause::Prop | BurnoutCause::CoffeeBreak
                ) =>
            {
                (
                    *unit,
                    vec![Spec {
                        text: format!("{} burnout", crate::copy::burnout_pct(*delta)),
                        size: 18.0,
                        color: HEAL_GREEN,
                        offset: Vec2::new(0.0, -30.0),
                        pop: false,
                    }],
                )
            }
            _ => continue,
        };
        let Some(anchor) = entity_of(&units, target) else {
            continue;
        };
        let fallback = sim
            .unit(target)
            .map_or(Vec3::ZERO, |u| Vec3::new(u.pos.x, u.pos.y, u.pos.z));
        let anchor_y = sim
            .unit(target)
            .map_or(PLAYER_HEIGHT, |u| head_height(&u.key))
            + FLOAT_TEXT_HEAD_GAP;
        for spec in specs {
            commands.spawn((
                FloatText {
                    anchor,
                    fallback,
                    age: 0.0,
                    life: FLOAT_TEXT_SECS,
                    offset: spec.offset,
                    color: spec.color,
                    anchor_y,
                    pop: spec.pop,
                },
                Text::new(spec.text),
                TextFont::from_font_size(spec.size),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.9),
                },
                UiTransform::default(),
                TextLayout::justify(Justify::Center),
                TextColor(spec.color),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(TEXT_BOX_W),
                    ..default()
                },
                Visibility::Hidden,
                GlobalZIndex(10),
            ));
        }
    }
}

fn place_texts(
    mut commands: Commands,
    time: Res<Time>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    ui_scale: Res<UiScale>,
    transforms: Query<&Transform, Without<FloatText>>,
    mut texts: Query<(
        Entity,
        &mut FloatText,
        &mut Node,
        &mut TextColor,
        &mut TextShadow,
        &mut UiTransform,
        &mut Visibility,
    )>,
) {
    let (cam, cam_tf) = *camera;
    // Node coordinates are multiplied by UiScale, viewport coordinates are not.
    let scale = ui_scale.0.max(0.01);
    for (entity, mut ft, mut node, mut color, mut shadow, mut tf_ui, mut vis) in &mut texts {
        ft.age += time.delta_secs();
        let k = ft.age / ft.life;
        if k >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let world = transforms
            .get(ft.anchor)
            .map_or(ft.fallback, |t| t.translation)
            + Vec3::Y * ft.anchor_y;
        let Ok(screen) = cam.world_to_viewport(cam_tf, world) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        node.left = px(screen.x / scale - TEXT_BOX_W / 2.0 + ft.offset.x);
        node.top = px(screen.y / scale - FLOAT_TEXT_RISE * k + ft.offset.y);
        // Hold full opacity for the first half, then fade.
        let alpha = (2.0 * (1.0 - k)).min(1.0);
        color.0 = ft.color.with_alpha(alpha);
        shadow.color = Color::srgba(0.0, 0.0, 0.0, 0.9 * alpha);
        // Crits and combos land big and settle: a short ease-out pop.
        let pop = if ft.pop {
            1.0 + 0.7 * (1.0 - (ft.age / POP_SECS).min(1.0)).powi(2)
        } else {
            1.0
        };
        tf_ui.scale = Vec2::splat(pop);
    }
}
