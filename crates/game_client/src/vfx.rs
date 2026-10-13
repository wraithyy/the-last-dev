//! Spell visuals: simple meshes with unlit colour materials (WebGL2 friendly).
//! Persistent effects (cast glow, shield, telegraph) read live sim state;
//! one-shot effects (orb, impact, bursts) are spawned from sim events.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, CombatSim, SimReady, SimUnit};
use crate::combat_anim::CombatAnim;
use crate::config::*;
use crate::player::Player;
use crate::presentation::{AUTO_ATTACK_COLOR, PresentEvent, PresentQueue};
use crate::units::Unit;
use bevy::prelude::*;
use combat_core::{EntityId, Event, SpellId};
use std::f32::consts::FRAC_PI_2;

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_assets).add_systems(
            Update,
            (
                attach_vfx,
                spawn_from_events,
                update_hand_glow,
                update_shield,
                update_telegraph,
                move_projectiles,
                fade_transients,
            )
                .chain()
                .in_set(SimReady),
        );
    }
}

#[derive(Resource)]
struct VfxAssets {
    sphere: Handle<Mesh>,
    ring: Handle<Mesh>,
}

#[derive(Component)]
struct VfxAttached;
#[derive(Component)]
struct HandGlow;
#[derive(Component)]
struct ShieldBubble;
#[derive(Component)]
struct Telegraph(EntityId);

#[derive(Component)]
struct Projectile {
    target: EntityId,
    color: Color,
    /// Presentation events held until this projectile lands.
    ticket: u64,
}

/// The projectile launched by the latest swing/cast, so its Damage/Miss can be held for impact.
struct Launch {
    source: EntityId,
    target: EntityId,
    spell: Option<SpellId>,
    ticket: u64,
}

/// Expanding, fading one-shot (impact flash, interrupt burst, death fade).
#[derive(Component)]
struct Transient {
    age: f32,
    life: f32,
    from: f32,
    to: f32,
    color: Color,
    material: Handle<StandardMaterial>,
}

fn rgb(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
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

fn setup_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(VfxAssets {
        sphere: meshes.add(Sphere::new(1.0)),
        ring: meshes.add(Annulus::new(0.9, 1.0)),
    });
}

/// Persistent effects hang off their owners once the bridge has linked them to the sim.
fn attach_vfx(
    mut commands: Commands,
    assets: Res<VfxAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q: Query<(Entity, &SimUnit, &Unit, Has<Player>), Without<VfxAttached>>,
) {
    for (entity, sim_unit, unit, is_player) in &q {
        let mut e = commands.entity(entity);
        e.insert(VfxAttached);
        if is_player {
            let glow_mat = materials.add(glow(Color::srgba(0.5, 0.8, 1.0, 0.9)));
            let shield_mat = materials.add(glow(rgb(SHIELD_COLOR).with_alpha(0.22)));
            e.with_children(|c| {
                c.spawn((
                    HandGlow,
                    Mesh3d(assets.sphere.clone()),
                    MeshMaterial3d(glow_mat),
                    Transform::from_translation(Vec3::from(HAND_GLOW_POS))
                        .with_scale(Vec3::splat(HAND_GLOW_RADIUS)),
                    Visibility::Hidden,
                ));
                c.spawn((
                    ShieldBubble,
                    Mesh3d(assets.sphere.clone()),
                    MeshMaterial3d(shield_mat),
                    Transform::from_xyz(0.0, SHIELD_CENTER_Y, 0.0)
                        .with_scale(Vec3::splat(SHIELD_RADIUS)),
                    Visibility::Hidden,
                ));
            });
        } else if unit.hostile {
            let mat = materials.add(glow(rgb(TELEGRAPH_COLOR).with_alpha(0.6)));
            e.with_children(|c| {
                c.spawn((
                    Telegraph(sim_unit.0),
                    Mesh3d(assets.ring.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(0.0, 0.06, 0.0)
                        .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
                    Visibility::Hidden,
                ));
            });
        }
    }
}

fn unit_pos<'a>(
    mut units: impl Iterator<Item = (&'a SimUnit, &'a Transform)>,
    id: EntityId,
) -> Option<Vec3> {
    units
        .find(|(su, _)| su.0 == id)
        .map(|(_, tf)| tf.translation)
}

