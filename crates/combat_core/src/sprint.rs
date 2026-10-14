//! Sprint mode: a replayable, timed run through one building at a chosen "story point" level.
//!
//! The group leader starts it in the campus (`Command::StartSprint`). The run owns its own world
//! state (the campaign's cleared zones and dead trash are parked and restored afterwards), picks
//! its stages from the building with a run seed, scales every enemy by the level and switches on
//! the level's affixes. The host (client offline, server online) moves the group: it loads
//! `Sim::sprint_travel` whenever that names a zone. Any other zone load abandons the run.
//!
//! Affix names, texts, icons, tiers and timings are content (`SprintDef`); what each one does is
//! keyed by its content key here. Affixes rotate per session seed ("this week's affixes"), so the
//! planning board can show them before anyone commits.

use crate::content::{Content, ContentError, School, SpellId, ticks};
use crate::economy::{GearSource, Rarity};
use crate::rng::Rng;
use crate::sim::{Hit, Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::time::ticks_to_ms;
use crate::types::{AiState, AuraEnd, Event};
use crate::zones::World;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Records kept per building and player.
pub const RECORDS_PER_BUILDING: usize = 5;
/// The add Scope Creep spawns, the Legacy Code elite and the auras the affixes use.
pub const SCOPE_CREEP_UNIT: &str = "additional_requirement";
pub const LEGACY_UNIT: &str = "legacy_monolith";
pub const LEGACY_AURA: &str = "do_not_touch";
pub const PAIR_AURA: &str = "pair_programming";
pub const BURNOUT_AURA: &str = "burnout";
/// Long enough to outlast any run; the affix code removes these auras itself.
const PERMANENT_MS: u32 = 3_600_000;
const STATUS_EVERY_TICKS: u64 = 30;
const HOTFIX_BELOW_PCT: u32 = 30;
/// Technical Debt stops compounding here, so a bad wipe streak is not a death spiral forever.
const TECH_DEBT_MAX: u32 = 10;

// ---- content ----

#[derive(Clone, Debug, Deserialize)]
pub struct SprintBuilding {
    pub key: String,
    pub name: String,
    /// Candidate stages in run order; `pick` of them are drawn (order kept).
    pub zones: Vec<String>,
    pub pick: u8,
    /// Par at sprint level 1.
    pub par_ms: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AffixDef {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub text: String,
    /// Which `SprintDef::affix_levels` slot the affix can fill.
    pub tier: u8,
    #[serde(default)]
    pub period_ms: u32,
    /// Affix strength in percent (damage of max HP, heal of max HP, ...).
    #[serde(default)]
    pub power_pct: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SprintRewardDef {
    pub sp_per_level: u32,
    /// Gear pieces at level 1, plus one per `items_per_levels` levels.
    pub items: u32,
    pub items_per_levels: u8,
    /// Chance per level for a named boss piece of the building.
    pub legendary_pct_per_level: u32,
}

/// Absent in minimal test content: no buildings, so no sprint can start.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SprintDef {
    pub unlock_level: u8,
    /// Killing this boss once (any session saved) unlocks sprints below `unlock_level` too.
    pub unlock_boss: String,
    /// Every sprint enemy spawns at least at this level.
    pub mob_level: u8,
    pub hp_pct_per_level: u32,
    pub damage_pct_per_level: u32,
    pub max_level: u8,
    pub affix_levels: Vec<u8>,
    /// Share of the run's trash that makes 100% "story points burned".
    pub forces_need_pct: u32,
    pub death_penalty_ms: u32,
    pub par_pct_per_level: u32,
    pub travel_delay_ms: u32,
    pub return_delay_ms: u32,
    /// Finishing within the first percent of par upgrades by 3, within the second by 2.
    pub upgrade_pct: (u32, u32),
    /// Technical Debt: enemy damage per player death, percent.
    pub tech_debt_pct: u32,
    pub mandatory_fun_spell: u32,
    pub buildings: Vec<SprintBuilding>,
    pub affixes: Vec<AffixDef>,
    pub reward: SprintRewardDef,
}

/// What an affix does; content names it by key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Affix {
    AiAssisted,
    Reorg,
    ScopeCreep,
    MandatoryFun,
    TechnicalDebt,
    HotfixFriday,
    BudgetFreeze,
    Burnout,
    PairProgramming,
    LegacyCode,
}

impl Affix {
    pub fn from_key(key: &str) -> Option<Affix> {
        Some(match key {
            "ai_assisted" => Affix::AiAssisted,
            "reorg" => Affix::Reorg,
            "scope_creep" => Affix::ScopeCreep,
            "mandatory_fun" => Affix::MandatoryFun,
            "technical_debt" => Affix::TechnicalDebt,
            "hotfix_friday" => Affix::HotfixFriday,
            "budget_freeze" => Affix::BudgetFreeze,
            "burnout" => Affix::Burnout,
            "pair_programming" => Affix::PairProgramming,
            "legacy_code" => Affix::LegacyCode,
            _ => return None,
        })
    }
}

impl SprintDef {
    pub fn building(&self, key: &str) -> Option<&SprintBuilding> {
        self.buildings.iter().find(|b| b.key == key)
    }

    pub fn affix(&self, key: &str) -> Option<&AffixDef> {
        self.affixes.iter().find(|a| a.key == key)
    }

    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("sprint: {m}")));
        for b in &self.buildings {
            if b.zones.is_empty() || usize::from(b.pick) == 0 || usize::from(b.pick) > b.zones.len()
            {
                return bad(format!(
                    "building {} picks {} of {}",
                    b.key,
                    b.pick,
                    b.zones.len()
                ));
            }
            if let Some(z) = b.zones.iter().find(|z| c.zone(z).is_none()) {
                return bad(format!("building {}: unknown zone {z}", b.key));
            }
        }
        for a in &self.affixes {
            if Affix::from_key(&a.key).is_none() {
                return bad(format!("affix {} has no behaviour", a.key));
            }
            if usize::from(a.tier) >= self.affix_levels.len() {
                return bad(format!("affix {} tier {} has no level", a.key, a.tier));
            }
        }
        if self.buildings.is_empty() {
            return Ok(());
        }
        for u in [SCOPE_CREEP_UNIT, LEGACY_UNIT] {
            if c.unit_def(u).is_none() {
                return bad(format!("missing unit {u}"));
            }
        }
        for a in [LEGACY_AURA, PAIR_AURA, BURNOUT_AURA] {
            if c.aura(a).is_none() {
                return bad(format!("missing aura {a}"));
            }
        }
        if c.spell(SpellId(self.mandatory_fun_spell))
            .is_none_or(|s| s.telegraph.is_none())
        {
            return bad("mandatory_fun_spell needs a telegraph".into());
        }
        Ok(())
    }
}

// ---- public views ----

/// Everything the HUD shows during a run; carried by the sprint events.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SprintStatus {
    pub building: String,
    pub level: u8,
    pub affixes: Vec<String>,
    pub par_ms: u32,
    /// Run time so far, without the death penalty.
    pub elapsed_ms: u32,
    pub penalty_ms: u32,
    /// "Story points burned", capped at 100.
    pub forces_pct: u8,
    pub bosses_left: u8,
    pub bosses_total: u8,
    /// 1-based stage being played, of `stages`.
    pub stage: u8,
    pub stages: u8,
    pub deaths: u16,
    pub tech_debt: u8,
}

