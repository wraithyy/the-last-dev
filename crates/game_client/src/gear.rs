//! Player-only accessories, parented to skeleton bones so they follow every
//! animation: over-ear headphones resting on the neck and a coffee mug in the
//! left fist. Offsets are in bone space (model units; the model root is scaled).

use crate::player::Player;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

const NECK_BONE: &str = "Neck";
const HAND_BONE: &str = "Fist.L";
/// Yaw of the mug relative to the player, so the handle faces the camera side.
/// Mug origin in bone space before the lift.
const MUG_GRIP: Vec3 = Vec3::new(0.0, 0.12, 0.24);
const MUG_YAW: f32 = 3.0 * std::f32::consts::FRAC_PI_4;
/// Where the mug sits relative to the fist, in player space (metres): above it and a bit ahead.
const MUG_LIFT: Vec3 = Vec3::new(0.0, 0.15, -0.05);

#[derive(Component)]
struct GearAttached;

pub struct GearPlugin;

impl Plugin for GearPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (attach_gear, animate_steam))
            .add_systems(
                PostUpdate,
                keep_mug_upright.before(TransformSystems::Propagate),
            );
    }
}

/// The glTF scene spawns asynchronously, so keep looking until both bones exist.
fn attach_gear(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Single<Entity, (With<Player>, Without<GearAttached>)>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    let bone = |wanted: &str| {
        children
            .iter_descendants(*player)
            .find(|e| names.get(*e).is_ok_and(|n| n.as_str() == wanted))
    };
    let (Some(neck), Some(hand)) = (bone(NECK_BONE), bone(HAND_BONE)) else {
        return;
    };
    commands.entity(*player).insert(GearAttached);
    let mut mat = |color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.6,
            ..default()
        })
    };
    let dark = mat(Color::srgb(0.32, 0.33, 0.37));
    let cushion = mat(Color::srgb(0.1, 0.1, 0.12));
    let band = meshes.add(Torus::new(0.3, 0.345));
    let cup = meshes.add(Cylinder::new(0.17, 0.1));
    let pad = meshes.add(Cylinder::new(0.13, 0.04));

    commands.entity(neck).with_children(|c| {
        // Band round the back of the neck, cups hanging at the collarbones.
        c.spawn((
            Mesh3d(band),
            MeshMaterial3d(dark.clone()),
            Transform::from_xyz(0.0, -0.1, 0.0).with_rotation(Quat::from_rotation_x(0.35)),
            NotShadowCaster,
        ));
        for side in [-1.0, 1.0] {
            c.spawn((
                Mesh3d(cup.clone()),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(side * 0.33, -0.16, 0.12).with_rotation(Quat::from_rotation_z(
                    std::f32::consts::FRAC_PI_2 - side * 0.3,
                )),
                NotShadowCaster,
            ))
            .with_child((
                Mesh3d(pad.clone()),
                MeshMaterial3d(cushion.clone()),
                Transform::from_xyz(0.0, -side * 0.065, 0.0),
            ));
        }
    });
    spawn_mug(&mut commands, &mut meshes, &mut materials, hand);
}

/// Keeps the mug upright in the world whatever the fist bone does.
#[derive(Component)]
struct MugHolder;

#[derive(Component)]
struct MugSteam(f32);

/// Mug local frame: +Y up, handle toward +X, origin at the body centre.
fn spawn_mug(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    hand: Entity,
) {
    let mut lit = |color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.4,
            ..default()
        })
    };
    let navy = lit(Color::srgb(0.1, 0.17, 0.5));
    let stripe_mat = lit(Color::srgb(0.95, 0.75, 0.15));
    let logo_mat = lit(Color::srgb(0.95, 0.95, 0.98));
    let coffee = lit(Color::srgb(0.12, 0.06, 0.03));
    let rim_mat = lit(Color::srgb(0.85, 0.87, 0.95));
    let steam_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.35),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let body = meshes.add(ConicalFrustum {
        radius_top: 0.2,
        radius_bottom: 0.17,
        height: 0.42,
    });
    let band = meshes.add(ConicalFrustum {
        radius_top: 0.1895,
        radius_bottom: 0.1845,
        height: 0.06,
    });
    let disc = meshes.add(Cylinder::new(0.188, 0.01));
    let rim = meshes.add(Torus::new(0.19, 0.21));
    let handle = meshes.add(Torus::new(0.05, 0.11));
    let logo = meshes.add(Cuboid::new(0.07, 0.07, 0.02));
    let puff = meshes.add(Sphere::new(1.0));

    commands.entity(hand).with_children(|c| {
        c.spawn((
            MugHolder,
            Transform::from_translation(MUG_GRIP),
            Visibility::default(),
        ))
        .with_children(|m| {
            let mut put = |mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>, t: Transform| {
                m.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mat.clone()),
                    t,
                    NotShadowCaster,
                ));
            };
            put(&body, &navy, Transform::IDENTITY);
            put(&band, &stripe_mat, Transform::from_xyz(0.0, -0.06, 0.0));
            // Small logo plate on the side facing away from the handle.
            put(&logo, &logo_mat, Transform::from_xyz(0.0, 0.04, -0.19));
            put(&disc, &coffee, Transform::from_xyz(0.0, 0.175, 0.0));
            put(&rim, &rim_mat, Transform::from_xyz(0.0, 0.21, 0.0));
            // The torus ring lies in XZ; stand it up in XY so it reads as a side handle.
            put(
                &handle,
                &navy,
                Transform::from_xyz(0.25, 0.02, 0.0)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                    .with_scale(Vec3::new(0.9, 0.9, 1.3)),
            );
            for i in 0..3 {
                m.spawn((
                    MugSteam(i as f32 / 3.0),
                    Mesh3d(puff.clone()),
                    MeshMaterial3d(steam_mat.clone()),
                    Transform::from_xyz(0.0, 0.3, 0.0).with_scale(Vec3::ZERO),
                    NotShadowCaster,
                ));
            }
        });
    });
}

/// The fist bone tumbles with every animation; counter-rotate so the coffee never spills.
fn keep_mug_upright(
    player: Single<&GlobalTransform, With<Player>>,
    globals: Query<&GlobalTransform>,
    mut mugs: Query<(&ChildOf, &mut Transform), With<MugHolder>>,
) {
    let world = player.rotation() * Quat::from_rotation_y(MUG_YAW);
    for (parent, mut tf) in &mut mugs {
        if let Ok(g) = globals.get(parent.parent()) {
            tf.rotation = g.rotation().inverse() * world;
            let lift = player.rotation() * MUG_LIFT;
            tf.translation = MUG_GRIP + g.affine().inverse().transform_vector3(lift);
        }
    }
}

/// Occasional faint wisps rising from the coffee.
fn animate_steam(time: Res<Time>, mut puffs: Query<(&MugSteam, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (s, mut tf) in &mut puffs {
        let k = (t * 0.4 + s.0).fract();
        let active = ((t * 0.3 + s.0 * 2.0).sin() > -0.2) as u8 as f32;
        let rise = (k * std::f32::consts::PI).sin();
        tf.translation = Vec3::new(
            0.04 * (t * 2.0 + s.0 * 6.0).sin(),
            0.26 + 0.45 * k,
            0.04 * (t * 1.7 + s.0 * 4.0).cos(),
        );
        tf.scale = Vec3::splat(0.045 * rise * active);
    }
}
