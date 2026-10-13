//! Bridge between the Bevy client and the authoritative `combat_core` simulation.
//!
//! The client owns player movement and reports it every tick; everything else
//! (HP, casts, auras, NPC positions) flows back out of the sim as state or events.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::arena::Colliders;
use crate::collision::to_core;
use crate::combat_anim::CombatAnim;
use crate::config::*;
use crate::input::{Paused, PlayerInput};
use crate::player::{Player, PlayerBody};
use crate::targeting::{Target, pick_unit};
use crate::units::Unit;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use combat_core::{CastError, Command, Content, EntityId, Event, Sim};

#[derive(Resource)]
pub struct CombatSim(pub Sim);

/// Link from a Bevy entity to its simulation unit.
/// Commands from Update-side input, flushed in `tick_sim` after `PlayerMoved`
/// so a cast always sees this tick's movement state.
#[derive(Resource, Default)]
pub struct PendingCommands(pub Vec<Command>);

#[derive(Component, Clone, Copy)]
pub struct SimUnit(pub EntityId);

/// Every sim event of a tick, re-broadcast for HUD, floating text and VFX.
#[derive(Message, Clone)]
pub struct CombatEvent(pub Event);

/// Client-side feedback that has no sim event (e.g. loot click out of reach).
#[derive(Message)]
pub struct UiNotice(pub String);

/// Orders HUD, VFX and the bridge after the sim resource exists.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimReady;

/// Last two sim poses of an NPC so rendering can interpolate between 30 Hz ticks.
#[derive(Component)]
struct SimPose {
    prev_pos: Vec3,
    pos: Vec3,
    prev_yaw: f32,
    yaw: f32,
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingCommands>()
            .add_message::<CombatEvent>()
            .add_message::<UiNotice>()
            .configure_sets(Update, SimReady.run_if(resource_exists::<CombatSim>))
            .configure_sets(
                FixedPostUpdate,
                SimReady.run_if(resource_exists::<CombatSim>),
            )
            .configure_sets(
                RunFixedMainLoop,
                SimReady.run_if(resource_exists::<CombatSim>),
            )
            .add_systems(Startup, create_sim)
            .add_systems(PostStartup, spawn_sim_units.in_set(SimReady))
            .add_systems(FixedPostUpdate, tick_sim.in_set(SimReady))
            .add_systems(
                RunFixedMainLoop,
                (
                    freeze_dead_player.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
                    interpolate_units.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
                )
                    .in_set(SimReady),
            )
            .add_systems(
                Update,
                (cast_keys, auto_attack_key, loot_input).in_set(SimReady),
            );
        if std::env::var_os("WOW_AUTOFIGHT").is_some() {
            app.init_resource::<AutoFight>()
                .add_systems(Update, (autofight, log_events).in_set(SimReady));
        }
    }
}

fn create_sim(mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    match Content::from_ron(include_str!("../../../content/game.ron")) {
        Ok(content) => commands.insert_resource(CombatSim(Sim::new(content, SIM_SEED))),
        Err(err) => {
            error!("embedded content/game.ron is invalid: {err:?}");
            exit.write(AppExit::error());
        }
    }
}

fn yaw_of(tf: &Transform) -> f32 {
    tf.rotation.to_euler(EulerRot::YXZ).0
}

fn spawn_sim_units(
    mut commands: Commands,
    mut sim: ResMut<CombatSim>,
    player: Single<(Entity, &PlayerBody), With<Player>>,
    units: Query<(Entity, &Transform, &Unit)>,
) {
    let (player_entity, body) = *player;
    let id = sim.0.spawn_player(to_core(body.pos), body.yaw);
    commands
        .entity(player_entity)
        .insert((SimUnit(id), CombatAnim::default()));
    for (entity, tf, unit) in &units {
        let key = unit.key;
        let yaw = yaw_of(tf);
        match sim.0.spawn_unit(key, to_core(tf.translation), yaw) {
            Ok(id) => {
                // Placeholder meshes carry their own level; the sim is authoritative.
                if sim
                    .0
                    .unit(id)
                    .is_some_and(|u| u32::from(u.level) != unit.level)
                {
                    warn!(
                        "level mismatch for {} ({key}): scene {} vs sim data",
                        unit.name, unit.level
                    );
                }
                commands.entity(entity).insert((
                    SimUnit(id),
                    CombatAnim::default(),
                    SimPose {
                        prev_pos: tf.translation,
                        pos: tf.translation,
                        prev_yaw: yaw,
                        yaw,
                    },
                ));
            }
            Err(err) => error!("cannot spawn sim unit {key}: {err:?}"),
        }
    }
}

