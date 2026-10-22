//! Client-side world: the campus, the tower, the data center and the lab (zones). `layout` builds each floor as
//! pure data (pieces, blockers, anchors); this module turns a layout into
//! entities and keeps the shared `Colliders` list in sync. The game flow
//! (`game.rs`) decides which floor to load; `WOW_LINEUP=1|2` shows the whole
//! cast side by side on a bare stage instead.

mod campus;
pub mod carpark;
mod class_spots;
mod client;
pub mod corporate;
mod datacenter;
mod districts;
mod dressing;
#[cfg(test)]
mod export;
mod frontier;
mod kit;
mod lab;
mod layout;
mod models;
mod office;
mod offsite;
mod onboarding;
mod props;
mod quest_spots;
mod shape;
mod teambuilding;
#[cfg(test)]
mod testgrid;
mod textures;
mod themes;
mod vibecon;
mod water;
mod zones;

pub use districts::{DISTRICTS, district_at};
use layout::build_floor;
pub use layout::{FloorLayout, elevator_exit_trigger};
pub use textures::{icon_image, z_image};
pub use zones::{build_zone, door_trigger};

/// Piece types the map draws roads and water from.
pub mod kit_for_map {
    pub use super::kit::{Mat, Piece, PrimKind};
}

/// The desk kit the Reorg moves around at run time (`minigame.rs`).
pub mod furniture {
    pub use super::kit::{DESK, KEYBOARD, SCREEN};
}

/// A kit model placed the way layout props are (footprint centred on `pos`, its front +Z
/// turned by `yaw`), for furniture that moves at run time. Returns the root entity.
pub fn spawn_model(
    commands: &mut Commands,
    assets: &AssetServer,
    model: &'static kit::Model,
    pos: Vec3,
    yaw: f32,
) -> Entity {
    let s = FURNITURE_SCALE;
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model.path()));
    let offset = Vec3::new(
        -(model.lo[0] + model.size[0] / 2.0),
        0.0,
        -(model.lo[1] + model.size[2] / 2.0),
    ) * s;
    commands
        .spawn((
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                WorldAssetRoot(scene),
                Transform::from_translation(offset).with_scale(Vec3::splat(s)),
            ));
        })
        .id()
}

/// Footprint of a kit model at furniture scale (width, height, depth).
pub fn model_size(model: &kit::Model) -> Vec3 {
    Vec3::from(model.size) * FURNITURE_SCALE
}

use crate::arena::{Colliders, FlatDecor, Ground};
use crate::config::WALL_HEIGHT;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::VisibilityRange;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use kit::{FURNITURE_SCALE, Mat, Piece, PrimKind, Tex};
use shape::Seg;
use std::collections::HashMap;

/// Ask the floors module to swap to tower floor `id` (1..=5) on the next frame.
#[derive(Message)]
pub struct LoadFloor(pub u8);

/// Ask the floors module to swap to any zone of the contract ("campus", "lab/2", ...).
#[derive(Message)]
pub struct LoadZone(pub String);

/// Only the floor, ceiling and lights: used by the cast lineup.
#[derive(Resource)]
pub struct BareStage;

#[derive(Component)]
struct FloorEntity;

#[derive(Component)]
struct Sun;

/// Decor further than this from the camera is not drawn outdoors (draw-call budget);
/// the fog hides the cut. Indoors the walls already hide most of it, so the range is shorter.
const DECOR_RANGE_OUTDOOR: f32 = 95.0;
const DECOR_RANGE_INDOOR: f32 = 70.0;
/// Pieces whose longest side is below this are decor, the rest are structure.
const DECOR_MAX_SIDE: f32 = 7.0;
/// Pieces lower than this cast no shadow: they would only add draw calls to the shadow pass.
const SHADOW_MIN_HEIGHT: f32 = 1.3;

pub struct FloorsPlugin;

