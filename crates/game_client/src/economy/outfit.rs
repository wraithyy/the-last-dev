//! Gear you can see: every player model (yours, teammates', lineup skins) wears its equipped
//! pieces, built from primitives on the skeleton so they follow every animation: headgear on the
//! head, the hoodie as a tint of the shirt with trims and logos, the keyboard slung on the back,
//! the mug in the right fist, the lanyard badge on the chest. Epic and Legendary pieces glow.
//! Also: loot beams over corpses holding your Uncommon-or-better gear.

use crate::combat::{CombatEvent, SimRead, SimReady, SimUnit, SimView};
use crate::units::Unit;
use bevy::gltf::GltfMaterialName;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use combat_core::{EntityId, Event, GearDef, Goods, Rarity, Slot};
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI};

/// The hoodie tints these skin materials.
const HOODIE_MATERIALS: [&str; 3] = ["Shirt", "Sleeves", "Hood"];

/// What a model currently wears (built), by slot.
#[derive(Component, PartialEq)]
struct OutfitShown(Vec<String>);

/// A spawned gear primitive (child of a bone).
#[derive(Component)]
struct OutfitPiece;

/// Lineup and other display-only skins: the outfit to wear instead of a sim unit's.
#[derive(Component)]
pub struct DisplayOutfit(pub Vec<String>);

/// The skin material a tinted mesh had before the hoodie.
#[derive(Component)]
struct TintBackup(Handle<StandardMaterial>);

/// Propellers turn.
#[derive(Component)]
struct Spin(f32);

#[derive(Resource, Default)]
struct OutfitAssets {
    mats: HashMap<(u32, u8), Handle<StandardMaterial>>,
    tints: HashMap<(AssetId<StandardMaterial>, u32), Handle<StandardMaterial>>,
    cube: Handle<Mesh>,
    sphere: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    torus: Handle<Mesh>,
    cone: Handle<Mesh>,
}

#[derive(Component)]
struct LootBeam {
    corpse: EntityId,
    rarity: Rarity,
    age: f32,
}

pub struct OutfitPlugin;

impl Plugin for OutfitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OutfitAssets>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    lineup_outfits,
                    rebuild_on_respawn,
                    sync_outfits,
                    spin,
                    beams,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn setup(mut assets: ResMut<OutfitAssets>, mut meshes: ResMut<Assets<Mesh>>) {
    assets.cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    assets.sphere = meshes.add(Sphere::new(0.5));
    assets.cylinder = meshes.add(Cylinder::new(0.5, 1.0));
    assets.torus = meshes.add(Torus::new(0.85, 1.0));
    assets.cone = meshes.add(Cone {
        radius: 0.5,
        height: 1.0,
    });
}

fn rgb_key(c: (f32, f32, f32)) -> u32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u32;
    (q(c.0) << 16) | (q(c.1) << 8) | q(c.2)
}

impl OutfitAssets {
    /// A lit material; `glow` 0 none, 1 subtle (Epic), 2 strong (Legendary, RGB keys).
    fn mat(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
        c: (f32, f32, f32),
        glow: u8,
    ) -> Handle<StandardMaterial> {
        self.mats
            .entry((rgb_key(c), glow))
            .or_insert_with(|| {
                let base = Color::srgb(c.0, c.1, c.2);
                let lin = base.to_linear();
                let k = [0.0, 0.6, 1.8][usize::from(glow.min(2))];
                materials.add(StandardMaterial {
                    base_color: base,
                    perceptual_roughness: 0.45,
                    emissive: LinearRgba::rgb(lin.red * k, lin.green * k, lin.blue * k),
                    ..default()
                })
            })
            .clone()
    }
}

/// Glow level from rarity: Epic subtle, Legendary strong.
fn glow_of(g: &GearDef) -> u8 {
    match g.rarity {
        Rarity::Epic => 1,
        Rarity::Legendary => 2,
        _ => 0,
    }
}

