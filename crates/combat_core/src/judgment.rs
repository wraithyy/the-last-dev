//! Judgment Day, the third Ilmari difficulty (docs/ILMARI_1_0.md D.1): Full plus a content block
//! of numbers (`JudgmentDef`) and one rider aura per boss, for a quorum of humans only. No Async
//! scaling, no companions (those are Story's), one revive per player per pull ("Redundancy") and
//! its own weekly lockout key (`jd:`).
//!
//! The mode is the lead's pick (`Command::SetIlmariMode`), validated here, and re-validated when a
//! wing loads: a pick that no longer holds (a member left, the zone is a Wing 1-4 one) loads as
//! Full instead of failing the door. Everything is sim state; a client claims nothing.

use crate::content::{Content, ContentError};
use crate::ilmari::IlmariMode;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::Event;
use serde::{Deserialize, Serialize};

/// Zone id prefixes of the content Judgment Day applies to: Wings 5-6 and the core. Wings 1-4
/// are not rebalanced (1.0 does not touch old content).
const JUDGMENT_ZONES: [&str; 3] = ["ilmari5/", "ilmari6/", "ilmari_core/"];
/// Rider auras last "until the fight ends": evade and death clear them, the tick puts them back.
const RIDER_MS: u32 = 3_600_000;

fn yes() -> bool {
    true
}

/// Whether Judgment Day applies in `zone`.
pub fn is_judgment_zone(zone: &str) -> bool {
    JUDGMENT_ZONES.iter().any(|p| zone.starts_with(p))
}

/// One boss's Judgment rider: an aura the boss wears for the whole pull. Individually switchable
/// so the weekly tuning pass can turn one off without a patch.
#[derive(Clone, Debug, Deserialize)]
pub struct RiderDef {
    /// The boss unit key.
    pub boss: String,
    /// The aura (`Content::auras`) its abilities read (`requires_aura`) or that carries the rule.
    pub aura: String,
    #[serde(default = "yes")]
    pub on: bool,
}

/// One unit's own Judgment figures instead of the block's (the tuning pass per boss or add, D15).
/// Unset fields keep the block's number.
#[derive(Clone, Debug, Deserialize)]
pub struct UnitPct {
    pub unit: String,
    #[serde(default)]
    pub hp_pct: Option<u32>,
    #[serde(default)]
    pub damage_pct: Option<u32>,
    #[serde(default)]
    pub enrage_pct: Option<u32>,
}

/// The numbers of Judgment Day, one block (D.1): percents sit on top of Full.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct JudgmentDef {
    pub hp_pct: u32,
    pub damage_pct: u32,
    /// Percent of the boss's enrage timer that is left (80: the soft enrage comes 20 % sooner).
    pub enrage_pct: u32,
    /// Connected humans needed to start; the quorum.
    pub min_humans: u8,
    /// Raid pieces each member rolls per boss kill (Full: the wing's `boss_items`).
    pub boss_items: u32,
    /// Legendary Fragments per clear as a multiple of Full's.
    pub fragment_mult: u32,
    pub riders: Vec<RiderDef>,
    /// Per-unit figures (bosses and the adds they summon) where the block's do not fit.
    pub units: Vec<UnitPct>,
}

impl Default for JudgmentDef {
    fn default() -> Self {
        JudgmentDef {
            hp_pct: 130,
            damage_pct: 125,
            enrage_pct: 80,
            min_humans: 3,
            boss_items: 3,
            fragment_mult: 2,
            riders: Vec::new(),
            units: Vec::new(),
        }
    }
}

impl JudgmentDef {
    /// (HP %, damage %) of `unit` on Judgment Day: its rider entry's own figures, else the block's.
    pub fn mob_pct(&self, unit: &str) -> (u32, u32) {
        let r = self.units.iter().find(|r| r.unit == unit);
        (
            r.and_then(|r| r.hp_pct).unwrap_or(self.hp_pct),
            r.and_then(|r| r.damage_pct).unwrap_or(self.damage_pct),
        )
    }

