//! Per-spell visuals built from primitives. Every mesh and material is created
//! once in `Fx`; effects animate by transform only (translucent materials have
//! a fixed alpha and fade by shrinking), so nothing allocates per frame or per
//! spawn. All short-lived pieces are `Particle`s, capped by `PARTICLE_CAP`.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{SimReady, SimUnit, SimView};
use crate::config::{PROJECTILE_ORIGIN_Y, PROJECTILE_TARGET_Y};
use crate::player::Player;
use crate::presentation::{PresentEvent, PresentQueue};
use bevy::prelude::*;
use combat_core::EntityId;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

/// Hard ceiling on live particles; spawns beyond it are dropped (WebGL2 draw-call budget).
const PARTICLE_CAP: usize = 260;
const CAGE_BARS: usize = 18;
const KEYCAPS: usize = 8;
const KEYCAP_SPELL: u32 = 11;
const CAFFEINE_AURA: &str = "caffeine";
const GUARDRAILS_AURA: &str = "guardrails";

pub struct SpellFxPlugin;

impl Plugin for SpellFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxBudget>()
            .init_resource::<TicketInfo>()
            .add_systems(Startup, setup_fx)
            .add_systems(
                Update,
                (
                    move_projectiles,
                    update_cluster,
                    update_particles,
                    face_camera,
                    update_wobble,
                    update_aura_fx,
                    update_cage,
                    update_keycap_ring,
                    update_speed_lines,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

#[derive(Resource, Default)]
pub struct FxBudget {
    live: usize,
}

/// What the sim said about the hit a projectile carries, read when it lands.
#[derive(Clone, Copy, Default)]
pub struct ImpactInfo {
    pub combo: bool,
    pub execute: bool,
}

#[derive(Resource, Default)]
pub struct TicketInfo(pub HashMap<u64, ImpactInfo>);

#[derive(Resource)]
pub struct Fx {
    sphere: Handle<Mesh>,
    /// Flat annulus in the XY plane (rotate to lay it on the ground).
    ring: Handle<Mesh>,
    torus: Handle<Mesh>,
    cube: Handle<Mesh>,
    cone: Handle<Mesh>,
    cyl: Handle<Mesh>,
    m: Mats,
}

struct Mats {
    white: Handle<StandardMaterial>,
    quill: Handle<StandardMaterial>,
    lens: Handle<StandardMaterial>,
    green: Handle<StandardMaterial>,
    stink: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    blood: Handle<StandardMaterial>,
    steam_brown: Handle<StandardMaterial>,
    orange: Handle<StandardMaterial>,
    magenta: Handle<StandardMaterial>,
    smoke: Handle<StandardMaterial>,
    stone: Handle<StandardMaterial>,
    duck: Handle<StandardMaterial>,
    beak: Handle<StandardMaterial>,
    key_body: Handle<StandardMaterial>,
    key_top: Handle<StandardMaterial>,
    gold: Handle<StandardMaterial>,
    beam: Handle<StandardMaterial>,
    speed: Handle<StandardMaterial>,
    cage: [Handle<StandardMaterial>; 3],
}

fn glow(color: Color) -> StandardMaterial {
    StandardMaterial {
        // Unlit ignores emissive, so the colour goes in base_color.
        base_color: color,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    }
}

fn solid(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.55,
        ..default()
    }
}

fn setup_fx(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mut g = |c: Color| mats.add(glow(c));
    let cage = [
        g(Color::srgba(0.35, 0.95, 1.0, 0.6)),
        g(Color::srgba(0.35, 0.95, 1.0, 0.4)),
        g(Color::srgba(0.35, 0.95, 1.0, 0.25)),
    ];
    let m = Mats {
        white: g(Color::srgba(1.0, 1.0, 0.95, 0.9)),
        quill: g(Color::srgba(1.0, 0.95, 0.55, 1.0)),
        lens: g(Color::srgba(0.45, 0.85, 1.0, 0.85)),
        green: g(Color::srgba(0.35, 1.0, 0.45, 0.8)),
        stink: g(Color::srgba(0.45, 0.75, 0.2, 0.28)),
        red: g(Color::srgba(1.0, 0.18, 0.15, 0.9)),
        blood: g(Color::srgba(0.6, 0.02, 0.05, 0.85)),
        steam_brown: g(Color::srgba(0.75, 0.45, 0.2, 0.45)),
        orange: g(Color::srgba(1.0, 0.6, 0.15, 0.85)),
        magenta: g(Color::srgba(0.95, 0.2, 1.0, 0.9)),
        smoke: g(Color::srgba(0.7, 0.7, 0.74, 0.5)),
        gold: g(Color::srgba(1.0, 0.85, 0.25, 0.95)),
        beam: g(Color::srgba(1.0, 1.0, 0.9, 0.35)),
        speed: g(Color::srgba(1.0, 1.0, 1.0, 0.2)),
        cage,
        stone: mats.add(solid(Color::srgb(0.45, 0.47, 0.5))),
        duck: mats.add(solid(Color::srgb(1.0, 0.85, 0.1))),
        beak: mats.add(solid(Color::srgb(1.0, 0.45, 0.05))),
        key_body: mats.add(solid(Color::srgb(0.16, 0.17, 0.2))),
        key_top: mats.add(solid(Color::srgb(0.85, 0.87, 0.9))),
    };
    commands.insert_resource(Fx {
        sphere: meshes.add(Sphere::new(1.0)),
        ring: meshes.add(Annulus::new(0.9, 1.0)),
        torus: meshes.add(Torus::new(0.92, 1.0)),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cone: meshes.add(Cone::new(0.5, 1.0)),
        cyl: meshes.add(Cylinder::new(0.5, 1.0)),
        m,
    });
}

// ---- building blocks

type Kids<'a> = ChildSpawnerCommands<'a>;

fn part(
    c: &mut Kids,
    mesh: &Handle<Mesh>,
    mat: &Handle<StandardMaterial>,
    t: Vec3,
    r: Quat,
    s: Vec3,
) -> Entity {
    c.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(mat.clone()),
        Transform {
            translation: t,
            rotation: r,
            scale: s,
        },
    ))
    .id()
}