/// Sample outfits for `WOW_LINEUP_GEAR=1` screenshots, by class skin order.
const LINEUP_GEAR: [&[&str]; 4] = [
    &[
        "Blame-Aware Headset",
        "Full-Stack Overflow: Hoodie of Every Layer",
        "The Monorepo",
        "Rubber Duck With a Pager",
        "Senior Title Without the Raise",
    ],
    &[
        "Dark Mode Shades",
        "Framework of the Week Hoodie",
        "Pixel-Perfect Mechanical Keyboard",
        "Pixel Perfect: Mug in #FF00FF",
        "Golden Visitor Badge",
    ],
    &[
        "Incident Commander Headset",
        "Horizontally Scaled Hoodie",
        "Consultant's Keyboard (Billable Keys Only)",
        "Backpressure Mug",
        "Root Password on a Sticky Note",
    ],
    &[
        "Propeller Hat of Disruptive Thinking",
        "Chaos Engineering Hoodie",
        "On-Call Rotation: Keyboard With a Crumb Layer",
        "Blue-Green Thermos",
        "The Pager That Goes Off at 3 a.m.",
    ],
];

fn lineup_outfits(
    mut commands: Commands,
    sim: SimView,
    skins: Query<
        (Entity, &crate::gear::Accessories),
        (Added<crate::gear::Accessories>, Without<SimUnit>),
    >,
) {
    if std::env::var_os("WOW_LINEUP_GEAR").is_none() {
        return;
    }
    for (e, kit) in &skins {
        let k = sim
            .content()
            .classes
            .iter()
            .position(|c| c.key == kit.0)
            .unwrap_or(0)
            .min(LINEUP_GEAR.len() - 1);
        let mut outfit = vec![String::new(); combat_core::SLOTS];
        for name in LINEUP_GEAR[k] {
            if let Some(g) = sim.content().economy.gear(name) {
                outfit[g.slot.index()] = (*name).to_string();
            }
        }
        commands.entity(e).insert(DisplayOutfit(outfit));
    }
}

/// A new skeleton (first spawn, role change) lost the old pieces: build again.
fn rebuild_on_respawn(
    mut commands: Commands,
    respawned: Query<Entity, Added<crate::anim::ModelSpawned>>,
) {
    for e in &respawned {
        commands.entity(e).remove::<OutfitShown>();
    }
}

fn wanted(
    sim: &dyn SimRead,
    unit: Option<&SimUnit>,
    display: Option<&DisplayOutfit>,
) -> Option<Vec<String>> {
    if let Some(d) = display {
        return Some(d.0.clone());
    }
    let id = unit?.0;
    sim.players()
        .contains(&id)
        .then(|| sim.unit(id).map(|u| super::goods::slots_of(&u.equipped)))
        .flatten()
}

