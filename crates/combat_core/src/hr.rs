//! Ilmari Wing 2, Human Resources (docs/ILMARI_09.md B.5): the three mechanics the generic AI and
//! the twin framework cannot express. `Effect::SummonMirror` (the HR Business Partner copies a
//! player), the Culture Committee's vote (ballot terminals, a policy aura on everyone) and
//! `Effect::Review` (Performance Review grades the last window of play). All of it runs on the
//! sim's own state and the combat rng, so a replay reproduces it exactly. Numbers and the grade
//! formula are in docs/FORMULAS.md.

use crate::content::{Role, ticks};
use crate::raid::{TwinsDef, VoteDef};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::Event;
use crate::world::PROP_OK;
use std::collections::HashMap;

/// A copy of a player: dies on its own at `until`, and takes the player's `mirrored` aura with it.
struct Mirror {
    unit: EntityId,
    player: EntityId,
    aura: String,
    until: u64,
}

/// What a player did since the last review. Damage taken only counts for tanks.
#[derive(Default, Clone, Copy)]
pub(crate) struct Perf {
    damage: u64,
    healing: u64,
    absorb: u64,
    taken: u64,
    interrupts: u32,
    fails: u32,
}

#[derive(Default)]
pub(crate) struct HrState {
    /// The loaded zone belongs to an Ilmari wing: scores are only kept there.
    tracking: bool,
    mirrors: Vec<Mirror>,
    perf: HashMap<EntityId, Perf>,
    /// The open vote: policy aura keys on the ballot, the uses of each slot, the policy that
    /// passed last.
    pub(crate) offered: Vec<String>,
    pub(crate) ballots: Vec<u32>,
    pub(crate) passed: String,
}

/// A player's grade, percent of their role's baseline: damage, healing, absorbs, interrupts at
/// `interrupt_value` and (tanks) twice the damage soaked, less `fail_cost` per avoidable hit.
fn grade_pct(
    role: Role,
    s: &Perf,
    interrupt_value: u32,
    fail_cost: u32,
    baselines: &[(Role, u32)],
) -> u32 {
    let base = baselines
        .iter()
        .find(|(r, _)| *r == role)
        .map_or(1, |b| u64::from(b.1));
    let soaked = if role == Role::Tank { s.taken * 2 } else { 0 };
    let raw = (s.damage
        + s.healing
        + s.absorb
        + soaked
        + u64::from(s.interrupts) * u64::from(interrupt_value))
    .saturating_sub(u64::from(s.fails) * u64::from(fail_cost));
    (raw * 100 / base.max(1)).min(999) as u32
}

impl Sim {
    pub(crate) fn hr_on_load(&mut self, zone: &str) {
        let tracking = self.content.raid_of(zone).is_some_and(|r| r.ilmari);
        self.raid.hr = HrState {
            tracking,
            ..HrState::default()
        };
    }

    /// Scores only count while a boss fights: the trash before it is not part of the review.
    fn hr_scoring(&self) -> bool {
        self.raid.hr.tracking
            && self
                .units
                .iter()
                .any(|u| u.state.boss && u.state.hostile && u.state.alive && u.state.in_combat)
    }

    pub(crate) fn hr_damage(&mut self, source: EntityId, target: EntityId, dealt: u32) {
        if !self.raid.hr.tracking || dealt == 0 || !self.hr_scoring() {
            return;
        }
        if self.is_player(source) && !self.is_player(target) {
            self.raid.hr.perf.entry(source).or_default().damage += u64::from(dealt);
        }
        if self.is_player(target) && !self.is_player(source) {
            self.raid.hr.perf.entry(target).or_default().taken += u64::from(dealt);
        }
    }

    pub(crate) fn hr_heal(&mut self, source: EntityId, amount: u32) {
        if self.raid.hr.tracking && amount > 0 && self.is_player(source) && self.hr_scoring() {
            self.raid.hr.perf.entry(source).or_default().healing += u64::from(amount);
        }
    }

