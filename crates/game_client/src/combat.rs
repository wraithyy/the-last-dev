//! Bridge between the Bevy client and the authoritative `combat_core` simulation.
//!
//! The client owns player movement and reports it every tick; everything else
//! (HP, casts, auras, NPC positions, which units exist) flows back out of the
//! sim as state or events. Unit visuals are kept in sync with `Sim::units()`.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::arena::Colliders;
use crate::camera::CameraShake;
use crate::catalog::IdlePose;
use crate::collision::{nearest_free, to_core};
use crate::combat_anim::CombatAnim;
use crate::config::*;
use crate::game::GameFlow;
use crate::input::{Paused, PlayerInput};
use crate::net::{NetClient, NetWorld};
use crate::player::{Player, PlayerBody};
use crate::targeting::{Target, pick_unit};
use crate::units::{Unit, UnitAssets, spawn_unit};
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use combat_core::{
    AiState, CastError, Command, Content, ContentError, CooldownView, EntityId, Event, ItemTotals,
    QuestLog, QuestStatus, Sim, SpellDef, SpellId, Tick, UnitState, ZoneLinkView,
};
use std::collections::HashSet;

/// The offline sim. Online it stays frozen (not stepped) and comes back on disconnect.
#[derive(Resource)]
pub struct CombatSim(pub Sim);

/// The read side of the game state that HUD, VFX and audio need. Offline it is the local
/// `Sim`; online it is `NetWorld`, rebuilt from server snapshots and events, so presentation
/// systems work unchanged in both modes.
pub trait SimRead: Send + Sync {
    fn content(&self) -> &Content;
    fn unit(&self, id: EntityId) -> Option<&UnitState>;
    fn units(&self) -> Box<dyn Iterator<Item = &UnitState> + '_>;
    /// The local player.
    fn player(&self) -> Option<EntityId>;
    /// Group members in join order.
    fn players(&self) -> &[EntityId];
    fn cooldown(&self, caster: EntityId, spell: SpellId) -> CooldownView;
    fn quest_status(&self, quest: &str) -> QuestStatus;
    fn quests(&self) -> &QuestLog;
    fn inventory(&self) -> &[(String, u32)];
    fn item_totals(&self) -> ItemTotals;
    /// Remaining and total ms of an on-use item's cooldown, or the consumables' (`"consumable"`).
    fn item_cooldown(&self, player: EntityId, key: &str) -> (u32, u32);
    fn zone_cleared(&self, zone: &str) -> bool;
    fn zone_links(&self, zone: &str) -> Vec<ZoneLinkView>;
    fn known_spells(&self, unit: EntityId) -> Vec<SpellId>;
    fn now(&self) -> Tick;

    /// The action bar of `unit`'s class (keys 1..9, 0); the default bar for unknown units.
    fn bar_of(&self, unit: EntityId) -> &[SpellId] {
        match self.unit(unit) {
            Some(u) => self.content().bar_for_unit(&u.key),
            None => self.content().action_bar(),
        }
    }

    /// `unit`'s class key and Career Ladder picks ("" and none for non-players).
    fn talents_of(&self, unit: EntityId) -> (String, combat_core::TalentPicks) {
        let Some(u) = self.unit(unit) else {
            return (String::new(), [0; combat_core::TALENT_ROWS]);
        };
        let class = self
            .content()
            .class_of_unit(&u.key)
            .map_or_else(String::new, |c| c.key.clone());
        (class, u.talents)
    }

    /// The perks `unit`'s talents grant (cost, cast time and cooldown math for tooltips).
    fn talent_perks(&self, unit: EntityId) -> Vec<combat_core::ActivePerk> {
        let (class, picks) = self.talents_of(unit);
        self.content().talents.perks(&class, &picks)
    }

    /// The bar spell as `unit` casts it under its talents (a Career Ladder variant or itself).
    fn talent_spell(&self, unit: EntityId, spell: SpellId) -> SpellId {
        let (class, picks) = self.talents_of(unit);
        self.content().talents.spell_for(&class, &picks, spell)
    }

    /// The resurrect of `unit`'s class.
    fn resurrect_of(&self, unit: EntityId) -> Option<&SpellDef> {
        let key = self.unit(unit).map_or("", |u| u.key.as_str());
        self.content().resurrect_for_unit(key)
    }
}

