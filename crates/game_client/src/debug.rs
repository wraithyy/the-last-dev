//! Env-gated QA hooks, inert unless the variables are set:
//! - `WOW_SCREENSHOT=<path.png>`: save the window after `WOW_SHOT_DELAY` seconds (default 4) and exit.
//! - `WOW_PLAYER_AT=x,z,yaw_deg`: teleport the player at startup to inspect other parts of the arena.
//! - `WOW_AUTORUN=1`: hold "forward" so animation and camera can be checked without input.

use crate::input::PlayerInput;
use crate::player::PlayerBody;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::env;

pub struct DebugPlugin;

#[derive(Resource)]
struct ShotPlan {
    path: String,
    /// Offscreen target: an occluded macOS window never presents, so a window capture is black.
    target: Handle<Image>,
    at: f32,
    taken: bool,
}

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(path) = env::var("WOW_SCREENSHOT") {
            let at = env::var("WOW_SHOT_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4.0);
            let target =
                app.world_mut()
                    .resource_mut::<Assets<Image>>()
                    .add(Image::new_target_texture(
                        1280,
                        720,
                        TextureFormat::Rgba8UnormSrgb,
                        None,
                    ));
            app.insert_resource(ShotPlan {
                path,
                target,
                at,
                taken: false,
            })
            .add_systems(Update, (retarget_camera, screenshot_and_exit));
        }
        if let Ok(v) = env::var("WOW_PLAYER_AT") {
            let n: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            if let [x, z, yaw] = n[..] {
                app.add_systems(PostStartup, move |mut body: Single<&mut PlayerBody>| {
                    body.pos.x = x;
                    body.pos.z = z;
                    body.yaw = yaw.to_radians();
                });
            }
        }
        if env::var("WOW_AUTORUN").is_ok() {
            // Fixed pre-update runs after the per-frame input snapshot and before the sim consumes it.
            app.add_systems(FixedPreUpdate, |mut input: ResMut<PlayerInput>| {
                input.forward = 1.0;
            });
        }
    }
}

fn screenshot_and_exit(
    mut commands: Commands,
    time: Res<Time>,
    mut plan: ResMut<ShotPlan>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs();
    if !plan.taken && t >= plan.at {
        plan.taken = true;
        commands
            .spawn(Screenshot::image(plan.target.clone()))
            .observe(save_to_disk(plan.path.clone()));
    } else if plan.taken && t >= plan.at + 2.0 {
        exit.write(AppExit::Success);
    }
}

fn retarget_camera(
    mut commands: Commands,
    plan: Res<ShotPlan>,
    cams: Query<Entity, Added<Camera3d>>,
) {
    for cam in &cams {
        commands
            .entity(cam)
            .insert(RenderTarget::Image(plan.target.clone().into()));
    }
}
