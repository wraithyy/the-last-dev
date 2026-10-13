//! Speech bubbles for `Event::Yell`: UI nodes pinned above the yelling unit's head.

use crate::combat::{CombatEvent, CombatSim, SimReady, SimUnit};
use crate::config::{BUBBLE_HEAD_GAP, PLAYER_HEIGHT, head_height};
use crate::copy::yell_color;
use crate::player::Player;
use bevy::prelude::*;
use combat_core::{EntityId, Event};

/// Bosses and friendly NPCs are read carefully; trash chatter should clear quickly.
const BUBBLE_SECS_LONG: f32 = 5.0;
const BUBBLE_SECS_SHORT: f32 = 3.5;
const BUBBLE_FADE_SECS: f32 = 0.5;
const BUBBLE_MAX_W: f32 = 300.0;
/// Bubbles of units farther than this from the player are not shown.
const BUBBLE_RANGE_M: f32 = 25.0;
const MAX_BUBBLES: usize = 3;
/// Window-pixel gap kept between two stacked bubbles.
const STACK_GAP: f32 = 6.0;
/// Height of the speech-bubble tail below the body.
const TAIL_PX: f32 = 8.0;
/// Exponential rate (1/s) at which a bubble eases to its de-overlapped position.
const OFFSET_RATE: f32 = 12.0;
/// Player and target frames (with cast bar and aura rows): bubbles are kept out of it.
const RESERVED: Rect = Rect {
    min: Vec2::new(0.0, 0.0),
    max: Vec2::new(520.0, 150.0),
};
const BUBBLE_BG: Color = Color::srgb(0.95, 0.97, 0.99);
const BUBBLE_TEXT: Color = Color::srgb(0.1, 0.1, 0.12);

#[derive(Component)]
struct YellBubble {
    sim: EntityId,
    anchor: Entity,
    /// World height above the unit's feet, from its model height.
    anchor_y: f32,
    age: f32,
    life: f32,
    /// Boss 2, player 1, everything else 0; decides who keeps a slot when more than
    /// `MAX_BUBBLES` compete.
    priority: u8,
    born: f32,
    trim: Color,
    /// Smoothed upward push (window px) that keeps this bubble off the others.
    offset: f32,
    placed: bool,
    body: Entity,
    tail: Entity,
    text: Entity,
    stem: Entity,
}

