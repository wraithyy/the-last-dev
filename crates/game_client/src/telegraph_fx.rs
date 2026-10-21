//! Enemy mechanics on screen: ground telegraph decals that fill until they land, cover desks
//! for the earnings call, and an "INTERRUPT!" marker over must-interrupt casters.
//! Event-driven (`TelegraphStarted` / `TelegraphResolved`), so it renders the same online.
//! Unlit alpha-blended meshes and UI text only: WebGL2 friendly.
// Bevy system signatures are inherently wide.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, SimReady, SimUnit, SimView};
use crate::config::head_height;
use bevy::prelude::*;
use combat_core::{EntityId, Event, TelegraphShape};
use std::f32::consts::FRAC_PI_2;

pub struct TelegraphFxPlugin;

impl Plugin for TelegraphFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (
                spawn_decals,
                grow_decals,
                kick_markers,
                shield_arcs,
                place_labels,
            )
                .chain()
                .in_set(SimReady),
        );
    }
}

/// Seconds a landed decal flashes before it disappears.
const FLASH_SECS: f32 = 0.35;
/// A decal whose `TelegraphResolved` never arrives (missed packet) is dropped this late.
const STALE_SECS: f32 = 1.5;
/// Shared (stack) telegraphs are gold: standing in them is the right call.
const SHARE: [f32; 3] = [1.0, 0.82, 0.2];
/// Telegraphs of friendly casters (the player's Batch Job) are cyan, not danger red.
const OWN: [f32; 3] = [0.36, 0.82, 0.86];
const KICK_RED: Color = Color::srgb(1.0, 0.15, 0.1);
const LABEL_W: f32 = 320.0;

#[derive(Resource)]
struct TgAssets {
    disc: Handle<Mesh>,
    ring: Handle<Mesh>,
    square: Handle<Mesh>,
    desk: Handle<Mesh>,
    desk_mat: Handle<StandardMaterial>,
    kick_ring: Handle<StandardMaterial>,
}

#[derive(Component)]
struct Decal {
    id: u32,
    age: f32,
    total: f32,
    shape: TelegraphShape,
    fill: Entity,
    fill_mat: Handle<StandardMaterial>,
    color: [f32; 3],
    landed: Option<f32>,
    /// Cover circles and desks that go away with the decal.
    extras: Vec<Entity>,
}

/// UI text pinned to a world point (or to a unit when `follow` is set).
#[derive(Component)]
struct WorldLabel {
    at: Vec3,
    follow: Option<EntityId>,
    owner: Entity,
}

/// Laptop-shield arc on the floor in front of a `frontal_shield` unit, shown while it is up.
#[derive(Component)]
struct ShieldArc(EntityId);
#[derive(Component)]
struct ShieldChecked;

/// Red ring and label over a unit casting a must-interrupt spell.
#[derive(Component)]
struct KickMarker {
    unit: EntityId,
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

fn rgba(c: [f32; 3], a: f32) -> Color {
    Color::srgba(c[0], c[1], c[2], a)
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(TgAssets {
        disc: meshes.add(Circle::new(1.0)),
        ring: meshes.add(Annulus::new(0.92, 1.0)),
        square: meshes.add(Rectangle::new(1.0, 1.0)),
        desk: meshes.add(Cuboid::new(1.6, 0.75, 0.8)),
        desk_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.32, 0.2),
            perceptual_roughness: 0.8,
            ..default()
        }),
        kick_ring: materials.add(unlit(KICK_RED.with_alpha(0.8))),
    });
}

fn v3(p: combat_core::Vec3) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

/// On a slope a flat decal would sink into the uphill side: rest it on the highest ground
/// around its centre instead.
fn on_ground(t: &combat_core::Terrain, p: Vec3) -> Vec3 {
    if t.is_flat() {
        return p;
    }
    let r = 2.5;
    let top = [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)]
        .into_iter()
        .map(|(dx, dz)| t.surface(p.x + dx, p.z + dz))
        .fold(f32::MIN, f32::max);
    Vec3::new(p.x, top.max(p.y), p.z)
}

/// Lays a flat 2D mesh on the floor with its local +Y along the telegraph facing (a sim yaw).
fn ground(pos: Vec3, facing: f32, lift: f32) -> Transform {
    Transform::from_translation(pos + Vec3::Y * lift)
        .with_rotation(Quat::from_rotation_y(facing) * Quat::from_rotation_x(-FRAC_PI_2))
}

