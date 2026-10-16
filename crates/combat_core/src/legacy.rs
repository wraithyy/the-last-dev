//! Legacy Mode, the heroic tier ("the code nobody dares to touch"): at the level cap the group
//! lead switches the group's difficulty to Legacy (`Command::SetLegacy`), and every zone listed
//! in `legacy.zones` then loads with enemies above the cap, one Sprint affix of its own, the
//! Legacy gear tier and department reputation on kills. Legacy floors play in a world of their
//! own (cleared floors and open elevators are tracked apart from the campaign's), and a floor
//! boss hands out its bonus pieces once per ISO week per room (the lockout; saved offline).
//! Content: `legacy` in `game.ron`; numbers in docs/FORMULAS.md.

use crate::content::{AiAbility, Content, ContentError, SpellId};
use crate::live::week_index;
use crate::rng::Rng;
use crate::sim::Sim;
use crate::sprint::{Affix, Fx};
use crate::targeting::EntityId;
use crate::types::Event;
use crate::zones::World;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyZone {
    pub zone: String,
    /// A `sprint.affixes` key: the floor's one extra affix.
    pub affix: String,
    /// Enemy HP and damage on top of the level scaling, percent; tunes floors whose authored
    /// units are far below the cap.
    #[serde(default = "full_pct")]
    pub hp_pct: u32,
    #[serde(default = "full_pct")]
    pub damage_pct: u32,
    /// The floor's one behaviour change on Legacy, beyond the numbers.
    #[serde(default)]
    pub twist: Option<LegacyTwist>,
}

/// What a Legacy floor does differently: legacy code keeps its old habits and grows new ones.
#[derive(Clone, Debug, Deserialize)]
pub enum LegacyTwist {
    /// Every trash unit of the floor (pack members and rares, not adds) also uses this ability.
    TrashAbility(AiAbility),
    /// The floor boss also uses this ability.
    BossAbility(AiAbility),
    /// The floor boss casts the second spell wherever its content says the first.
    BossSwap(SpellId, SpellId),
}

fn full_pct() -> u32 {
    100
}

/// Absent in minimal test content: no zone has a Legacy variant.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct LegacyDef {
    /// Every Legacy enemy spawns at least at this level, floor bosses at `boss_level`.
    pub mob_level: u8,
    pub boss_level: u8,
    pub zones: Vec<LegacyZone>,
    /// Legacy pieces a floor boss rolls for each member while the room is not locked out.
    pub boss_items: u32,
}

impl LegacyDef {
    pub fn zone(&self, id: &str) -> Option<&LegacyZone> {
        self.zones.iter().find(|z| z.zone == id)
    }

    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("legacy: {m}")));
        let cap = c.max_level();
        if !self.zones.is_empty() && (self.mob_level <= cap || self.boss_level < self.mob_level) {
            return bad(format!(
                "mob_level must be above the cap {cap}, boss_level at least mob_level"
            ));
        }
        for (k, z) in self.zones.iter().enumerate() {
            if self.zones[..k].iter().any(|o| o.zone == z.zone) {
                return bad(format!("zone {} listed twice", z.zone));
            }
            if c.zone(&z.zone).is_none() {
                return bad(format!("unknown zone {}", z.zone));
            }
            if c.sprint.affix(&z.affix).is_none() || Affix::from_key(&z.affix).is_none() {
                return bad(format!("zone {}: unknown affix {}", z.zone, z.affix));
            }
            if z.hp_pct == 0 || z.damage_pct == 0 {
                return bad(format!("zone {}: zero hp or damage percent", z.zone));
            }
            let boss = c
                .zone(&z.zone)
                .and_then(|d| d.boss.as_ref())
                .and_then(|b| c.unit_def(&b.unit))
                .and_then(|u| u.ai.as_ref());
            let ok = match &z.twist {
                None => true,
                Some(LegacyTwist::TrashAbility(a)) => c.spell(a.spell).is_some(),
                Some(LegacyTwist::BossAbility(a)) => boss.is_some() && c.spell(a.spell).is_some(),
                Some(LegacyTwist::BossSwap(from, to)) => {
                    c.spell(*to).is_some()
                        && boss.is_some_and(|ai| ai.abilities.iter().any(|a| a.spell == *from))
                }
            };
            if !ok {
                return bad(format!(
                    "zone {}: twist needs known spells and a boss",
                    z.zone
                ));
            }
        }
        Ok(())
    }
}

/// Why `Command::SetLegacy` did nothing (routed to the asker).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyError {
    NotLeader,
    LevelTooLow,
    InSprint,
}

impl LegacyError {
    pub fn message(self) -> &'static str {
        match self {
            LegacyError::NotLeader => "Only the group lead decides who touches the legacy code.",
            LegacyError::LevelTooLow => {
                "Legacy Mode needs everyone at level 10. Nobody junior touches that codebase."
            }
            LegacyError::InSprint => "Finish the sprint first. One fire at a time.",
        }
    }
}

