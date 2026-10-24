//! Swarm aggregation (docs/ILMARI_10.md C.3.1): a cluster of drones is one ordinary hostile unit
//! with an HP pool of `count * per_drone_hp`. Replication, threat, targeting and loot work on it
//! unchanged; the client draws as many drones as its quality preset allows and the sim never
//! sees a drone position. All HP and damage arithmetic is integer.
//!
//! Without a threat entry a cluster walks to the nearest un-eaten draft and eats it; a hit makes
//! it fight the hitter with the normal AI, and when the table empties it goes back to the
//! drafts, so the fight is a defence and not a tank-and-spank.

use crate::ai::{step_toward, yaw_toward};
use crate::content::{AiDef, SwarmDef};
use crate::effects::dist_xz;
use crate::geom::Vec3;
use crate::sim::{Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::time::TICK_HZ;
use crate::types::AiState;

/// Single-target hits count `1 + count / CROWD_DIV` drones' worth of damage...
const CROWD_DIV: u64 = 40;
/// ...up to this many per mille (x4); area hits triple whatever that came to.
const CROWD_CAP_PM: u64 = 4000;
const AREA_MULT: u64 = 3;
/// A cluster above this many drones sheds a daughter when an area effect hits it.
pub const SHED_ABOVE: u16 = 200;
/// The daughter takes this share of the pool, in percent.
const SHED_PCT: u32 = 25;
/// Within this many metres a cluster eats.
const EAT_RANGE_M: f32 = 2.5;
/// Where a walking cluster stops: inside the eat range with a margin for the step.
const STOP_M: f32 = 2.0;
/// Reach of the merge of tiny clusters (`SwarmDef::merge_below`).
pub const MERGE_RANGE_M: f32 = 4.0;
/// How many of the smallest clusters the population cap looks at for the closest pair.
const MERGE_POOL: usize = 4;
/// Integrity of a fresh draft, in thousandths (1000 points, the prop meter of the spec).
const DRAFT_FULL_MILLI: u32 = 1_000_000;

/// Something the swarm eats: a position and a meter.
#[derive(Clone, Copy, Debug)]
struct Draft {
    pos: Vec3,
    /// Thousandths of an integrity point left.
    milli: u32,
    /// Remainder of the per-tick split of the per-second bite (0..TICK_HZ).
    acc: u32,
    /// Mark Confidential: no bites until this tick.
    shield_until: u64,
    /// The next tick a Mark Confidential may be put on it (shared by every player).
    mark_ready: u64,
    /// Counted as lost once (the encounter reads it).
    lost: bool,
}

/// A draft as the client and tests read it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftView {
    pub pos: Vec3,
    /// 0..=1000.
    pub integrity: u32,
}

#[derive(Default)]
pub(crate) struct SwarmState {
    drafts: Vec<Draft>,
    /// Until these ticks the clusters walk at half speed (a DevOps rack's Rate Limit) and leave
    /// the drafts alone (a Security terminal's robots.txt).
    slow_until: u64,
    comply_until: u64,
}

impl Sim {
    /// Adds a draft at `pos` (a QA and content hook until Wing 3 places real ones); returns its
    /// index.
    pub fn add_draft(&mut self, pos: Vec3) -> usize {
        self.swarm.drafts.push(Draft {
            pos,
            milli: DRAFT_FULL_MILLI,
            acc: 0,
            shield_until: 0,
            mark_ready: 0,
            lost: false,
        });
        self.swarm.drafts.len() - 1
    }

    /// Debug/test: draft `k` at `integrity` points (0..=1000). False when there is no such draft.
    pub fn debug_set_draft(&mut self, k: usize, integrity: u32) -> bool {
        let Some(d) = self.swarm.drafts.get_mut(k) else {
            return false;
        };
        d.milli = integrity.min(1000) * 1000;
        true
    }

    /// A class prop's hold on the clusters: half speed for `slow_ms`, no eating for `comply_ms`.
    pub(crate) fn swarm_hold(&mut self, slow_ms: u32, comply_ms: u32) {
        let now = self.now;
        if slow_ms > 0 {
            self.swarm.slow_until = self
                .swarm
                .slow_until
                .max(now + crate::content::ticks(slow_ms));
        }
        if comply_ms > 0 {
            self.swarm.comply_until = self
                .swarm
                .comply_until
                .max(now + crate::content::ticks(comply_ms));
        }
    }