    /// Percent of `unit`'s enrage timer that is left on Judgment Day.
    pub fn enrage_of(&self, unit: &str) -> u32 {
        self.units
            .iter()
            .find(|r| r.unit == unit)
            .and_then(|r| r.enrage_pct)
            .unwrap_or(self.enrage_pct)
    }

    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        if self.hp_pct < 100 || self.damage_pct < 100 || !(1..=100).contains(&self.enrage_pct) {
            return bad(
                "judgment numbers sit on top of Full: hp and damage >= 100, enrage 1..=100".into(),
            );
        }
        if self.min_humans < 3 {
            return bad("judgment day needs a quorum of at least three".into());
        }
        if self.boss_items == 0 || self.fragment_mult == 0 {
            return bad("judgment day pays at least one piece and one fragment multiple".into());
        }
        for r in &self.riders {
            if c.unit_def(&r.boss).is_none_or(|u| !u.boss || !u.hostile)
                || c.aura(&r.aura).is_none()
            {
                return bad(format!(
                    "judgment rider {} / {} needs a hostile boss and a known aura",
                    r.boss, r.aura
                ));
            }
        }
        for u in &self.units {
            let ok = |n: Option<u32>, lo: u32, hi: u32| n.is_none_or(|n| (lo..=hi).contains(&n));
            if c.unit_def(&u.unit).is_none_or(|d| !d.hostile)
                || !ok(u.hp_pct, 100, 1000)
                || !ok(u.damage_pct, 100, 1000)
                || !ok(u.enrage_pct, 1, 100)
            {
                return bad(format!(
                    "judgment unit {}: a hostile unit and figures on top of Full (hp and damage 100..=1000, enrage 1..=100)",
                    u.unit
                ));
            }
        }
        Ok(())
    }
}

/// Why `Command::SetIlmariMode` did nothing, or why a revive was refused (routed to the player).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IlmariModeError {
    NotLeader,
    /// Fewer than `min_humans` connected.
    Quorum,
    /// The room has not killed Ilmari on Full yet.
    NotCleared,
    /// The lead stands in Wings 1-4, which have no Judgment Day.
    WrongZone,
    /// A boss fight is running.
    InPull,
    /// Judgment Day's one revive per player per pull is spent.
    Redundancy,
}

impl IlmariModeError {
    pub fn message(self) -> &'static str {
        match self {
            IlmariModeError::NotLeader => "Only the group lead sets the agenda.",
            IlmariModeError::Quorum => "Quorum not reached. This meeting has been rescheduled.",
            IlmariModeError::NotCleared => {
                "Judgment Day is scheduled after the Full programme. The Full programme has not been completed."
            }
            IlmariModeError::WrongZone => {
                "Judgment Day covers the Alignment Lab, the Boardroom and the core. The earlier wings were reviewed already."
            }
            IlmariModeError::InPull => "The agenda is frozen while the meeting is in session.",
            IlmariModeError::Redundancy => {
                "Redundancy applied. Your position has been eliminated for this pull."
            }
        }
    }
}

impl Sim {
    /// The difficulty of the loaded wing (`Full` outside one).
    pub fn ilmari_mode(&self) -> IlmariMode {
        self.raid.mode
    }

    /// The room's sim has killed Ilmari on Full: Judgment Day is open.
    pub fn ilmari_full_cleared(&self) -> bool {
        self.ilmari.core_full
    }

    /// The core's death on Full calls this (the core does not exist before D13); a Story or
    /// Judgment kill does not, and a save keeps it.
    pub fn mark_core_full_clear(&mut self) {
        self.ilmari.core_full = true;
    }

    /// Debug/QA: Judgment Day's prerequisite on or off without a core fight.
    pub fn debug_ilmari_core_cleared(&mut self, on: bool) {
        self.ilmari.core_full = on;
    }

    /// The lead's rule check for Judgment Day: quorum and the Full clear.
    fn judgment_open(&self) -> Result<(), IlmariModeError> {
        if self.players.len() < usize::from(self.content.ilmari.judgment.min_humans) {
            Err(IlmariModeError::Quorum)
        } else if !self.ilmari.core_full {
            Err(IlmariModeError::NotCleared)
        } else {
            Ok(())
        }
    }

