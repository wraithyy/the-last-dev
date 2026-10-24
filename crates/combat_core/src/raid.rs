//! Raids ("All-Hands Offsite", "VibeCon"): each a chain of zones (`RaidDef::zones`) reached
//! from the campus at the level cap, one boss encounter per zone. Two difficulties: Normal for
//! 3-5 players and Async Attendance (picked automatically for smaller groups) with weaker
//! enemies and less loot. Every encounter is locked per difficulty for the ISO week once killed
//! (per room online, per save offline); a locked boss does not come back until the reset. Boss
//! keys are unique across raids, so one `Lockout` holds every raid's encounters.
//! Encounter mechanics that the generic AI cannot express live here: "Pair Up" tethers
//! (`Effect::PairUp`) and linked bosses (the Synergy Twins, the Hype Panel) with one health bar
//! and a rotating shield. `Effect::Detonate` (VibeCon's unreviewed PRs) lives in `effects.rs`.
//! Content: `raids` in `game.ron`; numbers in docs/FORMULAS.md.

use crate::content::{Content, ContentError, School, SpellId, ticks};
use crate::legacy::Lockout;
use crate::sim::{Hit, Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::types::{AiState, AuraEnd, Event};
use serde::{Deserialize, Serialize};

/// Lockout and achievement key prefix of Async Attendance kills.
pub const ASYNC_PREFIX: &str = "async:";
/// The same for Story difficulty in an Ilmari wing, which keeps its own weekly lockout.
pub const STORY_PREFIX: &str = "story:";
/// And for Judgment Day (`judgment.rs`), the third Ilmari difficulty.
pub const JUDGMENT_PREFIX: &str = "jd:";
/// Key prefix of a dungeon's daily clear counter, kept in the same daily lockout as its bosses.
const CLEAR_PREFIX: &str = "clear:";
/// Tethers check their range this often.
const TETHER_TICK_MS: u32 = 1000;
/// Long enough to outlast any fight; the twin swap moves the aura itself.
const SHIELD_MS: u32 = 3_600_000;

fn yes() -> bool {
    true
}

/// Linked bosses: one shared health bar (proportional), one death, and every member but one
/// shielded; each swap hands the opening to the next member in order.
#[derive(Clone, Debug, Deserialize)]
pub struct TwinsDef {
    /// The zone boss (loot, lockout) and its linked partner, spawned beside it.
    pub boss: String,
    pub twin: String,
    /// Further linked members (the Hype Panel's third panelist), spawned on the other side.
    #[serde(default)]
    pub extra: Vec<String>,
    /// Carried by every member except the open one; the swap moves the opening on.
    pub shield_aura: String,
    pub swap_ms: u32,
    /// Shouted by the member that just lost its shield.
    #[serde(default)]
    pub swap_yells: Vec<String>,
    /// Twins trade places on a swap; a panel stays seated.
    #[serde(default = "yes")]
    pub swap_places: bool,
    /// The Culture Committee: each swap closes a vote on a raid-wide policy.
    #[serde(default)]
    pub vote: Option<VoteDef>,
}

/// A panel that votes on a policy at every swap (docs/ILMARI_09.md B.5). `show` of the
/// `policies` are on the ballot; each ballot terminal (a `PropKind::Ballot` prop) adds one to its
/// slot, and the slot with the most uses over the window passes (ties and silence: the first).
/// The passed policy is an aura on every living player. Story skips the ballot: `story_policy`
/// passes every time.
#[derive(Clone, Debug, Deserialize)]
pub struct VoteDef {
    pub policies: Vec<String>,
    pub show: u8,
    /// Ballot terminal prop keys, one per ballot slot (`show` of them).
    pub terminals: Vec<String>,
    pub duration_ms: u32,
    pub story_policy: String,
}

impl TwinsDef {
    /// The boss first, then the partners in spawn and rotation order.
    pub fn members(&self) -> Vec<&str> {
        std::iter::once(self.boss.as_str())
            .chain(std::iter::once(self.twin.as_str()))
            .chain(self.extra.iter().map(String::as_str))
            .collect()
    }
}

/// How often a boss lockout resets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum LockoutPeriod {
    /// ISO week (Monday 00:00 UTC): raids.
    #[default]
    Weekly,
    /// UTC day: dungeons, short chains that can be farmed daily.
    Daily,
}