impl SprintStatus {
    pub fn time_ms(&self) -> u32 {
        self.elapsed_ms.saturating_add(self.penalty_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SprintResult {
    /// Under par with the scope burned down.
    Delivered,
    /// Late, or done without enough story points burned.
    CarriedOver,
    /// Left the building.
    Abandoned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeterLine {
    pub unit: EntityId,
    pub class: String,
    pub damage: u64,
    pub healing: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardLine {
    pub unit: EntityId,
    pub items: Vec<String>,
    pub sp: u32,
}

/// The Sprint Review: final status, who did what and the chest.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SprintSummary {
    pub status: SprintStatus,
    pub meter: Vec<MeterLine>,
    pub rewards: Vec<RewardLine>,
    /// The level the next sprint of this building is recommended at.
    pub next_level: u8,
}

/// One leaderboard line: a delivered sprint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SprintRecord {
    pub building: String,
    pub level: u8,
    pub time_ms: u32,
    pub class: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SprintError {
    NotLeader,
    Locked,
    NotInCampus,
    InCombat,
    Dead,
    AlreadyRunning,
    UnknownBuilding,
    BadLevel,
}

impl SprintError {
    pub fn message(self) -> &'static str {
        match self {
            SprintError::NotLeader => "Only the group lead can commit the team to a sprint.",
            SprintError::Locked => {
                "Sprints unlock at level 6 or after the CEO. Prove you can deliver first."
            }
            SprintError::NotInCampus => "Sprint planning happens on campus. In person. Sadly.",
            SprintError::InCombat => "Finish the current incident before planning the next one.",
            SprintError::Dead => "Everyone has to be alive for planning. HR insists.",
            SprintError::AlreadyRunning => "A sprint is already running. Finish it or walk out.",
            SprintError::UnknownBuilding => "That building is not in the backlog.",
            SprintError::BadLevel => "That many story points do not exist. Yet.",
        }
    }
}

/// What the planning board shows for a building and level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SprintPreview {
    pub building: String,
    pub name: String,
    pub level: u8,
    pub affixes: Vec<String>,
    pub par_ms: u32,
    pub hp_pct: u32,
    pub damage_pct: u32,
}

/// This session's affixes for `level`: one per reached `affix_levels` slot, drawn from the
/// slot's tier with the session seed (so every run of the session agrees, like a weekly reset).
pub fn affixes_for(def: &SprintDef, seed: u64, level: u8) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (slot, &at) in def.affix_levels.iter().enumerate() {
        if level < at {
            break;
        }
        let pool: Vec<&AffixDef> = def
            .affixes
            .iter()
            .filter(|a| usize::from(a.tier) == slot && !out.contains(&a.key))
            .collect();
        if pool.is_empty() {
            continue;
        }
        let mut rng = Rng::new(seed ^ 0x5B21_7A11_0000 ^ slot as u64);
        let k = rng.below(pool.len() as u32) as usize;
        out.push(pool[k].key.clone());
    }
    out
}

/// The planning board's view of `building` at `level` for a session seed (a client online
/// has the room seed from `Welcome`, so it shows the same affixes as the server).
pub fn preview(def: &SprintDef, seed: u64, building: &str, level: u8) -> Option<SprintPreview> {
    let b = def.building(building)?;
    let lv = u32::from(level.saturating_sub(1));
    Some(SprintPreview {
        building: b.key.clone(),
        name: b.name.clone(),
        level,
        affixes: affixes_for(def, seed, level),
        par_ms: par_ms(def, b, level),
        hp_pct: 100 + def.hp_pct_per_level * lv,
        damage_pct: 100 + def.damage_pct_per_level * lv,
    })
}

/// Par of a building at a level.
pub fn par_ms(def: &SprintDef, building: &SprintBuilding, level: u8) -> u32 {
    let extra = def.par_pct_per_level * u32::from(level.saturating_sub(1));
    (u64::from(building.par_ms) * u64::from(100 + extra) / 100) as u32
}

/// Keystone-like upgrade for a delivered run by its margin under par (+1..=+3); 0 otherwise.
pub fn upgrade_for(def: &SprintDef, result: SprintResult, time_ms: u32, par: u32) -> i8 {
    if result != SprintResult::Delivered {
        return 0;
    }
    let pct = u64::from(time_ms) * 100 / u64::from(par.max(1));
    if pct <= u64::from(def.upgrade_pct.0) {
        3
    } else if pct <= u64::from(def.upgrade_pct.1) {
        2
    } else {
        1
    }
}

/// The level the next sprint is recommended at after a run of `level` ended like this.
pub fn next_level(def: &SprintDef, result: SprintResult, level: u8, upgrade: i8) -> u8 {
    let max = def.max_level.max(1);
    match result {
        SprintResult::Delivered => level.saturating_add(upgrade.max(0) as u8).min(max),
        _ => level.saturating_sub(1).max(1),
    }
}

fn mmss(ms: u32) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The Sprint Review's retrospective: three sticky notes generated from the stats.
pub fn retro_lines(result: SprintResult, s: &SprintSummary) -> Vec<String> {
    let st = &s.status;
    let time = st.time_ms();
    let margin = i64::from(st.par_ms) - i64::from(time);
    let top = s.meter.iter().max_by_key(|m| m.damage);
    let healer = s
        .meter
        .iter()
        .max_by_key(|m| m.healing)
        .filter(|m| m.healing > 0);
    let well = match result {
        SprintResult::Abandoned => "What went well: nothing. Nobody saw anything.".to_string(),
        _ if st.deaths == 0 && margin > 0 => format!(
            "What went well: everything, {} under par and zero deaths. Suspicious. Audit it.",
            mmss(margin as u32)
        ),
        SprintResult::Delivered => format!(
            "What went well: delivered with {} to spare. Leadership will take the credit.",
            mmss(margin.max(0) as u32)
        ),
        _ if st.forces_pct < 100 => format!(
            "What went well: {}% of the scope. The rest is 'phase two'.",
            st.forces_pct
        ),
        _ => "What went well: nothing.".to_string(),
    };
    let improve = match (st.deaths, top) {
        (0, Some(m)) if m.damage > 0 => format!(
            "What to improve: {} carried the sprint ({} damage). Bus factor: 1.",
            class_name(&m.class),
            m.damage
        ),
        (0, _) => "What to improve: everything.".to_string(),
        (d, _) => format!(
            "What to improve: {d} death{} cost {} s. Have you tried not standing in it?",
            if d == 1 { "" } else { "s" },
            st.penalty_ms / 1000
        ),
    };
    let action = if result == SprintResult::Abandoned {
        "Action items: blame the process. Schedule a meeting about the meeting.".to_string()
    } else if margin < 0 {
        format!(
            "Action items: more AI. Also re-estimate: we were {} over.",
            mmss((-margin) as u32)
        )
    } else if let Some(h) = healer {
        format!(
            "Action items: more AI. And give {} a raise ({} healed). They will get a pizza.",
            class_name(&h.class),
            h.healing
        )
    } else {
        "Action items: more AI.".to_string()
    };
    vec![well, improve, action]
}

fn class_name(key: &str) -> String {
    match key {
        "fullstack" => "the Fullstack",
        "frontend" => "the Frontend",
        "backend" => "the Backend",
        "devops" => "the DevOps",
        _ => "someone",
    }
    .to_string()
}

// ---- runtime ----

pub(crate) struct Run {
    building: String,
    level: u8,
    affixes: Vec<Affix>,
    affix_keys: Vec<String>,
    stages: Vec<String>,
    stage: usize,
    /// Tick of the arrival in the first stage; the clock runs from there.
    started: Option<u64>,
    par_ms: u32,
    deaths: u32,
    trash_killed: u32,
    forces_need: u32,
    bosses_total: u8,
    bosses_killed: u8,
    stage_done: bool,
    ended: bool,
    /// The clock stops here once the run ended.
    ended_at: Option<u64>,
    rng: Rng,
    /// The campaign's world state, restored when the run ends.
    campaign: World,
    meter: BTreeMap<EntityId, (u64, u64)>,
    last_cast: BTreeMap<EntityId, SpellId>,
    hotfixed: BTreeSet<EntityId>,
    pairs: BTreeMap<EntityId, EntityId>,
    tech_debt: u32,
    /// Next firing of AI-Assisted, Reorg, Mandatory Fun and Burnout.
    next: [u64; 4],
}

#[derive(Default)]
pub(crate) struct SprintState {
    run: Option<Run>,
    /// The zone the host should load, and from which tick.
    travel: Option<(u64, String)>,
    keys: BTreeMap<EntityId, BTreeMap<String, u8>>,
    records: BTreeMap<EntityId, Vec<SprintRecord>>,
    runs: u64,
    splitting: bool,
    /// How the last run ended: result, time and upgrade.
    last: Option<(SprintResult, u32, i8)>,
}

impl Run {
    fn has(&self, a: Affix) -> bool {
        self.affixes.contains(&a)
    }
}

impl Sim {
    fn sprint_def(&self) -> &SprintDef {
        &self.content.sprint
    }

