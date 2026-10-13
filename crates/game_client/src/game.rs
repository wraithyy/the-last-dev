//! Game flow over the zone graph (campus, tower floors, data center, lab): which zone is loaded
//! (sim and visuals together), doors and elevators between zones with their locks, fade
//! transitions with loading tips, the victory screen and restart.
//!
//! Debug hooks: `WOW_ZONE=<id>` starts in that zone (sim-backed) at its minimum level,
//! `WOW_FLOOR=<1..5>` is `WOW_ZONE=tower/<n>`, `WOW_LEVEL=<n>` overrides the start level,
//! `WOW_PLAYER_AT=x,z,yaw_deg[,cam_deg]` moves the player after the first zone loads (optional
//! camera yaw offset, 180 = front view), `WOW_UNLOCK_ALL=1` opens every door,
//! `WOW_QUESTS=id[:active|ready|done],..` forces quest states at start (default `active`).
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::camera::OrbitCamera;
use crate::catalog::{self, IdlePose};
use crate::collision::{nearest_free, to_core};
use crate::combat::{
    CombatEvent, CombatSim, PendingCommands, SimRead, SimReady, SimUnit, SimView, UiNotice, new_sim,
};
use crate::copy;
use crate::floors::{
    BareStage, FloorLayout, LoadZone, build_zone, door_trigger, elevator_exit_trigger,
};
use crate::hud::{ACCENT, Banner, DIM, label, set_text};
use crate::net::{GameMode, NetWorld};
use crate::player::{Player, PlayerBody};
use crate::targeting::Target;
use crate::units::spawn_visual;
use bevy::prelude::*;
use combat_core::{Aabb, Content, Event, QuestStatus, Unlock};
use std::f32::consts::PI;

const FADE_SECS: f32 = 0.6;
/// The loading line stays readable for a moment on a fast load.
const FADE_HOLD_SECS: f32 = 0.5;
const DOOR_OPEN_SECS: f32 = 1.2;
/// A locked door repeats its reason at most this often while the player stands in it.
const LOCKED_NOTICE_SECS: f32 = 3.0;
/// Lets the AGI's reveal and the "cleared" banner play before the credits.
const VICTORY_DELAY_SECS: f32 = 8.0;
/// Where the run ends: the AGI Prototype's rack room.
const FINAL_ZONE: &str = "lab/2";
const LAMP_RED: LinearRgba = LinearRgba::rgb(6.0, 0.3, 0.2);
const LAMP_GREEN: LinearRgba = LinearRgba::rgb(0.3, 6.0, 0.6);

/// Load `zone` in the sim and the client together; `from` picks the arrival door.
#[derive(Message, Clone)]
pub struct EnterZone {
    pub zone: String,
    pub from: Option<String>,
}

/// A fresh sim replaced the old one (victory restart): HUD state keyed to old sim ids must go.
#[derive(Message)]
pub struct GameRestarted;

/// The system that swaps the sim to a new zone; client zone visuals load right after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FloorSwap;

#[derive(Default)]
enum Fade {
    #[default]
    Idle,
    Out {
        t: f32,
        to: String,
    },
    Hold {
        t: f32,
    },
    In {
        t: f32,
    },
}

/// One way out of the current zone.
pub struct Exit {
    pub anchor: String,
    pub to: String,
    pub trigger: Aabb,
}

#[derive(Resource, Default)]
pub struct GameFlow {
    pub zone: String,
    /// Tower floor number of the current zone, 0 elsewhere.
    pub floor: u8,
    pub outdoor: bool,
    /// The current zone's boss is dead.
    pub cleared: bool,
    /// Anchors of the current zone (the sim spawns units at these).
    pub anchors: Vec<(&'static str, Vec3, f32)>,
    pub exits: Vec<Exit>,
    fade: Fade,
    door_open: f32,
    elevator_open: bool,
    locked_cooldown: f32,
    tip: usize,
    victory_in: Option<f32>,
    pub victory: bool,
    pub unlock_all: bool,
    /// Run statistics for the victory screen.
    pub deaths: u32,
    started: f32,
}

impl GameFlow {
    /// Movement and casting are frozen during a zone transition and on the victory screen.
    pub fn input_locked(&self) -> bool {
        !matches!(self.fade, Fade::Idle) || self.victory
    }

