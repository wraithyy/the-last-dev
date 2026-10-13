//! Zones: loading one swaps every non-player unit for its packs, boss and NPCs. The sim keeps
//! per-session world state across loads and player deaths: cleared zones (their boss stays dead),
//! recently killed trash, killed bosses and which gated links have opened.

use crate::content::{ContentError, Objective, Unlock, ticks};
use crate::floors::pack_offsets;
use crate::geom::{Aabb, Vec3};
use crate::sim::{Sim, SpawnOpts};
use crate::types::Event;
use std::collections::{BTreeMap, BTreeSet};

/// Trash killed in a zone stays dead if the player re-enters within this much sim time after
/// leaving it; after that the whole zone respawns (and gives XP again).
pub const TRASH_RESPAWN_MS: u32 = 300_000;

#[derive(Default)]
pub(crate) struct ZoneState {
    /// Pack slots (pack units numbered in content order) killed since the last fresh spawn.
    dead_trash: BTreeSet<usize>,
    left_at: Option<u64>,
}

#[derive(Default)]
pub(crate) struct World {
    zones: BTreeMap<String, ZoneState>,
    pub cleared: BTreeSet<String>,
    bosses_killed: BTreeSet<String>,
    /// Zones whose `Unlocked` event was already emitted.
    announced: BTreeSet<String>,
}

/// One exit of a zone as `Sim::zone_links` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneLinkView {
    pub anchor: String,
    pub to: String,
    pub unlocked: bool,
}

struct Planned {
    def: usize,
    pos: Vec3,
    yaw: f32,
    level: u8,
    pack: Option<u32>,
    boss: bool,
    slot: Option<usize>,
}

/// `tower/<n>` is legacy floor `n`.
pub(crate) fn floor_number(zone: &str) -> Option<u8> {
    zone.strip_prefix("tower/")?.parse().ok()
}

