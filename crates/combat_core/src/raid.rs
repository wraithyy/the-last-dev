//! The raid "All-Hands Offsite": a chain of zones (`raid.zones`) reached from the campus at
//! the level cap, one boss encounter per zone. Two difficulties: Normal for 3-5 players and
//! Async Attendance (picked automatically for smaller groups) with weaker enemies and less
//! loot. Every encounter is locked per difficulty for the ISO week once killed (per room
//! online, per save offline); a locked boss does not come back until the reset.
//! Encounter mechanics that the generic AI cannot express live here: "Pair Up" tethers
//! (`Effect::PairUp`) and the Synergy Twins' linked HP and swap.
//! Content: `raid` in `game.ron`; numbers in docs/FORMULAS.md.

use crate::content::{Content, ContentError, School, SpellId, ticks};
use crate::legacy::Lockout;
use crate::sim::{Hit, Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::types::{AiState, AuraEnd, Event};
use serde::{Deserialize, Serialize};

/// Lockout and achievement key prefix of Async Attendance kills.
pub const ASYNC_PREFIX: &str = "async:";
/// Tethers check their range this often.
const TETHER_TICK_MS: u32 = 1000;
/// Long enough to outlast any fight; the twin swap moves the aura itself.
const SHIELD_MS: u32 = 3_600_000;

#[derive(Clone, Debug, Deserialize)]
pub struct TwinsDef {
    /// The zone boss (loot, lockout) and its linked partner, spawned beside it.
    pub boss: String,
    pub twin: String,
    /// Carried by one twin at a time; the swap moves it to the other.
    pub shield_aura: String,
    pub swap_ms: u32,
    #[serde(default)]
    pub swap_yells: Vec<String>,
}

/// Absent in minimal test content: no raid.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RaidDef {
    pub name: String,
    /// Raid zones in walking order; each zone's boss is one encounter.
    pub zones: Vec<String>,
    /// Groups smaller than this play Async Attendance.
    pub normal_min_players: u8,
    /// Async Attendance enemy HP and damage, percent of Normal.
    pub async_hp_pct: u32,
    pub async_damage_pct: u32,
    /// Raid pieces each member rolls per Normal boss kill; Async rolls one at `async_item_pct`.
    pub boss_items: u32,
    pub async_item_pct: u32,
    /// Standing with every department per boss kill (halved on Async).
    pub boss_rep: u32,
    #[serde(default)]
    pub twins: Option<TwinsDef>,
}

impl RaidDef {
    pub fn is_raid_zone(&self, zone: &str) -> bool {
        self.zones.iter().any(|z| z == zone)
    }

    /// Boss unit keys of the encounters in walking order.
    pub fn bosses(&self, c: &Content) -> Vec<String> {
        self.zones
            .iter()
            .filter_map(|z| c.zone(z).and_then(|d| d.boss.as_ref()))
            .map(|b| b.unit.clone())
            .collect()
    }

    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("raid: {m}")));
        if self.zones.is_empty() {
            return Ok(());
        }
        for z in &self.zones {
            match c.zone(z) {
                None => return bad(format!("unknown zone {z}")),
                Some(d) if d.boss.is_none() => return bad(format!("zone {z} has no boss")),
                _ => {}
            }
        }
        if self.normal_min_players == 0 || self.async_hp_pct == 0 || self.async_damage_pct == 0 {
            return bad("zero player count or async percent".into());
        }
        if let Some(t) = &self.twins {
            for k in [&t.boss, &t.twin] {
                if !c.unit_def(k).is_some_and(|u| u.boss && u.hostile) {
                    return bad(format!("twin {k} is not a hostile boss"));
                }
            }
            if !self.bosses(c).contains(&t.boss) {
                return bad(format!("twin {} is no zone boss", t.boss));
            }
            if c.aura(&t.shield_aura).is_none() || t.swap_ms == 0 {
                return bad("twins need a shield aura and a swap period".into());
            }
        }
        Ok(())
    }
}

/// What the client shows and draws: the raid zone loaded ("" outside), the difficulty in force,
/// this week's lockout and the "Pair Up" tethers (`(a, b)`; `b` may be the boss itself).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaidView {
    pub zone: String,
    pub async_mode: bool,
    pub week: u32,
    /// Encounters (zone boss keys, `async:`-prefixed for Async Attendance) killed this week.
    pub locked: Vec<String>,
    pub tethers: Vec<(EntityId, EntityId)>,
    /// Tether range in metres.
    pub tether_m: u8,
}