fn tick_sim(
    mut sim: ResMut<CombatSim>,
    paused: Res<Paused>,
    colliders: Option<Res<Colliders>>,
    target: Res<Target>,
    mut blockers_set: Local<bool>,
    mut last_pos: Local<Option<Vec3>>,
    mut pending: ResMut<PendingCommands>,
    mut events: MessageWriter<CombatEvent>,
    mut player: Single<(&mut PlayerBody, &SimUnit, &mut CombatAnim), With<Player>>,
    mut npcs: Query<
        (&SimUnit, &mut Unit, &mut SimPose, &mut CombatAnim),
        (Without<Player>, Without<PlayerBody>),
    >,
    sim_units: Query<&SimUnit>,
) {
    if paused.0 {
        return;
    }
    if !*blockers_set && let Some(c) = &colliders {
        sim.0.set_blockers(c.0.clone());
        *blockers_set = true;
    }
    let (body, player_id, _) = &mut *player;
    let player_id = player_id.0;

    let moving = last_pos
        .is_some_and(|p| Vec2::new(p.x - body.pos.x, p.z - body.pos.z).length_squared() > 1e-6);
    *last_pos = Some(body.pos);
    sim.0.push(Command::PlayerMoved {
        pos: to_core(body.pos),
        yaw: body.yaw,
        moving,
        jumping: body.airborne(),
    });

    let wanted = target.0.and_then(|e| sim_units.get(e).ok()).map(|s| s.0);
    // Compare with the sim, not with what we last sent: death clears the sim's target.
    if sim.0.unit(player_id).is_some_and(|u| u.target != wanted) {
        sim.0.push(Command::SetTarget {
            unit: player_id,
            target: wanted,
        });
    }
    for cmd in pending.0.drain(..) {
        sim.0.push(cmd);
    }

    let tick_events = sim.0.tick();

    // Authoritative state back into the Bevy mirror.
    for (su, mut unit, mut pose, mut anim) in &mut npcs {
        let Some(state) = sim.0.unit(su.0) else {
            continue;
        };
        unit.alive = state.alive;
        unit.hostile = state.hostile;
        pose.prev_pos = pose.pos;
        pose.prev_yaw = pose.yaw;
        pose.pos = Vec3::new(state.pos.x, state.pos.y, state.pos.z);
        pose.yaw = state.yaw;
        anim.casting = state.cast.is_some();
        anim.dead = !state.alive;
    }
    let (body, _, anim) = &mut *player;
    if let Some(state) = sim.0.unit(player_id) {
        anim.casting = state.cast.is_some();
        anim.dead = !state.alive;
    }

    for ev in &tick_events {
        if let Event::Respawned { unit } = ev
            && *unit == player_id
        {
            body.teleport(Vec3::from(PLAYER_SPAWN));
        }
    }
    apply_anim_events(player_id, anim, &mut npcs, &tick_events);

    for ev in tick_events {
        events.write(CombatEvent(ev));
    }
}

/// One-shot animation triggers use sequence numbers so the animation side
/// never has to clear flags (see `combat_anim.rs`).
fn apply_anim_events(
    player_id: EntityId,
    player_anim: &mut CombatAnim,
    npcs: &mut Query<
        (&SimUnit, &mut Unit, &mut SimPose, &mut CombatAnim),
        (Without<Player>, Without<PlayerBody>),
    >,
    events: &[Event],
) {
    let mut bump = |id: EntityId, f: fn(&mut CombatAnim)| {
        if id == player_id {
            f(player_anim);
        } else if let Some((.., mut anim)) = npcs.iter_mut().find(|(su, ..)| su.0 == id) {
            f(&mut anim);
        }
    };
    for ev in events {
        match ev {
            Event::CastSucceeded { caster, .. } => bump(*caster, |a| a.cast_release_seq += 1),
            // The player's auto attack is a ranged spell-style gesture, not a melee chop.
            Event::MeleeSwing { attacker, .. } if *attacker == player_id => {
                bump(*attacker, |a| a.cast_release_seq += 1);
            }
            Event::MeleeSwing { attacker, .. } => bump(*attacker, |a| a.attack_seq += 1),
            _ => {}
        }
    }
}

/// Render-side smoothing of sim-owned NPC poses (the player has its own in `player.rs`).
fn interpolate_units(fixed: Res<Time<Fixed>>, mut q: Query<(&SimPose, &mut Transform)>) {
    let t = fixed.overstep_fraction();
    for (pose, mut tf) in &mut q {
        tf.translation = pose.prev_pos.lerp(pose.pos, t);
        tf.rotation =
            Quat::from_rotation_y(pose.prev_yaw).slerp(Quat::from_rotation_y(pose.yaw), t);
    }
}

/// A dead player must not walk around; movement input is the client's to gate.
fn freeze_dead_player(
    sim: Res<CombatSim>,
    player: Single<&SimUnit, With<Player>>,
    mut input: ResMut<PlayerInput>,
) {
    if sim.0.unit(player.0).is_some_and(|u| !u.alive) {
        *input = PlayerInput::default();
    }
}

const CAST_KEYS: [KeyCode; 4] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
];

fn cast_keys(
    keys: Res<ButtonInput<KeyCode>>,
    paused: Res<Paused>,
    sim: Res<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
) {
    if paused.0 {
        return;
    }
    for (slot, key) in CAST_KEYS.iter().enumerate() {
        if keys.just_pressed(*key) {
            cast_slot(&sim.0, &mut pending, player.0, slot);
        }
    }
}