    /// The dressing's NPC (and pose) at a layout anchor of the current zone.
    pub fn npc_dressing(&self, anchor: &str) -> Option<(&'static str, IdlePose)> {
        if self.floor > 0 {
            catalog::npc_at(self.floor, anchor)
        } else {
            catalog::zone_npc_at(&self.zone, anchor)
        }
    }
}

/// "Tower floor 3 (Sales & Marketing)" for towers, the content name elsewhere.
pub fn zone_label(content: &Content, id: &str) -> String {
    let name = content.zone(id).map_or(id, |z| z.name.as_str());
    match id.strip_prefix("tower/") {
        Some(n) => format!("Tower floor {n} ({name})"),
        None => name.to_string(),
    }
}

/// The building part of a zone name: "Innovation Lab: Showroom" -> "Innovation Lab".
pub fn building_name(content: &Content, id: &str) -> String {
    let name = content.zone(id).map_or(id, |z| z.name.as_str());
    name.split(':').next().unwrap_or(name).trim().to_string()
}

/// Why a gated link is closed, in the voice of the badge reader.
pub fn lock_reason(content: &Content, requires: Option<&Unlock>) -> String {
    match requires {
        Some(Unlock::ZoneCleared(z)) => format!("Requires: clear {}", zone_label(content, z)),
        Some(Unlock::BossKilled(k)) => format!(
            "Requires: defeat {}",
            content.unit_def(k).map_or(k.as_str(), |u| u.name.as_str())
        ),
        Some(Unlock::QuestDone(q)) => format!(
            "Requires: close the ticket \"{}\"",
            content.quest(q).map_or(q.as_str(), |d| d.name.as_str())
        ),
        None => "The badge reader is thinking.".into(),
    }
}

/// The `requires` of the link `anchor` of zone `zone`, if gated.
fn link_requires<'a>(content: &'a Content, zone: &str, anchor: &str) -> Option<&'a Unlock> {
    content
        .zone(zone)?
        .links
        .iter()
        .find(|l| l.anchor == anchor)?
        .requires
        .as_ref()
}

#[derive(Component)]
struct ExitDoor;
#[derive(Component)]
struct DoorPanel {
    /// -1 or 1: which half of the door pair.
    side: f32,
    center: Vec3,
    lateral: Vec3,
}
#[derive(Component)]
struct DoorLight;
#[derive(Component)]
struct FadeOverlay;
#[derive(Component)]
struct FadeText;
#[derive(Component)]
struct FadeTip;
#[derive(Component)]
struct VictoryScreen;
#[derive(Component)]
struct VictoryPrompt;
#[derive(Component)]
struct VictoryStats;
/// Client-only figures (ambient NPCs, the AGI's operator) that go with the zone.
#[derive(Component)]
pub struct ZoneDecor;

