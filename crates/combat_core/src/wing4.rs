//! Ilmari Wing 4, The Cloud (docs/ILMARI_10.md C.3.4, C.3.5, C.5): a boss that replicates when it
//! is hit hard and keeps a bill for it (the Autoscaler), and a room of platforms that go dark in
//! turn (the Region Outage). Both are armed by `UnitDef::encounter` through `wing3.rs`, and both
//! run on sim state and the combat rng only, so a replay reproduces them exactly.
//!
//! Splitting: the damage players deal to a unit with `UnitDef::split` goes into a rolling window;
//! past the threshold the unit spawns a replica of itself that takes a share of its current HP,
//! so the pool is conserved. A replica is an ordinary unit of the same definition: it keeps the
//! boss's rewards (every member is a floor boss) and leaves through the "decommission" path
//! (`split_decommission`) instead of `die` while another member lives, so only the last death
//! pays out. The family shares its threat (a replica starts with its parent's table), resets
//! together (the idle ones despawn) and the bill meter keeps one count for all of it.
//!
//! Platforms: `ZoneDef::platforms` is the static layout (offsets from the boss's spawn point);
//! the state lives here. Standing off every platform that is not `Dark` (with a margin) for the
//! grace period fails the player over: a fixed hit that never takes the last HP, and a teleport
//! to the nearest live platform. Movement stays client-owned; the sim only reads and teleports.

