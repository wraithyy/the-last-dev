//! Levels: player growth and spell unlocks, mob level scaling, kill XP.

use crate::content::{Content, SpellDef, SpellId};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::Event;

/// `100 + per * (level - authored)` percent, floored at 10% for down-scaled mobs.
fn scale_pct(per: u32, level: u8, authored: u8) -> u32 {
    let d = i64::from(level) - i64::from(authored);
    (100 + i64::from(per) * d).max(10) as u32
}

impl Content {
    /// Kill XP for a player of `player_level` killing a mob of `mob_level` worth `reward`
    /// (already level-scaled). See FORMULAS.md.
    pub fn kill_xp(&self, player_level: u8, mob_level: u8, reward: u32) -> u32 {
        let r = &self.config.xp_rules;
        let pct = if mob_level >= player_level {
            let d = u32::from(mob_level - player_level).min(r.above_max_levels);
            100 + r.above_pct_per_level * d
        } else {
            let d = u32::from(player_level - mob_level);
            if d >= r.gray_level_diff {
                0
            } else {
                100u32.saturating_sub(r.below_pct_per_level * d)
            }
        };
        (u64::from(reward) * u64::from(pct) / 100) as u32
    }
}

impl Sim {
    /// Sets level-dependent stats; HP and mana are refilled. Player: authored stats plus the
    /// cumulative level bonus. Others: authored stats scaled by `config.mob_scaling`.
    pub(crate) fn apply_level(&mut self, i: usize, level: u8) {
        let is_player = Some(Self::id_of(i)) == self.player;
        let d = &self.content.units[self.units[i].def];
        let u = &mut self.units[i];
        u.state.level = level;
        if is_player {
            let b = level
                .checked_sub(1)
                .and_then(|k| self.content.levels.get(usize::from(k)));
            let (hp, mana, sp) = b.map_or((0, 0, 0), |b| {
                (b.hp_bonus, b.mana_bonus, b.spell_power_bonus)
            });
            u.state.max_hp = d.max_hp + hp;
            u.state.max_mana = d.max_mana + mana;
            u.sp = d.spell_power + sp;
            u.dmg_pct = 100;
        } else {
            let s = &self.content.config.mob_scaling;
            let hp = scale_pct(s.hp_pct_per_level, level, d.level);
            u.state.max_hp = (u64::from(d.max_hp) * u64::from(hp) / 100).max(1) as u32;
            u.state.max_mana = d.max_mana;
            u.sp = d.spell_power;
            u.dmg_pct = scale_pct(s.damage_pct_per_level, level, d.level);
            if d.hostile {
                let xp = scale_pct(s.xp_pct_per_level, level, d.level);
                u.state.xp = (u64::from(d.xp_reward) * u64::from(xp) / 100) as u32;
            }
        }
        u.state.hp = u.state.max_hp;
        u.state.mana = u.state.max_mana;
    }

    /// Debug/test helper: puts the player at `level` (clamped to the table) with 0 XP,
    /// full HP and mana. Silent: no `LevelUp` or `SpellLearned` events.
    pub fn set_player_level(&mut self, level: u8) {
        let level = level.clamp(1, self.content.max_level());
        if let Some(i) = self.player.and_then(|p| self.index(p)) {
            self.apply_level(i, level);
            self.units[i].state.xp = 0;
        }
    }

    /// The player's learned action-bar spells in bar order; every bar spell for other units.
    pub fn known_spells(&self, unit: EntityId) -> Vec<SpellId> {
        let Some(i) = self.index(unit) else {
            return Vec::new();
        };
        self.content
            .action_bar()
            .iter()
            .copied()
            .filter(|id| {
                self.content
                    .spell(*id)
                    .is_some_and(|s| self.is_learned(i, s))
            })
            .collect()
    }

    pub(crate) fn is_learned(&self, i: usize, spell: &SpellDef) -> bool {
        Some(Self::id_of(i)) != self.player || spell.learn_level <= self.units[i].state.level
    }

    /// Only the player earns XP; levels carry over XP beyond the threshold.
    pub(crate) fn award_kill_xp(&mut self, killer: usize, mob: usize) {
        if Some(Self::id_of(killer)) != self.player || !self.units[killer].state.alive {
            return;
        }
        let (pl, ml) = (self.units[killer].state.level, self.units[mob].state.level);
        // Summoned adds give nothing, so long boss fights can't be farmed for XP.
        if self.content.xp_to_next(pl) == 0
            || !self.units[mob].state.hostile
            || self.units[mob].summoner.is_some()
        {
            return;
        }
        let amount = self.content.kill_xp(pl, ml, self.units[mob].state.xp);
        if amount == 0 {
            return;
        }
        let id = Self::id_of(killer);
        self.units[killer].state.xp += amount;
        self.events.push(Event::XpGained { unit: id, amount });
        loop {
            let level = self.units[killer].state.level;
            let need = self.content.xp_to_next(level);
            if need == 0 || self.units[killer].state.xp < need {
                break;
            }
            self.units[killer].state.xp -= need;
            let level = level + 1;
            self.apply_level(killer, level);
            if self.content.xp_to_next(level) == 0 {
                self.units[killer].state.xp = 0;
            }
            self.events.push(Event::LevelUp { unit: id, level });
            let learned: Vec<SpellId> = self
                .content
                .action_bar()
                .iter()
                .copied()
                .filter(|s| {
                    self.content
                        .spell(*s)
                        .is_some_and(|d| d.learn_level == level)
                })
                .collect();
            for spell in learned {
                self.events.push(Event::SpellLearned { unit: id, spell });
            }
        }
    }
}
