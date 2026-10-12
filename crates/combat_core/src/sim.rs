//! The deterministic combat simulation: commands in, events out, one 1/30 s tick at a time.
//!
//! All timers are integer ticks derived from content milliseconds; nothing
//! here reads a wall clock or frame time. Unit ids are `index + 1`.

use crate::content::{AuraDef, Content, ContentError, Effect, School, SpellDef, SpellId, ticks};
use crate::geom::{Aabb, Vec3, in_range};
use crate::rng::Rng;
use crate::targeting::{EntityId, TargetCandidate, validate_target};
use crate::time::{Tick, ticks_to_ms};
use crate::types::*;
use std::collections::BTreeMap;

/// A cast in progress. `end` moves later when pushback hits.
pub(crate) struct ActiveCast {
    pub spell: SpellId,
    pub target: Option<EntityId>,
    pub start: u64,
    pub end: u64,
    pub pushbacks: u8,
    pub interruptible: bool,
}

pub(crate) struct AuraRt {
    pub key: String,
    pub total_ms: u32,
    pub expires: u64,
    pub absorb: Option<u32>,
}

pub(crate) struct Unit {
    pub state: UnitState,
    /// Index into `Content::units`.
    pub def: usize,
    pub spawn_pos: Vec3,
    pub spawn_yaw: f32,
    pub gcd_end: u64,
    pub gcd_total_ms: u32,
    pub cooldowns: BTreeMap<SpellId, (u64, u32)>,
    /// Per-school lockout end tick, indexed by `School::index`.
    pub lockout_until: [u64; 2],
    pub cast: Option<ActiveCast>,
    pub queued: Option<SpellId>,
    /// Moving or jumping: either blocks cast-time spells.
    pub moving: bool,
    /// BTreeMap keeps tie-breaks (lowest id wins) deterministic.
    pub threat: BTreeMap<EntityId, u32>,
    pub melee_next: u64,
    pub ability_next: u64,
    pub auras: Vec<AuraRt>,
    /// Mana regen remainder, in 1/150 mana (5 s = 150 ticks).
    pub regen_acc: u32,
    pub respawn_at: Option<u64>,
    pub loot: Vec<String>,
}

/// One resolved damage instance, before absorbs.
pub(crate) struct Hit {
    pub spell: Option<SpellId>,
    pub amount: u32,
    pub crit: bool,
    pub combo: bool,
    pub school: School,
}

pub struct Sim {
    pub(crate) content: Content,
    pub(crate) rng: Rng,
    pub(crate) now: u64,
    pub(crate) units: Vec<Unit>,
    pub(crate) blockers: Vec<Aabb>,
    pub(crate) pending: Vec<Command>,
    pub(crate) events: Vec<Event>,
    pub(crate) player: Option<EntityId>,
}

impl Sim {
    pub fn new(content: Content, seed: u64) -> Sim {
        Sim {
            content,
            rng: Rng::new(seed),
            now: 0,
            units: Vec::new(),
            blockers: Vec::new(),
            pending: Vec::new(),
            events: Vec::new(),
            player: None,
        }
    }

    pub fn set_blockers(&mut self, blockers: Vec<Aabb>) {
        self.blockers = blockers;
    }

    pub fn spawn_player(&mut self, pos: Vec3, yaw: f32) -> EntityId {
        let key = self.content.config.player_unit.clone();
        // Content::validate guarantees the player unit exists.
        let id = self.spawn_unit(&key, pos, yaw).unwrap_or(0);
        self.player = Some(id);
        id
    }

