//! Enemy behaviour: idle -> aggro (with the pack) -> chase -> attack/abilities -> evade -> respawn.
//!
//! Abilities are a tiny rule interpreter: per rule a spell, a cooldown range, an
//! initial delay, a target rule and optional phase gates. First ready rule wins.

use crate::content::{AiDef, AiTarget, School, YellKind, ticks};
use crate::effects::dist_xz;
use crate::geom::{Vec3, distance, line_of_sight};
use crate::sim::{Hit, Sim};
use crate::targeting::EntityId;
use crate::time::TICK_HZ;
use crate::types::{AiState, AuraEnd, Event};

/// Inverse of `geom::forward` (yaw 0 faces -Z).
pub(crate) fn yaw_toward(from: Vec3, to: Vec3) -> f32 {
    (-(to.x - from.x)).atan2(-(to.z - from.z))
}

/// Moves `pos` toward `to` by at most `step`, never closer than `stop`.
pub(crate) fn step_toward(pos: Vec3, to: Vec3, step: f32, stop: f32) -> Vec3 {
    let d = dist_xz(pos, to);
    if d <= stop || d < 1e-6 {
        return pos;
    }
    let m = step.min(d - stop) / d;
    Vec3::new(
        pos.x + (to.x - pos.x) * m,
        pos.y,
        pos.z + (to.z - pos.z) * m,
    )
}

impl Sim {
    pub(crate) fn add_threat(&mut self, ti: usize, source: EntityId, amount: u32) {
        // Self-inflicted damage (Phishing Test) must not make a unit hunt itself.
        if source == Self::id_of(ti) {
            return;
        }
        let first = self.units[ti].threat.is_empty();
        let pct = self
            .index(source)
            .and_then(|si| self.class_at(si))
            .map_or(100, |c| c.threat_pct);
        let amount = if pct == 100 {
            amount
        } else {
            (u64::from(amount) * u64::from(pct) / 100).max(1) as u32
        };
        *self.units[ti].threat.entry(source).or_insert(0) += amount;
        if !first {
            return;
        }
        self.units[ti].combat_since = self.now;
        let abilities = self.content.units[self.units[ti].def]
            .ai
            .as_ref()
            .map(|a| a.abilities.clone())
            .unwrap_or_default();
        // First uses come after an initial delay (8-12 s for the Blast), not instantly.
        for (k, ab) in abilities.iter().enumerate() {
            let (lo, hi) = ab
                .initial_ms
                .unwrap_or((ab.cooldown_min_ms, ab.cooldown_max_ms));
            let ms = self.rng.range_inclusive(lo, hi);
            if let Some(slot) = self.units[ti].ability_next.get_mut(k) {
                *slot = self.now + ticks(ms);
            }
        }
        let Some(pack) = self.units[ti].state.pack else {
            return;
        };
        self.pull_linked(ti, pack, source);
        for o in 0..self.units.len() {
            let u = &self.units[o];
            let joins = o != ti
                && u.state.pack == Some(pack)
                && u.state.alive
                && !u.despawned
                && u.threat.is_empty()
                && u.state.ai == Some(AiState::Idle);
            if joins {
                self.add_threat(o, source, 1);
                self.set_ai(o, AiState::Chase);
            }
        }
    }

    /// Linked packs: idle trash of another pack standing within `pack_link_m` of the pulled
    /// unit joins the fight (and brings its own pack). Bosses never link.
    fn pull_linked(&mut self, ti: usize, pack: u32, source: EntityId) {
        let r = self.content.config.pack_link_m;
        if r <= 0.0 || self.units[ti].floor_boss {
            return;
        }
        let at = self.units[ti].state.pos;
        let linked: Vec<usize> = (0..self.units.len())
            .filter(|&o| {
                let u = &self.units[o];
                u.state.pack.is_some_and(|p| p != pack)
                    && u.state.alive
                    && !u.despawned
                    && !u.floor_boss
                    && u.summoner.is_none()
                    && u.threat.is_empty()
                    && u.state.ai == Some(AiState::Idle)
                    && dist_xz(u.state.pos, at) <= r
            })
            .collect();
        for o in linked {
            if self.units[o].threat.is_empty() {
                self.add_threat(o, source, 1);
                self.set_ai(o, AiState::Chase);
            }
        }
    }