    /// Forgets every draft (the encounter ended or reset).
    pub(crate) fn drafts_clear(&mut self) {
        self.swarm.drafts.clear();
        self.swarm.slow_until = 0;
        self.swarm.comply_until = 0;
    }

    /// Marks draft `k` Confidential: it takes no bites for `ms`, then waits `cooldown_ms` before
    /// anyone may do it again. False while it cannot (gone, or still cooling down).
    pub(crate) fn draft_mark(&mut self, k: usize, ms: u32, cooldown_ms: u32) -> bool {
        let now = self.now;
        let Some(d) = self.swarm.drafts.get_mut(k) else {
            return false;
        };
        if d.milli == 0 || now < d.mark_ready {
            return false;
        }
        d.shield_until = now + crate::content::ticks(ms);
        d.mark_ready = now + crate::content::ticks(cooldown_ms);
        true
    }

    /// Drafts newly at zero since the last call, as indices (each is reported once).
    pub(crate) fn drafts_newly_lost(&mut self) -> Vec<usize> {
        let mut out = Vec::new();
        for (k, d) in self.swarm.drafts.iter_mut().enumerate() {
            if d.milli == 0 && !d.lost {
                d.lost = true;
                out.push(k);
            }
        }
        out
    }

    /// Per draft: integrity 0..=1000 and whether a Mark Confidential is on it now.
    pub(crate) fn draft_status(&self) -> Vec<(u16, bool)> {
        self.swarm
            .drafts
            .iter()
            .map(|d| ((d.milli / 1000) as u16, self.now < d.shield_until))
            .collect()
    }

    pub fn drafts(&self) -> Vec<DraftView> {
        self.swarm
            .drafts
            .iter()
            .map(|d| DraftView {
                pos: d.pos,
                integrity: d.milli / 1000,
            })
            .collect()
    }

    /// Spawns a cluster of `count` drones of unit `key`. Past the unit's `max_clusters` the two
    /// smallest clusters closest together merge first, so the sim holds at most that many swarm
    /// units however often waves spawn.
    pub fn spawn_swarm(
        &mut self,
        key: &str,
        pos: Vec3,
        count: u16,
    ) -> Result<EntityId, crate::content::ContentError> {
        let def = self.def_index(key)?;
        let sw = self.content.units[def]
            .swarm
            .ok_or_else(|| crate::content::ContentError::UnknownUnit(key.to_string()))?;
        while self.swarm_clusters().len() >= usize::from(sw.max_clusters.max(1)) {
            if !self.merge_closest_small() {
                break;
            }
        }
        Ok(self.spawn_cluster(def, pos, u32::from(count.max(1)) * sw.per_drone_hp))
    }

    fn spawn_cluster(&mut self, def: usize, pos: Vec3, pool: u32) -> EntityId {
        let id = self.spawn(
            def,
            pos,
            0.0,
            SpawnOpts {
                level: None,
                pack: None,
                respawns: false,
                summoner: None,
                floor_boss: false,
                zone_slot: None,
            },
        );
        if let Some(i) = self.index(id) {
            self.units[i].state.hp = pool;
            self.units[i].state.max_hp = pool;
        }
        id
    }

    /// Living clusters as (unit id, drones), in id order.
    pub fn swarm_clusters(&self) -> Vec<(EntityId, u16)> {
        self.units
            .iter()
            .filter(|u| !u.despawned && u.state.alive)
            .filter_map(|u| {
                let sw = self.content.units[u.def].swarm?;
                Some((u.state.id, sw.count(u.state.hp)))
            })
            .collect()
    }

    /// Damage to a cluster counts for more than one drone's worth of HP, so a single hit is not
    /// 1/125th of the fight (C.3.1). Area hits triple that. Untouched for every other unit.
    pub(crate) fn swarm_crowd(&self, ti: usize, amount: u32) -> u32 {
        let u = &self.units[ti];
        let Some(sw) = self.content.units[u.def].swarm else {
            return amount;
        };
        let n = u64::from(sw.count(u.state.hp));
        let single = (1000 + n * 1000 / CROWD_DIV).min(CROWD_CAP_PM);
        let pm = if self.area_hit {
            single * AREA_MULT
        } else {
            single
        };
        (u64::from(amount) * pm / 1000).min(u64::from(u32::MAX)) as u32
    }