#[derive(Resource)]
struct DoorAssets {
    cube: Handle<Mesh>,
    panel: Handle<StandardMaterial>,
    shaft: Handle<StandardMaterial>,
    /// Re-created per zone so opening one elevator never recolours another.
    lamp: Option<Handle<StandardMaterial>>,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameFlow>()
            .add_message::<EnterZone>()
            .add_message::<GameRestarted>()
            .add_systems(Startup, (setup_doors, spawn_overlays))
            .add_systems(
                PostStartup,
                start_game
                    .after(crate::combat::spawn_sim_player)
                    .in_set(SimReady),
            )
            .add_systems(
                Update,
                (
                    restart_on_r,
                    enter_zone.in_set(FloorSwap),
                    zone_events,
                    check_exits,
                    run_fade,
                    animate_doors,
                    update_victory,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn env_u8(name: &str) -> Option<u8> {
    std::env::var(name).ok()?.trim().parse().ok()
}

/// `WOW_QUESTS=id[:active|ready|done],..`
fn debug_quests() -> Vec<(String, QuestStatus)> {
    let Ok(v) = std::env::var("WOW_QUESTS") else {
        return Vec::new();
    };
    v.split(',')
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let (id, state) = p.trim().split_once(':').unwrap_or((p.trim(), "active"));
            let status = match state {
                "ready" => QuestStatus::Ready,
                "done" => QuestStatus::Done,
                _ => QuestStatus::Active,
            };
            (id.to_string(), status)
        })
        .collect()
}

fn start_game(
    bare: Option<Res<BareStage>>,
    time: Res<Time>,
    mut sim: ResMut<CombatSim>,
    mut flow: ResMut<GameFlow>,
    mut enter: MessageWriter<EnterZone>,
) {
    if bare.is_some() {
        return;
    }
    let zone = std::env::var("WOW_ZONE")
        .ok()
        .or_else(|| env_u8("WOW_FLOOR").map(|f| format!("tower/{f}")))
        .filter(|z| sim.0.content().zone(z).is_some())
        .unwrap_or_else(|| "campus".into());
    let zone_min = sim.0.content().zone(&zone).map_or(1, |z| z.level_range.0);
    let level = env_u8("WOW_LEVEL").unwrap_or(zone_min);
    if level > 1 {
        sim.0.set_player_level(level);
    }
    for (id, status) in debug_quests() {
        if !sim.0.debug_set_quest(&id, status) {
            warn!("WOW_QUESTS: unknown quest {id:?}");
        }
    }
    flow.unlock_all = std::env::var_os("WOW_UNLOCK_ALL").is_some();
    flow.started = time.elapsed_secs();
    enter.write(EnterZone { zone, from: None });
}

fn debug_player_at() -> Option<(f32, f32, f32, f32)> {
    let v = std::env::var("WOW_PLAYER_AT").ok()?;
    let n: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    match n[..] {
        [x, z, yaw] => Some((x, z, yaw.to_radians(), 0.0)),
        [x, z, yaw, cam] => Some((x, z, yaw.to_radians(), cam.to_radians())),
        _ => None,
    }
}

/// In front of the door that leads back to `from`, clear of its trigger; else the zone spawn.
fn arrival(layout: &FloorLayout, from: Option<&str>) -> (Vec3, f32) {
    let door = from.and_then(|f| layout.anchor(&format!("door:{f}")));
    match door {
        Some((pos, yaw)) => {
            let facing = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
            (nearest_free(pos + facing * 2.6, pos, &layout.blockers), yaw)
        }
        None => layout
            .anchor("player_spawn")
            .unwrap_or((Vec3::ZERO, -PI / 2.0)),
    }
}

fn enter_zone(
    mut commands: Commands,
    mut requests: MessageReader<EnterZone>,
    mut sim: ResMut<CombatSim>,
    net: Option<Res<NetWorld>>,
    mut flow: ResMut<GameFlow>,
    mut target: ResMut<Target>,
    mut doors: ResMut<DoorAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut load: MessageWriter<LoadZone>,
    mut banners: MessageWriter<Banner>,
    mut body: Single<&mut PlayerBody, With<Player>>,
    mut cam: Single<&mut OrbitCamera>,
    old: Query<Entity, Or<(With<ExitDoor>, With<ZoneDecor>)>>,
    mut first: Local<bool>,
) {
    let Some(req) = requests.read().last().cloned() else {
        return;
    };
    let Some(layout) = build_zone(&req.zone) else {
        error!("unknown zone {:?}", req.zone);
        return;
    };
    let anchors = |name: &str| layout.anchor(name).map(|(p, yaw)| (to_core(p), yaw));
    // Online the server has already loaded the zone; only the client side follows.
    if net.is_none()
        && let Err(err) = sim
            .0
            .load_zone(&req.zone, &anchors, layout.blockers.clone())
    {
        error!("cannot load zone {}: {err:?}", req.zone);
        return;
    }
    let view: &dyn SimRead = match net.as_deref() {
        Some(n) => n,
        None => &sim.0,
    };
    load.write(LoadZone(req.zone.clone()));

    let first_load = !std::mem::replace(&mut *first, true);
    let (spawn, spawn_yaw) = arrival(&layout, req.from.as_deref());
    let (pos, yaw, cam_offset) = match debug_player_at().filter(|_| first_load) {
        Some((x, z, yaw, cam)) => (Vec3::new(x, 0.0, z), yaw, cam),
        None => (spawn, spawn_yaw, 0.0),
    };
    body.teleport_facing(pos, yaw);
    cam.set_yaw(yaw + cam_offset);
    target.0 = None;

    let content = view.content();
    flow.zone.clone_from(&req.zone);
    flow.floor = if layout.zone.starts_with("tower/") {
        layout.id
    } else {
        0
    };
    flow.outdoor = layout.outdoor;
    flow.anchors.clone_from(&layout.anchors);
    flow.exits = view
        .zone_links(&req.zone)
        .into_iter()
        .filter(|l| layout.anchor(&l.anchor).is_some())
        .map(|l| Exit {
            trigger: if l.anchor == "elevator_exit" {
                elevator_exit_trigger(&layout)
            } else {
                door_trigger(&layout, &l.anchor)
            },
            anchor: l.anchor,
            to: l.to,
        })
        .collect();
    flow.door_open = 0.0;
    flow.elevator_open = false;
    flow.locked_cooldown = 0.0;
    let title = match flow.floor {
        0 => content
            .zone(&req.zone)
            .map_or(req.zone.clone(), |z| z.name.clone()),
        n => format!("Floor {n} - {}", layout.name),
    };
    banners.write(Banner {
        title,
        subtitle: zone_tagline(&req.zone).into(),
        color: ACCENT,
    });

    for e in &old {
        commands.entity(e).despawn();
    }
    if let Some((exit, exit_yaw)) = layout.anchor("elevator_exit") {
        spawn_exit_doors(&mut commands, &mut doors, &mut materials, exit, exit_yaw);
    }
    // NPC anchors the sim leaves empty get a figure from the dressing (outside the towers).
    if flow.floor == 0 {
        let used: Vec<&str> = content
            .zone(&req.zone)
            .map(|z| z.friendly.iter().map(|f| f.anchor.as_str()).collect())
            .unwrap_or_default();
        for (name, at, yaw) in &layout.anchors {
            if !name.starts_with("npc_") || used.contains(name) {
                continue;
            }
            if let Some((key, pose)) = flow.npc_dressing(name) {
                let e = spawn_visual(&mut commands, key, *at, *yaw, pose);
                commands.entity(e).insert(ZoneDecor);
            }
        }
    }
}

fn zone_tagline(id: &str) -> &'static str {
    match id {
        "campus" => "Badge in. The AI has already rejected your expense report.",
        "tower/1" => "Where the vibes are immaculate and the tests are optional.",
        "tower/2" => "This meeting could have been an email. The email could have been nothing.",
        "tower/3" => "Selling features we have not built to customers we do not have.",
        "tower/4" => "Every Gantt chart is a work of fiction.",
        "tower/5" => "The view is great. The strategy is a chatbot.",
        "datacenter/1" => "Ten megawatts so a model can summarise a two-line email.",
        "datacenter/2" => "The cooling budget was reallocated to a keynote about cooling.",
        "lab/1" => "Demo Day. Everything works, as long as nobody touches it.",
        "lab/2" => "Definitely not sentient. Please stop asking it how it feels.",
        _ => "A new sprint begins.",
    }
}