    fn live_run(&self) -> Option<&Run> {
        self.sprint.run.as_ref().filter(|r| !r.ended)
    }

    /// This session's seed (the sprint affix rotation key).
    pub fn session_seed(&self) -> u64 {
        self.seed
    }

    /// A run is going on (not yet ended).
    pub fn sprint_active(&self) -> bool {
        self.live_run().is_some()
    }

    /// The zone the host must load now to follow the run (the next stage, or campus after
    /// the review); `None` while nothing is due.
    pub fn sprint_travel(&self) -> Option<&str> {
        self.sprint
            .travel
            .as_ref()
            .filter(|(at, _)| self.now >= *at)
            .map(|(_, z)| z.as_str())
    }

    /// What the planning board offers for `building` at `level` this session.
    pub fn sprint_preview(&self, building: &str, level: u8) -> Option<SprintPreview> {
        preview(self.sprint_def(), self.seed, building, level)
    }

    /// How the most recent run of this session ended (result, time ms, upgrade).
    pub fn sprint_last_result(&self) -> Option<(SprintResult, u32, i8)> {
        self.sprint.last
    }

    pub fn sprint_status(&self) -> Option<SprintStatus> {
        self.sprint.run.as_ref().map(|r| self.status_of(r))
    }

    /// Recommended level of `player`'s next sprint of `building` (1 before the first run).
    pub fn sprint_key(&self, player: EntityId, building: &str) -> u8 {
        self.sprint
            .keys
            .get(&player)
            .and_then(|m| m.get(building))
            .copied()
            .unwrap_or(1)
    }

    /// `player`'s delivered sprints, best first per building.
    pub fn sprint_records(&self, player: EntityId) -> &[SprintRecord] {
        self.sprint.records.get(&player).map_or(&[], Vec::as_slice)
    }

    /// Level 6+ or the unlock boss killed once.
    pub fn sprint_unlocked(&self, player: EntityId) -> bool {
        let def = self.sprint_def();
        let world = self.campaign_world();
        self.unit(player)
            .is_some_and(|u| u.level >= def.unlock_level)
            || world.bosses_killed.contains(&def.unlock_boss)
    }

    /// The campaign's world state, also while a run has it parked.
    pub(crate) fn campaign_world(&self) -> &World {
        self.sprint
            .run
            .as_ref()
            .filter(|r| !r.ended)
            .map_or(&self.world, |r| &r.campaign)
    }

    pub(crate) fn sprint_has(&self, a: Affix) -> bool {
        self.live_run().is_some_and(|r| r.has(a))
    }

    pub(crate) fn sprint_keys_of(&self, player: EntityId) -> Vec<(String, u8)> {
        self.sprint
            .keys
            .get(&player)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default()
    }

    pub(crate) fn set_sprint_progress(
        &mut self,
        player: EntityId,
        keys: Vec<(String, u8)>,
        mut records: Vec<SprintRecord>,
    ) {
        let def = &self.content.sprint;
        let max = def.max_level.max(1);
        let keys: BTreeMap<String, u8> = keys
            .into_iter()
            .filter(|(b, _)| def.building(b).is_some())
            .map(|(b, l)| (b, l.clamp(1, max)))
            .collect();
        records.retain(|r| def.building(&r.building).is_some() && r.level >= 1);
        self.sprint.keys.insert(player, keys);
        self.sprint.records.insert(player, records);
        self.sort_records(player);
    }

    fn sort_records(&mut self, player: EntityId) {
        let Some(list) = self.sprint.records.get_mut(&player) else {
            return;
        };
        list.sort_by(|a, b| {
            a.building
                .cmp(&b.building)
                .then(b.level.cmp(&a.level))
                .then(a.time_ms.cmp(&b.time_ms))
        });
        let mut per: BTreeMap<String, usize> = BTreeMap::new();
        list.retain(|r| {
            let n = per.entry(r.building.clone()).or_default();
            *n += 1;
            *n <= RECORDS_PER_BUILDING
        });
    }

    fn status_of(&self, r: &Run) -> SprintStatus {
        let now = r.ended_at.unwrap_or(self.now);
        let elapsed = r
            .started
            .map_or(0, |t| ticks_to_ms(now.saturating_sub(t)) as u32);
        let forces = (u64::from(r.trash_killed) * 100 / u64::from(r.forces_need.max(1))).min(100);
        SprintStatus {
            building: r.building.clone(),
            level: r.level,
            affixes: r.affix_keys.clone(),
            par_ms: r.par_ms,
            elapsed_ms: elapsed,
            penalty_ms: r
                .deaths
                .saturating_mul(self.content.sprint.death_penalty_ms),
            forces_pct: forces as u8,
            bosses_left: r.bosses_total.saturating_sub(r.bosses_killed),
            bosses_total: r.bosses_total,
            stage: (r.stage + 1).min(r.stages.len()) as u8,
            stages: r.stages.len() as u8,
            deaths: r.deaths.min(u32::from(u16::MAX)) as u16,
            tech_debt: r.tech_debt.min(255) as u8,
        }
    }

