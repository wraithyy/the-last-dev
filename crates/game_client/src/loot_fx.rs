//! Lootable-corpse feedback: rising golden sparkles, a gold nameplate with a
//! LOOT tag, and the hover cursor (crosshair / grab / pointer).
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::too_many_arguments)]

use crate::arena::Colliders;
use crate::combat::{CombatEvent, SimRead, SimReady, SimUnit, SimView};
use crate::input::{MouseState, Paused};
use crate::nameplates::{PlateLootTag, PlateName};
use crate::player::{Player, PlayerBody};
use crate::targeting::pick_unit;
use crate::units::Unit;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, SystemCursorIcon};
use combat_core::{EntityId, Event};
use std::collections::HashSet;

pub const GOLD: Color = Color::srgb(1.0, 0.82, 0.25);
const SPARKLE_CAP: usize = 40;
const SPARKLE_RANGE: f32 = 35.0;
const SPARKLE_EVERY: f32 = 0.14;
const SPARKLE_LIFE: f32 = 1.3;
/// Re-pick at least this often even with a still cursor: units walk under it.
const PICK_EVERY: f32 = 0.15;

/// Corpses whose drop was empty: nothing to show, though the sim still calls them lootable.
#[derive(Resource, Default)]
struct EmptyCorpses(HashSet<EntityId>);

#[derive(Resource)]
struct SparkleAssets {
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
}

#[derive(Component)]
struct Sparkle {
    corpse: EntityId,
    origin: Vec3,
    age: f32,
    phase: f32,
}

pub struct LootFxPlugin;

impl Plugin for LootFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EmptyCorpses>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    track_empty,
                    spawn_sparkles,
                    update_sparkles,
                    mark_plates,
                    hover_cursor,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(SparkleAssets {
        mesh: meshes.add(Sphere::new(0.5)),
        mat: mats.add(StandardMaterial {
            // Unlit ignores emissive, so the colour goes in base_color.
            base_color: GOLD,
            unlit: true,
            ..default()
        }),
    });
}

fn track_empty(
    mut events: MessageReader<CombatEvent>,
    sim: SimView,
    mut empty: ResMut<EmptyCorpses>,
) {
    for CombatEvent(e) in events.read() {
        if let Event::LootDropped { corpse, items, sp } = e
            && items.is_empty()
            && *sp == 0
        {
            empty.0.insert(*corpse);
        }
    }
    empty
        .0
        .retain(|id| sim.unit(*id).is_some_and(|u| u.lootable));
}

fn has_loot(sim: &dyn SimRead, empty: &EmptyCorpses, id: EntityId) -> bool {
    !empty.0.contains(&id) && sim.unit(id).is_some_and(|u| u.lootable)
}

fn spawn_sparkles(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<SparkleAssets>,
    sim: SimView,
    empty: Res<EmptyCorpses>,
    player: Single<&PlayerBody, With<Player>>,
    corpses: Query<(&Transform, &SimUnit), With<Unit>>,
    live: Query<(), With<Sparkle>>,
    mut clock: Local<(f32, u32)>,
) {
    clock.0 += time.delta_secs();
    if clock.0 < SPARKLE_EVERY {
        return;
    }
    clock.0 = 0.0;
    let mut room = SPARKLE_CAP.saturating_sub(live.iter().count());
    for (tf, su) in &corpses {
        if room == 0 {
            break;
        }
        if !has_loot(&*sim, &empty, su.0) || tf.translation.distance(player.pos) > SPARKLE_RANGE {
            continue;
        }
        clock.1 = clock.1.wrapping_add(1);
        // Golden-ratio phase spreads sparkles around the corpse without an RNG.
        let phase = clock.1 as f32 * 2.399_963;
        commands.spawn((
            Sparkle {
                corpse: su.0,
                origin: tf.translation + Vec3::Y * 0.2,
                age: 0.0,
                phase,
            },
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.mat.clone()),
            Transform::from_translation(tf.translation).with_scale(Vec3::ZERO),
        ));
        room -= 1;
    }
}