/// Inner bubble box, measured for overlap tests.
#[derive(Component)]
struct BubbleBody;

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
    time: Res<Time>,
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
        let trim = yell_color(
            sim.0.player() == Some(*unit),
            state.is_some_and(|u| u.hostile),
        );
        // Newer yell replaces the older bubble of the same unit (also one from this same frame).
        for (e, b) in &old {
            if b.sim == *unit {
                commands.entity(e).despawn();
            }
        }
        let is_player = sim.0.player() == Some(*unit);
        let boss = state.is_some_and(|u| u.boss);
        let hostile = state.is_some_and(|u| u.hostile);
        let (priority, life) = if boss {
            (2, BUBBLE_SECS_LONG)
        } else if is_player {
            (1, BUBBLE_SECS_SHORT)
        } else if hostile {
            (0, BUBBLE_SECS_SHORT)
        } else {
            (0, BUBBLE_SECS_LONG)
        };
        // Speech-bubble tail: a rotated square whose two outer edges carry the border.
        let tail = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(-TAIL_PX),
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
                BackgroundColor(BUBBLE_BG),
                BorderColor::all(trim),
            ))
            .id();
        // Grows when the bubble is pushed up, so the tail still reaches the unit.
        let stem = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(-TAIL_PX),
                    left: percent(50),
                    margin: UiRect::left(px(-1)),
                    width: px(2),
                    height: px(0),
                    ..default()
                },
                BackgroundColor(trim),
            ))
            .id();
        let text = commands
            .spawn((
                Text::new(text.clone()),
                TextFont::from_font_size(14.0),
                TextLayout::justify(Justify::Center),
                TextColor(BUBBLE_TEXT),
            ))
            .id();
        let body = commands
            .spawn((
                BubbleBody,
                Node {
                    padding: UiRect::axes(px(10), px(6)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(BUBBLE_BG),
                BorderColor::all(trim),
                crate::hud::drop_shadow(),
            ))
            .add_children(&[stem, tail, text])
            .id();
        // The outer node has a fixed width so the centred bubble can wrap at BUBBLE_MAX_W.
        commands
            .spawn((
                YellBubble {
                    sim: *unit,
                    anchor,
                    anchor_y,
                    age: 0.0,
                    life,
                    priority,
                    born: time.elapsed_secs(),
                    trim,
                    offset: 0.0,
                    placed: false,
                    body,
                    tail,
                    text,
                    stem,
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
            .add_child(body);
    }
}

/// One bubble that survived culling, in window-pixel space (y grows downwards).
struct Slot {
    entity: Entity,
    priority: u8,
    born: f32,
    x: f32,
    /// Bottom edge of the body at the unit's projected head, tail excluded.
    bottom: f32,
    w: f32,
    h: f32,
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters
fn place_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    transforms: Query<&Transform, Without<YellBubble>>,
    player: Query<&Transform, (With<Player>, Without<YellBubble>)>,
    sizes: Query<&ComputedNode, With<BubbleBody>>,
    mut bubbles: Query<(Entity, &mut YellBubble, &mut Node, &mut Visibility)>,
    mut stems: Query<&mut Node, Without<YellBubble>>,
    mut fills: Query<&mut BackgroundColor>,
    mut borders: Query<&mut BorderColor>,
    mut texts: Query<&mut TextColor>,
    mut shadows: Query<&mut BoxShadow>,
) {
    let (cam, cam_tf) = *camera;
    // Node coordinates are multiplied by UiScale, viewport coordinates are not.
    let scale = ui_scale.0.max(0.01);
    let dt = time.delta_secs();
    let eye = cam_tf.translation();
    let forward = cam_tf.forward();
    let player_pos = player.single().ok().map(|t| t.translation);
    let (win_w, win_h) = (window.width(), window.height());

    let mut slots: Vec<Slot> = Vec::new();
    let mut hidden: Vec<Entity> = Vec::new();
    for (entity, mut bubble, _, _) in &mut bubbles {
        bubble.age += dt;
        let Ok(tf) = transforms.get(bubble.anchor) else {
            commands.entity(entity).despawn();
            continue;
        };
        if bubble.age >= bubble.life {
            commands.entity(entity).despawn();
            continue;
        }
        let head = tf.translation + Vec3::Y * bubble.anchor_y;
        let near = player_pos.is_none_or(|p| p.distance(tf.translation) <= BUBBLE_RANGE_M);
        let screen = cam.world_to_viewport(cam_tf, head).ok().filter(|s| {
            near && (head - eye).dot(*forward) > 0.0
                && s.x >= 0.0
                && s.x <= win_w
                && s.y >= 0.0
                && s.y <= win_h
        });
        let Some(screen) = screen else {
            hidden.push(entity);
            continue;
        };
        // Physical size divided by the window factor is window-logical pixels. Until the
        // first layout pass a rough default keeps the bubble from being culled for a frame.
        let size = sizes
            .get(bubble.body)
            .map(|c| c.size() / window.scale_factor())
            .unwrap_or(Vec2::new(160.0, 34.0));
        slots.push(Slot {
            entity,
            priority: bubble.priority,
            born: bubble.born,
            x: screen.x,
            bottom: screen.y - TAIL_PX,
            w: size.x.max(1.0),
            h: size.y.max(1.0),
        });
    }

    // Keep the most important, then newest, bubbles.
    slots.sort_by(|a, b| b.priority.cmp(&a.priority).then(b.born.total_cmp(&a.born)));
    for s in slots.drain(MAX_BUBBLES.min(slots.len())..) {
        hidden.push(s.entity);
    }

    // Lowest on screen first; each later bubble is lifted above whatever it hits.
    slots.sort_by(|a, b| b.bottom.total_cmp(&a.bottom));
    let mut placed: Vec<Rect> = Vec::new();
    let mut targets: Vec<(Entity, f32)> = Vec::new();
    for s in &slots {
        let mut dy = 0.0_f32;
        let mut x = s.x;
        let rect_at = |x: f32, dy: f32| Rect {
            min: Vec2::new(x - s.w / 2.0, s.bottom - dy - s.h),
            max: Vec2::new(x + s.w / 2.0, s.bottom - dy),
        };
        if overlaps(&rect_at(x, dy), &RESERVED) {
            // Sideways: moving down would cover the unit.
            x = RESERVED.max.x + s.w / 2.0 + STACK_GAP;
        }
        // Each pass can only raise `dy`, so a few passes settle three bubbles.
        for _ in 0..MAX_BUBBLES {
            for r in &placed {
                if overlaps(&rect_at(x, dy), r) {
                    dy = s.bottom - s.h - (r.min.y - STACK_GAP);
                }
            }
        }
        placed.push(rect_at(x, dy));
        targets.push((s.entity, dy));
    }

    let ease = 1.0 - (-OFFSET_RATE * dt).exp();
    for (entity, mut bubble, mut node, mut vis) in &mut bubbles {
        let Some(slot) = slots.iter().find(|s| s.entity == entity) else {
            if hidden.contains(&entity) {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let target = targets
            .iter()
            .find(|(e, _)| *e == entity)
            .map_or(0.0, |(_, d)| *d);
        if bubble.placed {
            bubble.offset += (target - bubble.offset) * ease;
        } else {
            bubble.offset = target;
            bubble.placed = true;
        }
        *vis = Visibility::Inherited;
        let offset = bubble.offset.max(0.0);
        // Anchored by `bottom` so a taller (wrapped) bubble grows upwards, not into the unit.
        node.left = px(slot.x / scale - BUBBLE_MAX_W / 2.0);
        node.bottom = px((win_h - slot.bottom - TAIL_PX + offset) / scale);
        if let Ok(mut stem) = stems.get_mut(bubble.stem) {
            stem.height = px(offset / scale);
            stem.bottom = px(-TAIL_PX - offset / scale);
        }

        let alpha = ((bubble.life - bubble.age) / BUBBLE_FADE_SECS).clamp(0.0, 1.0);
        for e in [bubble.body, bubble.tail] {
            if let Ok(mut c) = fills.get_mut(e) {
                c.0 = BUBBLE_BG.with_alpha(alpha);
            }
            if let Ok(mut c) = borders.get_mut(e) {
                *c = BorderColor::all(bubble.trim.with_alpha(alpha));
            }
        }
        if let Ok(mut c) = fills.get_mut(bubble.stem) {
            c.0 = bubble.trim.with_alpha(alpha);
        }
        if let Ok(mut c) = texts.get_mut(bubble.text) {
            c.0 = BUBBLE_TEXT.with_alpha(alpha);
        }
        if let Ok(mut sh) = shadows.get_mut(bubble.body) {
            for st in &mut sh.0 {
                st.color = Color::srgba(0.0, 0.0, 0.0, 0.55 * alpha);
            }
        }
    }
}