impl RaidView {
    /// The lockout of ISO week `week`: empty once the reset passed.
    pub fn locked_in(&self, week: u32) -> &[String] {
        if self.week == week { &self.locked } else { &[] }
    }
}

/// The lockout key of `boss` on a difficulty.
pub fn lock_key(boss: &str, async_mode: bool) -> String {
    if async_mode {
        format!("{ASYNC_PREFIX}{boss}")
    } else {
        boss.to_string()
    }
}

struct Tether {
    a: EntityId,
    b: EntityId,
    source: EntityId,
    spell: SpellId,
    aura: String,
    until: u64,
    max_m: f32,
    amount: u32,
}

#[derive(Default)]
pub(crate) struct RaidState {
    async_mode: bool,
    /// QA/tests: the difficulty regardless of group size.
    force: Option<bool>,
    lock: Lockout,
    tethers: Vec<Tether>,
    next_tether_check: u64,
    /// Next twin swap; `u64::MAX` while the twins are not fighting.
    next_swap: u64,
    /// Guards the linked-HP mirror against recursion.
    linking: bool,
    announce: bool,
}

impl Sim {
    fn raid_week(&self) -> u32 {
        self.legacy_week()
    }

    /// The loaded zone is part of the raid.
    pub fn in_raid(&self) -> bool {
        self.zone
            .as_deref()
            .is_some_and(|z| self.content.raid.is_raid_zone(z))
    }

    pub fn raid_async(&self) -> bool {
        self.raid.async_mode
    }

    pub fn raid_view(&self) -> RaidView {
        let week = self.raid_week();
        let max = self.raid.tethers.first().map_or(0.0, |t| t.max_m);
        RaidView {
            zone: if self.in_raid() {
                self.zone.clone().unwrap_or_default()
            } else {
                String::new()
            },
            async_mode: self.raid.async_mode,
            week,
            locked: self.raid.lock.list(week),
            tethers: self.raid.tethers.iter().map(|t| (t.a, t.b)).collect(),
            tether_m: max.round().clamp(0.0, 255.0) as u8,
        }
    }

    /// True while encounter `boss` stays dead on this difficulty until the weekly reset.
    pub fn raid_locked(&self, boss: &str, async_mode: bool) -> bool {
        self.raid
            .lock
            .contains(self.raid_week(), &lock_key(boss, async_mode))
    }

    fn raid_broadcast(&mut self) {
        let view = self.raid_view();
        self.events.push(Event::Raid { view });
    }

    pub(crate) fn raid_on_join(&mut self, player: EntityId) {
        let view = self.raid_view();
        if view != RaidView::default() {
            self.emit_to(player, Event::Raid { view });
        }
    }

    /// The door from outside into the raid opens only with everyone at the level cap; links
    /// inside the raid follow their content `requires`.
    pub(crate) fn raid_link_open(&self, from: &str, to: &str) -> Option<bool> {
        let r = &self.content.raid;
        (r.is_raid_zone(to) && !r.is_raid_zone(from)).then(|| self.everyone_capped())
    }

    /// Called by `load_zone` before anything spawns: picks the difficulty for a raid zone and
    /// drops the tethers of the zone being left.
    pub(crate) fn raid_on_load(&mut self, id: &str) {
        let was = self.in_raid() || !self.raid.tethers.is_empty();
        self.raid.tethers.clear();
        self.raid.next_swap = u64::MAX;
        let raid = self.content.raid.is_raid_zone(id);
        if raid {
            let min = usize::from(self.content.raid.normal_min_players);
            self.raid.async_mode = self.raid.force.unwrap_or(self.players.len() < min);
        }
        self.raid.announce = raid || was;
    }

    /// Raid bosses come back every week: a locked encounter stays dead, an unlocked one spawns
    /// even in a zone cleared before. `None` outside the raid (the campaign rule applies).
    pub(crate) fn raid_boss_spawns(&self, zone: &str, boss: &str) -> Option<bool> {
        self.content
            .raid
            .is_raid_zone(zone)
            .then(|| !self.raid_locked(boss, self.raid.async_mode))
    }

