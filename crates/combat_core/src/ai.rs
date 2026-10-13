//! Enemy behaviour: idle -> aggro -> chase -> attack/cast -> evade -> respawn.

use crate::content::{AiDef, School, YellKind, ticks};
use crate::geom::{Vec3, distance, line_of_sight};
use crate::sim::{Hit, Sim};
use crate::targeting::EntityId;
use crate::time::TICK_HZ;
use crate::types::{AiState, AuraEnd, Event};

/// Horizontal distance: enemies path on the ground plane only.
fn dist_xz(a: Vec3, b: Vec3) -> f32 {
    let (dx, dz) = (a.x - b.x, a.z - b.z);
    (dx * dx + dz * dz).sqrt()
}

/// Inverse of `geom::forward` (yaw 0 faces -Z).
fn yaw_toward(from: Vec3, to: Vec3) -> f32 {
    (-(to.x - from.x)).atan2(-(to.z - from.z))
}

/// Moves `pos` toward `to` by at most `step`, never closer than `stop`.
fn step_toward(pos: Vec3, to: Vec3, step: f32, stop: f32) -> Vec3 {
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
        let ability = self.content.units[self.units[ti].def]
            .ai
            .as_ref()
            .and_then(|a| a.ability.clone());
        let first = self.units[ti].threat.is_empty();
        *self.units[ti].threat.entry(source).or_insert(0) += amount;
        if let (true, Some(ab)) = (first, ability) {
            // First blast comes 8-12 s after the pull, not instantly.
            let ms = self
                .rng
                .range_inclusive(ab.cooldown_min_ms, ab.cooldown_max_ms);
            self.units[ti].ability_next = self.now + ticks(ms);
        }
    }

    pub(crate) fn update_ai(&mut self) {
        for i in 0..self.units.len() {
            if let Some(ai) = self.content.units[self.units[i].def].ai.clone() {
                self.ai_unit(i, &ai);
            }
        }
    }

    fn set_ai(&mut self, i: usize, state: AiState) {
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

    fn try_aggro(&mut self, i: usize, ai: &AiDef) {
        let Some(p) = self.player.and_then(|p| self.index(p)) else {
            return;
        };
        let (me, them) = (self.units[i].state.pos, self.units[p].state.pos);
        if self.units[p].state.alive
            && dist_xz(me, them) <= ai.aggro_m
            && line_of_sight(me, them, &self.blockers)
        {
            self.add_threat(i, Self::id_of(p), 1);
            self.set_ai(i, AiState::Chase);
        }
    }

    fn fight(&mut self, i: usize, ai: &AiDef) {
        // Highest threat wins; BTreeMap order makes ties go to the lowest id.
        let top = self.units[i]
            .threat
            .iter()
            .fold(None, |best: Option<(EntityId, u32)>, (id, t)| match best {
                Some((_, bt)) if bt >= *t => best,
                _ => Some((*id, *t)),
            })
            .map(|(id, _)| id);
        let Some(ti) = top.and_then(|t| self.index(t)) else {
            return;
        };
        let (me, them) = (self.units[i].state.pos, self.units[ti].state.pos);
        let u = &mut self.units[i];
        u.state.target = top;
        u.state.yaw = yaw_toward(me, them);

        if self.units[i].cast.is_some() {
            return self.set_ai(i, AiState::Casting);
        }
        if self.try_start_ability(i, ai, ti) {
            return self.set_ai(i, AiState::Casting);
        }
        if dist_xz(me, them) > ai.melee_range_m {
            let step = ai.chase_speed_mps / TICK_HZ as f32;
            // Stop slightly inside melee range so float jitter cannot flap Chase/Attack.
            self.units[i].state.pos = step_toward(me, them, step, ai.melee_range_m * 0.9);
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
            let hit = Hit {
                spell: None,
                amount: ai.melee_damage,
                crit: false,
                combo: false,
                school: School::Physical,
            };
            self.deal_damage(src, dst, hit);
        }
    }

    fn try_start_ability(&mut self, i: usize, ai: &AiDef, ti: usize) -> bool {
        let Some(ab) = &ai.ability else { return false };
        let Some(spell) = self.content.spell(ab.spell).cloned() else {
            return false;
        };
        let u = &self.units[i];
        let (me, them) = (u.state.pos, self.units[ti].state.pos);
        let ready = u.ability_next <= self.now
            && self.now >= u.lockout_until[spell.school.index()]
            && distance(me, them) <= spell.range_m
            && line_of_sight(me, them, &self.blockers);
        if !ready {
            return false;
        }
        let ms = self
            .rng
            .range_inclusive(ab.cooldown_min_ms, ab.cooldown_max_ms);
        self.units[i].ability_next = self.now + ticks(ms);
        self.start_cast(i, &spell, Some(Self::id_of(ti)));
        self.request_yell(i, YellKind::Blast);
        true
    }

    fn begin_evade(&mut self, i: usize) {
        self.cancel_cast(i, None, 0);
        let harmful: Vec<String> = self.units[i]
            .auras
            .iter()
            .filter(|a| self.content.aura(&a.key).is_some_and(|d| d.harmful))
            .map(|a| a.key.clone())
            .collect();
        for k in harmful {
            self.remove_aura(i, &k, AuraEnd::Expired);
        }
        let u = &mut self.units[i];
        u.threat.clear();
        u.state.target = None;
        self.set_ai(i, AiState::Evade);
        self.request_yell(i, YellKind::Evade);
    }

    fn evade_step(&mut self, i: usize, ai: &AiDef) {
        let (pos, home) = (self.units[i].state.pos, self.units[i].spawn_pos);
        let step = ai.chase_speed_mps / TICK_HZ as f32;
        if dist_xz(pos, home) <= step {
            let u = &mut self.units[i];
            u.state.pos = home;
            u.state.yaw = u.spawn_yaw;
            u.state.hp = u.state.max_hp;
            u.state.mana = u.state.max_mana;
            return self.set_ai(i, AiState::Idle);
        }
        let u = &mut self.units[i];
        u.state.yaw = yaw_toward(pos, home);
        u.state.pos = step_toward(pos, home, step, 0.0);
    }

    fn respawn_unit(&mut self, i: usize) {
        let u = &mut self.units[i];
        u.state.alive = true;
        u.state.hp = u.state.max_hp;
        u.state.mana = u.state.max_mana;
        u.state.pos = u.spawn_pos;
        u.state.yaw = u.spawn_yaw;
        u.state.lootable = false;
        u.loot.clear();
        u.respawn_at = None;
        u.melee_next = 0;
        u.cooldowns.clear();
        u.lockout_until = [0; 2];
        self.events.push(Event::Respawned {
            unit: Self::id_of(i),
        });
        self.set_ai(i, AiState::Idle);
    }
}
