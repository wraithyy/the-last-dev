//! Floors: one call swaps the whole set of non-player units for a floor's packs, boss and NPCs.

use crate::content::ContentError;
use crate::geom::{Aabb, Vec3};
use crate::sim::{Sim, SpawnOpts};
use crate::types::Event;

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

struct Planned {
    def: usize,
    pos: Vec3,
    yaw: f32,
    level: u8,
    pack: Option<u32>,
    boss: bool,
}

impl Sim {
    /// Despawns every non-player unit (emitting `Despawned`), replaces the blockers and
    /// spawns the floor. Pack units alternate between the floor's level range (first half of
    /// the packs at the low level, the rest at the high level); boss and friendlies keep their
    /// authored level. Floor spawns never respawn. All anchors are resolved before anything
    /// changes, so an error leaves the sim untouched. Events are returned by the next `tick`.
    pub fn load_floor(
        &mut self,
        floor: u8,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
        blockers: Vec<Aabb>,
    ) -> Result<(), ContentError> {
        let f = self
            .content
            .floor(floor)
            .cloned()
            .ok_or_else(|| ContentError::Invalid(format!("unknown floor {floor}")))?;
        let anchor = |a: &str| {
            anchors(a).ok_or_else(|| ContentError::Invalid(format!("floor {floor}: no anchor {a}")))
        };
        let (lo, hi) = f.level_range;
        let mut plan = Vec::new();
        let mut pack_id = self.next_pack;
        for (k, p) in f.packs.iter().enumerate() {
            let (at, yaw) = anchor(&p.anchor)?;
            let level = if k * 2 < f.packs.len() { lo } else { hi };
            for (key, (dx, dz)) in p.units.iter().zip(pack_offsets(p.units.len())) {
                plan.push(Planned {
                    def: self.def_index(key)?,
                    pos: Vec3::new(at.x + dx, at.y, at.z + dz),
                    yaw,
                    level,
                    pack: Some(pack_id),
                    boss: false,
                });
            }
            pack_id += 1;
        }
        let (at, yaw) = anchor(&f.boss.anchor)?;
        let def = self.def_index(&f.boss.unit)?;
        plan.push(Planned {
            def,
            pos: at,
            yaw,
            level: self.content.units[def].level,
            pack: Some(pack_id),
            boss: true,
        });
        pack_id += 1;
        for s in &f.friendly {
            let (at, yaw) = anchor(&s.anchor)?;
            let def = self.def_index(&s.unit)?;
            plan.push(Planned {
                def,
                pos: at,
                yaw,
                level: self.content.units[def].level,
                pack: None,
                boss: false,
            });
        }

        let player = self.player.and_then(|p| self.index(p));
        // A cast started on the old floor would land on a unit that no longer exists.
        if let Some(pi) = player {
            if self.units[pi].state.cast.is_some() {
                self.cancel_cast(pi, None, 0);
            }
            self.units[pi].queued = None;
        }
        for i in 0..self.units.len() {
            if Some(i) != player && !self.units[i].despawned {
                self.despawn(i);
            }
        }
        self.blockers = blockers;
        self.floor = Some(floor);
        self.next_pack = pack_id;
        for p in plan {
            self.spawn(
                p.def,
                p.pos,
                p.yaw,
                SpawnOpts {
                    level: Some(p.level),
                    pack: p.pack,
                    respawns: false,
                    summoner: None,
                    floor_boss: p.boss,
                },
            );
        }
        self.events.push(Event::FloorLoaded { floor });
        Ok(())
    }

    /// Where `RespawnPlayer` puts the player (e.g. the floor's elevator).
    pub fn set_respawn_point(&mut self, pos: Vec3, yaw: f32) {
        if let Some(i) = self.player.and_then(|p| self.index(p)) {
            self.units[i].spawn_pos = pos;
            self.units[i].spawn_yaw = yaw;
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