fn spawn_transient(
    commands: &mut Commands,
    assets: &VfxAssets,
    materials: &mut Assets<StandardMaterial>,
    at: Vec3,
    (from, to, life): (f32, f32, f32),
    color: Color,
) {
    let material = materials.add(glow(color));
    commands.spawn((
        Transient {
            age: 0.0,
            life,
            from,
            to,
            color,
            material: material.clone(),
        },
        Mesh3d(assets.sphere.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(at).with_scale(Vec3::splat(from)),
    ));
}

fn spawn_from_events(
    mut commands: Commands,
    mut events: MessageReader<CombatEvent>,
    sim: Res<CombatSim>,
    assets: Res<VfxAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Single<&SimUnit, With<Player>>,
    mut queue: ResMut<PresentQueue>,
    mut present: MessageWriter<PresentEvent>,
    mut launch: Local<Option<Launch>>,
    units: Query<(&SimUnit, &Transform)>,
) {
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::CastSucceeded {
                caster,
                spell,
                target: Some(target),
            } => {
                let Some(def) = sim.0.content().spell(*spell) else {
                    continue;
                };
                let Some((_, rgb3)) = PROJECTILE_SPELLS.iter().find(|(k, _)| *k == def.icon) else {
                    continue;
                };
                launch_projectile(
                    &mut commands,
                    &assets,
                    &mut materials,
                    &mut queue,
                    &mut launch,
                    &units,
                    (*caster, *target, Some(*spell)),
                    rgb(*rgb3),
                );
            }
            // Only the player's auto attack is ranged; enemy melee needs no projectile.
            Event::MeleeSwing { attacker, target } if *attacker == player.0 => {
                launch_projectile(
                    &mut commands,
                    &assets,
                    &mut materials,
                    &mut queue,
                    &mut launch,
                    &units,
                    (*attacker, *target, None),
                    rgb(AUTO_ATTACK_COLOR),
                );
            }
            Event::Damage {
                source,
                target,
                spell,
                ..
            }
            | Event::Miss {
                source,
                target,
                spell,
            } => {
                let held = launch.as_ref().is_some_and(|l| {
                    l.source == *source && l.target == *target && l.spell == *spell
                });
                match launch.take().filter(|_| held) {
                    Some(l) => queue.hold(l.ticket, ev.clone()),
                    None => {
                        present.write(PresentEvent(ev.clone()));
                    }
                }
            }
            Event::CastInterrupted { caster, .. } => {
                if let Some(at) = unit_pos(units.iter(), *caster) {
                    spawn_transient(
                        &mut commands,
                        &assets,
                        &mut materials,
                        at + Vec3::Y * PROJECTILE_TARGET_Y,
                        (0.3, 1.6, BURST_SECS),
                        rgb(INTERRUPT_COLOR).with_alpha(0.8),
                    );
                }
            }
            Event::Death { unit, .. } => {
                if let Some(at) = unit_pos(units.iter(), *unit) {
                    spawn_transient(
                        &mut commands,
                        &assets,
                        &mut materials,
                        at + Vec3::Y * PROJECTILE_TARGET_Y,
                        (0.6, 1.8, DEATH_FADE_SECS),
                        rgb(DEATH_FADE_COLOR).with_alpha(0.55),
                    );
                }
            }
            _ => {}
        }
    }
}

