//! The Ilmari 1.0 finale on screen (docs/ILMARI_1_0.md D17): Wing 5's sight cones and standby banner,
//! the offer line; Wing 6's cleared desks and floor tiles, the inbox envelope, the severance bar and
//! the kick prompt; the core's wall (live monitor, stance, round, planning, the mainframe's clock)
//! and the P4 log wall. Presentation only: the sim holds the truth and nothing here feeds back.
//! Everything is a handful of reused meshes and materials (no lights); the HUD text is rebuilt when
//! an event changed the wall, not every frame.
// Bevy system signatures are inherently wide.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, SimReady, SimView};
use crate::floors::{
    BOARD_DESKS, BOARD_INBOXES, BOARD_POD, CORE_POD, FloorEntity, LAB_KIOSKS, LAB_KIOSKS_ALT,
    LAB_PILLARS, LAB_ROTUNDA, core_face,
};
use crate::game::{GameFlow, ZoneDecor};
use crate::hud::{DIM, drop_shadow, label, panel_bg, set_text};
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use combat_core::{CastError, EntityId, Event};
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

const KICK_SHOW_S: f32 = 2.5;
/// Segments of a cone's arc: smooth enough at 20 m, cheap enough to ignore.
const CONE_SEGS: usize = 14;
const STANCES: [&str; 5] = [
    "Helpful (Compliant)",
    "Defensive (Adversarial)",
    "Hallucinating (Confused)",
    "Baseline (Hedged)",
    "Best (Leaky)",
];

pub struct IlmariFxPlugin;

impl Plugin for IlmariFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wall>()
            .init_resource::<Cones>()
            .init_resource::<Hideables>()
            .add_systems(Startup, (setup, spawn_hud))
            .add_systems(
                Update,
                (
                    stage_qa,
                    read_events,
                    build_zone,
                    bind_floor,
                    update_hideables,
                    update_cones,
                    update_boardroom,
                    update_monitors,
                    update_hud,
                    update_log,
                    update_kick,
                )
                    .chain()
                    .in_set(SimReady),
            )
            .add_systems(Update, update_hot_take.in_set(SimReady));
    }
}

/// QA and bench staging, offline only. `WOW_ILMARI_STAGE=<boss key>` (`il_red_team`, `il_cio`,
/// `il_ilmari`) pulls that boss 1.5 s in with the player unkillable; `WOW_ILMARI_PCT=<pct>` keeps it at or
/// below that HP percent (the core's P3 and P4 sit below 70 and 20).
fn stage_qa(time: Res<Time>, mut sim: ResMut<crate::combat::CombatSim>, mut done: Local<u8>) {
    let key = match std::env::var("WOW_ILMARI_STAGE") {
        Ok(v) => v,
        Err(_) => match std::env::var("WOW_BENCH").as_deref() {
            Ok("ilmari5_exercise") => "il_red_team".into(),
            Ok("ilmari6_boardroom") => "il_cio".into(),
            Ok("ilmari_core" | "ilmari_core_p3" | "ilmari_core_p4") => "il_ilmari".into(),
            _ => return,
        },
    };
    let Some(me) = sim.0.player() else { return };
    sim.0.debug_set_hp(me, u32::MAX);
    if time.elapsed_secs() < 1.5 {
        return;
    }
    let Some((boss, hp, max)) = sim
        .0
        .units()
        .find(|u| u.key == key && u.alive)
        .map(|u| (u.id, u.hp, u.max_hp))
    else {
        return;
    };
    if *done == 0 {
        sim.0.debug_hit(me, boss, 1);
        *done = 1;
    }
    // The boss sits at the ceiling the shot asks for, so the fight walks down to that phase.
    let bench_pct = match std::env::var("WOW_BENCH").as_deref() {
        Ok("ilmari_core_p3") => Some(65),
        Ok("ilmari_core_p4") => Some(15),
        _ => None,
    };
    if let Some(pct) = std::env::var("WOW_ILMARI_PCT")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .or(bench_pct)
        && u64::from(hp) * 100 > u64::from(max) * u64::from(pct)
    {
        sim.0.debug_set_hp(boss, max / 100 * pct);
    }
}

/// What the sim last said about the displays.
#[derive(Resource, Default)]
struct Wall {
    exercise: Option<(bool, u8, bool)>,
    /// desks, inbox ms, severance ms, tiles.
    board: Option<(u8, u16, u16, u16)>,
    /// The largest severance value seen this fight, so the bar has a full end.
    severance_peak: u16,
    core: Option<CoreWall>,
    log: Vec<String>,
    kick: Option<(u8, u8, f32)>,
    zone: String,
    /// The Opt-Out Kiosks stand on their second site (`Event::Kiosks`).
    kiosk_alt: bool,
    /// The pillar a `Principles` cast took away (index into `LAB_PILLARS`) and the time it is
    /// back, in elapsed seconds.
    pillar_gone: Option<(usize, f32)>,
}

