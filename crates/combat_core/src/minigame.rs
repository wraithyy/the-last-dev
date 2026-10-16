//! The minigame frame every minigame shares (Teambuilding now, the Company Calendar's games
//! later): the group lead starts one from the campus (`Command::StartMinigame`, the hook a
//! calendar board calls too), the host follows `Sim::minigame_travel` to its zone, every member
//! gets a run with a timer, a score and at most one medal, and `Command::LeaveMinigame` ends a
//! run early without one (the client asks first). When every run is over the host takes the
//! group back to the campus. The rules of each game live in their own module and plug in
//! through `GameRun`; this file only knows starts, timers, scores, medals and rewards.
//! The Company Calendar's games (`corporate/`) add three shared pieces: a choice card the
//! player answers with `Command::MinigamePick`, status lines and gauges for the header, and
//! routed notes (`Event::MinigameNote`).
//! Content: `minigames` in `game.ron`; numbers in docs/FORMULAS.md.

use crate::burnout::BurnoutCause;
use crate::content::{Content, ContentError, ticks};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{Event, ItemError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Minigame keys travel on the wire.
pub const MINIGAME_KEY_MAX: usize = 32;
/// The fade to the venue starts this long after the lead said yes.
const TRAVEL_DELAY_MS: u32 = 1200;
/// Time on the medal screen before the group goes back to the campus.
const RETURN_DELAY_MS: u32 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Medal {
    Bronze,
    Silver,
    Gold,
}

impl Medal {
    pub const ALL: [Medal; 3] = [Medal::Bronze, Medal::Silver, Medal::Gold];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: u8) -> Option<Medal> {
        Self::ALL.get(usize::from(i)).copied()
    }
}

/// One medal tier: what it takes and what it pays. Tiers do not stack: each lists its whole
/// reward, so Gold simply lists the most.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MedalDef {
    /// "Attended", "Networked", "Was Never Seen".
    pub name: String,
    #[serde(default)]
    pub min_score: u32,
    /// Only a run nobody noticed qualifies.
    #[serde(default)]
    pub flawless: bool,
    /// Standing with the minigame's department.
    #[serde(default)]
    pub rep: u32,
    /// Burnout relief, per mille.
    #[serde(default)]
    pub relief: u16,
    /// Gear or goods, each granted once per character (never a second copy).
    #[serde(default)]
    pub items: Vec<String>,
    /// Grants the achievements with `Trigger::Event(<event>)` (a title).
    #[serde(default)]
    pub event: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MinigameDef {
    pub key: String,
    pub name: String,
    /// One joke line for the invite and the calendar.
    pub blurb: String,
    pub zone: String,
    /// Length estimate shown on the invite ("~6 min").
    pub length: String,
    pub limit_s: u32,
    /// Department (`EndgameDef::departments` name) whose standing the medals raise.
    pub department: String,
    /// Bronze, Silver, Gold.
    pub medals: Vec<MedalDef>,
    /// How to play, for the invite card.
    #[serde(default)]
    pub how: String,
    /// One line under the title when the venue loads.
    #[serde(default)]
    pub hint: String,
    /// The rules; exactly one game per minigame.
    #[serde(default)]
    pub teambuilding: Option<crate::teambuilding::TeambuildingDef>,
    #[serde(default)]
    pub review: Option<crate::corporate::review::ReviewDef>,
    #[serde(default)]
    pub reorg: Option<crate::corporate::reorg::ReorgDef>,
    #[serde(default)]
    pub migration: Option<crate::corporate::migration::MigrationDef>,
    #[serde(default)]
    pub hackathon: Option<crate::corporate::hackathon::HackathonDef>,
    #[serde(default)]
    pub focus: Option<crate::corporate::focus::FocusDef>,
}