    pub(crate) fn hr_absorb(&mut self, source: EntityId, amount: u32) {
        if self.raid.hr.tracking && self.is_player(source) && self.hr_scoring() {
            self.raid.hr.perf.entry(source).or_default().absorb += u64::from(amount);
        }
    }

    pub(crate) fn hr_interrupt(&mut self, by: EntityId) {
        if self.raid.hr.tracking && self.is_player(by) && self.hr_scoring() {
            self.raid.hr.perf.entry(by).or_default().interrupts += 1;
        }
    }

    pub(crate) fn hr_fail(&mut self, player: EntityId) {
        if self.raid.hr.tracking && self.is_player(player) && self.hr_scoring() {
            self.raid.hr.perf.entry(player).or_default().fails += 1;
        }
    }

    // ---- the mirror ----

    /// `Effect::SummonMirror`: copies a random living player that is not copied yet.
    pub(crate) fn summon_mirror(&mut self, ci: usize, keys: [&str; 3], aura: &str, life_ms: u32) {
        let caster = Self::id_of(ci);
        let mut pool: Vec<EntityId> = self
            .players
            .iter()
            .copied()
            .filter(|p| self.unit(*p).is_some_and(|u| u.alive))
            .filter(|p| !self.raid.hr.mirrors.iter().any(|m| m.player == *p))
            .collect();
        if pool.is_empty() {
            return;
        }
        let pick = pool.swap_remove(self.rng.below(pool.len() as u32) as usize);
        let Some(pi) = self.index(pick) else { return };
        let role = self.class_at(pi).map_or(Role::Melee, |c| c.role);
        let key = match role {
            Role::Tank => keys[0],
            Role::Healer => keys[1],
            Role::Melee | Role::Ranged => keys[2],
        };
        let Ok(def) = self.def_index(key) else { return };
        let before = self.units.len();
        // Beside the real player: the impostor stands among the teammates.
        let at = self.units[pi].state.pos;
        self.summon(ci, at, key, 1, u8::MAX, 0);
        let Some(mi) = (before..self.units.len()).find(|&i| self.units[i].def == def) else {
            return;
        };
        let unit = Self::id_of(mi);
        self.apply_aura(pi, aura, life_ms, None, caster, None);
        self.raid.hr.mirrors.push(Mirror {
            unit,
            player: pick,
            aura: aura.to_string(),
            until: self.now + ticks(life_ms),
        });
        self.events.push(Event::Mirrored {
            mirror: unit,
            player: pick,
        });
    }

    /// Mirrors expire on their own; either way the player's marker goes with the copy. A copy a
    /// player found makes the boss comment.
    fn update_mirrors(&mut self) {
        if self.raid.hr.mirrors.is_empty() {
            return;
        }
        let now = self.now;
        let mirrors = std::mem::take(&mut self.raid.hr.mirrors);
        for m in mirrors {
            let mi = self.index(m.unit);
            let alive = mi.is_some_and(|i| self.units[i].state.alive && !self.units[i].despawned);
            let boss_gone = mi.is_some_and(|i| {
                self.units[i]
                    .summoner
                    .and_then(|s| self.index(s))
                    .is_none_or(|b| !self.units[b].state.alive)
            });
            if alive && m.until > now && !boss_gone {
                self.raid.hr.mirrors.push(m);
                continue;
            }
            if let Some(i) = mi.filter(|_| alive) {
                self.die(i, None);
            }
            if let Some(pi) = self.index(m.player) {
                self.remove_aura(pi, &m.aura, crate::types::AuraEnd::Expired);
            }
        }
    }

    // ---- the vote ----