fn unlock_subtitle(id: &str) -> &'static str {
    match id {
        "datacenter/1" => {
            "Facilities found the data center. It was the warm building humming 'scale'."
        }
        "lab/1" => {
            "With the CEO gone, the AGI Prototype is in charge. It is behind the Innovation Lab."
        }
        _ => "Access granted. The access system is as surprised as you are.",
    }
}

fn setup_doors(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(DoorAssets {
        cube: meshes.add(Cuboid::from_length(1.0)),
        panel: materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.58, 0.62),
            metallic: 0.8,
            perceptual_roughness: 0.35,
            ..default()
        }),
        shaft: materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.05, 0.06),
            emissive: LinearRgba::rgb(0.6, 0.5, 0.3),
            ..default()
        }),
        lamp: None,
    });
}

/// Sliding doors and a status lamp in front of the static elevator dressing, so
/// the exit visibly unlocks. Geometry mirrors `kit::Plan::elevator` (doors 1.8 m
/// behind the anchor, LED 1.9 m to the side).
fn spawn_exit_doors(
    commands: &mut Commands,
    doors: &mut DoorAssets,
    materials: &mut Assets<StandardMaterial>,
    anchor: Vec3,
    yaw: f32,
) {
    let rot = Quat::from_rotation_y(yaw);
    let facing = rot * Vec3::NEG_Z;
    let lateral = rot * Vec3::X;
    let plane = anchor - facing * 1.5 + Vec3::Y * 1.2;
    commands.spawn((
        ExitDoor,
        Mesh3d(doors.cube.clone()),
        MeshMaterial3d(doors.shaft.clone()),
        Transform::from_translation(plane - facing * 0.025)
            .with_rotation(rot)
            .with_scale(Vec3::new(1.96, 2.4, 0.01)),
    ));
    for side in [-1.0, 1.0] {
        commands.spawn((
            ExitDoor,
            DoorPanel {
                side,
                center: plane,
                lateral,
            },
            Mesh3d(doors.cube.clone()),
            MeshMaterial3d(doors.panel.clone()),
            Transform::from_translation(plane + lateral * side * 0.5)
                .with_rotation(rot)
                .with_scale(Vec3::new(0.98, 2.4, 0.04)),
        ));
    }
    let lamp = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LAMP_RED,
        ..default()
    });
    doors.lamp = Some(lamp.clone());
    commands.spawn((
        ExitDoor,
        Mesh3d(doors.cube.clone()),
        MeshMaterial3d(lamp),
        Transform::from_translation(anchor - facing * 1.49 + Vec3::new(0.0, 1.25, 1.9))
            .with_rotation(rot)
            .with_scale(Vec3::new(0.14, 0.14, 0.05)),
    ));
    commands.spawn((
        ExitDoor,
        DoorLight,
        PointLight {
            color: Color::srgb(1.0, 0.2, 0.15),
            intensity: 15_000.0,
            range: 8.0,
            ..default()
        },
        Transform::from_translation(anchor - facing * 0.6 + Vec3::Y * 2.8),
    ));
}