    // ---- start ----

    pub(crate) fn start_sprint(&mut self, actor: EntityId, building: &str, level: u8) {
        if let Err(e) = self.check_start(actor, building, level) {
            self.emit_to(actor, Event::SprintRefused { reason: e });
            return;
        }
        let def = &self.content.sprint;
        let Some(b) = def.building(building).cloned() else {
            return;
        };
        self.sprint.runs += 1;
        let mut rng = Rng::new(self.seed ^ 0x5B21_7000_0000 ^ self.sprint.runs.rotate_left(17));
        // Draw `pick` stages, keeping the building's order (floors still go up).
        let mut idx: Vec<usize> = (0..b.zones.len()).collect();
        while idx.len() > usize::from(b.pick) {
            let k = rng.below(idx.len() as u32) as usize;
            idx.remove(k);
        }
        let stages: Vec<String> = idx.iter().map(|&k| b.zones[k].clone()).collect();
        let trash: u32 = stages
            .iter()
            .filter_map(|z| self.content.zone(z))
            .map(|z| z.packs.iter().map(|p| p.units.len() as u32).sum::<u32>())
            .sum();
        let bosses = stages
            .iter()
            .filter(|z| self.content.zone(z).is_some_and(|d| d.boss.is_some()))
            .count() as u8;
        let affix_keys = affixes_for(def, self.seed, level);
        let affixes = affix_keys
            .iter()
            .filter_map(|k| Affix::from_key(k))
            .collect();
        let need = (u64::from(trash) * u64::from(def.forces_need_pct)).div_ceil(100) as u32;
        let par = par_ms(def, &b, level);
        let delay = ticks(def.travel_delay_ms);
        // The run plays in a world of its own; zone ids stay "announced" so no unlock banners.
        let mut fresh = World::default();
        fresh
            .announced
            .extend(self.content.zones.iter().map(|z| z.id.clone()));
        let campaign = std::mem::replace(&mut self.world, fresh);
        let run = Run {
            building: b.key.clone(),
            level,
            affixes,
            affix_keys,
            stages: stages.clone(),
            stage: 0,
            started: None,
            par_ms: par,
            deaths: 0,
            trash_killed: 0,
            forces_need: need.max(1),
            bosses_total: bosses,
            bosses_killed: 0,
            stage_done: false,
            ended: false,
            ended_at: None,
            rng,
            campaign,
            meter: BTreeMap::new(),
            last_cast: BTreeMap::new(),
            hotfixed: BTreeSet::new(),
            pairs: BTreeMap::new(),
            tech_debt: 0,
            next: [u64::MAX; 4],
        };
        let status = self.status_of(&run);
        self.sprint.run = Some(run);
        self.sprint.travel = Some((self.now + delay, stages[0].clone()));
        self.events.push(Event::SprintStarted {
            status,
            zones: stages,
        });
    }

    fn check_start(&self, actor: EntityId, building: &str, level: u8) -> Result<(), SprintError> {
        let def = self.sprint_def();
        if self.sprint.run.is_some() {
            return Err(SprintError::AlreadyRunning);
        }
        if self.players.first() != Some(&actor) {
            return Err(SprintError::NotLeader);
        }
        if def.building(building).is_none() {
            return Err(SprintError::UnknownBuilding);
        }
        if level == 0 || level > def.max_level {
            return Err(SprintError::BadLevel);
        }
        if !self.sprint_unlocked(actor) {
            return Err(SprintError::Locked);
        }
        if self.zone.as_deref() != Some("campus") {
            return Err(SprintError::NotInCampus);
        }
        let members: Vec<&crate::types::UnitState> =
            self.players.iter().filter_map(|p| self.unit(*p)).collect();
        if members.iter().any(|u| !u.alive) {
            return Err(SprintError::Dead);
        }
        if members.iter().any(|u| u.in_combat) {
            return Err(SprintError::InCombat);
        }
        Ok(())
    }

    // ---- zone loads ----

    /// Called by `load_zone` before it spawns anything: the expected stage continues the run,
    /// any other zone abandons it (or closes an ended one).
    pub(crate) fn sprint_on_load(&mut self, id: &str) {
        let Some(run) = self.sprint.run.as_mut() else {
            self.sprint.travel = None;
            return;
        };
        let expected = self.sprint.travel.take().map(|t| t.1);
        if run.ended {
            self.sprint.run = None;
            return;
        }
        if expected.as_deref() == Some(id)
            && let Some(k) = run.stages.iter().position(|s| s == id)
        {
            run.stage = k;
            run.stage_done = false;
            run.hotfixed.clear();
            run.pairs.clear();
            let now = self.now;
            run.started.get_or_insert(now);
            return;
        }
        self.end_sprint(SprintResult::Abandoned);
    }

    /// Spawn level inside a sprint stage (sprint mob level at least); `level` otherwise.
    pub(crate) fn sprint_level(&self, level: u8) -> u8 {
        if self.live_run().is_some() {
            level.max(self.sprint_def().mob_level)
        } else {
            level
        }
    }

    /// Enemy (HP %, damage %) for the run's level; 100/100 outside a run.
    pub(crate) fn sprint_mob_pct(&self, i: usize) -> (u32, u32) {
        let Some(r) = self.live_run() else {
            return (100, 100);
        };
        let d = &self.content.units[self.units[i].def];
        if !d.hostile || d.ai.is_none() || self.units[i].class.is_some() {
            return (100, 100);
        }
        let def = self.sprint_def();
        let lv = u32::from(r.level.saturating_sub(1));
        (
            100 + def.hp_pct_per_level * lv,
            100 + def.damage_pct_per_level * lv,
        )
    }

    /// Exits of a sprint stage: only the way back to campus (which abandons) stays open.
    pub(crate) fn sprint_link_open(&self, to: &str) -> Option<bool> {
        self.live_run().map(|_| to == "campus")
    }

