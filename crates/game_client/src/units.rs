//! NPC visuals. Models and silhouettes differ so hostile vs friendly reads at a
//! glance. The combat bridge spawns one `spawn_unit` per sim unit, keyed by the
//! sim unit key; `WOW_LINEUP` shows the whole cast on a bare stage.

use crate::anim::{Character, ModelKind, Paints};
use crate::catalog::{self, IdlePose};
use crate::combat_anim::CombatAnim;
use crate::rigs::PrimRig;
use bevy::prelude::*;
use std::f32::consts::PI;

/// Sim-facing mirror of a unit, refreshed by the combat bridge every tick.
#[derive(Component, Clone, Copy)]
pub struct Unit {
    pub hostile: bool,
    pub alive: bool,
    /// A corpse the sim still lets us loot: the picker gives it a generous volume and priority.
    pub lootable: bool,
    /// A living enemy in combat: while any exists, clicks near corpses do not override targeting.
    pub fighting: bool,
}

pub struct UnitsPlugin;

#[derive(Component)]
struct Halo;

/// Shared meshes and materials for unit attachments (CEO halo, IT headphones).
#[derive(Resource)]
pub struct UnitAssets {
    cube: Handle<Mesh>,
    dark: Handle<StandardMaterial>,
    halo_mesh: Handle<Mesh>,
    halo: Handle<StandardMaterial>,
}

impl Plugin for UnitsPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(which) = std::env::var("WOW_LINEUP") {
            app.insert_resource(Lineup(which.parse().unwrap_or(1)))
                .add_systems(Startup, spawn_lineup);
        }
        app.add_systems(Startup, (setup_unit_assets, setup_sleep_assets))
            .add_systems(Update, (hide_dead_halos, emit_zs, animate_zs));
    }
}

fn setup_unit_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(UnitAssets {
        cube: meshes.add(Cuboid::from_length(1.0)),
        dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.08, 0.1),
            ..default()
        }),
        halo_mesh: meshes.add(Torus::new(0.2, 0.3)),
        halo: materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(5.0, 3.5, 0.4),
            ..default()
        }),
    });
}

/// Character with catalog model, scale and paints; no sim components.
pub fn spawn_visual(
    commands: &mut Commands,
    key: &str,
    pos: Vec3,
    yaw: f32,
    pose: IdlePose,
) -> Entity {
    let vis = catalog::unit_visual(key);
    if let Some(kind) = catalog::primitive_rig(key) {
        return commands
            .spawn((
                PrimRig(kind),
                CombatAnim::default(),
                pose,
                Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
            ))
            .id();
    }
    let e = commands
        .spawn((
            Character {
                kind: ModelKind(vis.model),
                scale: vis.scale,
            },
            Paints(vis.paints),
            pose,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .id();
    if key == "scooter_exec" {
        // Standing on his own scooter: the deck sits under the feet.
        commands.spawn((
            PrimRig("scooter"),
            ChildOf(e),
            Transform::default(),
            Visibility::default(),
        ));
    }
    e
}

/// `spawn_visual` plus the `Unit` mirror and per-character attachments.
pub fn spawn_unit(
    commands: &mut Commands,
    assets: &UnitAssets,
    key: &str,
    (pos, yaw): (Vec3, f32),
    pose: IdlePose,
    hostile: bool,
) -> Entity {
    let e = spawn_visual(commands, key, pos, yaw, pose);
    commands.entity(e).insert(Unit {
        hostile,
        alive: true,
        lootable: false,
        fighting: false,
    });
    let height = catalog::unit_visual(key).height;
    match key {
        "ceo" => {
            commands.entity(e).with_child((
                Mesh3d(assets.halo_mesh.clone()),
                MeshMaterial3d(assets.halo.clone()),
                Transform::from_xyz(0.0, height + 0.3, 0.0),
                Halo,
            ));
        }
        "it_support" if pose != IdlePose::Sleeping => {
            commands
                .entity(e)
                .with_children(|c| headphones(c, assets, height));
        }
        _ => {}
    }
    e
}

/// Headphones from three dark boxes, parented at head height.
fn headphones(c: &mut ChildSpawnerCommands, assets: &UnitAssets, h: f32) {
    for (offset, size) in [
        (Vec3::new(0.0, h - 0.04, 0.0), Vec3::new(0.34, 0.04, 0.06)),
        (Vec3::new(0.17, h - 0.14, 0.0), Vec3::new(0.06, 0.14, 0.12)),
        (Vec3::new(-0.17, h - 0.14, 0.0), Vec3::new(0.06, 0.14, 0.12)),
    ] {
        c.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.dark.clone()),
            Transform::from_translation(offset).with_scale(size),
        ));
    }
}

#[derive(Resource)]
struct Lineup(u32);

/// Player skins in class order (`Content::classes`): (class key, unit key).
pub const CLASS_SKINS: [(&str, &str); 6] = [
    ("fullstack", "senior_engineer"),
    ("frontend", "frontend_dev"),
    ("backend", "backend_dev"),
    ("devops", "devops_eng"),
    ("qa", "qa_engineer"),
    ("data_scientist", "data_scientist"),
];

/// A player skin with its class kit and a fixed hair stage, for display only.
pub fn spawn_skin(commands: &mut Commands, class: usize, pos: Vec3, yaw: f32, hair: u8) -> Entity {
    let (key, unit) = CLASS_SKINS[class.min(CLASS_SKINS.len() - 1)];
    let e = spawn_visual(commands, unit, pos, yaw, IdlePose::Standing);
    commands.entity(e).insert((
        crate::gear::Accessories(key.to_string()),
        crate::hair::HairStage(hair),
    ));
    e
}

