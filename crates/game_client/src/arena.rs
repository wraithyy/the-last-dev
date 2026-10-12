//! The office arena: primitives for structure, KayKit furniture for dressing.
//! Every solid prop registers an AABB so collision, camera clipping and line
//! of sight share one source.

use crate::collision::aabb;
use crate::config::{ARENA_HALF, WALL_HEIGHT, WALL_THICKNESS};
use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::math::Affine2;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use combat_core::Aabb;
use std::f32::consts::FRAC_PI_2;

#[derive(Component)]
pub struct Collider(pub Aabb);

/// Flat list of every static blocker, for combat_core line-of-sight queries.
#[derive(Resource, Default)]
pub struct Colliders(pub Vec<Aabb>);

const BACKDROP: Color = Color::srgb(0.03, 0.035, 0.06);

pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.7, 0.78, 1.0),
            brightness: 450.0,
            ..default()
        })
        .insert_resource(ClearColor(BACKDROP))
        .add_systems(Startup, build_arena)
        .add_systems(PostStartup, collect_colliders)
        .add_systems(Update, grade_camera);
    }
}

/// The camera is spawned by camera.rs; the look (tonemap, bloom, fog) is applied here.
fn grade_camera(mut commands: Commands, cams: Query<Entity, Added<Camera3d>>) {
    for cam in &cams {
        commands.entity(cam).insert((
            // AcesFitted needs no LUT texture, so it behaves the same on WebGL2.
            Tonemapping::AcesFitted,
            Bloom {
                intensity: 0.12,
                ..Bloom::NATURAL
            },
            DistanceFog {
                color: BACKDROP,
                falloff: FogFalloff::Linear {
                    start: 25.0,
                    end: 75.0,
                },
                ..default()
            },
        ));
    }
}

struct Palette {
    wall: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    accent: Handle<StandardMaterial>,
    fabric: [Handle<StandardMaterial>; 3],
    dark: Handle<StandardMaterial>,
    screens: [Handle<StandardMaterial>; 3],
    runner: Handle<StandardMaterial>,
    runner_edge: Handle<StandardMaterial>,
    cable: Handle<StandardMaterial>,
    rack: Handle<StandardMaterial>,
    rack_panel: Handle<StandardMaterial>,
    led_green: Handle<StandardMaterial>,
    led_red: Handle<StandardMaterial>,
    led_blue: Handle<StandardMaterial>,
}

fn matte(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        ..default()
    }
}

fn glow(color: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::BLACK,
        emissive: color,
        ..default()
    }
}

/// Carpet tile texture: 2x2 tiles in two shades with grout lines and a little noise.
/// Repeats via the sampler, so one small image covers the whole floor.
fn carpet_texture() -> Image {
    const N: u32 = 32;
    const TILE: u32 = N / 2;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let shade = if (x / TILE + y / TILE).is_multiple_of(2) {
                0.2
            } else {
                0.25
            };
            let grout = if x % TILE == 0 || y % TILE == 0 {
                0.8
            } else {
                1.0
            };
            let noise = 0.96 + 0.08 * ((x * 7 + y * 13 + x * y * 3) % 17) as f32 / 16.0;
            let v = (shade * grout * noise * 255.0).min(255.0) as u8;
            data.extend_from_slice(&[v, v, (v as f32 * 1.12).min(255.0) as u8, 255]);
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    img
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    unit_cube: Handle<Mesh>,
    assets: &'a AssetServer,
}