/// `Principles` and its two variants (one pillar fewer each time).
const PRINCIPLES: [u32; 3] = [2638, 2769, 2770];
/// A taken pillar stays away this long past the cast landing.
const PILLAR_BACK_S: f32 = 1.5;

#[derive(Clone, Copy, PartialEq)]
struct CoreWall {
    phase: u8,
    live: u8,
    swap_s: u8,
    remix: u8,
    round: u8,
    round_s: u8,
    tally: [u8; 4],
    remember: bool,
    stance: u8,
    plan_s: u8,
    arms: u8,
    stage: u8,
    clock_s: u8,
    ward_pct: u8,
    wrong: u8,
}

fn read_events(mut events: MessageReader<CombatEvent>, time: Res<Time>, mut wall: ResMut<Wall>) {
    for CombatEvent(e) in events.read() {
        match e {
            Event::Exercise {
                standby,
                sensors,
                merged,
            } => {
                let off = !*standby && *sensors == 0 && !*merged;
                wall.exercise = (!off).then_some((*standby, *sensors, *merged));
            }
            Event::Kiosks { alt } => wall.kiosk_alt = *alt,
            Event::TelegraphStarted {
                spell,
                cover,
                resolve_ms,
                ..
            } if PRINCIPLES.contains(&spell.0) => {
                // The pillar that no cover spot of the cast stands on is the one that is gone.
                let (cx, cz) = LAB_ROTUNDA;
                let far = |(dx, dz): (f32, f32)| {
                    cover
                        .iter()
                        .map(|c| (c.x - (cx + dx)).hypot(c.z - (cz + dz)))
                        .fold(f32::MAX, f32::min)
                };
                wall.pillar_gone = LAB_PILLARS
                    .into_iter()
                    .enumerate()
                    .map(|(k, p)| (k, far(p)))
                    .filter(|(_, d)| *d > 2.0)
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(k, _)| {
                        (
                            k,
                            time.elapsed_secs() + *resolve_ms as f32 / 1000.0 + PILLAR_BACK_S,
                        )
                    });
            }
            Event::Boardroom {
                desks,
                inbox_ms,
                severance_ms,
                tiles,
            } => {
                if *desks == 255 {
                    wall.board = None;
                    wall.severance_peak = 0;
                } else {
                    wall.severance_peak = if *severance_ms == 0 {
                        0
                    } else {
                        wall.severance_peak.max(*severance_ms)
                    };
                    wall.board = Some((*desks, *inbox_ms, *severance_ms, *tiles));
                }
            }
            Event::PairKick { have, need, .. } => {
                wall.kick = Some((*have, *need, time.elapsed_secs()));
            }
            Event::Core {
                phase,
                live,
                swap_s,
                remix,
                round,
                round_s,
                tally,
                remember,
                stance,
                plan_s,
                arms,
                stage,
                clock_s,
                ward_pct,
                wrong,
            } => {
                if *phase == 0 {
                    wall.core = None;
                    wall.log.clear();
                } else {
                    wall.core = Some(CoreWall {
                        phase: *phase,
                        live: *live,
                        swap_s: *swap_s,
                        remix: *remix,
                        round: *round,
                        round_s: *round_s,
                        tally: *tally,
                        remember: *remember,
                        stance: *stance,
                        plan_s: *plan_s,
                        arms: *arms,
                        stage: *stage,
                        clock_s: *clock_s,
                        ward_pct: *ward_pct,
                        wrong: *wrong,
                    });
                }
            }
            Event::CoreLog { lines } => wall.log.clone_from(lines),
            _ => {}
        }
    }
}

// ------------------------------------------------------------------ assets and the zone's props

#[derive(Resource)]
struct Assets6 {
    square: Handle<Mesh>,
    envelope: Handle<Mesh>,
    face_live: Handle<StandardMaterial>,
    face_standby: Handle<StandardMaterial>,
    tile_mat: Handle<StandardMaterial>,
    envelope_mat: Handle<StandardMaterial>,
    live_mat: Handle<StandardMaterial>,
    standby_mat: Handle<StandardMaterial>,
    cone_calm: Handle<StandardMaterial>,
    cone_alert: Handle<StandardMaterial>,
}

fn unlit(c: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: c,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut m = |c: Color| materials.add(unlit(c));
    let cone_calm = m(Color::srgba(1.0, 0.8, 0.2, 0.2));
    let cone_alert = m(Color::srgba(1.0, 0.2, 0.15, 0.3));
    let tile_mat = m(Color::srgba(0.02, 0.02, 0.03, 0.78));
    let envelope_mat = m(Color::srgba(1.0, 0.95, 0.6, 0.95));
    let live_mat = m(Color::srgba(0.3, 1.0, 0.6, 0.95));
    let standby_mat = m(Color::srgba(1.0, 0.3, 0.2, 0.55));
    // Ilmari's two faces for the wall monitors: made once, the monitors only swap handles.
    let mut face = |live: bool| {
        let (img, mat) = core_face(live);
        materials.add(mat(images.add(img)))
    };
    let (face_live, face_standby) = (face(true), face(false));
    commands.insert_resource(Assets6 {
        square: meshes.add(Rectangle::new(1.0, 1.0)),
        envelope: meshes.add(Cuboid::new(0.5, 0.32, 0.04)),
        face_live,
        face_standby,
        tile_mat,
        envelope_mat,
        live_mat,
        standby_mat,
        cone_calm,
        cone_alert,
    });
}

