//! World-space nameplates: UI nodes pinned above every NPC's head. Hostiles get
//! a red name and an HP bar, friendlies a green name and their title; bosses and
//! mini-bosses carry a tag. Plates are culled beyond `PLATE_RANGE` and behind the camera.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatSim, SimReady, SimUnit};
use crate::config::head_height;
use crate::copy;
use crate::hud::{ACCENT_WARM, BAR_BG, label};
use crate::player::Player;
use crate::targeting::Target;
use crate::units::Unit;
use bevy::prelude::*;
use combat_core::EntityId;

const PLATE_W: f32 = 180.0;
const PLATE_RANGE: f32 = 40.0;
const PLATE_GAP: f32 = 0.3;
const BAR_W: f32 = 110.0;
const HOSTILE: Color = Color::srgb(1.0, 0.3, 0.25);
const FRIENDLY: Color = Color::srgb(0.4, 1.0, 0.45);

#[derive(Component)]
struct Nameplate {
    anchor: Entity,
    sim: EntityId,
    height: f32,
}
#[derive(Component)]
struct PlateFill(EntityId);
/// Name text and "LOOT" tag of a plate; `loot_fx` recolours and reveals them on lootable corpses.
#[derive(Component)]
pub struct PlateName(pub EntityId);
#[derive(Component)]
pub struct PlateLootTag(pub EntityId);

pub struct NameplatePlugin;

impl Plugin for NameplatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_plates, place_plates).chain().in_set(SimReady),
        );
    }
}

fn spawn_plates(
    mut commands: Commands,
    sim: Res<CombatSim>,
    added: Query<(Entity, &SimUnit), (Added<SimUnit>, With<Unit>, Without<Player>)>,
) {
    for (anchor, su) in &added {
        let Some(u) = sim.0.unit(su.0) else { continue };
        let tag = if u.boss {
            Some(("BOSS", ACCENT_WARM))
        } else if copy::is_elite(&u.key) {
            Some(("ELITE", Color::srgb(0.8, 0.55, 1.0)))
        } else {
            None
        };
        let title = sim
            .0
            .content()
            .unit_def(&u.key)
            .map(|d| d.title.clone())
            .unwrap_or_default();
        let mut root = commands.spawn((
            Nameplate {
                anchor,
                sim: u.id,
                height: head_height(&u.key) + PLATE_GAP,
            },
            Node {
                position_type: PositionType::Absolute,
                width: px(PLATE_W),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2),
                ..default()
            },
            UiTransform::default(),
            Visibility::Hidden,
            GlobalZIndex(5),
        ));
        root.with_children(|p| {
            p.spawn(Node {
                column_gap: px(4),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                if let Some((text, color)) = tag {
                    row.spawn((
                        Node {
                            padding: UiRect::axes(px(3), px(0)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BorderColor::all(color),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        children![(label(text, 9.0), TextColor(color))],
                    ));
                }
                row.spawn((
                    PlateName(u.id),
                    label(format!("{} {}", u.level, u.name), 13.0),
                    TextColor(if u.hostile { HOSTILE } else { FRIENDLY }),
                ));
                if u.hostile {
                    row.spawn((
                        PlateLootTag(u.id),
                        Node {
                            display: Display::None,
                            padding: UiRect::axes(px(3), px(0)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BorderColor::all(crate::loot_fx::GOLD),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        children![(label("LOOT", 9.0), TextColor(crate::loot_fx::GOLD))],
                    ));
                }
            });
            if u.hostile {
                p.spawn((
                    Node {
                        width: px(BAR_W),
                        height: px(7),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(2)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.9)),
                    BackgroundColor(BAR_BG),
                ))
                .with_child((
                    PlateFill(u.id),
                    Node {
                        height: percent(100),
                        width: percent(100),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.85, 0.15, 0.12)),
                ));
            } else if !title.is_empty() {
                p.spawn((
                    label(format!("<{title}>"), 10.0),
                    TextColor(Color::srgb(0.75, 0.85, 0.78)),
                ));
            }
        });
    }
}

fn place_plates(
    mut commands: Commands,
    sim: Res<CombatSim>,
    target: Res<Target>,
    ui_scale: Res<UiScale>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    transforms: Query<&Transform, With<SimUnit>>,
    mut plates: Query<(
        Entity,
        &Nameplate,
        &mut Node,
        &mut UiTransform,
        &mut Visibility,
    )>,
    mut fills: Query<(&PlateFill, &mut Node), Without<Nameplate>>,
) {
    let (cam, cam_tf) = *camera;
    // Node coordinates are multiplied by UiScale, viewport coordinates are not.
    let scale = ui_scale.0.max(0.01);
    let eye = cam_tf.translation();
    let forward = cam_tf.forward();
    for (e, plate, mut node, mut tf_ui, mut vis) in &mut plates {
        let Ok(tf) = transforms.get(plate.anchor) else {
            commands.entity(e).despawn();
            continue;
        };
        // A lootable corpse keeps its plate so the gold name marks it.
        let alive = sim.0.unit(plate.sim).is_some_and(|u| u.alive || u.lootable);
        let world = tf.translation + Vec3::Y * plate.height;
        let dist = world.distance(eye);
        let in_front = (world - eye).dot(*forward) > 0.0;
        let screen = cam.world_to_viewport(cam_tf, world);
        let Some(screen) = screen
            .ok()
            .filter(|_| alive && in_front && dist <= PLATE_RANGE)
        else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        vis.set_if_neq(Visibility::Inherited);
        node.left = px(screen.x / scale - PLATE_W / 2.0);
        node.bottom = px((window.height() - screen.y) / scale);
        let targeted = target.0 == Some(plate.anchor);
        // Far plates shrink so a whole pack stays readable; the target stands out.
        let k = (1.1 - dist / 60.0).clamp(0.6, 1.0) * if targeted { 1.15 } else { 1.0 };
        tf_ui.scale = Vec2::splat(k);
    }
    for (fill, mut node) in &mut fills {
        let pct = sim
            .0
            .unit(fill.0)
            .map_or(0.0, |u| 100.0 * u.hp as f32 / u.max_hp.max(1) as f32);
        let w = percent(pct);
        if node.width != w {
            node.width = w;
        }
    }
}
