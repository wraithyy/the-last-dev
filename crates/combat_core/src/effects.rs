//! Spell effects and the aura model: stacks, periodic ticks, modifiers, expiry, dispel, spread.

use crate::content::{Area, AuraDef, Effect, School, SpellDef, SpellId, ticks};
use crate::geom::{Vec3, distance, forward};
use crate::sim::{AuraRt, Hit, Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::types::{AiState, AuraEnd, Event};

/// Horizontal distance: effects and AI work on the ground plane.
pub(crate) fn dist_xz(a: Vec3, b: Vec3) -> f32 {
    let (dx, dz) = (a.x - b.x, a.z - b.z);
    (dx * dx + dz * dz).sqrt()
}

/// Scales `v` by `100 + pct` percent, never below zero.
fn pct_mod(v: u64, pct: i64) -> u64 {
    (v as i64 * (100 + pct).max(0) / 100) as u64
}

impl Sim {
    pub(crate) fn has_aura_flag(&self, i: usize, f: impl Fn(&AuraDef) -> bool) -> bool {
        self.units[i]
            .auras
            .iter()
            .any(|a| self.content.aura(&a.key).is_some_and(&f))
    }

    /// Sum of a per-stack aura modifier on unit `i`.
    fn aura_sum(&self, i: usize, f: impl Fn(&AuraDef) -> i32) -> i64 {
        self.units[i]
            .auras
            .iter()
            .filter_map(|a| {
                let d = self.content.aura(&a.key)?;
                Some(i64::from(f(d)) * i64::from(a.stacks))
            })
            .sum()
    }

    pub(crate) fn haste(&self, i: usize) -> u32 {
        let base = self.content.units[self.units[i].def].haste_pct;
        base + self.units[i].item_bonus.haste_pct
            + self.aura_sum(i, |a| a.haste_pct as i32).max(0) as u32
    }

    /// Haste first, then cast-time auras (Review Fatigue, Pivot to AGI) and talent perks.
    pub(crate) fn cast_time(&self, i: usize, spell: &SpellDef) -> u32 {
        let hasted = self.content.cast_ms(spell, self.haste(i));
        let pct = self.aura_sum(i, |a| a.cast_time_pct) + self.perk_cast_time_pct(i, spell.id);
        pct_mod(u64::from(hasted), pct) as u32
    }

    /// Max-HP percent from auras (Horizontal Scaling); players only.
    pub(crate) fn max_hp_aura_pct(&self, i: usize) -> i64 {
        if self.units[i].class.is_none() {
            return 0;
        }
        self.aura_sum(i, |a| a.max_hp_pct)
    }

    /// Rebuilds max HP after a max-HP aura came or went: a gain also heals by the
    /// difference, a loss only clamps.
    fn refresh_max_hp(&mut self, i: usize) {
        let u = &self.units[i].state;
        let (old, hp, mana, level) = (u.max_hp, u.hp, u.mana, u.level);
        self.apply_level(i, level);
        let u = &mut self.units[i].state;
        u.mana = mana;
        u.hp = if !u.alive {
            0
        } else if u.max_hp > old {
            hp + (u.max_hp - old)
        } else {
            hp.min(u.max_hp)
        };
    }

    /// Attacker's damage-done and the target's damage-taken auras (plus the target's class
    /// armour), multiplied.
    pub(crate) fn modified_damage(&self, source: EntityId, ti: usize, amount: u32) -> u32 {
        let done = self
            .index(source)
            .map_or(0, |si| self.aura_sum(si, |a| a.damage_done_pct));
        let armour = self
            .class_at(ti)
            .map_or(0, |c| i64::from(c.damage_taken_pct));
        let taken =
            self.aura_sum(ti, |a| a.damage_taken_pct) + armour + self.perk_damage_taken_pct(ti);
        let gear = u64::from(self.units[ti].item_bonus.armor_tenths);
        let amount = u64::from(amount) * (1000 - gear.min(1000)) / 1000;
        pct_mod(pct_mod(amount, done), taken) as u32
    }

    /// Defensive Stakeholder: direct hits from inside the front arc are scaled down, unless the
    /// bearer is casting or stunned (its shield arm is busy).
    pub(crate) fn frontal_shield(&self, source: EntityId, ti: usize, amount: u32) -> u32 {
        let t = &self.units[ti];
        let Some(fs) = self.content.units[t.def].frontal_shield else {
            return amount;
        };
        let Some(si) = self.index(source).filter(|&si| si != ti) else {
            return amount;
        };
        if t.cast.is_some() || self.has_aura_flag(ti, |a| a.stun) {
            return amount;
        }
        let half = (fs.arc_deg / 2.0).to_radians();
        if crate::geom::is_facing(t.state.pos, t.state.yaw, self.units[si].state.pos, half) {
            pct_mod(u64::from(amount), i64::from(fs.damage_taken_pct)) as u32
        } else {
            amount
        }
    }

    /// Living, present units on the caster's side (`allies`) or the other side, within the radius.
    pub(crate) fn area_targets(&self, ci: usize, area: Area) -> Vec<usize> {
        let c = &self.units[ci].state;
        (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                u.state.alive
                    && !u.despawned
                    && u.state.ai != Some(AiState::Evade)
                    && self.hostile_to(ci, i) != area.allies
                    && !(area.allies && self.duel_blocks_help(ci, i))
                    && dist_xz(u.state.pos, c.pos) <= area.radius_m
            })
            .collect()
    }

    /// Amount from base + spell power, scaled by the caster's level.
    pub(crate) fn power(&self, ci: usize, base: u32, coefficient_pct: u32) -> u64 {
        let u = &self.units[ci];
        // Gear power (tenths of a percent); zero for everyone but geared players.
        (u64::from(base) + u64::from(u.sp) * u64::from(coefficient_pct) / 100)
            * u64::from(u.dmg_pct)
            * (1000 + u64::from(u.item_bonus.power_tenths))
            / 100_000
    }

    pub(crate) fn apply_effect(
        &mut self,
        ci: usize,
        spell: &SpellDef,
        e: &Effect,
        ti: Option<usize>,
    ) {
        let caster = Self::id_of(ci);
        if let Some(t) = ti {
            let u = &self.units[t];
            let revives = matches!(e, Effect::Resurrect { .. });
            if (!u.state.alive && !revives) || u.despawned || u.state.ai == Some(AiState::Evade) {
                return;
            }
        }
        match (e, ti) {
            (
                Effect::Damage {
                    base,
                    coefficient_pct,
                    apply_aura,
                    consume_aura,
                    execute,
                },
                Some(ti),
            ) => {
                // Hit roll first, so a miss neither consumes Code Smell nor applies an aura.
                if !self.rng.chance_pct(self.content.config.spell_hit_pct) {
                    self.events.push(Event::Miss {
                        source: caster,
                        target: Self::id_of(ti),
                        spell: Some(spell.id),
                    });
                    return;
                }
                let crit_pct = self.content.units[self.units[ci].def].crit_pct;
                let mut amount = self.power(ci, *base, *coefficient_pct);
                amount = self.perk_amount(ci, ti, spell.id, amount, false);
                let mut combo = false;
                let stacks = consume_aura.as_ref().and_then(|c| {
                    self.units[ti]
                        .auras
                        .iter()
                        .find(|a| a.key == c.aura)
                        .map(|a| u64::from(a.stacks))
                });
                if let (Some(c), Some(n)) = (consume_aura, stacks)
                    && self.remove_aura(ti, &c.aura, AuraEnd::Consumed)
                {
                    amount = amount * (100 + u64::from(c.bonus_pct) * n) / 100;
                    combo = true;
                }
                if let Some(x) = execute {
                    let t = &self.units[ti].state;
                    if u64::from(t.hp) * 100 <= u64::from(t.max_hp) * u64::from(x.below_pct) {
                        amount = amount * (100 + u64::from(x.bonus_pct)) / 100;
                    }
                }
                // Rolled for every hit so the RNG stream does not depend on crit chance.
                let crit = self
                    .rng
                    .chance_crit(crit_pct, self.units[ci].item_bonus.crit_tenths);
                if crit {
                    amount = amount * u64::from(self.content.config.crit_damage_pct) / 100;
                }
                if self.is_player(caster) {
                    self.units[ti].last_player_hit = Some((amount as u32, spell.school));
                }
                let hit = Hit {
                    spell: Some(spell.id),
                    amount: amount as u32,
                    crit,
                    combo,
                    school: spell.school,
                    dot: false,
                };
                self.deal_damage(caster, Self::id_of(ti), hit);
                self.perk_after_hit(ci, ti, spell, amount as u32);
                if let (Some(a), true) = (apply_aura, self.units[ti].state.alive) {
                    self.apply_aura(ti, &a.aura, a.duration_ms, None, caster, Some(spell.id));
                }
            }
            (
                Effect::Absorb {
                    aura,
                    amount,
                    duration_ms,
                },
                Some(ti),
            ) => self.apply_aura(
                ti,
                aura,
                *duration_ms,
                Some(*amount),
                caster,
                Some(spell.id),
            ),
            (Effect::Interrupt { lockout_ms }, Some(ti)) => {
                self.cancel_cast(ti, Some(caster), *lockout_ms);
            }
            (
                Effect::Heal {
                    base,
                    coefficient_pct,
                },
                Some(ti),
            ) => {
                let amount = self.power(ci, *base, *coefficient_pct);
                let amount = self.perk_amount(ci, ti, spell.id, amount, true) as u32;
                self.heal(caster, ti, Some(spell.id), amount);
            }
            (Effect::Aura { aura, duration_ms }, Some(ti)) => {
                self.apply_aura(ti, aura, *duration_ms, None, caster, Some(spell.id));
            }
            (Effect::RandomAura { auras, duration_ms }, Some(ti)) => {
                let k = self.rng.below(auras.len() as u32) as usize;
                if let Some(aura) = auras.get(k) {
                    self.apply_aura(ti, aura, *duration_ms, None, caster, Some(spell.id));
                }
            }
            (Effect::Dispel { count }, Some(ti)) => {
                for _ in 0..*count {
                    let newest = self.units[ti]
                        .auras
                        .iter()
                        .rev()
                        .find(|a| {
                            self.content.aura(&a.key).is_some_and(|d| {
                                d.harmful && d.dispel != crate::content::DispelType::Never
                            })
                        })
                        .map(|a| a.key.clone());
                    let Some(key) = newest else { break };
                    self.remove_aura(ti, &key, AuraEnd::Consumed);
                }
            }
            (Effect::Lockout { lockout_ms, school }, Some(ti)) => {
                let school = school
                    .or_else(|| {
                        let last = self.units[ti].last_cast?;
                        Some(self.content.spell(last)?.school)
                    })
                    .unwrap_or(School::Arcane);
                let until = self.now + ticks(*lockout_ms);
                let lock = &mut self.units[ti].lockout_until[school.index()];
                *lock = (*lock).max(until);
                let casting_school = self.units[ti]
                    .cast
                    .as_ref()
                    .and_then(|c| self.content.spell(c.spell))
                    .map(|s| s.school);
                if casting_school == Some(school) {
                    self.cancel_cast(ti, Some(caster), *lockout_ms);
                }
            }
            (Effect::Knockback { strength_m }, Some(ti)) => {
                let (from, to) = (self.units[ci].state.pos, self.units[ti].state.pos);
                let d = dist_xz(from, to);
                let dir = if d > 1e-4 {
                    Vec3::new((to.x - from.x) / d, 0.0, (to.z - from.z) / d)
                } else {
                    forward(self.units[ci].state.yaw)
                };
                if !self.is_player(Self::id_of(ti)) {
                    let p = &mut self.units[ti].state.pos;
                    p.x += dir.x * strength_m;
                    p.z += dir.z * strength_m;
                }
                self.events.push(Event::Knockback {
                    unit: Self::id_of(ti),
                    dir,
                    strength: *strength_m,
                });
            }
            (Effect::Teleport { distance_m }, Some(ti)) => {
                let c = &self.units[ci];
                let f = forward(c.spawn_yaw);
                let pos = Vec3::new(
                    c.spawn_pos.x + f.x * distance_m,
                    c.spawn_pos.y,
                    c.spawn_pos.z + f.z * distance_m,
                );
                self.units[ti].state.pos = pos;
                self.cancel_cast(ti, None, 0);
                self.events.push(Event::Teleported {
                    unit: Self::id_of(ti),
                    pos,
                });
            }
            (
                Effect::Summon {
                    unit,
                    count,
                    max_alive,
                },
                _,
            ) => self.summon(ci, unit, *count, *max_alive),
            (
                Effect::PairUp {
                    aura,
                    duration_ms,
                    max_m,
                    base,
                },
                _,
            ) => {
                let amount = self.power(ci, *base, 0).min(u64::from(u32::MAX)) as u32;
                self.raid_pair_up(
                    ci,
                    spell.id,
                    spell.range_m,
                    aura,
                    *duration_ms,
                    *max_m,
                    amount,
                );
            }
            (Effect::Detonate { boost_aura }, _) => {
                if let (Some(key), Some(si)) = (
                    boost_aura,
                    self.units[ci].summoner.and_then(|s| self.index(s)),
                ) && self.units[si].state.alive
                {
                    let summoner = Self::id_of(si);
                    self.apply_aura(si, key, 600_000, None, summoner, Some(spell.id));
                }
                if self.units[ci].state.alive {
                    self.die(ci, None);
                }
            }
            (Effect::ResetAllyThreat { radius_m }, _) => {
                let side = self.units[ci].state.hostile;
                let center = self.units[ci].state.pos;
                for u in &mut self.units {
                    if u.state.alive
                        && u.state.hostile == side
                        && dist_xz(u.state.pos, center) <= *radius_m
                    {
                        u.threat.values_mut().for_each(|t| *t = 1);
                    }
                }
            }
            (Effect::ThreatSwap, _) => {
                let mut order: Vec<(EntityId, u32)> = self.units[ci]
                    .threat
                    .iter()
                    .map(|(k, v)| (*k, *v))
                    .collect();
                order.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                if let [(a, ta), (b, tb), ..] = order[..] {
                    self.units[ci].threat.insert(a, tb);
                    self.units[ci].threat.insert(b, ta);
                }
            }
            (Effect::ForwardLastSpell { pct }, Some(ti)) => {
                let Some((amount, school)) = self.units[ci].last_player_hit else {
                    return;
                };
                if !self.rng.chance_pct(self.content.config.spell_hit_pct) {
                    self.events.push(Event::Miss {
                        source: caster,
                        target: Self::id_of(ti),
                        spell: Some(spell.id),
                    });
                    return;
                }
                let hit = Hit {
                    spell: Some(spell.id),
                    amount: (u64::from(amount) * u64::from(*pct) / 100) as u32,
                    crit: false,
                    combo: false,
                    school,
                    dot: false,
                };
                self.deal_damage(caster, Self::id_of(ti), hit);
            }
            (Effect::Resurrect { hp_pct, mana_pct }, Some(ti)) => {
                self.resurrect(ti, caster, *hp_pct, *mana_pct);
            }
            (Effect::Taunt, Some(ti)) => self.taunt(ti, caster),
            (Effect::Threat { amount }, Some(ti)) => {
                let t = &self.units[ti];
                if t.state.hostile && self.content.units[t.def].ai.is_some() {
                    self.add_threat(ti, caster, *amount);
                }
            }
            (Effect::Charge { stop_m }, Some(ti)) => {
                let (from, to) = (self.units[ci].state.pos, self.units[ti].state.pos);
                let d = dist_xz(from, to);
                if d > *stop_m {
                    let k = (d - stop_m) / d;
                    let pos = Vec3::new(
                        from.x + (to.x - from.x) * k,
                        from.y,
                        from.z + (to.z - from.z) * k,
                    );
                    self.units[ci].state.pos = pos;
                    self.events.push(Event::Teleported { unit: caster, pos });
                }
            }
            (Effect::RestoreHp { ms_ago }, Some(ti)) => {
                let k = (*ms_ago / 1000) as usize;
                let u = &self.units[ti];
                let then = if u.class.is_some() {
                    u.hp_hist[k.min(crate::sim::HP_HIST_LEN - 1)]
                } else {
                    0
                };
                let amount = then.saturating_sub(u.state.hp);
                self.heal(caster, ti, Some(spell.id), amount);
            }
            (Effect::DrainMana { pct }, Some(ti)) => {
                let t = &mut self.units[ti].state;
                let drain = (u64::from(t.max_mana) * u64::from(*pct) / 100) as u32;
                t.mana = t.mana.saturating_sub(drain);
            }
            _ => {}
        }
    }

    /// Revives a dead player where it fell, keeping its cooldowns (unlike a graveyard release).
    fn resurrect(&mut self, ti: usize, by: EntityId, hp_pct: u32, mana_pct: u32) {
        let unit = Self::id_of(ti);
        if self.units[ti].state.alive || !self.is_player(unit) {
            return;
        }
        let regen_next = self.now + ticks(self.content.config.regen.interval_ms);
        let u = &mut self.units[ti];
        let pct = |max: u32, p: u32| (u64::from(max) * u64::from(p) / 100) as u32;
        u.state.alive = true;
        u.state.hp = pct(u.state.max_hp, hp_pct).max(1);
        u.state.mana = pct(u.state.max_mana, mana_pct);
        u.moving = false;
        u.regen_next = regen_next;
        let (hp, mana) = (u.state.hp, u.state.mana);
        self.events.push(Event::Resurrected { unit, by, hp, mana });
    }

    /// Who Touched Prod?: 10% over the current top threat (enough to take aggro at once).
    fn taunt(&mut self, ti: usize, caster: EntityId) {
        let t = &self.units[ti];
        if !t.state.hostile || self.content.units[t.def].ai.is_none() {
            return;
        }
        let top = t.threat.values().copied().max().unwrap_or(0);
        let mine = t.threat.get(&caster).copied().unwrap_or(0);
        let want = top + top / 10 + 1;
        if want > mine {
            // Raw entry, not `add_threat`: the class multiplier must not inflate a taunt.
            if t.threat.is_empty() {
                self.add_threat(ti, caster, 1);
            }
            self.units[ti].threat.insert(caster, want);
            self.units[ti].state.target = Some(caster);
            if self.units[ti].state.ai == Some(AiState::Idle) {
                self.set_ai(ti, AiState::Chase);
            }
        }
    }

    /// Heals generate no threat; the event reports the effective amount (overheal dropped).
    pub(crate) fn heal(
        &mut self,
        source: EntityId,
        ti: usize,
        spell: Option<SpellId>,
        amount: u32,
    ) {
        let t = &mut self.units[ti].state;
        if !t.alive {
            return;
        }
        let amount = amount.min(t.max_hp - t.hp);
        t.hp += amount;
        self.events.push(Event::Heal {
            source,
            target: Self::id_of(ti),
            spell,
            amount,
        });
    }

    fn summon(&mut self, ci: usize, key: &str, count: u8, max_alive: u8) {
        let Ok(def) = self.def_index(key) else { return };
        let caster = Self::id_of(ci);
        let alive = self
            .units
            .iter()
            .filter(|u| u.summoner == Some(caster) && u.state.alive && u.def == def)
            .count();
        let n = usize::from(count).min(usize::from(max_alive).saturating_sub(alive));
        let (pos, yaw, level, pack) = {
            let c = &self.units[ci];
            (c.state.pos, c.state.yaw, c.state.level, c.state.pack)
        };
        let target = self.units[ci].state.target;
        // Offset ring index 0 is the centre, which the summoner occupies.
        let offsets = crate::floors::pack_offsets(n + 1);
        for (dx, dz) in offsets.into_iter().skip(1) {
            let at = Vec3::new(pos.x + dx, pos.y, pos.z + dz);
            let id = self.spawn(
                def,
                at,
                yaw,
                SpawnOpts {
                    level: Some(level),
                    pack,
                    respawns: false,
                    summoner: Some(caster),
                    floor_boss: false,
                    zone_slot: None,
                },
            );
            self.events.push(Event::Summoned {
                unit: id,
                summoner: caster,
            });
            if let (Some(t), Some(si)) = (target, self.index(id))
                && self.content.units[def].ai.is_some()
            {
                self.add_threat(si, t, 1);
                self.set_ai(si, AiState::Chase);
            }
        }
    }

    /// Refreshes duration; stacking auras gain a stack up to `max_stacks`.
    /// Periodic and expiry amounts are snapshotted from the caster now.
    pub(crate) fn apply_aura(
        &mut self,
        ti: usize,
        key: &str,
        duration_ms: u32,
        absorb: Option<u32>,
        source: EntityId,
        spell: Option<SpellId>,
    ) {
        let Some(def) = self.content.aura(key).cloned() else {
            return;
        };
        if def.stun && self.units[ti].state.boss {
            return;
        }
        // Outside a duel players never debuff each other (a cast that outlived the duel).
        let si = self.index(source);
        let pvp = si.is_some_and(|s| s != ti && self.is_player(source))
            && self.is_player(Self::id_of(ti));
        if def.harmful && pvp && !si.is_some_and(|s| self.duel_foes(s, ti)) {
            return;
        }
        let duration_ms = if pvp && (def.stun || def.silence) {
            match self.duel_cc_ms(ti, duration_ms) {
                0 => return,
                ms => ms,
            }
        } else {
            duration_ms
        };
        let old = self.units[ti].auras.iter().position(|a| a.key == key);
        let stacks = old.map_or(1, |k| {
            (self.units[ti].auras[k].stacks + 1).min(def.max_stacks)
        });
        if let Some(k) = old {
            self.units[ti].auras.remove(k);
        }
        let (sp, dmg_pct) = si.map_or((0, 100), |s| (self.units[s].sp, self.units[s].dmg_pct));
        let talent_pct = 100 + si.map_or(0, |s| self.perk_periodic_pct(s, spell));
        let tick_amount = def.periodic.as_ref().map_or(0, |p| {
            ((u64::from(p.base) + u64::from(sp) * u64::from(p.coefficient_pct) / 100)
                * u64::from(dmg_pct)
                / 100
                * talent_pct
                / 100) as u32
        });
        let interval = def.periodic.as_ref().map_or(0, |p| ticks(p.interval_ms));
        self.units[ti].auras.push(AuraRt {
            key: key.to_string(),
            total_ms: duration_ms,
            expires: self.now + ticks(duration_ms),
            absorb,
            stacks,
            source,
            spell,
            tick_amount,
            next_tick: self.now + interval,
            expire_amount: (u64::from(def.expire_damage) * u64::from(dmg_pct) / 100) as u32,
        });
        self.events.push(Event::AuraApplied {
            target: Self::id_of(ti),
            key: key.to_string(),
        });
        if def.harmful {
            self.burnout_on_aura(ti, key);
        }
        if def.max_hp_pct != 0 {
            self.refresh_max_hp(ti);
        }
        // A debuff pulls like a hit does (a DoT must not wait for its first tick to aggro).
        let pulls = def.harmful
            && self.content.units[self.units[ti].def].ai.is_some()
            && self.units[ti].state.hostile != si.is_some_and(|s| self.units[s].state.hostile)
            && !self.units[ti].threat.contains_key(&source);
        if pulls {
            self.add_threat(ti, source, 1);
        }
        if def.stun || def.silence {
            self.units[ti].queued = None;
            self.cancel_cast(ti, Some(source), 0);
        }
    }

    pub(crate) fn remove_aura(&mut self, ti: usize, key: &str, reason: AuraEnd) -> bool {
        let before = self.units[ti].auras.len();
        self.units[ti].auras.retain(|a| a.key != key);
        let removed = self.units[ti].auras.len() != before;
        if removed {
            self.events.push(Event::AuraRemoved {
                target: Self::id_of(ti),
                key: key.to_string(),
                reason,
            });
            if self.content.aura(key).is_some_and(|d| d.max_hp_pct != 0) {
                self.refresh_max_hp(ti);
            }
        }
        removed
    }

    /// Periodic ticks first (a tick due on the expiry tick still lands), then expiry.
    pub(crate) fn update_auras(&mut self) {
        for ti in 0..self.units.len() {
            if !self.units[ti].state.alive {
                continue;
            }
            let due: Vec<String> = self.units[ti]
                .auras
                .iter()
                .filter(|a| a.tick_amount > 0 && a.next_tick <= self.now && a.next_tick > 0)
                .map(|a| a.key.clone())
                .collect();
            for key in due {
                self.periodic_tick(ti, &key);
            }
            let expired: Vec<(String, EntityId, Option<SpellId>, u32)> = self.units[ti]
                .auras
                .iter()
                .filter(|a| a.expires <= self.now)
                .map(|a| (a.key.clone(), a.source, a.spell, a.expire_amount))
                .collect();
            for (key, source, spell, boom) in expired {
                if !self.remove_aura(ti, &key, AuraEnd::Expired) || boom == 0 {
                    continue;
                }
                let school = self.content.aura(&key).map_or(School::Arcane, |d| d.school);
                let hit = Hit {
                    spell,
                    amount: boom,
                    crit: false,
                    combo: false,
                    school,
                    dot: true,
                };
                self.deal_damage(source, Self::id_of(ti), hit);
            }
        }
    }

    fn periodic_tick(&mut self, ti: usize, key: &str) {
        let Some(def) = self.content.aura(key).cloned() else {
            return;
        };
        let Some(p) = def.periodic else { return };
        if !self.units[ti].state.alive {
            return;
        }
        let Some(a) = self.units[ti].auras.iter_mut().find(|a| a.key == key) else {
            return;
        };
        a.next_tick += ticks(p.interval_ms);
        let amount = a.tick_amount * u32::from(a.stacks);
        let (source, spell) = (a.source, a.spell);
        if p.heal {
            self.heal(source, ti, spell, amount);
        } else {
            let hit = Hit {
                spell,
                amount,
                crit: false,
                combo: false,
                school: def.school,
                dot: true,
            };
            self.deal_damage(source, Self::id_of(ti), hit);
        }
    }

    /// git blame: a dying bearer passes a fresh copy to the nearest enemy of the caster in range.
    pub(crate) fn spread_auras(&mut self, ti: usize) {
        let spreads: Vec<(String, EntityId, Option<SpellId>, u32, f32)> = self.units[ti]
            .auras
            .iter()
            .filter_map(|a| {
                let r = self.content.aura(&a.key)?.spread_radius_m;
                (r > 0.0).then(|| (a.key.clone(), a.source, a.spell, a.total_ms, r))
            })
            .collect();
        let at = self.units[ti].state.pos;
        for (key, source, spell, total_ms, radius) in spreads {
            let Some(src_side) = self.index(source).map(|s| self.units[s].state.hostile) else {
                continue;
            };
            let next = (0..self.units.len())
                .filter(|&o| {
                    let u = &self.units[o];
                    o != ti
                        && u.state.alive
                        && !u.despawned
                        && u.state.hostile != src_side
                        && u.state.ai != Some(AiState::Evade)
                        && !u.auras.iter().any(|a| a.key == key)
                        && dist_xz(u.state.pos, at) <= radius
                })
                .min_by(|&a, &b| {
                    distance(self.units[a].state.pos, at)
                        .total_cmp(&distance(self.units[b].state.pos, at))
                        .then(a.cmp(&b))
                });
            if let Some(o) = next {
                self.apply_aura(o, &key, total_ms, None, source, spell);
            }
        }
    }

    /// Unread Spec counts repeats; MeatProxy needs nothing here (it records hits instead).
    pub(crate) fn note_cast(&mut self, ci: usize, spell: SpellId) {
        let repeat = self.units[ci].last_cast == Some(spell);
        self.units[ci].last_cast = Some(spell);
        if !repeat {
            return;
        }
        let stackers: Vec<String> = self.units[ci]
            .auras
            .iter()
            .filter(|a| {
                self.content
                    .aura(&a.key)
                    .is_some_and(|d| d.stack_on_repeat_cast)
            })
            .map(|a| a.key.clone())
            .collect();
        for key in stackers {
            let max = self.content.aura(&key).map_or(1, |d| d.max_stacks);
            if let Some(a) = self.units[ci].auras.iter_mut().find(|a| a.key == key)
                && a.stacks < max
            {
                a.stacks += 1;
                self.events.push(Event::AuraApplied {
                    target: Self::id_of(ci),
                    key,
                });
            }
        }
    }
}