/// Absent in minimal test content: no raid.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RaidDef {
    pub name: String,
    /// Raid zones in walking order; each zone's boss is one encounter.
    pub zones: Vec<String>,
    /// Groups smaller than this play Async Attendance.
    pub normal_min_players: u8,
    /// Async Attendance enemy HP and damage, percent of Normal.
    pub async_hp_pct: u32,
    pub async_damage_pct: u32,
    /// Raid pieces each member rolls per Normal boss kill; Async rolls one at `async_item_pct`.
    pub boss_items: u32,
    pub async_item_pct: u32,
    /// Standing with every department per boss kill (halved on Async).
    pub boss_rep: u32,
    #[serde(default)]
    pub twins: Option<TwinsDef>,
    /// Tier sets (`GearDef::set`) its bosses drop; empty: every raid piece.
    #[serde(default)]
    pub sets: Vec<String>,
    /// `Daily` makes this entry a dungeon (3-player Normal, daily boss lockout); the rest of the
    /// framework (Async scaling, loot, reputation) is shared with raids.
    #[serde(default)]
    pub lockout: LockoutPeriod,
    /// An Ilmari wing (docs/ILMARI_09.md B.3): the difficulty is the group's choice, not its
    /// size. Story is available to any group, solo included, and uses the `async_*` figures;
    /// Full is the authored unit. Each locks apart (`STORY_PREFIX`).
    #[serde(default)]
    pub ilmari: bool,
    /// Ilmari tokens each member earns per boss kill on Full, half on Story (the weekly cap
    /// of `ilmari.rs` still applies).
    #[serde(default)]
    pub boss_tokens: u32,
}

impl RaidDef {
    pub fn is_dungeon(&self) -> bool {
        self.lockout == LockoutPeriod::Daily
    }

    /// Level every member needs at the door from outside: a dungeon opens with its first room's
    /// band, a raid with the level cap.
    pub fn entry_level(&self, c: &Content) -> u8 {
        if self.is_dungeon() {
            self.zones
                .first()
                .and_then(|z| c.zone(z))
                .map_or(1, |z| z.level_range.0)
        } else {
            c.max_level()
        }
    }

    pub fn is_raid_zone(&self, zone: &str) -> bool {
        self.zones.iter().any(|z| z == zone)
    }

    /// Boss unit keys of the encounters in walking order.
    pub fn bosses(&self, c: &Content) -> Vec<String> {
        self.zones
            .iter()
            .filter_map(|z| c.zone(z).and_then(|d| d.boss.as_ref()))
            .map(|b| b.unit.clone())
            .collect()
    }

    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("raid: {m}")));
        if self.zones.is_empty() {
            return Ok(());
        }
        for z in &self.zones {
            match c.zone(z) {
                None => return bad(format!("unknown zone {z}")),
                // A dungeon may open with a trash gauntlet (The Migration's loading dock), and so
                // may an Ilmari wing (Wing 4's Cooling Deck).
                Some(d) if d.boss.is_none() && !self.is_dungeon() && !self.ilmari => {
                    return bad(format!("zone {z} has no boss"));
                }
                _ => {}
            }
        }
        if self.normal_min_players == 0 || self.async_hp_pct == 0 || self.async_damage_pct == 0 {
            return bad("zero player count or async percent".into());
        }
        if let Some(t) = &self.twins {
            for k in t.members() {
                if !c.unit_def(k).is_some_and(|u| u.boss && u.hostile) {
                    return bad(format!("twin {k} is not a hostile boss"));
                }
            }
            if !self.bosses(c).contains(&t.boss) {
                return bad(format!("twin {} is no zone boss", t.boss));
            }
            if c.aura(&t.shield_aura).is_none() || t.swap_ms == 0 {
                return bad("twins need a shield aura and a swap period".into());
            }
            if let Some(v) = &t.vote
                && (v.show == 0
                    || usize::from(v.show) > v.policies.len()
                    || v.terminals.len() != usize::from(v.show)
                    || v.duration_ms == 0
                    || !v.policies.contains(&v.story_policy)
                    || v.policies.iter().any(|a| c.aura(a).is_none())
                    || v.terminals
                        .iter()
                        .any(|k| !c.world.props.iter().any(|p| p.key == *k)))
            {
                return bad("a vote needs known policies, terminals and a story policy".into());
            }
        }
        for s in &self.sets {
            if !c.economy.gear.iter().any(|g| g.set.as_ref() == Some(s)) {
                return bad(format!("{}: unknown tier set {s}", self.name));
            }
        }
        Ok(())
    }
}

/// Every raid on its own, then across raids: unique names, and no zone in two raids (a boss
/// key in two raids would share one lockout).
pub(crate) fn validate_all(c: &Content) -> Result<(), ContentError> {
    for (i, r) in c.raids.iter().enumerate() {
        r.validate(c)?;
        for o in &c.raids[..i] {
            if o.name == r.name || o.zones.iter().any(|z| r.is_raid_zone(z)) {
                return Err(ContentError::Invalid(format!(
                    "raid: {} and {} share a name or a zone",
                    o.name, r.name
                )));
            }
        }
    }
    Ok(())
}

