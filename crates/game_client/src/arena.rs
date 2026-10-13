//! The office arena: primitives for structure, Kenney furniture for dressing.
//! Every solid prop registers an AABB so collision, camera clipping and line
//! of sight share one source.

use crate::collision::aabb;
use crate::config::{ARENA_HALF, WALL_HEIGHT, WALL_THICKNESS};
use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
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

/// Kenney furniture is modelled ~2.4x too small for a 1.8 m character.
const FURNITURE_SCALE: f32 = 2.4;

/// Furniture GLB plus the bounding box measured from its glTF accessors
/// (model space, metres). Kit origins sit at a corner of the box or its
/// centre depending on the piece, so placement centres the footprint.
/// The model front is +Z.
struct Model {
    file: &'static str,
    /// Minimum x and z of the box.
    lo: [f32; 2],
    /// Width (x), height (y) and depth (z).
    size: [f32; 3],
}

impl Model {
    const fn new(file: &'static str, lo: [f32; 2], size: [f32; 3]) -> Self {
        Self { file, lo, size }
    }
}

const BOOKCASE_CLOSED_WIDE: Model =
    Model::new("bookcaseClosedWide", [0.0, -0.25], [0.8, 0.79, 0.25]);
const BOOKCASE_OPEN: Model = Model::new("bookcaseOpen", [0.0, -0.25], [0.4, 0.88, 0.25]);
const BOOKS: Model = Model::new("books", [0.0, -0.095], [0.15, 0.104, 0.095]);
const CHAIR_DESK: Model = Model::new("chairDesk", [0.0, -0.314], [0.335, 0.608, 0.314]);
const COAT_RACK: Model = Model::new("coatRackStanding", [-0.136, -0.136], [0.273, 0.77, 0.273]);
const KEYBOARD: Model = Model::new("computerKeyboard", [0.0, -0.118], [0.282, 0.028, 0.118]);
const MOUSE: Model = Model::new("computerMouse", [-0.025, -0.085], [0.05, 0.024, 0.085]);
const SCREEN: Model = Model::new("computerScreen", [0.0, -0.104], [0.393, 0.294, 0.104]);
const DESK: Model = Model::new("desk", [-0.01, -0.38], [0.734, 0.384, 0.392]);
const COFFEE_MACHINE: Model = Model::new("kitchenCoffeeMachine", [0.0, -0.24], [0.19, 0.177, 0.24]);
const FRIDGE: Model = Model::new("kitchenFridge", [0.0, -0.282], [0.43, 0.92, 0.292]);
const LAMP_TABLE: Model = Model::new("lampRoundTable", [-0.016, -0.148], [0.152, 0.314, 0.176]);
const LAMP_FLOOR: Model = Model::new("lampSquareFloor", [0.0, -0.12], [0.12, 0.86, 0.12]);
const LAPTOP: Model = Model::new("laptop", [0.0, -0.24], [0.264, 0.162, 0.24]);
const LOUNGE_CHAIR: Model = Model::new("loungeChair", [0.0, -0.41], [0.49, 0.46, 0.41]);
const LOUNGE_SOFA: Model = Model::new("loungeSofa", [0.0, -0.41], [0.98, 0.46, 0.41]);
const PLANTS_SMALL: [Model; 3] = [
    Model::new("plantSmall1", [-0.047, -0.047], [0.095, 0.14, 0.095]),
    Model::new("plantSmall2", [-0.047, -0.047], [0.095, 0.14, 0.095]),
    Model::new("plantSmall3", [-0.049, -0.042], [0.098, 0.145, 0.085]),
];
const POTTED_PLANT: Model = Model::new("pottedPlant", [-0.106, -0.121], [0.212, 0.654, 0.241]);
const RUG: Model = Model::new("rugRectangle", [0.0, -0.92], [1.57, 0.01, 0.92]);
const SIDE_TABLE: Model = Model::new("sideTable", [-0.01, -0.21], [0.534, 0.384, 0.22]);
const COFFEE_TABLE: Model = Model::new("tableCoffee", [-0.461, -0.3], [0.661, 0.23, 0.4]);
const TRASHCAN: Model = Model::new("trashcan", [-0.104, -0.112], [0.208, 0.428, 0.234]);

/// World-space AABB size of a box of `size` turned by `yaw` about Y.
fn rotated_footprint(size: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    Vec3::new(
        size.x * c.abs() + size.z * s.abs(),
        size.y,
        size.x * s.abs() + size.z * c.abs(),
    )
}

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
    board: Handle<StandardMaterial>,
    scribbles: [Handle<StandardMaterial>; 3],
    water: Handle<StandardMaterial>,
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