fn update_sparkles(
    mut commands: Commands,
    time: Res<Time>,
    sim: SimView,
    empty: Res<EmptyCorpses>,
    mut q: Query<(Entity, &mut Sparkle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut s, mut tf) in &mut q {
        s.age += dt;
        let k = s.age / SPARKLE_LIFE;
        if k >= 1.0 || !has_loot(&*sim, &empty, s.corpse) {
            commands.entity(e).despawn();
            continue;
        }
        let a = s.phase + s.age * 1.6;
        let r = 0.25 + 0.45 * (s.phase * 1.7).sin().abs();
        tf.translation = s.origin + Vec3::new(a.cos() * r, k * 1.9, a.sin() * r);
        // Scale envelope stands in for alpha fade so all sparkles share one material.
        let pulse = 0.75 + 0.25 * (s.age * 12.0 + s.phase).sin();
        tf.scale = Vec3::splat(0.09 * (k * std::f32::consts::PI).sin() * pulse);
    }
}

fn mark_plates(
    sim: SimView,
    empty: Res<EmptyCorpses>,
    mut names: Query<(&PlateName, &mut TextColor)>,
    mut tags: Query<(&PlateLootTag, &mut Node)>,
) {
    for (n, mut color) in &mut names {
        if has_loot(&*sim, &empty, n.0) {
            color.set_if_neq(TextColor(GOLD));
        }
    }
    for (t, mut node) in &mut tags {
        let d = if has_loot(&*sim, &empty, t.0) {
            Display::Flex
        } else {
            Display::None
        };
        node.set_if_neq(Node {
            display: d,
            ..node.clone()
        });
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hover {
    Nothing,
    Ui,
    Hostile,
    Friendly,
    Corpse { in_reach: bool },
}

fn cursor_for(h: Hover) -> SystemCursorIcon {
    match h {
        Hover::Nothing => SystemCursorIcon::Default,
        Hover::Ui | Hover::Friendly => SystemCursorIcon::Pointer,
        Hover::Hostile => SystemCursorIcon::Crosshair,
        Hover::Corpse { in_reach: true } => SystemCursorIcon::Grab,
        Hover::Corpse { in_reach: false } => SystemCursorIcon::NotAllowed,
    }
}

fn hover_cursor(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<MouseState>,
    paused: Res<Paused>,
    sim: SimView,
    empty: Res<EmptyCorpses>,
    colliders: Res<Colliders>,
    window: Single<(Entity, &Window), With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    player: Single<&PlayerBody, With<Player>>,
    ui: Query<&Interaction>,
    units: Query<(Entity, &Transform, &Unit)>,
    sims: Query<&SimUnit>,
    prop: Res<crate::office::PropHover>,
    mut state: Local<(Option<Vec2>, f32, Option<SystemCursorIcon>)>,
) {
    let (win_entity, win) = *window;
    // While steering the pointer is locked and hidden; leave the icon alone.
    if mouse.right || mouse.left {
        return;
    }
    let cursor = win.cursor_position();
    state.1 += time.delta_secs();
    if cursor == state.0 && state.1 < PICK_EVERY {
        return;
    }
    state.0 = cursor;
    state.1 = 0.0;
    let hover = match cursor {
        _ if paused.0 => Hover::Nothing,
        None => Hover::Nothing,
        Some(_) if ui.iter().any(|i| *i != Interaction::None) => Hover::Ui,
        Some(c) => pick_unit(c, *camera, &colliders.0, &units)
            .and_then(|e| {
                let (_, tf, unit) = units.get(e).ok()?;
                let id = sims.get(e).ok()?.0;
                Some(if unit.alive {
                    if unit.hostile {
                        Hover::Hostile
                    } else {
                        Hover::Friendly
                    }
                } else if has_loot(&*sim, &empty, id) {
                    let reach = sim.content().config.loot_range_m;
                    Hover::Corpse {
                        in_reach: tf.translation.distance(player.pos) <= reach,
                    }
                } else {
                    Hover::Nothing
                })
            })
            .unwrap_or(if prop.0.is_some() {
                Hover::Friendly
            } else {
                Hover::Nothing
            }),
    };
    let icon = cursor_for(hover);
    if state.2 != Some(icon) {
        state.2 = Some(icon);
        commands
            .entity(win_entity)
            .insert(bevy::window::CursorIcon::from(icon));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_maps_to_expected_cursor() {
        use SystemCursorIcon::*;
        assert_eq!(cursor_for(Hover::Nothing), Default);
        assert_eq!(cursor_for(Hover::Ui), Pointer);
        assert_eq!(cursor_for(Hover::Friendly), Pointer);
        assert_eq!(cursor_for(Hover::Hostile), Crosshair);
        assert_eq!(cursor_for(Hover::Corpse { in_reach: true }), Grab);
        assert_eq!(cursor_for(Hover::Corpse { in_reach: false }), NotAllowed);
    }
}
