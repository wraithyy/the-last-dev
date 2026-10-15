//! Endgame progression after the level cap: Promotion ranks (XP keeps counting past level 10,
//! each rank adds a capped Paragon bonus and a title) and the six department reputations
//! (gained from kills at the cap, dailies and the world boss; they gate vendor items).
//! Content: `endgame` in `game.ron`; numbers in docs/FORMULAS.md.

use crate::content::{Content, ContentError};
use crate::sim::Sim;
use crate::types::Event;
use crate::world::Trigger;
use serde::Deserialize;

/// Engineering, Security, HR, Marketing, Internal IT, Sales (content order).
pub const DEPARTMENTS: usize = 6;
/// Ranks travel as one byte and titles are achievements; far above what content needs.
pub const MAX_RANKS: usize = 50;

#[derive(Clone, Debug, Deserialize)]
pub struct RankDef {
    pub name: String,
    /// One line for the `~/endgame` panel.
    #[serde(default)]
    pub blurb: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DepartmentDef {
    pub name: String,
    /// Kills at the level cap in these zones earn this department's standing.
    pub zones: Vec<String>,
    #[serde(default)]
    pub blurb: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RepTier {
    pub name: String,
    /// Standing needed for this tier; the first tier starts at 0.
    pub at: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RepGains {
    pub trash: u32,
    pub elite: u32,
    pub boss: u32,
    /// Goes to the department that likes you least (ties: content order).
    pub daily: u32,
    pub weekly: u32,
    /// The Board: every department at once.
    pub world_boss: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct EndgameDef {
    /// Rank `k` (1-based) is `ranks[k - 1]`; its title is achievement `Event("rank:<k>")`.
    #[serde(default)]
    pub ranks: Vec<RankDef>,
    /// XP from rank `r` to `r + 1` is `rank_base_xp + rank_step_xp * r`.
    #[serde(default)]
    pub rank_base_xp: u32,
    #[serde(default)]
    pub rank_step_xp: u32,
    /// Paragon: max HP and mana +this percent per rank, up to `paragon_cap_pct`.
    #[serde(default)]
    pub paragon_pct_per_rank: u32,
    #[serde(default)]
    pub paragon_cap_pct: u32,
    /// Exactly `DEPARTMENTS` entries, or none (minimal test content).
    #[serde(default)]
    pub departments: Vec<DepartmentDef>,
    /// Ascending by `at`; the last one is the cap.
    #[serde(default)]
    pub rep_tiers: Vec<RepTier>,
    #[serde(default)]
    pub rep_gains: RepGains,
    #[serde(default)]
    pub burnout: crate::burnout::BurnoutDef,
}

impl EndgameDef {
    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        if self.ranks.len() > MAX_RANKS {
            return bad(format!("at most {MAX_RANKS} promotion ranks"));
        }
        if !self.ranks.is_empty() && self.rank_base_xp == 0 {
            return bad("promotion ranks need rank_base_xp".into());
        }
        for k in 1..=self.ranks.len() {
            let key = format!("rank:{k}");
            let titled = c
                .world
                .achievements
                .iter()
                .any(|a| a.trigger == Trigger::Event(key.clone()));
            if !titled {
                return bad(format!(
                    "promotion rank {k} has no Event(\"{key}\") achievement"
                ));
            }
        }
        if !self.departments.is_empty() && self.departments.len() != DEPARTMENTS {
            return bad(format!("exactly {DEPARTMENTS} departments"));
        }
        for d in &self.departments {
            if let Some(z) = d.zones.iter().find(|z| c.zone(z).is_none()) {
                return bad(format!("department {} has unknown zone {z}", d.name));
            }
        }
        let ascending = self.rep_tiers.windows(2).all(|w| w[0].at < w[1].at);
        if !ascending || self.rep_tiers.first().is_some_and(|t| t.at != 0) {
            return bad("rep tiers start at 0 and ascend".into());
        }
        if !self.departments.is_empty() && self.rep_tiers.is_empty() {
            return bad("departments need rep tiers".into());
        }
        self.burnout.validate(c)
    }

    /// XP from rank `rank` to the next; 0 once every rank is reached.
    pub fn rank_xp_to_next(&self, rank: u8) -> u32 {
        if usize::from(rank) >= self.ranks.len() {
            return 0;
        }
        self.rank_base_xp
            .saturating_add(self.rank_step_xp.saturating_mul(u32::from(rank)))
    }

    /// The name of rank `rank` (1-based); `None` for 0 or beyond the list.
    pub fn rank_name(&self, rank: u8) -> Option<&str> {
        usize::from(rank)
            .checked_sub(1)
            .and_then(|i| self.ranks.get(i))
            .map(|r| r.name.as_str())
    }

    /// Paragon bonus in percent of max HP and mana at `rank`.
    pub fn paragon_pct(&self, rank: u8) -> u32 {
        (u32::from(rank) * self.paragon_pct_per_rank).min(self.paragon_cap_pct)
    }

    /// Index into `rep_tiers` for a standing.
    pub fn tier_of(&self, rep: u32) -> usize {
        self.rep_tiers
            .iter()
            .rposition(|t| rep >= t.at)
            .unwrap_or(0)
    }

    /// Standing cap: the last tier's threshold.
    pub fn rep_cap(&self) -> u32 {
        self.rep_tiers.last().map_or(0, |t| t.at)
    }

    pub fn department_index(&self, name: &str) -> Option<usize> {
        self.departments.iter().position(|d| d.name == name)
    }

    /// The department whose zones include `zone`.
    pub fn department_of_zone(&self, zone: &str) -> Option<usize> {
        self.departments
            .iter()
            .position(|d| d.zones.iter().any(|z| z == zone))
    }
}

impl Sim {
    /// XP past the level cap fills Promotion ranks; each rank-up refreshes the Paragon bonus
    /// (HP kept, so it never heals) and unlocks the rank's title.
    pub(crate) fn grant_rank_xp(&mut self, i: usize, amount: u32) {
        let def = &self.content.endgame;
        let id = Self::id_of(i);
        let c = &self.units[i].state.career;
        if amount == 0 || def.rank_xp_to_next(c.rank) == 0 {
            return;
        }
        let (mut rank, mut xp) = (c.rank, c.rank_xp.saturating_add(amount));
        let mut ups = Vec::new();
        loop {
            let need = def.rank_xp_to_next(rank);
            if need == 0 || xp < need {
                break;
            }
            xp -= need;
            rank += 1;
            ups.push(rank);
        }
        if def.rank_xp_to_next(rank) == 0 {
            xp = 0;
        }
        let c = &mut self.units[i].state.career;
        c.rank = rank;
        c.rank_xp = xp;
        self.emit_to(
            id,
            Event::RankProgress {
                unit: id,
                rank,
                xp,
                gained: amount,
            },
        );
        if ups.is_empty() {
            return;
        }
        self.refresh_item_bonus(i);
        for r in ups {
            self.events.push(Event::RankUp { unit: id, rank: r });
            let key = format!("rank:{r}");
            self.check_achievements(id, |t| *t == Trigger::Event(key.clone()));
        }
    }

    /// Adds standing with department `dept`, capped at the last tier.
    pub(crate) fn gain_rep(&mut self, i: usize, dept: usize, amount: u32) {
        let cap = self.content.endgame.rep_cap();
        let id = Self::id_of(i);
        let Some(r) = self.units[i].state.career.rep.get_mut(dept) else {
            return;
        };
        let total = r.saturating_add(amount).min(cap);
        let gained = total - *r;
        if gained == 0 {
            return;
        }
        *r = total;
        self.emit_to(
            id,
            Event::Reputation {
                unit: id,
                dept: dept as u8,
                total,
                gained,
            },
        );
    }

    /// A daily or the Sprint Goal paid out: the least impressed department notices.
    pub(crate) fn rep_for_task(&mut self, i: usize, weekly: bool) {
        let g = &self.content.endgame.rep_gains;
        let amount = if weekly { g.weekly } else { g.daily };
        if amount == 0 || self.content.endgame.departments.is_empty() {
            return;
        }
        let rep = self.units[i].state.career.rep;
        let dept = (0..DEPARTMENTS).min_by_key(|&d| rep[d]).unwrap_or(0);
        self.gain_rep(i, dept, amount);
    }

    /// Endgame credit for a hostile kill, for every sharer: reputation for players at the cap
    /// (the zone's department, or all of them for the world boss) and burnout relief when the
    /// victim was management. Summoned adds count for burnout only.
    pub(crate) fn endgame_on_kill(&mut self, ti: usize, killer: Option<crate::EntityId>) {
        if !self.units[ti].state.hostile || self.sprint_active() {
            return;
        }
        let key = self.units[ti].state.key.clone();
        let boss = self.units[ti].state.boss;
        let summoned = self.units[ti].summoner.is_some();
        let world_boss = self.content.world_boss().is_some_and(|b| b.unit == key);
        let elite = self.content.economy.elites.contains(&key);
        let dept = self
            .zone
            .as_deref()
            .and_then(|z| self.content.endgame.department_of_zone(z));
        let g = &self.content.endgame.rep_gains;
        let amount = if world_boss {
            g.world_boss
        } else if boss {
            g.boss
        } else if elite {
            g.elite
        } else {
            g.trash
        };
        let cap = self.content.max_level();
        for p in self.kill_sharers(killer, ti) {
            self.burnout_on_kill(p, &key, boss);
            if summoned || self.units[p].state.level < cap {
                continue;
            }
            if world_boss {
                for d in 0..self.content.endgame.departments.len() {
                    self.gain_rep(p, d, amount);
                }
            } else if let Some(d) = dept {
                self.gain_rep(p, d, amount);
            }
        }
    }

    /// Debug/QA: sets a player's rank, rank XP and standings without events; refreshes Paragon.
    pub fn debug_set_career(
        &mut self,
        player: crate::EntityId,
        rank: u8,
        rank_xp: u32,
        rep: [u32; DEPARTMENTS],
    ) {
        let Some(i) = self.index(player).filter(|_| self.is_player(player)) else {
            return;
        };
        let max = self.content.endgame.ranks.len().min(usize::from(u8::MAX)) as u8;
        let cap = self.content.endgame.rep_cap();
        let c = &mut self.units[i].state.career;
        c.rank = rank.min(max);
        let need = self.content.endgame.rank_xp_to_next(c.rank);
        c.rank_xp = rank_xp.min(need.saturating_sub(1));
        c.rep = rep.map(|r| r.min(cap));
        self.refresh_item_bonus(i);
    }
}
