//! Procedural images and materials for the floors. Everything is drawn from
//! flat shapes and a tiny 5x7 bitmap font, so no external art or font is needed
//! and the same code runs on weak GPUs.

use super::kit::{Mat, Sign, Tex, sign_canvas};
use super::layout::{FloorKind, Theme};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

type Rgb = [u8; 3];

mod dc_art;

pub fn matte(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        ..default()
    }
}

pub fn glow(color: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::BLACK,
        emissive: color,
        ..default()
    }
}

fn shiny(color: Color, metallic: f32, roughness: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        metallic,
        perceptual_roughness: roughness,
        ..default()
    }
}

pub fn material(mat: Mat, t: &Theme) -> StandardMaterial {
    let c = Color::srgb;
    match mat {
        Mat::Wall => matte(t.wall),
        Mat::Trim => matte(t.trim),
        Mat::Accent => glow(t.accent),
        Mat::Fabric0 => matte(t.fabric[0]),
        Mat::Fabric1 => matte(t.fabric[1]),
        Mat::Fabric2 => matte(t.fabric[2]),
        Mat::Board => matte(c(0.92, 0.93, 0.95)),
        Mat::Scr0 => matte(c(0.85, 0.2, 0.2)),
        Mat::Scr1 => matte(c(0.2, 0.4, 0.85)),
        Mat::Scr2 => matte(c(0.2, 0.65, 0.35)),
        Mat::Water => matte(c(0.55, 0.8, 0.95)),
        Mat::Runner => matte(c(0.08, 0.11, 0.2)),
        Mat::RunnerEdge => glow(LinearRgba::rgb(0.1, 1.2, 2.2)),
        Mat::Cable => glow(LinearRgba::rgb(1.6, 0.25, 2.0)),
        Mat::Rack => matte(c(0.08, 0.08, 0.1)),
        Mat::RackPanel => matte(c(0.16, 0.17, 0.2)),
        Mat::LedGreen => glow(LinearRgba::rgb(0.2, 4.0, 0.6)),
        Mat::LedRed => glow(LinearRgba::rgb(4.0, 0.3, 0.2)),
        Mat::LedBlue => glow(LinearRgba::rgb(0.3, 1.0, 4.0)),
        Mat::LightPanel => glow(LinearRgba::rgb(2.4, 2.5, 2.2)),
        Mat::Glass => StandardMaterial {
            base_color: Color::srgba(0.6, 0.82, 0.92, 0.16),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.08,
            ..default()
        },
        Mat::Frosted => StandardMaterial {
            base_color: Color::srgba(0.93, 0.96, 0.98, 0.78),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.7,
            ..default()
        },
        Mat::Gold => shiny(c(1.0, 0.75, 0.25), 1.0, 0.3),
        Mat::Dark => matte(c(0.06, 0.06, 0.08)),
        Mat::Metal => shiny(c(0.62, 0.64, 0.68), 0.9, 0.35),
        Mat::Wood => matte(c(0.42, 0.28, 0.17)),
        Mat::Door => shiny(c(0.7, 0.72, 0.76), 1.0, 0.25),
        Mat::Beige => matte(c(0.78, 0.7, 0.55)),
        Mat::NeonPink => glow(LinearRgba::rgb(4.0, 0.4, 2.2)),
        Mat::NeonCyan => glow(LinearRgba::rgb(0.3, 2.6, 3.4)),
        Mat::NeonYellow => glow(LinearRgba::rgb(3.6, 3.0, 0.4)),
        Mat::Candle => glow(LinearRgba::rgb(4.0, 2.2, 0.5)),
        Mat::ScreenGlow => glow(LinearRgba::rgb(0.5, 1.4, 2.4)),
        Mat::BeanA => matte(c(0.9, 0.35, 0.25)),
        Mat::BeanB => matte(c(0.25, 0.55, 0.9)),
        Mat::BeanC => matte(c(0.95, 0.8, 0.25)),
        Mat::Felt => matte(c(0.1, 0.4, 0.22)),
        Mat::Cardboard => matte(c(0.62, 0.46, 0.28)),
        Mat::Paper => matte(c(0.93, 0.92, 0.88)),
        Mat::Yoga => matte(c(0.55, 0.3, 0.7)),
        Mat::Red => matte(c(0.75, 0.12, 0.12)),
        Mat::Yellow => matte(c(0.9, 0.75, 0.15)),
        Mat::White => matte(c(0.95, 0.95, 0.95)),
        Mat::Marble => StandardMaterial {
            perceptual_roughness: 0.25,
            ..matte(c(0.88, 0.86, 0.84))
        },
        Mat::MarbleDark => StandardMaterial {
            perceptual_roughness: 0.2,
            ..matte(c(0.12, 0.11, 0.13))
        },
        Mat::Plant => matte(c(0.2, 0.5, 0.22)),
        Mat::Dead => matte(c(0.45, 0.33, 0.14)),
        Mat::Monolith => StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(0.9, 0.2, 2.6),
            ..default()
        },
        Mat::Asphalt => matte(c(0.16, 0.16, 0.18)),
        Mat::Concrete => matte(c(0.55, 0.55, 0.56)),
        Mat::Brick => matte(c(0.5, 0.26, 0.2)),
        Mat::Bark => matte(c(0.28, 0.19, 0.12)),
        Mat::Leaf => matte(c(0.16, 0.45, 0.17)),
        Mat::LeafDark => matte(c(0.1, 0.32, 0.14)),
        Mat::CarRed => shiny(c(0.75, 0.1, 0.1), 0.6, 0.35),
        Mat::CarBlue => shiny(c(0.12, 0.25, 0.7), 0.6, 0.35),
        Mat::CarWhite => shiny(c(0.9, 0.9, 0.92), 0.6, 0.35),
        Mat::CarTeal => shiny(c(0.1, 0.55, 0.55), 0.6, 0.35),
        Mat::Tire => matte(c(0.04, 0.04, 0.05)),
        Mat::Hot => glow(LinearRgba::rgb(4.0, 1.0, 0.15)),
        Mat::Cold => glow(LinearRgba::rgb(1.4, 2.4, 3.0)),
        Mat::BlinkGreenA | Mat::BlinkGreenB => glow(LinearRgba::rgb(0.2, 4.0, 0.6)),
        Mat::BlinkRed => glow(LinearRgba::rgb(4.0, 0.3, 0.2)),
        Mat::BlinkBlue => glow(LinearRgba::rgb(0.3, 1.0, 4.0)),
        Mat::BlinkAmber => glow(LinearRgba::rgb(4.0, 2.2, 0.3)),
        Mat::Chrome => shiny(c(0.8, 0.82, 0.86), 1.0, 0.15),
        Mat::Facade => StandardMaterial {
            base_color: Color::srgb(0.18, 0.3, 0.4),
            metallic: 0.7,
            perceptual_roughness: 0.18,
            ..default()
        },
        Mat::NeonRed => glow(LinearRgba::rgb(4.0, 0.2, 0.3)),
        Mat::Hazard => matte(c(0.95, 0.72, 0.05)),
        Mat::Sand => matte(c(0.72, 0.64, 0.45)),
        Mat::Window => StandardMaterial {
            base_color: c(0.1, 0.15, 0.2),
            metallic: 0.5,
            perceptual_roughness: 0.2,
            ..default()
        },
        Mat::Lake => StandardMaterial {
            base_color: c(0.13, 0.33, 0.46),
            metallic: 0.25,
            perceptual_roughness: 0.1,
            ..default()
        },
    }
}

/// Screens and neon: drawn at full brightness regardless of the room lights.
fn unlit(tex: Tex) -> bool {
    matches!(
        tex,
        Tex::Sign(
            Sign::DaysAi
                | Sign::Billboard
                | Sign::InnovationNeon
                | Sign::MoveFast
                | Sign::AgiRoom
                | Sign::VrDemo
                | Sign::DemoDay
                | Sign::NocBoard
                | Sign::AiTraining
                | Sign::RackFire
                | Sign::Uptime
                | Sign::TempWarning
                | Sign::Printing
                | Sign::AgiArena
                | Sign::Shrine
                | Sign::KeynoteScreen
                | Sign::FamilyBanner
                | Sign::OffsiteBar
                | Sign::VibeConBanner
                | Sign::VibeConBooth
                | Sign::VibeConSwag
                | Sign::VibeConShipLive
                | Sign::VibeConJudging
                | Sign::VibeConAfterparty
                | Sign::VibeConFireside
                | Sign::VibeConKeynote,
        ) | Tex::NocGraph
            | Tex::NocAlerts
            | Tex::NocRegions
            | Tex::DaysCounter
            | Tex::CloseDeals
            | Tex::ChatbotFace
            | Tex::Skyline
            | Tex::DemoScreen
            | Tex::Vending
            | Tex::Kpi
            | Tex::Brand
            | Tex::Campaign
            | Tex::SocAlert
            | Tex::PraiseModel
            | Tex::Leaderboard
    )
}

pub fn tex_material(tex: Tex, image: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: Some(image),
        perceptual_roughness: 0.6,
        unlit: unlit(tex),
        alpha_mode: if tex == Tex::Cobweb {
            AlphaMode::Mask(0.35)
        } else {
            AlphaMode::Opaque
        },
        double_sided: tex == Tex::Cobweb,
        cull_mode: None,
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

fn hash(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Carpet tile texture: 2x2 tiles in two shades with grout lines and a little noise.
/// Repeats via the sampler, so one small image covers the whole floor.
fn carpet_texture(tint: [f32; 3]) -> Image {
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
            let v = shade * grout * noise;
            let px = |k: f32| (v * k * 255.0).min(255.0) as u8;
            data.extend_from_slice(&[px(tint[0]), px(tint[1]), px(tint[2]), 255]);
        }
    }
    mipped_texture(N, N, data, true)
}

/// Large pale slabs with a few darker veins: cheap marble.
fn marble_texture() -> Image {
    const N: u32 = 64;
    const TILE: u32 = N / 2;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let vein = ((x as f32 * 0.35 + y as f32 * 0.2 + hash(x / 6, y / 6) * 2.0).sin() * 0.5
                + 0.5)
                .powi(8);
            let base = 0.6 + 0.07 * hash(x / 3, y / 3) - 0.16 * vein;
            let grout = if x % TILE == 0 || y % TILE == 0 {
                0.55
            } else {
                1.0
            };
            let v = (base * grout).clamp(0.0, 1.0);
            let px = |k: f32| (v * k * 255.0) as u8;
            data.extend_from_slice(&[px(1.0), px(0.97), px(0.95), 255]);
        }
    }
    mipped_texture(N, N, data, true)
}

/// Mown lawn: two greens in noisy patches; repeats over 4 m.
fn grass_texture() -> Image {
    const N: u32 = 64;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let patch = hash(x / 8, y / 8);
            let blade = hash(x, y);
            let v = 0.55 + 0.25 * patch + 0.2 * blade;
            let px = |k: f32| (v * k * 255.0).min(255.0) as u8;
            data.extend_from_slice(&[px(0.23), px(0.5), px(0.2), 255]);
        }
    }
    mipped_texture(N, N, data, true)
}