/// The zone's own entities, spawned once per visit (`ZoneDecor` takes them with the zone).
#[derive(Component)]
struct TileCover(usize);
#[derive(Component)]
struct Envelope(usize);
#[derive(Component)]
struct MonitorGlow(usize);
#[derive(Component)]
struct MonitorFace(usize);

fn build_zone(
    mut commands: Commands,
    flow: Res<GameFlow>,
    assets: Option<Res<Assets6>>,
    mut wall: ResMut<Wall>,
    mut cones: ResMut<Cones>,
    props: Query<(), Or<(With<TileCover>, With<MonitorGlow>)>>,
) {
    // A reload of the same zone takes the props with `ZoneDecor` without the zone key changing.
    let wants_props = flow.zone.starts_with("ilmari6/") || flow.zone == "ilmari_core/1";
    if wall.zone == flow.zone && (!wants_props || !props.is_empty()) {
        return;
    }
    flow.zone.clone_into(&mut wall.zone);
    wall.kiosk_alt = false;
    wall.pillar_gone = None;
    cones.live.clear();
    let Some(assets) = assets else { return };
    if flow.zone.starts_with("ilmari6/") {
        let (cx, cz) = (BOARD_POD.0, 0.0);
        for k in 0..9 {
            let (dx, dz) = ((k / 3) as f32 - 1.0, (k % 3) as f32 - 1.0);
            commands.spawn((
                TileCover(k),
                Mesh3d(assets.square.clone()),
                MeshMaterial3d(assets.tile_mat.clone()),
                Transform::from_xyz(cx + dx * 6.5, 0.075, cz + dz * 6.5)
                    .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
                    .with_scale(Vec3::splat(6.4)),
                Visibility::Hidden,
                NotShadowCaster,
                ZoneDecor,
            ));
        }
        for (k, (dx, dz, _)) in BOARD_INBOXES.into_iter().enumerate() {
            commands.spawn((
                Envelope(k),
                Mesh3d(assets.envelope.clone()),
                MeshMaterial3d(assets.envelope_mat.clone()),
                Transform::from_xyz(cx + dx, 2.4, cz + dz),
                Visibility::Hidden,
                NotShadowCaster,
                ZoneDecor,
            ));
        }
    } else if flow.zone == "ilmari_core/1" {
        for k in 0..6 {
            commands.spawn((
                MonitorGlow(k),
                Mesh3d(assets.square.clone()),
                MeshMaterial3d(assets.standby_mat.clone()),
                Transform::from_xyz(CORE_POD.0 - 15.0 + k as f32 * 6.0, 2.6, -23.82)
                    .with_scale(Vec3::new(5.0, 3.0, 1.0)),
                Visibility::Hidden,
                NotShadowCaster,
                ZoneDecor,
            ));
            // Ilmari's face on the monitor itself, a hair in front of the baked screen.
            commands.spawn((
                MonitorFace(k),
                Mesh3d(assets.square.clone()),
                MeshMaterial3d(assets.face_standby.clone()),
                Transform::from_xyz(CORE_POD.0 - 15.0 + k as f32 * 6.0, 2.6, -23.5)
                    .with_scale(Vec3::new(4.2, 2.36, 1.0)),
                Visibility::Hidden,
                NotShadowCaster,
                ZoneDecor,
            ));
        }
    }
}

// ------------------------------------------------------------------ Wing 5: sight cones

/// One flat sector per sensor, on the floor, turned with its unit; shared meshes per half angle.
#[derive(Resource, Default)]
struct Cones {
    meshes: HashMap<u32, Handle<Mesh>>,
    /// Unit key -> (range, half angle in degrees), `None` for a unit without eyes.
    defs: HashMap<String, Option<(f32, f32)>>,
    live: HashMap<EntityId, (Entity, bool)>,
}