/// Fill transform at progress `k` (0..1), in the decal's local 2D plane.
fn fill_transform(shape: TelegraphShape, k: f32) -> Transform {
    let k = k.clamp(0.02, 1.0);
    match shape {
        TelegraphShape::Circle { radius_m } => Transform::from_scale(Vec3::splat(radius_m * k)),
        TelegraphShape::Cone { .. } => {
            Transform::from_xyz(0.0, 0.0, 0.002).with_scale(Vec3::new(k, k, 1.0))
        }
        TelegraphShape::Line { length_m, width_m } => Transform::from_xyz(
            0.0,
            length_m * k / 2.0,
            0.002,
        )
        .with_scale(Vec3::new(width_m, length_m * k, 1.0)),
    }
}

/// Where the label sits: the middle of the shape.
fn label_point(shape: TelegraphShape, pos: Vec3, facing: f32) -> Vec3 {
    let f = Vec3::new(-facing.sin(), 0.0, -facing.cos());
    match shape {
        TelegraphShape::Circle { .. } => pos,
        TelegraphShape::Cone { radius_m, .. } => pos + f * radius_m * 0.55,
        // Near the caster's end: the far end of a long beam is often behind the camera.
        TelegraphShape::Line { length_m, .. } => pos + f * length_m.min(8.0) * 0.5,
    }
}

fn label_bundle(text: String, color: Color, size: f32) -> impl Bundle {
    (
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(2.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.95),
        },
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: px(LABEL_W),
            ..default()
        },
        Visibility::Hidden,
        GlobalZIndex(9),
    )
}