    /// After the zone's units exist: the second twin joins its partner, shielded.
    pub(crate) fn raid_after_load(&mut self) {
        if self.in_raid()
            && let Some(t) = self.content.raid.twins.clone()
            && let Some(bi) = (0..self.units.len()).find(|&i| {
                let u = &self.units[i];
                u.floor_boss && !u.despawned && u.state.alive && u.state.key == t.boss
            })
            && let Ok(def) = self.def_index(&t.twin)
        {
            let b = &self.units[bi];
            let (pos, yaw, level, pack) = (b.state.pos, b.state.yaw, b.state.level, b.state.pack);
            let side = crate::telegraph::right(yaw);
            let at = crate::geom::Vec3::new(pos.x + side.x * 5.0, pos.y, pos.z + side.z * 5.0);
            let id = self.spawn(
                def,
                at,
                yaw,
                SpawnOpts {
                    level: Some(level),
                    pack,
                    respawns: false,
                    summoner: None,
                    floor_boss: false,
                    zone_slot: None,
                },
            );
            if let Some(i) = self.index(id) {
                self.apply_aura(i, &t.shield_aura, SHIELD_MS, None, id, None);
            }
        }
        if std::mem::take(&mut self.raid.announce) {
            self.raid_broadcast();
        }
    }

    /// Async Attendance enemy (HP %, damage %) in the raid; 100/100 elsewhere.
    pub(crate) fn raid_mob_pct(&self) -> (u32, u32) {
        let r = &self.content.raid;
        if self.in_raid() && self.raid.async_mode {
            (r.async_hp_pct, r.async_damage_pct)
        } else {
            (100, 100)
        }
    }

    // ---- Pair Up ----

    /// `Effect::PairUp`: shuffles the hostile players in range into pairs; an odd one out is
    /// paired with the caster. Every pair takes `base` (scaled like a hit) per second while
    /// farther apart than `max_m`, until `duration_ms` runs out.
    #[allow(clippy::too_many_arguments)] // the effect's fields
    pub(crate) fn raid_pair_up(
        &mut self,
        ci: usize,
        spell: SpellId,
        range_m: f32,
        aura: &str,
        duration_ms: u32,
        max_m: f32,
        amount: u32,
    ) {
        let caster = Self::id_of(ci);
        let me = self.units[ci].state.pos;
        let mut pool: Vec<EntityId> = self
            .players
            .iter()
            .copied()
            .filter(|p| {
                self.unit(*p).is_some_and(|u| {
                    u.alive && crate::geom::distance(u.pos, me) <= range_m.max(1.0)
                })
            })
            .collect();
        // Fisher-Yates on the combat stream: pairs differ between pulls but replay exactly.
        for k in (1..pool.len()).rev() {
            let j = self.rng.below(k as u32 + 1) as usize;
            pool.swap(k, j);
        }
        self.raid.tethers.retain(|t| t.source != caster);
        let until = self.now + ticks(duration_ms);
        let mut k = 0;
        while k < pool.len() {
            let a = pool[k];
            let b = pool.get(k + 1).copied().unwrap_or(caster);
            self.raid.tethers.push(Tether {
                a,
                b,
                source: caster,
                spell,
                aura: aura.to_string(),
                until,
                max_m,
                amount,
            });
            for p in [a, b] {
                if let Some(i) = self.index(p).filter(|_| self.is_player(p)) {
                    self.apply_aura(i, aura, duration_ms, None, caster, Some(spell));
                }
            }
            k += 2;
        }
        self.raid.next_tether_check = self.now + ticks(TETHER_TICK_MS);
        self.raid_broadcast();
    }