/// Shared by keys and action-button clicks.
pub fn cast_slot(sim: &Sim, pending: &mut PendingCommands, caster: EntityId, slot: usize) {
    if let Some(&spell) = sim.content().action_bar().get(slot) {
        pending.0.push(Command::Cast { caster, spell });
    }
}

/// Shared by the T key and the Attack button.
pub fn toggle_auto_attack(
    sim: &Sim,
    pending: &mut PendingCommands,
    notices: &mut MessageWriter<UiNotice>,
    unit: EntityId,
    has_target: bool,
) {
    if sim.unit(unit).is_some_and(|u| u.auto_attack) {
        pending.0.push(Command::StopAutoAttack { unit });
    } else if has_target {
        pending.0.push(Command::StartAutoAttack { unit });
    } else {
        // The sim ignores a start without a target silently.
        notices.write(UiNotice(CastError::NoTarget.message().into()));
    }
}

fn auto_attack_key(
    keys: Res<ButtonInput<KeyCode>>,
    paused: Res<Paused>,
    sim: Res<CombatSim>,
    target: Res<Target>,
    mut pending: ResMut<PendingCommands>,
    mut notices: MessageWriter<UiNotice>,
    player: Single<&SimUnit, With<Player>>,
) {
    if !paused.0 && keys.just_pressed(KeyCode::KeyT) {
        toggle_auto_attack(
            &sim.0,
            &mut pending,
            &mut notices,
            player.0,
            target.0.is_some(),
        );
    }
}

/// Right-click (without steering drag) on a living hostile starts auto attack;
/// on a lootable corpse in reach, or F on the target, it loots.
fn loot_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    colliders: Res<Colliders>,
    paused: Res<Paused>,
    mut target: ResMut<Target>,
    sim: Res<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    mut notices: MessageWriter<UiNotice>,
    mut press: Local<Option<(Vec2, f32)>>,
    player: Single<(&Transform, &SimUnit), With<Player>>,
    units: Query<(Entity, &Transform, &Unit)>,
    sim_units: Query<&SimUnit>,
) {
    if paused.0 {
        return;
    }
    let mut corpse: Option<Entity> = None;
    let mut clicked = false;
    if buttons.just_pressed(MouseButton::Right) {
        *press = window.cursor_position().map(|c| (c, 0.0));
    }
    if let Some((_, travel)) = press.as_mut()
        && buttons.pressed(MouseButton::Right)
    {
        *travel += motion.delta.length();
    }
    if buttons.just_released(MouseButton::Right)
        && let Some((cursor, travel)) = press.take()
        && travel <= CLICK_DRAG_THRESHOLD
    {
        corpse = pick_unit(cursor, *camera, &colliders.0, &units);
        clicked = true;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        corpse = target.0;
    }
    let Some(entity) = corpse else { return };
    let Ok(su) = sim_units.get(entity) else {
        return;
    };
    let Some(state) = sim.0.unit(su.0) else {
        return;
    };
    if state.alive {
        if clicked && state.hostile {
            // SetTarget is pushed by tick_sim before pending commands, so the start sees the new target.
            target.0 = Some(entity);
            pending
                .0
                .push(Command::StartAutoAttack { unit: player.1.0 });
        }
        return;
    }
    if !state.lootable {
        return;
    }
    let (player_tf, player_su) = *player;
    let Ok((_, corpse_tf, _)) = units.get(entity) else {
        return;
    };
    if player_tf.translation.distance(corpse_tf.translation) > LOOT_RANGE {
        notices.write(UiNotice("You are too far away to loot that.".into()));
        return;
    }
    pending.0.push(Command::Loot {
        looter: player_su.0,
        corpse: su.0,
    });
}

/// Scripted fight for screenshots and smoke runs (`WOW_AUTOFIGHT=1`).
#[derive(Resource, Default)]
struct AutoFight {
    elapsed: f32,
    step: usize,
}

fn autofight(
    time: Res<Time>,
    mut state: ResMut<AutoFight>,
    mut target: ResMut<Target>,
    sim: Res<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
    units: Query<(Entity, &Unit)>,
) {
    state.elapsed += time.delta_secs();
    // (time, action): slot index to cast, None to select the hostile unit, or AUTO to start auto attack.
    const AUTO: Option<usize> = Some(usize::MAX);
    const SCRIPT: [(f32, Option<usize>); 5] = [
        (0.5, None),
        (0.7, AUTO),
        (1.0, Some(0)),
        (3.4, Some(1)),
        (6.2, Some(2)),
    ];
    let Some(&(at, action)) = SCRIPT.get(state.step) else {
        return;
    };
    if state.elapsed < at {
        return;
    }
    state.step += 1;
    match action {
        None => {
            target.0 = units.iter().find(|(_, u)| u.hostile).map(|(e, _)| e);
        }
        Some(usize::MAX) => pending.0.push(Command::StartAutoAttack { unit: player.0 }),
        Some(slot) => cast_slot(&sim.0, &mut pending, player.0, slot),
    }
}

/// Smoke-run aid: prints every sim event so a headless run can be checked from the log.
fn log_events(mut events: MessageReader<CombatEvent>) {
    for CombatEvent(ev) in events.read() {
        info!("sim event: {ev:?}");
    }
}