impl SimRead for Sim {
    fn content(&self) -> &Content {
        Sim::content(self)
    }
    fn unit(&self, id: EntityId) -> Option<&UnitState> {
        Sim::unit(self, id)
    }
    fn units(&self) -> Box<dyn Iterator<Item = &UnitState> + '_> {
        Box::new(Sim::units(self))
    }
    fn player(&self) -> Option<EntityId> {
        Sim::player(self)
    }
    fn players(&self) -> &[EntityId] {
        Sim::players(self)
    }
    fn cooldown(&self, caster: EntityId, spell: SpellId) -> CooldownView {
        Sim::cooldown(self, caster, spell)
    }
    fn quest_status(&self, quest: &str) -> QuestStatus {
        Sim::quest_status(self, quest)
    }
    fn quests(&self) -> &QuestLog {
        Sim::quests(self)
    }
    fn inventory(&self) -> &[(String, u32)] {
        Sim::inventory(self)
    }
    fn item_totals(&self) -> ItemTotals {
        Sim::item_totals(self)
    }
    fn item_cooldown(&self, player: EntityId, key: &str) -> (u32, u32) {
        Sim::item_cooldown(self, player, key)
    }
    fn zone_cleared(&self, zone: &str) -> bool {
        Sim::zone_cleared(self, zone)
    }
    fn zone_links(&self, zone: &str) -> Vec<ZoneLinkView> {
        Sim::zone_links(self, zone)
    }
    fn known_spells(&self, unit: EntityId) -> Vec<SpellId> {
        Sim::known_spells(self, unit)
    }
    fn now(&self) -> Tick {
        Sim::now(self)
    }
}

/// Read-only game state for systems: `NetWorld` while online, else the offline sim.
#[derive(SystemParam)]
pub struct SimView<'w> {
    sim: Res<'w, CombatSim>,
    net: Option<Res<'w, NetWorld>>,
}

impl std::ops::Deref for SimView<'_> {
    type Target = dyn SimRead;
    fn deref(&self) -> &Self::Target {
        match &self.net {
            Some(net) => &**net,
            None => &self.sim.0,
        }
    }
}

/// Commands from Update-side input, flushed in `tick_sim` after `PlayerMoved`
/// so a cast always sees this tick's movement state.
#[derive(Resource, Default)]
pub struct PendingCommands(pub Vec<Command>);

/// Link from a Bevy entity to its simulation unit.
#[derive(Component, Clone, Copy)]
pub struct SimUnit(pub EntityId);

/// Every sim event of a tick, re-broadcast for HUD, floating text and VFX.
#[derive(Message, Clone)]
pub struct CombatEvent(pub Event);

/// The player spoke to a friendly unit (sim id) in reach; the quest dialog opens on it.
#[derive(Message)]
pub struct Talked(pub EntityId);

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
            .add_message::<Talked>()
            .configure_sets(Update, SimReady.run_if(resource_exists::<CombatSim>))
            .configure_sets(PostStartup, SimReady.run_if(resource_exists::<CombatSim>))
            .configure_sets(
                FixedPostUpdate,
                SimReady.run_if(resource_exists::<CombatSim>),
            )
            .configure_sets(
                RunFixedMainLoop,
                SimReady.run_if(resource_exists::<CombatSim>),
            )
            .add_systems(Startup, create_sim)
            .add_systems(PostStartup, spawn_sim_player.in_set(SimReady))
            .add_systems(
                FixedPostUpdate,
                (tick_sim, sync_units).chain().in_set(SimReady),
            )
            .add_systems(
                RunFixedMainLoop,
                (
                    freeze_player.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
                    interpolate_units.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
                )
                    .in_set(SimReady),
            )
            .add_systems(
                Update,
                (
                    cast_keys,
                    auto_attack_key,
                    coffee_key,
                    resurrect_key,
                    interact_input,
                )
                    .in_set(SimReady),
            );
        if std::env::var_os("WOW_AUTOFIGHT").is_some() {
            app.init_resource::<AutoFight>()
                .add_systems(Update, (autofight, log_events).in_set(SimReady))
                .add_systems(FixedPreUpdate, autofight_steer);
        }
    }
}

/// The embedded content; also used to restart with a fresh sim.
pub fn new_sim() -> Result<Sim, ContentError> {
    Content::from_ron(include_str!("../../../content/game.ron"))
        .map(|content| Sim::new(content, SIM_SEED))
}

fn create_sim(mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    match new_sim() {
        Ok(sim) => commands.insert_resource(CombatSim(sim)),
        Err(err) => {
            error!("embedded content/game.ron is invalid: {err:?}");
            exit.write(AppExit::error());
        }
    }
}