/// What the client shows and draws: the raid zone loaded ("" outside), the difficulty in force,
/// this week's lockout and the "Pair Up" tethers (`(a, b)`; `b` may be the boss itself).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaidView {
    pub zone: String,
    pub async_mode: bool,
    /// Story difficulty of an Ilmari wing (`async_mode` is true with it): the lockout keys carry
    /// `STORY_PREFIX` instead of `ASYNC_PREFIX`.
    pub story: bool,
    /// The difficulty the next wing door opens on (`true`: Story), for the switch at the door.
    pub story_next: bool,
    /// The loaded wing's difficulty (`Full` outside one); `story` stays for 0.9 readers.
    pub mode: crate::ilmari::IlmariMode,
    /// The difficulty the next wing door opens on, Judgment Day included.
    pub mode_next: crate::ilmari::IlmariMode,
    /// Judgment Day: players whose one revive of the running pull is spent.
    pub spent: Vec<EntityId>,
    pub week: u32,
    /// Encounters (zone boss keys, `async:`-prefixed for Async Attendance) killed this week.
    pub locked: Vec<String>,
    pub tethers: Vec<(EntityId, EntityId)>,
    /// Tether range in metres.
    pub tether_m: u8,
    /// The Culture Committee's open vote: the policy auras on the ballot, one per terminal ...
    pub vote: Vec<String>,
    /// ... the uses each has had this window ...
    pub ballots: Vec<u32>,
    /// ... and the policy that passed last (an aura key, "" before the first).
    pub passed: String,
}

impl RaidView {
    /// The lockout key of `boss` on the difficulty in force.
    pub fn key(&self, boss: &str) -> String {
        if self.story {
            format!("{STORY_PREFIX}{boss}")
        } else if self.mode == crate::ilmari::IlmariMode::Judgment {
            format!("{JUDGMENT_PREFIX}{boss}")
        } else {
            lock_key(boss, self.async_mode)
        }
    }

    /// The lockout of ISO week `week`: empty once the reset passed.
    pub fn locked_in(&self, week: u32) -> &[String] {
        if self.week == week { &self.locked } else { &[] }
    }
}

/// The lockout key of `boss` on a difficulty.
pub fn lock_key(boss: &str, async_mode: bool) -> String {
    if async_mode {
        format!("{ASYNC_PREFIX}{boss}")
    } else {
        boss.to_string()
    }
}

/// `clear:<first room>#`; the clear's number follows.
fn clear_prefix(r: &RaidDef) -> String {
    format!(
        "{CLEAR_PREFIX}{}#",
        r.zones.first().map_or("", String::as_str)
    )
}

struct Tether {
    a: EntityId,
    b: EntityId,
    source: EntityId,
    spell: SpellId,
    aura: String,
    until: u64,
    max_m: f32,
    amount: u32,
}

#[derive(Default)]
pub(crate) struct RaidState {
    async_mode: bool,
    /// QA/tests: the difficulty regardless of group size.
    pub(crate) force: Option<bool>,
    /// An Ilmari wing's difficulty as the group lead picked it; `None` follows the group size
    /// (solo and pairs get Story, three or more Full).
    pub(crate) pick: Option<crate::ilmari::IlmariMode>,
    /// The loaded wing's difficulty (`Full` outside an Ilmari wing).
    pub(crate) mode: crate::ilmari::IlmariMode,
    /// The loaded wing runs on Story. Implies `async_mode`, which carries the scaling and loot.
    pub(crate) story: bool,
    lock: Lockout,
    /// Dungeon encounters: the same bookkeeping keyed by UTC day instead of ISO week.
    lock_daily: Lockout,
    tethers: Vec<Tether>,
    next_tether_check: u64,
    /// Next twin swap; `u64::MAX` while the twins are not fighting.
    next_swap: u64,
    /// Guards the linked-HP mirror against recursion.
    linking: bool,
    announce: bool,
    /// Wing 2's mirrors, votes and review scores (`hr.rs`).
    pub(crate) hr: crate::hr::HrState,
}

impl Sim {
    fn raid_week(&self) -> u32 {
        self.legacy_week()
    }

    fn raid_day(&self) -> u32 {
        crate::live::day_index(self.unix_s())
    }

    /// The raid or dungeon whose encounter list holds `boss`.
    fn raid_def_of_boss(&self, boss: &str) -> Option<&RaidDef> {
        self.content
            .raids
            .iter()
            .find(|r| r.bosses(&self.content).iter().any(|b| b == boss))
    }

    /// The raid of the loaded zone.
    fn cur_raid(&self) -> Option<&RaidDef> {
        self.zone.as_deref().and_then(|z| self.content.raid_of(z))
    }

    /// The loaded zone is in an Ilmari wing (its bosses drop `GearSource::Ilmari` pieces).
    pub(crate) fn cur_raid_is_ilmari(&self) -> bool {
        self.cur_raid().is_some_and(|r| r.ilmari)
    }

    pub(crate) fn cur_raid_is_dungeon(&self) -> bool {
        self.cur_raid().is_some_and(RaidDef::is_dungeon)
    }

    /// The loaded zone is part of a raid.
    pub fn in_raid(&self) -> bool {
        self.cur_raid().is_some()
    }

    pub fn raid_async(&self) -> bool {
        self.raid.async_mode
    }