    /// A hostile boss is fighting: a pull is on.
    fn boss_pull_active(&self) -> bool {
        self.units.iter().any(|u| {
            u.state.boss && u.state.hostile && u.state.alive && !u.despawned && u.state.in_combat
        })
    }

    /// `Command::SetIlmariMode` (and its alias `SetIlmariStory`): the lead picks the next
    /// wing's difficulty.
    pub(crate) fn set_ilmari_mode(&mut self, actor: EntityId, mode: IlmariMode) {
        let refuse = if self.players.first() != Some(&actor) {
            Some(IlmariModeError::NotLeader)
        } else if self.raid.pick == Some(mode) {
            return;
        } else if self.in_ilmari_raid() && self.boss_pull_active() {
            Some(IlmariModeError::InPull)
        } else if mode != IlmariMode::Judgment {
            None
        } else if self
            .zone
            .as_deref()
            .is_some_and(|z| self.in_ilmari_raid() && !is_judgment_zone(z))
        {
            Some(IlmariModeError::WrongZone)
        } else {
            self.judgment_open().err()
        };
        if let Some(reason) = refuse {
            self.emit_to(actor, Event::IlmariRefused { reason });
            return;
        }
        self.raid.pick = Some(mode);
        self.raid_broadcast();
    }

    fn in_ilmari_raid(&self) -> bool {
        self.cur_raid_is_ilmari()
    }

    /// The mode the next (or, with a zone, that) wing loads in: QA force, else the lead's pick,
    /// else by group size. A Judgment pick that fails its rules there is Full.
    pub(crate) fn wing_mode(&self, zone: Option<&str>) -> IlmariMode {
        let min = self
            .content
            .raids
            .iter()
            .find(|r| r.ilmari)
            .map_or(3, |r| usize::from(r.normal_min_players));
        let by_size = || {
            if self.players.len() < min {
                IlmariMode::Story
            } else {
                IlmariMode::Full
            }
        };
        let mode = match (self.raid.force, self.raid.pick) {
            (Some(true), _) => IlmariMode::Story,
            (Some(false), _) => IlmariMode::Full,
            (None, Some(m)) => m,
            (None, None) => by_size(),
        };
        let holds = self.judgment_open().is_ok() && zone.is_none_or(is_judgment_zone);
        if mode == IlmariMode::Judgment && !holds {
            IlmariMode::Full
        } else {
            mode
        }
    }

    /// Judgment Day's "Redundancy": during a boss pull each player gets one revive (an ally's
    /// resurrect or their own release). False: the player stays down until the pull ends.
    pub(crate) fn jd_revive_ok(&mut self, pid: EntityId) -> bool {
        if self.raid.mode != IlmariMode::Judgment {
            return true;
        }
        if !self.boss_pull_active() {
            return true;
        }
        if self.ilmari.jd_spent.insert(pid) {
            self.raid_broadcast();
            true
        } else {
            self.emit_to(
                pid,
                Event::IlmariRefused {
                    reason: IlmariModeError::Redundancy,
                },
            );
            false
        }
    }

    /// Players whose revive is spent in the running pull (the party-frame icon).
    pub fn jd_spent(&self) -> Vec<EntityId> {
        self.ilmari.jd_spent.iter().copied().collect()
    }

    /// Every tick on Judgment: a finished pull gives the revives back, and each switched-on rider
    /// is on its boss (evade and death strip auras, so a reset boss gets it again).
    pub(crate) fn update_judgment(&mut self) {
        if self.raid.mode != IlmariMode::Judgment {
            return;
        }
        if !self.ilmari.jd_spent.is_empty() && !self.boss_pull_active() {
            self.ilmari.jd_spent.clear();
            self.raid_broadcast();
        }
        for k in 0..self.content.ilmari.judgment.riders.len() {
            let r = &self.content.ilmari.judgment.riders[k];
            if !r.on {
                continue;
            }
            let (boss, aura) = (r.boss.clone(), r.aura.clone());
            for i in 0..self.units.len() {
                let u = &self.units[i];
                if u.state.key == boss && u.state.alive && !u.despawned && !self.has_aura(i, &aura)
                {
                    self.apply_aura(i, &aura, RIDER_MS, None, Self::id_of(i), None);
                }
            }
        }
    }
}