    pub fn spawn_unit(
        &mut self,
        unit_key: &str,
        pos: Vec3,
        yaw: f32,
    ) -> Result<EntityId, ContentError> {
        let def = self
            .content
            .units
            .iter()
            .position(|u| u.key == unit_key)
            .ok_or_else(|| ContentError::UnknownUnit(unit_key.to_string()))?;
        let d = &self.content.units[def];
        let id = self.units.len() as EntityId + 1;
        let state = UnitState {
            id,
            key: d.key.clone(),
            name: d.name.clone(),
            level: d.level,
            hostile: d.hostile,
            pos,
            yaw,
            hp: d.max_hp,
            max_hp: d.max_hp,
            mana: d.max_mana,
            max_mana: d.max_mana,
            alive: true,
            in_combat: false,
            target: None,
            cast: None,
            auras: Vec::new(),
            ai: d.ai.as_ref().map(|_| AiState::Idle),
            lootable: false,
            respawn_in_ms: None,
            xp: if d.hostile { d.xp_reward } else { 0 },
        };
        self.units.push(Unit {
            state,
            def,
            spawn_pos: pos,
            spawn_yaw: yaw,
            gcd_end: 0,
            gcd_total_ms: 0,
            cooldowns: BTreeMap::new(),
            lockout_until: [0; 2],
            cast: None,
            queued: None,
            moving: false,
            threat: BTreeMap::new(),
            melee_next: 0,
            ability_next: 0,
            auras: Vec::new(),
            regen_acc: 0,
            respawn_at: None,
            loot: Vec::new(),
        });
        Ok(id)
    }

    /// Applied at the start of the next tick.
    pub fn push(&mut self, cmd: Command) {
        self.pending.push(cmd);
    }

    pub fn now(&self) -> Tick {
        Tick(self.now)
    }

    pub fn player(&self) -> Option<EntityId> {
        self.player
    }

    pub fn unit(&self, id: EntityId) -> Option<&UnitState> {
        self.index(id).map(|i| &self.units[i].state)
    }

    pub fn units(&self) -> impl Iterator<Item = &UnitState> {
        self.units.iter().map(|u| &u.state)
    }

    pub fn content(&self) -> &Content {
        &self.content
    }

    pub fn cooldown(&self, caster: EntityId, spell: SpellId) -> CooldownView {
        let (Some(ci), Some(def)) = (self.index(caster), self.content.spell(spell)) else {
            return CooldownView {
                gcd_remaining_ms: 0,
                gcd_total_ms: 0,
                cd_remaining_ms: 0,
                cd_total_ms: 0,
                usable: Err(CastError::InvalidTarget),
            };
        };
        let u = &self.units[ci];
        let (cd_end, cd_total) = u.cooldowns.get(&spell).copied().unwrap_or((0, 0));
        let (gcd_remaining_ms, gcd_total_ms) = if def.gcd {
            (self.remaining_ms(u.gcd_end), u.gcd_total_ms)
        } else {
            (0, 0)
        };
        let usable = self
            .validate(ci, def)
            .and_then(|_| self.busy(ci, def).map_or(Ok(()), |(e, _)| Err(e)));
        CooldownView {
            gcd_remaining_ms,
            gcd_total_ms,
            cd_remaining_ms: self.remaining_ms(cd_end),
            cd_total_ms: cd_total,
            usable,
        }
    }

    /// Advance exactly one 1/30 s tick.
    pub fn tick(&mut self) -> Vec<Event> {
        self.now += 1;
        let cmds = std::mem::take(&mut self.pending);
        for c in cmds {
            self.apply(c);
        }
        self.update_casts();
        self.run_queue();
        self.update_ai();
        self.update_auras();
        self.update_regen();
        self.update_combat_flags();
        self.sync_views();
        std::mem::take(&mut self.events)
    }

    // ---- helpers ----

    pub(crate) fn index(&self, id: EntityId) -> Option<usize> {
        let i = usize::try_from(id.checked_sub(1)?).ok()?;
        (i < self.units.len()).then_some(i)
    }

    pub(crate) fn id_of(i: usize) -> EntityId {
        i as EntityId + 1
    }

    pub(crate) fn remaining_ms(&self, end: u64) -> u32 {
        ticks_to_ms(end.saturating_sub(self.now)) as u32
    }

    pub(crate) fn haste(&self, i: usize) -> u32 {
        self.content.units[self.units[i].def].haste_pct
    }