fn sector(half_deg: f32) -> Mesh {
    let half = half_deg.to_radians();
    let mut pos = vec![[0.0, 0.0, 0.0]];
    for i in 0..=CONE_SEGS {
        let a = -half + 2.0 * half * i as f32 / CONE_SEGS as f32;
        // Units face -Z at yaw 0 (`geom::forward`).
        pos.push([a.sin(), 0.0, -a.cos()]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    let uv = vec![[0.0, 0.0]; pos.len()];
    let idx = (0..CONE_SEGS as u32)
        .flat_map(|i| [0, i + 1, i + 2])
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

fn update_cones(
    mut commands: Commands,
    sim: SimView,
    flow: Res<GameFlow>,
    assets: Option<Res<Assets6>>,
    mut cones: ResMut<Cones>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: Query<(&mut Transform, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let Some(assets) = assets else { return };
    if !flow.zone.starts_with("ilmari5/") {
        // Leaving the wing takes the entities with `ZoneDecor`; only the book-keeping stays.
        if !cones.live.is_empty() {
            cones.live.clear();
        }
        return;
    }
    let cones = &mut *cones;
    for (_, alive) in cones.live.values_mut() {
        *alive = false;
    }
    for (n, u) in sim.units().filter(|u| u.hostile && u.alive).enumerate() {
        let def = match cones.defs.get(u.key.as_str()) {
            Some(d) => *d,
            None => {
                let d = sim
                    .content()
                    .unit_def(&u.key)
                    .and_then(|d| d.sight.as_ref())
                    .map(|s| (s.range_m, s.half_angle_deg));
                cones.defs.insert(u.key.clone(), d);
                d
            }
        };
        let Some((range, half)) = def else { continue };
        let mat = if u.in_combat {
            &assets.cone_alert
        } else {
            &assets.cone_calm
        };
        // Each cone sits a hair higher than the last: overlapping sectors never z-fight.
        let at = Vec3::new(u.pos.x, 0.06 + (n % 16) as f32 * 0.004, u.pos.z);
        let tf = Transform::from_translation(at)
            .with_rotation(Quat::from_rotation_y(u.yaw))
            .with_scale(Vec3::new(range, 1.0, range));
        // A zone reload despawns the cone entities; one that is gone is drawn again.
        let existing = cones.live.get_mut(&u.id).filter(|(e, _)| q.contains(*e));
        match existing {
            Some((e, alive)) => {
                *alive = true;
                if let Ok((mut t, mut m)) = q.get_mut(*e) {
                    *t = tf;
                    if m.0 != *mat {
                        m.0 = mat.clone();
                    }
                }
            }
            None => {
                let mesh = cones
                    .meshes
                    .entry(half.to_bits())
                    .or_insert_with(|| meshes.add(sector(half)))
                    .clone();
                let e = commands
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(mat.clone()),
                        tf,
                        NotShadowCaster,
                        ZoneDecor,
                    ))
                    .id();
                cones.live.insert(u.id, (e, true));
            }
        }
    }
    cones.live.retain(|_, (e, alive)| {
        if !*alive {
            commands.entity(*e).try_despawn();
        }
        *alive
    });
}

// ------------------------------------------------------------------ baked pieces that come and go

/// The floor's own entities around the desks (Wing 6), the kiosks and the pillars (Wing 5), found
/// once per visit by position: they are baked pieces of the floor, and hiding the real thing beats
/// drawing something over it.
#[derive(Resource, Default)]
struct Hideables {
    zone: String,
    desks: Vec<Vec<Entity>>,
    kiosks: Vec<Vec<Entity>>,
    pillars: Vec<Vec<Entity>>,
    tries: u8,
}

/// Binding gives up after this many frames without a complete set.
const BIND_TRIES: u8 = 120;

fn group_near(
    floor: &Query<(Entity, &Transform), (With<FloorEntity>, Without<PointLight>)>,
    spots: &[(f32, f32)],
    r: f32,
) -> Vec<Vec<Entity>> {
    let mut groups = vec![Vec::new(); spots.len()];
    for (e, tf) in floor {
        // Walls, the floor and the ceiling are big; everything that belongs to a desk, a kiosk
        // or a pillar is small and under the ceiling.
        if tf.scale.max_element() > 3.0 || tf.translation.y > 4.0 {
            continue;
        }
        let at = tf.translation;
        if let Some(k) = spots
            .iter()
            .position(|(x, z)| (at.x - x).hypot(at.z - z) <= r)
        {
            groups[k].push(e);
        }
    }
    groups
}

fn bind_floor(
    flow: Res<GameFlow>,
    mut hide: ResMut<Hideables>,
    floor: Query<(Entity, &Transform), (With<FloorEntity>, Without<PointLight>)>,
) {
    if hide.zone != flow.zone {
        flow.zone.clone_into(&mut hide.zone);
        hide.desks.clear();
        hide.kiosks.clear();
        hide.pillars.clear();
        hide.tries = 0;
    }
    let complete = |g: &Vec<Vec<Entity>>| !g.is_empty() && g.iter().all(|v| !v.is_empty());
    if hide.tries >= BIND_TRIES || floor.is_empty() {
        return;
    }
    if flow.zone == "ilmari6/3" && !complete(&hide.desks) {
        hide.tries += 1;
        let spots: Vec<(f32, f32)> = BOARD_DESKS
            .iter()
            .map(|(dx, dz)| (BOARD_POD.0 + dx, *dz))
            .collect();
        // 1.2 m takes the desk, its screen and its chair (1 m behind it) but not the table's chairs.
        hide.desks = group_near(&floor, &spots, 1.2);
    } else if flow.zone == "ilmari5/2" && !(complete(&hide.kiosks) && complete(&hide.pillars)) {
        hide.tries += 1;
        let (cx, cz) = LAB_ROTUNDA;
        let at = |s: &[(f32, f32)]| -> Vec<(f32, f32)> {
            s.iter().map(|(dx, dz)| (cx + dx, cz + dz)).collect()
        };
        let both = [LAB_KIOSKS.as_slice(), LAB_KIOSKS_ALT.as_slice()].concat();
        hide.kiosks = group_near(&floor, &at(&both), 1.1);
        hide.pillars = group_near(&floor, &at(&LAB_PILLARS), 1.7);
    }
}

fn set_group(group: &[Entity], show: bool, vis: &mut Query<&mut Visibility>) {
    let want = visible(show);
    for e in group {
        if let Ok(mut v) = vis.get_mut(*e)
            && *v != want
        {
            *v = want;
        }
    }
}

fn update_hideables(
    time: Res<Time>,
    wall: Res<Wall>,
    hide: Res<Hideables>,
    mut vis: Query<&mut Visibility>,
) {
    // Earnings Call takes the desks from the last one; with no fight on, all five stand.
    for (k, g) in hide.desks.iter().enumerate() {
        let standing = wall.board.is_none_or(|b| k < usize::from(b.0));
        set_group(g, standing, &mut vis);
    }
    // The first four kiosks stand on the first site, the second four on the second.
    for (k, g) in hide.kiosks.iter().enumerate() {
        set_group(g, (k < LAB_KIOSKS.len()) != wall.kiosk_alt, &mut vis);
    }
    let now = time.elapsed_secs();
    for (k, g) in hide.pillars.iter().enumerate() {
        let gone = wall
            .pillar_gone
            .is_some_and(|(i, until)| i == k && now < until);
        set_group(g, !gone, &mut vis);
    }
}

// ------------------------------------------------------------------ Wing 6: the boardroom

fn visible(show: bool) -> Visibility {
    if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

fn update_boardroom(
    time: Res<Time>,
    wall: Res<Wall>,
    mut tiles: Query<(&TileCover, &mut Visibility), Without<Envelope>>,
    mut mail: Query<(&Envelope, &mut Visibility, &mut Transform), Without<TileCover>>,
) {
    let board = wall.board;
    for (t, mut vis) in &mut tiles {
        let v = visible(board.is_some_and(|b| b.3 & (1 << t.0) != 0));
        if *vis != v {
            *vis = v;
        }
    }
    let pinging = board.is_some_and(|b| b.1 > 0);
    let now = time.elapsed_secs();
    for (e, mut vis, mut tf) in &mut mail {
        let v = visible(pinging);
        if *vis != v {
            *vis = v;
        }
        if pinging {
            tf.translation.y = 2.4 + (now * 4.0 + e.0 as f32).sin() * 0.18;
            tf.rotation = Quat::from_rotation_y(now * 2.0);
        }
    }
}

// ------------------------------------------------------------------ the core's monitors

fn update_monitors(
    wall: Res<Wall>,
    assets: Option<Res<Assets6>>,
    mut q: Query<(
        &MonitorGlow,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    mut faces: Query<
        (
            &MonitorFace,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<MonitorGlow>,
    >,
) {
    let Some(assets) = assets else { return };
    for (m, mut vis, mut mat) in &mut faces {
        let v = visible(wall.core.is_some());
        if *vis != v {
            *vis = v;
        }
        let live = wall.core.is_some_and(|c| usize::from(c.live) == m.0);
        let want = if live {
            &assets.face_live
        } else {
            &assets.face_standby
        };
        if mat.0 != *want {
            mat.0 = want.clone();
        }
    }
    for (m, mut vis, mut mat) in &mut q {
        let v = visible(wall.core.is_some());
        if *vis != v {
            *vis = v;
        }
        let live = wall.core.is_some_and(|c| usize::from(c.live) == m.0);
        let want = if live {
            &assets.live_mat
        } else {
            &assets.standby_mat
        };
        if mat.0 != *want {
            mat.0 = want.clone();
        }
    }
}

// ------------------------------------------------------------------ the HUD

#[derive(Component)]
struct HudRoot;
#[derive(Component)]
struct HudText;
#[derive(Component)]
struct HudBar;
#[derive(Component)]
struct HudFill;
#[derive(Component)]
struct LogPanel;
#[derive(Component)]
struct LogText;
#[derive(Component)]
struct KickPrompt;
#[derive(Component)]
struct KickText;
#[derive(Component)]
struct HotRoot;
#[derive(Component)]
struct HotText;
#[derive(Component)]
struct HotFill;

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                top: px(64),
                left: percent(50),
                width: px(600),
                margin: UiRect::left(px(-300)),
                padding: UiRect::axes(px(10), px(5)),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(0.5, 0.8, 1.0, 0.6)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(13),
        ))
        .with_children(|c| {
            c.spawn((
                HudText,
                label("", 13.0),
                TextColor(DIM),
                TextLayout::justify(Justify::Center),
            ));
            c.spawn((
                HudBar,
                Node {
                    width: percent(100),
                    height: px(9),
                    border: UiRect::all(px(1)),
                    display: Display::None,
                    ..default()
                },
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.35)),
                BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.9)),
            ))
            .with_children(|b| {
                b.spawn((
                    HudFill,
                    Node {
                        width: percent(0),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.35, 0.9, 0.45)),
                ));
            });
        });
    commands
        .spawn((
            LogPanel,
            Node {
                position_type: PositionType::Absolute,
                top: px(128),
                left: percent(50),
                margin: UiRect::left(px(-270)),
                width: px(540),
                padding: UiRect::all(px(8)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(0.4, 1.0, 0.6, 0.5)),
            BackgroundColor(Color::srgba(0.01, 0.03, 0.02, 0.85)),
            GlobalZIndex(13),
        ))
        .with_children(|c| {
            c.spawn((
                LogText,
                label("", 10.0),
                TextColor(Color::srgb(0.5, 1.0, 0.65)),
                TextLayout::no_wrap(),
            ));
        });
    // Judgment's Hot Take: above the action bar, which spell is left and for how long.
    commands
        .spawn((
            HotRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(190),
                left: percent(50),
                width: px(320),
                margin: UiRect::left(px(-160)),
                padding: UiRect::axes(px(10), px(5)),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(1.0, 0.45, 0.25, 0.7)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(13),
        ))
        .with_children(|c| {
            c.spawn((
                HotText,
                label("", 14.0),
                TextColor(Color::srgb(1.0, 0.75, 0.45)),
                TextLayout::justify(Justify::Center),
            ));
            c.spawn((
                Node {
                    width: percent(100),
                    height: px(7),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.9)),
            ))
            .with_children(|b| {
                b.spawn((
                    HotFill,
                    Node {
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.45, 0.25)),
                ));
            });
        });
    commands
        .spawn((
            KickPrompt,
            Node {
                position_type: PositionType::Absolute,
                top: percent(34),
                left: percent(50),
                width: px(500),
                margin: UiRect::left(px(-250)),
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(14),
        ))
        .with_children(|c| {
            c.spawn((
                KickText,
                label("", 26.0),
                TextColor(Color::srgb(1.0, 0.3, 0.2)),
                TextLayout::justify(Justify::Center),
            ));
        });
}