fn to_linear(b: u8) -> f32 {
    (b as f32 / 255.0).powf(2.2)
}

fn to_srgb(v: f32) -> u8 {
    (v.powf(1.0 / 2.2) * 255.0 + 0.5).min(255.0) as u8
}

/// Appends a CPU box-filtered mip chain (averaged in linear light) to RGBA8 sRGB
/// pixels, returning all levels back to back and the level count.
fn mip_chain(mut w: u32, mut h: u32, base: Vec<u8>) -> (Vec<u8>, u32) {
    let mut data = base;
    let (mut offset, mut levels) = (0usize, 1);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = Vec::with_capacity((nw * nh * 4) as usize);
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0.0_f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(w - 1), (2 * y + dy).min(h - 1));
                    let i = offset + ((sy * w + sx) * 4) as usize;
                    for (c, a) in acc.iter_mut().take(3).enumerate() {
                        *a += to_linear(data[i + c]);
                    }
                    acc[3] += data[i + 3] as f32;
                }
                next.extend_from_slice(&[
                    to_srgb(acc[0] / 4.0),
                    to_srgb(acc[1] / 4.0),
                    to_srgb(acc[2] / 4.0),
                    (acc[3] / 4.0 + 0.5) as u8,
                ]);
            }
        }
        offset += (w * h * 4) as usize;
        data.extend(next);
        (w, h) = (nw, nh);
        levels += 1;
    }
    (data, levels)
}

/// Mipmapped, anisotropically filtered sRGB texture. Without mips the repeating
/// floor and ceiling shimmer at grazing angles.
fn mipped_texture(w: u32, h: u32, base: Vec<u8>, repeat: bool) -> Image {
    let (data, levels) = mip_chain(w, h, base);
    // `Image::new` only accepts the base level, so fill the chain manually.
    let mut img = Image::new_uninit(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.data = Some(data);
    img.texture_descriptor.mip_level_count = levels;
    let mode = if repeat {
        ImageAddressMode::Repeat
    } else {
        ImageAddressMode::ClampToEdge
    };
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode,
        address_mode_v: mode,
        anisotropy_clamp: 8,
        ..ImageSamplerDescriptor::linear()
    });
    img
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
    mipped_texture(N, N, data, true)
}

/// Drop-ceiling tile texture: 4x4 pale panels with dark grid lines and speckle.
fn ceiling_texture() -> Image {
    const N: u32 = 64;
    const TILE: u32 = N / 4;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let grid = x % TILE == 0 || y % TILE == 0;
            let speckle = 0.94 + 0.06 * ((x * 11 + y * 17 + x * y * 5) % 13) as f32 / 12.0;
            let v = if grid { 0.28 } else { 0.62 * speckle };
            let v = (v * 255.0) as u8;
            data.extend_from_slice(&[v, v, (v as f32 * 1.04) as u8, 255]);
        }
    }
    mipped_texture(N, N, data, true)
}

/// Tiny software canvas for the satirical posters: flat shapes only.
struct Canvas {
    w: u32,
    h: u32,
    px: Vec<[u8; 3]>,
}

impl Canvas {
    fn new(w: u32, h: u32, bg: [u8; 3]) -> Self {
        Self {
            w,
            h,
            px: vec![bg; (w * h) as usize],
        }
    }

    fn set(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h {
            self.px[(y as u32 * self.w + x as u32) as usize] = c;
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: [u8; 3]) {
        for y in 0..self.h as i32 {
            for x in 0..self.w as i32 {
                if (x as f32 - cx).hypot(y as f32 - cy) <= r {
                    self.set(x, y, c);
                }
            }
        }
    }

    fn line(&mut self, a: (f32, f32), b: (f32, f32), c: [u8; 3]) {
        let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().max(1.0) as i32 * 2;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            self.rect(x as i32, y as i32, 2, 2, c);
        }
    }

    fn into_image(self) -> Image {
        let data = self
            .px
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect();
        mipped_texture(self.w, self.h, data, false)
    }
}

const POSTER_W: u32 = 64;
const POSTER_H: u32 = 80;