    fn update_tethers(&mut self) {
        if self.raid.tethers.is_empty() {
            return;
        }
        let before = self.raid.tethers.len();
        let now = self.now;
        let alive = |s: &Sim, id: EntityId| {
            s.unit(id)
                .is_some_and(|u| u.alive && u.ai != Some(AiState::Evade))
        };
        let keep: Vec<bool> = self
            .raid
            .tethers
            .iter()
            .map(|t| t.until > now && alive(self, t.source) && alive(self, t.a) && alive(self, t.b))
            .collect();
        let mut gone: Vec<(EntityId, EntityId, String)> = Vec::new();
        let mut k = 0;
        self.raid.tethers.retain(|t| {
            let kept = keep[k];
            k += 1;
            if !kept {
                gone.push((t.a, t.b, t.aura.clone()));
            }
            kept
        });
        for (a, b, aura) in gone {
            for p in [a, b] {
                if let Some(i) = self.index(p) {
                    self.remove_aura(i, &aura, AuraEnd::Expired);
                }
            }
        }
        if self.now >= self.raid.next_tether_check {
            self.raid.next_tether_check = self.now + ticks(TETHER_TICK_MS);
            let mut hits = Vec::new();
            for t in &self.raid.tethers {
                let (Some(a), Some(b)) = (self.unit(t.a), self.unit(t.b)) else {
                    continue;
                };
                if crate::geom::distance(a.pos, b.pos) > t.max_m {
                    for p in [t.a, t.b] {
                        if self.is_player(p) {
                            hits.push((t.source, p, t.spell, t.amount));
                        }
                    }
                }
            }
            for (source, target, spell, amount) in hits {
                self.deal_damage(
                    source,
                    target,
                    Hit {
                        spell: Some(spell),
                        amount,
                        crit: false,
                        combo: false,
                        school: School::Arcane,
                        dot: true,
                    },
                );
            }
        }
        if self.raid.tethers.len() != before {
            self.raid_broadcast();
        }
    }

    // ---- the Synergy Twins ----

    /// Linked HP: whatever one twin loses, the other loses too.
    pub(crate) fn raid_link_hp(&mut self, ti: usize) {
        if self.raid.linking || !self.in_raid() {
            return;
        }
        let Some(t) = self.content.raid.twins.as_ref() else {
            return;
        };
        let key = &self.units[ti].state.key;
        let other = if *key == t.boss {
            &t.twin
        } else if *key == t.twin {
            &t.boss
        } else {
            return;
        };
        let hp = self.units[ti].state.hp;
        if let Some(pi) = (0..self.units.len()).find(|&i| {
            let u = &self.units[i];
            !u.despawned && u.state.alive && u.state.key == *other
        }) && self.units[pi].state.hp > hp
        {
            // Proportional, so a different max HP (group scaling) still empties both at once.
            let (max_t, max_p) = (self.units[ti].state.max_hp, self.units[pi].state.max_hp);
            let want = (u64::from(hp) * u64::from(max_p) / u64::from(max_t.max(1))) as u32;
            self.units[pi].state.hp = want.min(self.units[pi].state.hp);
        }
    }

    /// A twin's death takes the other with it (same killer, so the floor boss pays out).
    fn raid_twin_death(&mut self, ti: usize, killer: Option<EntityId>) {
        let Some(t) = self.content.raid.twins.as_ref() else {
            return;
        };
        let key = self.units[ti].state.key.clone();
        let other = if key == t.boss {
            t.twin.clone()
        } else if key == t.twin {
            t.boss.clone()
        } else {
            return;
        };
        if let Some(pi) = (0..self.units.len()).find(|&i| {
            let u = &self.units[i];
            !u.despawned && u.state.alive && u.state.key == other
        }) {
            self.raid.linking = true;
            self.die(pi, killer);
            self.raid.linking = false;
        }
    }

    fn update_twins(&mut self) {
        let Some(t) = self.content.raid.twins.clone() else {
            return;
        };
        if !self.in_raid() {
            return;
        }
        let find = |s: &Sim, k: &str| {
            (0..s.units.len()).find(|&i| {
                let u = &s.units[i];
                !u.despawned && u.state.alive && u.state.key == k
            })
        };
        let (Some(a), Some(b)) = (find(self, &t.boss), find(self, &t.twin)) else {
            return;
        };
        let fighting = self.units[a].state.in_combat || self.units[b].state.in_combat;
        if !fighting {
            self.raid.next_swap = u64::MAX;
            return;
        }
        if self.raid.next_swap == u64::MAX {
            self.raid.next_swap = self.now + ticks(t.swap_ms);
            return;
        }
        if self.now < self.raid.next_swap {
            return;
        }
        self.raid.next_swap = self.now + ticks(t.swap_ms);
        let (pa, pb) = (self.units[a].state.pos, self.units[b].state.pos);
        for (i, pos) in [(a, pb), (b, pa)] {
            self.units[i].state.pos = pos;
            self.events.push(Event::Teleported {
                unit: Self::id_of(i),
                pos,
            });
        }
        let (from, to) = if self.has_aura(a, &t.shield_aura) {
            (a, b)
        } else {
            (b, a)
        };
        self.remove_aura(from, &t.shield_aura, AuraEnd::Expired);
        let to_id = Self::id_of(to);
        self.apply_aura(to, &t.shield_aura, SHIELD_MS, None, to_id, None);
        if !t.swap_yells.is_empty() {
            let line = t.swap_yells[self.rng.below(t.swap_yells.len() as u32) as usize].clone();
            self.request_yell_line(from, crate::content::YellKind::Blast, Some(line));
        }
    }