    /// A big cluster that an area effect hit sheds a daughter (bounded by the cluster cap, and
    /// the pool is conserved: the daughter's HP leaves the parent).
    pub(crate) fn swarm_after_hit(&mut self, ti: usize) {
        if !self.area_hit || !self.units[ti].state.alive {
            return;
        }
        let def = self.units[ti].def;
        let Some(sw) = self.content.units[def].swarm else {
            return;
        };
        if sw.count(self.units[ti].state.hp) <= SHED_ABOVE
            || self.swarm_clusters().len() >= usize::from(sw.max_clusters)
        {
            return;
        }
        let share = self.units[ti].state.hp / 100 * SHED_PCT;
        let parent = &mut self.units[ti];
        parent.state.hp -= share;
        parent.state.max_hp = parent
            .state
            .max_hp
            .saturating_sub(share)
            .max(parent.state.hp);
        let (pos, id) = (parent.state.pos, parent.state.id);
        let threat: Vec<(EntityId, u32)> = parent.threat.iter().map(|(k, v)| (*k, *v)).collect();
        // Eight fixed compass offsets by parent id: deterministic and independent of the rng.
        let a = (id % 8) as f32 * std::f32::consts::FRAC_PI_4;
        let at = Vec3::new(pos.x + 2.0 * a.cos(), pos.y, pos.z + 2.0 * a.sin());
        let daughter = self.spawn_cluster(def, at, share);
        if let Some(di) = self.index(daughter) {
            for (src, amt) in threat {
                self.add_threat(di, src, (amt / 2).max(1));
            }
        }
    }

    /// Merges the closest pair among the smallest clusters; false when fewer than two live.
    fn merge_closest_small(&mut self) -> bool {
        let mut small: Vec<(u16, EntityId)> =
            self.swarm_clusters().iter().map(|&(i, n)| (n, i)).collect();
        small.sort_unstable();
        small.truncate(MERGE_POOL);
        let mut best: Option<(u64, EntityId, EntityId)> = None;
        for (k, &(_, a)) in small.iter().enumerate() {
            for &(_, b) in &small[k + 1..] {
                let (Some(ai), Some(bi)) = (self.index(a), self.index(b)) else {
                    continue;
                };
                // Centimetres keep the comparison integer.
                let d =
                    (dist_xz(self.units[ai].state.pos, self.units[bi].state.pos) * 100.0) as u64;
                if best.is_none_or(|(bd, ba, bb)| (d, a, b) < (bd, ba, bb)) {
                    best = Some((d, a, b));
                }
            }
        }
        let Some((_, a, b)) = best else { return false };
        self.merge_into(a, b);
        true
    }

    /// `from` joins `into`: pools add up, the position is the drone-weighted mean.
    fn merge_into(&mut self, into: EntityId, from: EntityId) {
        let (Some(ai), Some(bi)) = (self.index(into), self.index(from)) else {
            return;
        };
        let (pa, pb) = (self.units[ai].state.hp, self.units[bi].state.hp);
        let (wa, wb) = (u64::from(pa.max(1)), u64::from(pb.max(1)));
        let (a, b) = (self.units[ai].state.pos, self.units[bi].state.pos);
        let mix = |x: f32, y: f32| {
            ((f64::from(x) * wa as f64 + f64::from(y) * wb as f64) / (wa + wb) as f64) as f32
        };
        let threat: Vec<(EntityId, u32)> = self.units[bi]
            .threat
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        let max_b = self.units[bi].state.max_hp;
        let u = &mut self.units[ai];
        u.state.hp = pa.saturating_add(pb);
        u.state.max_hp = u.state.max_hp.saturating_add(max_b);
        u.state.pos = Vec3::new(mix(a.x, b.x), a.y, mix(a.z, b.z));
        for (src, amt) in threat {
            *self.units[ai].threat.entry(src).or_insert(0) += amt;
        }
        self.despawn(bi);
    }

