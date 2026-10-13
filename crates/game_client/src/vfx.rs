//! Spell visuals: simple meshes with unlit colour materials (WebGL2 friendly).
//! Persistent effects (cast glow, shield, telegraph) read live sim state;
//! one-shot effects (orb, impact, bursts) are spawned from sim events.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, CombatSim, SimReady, SimUnit};
use crate::combat_anim::CombatAnim;
use crate::config::*;
use crate::player::Player;
use crate::presentation::{PresentEvent, PresentQueue};
use crate::spell_fx::{self, Fx, FxBudget, ProjKind, TicketInfo};
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
                update_telegraph,
                update_stun_stars,
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
    star: Handle<StandardMaterial>,
}

#[derive(Component)]
struct VfxAttached;
#[derive(Component)]
struct HandGlow;
#[derive(Component)]
struct Telegraph(EntityId);
/// Spinning stars over a stunned unit's head.
#[derive(Component)]
struct StunStars(EntityId);

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

fn setup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(VfxAssets {
        sphere: meshes.add(Sphere::new(1.0)),
        ring: meshes.add(Annulus::new(0.9, 1.0)),
        star: materials.add(glow(rgb(STUN_COLOR))),
    });
}

/// Persistent effects hang off their owners once the bridge has linked them to the sim.
fn attach_vfx(
    mut commands: Commands,
    assets: Res<VfxAssets>,
    fx: Option<Res<Fx>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sim: Res<CombatSim>,
    q: Query<(Entity, &SimUnit, Option<&Unit>, Has<Player>), Without<VfxAttached>>,
) {
    let Some(fx) = fx else { return };
    for (entity, sim_unit, unit, is_player) in &q {
        let mut e = commands.entity(entity);
        e.insert(VfxAttached);
        let head = sim
            .0
            .unit(sim_unit.0)
            .map_or(PLAYER_HEIGHT, |u| head_height(&u.key));
        e.with_children(|c| {
            c.spawn((
                StunStars(sim_unit.0),
                Transform::from_xyz(0.0, head + 0.25, 0.0),
                Visibility::Hidden,
            ))
            .with_children(|stars| {
                for k in 0..3 {
                    let a = k as f32 * std::f32::consts::TAU / 3.0;
                    stars.spawn((
                        Mesh3d(assets.sphere.clone()),
                        MeshMaterial3d(assets.star.clone()),
                        Transform::from_xyz(0.32 * a.cos(), 0.0, 0.32 * a.sin())
                            .with_scale(Vec3::splat(0.07)),
                    ));
                }
            });
            spell_fx::attach_unit_fx(c, &fx, sim_unit.0, head, is_player);
        });
        if is_player {
            let glow_mat = materials.add(glow(Color::srgba(0.5, 0.8, 1.0, 0.9)));
            e.with_children(|c| {
                c.spawn((
                    HandGlow,
                    Mesh3d(assets.sphere.clone()),
                    MeshMaterial3d(glow_mat),
                    Transform::from_translation(Vec3::from(HAND_GLOW_POS))
                        .with_scale(Vec3::splat(HAND_GLOW_RADIUS)),
                    Visibility::Hidden,
                ));
            });
        } else if unit.is_some_and(|u| u.hostile) {
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

/// Flat expanding ring on the ground plane (level-up burst).
fn spawn_ring(
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
        Mesh3d(assets.ring.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(at)
            .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
            .with_scale(Vec3::splat(from)),
    ));
}

fn update_stun_stars(
    time: Res<Time>,
    sim: Res<CombatSim>,
    mut stars: Query<(&StunStars, &mut Visibility, &mut Transform)>,
) {
    for (s, mut vis, mut tf) in &mut stars {
        let stunned = sim.0.unit(s.0).is_some_and(|u| u.stunned && u.alive);
        vis.set_if_neq(if stunned {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if stunned {
            tf.rotation = Quat::from_rotation_y(time.elapsed_secs() * 4.0);
        }
    }
}

fn spawn_from_events(
    mut commands: Commands,
    mut events: MessageReader<CombatEvent>,
    sim: Res<CombatSim>,
    assets: Res<VfxAssets>,
    fx: Option<Res<Fx>>,
    mut budget: ResMut<FxBudget>,
    mut tickets: ResMut<TicketInfo>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Single<&SimUnit, With<Player>>,
    mut queue: ResMut<PresentQueue>,
    mut present: MessageWriter<PresentEvent>,
    mut launch: Local<Option<Launch>>,
    mut blame: Local<BlameTrack>,
    units: Query<(&SimUnit, &Transform)>,
) {
    let Some(fx) = fx else { return };
    let head_of = |id: EntityId| {
        sim.0
            .unit(id)
            .map_or(PLAYER_HEIGHT, |u| head_height(&u.key))
    };
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::CastSucceeded {
                caster,
                spell,
                target,
            } => {
                let Some(from) = unit_pos(units.iter(), *caster) else {
                    continue;
                };
                let tgt = target.and_then(|t| Some((t, unit_pos(units.iter(), t)?)));
                if let Some(kind) = spell_fx::proj_kind(spell.0)
                    && let Some((t, tpos)) = tgt
                {
                    let ticket = spell_fx::launch(
                        &mut commands,
                        &fx,
                        &mut budget,
                        &mut queue,
                        kind,
                        from,
                        (t, tpos),
                    );
                    *launch = Some(Launch {
                        source: *caster,
                        target: t,
                        spell: Some(*spell),
                        ticket,
                    });
                }
                if *caster == player.0 {
                    let tinfo = tgt.map(|(t, p)| (p, head_of(t)));
                    spell_fx::cast_instant(&mut commands, &fx, &mut budget, spell.0, from, tinfo);
                    if spell.0 == BLAME_SPELL {
                        blame.cast_pending = true;
                    }
                }
            }
            // Only the player's auto attack is ranged; enemy melee needs no projectile.
            Event::MeleeSwing { attacker, target } if *attacker == player.0 => {
                let (Some(from), Some(tpos)) = (
                    unit_pos(units.iter(), *attacker),
                    unit_pos(units.iter(), *target),
                ) else {
                    continue;
                };
                let ticket = spell_fx::launch(
                    &mut commands,
                    &fx,
                    &mut budget,
                    &mut queue,
                    ProjKind::Quill,
                    from,
                    (*target, tpos),
                );
                *launch = Some(Launch {
                    source: *attacker,
                    target: *target,
                    spell: None,
                    ticket,
                });
            }
            Event::Damage {
                source,
                target,
                spell,
                combo,
                amount,
                absorbed,
                ..
            } => {
                let held = launch.as_ref().is_some_and(|l| {
                    l.source == *source && l.target == *target && l.spell == *spell
                });
                match launch.take().filter(|_| held) {
                    Some(l) => {
                        // The sim state is post-hit: undo the damage to see if this was an execute.
                        let execute = sim.0.unit(*target).is_some_and(|u| {
                            (u.hp + amount + absorbed) as f32 / (u.max_hp.max(1) as f32) < 0.25
                        });
                        spell_fx::note_hit(&mut tickets, l.ticket, *combo, execute);
                        queue.hold(l.ticket, ev.clone());
                    }
                    None => {
                        present.write(PresentEvent(ev.clone()));
                    }
                }
            }
            Event::Miss {
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
            Event::CastInterrupted { caster, by, .. } => {
                if let Some(at) = unit_pos(units.iter(), *caster) {
                    spawn_transient(
                        &mut commands,
                        &assets,
                        &mut materials,
                        at + Vec3::Y * PROJECTILE_TARGET_Y,
                        (0.3, 1.6, BURST_SECS),
                        rgb(INTERRUPT_COLOR).with_alpha(0.8),
                    );
                    if *by == Some(player.0) {
                        spell_fx::interrupt_ring(&mut commands, &fx, &mut budget, at);
                    }
                }
            }
            Event::Summoned { unit, .. } => {
                if let Some(u) = sim.0.unit(*unit) {
                    spell_fx::summon_puff(
                        &mut commands,
                        &fx,
                        &mut budget,
                        Vec3::new(u.pos.x, u.pos.y, u.pos.z),
                    );
                }
            }
            Event::AuraApplied { target, key } if key == "git_blame" => {
                // A blame without a cast of ours is the spread: jump from where the last unit died.
                if std::mem::take(&mut blame.cast_pending) {
                    continue;
                }
                if let (Some(from), Some(to)) = (blame.last_death, unit_pos(units.iter(), *target))
                {
                    spell_fx::spread_arc(&mut commands, &fx, &mut budget, from, to);
                }
            }
            Event::LevelUp { unit, .. } if *unit == player.0 => {
                if let Some(at) = unit_pos(units.iter(), *unit) {
                    let gold = rgb(LEVEL_UP_COLOR);
                    for (from, to, life, y) in [(0.4, 3.2, 0.9, 0.1), (0.2, 2.2, 1.2, 1.0)] {
                        spawn_ring(
                            &mut commands,
                            &assets,
                            &mut materials,
                            at + Vec3::Y * y,
                            (from, to, life),
                            gold.with_alpha(0.9),
                        );
                    }
                    spawn_transient(
                        &mut commands,
                        &assets,
                        &mut materials,
                        at + Vec3::Y * 1.0,
                        (0.5, 1.8, 0.8),
                        gold.with_alpha(0.45),
                    );
                }
            }
            Event::Death { unit, .. } => {
                if let Some(at) = unit_pos(units.iter(), *unit) {
                    blame.last_death = Some(at);
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

/// Player's git blame spell id (content/game.ron); spread detection keys off it.
const BLAME_SPELL: u32 = 6;

#[derive(Default)]
struct BlameTrack {
    cast_pending: bool,
    last_death: Option<Vec3>,
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
