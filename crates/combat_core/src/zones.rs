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
    pub(crate) bosses_killed: BTreeSet<String>,
    /// Zones whose `Unlocked` event was already emitted.
    pub(crate) announced: BTreeSet<String>,
}

/// One exit of a zone as `Sim::zone_links` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneLinkView {
    pub anchor: String,
    pub to: String,
    pub unlocked: bool,
}

/// Most extra members one pack can gain from group size; their zone slots are reserved past
/// every regular slot, `MAX_PACK_EXTRA` per pack, so they stay dead across re-entries too.
const MAX_PACK_EXTRA: usize = 4;
/// Regular slots never reach this; extra-member slots start here.
const EXTRA_SLOT_BASE: usize = 10_000;

/// A trash pack of the loaded zone, kept for group-size growth after the load.
pub(crate) struct PackRt {
    pub(crate) id: u32,
    pub(crate) at: Vec3,
    pub(crate) yaw: f32,
    level: u8,
    defs: Vec<usize>,
    index: usize,
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
    /// spawned again. The optional `graveyard` anchor becomes every player's respawn point; with
    /// the optional `player_spawn` anchor the whole group is moved there (member `k` at
    /// `pack_offsets` slot `k`). All anchors are resolved first, so an error leaves the sim untouched.
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
        let spots = z.packs.iter().map(|p| &p.anchor);
        let spots = spots.chain(z.boss.iter().map(|b| &b.anchor));
        for a in spots.chain(z.friendly.iter().map(|f| &f.anchor)) {
            anchor(a)?;
        }
        // May swap the campaign world back in (leaving a sprint), so it goes before any lookup.
        self.sprint_on_load(id);
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
        let mut packs = Vec::new();
        for (k, p) in z.packs.iter().enumerate() {
            let (at, yaw) = anchor(&p.anchor)?;
            let level = self.sprint_level(if k * 2 < z.packs.len() { lo } else { hi });
            let mut defs = Vec::new();
            for key in &p.units {
                defs.push(self.def_index(key)?);
            }
            packs.push(PackRt {
                id: pack_id,
                at,
                yaw,
                level,
                defs,
                index: k,
            });
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
                    level: self.sprint_level(self.content.units[def].level),
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
        let arrival = anchors("player_spawn");

        let players: Vec<usize> = self.players.iter().filter_map(|p| self.index(*p)).collect();
        // A cast started in the old zone would land on a unit that no longer exists.
        for &pi in &players {
            if self.units[pi].state.cast.is_some() {
                self.cancel_cast(pi, None, 0);
            }
            self.units[pi].queued = None;
        }
        for i in 0..self.units.len() {
            if !players.contains(&i) && !self.units[i].despawned {
                self.despawn(i);
            }
        }
        if let Some((at, yaw)) = arrival {
            for (&pi, (dx, dz)) in players.iter().zip(pack_offsets(players.len())) {
                let s = &mut self.units[pi].state;
                s.pos = Vec3::new(at.x + dx, at.y, at.z + dz);
                s.yaw = yaw;
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
        self.zone_packs = packs;
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
        self.grow_packs();
        self.sprint_after_load();
        if let Some((pos, yaw)) = graveyard {
            self.set_respawn_point(pos, yaw);
        }
        self.load_world(id, anchors);
        self.load_live(id, anchors);
        self.events.push(Event::ZoneLoaded {
            zone: id.to_string(),
        });
        if let Some(floor) = floor_number(id) {
            self.events.push(Event::FloorLoaded { floor });
        }
        Ok(())
    }

    /// Co-op pack growth: every `group_pack_growth_players` players beyond the first add one
    /// member to each trash pack (cycling through its units, next pack offset). Runs on load and
    /// when someone joins; a pack already fighting or wiped out is left alone, and killed extras
    /// stay dead like regular trash.
    pub(crate) fn grow_packs(&mut self) {
        let per = self.content.config.group_pack_growth_players as usize;
        if per == 0 {
            return;
        }
        let want = (self.players.len().saturating_sub(1) / per).min(MAX_PACK_EXTRA);
        let dead = self
            .zone
            .as_ref()
            .and_then(|z| self.world.zones.get(z))
            .map(|s| s.dead_trash.clone())
            .unwrap_or_default();
        let mut spawns = Vec::new();
        for p in &self.zone_packs {
            let members: Vec<&crate::sim::Unit> = self
                .units
                .iter()
                .filter(|u| u.state.pack == Some(p.id) && !u.despawned)
                .collect();
            let busy = members
                .iter()
                .any(|u| u.state.in_combat || !u.threat.is_empty());
            if busy || !members.iter().any(|u| u.state.alive) {
                continue;
            }
            let offsets = pack_offsets(p.defs.len() + want);
            for e in 0..want {
                let slot = EXTRA_SLOT_BASE + p.index * MAX_PACK_EXTRA + e;
                let have = members.iter().any(|u| u.zone_slot == Some(slot));
                if have || dead.contains(&slot) {
                    continue;
                }
                let (dx, dz) = offsets.get(p.defs.len() + e).copied().unwrap_or((0.0, 0.0));
                let pos = Vec3::new(p.at.x + dx, p.at.y, p.at.z + dz);
                spawns.push((p.defs[e % p.defs.len()], pos, p.yaw, p.level, p.id, slot));
            }
        }
        for (def, pos, yaw, level, pack, slot) in spawns {
            self.spawn(
                def,
                pos,
                yaw,
                SpawnOpts {
                    level: Some(level),
                    pack: Some(pack),
                    respawns: false,
                    summoner: None,
                    floor_boss: false,
                    zone_slot: Some(slot),
                },
            );
        }
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
                    unlocked: self
                        .sprint_link_open(&l.to)
                        .unwrap_or_else(|| l.requires.as_ref().is_none_or(|r| self.unlock_met(r))),
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
            // The zone is shared, so one member's turn-in opens it for the whole group.
            Unlock::QuestDone(q) => self
                .quest_logs
                .values()
                .any(|l| l.completed.iter().any(|c| c == q)),
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
            for p in self.players.clone() {
                self.quest_credit(p, |o| matches!(o, Objective::ClearZone(c) if *c == z));
            }
        }
        // Classic: every group member near the kill gets the credit, not just the killer.
        for p in self.kill_sharers(killer, ti) {
            let p = Self::id_of(p);
            self.record_kill(p, &key);
        }
        self.check_unlocks();
    }
}