    /// After a stage's units exist: Pair Programming links, the Legacy Code elite, timers.
    pub(crate) fn sprint_after_load(&mut self) {
        let Some(run) = self.sprint.run.as_ref().filter(|r| !r.ended) else {
            return;
        };
        let (pairs, legacy) = (run.has(Affix::PairProgramming), run.has(Affix::LegacyCode));
        let def = self.content.sprint.clone();
        let period = |k: &str| {
            def.affixes
                .iter()
                .find(|a| a.key == k)
                .map_or(u64::MAX, |a| ticks(a.period_ms.max(1000)))
        };
        let now = self.now;
        let next = [
            now.saturating_add(period("ai_assisted")),
            now.saturating_add(period("reorg")),
            now.saturating_add(period("mandatory_fun")),
            now.saturating_add(period("burnout")),
        ];
        if let Some(r) = self.sprint.run.as_mut() {
            r.next = next;
        }
        let packs: Vec<(u32, crate::geom::Vec3, f32)> = self
            .zone_packs
            .iter()
            .map(|p| (p.id, p.at, p.yaw))
            .collect();
        if pairs {
            for &(pack, _, _) in &packs {
                let mut members: Vec<usize> = (0..self.units.len())
                    .filter(|&i| {
                        let u = &self.units[i];
                        !u.despawned && u.state.alive && u.state.pack == Some(pack)
                    })
                    .collect();
                if members.len() < 2 {
                    continue;
                }
                members.sort_by_key(|&i| std::cmp::Reverse(self.units[i].state.max_hp));
                let (a, b) = (members[0], members[1]);
                let (ia, ib) = (Self::id_of(a), Self::id_of(b));
                if let Some(r) = self.sprint.run.as_mut() {
                    r.pairs.insert(ia, ib);
                    r.pairs.insert(ib, ia);
                }
                self.apply_aura(a, PAIR_AURA, PERMANENT_MS, None, ia, None);
                self.apply_aura(b, PAIR_AURA, PERMANENT_MS, None, ib, None);
            }
        }
        if legacy && !packs.is_empty() {
            let k = match self.sprint.run.as_mut() {
                Some(r) => r.rng.below(packs.len() as u32) as usize,
                None => 0,
            };
            let (pack, at, yaw) = packs[k];
            if let Ok(d) = self.def_index(LEGACY_UNIT) {
                let level = self.sprint_level(self.content.units[d].level);
                let pos = crate::geom::Vec3::new(at.x, at.y, at.z - 3.0);
                let id = self.spawn(
                    d,
                    pos,
                    yaw,
                    SpawnOpts {
                        level: Some(level),
                        pack: Some(pack),
                        respawns: false,
                        summoner: None,
                        floor_boss: false,
                        zone_slot: None,
                    },
                );
                if let Some(i) = self.index(id) {
                    self.apply_aura(i, LEGACY_AURA, PERMANENT_MS, None, id, None);
                }
            }
        }
        self.emit_status();
    }

    // ---- damage ----

    /// Technical Debt on enemy hits; Pair Programming sends half of a hit to the partner.
    pub(crate) fn sprint_damage(
        &mut self,
        source: EntityId,
        ti: usize,
        amount: u32,
        spell: Option<SpellId>,
        school: School,
    ) -> u32 {
        let Some(r) = self.live_run() else {
            return amount;
        };
        let target = Self::id_of(ti);
        let hostile_source = self
            .index(source)
            .is_some_and(|si| self.units[si].state.hostile);
        let mut amount = amount;
        if hostile_source && self.is_player(target) && r.has(Affix::TechnicalDebt) {
            let pct = 100 + u64::from(r.tech_debt) * u64::from(self.content.sprint.tech_debt_pct);
            amount = (u64::from(amount) * pct / 100) as u32;
        }
        let partner = r
            .pairs
            .get(&target)
            .copied()
            .filter(|p| self.unit(*p).is_some_and(|u| u.alive));
        if let Some(p) = partner
            && !self.sprint.splitting
            && amount > 1
        {
            let share = amount / 2;
            self.sprint.splitting = true;
            self.deal_damage(
                source,
                p,
                Hit {
                    spell,
                    amount: share,
                    crit: false,
                    combo: false,
                    school,
                    dot: true,
                },
            );
            self.sprint.splitting = false;
            amount -= share;
        }
        amount
    }

    // ---- per tick ----

    pub(crate) fn update_sprint(&mut self) {
        if self.sprint.run.as_ref().is_none_or(|r| r.ended) {
            return;
        }
        self.scan_sprint_events();
        if self.sprint.run.as_ref().is_none_or(|r| r.started.is_none()) {
            return;
        }
        // A stage's units only exist once the host loaded it.
        let in_stage = self
            .sprint
            .run
            .as_ref()
            .is_some_and(|r| self.zone.as_deref() == r.stages.get(r.stage).map(String::as_str));
        if !in_stage || self.sprint.travel.is_some() {
            if self.now.is_multiple_of(STATUS_EVERY_TICKS) {
                self.emit_status();
            }
            return;
        }
        self.run_affixes();
        if self.now.is_multiple_of(STATUS_EVERY_TICKS) {
            self.check_stage();
            if self.sprint.run.as_ref().is_some_and(|r| !r.ended) {
                self.emit_status();
            }
        }
    }

    fn emit_status(&mut self) {
        if let Some(r) = self.sprint.run.as_ref() {
            let status = self.status_of(r);
            self.events.push(Event::SprintProgress { status });
        }
    }

    /// Deaths, kills, the meter, last casts and kicks from this tick's events.
    fn scan_sprint_events(&mut self) {
        let mut deaths: Vec<EntityId> = Vec::new();
        let mut kicked: Vec<EntityId> = Vec::new();
        let mut damage: Vec<(EntityId, u32, bool)> = Vec::new();
        let mut casts: Vec<(EntityId, SpellId)> = Vec::new();
        for e in &self.events {
            match e {
                Event::Death { unit, .. } => deaths.push(*unit),
                Event::Damage { source, amount, .. } if self.is_player(*source) => {
                    damage.push((*source, *amount, false))
                }
                Event::Heal { source, amount, .. } if self.is_player(*source) => {
                    damage.push((*source, *amount, true))
                }
                Event::CastSucceeded { caster, spell, .. } if self.is_player(*caster) => {
                    casts.push((*caster, *spell))
                }
                Event::CastInterrupted {
                    caster,
                    by: Some(_),
                    lockout_ms,
                    ..
                } if *lockout_ms > 0 => kicked.push(*caster),
                _ => {}
            }
        }
        if let Some(r) = self.sprint.run.as_mut() {
            for (p, n, heal) in damage {
                let m = r.meter.entry(p).or_default();
                if heal {
                    m.1 += u64::from(n);
                } else {
                    m.0 += u64::from(n);
                }
            }
            for (p, s) in casts {
                r.last_cast.insert(p, s);
            }
        }
        for id in kicked {
            let Some(i) = self.index(id) else { continue };
            if self.units[i].auras.iter().any(|a| a.key == LEGACY_AURA)
                && self.remove_aura(i, LEGACY_AURA, AuraEnd::Consumed)
            {
                self.affix_fired(
                    "legacy_code",
                    id,
                    "Somebody touched the legacy code. It is mortal now. So is the on-call.",
                );
            }
        }
        for id in deaths {
            self.sprint_death(id);
        }
    }