fn zone_events(
    mut commands: Commands,
    mut events: MessageReader<CombatEvent>,
    mut flow: ResMut<GameFlow>,
    sim: SimView,
    mode: Res<GameMode>,
    mut banners: MessageWriter<Banner>,
    units: Query<(&SimUnit, &Transform)>,
) {
    let content = sim.content();
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::ZoneCleared { zone } => {
                let (title, subtitle) = match zone.as_str() {
                    "tower/5" => (
                        "Floor cleared",
                        "The board is calling an emergency meeting.",
                    ),
                    z if z.starts_with("tower/") => {
                        ("Floor cleared", "The elevator is unlocked. Take it up.")
                    }
                    FINAL_ZONE => (
                        "AGI Prototype shut down",
                        "Someone should check what was inside the rack.",
                    ),
                    _ => ("Zone cleared", "Facilities will take credit for this."),
                };
                banners.write(Banner {
                    title: title.into(),
                    subtitle: subtitle.into(),
                    color: Color::srgb(0.4, 1.0, 0.5),
                });
                if zone == FINAL_ZONE {
                    flow.victory_in = Some(VICTORY_DELAY_SECS);
                }
            }
            // Tower floors announce their elevator with the floor clear.
            Event::Unlocked { zone } if !zone.starts_with("tower/") => {
                banners.write(Banner {
                    title: format!("New zone unlocked: {}", building_name(content, zone)),
                    subtitle: unlock_subtitle(zone).into(),
                    color: Color::srgb(1.0, 0.82, 0.3),
                });
            }
            // Online the server moves the group; the client fades and follows.
            Event::ZoneLoaded { zone }
                if mode.is_online() && *zone != flow.zone && matches!(flow.fade, Fade::Idle) =>
            {
                flow.fade = Fade::Out {
                    t: 0.0,
                    to: zone.clone(),
                };
                flow.tip = flow.tip.wrapping_add(1);
            }
            Event::Death { unit, .. } if Some(*unit) == sim.player() => flow.deaths += 1,
            // The AGI's shell opens on death (rigs.rs); this is who was inside.
            Event::Death { unit, .. }
                if sim.unit(*unit).is_some_and(|u| u.key == "agi_prototype") =>
            {
                if let Some((_, tf)) = units.iter().find(|(su, _)| su.0 == *unit) {
                    let (yaw, ..) = tf.rotation.to_euler(EulerRot::YXZ);
                    let e = spawn_visual(
                        &mut commands,
                        "meatproxy",
                        tf.translation,
                        yaw,
                        IdlePose::Sitting,
                    );
                    commands.entity(e).insert(ZoneDecor);
                }
            }
            _ => {}
        }
    }
}

