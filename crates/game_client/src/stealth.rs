//! Tailgating look: a stealthed player (yours or a co-op teammate's) turns translucent. The
//! state itself lives in the sim as an aura, so every client derives the look from the same
//! snapshot; nothing here is replicated on its own.

use crate::combat::{SimReady, SimUnit, SimView};
use crate::net::RemotePlayer;
use crate::player::Player;
use bevy::prelude::*;
use std::collections::HashMap;

/// You still need to see yourself; teammates only get a faint ghost.
const ALPHA_SELF: f32 = 0.45;
const ALPHA_OTHER: f32 = 0.3;

/// Swapped-in translucent materials of one cloaked character: mesh entity -> original handle.
#[derive(Component, Default)]
struct Cloak(HashMap<Entity, Handle<StandardMaterial>>);

pub struct StealthPlugin;

impl Plugin for StealthPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, cloak.in_set(SimReady));
    }
}

type Wearer = (
    Entity,
    &'static SimUnit,
    Option<&'static mut Cloak>,
    Has<Player>,
);

type IsPlayer = Or<(With<Player>, With<RemotePlayer>)>;

fn cloak(
    mut commands: Commands,
    sim: SimView,
    mut wearers: Query<Wearer, IsPlayer>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    respawned: Query<Entity, Added<crate::anim::ModelSpawned>>,
) {
    for (e, su, mut cloaked, is_player) in &mut wearers {
        let hidden = sim.unit(su.0).is_some_and(|u| {
            u.alive
                && u.auras.iter().any(|a| {
                    sim.content()
                        .aura(&a.key)
                        .is_some_and(|d| d.stealth_aggro_pct > 0)
                })
        });
        // A role change spawns a new model: the old mesh entities are gone, start over.
        if respawned.contains(e) && cloaked.is_some() {
            commands.entity(e).remove::<Cloak>();
            continue;
        }
        if hidden {
            let mut fresh = Cloak::default();
            let state = cloaked.as_deref_mut().unwrap_or(&mut fresh);
            let alpha = if is_player { ALPHA_SELF } else { ALPHA_OTHER };
            // Re-scanned every frame: gear and class kit attach a moment after the model.
            for m in children.iter_descendants(e) {
                if state.0.contains_key(&m) {
                    continue;
                }
                let Ok(mut handle) = meshes.get_mut(m) else {
                    continue;
                };
                let Some(mut ghost) = materials.get(&handle.0).cloned() else {
                    continue;
                };
                ghost.base_color = ghost.base_color.with_alpha(alpha);
                ghost.alpha_mode = AlphaMode::Blend;
                state.0.insert(m, handle.0.clone());
                handle.0 = materials.add(ghost);
            }
            if cloaked.is_none() && !fresh.0.is_empty() {
                commands.entity(e).insert(fresh);
            }
        } else if let Some(c) = cloaked {
            for (m, original) in &c.0 {
                if let Ok(mut handle) = meshes.get_mut(*m) {
                    handle.0 = original.clone();
                }
            }
            commands.entity(e).remove::<Cloak>();
        }
    }
}
