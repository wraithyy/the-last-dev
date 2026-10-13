//! Client-side world for the five-floor tower. `layout` builds each floor as
//! pure data (pieces, blockers, anchors); this module turns a layout into
//! entities and keeps the shared `Colliders` list in sync. The game flow
//! (`game.rs`) decides which floor to load; `WOW_LINEUP=1|2` shows the whole
//! cast side by side on a bare stage instead.

mod kit;
mod layout;
mod props;
mod shape;
mod textures;
mod themes;

pub use layout::{FLOOR_COUNT, build_floor, elevator_exit_trigger};
pub use textures::{icon_image, z_image};

use crate::arena::Colliders;
use crate::config::WALL_HEIGHT;
use bevy::asset::RenderAssetUsages;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use kit::{FURNITURE_SCALE, Mat, Piece, Tex};
use shape::Seg;
use std::collections::HashMap;

/// Ask the floors module to swap to floor `id` (1..=5) on the next frame.
#[derive(Message)]
pub struct LoadFloor(pub u8);

/// Only the floor, ceiling and lights: used by the cast lineup.
#[derive(Resource)]
pub struct BareStage;

#[derive(Component)]
struct FloorEntity;

#[derive(Component)]
struct Sun;

pub struct FloorsPlugin;

impl Plugin for FloorsPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("WOW_LINEUP").is_ok() {
            app.insert_resource(BareStage).add_systems(
                Startup,
                |mut w: MessageWriter<LoadFloor>| {
                    w.write(LoadFloor(1));
                },
            );
        }
        app.add_message::<LoadFloor>()
            .add_systems(Startup, spawn_sun)
            // Same frame as the sim floor swap, so movement never slides against old blockers.
            .add_systems(Update, load_floor.after(crate::game::FloorSwap));
    }
}

fn spawn_sun(mut commands: Commands) {
    // One shadow-casting light only: WebGL2 budget.
    commands.spawn((
        Sun,
        DirectionalLight {
            illuminance: 8000.0,
            color: Color::srgb(1.0, 0.95, 0.88),
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 1, // WebGL2 supports a single cascade
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

#[allow(clippy::too_many_arguments)] // Bevy system params
fn load_floor(
    mut commands: Commands,
    mut requests: MessageReader<LoadFloor>,
    old: Query<Entity, With<FloorEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    mut colliders: ResMut<Colliders>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut fogs: Query<&mut DistanceFog>,
    mut sun: Query<&mut DirectionalLight, With<Sun>>,
    bare: Option<Res<BareStage>>,
) {
    let Some(LoadFloor(id)) = requests.read().last() else {
        return;
    };
    let layout = build_floor(*id);
    for e in &old {
        commands.entity(e).despawn();
    }

    let t = &layout.theme;
    ambient.color = t.ambient;
    ambient.brightness = t.ambient_brightness;
    clear.0 = t.fog;
    for mut fog in &mut fogs {
        fog.color = t.fog;
    }
    for mut light in &mut sun {
        light.color = t.sun;
        light.illuminance = t.sun_illuminance;
    }
    colliders.0 = if bare.is_some() {
        Vec::new()
    } else {
        layout.blockers.clone()
    };

    let runs = layout.footprint.floor_runs();
    let floor = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(textures::floor_image(t.floor))),
        uv_transform: Affine2::from_scale(Vec2::splat(0.25)),
        perceptual_roughness: if matches!(t.floor, layout::FloorKind::Marble) {
            0.3
        } else {
            0.95
        },
        ..default()
    });
    // Own meshes (not the shared unit cube) so the floor and ceiling UVs tile in metres.
    commands.spawn((
        FloorEntity,
        Mesh3d(meshes.add(flat_mesh(&runs, 0.0, true))),
        MeshMaterial3d(floor),
    ));
    // Ceiling is single-sided (culled from above, so the overview camera sees through it).
    // It casts no shadow (it would black out the directional light) and is not a collider:
    // camera.rs clamps height instead.
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
        Mesh3d(meshes.add(flat_mesh(&runs, WALL_HEIGHT, false))),
        MeshMaterial3d(ceiling),
        NotShadowCaster,
    ));

    let unit_cube = meshes.add(Cuboid::from_length(1.0));
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    let mut mats: HashMap<Mat, Handle<StandardMaterial>> = HashMap::new();
    let mut texs: HashMap<Tex, Handle<StandardMaterial>> = HashMap::new();

    for piece in &layout.pieces {
        if bare.is_some() && !matches!(piece, Piece::Light { .. }) {
            continue;
        }
        match piece {
            Piece::Box {
                center, size, mat, ..
            } => {
                let mut e = commands.spawn((
                    FloorEntity,
                    Mesh3d(unit_cube.clone()),
                    MeshMaterial3d(get_mat(&mut mats, &mut materials, t, *mat)),
                    Transform::from_translation(*center).with_scale(*size),
                ));
                if matches!(mat, Mat::Glass | Mat::LightPanel) {
                    e.insert(NotShadowCaster);
                }
            }
            Piece::Prop {
                model,
                pos,
                yaw,
                scale,
                ..
            } => {
                let s = FURNITURE_SCALE * scale;
                let scene = assets.load(
                    GltfAssetLabel::Scene(0)
                        .from_asset(format!("models/furniture/{}.glb", model.file)),
                );
                let offset = Vec3::new(
                    -(model.lo[0] + model.size[0] / 2.0),
                    0.0,
                    -(model.lo[1] + model.size[2] / 2.0),
                ) * s;
                commands
                    .spawn((
                        FloorEntity,
                        Transform::from_translation(*pos)
                            .with_rotation(Quat::from_rotation_y(*yaw)),
                        Visibility::default(),
                    ))
                    .with_children(|c| {
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
                commands.spawn((
                    FloorEntity,
                    Mesh3d(quad.clone()),
                    MeshMaterial3d(handle),
                    Transform::from_translation(*center)
                        .with_rotation(Quat::from_rotation_y(*yaw))
                        .with_scale(size.extend(1.0)),
                    NotShadowCaster,
                ));
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
            } => {
                let mut e = commands.spawn((
                    FloorEntity,
                    Mesh3d(unit_cube.clone()),
                    MeshMaterial3d(get_mat(&mut mats, &mut materials, t, *mat)),
                    Transform::from_translation(*center)
                        .with_rotation(Quat::from_rotation_y(*yaw))
                        .with_scale(*size),
                ));
                if matches!(mat, Mat::Glass | Mat::LightPanel) {
                    e.insert(NotShadowCaster);
                }
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
                if matches!(mat, Mat::Glass | Mat::Accent) {
                    e.insert(NotShadowCaster);
                }
            }
            Piece::Cap { segs } => {
                commands.spawn((
                    FloorEntity,
                    Mesh3d(meshes.add(cap_mesh(segs))),
                    MeshMaterial3d(get_mat(&mut mats, &mut materials, t, Mat::Trim)),
                    NotShadowCaster,
                ));
            }
            Piece::Block(_) => {}
        }
    }
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

fn cap_mesh(segs: &[Seg]) -> Mesh {
    let (mut pos, mut uv, mut idx) = (Vec::new(), Vec::new(), Vec::new());
    for s in segs {
        let out = s.n * -shape::WALL_T;
        let b0 = pos.len() as u32;
        for p in [s.a, s.b, s.b + out, s.a + out] {
            pos.push([p.x, WALL_HEIGHT + 0.02, p.y]);
            uv.push([p.x, p.y]);
        }
        idx.extend([0, 1, 2, 0, 2, 3].map(|i| b0 + i));
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    mesh_from(pos, normals, uv, idx)
}