    fn candidate(&self, i: usize) -> TargetCandidate {
        let s = &self.units[i].state;
        TargetCandidate {
            id: s.id,
            pos: s.pos,
            hostile: s.hostile,
            alive: s.alive,
        }
    }

    fn fail(&mut self, caster: EntityId, spell: SpellId, reason: CastError) {
        self.events.push(Event::CastFailed {
            caster,
            spell,
            reason,
        });
    }

    // ---- commands ----

    fn apply(&mut self, cmd: Command) {
        match cmd {
            Command::PlayerMoved {
                pos,
                yaw,
                moving,
                jumping,
            } => self.on_player_moved(pos, yaw, moving || jumping),
            Command::SetTarget { unit, target } => {
                let valid = target.is_none_or(|t| self.index(t).is_some());
                if let (Some(i), true) = (self.index(unit), valid) {
                    self.units[i].state.target = target;
                }
            }
            Command::Cast { caster, spell } => self.handle_cast(caster, spell),
            Command::StopCasting { caster } => {
                if let Some(i) = self.index(caster) {
                    self.units[i].queued = None;
                    self.cancel_cast(i, None, 0);
                }
            }
            Command::Loot { looter, corpse } => self.handle_loot(looter, corpse),
            Command::RespawnPlayer => self.respawn_player(),
        }
    }

    fn on_player_moved(&mut self, pos: Vec3, yaw: f32, moving: bool) {
        let Some(i) = self.player.and_then(|p| self.index(p)) else {
            return;
        };
        if !self.units[i].state.alive {
            return;
        }
        let u = &mut self.units[i];
        u.state.pos = pos;
        u.state.yaw = yaw;
        u.moving = moving;
        if moving {
            self.cancel_cast(i, None, 0);
        }
    }

    fn handle_loot(&mut self, looter: EntityId, corpse: EntityId) {
        let (Some(li), Some(ci)) = (self.index(looter), self.index(corpse)) else {
            return;
        };
        let near = in_range(
            self.units[li].state.pos,
            self.units[ci].state.pos,
            self.content.config.loot_range_m,
        );
        if !self.units[li].state.alive || !self.units[ci].state.lootable || !near {
            return;
        }
        let items = std::mem::take(&mut self.units[ci].loot);
        self.units[ci].state.lootable = false;
        self.events.push(Event::Looted {
            looter,
            corpse,
            items,
        });
    }

    fn respawn_player(&mut self) {
        let Some(i) = self.player.and_then(|p| self.index(p)) else {
            return;
        };
        if self.units[i].state.alive {
            return;
        }
        let u = &mut self.units[i];
        u.state.alive = true;
        u.state.hp = u.state.max_hp;
        u.state.mana = u.state.max_mana;
        u.state.pos = u.spawn_pos;
        u.state.yaw = u.spawn_yaw;
        u.moving = false;
        u.gcd_end = 0;
        u.cooldowns.clear();
        u.lockout_until = [0; 2];
        self.events.push(Event::Respawned {
            unit: Self::id_of(i),
        });
    }

    // ---- casting ----

    /// Static legality: everything except GCD/cast-busy (which may be queued).
    pub(crate) fn validate(
        &self,
        ci: usize,
        spell: &SpellDef,
    ) -> Result<Option<EntityId>, CastError> {
        let u = &self.units[ci];
        if !u.state.alive {
            return Err(CastError::Dead);
        }
        if self.now < u.lockout_until[spell.school.index()] {
            return Err(CastError::SchoolLocked);
        }
        if u.cooldowns
            .get(&spell.id)
            .is_some_and(|(end, _)| *end > self.now)
        {
            return Err(CastError::OnCooldown);
        }
        let target = if spell.targets_enemy() {
            let tid = u.state.target.ok_or(CastError::NoTarget)?;
            let ti = self.index(tid).ok_or(CastError::InvalidTarget)?;
            if !self.units[ti].state.hostile || self.units[ti].state.ai == Some(AiState::Evade) {
                return Err(CastError::InvalidTarget);
            }
            validate_target(
                u.state.pos,
                u.state.yaw,
                Some(&self.candidate(ti)),
                spell.range_m,
                true,
                &self.blockers,
            )?;
            if matches!(spell.effect, Effect::Interrupt { .. }) {
                match &self.units[ti].cast {
                    None => return Err(CastError::NothingToInterrupt),
                    Some(c) if !c.interruptible => return Err(CastError::NotInterruptible),
                    Some(_) => {}
                }
            }
            Some(tid)
        } else {
            Some(u.state.id)
        };
        if spell.cast_ms > 0 && u.moving {
            return Err(CastError::Moving);
        }
        if u.state.mana < spell.mana {
            return Err(CastError::NotEnoughMana);
        }
        Ok(target)
    }

