//! Target selection (Tab, click, Escape) and the data the target frame shows.

use crate::arena::Colliders;
use crate::collision::to_core;
use crate::combat::CombatSim;
use crate::config::*;
use crate::input::{MouseState, Paused};
use crate::player::{Player, PlayerBody};
use crate::units::Unit;
use bevy::prelude::*;
use combat_core::{
    Aabb, EntityId, TAB_TARGET_RANGE, TargetCandidate, TargetError, line_of_sight, tab_target,
    validate_target,
};
use std::f32::consts::FRAC_PI_2;

#[derive(Resource, Default)]
pub struct Target(pub Option<Entity>);

/// Geometry checks against the current target, recomputed every frame for the HUD.
pub struct TargetReadout {
    pub spell_range: f32,
    pub distance: f32,
    pub in_range: bool,
    pub facing: bool,
    pub line_of_sight: bool,
}

#[derive(Resource, Default)]
pub struct TargetInfo(pub Option<TargetReadout>);

#[derive(Component)]
struct TargetRing {
    hostile: Handle<StandardMaterial>,
    friendly: Handle<StandardMaterial>,
}

pub struct TargetingPlugin;

impl Plugin for TargetingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Target>()
            .init_resource::<TargetInfo>()
            .add_systems(Startup, spawn_ring)
            .add_systems(
                Update,
                (
                    drop_invalid_target,
                    handle_keys,
                    click_select,
                    update_readout,
                    update_ring,
                )
                    .chain(),
            );
    }
}

fn spawn_ring(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut ring_material = |color: Color| {
        materials.add(StandardMaterial {
            // Unlit ignores emissive, so the colour goes in base_color.
            base_color: color,
            unlit: true,
            cull_mode: None,
            ..default()
        })
    };
    let hostile = ring_material(Color::srgb(1.0, 0.05, 0.05));
    let friendly = ring_material(Color::srgb(0.2, 1.0, 0.3));
    commands.spawn((
        TargetRing {
            hostile: hostile.clone(),
            friendly,
        },
        Mesh3d(meshes.add(Annulus::new(RING_INNER, RING_OUTER))),
        MeshMaterial3d(hostile),
        Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        Visibility::Hidden,
    ));
}

/// A despawned target is dropped, not left dangling. Dead targets stay
/// selected on purpose: the corpse is what you loot.
fn drop_invalid_target(mut target: ResMut<Target>, units: Query<&Unit>) {
    if let Some(e) = target.0
        && !units.contains(e)
    {
        target.0 = None;
    }
}

fn handle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
    mut target: ResMut<Target>,
    player: Single<&PlayerBody, With<Player>>,
    units: Query<(Entity, &Transform, &Unit)>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        if target.0.take().is_none() {
            paused.0 = !paused.0;
        }
        return;
    }
    if paused.0 || !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    let candidates: Vec<TargetCandidate> = units
        .iter()
        .map(|(e, tf, u)| TargetCandidate {
            id: e.to_bits(),
            pos: to_core(tf.translation),
            hostile: u.hostile,
            alive: u.alive,
        })
        .collect();
    let current: Option<EntityId> = target.0.map(Entity::to_bits);
    let picked = tab_target(
        to_core(player.pos),
        player.yaw,
        current,
        &candidates,
        TAB_TARGET_RANGE,
    );
    if let Some(id) = picked {
        target.0 = Some(Entity::from_bits(id));
    }
}

/// Own ray-vs-sphere picking: no render-backend dependency, so it behaves
/// the same on native and WebGL2. Corpses are pickable (for looting).
pub fn pick_unit(
    cursor: Vec2,
    camera: (&Camera, &GlobalTransform),
    colliders: &[Aabb],
    units: &Query<(Entity, &Transform, &Unit)>,
) -> Option<Entity> {
    let (cam, cam_tf) = camera;
    let ray = cam.viewport_to_world(cam_tf, cursor).ok()?;
    let (t, e) = units
        .iter()
        .filter_map(|(e, tf, _)| {
            let center = tf.translation + Vec3::Y * PICK_SPHERE_HEIGHT;
            ray_sphere(ray.origin, *ray.direction, center, PICK_SPHERE_RADIUS).map(|t| (t, e))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))?;
    line_of_sight(
        to_core(ray.origin),
        to_core(ray.origin + *ray.direction * t),
        colliders,
    )
    .then_some(e)
}

fn click_select(
    mouse: Res<MouseState>,
    paused: Res<Paused>,
    colliders: Res<Colliders>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    units: Query<(Entity, &Transform, &Unit)>,
    ui: Query<&Interaction>,
    mut target: ResMut<Target>,
) {
    let Some(cursor) = mouse.click else { return };
    // A click on a HUD button must not also pick the unit behind it.
    if paused.0 || ui.iter().any(|i| *i != Interaction::None) {
        return;
    }
    if let Some(e) = pick_unit(cursor, *camera, &colliders.0, &units) {
        target.0 = Some(e);
    }
}

fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = center - origin;
    let along = oc.dot(dir);
    let d2 = oc.length_squared() - along * along;
    let r2 = radius * radius;
    if along < 0.0 || d2 > r2 {
        return None;
    }
    Some(along - (r2 - d2).sqrt())
}

fn update_readout(
    sim: Option<Res<CombatSim>>,
    target: Res<Target>,
    colliders: Res<Colliders>,
    player: Single<&PlayerBody, With<Player>>,
    units: Query<(&Transform, &Unit)>,
    mut info: ResMut<TargetInfo>,
) {
    // Dev overlay only: range is taken from the first action-bar spell.
    let spell_range = sim
        .and_then(|s| {
            let id = *s.0.content().action_bar().first()?;
            s.0.content().spell(id).map(|d| d.range_m)
        })
        .unwrap_or(0.0);
    info.0 = target.0.and_then(|e| units.get(e).ok()).map(|(tf, u)| {
        let eye = player.pos + Vec3::Y * PLAYER_EYE;
        let chest = TargetCandidate {
            id: 0,
            pos: to_core(tf.translation + Vec3::Y * PICK_SPHERE_HEIGHT),
            hostile: u.hostile,
            alive: u.alive,
        };
        // Each chip isolates one rule by relaxing the other two, so the HUD
        // always agrees with what spell validation will say in Phase 2.
        let check = |range: f32, facing: bool, blockers: &[Aabb]| {
            validate_target(
                to_core(eye),
                player.yaw,
                Some(&chest),
                range,
                facing,
                blockers,
            )
        };
        let failed = |err: TargetError, result: Result<(), TargetError>| result == Err(err);
        TargetReadout {
            spell_range,
            distance: player.pos.distance(tf.translation),
            in_range: !failed(TargetError::OutOfRange, check(spell_range, false, &[])),
            facing: !failed(TargetError::NotFacing, check(f32::INFINITY, true, &[])),
            line_of_sight: !failed(
                TargetError::NoLineOfSight,
                check(f32::INFINITY, false, &colliders.0),
            ),
        }
    });
}

fn update_ring(
    target: Res<Target>,
    units: Query<(&Transform, &Unit), Without<TargetRing>>,
    ring: Single<(
        &TargetRing,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let (data, mut tf, mut vis, mut mat) = ring.into_inner();
    let Some((unit_tf, unit)) = target.0.and_then(|e| units.get(e).ok()) else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    // Intern is spawned with its centre at capsule height; the ring belongs on the floor.
    tf.translation = Vec3::new(unit_tf.translation.x, RING_HEIGHT, unit_tf.translation.z);
    mat.0 = if unit.hostile {
        data.hostile.clone()
    } else {
        data.friendly.clone()
    };
}
