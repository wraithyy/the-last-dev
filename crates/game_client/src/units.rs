//! Placeholder NPCs. Silhouettes differ so hostile vs friendly reads at a glance.

use bevy::prelude::*;
use std::f32::consts::PI;

#[derive(Component)]
pub struct Unit {
    pub name: &'static str,
    pub level: u32,
    pub hostile: bool,
    pub alive: bool,
}

pub struct UnitsPlugin;

impl Plugin for UnitsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_evangelist, spawn_intern));
    }
}

fn emissive(color: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::BLACK,
        emissive: color,
        ..default()
    }
}

fn spawn_evangelist(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let robe = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.15, 0.6),
        perceptual_roughness: 0.7,
        ..default()
    });
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.75, 0.7),
        ..default()
    });
    let halo = materials.add(emissive(LinearRgba::rgb(5.0, 3.5, 0.4)));
    let tablet = materials.add(emissive(LinearRgba::rgb(0.5, 2.0, 4.0)));

    // Turned half way round so it faces the player spawn (+Z).
    commands
        .spawn((
            Unit {
                name: "AI Evangelist",
                level: 3,
                hostile: true,
                alive: true,
            },
            Transform::from_xyz(0.0, 0.0, -8.0).with_rotation(Quat::from_rotation_y(PI)),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(Cone::new(0.7, 2.0))),
                MeshMaterial3d(robe),
                Transform::from_xyz(0.0, 1.0, 0.0),
            ));
            c.spawn((
                Mesh3d(meshes.add(Sphere::new(0.25))),
                MeshMaterial3d(skin),
                Transform::from_xyz(0.0, 2.2, 0.0),
            ));
            c.spawn((
                Mesh3d(meshes.add(Torus::new(0.22, 0.32))),
                MeshMaterial3d(halo),
                Transform::from_xyz(0.0, 2.75, 0.0),
            ));
            c.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.55, 0.75, 0.06))),
                MeshMaterial3d(tablet),
                Transform::from_xyz(0.9, 1.5, -0.3).with_rotation(Quat::from_rotation_y(-0.4)),
            ));
        });
}

fn spawn_intern(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands
        .spawn((
            Unit {
                name: "Intern",
                level: 1,
                hostile: false,
                alive: true,
            },
            Transform::from_xyz(-6.0, 0.0, 8.0),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(Capsule3d::new(0.3, 0.6))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.3, 0.7, 0.4),
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.6, 0.0),
            ));
        });
}