/// What the client shows: the group's difficulty, the zone loaded in Legacy Mode ("" for
/// none) and the floor bosses whose bonus loot the room already took in ISO week `week`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyView {
    pub on: bool,
    pub zone: String,
    pub week: u32,
    pub locked: Vec<String>,
}

impl LegacyView {
    /// The lockout of ISO week `week`: empty once the reset passed.
    pub fn locked_in(&self, week: u32) -> &[String] {
        if self.week == week { &self.locked } else { &[] }
    }
}

/// A weekly lockout: the keys taken in ISO week `week`. A new week empties it on the next
/// insert; readers pass the current week, so a stale set reads as empty. Legacy Mode locks floor
/// bosses with it, the raid (`raid.rs`) its encounters.
#[derive(Clone, Debug, Default)]
pub(crate) struct Lockout {
    week: u32,
    keys: BTreeSet<String>,
}

impl Lockout {
    pub(crate) fn contains(&self, week: u32, key: &str) -> bool {
        self.week == week && self.keys.contains(key)
    }

    /// The keys locked in `week` (empty once the reset passed).
    pub(crate) fn list(&self, week: u32) -> Vec<String> {
        if self.week == week {
            self.keys.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// True when `key` was not locked yet this week.
    pub(crate) fn insert(&mut self, week: u32, key: String) -> bool {
        if self.week != week {
            self.week = week;
            self.keys.clear();
        }
        self.keys.insert(key)
    }

    /// For the save: (week, keys).
    pub(crate) fn saved(&self) -> (u32, Vec<String>) {
        (self.week, self.keys.iter().cloned().collect())
    }

    pub(crate) fn restore(&mut self, week: u32, keys: impl IntoIterator<Item = String>) {
        self.week = week;
        self.keys = keys.into_iter().collect();
    }
}

#[derive(Default)]
pub(crate) struct LegacyState {
    /// The group's difficulty for the next zone load.
    on: bool,
    /// Affix runtime of the zone loaded in Legacy Mode; `None` on a Normal load.
    pub(crate) fx: Option<Fx>,
    /// Extra enemy (HP %, damage %) of that zone.
    pct: (u32, u32),
    /// The world not in play: the Legacy one on Normal floors, the campaign's on Legacy ones.
    pub(crate) parked: World,
    lock: Lockout,
    /// The load in progress touches Legacy Mode (switched on, or entering or leaving a floor),
    /// so clients need the new view; plain Normal loads stay quiet.
    announce: bool,
}

impl Sim {
    /// The group's difficulty for the next zone load (true = Legacy).
    pub fn legacy_on(&self) -> bool {
        self.legacy.on
    }

    /// The zone currently loaded in Legacy Mode, if any.
    pub fn legacy_zone(&self) -> Option<&str> {
        self.legacy.fx.as_ref().and(self.zone.as_deref())
    }

    pub(crate) fn legacy_week(&self) -> u32 {
        week_index(self.unix_s())
    }

    pub fn legacy_view(&self) -> LegacyView {
        let week = self.legacy_week();
        let locked = self.legacy.lock.list(week);
        LegacyView {
            on: self.legacy.on,
            zone: self.legacy_zone().unwrap_or_default().to_string(),
            week,
            locked,
        }
    }

    /// True while floor boss `boss` gives this room no bonus loot until the weekly reset.
    pub fn legacy_locked(&self, boss: &str) -> bool {
        self.legacy.lock.contains(self.legacy_week(), boss)
    }

    fn legacy_broadcast(&mut self) {
        let view = self.legacy_view();
        self.events.push(Event::Legacy { view });
    }

    /// A joining member learns the difficulty and the room's lockouts.
    pub(crate) fn legacy_on_join(&mut self, player: EntityId) {
        let view = self.legacy_view();
        // The default (Normal, nothing locked) is what a client assumes anyway.
        if view != LegacyView::default() {
            self.emit_to(player, Event::Legacy { view });
        }
    }

    pub(crate) fn everyone_capped(&self) -> bool {
        let cap = self.content.max_level();
        !self.players.is_empty()
            && self
                .players
                .iter()
                .all(|p| self.unit(*p).is_some_and(|u| u.level >= cap))
    }

    /// `Command::SetLegacy`: the group lead picks Normal or Legacy for the next zone loads.
    pub(crate) fn set_legacy(&mut self, actor: EntityId, on: bool) {
        let refuse = if self.players.first() != Some(&actor) {
            Some(LegacyError::NotLeader)
        } else if on && self.sprint_active() {
            Some(LegacyError::InSprint)
        } else if on && !self.everyone_capped() {
            Some(LegacyError::LevelTooLow)
        } else {
            None
        };
        if let Some(reason) = refuse {
            self.emit_to(actor, Event::LegacyRefused { reason });
            return;
        }
        if self.legacy.on != on {
            self.legacy.on = on;
            self.legacy_broadcast();
        }
    }

    /// Called by `load_zone` right after the sprint hook, before anything spawns: a Legacy
    /// load swaps the Legacy world in (a Normal one swaps the campaign's back) and arms the
    /// zone's affix. Sprint stages are never Legacy.
    pub(crate) fn legacy_on_load(&mut self, id: &str) {
        let def = self
            .content
            .legacy
            .zone(id)
            .filter(|_| self.legacy.on && !self.sprint_active() && self.everyone_capped())
            .cloned();
        self.legacy.announce = self.legacy.on || def.is_some() || self.legacy.fx.is_some();
        if def.is_some() != self.legacy.fx.is_some() {
            std::mem::swap(&mut self.world, &mut self.legacy.parked);
            // Floors were announced in the campaign; no unlock banners a second time.
            let ids: Vec<String> = self.content.zones.iter().map(|z| z.id.clone()).collect();
            self.world.announced.extend(ids);
        }
        self.legacy.pct = def
            .as_ref()
            .map_or((100, 100), |z| (z.hp_pct, z.damage_pct));
        let rng = Rng::new(self.seed ^ 0x1E6A_C700_0000 ^ self.now);
        self.legacy.fx = def.map(|z| Fx::new(vec![z.affix], rng));
    }

    /// Emitted once the load finished, so the zone in the view is the new one.
    pub(crate) fn legacy_after_load(&mut self) {
        if std::mem::take(&mut self.legacy.announce) {
            self.legacy_broadcast();
        }
    }

    /// Called for every spawn: on a Legacy floor, its trash and boss take the floor's twist.
    pub(crate) fn legacy_twist(&mut self, i: usize) {
        let Some(twist) = self
            .legacy
            .fx
            .as_ref()
            .and(self.zone.as_deref())
            .and_then(|z| self.content.legacy.zone(z))
            .and_then(|z| z.twist.clone())
        else {
            return;
        };
        let u = &self.units[i];
        let (boss, trash) = (u.floor_boss, u.zone_slot.is_some() && !u.floor_boss);
        let Some(mut ai) = self.content.units[u.def].ai.clone() else {
            return;
        };
        match twist {
            LegacyTwist::TrashAbility(a) if trash => ai.abilities.push(a),
            LegacyTwist::BossAbility(a) if boss => ai.abilities.push(a),
            LegacyTwist::BossSwap(from, to) if boss => {
                for a in ai.abilities.iter_mut().filter(|a| a.spell == from) {
                    a.spell = to;
                }
            }
            _ => return,
        }
        let n = ai.abilities.len();
        let u = &mut self.units[i];
        u.ability_next.resize(n, 0);
        u.ability_yell_cursor.resize(n, 0);
        u.ai_override = Some(Box::new(ai));
    }

    /// Floor bosses spawn at the Legacy boss level on a Legacy floor.
    pub(crate) fn legacy_boss_level(&self, level: u8) -> u8 {
        if self.legacy.fx.is_some() {
            level.max(self.content.legacy.boss_level)
        } else {
            level
        }
    }

    /// Extra enemy (HP %, damage %) of the Legacy floor or the raid's Async Attendance;
    /// 100/100 elsewhere.
    pub(crate) fn legacy_mob_pct(&self) -> (u32, u32) {
        if self.legacy.fx.is_some() {
            self.legacy.pct
        } else {
            self.raid_mob_pct()
        }
    }

    /// After a Legacy floor boss's loot was rolled: the room is locked out of its bonus
    /// pieces until the weekly reset.
    pub(crate) fn legacy_on_kill(&mut self, ti: usize) {
        if self.legacy.fx.is_none() || !self.units[ti].floor_boss {
            return;
        }
        let week = self.legacy_week();
        let key = self.units[ti].state.key.clone();
        if self.legacy.lock.insert(week, key) {
            self.legacy_broadcast();
        }
    }

    /// The room's lockout for the save: (ISO week, floor bosses).
    pub(crate) fn legacy_lockout(&self) -> (u32, Vec<String>) {
        self.legacy.lock.saved()
    }

    /// Restores a saved lockout; unknown bosses are dropped, an old week is simply stale.
    pub(crate) fn set_legacy_lockout(&mut self, week: u32, bosses: &[String]) {
        let known = |b: &String| {
            self.content.legacy.zones.iter().any(|z| {
                self.content
                    .zone(&z.zone)
                    .and_then(|d| d.boss.as_ref())
                    .is_some_and(|x| x.unit == *b)
            })
        };
        let locked: Vec<String> = bosses.iter().filter(|b| known(b)).cloned().collect();
        self.legacy.lock.restore(week, locked);
        self.legacy_broadcast();
    }

    /// Debug/QA: this week's lockout set to `bosses` (screenshots).
    pub fn debug_set_legacy_lockout(&mut self, bosses: &[String]) {
        let week = self.legacy_week();
        self.set_legacy_lockout(week, bosses);
    }

    /// Debug/QA: switches Legacy Mode without the checks (screenshots, tests).
    pub fn debug_set_legacy(&mut self, on: bool) {
        self.legacy.on = on;
        self.legacy_broadcast();
    }
}