impl Builder<'_, '_, '_> {
    /// Cuboids share one unit mesh scaled by the transform, so they batch together.
    fn boxed(
        &mut self,
        center: Vec3,
        size: Vec3,
        material: &Handle<StandardMaterial>,
        solid: bool,
    ) {
        let mut e = self.commands.spawn((
            Mesh3d(self.unit_cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(center).with_scale(size),
        ));
        if solid {
            e.insert(Collider(aabb(center, size)));
        }
    }

    /// KayKit furniture piece. `turns` is quarter turns about Y (the model's front is +Z).
    /// `footprint` is the unrotated world-space collider size, or None for a pure decoration.
    fn prop(&mut self, name: &str, pos: Vec3, turns: i32, scale: Vec3, footprint: Option<Vec3>) {
        let scene = self
            .assets
            .load(GltfAssetLabel::Scene(0).from_asset(format!("models/furniture/{name}.gltf")));
        let mut e = self.commands.spawn((
            WorldAssetRoot(scene),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(turns as f32 * FRAC_PI_2))
                .with_scale(scale),
        ));
        if let Some(size) = footprint {
            let size = if turns.rem_euclid(2) == 1 {
                Vec3::new(size.z, size.y, size.x)
            } else {
                size
            };
            e.insert(Collider(aabb(pos + Vec3::Y * size.y / 2.0, size)));
        }
    }

    /// Open towards +Z: back and side partitions, desk, glowing monitor, chair.
    fn cubicle(&mut self, p: &Palette, i: usize, x: f32, z: f32) {
        const SIZE: f32 = 3.0;
        const PART_H: f32 = 1.6;
        const PART_T: f32 = 0.1;
        let half = SIZE / 2.0;
        let fabric = &p.fabric[i % 3];
        let trim_h = 0.06;
        self.boxed(
            Vec3::new(x, PART_H / 2.0, z - half),
            Vec3::new(SIZE, PART_H, PART_T),
            fabric,
            true,
        );
        self.boxed(
            Vec3::new(x, PART_H + trim_h / 2.0, z - half),
            Vec3::new(SIZE, trim_h, PART_T + 0.04),
            &p.trim,
            false,
        );
        for side in [-1.0, 1.0] {
            self.boxed(
                Vec3::new(x + side * half, PART_H / 2.0, z),
                Vec3::new(PART_T, PART_H, SIZE),
                fabric,
                true,
            );
            self.boxed(
                Vec3::new(x + side * half, PART_H + trim_h / 2.0, z),
                Vec3::new(PART_T + 0.04, trim_h, SIZE),
                &p.trim,
                false,
            );
        }

        // Desk against the back partition.
        let desk_z = z - half + PART_T / 2.0 + 0.5;
        self.prop(
            "table_medium_long",
            Vec3::new(x, 0.0, desk_z),
            0,
            Vec3::new(0.75, 0.8, 0.5),
            Some(Vec3::new(2.25, 0.8, 1.0)),
        );
        let top = 0.8;
        let back = desk_z - 0.3;
        self.boxed(
            Vec3::new(x, top + 0.1, back),
            Vec3::new(0.12, 0.2, 0.1),
            &p.dark,
            false,
        );
        self.boxed(
            Vec3::new(x, top + 0.45, back),
            Vec3::new(0.78, 0.46, 0.04),
            &p.dark,
            false,
        );
        self.boxed(
            Vec3::new(x, top + 0.45, back + 0.025),
            Vec3::new(0.7, 0.38, 0.01),
            &p.screens[i % 3],
            false,
        );
        self.boxed(
            Vec3::new(x, top + 0.01, back + 0.4),
            Vec3::new(0.45, 0.02, 0.15),
            &p.dark,
            false,
        );
        self.prop(
            "lamp_table",
            Vec3::new(x + 0.9, top, desk_z - 0.2),
            0,
            Vec3::splat(0.45),
            None,
        );
        if i.is_multiple_of(2) {
            self.prop(
                "cactus_small_A",
                Vec3::new(x - 0.95, top, desk_z),
                0,
                Vec3::splat(0.5),
                None,
            );
        } else {
            self.prop(
                "book_set",
                Vec3::new(x - 0.9, top + 0.05, desk_z),
                0,
                Vec3::splat(0.35),
                None,
            );
        }
        // Chair faces the desk (-Z); a slight yaw per cubicle makes the room feel lived in.
        let chair = self
            .assets
            .load(GltfAssetLabel::Scene(0).from_asset("models/furniture/chair_A.gltf"));
        let yaw = std::f32::consts::PI + [0.35, -0.2, 0.1][i % 3];
        self.commands.spawn((
            WorldAssetRoot(chair),
            Transform::from_xyz(x + 0.1, 0.0, z + 0.15)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(0.75)),
        ));
    }

    fn server_rack(&mut self, p: &Palette, x: f32, z: f32) {
        self.boxed(
            Vec3::new(x, 1.1, z),
            Vec3::new(0.9, 2.2, 1.0),
            &p.rack,
            true,
        );
        let front = z + 0.5;
        for i in 0..8 {
            let y = 0.25 + i as f32 * 0.25;
            self.boxed(
                Vec3::new(x, y, front + 0.005),
                Vec3::new(0.78, 0.2, 0.01),
                &p.rack_panel,
                false,
            );
            for k in 0..3 {
                let led = match (i + k + x.abs() as usize) % 4 {
                    0 => &p.led_red,
                    1 | 2 => &p.led_green,
                    _ => &p.led_blue,
                };
                self.boxed(
                    Vec3::new(x + 0.22 + k as f32 * 0.1, y, front + 0.015),
                    Vec3::new(0.04, 0.04, 0.01),
                    led,
                    false,
                );
            }
        }
    }

    /// Couch on a rug with a coffee table and a lamp: a quiet corner away from the cubicles.
    fn lounge(&mut self, side: f32, plant: &str) {
        let x = side * 15.0;
        self.prop(
            "rug_rectangle_stripes_A",
            Vec3::new(x, 0.0, 16.0),
            0,
            Vec3::new(1.3, 1.0, 1.3),
            None,
        );
        self.prop(
            "couch",
            Vec3::new(x, 0.0, 18.9),
            2,
            Vec3::ONE,
            Some(Vec3::new(3.0, 1.0, 1.6)),
        );
        self.prop(
            "table_small",
            Vec3::new(x, 0.0, 16.3),
            0,
            Vec3::new(0.7, 0.45, 0.7),
            Some(Vec3::new(0.7, 0.45, 0.7)),
        );
        self.prop(
            "lamp_standing",
            Vec3::new(x + side * 2.6, 0.0, 18.9),
            0,
            Vec3::splat(0.8),
            Some(Vec3::new(0.4, 2.0, 0.4)),
        );
        self.prop(
            plant,
            Vec3::new(x - side * 2.6, 0.0, 18.9),
            0,
            Vec3::splat(1.0),
            Some(Vec3::new(0.7, 0.8, 0.7)),
        );
    }
}

