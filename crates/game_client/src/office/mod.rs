//! The interactive office: hover, tooltip and right-click / F use of the props a zone layout
//! places (`FloorLayout::props`, anchors `prop:<key>`), the chat lines they answer with,
//! achievement toasts, parked e-scooters that vanish when borrowed, and the bench's ambience.
//! Rules (reach, rolls, rewards, achievements) live in `combat_core::world`; this side only
//! sends `Command::Interact` and presents the `Interacted` / `Achievement` events.
//!
//! QA hooks: `WOW_INTERACT=<prop>@<secs>[x<n>][,...]` uses a prop at that time (`n` times,
//! 0.7 s apart, e.g. `printer_pmo@3x10`); `WOW_HOVER_PROP=<prop>` shows its tooltip without
//! a pointer.

pub mod lines;
mod ride;

pub use ride::{EmotePlayed, PoseClip, WorldPose, cast_world_spell};

use crate::arena::Colliders;
use crate::collision::to_core;
use crate::combat::{CombatEvent, PendingCommands, SimReady, SimUnit, SimView, UiNotice};
use crate::config::CLICK_DRAG_THRESHOLD;
use crate::floors::build_zone;
use crate::game::GameFlow;
use crate::hud::{ACCENT, DIM, LogLineMsg, drop_shadow, label, panel_bg, set_text};
use crate::input::{Paused, TextFocus};
use crate::keybinds::{Action, Keybinds};
use crate::net::EmoteRequest;
use crate::player::{Player, PlayerBody};
use crate::rigs::PrimRig;
use crate::targeting::{Target, pick_unit, ray_sphere};
use crate::units::Unit;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use combat_core::{Command, Event, PropKind, line_of_sight};
use net_proto::Emote;
use std::collections::HashMap;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);
const LINE: Color = Color::srgb(0.85, 0.8, 0.62);
const WARN: Color = Color::srgb(1.0, 0.35, 0.3);
const ACHIEVEMENT: Color = Color::srgb(1.0, 0.6, 0.15);
const TOAST_SECS: f32 = 5.0;
const BENCH_LINE_SECS: f32 = 9.0;
/// The "days without an AI incident" counter gains a day this often (it is a very fast office).
const DAY_SECS: f32 = 20.0;

/// One prop of the loaded zone.
pub struct PropSpot {
    pub key: &'static str,
    pub kind: PropKind,
    /// What the cursor picks.
    pub center: Vec3,
    /// Where the server measures reach from (anchor `prop:<key>`).
    pub stand: Vec3,
    pub stand_yaw: f32,
    pub reach: f32,
    radius: f32,
    /// Borrowed scooters: hidden until this time (seconds since start).
    gone_until: f32,
    /// Quest pickups nobody local needs right now: not drawn, hovered or used.
    hidden: bool,
}

#[derive(Resource, Default)]
pub struct Props {
    zone: String,
    pub list: Vec<PropSpot>,
    /// Every place to sit in the loaded zone.
    pub seats: Vec<crate::seat::SeatSpot>,
    days_reset: f32,
    /// The local player's prop uses this session (varies repeated lines).
    uses: u32,
    /// Quest pickups the local player took (`world.props` keys), mirroring the sim's per-player
    /// record; a new quest for their tag puts them back.
    picked: Vec<String>,
}

/// Entities drawn per zone by this module (parked scooters, quest pickups).
type ZoneProps = Or<(With<ParkedScooter>, With<QuestPickup>, With<ClassMarker>)>;

/// A glowing quest pickup on the floor (index into `Props::list`); `true` for its halo disc.
#[derive(Component)]
struct QuestPickup(usize, bool);

/// The class-coloured diamond (or halo ring) over a class prop: who may use it, at a glance.
#[derive(Component)]
struct ClassMarker;

/// Marker colour of a class key (the roles' colours are taken by the party frames).
fn class_color(key: &str) -> Color {
    match key {
        "devops" => Color::srgb(0.35, 0.95, 0.5),
        "frontend" => Color::srgb(1.0, 0.45, 0.7),
        "backend" => Color::srgb(0.45, 0.62, 1.0),
        "qa" => Color::srgb(1.0, 0.75, 0.3),
        "data_scientist" => Color::srgb(0.72, 0.5, 1.0),
        "security" => Color::srgb(1.0, 0.3, 0.3),
        "cobol" => Color::srgb(0.3, 0.9, 0.9),
        _ => Color::WHITE,
    }
}

