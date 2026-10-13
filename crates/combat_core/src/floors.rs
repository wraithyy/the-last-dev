//! Pack formation, the legacy floor loader and unit removal (adds, despawn).

use crate::content::ContentError;
use crate::geom::{Aabb, Vec3};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{AiState, AuraEnd, Event};

/// Fixed formation slots in world XZ metres; slot 0 is the anchor itself.
const SLOTS: [(f32, f32); 8] = [
    (0.0, 0.0),
    (1.8, 0.9),
    (-1.8, 0.9),
    (0.0, 2.0),
    (1.8, -1.2),
    (-1.8, -1.2),
    (3.2, 0.0),
    (-3.2, 0.0),
];

/// Deterministic `(dx, dz)` offsets for `n` units around a pack anchor (world axes, not
/// rotated by the anchor yaw). Shared by the sim and the client so both agree.
pub fn pack_offsets(n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|k| {
            SLOTS
                .get(k)
                .copied()
                .unwrap_or_else(|| (1.5 * (k - SLOTS.len()) as f32 - 3.0, 3.5))
        })
        .collect()
}

impl Sim {
    /// Legacy entry point: loads zone `tower/<floor>` (see `load_zone`).
    pub fn load_floor(
        &mut self,
        floor: u8,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
        blockers: Vec<Aabb>,
    ) -> Result<(), ContentError> {
        self.load_zone(&format!("tower/{floor}"), anchors, blockers)
    }

    /// Where `RespawnPlayer` puts the player; `load_zone` sets it from the `graveyard` anchor.
    pub fn set_respawn_point(&mut self, pos: Vec3, yaw: f32) {
        if let Some(i) = self.player.and_then(|p| self.index(p)) {
            self.units[i].spawn_pos = pos;
            self.units[i].spawn_yaw = yaw;
        }
    }

    /// A summoner's death takes its living adds with it: no killer, so no XP, loot or death
    /// spell. Floor-boss adds also vanish so the cleared floor is left tidy.
    pub(crate) fn kill_adds_of(&mut self, summoner: EntityId) {
        let tidy = self
            .index(summoner)
            .is_some_and(|s| self.units[s].floor_boss);
        for a in 0..self.units.len() {
            let u = &self.units[a];
            if u.summoner != Some(summoner) || u.despawned {
                continue;
            }
            if u.state.alive {
                self.kill_add(a);
                self.kill_adds_of(Self::id_of(a));
            }
            if tidy {
                self.despawn(a);
            }
        }
    }

    /// Living adds leave without a corpse when their summoner resets.
    pub(crate) fn despawn_adds_of(&mut self, summoner: EntityId) {
        for a in 0..self.units.len() {
            let u = &self.units[a];
            if u.summoner == Some(summoner) && u.state.alive && !u.despawned {
                self.despawn_adds_of(Self::id_of(a));
                self.despawn(a);
            }
        }
    }

    fn kill_add(&mut self, a: usize) {
        let id = Self::id_of(a);
        let keys: Vec<String> = self.units[a].auras.iter().map(|x| x.key.clone()).collect();
        for k in keys {
            self.remove_aura(a, &k, AuraEnd::Death);
        }
        self.cancel_cast(a, None, 0);
        self.set_auto_attack(a, false);
        for o in 0..self.units.len() {
            self.units[o].threat.remove(&id);
            if self.units[o].state.target == Some(id) {
                self.set_auto_attack(o, false);
            }
        }
        let u = &mut self.units[a];
        u.state.alive = false;
        u.state.hp = 0;
        u.queued = None;
        u.moving = false;
        u.threat.clear();
        u.state.target = None;
        u.respawn_at = None;
        self.events.push(Event::Death {
            unit: id,
            killer: None,
        });
        if self.units[a].state.ai.is_some() {
            self.units[a].state.ai = Some(AiState::Dead);
            self.events.push(Event::AiState {
                unit: id,
                state: AiState::Dead,
            });
        }
    }

    /// Removes a unit for good; its id is never reused and stops resolving in `unit()`.
    pub(crate) fn despawn(&mut self, i: usize) {
        let id = Self::id_of(i);
        self.cancel_cast(i, None, 0);
        self.set_auto_attack(i, false);
        let u = &mut self.units[i];
        u.despawned = true;
        u.state.alive = false;
        u.state.in_combat = false;
        u.state.lootable = false;
        u.auras.clear();
        u.threat.clear();
        u.respawn_at = None;
        u.queued = None;
        for o in 0..self.units.len() {
            self.units[o].threat.remove(&id);
            if self.units[o].state.target == Some(id) {
                self.set_auto_attack(o, false);
                self.units[o].state.target = None;
            }
        }
        self.events.push(Event::Despawned { unit: id });
    }
}

#[cfg(test)]
mod tests {
    use super::pack_offsets;

    #[test]
    fn offsets_are_distinct_and_start_at_the_anchor() {
        let o = pack_offsets(10);
        assert_eq!(o[0], (0.0, 0.0));
        for (a, x) in o.iter().enumerate() {
            for y in &o[a + 1..] {
                assert_ne!(x, y);
            }
        }
        assert!(pack_offsets(0).is_empty());
    }
}