    /// GCD or cast in progress: the error plus remaining ticks (for queueing).
    pub(crate) fn busy(&self, ci: usize, spell: &SpellDef) -> Option<(CastError, u64)> {
        // Off-GCD instants (Reject PR) are usable mid-cast and mid-GCD.
        if !spell.gcd && spell.cast_ms == 0 {
            return None;
        }
        let u = &self.units[ci];
        let gcd_rem = u.gcd_end.saturating_sub(self.now);
        match &u.cast {
            Some(c) => Some((
                CastError::AlreadyCasting,
                c.end.saturating_sub(self.now).max(gcd_rem),
            )),
            None if gcd_rem > 0 => Some((CastError::OnGcd, gcd_rem)),
            None => None,
        }
    }

    fn handle_cast(&mut self, caster: EntityId, spell_id: SpellId) {
        let Some(ci) = self.index(caster) else { return };
        let Some(spell) = self.content.spell(spell_id).cloned() else {
            return;
        };
        let target = match self.validate(ci, &spell) {
            Ok(t) => t,
            Err(reason) => return self.fail(caster, spell_id, reason),
        };
        match self.busy(ci, &spell) {
            None => self.start_cast(ci, &spell, target),
            Some((_, remaining)) if remaining <= ticks(self.content.config.queue_window_ms) => {
                // A newer press replaces an older queued one, as in WoW.
                self.units[ci].queued = Some(spell_id);
                self.events.push(Event::CastQueued {
                    caster,
                    spell: spell_id,
                });
            }
            Some((reason, _)) => self.fail(caster, spell_id, reason),
        }
    }

    /// Starts a queued cast on the first tick nothing is busy; re-validates then.
    fn run_queue(&mut self) {
        for ci in 0..self.units.len() {
            let Some(spell_id) = self.units[ci].queued else {
                continue;
            };
            let Some(spell) = self.content.spell(spell_id).cloned() else {
                self.units[ci].queued = None;
                continue;
            };
            if self.busy(ci, &spell).is_some() {
                continue;
            }
            self.units[ci].queued = None;
            match self.validate(ci, &spell) {
                Ok(target) => self.start_cast(ci, &spell, target),
                Err(reason) => self.fail(Self::id_of(ci), spell_id, reason),
            }
        }
    }

    pub(crate) fn start_cast(&mut self, ci: usize, spell: &SpellDef, target: Option<EntityId>) {
        let haste = self.haste(ci);
        let id = Self::id_of(ci);
        if spell.gcd {
            let gcd = self.content.gcd_ms(haste);
            let u = &mut self.units[ci];
            u.gcd_end = self.now + ticks(gcd);
            u.gcd_total_ms = gcd;
        }
        let cast_ms = self.content.cast_ms(spell, haste);
        if cast_ms == 0 {
            return self.resolve(ci, spell, target);
        }
        let end = self.now + ticks(cast_ms);
        self.units[ci].cast = Some(ActiveCast {
            spell: spell.id,
            target,
            start: self.now,
            end,
            pushbacks: 0,
            interruptible: spell.interruptible,
        });
        self.events.push(Event::CastStarted {
            caster: id,
            spell: spell.id,
            total_ms: ticks_to_ms(end - self.now) as u32,
        });
    }