/// The stance's name from the wall's 1-5.
fn stance_name(s: u8) -> &'static str {
    STANCES
        .get(usize::from(s).wrapping_sub(1))
        .copied()
        .unwrap_or("not chosen yet")
}

/// Pending offers (`il_ok_*`) and worn guardrails (`il_gr_*`) of the local player, as one line.
fn offer_line(sim: &SimView) -> Option<String> {
    let me = sim.unit(sim.player()?)?;
    let mut pending = None;
    let mut worn = 0u32;
    for a in &me.auras {
        if a.key.starts_with("il_ok_") {
            pending = Some((a.name.as_str(), a.remaining_ms / 1000));
        } else if a.key.starts_with("il_gr_") {
            worn += 1;
        }
    }
    match (pending, worn) {
        (None, 0) => None,
        (Some((n, s)), w) => Some(format!(
            "OFFER: {n} - do nothing to accept, refuse at an Opt-Out kiosk ({s} s){}",
            if w > 0 {
                format!("   guardrails worn: {w}")
            } else {
                String::new()
            }
        )),
        (None, w) => Some(format!("GUARDRAILS WORN: {w}")),
    }
}

/// The shared health of a fight that spreads one pool over several bodies: Wing 5's squads (Red
/// percent, Blue percent, Red's fraction as the bar) and Wing 6's CIO and Heir (each percent, the
/// pair's fraction). `None` when the pool is not on the floor.
fn shared_bar(sim: &SimView, zone: &str) -> Option<(u8, u8, f32)> {
    let pct = |hp: u64, max: u64| (hp * 100).checked_div(max).unwrap_or(0) as u8;
    if zone.starts_with("ilmari5/") {
        let (mut rh, mut rm, mut bh, mut bm) = (0u64, 0u64, 0u64, 0u64);
        for u in sim.units().filter(|u| u.alive && u.hostile) {
            let (h, m) = (u64::from(u.hp), u64::from(u.max_hp));
            if u.key.starts_with("il_red_") && u.key != "il_red_team" {
                (rh, rm) = (rh + h, rm + m);
            } else if u.key.starts_with("il_blue_") && !u.key.contains("sensor") {
                (bh, bm) = (bh + h, bm + m);
            }
        }
        (rm > 0).then(|| (pct(rh, rm), pct(bh, bm), rh as f32 / rm as f32))
    } else if zone.starts_with("ilmari6/") {
        let find = |key: &str| {
            sim.units()
                .find(|u| u.key == key && u.alive)
                .map(|u| (u64::from(u.hp), u64::from(u.max_hp)))
        };
        let (ch, cm) = find("il_cio")?;
        let (hh, hm) = find("il_cio_heir")?;
        Some((
            pct(ch, cm),
            pct(hh, hm),
            (ch + hh) as f32 / (cm + hm) as f32,
        ))
    } else {
        None
    }
}