/// `WOW_LINEUP=1|2`: half of the cast per screenshot, two rows facing the player.
/// `WOW_LINEUP=4`: the class skins in front, the Backend's five hair stages behind.
fn spawn_lineup(mut commands: Commands, lineup: Res<Lineup>) {
    // 0: bare stage, only the player (pose close-ups).
    if lineup.0 == 0 {
        return;
    }
    if lineup.0 == 4 {
        for k in 0..CLASS_SKINS.len() {
            let x = (k as f32 - (CLASS_SKINS.len() as f32 - 1.0) / 2.0) * 1.5;
            spawn_skin(&mut commands, k, Vec3::new(x, 0.0, -1.0), PI, 0);
        }
        for stage in 1..=5u8 {
            let x = (f32::from(stage) - 3.0) * 1.3;
            spawn_skin(&mut commands, 2, Vec3::new(x, 0.0, -3.6), PI, stage);
        }
        return;
    }
    // 3: only the campus, data center and lab units (everything after the tower cast), wider
    // spaced because the rigs are big; WOW_CAM_DIST=20 frames them.
    let new_only = lineup.0 >= 3;
    // 5: from the offsite raid cast on (raids and minigames), same framing as 3.
    let keys: Vec<&'static str> = if lineup.0 == 5 {
        catalog::unit_keys()
            .skip_while(|k| *k != "offsite_concierge")
            .collect()
    } else if new_only {
        catalog::unit_keys()
            .skip_while(|k| *k != "ceo")
            .skip(1)
            .collect()
    } else {
        catalog::unit_keys()
            .filter(|k| !CLASS_SKINS.iter().any(|(_, u)| u == k))
            .collect()
    };
    let half = keys.len().div_ceil(2);
    let slice = if new_only {
        &keys[..]
    } else if lineup.0 >= 2 {
        &keys[half..]
    } else {
        &keys[..half]
    };
    let rows = if new_only { 3 } else { 2 };
    let spacing = if new_only { 3.6 } else { 2.3 };
    let per_row = slice.len().div_ceil(rows);
    for (i, key) in slice.iter().enumerate() {
        let (row, col) = (i / per_row, i % per_row);
        // The back row is shifted half a slot so nobody hides behind a neighbour.
        let x = (col as f32 - (per_row as f32 - 1.0) / 2.0) * spacing + row as f32 * spacing / 2.0
            - 0.6;
        let z = -3.0 - row as f32 * if new_only { 4.6 } else { 3.8 };
        // Yaw PI faces +Z, towards the camera. No `Unit`, so lineup models are not targetable.
        spawn_visual(
            &mut commands,
            key,
            Vec3::new(x, 0.0, z),
            PI,
            IdlePose::Standing,
        );
    }
}

// ---- floating Z's above sleepers

#[derive(Resource)]
struct SleepAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct ZEmitter(Timer);

#[derive(Component)]
struct ZSprite {
    age: f32,
    origin: Vec3,
}

const Z_LIFE: f32 = 2.4;

fn setup_sleep_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let img = images.add(crate::floors::z_image());
    commands.insert_resource(SleepAssets {
        mesh: meshes.add(Rectangle::new(0.35, 0.35)),
        material: materials.add(StandardMaterial {
            base_color_texture: Some(img),
            unlit: true,
            // Cut-out glyph: masked sprites skip the transparent sort.
            alpha_mode: AlphaMode::Mask(0.4),
            cull_mode: None,
            ..default()
        }),
    });
}

fn emit_zs(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<SleepAssets>,
    mut sleepers: Query<(Entity, &GlobalTransform, &IdlePose, Option<&mut ZEmitter>)>,
) {
    for (e, gt, pose, emitter) in &mut sleepers {
        if *pose != IdlePose::Sleeping {
            continue;
        }
        let Some(mut emitter) = emitter else {
            commands
                .entity(e)
                .insert(ZEmitter(Timer::from_seconds(0.9, TimerMode::Repeating)));
            continue;
        };
        if emitter.0.tick(time.delta()).just_finished() {
            let head = gt.translation() + Vec3::new(0.0, 1.4, 0.0) + gt.forward() * 0.45;
            commands.spawn((
                Mesh3d(assets.mesh.clone()),
                MeshMaterial3d(assets.material.clone()),
                Transform::from_translation(head),
                ZSprite {
                    age: 0.0,
                    origin: head,
                },
                bevy::light::NotShadowCaster,
            ));
        }
    }
}

fn animate_zs(
    mut commands: Commands,
    time: Res<Time>,
    cam: Query<&GlobalTransform, With<crate::camera::OrbitCamera>>,
    mut zs: Query<(Entity, &mut Transform, &mut ZSprite)>,
) {
    let Some(cam) = cam.iter().next() else {
        return;
    };
    for (e, mut tf, mut z) in &mut zs {
        z.age += time.delta_secs();
        if z.age >= Z_LIFE {
            commands.entity(e).despawn();
            continue;
        }
        let k = z.age / Z_LIFE;
        tf.translation = z.origin + Vec3::new(0.15 * (k * 6.0).sin(), k * 0.9, 0.0);
        tf.rotation = cam.rotation();
        tf.scale = Vec3::splat(0.6 + 0.8 * k * (1.0 - k) * 2.0);
    }
}

// The halo is a separate mesh, so the death animation would leave it floating.
fn hide_dead_halos(units: Query<&Unit>, mut halos: Query<(&ChildOf, &mut Visibility), With<Halo>>) {
    for (child_of, mut vis) in &mut halos {
        let alive = units.get(child_of.parent()).is_ok_and(|u| u.alive);
        vis.set_if_neq(if alive {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}