    fn sprint_death(&mut self, id: EntityId) {
        let Some(i) = self.index(id) else { return };
        if self.is_player(id) {
            let debt = self.sprint_has(Affix::TechnicalDebt);
            if let Some(r) = self.sprint.run.as_mut() {
                r.deaths += 1;
                if debt {
                    r.tech_debt = (r.tech_debt + 1).min(TECH_DEBT_MAX);
                }
            }
            if debt {
                self.affix_fired(
                    "technical_debt",
                    id,
                    "A death was logged as technical debt. Enemies hit harder. Interest compounds.",
                );
            }
            return;
        }
        let d = &self.content.units[self.units[i].def];
        if !d.hostile || d.ai.is_none() {
            return;
        }
        let elite = d.elite
            || d.boss
            || d.key == LEGACY_UNIT
            || self.content.economy.elites.contains(&d.key);
        let trash = self.units[i].zone_slot.is_some();
        let boss = self.units[i].floor_boss;
        let pack = self.units[i].state.pack;
        let pos = self.units[i].state.pos;
        if let Some(r) = self.sprint.run.as_mut() {
            if trash {
                r.trash_killed += 1;
            }
            if boss {
                r.bosses_killed += 1;
            }
        }
        if elite && self.sprint_has(Affix::Burnout) {
            let mut cleared = false;
            for p in self.players.clone() {
                if let Some(pi) = self.index(p) {
                    cleared |= self.remove_aura(pi, BURNOUT_AURA, AuraEnd::Consumed);
                }
            }
            if cleared {
                self.affix_fired(
                    "burnout",
                    id,
                    "An elite went down. Burnout reset. Management calls it 'resilience'.",
                );
            }
        }
        let zone_pack = pack.is_some_and(|p| self.zone_packs.iter().any(|z| z.id == p));
        let wiped = zone_pack
            && !self
                .units
                .iter()
                .any(|u| !u.despawned && u.state.alive && u.state.pack == pack);
        if wiped && self.sprint_has(Affix::ScopeCreep) {
            self.spawn_scope_creep(pos);
        }
    }

    fn spawn_scope_creep(&mut self, pos: crate::geom::Vec3) {
        let Ok(d) = self.def_index(SCOPE_CREEP_UNIT) else {
            return;
        };
        let level = self.sprint_level(self.content.units[d].level);
        let pack = self.next_pack;
        self.next_pack += 1;
        let id = self.spawn(
            d,
            pos,
            0.0,
            SpawnOpts {
                level: Some(level),
                pack: Some(pack),
                respawns: false,
                summoner: None,
                floor_boss: false,
                zone_slot: None,
            },
        );
        let near = self.nearest_player(pos);
        if let (Some(i), Some(p)) = (self.index(id), near) {
            self.add_threat(i, p, 1);
            self.set_ai(i, AiState::Chase);
        }
        self.affix_fired(
            "scope_creep",
            id,
            "Pack done? Great. The client has one small additional requirement.",
        );
    }

    fn nearest_player(&self, pos: crate::geom::Vec3) -> Option<EntityId> {
        self.players
            .iter()
            .filter_map(|p| self.unit(*p))
            .filter(|u| u.alive)
            .min_by(|a, b| {
                crate::geom::distance(a.pos, pos).total_cmp(&crate::geom::distance(b.pos, pos))
            })
            .map(|u| u.id)
    }

    fn affix_fired(&mut self, affix: &str, unit: EntityId, text: &str) {
        self.events.push(Event::SprintAffix {
            affix: affix.to_string(),
            unit,
            text: text.to_string(),
        });
    }

    fn affix_power(&self, key: &str) -> u32 {
        self.content.sprint.affix(key).map_or(0, |a| a.power_pct)
    }

    fn affix_period(&self, key: &str) -> u64 {
        self.content
            .sprint
            .affix(key)
            .map_or(u64::MAX, |a| ticks(a.period_ms.max(1000)))
    }

    fn fighting_players(&self) -> Vec<usize> {
        self.players
            .iter()
            .filter_map(|p| self.index(*p))
            .filter(|&i| self.units[i].state.alive && self.units[i].state.in_combat)
            .collect()
    }

    fn fighting_enemies(&self) -> Vec<usize> {
        (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                !u.despawned
                    && u.state.alive
                    && u.state.hostile
                    && u.state.in_combat
                    && u.state.ai != Some(AiState::Evade)
                    && u.class.is_none()
            })
            .collect()
    }

    fn run_affixes(&mut self) {
        let Some(r) = self.sprint.run.as_ref() else {
            return;
        };
        let affixes = r.affixes.clone();
        let next = r.next;
        let now = self.now;
        let fighting = !self.fighting_players().is_empty();
        for a in affixes {
            match a {
                Affix::AiAssisted if fighting && now >= next[0] => {
                    self.set_next(0, "ai_assisted");
                    self.ai_assist();
                }
                Affix::Reorg if now >= next[1] => {
                    self.set_next(1, "reorg");
                    self.reorg();
                }
                Affix::MandatoryFun if fighting && now >= next[2] => {
                    self.set_next(2, "mandatory_fun");
                    self.mandatory_fun();
                }
                Affix::Burnout if now >= next[3] => {
                    self.set_next(3, "burnout");
                    self.burnout();
                }
                Affix::HotfixFriday if now.is_multiple_of(10) => self.hotfix_friday(),
                _ => {}
            }
        }
    }

    fn set_next(&mut self, k: usize, key: &str) {
        let at = self.now.saturating_add(self.affix_period(key));
        if let Some(r) = self.sprint.run.as_mut() {
            r.next[k] = at;
        }
    }

    fn pick(&mut self, n: usize) -> usize {
        match self.sprint.run.as_mut() {
            Some(r) => r.rng.below(n as u32) as usize,
            None => 0,
        }
    }

    /// An enemy in the fight replays the last spell a player cast, as a hit on that player.
    fn ai_assist(&mut self) {
        let enemies = self.fighting_enemies();
        if enemies.is_empty() {
            return;
        }
        let e = enemies[self.pick(enemies.len())];
        let victim = self.units[e]
            .state
            .target
            .filter(|t| self.is_player(*t))
            .or_else(|| self.nearest_player(self.units[e].state.pos));
        let Some(victim) = victim else { return };
        let Some(spell) = self
            .sprint
            .run
            .as_ref()
            .and_then(|r| r.last_cast.get(&victim).copied())
            .and_then(|s| self.content.spell(s).cloned())
        else {
            return;
        };
        let Some(vi) = self.index(victim) else { return };
        let (_, dmg_pct) = self.sprint_mob_pct(e);
        let amount = (u64::from(self.units[vi].state.max_hp)
            * u64::from(self.affix_power("ai_assisted"))
            * u64::from(dmg_pct)
            / 10_000)
            .max(1) as u32;
        let source = Self::id_of(e);
        let name = self.units[e].state.name.clone();
        self.affix_fired(
            "ai_assisted",
            source,
            &format!(
                "{name} asked an AI to regenerate your {}. It mostly compiles.",
                spell.name
            ),
        );
        self.deal_damage(
            source,
            victim,
            Hit {
                spell: Some(spell.id),
                amount,
                crit: false,
                combo: false,
                school: spell.school,
                dot: false,
            },
        );
    }