/// 1 m floor tiles with seams; `perforated` adds the vent holes of a raised data-centre floor.
fn tile_texture(tint: [f32; 3], perforated: bool) -> Image {
    const N: u32 = 64;
    const TILE: u32 = N / 4;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let seam = x % TILE <= 1 || y % TILE <= 1;
            let (lx, ly) = (x % TILE, y % TILE);
            let hole = perforated
                && lx > 3
                && ly > 3
                && lx < TILE - 3
                && ly < TILE - 3
                && lx % 3 == 0
                && ly % 3 == 0;
            let base = if perforated { 0.42 } else { 0.85 };
            let v = if seam {
                base * 0.55
            } else if hole {
                base * 0.35
            } else {
                base * (0.96 + 0.08 * hash(x / 2, y / 2))
            };
            let px = |k: f32| (v * k * 255.0).min(255.0) as u8;
            data.extend_from_slice(&[px(tint[0]), px(tint[1]), px(tint[2]), 255]);
        }
    }
    mipped_texture(N, N, data, true)
}

pub fn floor_image(kind: FloorKind) -> Image {
    match kind {
        FloorKind::Carpet(tint) => carpet_texture(tint),
        FloorKind::Marble => marble_texture(),
        FloorKind::Grass => grass_texture(),
        FloorKind::Tile(tint) => tile_texture(tint, true),
        FloorKind::Gloss(tint) => tile_texture(tint, false),
    }
}

/// Drop-ceiling tile texture: 4x4 pale panels with dark grid lines and speckle.
pub fn ceiling_image() -> Image {
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

/// 5x7 glyph rows, `/`-separated, `#` is ink.
const FONT: &[(char, &str)] = &[
    ('A', ".###./#...#/#...#/#####/#...#/#...#/#...#"),
    ('B', "####./#...#/#...#/####./#...#/#...#/####."),
    ('C', ".###./#...#/#..../#..../#..../#...#/.###."),
    ('D', "####./#...#/#...#/#...#/#...#/#...#/####."),
    ('E', "#####/#..../#..../####./#..../#..../#####"),
    ('F', "#####/#..../#..../####./#..../#..../#...."),
    ('G', ".###./#...#/#..../#.###/#...#/#...#/.####"),
    ('H', "#...#/#...#/#...#/#####/#...#/#...#/#...#"),
    ('I', ".###./..#../..#../..#../..#../..#../.###."),
    ('J', "..###/...#./...#./...#./...#./#..#./.##.."),
    ('K', "#...#/#..#./#.#../##.../#.#../#..#./#...#"),
    ('L', "#..../#..../#..../#..../#..../#..../#####"),
    ('M', "#...#/##.##/#.#.#/#.#.#/#...#/#...#/#...#"),
    ('N', "#...#/##..#/#.#.#/#..##/#...#/#...#/#...#"),
    ('O', ".###./#...#/#...#/#...#/#...#/#...#/.###."),
    ('P', "####./#...#/#...#/####./#..../#..../#...."),
    ('Q', ".###./#...#/#...#/#...#/#.#.#/#..#./.##.#"),
    ('R', "####./#...#/#...#/####./#.#../#..#./#...#"),
    ('S', ".####/#..../#..../.###./....#/....#/####."),
    ('T', "#####/..#../..#../..#../..#../..#../..#.."),
    ('U', "#...#/#...#/#...#/#...#/#...#/#...#/.###."),
    ('V', "#...#/#...#/#...#/#...#/#...#/.#.#./..#.."),
    ('W', "#...#/#...#/#...#/#.#.#/#.#.#/##.##/#...#"),
    ('X', "#...#/#...#/.#.#./..#../.#.#./#...#/#...#"),
    ('Y', "#...#/#...#/.#.#./..#../..#../..#../..#.."),
    ('Z', "#####/....#/...#./..#../.#.../#..../#####"),
    ('0', ".###./#...#/#..##/#.#.#/##..#/#...#/.###."),
    ('1', "..#../.##../..#../..#../..#../..#../.###."),
    ('2', ".###./#...#/....#/...#./..#../.#.../#####"),
    ('3', "####./....#/....#/.###./....#/....#/####."),
    ('4', "...#./..##./.#.#./#..#./#####/...#./...#."),
    ('5', "#####/#..../####./....#/....#/#...#/.###."),
    ('6', "..##./.#.../#..../####./#...#/#...#/.###."),
    ('7', "#####/....#/...#./..#../.#.../.#.../.#..."),
    ('8', ".###./#...#/#...#/.###./#...#/#...#/.###."),
    ('9', ".###./#...#/#...#/.####/....#/...#./.##.."),
    ('?', ".###./#...#/....#/...#./..#../...../..#.."),
    ('!', "..#../..#../..#../..#../..#../...../..#.."),
    ('.', "...../...../...../...../...../.##../.##.."),
    (':', "...../.##../.##../...../.##../.##../....."),
    ('-', "...../...../...../#####/...../...../....."),
    ('\'', "..#../..#../.#.../...../...../...../....."),
    ('%', "##..#/##..#/...#./..#../.#.../#..##/#..##"),
    ('+', "...../..#../..#../#####/..#../..#../....."),
    ('>', "#..../.#.../..#../...#./..#../.#.../#...."),
    (',', "...../...../...../...../.##../..#../.#..."),
    ('/', "....#/....#/...#./..#../.#.../#..../#...."),
    ('(', "...#./..#../.#.../.#.../.#.../..#../...#."),
    (')', ".#.../..#../...#./...#./...#./..#../.#..."),
    ('&', ".##../#..#./#.#../.#.../#.#.#/#..#./.##.#"),
    ('=', "...../#####/...../#####/...../...../....."),
    ('*', "...../#.#.#/.###./#####/.###./#.#.#/....."),
];

fn glyph(c: char) -> Option<&'static str> {
    FONT.iter().find(|(g, _)| *g == c).map(|(_, rows)| *rows)
}

/// Tiny software canvas: flat shapes, bitmap text, optional transparency.
struct Canvas {
    w: u32,
    h: u32,
    px: Vec<[u8; 4]>,
}

impl Canvas {
    fn new(w: u32, h: u32, bg: Rgb) -> Self {
        Self {
            w,
            h,
            px: vec![[bg[0], bg[1], bg[2], 255]; (w * h) as usize],
        }
    }

    fn transparent(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![[255, 255, 255, 0]; (w * h) as usize],
        }
    }

    fn set(&mut self, x: i32, y: i32, c: Rgb) {
        self.set_a(x, y, c, 255);
    }

    fn set_a(&mut self, x: i32, y: i32, c: Rgb, a: u8) {
        if x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h {
            self.px[(y as u32 * self.w + x as u32) as usize] = [c[0], c[1], c[2], a];
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, t: i32, c: Rgb) {
        self.rect(x, y, w, t, c);
        self.rect(x, y + h - t, w, t, c);
        self.rect(x, y, t, h, c);
        self.rect(x + w - t, y, t, h, c);
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        for y in 0..self.h as i32 {
            for x in 0..self.w as i32 {
                if (x as f32 - cx).hypot(y as f32 - cy) <= r {
                    self.set(x, y, c);
                }
            }
        }
    }

    fn line(&mut self, a: (f32, f32), b: (f32, f32), c: Rgb) {
        self.line_a(a, b, c, 255, 2);
    }

    fn line_a(&mut self, a: (f32, f32), b: (f32, f32), c: Rgb, alpha: u8, t: i32) {
        let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().max(1.0) as i32 * 2;
        for i in 0..=steps {
            let k = i as f32 / steps as f32;
            let (x, y) = (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k);
            for dy in 0..t {
                for dx in 0..t {
                    self.set_a(x as i32 + dx, y as i32 + dy, c, alpha);
                }
            }
        }
    }

    /// Draws `s` (unknown glyphs are blank) and returns its pixel width.
    fn text(&mut self, x: i32, y: i32, scale: i32, s: &str, c: Rgb) -> i32 {
        let mut cx = x;
        for ch in s.chars() {
            if let Some(rows) = glyph(ch.to_ascii_uppercase()) {
                for (ry, row) in rows.split('/').enumerate() {
                    for (rx, b) in row.bytes().enumerate() {
                        if b == b'#' {
                            self.rect(
                                cx + rx as i32 * scale,
                                y + ry as i32 * scale,
                                scale,
                                scale,
                                c,
                            );
                        }
                    }
                }
            }
            cx += 6 * scale;
        }
        cx - x - scale
    }

    fn text_c(&mut self, y: i32, scale: i32, s: &str, c: Rgb) {
        let w = s.chars().count() as i32 * 6 * scale - scale;
        self.text((self.w as i32 - w) / 2, y, scale, s, c);
    }

    fn vgradient(&mut self, top: Rgb, bottom: Rgb) {
        for y in 0..self.h {
            let k = y as f32 / (self.h - 1) as f32;
            let c =
                [0, 1, 2].map(|i| (top[i] as f32 + (bottom[i] as f32 - top[i] as f32) * k) as u8);
            for x in 0..self.w {
                self.set(x as i32, y as i32, c);
            }
        }
    }

    fn into_image(self) -> Image {
        let data = self.px.iter().flatten().copied().collect();
        mipped_texture(self.w, self.h, data, false)
    }
}

const WHITE: Rgb = [250, 250, 250];
const BLACK: Rgb = [20, 20, 24];
const RED: Rgb = [210, 40, 40];

struct SignSpec {
    lines: &'static [&'static str],
    bg: Rgb,
    fg: Rgb,
    /// Second colour: the last line and the frame.
    hi: Rgb,
}

