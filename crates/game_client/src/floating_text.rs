//! Floating combat text: UI text nodes pinned to a world-space anchor above a unit.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, CombatSim, SimReady, SimUnit};
use crate::config::*;
use crate::player::Player;
use bevy::prelude::*;
use combat_core::{EntityId, Event};

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
}

fn entity_of(units: &Query<(Entity, &SimUnit)>, id: EntityId) -> Option<Entity> {
    units.iter().find(|(_, su)| su.0 == id).map(|(e, _)| e)
}

fn spawn_texts(
    mut commands: Commands,
    mut events: MessageReader<CombatEvent>,
    sim: Res<CombatSim>,
    player: Single<&SimUnit, With<Player>>,
    units: Query<(Entity, &SimUnit)>,
    mut counter: Local<usize>,
) {
    for CombatEvent(ev) in events.read() {
        let (target, specs) = match ev {
            Event::Damage {
                target,
                amount,
                absorbed,
                crit,
                combo,
                ..
            } => {
                let on_player = *target == player.0;
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
                        size: if *crit {
                            FLOAT_TEXT_CRIT_SIZE
                        } else {
                            FLOAT_TEXT_SIZE
                        },
                        color: base,
                        offset: Vec2::new(stagger, 0.0),
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
                    });
                }
                if *combo && !on_player {
                    specs.push(Spec {
                        text: "COMBO!".into(),
                        size: 18.0,
                        color: Color::srgb(1.0, 0.4, 0.9),
                        offset: Vec2::new(stagger, -24.0),
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
                }],
            ),
            _ => continue,
        };
        let Some(anchor) = entity_of(&units, target) else {
            continue;
        };
        let fallback = sim
            .0
            .unit(target)
            .map_or(Vec3::ZERO, |u| Vec3::new(u.pos.x, u.pos.y, u.pos.z));
        for spec in specs {
            commands.spawn((
                FloatText {
                    anchor,
                    fallback,
                    age: 0.0,
                    life: FLOAT_TEXT_SECS,
                    offset: spec.offset,
                    color: spec.color,
                },
                Text::new(spec.text),
                TextFont::from_font_size(spec.size),
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
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    transforms: Query<&Transform, Without<FloatText>>,
    mut texts: Query<(
        Entity,
        &mut FloatText,
        &mut Node,
        &mut TextColor,
        &mut Visibility,
    )>,
) {
    let (cam, cam_tf) = *camera;
    for (entity, mut ft, mut node, mut color, mut vis) in &mut texts {
        ft.age += time.delta_secs();
        let k = ft.age / ft.life;
        if k >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let world = transforms
            .get(ft.anchor)
            .map_or(ft.fallback, |t| t.translation)
            + Vec3::Y * FLOAT_TEXT_ANCHOR_Y;
        let Ok(screen) = cam.world_to_viewport(cam_tf, world) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        node.left = px(screen.x - TEXT_BOX_W / 2.0 + ft.offset.x);
        node.top = px(screen.y - FLOAT_TEXT_RISE * k + ft.offset.y);
        // Hold full opacity for the first half, then fade.
        color.0 = ft.color.with_alpha((2.0 * (1.0 - k)).min(1.0));
    }
}