    pub fn raid_view(&self) -> RaidView {
        let week = self.raid_week();
        let max = self.raid.tethers.first().map_or(0.0, |t| t.max_m);
        RaidView {
            zone: if self.in_raid() {
                self.zone.clone().unwrap_or_default()
            } else {
                String::new()
            },
            async_mode: self.raid.async_mode,
            story: self.raid.story,
            story_next: self.content.raids.iter().any(|r| r.ilmari) && self.wing_story(),
            mode: self.raid.mode,
            mode_next: if self.content.raids.iter().any(|r| r.ilmari) {
                self.wing_mode(None)
            } else {
                Default::default()
            },
            spent: self.jd_spent(),
            week,
            // Dungeon and raid locks share one list; boss keys are unique across both.
            locked: {
                let mut l = self.raid.lock.list(week);
                l.extend(self.raid.lock_daily.list(self.raid_day()));
                l
            },
            tethers: self.raid.tethers.iter().map(|t| (t.a, t.b)).collect(),
            tether_m: max.round().clamp(0.0, 255.0) as u8,
            vote: self.raid.hr.offered.clone(),
            ballots: self.raid.hr.ballots.clone(),
            passed: self.raid.hr.passed.clone(),
        }
    }

    /// True while encounter `boss` stays dead on this difficulty until the weekly reset.
    pub fn raid_locked(&self, boss: &str, async_mode: bool) -> bool {
        let key = self.key_of(boss, async_mode);
        if self.raid_def_of_boss(boss).is_some_and(RaidDef::is_dungeon) {
            self.raid.lock_daily.contains(self.raid_day(), &key)
        } else {
            self.raid.lock.contains(self.raid_week(), &key)
        }
    }

    /// The lockout key of `boss` on a difficulty: in an Ilmari wing the reduced one is Story.
    fn key_of(&self, boss: &str, async_mode: bool) -> String {
        let ilmari = self.raid_def_of_boss(boss).filter(|r| r.ilmari);
        if self.raid.mode == crate::ilmari::IlmariMode::Judgment
            && !async_mode
            && ilmari.is_some_and(|r| {
                r.zones
                    .first()
                    .is_some_and(|z| crate::judgment::is_judgment_zone(z))
            })
        {
            format!("{JUDGMENT_PREFIX}{boss}")
        } else if async_mode && ilmari.is_some() {
            format!("{STORY_PREFIX}{boss}")
        } else {
            lock_key(boss, async_mode)
        }
    }

    /// Whether the next Ilmari wing loads on Story: QA force, else the lead's pick, else by group
    /// size (the wings' `normal_min_players`).
    pub(crate) fn wing_story(&self) -> bool {
        self.wing_mode(None) == crate::ilmari::IlmariMode::Story
    }

    pub(crate) fn raid_broadcast(&mut self) {
        let view = self.raid_view();
        self.events.push(Event::Raid { view });
    }

    pub(crate) fn raid_on_join(&mut self, player: EntityId) {
        let view = self.raid_view();
        // The wing switch alone is not news to a player outside every raid.
        let quiet = RaidView {
            story_next: false,
            mode_next: Default::default(),
            ..view.clone()
        };
        if quiet != RaidView::default() {
            self.emit_to(player, Event::Raid { view });
        }
        self.illusion_on_join(player);
        self.wing3_on_join(player);
        self.wing4_on_join(player);
        self.wing5_on_join(player);
    }

    /// The door from outside into a raid opens only with everyone at the level cap (a dungeon:
    /// at its band's lower level); links inside follow their content `requires`.
    pub(crate) fn raid_link_open(&self, from: &str, to: &str) -> Option<bool> {
        let into = self.content.raid_of(to)?;
        let inside = self
            .content
            .raid_of(from)
            .is_some_and(|r| r.name == into.name);
        (!inside).then(|| self.everyone_at(into.entry_level(&self.content)))
    }

    /// Called by `load_zone` before anything spawns: picks the difficulty for a raid zone and
    /// drops the tethers of the zone being left.
    pub(crate) fn raid_on_load(&mut self, id: &str) {
        let was = self.in_raid() || !self.raid.tethers.is_empty();
        self.raid.tethers.clear();
        self.raid.next_swap = u64::MAX;
        self.hr_on_load(id);
        let def = self
            .content
            .raid_of(id)
            .map(|r| (r.normal_min_players, r.ilmari));
        let min = def.map(|d| d.0);
        match def {
            // A wing's difficulty is a choice; Story is the Async scaling under its own name.
            Some((_, true)) => {
                self.raid.mode = self.wing_mode(Some(id));
                self.raid.story = self.raid.mode == crate::ilmari::IlmariMode::Story;
                self.raid.async_mode = self.raid.story;
            }
            Some((min, false)) => {
                self.raid.mode = crate::ilmari::IlmariMode::Full;
                self.raid.story = false;
                self.raid.async_mode = self
                    .raid
                    .force
                    .unwrap_or(self.players.len() < usize::from(min));
            }
            None => {
                self.raid.mode = crate::ilmari::IlmariMode::Full;
                self.raid.story = false;
            }
        }
        self.ilmari.jd_spent.clear();
        self.sight_on_load();
        self.offers_on_load();
        self.raid.announce = min.is_some() || was;
    }