fn sign_spec(s: Sign) -> SignSpec {
    let sp = |lines, bg, fg, hi| SignSpec { lines, bg, fg, hi };
    match s {
        Sign::DaysAi => sp(
            &["DAYS WITHOUT AN", "AI INCIDENT:", "0"],
            [14, 8, 8],
            [255, 180, 60],
            [255, 50, 40],
        ),
        Sign::AiStrategy => sp(
            &["AI STRATEGY:", "YES"],
            [20, 30, 70],
            [235, 240, 255],
            [250, 210, 60],
        ),
        Sign::Billboard => sp(
            &["AI", "NOW WITH MORE AI"],
            [30, 14, 70],
            [120, 240, 255],
            [255, 120, 220],
        ),
        Sign::Welcome => sp(
            &["WELCOME TO", "SYNERGY CAMPUS"],
            [30, 70, 60],
            [240, 245, 240],
            [250, 210, 90],
        ),
        Sign::ParkingAi => sp(
            &["PARKING", "RESERVED FOR", "AI AGENTS"],
            [20, 50, 110],
            [240, 240, 250],
            [250, 210, 60],
        ),
        Sign::NoFeed => sp(
            &["PLEASE DO NOT", "FEED THE MODEL"],
            [240, 230, 200],
            [60, 40, 30],
            [200, 40, 40],
        ),
        Sign::Compute => sp(
            &["COMPUTE IS OUR MOST", "VALUABLE EMPLOYEE"],
            [20, 24, 36],
            [200, 210, 230],
            [90, 230, 150],
        ),
        Sign::Cafe => sp(
            &["CAFETERIA", "TOKENS ACCEPTED"],
            [90, 55, 30],
            [250, 235, 210],
            [250, 190, 80],
        ),
        Sign::SmokingCorner => sp(
            &["SMOKING CORNER", "AI COMPLAINTS DEPT."],
            [50, 50, 54],
            [220, 220, 225],
            [250, 150, 60],
        ),
        Sign::TowerName => sp(
            &["THE TOWER"],
            [24, 34, 60],
            [225, 235, 255],
            [90, 200, 255],
        ),
        Sign::DataCenterName => sp(
            &["DATA CENTER", "KEEP COOL"],
            [40, 44, 50],
            [210, 220, 230],
            [90, 220, 255],
        ),
        Sign::InnovationNeon => sp(&["INNOVATION"], [10, 6, 20], [255, 90, 210], [255, 90, 210]),
        Sign::Reception => sp(
            &["RECEPTION", "PLEASE TAKE A NUMBER", "(WE LOST THE NUMBERS)"],
            [235, 235, 238],
            [40, 50, 70],
            [200, 60, 60],
        ),
        Sign::BikeRack => sp(
            &["BIKE RACK", "(1 BIKE)"],
            [70, 110, 70],
            [240, 245, 235],
            [250, 220, 90],
        ),
        Sign::AiCar => sp(
            &["AI-POWERED CAR", "PARKING: CONFIDENT"],
            [20, 30, 50],
            [120, 240, 255],
            [255, 140, 220],
        ),
        Sign::ShuttleBus => sp(
            &["SHUTTLE BUS", "ALWAYS LATE"],
            [200, 160, 30],
            [30, 30, 30],
            [160, 30, 30],
        ),
        Sign::ScooterZone => sp(
            &["SCOOTERS:", "PLEASE STEP OVER"],
            [230, 230, 235],
            [40, 40, 50],
            [200, 60, 60],
        ),
        Sign::CampusExit => sp(
            &["EXIT TO CAMPUS", "FRESH AIR"],
            [30, 80, 50],
            [235, 245, 235],
            [250, 230, 100],
        ),
        Sign::GraveSprint => sp(
            &["HERE LIES", "THE LAST SPRINT", "RIP"],
            [60, 62, 70],
            [215, 215, 220],
            [150, 150, 160],
        ),
        Sign::SprintBoard => sp(
            &["SPRINT PLANNING", "PICK A BUILDING", "STORY POINTS: YES"],
            [244, 246, 248],
            [30, 60, 160],
            [210, 60, 40],
        ),
        Sign::ScrumBoard => sp(
            &["DAILY STANDUP", "YESTERDAY / TODAY", "BLOCKERS: YOU"],
            [250, 246, 230],
            [40, 120, 70],
            [210, 60, 40],
        ),
        Sign::SprintTerminal => sp(
            &["SPRINT TERMINAL", "> START RUN_"],
            [10, 14, 20],
            [90, 255, 140],
            [255, 200, 80],
        ),
        Sign::AiTraining
        | Sign::Kubernetes
        | Sign::Mainframe
        | Sign::PowerStrip
        | Sign::Rack42
        | Sign::NoFood
        | Sign::RackFire
        | Sign::Escort => dc_art::sign_spec(s),
        Sign::ColdAisle => sp(
            &["COLD AISLE", "BRING A JACKET"],
            [14, 40, 70],
            [150, 225, 255],
            [255, 255, 255],
        ),
        Sign::HotAisle => sp(
            &["HOT AISLE", "BRING A WILL"],
            [70, 24, 10],
            [255, 190, 110],
            [255, 90, 40],
        ),
        Sign::NocBoard => sp(
            &["ALL SYSTEMS", "PROBABLY FINE", "(ALERTS MUTED)"],
            [6, 18, 14],
            [90, 255, 150],
            [255, 200, 60],
        ),
        Sign::Shrine => sp(
            &["PRAISE", "THE CLUSTER"],
            [16, 8, 24],
            [150, 255, 160],
            [255, 200, 90],
        ),
        Sign::Uptime => sp(
            &["UPTIME 99.9%", "*SEE FINE PRINT"],
            [8, 14, 24],
            [120, 230, 255],
            [255, 200, 60],
        ),
        Sign::DoNotUnplug => sp(
            &["PLEASE DO NOT", "UNPLUG THE MODEL", "IT KNOWS"],
            [230, 200, 40],
            [30, 30, 30],
            [170, 30, 30],
        ),
        Sign::ThisIsFine => sp(
            &["THIS IS FINE"],
            [210, 90, 30],
            [255, 245, 220],
            [255, 245, 220],
        ),
        Sign::TempWarning => sp(
            &["TEMP: 41 C", "THIS IS FINE"],
            [30, 6, 6],
            [255, 170, 70],
            [255, 70, 50],
        ),
        Sign::ToCampus => sp(
            &["EXIT TO CAMPUS", "SUNLIGHT AHEAD"],
            [30, 80, 50],
            [235, 245, 235],
            [250, 230, 100],
        ),
        Sign::ToDataCenter2 => sp(
            &["SUBLEVEL 2", "HOT AISLE ONLY"],
            [60, 20, 14],
            [255, 200, 140],
            [255, 90, 40],
        ),
        Sign::ToDataCenter1 => sp(
            &["BACK UP", "TO SUBLEVEL 1"],
            [24, 40, 60],
            [190, 225, 250],
            [255, 255, 255],
        ),
        Sign::WhiteboardA => sp(
            &[
                "SYNERGY", "DISRUPT", "PIVOT", "10X", "AGENTIC", "MOAT", "ALIGN",
            ],
            [244, 246, 248],
            [40, 80, 200],
            [210, 50, 50],
        ),
        Sign::WhiteboardB => sp(
            &[
                "FAIL FAST",
                "SHIP IT",
                "AI+AI=AI",
                "NORTH STAR",
                "PROMPT",
                "VIBES",
                "SCALE",
            ],
            [244, 246, 248],
            [30, 140, 70],
            [200, 80, 30],
        ),
        Sign::WhiteboardC => sp(
            &[
                "IDEA ???",
                "THEN AI",
                "PROFIT",
                "LEVERAGE",
                "PARADIGM",
                "HOCKEY STICK",
                "MVP",
            ],
            [244, 246, 248],
            [120, 50, 170],
            [40, 120, 200],
        ),
        Sign::InnovationHere => sp(
            &["INNOVATION HAPPENS HERE", "(RESULTS MAY VARY)"],
            [240, 244, 250],
            [30, 40, 70],
            [220, 60, 160],
        ),
        Sign::Printing => sp(
            &["PRINTING...", "0%"],
            [10, 14, 22],
            [120, 240, 255],
            [255, 200, 60],
        ),
        Sign::VrDemo => sp(
            &["VR DEMO", "NO ONE ASKED"],
            [40, 20, 70],
            [235, 220, 255],
            [120, 240, 255],
        ),
        Sign::FailFast => sp(
            &["FAIL FAST", "FAIL OFTEN", "FAIL"],
            [235, 240, 245],
            [40, 50, 70],
            [210, 50, 50],
        ),
        Sign::AgiArena => sp(
            &["AGI", "DEFINITELY NOT SENTIENT"],
            [6, 6, 16],
            [120, 255, 240],
            [255, 90, 220],
        ),
        Sign::SeriesB => sp(
            &["SERIES B", "OR BUST"],
            [20, 60, 40],
            [235, 250, 240],
            [250, 220, 90],
        ),
        Sign::ToLab2 => sp(
            &["RESTRICTED", "THE SANCTUM"],
            [14, 10, 30],
            [190, 170, 255],
            [255, 90, 220],
        ),
        Sign::ToLab1 => sp(
            &["BACK TO THE", "OPEN SPACE"],
            [235, 238, 245],
            [40, 50, 70],
            [60, 140, 220],
        ),
        Sign::ClientName => sp(
            &["VERY LARGE INSURANCE", "& BANKING GROUP"],
            [92, 92, 96],
            [226, 222, 208],
            [150, 150, 154],
        ),
        Sign::BadgePending => sp(
            &["BADGE", "PENDING"],
            [196, 190, 176],
            [60, 60, 66],
            [150, 60, 50],
        ),
        Sign::ClientQuiet => sp(
            &["QUIET WORKING", "AREA"],
            [200, 196, 184],
            [70, 70, 76],
            [130, 130, 134],
        ),
        Sign::ClientRoom => sp(
            &["ROOM 4.17", "IN A MEETING", "SINCE MARCH"],
            [86, 90, 98],
            [214, 218, 224],
            [150, 154, 162],
        ),
        Sign::ClientPrinter => sp(
            &["TONER LOW", "SINCE 2009"],
            [204, 200, 188],
            [70, 70, 76],
            [150, 60, 50],
        ),
        Sign::ClientHotDesk => sp(
            &["HOT DESKING", "ALL DESKS FREE"],
            [200, 196, 184],
            [70, 70, 76],
            [130, 130, 134],
        ),
        Sign::ClientStatus => sp(
            &["PROJECT STATUS", "GREEN"],
            [204, 200, 188],
            [70, 70, 76],
            [70, 130, 80],
        ),
        Sign::ClientTimesheet => sp(
            &["TIMESHEETS DUE", "EVERY DAY"],
            [204, 200, 188],
            [70, 70, 76],
            [130, 130, 134],
        ),
        Sign::OffsiteWelcome => sp(
            &["WELCOME TO THE", "ALL-HANDS OFFSITE"],
            [128, 30, 48],
            [250, 240, 220],
            [250, 205, 90],
        ),
        Sign::OffsiteAgenda => sp(
            &[
                "AGENDA",
                "09:00 ICEBREAKER",
                "11:00 ALIGNMENT",
                "14:00 KEYNOTE",
                "19:00 FAMILY DINNER",
            ],
            [244, 238, 224],
            [60, 40, 36],
            [150, 40, 50],
        ),
        Sign::OffsiteNameTags => sp(
            &["HELLO MY NAME IS", "ASK ME ABOUT ALIGNMENT"],
            [250, 250, 250],
            [200, 40, 50],
            [40, 50, 90],
        ),
        Sign::OffsiteShuttle => sp(
            &["ALL-HANDS OFFSITE", "NOW BOARDING"],
            [250, 244, 230],
            [128, 30, 48],
            [30, 120, 150],
        ),
        Sign::OffsiteBar => sp(
            &["OPEN BAR", "2 TOKENS EACH"],
            [14, 8, 20],
            [255, 190, 90],
            [255, 90, 190],
        ),
        Sign::BreakoutRoom => sp(
            &["BREAKOUT ROOM", "PLEASE ALIGN"],
            [236, 240, 244],
            [40, 50, 70],
            [40, 150, 130],
        ),
        Sign::KeynoteScreen => sp(
            &["KEYNOTE", "FUTURE OF WORK", "SLIDE 1 OF 412"],
            [4, 14, 24],
            [120, 245, 255],
            [255, 120, 230],
        ),
        Sign::FamilyBanner => sp(
            &["WE ARE A FAMILY", "NOBODY LEAVES EARLY"],
            [24, 10, 40],
            [255, 225, 160],
            [255, 110, 150],
        ),
        Sign::FamilyTable => sp(
            &["FAMILY TABLE", "SEATS ASSIGNED"],
            [250, 244, 230],
            [90, 30, 40],
            [180, 140, 40],
        ),
        Sign::ToLobby => sp(
            &["BACK TO THE", "LOBBY"],
            [244, 238, 224],
            [60, 40, 36],
            [150, 40, 50],
        ),
        Sign::ToBreakout => sp(
            &["BREAKOUT ROOMS", "MANDATORY FUN"],
            [236, 240, 244],
            [40, 50, 70],
            [40, 150, 130],
        ),
        Sign::ToTerrace => sp(
            &["THE TERRACE", "LAKE VIEW"],
            [30, 70, 90],
            [235, 245, 250],
            [250, 215, 120],
        ),
        Sign::ToBallroom => sp(
            &["GRAND BALLROOM", "FAMILY ONLY"],
            [24, 10, 40],
            [255, 225, 160],
            [255, 110, 150],
        ),
        // VibeCon: black and neon, like every conference that rebranded this year.
        Sign::VibeConBanner => sp(
            &["VIBECON 2026", "SHIP VIBES NOT CODE"],
            [16, 8, 30],
            [120, 245, 255],
            [255, 80, 200],
        ),
        Sign::VibeConRegistration => sp(
            &["REGISTRATION", "BADGES + LANYARDS"],
            [16, 8, 30],
            [250, 250, 250],
            [120, 255, 140],
        ),
        Sign::VibeConLanyards => sp(
            &["LANYARD QUEUE", "WAIT TIME: 40 MIN"],
            [250, 250, 250],
            [30, 20, 50],
            [255, 80, 200],
        ),
        Sign::VibeConBooth => sp(
            &["AGENTFLOWCHAINGRAPH", "FRAMEWORK OF THE WEEK"],
            [30, 30, 40],
            [250, 250, 250],
            [255, 150, 40],
        ),
        Sign::VibeConShipLive => sp(
            &["BOOTH 12: SHIP LIVE", "0 REVIEWS / 41 MERGES"],
            [10, 20, 14],
            [120, 255, 140],
            [255, 90, 90],
        ),
        Sign::VibeConSwag => sp(
            &["FREE SWAG", "STAR THE REPO"],
            [255, 200, 40],
            [30, 20, 50],
            [200, 30, 120],
        ),
        Sign::VibeConWifi => sp(
            &["WIFI: VIBECON-GUEST", "PASSWORD: SEE KEYNOTE"],
            [236, 240, 244],
            [40, 50, 70],
            [200, 40, 120],
        ),
        Sign::VibeConMasterclass => sp(
            &["MASTERCLASS", "PROMPTING IS THE", "NEW PROGRAMMING"],
            [20, 10, 40],
            [235, 225, 255],
            [180, 130, 255],
        ),
        Sign::VibeConWorkshop => sp(
            &["WORKSHOP", "PROMPTS FOR CEOS"],
            [236, 240, 244],
            [40, 30, 70],
            [140, 90, 230],
        ),
        Sign::VibeConHackathon => sp(
            &["HACKATHON", "HOUR 47 OF 48"],
            [20, 20, 24],
            [255, 230, 80],
            [255, 80, 200],
        ),
        Sign::VibeConJudging => sp(
            &["DEMO DAY: LIVE*", "*PRE-RECORDED"],
            [8, 14, 30],
            [120, 245, 255],
            [255, 200, 60],
        ),
        Sign::VibeConAfterparty => sp(
            &["AFTERPARTY", "SPONSORED BY A DATABASE"],
            [24, 6, 36],
            [255, 120, 230],
            [120, 245, 255],
        ),
        Sign::VibeConFireside => sp(
            &["FIRESIDE CHAT", "IS AI OVERHYPED? NO."],
            [30, 12, 8],
            [255, 210, 150],
            [255, 120, 60],
        ),
        Sign::VibeConKeynote => sp(
            &["THIS CHANGES", "EVERYTHING"],
            [4, 4, 10],
            [250, 250, 255],
            [120, 245, 255],
        ),
        Sign::ToExpo => sp(
            &["EXPO HALL", "BOOTHS + SWAG"],
            [16, 8, 30],
            [250, 250, 250],
            [255, 150, 40],
        ),
        Sign::ToWorkshops => sp(
            &["WORKSHOPS", "BRING A PROMPT"],
            [16, 8, 30],
            [250, 250, 250],
            [180, 130, 255],
        ),
        Sign::ToHackathon => sp(
            &["HACKATHON", "CORNER"],
            [16, 8, 30],
            [250, 250, 250],
            [255, 230, 80],
        ),
        Sign::ToAfterparty => sp(
            &["AFTERPARTY", "OPEN BAR 1 TOKEN"],
            [16, 8, 30],
            [250, 250, 250],
            [255, 120, 230],
        ),
        Sign::ToMainStage => sp(
            &["MAIN STAGE", "KEYNOTE AT 5"],
            [16, 8, 30],
            [250, 250, 250],
            [120, 245, 255],
        ),
        Sign::Calendar => sp(
            &[
                "COMPANY CALENDAR",
                "MINIGAMES  QUEST CHAINS",
                "EVERYTHING, ALL WEEK",
            ],
            [22, 26, 38],
            [245, 245, 250],
            [255, 90, 80],
        ),
        Sign::TbWelcome => sp(
            &["WELCOME TEAM!", "FUN STARTS AT 4:00"],
            [250, 244, 232],
            [40, 30, 50],
            [230, 70, 120],
        ),
        Sign::TbBuffet => sp(
            &["BUFFET", "ONE CANAPE EACH"],
            [250, 244, 232],
            [60, 40, 30],
            [200, 140, 40],
        ),
        Sign::TbVip => sp(
            &["VIP", "C-SUITE ONLY"],
            [20, 16, 24],
            [240, 210, 120],
            [240, 210, 120],
        ),
        Sign::TbQuiet => sp(
            &["QUIET CORNER", "1 BAR OF SIGNAL"],
            [236, 240, 244],
            [40, 50, 70],
            [60, 150, 90],
        ),
        Sign::TbTrustFall => sp(
            &["TRUST FALL", "CATCHING OPTIONAL"],
            [250, 244, 232],
            [40, 30, 50],
            [60, 120, 220],
        ),
        Sign::TbIcebreaker => sp(
            &["ICEBREAKER", "2 TRUTHS + 1 LIE"],
            [250, 244, 232],
            [40, 30, 50],
            [230, 90, 40],
        ),
        Sign::TbPhoto => sp(
            &["TEAM PHOTO!", "SAY 'SYNERGY'"],
            [230, 70, 120],
            [255, 255, 255],
            [255, 220, 90],
        ),
        Sign::TbToilets => sp(
            &["TOILETS", "QUEUE: 14 MIN"],
            [236, 240, 244],
            [40, 50, 70],
            [60, 120, 220],
        ),
        Sign::TbSwag => sp(
            &["FREE SWAG", "TAKE ONE (TAKE 5)"],
            [250, 244, 232],
            [40, 30, 50],
            [230, 70, 120],
        ),
        Sign::TbBanner => sp(
            &["FUN IS A", "DELIVERABLE"],
            [30, 60, 140],
            [255, 255, 255],
            [255, 220, 90],
        ),
        Sign::TbKaraoke => sp(
            &["KARAOKE", "NEXT: YOUR MANAGER"],
            [16, 8, 30],
            [250, 250, 250],
            [255, 120, 230],
        ),
        Sign::TbTaxi => sp(
            &["TAXI RANK", "EXIT >"],
            [250, 210, 40],
            [20, 20, 20],
            [20, 20, 20],
        ),
        Sign::ReviewRoom => sp(
            &["MEETING ROOM 4B", "BOOKED UNTIL Q4"],
            [236, 236, 240],
            [50, 56, 70],
            [90, 120, 200],
        ),
        Sign::ReviewScale => sp(
            &["1  2  3  4  5", "3 IS THE NEW 5"],
            [24, 30, 44],
            [230, 230, 236],
            [250, 200, 70],
        ),
        Sign::ReorgChart => sp(
            &["ORG CHART V14", "SUBJECT TO CHANGE"],
            [240, 240, 244],
            [40, 46, 60],
            [210, 70, 70],
        ),
        Sign::ReorgExit => sp(
            &["FIRE EXIT", "ALSO A HOT DESK"],
            [30, 140, 70],
            [250, 250, 250],
            [250, 250, 250],
        ),
        Sign::MigrationOld => sp(
            &["TICKETRON 2009", "DO NOT TURN OFF"],
            [12, 20, 12],
            [90, 240, 110],
            [90, 240, 110],
        ),
        Sign::MigrationNew => sp(
            &["FLOWSPHERE CLOUD", "THE FUTURE OF WORK"],
            [245, 248, 255],
            [40, 60, 110],
            [60, 150, 240],
        ),
        Sign::MigrationDeadline => sp(
            &["MIGRATION DONE BY", "FRIDAY. WHICH ONE?"],
            [250, 230, 120],
            [40, 30, 20],
            [180, 40, 30],
        ),
        Sign::HackBanner => sp(
            &["HACKATHON 24H", "AI FOR GOOD (FOR BIZ)"],
            [20, 16, 40],
            [250, 250, 250],
            [255, 120, 60],
        ),
        Sign::HackStage => sp(
            &["DEMO DAY", "IT WORKED AN HOUR AGO"],
            [30, 30, 36],
            [250, 250, 250],
            [90, 200, 255],
        ),
        Sign::HackPizza => sp(
            &["FREE PIZZA", "COLD BY 2 AM"],
            [250, 244, 232],
            [60, 30, 20],
            [220, 80, 40],
        ),
        Sign::OpenQuiet => sp(
            &["QUIET ZONE", "OPEN PLAN"],
            [236, 240, 236],
            [50, 60, 50],
            [70, 150, 90],
        ),
        Sign::OpenCollab => sp(
            &["COLLABORATE", "SPONTANEOUSLY"],
            [255, 210, 70],
            [40, 30, 20],
            [200, 60, 40],
        ),
        Sign::LabRules => sp(
            &["COMPUTE IS OUR MOST", "VALUABLE EMPLOYEE"],
            [235, 238, 245],
            [40, 50, 70],
            [60, 140, 220],
        ),
        Sign::HypeWall => sp(
            &["PIVOTS THIS QUARTER", "SEVEN", "(ALL ARE STRATEGIC)"],
            [250, 244, 232],
            [60, 60, 80],
            [210, 60, 60],
        ),
        Sign::Kombucha => sp(
            &["KOMBUCHA", "ON TAP", "NOW WITH AI"],
            [30, 70, 50],
            [240, 245, 220],
            [250, 200, 70],
        ),
        Sign::MoveFast => sp(
            &["MOVE FAST", "AND BREAK PROD"],
            [14, 8, 24],
            [255, 90, 210],
            [90, 240, 255],
        ),
        Sign::AgiRoom => sp(
            &["AGI ROOM", "PLEASE DO NOT", "KNOCK"],
            [18, 22, 40],
            [210, 220, 245],
            [255, 90, 70],
        ),
        Sign::ProtoGraveyard => sp(
            &["PROTOTYPE", "GRAVEYARD", "41 DEMOS, 0 SHIPPED"],
            [40, 40, 46],
            [210, 210, 215],
            [230, 170, 70],
        ),
        Sign::DemoDay => sp(
            &["DEMO DAY", "LIVE (PRERECORDED)"],
            [60, 20, 40],
            [255, 235, 200],
            [250, 200, 70],
        ),
        Sign::RobotArm => sp(
            &["ROBOT ARM V3", "AWAITING", "REQUIREMENTS"],
            [236, 238, 244],
            [50, 60, 80],
            [210, 80, 40],
        ),
        Sign::LabNda => sp(
            &["PLEASE SIGN", "THE NDA", "WE HAVE NOTHING"],
            [236, 238, 244],
            [50, 60, 80],
            [210, 80, 40],
        ),
        Sign::ClientValues => sp(
            &[
                "OUR VALUES",
                "PROCESS",
                "ACCOUNTABILITY",
                "COMPLIANCE",
                "SEE POLICY 4.2.1",
            ],
            [214, 208, 192],
            [64, 64, 72],
            [120, 120, 126],
        ),
        Sign::ClientCoffee => sp(
            &["OUT OF ORDER", "SEE TICKET 40412"],
            [226, 222, 208],
            [70, 70, 76],
            [150, 60, 50],
        ),
        Sign::ClientPlaque => sp(
            &["UNTITLED NO. 3", "LOAN FROM FACILITIES"],
            [150, 150, 154],
            [232, 228, 214],
            [190, 186, 174],
        ),
        Sign::ClientEvac => sp(
            &["ASSEMBLY POINT", "THE LOBBY", "(ALSO CLOSED)"],
            [204, 200, 188],
            [70, 70, 76],
            [70, 130, 80],
        ),
        Sign::ParkName => sp(
            &["WELLNESS PARK", "DO NOT APPROACH", "THE GOOSE"],
            [40, 80, 50],
            [235, 245, 230],
            [250, 210, 90],
        ),
        Sign::AssemblyPoint => sp(
            &["FIRE ASSEMBLY POINT", "SURPRISE DRILLS", "TWICE AN HOUR"],
            [30, 120, 60],
            [245, 250, 245],
            [255, 230, 120],
        ),
        Sign::FoodTrucks => sp(
            &["FOOD TRUCK PLAZA", "NOW WITH AI TACOS"],
            [200, 90, 30],
            [255, 245, 225],
            [255, 220, 90],
        ),
        Sign::IncubatorStreet => sp(
            &["INCUBATOR STREET", "PRE-REVENUE ZONE"],
            [24, 16, 40],
            [140, 240, 255],
            [255, 110, 220],
        ),
        Sign::CoworkCafe => sp(
            &["HOT DESK CAFE", "BRING YOUR OWN DESK"],
            [60, 45, 35],
            [245, 235, 215],
            [120, 220, 160],
        ),
        Sign::GymName => sp(
            &["CORPORATE WELLNESS", "PARTICIPATION", "IS TRACKED"],
            [20, 30, 45],
            [230, 240, 250],
            [120, 255, 140],
        ),
        Sign::StationName => sp(
            &["SYNERGY CAMPUS", "STATION"],
            [20, 40, 100],
            [240, 245, 255],
            [250, 210, 60],
        ),
        Sign::NewHq => sp(
            &["NEW HQ", "OPENING 2019"],
            [235, 235, 230],
            [30, 40, 60],
            [210, 50, 40],
        ),
        Sign::Garage => sp(
            &["AUTONOMOUS GARAGE", "IT REMEMBERS NOTHING"],
            [30, 34, 40],
            [220, 225, 235],
            [250, 180, 60],
        ),
        Sign::CampusShuttle => sp(
            &["CAMPUS SHUTTLE", "SELF-DRIVING", "(MOSTLY)"],
            [20, 90, 130],
            [240, 248, 255],
            [255, 220, 90],
        ),
        Sign::StepBoard => sp(
            &["STEP LEADERBOARD", "1 CEILING FAN 41002", "LAST: YOU"],
            [10, 14, 20],
            [120, 255, 160],
            [255, 90, 80],
        ),
        Sign::DepartureBoard => sp(
            &[
                "08:12 NEW HQ  DELAYED",
                "08:14 CITY  CANCELLED",
                "08:20 RTO  ON TIME?",
            ],
            [10, 10, 14],
            [255, 200, 60],
            [255, 90, 60],
        ),
        Sign::TruckMenu => sp(
            &["TACO, CONCEPTUALLY", "PROMPT BOWL", "TODAY: FREE PIZZA?"],
            [25, 25, 28],
            [245, 245, 240],
            [255, 200, 70],
        ),
        Sign::PitchBooth => sp(
            &["UBER FOR UBERS", "SEED ROUND OPEN"],
            [16, 10, 30],
            [255, 120, 230],
            [120, 240, 255],
        ),
        Sign::HardHat => sp(
            &["HARD HAT AREA", "LEADERSHIP EXEMPT"],
            [250, 200, 40],
            [20, 20, 20],
            [180, 30, 30],
        ),
        Sign::Creek => sp(
            &["LIQUIDITY CREEK", "RUNS DRY IN Q4", "NO SWIMMING"],
            [30, 70, 110],
            [230, 245, 255],
            [255, 220, 90],
        ),
        Sign::JoggingLoop => sp(
            &[
                "JOGGING LOOP 2.1 KM",
                "YOUR PACE IS SHARED",
                "WITH YOUR MANAGER",
            ],
            [40, 90, 60],
            [240, 250, 240],
            [255, 230, 120],
        ),
        Sign::OverflowLot => sp(
            &["OVERFLOW LOT B", "FULL SINCE 2022", "TRY CAR PARK P3"],
            [30, 34, 40],
            [230, 232, 236],
            [250, 180, 60],
        ),
        Sign::OldCampus => sp(
            &["OLD CAMPUS", "EST. 1998", "LEGACY: DO NOT REWRITE"],
            [90, 40, 30],
            [245, 230, 210],
            [255, 200, 90],
        ),
        Sign::LegacyDepot => sp(
            &[
                "LEGACY SYSTEMS DEPOT",
                "IT WORKS. NOBODY",
                "KNOWS WHY. DO NOT TOUCH",
            ],
            [60, 40, 30],
            [240, 225, 200],
            [255, 170, 60],
        ),
        Sign::Building1 => sp(
            &["BUILDING 1", "WHERE IT ALL BEGAN", "NOW: STORAGE"],
            [70, 30, 25],
            [240, 228, 210],
            [230, 200, 120],
        ),
        Sign::FoundersTerrace => sp(
            &[
                "FOUNDERS TERRACE",
                "THE ORIGINAL GARAGE",
                "(RENTED. BY THE HOUR)",
            ],
            [40, 40, 50],
            [240, 236, 220],
            [255, 210, 90],
        ),
        Sign::CoLiving => sp(
            &[
                "CO-LIVING COMMONS",
                "COMMUTE: 40 SECONDS",
                "RENT: ALL OF IT",
            ],
            [30, 60, 70],
            [235, 248, 250],
            [255, 200, 120],
        ),
        Sign::Bus9 => sp(
            &["BUS 9  CAMPUS LOOP", "EVERY 40 MIN", "(IN THEORY)"],
            [20, 60, 120],
            [240, 245, 255],
            [255, 220, 90],
        ),
        Sign::ExecHill => sp(
            &["EXECUTIVE HILL", "PRIVATE ROAD", "VISION HOLDERS ONLY"],
            [20, 20, 24],
            [235, 215, 150],
            [255, 240, 200],
        ),
        Sign::Villa => sp(
            &["THE RETREAT", "BOARD OFFSITE", "IN SESSION SINCE 2015"],
            [245, 245, 240],
            [40, 40, 48],
            [180, 140, 40],
        ),
        Sign::SouthGate => sp(
            &["SYNERGY CAMPUS", "SOUTH GATE", "TAILGATING IS TRACKED"],
            [20, 40, 70],
            [240, 245, 255],
            [120, 220, 255],
        ),
        Sign::ToCarPark => sp(
            &["CAR PARK P3", "2.4 KM THAT WAY", "SHUTTLE: CANCELLED"],
            [30, 34, 40],
            [235, 238, 242],
            [255, 200, 70],
        ),
        Sign::Boulevard => sp(
            &[
                "SYNERGY BOULEVARD",
                "NO CARS. NO BIKES.",
                "VIBES: ENCOURAGED",
            ],
            [40, 40, 70],
            [240, 240, 255],
            [255, 120, 220],
        ),
        Sign::ExecParking => sp(
            &[
                "EXECUTIVE PARKING",
                "RESERVED FOR VISION",
                "VIOLATORS PROMOTED",
            ],
            [20, 20, 24],
            [235, 215, 150],
            [255, 90, 80],
        ),
        Sign::Padel => sp(
            &["LEADERSHIP PADEL", "BOOKED UNTIL 2031"],
            [20, 60, 120],
            [240, 248, 255],
            [255, 230, 90],
        ),
        Sign::LunchLawn => sp(
            &["LUNCH LAWN", "EAT AT YOUR DESK", "THIS IS A DESK"],
            [40, 90, 40],
            [245, 250, 235],
            [255, 220, 90],
        ),
        Sign::WeWash => sp(
            &["WEWASH", "LAUNDRY AS A SERVICE", "SPIN CYCLE: SERIES A"],
            [245, 245, 250],
            [40, 60, 120],
            [220, 60, 140],
        ),
        Sign::CarParkP3 => sp(
            &["CAR PARK P3", "EMPLOYEE PARKING", "CAMPUS: 2.4 KM NORTH"],
            [30, 34, 40],
            [235, 238, 242],
            [255, 200, 70],
        ),
        Sign::BadgeOffice => sp(
            &["BADGE OFFICE", "NOW SERVING: 3", "YOU ARE: 4112"],
            [200, 200, 195],
            [30, 40, 60],
            [200, 40, 40],
        ),
        Sign::PayDisplay => sp(
            &[
                "PAY AND DISPLAY",
                "EMPLOYEES TOO",
                "THANK YOU FOR YOUR LOYALTY",
            ],
            [20, 70, 140],
            [240, 245, 255],
            [255, 220, 90],
        ),
        // The Onboarding: pastel and cheerful, the dread is in the small print.
        Sign::ObNewStarters => sp(
            &[
                "NEW STARTERS - WEEK 1",
                "(door may be locked, please",
                "wait for someone to let you in)",
            ],
            [214, 232, 226],
            [40, 70, 80],
            [220, 110, 90],
        ),
        Sign::ObWelcome => sp(
            &["WELCOME, DANIELA", "(THE PREVIOUS HIRE)"],
            [226, 238, 250],
            [40, 60, 100],
            [230, 120, 100],
        ),
        Sign::ObQueue => sp(
            &["PLEASE QUEUE HERE", "(THE QUEUE IS THE WALL)"],
            [250, 236, 214],
            [90, 60, 40],
            [200, 90, 70],
        ),
        Sign::ObCage => sp(
            &["EQUIPMENT CAGE", "1 LAPTOP PER 10 BOXES"],
            [220, 226, 232],
            [50, 60, 80],
            [210, 110, 60],
        ),
        Sign::ObWeek => sp(
            &["WEEK 1  WEEK 2", "WEEK 3  WEEK 4", "WEEK 5  WEEK 6"],
            [250, 248, 240],
            [60, 60, 80],
            [210, 50, 50],
        ),
        Sign::ObNameplate => sp(
            &["[NEW STARTER]", "(in marker)"],
            [250, 246, 226],
            [60, 60, 70],
            [120, 150, 200],
        ),
        Sign::ObCulture => sp(
            &["CULTURE DECK", "IGNORE PAGE 4"],
            [255, 236, 130],
            [70, 60, 40],
            [210, 90, 60],
        ),
        Sign::ObToReception => sp(
            &["BACK TO", "RECEPTION"],
            [226, 238, 250],
            [40, 60, 100],
            [230, 120, 100],
        ),
        Sign::ObToCage => sp(
            &["EQUIPMENT CAGE", "WEEK 2 (APPROX.)"],
            [220, 226, 232],
            [50, 60, 80],
            [210, 110, 60],
        ),
        Sign::ObToDesk => sp(
            &["YOUR DESK", "HOT DESK 14"],
            [250, 246, 226],
            [60, 60, 70],
            [120, 150, 200],
        ),
        // The Migration: a cold basement, hazard yellow and "cloud" blue; the dread is in the small print.
        Sign::MigDoor => sp(
            &[
                "BASEMENT B",
                "AUTHORISED PERSONS ONLY",
                "(ALSO EVERYONE WITH A TICKET)",
            ],
            [230, 200, 40],
            [30, 30, 30],
            [170, 30, 30],
        ),
        Sign::MigCloud => sp(
            &["WE ARE GOING", "TO THE CLOUD"],
            [40, 120, 210],
            [255, 255, 255],
            [255, 220, 90],
        ),
        Sign::MigPlan => sp(
            &[
                "MIGRATION PLAN V14",
                "1 DISCOVER",
                "2 ASSESS  3-13 SEE 1",
                "14 ???",
            ],
            [244, 246, 240],
            [40, 50, 80],
            [210, 50, 50],
        ),
        Sign::MigTickets => sp(
            &[
                "MIG-1 BLOCKED BY MIG-2",
                "MIG-2 BLOCKED BY MIG-1",
                "MIG-3 WONT FIX",
                "MIG-4 SEE MIG-1",
            ],
            [226, 214, 150],
            [60, 50, 30],
            [190, 60, 40],
        ),
        Sign::MigDock => sp(
            &["LOADING DOCK B", "MIND THE LEGACY"],
            [60, 66, 74],
            [240, 240, 235],
            [255, 200, 40],
        ),
        Sign::MigLegacy => sp(
            &["LEGACY", "DO NOT STACK", "(STACKED)"],
            [226, 226, 220],
            [40, 40, 50],
            [190, 40, 40],
        ),
        Sign::MigLift => sp(
            &["LIFT AND SHIFT", "THIS WAY UP", "(WHICH?)"],
            [236, 150, 40],
            [30, 24, 20],
            [30, 24, 20],
        ),
        Sign::MigLeak => sp(
            &["WET FLOOR", "(IT IS THE CLOUD)"],
            [250, 220, 40],
            [30, 30, 30],
            [30, 90, 190],
        ),
        Sign::MigToDock => sp(
            &["LOADING DOCK", "(WHERE IT STARTED)"],
            [60, 66, 74],
            [240, 240, 235],
            [255, 200, 40],
        ),
        Sign::MigToAisle => sp(
            &["COLD AISLE", "BRING A JACKET"],
            [14, 40, 70],
            [180, 230, 255],
            [90, 200, 255],
        ),
        Sign::MigToCage => sp(
            &["MAINFRAME CAGE", "AUTHORISED ONLY"],
            [60, 56, 48],
            [235, 225, 200],
            [255, 190, 60],
        ),
        Sign::MigToWar => sp(
            &["CUTOVER", "WAR ROOM"],
            [60, 10, 10],
            [255, 220, 210],
            [255, 70, 50],
        ),
        Sign::MigToDay => sp(
            &["BACK TO DAYLIGHT", "(VISITING HOURS)"],
            [230, 200, 40],
            [30, 30, 30],
            [170, 30, 30],
        ),
        Sign::MigCage => sp(
            &["MAINFRAME CAGE", "PLEASE CLOSE THE GATE", "(IT CLOSES YOU)"],
            [70, 64, 54],
            [240, 230, 205],
            [255, 190, 60],
        ),
        Sign::MigRates => sp(
            &[
                "RATE CARD",
                "PARTNER: A LOT / HR",
                "MANAGER: QUITE A LOT",
                "YOU: BILLED",
            ],
            [248, 248, 244],
            [30, 40, 70],
            [190, 40, 40],
        ),
        Sign::MigLabelA => sp(
            &["SRV-03", "(IS SRV-07)"],
            [250, 244, 200],
            [40, 40, 40],
            [190, 50, 40],
        ),
        Sign::MigLabelB => sp(
            &["PROD", "(IS TEST)"],
            [250, 244, 200],
            [40, 40, 40],
            [190, 50, 40],
        ),
        Sign::MigClock => sp(
            &["T-MINUS 00:10:00", "(FOR 11 HOURS)"],
            [18, 4, 4],
            [255, 70, 50],
            [255, 200, 60],
        ),
        Sign::MigNoRollback => sp(
            &["NO ROLLBACK PLAN", "ONLY FORWARD"],
            [24, 20, 20],
            [255, 240, 220],
            [255, 70, 50],
        ),
        Sign::MigHypercare => sp(
            &["HYPERCARE", "UNTIL TUESDAY"],
            [20, 40, 60],
            [200, 235, 255],
            [255, 200, 60],
        ),
        Sign::MigDash => sp(
            &["DASHBOARD: GREEN", "(DO NOT LOOK)"],
            [10, 50, 24],
            [150, 255, 160],
            [255, 240, 120],
        ),
        Sign::MigCutover => sp(
            &[
                "CUTOVER PLAN",
                "FRI 18:00 GO LIVE",
                "SAT DATA SYNC 99%",
                "SUN HYPERCARE",
            ],
            [244, 246, 240],
            [40, 50, 80],
            [210, 50, 50],
        ),
        Sign::MigGhost => sp(
            &["V1.9", "(STABLE)"],
            [150, 200, 230],
            [20, 40, 70],
            [60, 110, 170],
        ),
        Sign::MigHeart => sp(
            &["HEARTBEAT", "60 BPM (FLAT)"],
            [10, 24, 30],
            [120, 255, 200],
            [255, 90, 90],
        ),
        Sign::P3Shuttle => sp(
            &["P3 SHUTTLE", "SUSPENDED", "SINCE THE LAST REORG"],
            [20, 90, 130],
            [240, 248, 255],
            [255, 90, 80],
        ),
        Sign::ToCampusGate => sp(
            &["TO CAMPUS", "WALKING IS WELLNESS", "(MANDATORY)"],
            [30, 120, 60],
            [245, 250, 245],
            [255, 230, 120],
        ),
        Sign::FirstDay => sp(
            &["FIRST DAY?", "ONBOARDING THIS WAY", "SMILE FOR THE BADGE"],
            [220, 90, 40],
            [255, 245, 230],
            [255, 230, 120],
        ),
        Sign::ClampWarning => sp(
            &["WHEEL CLAMPING", "IN OPERATION", "BY THE CLAMP DRONES"],
            [250, 200, 40],
            [20, 20, 20],
            [180, 30, 30],
        ),
        Sign::AgentsAd => sp(
            &["REPLACE YOUR TEAM", "WITH AGENTS", "NOW 30% FEWER PEOPLE"],
            [90, 40, 200],
            [245, 242, 255],
            [120, 255, 200],
        ),
        Sign::FamilyAd => sp(
            &["WE'RE A FAMILY", "UNLIMITED PTO*", "*LAYOFFS THURSDAY"],
            [240, 120, 150],
            [255, 250, 250],
            [90, 20, 40],
        ),
        Sign::RtoAd => sp(
            &["RETURN TO OFFICE", "THE OFFICE IS", "2.4 KM THAT WAY"],
            [40, 60, 90],
            [235, 240, 248],
            [255, 200, 70],
        ),
    }
}

