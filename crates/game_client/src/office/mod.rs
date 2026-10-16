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
        PropKind::Bench | PropKind::BeanBag | PropKind::Dog | PropKind::VrDemo => 0.9,
        PropKind::QuietRoom => 1.2,
        PropKind::Scooter | PropKind::Printer => 0.6,
        PropKind::Coffee => 0.5,
        PropKind::FireAlarm | PropKind::Music => 0.4,
    }
}

/// Rebuilds the prop list (and the parked scooters) whenever the zone changes.
fn track_zone(
    mut commands: Commands,
    time: Res<Time>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut props: ResMut<Props>,
    parked: Query<Entity, With<ParkedScooter>>,
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
            })
        })
        .collect();
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
            .filter(|(_, p)| p.gone_until <= now)
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
                .filter(|(_, p)| p.stand.distance(body.pos) <= p.reach)
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
                    lines::mine(def, *outcome, props.uses, sim.content())
                } else {
                    let name = sim.unit(*unit).map_or("A colleague", |u| u.name.as_str());
                    lines::witnessed(def, name)
                };
                if let Some(text) = line {
                    let color = match def.kind {
                        PropKind::FireAlarm => WARN,
                        _ => LINE,
                    };
                    log.write(LogLineMsg(text, color));
                }
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
    if p.kind == PropKind::DaysSign {
        let days = ((time.elapsed_secs() - props.days_reset) / DAY_SECS).max(0.0) as u32;
        extra = format!("Days without an AI incident: {days}");
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
