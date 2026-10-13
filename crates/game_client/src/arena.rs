//! Shared world state: the flat blocker list every system queries (movement,
//! camera clipping, line of sight, picking), and the camera grade. The floors
//! module fills the list when it builds a floor.

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use combat_core::Aabb;

/// Flat list of every static blocker, for combat_core line-of-sight queries.
#[derive(Resource, Default)]
pub struct Colliders(pub Vec<Aabb>);

/// Highest the camera may go: just under the ceiling indoors, high above the campus outdoors.
#[derive(Resource)]
pub struct CameraCeiling(pub f32);

impl Default for CameraCeiling {
    fn default() -> Self {
        Self(crate::config::CAM_MAX_HEIGHT)
    }
}

const BACKDROP: Color = Color::srgb(0.03, 0.035, 0.06);

pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Colliders>()
            .init_resource::<CameraCeiling>()
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.7, 0.78, 1.0),
                brightness: 450.0,
                ..default()
            })
            .insert_resource(ClearColor(BACKDROP))
            .add_systems(Update, grade_camera);
    }
}

/// The camera is spawned by camera.rs; the look (tonemap, bloom, fog) is applied here.
fn grade_camera(mut commands: Commands, cams: Query<Entity, Added<Camera3d>>) {
    for cam in &cams {
        commands.entity(cam).insert((
            // AcesFitted needs no LUT texture, so it behaves the same on WebGL2.
            Tonemapping::AcesFitted,
            Bloom {
                intensity: 0.12,
                ..Bloom::NATURAL
            },
            DistanceFog {
                color: BACKDROP,
                falloff: FogFalloff::Linear {
                    start: 25.0,
                    end: 75.0,
                },
                ..default()
            },
        ));
    }
}
