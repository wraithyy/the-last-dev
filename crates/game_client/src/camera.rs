//! Third-person orbit camera: zoom, pitch clamp, snap-behind, wall clipping.

use crate::arena::{CameraCeiling, Colliders};
use crate::collision::to_core;
use crate::config::*;
use crate::input::{MouseState, Paused, PlayerInput};
use crate::player::Player;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use combat_core::{Aabb, Vec3 as CVec3, line_of_sight};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Component)]
pub struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    /// Distance the user asked for via the wheel.
    zoom: f32,
    /// Distance actually used after wall clipping and easing.
    dist: f32,
}

impl OrbitCamera {
    /// Turn the camera to look along a player yaw (start of a floor, debug teleports).
    pub fn set_yaw(&mut self, yaw: f32) {
        self.yaw = yaw;
    }

    /// Debug close-ups (`WOW_CAM_YAW_OFFSET`, radians from the current heading).
    pub fn add_yaw(&mut self, delta: f32) {
        self.yaw += delta;
    }

    /// Debug close-ups (`WOW_CAM_PITCH`, radians).
    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch.clamp(-CAM_PITCH_LIMIT, CAM_PITCH_LIMIT);
    }

    /// First view of an indoor room: a higher, longer arm than the walking default so the room
    /// reads instead of a tunnel. `cam_ceiling` is the highest the camera may go. Under a low
    /// ceiling the arm gets its length from a shallower pitch (down to `CAM_ARRIVAL_MIN_PITCH`)
    /// rather than being cut short, so a corridor keeps room in front of the player.
    pub fn frame_arrival(&mut self, cam_ceiling: f32) {
        let rise = (cam_ceiling - CAM_FOCUS_HEIGHT).max(0.1);
        self.pitch = (rise / CAM_ARRIVAL_MAX_DIST)
            .asin()
            .clamp(CAM_ARRIVAL_MIN_PITCH, CAM_ARRIVAL_PITCH);
        let reach = rise / self.pitch.sin();
        self.zoom = reach.clamp(CAM_ARRIVAL_MIN_DIST, CAM_ARRIVAL_MAX_DIST);
        self.dist = self.zoom;
    }

    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// The zoom the player asked for (the onboarding's "look around" step watches it).
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Touch drag: same axes as the mouse orbit, in radians.
    pub fn orbit(&mut self, dyaw: f32, dpitch: f32) {
        self.yaw += dyaw;
        self.pitch = (self.pitch + dpitch).clamp(-CAM_PITCH_LIMIT, CAM_PITCH_LIMIT);
    }

    /// Pinch: `factor` > 1 moves the camera away.
    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(CAM_MIN_DIST, CAM_MAX_DIST);
    }

    /// Debug close-ups (`WOW_CAM_DIST`).
    pub fn set_zoom(&mut self, dist: f32) {
        self.zoom = dist.clamp(CAM_MIN_DIST, CAM_MAX_DIST);
        self.dist = self.zoom;
    }
}

/// Short screen shake (knockbacks); `trauma` decays to zero in about half a second.
#[derive(Resource, Default)]
pub struct CameraShake {
    pub trauma: f32,
}

/// `WOW_TOPDOWN=<height m>` (1 = default 110): straight-down overview of the whole floor for
/// layout screenshots; below 100 m it follows the player for close-up plan checks. The ceiling is single-sided, so it does not hide the plan.
#[derive(Resource)]
struct TopDown(f32);

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraShake>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (follow_player, orbit_input, place_camera).chain());
        if let Some(look) = std::env::var("WOW_CAM_LOOK")
            .ok()
            .and_then(|v| parse_look(&v))
        {
            app.insert_resource(FixedLook(look))
                .add_systems(Update, fixed_look.after(place_camera));
        }
        if let Some(v) = std::env::var("WOW_TOPDOWN")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            app.insert_resource(TopDown(if v > 5.0 { v } else { 110.0 }))
                .add_systems(Update, topdown.after(place_camera));
        }
    }
}

/// `WOW_CAM_LOOK=ex,ey,ez,tx,ty,tz`: the camera sits at `e` and looks at `t` (world metres), for
/// close-ups of poses from any side without the player in the way (QA only).
#[derive(Resource)]
struct FixedLook((Vec3, Vec3));

fn parse_look(v: &str) -> Option<(Vec3, Vec3)> {
    let n: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    match n[..] {
        [ex, ey, ez, tx, ty, tz] => Some((Vec3::new(ex, ey, ez), Vec3::new(tx, ty, tz))),
        _ => None,
    }
}

fn fixed_look(look: Res<FixedLook>, mut cam: Single<&mut Transform, With<OrbitCamera>>) {
    **cam = Transform::from_translation(look.0.0).looking_at(look.0.1, Vec3::Y);
}

fn topdown(
    mut commands: Commands,
    height: Res<TopDown>,
    player: Single<&Transform, (With<Player>, Without<OrbitCamera>)>,
    cam: Single<(Entity, &mut Transform), With<OrbitCamera>>,
) {
    let (entity, mut tf) = cam.into_inner();
    // Whole floor by default; WOW_PLAYER_AT pans the view for close-up plan checks.
    let centre = if height.0 >= 100.0 {
        Vec3::ZERO
    } else {
        Vec3::new(player.translation.x, 0.0, player.translation.z)
    };
    *tf = Transform::from_translation(centre + Vec3::Y * height.0).looking_at(centre, Vec3::NEG_Z);
    // Fog would swallow a 100 m view.
    commands.entity(entity).remove::<bevy::pbr::DistanceFog>();
}