/// Cone tip along local -Z (the travel direction of projectiles).
fn cone_forward() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2)
}

#[derive(Component)]
pub struct Particle {
    age: f32,
    life: f32,
    delay: f32,
    vel: Vec3,
    grav: f32,
    spin: Vec3,
    base: Vec3,
    s0: f32,
    s1: f32,
    orbit: Option<Orbit>,
}

#[derive(Clone, Copy)]
struct Orbit {
    center: Vec3,
    r: (f32, f32),
    y: (f32, f32),
    ang0: f32,
    speed: f32,
}

#[derive(Clone, Copy)]
struct P {
    life: f32,
    delay: f32,
    vel: Vec3,
    grav: f32,
    spin: Vec3,
    base: Vec3,
    s: (f32, f32),
    orbit: Option<Orbit>,
}

impl P {
    fn new(life: f32, s: (f32, f32)) -> Self {
        P {
            life,
            delay: 0.0,
            vel: Vec3::ZERO,
            grav: 0.0,
            spin: Vec3::ZERO,
            base: Vec3::ONE,
            s,
            orbit: None,
        }
    }
    fn vel(mut self, v: Vec3) -> Self {
        self.vel = v;
        self
    }
    fn grav(mut self, g: f32) -> Self {
        self.grav = g;
        self
    }
    fn spin(mut self, v: Vec3) -> Self {
        self.spin = v;
        self
    }
    fn base(mut self, b: Vec3) -> Self {
        self.base = b;
        self
    }
    fn delay(mut self, d: f32) -> Self {
        self.delay = d;
        self
    }
    fn orbit(mut self, o: Orbit) -> Self {
        self.orbit = Some(o);
        self
    }
}

/// Spawns a particle root; the caller adds a mesh or child parts. `None` when over budget.
fn emit<'a>(
    commands: &'a mut Commands,
    budget: &mut FxBudget,
    at: Vec3,
    rot: Quat,
    p: P,
) -> Option<EntityCommands<'a>> {
    if budget.live >= PARTICLE_CAP {
        return None;
    }
    budget.live += 1;
    Some(commands.spawn((
        Particle {
            age: 0.0,
            life: p.life,
            delay: p.delay,
            vel: p.vel,
            grav: p.grav,
            spin: p.spin,
            base: p.base,
            s0: p.s.0,
            s1: p.s.1,
            orbit: p.orbit,
        },
        Transform {
            translation: at,
            rotation: rot,
            scale: Vec3::ZERO,
        },
        Visibility::default(),
    )))
}

fn emit_mesh(
    commands: &mut Commands,
    budget: &mut FxBudget,
    (mesh, mat): (&Handle<Mesh>, &Handle<StandardMaterial>),
    at: Vec3,
    rot: Quat,
    p: P,
) {
    if let Some(mut e) = emit(commands, budget, at, rot, p) {
        e.insert((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone())));
    }
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut budget: ResMut<FxBudget>,
    mut q: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let mut live = 0;
    for (e, mut p, mut tf) in &mut q {
        if p.delay > 0.0 {
            p.delay -= dt;
            live += 1;
            continue;
        }
        p.age += dt;
        let k = p.age / p.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        live += 1;
        p.vel.y -= p.grav * dt;
        if let Some(o) = p.orbit {
            let a = o.ang0 + o.speed * p.age;
            let r = o.r.0 + (o.r.1 - o.r.0) * k;
            tf.translation =
                o.center + Vec3::new(a.cos() * r, o.y.0 + (o.y.1 - o.y.0) * k, a.sin() * r);
        } else {
            tf.translation += p.vel * dt;
        }
        if p.spin != Vec3::ZERO {
            tf.rotate(Quat::from_scaled_axis(p.spin * dt));
        }
        let ease = 1.0 - (1.0 - k) * (1.0 - k);
        let env = (k * 8.0).min(1.0) * (1.0 - k * k);
        tf.scale = p.base * ((p.s0 + (p.s1 - p.s0) * ease) * env);
    }
    budget.live = live;
}

#[derive(Component, Clone, Copy)]
enum FaceCamera {
    Full,
    Yaw,
}

fn face_camera(
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut q: Query<(&FaceCamera, &mut Transform), Without<Camera3d>>,
) {
    let Some(cam) = cams.iter().next() else {
        return;
    };
    let cam_pos = cam.translation();
    for (f, mut tf) in &mut q {
        tf.rotation = match f {
            FaceCamera::Full => cam.rotation(),
            FaceCamera::Yaw => {
                let d = cam_pos - tf.translation;
                Quat::from_rotation_y(d.x.atan2(d.z))
            }
        };
    }
}

/// Cheap deterministic noise for jitter, no RNG dependency.
fn hash(n: f32) -> f32 {
    (n.sin() * 43758.547).fract()
}

// ---- models

fn model_quill(c: &mut Kids, fx: &Fx) {
    // Shaft, tip and a short fading trail behind it.
    part(
        c,
        &fx.cone,
        &fx.m.quill,
        Vec3::ZERO,
        cone_forward(),
        Vec3::new(0.09, 0.5, 0.09),
    );
    part(
        c,
        &fx.sphere,
        &fx.m.white,
        Vec3::new(0.0, 0.0, -0.3),
        Quat::IDENTITY,
        Vec3::splat(0.04),
    );
    for i in 1..=3 {
        let f = i as f32;
        part(
            c,
            &fx.sphere,
            &fx.m.white,
            Vec3::new(0.0, 0.0, 0.2 + 0.2 * f),
            Quat::IDENTITY,
            Vec3::splat(0.07 - 0.015 * f),
        );
    }
}

fn model_magnifier(c: &mut Kids, fx: &Fx) {
    // Ring faces the direction of travel; the handle sticks out down-right.
    part(
        c,
        &fx.torus,
        &fx.m.lens,
        Vec3::ZERO,
        cone_forward(),
        Vec3::splat(0.34),
    );
    let dir = Vec3::new(0.7, -0.7, 0.0);
    part(
        c,
        &fx.cyl,
        &fx.m.stone,
        dir * 0.58,
        Quat::from_rotation_z(FRAC_PI_4),
        Vec3::new(0.07, 0.4, 0.07),
    );
}