    /// Opens a vote: `show` policies picked by the rng, none on Story (the ballot is skipped).
    pub(crate) fn vote_open(&mut self, t: &TwinsDef) {
        let Some(v) = &t.vote else { return };
        let hr = &mut self.raid.hr;
        hr.offered.clear();
        hr.ballots.clear();
        if !self.raid.story {
            let mut pool = v.policies.clone();
            // Content may list fewer policies than `show`: offer what exists, never panic.
            let show = usize::from(v.show).min(pool.len());
            for k in 0..show {
                let j = k + self.rng.below((pool.len() - k) as u32) as usize;
                pool.swap(k, j);
            }
            let hr = &mut self.raid.hr;
            hr.offered = pool[..show].to_vec();
            hr.ballots = vec![0; show];
        }
        self.raid_broadcast();
    }

    /// The window is over: the slot with the most uses passes (the first on a tie or silence),
    /// Story passes its own policy. The policy lands on every living player.
    pub(crate) fn vote_close(&mut self, t: &TwinsDef, speaker: usize) {
        let Some(v) = &t.vote else { return };
        let policy = if self.raid.story {
            v.story_policy.clone()
        } else {
            let hr = &self.raid.hr;
            let mut best = 0;
            for (k, n) in hr.ballots.iter().enumerate() {
                if *n > hr.ballots[best] {
                    best = k;
                }
            }
            match hr.offered.get(best) {
                Some(p) => p.clone(),
                None => return,
            }
        };
        self.pass_policy(v, &policy, speaker);
        self.raid.hr.passed = policy;
        self.vote_open(t);
    }

    fn pass_policy(&mut self, v: &VoteDef, policy: &str, speaker: usize) {
        let source = Self::id_of(speaker);
        for p in self.players.clone() {
            if let Some(pi) = self.index(p).filter(|&i| self.units[i].state.alive) {
                self.apply_aura(pi, policy, v.duration_ms, None, source, None);
            }
        }
    }

    /// The fight ended: no vote stays open and the wall screen clears.
    pub(crate) fn vote_reset(&mut self) {
        let hr = &self.raid.hr;
        if hr.offered.is_empty() && hr.ballots.is_empty() && hr.passed.is_empty() {
            return;
        }
        let hr = &mut self.raid.hr;
        hr.offered.clear();
        hr.ballots.clear();
        hr.passed.clear();
        self.raid_broadcast();
    }