    /// Raid bosses come back every week: a locked encounter stays dead, an unlocked one spawns
    /// even in a zone cleared before. `None` outside the raids (the campaign rule applies).
    pub(crate) fn raid_boss_spawns(&self, zone: &str, boss: &str) -> Option<bool> {
        self.content
            .raid_of(zone)
            // Dungeons are meant to be rerun: a locked boss stays fightable, it only drops no gear.
            .map(|r| r.is_dungeon() || !self.raid_locked(boss, self.raid.async_mode))
    }

    pub(crate) fn twins(&self) -> Option<TwinsDef> {
        self.cur_raid().and_then(|r| r.twins.clone())
    }

    /// The living unit of each member, in member order; `None` unless all of them live.
    fn twin_units(&self, t: &TwinsDef) -> Option<Vec<usize>> {
        t.members()
            .into_iter()
            .map(|k| {
                (0..self.units.len()).find(|&i| {
                    let u = &self.units[i];
                    !u.despawned && u.state.alive && u.state.key == k
                })
            })
            .collect()
    }

    /// After the zone's units exist: the linked partners join the boss, shielded (the twin on
    /// its right, further members on its left).
    pub(crate) fn raid_after_load(&mut self) {
        if let Some(t) = self.twins()
            && let Some(bi) = (0..self.units.len()).find(|&i| {
                let u = &self.units[i];
                u.floor_boss && !u.despawned && u.state.alive && u.state.key == t.boss
            })
        {
            let b = &self.units[bi];
            let (pos, yaw, level, pack) = (b.state.pos, b.state.yaw, b.state.level, b.state.pack);
            let side = crate::telegraph::right(yaw);
            for (k, key) in t.members().into_iter().skip(1).enumerate() {
                let Ok(def) = self.def_index(key) else {
                    continue;
                };
                let off = if k == 0 { 5.0 } else { -5.0 * k as f32 };
                let at = crate::geom::Vec3::new(pos.x + side.x * off, pos.y, pos.z + side.z * off);
                let id = self.spawn(
                    def,
                    at,
                    yaw,
                    SpawnOpts {
                        level: Some(level),
                        pack,
                        respawns: false,
                        summoner: None,
                        floor_boss: false,
                        zone_slot: None,
                    },
                );
                if let Some(i) = self.index(id) {
                    self.apply_aura(i, &t.shield_aura, SHIELD_MS, None, id, None);
                }
            }
        }
        if std::mem::take(&mut self.raid.announce) {
            self.raid_broadcast();
        }
    }

    /// Async Attendance enemy (HP %, damage %) in a raid; 100/100 elsewhere.
    pub(crate) fn raid_mob_pct(&self) -> (u32, u32) {
        match self.cur_raid() {
            Some(_) if self.raid.mode == crate::ilmari::IlmariMode::Judgment => {
                let jd = &self.content.ilmari.judgment;
                (jd.hp_pct, jd.damage_pct)
            }
            Some(r) if self.raid.async_mode => (r.async_hp_pct, r.async_damage_pct),
            _ => (100, 100),
        }
    }

    // ---- Pair Up ----

    /// `Effect::PairUp`: shuffles the hostile players in range into pairs; an odd one out is
    /// paired with the caster. Every pair takes `base` (scaled like a hit) per second while
    /// farther apart than `max_m`, until `duration_ms` runs out.
    #[allow(clippy::too_many_arguments)] // the effect's fields
    pub(crate) fn raid_pair_up(
        &mut self,
        ci: usize,
        spell: SpellId,
        range_m: f32,
        aura: &str,
        duration_ms: u32,
        max_m: f32,
        amount: u32,
    ) {
        let caster = Self::id_of(ci);
        let me = self.units[ci].state.pos;
        let mut pool: Vec<EntityId> = self
            .players
            .iter()
            .copied()
            .filter(|p| {
                self.unit(*p).is_some_and(|u| {
                    u.alive && crate::geom::distance(u.pos, me) <= range_m.max(1.0)
                })
            })
            .collect();
        // Fisher-Yates on the combat stream: pairs differ between pulls but replay exactly.
        for k in (1..pool.len()).rev() {
            let j = self.rng.below(k as u32 + 1) as usize;
            pool.swap(k, j);
        }
        self.raid.tethers.retain(|t| t.source != caster);
        let until = self.now + ticks(duration_ms);
        let mut k = 0;
        while k < pool.len() {
            let a = pool[k];
            let b = pool.get(k + 1).copied().unwrap_or(caster);
            self.raid.tethers.push(Tether {
                a,
                b,
                source: caster,
                spell,
                aura: aura.to_string(),
                until,
                max_m,
                amount,
            });
            for p in [a, b] {
                if let Some(i) = self.index(p).filter(|_| self.is_player(p)) {
                    self.apply_aura(i, aura, duration_ms, None, caster, Some(spell));
                }
            }
            k += 2;
        }
        self.raid.next_tether_check = self.now + ticks(TETHER_TICK_MS);
        self.raid_broadcast();
    }