fn check_exits(
    time: Res<Time>,
    mut flow: ResMut<GameFlow>,
    sim: SimView,
    mode: Res<GameMode>,
    mut notices: MessageWriter<UiNotice>,
    body: Single<&PlayerBody, With<Player>>,
) {
    flow.locked_cooldown -= time.delta_secs();
    flow.cleared = sim.zone_cleared(&flow.zone);
    let links = sim.zone_links(&flow.zone);
    let all = flow.unlock_all;
    let open = |anchor: &str| all || links.iter().any(|l| l.anchor == anchor && l.unlocked);
    flow.elevator_open = open("elevator_exit");
    let alive = sim
        .player()
        .and_then(|p| sim.unit(p))
        .is_some_and(|u| u.alive);
    // Dying mid fade-out cancels the trip, so the corpse is not teleported into the next zone.
    if let Fade::Out { t, .. } = flow.fade
        && !alive
    {
        flow.fade = Fade::In {
            t: FADE_SECS - t.min(FADE_SECS),
        };
    }
    // Online the server decides when the group travels (see `zone_events`).
    if !matches!(flow.fade, Fade::Idle) || flow.victory || !alive || mode.is_online() {
        return;
    }
    let p = body.pos;
    let Some(exit) = flow.exits.iter().find(|e| {
        let b = e.trigger;
        p.x > b.min.x && p.x < b.max.x && p.z > b.min.z && p.z < b.max.z && p.y < b.max.y
    }) else {
        return;
    };
    if open(&exit.anchor) {
        flow.fade = Fade::Out {
            t: 0.0,
            to: exit.to.clone(),
        };
        flow.tip = flow.tip.wrapping_add(1);
    } else if flow.locked_cooldown <= 0.0 {
        let reason = lock_reason(
            sim.content(),
            link_requires(sim.content(), &flow.zone, &exit.anchor),
        );
        notices.write(UiNotice(reason));
        flow.locked_cooldown = LOCKED_NOTICE_SECS;
    }
}

