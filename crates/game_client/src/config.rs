//! All tunable gameplay and presentation constants live here.

use std::f32::consts::PI;

pub const WINDOW_TITLE: &str = "The Last Dev";

// Floors (metres). Outlines are free-form (see floors/shape.rs); every floor fits inside
// [-FLOOR_HALF_X, FLOOR_HALF_X] x [-FLOOR_HALF_Z, FLOOR_HALF_Z], which is also the generous
// outer clamp that collision keeps as a safety net behind the walls.
pub const FLOOR_HALF_X: f32 = 262.0;
pub const FLOOR_HALF_Z: f32 = 182.0;
/// Also the ceiling height: the ceiling slab sits on top of the walls.
pub const WALL_HEIGHT: f32 = 4.5;

// Player
/// Knockback speed decay rate (1/s); also scales the initial speed so the push covers `strength` m.
pub const KNOCK_DECAY: f32 = 7.0;
pub const KNOCK_HOP: f32 = 3.5;
pub const PLAYER_RADIUS: f32 = 0.4;
pub const PLAYER_HEIGHT: f32 = 1.8;
/// Eye height used as ray origin for LoS and camera collision.
pub const PLAYER_EYE: f32 = 1.6;
pub const RUN_SPEED: f32 = 7.0;
pub const BACKPEDAL_FACTOR: f32 = 0.65;
pub const KEY_TURN_RATE: f32 = PI; // 180 deg/s
pub const JUMP_SPEED: f32 = 7.5;
pub const GRAVITY: f32 = 22.0;
/// Props at most this far above the feet are stepped onto instead of blocking.
pub const STEP_HEIGHT: f32 = 0.3;
/// Radians of yaw per pixel of mouse travel.
pub const MOUSE_YAW_SENS: f32 = 0.005;

/// Quaternius characters stand about 3.2 units tall (hair included) at scale 1, measured from the mesh bounds.
pub const MODEL_NATIVE_HEIGHT: f32 = 3.2;
pub const PLAYER_MODEL_SCALE: f32 = PLAYER_HEIGHT / MODEL_NATIVE_HEIGHT;
/// Tolerance for "standing on the ground" when deciding if a jump is allowed.
pub const GROUND_EPSILON: f32 = 1e-3;
/// A grounded body follows the terrain down this far per step instead of falling.
pub const SLOPE_SNAP: f32 = 0.5;

// Camera
/// Bisection steps for camera clipping (precision = arm length / 2^steps).
pub const CAM_BISECT_STEPS: u32 = 10;
/// Clearance kept between the camera and any wall (near-plane safety).
pub const CAM_RADIUS: f32 = 0.3;
/// Never pull the camera closer than this, so it stays out of the head mesh.
/// Clip-distance drops up to this size ease in at `CAM_CLIP_RATE` instead of snapping.
pub const CAM_CLIP_SOFT: f32 = 0.35;
pub const CAM_CLIP_RATE: f32 = 40.0;
pub const CAM_MIN_CLIP_DIST: f32 = 0.8;
pub const CAM_MIN_DIST: f32 = 2.0;
pub const CAM_MAX_DIST: f32 = 20.0;
/// WoW-like default: roughly 10 m behind, shallow pitch so the corridor stays in view.
pub const CAM_START_DIST: f32 = 10.5;
pub const CAM_START_PITCH: f32 = 0.22;
pub const CAM_ARRIVAL_PITCH: f32 = 0.32;
pub const CAM_ARRIVAL_MIN_DIST: f32 = 7.5;
pub const CAM_ARRIVAL_MAX_DIST: f32 = 11.5;
pub const CAM_PITCH_LIMIT: f32 = 80.0 * PI / 180.0;
pub const CAM_ORBIT_SENS: f32 = 0.005;
pub const CAM_ZOOM_STEP: f32 = 1.5;
/// Pixel-unit scroll (browsers) is scaled down to match one wheel "line".
pub const CAM_ZOOM_PIXELS_PER_LINE: f32 = 40.0;
/// Higher = snappier follow (exponential smoothing rate, 1/s).
pub const CAM_FOLLOW_RATE: f32 = 12.0;
pub const CAM_SNAP_BEHIND_RATE: f32 = 4.0;
pub const CAM_MIN_HEIGHT: f32 = 0.2;
/// Keeps the camera (and its near plane) below the ceiling slab.
pub const CAM_MAX_HEIGHT: f32 = WALL_HEIGHT - 0.4;
/// Indoors the camera stays this far under the ceiling, clear of the hanging light panels.
pub const CAM_CEILING_GAP: f32 = 0.8;
pub const CAM_FOCUS_HEIGHT: f32 = 1.5;
/// Peak camera offset (m) of a full-trauma shake.
pub const CAM_SHAKE_AMPLITUDE: f32 = 0.18;