/// The rules block of a minigame.
#[derive(Clone, Copy, Debug)]
pub enum Rules<'a> {
    Teambuilding(&'a crate::teambuilding::TeambuildingDef),
    Review(&'a crate::corporate::review::ReviewDef),
    Reorg(&'a crate::corporate::reorg::ReorgDef),
    Migration(&'a crate::corporate::migration::MigrationDef),
    Hackathon(&'a crate::corporate::hackathon::HackathonDef),
    Focus(&'a crate::corporate::focus::FocusDef),
}

impl MinigameDef {
    /// The best medal a finished run earns (`None` only if Bronze itself asks for more).
    pub fn medal_for(&self, score: u32, flawless: bool) -> Option<Medal> {
        Medal::ALL.into_iter().rev().find(|m| {
            self.medals
                .get(m.index())
                .is_some_and(|d| score >= d.min_score && (flawless || !d.flawless))
        })
    }

    pub fn medal(&self, m: Medal) -> Option<&MedalDef> {
        self.medals.get(m.index())
    }

    /// Every rules block present, in a fixed order (validation wants exactly one).
    pub fn all_rules(&self) -> Vec<Rules<'_>> {
        let mut v = Vec::new();
        v.extend(self.teambuilding.as_ref().map(Rules::Teambuilding));
        v.extend(self.review.as_ref().map(Rules::Review));
        v.extend(self.reorg.as_ref().map(Rules::Reorg));
        v.extend(self.migration.as_ref().map(Rules::Migration));
        v.extend(self.hackathon.as_ref().map(Rules::Hackathon));
        v.extend(self.focus.as_ref().map(Rules::Focus));
        v
    }

    pub fn rules(&self) -> Option<Rules<'_>> {
        self.all_rules().into_iter().next()
    }

    /// "Mandatory Fun: Teambuilding" -> "TEAMBUILDING": the header's short name.
    pub fn short_name(&self) -> String {
        self.name
            .rsplit(':')
            .next()
            .unwrap_or("")
            .trim()
            .to_uppercase()
    }
}

pub(crate) fn validate(c: &Content) -> Result<(), ContentError> {
    let bad = |m: String| Err(ContentError::Invalid(format!("minigame: {m}")));
    for (i, m) in c.minigames.iter().enumerate() {
        if m.key.is_empty()
            || m.key.len() > MINIGAME_KEY_MAX
            || c.minigames[..i].iter().any(|o| o.key == m.key)
        {
            return bad(format!("bad or duplicate key {}", m.key));
        }
        if c.zone(&m.zone).is_none() || m.limit_s == 0 {
            return bad(format!("{}: unknown zone or no time limit", m.key));
        }
        if m.medals.len() != Medal::ALL.len() {
            return bad(format!("{}: needs Bronze, Silver and Gold", m.key));
        }
        if c.endgame.department_index(&m.department).is_none() {
            return bad(format!("{}: unknown department {}", m.key, m.department));
        }
        for d in &m.medals {
            if let Some(it) = d.items.iter().find(|it| c.economy.goods(it).is_none()) {
                return bad(format!("{}: unknown reward {it}", m.key));
            }
            if let Some(e) = &d.event
                && !c
                    .world
                    .achievements
                    .iter()
                    .any(|a| a.trigger == crate::world::Trigger::Event(e.clone()))
            {
                return bad(format!("{}: no achievement for event {e}", m.key));
            }
        }
        match m.all_rules()[..] {
            [Rules::Teambuilding(t)] => t.validate(c)?,
            [Rules::Review(r)] => r.validate(c)?,
            [Rules::Reorg(r)] => r.validate(c)?,
            [Rules::Migration(r)] => r.validate()?,
            [Rules::Hackathon(r)] => r.validate()?,
            [Rules::Focus(r)] => r.validate(c)?,
            [] => return bad(format!("{}: no rules", m.key)),
            _ => return bad(format!("{}: more than one rules block", m.key)),
        }
    }
    Ok(())
}

/// What a run's HUD shows (routed to its player, on change and once a second).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinigameView {
    pub key: String,
    pub elapsed_ms: u32,
    pub limit_ms: u32,
    /// Bit `i`: objective `i` is done.
    pub objectives: u16,
    /// The objective being worked on and its progress in percent.
    pub dwell: Option<(u8, u8)>,
    pub collected: u8,
    pub collectibles: u8,
    /// Bit `i`: collectible `i` is taken (its model goes away for this player).
    pub taken: u32,
    pub catches: u8,
    /// 0..=100: how close a watcher is to noticing you.
    pub suspicion: u8,
    pub hidden: bool,
    /// Moving fast enough to be heard.
    pub noisy: bool,
    /// The score if the run ended now.
    pub score: u32,
    /// The Company Calendar's games: status lines under the title.
    pub lines: Vec<String>,
    /// Their bars (at most two).
    pub gauges: Vec<Gauge>,
    /// The choice in front of the player, answered with `Command::MinigamePick`.
    pub card: Option<Card>,
    /// Name badges that differ from the unit's own (Reorg: "Sam, VP of Vibes").
    pub badges: Vec<(EntityId, String)>,
    /// Per-game state the world shows (Reorg: bit `i` desk slot `i` in use, bit `16 + i`
    /// partition `i` standing; Tool Migration: bit 0 carrying a ticket).
    pub layout: u32,
}

/// A header bar: `value` percent full, with an optional tick mark at `mark` percent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gauge {
    pub label: String,
    pub value: u8,
    pub mark: Option<u8>,
}

/// A decision: `who` says `text`, the player picks one of `options` (none: just read it).
/// `seq` names the decision, so a late pick for an older card does nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub seq: u16,
    pub who: String,
    pub text: String,
    pub options: Vec<String>,
}