/// The prop under the cursor (index into `Props::list`), for the cursor icon and the tooltip.
#[derive(Resource, Default)]
pub struct PropHover(pub Option<usize>);

#[derive(Component)]
struct ParkedScooter(usize);
#[derive(Component)]
struct PropTip;
#[derive(Component)]
enum TipLine {
    Name,
    Action,
    Extra,
}
#[derive(Resource, Default)]
struct Toast {
    left: f32,
}
#[derive(Component)]
struct ToastRoot;
#[derive(Component)]
enum ToastLine {
    Name,
    Title,
    Text,
}

pub struct OfficePlugin;

impl Plugin for OfficePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EmotePlayed>()
            .init_resource::<Props>()
            .init_resource::<PropHover>()
            .init_resource::<Toast>()
            .add_systems(Startup, (spawn_tip, spawn_toast))
            .add_systems(
                Update,
                (
                    track_zone,
                    parked_scooters,
                    quest_pickups,
                    hover_props,
                    use_props,
                    prop_events,
                    show_tip,
                    show_toast,
                    bench_ambience,
                    qa_interact,
                )
                    .chain()
                    .in_set(SimReady),
            )
            .add_systems(
                Update,
                (
                    ride::add_poses,
                    ride::apply_emotes,
                    ride::update_poses,
                    ride::ride_models,
                    ride::player_speed,
                    ride::ride_keys,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn hover_radius(kind: PropKind) -> f32 {
    match kind {
        PropKind::Billboard => 3.5,
        PropKind::AiCar => 1.8,
        PropKind::DaysSign => 1.6,
        PropKind::Vending
        | PropKind::Table
        | PropKind::BikeRack
        | PropKind::Whiteboard
        | PropKind::SprintBoard
        | PropKind::ScrumBoard
        | PropKind::Calendar => 1.0,
        PropKind::Bench
        | PropKind::BeanBag
        | PropKind::Dog
        | PropKind::VrDemo
        | PropKind::ShuttleStop
        | PropKind::Inspect
        | PropKind::Class
        | PropKind::Device => 0.9,
        PropKind::Pickup => 0.6,
        PropKind::QuietRoom => 1.2,
        PropKind::Scooter | PropKind::Printer => 0.6,
        PropKind::Coffee => 0.5,
        PropKind::FireAlarm | PropKind::Music => 0.4,
    }
}

/// Icon (an existing HUD icon, same art as the item) and halo tint of a quest pickup tag.
fn pickup_look(tag: &str) -> (&'static str, Color) {
    match tag {
        "lost_lanyard" => ("demo_lanyard", Color::srgb(0.4, 0.8, 1.0)),
        "t1_password" => ("sticky", Color::srgb(1.0, 0.9, 0.3)),
        "t3_badge_log" => ("visitor_badge", Color::srgb(0.5, 1.0, 0.6)),
        "t4_gantt" => ("chart", Color::srgb(0.7, 0.55, 1.0)),
        "t5_pr" => ("pull_request", Color::srgb(0.4, 1.0, 0.9)),
        "dc1_fan_blade" => ("fan_blade", Color::srgb(0.75, 0.8, 0.9)),
        "dc2_tape" => ("rollback", Color::srgb(1.0, 0.55, 0.35)),
        "lab1_usb" => ("gear_usb", Color::srgb(1.0, 0.45, 0.8)),
        _ => ("box", Color::srgb(1.0, 0.8, 0.25)),
    }
}

/// Rebuilds the prop list (and the parked scooters, quest pickups) whenever the zone changes.
#[allow(clippy::too_many_arguments)] // Bevy system: one parameter per resource it reads
fn track_zone(
    mut commands: Commands,
    time: Res<Time>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut props: ResMut<Props>,
    parked: Query<Entity, ZoneProps>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
    mut icons: ResMut<crate::hud::IconCache>,
) {
    if flow.zone == props.zone || flow.zone.is_empty() {
        return;
    }
    props.zone.clone_from(&flow.zone);
    props.days_reset = time.elapsed_secs() - 47.0 * DAY_SECS;
    for e in &parked {
        commands.entity(e).despawn();
    }
    let Some(layout) = build_zone(&flow.zone) else {
        props.list.clear();
        props.seats.clear();
        return;
    };
    props.seats.clone_from(&layout.seats);
    let content = sim.content();
    props.list = layout
        .props
        .iter()
        .filter_map(|(key, center)| {
            let def = content.prop(key)?;
            let (stand, stand_yaw) = layout.anchor(&format!("prop:{key}"))?;
            Some(PropSpot {
                key,
                kind: def.kind,
                center: *center,
                stand,
                stand_yaw,
                reach: def.reach_m,
                radius: hover_radius(def.kind),
                gone_until: 0.0,
                hidden: def.kind == PropKind::Pickup,
            })
        })
        .collect();
    // Each kind of pickup is its own glowing icon slab over a tinted halo (so a lanyard never
    // reads as a sticky note); the item's name is in the tooltip. Materials are shared per tag.
    let mesh = meshes.add(Cuboid::new(0.5, 0.05, 0.5));
    let disc = meshes.add(Cylinder::new(0.55, 0.02));
    let mut looks: HashMap<String, (Handle<StandardMaterial>, Handle<StandardMaterial>)> =
        HashMap::new();
    for (i, p) in props.list.iter().enumerate() {
        if p.kind != PropKind::Pickup {
            continue;
        }
        let tag = content.prop(p.key).map_or("", |d| d.tag.as_str());
        let (glow, halo) = looks.entry(tag.to_string()).or_insert_with(|| {
            let (icon, tint) = pickup_look(tag);
            let c = tint.to_linear();
            let glow = materials.add(StandardMaterial {
                base_color_texture: Some(icons.get(&assets, icon)),
                base_color: Color::WHITE,
                emissive: LinearRgba::rgb(c.red * 0.9, c.green * 0.9, c.blue * 0.9),
                alpha_mode: AlphaMode::Mask(0.4),
                ..default()
            });
            let halo = materials.add(StandardMaterial {
                base_color: tint.with_alpha(0.3),
                emissive: LinearRgba::rgb(c.red * 1.6, c.green * 1.6, c.blue * 1.6),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            });
            (glow, halo)
        });
        commands.spawn((
            QuestPickup(i, false),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(glow.clone()),
            Transform::from_translation(p.center),
            Visibility::Hidden,
        ));
        commands.spawn((
            QuestPickup(i, true),
            Mesh3d(disc.clone()),
            MeshMaterial3d(halo.clone()),
            Transform::from_xyz(p.center.x, 0.03, p.center.z),
            Visibility::Hidden,
        ));
    }
    let diamond = meshes.add(Cuboid::new(0.2, 0.2, 0.2));
    let ring = meshes.add(Annulus::new(0.62, 0.78));
    for p in props.list.iter().filter(|p| p.kind == PropKind::Class) {
        let class = content.prop(p.key).map_or("", |d| d.class.as_str());
        let color = class_color(class);
        let c = color.to_linear();
        let glow = materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::rgb(c.red * 3.0, c.green * 3.0, c.blue * 3.0),
            unlit: true,
            ..default()
        });
        commands.spawn((
            ClassMarker,
            Mesh3d(diamond.clone()),
            MeshMaterial3d(glow.clone()),
            Transform::from_xyz(p.center.x, p.center.y + 1.0, p.center.z)
                .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.6, 0.78, 0.0)),
        ));
        // An outline on the floor in front of the console, where the use spot is.
        commands.spawn((
            ClassMarker,
            Mesh3d(ring.clone()),
            MeshMaterial3d(glow),
            Transform::from_xyz(p.stand.x, p.stand.y + 0.04, p.stand.z)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        ));
    }
    for (i, p) in props.list.iter().enumerate() {
        if p.kind == PropKind::Scooter {
            commands.spawn((
                ParkedScooter(i),
                PrimRig("scooter"),
                Transform::from_xyz(p.center.x, 0.0, p.center.z)
                    .with_rotation(Quat::from_rotation_y(p.stand_yaw + 1.2 * i as f32)),
                Visibility::default(),
            ));
        }
    }
}

