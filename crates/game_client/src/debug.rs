//! Env-gated QA hooks, inert unless the variables are set:
//! - `WOW_SCREENSHOT=<path.png>`: save the window after `WOW_SHOT_DELAY` seconds (default 4) and exit.
//! - `WOW_LINEUP=1|2`: bare stage with half of the cast in two rows (see `units.rs`).
//! - `WOW_HOVER_SLOT=<0-9>`: show that action slot's tooltip without a pointer.
//!
//! Floor, level and start-position hooks live in `game.rs`, the autofight bot in `combat.rs`.
//! - `WOW_TALK=1`: talk to the nearest friendly NPC (within reach) two seconds in.
//! - `WOW_CAM_DIST=<m>`: start with the camera zoomed to that distance.
//! - `WOW_AUTORUN=1`: hold "forward" so animation and camera can be checked without input.

use crate::camera::OrbitCamera;
use crate::combat::{CombatSim, PendingCommands, SimReady};
use crate::input::PlayerInput;
use crate::player::PlayerBody;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use combat_core::{Command, distance};
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
        if env::var("WOW_LINEUP").is_ok() {
            app.add_systems(
                PostStartup,
                |mut body: Single<&mut PlayerBody>, mut cam: Single<&mut OrbitCamera>| {
                    body.teleport_facing(Vec3::new(0.0, 0.0, 2.0), 0.0);
                    cam.set_yaw(0.0);
                },
            );
        }
        if env::var("WOW_TALK").is_ok() {
            app.add_systems(Update, talk_once.in_set(SimReady));
        }
        if let Some(dist) = env::var("WOW_CAM_DIST").ok().and_then(|v| v.parse().ok()) {
            app.add_systems(PostStartup, move |mut cam: Single<&mut OrbitCamera>| {
                cam.set_zoom(dist);
            });
        }
        if let Some(slot) = env::var("WOW_HOVER_SLOT").ok().and_then(|v| v.parse().ok()) {
            app.insert_resource(crate::hud::DebugHoverSlot(slot));
        }
        if env::var("WOW_AUTORUN").is_ok() {
            // Fixed pre-update runs after the per-frame input snapshot and before the sim consumes it.
            app.add_systems(FixedPreUpdate, |mut input: ResMut<PlayerInput>| {
                input.forward = 1.0;
            });
        }
    }
}

fn talk_once(
    time: Res<Time>,
    sim: Res<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    mut done: Local<bool>,
) {
    if *done || time.elapsed_secs() < 2.0 {
        return;
    }
    *done = true;
    let Some(me) = sim.0.player().and_then(|id| sim.0.unit(id)) else {
        return;
    };
    let reach = sim.0.content().config.loot_range_m;
    let dist = |u: &combat_core::UnitState| distance(u.pos, me.pos);
    if let Some(npc) = sim
        .0
        .units()
        .filter(|u| !u.hostile && u.alive && u.id != me.id && dist(u) <= reach)
        .min_by(|a, b| dist(a).total_cmp(&dist(b)))
    {
        pending.0.push(Command::Talk { unit: npc.id });
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
            // Without this marker the UI would find no camera aimed at the window and vanish from the shot.
            .insert((
                RenderTarget::Image(plan.target.clone().into()),
                IsDefaultUiCamera,
            ));
    }
}