fn spawn_decals(
    mut commands: Commands,
    mut events: MessageReader<CombatEvent>,
    sim: SimView,
    assets: Res<TgAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut decals: Query<&mut Decal>,
    pal: Res<crate::settings::Palette>,
    terrain: Res<crate::arena::Ground>,
) {
    // Danger and safe-spot colours follow the colourblind palette.
    let (danger, safe) = (pal.danger, pal.safe);
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::TelegraphStarted {
                id,
                source,
                spell,
                shape,
                pos,
                facing,
                resolve_ms,
                cover,
            } => {
                let def = sim
                    .content()
                    .spell(*spell)
                    .and_then(|s| s.telegraph.clone());
                let split = def.as_ref().is_some_and(|d| d.split);
                // A ring the player's own spell paints (Batch Job) is not a danger to dodge.
                let mine = sim.unit(*source).is_some_and(|u| !u.hostile);
                let color = if split {
                    SHARE
                } else if mine {
                    OWN
                } else {
                    danger
                };
                let pos = on_ground(&terrain.0, v3(*pos));
                // Stacked decals each get their own height so they do not z-fight.
                let lift = 0.03 + (id % 8) as f32 * 0.004;
                let (edge_mesh, fill_mesh, edge_tf) = match *shape {
                    TelegraphShape::Circle { radius_m } => (
                        assets.ring.clone(),
                        assets.disc.clone(),
                        Transform::from_scale(Vec3::splat(radius_m)),
                    ),
                    TelegraphShape::Cone {
                        radius_m,
                        angle_deg,
                    } => {
                        let sector = meshes.add(CircularSector::new(
                            radius_m,
                            (angle_deg / 2.0).to_radians(),
                        ));
                        (sector.clone(), sector, Transform::IDENTITY)
                    }
                    TelegraphShape::Line { length_m, width_m } => (
                        assets.square.clone(),
                        assets.square.clone(),
                        Transform::from_xyz(0.0, length_m / 2.0, 0.0)
                            .with_scale(Vec3::new(width_m, length_m, 1.0)),
                    ),
                };
                let edge_alpha = if matches!(shape, TelegraphShape::Circle { .. }) {
                    0.85
                } else {
                    0.14
                };
                let edge_mat = materials.add(unlit(rgba(color, edge_alpha)));
                let fill_mat = materials.add(unlit(rgba(color, 0.25)));
                let root = commands
                    .spawn((ground(pos, *facing, lift), Visibility::Inherited))
                    .id();
                let fill = commands
                    .spawn((
                        Mesh3d(fill_mesh),
                        MeshMaterial3d(fill_mat.clone()),
                        fill_transform(*shape, 0.0),
                    ))
                    .id();
                // Circles also get a faint full-size floor so the danger area reads at once.
                let base = matches!(shape, TelegraphShape::Circle { .. }).then(|| {
                    commands
                        .spawn((
                            Mesh3d(assets.disc.clone()),
                            MeshMaterial3d(materials.add(unlit(rgba(color, 0.12)))),
                            edge_tf.with_translation(Vec3::new(0.0, 0.0, -0.001)),
                        ))
                        .id()
                });
                let edge = commands
                    .spawn((Mesh3d(edge_mesh), MeshMaterial3d(edge_mat), edge_tf))
                    .id();
                commands.entity(root).add_children(&[fill, edge]);
                if let Some(b) = base {
                    commands.entity(root).add_child(b);
                }
                let mut extras = Vec::new();
                let cover_r = def.as_ref().map_or(2.0, |d| d.cover_radius_m);
                for c in cover {
                    let at = v3(*c);
                    let ring = commands
                        .spawn((
                            Mesh3d(assets.disc.clone()),
                            MeshMaterial3d(materials.add(unlit(rgba(safe, 0.35)))),
                            ground(at, 0.0, lift + 0.01).with_scale(Vec3::splat(cover_r)),
                        ))
                        .id();
                    let desk = commands
                        .spawn((
                            Mesh3d(assets.desk.clone()),
                            MeshMaterial3d(assets.desk_mat.clone()),
                            Transform::from_translation(at + Vec3::Y * 0.375),
                        ))
                        .id();
                    let tag = commands
                        .spawn(label_bundle(
                            "Behind the desk: safe".into(),
                            rgba(safe, 1.0),
                            14.0,
                        ))
                        .id();
                    commands.entity(tag).insert(WorldLabel {
                        at: at + Vec3::Y * 1.2,
                        follow: None,
                        owner: root,
                    });
                    extras.extend([ring, desk, tag]);
                }
                if let Some(d) = &def {
                    let text = commands
                        .spawn(label_bundle(d.label.clone(), rgba(color, 1.0), 16.0))
                        .id();
                    commands.entity(text).insert(WorldLabel {
                        at: label_point(*shape, pos, *facing) + Vec3::Y * 0.4,
                        follow: None,
                        owner: root,
                    });
                    extras.push(text);
                }
                commands.entity(root).insert(Decal {
                    id: *id,
                    age: 0.0,
                    total: (*resolve_ms as f32 / 1000.0).max(0.05),
                    shape: *shape,
                    fill,
                    fill_mat,
                    color,
                    landed: None,
                    extras,
                });
            }
            Event::TelegraphResolved { id, landed } => {
                for mut d in &mut decals {
                    if d.id == *id {
                        d.landed = Some(if *landed { 0.0 } else { FLASH_SECS });
                    }
                }
            }
            _ => {}
        }
    }
}

fn grow_decals(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut decals: Query<(Entity, &mut Decal)>,
    mut fills: Query<&mut Transform>,
) {
    let dt = time.delta_secs();
    for (entity, mut d) in &mut decals {
        d.age += dt;
        let k = d.age / d.total;
        if let Ok(mut tf) = fills.get_mut(d.fill) {
            *tf = fill_transform(d.shape, k);
        }
        let landed = d.landed.map(|t| t + dt);
        d.landed = landed;
        let alpha = match landed {
            // The impact flash: bright, then gone.
            Some(t) => 0.85 * (1.0 - t / FLASH_SECS).max(0.0),
            // Brightens as it fills so the last moment is unmistakable.
            None => 0.2 + 0.3 * k.clamp(0.0, 1.0),
        };
        if let Some(mut m) = materials.get_mut(&d.fill_mat) {
            m.base_color = rgba(d.color, alpha);
        }
        let stale = landed.is_none() && d.age > d.total + STALE_SECS;
        if landed.is_some_and(|t| t >= FLASH_SECS) || stale {
            for e in d.extras.drain(..) {
                commands.entity(e).despawn();
            }
            commands.entity(entity).despawn();
        }
    }
}

