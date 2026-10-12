//! NPCs. Models and silhouettes differ so hostile vs friendly reads at a glance.

use crate::anim::{Character, ModelKind};

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

#[derive(Component)]
struct Halo;

impl Plugin for UnitsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_evangelist, spawn_intern))
            .add_systems(Update, hide_dead_halos);
    }
}

const EVANGELIST_SCALE: f32 = 1.05;
const INTERN_SCALE: f32 = 0.75;

fn spawn_evangelist(
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
                name: "AI Evangelist",
                level: 3,
                hostile: true,
                alive: true,
            },
            Character {
                kind: ModelKind::Rogue,
                scale: EVANGELIST_SCALE,
                // Purple glow so the silhouette reads as hostile.
                tint: Some(LinearRgba::rgb(0.25, 0.04, 0.4)),
            },
            Transform::from_xyz(0.0, 0.0, -8.0).with_rotation(Quat::from_rotation_y(PI)),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(Torus::new(0.2, 0.3))),
                MeshMaterial3d(halo),
                Transform::from_xyz(0.0, 2.65, 0.0),
                Halo,
            ));
        });
}

fn spawn_intern(mut commands: Commands) {
    commands.spawn((
        Unit {
            name: "Intern",
            level: 1,
            hostile: false,
            alive: true,
        },
        Character {
            kind: ModelKind::Barbarian,
            scale: INTERN_SCALE,
            tint: None,
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