    pub(crate) fn update_ai(&mut self) {
        for i in 0..self.units.len() {
            if self.units[i].despawned {
                continue;
            }
            if let Some(ai) = self.content.units[self.units[i].def].ai.clone() {
                self.ai_unit(i, &ai);
            }
        }
    }

    pub(crate) fn set_ai(&mut self, i: usize, state: AiState) {
        if self.units[i].state.ai != Some(state) {
            self.units[i].state.ai = Some(state);
            self.events.push(Event::AiState {
                unit: Self::id_of(i),
                state,
            });
        }
    }

    fn ai_unit(&mut self, i: usize, ai: &AiDef) {
        if !self.units[i].state.alive {
            if self.units[i].respawn_at.is_some_and(|t| t <= self.now) {
                self.respawn_unit(i);
            }
            return;
        }
        let dead: Vec<EntityId> = self.units[i]
            .threat
            .keys()
            .copied()
            .filter(|t| self.index(*t).is_none_or(|ti| !self.units[ti].state.alive))
            .collect();
        for t in dead {
            self.units[i].threat.remove(&t);
        }
        match self.units[i].state.ai {
            Some(AiState::Evade) => self.evade_step(i, ai),
            _ if self.units[i].threat.is_empty() => {
                if self.units[i].state.ai != Some(AiState::Idle) {
                    self.begin_evade(i);
                } else {
                    self.try_aggro(i, ai);
                }
            }
            _ => {
                if dist_xz(self.units[i].state.pos, self.units[i].spawn_pos) > ai.leash_m {
                    self.begin_evade(i);
                } else {
                    self.fight(i, ai);
                }
            }
        }
    }

    /// Pulls on the nearest living group member in aggro range and sight (ties: join order).
    fn try_aggro(&mut self, i: usize, ai: &AiDef) {
        let me = self.units[i].state.pos;
        let mut best: Option<(usize, f32)> = None;
        for p in self.players.iter().filter_map(|p| self.index(*p)) {
            let them = self.units[p].state.pos;
            let d = dist_xz(me, them);
            if self.units[p].state.alive
                && d <= ai.aggro_m
                && best.is_none_or(|(_, bd)| d < bd)
                && line_of_sight(me, them, &self.blockers)
            {
                best = Some((p, d));
            }
        }
        if let Some((p, _)) = best {
            self.add_threat(i, Self::id_of(p), 1);
            self.set_ai(i, AiState::Chase);
        }
    }

    fn top_threat(&self, i: usize) -> Option<EntityId> {
        // Highest threat wins; BTreeMap order makes ties go to the lowest id.
        self.units[i]
            .threat
            .iter()
            .fold(None, |best: Option<(EntityId, u32)>, (id, t)| match best {
                Some((_, bt)) if bt >= *t => best,
                _ => Some((*id, *t)),
            })
            .map(|(id, _)| id)
    }