const CLUSTER_DIRS: [Vec3; 5] = [
    Vec3::new(1.0, 0.4, 0.0),
    Vec3::new(-0.8, 0.6, 0.3),
    Vec3::new(0.2, -0.9, 0.5),
    Vec3::new(-0.3, 0.0, -1.0),
    Vec3::new(0.4, 0.8, -0.7),
];

#[derive(Component)]
struct ClusterPart(Vec3);

fn model_cubes(c: &mut Kids, fx: &Fx) {
    for (i, d) in CLUSTER_DIRS.iter().enumerate() {
        let mat = if i % 2 == 0 { &fx.m.green } else { &fx.m.white };
        let e = part(
            c,
            &fx.cube,
            mat,
            *d * 0.7,
            Quat::from_rotation_y(i as f32),
            Vec3::splat(0.2),
        );
        c.commands().entity(e).insert(ClusterPart(*d));
    }
}

fn model_arrow(c: &mut Kids, fx: &Fx) {
    // A pointing "finger": cone nose on a rounded fist.
    part(
        c,
        &fx.cone,
        &fx.m.red,
        Vec3::new(0.0, 0.0, -0.1),
        cone_forward(),
        Vec3::new(0.2, 0.55, 0.2),
    );
    part(
        c,
        &fx.sphere,
        &fx.m.red,
        Vec3::new(0.0, 0.0, 0.25),
        Quat::IDENTITY,
        Vec3::splat(0.17),
    );
}

fn model_duck(c: &mut Kids, fx: &Fx, s: f32) {
    part(
        c,
        &fx.sphere,
        &fx.m.duck,
        Vec3::ZERO,
        Quat::IDENTITY,
        Vec3::new(0.3, 0.22, 0.26) * s,
    );
    part(
        c,
        &fx.sphere,
        &fx.m.duck,
        Vec3::new(0.0, 0.26, -0.14) * s,
        Quat::IDENTITY,
        Vec3::splat(0.16) * s,
    );
    // Beak points toward -Z, the duck's forward.
    part(
        c,
        &fx.cone,
        &fx.m.beak,
        Vec3::new(0.0, 0.25, -0.3) * s,
        cone_forward(),
        Vec3::new(0.12, 0.14, 0.12) * s,
    );
}

fn model_slab(c: &mut Kids, fx: &Fx) {
    part(
        c,
        &fx.cube,
        &fx.m.stone,
        Vec3::ZERO,
        Quat::IDENTITY,
        Vec3::new(0.8, 1.0, 0.2),
    );
    part(
        c,
        &fx.cyl,
        &fx.m.stone,
        Vec3::new(0.0, 0.5, 0.0),
        Quat::from_rotation_x(FRAC_PI_2),
        Vec3::new(0.8, 0.2, 0.8),
    );
    // Engraved cross, proud of the front face.
    part(
        c,
        &fx.cube,
        &fx.m.key_top,
        Vec3::new(0.0, 0.15, 0.11),
        Quat::IDENTITY,
        Vec3::new(0.08, 0.5, 0.03),
    );
    part(
        c,
        &fx.cube,
        &fx.m.key_top,
        Vec3::new(0.0, 0.25, 0.11),
        Quat::IDENTITY,
        Vec3::new(0.32, 0.08, 0.03),
    );
}

fn model_keycap(c: &mut Kids, fx: &Fx, s: f32) {
    part(
        c,
        &fx.cube,
        &fx.m.key_body,
        Vec3::ZERO,
        Quat::IDENTITY,
        Vec3::new(0.22, 0.13, 0.22) * s,
    );
    part(
        c,
        &fx.cube,
        &fx.m.key_top,
        Vec3::new(0.0, 0.075, 0.0) * s,
        Quat::IDENTITY,
        Vec3::new(0.17, 0.03, 0.17) * s,
    );
}

fn model_x(c: &mut Kids, fx: &Fx) {
    for a in [FRAC_PI_4, -FRAC_PI_4] {
        part(
            c,
            &fx.cube,
            &fx.m.red,
            Vec3::ZERO,
            Quat::from_rotation_z(a),
            Vec3::new(1.5, 0.28, 0.08),
        );
    }
}

fn model_mug(c: &mut Kids, fx: &Fx) {
    part(
        c,
        &fx.cyl,
        &fx.m.orange,
        Vec3::ZERO,
        Quat::IDENTITY,
        Vec3::new(0.3, 0.34, 0.3),
    );
    part(
        c,
        &fx.torus,
        &fx.m.orange,
        Vec3::new(0.17, 0.0, 0.0),
        Quat::from_rotation_x(FRAC_PI_2),
        Vec3::splat(0.11),
    );
}

fn model_exclaim(c: &mut Kids, fx: &Fx) {
    part(
        c,
        &fx.cube,
        &fx.m.gold,
        Vec3::new(0.0, 0.12, 0.0),
        Quat::IDENTITY,
        Vec3::new(0.14, 0.42, 0.06),
    );
    part(
        c,
        &fx.sphere,
        &fx.m.gold,
        Vec3::new(0.0, -0.18, 0.0),
        Quat::IDENTITY,
        Vec3::splat(0.09),
    );
}

// ---- projectiles

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjKind {
    Quill,
    Magnifier,
    Cubes,
    Arrow,
    Duck,
    Slab,
    Keycaps,
    Glitch,
}

/// Spell ids with a travelling visual (see content/game.ron); `None` = instant or aura-only.
pub fn proj_kind(spell: u32) -> Option<ProjKind> {
    Some(match spell {
        1 => ProjKind::Magnifier,
        2 => ProjKind::Cubes,
        5 => ProjKind::Glitch,
        6 => ProjKind::Arrow,
        9 => ProjKind::Duck,
        10 => ProjKind::Slab,
        11 => ProjKind::Keycaps,
        _ => return None,
    })
}