/// The game flow moves the player to the floor spawn right after this.
pub fn spawn_sim_player(
    mut commands: Commands,
    class: Res<crate::classes::PlayerClass>,
    mut sim: ResMut<CombatSim>,
    player: Single<(Entity, &PlayerBody), With<Player>>,
) {
    let (player_entity, body) = *player;
    let id = sim.0.spawn_player(&class.0, to_core(body.pos), body.yaw);
    commands
        .entity(player_entity)
        .insert((SimUnit(id), CombatAnim::default()));
}

fn v3(p: combat_core::Vec3) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

/// Online position corrections above this distance teleport the body instead of blending.
const CORRECTION_SNAP_M: f32 = 1.5;

/// One 30 Hz step. Offline it advances the local sim; online it sends this tick's input to
/// the server and takes the server events that arrived since the last step. Either way the
/// resulting events and unit states drive the same presentation below.
fn tick_sim(
    mut sim: ResMut<CombatSim>,
    mut net: Option<ResMut<NetWorld>>,
    client: Res<NetClient>,
    paused: Res<Paused>,
    colliders: Res<Colliders>,
    target: Res<Target>,
    mut shake: ResMut<CameraShake>,
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
    // The server does not pause, so online the world keeps moving under the menu.
    if paused.0 && net.is_none() {
        return;
    }
    let (body, player_id, _) = &mut *player;
    let player_id = player_id.0;

    let moving = last_pos
        .is_some_and(|p| Vec2::new(p.x - body.pos.x, p.z - body.pos.z).length_squared() > 1e-6);
    *last_pos = Some(body.pos);
    let wanted = target.0.and_then(|e| sim_units.get(e).ok()).map(|s| s.0);

    let tick_events = if let Some(net) = net.as_deref_mut() {
        if let Some(d) = net.take_correction() {
            let fixed = body.pos + v3(d);
            // Small errors blend through the render interpolation; big ones are teleports.
            if d.x * d.x + d.z * d.z > CORRECTION_SNAP_M * CORRECTION_SNAP_M {
                body.teleport(fixed);
            } else {
                body.pos = fixed;
            }
        }
        crate::net::online_step(
            net,
            &client,
            crate::net::MoveInput {
                pos: body.pos,
                yaw: body.yaw,
                moving,
                jumping: body.airborne(),
            },
            wanted,
            &mut pending.0,
        )
    } else {
        sim.0.push(Command::PlayerMoved {
            pos: to_core(body.pos),
            yaw: body.yaw,
            moving,
            jumping: body.airborne(),
        });
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
        sim.0.tick()
    };
    let view: &dyn SimRead = match net.as_deref() {
        Some(n) => n,
        None => &sim.0,
    };
    let coffee = view.content().channel_spell().map(|s| s.id);

    // Authoritative state back into the Bevy mirror.
    for (su, mut unit, mut pose, mut anim) in &mut npcs {
        let Some(state) = view.unit(su.0) else {
            continue;
        };
        unit.alive = state.alive;
        unit.hostile = state.hostile;
        pose.prev_pos = pose.pos;
        pose.prev_yaw = pose.yaw;
        pose.pos = v3(state.pos);
        pose.yaw = state.yaw;
        anim.casting = state.cast.is_some();
        anim.dead = !state.alive;
    }
    let (body, _, anim) = &mut *player;
    if let Some(state) = view.unit(player_id) {
        let drinking = state.cast.is_some_and(|c| Some(c.spell) == coffee);
        anim.drinking = drinking;
        anim.casting = state.cast.is_some() && !drinking;
        anim.dead = !state.alive;
    }

    for ev in &tick_events {
        match ev {
            Event::Respawned { unit } if *unit == player_id => {
                if let Some(state) = view.unit(player_id) {
                    body.teleport_facing(v3(state.pos), state.yaw);
                }
            }
            Event::Knockback {
                unit,
                dir,
                strength,
            } if *unit == player_id => {
                body.knock_back(v3(*dir), *strength);
                shake.trauma = (shake.trauma + 0.6).min(1.0);
            }
            // The sim does not check walls for teleports; keep the body out of them.
            Event::Teleported { unit, pos } if *unit == player_id => {
                let from = body.pos;
                body.teleport(nearest_free(v3(*pos), from, &colliders.0));
            }
            _ => {}
        }
    }
    apply_anim_events(player_id, coffee, anim, &mut npcs, &tick_events);

    for ev in tick_events {
        events.write(CombatEvent(ev));
    }
}

