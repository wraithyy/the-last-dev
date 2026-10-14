//! Rides, sprint and animated emotes. Every character carries a `WorldPose` (mounted, emote,
//! lean) that `anim.rs` reads; riders get the campus scooter rig under their feet, and the
//! player's speed auras feed `MoveSpeed`, so prediction matches the server's movement budget.

use crate::combat::{PendingCommands, SimRead, SimUnit, SimView, can_act};
use crate::game::GameFlow;
use crate::input::Paused;
use crate::keybinds::{Action, Keybinds};
use crate::net::NetWorld;
use crate::player::{MoveSpeed, Player};
use crate::rigs::PrimRig;
use bevy::prelude::*;
use combat_core::{Command, EntityId};
use net_proto::Emote;

/// Height of the scooter deck the rider stands on.
const DECK: f32 = 0.14;
const FACEPALM_SECS: f32 = 2.6;
const DANCE_SECS: f32 = 20.0;
/// Moving this far (XZ) ends a sit or a dance.
const POSE_BREAK_M: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PoseClip {
    Sit,
    Dance,
    Facepalm,
}

/// The pose an emote plays; phrase emotes have none.
pub fn pose_of(e: Emote) -> Option<PoseClip> {
    match e {
        Emote::Sit => Some(PoseClip::Sit),
        Emote::Dance => Some(PoseClip::Dance),
        Emote::Facepalm => Some(PoseClip::Facepalm),
        _ => None,
    }
}

/// Presentation state of rides and emotes for one character.
#[derive(Component, Default)]
pub struct WorldPose {
    pub mounted: bool,
    /// Roll into turns plus a little wobble, radians.
    pub lean: f32,
    /// (pose, started at, where).
    emote: Option<(PoseClip, f32, Vec3)>,
    prev_yaw: Option<f32>,
}

impl WorldPose {
    pub fn pose(&self) -> Option<PoseClip> {
        self.emote.map(|e| e.0)
    }

    /// (lift, roll) of the model root: riders stand on the deck and lean.
    pub fn ride_offset(&self) -> (f32, f32) {
        if self.mounted {
            (DECK, self.lean)
        } else {
            (0.0, 0.0)
        }
    }

    pub fn start(&mut self, pose: PoseClip, now: f32, at: Vec3) {
        self.emote = Some((pose, now, at));
    }
}

/// An animated emote played offline (online they come back as `Emoted`, see `NetWorld::poses`).
#[derive(Message, Clone, Copy)]
pub struct EmotePlayed(pub EntityId, pub Emote);

/// The scooter rig under a rider.
#[derive(Component)]
pub struct RideModel;
/// The rider's `RideModel` child.
#[derive(Component)]
pub struct HasRide(Entity);

pub fn add_poses(mut commands: Commands, new: Query<Entity, (Added<SimUnit>, Without<WorldPose>)>) {
    for e in &new {
        commands.entity(e).insert(WorldPose::default());
    }
}

pub fn apply_emotes(
    time: Res<Time>,
    mut played: MessageReader<EmotePlayed>,
    net: Option<ResMut<NetWorld>>,
    mut poses: Query<(&SimUnit, &Transform, &mut WorldPose)>,
) {
    let mut list: Vec<(EntityId, Emote)> = played.read().map(|m| (m.0, m.1)).collect();
    if let Some(mut net) = net
        && !net.poses.is_empty()
    {
        list.append(&mut net.poses);
    }
    let now = time.elapsed_secs();
    for (id, emote) in list {
        let Some(pose) = pose_of(emote) else { continue };
        if let Some((_, tf, mut w)) = poses.iter_mut().find(|(s, ..)| s.0 == id) {
            w.start(pose, now, tf.translation);
        }
    }
}

fn yaw_of(tf: &Transform) -> f32 {
    tf.rotation.to_euler(EulerRot::YXZ).0
}