    fn has_aura(&self, i: usize, key: &str) -> bool {
        self.units[i].auras.iter().any(|a| a.key == key)
    }

    pub(crate) fn update_raid(&mut self) {
        self.update_tethers();
        self.update_twins();
    }

    // ---- kills, loot, lockout ----

    /// After a raid death: the twin goes along; a zone boss locks its encounter for the week
    /// and grants the raid achievements (`raid:<boss>`, `raid:<normal|async>:<boss>`).
    pub(crate) fn raid_on_kill(&mut self, ti: usize, killer: Option<EntityId>) {
        if !self.in_raid() {
            return;
        }
        if !self.raid.linking {
            self.raid_twin_death(ti, killer);
        }
        if !self.units[ti].floor_boss {
            return;
        }
        let key = self.units[ti].state.key.clone();
        let week = self.raid_week();
        let async_mode = self.raid.async_mode;
        self.raid.lock.insert(week, lock_key(&key, async_mode));
        self.raid.tethers.clear();
        let mode = if async_mode { "async" } else { "normal" };
        let keys = [format!("raid:{key}"), format!("raid:{mode}:{key}")];
        for p in self.players.clone() {
            self.check_achievements(
                p,
                |t| matches!(t, crate::world::Trigger::Event(k) if keys.contains(k)),
            );
        }
        self.raid_broadcast();
    }

    /// Raid-piece rolls of a raid boss kill: `Some((rolls, chance %, legendary %))`; the twin
    /// rolls nothing. `None` for everything else (raid trash rolls the Legacy tier).
    pub(crate) fn raid_loot(&self, mob: usize) -> Option<(u32, u32, u32)> {
        let u = &self.units[mob];
        if !self.in_raid() || !u.state.boss {
            return None;
        }
        let r = &self.content.raid;
        let legendary = self.content.economy.legendary_pct;
        Some(match (u.floor_boss, self.raid.async_mode) {
            (true, false) => (r.boss_items.max(1), 100, legendary),
            (true, true) => (1, r.async_item_pct, legendary / 2),
            (false, _) => (0, 0, 0),
        })
    }

    /// Department standing per raid boss kill: every department, halved on Async.
    pub(crate) fn raid_boss_rep(&self, ti: usize) -> Option<u32> {
        (self.in_raid() && self.units[ti].floor_boss).then(|| {
            let r = self.content.raid.boss_rep;
            if self.raid.async_mode { r / 2 } else { r }
        })
    }

    /// The lockout for the save: (ISO week, encounter keys).
    pub(crate) fn raid_lockout(&self) -> (u32, Vec<String>) {
        self.raid.lock.saved()
    }

    /// Restores a saved lockout; unknown encounters are dropped, an old week is stale.
    pub(crate) fn set_raid_lockout(&mut self, week: u32, keys: &[String]) {
        let bosses = self.content.raid.bosses(&self.content);
        let known: Vec<String> = keys
            .iter()
            .filter(|k| {
                bosses
                    .iter()
                    .any(|b| k.strip_prefix(ASYNC_PREFIX).unwrap_or(k) == b)
            })
            .cloned()
            .collect();
        self.raid.lock.restore(week, known);
        self.raid_broadcast();
    }

    /// Debug/QA: this week's raid lockout set to `keys` (screenshots).
    pub fn debug_set_raid_lockout(&mut self, keys: &[String]) {
        let week = self.raid_week();
        self.set_raid_lockout(week, keys);
    }

    /// Debug/QA: the difficulty for the next raid loads regardless of group size (`None`: by
    /// group size again).
    pub fn debug_force_raid_async(&mut self, async_mode: Option<bool>) {
        self.raid.force = async_mode;
    }
}