fn draw_sign(s: Sign) -> Canvas {
    let spec = sign_spec(s);
    let (w, h) = sign_canvas(s);
    let mut c = Canvas::new(w, h, spec.bg);
    let (wi, hi) = (w as i32, h as i32);
    if matches!(s, Sign::WhiteboardA | Sign::WhiteboardB | Sign::WhiteboardC) {
        c.frame(2, 2, wi - 4, hi - 4, 3, [150, 150, 158]);
        let seed = s as u32 * 31;
        for (i, word) in spec.lines.iter().enumerate() {
            let col = if i % 2 == 0 { spec.fg } else { spec.hi };
            let (col_i, row_i) = ((i % 2) as i32, (i / 2) as i32);
            let x = 10 + col_i * 92 + (hash(seed + i as u32, 3) * 14.0) as i32;
            let y = 12 + row_i * 28 + (hash(seed + i as u32, 5) * 6.0) as i32;
            let scale = if word.len() > 8 { 1 } else { 2 };
            let tw = c.text(x, y, scale, word, col);
            c.rect(x, y + 7 * scale + 2, tw, 1, col);
        }
        // Arrows between the words, because every whiteboard has arrows.
        c.line((60.0, 40.0), (110.0, 66.0), [60, 60, 70]);
        c.line((104.0, 62.0), (110.0, 66.0), [60, 60, 70]);
        c.line((140.0, 100.0), (90.0, 112.0), [60, 60, 70]);
        return c;
    }
    c.frame(3, 3, wi - 6, hi - 6, 2, spec.hi);
    let n = spec.lines.len() as i32;
    let slot = (hi - 12) / n;
    for (i, line) in spec.lines.iter().enumerate() {
        let len = line.chars().count() as i32;
        let fit = (wi - 20) / (6 * len).max(1);
        let scale = fit
            .clamp(1, if n == 1 { 6 } else { 4 })
            .min((slot - 4) / 7)
            .max(1);
        let col = if i as i32 == n - 1 { spec.hi } else { spec.fg };
        let y = 6 + i as i32 * slot + (slot - 7 * scale) / 2;
        c.text_c(y, scale, line, col);
    }
    c
}