/// Mounted from the auras (so remote riders show too), emotes end on movement or time, and
/// the lean follows the turn rate.
pub fn update_poses(
    time: Res<Time>,
    sim: SimView,
    mut poses: Query<(&SimUnit, &Transform, &mut WorldPose)>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    for (su, tf, mut w) in &mut poses {
        let mounted = sim
            .unit(su.0)
            .is_some_and(|u| u.alive && u.auras.iter().any(|a| sim.content().is_mount(&a.key)));
        if w.mounted != mounted {
            w.mounted = mounted;
        }
        if let Some((pose, t0, at)) = w.emote {
            let moved = Vec2::new(tf.translation.x - at.x, tf.translation.z - at.z).length();
            let over = match pose {
                PoseClip::Facepalm => now - t0 > FACEPALM_SECS,
                PoseClip::Dance => now - t0 > DANCE_SECS,
                PoseClip::Sit => false,
            };
            let dead = sim.unit(su.0).is_some_and(|u| !u.alive);
            if over || dead || moved > POSE_BREAK_M || mounted {
                w.emote = None;
            }
        }
        let yaw = yaw_of(tf);
        let turn = w.prev_yaw.map_or(0.0, |p| {
            let d = (yaw - p + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            if dt > 0.0 { d / dt } else { 0.0 }
        });
        w.prev_yaw = Some(yaw);
        let target = if mounted {
            (-turn * 0.12).clamp(-0.3, 0.3) + (now * 7.0).sin() * 0.025
        } else {
            0.0
        };
        let k = 1.0 - (-8.0 * dt).exp();
        w.lean += (target - w.lean) * k;
    }
}

/// Spawns or drops the scooter rig under each character as the ride starts and ends.
pub fn ride_models(
    mut commands: Commands,
    riders: Query<(Entity, &WorldPose, Option<&HasRide>)>,
    mut models: Query<(&ChildOf, &mut Transform), With<RideModel>>,
) {
    for (e, w, has) in &riders {
        match (w.mounted, has) {
            (true, None) => {
                let child = commands
                    .spawn((
                        RideModel,
                        PrimRig("scooter"),
                        ChildOf(e),
                        Transform::default(),
                        Visibility::default(),
                    ))
                    .id();
                commands.entity(e).insert(HasRide(child));
            }
            (false, Some(h)) => {
                commands.entity(h.0).despawn();
                commands.entity(e).remove::<HasRide>();
            }
            _ => {}
        }
    }
    for (parent, mut tf) in &mut models {
        let lean = riders.get(parent.parent()).map_or(0.0, |(_, w, _)| w.lean);
        tf.rotation = Quat::from_rotation_z(lean * 0.6);
    }
}

pub fn player_speed(
    sim: SimView,
    player: Single<&SimUnit, With<Player>>,
    mut speed: ResMut<MoveSpeed>,
) {
    let pct = sim.unit(player.0).map_or(100, |u| {
        sim.content()
            .speed_pct(u.auras.iter().map(|a| a.key.as_str()))
    });
    let want = pct as f32 / 100.0;
    if speed.0 != want {
        speed.0 = want;
    }
}

/// Shared by the keys and the action bar buttons: the sim validates (zone, combat, cooldown).
pub fn cast_world_spell(
    sim: &dyn SimRead,
    pending: &mut PendingCommands,
    caster: EntityId,
    mount: bool,
) {
    let c = sim.content();
    let spell = if mount {
        c.mount_spell()
    } else {
        c.sprint_spell()
    };
    if let Some(s) = spell {
        pending.0.push(Command::Cast {
            caster,
            spell: s.id,
        });
    }
}

pub fn ride_keys(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
) {
    if paused.0 || !can_act(&*sim, &flow, player.0) {
        return;
    }
    if binds.just(&keys, Action::Scooter) {
        cast_world_spell(&*sim, &mut pending, player.0, true);
    }
    if binds.just(&keys, Action::Sprint) {
        cast_world_spell(&*sim, &mut pending, player.0, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_animated_emotes_have_a_pose() {
        assert_eq!(pose_of(Emote::Sit), Some(PoseClip::Sit));
        assert_eq!(pose_of(Emote::Dance), Some(PoseClip::Dance));
        assert_eq!(pose_of(Emote::Facepalm), Some(PoseClip::Facepalm));
        assert_eq!(pose_of(Emote::Lgtm), None);
        let mut w = WorldPose::default();
        assert_eq!(w.ride_offset(), (0.0, 0.0));
        w.mounted = true;
        w.lean = 0.1;
        assert_eq!(w.ride_offset(), (DECK, 0.1));
    }
}