fn run_fade(
    time: Res<Time>,
    mut flow: ResMut<GameFlow>,
    mut enter: MessageWriter<EnterZone>,
    mut overlay: Single<(&mut BackgroundColor, &mut Visibility), With<FadeOverlay>>,
    mut line: Single<&mut TextColor, (With<FadeText>, Without<FadeTip>)>,
    mut tip: Single<(&mut Text, &mut TextColor), (With<FadeTip>, Without<FadeText>)>,
) {
    let dt = time.delta_secs();
    let from = flow.zone.clone();
    let alpha = match &mut flow.fade {
        Fade::Idle => 0.0,
        Fade::Out { t, to } => {
            *t += dt;
            if *t >= FADE_SECS {
                enter.write(EnterZone {
                    zone: to.clone(),
                    from: Some(from),
                });
                flow.fade = Fade::Hold { t: 0.0 };
                1.0
            } else {
                *t / FADE_SECS
            }
        }
        Fade::Hold { t } => {
            *t += dt;
            if *t >= FADE_HOLD_SECS {
                flow.fade = Fade::In { t: 0.0 };
            }
            1.0
        }
        Fade::In { t } => {
            *t += dt;
            if *t >= FADE_SECS {
                flow.fade = Fade::Idle;
                0.0
            } else {
                1.0 - *t / FADE_SECS
            }
        }
    };
    // Hidden when clear: the labels' drop shadows do not fade with their text colour.
    overlay.1.set_if_neq(if alpha > 0.01 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    overlay.0.0 = Color::srgba(0.0, 0.0, 0.0, alpha);
    line.0 = ACCENT.with_alpha(alpha);
    let (text, color) = &mut *tip;
    set_text(
        text,
        copy::LOADING_TIPS[flow.tip % copy::LOADING_TIPS.len()],
    );
    color.0 = DIM.with_alpha(alpha);
}

fn animate_doors(
    time: Res<Time>,
    mut flow: ResMut<GameFlow>,
    doors: Res<DoorAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut panels: Query<(&DoorPanel, &mut Transform)>,
    mut lights: Query<&mut PointLight, With<DoorLight>>,
) {
    if !flow.elevator_open || flow.door_open >= 1.0 {
        return;
    }
    if flow.door_open == 0.0 {
        if let Some(mut m) = doors.lamp.as_ref().and_then(|h| materials.get_mut(h)) {
            m.emissive = LAMP_GREEN;
        }
        for mut light in &mut lights {
            light.color = Color::srgb(0.3, 1.0, 0.45);
            light.intensity = 60_000.0;
        }
    }
    flow.door_open = (flow.door_open + time.delta_secs() / DOOR_OPEN_SECS).min(1.0);
    // Smoothstep, then each half shrinks towards its outer edge (retracting into the frame).
    let k = flow.door_open * flow.door_open * (3.0 - 2.0 * flow.door_open);
    let width = 0.98 + (0.06 - 0.98) * k;
    for (panel, mut tf) in &mut panels {
        tf.scale.x = width;
        tf.translation = panel.center + panel.lateral * panel.side * (0.99 - width / 2.0);
    }
}

fn spawn_overlays(mut commands: Commands) {
    commands
        .spawn((
            FadeOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(10),
                padding: UiRect::horizontal(px(40)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Visibility::Hidden,
            // Above the banners (58), so a queued zone banner never prints over the loading line.
            GlobalZIndex(59),
        ))
        .with_children(|c| {
            c.spawn((
                FadeText,
                label(copy::LOADING_LINE, 24.0),
                TextColor(Color::NONE),
            ));
            c.spawn((
                FadeTip,
                label("", 14.0),
                TextColor(Color::NONE),
                TextLayout::justify(Justify::Center),
                Node {
                    max_width: px(640),
                    ..default()
                },
            ));
        });
    commands
        .spawn((
            VictoryScreen,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(10),
                padding: UiRect::horizontal(px(40)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.015, 0.03, 0.92)),
            GlobalZIndex(60),
        ))
        .with_children(|c| {
            c.spawn((label("// final sprint retrospective", 13.0), TextColor(DIM)));
            c.spawn((
                label(copy::VICTORY_TITLE, 34.0),
                TextColor(Color::srgb(1.0, 0.82, 0.3)),
                TextLayout::justify(Justify::Center),
            ));
            c.spawn((
                label(copy::VICTORY_SUBTITLE, 18.0),
                TextColor(Color::srgb(0.9, 0.92, 1.0)),
                TextLayout::justify(Justify::Center),
            ));
            c.spawn(Node {
                height: px(12),
                ..default()
            });
            c.spawn((label("$ git shortlog --stats", 13.0), TextColor(ACCENT)));
            c.spawn((
                VictoryStats,
                label("", 13.0),
                TextColor(Color::srgb(0.85, 0.9, 0.98)),
            ));
            c.spawn(Node {
                height: px(12),
                ..default()
            });
            c.spawn((label("$ git log --credits", 13.0), TextColor(ACCENT)));
            for line in copy::CREDITS {
                c.spawn((label(line, 13.0), TextColor(Color::srgb(0.7, 0.76, 0.86))));
            }
            c.spawn(Node {
                height: px(12),
                ..default()
            });
            c.spawn((
                VictoryPrompt,
                label(copy::VICTORY_PROMPT, 18.0),
                TextColor(ACCENT),
            ));
        });
}

fn update_victory(
    time: Res<Time>,
    sim: SimView,
    mut flow: ResMut<GameFlow>,
    mut screen: Single<&mut Node, With<VictoryScreen>>,
    mut prompt: Single<&mut TextColor, With<VictoryPrompt>>,
    mut stats: Single<&mut Text, With<VictoryStats>>,
) {
    if let Some(left) = flow.victory_in.as_mut() {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            flow.victory_in = None;
            flow.victory = true;
            let me = sim.player().and_then(|p| sim.unit(p));
            let mins = ((time.elapsed_secs() - flow.started) / 60.0).max(0.0) as u32;
            let trophies: u32 = sim.inventory().iter().map(|(_, n)| n).sum();
            let text = format!(
                "level {:>2}   tickets closed {:>2}   trophies {:>3}   deaths {:>2}   time {}h {:02}m",
                me.map_or(1, |u| u.level),
                sim.quests().completed.len(),
                trophies,
                flow.deaths,
                mins / 60,
                mins % 60
            );
            set_text(&mut stats, &text);
        }
    }
    screen.display = if flow.victory {
        Display::Flex
    } else {
        Display::None
    };
    // Terminal-cursor blink on the restart prompt.
    let on = (time.elapsed_secs() * 2.0).fract() < 0.6;
    prompt.0 = ACCENT.with_alpha(if on { 1.0 } else { 0.35 });
}