pub fn tex_image(tex: Tex) -> Image {
    draw(tex).into_image()
}

fn draw(tex: Tex) -> Canvas {
    match tex {
        Tex::Sign(s) => draw_sign(s),
        Tex::Blinds => {
            let mut c = Canvas::new(96, 64, [206, 210, 212]);
            // Slats down to two thirds, each with a shadowed lower edge; one hangs crooked.
            for k in 0..11 {
                let y = 2 + k * 4;
                c.rect(2, y, 92, 3, [200, 192, 176]);
                c.rect(2, y + 3, 92, 1, [146, 138, 122]);
            }
            c.line((2.0, 22.0), (93.0, 25.0), [146, 138, 122]);
            c.rect(2, 46, 92, 2, [176, 168, 152]);
            c.rect(88, 46, 1, 12, [120, 116, 104]);
            c.rect(86, 58, 5, 3, [120, 116, 104]);
            c.frame(0, 0, 96, 64, 2, [110, 108, 100]);
            c
        }
        Tex::CorpArt => {
            let mut c = Canvas::new(64, 48, [224, 218, 202]);
            c.rect(6, 6, 52, 36, [236, 230, 216]);
            c.rect(10, 22, 30, 16, [188, 168, 128]);
            c.rect(30, 10, 22, 14, [128, 146, 156]);
            c.disc(46.0, 30.0, 6.0, [150, 124, 100]);
            c.rect(10, 18, 44, 1, [110, 110, 110]);
            // Dust on the glass.
            for i in 0..20u32 {
                c.set(
                    (hash(i, 2) * 64.0) as i32,
                    (hash(i, 8) * 48.0) as i32,
                    [244, 242, 236],
                );
            }
            c.frame(0, 0, 64, 48, 2, [120, 114, 104]);
            c
        }
        Tex::RackBlades
        | Tex::RackPatch
        | Tex::RackDrives
        | Tex::RackBack
        | Tex::RackScorched
        | Tex::RackBackHot
        | Tex::NocGraph
        | Tex::NocAlerts
        | Tex::NocRegions => dc_art::draw(tex),
        Tex::PosterBars => {
            let mut c = Canvas::new(64, 80, [22, 46, 92]);
            for i in 0..5 {
                let bh = 10 + i * 9;
                c.rect(
                    6 + i * 11,
                    80 - 12 - bh,
                    8,
                    bh,
                    [90 + i as u8 * 30, 200, 120],
                );
            }
            c.line((6.0, 52.0), (58.0, 10.0), [250, 220, 60]);
            c.rect(6, 4, 52, 3, WHITE);
            c
        }
        Tex::PosterBurn => {
            let mut c = Canvas::new(64, 80, [244, 238, 220]);
            c.rect(8, 8, 2, 62, [40, 40, 40]);
            c.rect(8, 68, 50, 2, [40, 40, 40]);
            c.line((10.0, 14.0), (58.0, 68.0), [150, 150, 150]);
            c.line((10.0, 16.0), (44.0, 18.0), RED);
            c.line((44.0, 18.0), (58.0, 67.0), RED);
            c
        }
        Tex::PosterHang => {
            let mut c = Canvas::new(64, 80, [150, 205, 235]);
            c.rect(0, 14, 64, 4, [110, 75, 40]);
            c.line((24.0, 18.0), (28.0, 40.0), [60, 60, 60]);
            c.line((40.0, 18.0), (36.0, 40.0), [60, 60, 60]);
            c.disc(32.0, 52.0, 14.0, [250, 215, 70]);
            c.disc(27.0, 49.0, 2.0, [40, 40, 40]);
            c.disc(37.0, 49.0, 2.0, [40, 40, 40]);
            c.rect(27, 58, 10, 2, [40, 40, 40]);
            c
        }
        Tex::PosterPie => {
            let mut c = Canvas::new(64, 80, [40, 36, 48]);
            for y in 0..80 {
                for x in 0..64 {
                    let (dx, dy) = (x as f32 - 32.0, y as f32 - 36.0);
                    if dx.hypot(dy) <= 24.0 {
                        let a =
                            dy.atan2(dx).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
                        let col = match a {
                            a if a < 0.62 => [200, 70, 70],
                            a if a < 0.9 => [70, 150, 210],
                            _ => [230, 200, 80],
                        };
                        c.set(x, y, col);
                    }
                }
            }
            c.rect(8, 68, 48, 3, [230, 230, 230]);
            c
        }
        Tex::AiFirst => {
            let mut c = Canvas::new(192, 128, [18, 24, 48]);
            c.frame(4, 4, 184, 120, 2, [250, 200, 60]);
            c.text_c(16, 4, "AI FIRST.", [250, 220, 60]);
            c.text_c(56, 2, "HUMANS SECOND.", WHITE);
            c.text_c(86, 2, "COFFEE THIRD.", [215, 140, 70]);
            c
        }
        Tex::Synergy => {
            let mut c = Canvas::new(128, 160, [20, 90, 110]);
            c.vgradient([20, 70, 110], [30, 140, 130]);
            c.line((64.0, 24.0), (64.0, 38.0), [200, 200, 210]);
            c.disc(64.0, 22.0, 5.0, RED);
            c.rect(40, 38, 48, 38, [185, 195, 205]);
            c.disc(54.0, 54.0, 7.0, [250, 220, 60]);
            c.disc(74.0, 54.0, 7.0, [250, 220, 60]);
            c.rect(52, 66, 24, 3, BLACK);
            c.rect(34, 80, 60, 36, [150, 160, 175]);
            c.rect(26, 84, 8, 28, [150, 160, 175]);
            c.rect(94, 84, 8, 28, [150, 160, 175]);
            c.text_c(128, 2, "SYNERGY", WHITE);
            c
        }
        Tex::AiTransformation => {
            let mut c = Canvas::new(208, 96, [30, 14, 60]);
            c.text_c(10, 5, "AI", [120, 240, 255]);
            c.text_c(56, 2, "TRANSFORMATION", [255, 120, 220]);
            c.line((20.0, 84.0), (190.0, 84.0), [255, 200, 60]);
            c
        }
        Tex::Flowchart => {
            let mut c = Canvas::new(192, 128, [244, 246, 248]);
            c.text(8, 8, 1, "STRATEGY", [60, 60, 70]);
            for (x, label, col) in [
                (10, "IDEA", [60, 110, 210]),
                (72, "???", RED),
                (136, "AI", [40, 150, 80]),
            ] {
                c.frame(x, 38, 48, 30, 2, col);
                c.text(
                    x + (48 - label.len() as i32 * 12) / 2 + 1,
                    46,
                    2,
                    label,
                    col,
                );
            }
            for x in [58, 120] {
                c.line((x as f32, 53.0), (x as f32 + 12.0, 53.0), [60, 60, 70]);
                c.line(
                    (x as f32 + 8.0, 49.0),
                    (x as f32 + 12.0, 53.0),
                    [60, 60, 70],
                );
                c.line(
                    (x as f32 + 8.0, 57.0),
                    (x as f32 + 12.0, 53.0),
                    [60, 60, 70],
                );
            }
            c.text(10, 90, 1, "PROFIT", [150, 150, 160]);
            c.line((8.0, 94.0), (50.0, 94.0), RED);
            c
        }
        Tex::DaysCounter => {
            let mut c = Canvas::new(160, 64, [12, 8, 8]);
            c.frame(2, 2, 156, 60, 2, [90, 20, 20]);
            c.text_c(8, 1, "DAYS SINCE LAST AI PIVOT", [255, 180, 60]);
            c.text_c(22, 5, "0", [255, 50, 40]);
            c
        }
        Tex::MandatoryFun => {
            let mut c = Canvas::new(256, 48, [200, 40, 150]);
            for i in 0..40 {
                let (x, y) = ((hash(i, 1) * 250.0) as i32, (hash(i, 2) * 44.0) as i32);
                let col = [[250, 220, 60], [80, 220, 250], [250, 250, 250]][(i % 3) as usize];
                c.rect(x, y, 4, 4, col);
            }
            c.rect(40, 10, 176, 28, [90, 20, 70]);
            c.text_c(17, 2, "MANDATORY FUN", [255, 240, 120]);
            c
        }
        Tex::Gantt => {
            let mut c = Canvas::new(256, 128, [244, 244, 240]);
            for x in (0..256).step_by(32) {
                c.rect(x, 0, 1, 128, [200, 200, 200]);
            }
            for row in 0..9 {
                let y = 10 + row * 13;
                c.rect(4, y + 2, 40, 6, [170, 170, 180]);
                let start = 50 + (row * 17) % 90;
                let len = 60 + (row * 23) % 100;
                c.rect(start, y, len.min(250 - start), 10, RED);
                c.rect(start, y, len.min(250 - start), 2, [140, 20, 20]);
            }
            c.rect(150, 0, 2, 128, [40, 90, 220]);
            c.rect(0, 0, 256, 6, [60, 60, 70]);
            c
        }
        Tex::Stickies => {
            let mut c = Canvas::new(128, 96, [246, 247, 250]);
            let cols = [
                [250, 220, 70],
                [250, 140, 190],
                [130, 220, 140],
                [120, 190, 250],
            ];
            for i in 0..14u32 {
                let (x, y) = (6 + (i % 5) as i32 * 24, 6 + (i / 5) as i32 * 30);
                let jitter = (hash(i, 9) * 4.0) as i32;
                c.rect(x + jitter, y, 18, 18, cols[(i % 4) as usize]);
                c.rect(x + jitter + 3, y + 5, 12, 1, [90, 90, 90]);
                c.rect(x + jitter + 3, y + 10, 8, 1, [90, 90, 90]);
            }
            c
        }
        Tex::PromptTips => {
            let mut c = Canvas::new(96, 48, [250, 240, 205]);
            c.frame(1, 1, 94, 46, 1, [180, 120, 60]);
            c.text_c(8, 2, "PROMPT", [60, 40, 20]);
            c.text_c(28, 2, "TIPS", [60, 40, 20]);
            c
        }
        Tex::OutOfOrder => {
            let mut c = Canvas::new(96, 64, [250, 250, 245]);
            c.frame(1, 1, 94, 62, 3, RED);
            c.text_c(8, 2, "OUT OF", RED);
            c.text_c(26, 2, "ORDER", RED);
            c.text_c(48, 1, "SINCE Q3", [90, 90, 90]);
            c
        }
        Tex::HrDoor => {
            let mut c = Canvas::new(192, 32, [120, 100, 70]);
            c.text_c(9, 2, "HUMAN RESOURCES", [250, 245, 230]);
            c
        }
        Tex::Family => {
            let mut c = Canvas::new(128, 160, [250, 205, 215]);
            c.frame(4, 4, 120, 152, 3, [200, 120, 150]);
            c.text_c(24, 3, "WE'RE", [150, 40, 80]);
            c.text_c(52, 2, "A FAMILY", [150, 40, 80]);
            for (x, y) in [(40.0, 102.0), (64.0, 112.0), (88.0, 102.0)] {
                c.disc(x - 5.0, y, 8.0, [225, 60, 90]);
                c.disc(x + 5.0, y, 8.0, [225, 60, 90]);
                c.rect(x as i32 - 11, y as i32, 22, 6, [225, 60, 90]);
            }
            c.text_c(138, 1, "(TERMS APPLY)", [150, 90, 110]);
            c
        }
        Tex::PizzaBox => {
            let mut c = Canvas::new(192, 128, [150, 112, 70]);
            c.frame(4, 4, 184, 120, 2, [110, 80, 45]);
            c.text_c(18, 2, "PIZZA FRIDAY", [250, 240, 220]);
            c.text_c(48, 2, "COUNTS AS", [250, 240, 220]);
            c.text_c(78, 2, "A RAISE", [250, 220, 90]);
            c
        }
        Tex::ChatbotFace => {
            let mut c = Canvas::new(128, 96, [14, 22, 50]);
            c.rect(24, 8, 80, 58, [190, 215, 240]);
            c.rect(30, 14, 68, 46, [30, 50, 110]);
            c.disc(48.0, 34.0, 7.0, [120, 240, 255]);
            c.disc(80.0, 34.0, 7.0, [120, 240, 255]);
            c.line((46.0, 48.0), (56.0, 52.0), [120, 240, 255]);
            c.line((56.0, 52.0), (72.0, 52.0), [120, 240, 255]);
            c.line((72.0, 52.0), (82.0, 48.0), [120, 240, 255]);
            c.text_c(74, 1, "HOW CAN I HELP?", [120, 240, 255]);
            c
        }
        Tex::Suggestions => {
            let mut c = Canvas::new(144, 32, [60, 60, 66]);
            c.text_c(9, 2, "SUGGESTIONS", [240, 240, 240]);
            c
        }
        Tex::CloseDeals => {
            let mut c = Canvas::new(256, 64, [10, 4, 14]);
            c.text_c(15, 3, "CLOSE DEALS", [120, 20, 70]);
            c.text_c(14, 3, "CLOSE DEALS", [255, 90, 190]);
            c.frame(2, 2, 252, 60, 2, [255, 90, 190]);
            c
        }
        Tex::Skyline => {
            let mut c = Canvas::new(256, 128, [10, 10, 30]);
            c.vgradient([20, 20, 60], [230, 120, 90]);
            for i in 0..30u32 {
                c.rect(
                    (hash(i, 3) * 255.0) as i32,
                    (hash(i, 4) * 50.0) as i32,
                    1,
                    1,
                    [255, 255, 230],
                );
            }
            let mut x = 0;
            let mut i = 0;
            while x < 256 {
                let w = 14 + (hash(i, 5) * 18.0) as i32;
                let h = 30 + (hash(i, 6) * 70.0) as i32;
                c.rect(x, 128 - h, w, h, [14, 12, 28]);
                for wy in (128 - h + 4..124).step_by(8) {
                    for wx in (x + 3..x + w - 3).step_by(6) {
                        if hash(wx as u32, wy as u32) > 0.55 {
                            c.rect(wx, wy, 2, 3, [255, 220, 120]);
                        }
                    }
                }
                x += w + 2;
                i += 1;
            }
            c
        }
        Tex::Documentation => {
            let mut c = Canvas::new(160, 32, [70, 55, 40]);
            c.text_c(9, 2, "DOCUMENTATION", [220, 200, 160]);
            c
        }
        Tex::Wellness => {
            let mut c = Canvas::new(192, 32, [110, 160, 130]);
            c.text_c(9, 2, "WELLNESS CORNER", [250, 250, 240]);
            c
        }
        Tex::Cobweb => {
            let mut c = Canvas::transparent(64, 64);
            let w = [235, 235, 240];
            for k in 0..6 {
                let a = k as f32 * 0.31;
                c.line_a((0.0, 0.0), (62.0 * a.cos(), 62.0 * a.sin()), w, 170, 1);
            }
            for r in [12.0_f32, 22.0, 34.0, 48.0] {
                for k in 0..20 {
                    let (a0, a1) = (k as f32 * 0.0785, (k + 1) as f32 * 0.0785);
                    let sag = 1.0 - 0.06 * (k % 3) as f32;
                    c.line_a(
                        (r * a0.cos() * sag, r * a0.sin() * sag),
                        (r * a1.cos(), r * a1.sin()),
                        w,
                        150,
                        1,
                    );
                }
            }
            c
        }
        Tex::Standup => {
            let mut c = Canvas::new(192, 64, [40, 70, 110]);
            c.frame(2, 2, 188, 60, 2, [250, 220, 60]);
            c.text_c(10, 2, "STAND-UP AREA", [250, 250, 250]);
            c.text_c(38, 1, "EST. 15 MIN. SINCE 9:00", [250, 220, 60]);
            c
        }
        Tex::DemoScreen => {
            let mut c = Canvas::new(128, 80, [10, 20, 50]);
            for i in 0..8 {
                c.rect(8 + i * 14, 66 - (8 + i * 6), 9, 8 + i * 6, [60, 200, 130]);
            }
            c.line((8.0, 56.0), (116.0, 12.0), [250, 220, 60]);
            c.text(6, 4, 1, "DEMO: LIVE", [120, 240, 255]);
            c
        }
        Tex::WarRoom => {
            let mut c = Canvas::new(192, 32, [30, 30, 36]);
            c.text_c(9, 2, "PMO WAR ROOM", [255, 70, 60]);
            c
        }
        Tex::PraiseModel => {
            let mut c = Canvas::new(200, 48, [20, 6, 40]);
            c.text_c(17, 2, "PRAISE THE MODEL", [230, 120, 255]);
            c.frame(2, 2, 196, 44, 2, [230, 120, 255]);
            c
        }
        Tex::Vending => {
            let mut c = Canvas::new(64, 96, [200, 30, 40]);
            c.rect(6, 6, 52, 62, [20, 24, 30]);
            for r in 0..4 {
                for k in 0..3 {
                    let col = [
                        [250, 200, 60],
                        [80, 200, 250],
                        [240, 240, 240],
                        [120, 220, 120],
                    ][((r + k) % 4) as usize];
                    c.rect(10 + k * 16, 10 + r * 15, 12, 10, col);
                }
            }
            c.rect(10, 74, 30, 14, [10, 10, 12]);
            c.text(44, 74, 1, "AI", WHITE);
            c
        }
        Tex::Kpi => {
            let mut c = Canvas::new(128, 80, [10, 24, 14]);
            c.text_c(8, 2, "ALL KPIS", [80, 255, 120]);
            c.text_c(30, 2, "GREEN", [80, 255, 120]);
            for i in 0..8 {
                c.disc(
                    14.0 + i as f32 * 14.0,
                    66.0,
                    4.0,
                    if i == 5 { RED } else { [60, 220, 100] },
                );
            }
            c
        }
        Tex::Leaderboard => {
            let mut c = Canvas::new(96, 128, [24, 20, 40]);
            c.text_c(6, 1, "TOP SELLERS", [250, 220, 60]);
            for i in 0..6 {
                let len = 70 - i * 10;
                c.rect(8, 24 + i * 16, len, 10, [250, 190 - i as u8 * 15, 60]);
            }
            c
        }
        Tex::Roadmap => {
            let mut c = Canvas::new(192, 96, [246, 247, 250]);
            c.rect(10, 48, 172, 3, [60, 60, 70]);
            for (i, q) in ["Q1", "Q2", "Q3", "Q4"].iter().enumerate() {
                let x = 14 + i as i32 * 44;
                c.disc(x as f32 + 12.0, 49.0, 5.0, [60, 110, 210]);
                c.text(x, 18, 2, q, [60, 60, 70]);
                c.text(x + 2, 62, 2, "AI", [40, 150, 80]);
            }
            c.text(10, 84, 1, "TIMELINE SUBJECT TO AI", [150, 150, 160]);
            c
        }
        Tex::Brand => {
            let mut c = Canvas::new(192, 64, [8, 4, 18]);
            c.text_c(15, 5, "BRAND", [90, 20, 110]);
            c.text_c(14, 5, "BRAND", [90, 240, 255]);
            c.frame(2, 2, 188, 60, 2, [255, 90, 190]);
            c
        }
        Tex::MoodBoards => {
            let mut c = Canvas::new(192, 128, [235, 230, 225]);
            let pal = [
                [250, 190, 200],
                [190, 220, 250],
                [250, 235, 160],
                [190, 245, 210],
                [220, 190, 250],
                [250, 210, 160],
            ];
            for r in 0..4u32 {
                for k in 0..6u32 {
                    let w = 24 + (hash(k, r) * 8.0) as i32;
                    c.rect(
                        4 + k as i32 * 31,
                        4 + r as i32 * 31,
                        w,
                        26,
                        pal[((k + r * 2) % 6) as usize],
                    );
                }
            }
            c
        }
        Tex::Campaign => {
            let mut c = Canvas::new(192, 112, [14, 18, 32]);
            for y in (20..100).step_by(16) {
                c.rect(8, y, 176, 1, [50, 60, 80]);
            }
            c.text(8, 4, 1, "CAMPAIGN PERFORMANCE", [120, 240, 255]);
            c.line((10.0, 96.0), (80.0, 70.0), [80, 255, 140]);
            c.line((80.0, 70.0), (140.0, 36.0), [80, 255, 140]);
            c.line((140.0, 36.0), (191.0, -20.0), [80, 255, 140]);
            c.text(130, 96, 1, "+9999%", [80, 255, 140]);
            c
        }
        Tex::SocAlert => {
            let mut c = Canvas::new(64, 64, [60, 4, 6]);
            c.frame(2, 2, 60, 60, 2, [255, 40, 40]);
            c.text(17, 12, 6, "!", [255, 60, 50]);
            c
        }
        Tex::Phishing => {
            let mut c = Canvas::new(160, 128, [244, 244, 240]);
            c.rect(0, 0, 160, 8, RED);
            c.text_c(14, 2, "PHISHING TEST", BLACK);
            c.text_c(32, 2, "RESULTS", BLACK);
            c.rect(16, 90, 50, 2, BLACK);
            c.rect(24, 54, 34, 36, RED);
            c.rect(94, 88, 34, 2, [60, 160, 80]);
            c.text(12, 98, 1, "CLICKED 100%", RED);
            c.text(76, 108, 1, "REPORTED 0%", [60, 160, 80]);
            c
        }
        Tex::Misc => {
            let mut c = Canvas::new(64, 32, [170, 130, 85]);
            c.text_c(7, 3, "MISC", [40, 30, 20]);
            c
        }
        Tex::TurnItOff => {
            let mut c = Canvas::new(192, 96, [250, 245, 225]);
            c.frame(3, 3, 186, 90, 2, [60, 60, 70]);
            c.text_c(14, 2, "HAVE YOU TRIED", BLACK);
            c.text_c(40, 2, "TURNING IT OFF", BLACK);
            c.text_c(66, 2, "AND ON AGAIN?", [40, 90, 200]);
            c
        }
        Tex::NextAgent => {
            let mut c = Canvas::new(192, 64, [20, 20, 26]);
            c.frame(2, 2, 188, 60, 2, [250, 190, 60]);
            c.text_c(9, 1, "NEXT AVAILABLE AGENT:", [250, 190, 60]);
            c.text_c(26, 3, "6-8 WEEKS", [255, 80, 60]);
            c
        }
        Tex::StickyPass => {
            let mut c = Canvas::new(48, 48, [250, 225, 80]);
            c.text(3, 12, 1, "PASS:", [60, 60, 60]);
            c.text(3, 26, 1, "HUNTER2", [60, 60, 60]);
            c
        }
    }
}

