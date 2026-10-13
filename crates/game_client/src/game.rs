//! Game flow over the five-floor tower: which floor is loaded (sim and visuals
//! together), the exit elevator, fade transitions, the victory screen and restart.
//!
//! Debug hooks: `WOW_FLOOR=<1..5>` starts on that floor at its minimum level,
//! `WOW_LEVEL=<n>` overrides the start level, `WOW_PLAYER_AT=x,z,yaw_deg[,cam_deg]`
//! moves the player after the first floor loads (optional camera yaw offset, 180 = front view),
//! `WOW_CLEARED=1` opens the first floor's exit elevator without killing the boss.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::camera::OrbitCamera;
use crate::collision::to_core;
use crate::combat::{CombatEvent, CombatSim, PendingCommands, SimReady, SimUnit, new_sim};
use crate::copy;
use crate::floors::{BareStage, FLOOR_COUNT, LoadFloor, build_floor, elevator_exit_trigger};
use crate::hud::{ACCENT, Banner, DIM, label};
use crate::player::{Player, PlayerBody};
use crate::targeting::Target;
use bevy::prelude::*;
use combat_core::{Aabb, Event};
use std::f32::consts::PI;

const FADE_SECS: f32 = 0.6;
const DOOR_OPEN_SECS: f32 = 1.2;
/// Lets the CEO's death animation and the "cleared" banner play before the credits.
const VICTORY_DELAY_SECS: f32 = 3.0;
const LAMP_RED: LinearRgba = LinearRgba::rgb(6.0, 0.3, 0.2);
const LAMP_GREEN: LinearRgba = LinearRgba::rgb(0.3, 6.0, 0.6);

/// Load `floor` in the sim and the client together and put the player at its spawn.
#[derive(Message)]
pub struct EnterFloor(pub u8);

/// A fresh sim replaced the old one (victory restart): HUD state keyed to old sim ids must go.
#[derive(Message)]
pub struct GameRestarted;

/// The system that swaps the sim to a new floor; client floor visuals load right after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FloorSwap;

#[derive(Default)]
enum Fade {
    #[default]
    Idle,
    Out {
        t: f32,
        to: u8,
    },
    In {
        t: f32,
    },
}