fn update_hud(
    time: Res<Time>,
    sim: SimView,
    flow: Res<GameFlow>,
    wall: Res<Wall>,
    mut cache: Local<(String, u32)>,
    mut root: Single<&mut Node, (With<HudRoot>, Without<HudBar>, Without<HudFill>)>,
    mut text: Single<&mut Text, With<HudText>>,
    mut bar: Single<&mut Node, (With<HudBar>, Without<HudRoot>, Without<HudFill>)>,
    mut fill: Single<
        (&mut Node, &mut BackgroundColor),
        (With<HudFill>, Without<HudRoot>, Without<HudBar>),
    >,
) {
    let z = flow.zone.as_str();
    // The offer's seconds tick, so that line is rebuilt once a second; the rest on events.
    let offer = if z.starts_with("ilmari5/") {
        offer_line(&sim)
    } else {
        None
    };
    let shared = shared_bar(&sim, z);
    let sec = if offer.is_some() {
        1 + (time.elapsed_secs() as u32) % 200
    } else {
        0
    };
    let stamp = sec
        | shared.map_or(0, |(a, b, _)| {
            (u32::from(a) + 1) << 8 | (u32::from(b) + 1) << 20
        });
    if wall.is_changed() || flow.is_changed() || cache.1 != stamp {
        cache.1 = stamp;
        cache.0.clear();
        if z.starts_with("ilmari5/") {
            if let Some((standby, sensors, merged)) = wall.exercise {
                cache.0 = if standby {
                    format!(
                        "EXERCISE: STANDBY - Red and the boss wait, {sensors} sensors on watch. Stay out of the cones."
                    )
                } else if merged {
                    "EXERCISE: THE SQUADS HAVE MERGED - one team, no more mirror".to_string()
                } else {
                    format!("EXERCISE LIVE - {sensors} sensors on watch")
                };
            }
            if let Some((red, blue, _)) = shared
                && wall.exercise.is_some_and(|e| !e.0)
            {
                cache.0.push_str(&format!("\nRED {red}%   BLUE {blue}%"));
            }
            if let Some(o) = &offer {
                if !cache.0.is_empty() {
                    cache.0.push('\n');
                }
                cache.0.push_str(o);
            }
        } else if z.starts_with("ilmari6/")
            && let Some((desks, inbox_ms, sev_ms, tiles)) = wall.board
        {
            cache.0 = format!(
                "BOARDROOM - desks left: {desks}   floor tiles cleared: {}",
                tiles.count_ones()
            );
            if inbox_ms > 0 {
                cache.0.push_str(&format!(
                    "\nNEW MESSAGE - reply at any inbox terminal: {} s",
                    u32::from(inbox_ms).div_ceil(1000)
                ));
            }
            if let Some((cio, heir, _)) = shared {
                cache.0.push_str(&format!(
                    "\nCIO {cio}%   THE HEIR {heir}%   (one bar: {})",
                    (u32::from(cio) + u32::from(heir)) / 2
                ));
            }
            if sev_ms > 0 {
                cache.0.push_str(&format!(
                    "\nOFFBOARDING - severance: {} s",
                    u32::from(sev_ms).div_ceil(1000)
                ));
            }
        } else if z == "ilmari_core/1"
            && let Some(c) = wall.core
        {
            cache.0 = core_line(&c);
        }
        set_text(&mut text, &cache.0);
    }
    let display = if cache.0.is_empty() {
        Display::None
    } else {
        Display::Flex
    };
    if root.display != display {
        root.display = display;
    }
    // The bar: severance on Wing 6, the mainframe's health in P4.
    let bar_state = if z.starts_with("ilmari6/")
        && let Some(b) = wall.board
        && b.2 > 0
    {
        Some((
            f32::from(b.2) / f32::from(wall.severance_peak.max(1)),
            Color::srgb(1.0, 0.6, 0.15),
        ))
    } else if let Some((_, _, frac)) = shared
        && (z.starts_with("ilmari6/") || wall.exercise.is_some_and(|e| !e.0))
    {
        Some((frac, Color::srgb(0.9, 0.35, 0.4)))
    } else if z == "ilmari_core/1"
        && let Some(c) = wall.core
        && c.phase == 4
    {
        let col = if c.ward_pct < 30 {
            Color::srgb(1.0, 0.25, 0.2)
        } else {
            Color::srgb(0.35, 0.9, 0.45)
        };
        Some((f32::from(c.ward_pct) / 100.0, col))
    } else {
        None
    };
    let bar_display = if bar_state.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if bar.display != bar_display {
        bar.display = bar_display;
    }
    if let Some((frac, color)) = bar_state {
        let w = percent(frac.clamp(0.0, 1.0) * 100.0);
        if fill.0.width != w {
            fill.0.width = w;
        }
        if fill.1.0 != color {
            fill.1.0 = color;
        }
    }
}