/// `WOW_CAM_FOCUS=<m>` lowers the look-at point for close-ups of the hands (QA only).
fn focus_height() -> f32 {
    static FOCUS: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *FOCUS.get_or_init(|| {
        std::env::var("WOW_CAM_FOCUS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(CAM_FOCUS_HEIGHT)
    })
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        OrbitCamera {
            // Floors run west to east and the player starts facing east.
            yaw: -FRAC_PI_2,
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
    touch: Res<crate::touch::TouchState>,
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
    // The stick keeps the body facing the camera, so easing back would fight a drag.
    if input.forward > 0.0 && !mouse.any_button() && !touch.move_active {
        let player_yaw = player.rotation.to_euler(EulerRot::YXZ).0;
        let diff = wrap_angle(player_yaw - cam.yaw);
        cam.yaw += diff * (1.0 - (-CAM_SNAP_BEHIND_RATE * dt).exp());
    }
}

fn orbit_input(
    paused: Res<Paused>,
    over_list: Res<crate::hud::scroll::ScrollHover>,
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
    // The wheel scrolls a window's list while the pointer is over one.
    if !over_list.0 {
        cam.zoom = (cam.zoom - lines * CAM_ZOOM_STEP).clamp(CAM_MIN_DIST, CAM_MAX_DIST);
    }
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
    ceiling: Res<CameraCeiling>,
    mut shake: ResMut<CameraShake>,
    settings: Res<crate::settings::Settings>,
    player: Single<&Transform, (With<Player>, Without<OrbitCamera>)>,
    cam: Single<(&mut OrbitCamera, &mut Transform)>,
) {
    if settings.reduce_motion {
        shake.trauma = 0.0;
    }
    let (mut cam, mut tf) = cam.into_inner();
    let focus = player.translation + Vec3::Y * focus_height();
    let dir = Quat::from_rotation_y(cam.yaw) * Vec3::new(0.0, cam.pitch.sin(), cam.pitch.cos());

    // Inflating the boxes by the camera radius keeps the near plane out of walls.
    let inflated: Vec<Aabb> = colliders.0.iter().map(|b| inflate(b, CAM_RADIUS)).collect();
    let mut free = clear_distance(focus, dir, cam.zoom, &inflated);
    // Looking up from below: shorten the arm so the camera stops at the floor
    // instead of being clamped in Y (which would bend the view direction).
    if dir.y < -1e-3 {
        free = free.min((CAM_MIN_HEIGHT - focus.y) / dir.y);
    }
    // Same idea against the ceiling; the slab is not a collider because it
    // must not block line of sight for combat.
    if dir.y > 1e-3 {
        free = free.min((ceiling.0 - focus.y) / dir.y);
    }
    let target = free.max(CAM_MIN_CLIP_DIST);
    // Snap in when a wall closes in fast (never show it), ease out when it clears.
    // Curved walls are AABB staircases whose steps move the clip point by a few tens of
    // centimetres; those small bites are eased in quickly (the 0.3 m inflation keeps the near
    // plane out of the wall meanwhile) so the camera does not pop while strafing along a curve.
    cam.dist = if target < cam.dist - CAM_CLIP_SOFT {
        target
    } else if target < cam.dist {
        cam.dist + (target - cam.dist) * (1.0 - (-CAM_CLIP_RATE * time.delta_secs()).exp())
    } else {
        cam.dist + (target - cam.dist) * (1.0 - (-CAM_FOLLOW_RATE * time.delta_secs()).exp())
    };

    *tf = Transform::from_translation(focus + dir * cam.dist).looking_at(focus, Vec3::Y);
    if shake.trauma > 0.0 {
        let t = time.elapsed_secs() * 45.0;
        let amp = CAM_SHAKE_AMPLITUDE * shake.trauma * shake.trauma;
        let jitter = Vec3::new(t.sin(), (t * 1.3).cos(), 0.0) * amp;
        let offset = tf.rotation * jitter;
        tf.translation += offset;
        shake.trauma = (shake.trauma - 2.0 * time.delta_secs()).max(0.0);
    }
}

pub(crate) fn inflate(b: &Aabb, r: f32) -> Aabb {
    Aabb {
        min: CVec3::new(b.min.x - r, b.min.y - r, b.min.z - r),
        max: CVec3::new(b.max.x + r, b.max.y + r, b.max.z + r),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walks round the stage drum of floor 3 (a curved wall made of AABBs) with the camera arm
    /// pointing at the wall, and reports how far the clipped distance jumps per 10 cm step.
    #[test]
    fn camera_distance_does_not_pop_along_a_curved_wall() {
        let layout = crate::floors::build_zone("tower/3").unwrap_or_else(|| panic!("tower/3"));
        let boxes: Vec<Aabb> = layout
            .blockers
            .iter()
            .map(|b| inflate(b, CAM_RADIUS))
            .collect();
        let (r, step) = (14.5_f32, 0.1_f32);
        let n = (PI * r / step) as usize;
        let mut prev: Option<f32> = None;
        let mut worst = 0.0_f32;
        for i in 0..n {
            // The middle of the hall's arc, away from its open ends.
            let a = (90.0 + 180.0 * i as f32 / n as f32).to_radians();
            let p = Vec3::new(r * a.cos(), CAM_FOCUS_HEIGHT, r * a.sin());
            // Arm points at the drum centre, the wall the camera would be pushed against.
            let dir = Vec3::new(-a.cos(), 0.3, -a.sin()).normalize();
            let d = clear_distance(p, dir, CAM_START_DIST, &boxes);
            // Only the stretches where the drum, not the outer wall, clips the arm.
            if d < CAM_START_DIST - 0.5 {
                if let Some(q) = prev {
                    worst = worst.max((d - q).abs());
                }
                prev = Some(d);
            } else {
                prev = None;
            }
        }
        println!("worst camera pop per 10 cm step: {worst:.3} m");
        assert!(worst < 0.4, "camera pops {worst:.2} m per step");
    }
}
