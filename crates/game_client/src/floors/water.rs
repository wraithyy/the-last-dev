//! Lake and creek water. Not a scaled unit cube: the surface is its own thin mesh whose vertex
//! colours carry the shading (foam at the bank, shallows, deep centre), plus one tiling ripple
//! normal map that `animate` slides along. One shared StandardMaterial, no custom shader, so it
//! costs a handful of quads and runs on WebGL2.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::f32::consts::TAU;

/// Metres of water per repeat of the ripple texture.
const TILE_M: f32 = 7.0;
const NORMAL_PX: u32 = 128;
/// Distances in from the waterline-ish rim where the shading changes: foam, foam, shallows, deep.
/// The rim itself lies under the bank, so the foam band is what shows along the shore.
const BANDS: [f32; 4] = [0.0, 1.0, 1.8, 3.2];
const FOAM: [f32; 3] = [0.4, 0.55, 0.58];
const SHALLOW: [f32; 3] = [0.1, 0.3, 0.38];
const DEEP: [f32; 3] = [0.025, 0.1, 0.17];
const DISC_SEGMENTS: u32 = 48;

#[derive(Component)]
pub(super) struct Water;

pub(super) enum Shape {
    /// Full extent in x and z.
    Rect(Vec2),
    Disc(f32),
}

fn shade(e: f32) -> [f32; 4] {
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    let c = if e <= BANDS[1] {
        FOAM
    } else if e <= BANDS[2] {
        mix(FOAM, SHALLOW, (e - BANDS[1]) / (BANDS[2] - BANDS[1]))
    } else if e <= BANDS[3] {
        mix(SHALLOW, DEEP, (e - BANDS[2]) / (BANDS[3] - BANDS[2]))
    } else {
        DEEP
    };
    [c[0], c[1], c[2], 1.0]
}

/// Band distances that fit inside `half`, ending on `half` itself (the centre line).
fn bands(half: f32) -> Vec<f32> {
    let mut es: Vec<f32> = BANDS.iter().copied().filter(|e| *e < half).collect();
    es.push(half);
    es
}