    /// Idle packs swap their spots.
    fn reorg(&mut self) {
        let idle: Vec<usize> = (0..self.zone_packs.len())
            .filter(|&k| {
                let id = self.zone_packs[k].id;
                let members: Vec<&crate::sim::Unit> = self
                    .units
                    .iter()
                    .filter(|u| !u.despawned && u.state.alive && u.state.pack == Some(id))
                    .collect();
                !members.is_empty()
                    && members
                        .iter()
                        .all(|u| !u.state.in_combat && u.threat.is_empty())
            })
            .collect();
        if idle.len() < 2 {
            return;
        }
        let mut spots: Vec<crate::geom::Vec3> =
            idle.iter().map(|&k| self.zone_packs[k].at).collect();
        // Rotate by a random non-zero offset so every pack really moves.
        let shift = 1 + self.pick(spots.len() - 1);
        spots.rotate_left(shift);
        for (&k, to) in idle.iter().zip(spots) {
            let from = self.zone_packs[k].at;
            let id = self.zone_packs[k].id;
            self.zone_packs[k].at = to;
            let (dx, dz) = (to.x - from.x, to.z - from.z);
            for u in self.units.iter_mut().filter(|u| u.state.pack == Some(id)) {
                u.state.pos.x += dx;
                u.state.pos.z += dz;
                u.spawn_pos.x += dx;
                u.spawn_pos.z += dz;
                u.nav = crate::nav::NavAgent::default();
            }
        }
        self.affix_fired(
            "reorg",
            0,
            &format!(
                "Reorg! {} teams were moved to new desks. Nobody knows where anyone sits now.",
                idle.len()
            ),
        );
    }

    /// A shared circle on a fighting player: stand in it together.
    fn mandatory_fun(&mut self) {
        let players = self.fighting_players();
        let enemies = self.fighting_enemies();
        if players.is_empty() || enemies.is_empty() {
            return;
        }
        let p = players[self.pick(players.len())];
        let e = enemies[self.pick(enemies.len())];
        let Some(spell) = self
            .content
            .spell(SpellId(self.content.sprint.mandatory_fun_spell))
            .cloned()
        else {
            return;
        };
        self.start_telegraphs(e, &spell, Some(p));
        self.affix_fired(
            "mandatory_fun",
            Self::id_of(p),
            "Mandatory Fun! Team building in the circle. Attendance is tracked. Stack up.",
        );
    }

    /// Every living player loses a slice of max HP until an elite dies.
    fn burnout(&mut self) {
        for p in self.players.clone() {
            if let Some(i) = self.index(p).filter(|&i| self.units[i].state.alive) {
                self.apply_aura(i, BURNOUT_AURA, PERMANENT_MS, None, p, None);
            }
        }
        self.affix_fired(
            "burnout",
            0,
            "Another minute of crunch. Max HP down. Kill an elite to take a mental health day.",
        );
    }