/// Wire caps of the view's texts.
pub const CARD_OPTIONS_MAX: usize = 4;
pub const VIEW_LINES_MAX: usize = 4;

/// A run is over (everyone sees it; the medal screen and the group total read it).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinigameResult {
    pub unit: EntityId,
    pub key: String,
    pub medal: Option<Medal>,
    pub score: u32,
    pub time_ms: u32,
    pub catches: u8,
    pub collected: u8,
    /// Left early or ran out of time: no medal, no score.
    pub quit: bool,
    /// Sum of every finished run of this outing so far, this one included.
    pub group_score: u32,
    /// Rewards that landed (items new to the character, the title's event).
    pub items: Vec<String>,
    /// The game's own one-line summary for the medal screen (Teambuilding leaves it empty).
    pub summary: String,
}

/// The rules part of a run.
pub(crate) enum GameRun {
    Teambuilding(crate::teambuilding::TbRun),
    Review(crate::corporate::review::Run),
    Reorg(crate::corporate::reorg::Run),
    Migration(crate::corporate::migration::Run),
    Hackathon(crate::corporate::hackathon::Run),
    Focus(crate::corporate::focus::Run),
}

impl GameRun {
    /// (catches or mistakes, things collected) for the result.
    fn tallies(&self) -> (u8, u8) {
        match self {
            GameRun::Teambuilding(t) => (t.catches, t.collected_count()),
            GameRun::Review(r) => r.tallies(),
            GameRun::Reorg(r) => r.tallies(),
            GameRun::Migration(r) => r.tallies(),
            GameRun::Hackathon(r) => r.tallies(),
            GameRun::Focus(r) => r.tallies(),
        }
    }
}

pub(crate) struct Run {
    pub(crate) game: GameRun,
    /// (tick, result) once over.
    pub(crate) over: Option<MinigameResult>,
    pub(crate) last_view: Option<MinigameView>,
    pub(crate) last_view_at: u64,
}

/// The minigame the group is in: started on the venue's load.
pub(crate) struct Outing {
    pub(crate) def: usize,
    pub(crate) zone: String,
    pub(crate) started: u64,
    pub(crate) runs: BTreeMap<EntityId, Run>,
    /// Teambuilding's anchors and walkers (Open-Space Focus uses only the walkers).
    pub(crate) venue: crate::teambuilding::Venue,
    /// State every run of the outing shares (Reorg's org chart, the anchors of the others).
    pub(crate) stage: crate::corporate::Stage,
}

#[derive(Default)]
pub(crate) struct MinigameState {
    /// (tick, zone) the host must load once due.
    pub(crate) travel: Option<(u64, String)>,
    /// The minigame whose zone is pending (set with `travel`).
    pub(crate) pending: Option<usize>,
    pub(crate) outing: Option<Outing>,
    /// Best medal per player and minigame key.
    pub(crate) best: BTreeMap<EntityId, BTreeMap<String, Medal>>,
}

impl Sim {
    /// The zone the host must load now to follow the minigame (the venue, or the campus once
    /// every run is over); `None` while nothing is due.
    pub fn minigame_travel(&self) -> Option<&str> {
        self.minigame
            .travel
            .as_ref()
            .filter(|(at, _)| self.now >= *at)
            .map(|(_, z)| z.as_str())
    }

    /// The minigame the group is in right now, if any.
    pub fn minigame_active(&self) -> Option<&MinigameDef> {
        let o = self.minigame.outing.as_ref()?;
        self.content.minigames.get(o.def)
    }