fn parked_scooters(
    time: Res<Time>,
    props: Res<Props>,
    mut parked: Query<(&ParkedScooter, &mut Visibility)>,
) {
    let now = time.elapsed_secs();
    for (p, mut vis) in &mut parked {
        let gone = props.list.get(p.0).is_some_and(|s| s.gone_until > now);
        let want = if gone {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Shows a quest pickup only while the local player's quest still wants it and they have not
/// taken it, bobbing gently so it reads as "for you" next to the static dressing.
fn quest_pickups(
    time: Res<Time>,
    sim: SimView,
    mut props: ResMut<Props>,
    mut q: Query<(&QuestPickup, &mut Transform, &mut Visibility)>,
) {
    let content = sim.content();
    let log = sim.quests();
    let wanted = |tag: &str| {
        log.active.iter().any(|a| {
            content.quest(&a.id).is_some_and(|def| {
                def.objectives.iter().zip(&a.counts).any(|(o, c)| {
                    matches!(o, combat_core::Objective::Collect(t, _) if t == tag) && *c < o.need()
                })
            })
        })
    };
    let t = time.elapsed_secs();
    let taken = sim.taken_props();
    let picked = std::mem::take(&mut props.picked);
    for (pick, mut tf, mut vis) in &mut q {
        let Some(spot) = props.list.get_mut(pick.0) else {
            continue;
        };
        let tag = content.prop(spot.key).map_or("", |d| d.tag.as_str());
        spot.hidden = !wanted(tag)
            || picked.iter().any(|k| k == spot.key)
            || taken.iter().any(|k| k == spot.key);
        let want = if spot.hidden {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *vis != want {
            *vis = want;
        }
        if !spot.hidden {
            let phase = pick.0 as f32;
            if pick.1 {
                tf.scale = Vec3::splat(0.85 + 0.15 * (t * 2.2 + phase).sin());
            } else {
                tf.translation.y = spot.center.y + 0.1 + 0.08 * (t * 2.2 + phase).sin();
                tf.rotation = Quat::from_rotation_y(t * 0.8 + phase);
            }
        }
    }
    props.picked = picked;
}

/// Units under the cursor win; otherwise the nearest prop sphere in line of sight.
#[allow(clippy::too_many_arguments)] // Bevy system: one parameter per resource it reads
fn hover_props(
    time: Res<Time>,
    paused: Res<Paused>,
    props: Res<Props>,
    colliders: Res<Colliders>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    ui: Query<&Interaction>,
    units: Query<(Entity, &Transform, &Unit)>,
    mut hover: ResMut<PropHover>,
) {
    if std::env::var_os("WOW_HOVER_PROP").is_some() {
        return;
    }
    let now = time.elapsed_secs();
    let picked = (|| {
        let cursor = window.cursor_position()?;
        if paused.0 || ui.iter().any(|i| *i != Interaction::None) {
            return None;
        }
        if pick_unit(cursor, *camera, &colliders.0, &units, 0.0).is_some() {
            return None;
        }
        let (cam, cam_tf) = *camera;
        let ray = cam.viewport_to_world(cam_tf, cursor).ok()?;
        let (t, i) = props
            .list
            .iter()
            .enumerate()
            .filter(|(_, p)| p.gone_until <= now && !p.hidden)
            .filter_map(|(i, p)| {
                ray_sphere(ray.origin, *ray.direction, p.center, p.radius).map(|t| (t, i))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        // The prop is its own blocker: stop the sight line a little short of it.
        let end = ray.origin + *ray.direction * (t - 1.0).max(0.0);
        line_of_sight(to_core(ray.origin), to_core(end), &colliders.0).then_some(i)
    })();
    if hover.0 != picked {
        hover.0 = picked;
    }
}

/// Right-click (without a steering drag) on the hovered prop, or F next to one when there is
/// no friendly unit or corpse to talk to or loot first.
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters
fn use_props(
    (keys, binds, buttons, motion): (
        Res<ButtonInput<KeyCode>>,
        Res<Keybinds>,
        Res<ButtonInput<MouseButton>>,
        Res<AccumulatedMouseMotion>,
    ),
    (paused, focus, flow, target): (Res<Paused>, Res<TextFocus>, Res<GameFlow>, Res<Target>),
    window: Single<&Window, With<PrimaryWindow>>,
    sim: SimView,
    hover: Res<PropHover>,
    props: Res<Props>,
    mut pending: ResMut<PendingCommands>,
    mut notices: MessageWriter<UiNotice>,
    player: Single<(&PlayerBody, &SimUnit), With<Player>>,
    units: Query<(Entity, &Transform, &Unit)>,
    mut press: Local<Option<(Vec2, f32)>>,
) {
    if paused.0 || focus.0 || flow.input_locked() {
        return;
    }
    let (body, me) = *player;
    let mut chosen = None;
    if buttons.just_pressed(MouseButton::Right) {
        *press = window.cursor_position().map(|c| (c, 0.0));
    }
    if let Some((_, travel)) = press.as_mut()
        && buttons.pressed(MouseButton::Right)
    {
        *travel += motion.delta.length();
    }
    if buttons.just_released(MouseButton::Right)
        && let Some((_, travel)) = press.take()
        && travel <= CLICK_DRAG_THRESHOLD
    {
        chosen = hover.0;
    }
    if chosen.is_none() && binds.just(&keys, Action::Interact) {
        let reach = sim.content().config.loot_range_m;
        let busy_target = target
            .0
            .and_then(|t| units.get(t).ok())
            .is_some_and(|(_, _, u)| !u.hostile || !u.alive);
        let friend_near = units
            .iter()
            .any(|(_, tf, u)| !u.hostile && u.alive && tf.translation.distance(body.pos) <= reach);
        if !busy_target && !friend_near {
            chosen = props
                .list
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.hidden && p.stand.distance(body.pos) <= p.reach)
                .min_by(|a, b| {
                    a.1.stand
                        .distance(body.pos)
                        .total_cmp(&b.1.stand.distance(body.pos))
                })
                .map(|(i, _)| i);
        }
    }
    let Some(p) = chosen.and_then(|i| props.list.get(i)) else {
        return;
    };
    if p.stand.distance(body.pos) > p.reach {
        notices.write(UiNotice("Too far away. Walk closer.".into()));
        return;
    }
    if sim.unit(me.0).is_none_or(|u| !u.alive) {
        return;
    }
    pending.0.push(Command::Interact {
        prop: p.key.to_string(),
    });
}

/// Chat lines, toasts and scooter visibility from the sim's world events.
#[allow(clippy::too_many_arguments)] // Bevy system parameters
fn prop_events(
    time: Res<Time>,
    sim: SimView,
    mut events: MessageReader<CombatEvent>,
    mut props: ResMut<Props>,
    mut toast: ResMut<Toast>,
    mut log: MessageWriter<LogLineMsg>,
    mut emotes: MessageWriter<EmoteRequest>,
    mut body: Single<&mut PlayerBody, With<Player>>,
    mut texts: Query<(&ToastLine, &mut Text)>,
) {
    let now = time.elapsed_secs();
    let me = sim.player();
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::Interacted {
                unit,
                prop,
                outcome,
            } => {
                let Some(def) = sim.content().prop(prop) else {
                    continue;
                };
                if def.kind == PropKind::Scooter
                    && let Some(spot) = props.list.iter_mut().find(|p| p.key == prop.as_str())
                {
                    spot.gone_until = now + def.respawn_ms as f32 / 1000.0;
                }
                if def.kind == PropKind::DaysSign {
                    props.days_reset = now;
                }
                if def.kind == PropKind::Pickup
                    && Some(*unit) == me
                    && matches!(
                        *outcome,
                        combat_core::QUEST_PROP_COUNTED | combat_core::QUEST_PROP_TAKEN
                    )
                {
                    props.picked.push(prop.clone());
                }
                let spot = props.list.iter().find(|p| p.key == prop.as_str());
                match (def.kind, spot) {
                    // `/sit` snaps to the nearest free slot of the bench (`ride::apply_emotes`);
                    // the stand spot is within reach of it.
                    (PropKind::Bench | PropKind::BeanBag, Some(s)) if Some(*unit) == me => {
                        body.teleport(s.stand);
                        emotes.write(EmoteRequest(Emote::Sit));
                    }
                    (PropKind::Table, _) if Some(*unit) == me && lines::table_won(*outcome) => {
                        emotes.write(EmoteRequest(Emote::Dance));
                    }
                    _ => {}
                }
                let line = if Some(*unit) == me {
                    props.uses += 1;
                    let class = sim
                        .unit(*unit)
                        .and_then(|u| sim.content().class_of_unit(&u.key))
                        .map_or("", |c| c.key.as_str());
                    lines::mine(def, *outcome, props.uses, class, sim.content())
                } else {
                    let name = sim.unit(*unit).map_or("A colleague", |u| u.name.as_str());
                    lines::witnessed(def, *outcome, name)
                };
                if let Some(text) = line {
                    let color = match def.kind {
                        PropKind::FireAlarm => WARN,
                        _ => LINE,
                    };
                    log.write(LogLineMsg(text, color));
                }
            }
            // The sim puts a quest's pickups back for a player who takes it (again).
            Event::QuestAccepted { quest } => {
                let content = sim.content();
                let tags: Vec<&str> = content
                    .quest(quest)
                    .map(|q| {
                        q.objectives
                            .iter()
                            .filter_map(|o| match o {
                                combat_core::Objective::Collect(t, _) => Some(t.as_str()),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                props.picked.retain(|k| {
                    content
                        .prop(k)
                        .is_none_or(|d| !tags.contains(&d.tag.as_str()))
                });
            }
            Event::Achievement { unit, key } => {
                let Some(a) = sim.content().achievement(key) else {
                    continue;
                };
                let name = sim.unit(*unit).map_or("Someone", |u| u.name.as_str());
                log.write(LogLineMsg(
                    format!("{name} has earned the achievement [{}]!", a.name),
                    ACHIEVEMENT,
                ));
                if Some(*unit) == me {
                    toast.left = TOAST_SECS;
                    for (line, mut text) in &mut texts {
                        let s = match line {
                            ToastLine::Name => a.name.clone(),
                            ToastLine::Title => format!("Title unlocked: <{}>", a.title),
                            ToastLine::Text => a.text.clone(),
                        };
                        set_text(&mut text, &s);
                    }
                }
            }
            _ => {}
        }
    }
}

fn spawn_tip(mut commands: Commands) {
    commands
        .spawn((
            PropTip,
            Node {
                position_type: PositionType::Absolute,
                width: px(300),
                padding: UiRect::axes(px(10), px(6)),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.7)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(44),
        ))
        .with_children(|t| {
            t.spawn((TipLine::Name, label("", 14.0), TextColor(GOLD)));
            t.spawn((TipLine::Action, label("", 12.0), TextColor(Color::WHITE)));
            t.spawn((TipLine::Extra, label("", 12.0), TextColor(DIM)));
        });
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters
fn show_tip(
    time: Res<Time>,
    sim: SimView,
    hover: Res<PropHover>,
    props: Res<Props>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    player: Single<&PlayerBody, With<Player>>,
    mut tip: Single<&mut Node, With<PropTip>>,
    mut lines_q: Query<(&TipLine, &mut Text, &mut TextColor)>,
) {
    let now = time.elapsed_secs();
    let spot = hover
        .0
        .and_then(|i| props.list.get(i))
        .filter(|p| p.gone_until <= now);
    let cursor = forced_tip_at(&props, &hover, *camera).or_else(|| window.cursor_position());
    let (Some(p), Some(cursor)) = (spot, cursor) else {
        if tip.display != Display::None {
            tip.display = Display::None;
        }
        return;
    };
    let Some(def) = sim.content().prop(p.key) else {
        return;
    };
    tip.display = Display::Flex;
    tip.left = px(cursor.x + 18.0);
    tip.top = px(cursor.y + 18.0);
    let (action, mut extra) = lines::tooltip(def);
    let action = if def.action.is_empty() {
        action
    } else {
        def.action.as_str()
    };
    if p.kind == PropKind::Class {
        extra = lines::class_tip(def, sim.content());
    }
    if p.kind == PropKind::DaysSign {
        let days = ((time.elapsed_secs() - props.days_reset) / DAY_SECS).max(0.0) as u32;
        extra = format!("{}: {days}", def.name);
    }
    let far = p.stand.distance(player.pos) > p.reach;
    for (line, mut text, mut color) in &mut lines_q {
        match line {
            TipLine::Name => set_text(&mut text, &def.name),
            TipLine::Action => {
                set_text(
                    &mut text,
                    &if far {
                        format!("{action}  (too far away)")
                    } else {
                        format!("Right-click / F: {action}")
                    },
                );
                color.0 = if far { DIM } else { Color::WHITE };
            }
            TipLine::Extra => {
                set_text(&mut text, &extra);
                color.0 = if p.kind == PropKind::FireAlarm {
                    WARN
                } else if p.kind == PropKind::Class {
                    class_color(&def.class)
                } else {
                    DIM
                };
            }
        }
    }
}

fn spawn_toast(mut commands: Commands) {
    commands
        .spawn((
            ToastRoot,
            Node {
                position_type: PositionType::Absolute,
                top: px(120),
                left: percent(50),
                width: px(420),
                margin: UiRect::left(px(-210)),
                padding: UiRect::axes(px(14), px(10)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(3),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(6)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACHIEVEMENT),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(46),
        ))
        .with_children(|t| {
            t.spawn((
                label("SECRET ACHIEVEMENT UNLOCKED", 11.0),
                TextColor(ACHIEVEMENT),
            ));
            t.spawn((ToastLine::Name, label("", 20.0), TextColor(GOLD)));
            t.spawn((ToastLine::Title, label("", 13.0), TextColor(Color::WHITE)));
            t.spawn((ToastLine::Text, label("", 12.0), TextColor(DIM)));
        });
}

fn show_toast(
    time: Res<Time>,
    mut toast: ResMut<Toast>,
    mut root: Single<&mut Node, With<ToastRoot>>,
) {
    toast.left = (toast.left - time.delta_secs()).max(0.0);
    let want = if toast.left > 0.0 {
        Display::Flex
    } else {
        Display::None
    };
    if root.display != want {
        root.display = want;
    }
}

/// `WOW_INTERACT` schedule: (prop, time) pairs.
fn qa_schedule() -> Vec<(String, f32)> {
    let Ok(v) = std::env::var("WOW_INTERACT") else {
        return Vec::new();
    };
    v.split(',')
        .filter_map(|e| {
            let (key, when) = e.trim().split_once('@')?;
            let (t, n) = when.split_once('x').unwrap_or((when, "1"));
            let (t, n): (f32, u32) = (t.parse().ok()?, n.parse().ok()?);
            Some((0..n).map(move |k| (key.to_string(), t + 0.7 * k as f32)))
        })
        .flatten()
        .collect()
}

fn qa_interact(
    time: Res<Time>,
    props: Res<Props>,
    mut hover: ResMut<PropHover>,
    mut pending: ResMut<PendingCommands>,
    mut plan: Local<Option<Vec<(String, f32)>>>,
) {
    let plan = plan.get_or_insert_with(qa_schedule);
    let now = time.elapsed_secs();
    plan.retain(|(key, t)| {
        if *t > now {
            return true;
        }
        pending.0.push(Command::Interact { prop: key.clone() });
        false
    });
    if let Ok(key) = std::env::var("WOW_HOVER_PROP") {
        hover.0 = props.list.iter().position(|p| p.key == key.as_str());
    }
}

/// Screen point of the prop under a forced (`WOW_HOVER_PROP`) hover.
fn forced_tip_at(
    props: &Props,
    hover: &PropHover,
    camera: (&Camera, &GlobalTransform),
) -> Option<Vec2> {
    std::env::var_os("WOW_HOVER_PROP")?;
    let p = props.list.get(hover.0?)?;
    camera.0.world_to_viewport(camera.1, p.center).ok()
}

/// Sitting on the smoking-corner bench, the world talks a little.
fn bench_ambience(
    time: Res<Time>,
    props: Res<Props>,
    player: Single<(&PlayerBody, &WorldPose), With<Player>>,
    mut log: MessageWriter<LogLineMsg>,
    mut state: Local<(f32, usize)>,
) {
    let (body, pose) = *player;
    let on_bench = pose.pose() == Some(PoseClip::Sit)
        && props
            .list
            .iter()
            .any(|p| p.kind == PropKind::Bench && p.stand.distance(body.pos) <= p.reach + 0.5);
    if !on_bench {
        state.0 = 0.0;
        return;
    }
    state.0 += time.delta_secs();
    if state.0 >= BENCH_LINE_SECS {
        state.0 = 0.0;
        state.1 += 1;
        let line = lines::BENCH[state.1 % lines::BENCH.len()];
        log.write(LogLineMsg(line.to_string(), LINE));
    }
}
