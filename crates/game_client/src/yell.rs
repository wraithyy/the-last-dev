//! Speech bubbles for `Event::Yell`: UI nodes pinned above the yelling unit's head.

use crate::combat::{CombatEvent, CombatSim, SimReady, SimUnit};
use crate::config::{BUBBLE_HEAD_GAP, PLAYER_HEIGHT, head_height};
use crate::copy::yell_color;
use bevy::prelude::*;
use combat_core::{EntityId, Event};

const BUBBLE_SECS: f32 = 5.0;
const BUBBLE_MAX_W: f32 = 300.0;

#[derive(Component)]
struct YellBubble {
    sim: EntityId,
    anchor: Entity,
    /// World height above the unit's feet, from its model height.
    anchor_y: f32,
    age: f32,
}

pub struct YellPlugin;

impl Plugin for YellPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_bubbles, place_bubbles).chain().in_set(SimReady),
        );
    }
}

fn spawn_bubbles(
    mut commands: Commands,
    mut events: MessageReader<CombatEvent>,
    sim: Res<CombatSim>,
    units: Query<(Entity, &SimUnit)>,
    old: Query<(Entity, &YellBubble)>,
) {
    for CombatEvent(ev) in events.read() {
        let Event::Yell { unit, text } = ev else {
            continue;
        };
        let Some((anchor, _)) = units.iter().find(|(_, su)| su.0 == *unit) else {
            continue;
        };
        let state = sim.0.unit(*unit);
        let anchor_y = state.map_or(PLAYER_HEIGHT, |u| head_height(&u.key)) + BUBBLE_HEAD_GAP;
        let trim = yell_color(sim.0.player() == Some(*unit));
        // Newer yell replaces the older bubble of the same unit (also one from this same frame).
        for (e, b) in &old {
            if b.sim == *unit {
                commands.entity(e).despawn();
            }
        }
        // The outer node has a fixed width so the centred bubble can wrap at BUBBLE_MAX_W.
        commands
            .spawn((
                YellBubble {
                    sim: *unit,
                    anchor,
                    anchor_y,
                    age: 0.0,
                },
                Node {
                    position_type: PositionType::Absolute,
                    width: px(BUBBLE_MAX_W),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Visibility::Hidden,
                GlobalZIndex(12),
            ))
            .with_child((
                Node {
                    padding: UiRect::axes(px(10), px(6)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.95, 0.97, 0.99)),
                BorderColor::all(trim),
                crate::hud::drop_shadow(),
                children![
                    // Speech-bubble tail: a rotated square whose two outer edges carry the border.
                    (
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: px(-8),
                            left: percent(50),
                            margin: UiRect::left(px(-6)),
                            width: px(12),
                            height: px(12),
                            border: UiRect {
                                right: px(2),
                                bottom: px(2),
                                ..default()
                            },
                            ..default()
                        },
                        UiTransform::from_rotation(Rot2::degrees(45.0)),
                        BackgroundColor(Color::srgb(0.95, 0.97, 0.99)),
                        BorderColor::all(trim),
                    ),
                    (
                        Text::new(text.clone()),
                        TextFont::from_font_size(14.0),
                        TextLayout::justify(Justify::Center),
                        TextColor(Color::srgb(0.1, 0.1, 0.12)),
                    )
                ],
            ));
    }
}

fn place_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    transforms: Query<&Transform, Without<YellBubble>>,
    mut bubbles: Query<(Entity, &mut YellBubble, &mut Node, &mut Visibility)>,
) {
    let (cam, cam_tf) = *camera;
    // Node coordinates are multiplied by UiScale, viewport coordinates are not.
    let scale = ui_scale.0.max(0.01);
    for (entity, mut bubble, mut node, mut vis) in &mut bubbles {
        bubble.age += time.delta_secs();
        let Ok(tf) = transforms.get(bubble.anchor) else {
            commands.entity(entity).despawn();
            continue;
        };
        if bubble.age >= BUBBLE_SECS {
            commands.entity(entity).despawn();
            continue;
        }
        let Ok(screen) = cam.world_to_viewport(cam_tf, tf.translation + Vec3::Y * bubble.anchor_y)
        else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        // Anchored by `bottom` so a taller (wrapped) bubble grows upwards, not into the unit.
        node.left = px(screen.x / scale - BUBBLE_MAX_W / 2.0);
        node.bottom = px((window.height() - screen.y) / scale);
    }
}