fn sync_outfits(
    mut commands: Commands,
    sim: SimView,
    mut assets: ResMut<OutfitAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    wearers: Query<
        (
            Entity,
            Option<&SimUnit>,
            Option<&DisplayOutfit>,
            Option<&OutfitShown>,
        ),
        (
            With<crate::anim::ModelSpawned>,
            Or<(With<SimUnit>, With<DisplayOutfit>)>,
        ),
    >,
    children: Query<&Children>,
    names: Query<&Name>,
    pieces: Query<(), With<OutfitPiece>>,
    mut skin: Query<(
        Entity,
        &GltfMaterialName,
        &mut MeshMaterial3d<StandardMaterial>,
        Option<&TintBackup>,
    )>,
) {
    for (wearer, unit, display, shown) in &wearers {
        let Some(want) = wanted(&*sim, unit, display) else {
            continue;
        };
        if shown.is_some_and(|s| s.0 == want) {
            continue;
        }
        let bone = |wanted: &str| {
            children
                .iter_descendants(wearer)
                .find(|e| names.get(*e).is_ok_and(|n| n.as_str() == wanted))
        };
        let (Some(head), Some(torso), Some(fist)) = (bone("Head"), bone("Torso"), bone("Fist.R"))
        else {
            continue;
        };
        for e in children.iter_descendants(wearer) {
            if pieces.contains(e) {
                commands.entity(e).despawn();
            }
        }
        let eco = &sim.content().economy;
        let hoodie = want
            .get(Slot::Hoodie.index())
            .and_then(|n| eco.gear(n))
            .map(|g| g.visual.color);
        // Hoodie: tint the skin's shirt, sleeves and hood (restored when it comes off).
        for d in children.iter_descendants(wearer).collect::<Vec<_>>() {
            let Ok((e, name, mut handle, backup)) = skin.get_mut(d) else {
                continue;
            };
            if !HOODIE_MATERIALS.contains(&name.0.as_str()) {
                continue;
            }
            let original = backup.map_or_else(|| handle.0.clone(), |b| b.0.clone());
            if backup.is_none() {
                commands.entity(e).insert(TintBackup(original.clone()));
            }
            handle.0 = match hoodie {
                None => original,
                Some(c) => {
                    let key = (original.id(), rgb_key(c));
                    if let Some(h) = assets.tints.get(&key) {
                        h.clone()
                    } else {
                        let Some(base) = materials.get(&original) else {
                            continue;
                        };
                        let mut m = base.clone();
                        m.base_color = Color::srgb(c.0, c.1, c.2);
                        // A patterned shirt (plaid) would darken the tint into mud: the hoodie
                        // covers it with a flat colour instead.
                        m.base_color_texture = None;
                        let h = materials.add(m);
                        assets.tints.insert(key, h.clone());
                        h
                    }
                }
            };
        }
        for name in &want {
            let Some(g) = eco.gear(name) else { continue };
            let parent = match g.slot {
                Slot::Headgear => head,
                Slot::Mug => fist,
                Slot::Hoodie | Slot::Keyboard | Slot::Lanyard => torso,
            };
            let parts = build_piece(g, &mut assets, &mut materials);
            commands.entity(parent).with_children(|c| {
                for (mesh, mat, tf, spin) in parts {
                    let mut e = c.spawn((
                        OutfitPiece,
                        Mesh3d(mesh),
                        MeshMaterial3d(mat),
                        tf,
                        NotShadowCaster,
                    ));
                    if let Some(s) = spin {
                        e.insert(Spin(s));
                    }
                }
            });
        }
        commands.entity(wearer).insert(OutfitShown(want));
    }
}

type Part = (
    Handle<Mesh>,
    Handle<StandardMaterial>,
    Transform,
    Option<f32>,
);

