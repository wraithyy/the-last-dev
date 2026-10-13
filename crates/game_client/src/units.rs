//! NPCs. Models and silhouettes differ so hostile vs friendly reads at a glance.

use crate::anim::{Character, ModelKind};
use crate::config::{CEO_HEIGHT, INTERN_HEIGHT, MODEL_NATIVE_HEIGHT};

use bevy::prelude::*;
use std::f32::consts::PI;

#[derive(Component)]
pub struct Unit {
    /// Sim unit key; separate from the display name so a rename cannot break the sim link.
    pub key: &'static str,
    pub name: &'static str,
    pub level: u32,
    pub hostile: bool,
    pub alive: bool,
}

pub struct UnitsPlugin;

#[derive(Component)]
struct Halo;

impl Plugin for UnitsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_ceo, spawn_intern))
            .add_systems(Update, hide_dead_halos);
    }
}

const CEO_SCALE: f32 = CEO_HEIGHT / MODEL_NATIVE_HEIGHT;
const INTERN_SCALE: f32 = INTERN_HEIGHT / MODEL_NATIVE_HEIGHT;

fn spawn_ceo(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let halo = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(5.0, 3.5, 0.4),
        ..default()
    });

    // Turned half way round so it faces the player spawn (+Z).
    commands
        .spawn((
            Unit {
                key: "ceo",
                name: "CEO",
                level: 3,
                hostile: true,
                alive: true,
            },
            Character {
                kind: ModelKind::Ceo,
                scale: CEO_SCALE,
            },
            Transform::from_xyz(0.0, 0.0, -8.0).with_rotation(Quat::from_rotation_y(PI)),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(Torus::new(0.2, 0.3))),
                MeshMaterial3d(halo),
                Transform::from_xyz(0.0, CEO_HEIGHT + 0.3, 0.0),
                Halo,
            ));
        });
}

fn spawn_intern(mut commands: Commands) {
    commands.spawn((
        Unit {
            key: "intern",
            name: "AI-Native Intern",
            level: 1,
            hostile: false,
            alive: true,
        },
        Character {
            kind: ModelKind::Intern,
            scale: INTERN_SCALE,
        },
        Transform::from_xyz(-6.0, 0.0, 8.0),
        Visibility::default(),
    ));
}

// The halo is a separate mesh, so the death animation would leave it floating.
fn hide_dead_halos(units: Query<&Unit>, mut halos: Query<(&ChildOf, &mut Visibility), With<Halo>>) {
    for (child_of, mut vis) in &mut halos {
        let alive = units.get(child_of.parent()).is_ok_and(|u| u.alive);
        vis.set_if_neq(if alive {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}