    fn fight(&mut self, i: usize, ai: &AiDef) {
        let top = self.top_threat(i);
        let Some(ti) = top.and_then(|t| self.index(t)) else {
            return;
        };
        self.units[i].state.target = top;
        if self.has_aura_flag(i, |a| a.stun) {
            return;
        }
        let (me, them) = (self.units[i].state.pos, self.units[ti].state.pos);
        self.units[i].state.yaw = yaw_toward(me, them);
        self.check_phase(i, ai);
        if !self.units[i].state.alive {
            return;
        }
        self.check_enrage(i, ai);
        if self.units[i].cast.is_some() {
            return self.set_ai(i, AiState::Casting);
        }
        if self.flee_step(i, ai) {
            return;
        }
        if self.try_abilities(i, ai, false) && self.units[i].cast.is_some() {
            return self.set_ai(i, AiState::Casting);
        }
        // An instant ability may have moved or killed someone.
        let Some(ti) = self
            .top_threat(i)
            .and_then(|t| self.index(t))
            .filter(|_| self.units[i].state.alive)
        else {
            return;
        };
        let (me, them) = (self.units[i].state.pos, self.units[ti].state.pos);
        if dist_xz(me, them) > ai.melee_range_m {
            let step = ai.chase_speed_mps / TICK_HZ as f32;
            match self.nav_target(i, them) {
                Some((wp, last)) => {
                    // Stop slightly inside melee range so float jitter cannot flap Chase/Attack.
                    let stop = if last { ai.melee_range_m * 0.9 } else { 0.0 };
                    if !last {
                        self.units[i].state.yaw = yaw_toward(me, wp);
                    }
                    self.units[i].state.pos = step_toward(me, wp, step, stop);
                }
                None if self.nav_gave_up(i) => return self.begin_evade(i),
                None => {}
            }
            return self.set_ai(i, AiState::Chase);
        }
        self.set_ai(i, AiState::Attack);
        if self.units[i].melee_next <= self.now {
            self.units[i].melee_next = self.now + ticks(ai.melee_ms);
            let (src, dst) = (Self::id_of(i), Self::id_of(ti));
            self.events.push(Event::MeleeSwing {
                attacker: src,
                target: dst,
            });
            if !self.rng.chance_pct(self.content.config.white_hit_pct) {
                self.events.push(Event::Miss {
                    source: src,
                    target: dst,
                    spell: None,
                });
                return;
            }
            let amount = u64::from(ai.melee_damage) * u64::from(self.units[i].dmg_pct) / 100;
            let hit = Hit {
                spell: None,
                amount: amount as u32,
                crit: false,
                combo: false,
                school: School::Physical,
                dot: false,
            };
            self.deal_damage(src, dst, hit);
        }
    }

    /// Enters every phase whose threshold the HP has crossed, one after another.
    fn check_phase(&mut self, i: usize, ai: &AiDef) {
        loop {
            let u = &self.units[i];
            let Some(p) = ai.phases.get(usize::from(u.state.phase) - 1) else {
                return;
            };
            if u64::from(u.state.hp) * 100 > u64::from(u.state.max_hp) * u64::from(p.below_hp_pct) {
                return;
            }
            let phase = u.state.phase + 1;
            self.units[i].state.phase = phase;
            // Abilities gated to the new phase start their initial delay now, not at the pull.
            for (k, ab) in ai.abilities.iter().enumerate() {
                if ab.phases.contains(&phase) {
                    let (lo, hi) = ab
                        .initial_ms
                        .unwrap_or((ab.cooldown_min_ms, ab.cooldown_max_ms));
                    let ms = self.rng.range_inclusive(lo, hi);
                    if let Some(slot) = self.units[i].ability_next.get_mut(k) {
                        *slot = self.now + ticks(ms);
                    }
                }
            }
            self.events.push(Event::PhaseChanged {
                unit: Self::id_of(i),
                phase,
            });
            if !p.yells.is_empty() {
                let cur = &mut self.units[i].yell_cursor[YellKind::Phase as usize];
                let line = p.yells[*cur % p.yells.len()].clone();
                *cur += 1;
                self.request_yell_line(i, YellKind::Phase, Some(line));
            }
            if let Some(spell) = p.spell.and_then(|s| self.content.spell(s)).cloned() {
                let target = self.pick_target(i, p.target, spell.range_m);
                self.resolve(i, &spell, target.map(Self::id_of));
            }
        }
    }

    /// `None` when no unit satisfies the rule (the ability then waits, cooldown untouched).
    fn pick_target(&mut self, i: usize, rule: AiTarget, range: f32) -> Option<usize> {
        let me = self.units[i].state.pos;
        let side = self.units[i].state.hostile;
        let reachable = |s: &Sim, o: usize| {
            let u = &s.units[o];
            u.state.alive
                && !u.despawned
                && distance(me, u.state.pos) <= range
                && line_of_sight(me, u.state.pos, &s.blockers)
        };
        match rule {
            AiTarget::Caster => Some(i),
            AiTarget::Current => self
                .top_threat(i)
                .and_then(|t| self.index(t))
                .filter(|&t| reachable(self, t)),
            AiTarget::AllyLowestHp => (0..self.units.len())
                .filter(|&o| {
                    let u = &self.units[o];
                    (o == i || reachable(self, o))
                        && u.state.alive
                        && u.state.hostile == side
                        && u.state.hp < u.state.max_hp
                        && u.state.ai.is_some()
                })
                .min_by_key(|&o| {
                    let s = &self.units[o].state;
                    (u64::from(s.hp) * 1000 / u64::from(s.max_hp.max(1)), o)
                }),
            AiTarget::RandomHostile => {
                let pool: Vec<usize> = (0..self.units.len())
                    .filter(|&o| {
                        self.units[o].state.hostile != side
                            && self.units[o].state.ai != Some(AiState::Evade)
                            && reachable(self, o)
                    })
                    .collect();
                if pool.is_empty() {
                    return None;
                }
                let k = self.rng.below(pool.len() as u32) as usize;
                pool.get(k).copied()
            }
        }
    }