    /// Trash under the threshold heals once (bosses and elites do not touch code).
    fn hotfix_friday(&mut self) {
        let pct = self.affix_power("hotfix_friday");
        let low: Vec<usize> = (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                !u.despawned
                    && u.state.alive
                    && u.state.hostile
                    && u.class.is_none()
                    && self.content.units[u.def].ai.is_some()
                    && !self.content.units[u.def].boss
                    && !self.content.units[u.def].elite
                    && !self.content.economy.elites.contains(&u.state.key)
                    && u.state.hp * 100 < u.state.max_hp * HOTFIX_BELOW_PCT
            })
            .collect();
        for i in low {
            let id = Self::id_of(i);
            let fresh = self
                .sprint
                .run
                .as_mut()
                .is_some_and(|r| r.hotfixed.insert(id));
            if !fresh {
                continue;
            }
            let amount = (u64::from(self.units[i].state.max_hp) * u64::from(pct) / 100) as u32;
            self.heal(id, i, None, amount);
            let name = self.units[i].state.name.clone();
            self.affix_fired(
                "hotfix_friday",
                id,
                &format!("{name} pushed a hotfix to prod on a Friday. It worked. This time."),
            );
        }
    }

    // ---- stages and the end ----

    fn check_stage(&mut self) {
        let Some(r) = self.sprint.run.as_ref() else {
            return;
        };
        let zone = r.stages.get(r.stage).cloned().unwrap_or_default();
        let next = r.stages.get(r.stage + 1).cloned();
        let (stage_done, forces_ok) = (r.stage_done, r.trash_killed >= r.forces_need);
        let status = self.status_of(r);
        let has_boss = self.content.zone(&zone).is_some_and(|z| z.boss.is_some());
        let trash_left = self
            .units
            .iter()
            .any(|u| !u.despawned && u.state.alive && u.zone_slot.is_some());
        if !stage_done {
            let done = if has_boss {
                self.world.cleared.contains(&zone)
            } else {
                !trash_left
            };
            if !done {
                return;
            }
            let delay = ticks(self.content.sprint.travel_delay_ms);
            if let Some(r) = self.sprint.run.as_mut() {
                r.stage_done = true;
            }
            if let Some(next) = next {
                self.sprint.travel = Some((self.now + delay, next));
                return;
            }
        } else if next.is_some() {
            return;
        }
        if forces_ok || !trash_left {
            let result = if forces_ok && status.time_ms() <= status.par_ms {
                SprintResult::Delivered
            } else {
                SprintResult::CarriedOver
            };
            self.end_sprint(result);
        }
    }

    /// Ends the run: keys, records, the chest, the review event; the campaign world comes back.
    fn end_sprint(&mut self, result: SprintResult) {
        let Some(r) = self.sprint.run.as_ref() else {
            return;
        };
        let status = self.status_of(r);
        let def = self.content.sprint.clone();
        let time = status.time_ms();
        let upgrade = upgrade_for(&def, result, time, status.par_ms);
        let next = next_level(&def, result, r.level, upgrade);
        let building = r.building.clone();
        let level = r.level;
        let meter_raw = r.meter.clone();
        let players = self.players.clone();
        let mut meter = Vec::new();
        let mut rewards = Vec::new();
        for &p in &players {
            let class = self
                .index(p)
                .and_then(|i| self.class_at(i))
                .map(|c| c.key.clone())
                .unwrap_or_default();
            let (damage, healing) = meter_raw.get(&p).copied().unwrap_or_default();
            meter.push(MeterLine {
                unit: p,
                class: class.clone(),
                damage,
                healing,
            });
            self.sprint
                .keys
                .entry(p)
                .or_default()
                .insert(building.clone(), next);
            if result == SprintResult::Delivered {
                self.sprint
                    .records
                    .entry(p)
                    .or_default()
                    .push(SprintRecord {
                        building: building.clone(),
                        level,
                        time_ms: time,
                        class,
                        name: String::new(),
                    });
                self.sort_records(p);
            }
            if result != SprintResult::Abandoned {
                rewards.push(self.grant_chest(p, level, result));
            }
        }
        meter.sort_by_key(|m| std::cmp::Reverse(m.damage + m.healing));
        self.sprint.last = Some((result, time, upgrade));
        if let Some(r) = self.sprint.run.as_mut() {
            r.ended = true;
            r.ended_at = Some(self.now);
            self.world = std::mem::take(&mut r.campaign);
        }
        if result == SprintResult::Abandoned {
            self.sprint.run = None;
            self.sprint.travel = None;
        } else {
            self.sprint.travel = Some((self.now + ticks(def.return_delay_ms), "campus".into()));
        }
        self.events.push(Event::SprintEnded {
            result,
            time_ms: time,
            upgrade,
            summary: SprintSummary {
                status,
                meter,
                rewards,
                next_level: next,
            },
        });
    }

    /// The end-of-sprint chest for one player: Story Points and top-tier gear whose rarity
    /// climbs with the level, sometimes a named piece of one of the building's bosses.
    fn grant_chest(&mut self, p: EntityId, level: u8, result: SprintResult) -> RewardLine {
        let def = self.content.sprint.reward.clone();
        let freeze = self.sprint_has(Affix::BudgetFreeze);
        let mut sp = if freeze {
            0
        } else {
            def.sp_per_level * u32::from(level)
        };
        let mut n = def.items + u32::from(level) / u32::from(def.items_per_levels.max(1));
        if freeze {
            n += 1;
        }
        if result == SprintResult::CarriedOver {
            sp /= 2;
            n = n.saturating_sub(1).max(1);
        }
        let Some(pi) = self.index(p) else {
            return RewardLine {
                unit: p,
                items: Vec::new(),
                sp: 0,
            };
        };
        let role = self.class_at(pi).map(|c| c.role);
        let bosses: Vec<String> = self
            .sprint
            .run
            .as_ref()
            .map(|r| {
                r.stages
                    .iter()
                    .filter_map(|z| self.content.zone(z)?.boss.as_ref().map(|b| b.unit.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let eco = &self.content.economy;
        let tier = eco.gear.iter().map(|g| g.tier()).max().unwrap_or(1);
        let rarity = if level >= 7 {
            Rarity::Epic
        } else if level >= 3 {
            Rarity::Rare
        } else {
            Rarity::Uncommon
        };
        let mut items = Vec::new();
        for _ in 0..n {
            let mut picked = None;
            for r in Rarity::ALL[..=rarity as usize].iter().rev() {
                let pool: Vec<&String> = eco
                    .gear
                    .iter()
                    .filter(|g| {
                        g.source == GearSource::World
                            && g.tier() == tier
                            && g.rarity == *r
                            && (g.role.is_none() || g.role == role)
                            && !items.contains(&g.name)
                    })
                    .map(|g| &g.name)
                    .collect();
                if !pool.is_empty() {
                    let k = self.loot_rng.below(pool.len() as u32) as usize;
                    picked = Some(pool[k].clone());
                    break;
                }
            }
            items.extend(picked);
        }
        let named: Vec<String> = eco
            .gear
            .iter()
            .filter(|g| matches!(&g.source, GearSource::Boss(k) if bosses.contains(k)))
            .filter(|g| g.role.is_none() || g.role == role)
            .map(|g| g.name.clone())
            .collect();
        if !named.is_empty()
            && self
                .loot_rng
                .chance_pct(def.legendary_pct_per_level * u32::from(level))
        {
            let k = self.loot_rng.below(named.len() as u32) as usize;
            items.push(named[k].clone());
        }
        let left = self.stow(pi, items.clone());
        if !left.is_empty() {
            self.item_error(p, crate::types::ItemError::BagFull);
        }
        items.retain(|i| !left.contains(i));
        self.units[pi].state.sp = self.units[pi].state.sp.saturating_add(sp);
        self.bag_event(pi);
        RewardLine { unit: p, items, sp }
    }

    /// QA: ends the running sprint now as if its last boss just died.
    pub fn debug_end_sprint(&mut self, delivered: bool) -> bool {
        if !self.sprint_active() {
            return false;
        }
        if let Some(r) = self.sprint.run.as_mut() {
            r.trash_killed = r.forces_need;
            r.bosses_killed = r.bosses_total;
        }
        self.end_sprint(if delivered {
            SprintResult::Delivered
        } else {
            SprintResult::CarriedOver
        });
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn def() -> SprintDef {
        let affix = |key: &str, tier: u8| AffixDef {
            key: key.into(),
            name: key.into(),
            icon: String::new(),
            text: String::new(),
            tier,
            period_ms: 0,
            power_pct: 0,
        };
        SprintDef {
            affix_levels: vec![2, 4, 7, 10],
            affixes: vec![
                affix("ai_assisted", 0),
                affix("scope_creep", 0),
                affix("reorg", 1),
                affix("burnout", 1),
                affix("mandatory_fun", 2),
                affix("technical_debt", 3),
            ],
            upgrade_pct: (60, 80),
            max_level: 20,
            par_pct_per_level: 2,
            ..SprintDef::default()
        }
    }

    #[test]
    fn affixes_unlock_at_their_levels_and_follow_the_seed() {
        let d = def();
        assert!(affixes_for(&d, 1, 1).is_empty());
        assert_eq!(affixes_for(&d, 1, 3).len(), 1);
        assert_eq!(affixes_for(&d, 1, 6).len(), 2);
        assert_eq!(affixes_for(&d, 1, 9).len(), 3);
        let ten = affixes_for(&d, 1, 10);
        assert_eq!(ten.len(), 4);
        assert_eq!(ten[3], "technical_debt");
        // Same seed, same week; the lower levels are a prefix of the higher ones.
        assert_eq!(affixes_for(&d, 1, 10)[..2], affixes_for(&d, 1, 5)[..]);
        let firsts: BTreeSet<String> = (0..32).map(|s| affixes_for(&d, s, 2)[0].clone()).collect();
        assert_eq!(firsts.len(), 2, "both tier-0 affixes rotate in");
    }

    #[test]
    fn upgrades_follow_the_margin_and_failures_carry_over() {
        let d = def();
        let up = |t| upgrade_for(&d, SprintResult::Delivered, t, 1000);
        assert_eq!(
            (up(500), up(600), up(700), up(800), up(1000)),
            (3, 3, 2, 2, 1)
        );
        assert_eq!(upgrade_for(&d, SprintResult::CarriedOver, 100, 1000), 0);
        assert_eq!(next_level(&d, SprintResult::Delivered, 4, 3), 7);
        assert_eq!(next_level(&d, SprintResult::Delivered, 19, 3), 20);
        assert_eq!(next_level(&d, SprintResult::CarriedOver, 4, 0), 3);
        assert_eq!(next_level(&d, SprintResult::Abandoned, 1, 0), 1);
    }

    #[test]
    fn par_grows_with_the_level() {
        let d = def();
        let b = SprintBuilding {
            key: "x".into(),
            name: "x".into(),
            zones: vec!["z".into()],
            pick: 1,
            par_ms: 100_000,
        };
        assert_eq!(par_ms(&d, &b, 1), 100_000);
        assert_eq!(par_ms(&d, &b, 6), 110_000);
    }

    #[test]
    fn the_retro_is_honest() {
        let mut s = SprintSummary::default();
        s.status.par_ms = 600_000;
        s.status.elapsed_ms = 700_000;
        s.status.deaths = 2;
        s.status.penalty_ms = 10_000;
        s.status.forces_pct = 100;
        let lines = retro_lines(SprintResult::CarriedOver, &s);
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("What went well: nothing"));
        assert!(lines[1].contains("2 deaths cost 10 s"));
        assert!(lines[2].starts_with("Action items: more AI"));
        assert!(lines[2].contains("1:50 over"));
    }
}