impl ProjKind {
    fn speed(self) -> f32 {
        match self {
            ProjKind::Quill => 42.0,
            ProjKind::Magnifier => 30.0,
            ProjKind::Cubes => 26.0,
            ProjKind::Arrow => 34.0,
            ProjKind::Duck => 20.0,
            ProjKind::Slab => 24.0,
            ProjKind::Keycaps => 18.0,
            ProjKind::Glitch => 16.0,
        }
    }
    fn arc(self) -> f32 {
        if self == ProjKind::Duck { 2.2 } else { 0.0 }
    }
    /// Falls from above the target instead of flying from the caster.
    fn drops(self) -> Option<f32> {
        match self {
            ProjKind::Slab => Some(6.0),
            ProjKind::Keycaps => Some(8.0),
            _ => None,
        }
    }
}

#[derive(Component)]
pub struct Projectile {
    target: EntityId,
    kind: ProjKind,
    ticket: u64,
    origin: Vec3,
    dist0: f32,
    traveled: f32,
    age: f32,
    progress: f32,
}

pub fn launch(
    commands: &mut Commands,
    fx: &Fx,
    budget: &mut FxBudget,
    queue: &mut PresentQueue,
    kind: ProjKind,
    from: Vec3,
    (target, target_pos): (EntityId, Vec3),
) -> u64 {
    let ticket = queue.ticket();
    let dest = target_pos + Vec3::Y * PROJECTILE_TARGET_Y;
    let origin = match kind.drops() {
        Some(h) => dest + Vec3::Y * h,
        None => from + Vec3::Y * PROJECTILE_ORIGIN_Y,
    };
    let mut e = commands.spawn((
        Projectile {
            target,
            kind,
            ticket,
            origin,
            dist0: origin.distance(dest).max(0.5),
            traveled: 0.0,
            age: 0.0,
            progress: 0.0,
        },
        Transform::from_translation(origin),
        Visibility::default(),
    ));
    if kind == ProjKind::Slab {
        e.insert(FaceCamera::Yaw);
    }
    e.with_children(|c| match kind {
        ProjKind::Quill => model_quill(c, fx),
        ProjKind::Magnifier => model_magnifier(c, fx),
        ProjKind::Cubes => model_cubes(c, fx),
        ProjKind::Arrow => model_arrow(c, fx),
        ProjKind::Duck => model_duck(c, fx, 1.0),
        ProjKind::Slab => model_slab(c, fx),
        ProjKind::Keycaps => model_keycap(c, fx, 2.0),
        ProjKind::Glitch => {
            part(
                c,
                &fx.sphere,
                &fx.m.magenta,
                Vec3::ZERO,
                Quat::IDENTITY,
                Vec3::splat(0.28),
            );
            part(
                c,
                &fx.cube,
                &fx.m.white,
                Vec3::new(0.12, 0.08, 0.0),
                Quat::from_rotation_y(0.6),
                Vec3::splat(0.14),
            );
        }
    });
    if kind == ProjKind::Keycaps {
        // The rest of the column trails the lead keycap down, landing in sequence.
        for i in 0..14 {
            let h = hash(i as f32 * 12.9) * 0.5 + 0.5;
            let off = Vec3::new(
                (h - 0.5) * 1.4,
                0.6 + 0.5 * i as f32,
                (hash(i as f32 * 7.7) * 0.5) * 0.8,
            );
            let fall = off.y / kind.speed();
            if let Some(mut p) = emit(
                commands,
                budget,
                dest + off,
                Quat::from_euler(EulerRot::XYZ, h * 3.0, h * 6.0, 0.0),
                P::new(fall, (1.0, 1.0))
                    .vel(Vec3::NEG_Y * kind.speed())
                    .spin(Vec3::new(2.0, 3.0, 1.0)),
            ) {
                p.with_children(|c| model_keycap(c, fx, 1.6));
            }
        }
    }
    ticket
}

/// Records combo/execute flags of the hit a projectile will deliver.
pub fn note_hit(info: &mut TicketInfo, ticket: u64, combo: bool, execute: bool) {
    info.0.insert(ticket, ImpactInfo { combo, execute });
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    fx: Option<Res<Fx>>,
    mut budget: ResMut<FxBudget>,
    mut queue: ResMut<PresentQueue>,
    mut info: ResMut<TicketInfo>,
    mut present: MessageWriter<PresentEvent>,
    sim: SimView,
    units: Query<(&SimUnit, &Transform), Without<Projectile>>,
    mut shots: Query<(Entity, &mut Projectile, &mut Transform)>,
) {
    let Some(fx) = fx else { return };
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (entity, mut s, mut tf) in &mut shots {
        s.age += dt;
        let target_tf = units.iter().find(|(su, _)| su.0 == s.target);
        let Some((_, ttf)) = target_tf else {
            commands.entity(entity).despawn();
            for ev in queue.release(s.ticket) {
                present.write(PresentEvent(ev));
            }
            info.0.remove(&s.ticket);
            continue;
        };
        let feet = ttf.translation;
        let dest = feet + Vec3::Y * PROJECTILE_TARGET_Y;
        s.traveled += s.kind.speed() * dt;
        let k = (s.traveled / s.dist0).min(1.0);
        s.progress = k;
        if k >= 1.0 {
            commands.entity(entity).despawn();
            for ev in queue.release(s.ticket) {
                present.write(PresentEvent(ev));
            }
            let flags = info.0.remove(&s.ticket).unwrap_or_default();
            let head = sim
                .units()
                .find(|u| u.id == s.target)
                .map_or(1.8, |u| crate::config::head_height(&u.key));
            impact(&mut commands, &fx, &mut budget, s.kind, feet, head, flags);
            continue;
        }
        tf.translation = s.origin.lerp(dest, k) + Vec3::Y * (s.kind.arc() * 4.0 * k * (1.0 - k));
        let dir = (dest - s.origin).normalize_or_zero();
        match s.kind {
            ProjKind::Quill | ProjKind::Arrow => {
                tf.look_to(dir, Vec3::Y);
            }
            ProjKind::Magnifier => {
                tf.look_to(dir, Vec3::Y);
                tf.rotate_local_z(s.age * 5.0);
            }
            ProjKind::Cubes => tf.rotation = Quat::from_rotation_y(s.age * 7.0),
            ProjKind::Duck => {
                tf.look_to(dir, Vec3::Y);
                tf.rotate_local_x(s.age * 14.0);
            }
            ProjKind::Keycaps => tf.rotation = Quat::from_rotation_y(s.age * 8.0),
            ProjKind::Glitch => {
                tf.scale = Vec3::new(
                    1.0 + 0.35 * (t * 37.0).sin(),
                    1.0 + 0.35 * (t * 43.0 + 1.0).sin(),
                    1.0 + 0.35 * (t * 31.0 + 2.0).sin(),
                );
                tf.translation += Vec3::new(hash(t * 91.0), hash(t * 53.0), hash(t * 77.0)) * 0.12;
            }
            ProjKind::Slab => {}
        }
    }
}

