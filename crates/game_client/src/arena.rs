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

/// One flat piece of floor decor (rug, carpet runner, mat): a corpse lying on it must rest on
/// its top face instead of sinking into it.
#[derive(Clone, Copy, Debug)]
pub struct DecorRect {
    pub center: Vec2,
    pub half: Vec2,
    pub yaw: f32,
    pub top: f32,
    pub round: bool,
}

impl DecorRect {
    fn covers(&self, p: Vec2) -> bool {
        let d = p - self.center;
        if self.round {
            return (d / self.half).length_squared() <= 1.0;
        }
        // Slab yaw is `Quat::from_rotation_y`: local x maps to (cos, -sin) on the (x, z) plane.
        let (s, c) = self.yaw.sin_cos();
        let local = Vec2::new(d.x * c - d.y * s, d.x * s + d.y * c);
        local.x.abs() <= self.half.x && local.y.abs() <= self.half.y
    }
}

/// Flat decor of the loaded floor, filled by `floors::load_floor`.
#[derive(Resource, Default)]
pub struct FlatDecor(pub Vec<DecorRect>);

impl FlatDecor {
    /// Height of the highest decor under a body of radius `r` around `at` (0 on bare floor).
    /// Five samples cover a lying body that is only half on a rug.
    pub fn top_near(&self, at: Vec3, r: f32) -> f32 {
        let c = Vec2::new(at.x, at.z);
        [Vec2::ZERO, Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y]
            .into_iter()
            .map(|o| c + o * r)
            .flat_map(|p| self.0.iter().filter(move |d| d.covers(p)).map(|d| d.top))
            .fold(0.0, f32::max)
    }
}

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
            .init_resource::<FlatDecor>()
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