/// The sim's unit list (online: the server's, via `NetWorld`) is the source of truth: spawn
/// visuals for new units (zone loads, summons, joining players) and drop despawned ones.
fn sync_units(
    mut commands: Commands,
    mut sim: ResMut<CombatSim>,
    net: Option<Res<NetWorld>>,
    flow: Res<GameFlow>,
    assets: Res<UnitAssets>,
    visuals: Query<(Entity, &SimUnit), Without<Player>>,
) {
    let view: &dyn SimRead = match net.as_deref() {
        Some(n) => n,
        None => &sim.0,
    };
    let player = view.player();
    let mut shown = HashSet::new();
    for (e, su) in &visuals {
        if view.unit(su.0).is_some() {
            shown.insert(su.0);
        } else {
            commands.entity(e).despawn();
        }
    }
    let fresh: Vec<(UnitState, bool, Option<String>)> = view
        .units()
        .filter(|u| Some(u.id) != player && !shown.contains(&u.id))
        .map(|u| {
            let class = view.content().class_of_unit(&u.key).map(|c| c.key.clone());
            (u.clone(), view.players().contains(&u.id), class)
        })
        .collect();
    for (state, is_player, class) in &fresh {
        let pos = v3(state.pos);
        let pose = if *is_player {
            IdlePose::Standing
        } else {
            idle_pose(state, &flow)
        };
        // The dressing may seat an NPC the sim thinks is standing; seated units must not stroll.
        // Online the server owns the poses, so only the offline sim is told.
        if net.is_none() {
            sim.0.set_idle_pose(
                state.id,
                match pose {
                    IdlePose::Sleeping => Some(combat_core::IdlePose::Sleeping),
                    IdlePose::Sitting => Some(combat_core::IdlePose::Sitting),
                    IdlePose::Chatting => Some(combat_core::IdlePose::Chatting),
                    IdlePose::Standing => None,
                },
            );
        }
        let e = spawn_unit(
            &mut commands,
            &assets,
            &state.key,
            (pos, state.yaw),
            pose,
            state.hostile,
        );
        commands.entity(e).insert((
            SimUnit(state.id),
            CombatAnim {
                dead: !state.alive,
                ..default()
            },
            SimPose {
                prev_pos: pos,
                pos,
                prev_yaw: state.yaw,
                yaw: state.yaw,
            },
        ));
        if *is_player {
            commands.entity(e).insert(crate::net::RemotePlayer);
            if let Some(c) = class {
                commands
                    .entity(e)
                    .insert(crate::gear::Accessories(c.clone()));
            }
        }
    }
}

/// The floor dressing knows where the chairs and desks are, so the catalog's pose
/// for an NPC anchor wins (e.g. floor 1's IT Support asleep at a desk); the sim's
/// per-kind hint covers everything else.
fn idle_pose(state: &UnitState, flow: &GameFlow) -> IdlePose {
    let at = Vec3::new(state.pos.x, 0.0, state.pos.z);
    let dressed = flow
        .anchors
        .iter()
        .find(|(name, p, _)| name.starts_with("npc_") && p.distance(at) < 0.5)
        .and_then(|(name, ..)| flow.npc_dressing(name));
    if let Some((_, pose)) = dressed {
        return pose;
    }
    match state.idle_pose {
        Some(combat_core::IdlePose::Sleeping) => IdlePose::Sleeping,
        Some(combat_core::IdlePose::Chatting) => IdlePose::Chatting,
        Some(combat_core::IdlePose::Sitting) => IdlePose::Sitting,
        None => IdlePose::Standing,
    }
}