/// Pulls the cube cluster together as it approaches, so it "snaps" shut on arrival.
fn update_cluster(
    shots: Query<&Projectile>,
    mut parts: Query<(&ClusterPart, &ChildOf, &mut Transform)>,
) {
    for (part, parent, mut tf) in &mut parts {
        if let Ok(s) = shots.get(parent.parent()) {
            tf.translation = part.0.normalize_or_zero() * (0.1 + 0.8 * (1.0 - s.progress));
        }
    }
}

fn flash(
    commands: &mut Commands,
    fx: &Fx,
    budget: &mut FxBudget,
    mat: &Handle<StandardMaterial>,
    at: Vec3,
    (life, from, to): (f32, f32, f32),
) {
    emit_mesh(
        commands,
        budget,
        (&fx.sphere, mat),
        at,
        Quat::IDENTITY,
        P::new(life, (from, to)),
    );
}

fn ground_ring(
    commands: &mut Commands,
    fx: &Fx,
    budget: &mut FxBudget,
    mat: &Handle<StandardMaterial>,
    at: Vec3,
    (life, from, to): (f32, f32, f32),
) {
    emit_mesh(
        commands,
        budget,
        (&fx.ring, mat),
        at,
        Quat::from_rotation_x(-FRAC_PI_2),
        P::new(life, (from, to)),
    );
}

/// Outward-flying cubes of a burst.
fn sparks(
    commands: &mut Commands,
    fx: &Fx,
    budget: &mut FxBudget,
    mat: &Handle<StandardMaterial>,
    at: Vec3,
    (n, speed, size): (usize, f32, f32),
) {
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        let up = 1.0 + hash(i as f32 * 3.1).abs() * 2.0;
        emit_mesh(
            commands,
            budget,
            (&fx.cube, mat),
            at,
            Quat::from_rotation_y(a),
            P::new(0.5, (1.0, 0.3))
                .vel(Vec3::new(a.cos() * speed, up, a.sin() * speed))
                .grav(7.0)
                .spin(Vec3::new(5.0, 7.0, 3.0))
                .base(Vec3::splat(size)),
        );
    }
}

fn impact(
    commands: &mut Commands,
    fx: &Fx,
    budget: &mut FxBudget,
    kind: ProjKind,
    feet: Vec3,
    head: f32,
    info: ImpactInfo,
) {
    let mid = feet + Vec3::Y * PROJECTILE_TARGET_Y;
    let m = &fx.m;
    match kind {
        ProjKind::Quill => {
            flash(commands, fx, budget, &m.quill, mid, (0.22, 0.1, 0.4));
            sparks(commands, fx, budget, &m.white, mid, (4, 1.5, 0.05));
        }
        ProjKind::Magnifier => {
            flash(commands, fx, budget, &m.lens, mid, (0.25, 0.2, 0.8));
            // The scan line sweeps feet to head; the "!" pops above.
            emit_mesh(
                commands,
                budget,
                (&fx.ring, &m.lens),
                feet + Vec3::Y * 0.05,
                Quat::from_rotation_x(-FRAC_PI_2),
                P::new(0.5, (0.8, 0.8)).vel(Vec3::Y * head / 0.5),
            );
            if let Some(mut p) = emit(
                commands,
                budget,
                feet + Vec3::Y * (head + 0.55),
                Quat::IDENTITY,
                P::new(0.9, (0.6, 1.1)).vel(Vec3::Y * 0.5),
            ) {
                p.insert(FaceCamera::Full);
                p.with_children(|c| model_exclaim(c, fx));
            }
        }
        ProjKind::Cubes => {
            flash(commands, fx, budget, &m.white, mid, (0.3, 0.3, 1.3));
            sparks(commands, fx, budget, &m.green, mid, (6, 3.0, 0.14));
            if info.combo {
                flash(commands, fx, budget, &m.green, mid, (0.5, 0.4, 2.4));
                ground_ring(
                    commands,
                    fx,
                    budget,
                    &m.green,
                    feet + Vec3::Y * 0.1,
                    (0.5, 0.5, 3.4),
                );
                sparks(commands, fx, budget, &m.stink, mid, (10, 5.0, 0.2));
            }
        }
        ProjKind::Arrow => {
            flash(commands, fx, budget, &m.red, mid, (0.3, 0.2, 0.9));
            ground_ring(
                commands,
                fx,
                budget,
                &m.red,
                feet + Vec3::Y * 0.1,
                (0.4, 0.4, 1.6),
            );
        }
        ProjKind::Duck => {
            flash(commands, fx, budget, &m.gold, mid, (0.25, 0.2, 0.8));
            // The duck bounces off the target's head.
            if let Some(mut p) = emit(
                commands,
                budget,
                mid,
                Quat::IDENTITY,
                P::new(0.7, (1.0, 1.0))
                    .vel(Vec3::new(0.8, 5.0, 0.0))
                    .grav(16.0)
                    .spin(Vec3::new(0.0, 0.0, 9.0)),
            ) {
                p.with_children(|c| model_duck(c, fx, 1.0));
            }
        }
        ProjKind::Slab => {
            ground_ring(
                commands,
                fx,
                budget,
                &m.smoke,
                feet + Vec3::Y * 0.1,
                (0.5, 0.4, 2.4),
            );
            flash(
                commands,
                fx,
                budget,
                &m.smoke,
                feet + Vec3::Y * 0.4,
                (0.4, 0.3, 1.2),
            );
            if let Some(mut p) = emit(
                commands,
                budget,
                feet + Vec3::Y * 0.5,
                Quat::IDENTITY,
                P::new(0.6, (1.0, 1.0)),
            ) {
                p.insert(FaceCamera::Yaw);
                p.with_children(|c| model_slab(c, fx));
            }
            if info.execute {
                flash(commands, fx, budget, &m.blood, mid, (0.6, 0.5, 2.8));
                ground_ring(
                    commands,
                    fx,
                    budget,
                    &m.blood,
                    feet + Vec3::Y * 0.1,
                    (0.6, 0.5, 4.2),
                );
                sparks(commands, fx, budget, &m.blood, mid, (12, 5.5, 0.22));
            }
        }
        ProjKind::Keycaps => {
            flash(commands, fx, budget, &m.white, mid, (0.45, 0.6, 3.2));
            ground_ring(
                commands,
                fx,
                budget,
                &m.gold,
                feet + Vec3::Y * 0.1,
                (0.5, 0.6, 3.8),
            );
            emit_mesh(
                commands,
                budget,
                (&fx.cyl, &m.beam),
                feet + Vec3::Y * 3.0,
                Quat::IDENTITY,
                P::new(0.4, (1.0, 0.4)).base(Vec3::new(1.4, 6.0, 1.4)),
            );
            sparks(commands, fx, budget, &m.white, mid, (10, 4.0, 0.16));
        }
        ProjKind::Glitch => {
            flash(commands, fx, budget, &m.magenta, mid, (0.3, 0.2, 1.0));
            sparks(commands, fx, budget, &m.magenta, mid, (6, 2.5, 0.12));
        }
    }
}