    /// Sum of the finished runs' scores of the current outing.
    pub fn minigame_group_score(&self) -> u32 {
        self.minigame.outing.as_ref().map_or(0, |o| {
            o.runs
                .values()
                .filter_map(|r| r.over.as_ref())
                .filter(|r| !r.quit)
                .map(|r| r.score)
                .sum()
        })
    }

    /// A player's run result in the current outing, once it is over.
    pub fn minigame_result(&self, player: EntityId) -> Option<&MinigameResult> {
        self.minigame
            .outing
            .as_ref()?
            .runs
            .get(&player)?
            .over
            .as_ref()
    }

    /// The HUD view last sent to `player`'s running run.
    pub fn minigame_view(&self, player: EntityId) -> Option<&MinigameView> {
        let r = self.minigame.outing.as_ref()?.runs.get(&player)?;
        r.over.is_none().then_some(r.last_view.as_ref()).flatten()
    }

    /// The best medal `player` ever took in `key`.
    pub fn minigame_best(&self, player: EntityId, key: &str) -> Option<Medal> {
        self.minigame.best.get(&player)?.get(key).copied()
    }

    /// Every best medal of `player`, for the save: (key, medal).
    pub fn minigame_bests(&self, player: EntityId) -> Vec<(String, Medal)> {
        self.minigame
            .best
            .get(&player)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default()
    }

    /// Restores saved best medals; unknown minigames are dropped.
    pub fn set_minigame_bests(&mut self, player: EntityId, bests: &[(String, Medal)]) {
        let known: BTreeMap<String, Medal> = bests
            .iter()
            .filter(|(k, _)| self.content.minigame(k).is_some())
            .cloned()
            .collect();
        self.minigame.best.insert(player, known);
    }

    /// The group lead commits everyone to `key` from the campus, alive and calm. Refusals come
    /// back as an `ItemError` to the asker.
    pub(crate) fn start_minigame(&mut self, actor: EntityId, key: &str) {
        let Some(def) = self.content.minigames.iter().position(|m| m.key == key) else {
            return;
        };
        let refuse = |s: &mut Sim, e: ItemError| s.emit_to(actor, Event::ItemError { reason: e });
        if self.players.first() != Some(&actor)
            || self.zone.as_deref() != Some("campus")
            || self.minigame.travel.is_some()
            || self.sprint_active()
        {
            return refuse(self, ItemError::NotEligible);
        }
        let members: Vec<&crate::types::UnitState> =
            self.players.iter().filter_map(|p| self.unit(*p)).collect();
        if members.iter().any(|u| !u.alive) {
            return refuse(self, ItemError::Dead);
        }
        if members.iter().any(|u| u.in_combat) {
            return refuse(self, ItemError::InCombat);
        }
        let zone = self.content.minigames[def].zone.clone();
        self.minigame.travel = Some((self.now + ticks(TRAVEL_DELAY_MS), zone));
        self.minigame.pending = Some(def);
    }

    /// Ends the asker's run without a medal.
    pub(crate) fn leave_minigame(&mut self, actor: EntityId) {
        self.end_run(actor, None);
    }

    /// Called by `load_zone` before it spawns anything: the awaited venue starts an outing,
    /// any other zone ends the current one.
    pub(crate) fn minigame_on_load(&mut self, id: &str) {
        let expected = self.minigame.travel.take().map(|t| t.1);
        let pending = self.minigame.pending.take();
        self.minigame.outing = None;
        if let (Some(def), Some(z)) = (pending, expected)
            && z == id
        {
            self.minigame.pending = Some(def);
        }
    }

    /// Called by `load_zone` once the zone's units stand: the outing begins with a run each.
    pub(crate) fn minigame_after_load(
        &mut self,
        id: &str,
        anchors: &dyn Fn(&str) -> Option<(crate::geom::Vec3, f32)>,
    ) {
        let Some(def) = self.minigame.pending.take() else {
            return;
        };
        let set = match self.corporate_setup(def, anchors) {
            Ok(s) => s,
            Err(e) => {
                // A layout without the anchors cannot host it; the group stays where it is.
                debug_assert!(false, "{e:?}");
                return;
            }
        };
        let limit_ms = self.content.minigames[def].limit_s.saturating_mul(1000);
        let runs = self
            .players
            .iter()
            .map(|p| {
                (
                    *p,
                    Run {
                        game: set.new_run(*p),
                        over: None,
                        last_view: None,
                        last_view_at: 0,
                    },
                )
            })
            .collect();
        self.minigame.outing = Some(Outing {
            def,
            zone: id.to_string(),
            started: self.now,
            runs,
            venue: set.venue,
            stage: set.stage,
        });
        self.events.push(Event::MinigameStarted {
            key: self.content.minigames[def].key.clone(),
            limit_ms,
        });
        self.teambuilding_place_watchers();
        self.corporate_after_start();
    }