/// Spawns the orb and remembers it, so the Damage/Miss that follows is held until impact.
fn launch_projectile(
    commands: &mut Commands,
    assets: &VfxAssets,
    materials: &mut Assets<StandardMaterial>,
    queue: &mut PresentQueue,
    launch: &mut Option<Launch>,
    units: &Query<(&SimUnit, &Transform)>,
    (source, target, spell): (EntityId, EntityId, Option<SpellId>),
    color: Color,
) {
    let Some(from) = unit_pos(units.iter(), source) else {
        return;
    };
    let ticket = queue.ticket();
    let material = materials.add(glow(color.with_alpha(0.95)));
    commands.spawn((
        Projectile {
            target,
            color,
            ticket,
        },
        Mesh3d(assets.sphere.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(from + Vec3::Y * PROJECTILE_ORIGIN_Y)
            .with_scale(Vec3::splat(PROJECTILE_RADIUS)),
    ));
    *launch = Some(Launch {
        source,
        target,
        spell,
        ticket,
    });
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<VfxAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut queue: ResMut<PresentQueue>,
    mut present: MessageWriter<PresentEvent>,
    units: Query<(&SimUnit, &Transform), Without<Projectile>>,
    mut orbs: Query<(Entity, &Projectile, &mut Transform)>,
) {
    let mut land = |orb: &Projectile| {
        for ev in queue.release(orb.ticket) {
            present.write(PresentEvent(ev));
        }
    };
    for (entity, orb, mut tf) in &mut orbs {
        let Some(dest) = unit_pos(units.iter(), orb.target) else {
            commands.entity(entity).despawn();
            land(orb);
            continue;
        };
        let dest = dest + Vec3::Y * PROJECTILE_TARGET_Y;
        let step = PROJECTILE_SPEED * time.delta_secs();
        let to = dest - tf.translation;
        if to.length() <= step.max(0.05) {
            commands.entity(entity).despawn();
            land(orb);
            spawn_transient(
                &mut commands,
                &assets,
                &mut materials,
                dest,
                (0.2, 0.9, IMPACT_SECS),
                orb.color.with_alpha(0.9),
            );
        } else {
            tf.translation += to.normalize() * step;
        }
    }
}

fn update_hand_glow(
    time: Res<Time>,
    player: Single<&CombatAnim, With<Player>>,
    mut glows: Query<(&mut Visibility, &mut Transform), With<HandGlow>>,
) {
    for (mut vis, mut tf) in &mut glows {
        *vis = if player.casting {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let pulse = 1.0 + 0.35 * (time.elapsed_secs() * 14.0).sin();
        tf.scale = Vec3::splat(HAND_GLOW_RADIUS * pulse);
    }
}

fn update_shield(
    time: Res<Time>,
    sim: Res<CombatSim>,
    player: Single<&SimUnit, With<Player>>,
    mut bubbles: Query<(&mut Visibility, &mut Transform), With<ShieldBubble>>,
) {
    let active = sim.0.unit(player.0).is_some_and(|u| {
        u.auras
            .iter()
            .any(|a| a.absorb_remaining.is_some_and(|n| n > 0))
    });
    for (mut vis, mut tf) in &mut bubbles {
        *vis = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        tf.scale = Vec3::splat(SHIELD_RADIUS * (1.0 + 0.03 * (time.elapsed_secs() * 3.0).sin()));
    }
}

/// The ring tightens as the enemy cast progresses, so it doubles as a second cast indicator.
fn update_telegraph(
    sim: Res<CombatSim>,
    mut rings: Query<(&Telegraph, &mut Visibility, &mut Transform)>,
) {
    for (ring, mut vis, mut tf) in &mut rings {
        let cast = sim.0.unit(ring.0).and_then(|u| u.cast);
        match cast {
            Some(c) if c.total_ms > 0 => {
                *vis = Visibility::Inherited;
                let progress = c.elapsed_ms as f32 / c.total_ms as f32;
                tf.scale = Vec3::splat(TELEGRAPH_RADIUS * (1.3 - 0.5 * progress.clamp(0.0, 1.0)));
            }
            _ => *vis = Visibility::Hidden,
        }
    }
}

fn fade_transients(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(Entity, &mut Transient, &mut Transform)>,
) {
    for (entity, mut t, mut tf) in &mut q {
        t.age += time.delta_secs();
        let k = (t.age / t.life).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(t.from + (t.to - t.from) * k);
        if let Some(mut m) = materials.get_mut(&t.material) {
            m.base_color = t.color.with_alpha(t.color.alpha() * (1.0 - k));
        }
        if k >= 1.0 {
            commands.entity(entity).despawn();
        }
    }
}
