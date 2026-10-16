//! Senior Engineer: fixed-step movement simulation plus interpolated rendering.

use crate::anim::{Character, ModelKind};
use crate::arena::Colliders;
use crate::collision::{ground_height, slide};
use crate::config::*;
use crate::input::PlayerInput;
use bevy::prelude::*;

#[derive(Component)]
pub struct Player;

/// Authoritative simulation state; `Transform` is only its interpolated view.
#[derive(Component)]
pub struct PlayerBody {
    pub pos: Vec3,
    pub yaw: f32,
    prev_pos: Vec3,
    prev_yaw: f32,
    vy: f32,
    /// Horizontal knockback velocity, decaying each tick.
    knock: Vec3,
}

impl PlayerBody {
    /// Hard move (respawn): also drops interpolation history and vertical speed.
    pub fn teleport(&mut self, pos: Vec3) {
        self.pos = pos;
        self.prev_pos = pos;
        self.vy = 0.0;
        self.knock = Vec3::ZERO;
    }

    /// Teleport that also faces `yaw` without interpolating from the old heading.
    pub fn teleport_facing(&mut self, pos: Vec3, yaw: f32) {
        self.teleport(pos);
        self.yaw = yaw;
        self.prev_yaw = yaw;
    }

    /// Sim knockback: travel about `strength` metres along `dir` with a small hop.
    pub fn knock_back(&mut self, dir: Vec3, strength: f32) {
        self.knock = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero() * strength * KNOCK_DECAY;
        self.vy = self.vy.max(KNOCK_HOP);
    }

    /// Off the ground: `simulate` zeroes `vy` whenever it snaps the feet to a surface.
    pub fn airborne(&self) -> bool {
        self.vy != 0.0
    }
}

/// Run-speed multiplier from the player's speed auras (e-scooter, sprint), kept by
/// `office::ride` so prediction matches the server's movement budget.
#[derive(Resource)]
pub struct MoveSpeed(pub f32);

impl Default for MoveSpeed {
    fn default() -> Self {
        MoveSpeed(1.0)
    }
}

/// VibeCon's "Ignore Previous Instructions": movement input is mirrored while the player wears
/// a `reverse_controls` aura (kept by `office::ride` with the speed). Movement is client-owned,
/// so this is the whole mechanic; the sim only applies the aura.
#[derive(Resource, Default)]
pub struct ControlsReversed(pub bool);

/// Teambuilding: while a stealth run is on (`venue`), the jump key toggles sneaking, which
/// walks at `SNEAK_FACTOR` of the run speed; the sim hears anyone faster (`noisy_mps`).
#[derive(Resource, Default)]
pub struct Sneak {
    pub venue: bool,
    pub on: bool,
}

/// 7 m/s * 0.5 stays under the sim's 5 m/s noise line with room for frame jitter.
pub const SNEAK_FACTOR: f32 = 0.5;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MoveSpeed>()
            .init_resource::<ControlsReversed>()
            .init_resource::<Sneak>()
            .add_systems(Startup, spawn_player)
            .add_systems(FixedUpdate, simulate)
            .add_systems(
                RunFixedMainLoop,
                interpolate.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            );
    }
}

/// Placed at the floor's `player_spawn` by the game flow once the first floor loads.
fn spawn_player(mut commands: Commands) {
    let pos = Vec3::ZERO;
    let yaw = -std::f32::consts::FRAC_PI_2;
    commands.spawn((
        Player,
        PlayerBody {
            pos,
            yaw,
            prev_pos: pos,
            prev_yaw: yaw,
            vy: 0.0,
            knock: Vec3::ZERO,
        },
        Character {
            kind: ModelKind::ENGINEER,
            scale: PLAYER_MODEL_SCALE,
        },
        Transform::from_translation(pos),
        Visibility::default(),
    ));
}

fn simulate(
    time: Res<Time>,
    mut input: ResMut<PlayerInput>,
    colliders: Res<Colliders>,
    speed: Res<MoveSpeed>,
    reversed: Res<ControlsReversed>,
    mut sneak: ResMut<Sneak>,
    mut body: Single<&mut PlayerBody>,
) {
    let dt = time.delta_secs();
    body.prev_pos = body.pos;
    body.prev_yaw = body.yaw;

    if let Some(yaw) = input.face_yaw.take() {
        body.yaw = yaw;
    }
    let flip = if reversed.0 { -1.0 } else { 1.0 };
    let (forward, strafe, turn) = (input.forward * flip, input.strafe * flip, input.turn * flip);
    body.yaw += turn * KEY_TURN_RATE * dt + input.mouse_yaw;
    input.mouse_yaw = 0.0;

    let rot = Quat::from_rotation_y(body.yaw);
    if sneak.venue && input.jump {
        input.jump = false;
        sneak.on = !sneak.on;
    }
    let stealth = if sneak.venue && sneak.on {
        SNEAK_FACTOR
    } else {
        1.0
    };
    let run = RUN_SPEED * speed.0 * stealth;
    let max_speed = if forward < 0.0 {
        run * BACKPEDAL_FACTOR
    } else {
        run
    };
    let local = Vec3::new(strafe, 0.0, -forward) * run;
    let velocity = rot * local.clamp_length_max(max_speed);
    let knock = body.knock;
    body.pos = slide(body.pos, (velocity + knock) * dt, &colliders.0);
    // Exponential decay: the integral of the initial speed is `strength` metres.
    body.knock *= (-KNOCK_DECAY * dt).exp();
    if body.knock.length_squared() < 0.01 {
        body.knock = Vec3::ZERO;
    }

    let ground = ground_height(body.pos, &colliders.0);
    if input.jump {
        input.jump = false;
        if body.pos.y <= ground + GROUND_EPSILON {
            body.vy = JUMP_SPEED;
        }
    }
    body.vy -= GRAVITY * dt;
    body.pos.y += body.vy * dt;
    if body.pos.y <= ground && body.vy <= 0.0 {
        body.pos.y = ground;
        body.vy = 0.0;
    }
}

/// Render-side smoothing: blend the last two sim states by the fixed-step
/// overstep so motion stays smooth at any frame rate (sim stays 30 Hz).
fn interpolate(fixed: Res<Time<Fixed>>, player: Single<(&PlayerBody, &mut Transform)>) {
    let (body, mut tf) = player.into_inner();
    let t = fixed.overstep_fraction();
    tf.translation = body.prev_pos.lerp(body.pos, t);
    tf.rotation = Quat::from_rotation_y(body.prev_yaw + (body.yaw - body.prev_yaw) * t);
}