    /// Soft timer: the aura goes on once per pull, `after_ms` after it.
    fn check_enrage(&mut self, i: usize, ai: &AiDef) {
        let Some(en) = &ai.enrage else { return };
        let u = &self.units[i];
        let due = u.combat_since > 0 && self.now >= u.combat_since + ticks(en.after_ms);
        if !due || u.auras.iter().any(|a| a.key == en.aura) {
            return;
        }
        // Lasts until the fight ends (evade and death clear every aura).
        self.apply_aura(i, &en.aura, 3_600_000, None, Self::id_of(i), None);
        if !en.yells.is_empty() {
            let cur = &mut self.units[i].yell_cursor[YellKind::Phase as usize];
            let line = en.yells[*cur % en.yells.len()].clone();
            *cur += 1;
            self.request_yell_line(i, YellKind::Phase, Some(line));
        }
    }

    /// `AiDef::flee`: runs for the exit (instant abilities only on the way), casts the flee
    /// spell on arrival, then fights for `pause_ms`. True while it owns this tick.
    fn flee_step(&mut self, i: usize, ai: &AiDef) -> bool {
        let Some(f) = &ai.flee else { return false };
        let u = &self.units[i];
        if u.state.phase < f.phase || (!u.fleeing && self.now < u.flee_next) {
            return false;
        }
        if !self.units[i].fleeing {
            self.units[i].fleeing = true;
            self.units[i].nav.reset();
            if !f.yells.is_empty() {
                let cur = &mut self.units[i].yell_cursor[YellKind::Phase as usize];
                let line = f.yells[*cur % f.yells.len()].clone();
                *cur += 1;
                self.request_yell_line(i, YellKind::Phase, Some(line));
            }
        }
        let (spawn, yaw) = (self.units[i].spawn_pos, self.units[i].spawn_yaw);
        let (fw, rt) = (crate::geom::forward(yaw), crate::telegraph::right(yaw));
        let exit = Vec3::new(
            spawn.x + rt.x * f.to.0 + fw.x * f.to.1,
            spawn.y,
            spawn.z + rt.z * f.to.0 + fw.z * f.to.1,
        );
        let pos = self.units[i].state.pos;
        let step = f.speed_mps / TICK_HZ as f32;
        if dist_xz(pos, exit) <= step {
            self.units[i].state.pos = exit;
            self.units[i].fleeing = false;
            self.units[i].flee_next = self.now + ticks(f.pause_ms);
            if let Some(spell) = self.content.spell(f.spell).cloned() {
                self.start_cast(i, &spell, Some(Self::id_of(i)));
            }
            self.set_ai(i, AiState::Casting);
            return true;
        }
        self.try_abilities(i, ai, true);
        let wp = self.nav_target(i, exit).map_or(exit, |(wp, _)| wp);
        self.units[i].state.yaw = yaw_toward(pos, wp);
        self.units[i].state.pos = step_toward(pos, wp, step, 0.0);
        self.set_ai(i, AiState::Chase);
        true
    }

