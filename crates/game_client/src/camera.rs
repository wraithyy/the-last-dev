//! Third-person orbit camera: zoom, pitch clamp, snap-behind, wall clipping.

use crate::arena::Colliders;
use crate::collision::to_core;
use crate::config::*;
use crate::input::{MouseState, Paused, PlayerInput};
use crate::player::Player;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use combat_core::{Aabb, Vec3 as CVec3, line_of_sight};
use std::f32::consts::{PI, TAU};

#[derive(Component)]
pub struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    /// Distance the user asked for via the wheel.
    zoom: f32,
    /// Distance actually used after wall clipping and easing.
    dist: f32,
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, (follow_player, orbit_input, place_camera).chain());
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        OrbitCamera {
            yaw: 0.0,
            pitch: CAM_START_PITCH,
            zoom: CAM_START_DIST,
            dist: CAM_START_DIST,
        },
    ));
}

fn wrap_angle(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

/// Keyboard turning rotates the camera with the character; RMB steering is
/// applied to the camera directly in `orbit_input` so it never lags the mouse.
/// While walking forward with no button held the camera eases behind the character.
fn follow_player(
    time: Res<Time>,
    mouse: Res<MouseState>,
    mut input: ResMut<PlayerInput>,
    player: Single<&Transform, With<Player>>,
    mut cam: Single<&mut OrbitCamera>,
) {
    let dt = time.delta_secs();
    if mouse.right_just_pressed {
        // WoW: pressing RMB turns the character to face where the camera looks.
        input.face_yaw = Some(cam.yaw);
    }
    if !mouse.right {
        cam.yaw += input.turn * KEY_TURN_RATE * dt;
    }
    if input.forward > 0.0 && !mouse.any_button() {
        let player_yaw = player.rotation.to_euler(EulerRot::YXZ).0;
        let diff = wrap_angle(player_yaw - cam.yaw);
        cam.yaw += diff * (1.0 - (-CAM_SNAP_BEHIND_RATE * dt).exp());
    }
}

fn orbit_input(
    paused: Res<Paused>,
    mouse: Res<MouseState>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut cam: Single<&mut OrbitCamera>,
) {
    if paused.0 {
        return;
    }
    if mouse.any_button() {
        cam.pitch =
            (cam.pitch + motion.delta.y * CAM_ORBIT_SENS).clamp(-CAM_PITCH_LIMIT, CAM_PITCH_LIMIT);
    }
    // Same sign and yaw rate as the character's mouse yaw, which the sim
    // applies on the next tick, so camera and body stay in step.
    if mouse.right {
        cam.yaw -= motion.delta.x * MOUSE_YAW_SENS;
    } else if mouse.left {
        cam.yaw -= motion.delta.x * CAM_ORBIT_SENS;
    }

    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / CAM_ZOOM_PIXELS_PER_LINE,
    };
    cam.zoom = (cam.zoom - lines * CAM_ZOOM_STEP).clamp(CAM_MIN_DIST, CAM_MAX_DIST);
}

/// Largest distance along `dir` from `origin` (up to `max`) with clear sight.
/// Bisects on `line_of_sight`, which is monotonic along a growing segment, so
/// the camera reuses the same geometry code as combat.
fn clear_distance(origin: Vec3, dir: Vec3, max: f32, blockers: &[Aabb]) -> f32 {
    let clear = |d: f32| line_of_sight(to_core(origin), to_core(origin + dir * d), blockers);
    if clear(max) {
        return max;
    }
    let (mut lo, mut hi) = (0.0, max);
    for _ in 0..CAM_BISECT_STEPS {
        let mid = (lo + hi) / 2.0;
        if clear(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

fn place_camera(
    time: Res<Time>,
    colliders: Res<Colliders>,
    player: Single<&Transform, (With<Player>, Without<OrbitCamera>)>,
    cam: Single<(&mut OrbitCamera, &mut Transform)>,
) {
    let (mut cam, mut tf) = cam.into_inner();
    let focus = player.translation + Vec3::Y * CAM_FOCUS_HEIGHT;
    let dir = Quat::from_rotation_y(cam.yaw) * Vec3::new(0.0, cam.pitch.sin(), cam.pitch.cos());

    // Inflating the boxes by the camera radius keeps the near plane out of walls.
    let inflated: Vec<Aabb> = colliders.0.iter().map(|b| inflate(b, CAM_RADIUS)).collect();
    let mut free = clear_distance(focus, dir, cam.zoom, &inflated);
    // Looking up from below: shorten the arm so the camera stops at the floor
    // instead of being clamped in Y (which would bend the view direction).
    if dir.y < -1e-3 {
        free = free.min((CAM_MIN_HEIGHT - focus.y) / dir.y);
    }
    let target = free.max(CAM_MIN_CLIP_DIST);
    // Snap in when clipped (never show the wall), ease out when it clears.
    cam.dist = if target < cam.dist {
        target
    } else {
        cam.dist + (target - cam.dist) * (1.0 - (-CAM_FOLLOW_RATE * time.delta_secs()).exp())
    };

    *tf = Transform::from_translation(focus + dir * cam.dist).looking_at(focus, Vec3::Y);
}

fn inflate(b: &Aabb, r: f32) -> Aabb {
    Aabb {
        min: CVec3::new(b.min.x - r, b.min.y - r, b.min.z - r),
        max: CVec3::new(b.max.x + r, b.max.y + r, b.max.z + r),
    }
}