struct Builder {
    centre: Vec2,
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl Builder {
    fn vertex(&mut self, x: f32, z: f32, e: f32) -> u32 {
        self.pos.push([x, 0.0, z]);
        // UVs in world metres so neighbouring pieces ripple in step.
        self.uv.push([self.centre.x + x, self.centre.y + z]);
        self.col.push(shade(e));
        (self.pos.len() - 1) as u32
    }

    fn finish(self) -> Mesh {
        let n = self.pos.len();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
        // Tangent along +x with v running along +z, so the normal map needs no derivatives.
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1.0, 0.0, 0.0, -1.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
        .with_inserted_indices(Indices::U32(self.idx))
    }
}

fn mesh(shape: &Shape, centre: Vec2) -> Mesh {
    let mut b = Builder {
        centre,
        pos: Vec::new(),
        uv: Vec::new(),
        col: Vec::new(),
        idx: Vec::new(),
    };
    match *shape {
        Shape::Rect(size) => {
            let across_x = size.x <= size.y;
            let (short, long) = if across_x {
                (size.x, size.y)
            } else {
                (size.y, size.x)
            };
            let half = short / 2.0;
            let es = bands(half);
            // Cross-section from one bank to the other, shading mirrored about the centre line.
            let mut rows: Vec<(f32, f32)> = es.iter().map(|e| (-half + e, *e)).collect();
            rows.extend(es.iter().rev().skip(1).map(|e| (half - e, *e)));
            for &(c, e) in &rows {
                for a in [-long / 2.0, long / 2.0] {
                    if across_x {
                        b.vertex(c, a, e);
                    } else {
                        b.vertex(a, c, e);
                    }
                }
            }
            for i in 0..rows.len() as u32 - 1 {
                let (a, c, bb, d) = (i * 2, i * 2 + 1, i * 2 + 2, i * 2 + 3);
                // Vertices pair up along the length; `across_x` decides which way is up.
                let tris = if across_x {
                    [a, c, bb, bb, c, d]
                } else {
                    [a, bb, c, bb, d, c]
                };
                b.idx.extend(tris);
            }
        }
        Shape::Disc(r) => {
            let es = bands(r);
            let n = DISC_SEGMENTS;
            for e in &es {
                for s in 0..n {
                    let a = s as f32 * TAU / n as f32;
                    let rad = r - e;
                    b.vertex(rad * a.cos(), rad * a.sin(), *e);
                }
            }
            for k in 0..es.len() as u32 - 1 {
                for s in 0..n {
                    let s1 = (s + 1) % n;
                    let (o0, o1) = (k * n + s, k * n + s1);
                    let (i0, i1) = ((k + 1) * n + s, (k + 1) * n + s1);
                    b.idx.extend([o0, i0, o1, o1, i0, i1]);
                }
            }
        }
    }
    b.finish()
}

/// Tileable ripples: a few sine waves with whole-number frequencies, so the edges meet.
fn normal_map() -> Image {
    const WAVES: [(f32, f32, f32, f32); 4] = [
        (2.0, 1.0, 1.0, 0.3),
        (1.0, 3.0, 0.8, 1.7),
        (4.0, -2.0, 0.5, 2.9),
        (-3.0, 5.0, 0.35, 0.9),
    ];
    let mut data = Vec::with_capacity((NORMAL_PX * NORMAL_PX * 4) as usize);
    for y in 0..NORMAL_PX {
        for x in 0..NORMAL_PX {
            let (u, v) = (x as f32 / NORMAL_PX as f32, y as f32 / NORMAL_PX as f32);
            let (mut gx, mut gy) = (0.0, 0.0);
            for (fx, fy, a, ph) in WAVES {
                let c = a * (TAU * (fx * u + fy * v) + ph).cos() * TAU;
                gx += c * fx;
                gy += c * fy;
            }
            // Slope strength: enough to catch the sun in ripples without choppy-sea noise.
            let n = Vec3::new(-gx * 0.002, -gy * 0.002, 1.0).normalize();
            let enc = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
            data.extend([enc(n.x), enc(n.y), enc(n.z), 255]);
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: NORMAL_PX,
            height: NORMAL_PX,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    img
}

pub(super) fn material(images: &mut Assets<Image>) -> StandardMaterial {
    StandardMaterial {
        // The tint lives in the vertex colours.
        base_color: Color::WHITE,
        normal_map_texture: Some(images.add(normal_map())),
        perceptual_roughness: 0.5,
        reflectance: 0.25,
        // A faint glow keeps the water from going pitch black in the evening.
        emissive: LinearRgba::rgb(0.004, 0.018, 0.03),
        uv_transform: Affine2::from_scale(Vec2::splat(1.0 / TILE_M)),
        ..default()
    }
}

/// `surface` is the world position of the water plane's centre.
pub(super) fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    surface: Vec3,
    shape: Shape,
    marker: impl Bundle,
) {
    commands.spawn((
        marker,
        Water,
        Mesh3d(meshes.add(mesh(&shape, Vec2::new(surface.x, surface.z)))),
        MeshMaterial3d(material),
        Transform::from_translation(surface),
        NotShadowCaster,
    ));
}

/// Slides the ripples; the material is shared, so one write moves every water surface.
pub(super) fn animate(
    time: Res<Time>,
    water: Query<&MeshMaterial3d<StandardMaterial>, With<Water>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last: Local<f32>,
) {
    // Every write re-uploads the material, so slide at ~15 Hz instead of every frame.
    if time.elapsed_secs() - *last < 1.0 / 15.0 {
        return;
    }
    *last = time.elapsed_secs();
    let Some(handle) = water.iter().next() else {
        return;
    };
    let Some(mut m) = materials.get_mut(&handle.0) else {
        return;
    };
    let t = time.elapsed_secs();
    m.uv_transform = Affine2::from_scale_angle_translation(
        Vec2::splat(1.0 / TILE_M),
        0.0,
        Vec2::new(t * 0.02, t * 0.013),
    );
}