/// Four original office-motivation posters drawn from primitives (no text, no
/// third-party art): rising synergy bars, a burndown that never burns, a
/// hang-in-there blob and a pie chart where every slice is "other".
fn poster_images() -> [Image; 4] {
    let (w, h) = (POSTER_W as i32, POSTER_H as i32);

    let mut bars = Canvas::new(POSTER_W, POSTER_H, [22, 46, 92]);
    for i in 0..5 {
        let bh = 10 + i * 9;
        bars.rect(
            6 + i * 11,
            h - 12 - bh,
            8,
            bh,
            [90 + i as u8 * 30, 200, 120],
        );
    }
    bars.line((6.0, 52.0), (58.0, 10.0), [250, 220, 60]);
    bars.rect(6, 4, 52, 3, [250, 250, 250]);

    let mut burn = Canvas::new(POSTER_W, POSTER_H, [244, 238, 220]);
    burn.rect(8, 8, 2, h - 18, [40, 40, 40]);
    burn.rect(8, h - 12, w - 14, 2, [40, 40, 40]);
    burn.line((10.0, 14.0), (58.0, h as f32 - 12.0), [150, 150, 150]);
    burn.line((10.0, 16.0), (44.0, 18.0), [200, 40, 40]);
    burn.line((44.0, 18.0), (58.0, h as f32 - 13.0), [200, 40, 40]);

    let mut hang = Canvas::new(POSTER_W, POSTER_H, [150, 205, 235]);
    hang.rect(0, 14, w, 4, [110, 75, 40]);
    hang.line((24.0, 18.0), (28.0, 40.0), [60, 60, 60]);
    hang.line((40.0, 18.0), (36.0, 40.0), [60, 60, 60]);
    hang.disc(32.0, 52.0, 14.0, [250, 215, 70]);
    hang.disc(27.0, 49.0, 2.0, [40, 40, 40]);
    hang.disc(37.0, 49.0, 2.0, [40, 40, 40]);
    hang.rect(27, 58, 10, 2, [40, 40, 40]);

    let mut pie = Canvas::new(POSTER_W, POSTER_H, [40, 36, 48]);
    let (cx, cy) = (32.0_f32, 36.0_f32);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            if dx.hypot(dy) <= 24.0 {
                let a = dy.atan2(dx).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
                let c = match a {
                    a if a < 0.62 => [200, 70, 70],
                    a if a < 0.9 => [70, 150, 210],
                    _ => [230, 200, 80],
                };
                pie.set(x, y, c);
            }
        }
    }
    pie.rect(8, 68, 48, 3, [230, 230, 230]);

    [
        bars.into_image(),
        burn.into_image(),
        hang.into_image(),
        pie.into_image(),
    ]
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

    /// Kenney furniture piece with its footprint centred on `pos` (floor level) and
    /// turned by `yaw`. Solid pieces register an AABB.
    fn prop(&mut self, m: &Model, pos: Vec3, yaw: f32, solid: bool) -> Entity {
        let s = FURNITURE_SCALE;
        let scene = self
            .assets
            .load(GltfAssetLabel::Scene(0).from_asset(format!("models/furniture/{}.glb", m.file)));
        let offset = Vec3::new(
            -(m.lo[0] + m.size[0] / 2.0),
            0.0,
            -(m.lo[1] + m.size[2] / 2.0),
        ) * s;
        let mut e = self.commands.spawn((
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ));
        e.with_children(|c| {
            c.spawn((
                WorldAssetRoot(scene),
                Transform::from_translation(offset).with_scale(Vec3::splat(s)),
            ));
        });
        if solid {
            let size = Vec3::from(m.size) * s;
            e.insert(Collider(aabb(
                pos + Vec3::Y * size.y / 2.0,
                rotated_footprint(size, yaw),
            )));
        }
        e.id()
    }

    /// Open towards +Z: back and side partitions, desk with a computer, chair.
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
        let top = DESK.size[1] * FURNITURE_SCALE;
        let desk_d = DESK.size[2] * FURNITURE_SCALE;
        let desk_z = z - half + PART_T / 2.0 + desk_d / 2.0;
        self.prop(&DESK, Vec3::new(x, 0.0, desk_z), 0.0, true);
        let back = desk_z - desk_d / 2.0;
        if i.is_multiple_of(2) {
            self.prop(&SCREEN, Vec3::new(x, top, back + 0.2), 0.0, false);
            self.prop(&KEYBOARD, Vec3::new(x, top, back + 0.62), 0.0, false);
            self.prop(&MOUSE, Vec3::new(x + 0.55, top, back + 0.62), 0.0, false);
        } else {
            self.prop(&LAPTOP, Vec3::new(x, top, back + 0.45), 0.0, false);
            self.prop(&MOUSE, Vec3::new(x + 0.55, top, back + 0.5), 0.0, false);
            self.prop(&BOOKS, Vec3::new(x + 0.72, top, back + 0.2), 0.0, false);
        }
        self.prop(
            &PLANTS_SMALL[i % 3],
            Vec3::new(x - 0.72, top, back + 0.25),
            0.0,
            false,
        );
        if i.is_multiple_of(3) {
            self.prop(
                &LAMP_TABLE,
                Vec3::new(x + 0.72, top, back + 0.25),
                0.0,
                false,
            );
        }
        // Chair faces the desk (-Z); a slight yaw per cubicle makes the room feel lived in.
        let yaw = std::f32::consts::PI + [0.35, -0.2, 0.1][i % 3];
        let chair_pos = Vec3::new(x + 0.1, 0.0, z - 0.1);
        let chair = self.prop(&CHAIR_DESK, chair_pos, yaw, false);
        // The collider floats 0.12 m above the floor: combat LoS rays run at
        // ground level and the same list feeds them, so chairs must not block
        // sight, yet still stop a body-height walk. See KNOWN_LIMITATIONS.
        let chair_box = Vec3::new(0.6, 0.8, 0.6);
        self.commands.entity(chair).insert(Collider(aabb(
            chair_pos + Vec3::Y * (0.12 + chair_box.y / 2.0),
            chair_box,
        )));
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

    /// Sofa on a rug with a coffee table, an armchair and a lamp: a quiet corner away from the cubicles.
    fn lounge(&mut self, side: f32, plant: bool) {
        let x = side * 15.0;
        let pi = std::f32::consts::PI;
        self.prop(&RUG, Vec3::new(x, 0.0, 16.4), 0.0, false);
        self.prop(&LOUNGE_SOFA, Vec3::new(x, 0.0, 18.9), pi, true);
        self.prop(&COFFEE_TABLE, Vec3::new(x, 0.0, 16.4), 0.0, true);
        self.prop(
            &LOUNGE_CHAIR,
            Vec3::new(x + side * 2.7, 0.0, 16.6),
            -side * FRAC_PI_2,
            true,
        );
        self.prop(&LAMP_FLOOR, Vec3::new(x + side * 2.7, 0.0, 18.9), 0.0, true);
        if plant {
            self.prop(
                &POTTED_PLANT,
                Vec3::new(x - side * 2.7, 0.0, 18.9),
                0.0,
                true,
            );
        } else {
            self.prop(&COAT_RACK, Vec3::new(x - side * 2.7, 0.0, 18.9), 0.0, true);
        }
    }

    /// Wall-mounted board facing +Z at `x` on the north wall, with a roadmap that never ships.
    fn whiteboard(&mut self, p: &Palette, x: f32) {
        let z = -ARENA_HALF + 0.06;
        self.boxed(
            Vec3::new(x, 2.0, z),
            Vec3::new(3.2, 1.7, 0.06),
            &p.trim,
            false,
        );
        self.boxed(
            Vec3::new(x, 2.0, z + 0.04),
            Vec3::new(3.0, 1.5, 0.03),
            &p.board,
            false,
        );
        for (n, (dx, dy, w)) in [
            (-0.6, 0.45, 1.4),
            (0.2, 0.15, 1.8),
            (-0.3, -0.2, 1.0),
            (0.5, -0.45, 1.2),
        ]
        .into_iter()
        .enumerate()
        {
            self.boxed(
                Vec3::new(x + dx, 2.0 + dy, z + 0.06),
                Vec3::new(w, 0.07, 0.01),
                &p.scribbles[n % 3],
                false,
            );
        }
    }

    /// Water cooler from primitives (the furniture kit has none).
    fn water_cooler(&mut self, p: &Palette, x: f32, z: f32) {
        self.boxed(
            Vec3::new(x, 0.45, z),
            Vec3::new(0.4, 0.9, 0.4),
            &p.board,
            true,
        );
        self.boxed(
            Vec3::new(x, 1.12, z),
            Vec3::new(0.3, 0.45, 0.3),
            &p.water,
            false,
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
        wall: materials.add(matte(Color::srgb(0.56, 0.55, 0.6))),
        trim: materials.add(matte(Color::srgb(0.12, 0.13, 0.17))),
        accent: materials.add(glow(LinearRgba::rgb(0.1, 0.9, 1.4))),
        fabric: [
            materials.add(matte(Color::srgb(0.25, 0.4, 0.55))),
            materials.add(matte(Color::srgb(0.35, 0.33, 0.5))),
            materials.add(matte(Color::srgb(0.22, 0.45, 0.42))),
        ],
        board: materials.add(matte(Color::srgb(0.92, 0.93, 0.95))),
        scribbles: [
            materials.add(matte(Color::srgb(0.85, 0.2, 0.2))),
            materials.add(matte(Color::srgb(0.2, 0.4, 0.85))),
            materials.add(matte(Color::srgb(0.2, 0.65, 0.35))),
        ],
        water: materials.add(matte(Color::srgb(0.55, 0.8, 0.95))),
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
    let ceiling = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(ceiling_texture())),
        // 4 tiles per repeat over 40 m: ~0.6 m panels.
        uv_transform: Affine2::from_scale(Vec2::splat(16.0)),
        perceptual_roughness: 0.95,
        // The slab is only lit from below by point lights; a faint floor keeps it readable.
        emissive: LinearRgba::rgb(0.05, 0.05, 0.06),
        ..default()
    });
    let light_panel = materials.add(glow(LinearRgba::rgb(2.4, 2.5, 2.2)));
    let posters = poster_images().map(|img| {
        materials.add(StandardMaterial {
            base_color_texture: Some(images.add(img)),
            perceptual_roughness: 0.6,
            ..default()
        })
    });
    let poster_mesh = meshes.add(Rectangle::new(1.0, 1.25));
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

    // Ceiling slab on top of the walls. It casts no shadow (it would black out the
    // directional light) and is not a collider: camera.rs clamps height instead.
    b.commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(span + 1.0, 0.1, span + 1.0))),
        MeshMaterial3d(ceiling),
        Transform::from_xyz(0.0, WALL_HEIGHT + 0.05, 0.0),
        NotShadowCaster,
    ));
    for x in [-15.0, -9.0, -3.0, 3.0, 9.0, 15.0] {
        for z in [-16.0, -8.0, 0.0, 8.0, 16.0] {
            b.boxed(
                Vec3::new(x, WALL_HEIGHT - 0.02, z),
                Vec3::new(1.2, 0.04, 0.6),
                &light_panel,
                false,
            );
        }
    }

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

    b.lounge(-1.0, true);
    b.lounge(1.0, false);

    // Coffee corner on the east wall between the cubicle bank and the lounge.
    let east = ARENA_HALF - 0.3;
    let turn_in = -FRAC_PI_2; // faces -X, into the room
    b.prop(&SIDE_TABLE, Vec3::new(east, 0.0, 10.0), turn_in, true);
    b.prop(
        &COFFEE_MACHINE,
        Vec3::new(east, SIDE_TABLE.size[1] * FURNITURE_SCALE, 10.0),
        turn_in,
        false,
    );
    b.prop(&FRIDGE, Vec3::new(east, 0.0, 12.2), turn_in, true);
    b.prop(&TRASHCAN, Vec3::new(east, 0.0, 8.2), 0.0, true);
    b.water_cooler(&p, east, 6.8);
    b.whiteboard(&p, -10.0);
    b.whiteboard(&p, 10.0);

    // Posters on the walls (quarter turns face them into the room).
    for (n, (x, z, turns)) in [
        (-8.0, 19.97, 2),
        (8.0, 19.97, 2),
        (19.97, -8.0, -1),
        (19.97, 0.0, -1),
        (-19.97, -8.0, 1),
        (-19.97, 6.0, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let yaw = turns as f32 * FRAC_PI_2;
        let facing = Quat::from_rotation_y(yaw) * Vec3::Z;
        let pos = Vec3::new(x, 2.0, z);
        let size = rotated_footprint(Vec3::new(1.2, 1.45, 0.04), yaw);
        b.boxed(pos, size, &p.trim, false);
        b.commands.spawn((
            Mesh3d(poster_mesh.clone()),
            MeshMaterial3d(posters[n % posters.len()].clone()),
            Transform::from_translation(pos + facing * 0.03)
                .with_rotation(Quat::from_rotation_y(yaw)),
        ));
    }
    // Bookcases along the east/west walls, between the cubicle banks and the wall.
    for side in [-1.0, 1.0] {
        let yaw = if side > 0.0 { -FRAC_PI_2 } else { FRAC_PI_2 };
        let wall_x = side * (ARENA_HALF - 0.3);
        for z in [-14.0, -1.0] {
            b.prop(&BOOKCASE_CLOSED_WIDE, Vec3::new(wall_x, 0.0, z), yaw, true);
        }
        b.prop(&BOOKCASE_OPEN, Vec3::new(wall_x, 0.0, -17.5), yaw, true);
        b.prop(&POTTED_PLANT, Vec3::new(side * 18.8, 0.0, -18.8), 0.0, true);
    }
    b.prop(&POTTED_PLANT, Vec3::new(-19.0, 0.0, 12.5), 0.0, true);

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