/// A fresh sim (level 1, campus); the Bevy player entity is kept and relinked.
fn restart_on_r(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut flow: ResMut<GameFlow>,
    mut sim: ResMut<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    mut target: ResMut<Target>,
    mut enter: MessageWriter<EnterZone>,
    mut restarted: MessageWriter<GameRestarted>,
    mode: Res<GameMode>,
    player: Single<(Entity, &PlayerBody), With<Player>>,
    visuals: Query<Entity, (With<SimUnit>, Without<Player>)>,
) {
    // The server owns an online run; R only restarts the solo game.
    if !flow.victory || !keys.just_pressed(KeyCode::KeyR) || mode.is_online() {
        return;
    }
    let fresh = match new_sim() {
        Ok(s) => s,
        Err(err) => {
            error!("cannot restart: {err:?}");
            return;
        }
    };
    sim.0 = fresh;
    let (entity, body) = *player;
    let id = sim.0.spawn_player(to_core(body.pos), body.yaw);
    commands.entity(entity).insert(SimUnit(id));
    // New sim ids restart at 1, so old visuals must go before the sync could match them.
    for e in &visuals {
        commands.entity(e).despawn();
    }
    pending.0.clear();
    target.0 = None;
    let unlock_all = flow.unlock_all;
    *flow = GameFlow {
        unlock_all,
        started: time.elapsed_secs(),
        ..default()
    };
    enter.write(EnterZone {
        zone: "campus".into(),
        from: None,
    });
    restarted.write(GameRestarted);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        match crate::combat::new_sim() {
            Ok(s) => s.content().clone(),
            Err(e) => panic!("{e:?}"),
        }
    }

    #[test]
    fn lock_reasons_name_what_to_do() {
        let c = content();
        let dc = link_requires(&c, "campus", "door:datacenter/1");
        assert_eq!(
            lock_reason(&c, dc),
            "Requires: clear Tower floor 3 (Sales & Marketing)"
        );
        let lab = link_requires(&c, "campus", "door:lab/1");
        assert!(lock_reason(&c, lab).starts_with("Requires: defeat "));
        assert!(link_requires(&c, "campus", "door:tower/1").is_none());
        assert_eq!(building_name(&c, "lab/1"), "Innovation Lab");
    }

    #[test]
    fn every_link_anchor_exists_in_its_layout_and_back_doors_land_outside_triggers() {
        let c = content();
        for z in &c.zones {
            let Some(l) = build_zone(&z.id) else {
                panic!("no layout for {}", z.id)
            };
            for link in &z.links {
                assert!(
                    l.anchor(&link.anchor).is_some(),
                    "{}: {}",
                    z.id,
                    link.anchor
                );
            }
            // Arriving from any neighbour must not drop the player into a door trigger.
            for from in c.zones.iter().map(|o| o.id.as_str()) {
                let (p, _) = arrival(&l, Some(from));
                for link in z.links.iter().filter(|k| k.anchor.starts_with("door:")) {
                    let b = door_trigger(&l, &link.anchor);
                    let inside = p.x > b.min.x && p.x < b.max.x && p.z > b.min.z && p.z < b.max.z;
                    assert!(!inside, "{} from {from}: lands in {}", z.id, link.anchor);
                }
            }
        }
    }
}