// ---- instant casts and one-shots

/// Effects of casts that have no projectile. `target` is (feet, head height) when targeted.
pub fn cast_instant(
    commands: &mut Commands,
    fx: &Fx,
    budget: &mut FxBudget,
    spell: u32,
    from: Vec3,
    target: Option<(Vec3, f32)>,
) {
    let m = &fx.m;
    match spell {
        3 => {
            ground_ring(
                commands,
                fx,
                budget,
                &m.cage[0],
                from + Vec3::Y * 0.1,
                (0.5, 0.4, 2.4),
            );
            ground_ring(
                commands,
                fx,
                budget,
                &m.cage[1],
                from + Vec3::Y * 1.0,
                (0.6, 0.4, 2.0),
            );
        }
        4 => {
            if let Some((feet, head)) = target {
                let at = feet + Vec3::Y * (head * 0.6);
                if let Some(mut p) = emit(
                    commands,
                    budget,
                    at + Vec3::Y * 0.2,
                    Quat::IDENTITY,
                    P::new(0.7, (3.2, 1.0)).base(Vec3::splat(0.85)),
                ) {
                    p.insert(FaceCamera::Full);
                    p.with_children(|c| model_x(c, fx));
                }
                flash(commands, fx, budget, &m.red, at, (0.25, 0.3, 1.0));
            }
        }
        7 => {
            // Rewind: green cubes spiral inwards and drop into the player.
            for i in 0..14 {
                let a = i as f32 * 0.9;
                emit_mesh(
                    commands,
                    budget,
                    (&fx.cube, &m.green),
                    from + Vec3::Y * 1.8,
                    Quat::IDENTITY,
                    P::new(0.65, (1.0, 0.6))
                        .base(Vec3::splat(0.11))
                        .spin(Vec3::new(4.0, 6.0, 0.0))
                        .delay(i as f32 * 0.025)
                        .orbit(Orbit {
                            center: from,
                            r: (1.7, 0.05),
                            y: (1.9, 0.9),
                            ang0: a,
                            speed: -9.0,
                        }),
                );
            }
            for (y, d) in [(1.5, 0.0), (1.0, 0.08), (0.5, 0.16)] {
                emit_mesh(
                    commands,
                    budget,
                    (&fx.torus, &m.green),
                    from + Vec3::Y * y,
                    Quat::from_rotation_x(-FRAC_PI_2),
                    P::new(0.6, (1.6, 0.15)).delay(d),
                );
            }
            flash(
                commands,
                fx,
                budget,
                &m.green,
                from + Vec3::Y * 1.0,
                (0.3, 0.4, 1.1),
            );
        }
        8 => {
            for i in 0..12 {
                emit_mesh(
                    commands,
                    budget,
                    (&fx.sphere, &m.steam_brown),
                    from,
                    Quat::IDENTITY,
                    P::new(1.1, (0.12, 0.34))
                        .delay(i as f32 * 0.05)
                        .orbit(Orbit {
                            center: from,
                            r: (0.35, 0.95),
                            y: (0.1, 2.3),
                            ang0: i as f32 * 1.3,
                            speed: 3.5,
                        }),
                );
            }
            if let Some(mut p) = emit(
                commands,
                budget,
                from + Vec3::Y * 2.3,
                Quat::IDENTITY,
                P::new(1.0, (0.6, 1.5))
                    .vel(Vec3::Y * 0.8)
                    .spin(Vec3::Y * 3.0),
            ) {
                p.with_children(|c| model_mug(c, fx));
            }
            ground_ring(
                commands,
                fx,
                budget,
                &m.orange,
                from + Vec3::Y * 0.1,
                (0.6, 0.4, 2.2),
            );
        }
        _ => {}
    }
}