/// One marker per unit casting a must-interrupt spell; it follows the unit until the cast ends.
fn kick_markers(
    mut commands: Commands,
    sim: SimView,
    assets: Res<TgAssets>,
    time: Res<Time>,
    units: Query<(&SimUnit, &Transform), Without<KickMarker>>,
    mut markers: Query<(Entity, &KickMarker, &mut Transform)>,
) {
    let kicking = |id: EntityId| {
        sim.unit(id).is_some_and(|u| {
            u.alive
                && u.cast.is_some_and(|c| {
                    c.interruptible && sim.content().spell(c.spell).is_some_and(|s| s.kick)
                })
        })
    };
    let pulse = 1.0 + 0.15 * (time.elapsed_secs() * 10.0).sin();
    let mut have = Vec::new();
    for (e, m, mut tf) in &mut markers {
        let at = units.iter().find(|(su, _)| su.0 == m.unit).map(|(_, t)| t);
        match at {
            Some(t) if kicking(m.unit) => {
                tf.translation = t.translation + Vec3::Y * 0.06;
                tf.scale = Vec3::splat(1.6 * pulse);
                have.push(m.unit);
            }
            _ => commands.entity(e).despawn(),
        }
    }
    for (su, t) in &units {
        if have.contains(&su.0) || !kicking(su.0) {
            continue;
        }
        let root = commands
            .spawn((
                KickMarker { unit: su.0 },
                Mesh3d(assets.ring.clone()),
                MeshMaterial3d(assets.kick_ring.clone()),
                ground(t.translation, 0.0, 0.06).with_scale(Vec3::splat(1.6)),
            ))
            .id();
        let head = sim.unit(su.0).map_or(2.0, |u| head_height(&u.key));
        let text = commands
            .spawn(label_bundle("INTERRUPT!".into(), KICK_RED, 22.0))
            .id();
        commands.entity(text).insert(WorldLabel {
            at: Vec3::Y * (head + 0.9),
            follow: Some(su.0),
            owner: root,
        });
    }
}

fn place_labels(
    mut commands: Commands,
    // The play camera only: preview cameras (role picker, character sheet) may coexist.
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    ui_scale: Res<UiScale>,
    owners: Query<(), Or<(With<Decal>, With<KickMarker>)>>,
    units: Query<(&SimUnit, &Transform)>,
    mut labels: Query<(Entity, &WorldLabel, &mut Node, &mut Visibility)>,
) {
    let (cam, cam_tf) = *camera;
    let scale = ui_scale.0.max(0.01);
    for (e, l, mut node, mut vis) in &mut labels {
        if owners.get(l.owner).is_err() {
            commands.entity(e).despawn();
            continue;
        }
        let base = match l.follow {
            Some(id) => match units.iter().find(|(su, _)| su.0 == id) {
                Some((_, t)) => t.translation,
                None => {
                    *vis = Visibility::Hidden;
                    continue;
                }
            },
            None => Vec3::ZERO,
        };
        let Ok(screen) = cam.world_to_viewport(cam_tf, base + l.at) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        node.left = px(screen.x / scale - LABEL_W / 2.0);
        node.top = px(screen.y / scale - 10.0);
    }
}

/// Attaches the shield arc once per shielded unit, then shows it only while the shield works
/// (not casting, not stunned, alive), matching the sim rule.
fn shield_arcs(
    mut commands: Commands,
    sim: SimView,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    fresh: Query<(Entity, &SimUnit), Without<ShieldChecked>>,
    mut arcs: Query<(&ShieldArc, &mut Visibility)>,
) {
    for (e, su) in &fresh {
        let Some(u) = sim.unit(su.0) else { continue };
        commands.entity(e).insert(ShieldChecked);
        let Some(fs) = sim
            .content()
            .unit_def(&u.key)
            .and_then(|d| d.frontal_shield)
        else {
            continue;
        };
        let mesh = meshes.add(CircularSector::new(1.8, (fs.arc_deg / 2.0).to_radians()));
        let mat = materials.add(unlit(Color::srgba(0.45, 0.75, 1.0, 0.4)));
        commands.entity(e).with_child((
            ShieldArc(su.0),
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            // Local -Z is the unit's front; the 2D sector opens along +Y before the tilt.
            Transform::from_xyz(0.0, 0.07, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            Visibility::Hidden,
        ));
    }
    for (arc, mut vis) in &mut arcs {
        let up = sim
            .unit(arc.0)
            .is_some_and(|u| u.alive && u.cast.is_none() && !u.stunned);
        vis.set_if_neq(if up {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}
