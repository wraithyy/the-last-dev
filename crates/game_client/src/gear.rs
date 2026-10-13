//! Player-only accessories, parented to skeleton bones so they follow every
//! animation: over-ear headphones resting on the neck and a coffee mug in the
//! left fist. Offsets are in bone space (model units; the model root is scaled).

use crate::player::Player;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

const NECK_BONE: &str = "Neck";
const HAND_BONE: &str = "Fist.L";

#[derive(Component)]
struct GearAttached;

pub struct GearPlugin;

impl Plugin for GearPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, attach_gear);
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
    let white = mat(Color::srgb(0.92, 0.93, 0.95));
    let blue = mat(Color::srgb(0.2, 0.45, 0.9));
    let band = meshes.add(Torus::new(0.3, 0.345));
    let cup = meshes.add(Cylinder::new(0.17, 0.1));
    let pad = meshes.add(Cylinder::new(0.13, 0.04));
    let mug = meshes.add(Cylinder::new(0.15, 0.34));
    let stripe = meshes.add(Cylinder::new(0.155, 0.08));
    let handle = meshes.add(Torus::new(0.022, 0.085));

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
    commands.entity(hand).with_children(|c| {
        // Bone +Y runs down the fingers; the mug hangs in front of the fist by its handle.
        c.spawn((
            Mesh3d(mug),
            MeshMaterial3d(white),
            Transform::from_xyz(0.0, 0.12, 0.24),
            NotShadowCaster,
        ))
        .with_children(|m| {
            m.spawn((
                Mesh3d(stripe),
                MeshMaterial3d(blue.clone()),
                Transform::from_xyz(0.0, 0.05, 0.0),
            ));
            m.spawn((
                Mesh3d(handle),
                MeshMaterial3d(blue),
                Transform::from_xyz(0.0, 0.0, -0.17)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ));
        });
    });
}