pub fn interrupt_ring(commands: &mut Commands, fx: &Fx, budget: &mut FxBudget, at: Vec3) {
    ground_ring(
        commands,
        fx,
        budget,
        &fx.m.red,
        at + Vec3::Y * 0.1,
        (0.5, 0.4, 3.6),
    );
    flash(
        commands,
        fx,
        budget,
        &fx.m.red,
        at + Vec3::Y * 1.2,
        (0.3, 0.3, 1.4),
    );
}

pub fn summon_puff(commands: &mut Commands, fx: &Fx, budget: &mut FxBudget, at: Vec3) {
    for i in 0..7 {
        let a = i as f32 / 7.0 * TAU;
        emit_mesh(
            commands,
            budget,
            (&fx.sphere, &fx.m.smoke),
            at + Vec3::Y * 0.3,
            Quat::IDENTITY,
            P::new(0.9, (0.25, 0.6)).vel(Vec3::new(a.cos() * 0.9, 0.9, a.sin() * 0.9)),
        );
    }
    ground_ring(
        commands,
        fx,
        budget,
        &fx.m.magenta,
        at + Vec3::Y * 0.08,
        (0.6, 0.3, 1.8),
    );
}

/// A red arc of dots hopping from the dead unit's spot to the new victim.
pub fn spread_arc(commands: &mut Commands, fx: &Fx, budget: &mut FxBudget, from: Vec3, to: Vec3) {
    const N: usize = 9;
    for i in 0..N {
        let k = i as f32 / (N - 1) as f32;
        let pos = from.lerp(to, k) + Vec3::Y * (1.2 + 2.0 * 4.0 * k * (1.0 - k));
        emit_mesh(
            commands,
            budget,
            (&fx.sphere, &fx.m.red),
            pos,
            Quat::IDENTITY,
            P::new(0.5, (0.18, 0.05)).delay(k * 0.25),
        );
    }
}

// ---- persistent per-unit effects

/// Shown while the unit carries the aura `key`.
#[derive(Component)]
struct AuraFx {
    id: EntityId,
    key: &'static str,
}

/// Slow local motion for puffs, markers and the stun duck.
#[derive(Component)]
struct Wobble {
    phase: f32,
    speed: f32,
    radius: f32,
    bob: f32,
    center: Vec3,
    base: f32,
    pulse: f32,
}

#[derive(Component)]
struct Cage;
#[derive(Component)]
struct CageBar(usize);
#[derive(Component)]
struct KeycapRing;
#[derive(Component)]
struct SpeedLines;
#[derive(Component)]
struct Streak {
    phase: f32,
    home: Vec3,
}

pub fn attach_unit_fx(c: &mut Kids, fx: &Fx, id: EntityId, head: f32, is_player: bool) {
    let m = &fx.m;
    // Code Smell: slow green puffs around the body.
    c.spawn((
        AuraFx {
            id,
            key: "code_smell",
        },
        Transform::from_xyz(0.0, head * 0.5, 0.0),
        Visibility::Hidden,
    ))
    .with_children(|s| {
        for i in 0..4 {
            let e = part(
                s,
                &fx.sphere,
                &m.stink,
                Vec3::ZERO,
                Quat::IDENTITY,
                Vec3::splat(0.3),
            );
            s.commands().entity(e).insert(Wobble {
                phase: i as f32 * 1.6,
                speed: 0.6 + 0.1 * i as f32,
                radius: 0.55,
                bob: head * 0.3,
                center: Vec3::ZERO,
                base: 0.3,
                pulse: 0.25,
            });
        }
    });
    // git blame: pulsing red marker above the head.
    c.spawn((
        AuraFx {
            id,
            key: "git_blame",
        },
        Transform::from_xyz(0.0, head + 0.65, 0.0),
        Visibility::Hidden,
    ))
    .with_children(|s| {
        let e = s
            .spawn((Transform::default(), Visibility::default()))
            .with_children(|b| {
                part(
                    b,
                    &fx.cone,
                    &fx.m.red,
                    Vec3::ZERO,
                    Quat::from_rotation_x(std::f32::consts::PI),
                    Vec3::new(0.3, 0.42, 0.3),
                );
                part(
                    b,
                    &fx.cyl,
                    &fx.m.red,
                    Vec3::new(0.0, 0.3, 0.0),
                    Quat::IDENTITY,
                    Vec3::new(0.12, 0.25, 0.12),
                );
            })
            .id();
        s.commands().entity(e).insert(Wobble {
            phase: 0.0,
            speed: 5.0,
            radius: 0.0,
            bob: 0.1,
            center: Vec3::ZERO,
            base: 1.0,
            pulse: 0.2,
        });
    });
    // Rubber Duck stun: a small duck circles the head.
    c.spawn((
        AuraFx {
            id,
            key: "rubber_duck",
        },
        Transform::from_xyz(0.0, head + 0.3, 0.0),
        Visibility::Hidden,
    ))
    .with_children(|s| {
        let e = s
            .spawn((Transform::default(), Visibility::default()))
            .with_children(|d| model_duck(d, fx, 0.7))
            .id();
        s.commands().entity(e).insert(Wobble {
            phase: 0.0,
            speed: 4.0,
            radius: 0.5,
            bob: 0.05,
            center: Vec3::ZERO,
            base: 1.0,
            pulse: 0.0,
        });
    });

    if !is_player {
        return;
    }
    // Guardrails: rotating hexagonal cage; bars drop out as the absorb depletes.
    c.spawn((
        Cage,
        Transform::from_xyz(0.0, 0.95, 0.0),
        Visibility::Hidden,
    ))
    .with_children(|cage| {
        let r = 0.95;
        let vertex = |i: usize| {
            let a = i as f32 / 6.0 * TAU;
            Vec3::new(a.cos() * r, 0.0, a.sin() * r)
        };
        let mut n = 0;
        let mut bar = |cage: &mut Kids, t: Vec3, rot: Quat, s: Vec3| {
            // Spread the crack order so neighbours do not vanish together.
            let rank = (n * 7) % CAGE_BARS;
            n += 1;
            let e = part(cage, &fx.cube, &fx.m.cage[0], t, rot, s);
            cage.commands().entity(e).insert(CageBar(rank));
        };
        for i in 0..6 {
            bar(cage, vertex(i), Quat::IDENTITY, Vec3::new(0.05, 1.9, 0.05));
        }
        for y in [-0.95, 0.95] {
            for i in 0..6 {
                let (a, b) = (vertex(i), vertex(i + 1));
                let mid = (a + b) * 0.5 + Vec3::Y * y;
                let ang = (b.z - a.z).atan2(b.x - a.x);
                bar(
                    cage,
                    mid,
                    Quat::from_rotation_y(-ang),
                    Vec3::new(r, 0.05, 0.05),
                );
            }
        }
    });
    // Write It By Hand: keycaps orbit the player, faster as the cast fills.
    c.spawn((
        KeycapRing,
        Transform::from_xyz(0.0, 1.0, 0.0),
        Visibility::Hidden,
    ))
    .with_children(|ring| {
        for i in 0..KEYCAPS {
            let a = i as f32 / KEYCAPS as f32 * TAU;
            ring.spawn((
                Transform::from_xyz(a.cos() * 1.05, 0.25 * (i % 2) as f32, a.sin() * 1.05)
                    .with_rotation(Quat::from_rotation_y(-a)),
                Visibility::default(),
            ))
            .with_children(|k| model_keycap(k, fx, 1.5));
        }
    });
    // Caffeine haste: faint streaks sliding past.
    c.spawn((SpeedLines, Transform::default(), Visibility::Hidden))
        .with_children(|lines| {
            for i in 0..8 {
                let a = i as f32 * 2.4;
                let home = Vec3::new(a.cos() * 0.8, 0.3 + 1.4 * (hash(i as f32).abs()), 0.0);
                let e = part(
                    lines,
                    &fx.cube,
                    &fx.m.speed,
                    home,
                    Quat::IDENTITY,
                    Vec3::new(0.03, 0.03, 1.1),
                );
                lines.commands().entity(e).insert(Streak {
                    phase: i as f32 * 0.37,
                    home,
                });
            }
        });
}