/// Small "Z" sprite for sleeping units.
/// Generated stand-in for a missing icon file: an IDE-style letter badge
/// (initials of the key on a gradient tinted by a hash of the key).
pub fn icon_image(key: &str) -> Image {
    let hash = key.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    let hue = (hash % 360) as f32;
    let tint = |v: f32| {
        let c = Color::hsv(hue, 0.55, v).to_srgba();
        [c.red, c.green, c.blue].map(|x| (x * 255.0) as u8)
    };
    let mut c = Canvas::new(64, 64, BLACK);
    c.vgradient(tint(0.72), tint(0.3));
    c.frame(0, 0, 64, 64, 3, tint(0.18));
    let initials = icon_initials(key);
    c.text_c(20, 4, &initials, [10, 10, 14]);
    let w = initials.chars().count() as i32 * 24 - 4;
    c.text((64 - w) / 2 - 2, 18, 4, &initials, WHITE);
    c.into_image()
}

/// "git_blame" -> "GB", "soc" -> "SO".
fn icon_initials(key: &str) -> String {
    let words: Vec<&str> = key.split('_').filter(|w| !w.is_empty()).collect();
    let s: String = match words.as_slice() {
        [one] => one.chars().take(2).collect(),
        many => many
            .iter()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect(),
    };
    s.to_ascii_uppercase()
}

pub fn z_image() -> Image {
    let mut c = Canvas::transparent(32, 32);
    c.text(4, 1, 5, "Z", [230, 240, 255]);
    c.into_image()
}

#[cfg(test)]
mod icon_tests {
    use super::icon_initials;

    #[test]
    fn badge_initials() {
        assert_eq!(icon_initials("git_blame"), "GB");
        assert_eq!(icon_initials("soc"), "SO");
        assert_eq!(icon_initials(""), "");
    }
}
