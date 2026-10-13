//! Env-gated QA hooks, inert unless the variables are set:
//! - `WOW_SCREENSHOT=<path.png>`: save the window after `WOW_SHOT_DELAY` seconds (default 4) and exit.
//!   `WOW_SHOT_TIMES=t1,t2,..` instead saves one `<path>_<n>.png` per time (seconds since start).
//! - `WOW_LINEUP=1|2`: bare stage with half of the cast in two rows (see `units.rs`); `0` leaves the stage empty.
//! - `WOW_HOVER_SLOT=<0-9>`: show that action slot's tooltip without a pointer.
//!
//! Floor, level and start-position hooks live in `game.rs`, the autofight bot in `combat.rs`.
//! - `WOW_TALK=1`: talk to the nearest friendly NPC (within reach) two seconds in.
//! - `WOW_CAM_DIST=<m>`: start with the camera zoomed to that distance; `WOW_CAM_PITCH=<deg>` tilts it.
//! - `WOW_GIVE_ITEMS=1`: grant a sample set of trophies at start (stacks included).
//! - `WOW_TROPHIES=1`: open the `~/trophies` window at start.
//! - `WOW_LOOT_DEMO=1`: show the loot window and log lines for three sample items (display only; see `hud/feed.rs`).
//! - `WOW_COFFEE=<secs>`: drain mana to 30 and start a Coffee Break that many seconds in (1 if not a number).
//! - `WOW_AUTORUN=1`: hold "forward" so animation and camera can be checked without input.

use crate::camera::OrbitCamera;
use crate::combat::{CombatSim, PendingCommands, SimReady};
use crate::input::{PlayerInput, TrophiesOpen};
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
    /// Extra shots (see `WOW_SHOT_TIMES`); the plan ends after the last one.
    times: Vec<f32>,
    next: usize,
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
            let times: Vec<f32> = env::var("WOW_SHOT_TIMES")
                .map(|v| v.split(',').filter_map(|p| p.trim().parse().ok()).collect())
                .unwrap_or_default();
            app.insert_resource(ShotPlan {
                path,
                target,
                at,
                taken: false,
                times,
                next: 0,
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
        if let Some(deg) = env::var("WOW_CAM_PITCH")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            app.add_systems(PostStartup, move |mut cam: Single<&mut OrbitCamera>| {
                cam.set_pitch(deg.to_radians());
            });
        }
        if let Some(deg) = env::var("WOW_CAM_YAW_OFFSET")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            // After the floor load has set the start heading.
            app.add_systems(
                Update,
                move |time: Res<Time>, mut cam: Single<&mut OrbitCamera>, mut done: Local<bool>| {
                    if !*done && time.elapsed_secs() > 3.0 {
                        *done = true;
                        cam.add_yaw(deg.to_radians());
                    }
                },
            );
        }
        if let Some(slot) = env::var("WOW_HOVER_SLOT").ok().and_then(|v| v.parse().ok()) {
            app.insert_resource(crate::hud::DebugHoverSlot(slot));
        }
        if env::var("WOW_GIVE_ITEMS").is_ok() {
            app.add_systems(Update, give_items_once.in_set(SimReady));
        }
        if let Ok(v) = env::var("WOW_COFFEE") {
            let at = v.parse().unwrap_or(1.0);
            app.add_systems(
                Update,
                (move |time: Res<Time>,
                       sim: ResMut<CombatSim>,
                       pending: ResMut<PendingCommands>,
                       done: Local<bool>| {
                    coffee_once(at, time, sim, pending, done)
                })
                .in_set(SimReady),
            );
        }
        if env::var("WOW_TROPHIES").is_ok() {
            app.insert_resource(TrophiesOpen(true));
        }
        if env::var("WOW_AUTORUN").is_ok() {
            // Fixed pre-update runs after the per-frame input snapshot and before the sim consumes it.
            app.add_systems(FixedPreUpdate, |mut input: ResMut<PlayerInput>| {
                input.forward = 1.0;
            });
        }
    }
}

/// Cap (3) is hit by the invites and exceeded by the deck, so every badge style shows up.
const SAMPLE_TROPHIES: [&str; 13] = [
    "Unread AI Strategy Deck",
    "Unread AI Strategy Deck",
    "Prompt Fragment",
    "Expired GPU Credits",
    "Mandatory Prompt Training Certificate",
    "Recurring Meeting Invite",
    "Recurring Meeting Invite",
    "Recurring Meeting Invite",
    "Sticky Note with Admin Password",
    "Org Chart (Already Outdated)",
    "Pizza Party Voucher (Expired)",
    "Logo, Slightly Rounder",
    "Lanyard of Authority",
];

fn give_items_once(mut sim: ResMut<CombatSim>, mut done: Local<bool>) {
    if !*done {
        *done = true;
        for name in SAMPLE_TROPHIES {
            sim.0.grant_trophy(name);
        }
    }
}

fn coffee_once(
    at: f32,
    time: Res<Time>,
    mut sim: ResMut<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    mut done: Local<bool>,
) {
    if *done || time.elapsed_secs() < at {
        return;
    }
    *done = true;
    sim.0.set_player_mana(30);
    if let Some(me) = sim.0.player() {
        crate::combat::cast_coffee_break(&sim.0, &mut pending, me);
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
    if !plan.times.is_empty() {
        if let Some(&at) = plan.times.get(plan.next)
            && t >= at
        {
            let path = match plan.path.rsplit_once('.') {
                Some((stem, ext)) => format!("{stem}_{}.{ext}", plan.next),
                None => format!("{}_{}", plan.path, plan.next),
            };
            commands
                .spawn(Screenshot::image(plan.target.clone()))
                .observe(save_to_disk(path));
            plan.next += 1;
        } else if plan.next >= plan.times.len() && t >= plan.times[plan.times.len() - 1] + 2.0 {
            exit.write(AppExit::Success);
        }
        return;
    }
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