fn update_wobble(time: Res<Time>, mut q: Query<(&Wobble, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (w, mut tf) in &mut q {
        let a = t * w.speed + w.phase;
        tf.translation = w.center
            + Vec3::new(
                a.cos() * w.radius,
                (a * 1.3).sin() * w.bob,
                a.sin() * w.radius,
            );
        tf.scale = Vec3::splat(w.base * (1.0 + w.pulse * (a * 2.0).sin()));
    }
}

fn update_aura_fx(sim: SimView, mut q: Query<(&AuraFx, &mut Visibility)>) {
    for (a, mut vis) in &mut q {
        let on = sim
            .unit(a.id)
            .is_some_and(|u| u.alive && u.auras.iter().any(|x| x.key == a.key));
        vis.set_if_neq(if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

fn update_cage(
    time: Res<Time>,
    fx: Option<Res<Fx>>,
    sim: SimView,
    player: Single<&SimUnit, With<Player>>,
    mut peak: Local<u32>,
    mut cage: Query<(&mut Visibility, &mut Transform), With<Cage>>,
    mut bars: Query<
        (
            &CageBar,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<Cage>,
    >,
) {
    let Some(fx) = fx else { return };
    let rem = sim.unit(player.0).and_then(|u| {
        u.auras
            .iter()
            .find(|a| a.key == GUARDRAILS_AURA)
            .and_then(|a| a.absorb_remaining)
            .filter(|n| *n > 0)
    });
    let Some(rem) = rem else {
        *peak = 0;
        for (mut v, _) in &mut cage {
            v.set_if_neq(Visibility::Hidden);
        }
        return;
    };
    *peak = (*peak).max(rem);
    let frac = rem as f32 / *peak as f32;
    let visible = ((frac * CAGE_BARS as f32).ceil() as usize).clamp(4, CAGE_BARS);
    let level = if frac > 0.66 {
        0
    } else if frac > 0.33 {
        1
    } else {
        2
    };
    let t = time.elapsed_secs();
    for (mut v, mut tf) in &mut cage {
        v.set_if_neq(Visibility::Inherited);
        tf.rotation = Quat::from_rotation_y(t * 0.9);
    }
    for (bar, mut v, mut mat) in &mut bars {
        v.set_if_neq(if bar.0 < visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if mat.0 != fx.m.cage[level] {
            mat.0 = fx.m.cage[level].clone();
        }
    }
}

fn update_keycap_ring(
    time: Res<Time>,
    sim: SimView,
    player: Single<&SimUnit, With<Player>>,
    mut angle: Local<f32>,
    mut ring: Query<(&mut Visibility, &mut Transform), With<KeycapRing>>,
) {
    let cast = sim
        .unit(player.0)
        .and_then(|u| u.cast)
        .filter(|c| c.spell.0 == KEYCAP_SPELL);
    for (mut v, mut tf) in &mut ring {
        let Some(c) = cast else {
            v.set_if_neq(Visibility::Hidden);
            *angle = 0.0;
            continue;
        };
        v.set_if_neq(Visibility::Inherited);
        let progress = c.elapsed_ms as f32 / c.total_ms.max(1) as f32;
        *angle += time.delta_secs() * (2.0 + 16.0 * progress * progress);
        tf.rotation = Quat::from_rotation_y(*angle);
    }
}

fn update_speed_lines(
    time: Res<Time>,
    sim: SimView,
    player: Single<&SimUnit, With<Player>>,
    mut root: Query<&mut Visibility, With<SpeedLines>>,
    mut streaks: Query<(&Streak, &mut Transform)>,
) {
    let on = sim
        .unit(player.0)
        .is_some_and(|u| u.auras.iter().any(|a| a.key == CAFFEINE_AURA));
    for mut v in &mut root {
        v.set_if_neq(if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if !on {
        return;
    }
    let t = time.elapsed_secs();
    for (s, mut tf) in &mut streaks {
        // Slide from in front of the player to behind, then wrap.
        let k = (t * 2.5 + s.phase).fract();
        tf.translation = Vec3::new(s.home.x, s.home.y, -1.2 + 3.0 * k);
    }
}