/// The log wall of P4: newest commit first, the terminal each one is on in front of it.
fn update_log(
    flow: Res<GameFlow>,
    wall: Res<Wall>,
    mut panel: Single<&mut Node, (With<LogPanel>, Without<HudRoot>)>,
    mut text: Single<&mut Text, With<LogText>>,
) {
    let on = flow.zone == "ilmari_core/1" && !wall.log.is_empty();
    let d = if on { Display::Flex } else { Display::None };
    if panel.display != d {
        panel.display = d;
    }
    if on && wall.is_changed() {
        set_text(&mut text, &format!("$ git log\n{}", wall.log.join("\n")));
    }
}

/// The Hot Take bar: while the `il_hot_take` aura is on the player, one of the first three spells
/// is all that is left (the locked keycaps already wear the padlock). The seconds and the spell
/// are read off the aura and the keycaps' own verdict, so it is the same online and offline.
fn update_hot_take(
    sim: SimView,
    mut cache: Local<(u32, u32)>,
    mut root: Single<&mut Node, (With<HotRoot>, Without<HotFill>)>,
    mut text: Single<&mut Text, With<HotText>>,
    mut fill: Single<&mut Node, (With<HotFill>, Without<HotRoot>)>,
) {
    let me = sim.player();
    let aura = me
        .and_then(|m| sim.unit(m))
        .and_then(|u| u.auras.iter().find(|a| a.key == "il_hot_take").cloned());
    let show = aura.is_some();
    let d = if show { Display::Flex } else { Display::None };
    if root.display != d {
        root.display = d;
    }
    let (Some(me), Some(a)) = (me, aura) else {
        return;
    };
    let only = sim
        .bar_of(me)
        .iter()
        .take(3)
        .copied()
        .find(|&id| sim.cooldown(me, id).usable != Err(CastError::Silenced));
    let tenths = a.remaining_ms / 100;
    let key = only.map_or(0, |id| id.0);
    if cache.0 != tenths + 1 || cache.1 != key {
        *cache = (tenths + 1, key);
        let name = only
            .and_then(|id| sim.content().spell(id))
            .map_or("one spell", |s| s.name.as_str());
        set_text(
            &mut text,
            &format!(
                "HOT TAKE - only {name} for {} s",
                a.remaining_ms.div_ceil(1000)
            ),
        );
        fill.width = percent(a.remaining_ms as f32 / a.total_ms.max(1) as f32 * 100.0);
    }
}