    fn update_tethers(&mut self) {
        if self.raid.tethers.is_empty() {
            return;
        }
        let before = self.raid.tethers.len();
        let now = self.now;
        let alive = |s: &Sim, id: EntityId| {
            s.unit(id)
                .is_some_and(|u| u.alive && u.ai != Some(AiState::Evade))
        };
        let keep: Vec<bool> = self
            .raid
            .tethers
            .iter()
            .map(|t| t.until > now && alive(self, t.source) && alive(self, t.a) && alive(self, t.b))
            .collect();
        let mut gone: Vec<(EntityId, EntityId, String)> = Vec::new();
        let mut k = 0;
        self.raid.tethers.retain(|t| {
            let kept = keep[k];
            k += 1;
            if !kept {
                gone.push((t.a, t.b, t.aura.clone()));
            }
            kept
        });
        for (a, b, aura) in gone {
            for p in [a, b] {
                if let Some(i) = self.index(p) {
                    self.remove_aura(i, &aura, AuraEnd::Expired);
                }
            }
        }
        if self.now >= self.raid.next_tether_check {
            self.raid.next_tether_check = self.now + ticks(TETHER_TICK_MS);
            let mut hits = Vec::new();
            for t in &self.raid.tethers {
                let (Some(a), Some(b)) = (self.unit(t.a), self.unit(t.b)) else {
                    continue;
                };
                if crate::geom::distance(a.pos, b.pos) > t.max_m {
                    for p in [t.a, t.b] {
                        if self.is_player(p) {
                            hits.push((t.source, p, t.spell, t.amount));
                        }
                    }
                }
            }
            for (source, target, spell, amount) in hits {
                self.deal_damage(
                    source,
                    target,
                    Hit {
                        spell: Some(spell),
                        amount,
                        crit: false,
                        combo: false,
                        school: School::Arcane,
                        dot: true,
                    },
                );
            }
        }
        if self.raid.tethers.len() != before {
            self.raid_broadcast();
        }
    }

    // ---- linked bosses (the Synergy Twins, the Hype Panel) ----

    /// Linked HP: whatever one member loses, the others lose too.
    pub(crate) fn raid_link_hp(&mut self, ti: usize) {
        if self.raid.linking {
            return;
        }
        let Some(t) = self.twins() else {
            return;
        };
        let key = self.units[ti].state.key.clone();
        if !t.members().contains(&key.as_str()) {
            return;
        }
        let (hp, max_t) = (self.units[ti].state.hp, self.units[ti].state.max_hp);
        for pi in 0..self.units.len() {
            let u = &self.units[pi];
            if pi == ti
                || u.despawned
                || !u.state.alive
                || !t.members().contains(&u.state.key.as_str())
            {
                continue;
            }
            // Proportional, so a different max HP (group scaling) still empties all at once.
            let want = (u64::from(hp) * u64::from(u.state.max_hp) / u64::from(max_t.max(1))) as u32;
            if u.state.hp > want {
                self.units[pi].state.hp = want;
            }
        }
    }