    /// A member left the session: their run goes with them.
    pub(crate) fn minigame_on_leave(&mut self, player: EntityId) {
        if let Some(o) = self.minigame.outing.as_mut() {
            o.runs.remove(&player);
        }
        self.minigame.best.remove(&player);
    }

    pub(crate) fn update_minigame(&mut self) {
        let Some(o) = self.minigame.outing.as_ref() else {
            return;
        };
        if self.zone.as_deref() != Some(o.zone.as_str()) {
            return;
        }
        let def = o.def;
        let limit = ticks(self.content.minigames[def].limit_s.saturating_mul(1000));
        let started = o.started;
        self.update_teambuilding();
        self.update_corporate();
        let open: Vec<EntityId> = self
            .minigame
            .outing
            .as_ref()
            .map(|o| {
                o.runs
                    .iter()
                    .filter(|(_, r)| r.over.is_none())
                    .map(|(p, _)| *p)
                    .collect()
            })
            .unwrap_or_default();
        if self.now.saturating_sub(started) >= limit {
            for p in open.iter().copied() {
                self.end_run(p, None);
            }
        }
        self.send_views();
        let all_over = self
            .minigame
            .outing
            .as_ref()
            .is_some_and(|o| o.runs.values().all(|r| r.over.is_some()));
        if all_over && self.minigame.travel.is_none() {
            self.minigame.travel = Some((self.now + ticks(RETURN_DELAY_MS), "campus".into()));
        }
    }

    /// Ms since the outing started.
    pub(crate) fn minigame_elapsed_ms(&self) -> u32 {
        self.minigame.outing.as_ref().map_or(0, |o| {
            crate::time::ticks_to_ms(self.now.saturating_sub(o.started)) as u32
        })
    }

    /// Ends `player`'s run: `Some(score, flawless)` finished it, `None` gave up (quit or out of
    /// time). Rewards land here; everyone hears the result.
    pub(crate) fn end_run(&mut self, player: EntityId, finished: Option<(u32, bool)>) {
        let elapsed = self.minigame_elapsed_ms();
        let Some(o) = self.minigame.outing.as_ref() else {
            return;
        };
        let def = self.content.minigames[o.def].clone();
        let Some(run) = o.runs.get(&player).filter(|r| r.over.is_none()) else {
            return;
        };
        let (catches, collected) = run.game.tallies();
        let summary = self.corporate_summary(player);
        let medal = finished.and_then(|(score, flawless)| def.medal_for(score, flawless));
        let score = finished.map_or(0, |f| f.0);
        let items = match (medal, self.index(player)) {
            (Some(m), Some(i)) => self.grant_medal(i, &def, m),
            _ => Vec::new(),
        };
        let mut result = MinigameResult {
            unit: player,
            key: def.key.clone(),
            medal,
            score,
            time_ms: elapsed,
            catches,
            collected,
            quit: finished.is_none(),
            group_score: 0,
            items,
            summary,
        };
        if let Some(o) = self.minigame.outing.as_mut()
            && let Some(r) = o.runs.get_mut(&player)
        {
            r.over = Some(result.clone());
        }
        result.group_score = self.minigame_group_score();
        if let Some(o) = self.minigame.outing.as_mut()
            && let Some(r) = o.runs.get_mut(&player)
        {
            r.over = Some(result.clone());
        }
        self.events.push(Event::MinigameEnded { result });
    }