    /// A ballot terminal: one use adds one to its slot; every player may use it every few seconds
    /// (`PropDef::cooldown_ms`). Outcome is the slot number (1-based), 0 when no vote is open.
    pub(crate) fn use_ballot_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        if !self.prop_ready(player, k, def.cooldown_ms) {
            return;
        }
        let slot = self
            .twins()
            .and_then(|t| t.vote)
            .and_then(|v| v.terminals.iter().position(|key| *key == def.key));
        let outcome = match slot {
            Some(s) if self.raid.hr.ballots.len() > s => {
                self.raid.hr.ballots[s] += 1;
                self.raid_broadcast();
                s as u32 + 1
            }
            _ => 0,
        };
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key,
            outcome,
        });
    }

    /// The HR Portal: files an appeal, which takes the listed auras (`PropDef::auras`) off the
    /// user. Outcome `PROP_OK` when something came off.
    pub(crate) fn use_portal_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        if !self.prop_ready(player, k, def.cooldown_ms) {
            return;
        }
        let mut cleared = false;
        for (aura, _) in &def.auras {
            if self.has_aura(pi, aura) {
                self.remove_aura(pi, aura, crate::types::AuraEnd::Consumed);
                cleared = true;
            }
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key,
            outcome: if cleared { PROP_OK } else { 0 },
        });
    }

    // ---- the review ----

    /// `Effect::Review`: grade every living player on the window since the last review. Raw
    /// points are damage, healing, absorbs, interrupts and (tanks) twice the damage soaked, less
    /// avoidable hits, divided by the role's baseline.
    #[allow(clippy::too_many_arguments)] // the effect's fields
    pub(crate) fn review(
        &mut self,
        ci: usize,
        worst: &str,
        worst_ms: u32,
        best: Option<&str>,
        best_ms: u32,
        fine_pct: u32,
        interrupt_value: u32,
        fail_cost: u32,
        baselines: &[(Role, u32)],
    ) {
        let boss = Self::id_of(ci);
        let perf = std::mem::take(&mut self.raid.hr.perf);
        let mut grades: Vec<(EntityId, u32)> = Vec::new();
        for p in self.players.clone() {
            let Some(pi) = self.index(p).filter(|&i| self.units[i].state.alive) else {
                continue;
            };
            let role = self.class_at(pi).map_or(Role::Melee, |c| c.role);
            let s = perf.get(&p).copied().unwrap_or_default();
            grades.push((
                p,
                grade_pct(role, &s, interrupt_value, fail_cost, baselines),
            ));
        }
        // Lowest first; ties go to the earlier player id so a replay agrees.
        grades.sort_by_key(|g| (g.1, g.0));
        let worst_p = grades.first().filter(|g| g.1 < fine_pct).map(|g| g.0);
        let best_p = grades
            .last()
            .filter(|g| g.1 >= 100 && grades.len() > 1 && Some(g.0) != worst_p)
            .map(|g| g.0);
        if let Some(i) = worst_p.and_then(|p| self.index(p)) {
            self.apply_aura(i, worst, worst_ms, None, boss, None);
        }
        if let (Some(i), Some(aura)) = (best_p.and_then(|p| self.index(p)), best) {
            self.apply_aura(i, aura, best_ms, None, boss, None);
        }
        self.events.push(Event::Review {
            grades,
            worst: worst_p,
            best: best_p,
        });
    }

    /// Debug/test: `source` lands a plain direct hit of `amount` on `target`, which the review
    /// counts like any other damage.
    pub fn debug_hit(&mut self, source: EntityId, target: EntityId, amount: u32) {
        self.deal_damage(
            source,
            target,
            crate::sim::Hit {
                spell: None,
                amount,
                crit: false,
                combo: false,
                school: crate::content::School::Physical,
                dot: false,
            },
        );
    }

    pub(crate) fn update_hr(&mut self) {
        self.update_mirrors();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: [(Role, u32); 4] = [
        (Role::Tank, 1700),
        (Role::Healer, 1400),
        (Role::Melee, 1400),
        (Role::Ranged, 1400),
    ];

    #[test]
    fn the_grade_counts_what_the_formula_says_per_role() {
        let hit = |damage, healing, absorb, taken, interrupts, fails| Perf {
            damage,
            healing,
            absorb,
            taken,
            interrupts,
            fails,
        };
        // A damage dealer at exactly the baseline is a 100.
        assert_eq!(
            grade_pct(Role::Ranged, &hit(1400, 0, 0, 0, 0, 0), 150, 150, &BASE),
            100
        );
        // Soaking counts twice, but only for a tank.
        assert_eq!(
            grade_pct(Role::Tank, &hit(0, 0, 0, 850, 0, 0), 150, 150, &BASE),
            100
        );
        assert_eq!(
            grade_pct(Role::Melee, &hit(0, 0, 0, 850, 0, 0), 150, 150, &BASE),
            0
        );
        // Healing and shields count as much as damage; an interrupt is worth `interrupt_value`.
        assert_eq!(
            grade_pct(Role::Healer, &hit(0, 700, 700, 0, 0, 0), 150, 150, &BASE),
            100
        );
        assert_eq!(
            grade_pct(Role::Melee, &hit(0, 0, 0, 0, 4, 0), 150, 150, &BASE),
            42
        );
        // Every avoidable hit costs; the grade never goes below zero.
        assert_eq!(
            grade_pct(Role::Ranged, &hit(1400, 0, 0, 0, 0, 2), 150, 150, &BASE),
            78
        );
        assert_eq!(
            grade_pct(Role::Ranged, &hit(100, 0, 0, 0, 0, 9), 150, 150, &BASE),
            0
        );
    }
}
