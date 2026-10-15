//! Levels: player growth and spell unlocks, mob level scaling, kill XP.

use crate::content::{Content, SpellDef, SpellId};
use crate::geom::in_range;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::Event;

/// `100 + per * (level - authored)` percent, floored at 10% for down-scaled mobs.
fn scale_pct(per: u32, level: u8, authored: u8) -> u32 {
    let d = i64::from(level) - i64::from(authored);
    (100 + i64::from(per) * d).max(10) as u32
}

/// Group members within this distance of a kill share its XP and quest credit.
pub const GROUP_SHARE_RANGE_M: f32 = 60.0;

/// Classic group XP bonus by eligible members (1..=5), in percent of the solo kill XP total.
const GROUP_XP_BONUS_PCT: [u32; 5] = [100, 100, 116, 130, 140];

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
        let is_player = self.is_player(Self::id_of(i));
        let group_pct = self.group_hp_pct();
        let group_dmg_pct = self.group_damage_pct();
        let group_extra = self.players.len().saturating_sub(1) as u32;
        let hp_level_pct = self.class_at(i).map_or(100, |c| c.level_hp_pct);
        let hp_aura_pct = self.max_hp_aura_pct(i);
        let (sprint_hp, sprint_dmg) = self.sprint_mob_pct(i);
        // Sprint mobs sit at the run's level, not in the levelling curve.
        let reward_pct = if self.live_run().is_some() {
            100
        } else {
            self.content.xp_reward_pct(level)
        };
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
            let t = u.item_bonus;
            let hp = (u64::from(hp) * u64::from(hp_level_pct) / 100) as u32;
            let base_hp = d.max_hp + hp + t.max_hp;
            u.state.max_hp = if hp_aura_pct == 0 {
                base_hp
            } else {
                (i64::from(base_hp) * (100 + hp_aura_pct).max(10) / 100) as u32
            };
            u.state.max_mana = d.max_mana + mana + t.max_mana;
            // Paragon: Promotion ranks add a small capped share of HP and mana.
            let paragon = u64::from(self.content.endgame.paragon_pct(u.state.career.rank));
            u.state.max_hp = (u64::from(u.state.max_hp) * (100 + paragon) / 100) as u32;
            u.state.max_mana = (u64::from(u.state.max_mana) * (100 + paragon) / 100) as u32;
            u.sp = d.spell_power + sp + t.spell_power;
            u.dmg_pct = 100;
        } else {
            let s = &self.content.config.mob_scaling;
            let hp = scale_pct(s.hp_pct_per_level, level, d.level);
            let (group, group_dmg) = if d.hostile && d.ai.is_some() {
                (group_pct, group_dmg_pct)
            } else {
                (100, 100)
            };
            u.state.max_hp = (u64::from(d.max_hp) * u64::from(hp) * u64::from(group) / 10_000
                * u64::from(sprint_hp)
                / 100)
                .max(1) as u32;
            u.state.max_mana = d.max_mana;
            u.sp = d.spell_power;
            let trash = if d.hostile && d.ai.is_some() && !d.boss {
                self.content.config.trash_damage_pct
            } else {
                100
            };
            // Bosses are the group's real test: they get an extra slice per member on top.
            let group_dmg = if d.boss && d.hostile {
                group_dmg + self.content.config.group_boss_damage_pct_per_player * group_extra
            } else {
                group_dmg
            };
            u.dmg_pct = scale_pct(s.damage_pct_per_level, level, d.level) * group_dmg / 100 * trash
                / 100
                * sprint_dmg
                / 100;
            if d.hostile {
                let xp = scale_pct(s.xp_pct_per_level, level, d.level);
                u.state.xp = (u64::from(d.xp_reward) * u64::from(xp) / 100 * u64::from(reward_pct)
                    / 100) as u32;
            }
        }
        u.state.hp = u.state.max_hp;
        u.state.mana = u.state.max_mana;
    }

    /// Enemy HP percent for the current group size (100 solo); see FORMULAS.md.
    pub(crate) fn group_hp_pct(&self) -> u32 {
        let extra = self.players.len().saturating_sub(1) as u32;
        100 + self.content.config.group_hp_pct_per_player * extra
    }

    /// Enemy damage percent for the current group size (100 solo); see FORMULAS.md.
    pub(crate) fn group_damage_pct(&self) -> u32 {
        let extra = self.players.len().saturating_sub(1) as u32;
        100 + self.content.config.group_damage_pct_per_player * extra
    }

    /// Re-applies the group HP scaling to every enemy after a join or leave, keeping each
    /// one's HP fraction (a living unit never drops to 0 from this).
    pub(crate) fn rescale_group_hp(&mut self) {
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.despawned || self.is_player(u.state.id) {
                continue;
            }
            let (old_max, hp, mana, level) =
                (u.state.max_hp, u.state.hp, u.state.mana, u.state.level);
            self.apply_level(i, level);
            let u = &mut self.units[i].state;
            let new_max = u.max_hp;
            u.mana = mana;
            u.hp = if old_max == new_max {
                hp
            } else if !u.alive {
                0
            } else {
                ((u64::from(hp) * u64::from(new_max) / u64::from(old_max.max(1))) as u32)
                    .clamp(1, new_max)
            };
        }
    }

    /// Debug/test helper: puts the player at `level` (clamped to the table) with 0 XP,
    /// full HP and mana. Silent: no `LevelUp` or `SpellLearned` events.
    pub fn set_player_level(&mut self, level: u8) {
        if let Some(id) = self.player() {
            self.set_level_of(id, level);
        }
    }

    /// `set_player_level` for any group member (server dev start level); false for a non-player.
    pub fn set_level_of(&mut self, player: EntityId, level: u8) -> bool {
        let level = level.clamp(1, self.content.max_level());
        let Some(i) = self.index(player).filter(|_| self.is_player(player)) else {
            return false;
        };
        self.apply_level(i, level);
        let u = &mut self.units[i].state;
        u.xp = 0;
        u.xp_to_next = self.content.xp_to_next(level);
        true
    }

    /// Debug/test helper: sets the player's mana (clamped to max). Silent.
    pub fn set_player_mana(&mut self, mana: u32) {
        if let Some(i) = self.first_player_index() {
            let u = &mut self.units[i].state;
            u.mana = mana.min(u.max_mana);
        }
    }

    /// The player's learned class-bar spells in bar order; every default-bar spell for others.
    pub fn known_spells(&self, unit: EntityId) -> Vec<SpellId> {
        let Some(i) = self.index(unit) else {
            return Vec::new();
        };
        self.bar_at(i)
            .iter()
            .copied()
            .filter(|id| {
                self.content
                    .spell(*id)
                    .is_some_and(|s| self.is_learned(i, s))
            })
            .collect()
    }

    /// The class bar of unit `i` (the default bar for non-players).
    pub(crate) fn bar_at(&self, i: usize) -> &[SpellId] {
        self.class_at(i)
            .map_or_else(|| self.content.action_bar(), |c| &c.action_bar)
    }

    /// Players cast only their own class's spells, once their level allows.
    pub(crate) fn is_learned(&self, i: usize, spell: &SpellDef) -> bool {
        if !self.is_player(Self::id_of(i)) {
            return true;
        }
        let in_class = self
            .class_at(i)
            .is_none_or(|c| c.knows(&self.content, spell.id))
            || self.is_variant_of_known(i, spell.id);
        in_class && spell.learn_level <= self.units[i].state.level
    }

    /// Group members sharing a kill by a player: the killer (even dead, for quest credit) plus
    /// every other living member within `GROUP_SHARE_RANGE_M` of the corpse, in join order.
    /// Empty when the killer is not a player.
    pub(crate) fn kill_sharers(&self, killer: Option<EntityId>, mob: usize) -> Vec<usize> {
        let Some(k) = killer.filter(|k| self.is_player(*k)) else {
            return Vec::new();
        };
        let at = self.units[mob].state.pos;
        self.players
            .iter()
            .filter_map(|p| self.index(*p))
            .filter(|&p| {
                let s = &self.units[p].state;
                s.id == k || (s.alive && in_range(s.pos, at, GROUP_SHARE_RANGE_M))
            })
            .collect()
    }

    /// Only players earn XP: each living sharer gets its own level-adjusted kill XP times the
    /// group bonus, divided by the sharer count. Levels carry over XP beyond the threshold.
    pub(crate) fn award_kill_xp(&mut self, killer: EntityId, mob: usize) {
        // Summoned adds give nothing, so long boss fights can't be farmed for XP.
        if !self.units[mob].state.hostile || self.units[mob].summoner.is_some() {
            return;
        }
        let sharers: Vec<usize> = self
            .kill_sharers(Some(killer), mob)
            .into_iter()
            .filter(|&p| self.units[p].state.alive)
            .collect();
        let n = sharers.len();
        let bonus = GROUP_XP_BONUS_PCT[n.clamp(1, GROUP_XP_BONUS_PCT.len()) - 1];
        let (ml, reward) = (self.units[mob].state.level, self.units[mob].state.xp);
        for p in sharers {
            let pl = self.units[p].state.level;
            let solo = u64::from(self.content.kill_xp(pl, ml, reward));
            self.grant_xp(p, (solo * u64::from(bonus) / 100 / n as u64) as u32);
        }
    }

    /// Adds XP to the player at `killer` (kills, quest rewards) and runs the level-ups.
    pub(crate) fn grant_xp(&mut self, killer: usize, amount: u32) {
        let level = self.units[killer].state.level;
        if amount == 0 {
            return;
        }
        if self.content.xp_to_next(level) == 0 {
            return self.grant_rank_xp(killer, amount);
        }
        let id = Self::id_of(killer);
        self.units[killer].state.xp += amount;
        self.emit_to(id, Event::XpGained { unit: id, amount });
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
                .bar_at(killer)
                .iter()
                .copied()
                .filter(|s| {
                    self.content
                        .spell(*s)
                        .is_some_and(|d| d.learn_level == level)
                })
                .collect();
            for spell in learned {
                self.emit_to(id, Event::SpellLearned { unit: id, spell });
            }
        }
    }
}