/// Derives the flat LoS list from the components so there is one source of truth.
fn collect_colliders(mut commands: Commands, q: Query<&Collider>) {
    commands.insert_resource(Colliders(q.iter().map(|c| c.0).collect()));
}

fn build_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
) {
    let p = Palette {
        wall: materials.add(matte(Color::srgb(0.45, 0.47, 0.56))),
        trim: materials.add(matte(Color::srgb(0.12, 0.13, 0.17))),
        accent: materials.add(glow(LinearRgba::rgb(0.1, 0.9, 1.4))),
        fabric: [
            materials.add(matte(Color::srgb(0.25, 0.4, 0.55))),
            materials.add(matte(Color::srgb(0.35, 0.33, 0.5))),
            materials.add(matte(Color::srgb(0.22, 0.45, 0.42))),
        ],
        dark: materials.add(matte(Color::srgb(0.06, 0.06, 0.08))),
        screens: [
            materials.add(glow(LinearRgba::rgb(0.15, 1.3, 1.8))),
            materials.add(glow(LinearRgba::rgb(1.8, 1.0, 0.25))),
            materials.add(glow(LinearRgba::rgb(0.3, 1.8, 0.7))),
        ],
        runner: materials.add(matte(Color::srgb(0.08, 0.11, 0.2))),
        runner_edge: materials.add(glow(LinearRgba::rgb(0.1, 1.2, 2.2))),
        cable: materials.add(glow(LinearRgba::rgb(1.6, 0.25, 2.0))),
        rack: materials.add(matte(Color::srgb(0.08, 0.08, 0.1))),
        rack_panel: materials.add(matte(Color::srgb(0.16, 0.17, 0.2))),
        led_green: materials.add(glow(LinearRgba::rgb(0.2, 4.0, 0.6))),
        led_red: materials.add(glow(LinearRgba::rgb(4.0, 0.3, 0.2))),
        led_blue: materials.add(glow(LinearRgba::rgb(0.3, 1.0, 4.0))),
    };
    let floor = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(carpet_texture())),
        uv_transform: Affine2::from_scale(Vec2::splat(10.0)),
        perceptual_roughness: 0.95,
        ..default()
    });
    let mut b = Builder {
        commands: &mut commands,
        unit_cube: meshes.add(Cuboid::from_length(1.0)),
        assets: &assets,
    };

    let span = ARENA_HALF * 2.0;
    // Own mesh (not the shared unit cube) so the carpet UVs tile in metres.
    b.commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(span, 0.2, span))),
        MeshMaterial3d(floor),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));

    // Walls sit outside the playable square so their inner face is at +-ARENA_HALF.
    let off = ARENA_HALF + WALL_THICKNESS / 2.0;
    let len = span + WALL_THICKNESS * 2.0;
    let inner = ARENA_HALF - 0.04;
    for s in [-1.0, 1.0] {
        b.boxed(
            Vec3::new(0.0, WALL_HEIGHT / 2.0, s * off),
            Vec3::new(len, WALL_HEIGHT, WALL_THICKNESS),
            &p.wall,
            true,
        );
        b.boxed(
            Vec3::new(s * off, WALL_HEIGHT / 2.0, 0.0),
            Vec3::new(WALL_THICKNESS, WALL_HEIGHT, len),
            &p.wall,
            true,
        );
        // Dark baseboards and a glowing accent band round the room.
        for (h, y, mat) in [(0.2, 0.1, &p.trim), (0.06, 2.9, &p.accent)] {
            b.boxed(
                Vec3::new(0.0, y, s * inner),
                Vec3::new(span, h, 0.06),
                mat,
                false,
            );
            b.boxed(
                Vec3::new(s * inner, y, 0.0),
                Vec3::new(0.06, h, span),
                mat,
                false,
            );
        }
    }

    // Two cubicle banks flank a clear central corridor (|x| < 12).
    let mut i = 0;
    for z in [-11.0, -4.0, 3.0] {
        for x in [-16.5, -13.5, 13.5, 16.5] {
            b.cubicle(&p, i, x, z);
            i += 1;
        }
    }

    for x in [-3.0, 0.0, 3.0] {
        b.server_rack(&p, x, -ARENA_HALF + 0.6);
    }

    // Corridor runner with glowing edges, plus decorative cables (not collidable).
    b.boxed(
        Vec3::new(0.0, 0.01, -1.0),
        Vec3::new(6.0, 0.02, 34.0),
        &p.runner,
        false,
    );
    for s in [-1.0, 1.0] {
        b.boxed(
            Vec3::new(s * 3.0, 0.02, -1.0),
            Vec3::new(0.08, 0.03, 34.0),
            &p.runner_edge,
            false,
        );
    }
    for x in [-1.5, 0.0, 1.5] {
        b.boxed(
            Vec3::new(x + 0.3, 0.035, -8.0),
            Vec3::new(0.08, 0.03, 22.0),
            &p.cable,
            false,
        );
    }
    b.boxed(
        Vec3::new(0.0, 0.035, 9.0),
        Vec3::new(6.0, 0.03, 0.08),
        &p.cable,
        false,
    );

    b.lounge(-1.0, "cactus_medium_A");
    b.lounge(1.0, "cabinet_small_decorated");

    // Pictures on the walls (quarter turns face them into the room).
    for (x, z, turns) in [
        (-8.0, 19.97, 2),
        (8.0, 19.97, 2),
        (-8.0, -19.97, 0),
        (8.0, -19.97, 0),
        (19.97, -8.0, -1),
        (19.97, 6.0, -1),
        (-19.97, -8.0, 1),
        (-19.97, 6.0, 1),
    ] {
        b.prop(
            "pictureframe_large_A",
            Vec3::new(x, 2.0, z),
            turns,
            Vec3::splat(1.4),
            None,
        );
    }
    // Cabinets along the east/west walls, between the cubicle banks and the wall.
    for (z, side) in [(-14.0, 1.0), (-14.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        b.prop(
            "cabinet_medium_decorated",
            Vec3::new(side * 19.2, 0.0, z),
            if side > 0.0 { -1 } else { 1 },
            Vec3::new(0.8, 0.9, 0.8),
            Some(Vec3::new(0.8, 1.65, 1.6)),
        );
    }

    // One shadow-casting light only: WebGL2 budget.
    commands.spawn((
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
    // Cool fill over the cubicle banks, violet at the racks, warm in the lounges.
    for (pos, color) in [
        (Vec3::new(-9.0, 3.4, -8.0), Color::srgb(0.4, 0.7, 1.0)),
        (Vec3::new(9.0, 3.4, -8.0), Color::srgb(0.4, 0.7, 1.0)),
        (Vec3::new(0.0, 3.0, -17.0), Color::srgb(0.8, 0.4, 1.0)),
        (Vec3::new(-15.0, 3.0, 16.0), Color::srgb(1.0, 0.75, 0.45)),
        (Vec3::new(15.0, 3.0, 16.0), Color::srgb(1.0, 0.75, 0.45)),
    ] {
        commands.spawn((
            PointLight {
                color,
                intensity: 500_000.0,
                range: 22.0,
                ..default()
            },
            Transform::from_translation(pos),
        ));
    }
}