/// Primitives for one piece in its parent bone's space (model units; the head spans x +-0.54,
/// y -0.1..1.0, face at z +0.5).
fn build_piece(
    g: &GearDef,
    a: &mut OutfitAssets,
    materials: &mut Assets<StandardMaterial>,
) -> Vec<Part> {
    let c = g.visual.color;
    let glow = glow_of(g);
    let main = a.mat(materials, c, glow);
    let dark = a.mat(materials, (0.08, 0.08, 0.1), 0);
    let white = a.mat(materials, (0.92, 0.92, 0.95), 0);
    let gold = a.mat(materials, (1.0, 0.78, 0.2), glow.max(1));
    let (cube, sphere, cyl, torus, cone) = (
        a.cube.clone(),
        a.sphere.clone(),
        a.cylinder.clone(),
        a.torus.clone(),
        a.cone.clone(),
    );
    let at = |x: f32, y: f32, z: f32, s: Vec3| Transform::from_xyz(x, y, z).with_scale(s);
    let mut out: Vec<Part> = Vec::new();
    match g.slot {
        Slot::Headgear => match g.visual.style {
            0 => {
                // Cap, worn backwards: dome and a brim over the neck.
                out.push((
                    sphere.clone(),
                    main.clone(),
                    at(0.0, 0.66, -0.02, Vec3::new(1.2, 0.62, 1.2)),
                    None,
                ));
                out.push((
                    cube,
                    main.clone(),
                    at(0.0, 0.7, -0.66, Vec3::new(0.55, 0.05, 0.4)),
                    None,
                ));
                out.push((sphere, white, at(0.0, 0.98, -0.02, Vec3::splat(0.1)), None));
            }
            1 => {
                out.push((
                    sphere.clone(),
                    main.clone(),
                    at(0.0, 0.7, 0.0, Vec3::new(1.22, 0.85, 1.22)),
                    None,
                ));
                out.push((
                    torus,
                    main.clone(),
                    at(0.0, 0.52, 0.0, Vec3::new(0.62, 0.9, 0.62)),
                    None,
                ));
                out.push((sphere, white, at(0.0, 1.18, 0.0, Vec3::splat(0.26)), None));
            }
            3 => {
                // Propeller beanie: quartered cap, stalk, spinning blades.
                out.push((
                    sphere.clone(),
                    main.clone(),
                    at(0.0, 0.7, 0.0, Vec3::new(1.2, 0.8, 1.2)),
                    None,
                ));
                out.push((
                    cyl,
                    dark,
                    at(0.0, 1.12, 0.0, Vec3::new(0.05, 0.2, 0.05)),
                    None,
                ));
                let blade = a.mat(materials, (0.95, 0.2, 0.2), glow);
                out.push((
                    cube.clone(),
                    blade,
                    at(0.0, 1.24, 0.0, Vec3::new(0.9, 0.025, 0.12)),
                    Some(9.0),
                ));
                let blade2 = a.mat(materials, (0.2, 0.5, 1.0), glow);
                out.push((
                    cube,
                    blade2,
                    at(0.0, 1.245, 0.0, Vec3::new(0.12, 0.025, 0.9)),
                    Some(9.0),
                ));
                out.push((sphere, white, at(0.0, 1.25, 0.0, Vec3::splat(0.08)), None));
            }
            4 => {
                // VR visor across the face, strap around the head.
                out.push((
                    cube.clone(),
                    main.clone(),
                    at(0.0, 0.46, 0.6, Vec3::new(0.95, 0.34, 0.28)),
                    None,
                ));
                let lens = a.mat(materials, (0.2, 0.8, 1.0), 2);
                out.push((
                    cube,
                    lens,
                    at(0.0, 0.46, 0.75, Vec3::new(0.8, 0.2, 0.02)),
                    None,
                ));
                out.push((
                    torus,
                    dark,
                    at(0.0, 0.48, 0.0, Vec3::new(0.6, 0.5, 0.6)),
                    None,
                ));
            }
            5 => {
                out.push((
                    torus,
                    gold.clone(),
                    at(0.0, 1.06, 0.0, Vec3::new(0.42, 0.6, 0.42)),
                    None,
                ));
                for k in 0..5 {
                    let ang = k as f32 * 2.0 * PI / 5.0;
                    out.push((
                        cone.clone(),
                        gold.clone(),
                        at(
                            ang.sin() * 0.4,
                            1.2,
                            ang.cos() * 0.4,
                            Vec3::new(0.12, 0.25, 0.12),
                        ),
                        None,
                    ));
                }
            }
            6 => {
                // Shades.
                for side in [-1.0f32, 1.0] {
                    out.push((
                        cube.clone(),
                        main.clone(),
                        at(side * 0.2, 0.47, 0.56, Vec3::new(0.34, 0.17, 0.04)),
                        None,
                    ));
                }
                out.push((
                    cube,
                    main,
                    at(0.0, 0.5, 0.56, Vec3::new(0.12, 0.03, 0.03)),
                    None,
                ));
            }
            _ => {
                // Headset: band over the hair, cups on the ears, a mic boom to the mouth.
                for k in 0..=6 {
                    let ang = -1.25 + 2.5 * k as f32 / 6.0;
                    let r = 0.68;
                    out.push((
                        cube.clone(),
                        main.clone(),
                        Transform::from_xyz(ang.sin() * r, 0.42 + ang.cos() * r, 0.05)
                            .with_rotation(Quat::from_rotation_z(-ang))
                            .with_scale(Vec3::new(0.22, 0.07, 0.13)),
                        None,
                    ));
                }
                for side in [-1.0f32, 1.0] {
                    out.push((
                        cyl.clone(),
                        main.clone(),
                        Transform::from_xyz(side * 0.6, 0.42, 0.05)
                            .with_rotation(Quat::from_rotation_z(FRAC_PI_2))
                            .with_scale(Vec3::new(0.42, 0.16, 0.42)),
                        None,
                    ));
                }
                out.push((
                    cube,
                    dark,
                    Transform::from_xyz(-0.5, 0.26, 0.35)
                        .with_rotation(Quat::from_rotation_y(-0.6))
                        .with_scale(Vec3::new(0.04, 0.04, 0.5)),
                    None,
                ));
                out.push((sphere, main, at(-0.32, 0.22, 0.58, Vec3::splat(0.1)), None));
            }
        },
        Slot::Hoodie => {
            // The tint does most of the work; logo and trims make tiers readable.
            if g.visual.style >= 1 {
                out.push((
                    cube.clone(),
                    white.clone(),
                    at(0.13, 0.16, 0.3, Vec3::new(0.14, 0.14, 0.02)),
                    None,
                ));
            }
            if g.visual.style >= 2 {
                let trim = a.mat(
                    materials,
                    (c.0 * 0.5 + 0.5, c.1 * 0.5 + 0.5, c.2 * 0.5 + 0.5),
                    glow.max(1),
                );
                for side in [-1.0f32, 1.0] {
                    out.push((
                        cube.clone(),
                        trim.clone(),
                        at(side * 0.07, 0.05, 0.31, Vec3::new(0.025, 0.45, 0.02)),
                        None,
                    ));
                }
            }
            if g.visual.style >= 3 {
                out.push((
                    torus,
                    gold,
                    at(0.0, -0.18, 0.0, Vec3::new(0.36, 0.5, 0.3)),
                    None,
                ));
            }
        }
        Slot::Keyboard => {
            // Slung diagonally across the back, keys facing out.
            let (w, split) = match g.visual.style {
                0 => (0.8, false),
                2 => (0.95, true),
                _ => (1.05, false),
            };
            let rot = Quat::from_rotation_y(PI) * Quat::from_rotation_z(0.5);
            let base = Transform::from_xyz(0.0, 0.12, -0.36).with_rotation(rot);
            let halves: Vec<(f32, f32)> = if split {
                vec![(-0.26, 0.45), (0.26, 0.45)]
            } else {
                vec![(0.0, w)]
            };
            let keys = if g.visual.style == 3 {
                vec![
                    a.mat(materials, (1.0, 0.2, 0.4), 2),
                    a.mat(materials, (0.2, 1.0, 0.5), 2),
                    a.mat(materials, (0.3, 0.5, 1.0), 2),
                ]
            } else {
                let k = a.mat(
                    materials,
                    (c.0 * 0.4 + 0.6, c.1 * 0.4 + 0.6, c.2 * 0.4 + 0.6),
                    glow,
                );
                vec![k.clone(), k.clone(), k]
            };
            for (dx, hw) in halves {
                out.push((
                    cube.clone(),
                    main.clone(),
                    base * Transform::from_xyz(dx, 0.0, 0.0).with_scale(Vec3::new(hw, 0.34, 0.06)),
                    None,
                ));
                for (row, mat) in keys.iter().enumerate() {
                    out.push((
                        cube.clone(),
                        mat.clone(),
                        base * Transform::from_xyz(dx, 0.1 - row as f32 * 0.1, 0.045)
                            .with_scale(Vec3::new(hw * 0.88, 0.06, 0.03)),
                        None,
                    ));
                }
            }
            // The strap over the shoulder.
            out.push((
                cube,
                dark,
                Transform::from_xyz(0.0, 0.2, 0.0)
                    .with_rotation(Quat::from_rotation_z(0.5))
                    .with_scale(Vec3::new(0.05, 0.9, 0.72)),
                None,
            ));
        }
        Slot::Mug => {
            let grip = Vec3::new(0.0, 0.12, 0.24);
            // Mug pieces are authored at full size; 0.75 keeps them hand-sized.
            let k = 0.75;
            match g.visual.style {
                1 => {
                    out.push((
                        cyl.clone(),
                        main.clone(),
                        Transform::from_translation(grip + Vec3::Y * 0.08)
                            .with_scale(Vec3::new(0.28, 0.62, 0.28)),
                        None,
                    ));
                    out.push((
                        cyl,
                        dark,
                        Transform::from_translation(grip + Vec3::Y * 0.43)
                            .with_scale(Vec3::new(0.24, 0.1, 0.24)),
                        None,
                    ));
                }
                2 => {
                    out.push((
                        cyl.clone(),
                        main.clone(),
                        Transform::from_translation(grip + Vec3::Y * 0.05)
                            .with_scale(Vec3::new(0.44, 0.48, 0.44)),
                        None,
                    ));
                    out.push((
                        cyl,
                        gold,
                        Transform::from_translation(grip + Vec3::Y * 0.31)
                            .with_scale(Vec3::new(0.46, 0.05, 0.46)),
                        None,
                    ));
                    out.push((
                        torus,
                        main,
                        Transform::from_translation(grip + Vec3::new(0.0, 0.05, -0.25))
                            .with_rotation(
                                Quat::from_rotation_y(FRAC_PI_2) * Quat::from_rotation_x(FRAC_PI_2),
                            )
                            .with_scale(Vec3::splat(0.12)),
                        None,
                    ));
                }
                3 => {
                    out.push((
                        cone.clone(),
                        main.clone(),
                        Transform::from_translation(grip + Vec3::Y * 0.2)
                            .with_rotation(Quat::from_rotation_x(PI))
                            .with_scale(Vec3::new(0.42, 0.3, 0.42)),
                        None,
                    ));
                    out.push((
                        cyl.clone(),
                        gold.clone(),
                        Transform::from_translation(grip).with_scale(Vec3::new(0.06, 0.25, 0.06)),
                        None,
                    ));
                    out.push((
                        cyl,
                        gold,
                        Transform::from_translation(grip - Vec3::Y * 0.13)
                            .with_scale(Vec3::new(0.26, 0.03, 0.26)),
                        None,
                    ));
                }
                _ => {
                    out.push((
                        cyl.clone(),
                        main.clone(),
                        Transform::from_translation(grip).with_scale(Vec3::new(0.38, 0.42, 0.38)),
                        None,
                    ));
                    out.push((
                        cyl,
                        a.mat(materials, (0.12, 0.06, 0.03), 0),
                        Transform::from_translation(grip + Vec3::Y * 0.205)
                            .with_scale(Vec3::new(0.34, 0.01, 0.34)),
                        None,
                    ));
                    out.push((
                        torus,
                        main,
                        Transform::from_translation(grip + Vec3::new(0.0, 0.0, -0.22))
                            .with_rotation(
                                Quat::from_rotation_y(FRAC_PI_2) * Quat::from_rotation_x(FRAC_PI_2),
                            )
                            .with_scale(Vec3::splat(0.1)),
                        None,
                    ));
                }
            }
            for part in &mut out {
                part.2.translation = grip + (part.2.translation - grip) * k;
                part.2.scale *= k;
            }
        }
        Slot::Lanyard => {
            let cord = a.mat(materials, c, 0);
            for side in [-1.0f32, 1.0] {
                out.push((
                    cube.clone(),
                    cord.clone(),
                    Transform::from_xyz(side * 0.1, 0.24, 0.3)
                        .with_rotation(Quat::from_rotation_z(side * 0.35))
                        .with_scale(Vec3::new(0.03, 0.4, 0.02)),
                    None,
                ));
            }
            let card = if g.visual.style == 2 {
                gold.clone()
            } else {
                white.clone()
            };
            out.push((
                cube.clone(),
                card,
                at(0.0, 0.0, 0.32, Vec3::new(0.2, 0.26, 0.02)),
                None,
            ));
            out.push((
                cube.clone(),
                main.clone(),
                at(0.0, 0.05, 0.335, Vec3::new(0.12, 0.1, 0.01)),
                None,
            ));
            if g.visual.style == 1 {
                let pins = [
                    (1.0, 0.3, 0.3),
                    (0.3, 0.9, 0.4),
                    (0.4, 0.5, 1.0),
                    (1.0, 0.85, 0.2),
                ];
                for (k, p) in pins.iter().enumerate() {
                    let m = a.mat(materials, *p, 0);
                    let side = if k % 2 == 0 { -1.0 } else { 1.0 };
                    out.push((
                        sphere.clone(),
                        m,
                        at(side * 0.12, 0.3 - k as f32 * 0.06, 0.32, Vec3::splat(0.06)),
                        None,
                    ));
                }
            }
        }
    }
    out
}

