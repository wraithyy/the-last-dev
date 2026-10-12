//! All tunable gameplay and presentation constants live here.

use std::f32::consts::PI;

pub const WINDOW_TITLE: &str = "World of Workcraft";

// Arena (metres). The floor spans [-ARENA_HALF, ARENA_HALF] on X and Z.
pub const ARENA_HALF: f32 = 20.0;
pub const WALL_HEIGHT: f32 = 4.0;
pub const WALL_THICKNESS: f32 = 0.5;

// Player
pub const PLAYER_NAME: &str = "Senior Engineer";
pub const PLAYER_LEVEL: u32 = 3;
pub const PLAYER_SPAWN: [f32; 3] = [0.0, 0.0, 15.0];
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

pub const PLAYER_BODY_RADIUS: f32 = 0.35;
pub const PLAYER_BODY_LENGTH: f32 = 0.7;
pub const PLAYER_BODY_Y: f32 = 0.7;
pub const PLAYER_HEAD_RADIUS: f32 = 0.25;
pub const PLAYER_HEAD_Y: f32 = 1.55;
pub const PLAYER_LAPTOP_SIZE: [f32; 3] = [0.5, 0.04, 0.35];
pub const PLAYER_LAPTOP_POS: [f32; 3] = [0.0, 1.0, -0.5];
pub const PLAYER_LAPTOP_TILT: f32 = 0.5;
/// Tolerance for "standing on the ground" when deciding if a jump is allowed.
pub const GROUND_EPSILON: f32 = 1e-3;

// Camera
/// Bisection steps for camera clipping (precision = arm length / 2^steps).
pub const CAM_BISECT_STEPS: u32 = 10;
/// Clearance kept between the camera and any wall (near-plane safety).
pub const CAM_RADIUS: f32 = 0.3;
/// Never pull the camera closer than this, so it stays out of the head mesh.
pub const CAM_MIN_CLIP_DIST: f32 = 0.8;
pub const CAM_MIN_DIST: f32 = 2.0;
pub const CAM_MAX_DIST: f32 = 25.0;
pub const CAM_START_DIST: f32 = 9.0;
pub const CAM_START_PITCH: f32 = 0.35;
pub const CAM_PITCH_LIMIT: f32 = 80.0 * PI / 180.0;
pub const CAM_ORBIT_SENS: f32 = 0.005;
pub const CAM_ZOOM_STEP: f32 = 1.5;
/// Pixel-unit scroll (browsers) is scaled down to match one wheel "line".
pub const CAM_ZOOM_PIXELS_PER_LINE: f32 = 40.0;
/// Higher = snappier follow (exponential smoothing rate, 1/s).
pub const CAM_FOLLOW_RATE: f32 = 12.0;
pub const CAM_SNAP_BEHIND_RATE: f32 = 4.0;
pub const CAM_MIN_HEIGHT: f32 = 0.2;
pub const CAM_FOCUS_HEIGHT: f32 = 1.5;

// Input
/// Mouse travel (px) below which a left-button release counts as a click.
pub const CLICK_DRAG_THRESHOLD: f32 = 4.0;

// Targets
pub const RING_INNER: f32 = 0.85;
pub const RING_OUTER: f32 = 1.05;
pub const RING_HEIGHT: f32 = 0.03;
pub const PICK_SPHERE_RADIUS: f32 = 1.1;
pub const PICK_SPHERE_HEIGHT: f32 = 1.2;
/// Placeholder until Phase 2 introduces real spell definitions.
pub const PREVIEW_SPELL_RANGE: f32 = 30.0;