/// One-shot animation triggers use sequence numbers so the animation side
/// never has to clear flags (see `combat_anim.rs`).
fn apply_anim_events(
    player_id: EntityId,
    coffee: Option<combat_core::SpellId>,
    player_anim: &mut CombatAnim,
    npcs: &mut Query<
        (&SimUnit, &mut Unit, &mut SimPose, &mut CombatAnim),
        (Without<Player>, Without<PlayerBody>),
    >,
    events: &[Event],
) {
    let mut bump = |id: EntityId, f: &dyn Fn(&mut CombatAnim)| {
        if id == player_id {
            f(player_anim);
        } else if let Some((.., mut anim)) = npcs.iter_mut().find(|(su, ..)| su.0 == id) {
            f(&mut anim);
        }
    };
    for ev in events {
        match ev {
            // The sip has its own pose; a release or cast loop would fight it.
            Event::CastStarted { spell, .. } | Event::CastSucceeded { spell, .. }
                if Some(*spell) == coffee => {}
            Event::CastStarted { caster, spell, .. } => bump(*caster, &|a| a.cast_spell = spell.0),
            Event::CastSucceeded { caster, spell, .. } => bump(*caster, &|a| {
                a.cast_release_seq += 1;
                a.release_spell = spell.0;
            }),
            // The player's auto attack is a ranged spell-style gesture, not a melee chop.
            Event::MeleeSwing { attacker, .. } if *attacker == player_id => {
                bump(*attacker, &|a| {
                    a.cast_release_seq += 1;
                    a.release_spell = 0;
                });
            }
            Event::MeleeSwing { attacker, .. } => bump(*attacker, &|a| a.attack_seq += 1),
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

/// Dead, stunned or mid-transition players must not move; movement input is the client's to gate.
fn freeze_player(
    sim: SimView,
    flow: Res<GameFlow>,
    player: Single<&SimUnit, With<Player>>,
    mut input: ResMut<PlayerInput>,
) {
    if flow.input_locked() || sim.unit(player.0).is_some_and(|u| !u.alive || u.stunned) {
        *input = PlayerInput::default();
    }
}

/// True when the player may act at all (alive, not stunned, not in a transition).
pub fn can_act(sim: &dyn SimRead, flow: &GameFlow, player: EntityId) -> bool {
    !flow.input_locked() && sim.unit(player).is_some_and(|u| u.alive && !u.stunned)
}

/// Slot keys are physical positions (see `keybinds`), so the number row works on any layout
/// (e.g. the Czech "+ěščřžýáíé" row without Shift); the numpad is the alternative.
/// Slot order matches the class bar (`SimRead::bar_of`, keys 1..9, 0).
pub const SLOT_COUNT: usize = 10;

fn cast_keys(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
) {
    if paused.0 || !can_act(&*sim, &flow, player.0) {
        return;
    }
    for slot in 0..SLOT_COUNT {
        if keys.any_just_pressed(binds.slot_keys(slot)) {
            cast_slot(&*sim, &mut pending, player.0, slot);
        }
    }
}

/// Shared by keys and action-button clicks.
pub fn cast_slot(sim: &dyn SimRead, pending: &mut PendingCommands, caster: EntityId, slot: usize) {
    if let Some(&spell) = sim.bar_of(caster).get(slot) {
        pending.0.push(Command::Cast { caster, spell });
    }
}

/// Shared by the T key and the Attack button.
pub fn toggle_auto_attack(
    sim: &dyn SimRead,
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
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    sim: SimView,
    target: Res<Target>,
    mut pending: ResMut<PendingCommands>,
    mut notices: MessageWriter<UiNotice>,
    player: Single<&SimUnit, With<Player>>,
) {
    if !paused.0 && binds.just(&keys, crate::keybinds::Action::AutoAttack) {
        toggle_auto_attack(
            &*sim,
            &mut pending,
            &mut notices,
            player.0,
            target.0.is_some(),
        );
    }
}

/// Shared by the C key and the Coffee button. The sim validates (full mana, cooldown, movement).
pub fn cast_coffee_break(sim: &dyn SimRead, pending: &mut PendingCommands, caster: EntityId) {
    if let Some(spell) = sim.content().channel_spell() {
        pending.0.push(Command::Cast {
            caster,
            spell: spell.id,
        });
    }
}

fn coffee_key(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
) {
    if !paused.0
        && binds.just(&keys, crate::keybinds::Action::Coffee)
        && can_act(&*sim, &flow, player.0)
    {
        cast_coffee_break(&*sim, &mut pending, player.0);
    }
}

/// Shared by the V key and the reboot button: the co-op resurrect on the current target.
/// The sim checks the target (a dead teammate), range, line of sight and combat.
pub fn cast_resurrect(sim: &dyn SimRead, pending: &mut PendingCommands, caster: EntityId) {
    if let Some(spell) = sim.resurrect_of(caster) {
        pending.0.push(Command::Cast {
            caster,
            spell: spell.id,
        });
    }
}

fn resurrect_key(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
) {
    if !paused.0
        && binds.just(&keys, crate::keybinds::Action::Resurrect)
        && can_act(&*sim, &flow, player.0)
    {
        cast_resurrect(&*sim, &mut pending, player.0);
    }
}

/// Right-click (without steering drag): a living hostile starts auto attack, a
/// friendly NPC is talked to, a lootable corpse in reach is looted. F does the
/// same for the target, or talks to the nearest friendly in reach.
fn interact_input(
    (keys, binds): (Res<ButtonInput<KeyCode>>, Res<crate::keybinds::Keybinds>),
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    colliders: Res<Colliders>,
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    mut target: ResMut<Target>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    (mut notices, mut talked): (MessageWriter<UiNotice>, MessageWriter<Talked>),
    mut press: Local<Option<(Vec2, f32)>>,
    player: Single<(&Transform, &SimUnit), With<Player>>,
    units: Query<(Entity, &Transform, &Unit)>,
    sim_units: Query<&SimUnit>,
) {
    if paused.0 || flow.input_locked() {
        return;
    }
    let (player_tf, player_su) = *player;
    let reach = sim.content().config.loot_range_m;
    let mut picked: Option<Entity> = None;
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
        picked = pick_unit(cursor, *camera, &colliders.0, &units);
        clicked = true;
    }
    if binds.just(&keys, crate::keybinds::Action::Interact) {
        picked = target.0.or_else(|| {
            units
                .iter()
                .filter(|(_, tf, u)| {
                    !u.hostile && u.alive && tf.translation.distance(player_tf.translation) <= reach
                })
                .min_by(|a, b| {
                    let d = |t: &Transform| t.translation.distance(player_tf.translation);
                    d(a.1).total_cmp(&d(b.1))
                })
                .map(|(e, ..)| e)
        });
    }
    let Some(entity) = picked else { return };
    let (Ok(su), Ok((_, unit_tf, _))) = (sim_units.get(entity), units.get(entity)) else {
        return;
    };
    let Some(state) = sim.unit(su.0) else {
        return;
    };
    let far = player_tf.translation.distance(unit_tf.translation) > reach;
    if state.alive && state.hostile {
        if clicked {
            // SetTarget is pushed by tick_sim before pending commands, so the start sees the new target.
            target.0 = Some(entity);
            pending
                .0
                .push(Command::StartAutoAttack { unit: player_su.0 });
        }
    } else if state.alive {
        target.0 = Some(entity);
        if far {
            notices.write(UiNotice("You are too far away to talk to them.".into()));
        } else {
            pending.0.push(Command::Talk { unit: su.0 });
            talked.write(Talked(su.0));
        }
    } else if state.lootable {
        if far {
            notices.write(UiNotice("You are too far away to loot that.".into()));
        } else {
            pending.0.push(Command::Loot {
                looter: player_su.0,
                corpse: su.0,
            });
        }
    }
}

// ---- WOW_AUTOFIGHT: a scripted player for screenshots and smoke runs.

/// Spell ids of the action bar, by role (see content/game.ron).
const CODE_REVIEW: u32 = 1;
const REFACTOR: u32 = 2;
const GUARDRAILS: u32 = 3;
const REJECT_PR: u32 = 4;
const GIT_BLAME: u32 = 6;
const ROLLBACK: u32 = 7;
const CAFFEINE: u32 = 8;
const RUBBER_DUCK: u32 = 9;
const POSTMORTEM: u32 = 10;
const BY_HAND: u32 = 11;
/// Stand this close before casting, inside every 20-30 m spell range.
const AUTOFIGHT_RANGE: f32 = 18.0;

/// `WOW_RANGE=<m>` shortens the stand-off so close-up VFX shots fit the frame.
fn autofight_range() -> f32 {
    static RANGE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *RANGE.get_or_init(|| {
        std::env::var("WOW_RANGE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(AUTOFIGHT_RANGE)
    })
}
/// Corpses farther away than this are left alone (they never block the way to the next pull).
const AUTOFIGHT_LOOT_WALK: f32 = 24.0;

#[derive(Resource, Default)]
struct AutoFight {
    /// Where to walk this frame (yaw), or stand still.
    steer: Option<f32>,
    face: Option<f32>,
    cooldown: f32,
    dead_for: f32,
    /// Unstick: progress check and a short sidestep when walking makes no headway.
    anchor: Vec3,
    since: f32,
    sidestep: f32,
    side: f32,
    /// Cached `Sim::find_path` route (waypoints after the player) and what it was planned for.
    path: Vec<Vec3>,
    path_goal: Vec3,
    path_age: f32,
}

/// Yaw toward the next waypoint of a walkable route to `to`; replans twice a second or when the
/// goal moves, and falls back to the straight line when the sim finds no route.
fn walk_yaw(sim: &Sim, state: &mut AutoFight, from: Vec3, to: Vec3, dt: f32) -> f32 {
    state.path_age += dt;
    if state.path.is_empty() || state.path_age > 0.5 || state.path_goal.distance(to) > 1.5 {
        state.path = sim
            .find_path(to_core(from), to_core(to))
            .map(|p| p.into_iter().map(v3).collect())
            .unwrap_or_else(|| vec![to]);
        state.path_goal = to;
        state.path_age = 0.0;
    }
    while state.path.len() > 1 && from.distance(state.path[0]) < 0.35 {
        state.path.remove(0);
    }
    yaw_to(from, state.path[0])
}

fn yaw_to(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    (-d.x).atan2(-d.z)
}

/// Nearest living hostile, preferring anything already fighting the player (adds first).
fn pick_enemy(sim: &Sim, me: EntityId, at: Vec3) -> Option<&UnitState> {
    let dist = |u: &UnitState| v3(u.pos).distance(at);
    sim.units()
        .filter(|u| u.hostile && u.alive && u.ai != Some(AiState::Evade))
        .min_by(|a, b| {
            let score = |u: &UnitState| {
                let engaged = u.target == Some(me) && u.in_combat;
                // Engaged non-bosses first, so summons die before their summoner.
                dist(u) - if engaged && !u.boss { 60.0 } else { 0.0 }
            };
            score(a).total_cmp(&score(b))
        })
}

fn autofight(
    time: Res<Time>,
    mut state: ResMut<AutoFight>,
    mut target: ResMut<Target>,
    flow: Res<GameFlow>,
    sim: Res<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    player: Single<(&SimUnit, &PlayerBody), With<Player>>,
    units: Query<(Entity, &SimUnit)>,
) {
    let (me, body) = *player;
    let me = me.0;
    let dt = time.delta_secs();
    let was_steering = state.steer.is_some();
    state.steer = None;
    state.face = None;
    state.sidestep = (state.sidestep - dt).max(0.0);
    if was_steering {
        state.since += dt;
        if state.since > 1.0 {
            if body.pos.distance(state.anchor) < 0.5 {
                state.sidestep = 0.8;
                state.side = if state.side > 0.0 { -1.0 } else { 1.0 };
            }
            state.anchor = body.pos;
            state.since = 0.0;
        }
    }
    let Some(my) = sim.0.unit(me) else { return };
    if !my.alive {
        state.dead_for += time.delta_secs();
        if state.dead_for > 3.0 {
            state.dead_for = 0.0;
            pending.0.push(Command::RespawnPlayer);
        }
        return;
    }
    if flow.input_locked() || my.stunned {
        return;
    }
    let engaged = |u: &&UnitState| u.in_combat && u.target == Some(me);
    // Pick up the trophies of the last kill once nothing is fighting us.
    let reach = sim.0.content().config.loot_range_m;
    let corpse_dist = |u: &UnitState| v3(u.pos).distance(body.pos);
    // WOW_NO_LOOT leaves corpses lootable so screenshots can show them.
    if std::env::var_os("WOW_NO_LOOT").is_none()
        && !sim.0.units().any(|u| u.hostile && u.alive && engaged(&u))
        && let Some(corpse) = sim
            .0
            .units()
            .filter(|u| u.lootable && corpse_dist(u) < AUTOFIGHT_LOOT_WALK)
            .min_by(|a, b| corpse_dist(a).total_cmp(&corpse_dist(b)))
    {
        if corpse_dist(corpse) < reach - 0.5 {
            pending.0.push(Command::Loot {
                looter: me,
                corpse: corpse.id,
            });
        } else {
            state.steer = Some(walk_yaw(&sim.0, &mut state, body.pos, v3(corpse.pos), dt));
        }
        return;
    }
    let enemy = pick_enemy(&sim.0, me, body.pos);
    // Boss down and nothing on us: skip leftover trash and take the exit elevator.
    let Some(enemy) = enemy.filter(|u| !flow.cleared || engaged(u)) else {
        if flow.cleared
            && let Some((_, exit, _)) = flow.anchors.iter().find(|a| a.0 == "elevator_exit")
        {
            state.steer = Some(walk_yaw(&sim.0, &mut state, body.pos, *exit, dt));
        }
        return;
    };
    let enemy_pos = v3(enemy.pos);
    if let Some((e, _)) = units.iter().find(|(_, su)| su.0 == enemy.id)
        && target.0 != Some(e)
    {
        target.0 = Some(e);
        pending.0.push(Command::StartAutoAttack { unit: me });
    }
    let yaw = yaw_to(body.pos, enemy_pos);
    // Other classes play their bar generically: the first slot probes range, the highest
    // ready slot fires (enough for screenshots and smoke runs).
    let bar: Vec<combat_core::SpellId> = SimRead::bar_of(&sim.0, me).to_vec();
    let fullstack = bar.first() == Some(&combat_core::SpellId(CODE_REVIEW));
    let probe = bar
        .first()
        .copied()
        .unwrap_or(combat_core::SpellId(CODE_REVIEW));
    let probe_range = sim.0.content().spell(probe).map_or(20.0, |s| s.range_m);
    let stand_off = autofight_range().min(probe_range * 0.6);
    // The probe's verdict stands in for "can I hit from here" (range and line of sight).
    let blocked = matches!(
        sim.0.cooldown(me, probe).usable,
        Err(CastError::NoLineOfSight | CastError::OutOfRange)
    );
    if (blocked || body.pos.distance(enemy_pos) > stand_off) && my.cast.is_none() {
        state.steer = Some(walk_yaw(&sim.0, &mut state, body.pos, enemy_pos, dt));
        return;
    }
    state.face = Some(yaw);
    state.cooldown -= time.delta_secs();
    if state.cooldown > 0.0 {
        return;
    }
    let ready = |id: u32| sim.0.cooldown(me, combat_core::SpellId(id)).usable.is_ok();
    let has = |u: &UnitState, key: &str| u.auras.iter().any(|a| a.key == key);
    let hp = |u: &UnitState| u.hp as f32 / u.max_hp.max(1) as f32;
    let kick = enemy.cast.is_some_and(|c| c.interruptible);
    let generic = || {
        bar.iter()
            .rev()
            .map(|s| s.0)
            .find(|id| ready(*id))
            .filter(|_| my.cast.is_none())
    };
    let spell = if !fullstack {
        generic()
    } else if kick && ready(REJECT_PR) {
        Some(REJECT_PR)
    } else if kick && !enemy.boss && ready(RUBBER_DUCK) {
        Some(RUBBER_DUCK)
    } else if hp(my) < 0.5 && ready(ROLLBACK) {
        Some(ROLLBACK)
    } else if hp(my) < 0.75 && ready(GUARDRAILS) {
        Some(GUARDRAILS)
    } else if enemy.boss && ready(CAFFEINE) {
        Some(CAFFEINE)
    } else if my.cast.is_some() {
        None
    } else if hp(enemy) < 0.25 && ready(POSTMORTEM) {
        Some(POSTMORTEM)
    } else if !has(enemy, "git_blame") && ready(GIT_BLAME) {
        Some(GIT_BLAME)
    } else if enemy.boss && ready(BY_HAND) {
        Some(BY_HAND)
    } else if !has(enemy, "code_smell") && ready(CODE_REVIEW) {
        Some(CODE_REVIEW)
    } else if ready(REFACTOR) {
        Some(REFACTOR)
    } else if ready(CODE_REVIEW) {
        Some(CODE_REVIEW)
    } else {
        None
    };
    // QA hook: `WOW_CAST=<spell id>` casts only that spell, whenever it is ready.
    let spell = match std::env::var("WOW_CAST").ok().and_then(|v| v.parse().ok()) {
        Some(id) => ready(id).then_some(id).filter(|_| my.cast.is_none()),
        None => spell,
    };
    if let Some(id) = spell {
        pending.0.push(Command::Cast {
            caster: me,
            spell: combat_core::SpellId(id),
        });
        state.cooldown = 0.2;
    }
}

/// Fixed pre-update runs after the per-frame input snapshot and before the movement sim consumes it.
fn autofight_steer(state: Res<AutoFight>, mut input: ResMut<PlayerInput>) {
    if let Some(yaw) = state.steer.or(state.face) {
        input.face_yaw = Some(yaw);
    }
    if state.steer.is_some() {
        input.forward = 1.0;
        if state.sidestep > 0.0 {
            input.strafe = state.side;
        }
    }
}

/// Smoke-run aid: prints every sim event so a headless run can be checked from the log.
fn log_events(mut events: MessageReader<CombatEvent>) {
    for CombatEvent(ev) in events.read() {
        info!("sim event: {ev:?}");
    }
}