#[derive(Resource, Default)]
pub struct GameFlow {
    pub floor: u8,
    pub cleared: bool,
    /// Anchors of the current floor (the sim spawns units at these).
    pub anchors: Vec<(&'static str, Vec3, f32)>,
    exit: Option<Aabb>,
    fade: Fade,
    door_open: f32,
    victory_in: Option<f32>,
    pub victory: bool,
}

impl GameFlow {
    /// Movement and casting are frozen during a floor transition and on the victory screen.
    pub fn input_locked(&self) -> bool {
        !matches!(self.fade, Fade::Idle) || self.victory
    }
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
struct VictoryScreen;
#[derive(Component)]
struct VictoryPrompt;

#[derive(Resource)]
struct DoorAssets {
    cube: Handle<Mesh>,
    panel: Handle<StandardMaterial>,
    shaft: Handle<StandardMaterial>,
    /// Re-created per floor so opening one elevator never recolours another.
    lamp: Option<Handle<StandardMaterial>>,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameFlow>()
            .add_message::<EnterFloor>()
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
                    enter_floor.in_set(FloorSwap),
                    floor_events,
                    check_exit,
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

fn start_game(
    bare: Option<Res<BareStage>>,
    mut sim: ResMut<CombatSim>,
    mut enter: MessageWriter<EnterFloor>,
) {
    if bare.is_some() {
        return;
    }
    let floor = env_u8("WOW_FLOOR")
        .filter(|f| (1..=FLOOR_COUNT).contains(f))
        .unwrap_or(1);
    let floor_min = sim.0.content().floor(floor).map_or(1, |f| f.level_range.0);
    let level = env_u8("WOW_LEVEL").unwrap_or(floor_min);
    if level > 1 {
        sim.0.set_player_level(level);
    }
    enter.write(EnterFloor(floor));
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

fn enter_floor(
    mut commands: Commands,
    mut requests: MessageReader<EnterFloor>,
    mut sim: ResMut<CombatSim>,
    mut flow: ResMut<GameFlow>,
    mut target: ResMut<Target>,
    mut doors: ResMut<DoorAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut load: MessageWriter<LoadFloor>,
    mut banners: MessageWriter<Banner>,
    mut body: Single<&mut PlayerBody, With<Player>>,
    mut cam: Single<&mut OrbitCamera>,
    old_doors: Query<Entity, With<ExitDoor>>,
    mut first: Local<bool>,
) {
    let Some(&EnterFloor(id)) = requests.read().last() else {
        return;
    };
    let layout = build_floor(id);
    let anchors = |name: &str| layout.anchor(name).map(|(p, yaw)| (to_core(p), yaw));
    if let Err(err) = sim.0.load_floor(id, &anchors, layout.blockers.clone()) {
        error!("cannot load floor {id}: {err:?}");
        return;
    }
    let (spawn, spawn_yaw) = layout
        .anchor("player_spawn")
        .unwrap_or((Vec3::ZERO, -PI / 2.0));
    sim.0.set_respawn_point(to_core(spawn), spawn_yaw);
    load.write(LoadFloor(id));

    let first_load = !std::mem::replace(&mut *first, true);
    let (pos, yaw, cam_offset) = match debug_player_at().filter(|_| first_load) {
        Some((x, z, yaw, cam)) => (Vec3::new(x, 0.0, z), yaw, cam),
        None => (spawn, spawn_yaw, 0.0),
    };
    body.teleport_facing(pos, yaw);
    cam.set_yaw(yaw + cam_offset);
    target.0 = None;

    flow.floor = layout.id;
    flow.cleared = first_load && std::env::var_os("WOW_CLEARED").is_some();
    flow.door_open = 0.0;
    flow.exit = Some(elevator_exit_trigger(&layout));
    flow.anchors.clone_from(&layout.anchors);
    banners.write(Banner {
        title: format!("Floor {} - {}", layout.id, layout.name),
        subtitle: floor_tagline(layout.id).into(),
        color: ACCENT,
    });

    for e in &old_doors {
        commands.entity(e).despawn();
    }
    if let Some((exit, exit_yaw)) = layout.anchor("elevator_exit") {
        spawn_exit_doors(&mut commands, &mut doors, &mut materials, exit, exit_yaw);
    }
}

fn floor_tagline(id: u8) -> &'static str {
    match id {
        1 => "Where the vibes are immaculate and the tests are optional.",
        2 => "This meeting could have been an email. The email could have been nothing.",
        3 => "Selling features we have not built to customers we do not have.",
        4 => "Every Gantt chart is a work of fiction.",
        _ => "The view is great. The strategy is a chatbot.",
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

fn floor_events(
    mut events: MessageReader<CombatEvent>,
    mut flow: ResMut<GameFlow>,
    mut banners: MessageWriter<Banner>,
) {
    for CombatEvent(ev) in events.read() {
        if let Event::FloorCleared { floor } = ev {
            flow.cleared = true;
            let last = *floor >= FLOOR_COUNT;
            banners.write(Banner {
                title: "Floor cleared".into(),
                subtitle: if last {
                    "The board is calling an emergency meeting.".into()
                } else {
                    "The elevator is unlocked. Take it up.".into()
                },
                color: Color::srgb(0.4, 1.0, 0.5),
            });
            if last {
                flow.victory_in = Some(VICTORY_DELAY_SECS);
            }
        }
    }
}

fn check_exit(mut flow: ResMut<GameFlow>, body: Single<&PlayerBody, With<Player>>) {
    if !flow.cleared || flow.floor >= FLOOR_COUNT || !matches!(flow.fade, Fade::Idle) {
        return;
    }
    let p = body.pos;
    if flow.exit.is_some_and(|b| {
        p.x > b.min.x && p.x < b.max.x && p.z > b.min.z && p.z < b.max.z && p.y < b.max.y
    }) {
        flow.fade = Fade::Out {
            t: 0.0,
            to: flow.floor + 1,
        };
    }
}

fn run_fade(
    time: Res<Time>,
    mut flow: ResMut<GameFlow>,
    mut enter: MessageWriter<EnterFloor>,
    mut overlay: Single<&mut BackgroundColor, With<FadeOverlay>>,
) {
    let dt = time.delta_secs();
    let alpha = match &mut flow.fade {
        Fade::Idle => 0.0,
        Fade::Out { t, to } => {
            *t += dt;
            if *t >= FADE_SECS {
                enter.write(EnterFloor(*to));
                flow.fade = Fade::In { t: 0.0 };
                1.0
            } else {
                *t / FADE_SECS
            }
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
    overlay.0 = Color::srgba(0.0, 0.0, 0.0, alpha);
}

fn animate_doors(
    time: Res<Time>,
    mut flow: ResMut<GameFlow>,
    doors: Res<DoorAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut panels: Query<(&DoorPanel, &mut Transform)>,
    mut lights: Query<&mut PointLight, With<DoorLight>>,
) {
    if !flow.cleared || flow.door_open >= 1.0 {
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
    commands.spawn((
        FadeOverlay,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::NONE),
        GlobalZIndex(55),
    ));
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
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.015, 0.03, 0.9)),
            GlobalZIndex(60),
        ))
        .with_children(|c| {
            c.spawn((label("// sprint 5 retrospective", 13.0), TextColor(DIM)));
            c.spawn((
                label(copy::VICTORY_TITLE, 40.0),
                TextColor(Color::srgb(1.0, 0.82, 0.3)),
            ));
            c.spawn((
                label(copy::VICTORY_SUBTITLE, 20.0),
                TextColor(Color::srgb(0.9, 0.92, 1.0)),
            ));
            c.spawn(Node {
                height: px(18),
                ..default()
            });
            c.spawn((label("$ git log --credits", 13.0), TextColor(ACCENT)));
            for line in copy::CREDITS {
                c.spawn((label(line, 13.0), TextColor(Color::srgb(0.7, 0.76, 0.86))));
            }
            c.spawn(Node {
                height: px(18),
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
    mut flow: ResMut<GameFlow>,
    mut screen: Single<&mut Node, With<VictoryScreen>>,
    mut prompt: Single<&mut TextColor, With<VictoryPrompt>>,
) {
    if let Some(left) = flow.victory_in.as_mut() {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            flow.victory_in = None;
            flow.victory = true;
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

/// A fresh sim (level 1, floor 1); the Bevy player entity is kept and relinked.
fn restart_on_r(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut flow: ResMut<GameFlow>,
    mut sim: ResMut<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    mut target: ResMut<Target>,
    mut enter: MessageWriter<EnterFloor>,
    mut restarted: MessageWriter<GameRestarted>,
    player: Single<(Entity, &PlayerBody), With<Player>>,
    visuals: Query<Entity, (With<SimUnit>, Without<Player>)>,
) {
    if !flow.victory || !keys.just_pressed(KeyCode::KeyR) {
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
    *flow = GameFlow::default();
    enter.write(EnterFloor(1));
    restarted.write(GameRestarted);
}