    /// `instant_only`: on the run, cast-time abilities wait (they would root the unit).
    fn try_abilities(&mut self, i: usize, ai: &AiDef, instant_only: bool) -> bool {
        if self.has_aura_flag(i, |a| a.silence) {
            return false;
        }
        for (k, ab) in ai.abilities.iter().enumerate() {
            let u = &self.units[i];
            if !ab.phases.is_empty() && !ab.phases.contains(&u.state.phase) {
                continue;
            }
            if u.ability_next.get(k).is_none_or(|t| *t > self.now) {
                continue;
            }
            let Some(spell) = self.content.spell(ab.spell).cloned() else {
                continue;
            };
            if instant_only && spell.cast_ms > 0 {
                continue;
            }
            if self.now < u.lockout_until[spell.school.index()] {
                continue;
            }
            let Some(target) = self.pick_target(i, ab.target, spell.range_m) else {
                continue;
            };
            let ms = self
                .rng
                .range_inclusive(ab.cooldown_min_ms, ab.cooldown_max_ms);
            self.units[i].ability_next[k] = self.now + ticks(ms);
            self.start_cast(i, &spell, Some(Self::id_of(target)));
            if ab.yells.is_empty() {
                self.request_yell(i, YellKind::Blast);
            } else {
                let cur = &mut self.units[i].ability_yell_cursor[k];
                let line = ab.yells[*cur % ab.yells.len()].clone();
                *cur += 1;
                self.request_yell_line(i, YellKind::Blast, Some(line));
            }
            return true;
        }
        false
    }

    /// Resets the encounter: cast, every aura (Scope Creep stacks too), threat and phase.
    fn begin_evade(&mut self, i: usize) {
        // Kiting a pull away until it resets is not a free reset: no regen or rez for a while.
        let lock = self.now + ticks(self.content.config.combat_lock_ms);
        let chased: Vec<EntityId> = self.units[i].threat.keys().copied().collect();
        for p in chased {
            if let Some(pi) = self.index(p).filter(|_| self.is_player(p)) {
                let u = &mut self.units[pi];
                u.combat_lock_until = u.combat_lock_until.max(lock);
            }
        }
        self.cancel_cast(i, None, 0);
        self.cancel_telegraphs_of(Self::id_of(i));
        let keys: Vec<String> = self.units[i].auras.iter().map(|a| a.key.clone()).collect();
        for k in keys {
            self.remove_aura(i, &k, AuraEnd::Expired);
        }
        let u = &mut self.units[i];
        u.threat.clear();
        u.state.target = None;
        u.nav.reset();
        u.combat_since = 0;
        u.fleeing = false;
        u.flee_next = 0;
        if u.state.phase != 1 {
            u.state.phase = 1;
            self.events.push(Event::PhaseChanged {
                unit: Self::id_of(i),
                phase: 1,
            });
        }
        // Adds belong to this encounter; leaving them behind would strand them mid-room.
        self.despawn_adds_of(Self::id_of(i));
        self.set_ai(i, AiState::Evade);
        self.request_yell(i, YellKind::Evade);
    }

    fn evade_step(&mut self, i: usize, ai: &AiDef) {
        let (pos, home) = (self.units[i].state.pos, self.units[i].spawn_pos);
        let step = ai.chase_speed_mps / TICK_HZ as f32;
        if dist_xz(pos, home) <= step {
            // Summoned adds have no home to return to.
            if self.units[i].summoner.is_some() {
                return self.despawn(i);
            }
            let u = &mut self.units[i];
            u.state.pos = home;
            u.state.yaw = u.spawn_yaw;
            u.state.hp = u.state.max_hp;
            u.state.mana = u.state.max_mana;
            return self.set_ai(i, AiState::Idle);
        }
        let Some((wp, _)) = self.nav_target(i, home) else {
            // Walled off from home for good: reset in place, like an MMO evade teleport.
            if self.nav_gave_up(i) {
                self.units[i].state.pos = home;
                self.units[i].nav.reset();
            }
            return;
        };
        let u = &mut self.units[i];
        u.state.yaw = yaw_toward(pos, wp);
        u.state.pos = step_toward(pos, wp, step, 0.0);
    }

    fn respawn_unit(&mut self, i: usize) {
        let u = &mut self.units[i];
        u.state.alive = true;
        u.state.hp = u.state.max_hp;
        u.state.mana = u.state.max_mana;
        u.state.pos = u.spawn_pos;
        u.state.yaw = u.spawn_yaw;
        u.state.lootable = false;
        u.state.phase = 1;
        u.loot.clear();
        u.respawn_at = None;
        u.corpse_until = None;
        u.melee_next = 0;
        u.cooldowns.clear();
        u.lockout_until = [0; 2];
        u.last_player_hit = None;
        u.combat_since = 0;
        u.fleeing = false;
        u.flee_next = 0;
        self.events.push(Event::Respawned {
            unit: Self::id_of(i),
        });
        self.set_ai(i, AiState::Idle);
    }
}