// Input
/// Mouse travel (px) below which a left-button release counts as a click.
pub const CLICK_DRAG_THRESHOLD: f32 = 4.0;

// Targets
pub const RING_INNER: f32 = 0.85;
pub const RING_OUTER: f32 = 1.05;
pub const RING_HEIGHT: f32 = 0.03;
pub const PICK_SPHERE_RADIUS: f32 = 1.2;
pub const PICK_SPHERE_HEIGHT: f32 = 1.3;

// Combat bridge
pub const SIM_SEED: u64 = 0x5EED_0000_C0DE;

// HUD layout (logical px)
pub const FRAME_W: f32 = 250.0;
pub const FRAME_BAR_H: f32 = 13.0;
pub const PORTRAIT_SIZE: f32 = 64.0;
pub const ACTION_SLOT: f32 = 46.0;
pub const ACTION_GAP: f32 = 5.0;
pub const AURA_ICON: f32 = 30.0;
pub const AURA_SLOTS: usize = 8;
pub const CAST_BAR_W: f32 = 300.0;
pub const CAST_BAR_H: f32 = 22.0;
/// Seconds the "success" / "interrupted" state of the player cast bar lingers.
pub const CAST_FLASH_SECS: f32 = 0.45;
pub const ERROR_TEXT_SECS: f32 = 2.0;
pub const FLOAT_TEXT_SECS: f32 = 1.4;
pub const FLOAT_TEXT_RISE: f32 = 60.0;
/// Gap above a unit's head where its floating numbers start.
pub const FLOAT_TEXT_HEAD_GAP: f32 = 0.25;
/// Speech bubbles sit above the floating-number start so numbers do not overprint them.
pub const BUBBLE_HEAD_GAP: f32 = 1.05;
pub const FLOAT_TEXT_SIZE: f32 = 26.0;
pub const FLOAT_TEXT_CRIT_SIZE: f32 = 36.0;

// VFX
pub const PROJECTILE_ORIGIN_Y: f32 = 1.25;
pub const PROJECTILE_TARGET_Y: f32 = 1.2;
pub const HAND_GLOW_POS: [f32; 3] = [0.0, 1.0, -0.85];
pub const HAND_GLOW_RADIUS: f32 = 0.16;
pub const BURST_SECS: f32 = 0.45;
pub const DEATH_FADE_SECS: f32 = 1.2;
pub const INTERRUPT_COLOR: [f32; 3] = [1.0, 0.85, 0.2];
pub const DEATH_FADE_COLOR: [f32; 3] = [0.5, 0.25, 0.7];
pub const TELEGRAPH_COLOR: [f32; 3] = [0.7, 0.2, 1.0];
pub const TELEGRAPH_RADIUS: f32 = 2.4;
pub const STUN_COLOR: [f32; 3] = [1.0, 0.9, 0.3];
pub const LEVEL_UP_COLOR: [f32; 3] = [1.0, 0.8, 0.25];

// HUD scaling
pub const UI_REFERENCE_HEIGHT: f32 = 900.0;
/// Narrowest logical width where the combat log (right of the action bar) and the keycaps do not overlap.
pub const UI_REFERENCE_WIDTH: f32 = 1600.0;
pub const UI_MIN_SCALE: f32 = 0.7;
pub const UI_MAX_SCALE: f32 = 2.0;
/// Touch layout: the logs collapse into a tap strip, so only the action bar sets the minimum width.
pub const TOUCH_UI_REFERENCE_HEIGHT: f32 = 500.0;
pub const TOUCH_UI_REFERENCE_WIDTH: f32 = 1000.0;
pub const TOUCH_UI_MIN_SCALE: f32 = 0.45;

/// Model height in metres by sim unit key; HUD anchors (numbers, bubbles) follow the unit's real head.
pub fn head_height(key: &str) -> f32 {
    if crate::catalog::is_known(key) {
        crate::catalog::unit_visual(key).height
    } else {
        PLAYER_HEIGHT
    }
}