    fn update_casts(&mut self) {
        for ci in 0..self.units.len() {
            let due = self.units[ci]
                .cast
                .as_ref()
                .is_some_and(|c| c.end <= self.now);
            if due {
                self.complete_cast(ci);
            }
        }
    }

    fn complete_cast(&mut self, ci: usize) {
        let Some(cast) = self.units[ci].cast.take() else {
            return;
        };
        let Some(spell) = self.content.spell(cast.spell).cloned() else {
            return;
        };
        if let Err(reason) = self.revalidate(ci, &spell, cast.target) {
            return self.fail(Self::id_of(ci), spell.id, reason);
        }
        self.resolve(ci, &spell, cast.target);
    }

    /// Completion-time checks: target alive, range and LoS (no facing, as in
    /// WotLK), and mana, which is only spent now. Hostility is not rechecked
    /// because enemies also cast "hostile" spells at the player.
    fn revalidate(
        &self,
        ci: usize,
        spell: &SpellDef,
        target: Option<EntityId>,
    ) -> Result<(), CastError> {
        if spell.targets_enemy() {
            let ti = target
                .and_then(|t| self.index(t))
                .ok_or(CastError::InvalidTarget)?;
            // Evading units are untargetable; checked here so no mana is spent on a dropped hit.
            if self.units[ti].state.ai == Some(AiState::Evade) {
                return Err(CastError::InvalidTarget);
            }
            let u = &self.units[ci].state;
            validate_target(
                u.pos,
                u.yaw,
                Some(&self.candidate(ti)),
                spell.range_m,
                false,
                &self.blockers,
            )?;
        }
        if self.units[ci].state.mana < spell.mana {
            return Err(CastError::NotEnoughMana);
        }
        Ok(())
    }

    /// Spend mana, start the spell cooldown, report success, apply the effect.
    fn resolve(&mut self, ci: usize, spell: &SpellDef, target: Option<EntityId>) {
        let caster = Self::id_of(ci);
        let u = &mut self.units[ci];
        u.state.mana -= spell.mana;
        if spell.cooldown_ms > 0 {
            u.cooldowns.insert(
                spell.id,
                (self.now + ticks(spell.cooldown_ms), spell.cooldown_ms),
            );
        }
        self.events.push(Event::CastSucceeded {
            caster,
            spell: spell.id,
            target,
        });
        match &spell.effect {
            Effect::Damage {
                base,
                coefficient_pct,
                apply_aura,
                consume_aura,
            } => {
                let Some(ti) = target.and_then(|t| self.index(t)) else {
                    return;
                };
                if self.units[ti].state.ai == Some(AiState::Evade) {
                    return;
                }
                let sp = self.content.units[self.units[ci].def].spell_power;
                let crit_pct = self.content.units[self.units[ci].def].crit_pct;
                let mut amount =
                    u64::from(*base) + u64::from(sp) * u64::from(*coefficient_pct) / 100;
                let mut combo = false;
                if let Some(c) = consume_aura
                    && self.remove_aura(ti, &c.aura, AuraEnd::Consumed)
                {
                    amount = amount * (100 + u64::from(c.bonus_pct)) / 100;
                    combo = true;
                }
                // Rolled for every hit so the RNG stream does not depend on crit chance.
                let crit = self.rng.chance_pct(crit_pct);
                if crit {
                    amount = amount * u64::from(self.content.config.crit_damage_pct) / 100;
                }
                let hit = Hit {
                    spell: Some(spell.id),
                    amount: amount as u32,
                    crit,
                    combo,
                    school: spell.school,
                };
                self.deal_damage(caster, Self::id_of(ti), hit);
                if let (Some(a), true) = (apply_aura, self.units[ti].state.alive) {
                    self.apply_aura(ti, &a.aura, a.duration_ms, None);
                }
            }
            Effect::Absorb {
                aura,
                amount,
                duration_ms,
            } => self.apply_aura(ci, aura, *duration_ms, Some(*amount)),
            Effect::Interrupt { lockout_ms } => {
                if let Some(ti) = target.and_then(|t| self.index(t)) {
                    self.cancel_cast(ti, Some(caster), *lockout_ms);
                }
            }
        }
    }