impl Sim {
    /// Despawns every non-player unit (emitting `Despawned`), replaces the blockers and spawns
    /// the zone. Pack units alternate between the zone's level range (first half of the packs
    /// low, the rest high); boss and friendlies keep their authored level. Zone spawns never
    /// respawn on their own; see `TRASH_RESPAWN_MS` for re-entry. A cleared zone's boss is not
    /// spawned again. The optional `graveyard` anchor becomes the player's respawn point.
    /// All anchors are resolved first, so an error leaves the sim untouched.
    pub fn load_zone(
        &mut self,
        id: &str,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
        blockers: Vec<Aabb>,
    ) -> Result<(), ContentError> {
        let z = self
            .content
            .zone(id)
            .cloned()
            .ok_or_else(|| ContentError::Invalid(format!("unknown zone {id}")))?;
        let anchor = |a: &str| {
            anchors(a).ok_or_else(|| ContentError::Invalid(format!("zone {id}: no anchor {a}")))
        };
        let state = self.world.zones.get(id);
        let recent = state
            .and_then(|s| s.left_at)
            .is_some_and(|t| self.now.saturating_sub(t) < ticks(TRASH_RESPAWN_MS));
        let dead = match state {
            Some(s) if recent || self.zone.as_deref() == Some(id) => s.dead_trash.clone(),
            _ => BTreeSet::new(),
        };
        let (lo, hi) = z.level_range;
        let mut plan = Vec::new();
        let mut pack_id = self.next_pack;
        let mut slot = 0;
        for (k, p) in z.packs.iter().enumerate() {
            let (at, yaw) = anchor(&p.anchor)?;
            let level = if k * 2 < z.packs.len() { lo } else { hi };
            for (key, (dx, dz)) in p.units.iter().zip(pack_offsets(p.units.len())) {
                if !dead.contains(&slot) {
                    plan.push(Planned {
                        def: self.def_index(key)?,
                        pos: Vec3::new(at.x + dx, at.y, at.z + dz),
                        yaw,
                        level,
                        pack: Some(pack_id),
                        boss: false,
                        slot: Some(slot),
                    });
                }
                slot += 1;
            }
            pack_id += 1;
        }
        if let Some(b) = &z.boss {
            let (at, yaw) = anchor(&b.anchor)?;
            let def = self.def_index(&b.unit)?;
            if !self.world.cleared.contains(id) {
                plan.push(Planned {
                    def,
                    pos: at,
                    yaw,
                    level: self.content.units[def].level,
                    pack: Some(pack_id),
                    boss: true,
                    slot: None,
                });
            }
            pack_id += 1;
        }
        for s in &z.friendly {
            let (at, yaw) = anchor(&s.anchor)?;
            let def = self.def_index(&s.unit)?;
            plan.push(Planned {
                def,
                pos: at,
                yaw,
                level: self.content.units[def].level,
                pack: None,
                boss: false,
                slot: None,
            });
        }
        let graveyard = anchors("graveyard");

        let player = self.player.and_then(|p| self.index(p));
        // A cast started in the old zone would land on a unit that no longer exists.
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
        self.rebuild_nav();
        if let Some(prev) = self.zone.take().filter(|p| p != id) {
            self.world.zones.entry(prev).or_default().left_at = Some(self.now);
        }
        let st = self.world.zones.entry(id.to_string()).or_default();
        st.dead_trash = dead;
        st.left_at = None;
        self.zone = Some(id.to_string());
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
                    zone_slot: p.slot,
                },
            );
        }
        if let Some((pos, yaw)) = graveyard {
            self.set_respawn_point(pos, yaw);
        }
        self.events.push(Event::ZoneLoaded {
            zone: id.to_string(),
        });
        if let Some(floor) = floor_number(id) {
            self.events.push(Event::FloorLoaded { floor });
        }
        Ok(())
    }

    /// The zone loaded by `load_zone`, if any.
    pub fn zone(&self) -> Option<&str> {
        self.zone.as_deref()
    }

    /// Exits of zone `id` in content order with their current unlock state; empty for an unknown zone.
    pub fn zone_links(&self, id: &str) -> Vec<ZoneLinkView> {
        self.content.zone(id).map_or_else(Vec::new, |z| {
            z.links
                .iter()
                .map(|l| ZoneLinkView {
                    anchor: l.anchor.clone(),
                    to: l.to.clone(),
                    unlocked: l.requires.as_ref().is_none_or(|r| self.unlock_met(r)),
                })
                .collect()
        })
    }

    /// True once the zone's boss died this session.
    pub fn zone_cleared(&self, id: &str) -> bool {
        self.world.cleared.contains(id)
    }

    pub(crate) fn unlock_met(&self, r: &Unlock) -> bool {
        match r {
            Unlock::ZoneCleared(z) => self.world.cleared.contains(z),
            Unlock::BossKilled(k) => self.world.bosses_killed.contains(k),
            Unlock::QuestDone(q) => self.quests().completed.iter().any(|c| c == q),
        }
    }

    /// Emits `Unlocked` for every zone whose first gated link just opened.
    pub(crate) fn check_unlocks(&mut self) {
        let opened: Vec<String> = self
            .content
            .zones
            .iter()
            .flat_map(|z| z.links.iter())
            .filter(|l| l.requires.as_ref().is_some_and(|r| self.unlock_met(r)))
            .map(|l| l.to.clone())
            .collect();
        for zone in opened {
            if self.world.announced.insert(zone.clone()) {
                self.events.push(Event::Unlocked { zone });
            }
        }
    }

    /// World bookkeeping for a hostile death, after its loot: trash slot, zone clear, boss kill,
    /// quest kill credit, then unlocks.
    pub(crate) fn on_hostile_death(&mut self, ti: usize, killer: Option<crate::EntityId>) {
        let key = self.units[ti].state.key.clone();
        let zone = self.zone.clone();
        if let (Some(slot), Some(z)) = (self.units[ti].zone_slot, &zone) {
            self.world
                .zones
                .entry(z.clone())
                .or_default()
                .dead_trash
                .insert(slot);
        }
        if self.units[ti].floor_boss {
            self.world.bosses_killed.insert(key.clone());
        }
        if self.units[ti].floor_boss
            && let Some(z) = zone
            && self.world.cleared.insert(z.clone())
        {
            self.events.push(Event::ZoneCleared { zone: z.clone() });
            if let Some(floor) = floor_number(&z) {
                self.events.push(Event::FloorCleared { floor });
            }
            self.quest_credit(|o| matches!(o, Objective::ClearZone(c) if *c == z));
        }
        if killer.is_some() && killer == self.player {
            self.quest_credit(|o| matches!(o, Objective::Kill(k, _) if *k == key));
        }
        self.check_unlocks();
    }
}