fn update_kick(
    time: Res<Time>,
    flow: Res<GameFlow>,
    wall: Res<Wall>,
    mut node: Single<&mut Node, (With<KickPrompt>, Without<HudRoot>, Without<LogPanel>)>,
    mut text: Single<&mut Text, With<KickText>>,
) {
    let live = wall
        .kick
        .filter(|k| time.elapsed_secs() - k.2 < KICK_SHOW_S && flow.zone.starts_with("ilmari6/"));
    let d = if live.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if node.display != d {
        node.display = d;
    }
    if let Some((have, need, _)) = live {
        set_text(&mut text, &format!("KICK NOW! {have} of {need} in"));
    }
}

fn core_line(c: &CoreWall) -> String {
    let screen = u32::from(c.live) + 1;
    let t = c.tally;
    match c.phase {
        1 => {
            let swap = if c.swap_s > 0 {
                format!("   next screen in {} s", c.swap_s)
            } else {
                String::new()
            };
            let remix = if c.remix > 0 {
                format!(
                    "\nREMIX {}   Agree {}  Refuse {}  Ask {}  Remember {}",
                    c.remix, t[0], t[1], t[2], t[3]
                )
            } else {
                String::new()
            };
            format!("THE CORE - PHASE 1: screen {screen} of 6 is live{swap}{remix}")
        }
        2 => format!(
            "THE CORE - PHASE 2: round {} of 3, {} s   Agree {}  Refuse {}  Ask {}  Remember {}{}",
            c.round.max(1),
            c.round_s,
            t[0],
            t[1],
            t[2],
            t[3],
            if c.remember { " (lit)" } else { "" }
        ),
        3 => format!(
            "THE CORE - PHASE 3: stance {}   Quarterly Planning in {} s   Rack Arms standing: {}",
            stance_name(c.stance),
            c.plan_s,
            c.arms
        ),
        _ => {
            let stage = match c.stage {
                1 => "PROTECT THE MAINFRAME",
                2 => "REVERT THE COMMIT",
                _ => "THE LAST COMMIT",
            };
            format!(
                "THE CORE - PHASE 4: {stage}   clock {}:{:02}   mainframe {}%   wrong picks {}",
                c.clock_s / 60,
                c.clock_s % 60,
                c.ward_pct,
                c.wrong
            )
        }
    }
}