fn spin(time: Res<Time>, mut q: Query<(&Spin, &mut Transform)>) {
    for (s, mut tf) in &mut q {
        tf.rotate_y(s.0 * time.delta_secs());
    }
}

/// Best gear rarity in one of this player's rolls, if it deserves a beam (Uncommon or better).
fn beam_rarity(content: &combat_core::Content, items: &[String]) -> Option<Rarity> {
    items
        .iter()
        .filter_map(|n| match content.economy.goods(n) {
            Some(Goods::Gear(g)) => Some(g.rarity),
            _ => None,
        })
        .max()
        .filter(|r| *r >= Rarity::Uncommon)
}

fn beams(
    mut commands: Commands,
    time: Res<Time>,
    sim: SimView,
    mut events: MessageReader<CombatEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    corpses: Query<(&SimUnit, &Transform), With<Unit>>,
    mut live: Query<(Entity, &mut LootBeam, &mut Transform), Without<Unit>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
) {
    let mesh = mesh
        .get_or_insert_with(|| meshes.add(Cylinder::new(0.5, 1.0)))
        .clone();
    for CombatEvent(e) in events.read() {
        let Event::LootDropped { corpse, items, .. } = e else {
            continue;
        };
        let Some(rarity) = beam_rarity(sim.content(), items) else {
            continue;
        };
        let Some((_, tf)) = corpses.iter().find(|(s, _)| s.0 == *corpse) else {
            continue;
        };
        let c = super::goods::rarity_color(rarity).to_linear();
        let mat = materials.add(StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::new(c.red, c.green, c.blue, 0.35)),
            emissive: LinearRgba::rgb(c.red * 2.0, c.green * 2.0, c.blue * 2.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        });
        commands.spawn((
            LootBeam {
                corpse: *corpse,
                rarity,
                age: 0.0,
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(tf.translation),
            NotShadowCaster,
        ));
    }
    for (e, mut beam, mut tf) in &mut live {
        beam.age += time.delta_secs();
        let still = sim
            .unit(beam.corpse)
            .is_some_and(|u| u.lootable && !u.alive);
        if !still {
            commands.entity(e).despawn();
            continue;
        }
        // Rare and up: a tall, thick column; Uncommon: a short green wisp.
        let (w, h) = match beam.rarity {
            Rarity::Uncommon => (0.18, 2.5),
            Rarity::Rare => (0.35, 7.0),
            Rarity::Epic => (0.45, 9.0),
            _ => (0.6, 12.0),
        };
        let pulse = 1.0 + 0.12 * (beam.age * 3.0).sin();
        let grow = (beam.age * 2.5).min(1.0);
        if let Some((_, corpse_tf)) = corpses.iter().find(|(s, _)| s.0 == beam.corpse) {
            tf.translation = corpse_tf.translation + Vec3::Y * (h * grow * 0.5);
        }
        tf.scale = Vec3::new(w * pulse, h * grow, w * pulse);
    }
}