    /// Cancels the active cast; `by: Some` also applies a school lockout.
    pub(crate) fn cancel_cast(&mut self, ci: usize, by: Option<EntityId>, lockout_ms: u32) {
        let Some(cast) = self.units[ci].cast.take() else {
            return;
        };
        if let Some(spell) = self.content.spell(cast.spell)
            && lockout_ms > 0
        {
            self.units[ci].lockout_until[spell.school.index()] = self.now + ticks(lockout_ms);
        }
        self.events.push(Event::CastInterrupted {
            caster: Self::id_of(ci),
            spell: cast.spell,
            by,
            lockout_ms,
        });
    }

    // ---- damage, auras, death ----

    pub(crate) fn deal_damage(&mut self, source: EntityId, target: EntityId, hit: Hit) {
        let Hit {
            spell,
            amount,
            crit,
            combo,
            school,
        } = hit;
        let Some(ti) = self.index(target) else { return };
        let t = &self.units[ti];
        // Evading units ignore damage so they cannot be kited back into combat.
        if !t.state.alive || t.state.ai == Some(AiState::Evade) {
            return;
        }
        let mut absorbed = 0;
        let mut depleted: Vec<String> = Vec::new();
        for a in &mut self.units[ti].auras {
            if let Some(left) = a.absorb.as_mut() {
                let take = (*left).min(amount - absorbed);
                *left -= take;
                absorbed += take;
                if *left == 0 {
                    depleted.push(a.key.clone());
                }
                if absorbed == amount {
                    break;
                }
            }
        }
        let dealt = amount - absorbed;
        self.events.push(Event::Damage {
            source,
            target,
            spell,
            amount: dealt,
            absorbed,
            crit,
            combo,
            school,
        });
        for key in depleted {
            self.remove_aura(ti, &key, AuraEnd::Depleted);
        }
        let hp = self.units[ti].state.hp.saturating_sub(dealt);
        self.units[ti].state.hp = hp;
        if self.content.units[self.units[ti].def].ai.is_some() {
            // Threat counts the full hit, absorbed or not.
            self.add_threat(ti, source, amount.max(1));
        } else if dealt > 0 {
            self.pushback(ti);
        }
        if hp == 0 {
            self.die(ti, Some(source));
        }
    }

    /// Pushback is player-only: enemy telegraphs must keep a fixed 2.5 s window.
    fn pushback(&mut self, ti: usize) {
        let (ms, max) = (
            self.content.config.pushback_ms,
            self.content.config.pushback_max,
        );
        if let Some(c) = self.units[ti].cast.as_mut()
            && c.pushbacks < max
        {
            c.pushbacks += 1;
            c.end += ticks(ms);
        }
    }