    /// Per tick: tiny clusters join a neighbour (once a second).
    pub(crate) fn update_swarms(&mut self) {
        if !self.now.is_multiple_of(u64::from(TICK_HZ)) {
            return;
        }
        let list = self.swarm_clusters();
        for &(id, n) in &list {
            let Some(i) = self.index(id) else { continue };
            let Some(sw) = self.content.units[self.units[i].def].swarm else {
                continue;
            };
            if n >= sw.merge_below || !self.units[i].state.alive || self.units[i].despawned {
                continue;
            }
            let pos = self.units[i].state.pos;
            let near = list
                .iter()
                .filter(|&&(o, _)| o != id)
                .filter_map(|&(o, _)| self.index(o).map(|oi| (oi, o)))
                .filter(|&(oi, _)| {
                    !self.units[oi].despawned
                        && self.units[oi].state.alive
                        && dist_xz(self.units[oi].state.pos, pos) <= MERGE_RANGE_M
                })
                .map(|(_, o)| o)
                .min();
            if let Some(o) = near {
                self.merge_into(o, id);
            }
        }
    }

    /// The cluster's brain, called by `update_ai` in place of the normal idle/aggro logic.
    pub(crate) fn swarm_unit(&mut self, i: usize, ai: &AiDef, sw: SwarmDef) {
        if !self.units[i].state.alive {
            return;
        }
        if !self.units[i].threat.is_empty() {
            return self.ai_unit(i, ai);
        }
        // The table emptied (the hitter died or left): no evade and no heal, straight back to work.
        if self.units[i].state.ai != Some(AiState::Idle) {
            self.set_ai(i, AiState::Idle);
        }
        let me = self.units[i].state.pos;
        // ponytail: a straight walk and a linear scan; a nav path and a spatial index when a
        // zone has walls between the clusters and the drafts or more than a handful of drafts.
        let Some(di) = (0..self.swarm.drafts.len())
            .filter(|&d| self.swarm.drafts[d].milli > 0)
            .min_by(|&a, &b| {
                let (da, db) = (
                    dist_xz(me, self.swarm.drafts[a].pos),
                    dist_xz(me, self.swarm.drafts[b].pos),
                );
                da.total_cmp(&db).then(a.cmp(&b))
            })
        else {
            return;
        };
        let goal = self.swarm.drafts[di].pos;
        if dist_xz(me, goal) > EAT_RANGE_M {
            let slow = if self.now < self.swarm.slow_until {
                0.5
            } else {
                1.0
            };
            let step = sw.speed * slow / TICK_HZ as f32;
            self.units[i].state.yaw = yaw_toward(me, goal);
            self.units[i].state.pos = step_toward(me, goal, step, STOP_M);
            self.units[i].moving = true;
            return;
        }
        self.units[i].moving = false;
        let count = u64::from(sw.count(self.units[i].state.hp));
        let now = self.now;
        let d = &mut self.swarm.drafts[di];
        if now < d.shield_until || now < self.swarm.comply_until {
            return;
        }
        let total = count * u64::from(sw.eat_per_drone_per_s) + u64::from(d.acc);
        let tick_hz = u64::from(TICK_HZ);
        d.acc = (total % tick_hz) as u32;
        d.milli = d
            .milli
            .saturating_sub((total / tick_hz).min(u64::from(u32::MAX)) as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Content;

    const KEY: &str = "il_scraper_swarm";

    fn sim() -> Sim {
        let c = Content::from_ron(include_str!("../../../content/game.ron")).expect("content");
        let mut s = Sim::new(c, 7);
        s.set_wandering(false);
        s
    }

    fn run(s: &mut Sim, secs: u32) {
        for _ in 0..secs * TICK_HZ {
            s.tick();
        }
    }

    fn at(x: f32) -> Vec3 {
        Vec3::new(x, 0.0, 0.0)
    }

    #[test]
    fn hp_pool_gives_the_drone_count() {
        let mut s = sim();
        let id = s.spawn_swarm(KEY, at(0.0), 125).unwrap();
        assert_eq!(s.unit(id).unwrap().hp, 3750);
        assert_eq!(s.swarm_clusters(), vec![(id, 125)]);
        let sw = s.content.units[s.def_index(KEY).unwrap()].swarm.unwrap();
        assert_eq!(
            [sw.count(0), sw.count(1), sw.count(30), sw.count(31)],
            [0, 1, 1, 2]
        );
    }

    #[test]
    fn crowd_multiplier_single_and_area() {
        let mut s = sim();
        let big = s.spawn_swarm(KEY, at(0.0), 125).unwrap();
        let mid = s.spawn_swarm(KEY, at(50.0), 40).unwrap();
        let (bi, mi) = (s.index(big).unwrap(), s.index(mid).unwrap());
        // 125 drones: 1 + 125/40 = 4.125 capped at x4; 40 drones: 1 + 1 = x2.
        assert_eq!((s.swarm_crowd(bi, 100), s.swarm_crowd(mi, 100)), (400, 200));
        s.area_hit = true;
        assert_eq!(
            (s.swarm_crowd(bi, 100), s.swarm_crowd(mi, 100)),
            (1200, 600)
        );
    }

    #[test]
    fn non_swarm_units_are_untouched() {
        let mut s = sim();
        let ceo = s.spawn_unit("ceo", at(0.0), 0.0).unwrap();
        let i = s.index(ceo).unwrap();
        s.area_hit = true;
        assert_eq!(s.swarm_crowd(i, 100), 100);
    }

    #[test]
    fn population_never_exceeds_the_cap_and_pools_are_conserved() {
        let mut s = sim();
        let mut spawned = 0u64;
        // Ten minutes, a wave of two clusters every second, nothing killing them.
        for t in 0..600u32 {
            for k in 0..2 {
                let n = 20 + (t % 7) as u16 * 5;
                spawned += u64::from(n) * 30;
                s.spawn_swarm(KEY, at((t % 50) as f32 + k as f32 * 0.5), n)
                    .unwrap();
            }
            run(&mut s, 1);
            assert!(s.swarm_clusters().len() <= 32, "tick {t}");
        }
        let pool: u64 = s
            .units
            .iter()
            .filter(|u| !u.despawned && u.state.alive && s.content.units[u.def].swarm.is_some())
            .map(|u| u64::from(u.state.hp))
            .sum();
        assert_eq!(pool, spawned);
    }

    #[test]
    fn merge_picks_the_closest_of_the_smallest_with_id_tie_break() {
        let mut s = sim();
        // Smallest four: 20 at x=0, 20 at x=30, 20 at x=31, 20 at x=60 (far from all); the
        // closest pair is (x=30, x=31) and the lower id absorbs the other.
        let ids: Vec<_> = [0.0, 30.0, 31.0, 60.0]
            .iter()
            .map(|&x| s.spawn_swarm(KEY, at(x), 20).unwrap())
            .collect();
        assert!(s.merge_closest_small());
        let left: Vec<_> = s.swarm_clusters();
        assert_eq!(left.len(), 3);
        assert!(left.contains(&(ids[0], 20)) && left.contains(&(ids[3], 20)));
        assert!(left.contains(&(ids[1], 40)));
    }

    #[test]
    fn tiny_clusters_join_a_neighbour() {
        let mut s = sim();
        let big = s.spawn_swarm(KEY, at(0.0), 100).unwrap();
        s.spawn_swarm(KEY, at(2.0), 5).unwrap();
        run(&mut s, 2);
        assert_eq!(s.swarm_clusters(), vec![(big, 105)]);
    }

    #[test]
    fn a_frozen_cluster_eats_by_the_formula() {
        let mut s = sim();
        s.add_draft(at(0.0));
        // On top of the draft: 100 drones * 40 thousandths per second = 4 points per second.
        s.spawn_swarm(KEY, at(1.0), 100).unwrap();
        run(&mut s, 10);
        assert_eq!(s.drafts()[0].integrity, 1000 - 40);
    }

    #[test]
    fn a_cluster_walks_to_the_draft_then_eats() {
        let mut s = sim();
        s.add_draft(at(20.0));
        let id = s.spawn_swarm(KEY, at(0.0), 100).unwrap();
        run(&mut s, 3);
        assert!(s.drafts()[0].integrity == 1000, "still walking");
        run(&mut s, 12);
        assert!(s.drafts()[0].integrity < 1000);
        let d = dist_xz(s.unit(id).unwrap().pos, at(20.0));
        assert!(d <= EAT_RANGE_M, "{d}");
    }

    #[test]
    fn a_big_cluster_hit_by_an_area_effect_sheds_a_daughter() {
        let mut s = sim();
        let id = s.spawn_swarm(KEY, at(0.0), 250).unwrap();
        let i = s.index(id).unwrap();
        s.area_hit = true;
        s.swarm_after_hit(i);
        s.area_hit = false;
        let mut counts: Vec<u16> = s.swarm_clusters().iter().map(|c| c.1).collect();
        counts.sort_unstable();
        assert_eq!(counts, vec![63, 188]);
        // Single-target hits never shed.
        s.swarm_after_hit(i);
        assert_eq!(s.swarm_clusters().len(), 2);
    }
}