impl Plugin for FloorsPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(which) = std::env::var("WOW_LINEUP") {
            // The big rigs of lineup 3 need open sky above them; the others fit under the tower ceiling.
            let outdoors = which == "3";
            app.insert_resource(BareStage).add_systems(
                Startup,
                move |mut floor: MessageWriter<LoadFloor>, mut zone: MessageWriter<LoadZone>| {
                    if outdoors {
                        zone.write(LoadZone("campus".into()));
                    } else {
                        floor.write(LoadFloor(1));
                    }
                },
            );
        }
        app.add_message::<LoadFloor>()
            .add_message::<LoadZone>()
            .init_resource::<BlinkMats>()
            .add_systems(Startup, spawn_sun)
            // Same frame as the sim floor swap, so movement never slides against old blockers.
            .add_systems(Update, load_floor.after(crate::game::FloorSwap))
            .add_systems(Update, (blink_leds, water::animate, cap_point_lights));
    }
}

fn spawn_sun(mut commands: Commands) {
    // One shadow-casting light only: GPU budget.
    commands.spawn((
        Sun,
        DirectionalLight {
            illuminance: 8000.0,
            color: Color::srgb(1.0, 0.95, 0.88),
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 1, // one cascade: cheapest, and phones are a target
            maximum_distance: 60.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(8.0, 20.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn get_mat(
    cache: &mut HashMap<Mat, Handle<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
    theme: &layout::Theme,
    mat: Mat,
) -> Handle<StandardMaterial> {
    cache
        .entry(mat)
        .or_insert_with(|| materials.add(textures::material(mat, theme)))
        .clone()
}

/// Scene-wide look settings a zone load rewrites.
#[derive(bevy::ecs::system::SystemParam)]
struct Look<'w, 's> {
    ambient: ResMut<'w, GlobalAmbientLight>,
    clear: ResMut<'w, ClearColor>,
    ceiling: ResMut<'w, crate::arena::CameraCeiling>,
    fogs: Query<'w, 's, &'static mut DistanceFog>,
    sun: Query<'w, 's, &'static mut DirectionalLight, With<Sun>>,
}

/// The zone's shared office-window material, if its layout has windows.
#[derive(Resource, Default)]
pub struct WindowMats(pub Option<Handle<StandardMaterial>>);

/// The shared materials of the blinking LEDs with their resting emissive colour and pattern.
#[derive(Resource, Default)]
struct BlinkMats(Vec<(Handle<StandardMaterial>, LinearRgba, Mat)>);

/// Square-ish blink: each pattern has its own period and duty, so racks never pulse in unison.
fn blink_level(pattern: Mat, t: f32) -> f32 {
    let (period, duty, phase) = match pattern {
        Mat::BlinkGreenA => (1.3, 0.7, 0.0),
        Mat::BlinkGreenB => (0.7, 0.5, 0.3),
        Mat::BlinkRed => (2.1, 0.35, 0.9),
        Mat::BlinkBlue => (1.7, 0.6, 0.5),
        _ => (0.9, 0.45, 0.1),
    };
    let x = (t / period + phase).fract();
    if x < duty { 1.0 } else { 0.08 }
}

fn blink_leds(
    time: Res<Time>,
    blink: Res<BlinkMats>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    for (handle, base, pattern) in &blink.0 {
        let k = blink_level(*pattern, t);
        if let Some(mut m) = materials.get_mut(handle) {
            let e = *base * k;
            if m.emissive != e {
                m.emissive = e;
            }
        }
    }
}

/// Only the lights nearest the camera stay on: every extra clustered light costs each
/// lit fragment, and each costs shader time.
#[allow(clippy::type_complexity)] // Bevy query tuple
fn cap_point_lights(
    time: Res<Time>,
    settings: Res<crate::settings::Settings>,
    cam: Query<&GlobalTransform, With<crate::camera::OrbitCamera>>,
    mut lights: Query<(&GlobalTransform, &mut Visibility), (With<PointLight>, With<FloorEntity>)>,
    mut next: Local<f32>,
) {
    // Light positions are static, so a few updates per second are enough.
    *next -= time.delta_secs();
    if *next > 0.0 {
        return;
    }
    *next = 0.25;
    let Some(cam) = cam.iter().next() else { return };
    let max = settings.quality.max_lights();
    if lights.iter().len() <= max {
        for (_, mut v) in &mut lights {
            v.set_if_neq(Visibility::Inherited);
        }
        return;
    }
    let eye = cam.translation();
    let mut dist: Vec<f32> = lights
        .iter()
        .map(|(t, _)| t.translation().distance_squared(eye))
        .collect();
    dist.sort_by(f32::total_cmp);
    let cutoff = dist[max - 1];
    for (t, mut v) in &mut lights {
        let on = t.translation().distance_squared(eye) <= cutoff;
        v.set_if_neq(if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

fn is_blink(mat: Mat) -> bool {
    matches!(
        mat,
        Mat::BlinkGreenA | Mat::BlinkGreenB | Mat::BlinkRed | Mat::BlinkBlue | Mat::BlinkAmber
    )
}

/// Longest horizontal or vertical side.
fn max_side(v: Vec3) -> f32 {
    v.x.max(v.y).max(v.z)
}

#[allow(clippy::too_many_arguments)] // Bevy system params
fn load_floor(
    mut commands: Commands,
    mut floor_requests: MessageReader<LoadFloor>,
    mut zone_requests: MessageReader<LoadZone>,
    old: Query<Entity, With<FloorEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    mut colliders: ResMut<Colliders>,
    mut decor: ResMut<FlatDecor>,
    mut ground: ResMut<Ground>,
    mut look: Look,
    mut blink: ResMut<BlinkMats>,
    bare: Option<Res<BareStage>>,
) {
    let floor_req = floor_requests.read().last().map(|r| r.0);
    let zone_req = zone_requests.read().last().map(|r| r.0.clone());
    let layout = match (zone_req, floor_req) {
        (Some(id), _) => match build_zone(&id) {
            Some(l) => l,
            None => {
                error!("unknown zone {id:?}");
                return;
            }
        },
        (None, Some(id)) => build_floor(id),
        (None, None) => return,
    };
    for e in &old {
        commands.entity(e).despawn();
    }
    let bare = bare.is_some();

    let t = &layout.theme;
    look.ambient.color = t.ambient;
    look.ambient.brightness = t.ambient_brightness;
    look.clear.0 = t.fog;
    for mut fog in &mut look.fogs {
        fog.color = t.fog;
        fog.falloff = bevy::pbr::FogFalloff::Linear {
            start: t.fog_range.0,
            end: t.fog_range.1,
        };
    }
    for mut light in &mut look.sun {
        light.color = t.sun;
        light.illuminance = t.sun_illuminance;
    }
    look.ceiling.0 = if layout.outdoor {
        60.0
    } else {
        layout.ceiling - (WALL_HEIGHT - crate::config::CAM_MAX_HEIGHT)
    };
    colliders.0 = if bare {
        Vec::new()
    } else {
        layout.blockers.clone()
    };
    decor.0 = layout::flat_decor(&layout.pieces);
    ground.0 = layout.terrain.clone();

    let runs = layout.footprint.floor_runs();
    let floor = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(textures::floor_image(t.floor))),
        uv_transform: Affine2::from_scale(Vec2::splat(0.25)),
        perceptual_roughness: match t.floor {
            layout::FloorKind::Marble | layout::FloorKind::Gloss(_) => 0.3,
            layout::FloorKind::Tile(_) => 0.5,
            _ => 0.95,
        },
        ..default()
    });
    // Own meshes (not the shared unit cube) so the floor and ceiling UVs tile in metres.
    let floor_mesh = if layout.terrain.is_flat() {
        flat_mesh(&runs, 0.0, true)
    } else {
        terrain_mesh(&runs, &layout.terrain)
    };
    commands.spawn((
        FloorEntity,
        Mesh3d(meshes.add(floor_mesh)),
        MeshMaterial3d(floor),
    ));
    // Ceiling is single-sided (culled from above, so the overview camera sees through it).
    // It casts no shadow (it would black out the directional light) and is not a collider:
    // camera.rs clamps height instead. Outdoors there is none: that is the sky.
    if !layout.outdoor {
        let ceiling = materials.add(StandardMaterial {
            base_color_texture: Some(images.add(textures::ceiling_image())),
            // 4 tiles per repeat: ~0.6 m panels.
            uv_transform: Affine2::from_scale(Vec2::splat(0.4)),
            perceptual_roughness: 0.95,
            // The slab is only lit from below by point lights; a faint floor keeps it readable.
            emissive: LinearRgba::rgb(0.05, 0.05, 0.06),
            ..default()
        });
        commands.spawn((
            FloorEntity,
            Mesh3d(meshes.add(flat_mesh(&runs, layout.ceiling, false))),
            MeshMaterial3d(ceiling),
            NotShadowCaster,
        ));
    }

    let unit_cube = meshes.add(Cuboid::from_length(1.0));
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    let cylinder = meshes.add(Cylinder::new(0.5, 1.0));
    let sphere = meshes.add(Sphere::new(0.5).mesh().uv(24, 12));
    let cone = meshes.add(Cone {
        radius: 0.5,
        height: 1.0,
    });
    let pennant = meshes.add(pennant_mesh());
    let mut mats: HashMap<Mat, Handle<StandardMaterial>> = HashMap::new();
    let mut texs: HashMap<Tex, Handle<StandardMaterial>> = HashMap::new();
    // Created on the first water piece: most zones have none.
    let mut water_mat: Option<Handle<StandardMaterial>> = None;
    // The QA overview camera sits far above everything: culling would hide all the decor.
    let overview = std::env::var_os("WOW_TOPDOWN").is_some();
    let decor_range = VisibilityRange::abrupt(
        0.0,
        match (overview, layout.outdoor) {
            (true, _) => f32::MAX,
            (_, true) => DECOR_RANGE_OUTDOOR,
            (_, false) => DECOR_RANGE_INDOOR,
        },
    );
    // Small, low or see-through things: no shadow, culled with distance when small.
    let finish = |e: &mut EntityCommands, size: Vec3, mat: Option<Mat>| {
        if mat.is_some_and(|m| {
            matches!(
                m,
                Mat::Glass | Mat::Frosted | Mat::LightPanel | Mat::Facade | Mat::Window
            )
        }) || size.y < SHADOW_MIN_HEIGHT && layout.outdoor
        {
            e.insert(NotShadowCaster);
        }
        if max_side(size) < DECOR_MAX_SIDE {
            e.insert(decor_range.clone());
        }
    };

    for piece in &layout.pieces {
        if bare && !matches!(piece, Piece::Light { .. }) {
            continue;
        }
        match piece {
            Piece::Box {
                center, size, mat, ..
            } => {
                if *mat == Mat::Lake {
                    let m = water_mat
                        .get_or_insert_with(|| materials.add(water::material(&mut images)))
                        .clone();
                    let top = *center + Vec3::Y * (size.y / 2.0);
                    let rect = water::Shape::Rect(Vec2::new(size.x, size.z));
                    water::spawn(&mut commands, &mut meshes, m, top, rect, FloorEntity);
                    continue;
                }
                let mut e = commands.spawn((
                    FloorEntity,
                    Mesh3d(unit_cube.clone()),
                    MeshMaterial3d(get_mat(&mut mats, &mut materials, t, *mat)),
                    Transform::from_translation(*center).with_scale(*size),
                ));
                finish(&mut e, *size, Some(*mat));
            }
            Piece::Prop {
                model,
                pos,
                yaw,
                scale,
                ..
            } => {
                let s = FURNITURE_SCALE * scale;
                let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model.path()));
                let offset = Vec3::new(
                    -(model.lo[0] + model.size[0] / 2.0),
                    0.0,
                    -(model.lo[1] + model.size[2] / 2.0),
                ) * s;
                let mut e = commands.spawn((
                    FloorEntity,
                    Transform::from_translation(*pos).with_rotation(Quat::from_rotation_y(*yaw)),
                    Visibility::default(),
                ));
                finish(&mut e, Vec3::from(model.size) * s, None);
                e.with_children(|c| {
                    c.spawn((
                        WorldAssetRoot(scene),
                        Transform::from_translation(offset).with_scale(Vec3::splat(s)),
                    ));
                });
            }
            Piece::Panel {
                center,
                yaw,
                size,
                tex,
            } => {
                let handle = texs
                    .entry(*tex)
                    .or_insert_with(|| {
                        let img = images.add(textures::tex_image(*tex));
                        materials.add(textures::tex_material(*tex, img))
                    })
                    .clone();
                let mut e = commands.spawn((
                    FloorEntity,
                    Mesh3d(quad.clone()),
                    MeshMaterial3d(handle),
                    Transform::from_translation(*center)
                        .with_rotation(Quat::from_rotation_y(*yaw))
                        .with_scale(size.extend(1.0)),
                    NotShadowCaster,
                ));
                if size.x.max(size.y) < DECOR_MAX_SIDE {
                    e.insert(decor_range.clone());
                }
            }
            Piece::Light {
                pos,
                color,
                intensity,
            } => {
                commands.spawn((
                    FloorEntity,
                    PointLight {
                        color: *color,
                        intensity: *intensity,
                        range: 22.0,
                        ..default()
                    },
                    Transform::from_translation(*pos),
                ));
            }
            Piece::Slab {
                center,
                yaw,
                size,
                mat,
                pitch,
            } => {
                let mut e = commands.spawn((
                    FloorEntity,
                    Mesh3d(unit_cube.clone()),
                    MeshMaterial3d(get_mat(&mut mats, &mut materials, t, *mat)),
                    Transform::from_translation(*center)
                        .with_rotation(Quat::from_rotation_y(*yaw) * Quat::from_rotation_z(*pitch))
                        .with_scale(*size),
                ));
                finish(&mut e, *size, Some(*mat));
            }
            Piece::Prim {
                kind,
                center,
                size,
                yaw,
                mat,
            } => {
                if *mat == Mat::Lake && matches!(kind, PrimKind::Cylinder) {
                    let m = water_mat
                        .get_or_insert_with(|| materials.add(water::material(&mut images)))
                        .clone();
                    let top = *center + Vec3::Y * (size.y / 2.0);
                    let disc = water::Shape::Disc(size.x / 2.0);
                    water::spawn(&mut commands, &mut meshes, m, top, disc, FloorEntity);
                    continue;
                }
                let mesh = match kind {
                    PrimKind::Cylinder => &cylinder,
                    PrimKind::Sphere => &sphere,
                    PrimKind::Cone => &cone,
                    PrimKind::Pennant => &pennant,
                };
                let mut e = commands.spawn((
                    FloorEntity,
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(get_mat(&mut mats, &mut materials, t, *mat)),
                    Transform::from_translation(*center)
                        .with_rotation(Quat::from_rotation_y(*yaw))
                        .with_scale(*size),
                ));
                finish(&mut e, *size, Some(*mat));
            }
            Piece::Run {
                segs,
                mat,
                y0,
                y1,
                inset,
            } => {
                let mut e = commands.spawn((
                    FloorEntity,
                    Mesh3d(meshes.add(strip_mesh(segs, *y0, *y1, *inset))),
                    MeshMaterial3d(get_mat(&mut mats, &mut materials, t, *mat)),
                ));
                if matches!(mat, Mat::Glass | Mat::Frosted | Mat::Accent) {
                    e.insert(NotShadowCaster);
                }
            }
            Piece::Cap { segs, y } => {
                commands.spawn((
                    FloorEntity,
                    Mesh3d(meshes.add(cap_mesh(segs, *y))),
                    MeshMaterial3d(get_mat(&mut mats, &mut materials, t, Mat::Trim)),
                    NotShadowCaster,
                ));
            }
            Piece::Block(_) => {}
        }
    }
    // Office windows light up at night (`live::daynight`); one shared material per zone.
    commands.insert_resource(WindowMats(mats.get(&Mat::Window).cloned()));
    blink.0 = mats
        .iter()
        .filter(|(m, _)| is_blink(**m))
        .filter_map(|(m, h)| materials.get(h).map(|mat| (h.clone(), mat.emissive, *m)))
        .collect();
}

fn mesh_from(pos: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, uv: Vec<[f32; 2]>, idx: Vec<u32>) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

/// Unit triangle hanging point down, drawn on both faces (materials cull back faces).
fn pennant_mesh() -> Mesh {
    let tri = [[-0.5, 0.5], [0.0, -0.5], [0.5, 0.5]];
    let (mut pos, mut normals, mut uv, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (side, n) in [(0.0_f32, 1.0_f32), (3.0, -1.0)] {
        for [x, y] in tri {
            pos.push([x, y, 0.0]);
            normals.push([0.0, 0.0, n]);
            uv.push([x + 0.5, 0.5 - y]);
        }
        let b = side as u32;
        idx.extend(if n > 0.0 {
            [b, b + 1, b + 2]
        } else {
            [b, b + 2, b + 1]
        });
    }
    mesh_from(pos, normals, uv, idx)
}

/// Horizontal quads at height `y`; UVs are world metres.
fn flat_mesh(runs: &[(f32, f32, f32, f32)], y: f32, up: bool) -> Mesh {
    let (mut pos, mut uv, mut idx) = (Vec::new(), Vec::new(), Vec::new());
    for &(x0, x1, z0, z1) in runs {
        let b = pos.len() as u32;
        for (x, z) in [(x0, z0), (x0, z1), (x1, z1), (x1, z0)] {
            pos.push([x, y, z]);
            uv.push([x, z]);
        }
        let quad = if up {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        idx.extend(quad.map(|i| b + i));
    }
    let n = if up {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, -1.0, 0.0]
    };
    let normals = vec![n; pos.len()];
    mesh_from(pos, normals, uv, idx)
}

/// Grid size of the ground mesh: fine enough for the river banks and plaza ramps, coarse
/// enough to stay one cheap draw call on weak GPUs.
const TERRAIN_CELL_M: f32 = 3.0;

/// The ground over the bounds of the floor runs as one height-mapped grid (UVs in metres, like
/// `flat_mesh`). It follows `Terrain::surface`, so river beds stay open under the bridges.
fn terrain_mesh(runs: &[(f32, f32, f32, f32)], t: &combat_core::Terrain) -> Mesh {
    let (mut x0, mut x1, mut z0, mut z1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for &(a, b, c, d) in runs {
        (x0, x1, z0, z1) = (x0.min(a), x1.max(b), z0.min(c), z1.max(d));
    }
    if x0 > x1 {
        return flat_mesh(runs, 0.0, true);
    }
    let nx = ((x1 - x0) / TERRAIN_CELL_M).ceil().max(1.0) as u32;
    let nz = ((z1 - z0) / TERRAIN_CELL_M).ceil().max(1.0) as u32;
    let (sx, sz) = ((x1 - x0) / nx as f32, (z1 - z0) / nz as f32);
    let (mut pos, mut normals, mut uv) = (Vec::new(), Vec::new(), Vec::new());
    for j in 0..=nz {
        for i in 0..=nx {
            let (x, z) = (x0 + i as f32 * sx, z0 + j as f32 * sz);
            pos.push([x, t.surface(x, z), z]);
            let e = 0.75;
            let n = Vec3::new(
                t.surface(x - e, z) - t.surface(x + e, z),
                2.0 * e,
                t.surface(x, z - e) - t.surface(x, z + e),
            )
            .normalize_or(Vec3::Y);
            normals.push(n.to_array());
            uv.push([x, z]);
        }
    }
    let row = nx + 1;
    let mut idx = Vec::with_capacity((nx * nz * 6) as usize);
    for j in 0..nz {
        for i in 0..nx {
            let a = j * row + i;
            let (b, c, d) = (a + row, a + row + 1, a + 1);
            idx.extend([a, b, c, a, c, d]);
        }
    }
    mesh_from(pos, normals, uv, idx)
}

/// Vertical band along wall segments, facing the walkable side.
fn strip_mesh(segs: &[Seg], y0: f32, y1: f32, inset: f32) -> Mesh {
    let (mut pos, mut normals, mut uv, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for s in segs {
        let (a, b) = (s.a + s.n * inset, s.b + s.n * inset);
        let b0 = pos.len() as u32;
        for (p, y) in [(a, y0), (b, y0), (b, y1), (a, y1)] {
            pos.push([p.x, y, p.y]);
            normals.push([s.n.x, 0.0, s.n.y]);
            uv.push([p.x, y]);
        }
        idx.extend([0, 1, 2, 0, 2, 3].map(|i| b0 + i));
    }
    mesh_from(pos, normals, uv, idx)
}

fn cap_mesh(segs: &[Seg], y: f32) -> Mesh {
    let (mut pos, mut uv, mut idx) = (Vec::new(), Vec::new(), Vec::new());
    for s in segs {
        let out = s.n * -shape::WALL_T;
        let b0 = pos.len() as u32;
        for p in [s.a, s.b, s.b + out, s.a + out] {
            pos.push([p.x, y + 0.02, p.y]);
            uv.push([p.x, p.y]);
        }
        idx.extend([0, 1, 2, 0, 2, 3].map(|i| b0 + i));
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    mesh_from(pos, normals, uv, idx)
}