    pub(crate) fn apply_aura(
        &mut self,
        ti: usize,
        key: &str,
        duration_ms: u32,
        absorb: Option<u32>,
    ) {
        self.units[ti].auras.retain(|a| a.key != key);
        self.units[ti].auras.push(AuraRt {
            key: key.to_string(),
            total_ms: duration_ms,
            expires: self.now + ticks(duration_ms),
            absorb,
        });
        self.events.push(Event::AuraApplied {
            target: Self::id_of(ti),
            key: key.to_string(),
        });
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
        }
        removed
    }

    fn update_auras(&mut self) {
        for ti in 0..self.units.len() {
            let expired: Vec<String> = self.units[ti]
                .auras
                .iter()
                .filter(|a| a.expires <= self.now)
                .map(|a| a.key.clone())
                .collect();
            for key in expired {
                self.remove_aura(ti, &key, AuraEnd::Expired);
            }
        }
    }

    pub(crate) fn die(&mut self, ti: usize, killer: Option<EntityId>) {
        let id = Self::id_of(ti);
        let keys: Vec<String> = self.units[ti].auras.iter().map(|a| a.key.clone()).collect();
        for k in keys {
            self.remove_aura(ti, &k, AuraEnd::Death);
        }
        self.cancel_cast(ti, None, 0);
        let u = &mut self.units[ti];
        u.state.alive = false;
        u.state.hp = 0;
        u.queued = None;
        u.moving = false;
        u.threat.clear();
        u.state.target = None;
        self.events.push(Event::Death { unit: id, killer });
        for other in &mut self.units {
            other.threat.remove(&id);
        }
        let def = &self.content.units[self.units[ti].def];
        let Some(ai) = def.ai.as_ref() else { return };
        let (respawn_ms, xp) = (ai.respawn_ms, def.xp_reward);
        let loot_table = def.loot.clone();
        self.units[ti].state.ai = Some(AiState::Dead);
        self.events.push(Event::AiState {
            unit: id,
            state: AiState::Dead,
        });
        let items: Vec<String> = loot_table
            .into_iter()
            .filter(|e| self.rng.chance_pct(e.chance_pct))
            .map(|e| e.item)
            .collect();
        self.units[ti].loot = items.clone();
        self.units[ti].state.lootable = true;
        self.units[ti].respawn_at = Some(self.now + ticks(respawn_ms));
        if let Some(k) = killer.and_then(|k| self.index(k))
            && self.units[k].state.alive
            && xp > 0
        {
            self.units[k].state.xp += xp;
            self.events.push(Event::XpGained {
                unit: Self::id_of(k),
                amount: xp,
            });
        }
        self.events.push(Event::LootDropped { corpse: id, items });
    }

    // ---- per-tick upkeep ----

    fn update_regen(&mut self) {
        for u in &mut self.units {
            let d = &self.content.units[u.def];
            if !u.state.alive || u.state.mana >= u.state.max_mana {
                u.regen_acc = 0;
                continue;
            }
            let rate = if u.state.in_combat {
                d.mana_regen_per_5s_combat
            } else {
                d.mana_regen_per_5s_ooc
            };
            u.regen_acc += rate;
            u.state.mana = (u.state.mana + u.regen_acc / 150).min(u.state.max_mana);
            u.regen_acc %= 150;
        }
    }

    fn update_combat_flags(&mut self) {
        let player = self.player;
        let player_engaged = player.is_some_and(|p| {
            self.units
                .iter()
                .any(|u| u.state.alive && u.threat.contains_key(&p))
        });
        for i in 0..self.units.len() {
            let u = &self.units[i];
            let id = u.state.id;
            let now_in = u.state.alive
                && if Some(id) == player {
                    player_engaged
                } else {
                    !u.threat.is_empty()
                };
            if now_in != u.state.in_combat {
                self.units[i].state.in_combat = now_in;
                self.events.push(if now_in {
                    Event::EnterCombat { unit: id }
                } else {
                    Event::LeaveCombat { unit: id }
                });
            }
        }
    }

    fn sync_views(&mut self) {
        let now = self.now;
        for u in &mut self.units {
            u.state.cast = u.cast.as_ref().map(|c| {
                let total_ms = ticks_to_ms(c.end - c.start) as u32;
                CastView {
                    spell: c.spell,
                    elapsed_ms: (ticks_to_ms(now.saturating_sub(c.start)) as u32).min(total_ms),
                    total_ms,
                    interruptible: c.interruptible,
                }
            });
            let defs: &[AuraDef] = &self.content.auras;
            u.state.auras = u
                .auras
                .iter()
                .filter_map(|a| {
                    let d = defs.iter().find(|d| d.key == a.key)?;
                    Some(AuraView {
                        key: a.key.clone(),
                        name: d.name.clone(),
                        icon: d.icon.clone(),
                        harmful: d.harmful,
                        remaining_ms: ticks_to_ms(a.expires.saturating_sub(now)) as u32,
                        total_ms: a.total_ms,
                        absorb_remaining: a.absorb,
                    })
                })
                .collect();
            u.state.respawn_in_ms = u
                .respawn_at
                .filter(|_| !u.state.alive)
                .map(|t| ticks_to_ms(t.saturating_sub(now)) as u32);
        }
    }
}
