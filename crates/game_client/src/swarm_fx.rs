//! Swarm clusters on screen (docs/ILMARI_10.md C.3.1). The sim holds one unit per cluster; this
//! draws up to `Quality::drone_budget` drones from one pooled set of entities that share a mesh
//! and a material (so Bevy batches them). Drone positions are a pure function of
//! (cluster, index, time): the sim never sees one.
// Bevy system signatures are inherently wide.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatSim, SimReady, SimUnit, SimView};
use crate::settings::Settings;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use combat_core::SwarmDef;
use std::collections::HashMap;

pub struct SwarmFxPlugin;

impl Plugin for SwarmFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SwarmDrawn>()
            .add_systems(Startup, setup_drones)
            .add_systems(Update, (draw_drones, stage_swarm).in_set(SimReady));
    }
}

#[derive(Component)]
struct Drone;

#[derive(Resource)]
struct DroneAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    /// Entities spawned so far; a pool that never shrinks, so a wave costs no spawns twice.
    pool: Vec<Entity>,
}

/// Drones drawn this frame and drones the sim claims, for the bench line and the QA log.
#[derive(Resource, Default)]
pub struct SwarmDrawn {
    pub drawn: usize,
    pub claimed: usize,
    pub clusters: usize,
}

fn setup_drones(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(DroneAssets {
        mesh: meshes.add(Cuboid::new(0.14, 0.05, 0.14)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.75, 0.77, 0.8),
            emissive: LinearRgba::rgb(0.5, 0.03, 0.02),
            ..default()
        }),
        pool: Vec::new(),
    });
}

/// splitmix64 finaliser: cheap, stateless scatter for the drone offsets.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Drones to draw per cluster: the budget shared in proportion to the counts, never more than
/// the cluster has and never fewer than one so a small cluster stays visible.
pub fn shares(counts: &[u16], budget: usize) -> Vec<usize> {
    let total: usize = counts.iter().map(|&c| usize::from(c)).sum();
    counts
        .iter()
        .map(|&c| {
            let c = usize::from(c);
            if total <= budget {
                c
            } else {
                (c * budget / total).clamp(1, c.max(1))
            }
        })
        .collect()
}

/// Where drone `idx` of a cluster of `count` hovers around the cluster centre at time `t`.
fn offset(cluster: u32, idx: usize, count: u16, t: f32) -> Vec3 {
    let h = mix(u64::from(cluster) << 32 | idx as u64);
    let unit = |shift: u32| ((h >> shift) & 0xffff) as f32 / 65535.0;
    let reach = 0.8 + 0.12 * f32::from(count).sqrt();
    let phase = unit(32);
    let a = unit(0) * std::f32::consts::TAU + t * (0.4 + 0.5 * phase);
    let r = reach * unit(16).sqrt();
    Vec3::new(
        a.cos() * r,
        0.9 + unit(48) * 1.6 + (t * 3.0 + phase * 6.0).sin() * 0.12,
        a.sin() * r,
    )
}

fn draw_drones(
    time: Res<Time>,
    settings: Res<Settings>,
    view: SimView,
    units: Query<(&SimUnit, &Transform), Without<Drone>>,
    mut assets: ResMut<DroneAssets>,
    mut drawn: ResMut<SwarmDrawn>,
    mut commands: Commands,
    mut drones: Query<(&mut Transform, &mut Visibility), With<Drone>>,
    mut defs: Local<HashMap<String, Option<SwarmDef>>>,
) {
    let mut clusters: Vec<(u32, u16, Vec3)> = Vec::new();
    for u in view.units().filter(|u| u.alive && u.hostile) {
        let def = *defs.entry(u.key.clone()).or_insert_with(|| {
            view.content()
                .units
                .iter()
                .find(|d| d.key == u.key)
                .and_then(|d| d.swarm)
        });
        let Some(def) = def else { continue };
        let at = units
            .iter()
            .find(|(su, _)| su.0 == u.id)
            .map_or(Vec3::new(u.pos.x, u.pos.y, u.pos.z), |(_, t)| t.translation);
        clusters.push((u.id as u32, def.count(u.hp), at));
    }
    clusters.sort_by_key(|c| c.0);
    let counts: Vec<u16> = clusters.iter().map(|c| c.1).collect();
    let share = shares(&counts, settings.quality.drone_budget());
    let want: usize = share.iter().sum();
    *drawn = SwarmDrawn {
        drawn: want,
        claimed: counts.iter().map(|&c| usize::from(c)).sum(),
        clusters: clusters.len(),
    };
    while assets.pool.len() < want {
        let e = commands
            .spawn((
                Drone,
                NotShadowCaster,
                Mesh3d(assets.mesh.clone()),
                MeshMaterial3d(assets.material.clone()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();
        assets.pool.push(e);
    }
    let t = time.elapsed_secs();
    let mut slot = 0;
    for (&(id, count, at), &n) in clusters.iter().zip(&share) {
        for idx in 0..n {
            if let Ok((mut tf, mut vis)) = drones.get_mut(assets.pool[slot]) {
                tf.translation = at + offset(id, idx, count, t);
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
            }
            slot += 1;
        }
    }
    for &e in &assets.pool[slot.min(assets.pool.len())..] {
        if let Ok((_, mut vis)) = drones.get_mut(e)
            && *vis != Visibility::Hidden
        {
            *vis = Visibility::Hidden;
        }
    }
}

/// QA and bench staging: `WOW_SWARM=<clusters>` (the `swarm` bench implies 32) drops that many
/// 75-drone clusters in a ring around the player, plus five drafts for them to eat. Offline only.
fn stage_swarm(time: Res<Time>, mut sim: ResMut<CombatSim>, mut done: Local<bool>) {
    if *done || time.elapsed_secs() < 1.5 {
        return;
    }
    let n: u32 = std::env::var("WOW_SWARM")
        .ok()
        .and_then(|v| v.parse().ok())
        .or_else(|| (std::env::var("WOW_BENCH").is_ok_and(|v| v == "swarm")).then_some(32))
        .unwrap_or(0);
    *done = true;
    let Some(me) = sim.0.player() else { return };
    let Some(c) = sim.0.unit(me).map(|u| u.pos) else {
        return;
    };
    for k in 0..5 {
        let a = k as f32 / 5.0 * std::f32::consts::TAU;
        sim.0.add_draft(combat_core::Vec3::new(
            c.x + a.cos() * 3.0,
            c.y,
            c.z + a.sin() * 3.0,
        ));
    }
    for k in 0..n {
        let a = k as f32 / n.max(1) as f32 * std::f32::consts::TAU;
        let r = 7.0 + (k % 3) as f32 * 4.0;
        let at = combat_core::Vec3::new(c.x + a.cos() * r, c.y, c.z + a.sin() * r);
        if sim.0.spawn_swarm("il_scraper_swarm", at, 75).is_err() {
            warn!("swarm: il_scraper_swarm is not in the content");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_is_shared_by_count_and_never_exceeded() {
        let counts = [125, 125, 125, 25];
        let s = shares(&counts, 200);
        assert!(s.iter().sum::<usize>() <= 200);
        assert!(s[0] > s[3] && s[3] >= 1);
        assert_eq!(shares(&[10, 5], 600), vec![10, 5]);
    }

    #[test]
    fn offsets_are_stable_and_bounded() {
        let a = offset(7, 3, 125, 1.0);
        assert_eq!(a, offset(7, 3, 125, 1.0));
        assert!(a.x.hypot(a.z) <= 0.8 + 0.12 * 125f32.sqrt() + 1e-3);
    }
}
