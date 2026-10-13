//! Player "Nitpick" auto attack: a ranged swing timer that runs beside spell casting.

use crate::content::{School, ticks};
use crate::sim::{Hit, Sim};
use crate::targeting::{EntityId, TargetCandidate, validate_target};
use crate::types::{AiState, Event};

impl Sim {
    /// Emits `AutoAttackToggled` only when the state actually changes.
    pub(crate) fn set_auto_attack(&mut self, i: usize, on: bool) {
        if self.units[i].state.auto_attack != on {
            self.units[i].state.auto_attack = on;
            self.events.push(Event::AutoAttackToggled {
                unit: Self::id_of(i),
                on,
            });
        }
    }

    /// Silently refused (no event) without a living hostile target: there is no
    /// spell to fail, and the HUD reads `UnitState::auto_attack` instead.
    pub(crate) fn start_auto_attack(&mut self, unit: EntityId) {
        let Some(i) = self.index(unit) else { return };
        let u = &self.units[i];
        let has_def = self.content.units[u.def].auto_attack.is_some();
        let target_ok = u
            .state
            .target
            .and_then(|t| self.index(t))
            .is_some_and(|ti| self.units[ti].state.alive && self.units[ti].state.hostile);
        if u.state.alive && has_def && target_ok {
            self.set_auto_attack(i, true);
        }
    }

    pub(crate) fn update_auto_attack(&mut self) {
        for i in 0..self.units.len() {
            if self.units[i].state.auto_attack {
                self.auto_attack_unit(i);
            }
        }
    }

    fn auto_attack_unit(&mut self, i: usize) {
        let Some(def) = self.content.units[self.units[i].def].auto_attack.clone() else {
            return self.set_auto_attack(i, false);
        };
        let u = &self.units[i];
        let Some(ti) = u.state.target.and_then(|t| self.index(t)) else {
            return self.set_auto_attack(i, false);
        };
        let t = &self.units[ti].state;
        if !u.state.alive || !t.alive || !t.hostile {
            return self.set_auto_attack(i, false);
        }
        // Out of range, facing or LoS, a cast, or an evading target just waits.
        let cand = TargetCandidate {
            id: t.id,
            pos: t.pos,
            hostile: t.hostile,
            alive: t.alive,
        };
        let in_position = validate_target(
            u.state.pos,
            u.state.yaw,
            Some(&cand),
            def.range_m,
            true,
            &self.blockers,
        )
        .is_ok();
        if u.swing_next > self.now
            || u.cast.is_some()
            || self.has_aura_flag(i, |a| a.stun)
            || t.ai == Some(AiState::Evade)
            || !in_position
        {
            return;
        }
        self.units[i].swing_next = self.now + ticks(def.swing_ms);
        let (src, dst) = (Self::id_of(i), Self::id_of(ti));
        self.events.push(Event::MeleeSwing {
            attacker: src,
            target: dst,
        });
        // Roll order: hit, crit, damage. A miss generates no threat.
        if !self.rng.chance_pct(self.content.config.white_hit_pct) {
            self.events.push(Event::Miss {
                source: src,
                target: dst,
                spell: None,
            });
            return;
        }
        let crit_pct = self.content.units[self.units[i].def].crit_pct;
        let crit = self.rng.chance_pct(crit_pct);
        let mut amount = u64::from(self.rng.range_inclusive(def.damage_min, def.damage_max));
        if crit {
            amount = amount * u64::from(self.content.config.crit_damage_pct) / 100;
        }
        let hit = Hit {
            spell: None,
            amount: amount as u32,
            crit,
            combo: false,
            school: School::Physical,
        };
        self.deal_damage(src, dst, hit);
    }
}