use crate::content::{
    BillDef, EncounterDef, OutageDef, PlatformDef, PlatformKind, PlatformShape, SplitDef, ticks,
};
use crate::effects::dist_xz;
use crate::geom::Vec3;
use crate::sim::{Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::time::ticks_to_ms;
use crate::types::{AiState, Event, PlatformPhase};
use crate::world::PROP_OK;
use std::collections::{BTreeMap, VecDeque};

/// Failover and the bill's aura upkeep run this often (ticks).
const CHECK_TICKS: u64 = 6;
/// The bill meter in 1/300 points, so that a tenth of a point per second is one unit per tick.
const UNITS_PER_POINT: u32 = 300;
const BILL_MAX: u32 = 1000;
/// Replicas appear on a ring this far from their parent.
const RING_M: f32 = 5.0;
/// A replica splits only while its parent still has this share of its max HP (a husk would just
/// multiply the mechanics of a dying fight).
const MIN_SPLIT_HP_PCT: u32 = 10;
/// Auras of the bill last until the fight ends.
const BILL_AURA_MS: u32 = 3_600_000;
/// Landing spots stay this far inside the platform's edge (share of its size).
const LAND_PCT: f32 = 0.6;

struct BillRt {
    cfg: BillDef,
    /// In 1/300 points.
    units: u32,
    reserve_until: u64,
    step: u8,
    budget: bool,
    /// What the clients were last told: (tenths of the meter, copies, reserved).
    sent: (u16, u8, bool),
}

struct OutageRt {
    cfg: OutageDef,
    origin: Vec3,
    defs: Vec<PlatformDef>,
    /// Phase and the tick it began, per platform.
    phase: Vec<(PlatformPhase, u64)>,
    /// Region indices in turn order, the first region first; the tour repeats.
    order: Vec<usize>,
    next: usize,
    next_start: u64,
    warned: bool,
    /// The tick each player was first seen off the grid.
    grace: BTreeMap<EntityId, u64>,
    /// Beyond this distance from the origin a player is in the corridor, not on the cloud: no
    /// failover for the one still walking in from the lobby.
    reach: f32,
}

#[derive(Default)]
pub(crate) struct Wing4State {
    /// Player damage per split unit: (tick, HP lost).
    window: BTreeMap<EntityId, VecDeque<(u64, u32)>>,
    /// Members born of a split (the original is not one until it is the only one left).
    copies: Vec<EntityId>,
    next_split: u64,
    bill: Option<BillRt>,
    outage: Option<OutageRt>,
}

impl Sim {
    pub(crate) fn update_wing4(&mut self) {
        self.split_update();
        if self.wing4.bill.is_some() {
            self.bill_update();
        }
        if self.wing4.outage.is_some() {
            self.outage_update();
        }
    }

    // ------------------------------------------------------------------ arming

    pub(crate) fn wing4_arm(&mut self, bi: usize, enc: &EncounterDef) {
        match enc {
            EncounterDef::Cloud(cfg) => {
                self.wing4.bill = Some(BillRt {
                    cfg: cfg.clone(),
                    units: 0,
                    reserve_until: 0,
                    step: 0,
                    budget: false,
                    sent: (0, 1, false),
                });
                self.events.push(Event::CloudBill {
                    bill: 0,
                    copies: 1,
                    reserved: false,
                });
            }
            EncounterDef::Outage(cfg) => self.outage_arm(bi, cfg.clone()),
            _ => {}
        }
    }

    /// The fight ended one way or another: the bill and the platforms reset with it.
    pub(crate) fn wing4_disarm(&mut self) {
        if let Some(b) = self.wing4.bill.take()
            && b.sent != (0, 0, false)
        {
            self.events.push(Event::CloudBill {
                bill: 0,
                copies: 0,
                reserved: false,
            });
        }
        if let Some(o) = self.wing4.outage.take() {
            let at_ms = ticks_to_ms(self.now);
            for idx in 0..o.defs.len() {
                if o.phase[idx].0 != PlatformPhase::Live {
                    self.events.push(Event::PlatformState {
                        idx: idx as u8,
                        state: PlatformPhase::Live,
                        at_ms,
                    });
                }
            }
            self.events.push(Event::PlatformWarn {
                idx: u8::MAX,
                at_ms,
                order: Vec::new(),
            });
        }
    }

    /// A joiner sees the bill and the platforms as they stand.
    pub(crate) fn wing4_on_join(&mut self, player: EntityId) {
        if let Some(b) = &self.wing4.bill {
            let e = Event::CloudBill {
                bill: (b.units / UNITS_PER_POINT) as u16,
                copies: self.split_members().len().max(1) as u8,
                reserved: b.reserve_until > self.now,
            };
            self.emit_to(player, e);
        }
        if let Some(o) = &self.wing4.outage {
            let states: Vec<Event> = (0..o.defs.len())
                .map(|idx| Event::PlatformState {
                    idx: idx as u8,
                    state: o.phase[idx].0,
                    at_ms: ticks_to_ms(o.phase[idx].1),
                })
                .collect();
            let warn = self.outage_warn_event(o);
            for e in states {
                self.emit_to(player, e);
            }
            self.emit_to(player, warn);
        }
    }

    // ------------------------------------------------------------------ splitting

    /// Living units of any split definition, in index order.
    fn split_members(&self) -> Vec<usize> {
        (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                !u.despawned && u.state.alive && self.content.units[u.def].split.is_some()
            })
            .collect()
    }

    /// Called by `deal_damage` with the HP a hit took off a unit: only players feed the window
    /// (a companion's damage is small by design and would make Story solo unpredictable).
    pub(crate) fn split_note(&mut self, ti: usize, source: EntityId, dealt: u32) {
        if dealt == 0
            || self.content.units[self.units[ti].def].split.is_none()
            || !self.is_player(source)
        {
            return;
        }
        let id = Self::id_of(ti);
        self.wing4
            .window
            .entry(id)
            .or_default()
            .push_back((self.now, dealt));
    }

    /// True for a replica that has company: it runs the trimmed ability list.
    pub(crate) fn split_is_copy(&self, i: usize) -> bool {
        self.wing4.copies.contains(&Self::id_of(i)) && self.split_members().len() > 1
    }

    fn split_update(&mut self) {
        // Nothing but the Autoscaler has a split: the common case costs one scan of the units.
        if !self
            .units
            .iter()
            .any(|u| !u.despawned && u.state.alive && self.content.units[u.def].split.is_some())
        {
            if !self.wing4.window.is_empty() || !self.wing4.copies.is_empty() {
                self.wing4.window.clear();
                self.wing4.copies.clear();
            }
            return;
        }
        let members = self.split_members();
        // The fight is over for all of them: the replicas go, one unit walks home whole.
        if members.len() > 1
            && members
                .iter()
                .all(|&i| self.units[i].state.ai == Some(AiState::Idle))
        {
            for &i in &members[1..] {
                self.despawn(i);
            }
            self.wing4.window.clear();
            self.wing4.copies.clear();
            return;
        }
        if members.len() > 1 {
            // One threat table in effect: whoever one member has met, the others know.
            let mut known: BTreeMap<EntityId, u32> = BTreeMap::new();
            for &i in &members {
                for (&k, &t) in &self.units[i].threat {
                    let e = known.entry(k).or_insert(0);
                    *e = (*e).max(t);
                }
            }
            for &i in &members {
                for (&k, &t) in &known {
                    self.units[i].threat.entry(k).or_insert(t);
                }
            }
        }
        for &i in &members {
            let Some(sd) = self.content.units[self.units[i].def].split.clone() else {
                continue;
            };
            let id = Self::id_of(i);
            let window = ticks(sd.window_ms);
            let q = self.wing4.window.entry(id).or_default();
            while q.front().is_some_and(|(t, _)| t + window < self.now) {
                q.pop_front();
            }
            let sum: u64 = q.iter().map(|(_, d)| u64::from(*d)).sum();
            let (threshold, max_copies) = if self.raid.story {
                (sd.story_threshold_pct, sd.story_max_copies)
            } else {
                (sd.threshold_pct, sd.max_copies)
            };
            let u = &self.units[i];
            let spike = sum * 100 > u64::from(u.state.max_hp) * u64::from(threshold);
            let healthy = u64::from(u.state.hp) * 100
                >= u64::from(u.state.max_hp) * u64::from(MIN_SPLIT_HP_PCT);
            if spike
                && healthy
                && u.state.in_combat
                && self.now >= self.wing4.next_split
                && members.len() < usize::from(max_copies)
            {
                self.split_spawn(i, &sd, members.len());
                break;
            }
        }
        let alive: Vec<EntityId> = self.split_members().into_iter().map(Self::id_of).collect();
        self.wing4.window.retain(|k, _| alive.contains(k));
        self.wing4.copies.retain(|k| alive.contains(k));
    }

    /// The parent hands `hp_share_pct` of its current HP to a replica beside it.
    fn split_spawn(&mut self, pi: usize, sd: &SplitDef, out: usize) {
        let p = &self.units[pi];
        let hp = p.state.hp;
        let share = (u64::from(hp) * u64::from(sd.hp_share_pct) / 100) as u32;
        if share == 0 || share >= hp {
            return;
        }
        let (def, pos, yaw, level, pack, floor_boss) = (
            p.def,
            p.state.pos,
            p.state.yaw,
            p.state.level,
            p.state.pack,
            p.floor_boss,
        );
        let (home, home_yaw, phase, since, target) = (
            p.spawn_pos,
            p.spawn_yaw,
            p.state.phase,
            p.combat_since,
            p.state.target,
        );
        let threat = p.threat.clone();
        let next_ability = p.ability_next.clone();
        let a = (60.0 + 110.0 * out as f32).to_radians();
        let at = Vec3::new(pos.x + RING_M * a.cos(), pos.y, pos.z + RING_M * a.sin());
        let id = self.spawn(
            def,
            at,
            yaw,
            SpawnOpts {
                level: Some(level),
                pack,
                respawns: false,
                summoner: None,
                floor_boss,
                zone_slot: None,
            },
        );
        let Some(ci) = self.index(id) else { return };
        self.units[pi].state.hp = hp - share;
        let late = ticks(sd.offset_ms) * out as u64;
        let c = &mut self.units[ci];
        c.state.hp = share;
        c.spawn_pos = home;
        c.spawn_yaw = home_yaw;
        c.state.phase = phase;
        c.combat_since = since;
        c.state.target = target;
        c.threat = threat;
        c.ability_next = next_ability.iter().map(|t| t + late).collect();
        self.set_ai(ci, AiState::Chase);
        self.wing4.copies.push(id);
        self.wing4.next_split = self.now + ticks(sd.cooldown_ms);
        self.wing4.window.remove(&Self::id_of(pi));
        if !sd.yells.is_empty() {
            let line = sd.yells[self.rng.below(sd.yells.len() as u32) as usize].clone();
            self.request_yell_line(pi, crate::content::YellKind::Blast, Some(line));
        }
    }

    /// A member of a split family that dies while another lives is decommissioned: no loot, XP or
    /// lockout, a corpse that stays, and `relief` points off the bill. True when it took the death.
    pub(crate) fn split_decommission(&mut self, ti: usize) -> bool {
        let def = self.units[ti].def;
        let Some(sd) = self.content.units[def].split.as_ref() else {
            return false;
        };
        let lines = sd.decommission_yells.clone();
        let company = (0..self.units.len()).find(|&o| {
            let u = &self.units[o];
            o != ti && !u.despawned && u.state.alive && u.def == def
        });
        let Some(other) = company else {
            return false;
        };
        let id = Self::id_of(ti);
        self.kill_add(ti);
        self.kill_adds_of(id);
        self.wing4.window.remove(&id);
        if let Some(b) = &mut self.wing4.bill {
            let relief = b.cfg.relief.saturating_mul(UNITS_PER_POINT);
            b.units = b.units.saturating_sub(relief);
        }
        if !lines.is_empty() {
            let line = lines[self.rng.below(lines.len() as u32) as usize].clone();
            self.request_yell_line(other, crate::content::YellKind::Blast, Some(line));
        }
        true
    }

    // ------------------------------------------------------------------ the cloud bill

    fn bill_update(&mut self) {
        let Some(mut b) = self.wing4.bill.take() else {
            return;
        };
        let members = self.split_members();
        let copies = members.len().max(1);
        let table = &b.cfg.rate_tenths;
        let mut rate = table[(copies - 1).min(table.len() - 1)];
        if self.raid.story {
            rate = rate * b.cfg.story_rate_pct / 100;
        }
        if b.reserve_until > self.now {
            rate /= 2;
        }
        b.units = (b.units + rate).min(BILL_MAX * UNITS_PER_POINT);
        let points = b.units / UNITS_PER_POINT;
        let step = b.cfg.steps.iter().filter(|s| points >= **s).count() as u8;
        let budget = points >= BILL_MAX;
        // Yells on the way up; the replicas wear whatever the bill says.
        if step > b.step
            && let Some(&i) = members.first()
            && let Some(line) = b.cfg.step_yells.get(usize::from(step) - 1).cloned()
        {
            self.request_yell_line(i, crate::content::YellKind::Blast, Some(line));
        }
        if budget
            && !b.budget
            && let Some(&i) = members.first()
            && !b.cfg.budget_yells.is_empty()
        {
            let line = b.cfg.budget_yells[self.rng.below(b.cfg.budget_yells.len() as u32) as usize]
                .clone();
            self.request_yell_line(i, crate::content::YellKind::Blast, Some(line));
        }
        b.step = step;
        b.budget = budget;
        if self.now.is_multiple_of(CHECK_TICKS) {
            let key = if self.raid.story {
                b.cfg.story_overage_aura.clone()
            } else {
                b.cfg.overage_aura.clone()
            };
            for &i in &members {
                let id = Self::id_of(i);
                let stacks = self.units[i]
                    .auras
                    .iter()
                    .find(|a| a.key == key)
                    .map_or(0, |a| a.stacks);
                if stacks != step {
                    self.remove_aura(i, &key, crate::types::AuraEnd::Expired);
                    for _ in 0..step {
                        self.apply_aura(i, &key, BILL_AURA_MS, None, id, None);
                    }
                }
                let has = self.has_aura(i, &b.cfg.budget_aura);
                if budget && !has {
                    self.apply_aura(i, &b.cfg.budget_aura, BILL_AURA_MS, None, id, None);
                } else if !budget && has {
                    self.remove_aura(i, &b.cfg.budget_aura, crate::types::AuraEnd::Expired);
                }
            }
        }
        let reserved = b.reserve_until > self.now;
        // Tenths of the meter only: a bill of 0..=1000 sends at most a hundred frames a fight.
        let seen = (b.sent.0 / 10, b.sent.1, b.sent.2);
        if (points as u16 / 10, copies as u8, reserved) != seen {
            b.sent = (points as u16, copies as u8, reserved);
            self.events.push(Event::CloudBill {
                bill: points as u16,
                copies: copies as u8,
                reserved,
            });
        }
        self.wing4.bill = Some(b);
    }

    /// A Reserved Instance rack: the bill's rate is halved for the encounter's `reserve_ms`.
    pub(crate) fn use_rack_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        let mut outcome = 0;
        let ms = self.wing4.bill.as_ref().map(|b| b.cfg.reserve_ms);
        if let Some(ms) = ms
            && self.prop_ready(player, k, def.cooldown_ms)
        {
            let until = self.now + ticks(ms);
            if let Some(b) = &mut self.wing4.bill {
                b.reserve_until = until;
            }
            outcome = PROP_OK;
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key,
            outcome,
        });
    }

    // ------------------------------------------------------------------ the region outage

    fn outage_arm(&mut self, bi: usize, cfg: OutageDef) {
        let Some(zone) = self.zone.clone() else {
            return;
        };
        let defs = self
            .content
            .zone(&zone)
            .map(|z| z.platforms.clone())
            .unwrap_or_default();
        let first = defs.iter().position(|p| p.id == cfg.first);
        let Some(first) = first else { return };
        let mut rest: Vec<usize> = (0..defs.len())
            .filter(|&k| defs[k].kind == PlatformKind::Region && k != first)
            .collect();
        // One draw at the start of the encounter, not per player: a wipe starts over and a test
        // can fix it with the seed.
        for k in (1..rest.len()).rev() {
            let j = self.rng.below(k as u32 + 1) as usize;
            rest.swap(k, j);
        }
        let mut order = vec![first];
        order.extend(rest);
        let now = self.now;
        let at_ms = ticks_to_ms(now);
        for idx in 0..defs.len() {
            self.events.push(Event::PlatformState {
                idx: idx as u8,
                state: PlatformPhase::Live,
                at_ms,
            });
        }
        let mut rt = OutageRt {
            origin: self.units[bi].spawn_pos,
            phase: vec![(PlatformPhase::Live, now); defs.len()],
            defs,
            order,
            next: 0,
            next_start: now + ticks(cfg.first_ms),
            warned: false,
            grace: BTreeMap::new(),
            reach: 0.0,
            cfg,
        };
        rt.reach = rt
            .defs
            .iter()
            .map(|d| {
                let size = match d.shape {
                    PlatformShape::Disc { r } => r,
                    PlatformShape::Rect { hx, hz, .. } => hx.hypot(hz),
                };
                d.x.hypot(d.z) + size
            })
            .fold(0.0, f32::max)
            + 2.0 * rt.cfg.margin_m
            + 3.0;
        // Story shows the whole tour from the first moment.
        let e = self.outage_warn_event(&rt);
        self.events.push(e);
        rt.warned = false;
        self.wing4.outage = Some(rt);
    }

    fn outage_warn_event(&self, o: &OutageRt) -> Event {
        let idx = o.order[o.next % o.order.len()];
        if !o.warned && self.raid.story {
            // Story lists the order from the start; Full waits for the warning.
            return Event::PlatformWarn {
                idx: idx as u8,
                at_ms: ticks_to_ms(o.next_start),
                order: Self::rotation(o),
            };
        }
        if !o.warned {
            return Event::PlatformWarn {
                idx: u8::MAX,
                at_ms: 0,
                order: Vec::new(),
            };
        }
        Event::PlatformWarn {
            idx: idx as u8,
            at_ms: ticks_to_ms(o.next_start),
            order: if self.raid.story {
                Self::rotation(o)
            } else {
                Vec::new()
            },
        }
    }

    /// The tour from the next region on, as platform indices.
    fn rotation(o: &OutageRt) -> Vec<u8> {
        (0..o.order.len())
            .map(|k| o.order[(o.next + k) % o.order.len()] as u8)
            .collect()
    }

    fn outage_set(&mut self, o: &mut OutageRt, idx: usize, state: PlatformPhase) {
        o.phase[idx] = (state, self.now);
        self.events.push(Event::PlatformState {
            idx: idx as u8,
            state,
            at_ms: ticks_to_ms(self.now),
        });
    }

    /// A line of the armed boss's `lines`, by the rng.
    fn outage_yell(&mut self, lines: &[String]) {
        if lines.is_empty() {
            return;
        }
        let Some(bi) = self.wing3.boss.and_then(|b| self.index(b)) else {
            return;
        };
        let line = lines[self.rng.below(lines.len() as u32) as usize].clone();
        self.request_yell_line(bi, crate::content::YellKind::Blast, Some(line));
    }

    fn outage_update(&mut self) {
        let Some(mut o) = self.wing4.outage.take() else {
            return;
        };
        let story = self.raid.story;
        let now = self.now;
        // The lifecycle of each region: degraded, dark, restoring, live.
        for idx in 0..o.defs.len() {
            let (state, since) = o.phase[idx];
            let (len, next) = match state {
                PlatformPhase::Degraded => (o.cfg.degraded_ms, PlatformPhase::Dark),
                PlatformPhase::Dark => (o.cfg.dark_ms, PlatformPhase::Restoring),
                PlatformPhase::Restoring => (o.cfg.restoring_ms, PlatformPhase::Live),
                PlatformPhase::Live => continue,
            };
            if now < since + ticks(len) {
                continue;
            }
            self.outage_set(&mut o, idx, next);
            match next {
                PlatformPhase::Dark => {
                    let lines = if o.order.first() == Some(&idx) && o.next == 1 {
                        o.cfg.first_dark_yells.clone()
                    } else {
                        o.cfg.dark_yells.clone()
                    };
                    self.outage_yell(&lines);
                }
                PlatformPhase::Live => {
                    let lines = o.cfg.restored_yells.clone();
                    self.outage_yell(&lines);
                }
                _ => {}
            }
        }
        // The next region's turn.
        if now >= o.next_start {
            let idx = o.order[o.next % o.order.len()];
            // Story is one at a time in every phase, whatever a Health Check brought forward.
            let alone = !story || o.order.iter().all(|&r| o.phase[r].0 == PlatformPhase::Live);
            if o.phase[idx].0 == PlatformPhase::Live && alone {
                self.outage_set(&mut o, idx, PlatformPhase::Degraded);
                o.next += 1;
                let cascading = !story && self.outage_boss_pct() <= o.cfg.cascade_below_pct;
                let period = if story {
                    o.cfg.story_period_ms
                } else if cascading {
                    o.cfg.period_ms / 2
                } else {
                    o.cfg.period_ms
                };
                o.next_start = now + ticks(period);
                o.warned = false;
            } else {
                // Still dark from its last turn (a short tour): try again in a second.
                o.next_start = now + ticks(1000);
            }
        }
        let warn_ms = if story {
            o.cfg.story_warn_ms
        } else {
            o.cfg.warn_ms
        };
        if !o.warned && o.next_start <= now + ticks(warn_ms) {
            o.warned = true;
            let e = self.outage_warn_event(&o);
            self.events.push(e);
        }
        if now.is_multiple_of(CHECK_TICKS) {
            self.outage_failover(&mut o);
        }
        self.wing4.outage = Some(o);
    }

    /// Boss HP in percent, 100 when it is not there.
    fn outage_boss_pct(&self) -> u32 {
        self.wing3
            .boss
            .and_then(|b| self.index(b))
            .map_or(100, |i| {
                let s = &self.units[i].state;
                (u64::from(s.hp) * 100 / u64::from(s.max_hp.max(1))) as u32
            })
    }

    /// Whether `pos` stands on a platform that is not dark (with the encounter's margin).
    fn on_grid(o: &OutageRt, pos: Vec3) -> bool {
        let (x, z) = (pos.x - o.origin.x, pos.z - o.origin.z);
        o.defs.iter().enumerate().any(|(i, d)| {
            o.phase[i].0 != PlatformPhase::Dark && d.dist_outside(x, z) <= o.cfg.margin_m
        })
    }

    fn outage_failover(&mut self, o: &mut OutageRt) {
        let story = self.raid.story;
        let players = self.players.clone();
        for p in players {
            let Some(pi) = self.index(p) else { continue };
            let s = &self.units[pi].state;
            if !s.alive || Self::on_grid(o, s.pos) || dist_xz(s.pos, o.origin) > o.reach {
                o.grace.remove(&p);
                continue;
            }
            let since = *o.grace.entry(p).or_insert(self.now);
            if self.now < since + ticks(o.cfg.grace_ms) {
                continue;
            }
            o.grace.remove(&p);
            let pct = if story {
                o.cfg.story_failover_pct
            } else {
                o.cfg.failover_pct
            };
            self.failover(o, pi, pct);
        }
    }

    /// The fixed hit, never the last point, and the teleport to the nearest platform that is live.
    fn failover(&mut self, o: &OutageRt, pi: usize, pct: u32) {
        let from = self.units[pi].state.pos;
        let (rx, rz) = (from.x - o.origin.x, from.z - o.origin.z);
        let s = &self.units[pi].state;
        let hit = ((u64::from(s.max_hp) * u64::from(pct) / 100) as u32).min(s.hp.saturating_sub(1));
        let live = |i: usize| {
            o.defs[i].kind != PlatformKind::Bridge && o.phase[i].0 == PlatformPhase::Live
        };
        let best = (0..o.defs.len())
            .filter(|&i| live(i))
            .min_by(|&a, &b| {
                o.defs[a]
                    .dist_outside(rx, rz)
                    .total_cmp(&o.defs[b].dist_outside(rx, rz))
                    .then(a.cmp(&b))
            })
            .or_else(|| (0..o.defs.len()).find(|&i| o.defs[i].kind == PlatformKind::Hub));
        let Some(best) = best else { return };
        let (lx, lz) = land_on(&o.defs[best], rx, rz);
        let pos = Vec3::new(o.origin.x + lx, from.y, o.origin.z + lz);
        let id = Self::id_of(pi);
        let boss = self.wing3.boss.unwrap_or(id);
        let u = &mut self.units[pi];
        u.state.hp -= hit;
        u.state.pos = pos;
        u.moving = false;
        self.events.push(Event::Damage {
            source: boss,
            target: id,
            spell: None,
            amount: hit,
            absorbed: 0,
            crit: false,
            combo: false,
            school: crate::content::School::Arcane,
        });
        self.events.push(Event::Teleported { unit: id, pos });
        let lines = o.cfg.failover_yells.clone();
        self.outage_yell(&lines);
    }

    /// `Effect::AdvanceOutage`: the next region goes `ms` sooner.
    pub(crate) fn outage_advance(&mut self, ms: u32) {
        if let Some(o) = &mut self.wing4.outage {
            o.next_start = o.next_start.saturating_sub(ticks(ms)).max(self.now);
        }
    }

    // ------------------------------------------------------------------ companions

    /// A companion standing on a platform that is not safe, or on another platform than its
    /// owner, joins the owner (it never takes a hit it cannot dodge, and never strands itself).
    pub(crate) fn companion_platform_snap(&self, ci: usize, oi: usize) -> bool {
        let Some(o) = &self.wing4.outage else {
            return false;
        };
        let mine = platform_index(o, self.units[ci].state.pos);
        let theirs = platform_index(o, self.units[oi].state.pos);
        match mine {
            Some(m) => {
                let unsafe_here =
                    matches!(o.phase[m].0, PlatformPhase::Dark | PlatformPhase::Degraded);
                theirs.is_some_and(|t| t != m) || (unsafe_here && theirs != Some(m))
            }
            // Off every platform and not on a bridge either.
            None => !Self::on_grid(o, self.units[ci].state.pos),
        }
    }

    // ------------------------------------------------------------------ views (tests, bots, UI)

    /// Cloud bill points (0..=1000) while the Autoscaler's fight is armed.
    pub fn cloud_bill(&self) -> Option<u32> {
        self.wing4.bill.as_ref().map(|b| b.units / UNITS_PER_POINT)
    }

    /// QA and tests: sets the bill to `points`.
    pub fn debug_set_cloud_bill(&mut self, points: u32) -> bool {
        match &mut self.wing4.bill {
            Some(b) => {
                b.units = points.min(BILL_MAX) * UNITS_PER_POINT;
                true
            }
            None => false,
        }
    }

    /// Living units of the split family (1 before the first split).
    pub fn split_copies(&self) -> usize {
        self.split_members().len()
    }

    /// The platforms of the armed outage: id, label and phase, in the zone's order.
    pub fn platform_phases(&self) -> Vec<(String, PlatformPhase)> {
        self.wing4.outage.as_ref().map_or_else(Vec::new, |o| {
            o.defs
                .iter()
                .zip(&o.phase)
                .map(|(d, p)| (d.id.clone(), p.0))
                .collect()
        })
    }

    /// The tour of the armed outage, region ids in turn order.
    pub fn platform_order(&self) -> Vec<String> {
        self.wing4.outage.as_ref().map_or_else(Vec::new, |o| {
            o.order.iter().map(|&i| o.defs[i].id.clone()).collect()
        })
    }

    /// World-space centre and radius-ish size of every platform of the armed outage with its id,
    /// for a bot that stands on a live one.
    pub fn platform_spots(&self) -> Vec<(String, Vec3, PlatformKind, PlatformPhase)> {
        self.wing4.outage.as_ref().map_or_else(Vec::new, |o| {
            o.defs
                .iter()
                .zip(&o.phase)
                .map(|(d, p)| {
                    (
                        d.id.clone(),
                        Vec3::new(o.origin.x + d.x, o.origin.y, o.origin.z + d.z),
                        d.kind,
                        p.0,
                    )
                })
                .collect()
        })
    }

    /// Whether `pos` stands on a platform that is not dark, margin included; `None` while no
    /// outage is armed. What a bot (or a player reading the colours) steers by.
    pub fn on_platform_grid(&self, pos: Vec3) -> Option<bool> {
        self.wing4
            .outage
            .as_ref()
            .map(|o| Self::on_grid(o, pos) || dist_xz(pos, o.origin) > o.reach)
    }

    /// Debug/test: puts any unit at `pos` (a companion left behind, say).
    pub fn debug_teleport_unit(&mut self, id: EntityId, pos: Vec3) -> bool {
        match self.index(id) {
            Some(i) => {
                self.units[i].state.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Tick of the next region's turn (QA).
    pub fn outage_next_in_ms(&self) -> Option<u64> {
        self.wing4
            .outage
            .as_ref()
            .map(|o| ticks_to_ms(o.next_start.saturating_sub(self.now)))
    }
}

/// The index of the region or hub platform under `pos`, bridges excluded.
fn platform_index(o: &OutageRt, pos: Vec3) -> Option<usize> {
    let (x, z) = (pos.x - o.origin.x, pos.z - o.origin.z);
    (0..o.defs.len()).find(|&i| {
        o.defs[i].kind != PlatformKind::Bridge && o.defs[i].dist_outside(x, z) <= o.cfg.margin_m
    })
}

/// The landing spot on `d` nearest to `(x, z)` (offsets from the origin), pulled in from the edge.
fn land_on(d: &PlatformDef, x: f32, z: f32) -> (f32, f32) {
    let (dx, dz) = (x - d.x, z - d.z);
    match d.shape {
        PlatformShape::Disc { r } => {
            let len = (dx * dx + dz * dz).sqrt();
            if len < 1e-3 {
                return (d.x + r * 0.3, d.z);
            }
            let k = len.min(r * LAND_PCT) / len;
            (d.x + dx * k, d.z + dz * k)
        }
        PlatformShape::Rect { hx, hz, yaw_deg } => {
            let (sn, cs) = yaw_deg.to_radians().sin_cos();
            let (lx, lz) = (dx * cs + dz * sn, -dx * sn + dz * cs);
            let (lx, lz) = (
                lx.clamp(-hx * LAND_PCT, hx * LAND_PCT),
                lz.clamp(-hz * LAND_PCT, hz * LAND_PCT),
            );
            (d.x + lx * cs - lz * sn, d.z + lx * sn + lz * cs)
        }
    }
}