    /// A member's death takes the others with it (same killer, so the floor boss pays out).
    fn raid_twin_death(&mut self, ti: usize, killer: Option<EntityId>) {
        let Some(t) = self.twins() else {
            return;
        };
        let key = self.units[ti].state.key.clone();
        if !t.members().contains(&key.as_str()) {
            return;
        }
        let others: Vec<usize> = (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                i != ti
                    && !u.despawned
                    && u.state.alive
                    && t.members().contains(&u.state.key.as_str())
            })
            .collect();
        self.raid.linking = true;
        for pi in others {
            if self.units[pi].state.alive {
                self.die(pi, killer);
            }
        }
        self.raid.linking = false;
    }

    fn update_twins(&mut self) {
        let Some(t) = self.twins() else {
            return;
        };
        let Some(idx) = self.twin_units(&t) else {
            return;
        };
        self.link_threat(&idx);
        let fighting = idx.iter().any(|&i| self.units[i].state.in_combat);
        if !fighting {
            self.raid.next_swap = u64::MAX;
            self.vote_reset();
            return;
        }
        if self.raid.next_swap == u64::MAX {
            self.raid.next_swap = self.now + ticks(t.swap_ms);
            self.vote_open(&t);
            return;
        }
        if self.now < self.raid.next_swap {
            return;
        }
        self.raid.next_swap = self.now + ticks(t.swap_ms);
        self.vote_close(&t, idx[0]);
        let n = idx.len();
        if t.swap_places {
            // Everyone takes the seat of the next member (two twins simply trade places).
            let pos: Vec<_> = idx.iter().map(|&i| self.units[i].state.pos).collect();
            for (k, &i) in idx.iter().enumerate() {
                let to = pos[(k + 1) % n];
                self.units[i].state.pos = to;
                self.events.push(Event::Teleported {
                    unit: Self::id_of(i),
                    pos: to,
                });
            }
        }
        let open = (0..n)
            .find(|&k| !self.has_aura(idx[k], &t.shield_aura))
            .unwrap_or(0);
        let next = idx[(open + 1) % n];
        self.remove_aura(next, &t.shield_aura, AuraEnd::Expired);
        let was = idx[open];
        let was_id = Self::id_of(was);
        self.apply_aura(was, &t.shield_aura, SHIELD_MS, None, was_id, None);
        if !t.swap_yells.is_empty() {
            let line = t.swap_yells[self.rng.below(t.swap_yells.len() as u32) as usize].clone();
            self.request_yell_line(next, crate::content::YellKind::Blast, Some(line));
        }
    }

    /// Linked members pull and reset as one: a pull on any member pulls them all, and when the
    /// boss evades the others do too (a seated partner cannot chase, so it would never leash).
    fn link_threat(&mut self, idx: &[usize]) {
        let boss = idx[0];
        if self.units[boss].state.ai == Some(AiState::Evade) {
            for &i in &idx[1..] {
                if !matches!(
                    self.units[i].state.ai,
                    Some(AiState::Evade) | Some(AiState::Idle)
                ) {
                    self.begin_evade(i);
                }
            }
            return;
        }
        let foes: Vec<EntityId> = idx
            .iter()
            .flat_map(|&i| self.units[i].threat.keys().copied())
            .collect();
        for &i in idx {
            for &f in &foes {
                if !self.units[i].threat.contains_key(&f) {
                    self.add_threat(i, f, 1);
                    if self.units[i].state.ai == Some(AiState::Idle) {
                        self.set_ai(i, AiState::Chase);
                    }
                }
            }
        }
    }

    pub(crate) fn has_aura(&self, i: usize, key: &str) -> bool {
        self.units[i].auras.iter().any(|a| a.key == key)
    }

    pub(crate) fn update_raid(&mut self) {
        self.update_tethers();
        self.update_twins();
        self.update_hr();
        self.update_wing3();
        self.update_wing4();
        self.update_wing5();
        self.update_judgment();
    }

    // ---- kills, loot, lockout ----

    /// After a raid death: the linked members go along; a zone boss locks its encounter for
    /// the week and grants the raid achievements (`raid:<boss>`, `raid:<normal|async>:<boss>`).
    pub(crate) fn raid_on_kill(&mut self, ti: usize, killer: Option<EntityId>) {
        if !self.in_raid() {
            return;
        }
        if !self.raid.linking {
            self.raid_twin_death(ti, killer);
        }
        if !self.units[ti].floor_boss {
            return;
        }
        let key = self.units[ti].state.key.clone();
        let async_mode = self.raid.async_mode;
        if let Some(r) = self.cur_raid().filter(|r| r.is_dungeon()) {
            let day = self.raid_day();
            let finale = r.bosses(&self.content).last() == Some(&key);
            let clear =
                finale.then(|| format!("{}{}", clear_prefix(r), self.dungeon_clears(r) + 1));
            self.raid
                .lock_daily
                .insert(day, self.key_of(&key, async_mode));
            if let Some(c) = clear {
                self.raid.lock_daily.insert(day, c);
            }
        } else {
            let week = self.raid_week();
            let lock = self.key_of(&key, async_mode);
            self.raid.lock.insert(week, lock);
        }
        self.raid.tethers.clear();
        self.raid_pay_tokens(self.raid.story);
        let mode = match (async_mode, self.raid.story) {
            _ if self.raid.mode == crate::ilmari::IlmariMode::Judgment => "judgment",
            (_, true) => "story",
            (true, _) => "async",
            _ => "normal",
        };
        let mut keys = vec![format!("raid:{key}"), format!("raid:{mode}:{key}")];
        let pulled = self.units[ti].pulled_at;
        keys.extend(self.wing5_clean_keys(&key, pulled));
        for p in self.players.clone() {
            self.check_achievements(
                p,
                |t| matches!(t, crate::world::Trigger::Event(k) if keys.contains(k)),
            );
        }
        self.raid_broadcast();
    }

    /// A player's kill of a unit with `UnitDef::death_spawn` (the Hydra's cables) grows its
    /// replacements where it fell, under the same boss and never past the cap. A boss's death
    /// takes its adds with it first (their summoner is already dead), so nothing grows then.
    pub(crate) fn death_spawn(&mut self, ti: usize, killer: Option<EntityId>) {
        if !killer.is_some_and(|k| self.is_player(k)) {
            return;
        }
        let Some(d) = self.content.units[self.units[ti].def].death_spawn.clone() else {
            return;
        };
        let Some(owner) = self.units[ti]
            .summoner
            .and_then(|s| self.index(s))
            .filter(|&o| self.units[o].state.alive)
        else {
            return;
        };
        let n = if self.raid.story {
            d.story_count.unwrap_or(d.count)
        } else {
            d.count
        };
        let at = self.units[ti].state.pos;
        self.summon(owner, at, &d.unit, n, d.max_alive, 0);
    }

    /// An Ilmari wing's boss pays tokens to every member (half on Story), within the weekly cap.
    fn raid_pay_tokens(&mut self, story: bool) {
        let Some(n) = self
            .cur_raid()
            .filter(|r| r.ilmari)
            .map(|r| r.boss_tokens)
            .filter(|n| *n > 0)
        else {
            return;
        };
        let n = if story { n / 2 } else { n };
        for p in self.players.clone() {
            self.grant_ilmari_tokens(p, n);
        }
    }

    /// Full clears of the loaded dungeon this UTC day (finale kills), whatever the difficulty.
    fn dungeon_clears(&self, r: &RaidDef) -> usize {
        let prefix = clear_prefix(r);
        self.raid
            .lock_daily
            .list(self.raid_day())
            .iter()
            .filter(|k| k.starts_with(&prefix))
            .count()
    }

    /// Kill XP percent of the loaded zone: a dungeon pays in full on the first clear of the UTC
    /// day, a quarter on the second and nothing after (Kudos, reputation and loot rules aside).
    pub(crate) fn raid_xp_pct(&self) -> u32 {
        match self.cur_raid() {
            Some(r) if r.is_dungeon() => match self.dungeon_clears(r) {
                0 => 100,
                1 => 25,
                _ => 0,
            },
            _ => 100,
        }
    }

    /// Raid-piece rolls of a raid boss kill: `Some((rolls, chance %, legendary %))`; linked
    /// partners roll nothing. `None` for everything else (raid trash rolls the Legacy tier).
    pub(crate) fn raid_loot(&self, mob: usize) -> Option<(u32, u32, u32)> {
        let u = &self.units[mob];
        let r = self.cur_raid()?;
        if !u.state.boss {
            return None;
        }
        let legendary = self.content.economy.legendary_pct;
        // A locked dungeon boss still fights and pays Kudos, but drops no gear.
        let locked = r.is_dungeon() && self.raid_locked(&u.state.key, self.raid.async_mode);
        Some(match (u.floor_boss, self.raid.async_mode) {
            _ if locked => (0, 0, 0),
            (true, false) if self.raid.mode == crate::ilmari::IlmariMode::Judgment => (
                self.content.ilmari.judgment.boss_items.max(1),
                100,
                legendary,
            ),
            (true, false) => (r.boss_items.max(1), 100, legendary),
            (true, true) => (1, r.async_item_pct, legendary / 2),
            (false, _) => (0, 0, 0),
        })
    }

    /// The tier sets the loaded raid drops (empty: any raid piece).
    pub(crate) fn raid_sets(&self) -> Vec<String> {
        self.cur_raid().map(|r| r.sets.clone()).unwrap_or_default()
    }

    /// Department standing per raid boss kill: every department, halved on Async.
    pub(crate) fn raid_boss_rep(&self, ti: usize) -> Option<u32> {
        let r = self.cur_raid()?.boss_rep;
        self.units[ti]
            .floor_boss
            .then_some(if self.raid.async_mode { r / 2 } else { r })
    }

    /// The lockout for the save: (ISO week, encounter keys).
    pub(crate) fn raid_lockout(&self) -> (u32, Vec<String>) {
        self.raid.lock.saved()
    }

    /// The dungeon lockout for the save: (UTC day, encounter keys).
    pub(crate) fn dungeon_lockout(&self) -> (u32, Vec<String>) {
        self.raid.lock_daily.saved()
    }

    /// Saved encounter keys that still exist; the rest are dropped.
    fn known_raid_keys(&self, keys: &[String]) -> Vec<String> {
        let bosses: Vec<String> = self
            .content
            .raids
            .iter()
            .flat_map(|r| r.bosses(&self.content))
            .collect();
        keys.iter()
            .filter(|k| {
                k.starts_with(CLEAR_PREFIX)
                    || bosses.iter().any(|b| {
                        k.strip_prefix(ASYNC_PREFIX)
                            .or_else(|| k.strip_prefix(STORY_PREFIX))
                            .or_else(|| k.strip_prefix(JUDGMENT_PREFIX))
                            .unwrap_or(k)
                            == b
                    })
            })
            .cloned()
            .collect()
    }

    /// Restores a saved lockout; unknown encounters are dropped, an old week is stale.
    pub(crate) fn set_raid_lockout(&mut self, week: u32, keys: &[String]) {
        let known = self.known_raid_keys(keys);
        self.raid.lock.restore(week, known);
        self.raid_broadcast();
    }

    /// Restores a saved dungeon lockout; an old day is stale.
    pub(crate) fn set_dungeon_lockout(&mut self, day: u32, keys: &[String]) {
        let known = self.known_raid_keys(keys);
        self.raid.lock_daily.restore(day, known);
        self.raid_broadcast();
    }

    /// Debug/QA: this week's raid lockout set to `keys` (screenshots).
    pub fn debug_set_raid_lockout(&mut self, keys: &[String]) {
        let week = self.raid_week();
        self.set_raid_lockout(week, keys);
    }

    /// Debug/QA: the difficulty for the next raid loads regardless of group size (`None`: by
    /// group size again).
    pub fn debug_force_raid_async(&mut self, async_mode: Option<bool>) {
        self.raid.force = async_mode;
    }
}