    /// Pays medal `m`: standing, burnout relief, first-time items, the title event, and the
    /// best medal. Returns the items that landed (plus the event key, for the screen).
    fn grant_medal(&mut self, i: usize, def: &MinigameDef, m: Medal) -> Vec<String> {
        let player = Self::id_of(i);
        let Some(md) = def.medal(m).cloned() else {
            return Vec::new();
        };
        // Items only land the first time a tier is reached: checking the bag alone let a
        // player discard or sell them and farm them again run after run.
        let first = self
            .minigame
            .best
            .get(&player)
            .and_then(|b| b.get(&def.key))
            .is_none_or(|b| *b < m);
        if let Some(dept) = self.content.endgame.department_index(&def.department) {
            self.gain_rep(i, dept, md.rep);
        }
        // Repeat runs still help, but a quarter as much, so burnout can't be zeroed on demand.
        let relief = if first { md.relief } else { md.relief / 4 };
        self.add_burnout(i, -i32::from(relief), BurnoutCause::Minigame);
        let fresh: Vec<String> = if first {
            md.items
                .iter()
                .filter(|it| !self.owns_reward(i, it))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let left = self.stow(i, fresh.clone());
        let landed: Vec<String> = fresh.into_iter().filter(|f| !left.contains(f)).collect();
        if !landed.is_empty() {
            self.bag_event(i);
        }
        if let Some(e) = &md.event {
            self.check_achievements(
                player,
                |t| matches!(t, crate::world::Trigger::Event(k) if k == e),
            );
        }
        let best = self.minigame.best.entry(player).or_default();
        if best.get(&def.key).is_none_or(|b| *b < m) {
            best.insert(def.key.clone(), m);
        }
        landed
    }

    /// Carried, worn, or (for a pet carrier) already adopted.
    fn owns_reward(&self, i: usize, item: &str) -> bool {
        let s = &self.units[i].state;
        s.bag.iter().any(|b| b == item)
            || s.equipped.iter().any(|e| e == item)
            || self
                .content
                .pet_for_carrier(item)
                .is_some_and(|p| s.pets.contains(&p.key))
    }

    /// Routed HUD views: on any change, else once a second for the timer.
    fn send_views(&mut self) {
        let Some(o) = self.minigame.outing.as_ref() else {
            return;
        };
        let def = &self.content.minigames[o.def];
        let elapsed = self.minigame_elapsed_ms();
        let base = MinigameView {
            key: def.key.clone(),
            elapsed_ms: elapsed,
            limit_ms: def.limit_s.saturating_mul(1000),
            ..Default::default()
        };
        let mut out = Vec::new();
        for (p, r) in &o.runs {
            if r.over.is_some() {
                continue;
            }
            let view = match (&r.game, def.rules()) {
                (GameRun::Teambuilding(t), Some(Rules::Teambuilding(rules))) => {
                    t.view(rules, &o.venue, base.clone())
                }
                (game, Some(rules)) => {
                    crate::corporate::view(game, rules, &o.stage, self.now, base.clone())
                }
                _ => continue,
            };
            let same = r.last_view.as_ref().is_some_and(|v| {
                MinigameView {
                    elapsed_ms: view.elapsed_ms,
                    score: view.score,
                    ..v.clone()
                } == view
            });
            if !same || self.now.saturating_sub(r.last_view_at) >= ticks(1000) {
                out.push((*p, view));
            }
        }
        for (p, view) in out {
            if let Some(r) = self
                .minigame
                .outing
                .as_mut()
                .and_then(|o| o.runs.get_mut(&p))
            {
                r.last_view = Some(view.clone());
                r.last_view_at = self.now;
            }
            self.emit_to(p, Event::Minigame { unit: p, view });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medals_need_the_score_and_gold_needs_a_clean_run() {
        let md = |min: u32, flawless: bool| MedalDef {
            name: String::new(),
            min_score: min,
            flawless,
            ..Default::default()
        };
        let def = MinigameDef {
            key: "k".into(),
            name: String::new(),
            blurb: String::new(),
            zone: String::new(),
            length: String::new(),
            limit_s: 60,
            department: String::new(),
            medals: vec![md(0, false), md(100, false), md(200, true)],
            ..Default::default()
        };
        assert_eq!(def.medal_for(0, false), Some(Medal::Bronze));
        assert_eq!(def.medal_for(150, false), Some(Medal::Silver));
        assert_eq!(def.medal_for(250, false), Some(Medal::Silver));
        assert_eq!(def.medal_for(250, true), Some(Medal::Gold));
        assert_eq!(Medal::from_index(2), Some(Medal::Gold));
        assert_eq!(Medal::from_index(3), None);
    }
}
