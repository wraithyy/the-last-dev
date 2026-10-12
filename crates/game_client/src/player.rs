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
}

impl PlayerBody {
    /// Hard move (respawn): also drops interpolation history and vertical speed.
    pub fn teleport(&mut self, pos: Vec3) {
        self.pos = pos;
        self.prev_pos = pos;
        self.vy = 0.0;
    }

    /// Off the ground: `simulate` zeroes `vy` whenever it snaps the feet to a surface.
    pub fn airborne(&self) -> bool {
        self.vy != 0.0
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(FixedUpdate, simulate)
            .add_systems(
                RunFixedMainLoop,
                interpolate.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            );
    }
}

fn spawn_player(mut commands: Commands) {
    let pos = Vec3::from(PLAYER_SPAWN);
    commands.spawn((
        Player,
        PlayerBody {
            pos,
            yaw: 0.0,
            prev_pos: pos,
            prev_yaw: 0.0,
            vy: 0.0,
        },
        Character {
            kind: ModelKind::Mage,
            scale: PLAYER_MODEL_SCALE,
            tint: None,
        },
        Transform::from_translation(pos),
        Visibility::default(),
    ));
}

fn simulate(
    time: Res<Time>,
    mut input: ResMut<PlayerInput>,
    colliders: Res<Colliders>,
    mut body: Single<&mut PlayerBody>,
) {
    let dt = time.delta_secs();
    body.prev_pos = body.pos;
    body.prev_yaw = body.yaw;

    if let Some(yaw) = input.face_yaw.take() {
        body.yaw = yaw;
    }
    body.yaw += input.turn * KEY_TURN_RATE * dt + input.mouse_yaw;
    input.mouse_yaw = 0.0;

    let rot = Quat::from_rotation_y(body.yaw);
    let max_speed = if input.forward < 0.0 {
        RUN_SPEED * BACKPEDAL_FACTOR
    } else {
        RUN_SPEED
    };
    let local = Vec3::new(input.strafe, 0.0, -input.forward) * RUN_SPEED;
    let velocity = rot * local.clamp_length_max(max_speed);
    body.pos = slide(body.pos, velocity * dt, &colliders.0);

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
