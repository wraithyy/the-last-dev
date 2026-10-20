//! Quests: a log per player, advanced by sim outcomes (the player's kills, loot pickups, talking,
//! zone clears). No rng is involved. Kills are also counted per unit key for the whole run, so a
//! kill quest accepted after the work was done is ready at once (retro credit), like a cleared
//! zone or carried items.

use crate::content::{Content, Objective, QuestDef, QuestGiver, Role, ticks};
use crate::effects::dist_xz;
use crate::geom::{Vec3, in_range};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{Event, ItemError};
use crate::world::PropKind;
use std::collections::{BTreeMap, BTreeSet};

/// An escort counts as delivered this close to its goal anchor (it stops short of whoever
/// stands there).
pub const ESCORT_DONE_M: f32 = 2.5;
/// Escorts amble: slower than a walking player, so the guard is never left behind.
const ESCORT_MPS: f32 = 2.4;
/// An `Inspect` prop counts for a player in reach whose facing is this close to the prop's.
const INSPECT_FACING_RAD: f32 = 1.2;
/// `Interacted` outcomes of quest props, for the client's lines.
pub const QUEST_PROP_IDLE: u32 = 0;
pub const QUEST_PROP_COUNTED: u32 = 1;
pub const QUEST_PROP_WRONG_ORDER: u32 = 2;

/// A `Defend` second counts while the player stands this close to the defended unit.
pub const DEFEND_RANGE_M: f32 = 25.0;
/// At most this many wave units of one `Defend` objective are alive at once.
const DEFEND_MAX_WAVES: usize = 3;
/// Wave units open on the defended unit with this much threat, so a stray hit does not peel
/// them off it at once; taunts and real damage still do.
const DEFEND_WAVE_THREAT: u32 = 200;

/// Quest commands are accepted a little past talk range, so a dialog the client keeps open (it closes
/// only when the player walks this far away) never submits a command the sim would reject silently.
pub const GIVER_REACH_SLACK_M: f32 = 3.0;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActiveQuest {
    pub id: String,
    /// Progress per `QuestDef::objectives` entry, capped at its need.
    pub counts: Vec<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestLog {
    /// Accept order.
    pub active: Vec<ActiveQuest>,
    /// Turn-in order.
    pub completed: Vec<String>,
    /// Own and shared kills per unit key over the run (retro credit); sim side only.
    pub kills: BTreeMap<String, u32>,
}

/// Runtime of the objectives that watch the world rather than count events.
#[derive(Default)]
pub(crate) struct QuestRt {
    /// Last tick a ground telegraph landed on each player (`KillClean`).
    pub(crate) telegraph_hit: BTreeMap<EntityId, u64>,
    /// Next wave tick per defended or escorted unit key (`Defend`, `Escort`).
    next_wave: BTreeMap<String, u64>,
    /// Quest props each player has already used up (index into `world.props`): pickups taken,
    /// spots inspected, devices an item went on. Not saved: pickups respawn per player.
    used_props: BTreeMap<EntityId, BTreeSet<usize>>,
    /// Units each player already applied a `UseOn` item to.
    used_units: BTreeMap<EntityId, BTreeSet<EntityId>>,
    /// `Deliver` deadline tick per (player, quest).
    deadlines: BTreeMap<(EntityId, String), u64>,
    /// Escorted unit key -> goal of the loaded zone.
    escorts: BTreeMap<String, Vec3>,
    /// `Inspect` props of the loaded zone: (index into `world.props`, stand spot, its yaw).
    inspect_spots: Vec<(usize, Vec3, f32)>,
}

static EMPTY_LOG: QuestLog = QuestLog {
    active: Vec::new(),
    completed: Vec::new(),
    kills: BTreeMap::new(),
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestStatus {
    /// Missing its `requires` quest or the player is below `level_min`.
    Locked,
    Available,
    Active,
    /// All objectives done; turn it in.
    Ready,
    Done,
}

fn is_ready(def: &QuestDef, q: &ActiveQuest) -> bool {
    def.objectives
        .iter()
        .zip(&q.counts)
        .all(|(o, c)| *c >= o.need())
}

impl Sim {
    /// The first player's quest log (empty without a player).
    pub fn quests(&self) -> &QuestLog {
        self.quests_of(self.player().unwrap_or(0))
    }

    /// A group member's quest log (empty for anyone else).
    pub fn quests_of(&self, player: EntityId) -> &QuestLog {
        self.quest_logs.get(&player).unwrap_or(&EMPTY_LOG)
    }

    /// Where the first player stands with quest `id`; `Locked` for an unknown id.
    pub fn quest_status(&self, id: &str) -> QuestStatus {
        self.quest_status_of(self.player().unwrap_or(0), id)
    }

    /// Where group member `player` stands with quest `id`; `Locked` for an unknown id.
    pub fn quest_status_of(&self, player: EntityId, id: &str) -> QuestStatus {
        let Some(def) = self.content.quest(id) else {
            return QuestStatus::Locked;
        };
        let log = self.quests_of(player);
        if log.completed.iter().any(|c| c == id) {
            return QuestStatus::Done;
        }
        if let Some(q) = log.active.iter().find(|q| q.id == id) {
            return if is_ready(def, q) {
                QuestStatus::Ready
            } else {
                QuestStatus::Active
            };
        }
        let level = self.unit(player).map_or(0, |u| u.level);
        let chain = def
            .requires
            .as_ref()
            .is_none_or(|r| log.completed.iter().any(|c| c == r));
        if chain && level >= def.level_min {
            QuestStatus::Available
        } else {
            QuestStatus::Locked
        }
    }

    /// A living player within talk range of a living `npc`, in its zone.
    fn near_npc(&self, player: EntityId, npc: &QuestGiver) -> bool {
        let Some(p) = self.unit(player).filter(|p| p.alive) else {
            return false;
        };
        if self.zone() != Some(npc.zone.as_str()) {
            return false;
        }
        let range = self.content.config.loot_range_m + GIVER_REACH_SLACK_M;
        self.units()
            .any(|u| u.alive && !u.hostile && u.key == npc.npc && in_range(u.pos, p.pos, range))
    }

    pub(crate) fn accept_quest(&mut self, pid: EntityId, id: &str) {
        let Some(def) = self.content.quest(id).cloned() else {
            return;
        };
        // The Company Calendar hands out the first quest of its chains from anywhere.
        if self.quest_status_of(pid, id) != QuestStatus::Available
            || !(self.near_npc(pid, &def.giver) || self.calendar_starts(id))
        {
            return;
        }
        // Work done before accepting counts at once: a cleared zone (a dead boss stays dead),
        // items carried and earlier kills.
        let log = self.quests_of(pid);
        let counts: Vec<u32> = def
            .objectives
            .iter()
            .map(|o| match o {
                Objective::ClearZone(z) if self.world.cleared.contains(z) => 1,
                Objective::Kill(k, n) => log.kills.get(k).map_or(0, |c| (*c).min(*n)),
                Objective::Loot(it, n) => self
                    .unit(pid)
                    .map_or(&[][..], |u| &u.inventory)
                    .iter()
                    .find(|(name, _)| name == it)
                    .map_or(0, |(_, c)| (*c).min(*n)),
                _ => 0,
            })
            .collect();
        let q = ActiveQuest {
            id: id.to_string(),
            counts: counts.clone(),
        };
        let ready = is_ready(&def, &q);
        self.quest_logs.entry(pid).or_default().active.push(q);
        self.quest_accept_rt(pid, &def);
        self.emit_to(
            pid,
            Event::QuestAccepted {
                quest: id.to_string(),
            },
        );
        // Remote logs start from zero on accept; retro credit reaches them as progress.
        for (k, (o, c)) in def.objectives.iter().zip(counts).enumerate() {
            if c > 0 {
                self.emit_to(
                    pid,
                    Event::QuestProgress {
                        quest: id.to_string(),
                        objective: k,
                        count: c,
                        need: o.need(),
                    },
                );
            }
        }
        if ready {
            self.emit_to(
                pid,
                Event::QuestReady {
                    quest: id.to_string(),
                },
            );
        }
        self.refill_quest_sources();
        self.check_unlocks();
    }

    /// `choice` indexes `Content::quest_gear_choices` for the player's role (clamped); a gear
    /// reward needs a free bag slot, else the turn-in is refused with `BagFull`.
    pub(crate) fn turn_in_quest(&mut self, pid: EntityId, id: &str, choice: u8) {
        let Some(def) = self.content.quest(id).cloned() else {
            return;
        };
        let Some(pi) = self.index(pid) else {
            return;
        };
        if self.quest_status_of(pid, id) != QuestStatus::Ready || !self.near_npc(pid, def.ender()) {
            return;
        }
        let role = self.class_at(pi).map(|c| c.role);
        let choices = self.content.quest_gear_choices(&def, role);
        let gear = choices
            .get(usize::from(choice).min(choices.len().saturating_sub(1)))
            .cloned();
        if gear.is_some() && self.bag_full(pi) {
            self.item_error(pid, ItemError::BagFull);
            return;
        }
        let log = self.quest_logs.entry(pid).or_default();
        log.active.retain(|q| q.id != id);
        log.completed.push(id.to_string());
        self.emit_to(
            pid,
            Event::QuestCompleted {
                quest: id.to_string(),
            },
        );
        if let Some(t) = &def.reward.trophy {
            self.add_trophies(pi, std::slice::from_ref(t));
        }
        if let Some(g) = gear {
            self.stow(pi, vec![g]);
            self.bag_event(pi);
        }
        self.grant_xp(pi, self.content.quest_xp(&def));
        self.quest_sp(pi, def.reward.xp, def.reward.sp);
        if let Some((d, amount)) = &def.reward.rep
            && let Some(dept) = self.content.endgame.department_index(d)
        {
            self.gain_rep(pi, dept, *amount);
        }
        self.add_burnout(
            pi,
            i32::from(def.reward.burnout),
            crate::burnout::BurnoutCause::Quest,
        );
        // A quest can carry a title: achievements keyed `quest:<id>` unlock on its turn-in.
        let key = format!("quest:{id}");
        self.check_achievements(
            pid,
            |t| matches!(t, crate::world::Trigger::Event(k) if *k == key),
        );
        self.check_unlocks();
    }

    /// Once a second: every living player in a zone counts a `Wait` objective of that zone.
    pub(crate) fn quest_waits(&mut self) {
        let Some(zone) = self.zone.clone() else {
            return;
        };
        for pid in self.players.clone() {
            if self.unit(pid).is_some_and(|u| u.alive) {
                self.quest_credit(
                    pid,
                    |o| matches!(o, Objective::Wait { zone: z, .. } if *z == zone),
                );
            }
        }
        self.quest_defends();
        self.quest_inspects();
        self.quest_deadlines();
    }

    /// `Defend` and `Escort` objectives, once a second: waves spawn while someone guards,
    /// guarding players count (a second held, or the share of the way walked), and a fallen
    /// unit resets everyone's count and stands up again at its start.
    fn quest_defends(&mut self) {
        // (unit, wave, every_ms, escort)
        let mut wanted: Vec<(String, String, u32, bool)> = Vec::new();
        for pid in &self.players {
            for q in &self.quests_of(*pid).active {
                let Some(def) = self.content.quest(&q.id) else {
                    continue;
                };
                for (o, c) in def.objectives.iter().zip(&q.counts) {
                    let (unit, wave, every_ms, escort) = match o {
                        Objective::Defend {
                            unit,
                            wave,
                            every_ms,
                            ..
                        } => (unit, wave, every_ms, false),
                        Objective::Escort {
                            unit,
                            wave,
                            every_ms,
                            ..
                        } => (unit, wave, every_ms, true),
                        _ => continue,
                    };
                    if *c < o.need() && !wanted.iter().any(|w| w.0 == *unit) {
                        wanted.push((unit.clone(), wave.clone(), *every_ms, escort));
                    }
                }
            }
        }
        self.quest_rt
            .next_wave
            .retain(|k, _| wanted.iter().any(|w| w.0 == *k));
        for (unit, wave, every_ms, escort) in wanted {
            let Some(gi) = self
                .units
                .iter()
                .position(|u| !u.despawned && !u.state.hostile && u.state.key == unit)
            else {
                continue;
            };
            if !self.units[gi].state.alive {
                self.defend_reset(gi, &unit, &wave);
                continue;
            }
            let at = self.units[gi].state.pos;
            let guards: Vec<EntityId> = self
                .players
                .iter()
                .copied()
                .filter(|p| {
                    self.unit(*p)
                        .is_some_and(|u| u.alive && in_range(u.pos, at, DEFEND_RANGE_M))
                })
                .collect();
            if guards.is_empty() {
                continue;
            }
            let pct = if escort {
                let Some(goal) = self.quest_rt.escorts.get(&unit).copied() else {
                    continue;
                };
                let total = dist_xz(self.units[gi].spawn_pos, goal).max(1.0);
                let left = dist_xz(at, goal);
                if left <= ESCORT_DONE_M {
                    100
                } else {
                    ((1.0 - left / total).clamp(0.0, 0.99) * 100.0) as u32
                }
            } else {
                0
            };
            let next = *self
                .quest_rt
                .next_wave
                .entry(unit.clone())
                .or_insert(self.now);
            let alive = self
                .units
                .iter()
                .filter(|u| !u.despawned && u.state.alive && u.state.key == wave)
                .count();
            if next <= self.now && alive < DEFEND_MAX_WAVES && pct < 100 {
                self.quest_rt
                    .next_wave
                    .insert(unit.clone(), self.now + ticks(every_ms));
                self.defend_wave(gi, &wave);
            }
            for p in guards {
                if escort {
                    self.quest_update(p, |o, c| match o {
                        Objective::Escort { unit: u, .. } if *u == unit => Some(c.max(pct)),
                        _ => None,
                    });
                } else {
                    self.quest_credit(
                        p,
                        |o| matches!(o, Objective::Defend { unit: u, .. } if *u == unit),
                    );
                }
            }
        }
    }

    /// One wave unit a few metres off the defended unit, already going for it. It counts as the
    /// defended unit's add, so it drops no loot or Kudos and leaves when that unit falls.
    fn defend_wave(&mut self, gi: usize, wave: &str) {
        let Ok(def) = self.def_index(wave) else {
            return;
        };
        let at = self.units[gi].state.pos;
        let k = self.rng.range_inclusive(0, 7) as f32;
        let a = k * std::f32::consts::TAU / 8.0;
        let pos = crate::geom::Vec3::new(at.x + 6.0 * a.cos(), at.y, at.z + 6.0 * a.sin());
        let id = self.spawn(
            def,
            pos,
            crate::ai::yaw_toward(pos, at),
            crate::sim::SpawnOpts {
                level: None,
                pack: None,
                respawns: false,
                summoner: Some(Self::id_of(gi)),
                floor_boss: false,
                zone_slot: None,
            },
        );
        if let Some(wi) = self.index(id) {
            self.add_threat(wi, Self::id_of(gi), DEFEND_WAVE_THREAT);
            self.set_ai(wi, crate::types::AiState::Chase);
        }
    }

    /// The defended unit fell: every guard starts over, the waves leave and it stands up again.
    fn defend_reset(&mut self, gi: usize, unit: &str, wave: &str) {
        for pid in self.players.clone() {
            let Some(log) = self.quest_logs.get_mut(&pid) else {
                continue;
            };
            for q in &mut log.active {
                let Some(def) = self.content.quests.iter().find(|d| d.id == q.id) else {
                    continue;
                };
                for (k, o) in def.objectives.iter().enumerate() {
                    let hit = matches!(
                        o,
                        Objective::Defend { unit: u, .. } | Objective::Escort { unit: u, .. }
                            if u == unit
                    );
                    if let Some(c) = q.counts.get_mut(k)
                        && hit
                        && *c > 0
                        && *c < o.need()
                    {
                        *c = 0;
                        self.routed.push((self.events.len(), pid));
                        self.events.push(Event::QuestProgress {
                            quest: q.id.clone(),
                            objective: k,
                            count: 0,
                            need: o.need(),
                        });
                    }
                }
            }
        }
        for i in 0..self.units.len() {
            if !self.units[i].despawned && self.units[i].state.key == wave {
                self.despawn(i);
            }
        }
        let (def, pos, yaw) = (
            self.units[gi].def,
            self.units[gi].spawn_pos,
            self.units[gi].spawn_yaw,
        );
        self.despawn(gi);
        self.spawn(
            def,
            pos,
            yaw,
            crate::sim::SpawnOpts {
                level: None,
                pack: None,
                respawns: false,
                summoner: None,
                floor_boss: false,
                zone_slot: None,
            },
        );
        self.quest_rt.next_wave.remove(unit);
    }

    /// A telegraph landed on `pid`: clean kills of fights already under way are off.
    pub(crate) fn note_telegraph_hit(&mut self, pid: EntityId) {
        if self.is_player(pid) {
            self.quest_rt.telegraph_hit.insert(pid, self.now);
        }
    }

    /// `KillClean` credit for a kill of the unit at `ti` that `pid` shares.
    pub(crate) fn record_clean_kill(&mut self, pid: EntityId, ti: usize) {
        let since = self.units[ti].pulled_at;
        let hit = self.quest_rt.telegraph_hit.get(&pid).copied();
        if hit.is_some_and(|t| t >= since) {
            return;
        }
        let key = self.units[ti].state.key.clone();
        self.quest_credit(
            pid,
            |o| matches!(o, Objective::KillClean(k, _) if *k == key),
        );
    }

    /// Counts a kill (own or shared) of unit `key` for `pid`'s retro credit, then the quests.
    pub(crate) fn record_kill(&mut self, pid: EntityId, key: &str) {
        *self
            .quest_logs
            .entry(pid)
            .or_default()
            .kills
            .entry(key.to_string())
            .or_default() += 1;
        self.quest_credit(pid, |o| matches!(o, Objective::Kill(k, _) if k == key));
        self.task_kill(pid, key);
    }

    /// Whether quest-only drop `item` of `quest` should roll for `pid`: active, objective not full.
    pub(crate) fn wants_quest_drop(&self, pid: EntityId, quest: &str, item: &str) -> bool {
        let (Some(def), Some(q)) = (
            self.content.quest(quest),
            self.quests_of(pid).active.iter().find(|q| q.id == quest),
        ) else {
            return false;
        };
        def.objectives
            .iter()
            .zip(&q.counts)
            .any(|(o, c)| matches!(o, Objective::Loot(it, n) if it == item && c < n))
    }

    pub(crate) fn abandon_quest(&mut self, pid: EntityId, id: &str) {
        if let Some(log) = self.quest_logs.get_mut(&pid) {
            log.active.retain(|q| q.id != id);
        }
        self.quest_rt.deadlines.remove(&(pid, id.to_string()));
    }

    /// Debug/test: forces quest `id` into `Active` (no progress), `Ready` (objectives full) or
    /// `Done` (completed without reward), ignoring givers, chains and levels; silent. `Available`
    /// or `Locked` drop it from the log. False for an unknown quest or without a player.
    pub fn debug_set_quest(&mut self, id: &str, status: QuestStatus) -> bool {
        let (Some(def), Some(pid)) = (self.content.quest(id), self.player()) else {
            return false;
        };
        let full: Vec<u32> = def.objectives.iter().map(Objective::need).collect();
        let log = self.quest_logs.entry(pid).or_default();
        log.active.retain(|q| q.id != id);
        log.completed.retain(|q| q != id);
        match status {
            QuestStatus::Active | QuestStatus::Ready => log.active.push(ActiveQuest {
                id: id.to_string(),
                counts: if status == QuestStatus::Ready {
                    full
                } else {
                    vec![0; full.len()]
                },
            }),
            QuestStatus::Done => log.completed.push(id.to_string()),
            QuestStatus::Available | QuestStatus::Locked => {}
        }
        self.check_unlocks();
        true
    }

    /// Debug/test: sets the first player's objective counts of active quest `id` (capped at
    /// each need); silent. False when the quest is not active.
    pub fn debug_quest_counts(&mut self, id: &str, counts: &[u32]) -> bool {
        let (Some(def), Some(pid)) = (self.content.quest(id), self.player()) else {
            return false;
        };
        let Some(q) = self
            .quest_logs
            .get_mut(&pid)
            .and_then(|l| l.active.iter_mut().find(|q| q.id == id))
        else {
            return false;
        };
        for (k, (c, o)) in q.counts.iter_mut().zip(&def.objectives).enumerate() {
            *c = counts.get(k).copied().unwrap_or(*c).min(o.need());
        }
        true
    }

    /// Adds one to every matching objective of `pid`'s active quests; true if any counted.
    pub(crate) fn quest_credit(&mut self, pid: EntityId, hit: impl Fn(&Objective) -> bool) -> bool {
        self.quest_update(pid, |o, c| hit(o).then_some(c + 1))
    }

    /// Sets every unfinished objective of `pid`'s active quests that `f` maps (objective,
    /// count) to a new count (capped at its need); true if any count changed.
    pub(crate) fn quest_update(
        &mut self,
        pid: EntityId,
        f: impl Fn(&Objective, u32) -> Option<u32>,
    ) -> bool {
        let Some(log) = self.quest_logs.get_mut(&pid) else {
            return false;
        };
        let mut changed = false;
        for q in &mut log.active {
            let Some(def) = self.content.quests.iter().find(|d| d.id == q.id) else {
                continue;
            };
            let was_ready = is_ready(def, q);
            for (k, o) in def.objectives.iter().enumerate() {
                let need = o.need();
                if let Some(c) = q.counts.get_mut(k)
                    && *c < need
                    && let Some(n) = f(o, *c).map(|n| n.min(need))
                    && n != *c
                {
                    *c = n;
                    changed = true;
                    self.routed.push((self.events.len(), pid));
                    self.events.push(Event::QuestProgress {
                        quest: q.id.clone(),
                        objective: k,
                        count: *c,
                        need,
                    });
                }
            }
            if !was_ready && is_ready(def, q) {
                self.routed.push((self.events.len(), pid));
                self.events.push(Event::QuestReady {
                    quest: q.id.clone(),
                });
            }
        }
        changed
    }

    /// `QuestOffered` for a giver with quests `pid` can accept or turn in.
    pub(crate) fn offer_quests(&mut self, pid: EntityId, npc: usize) {
        let key = &self.units[npc].state.key;
        let quests: Vec<String> = self
            .content
            .quests
            .iter()
            .filter(|q| match self.quest_status_of(pid, &q.id) {
                QuestStatus::Available => q.giver.npc == *key,
                QuestStatus::Ready => q.ender().npc == *key,
                _ => false,
            })
            .map(|q| q.id.clone())
            .collect();
        if !quests.is_empty() {
            let npc = Self::id_of(npc);
            self.emit_to(pid, Event::QuestOffered { npc, quests });
        }
    }
}

/// The interactive objectives: props, item hand-overs, deliveries and escorts.
impl Sim {
    /// Called by `load_zone`: where this zone's escorts head and its inspect spots stand.
    pub(crate) fn load_quests(
        &mut self,
        zone: &str,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    ) {
        self.quest_rt.inspect_spots = self
            .content
            .world
            .props
            .iter()
            .enumerate()
            .filter(|(_, p)| p.zone == zone && p.kind == PropKind::Inspect)
            .filter_map(|(k, p)| anchors(&format!("prop:{}", p.key)).map(|(at, yaw)| (k, at, yaw)))
            .collect();
        self.quest_rt.escorts.clear();
        let Some(z) = self.content.zone(zone) else {
            return;
        };
        for o in self.content.quests.iter().flat_map(|q| &q.objectives) {
            if let Objective::Escort { unit, to, .. } = o
                && z.friendly.iter().any(|f| f.unit == *unit)
                && let Some((goal, _)) = anchors(to)
            {
                self.quest_rt.escorts.insert(unit.clone(), goal);
            }
        }
    }

    /// Escorted units of the loaded zone do not wander off on their own.
    pub(crate) fn escorted(&self, i: usize) -> bool {
        self.quest_rt.escorts.contains_key(&self.units[i].state.key)
    }

    /// A quest was accepted: its pickups and spots are fresh again and its delivery clock runs.
    fn quest_accept_rt(&mut self, pid: EntityId, def: &QuestDef) {
        for o in &def.objectives {
            match o {
                Objective::Collect(tag, _) | Objective::Inspect(tag, _) => {
                    let props = &self.content.world.props;
                    if let Some(used) = self.quest_rt.used_props.get_mut(&pid) {
                        used.retain(|k| props.get(*k).is_none_or(|p| p.tag != *tag));
                    }
                }
                Objective::Deliver { secs, .. } if *secs > 0 => {
                    self.quest_rt
                        .deadlines
                        .insert((pid, def.id.clone()), self.now + ticks(secs * 1000));
                }
                _ => {}
            }
        }
    }

    /// A player used quest prop `k` (reach already checked): pickups and inspect spots count
    /// once, devices advance a sequence or take a `UseOn` item. Answers with `Interacted`
    /// (`QUEST_PROP_*` outcome) when something happened, and always for devices.
    pub(crate) fn use_quest_prop(&mut self, pid: EntityId, k: usize) {
        let Some(def) = self.content.world.props.get(k).cloned() else {
            return;
        };
        let used = self
            .quest_rt
            .used_props
            .get(&pid)
            .is_some_and(|s| s.contains(&k));
        let mut outcome = QUEST_PROP_IDLE;
        let mut take = false;
        match def.kind {
            PropKind::Pickup if !used => {
                take = self.quest_credit(
                    pid,
                    |o| matches!(o, Objective::Collect(t, _) if *t == def.tag),
                );
            }
            PropKind::Inspect if !used => {
                take = self.quest_credit(
                    pid,
                    |o| matches!(o, Objective::Inspect(t, _) if *t == def.tag),
                );
            }
            PropKind::Device => {
                let wrong = std::cell::Cell::new(false);
                let stepped = self.quest_update(pid, |o, c| match o {
                    Objective::Sequence { props, .. } => {
                        let at = c as usize;
                        if props.get(at) == Some(&def.key) {
                            Some(c + 1)
                        } else if props.iter().skip(at + 1).any(|p| *p == def.key) {
                            wrong.set(true);
                            Some(0)
                        } else {
                            None
                        }
                    }
                    _ => None,
                });
                // A step too early when nothing was done yet changes no count: still say so.
                let skipped = wrong.get();
                if skipped {
                    outcome = QUEST_PROP_WRONG_ORDER;
                } else if stepped {
                    outcome = QUEST_PROP_COUNTED;
                }
                if !used {
                    take = self.quest_credit(pid, |o| {
                        matches!(o, Objective::UseOn { target, .. }
                            if *target == def.key || (!def.tag.is_empty() && *target == def.tag))
                    });
                }
            }
            _ => {}
        }
        if take {
            self.quest_rt.used_props.entry(pid).or_default().insert(k);
            outcome = QUEST_PROP_COUNTED;
        }
        if outcome != QUEST_PROP_IDLE || def.kind == PropKind::Device {
            self.events.push(Event::Interacted {
                unit: pid,
                prop: def.key,
                outcome,
            });
        }
    }

    /// Talking to (or, for a hostile, pressing F on) the unit at `ti` within reach: deliveries
    /// to it and items handed to it count.
    pub(crate) fn quest_talk(&mut self, pid: EntityId, ti: usize) {
        let key = self.units[ti].state.key.clone();
        let uid = Self::id_of(ti);
        if !self.units[ti].state.hostile {
            self.quest_credit(
                pid,
                |o| matches!(o, Objective::Deliver { npc, .. } if *npc == key),
            );
        }
        let used = self
            .quest_rt
            .used_units
            .get(&pid)
            .is_some_and(|s| s.contains(&uid));
        if !used
            && self.quest_credit(
                pid,
                |o| matches!(o, Objective::UseOn { target, .. } if *target == key),
            )
        {
            self.quest_rt.used_units.entry(pid).or_default().insert(uid);
        }
    }

    /// Whether `pid` has an unfinished `UseOn` objective for units with key `key`.
    pub fn wants_use_on(&self, pid: EntityId, key: &str) -> bool {
        self.quests_of(pid).active.iter().any(|q| {
            self.content.quest(&q.id).is_some_and(|def| {
                def.objectives.iter().zip(&q.counts).any(|(o, c)| {
                    matches!(o, Objective::UseOn { target, .. } if target == key) && *c < o.need()
                })
            })
        })
    }

    /// Once a second: players standing at an `Inspect` spot, facing what it shows, count it.
    fn quest_inspects(&mut self) {
        if self.quest_rt.inspect_spots.is_empty() {
            return;
        }
        for pid in self.players.clone() {
            let Some((pos, yaw)) = self.unit(pid).filter(|u| u.alive).map(|u| (u.pos, u.yaw))
            else {
                continue;
            };
            for (k, at, face) in self.quest_rt.inspect_spots.clone() {
                let Some(reach) = self.content.world.props.get(k).map(|p| p.reach_m) else {
                    continue;
                };
                let turn = (yaw - face + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                if in_range(pos, at, reach) && turn.abs() <= INSPECT_FACING_RAD {
                    self.use_quest_prop(pid, k);
                }
            }
        }
    }

    /// Once a second: a delivery whose time ran out fails its quest. A timed delivery without a
    /// clock (a loaded save, a debug-forced quest) gets a full one now.
    fn quest_deadlines(&mut self) {
        let now = self.now;
        for (pid, log) in &self.quest_logs {
            for q in &log.active {
                let secs = self.content.quest(&q.id).and_then(|d| {
                    d.objectives.iter().find_map(|o| match o {
                        Objective::Deliver { secs, .. } if *secs > 0 => Some(*secs),
                        _ => None,
                    })
                });
                if let Some(secs) = secs {
                    self.quest_rt
                        .deadlines
                        .entry((*pid, q.id.clone()))
                        .or_insert(now + ticks(secs * 1000));
                }
            }
        }
        let logs = &self.quest_logs;
        self.quest_rt.deadlines.retain(|(pid, quest), _| {
            logs.get(pid)
                .is_some_and(|l| l.active.iter().any(|q| q.id == *quest))
        });
        let late: Vec<(EntityId, String)> = self
            .quest_rt
            .deadlines
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(k, _)| k.clone())
            .collect();
        for (pid, quest) in late {
            self.quest_rt.deadlines.remove(&(pid, quest.clone()));
            let delivered = self.content.quest(&quest).is_some_and(|def| {
                self.quests_of(pid)
                    .active
                    .iter()
                    .find(|q| q.id == quest)
                    .is_some_and(|q| {
                        def.objectives
                            .iter()
                            .zip(&q.counts)
                            .all(|(o, c)| !matches!(o, Objective::Deliver { .. }) || *c >= o.need())
                    })
            });
            if delivered {
                continue;
            }
            if let Some(log) = self.quest_logs.get_mut(&pid) {
                log.active.retain(|q| q.id != quest);
            }
            self.emit_to(pid, Event::QuestFailed { quest });
        }
    }

    /// Once a tick: escorted units walk towards their goal while someone with the quest is near.
    pub(crate) fn update_escorts(&mut self) {
        if self.quest_rt.escorts.is_empty() {
            return;
        }
        for (unit, goal) in self.quest_rt.escorts.clone() {
            let Some(gi) = self.units.iter().position(|u| {
                !u.despawned && u.state.alive && !u.state.hostile && u.state.key == unit
            }) else {
                continue;
            };
            let pos = self.units[gi].state.pos;
            if dist_xz(pos, goal) <= ESCORT_DONE_M {
                continue;
            }
            let guarded = self.players.iter().any(|p| {
                self.unit(*p)
                    .is_some_and(|u| u.alive && in_range(u.pos, pos, DEFEND_RANGE_M))
                    && self.wants_escort(*p, &unit)
            });
            if !guarded {
                continue;
            }
            let Some((target, _)) = self.nav_target(gi, goal) else {
                continue;
            };
            self.units[gi].state.yaw = crate::ai::yaw_toward(pos, target);
            let step = ESCORT_MPS / crate::time::TICK_HZ as f32;
            self.units[gi].state.pos = crate::ai::step_toward(pos, target, step, 0.0);
        }
    }

    fn wants_escort(&self, pid: EntityId, key: &str) -> bool {
        self.quests_of(pid).active.iter().any(|q| {
            self.content.quest(&q.id).is_some_and(|def| {
                def.objectives.iter().zip(&q.counts).any(|(o, c)| {
                    matches!(o, Objective::Escort { unit, .. } if unit == key) && *c < o.need()
                })
            })
        })
    }

    /// Ticks left on `pid`'s delivery for `quest`, if its clock runs.
    pub fn deliver_ticks_left(&self, pid: EntityId, quest: &str) -> Option<u64> {
        self.quest_rt
            .deadlines
            .get(&(pid, quest.to_string()))
            .map(|at| at.saturating_sub(self.now))
    }
}

impl Content {
    /// "Choose one" gear for a quest and role (`gear_choices` of its tier; Epic only for group
    /// quests).
    pub fn quest_gear_choices(&self, def: &QuestDef, role: Option<Role>) -> Vec<String> {
        // FNV-1a of the quest id: a stable per-quest rotation of the candidates.
        let seed = def.id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        self.gear_choices(def.reward.gear_tier, def.group, role, seed)
    }

    /// Zones where an objective can be worked on: the zones spawning the unit (pack or boss),
    /// dropping the item, hosting the NPC, or the zone itself. Content order.
    pub fn objective_zones<'a>(&'a self, o: &'a Objective) -> Vec<&'a str> {
        let spawns = |key: &str| {
            self.zones
                .iter()
                .filter(|z| {
                    z.packs.iter().any(|p| p.units.iter().any(|u| u == key))
                        || z.boss.as_ref().is_some_and(|b| b.unit == key)
                        || z.rares.iter().any(|r| r.unit == key)
                        // Fixed spawns can be hostile too (the campus training dummy).
                        || z.friendly.iter().any(|f| f.unit == key)
                })
                .map(|z| z.id.as_str())
                .collect::<Vec<_>>()
        };
        match o {
            Objective::Kill(k, _) | Objective::KillClean(k, _) => spawns(k),
            Objective::Defend { unit, .. } => self
                .zones
                .iter()
                .filter(|z| z.friendly.iter().any(|f| f.unit == *unit))
                .map(|z| z.id.as_str())
                .collect(),
            Objective::Loot(it, _) => {
                let mut out: Vec<&str> = Vec::new();
                for u in self
                    .units
                    .iter()
                    .filter(|u| u.loot.iter().any(|l| l.item == *it))
                {
                    for z in spawns(&u.key) {
                        if !out.contains(&z) {
                            out.push(z);
                        }
                    }
                }
                out
            }
            Objective::Talk(k) => self
                .zones
                .iter()
                .filter(|z| z.friendly.iter().any(|f| f.unit == *k))
                .map(|z| z.id.as_str())
                .collect(),
            Objective::ClearZone(z) | Objective::Wait { zone: z, .. } => vec![z.as_str()],
            Objective::Interact(k, _) => {
                self.prop(k).map(|p| p.zone.as_str()).into_iter().collect()
            }
            Objective::Collect(tag, _) | Objective::Inspect(tag, _) => {
                self.prop_zones(|p| p.tag == *tag)
            }
            Objective::Sequence { props, .. } => self.prop_zones(|p| props.contains(&p.key)),
            Objective::UseOn { target, .. } => {
                let mut out = spawns(target);
                for z in self.prop_zones(|p| p.key == *target || p.tag == *target) {
                    if !out.contains(&z) {
                        out.push(z);
                    }
                }
                out
            }
            Objective::Deliver { npc: unit, .. } | Objective::Escort { unit, .. } => self
                .zones
                .iter()
                .filter(|z| z.friendly.iter().any(|f| f.unit == *unit))
                .map(|z| z.id.as_str())
                .collect(),
        }
    }

    /// Zones of the props `f` picks, content order of the zones.
    fn prop_zones(&self, f: impl Fn(&crate::world::PropDef) -> bool) -> Vec<&str> {
        self.zones
            .iter()
            .filter(|z| self.world.props.iter().any(|p| p.zone == z.id && f(p)))
            .map(|z| z.id.as_str())
            .collect()
    }

    /// Props of `kind` carrying `tag`.
    pub fn tagged_props(&self, tag: &str, kind: PropKind) -> u32 {
        self.world
            .props
            .iter()
            .filter(|p| p.kind == kind && p.tag == tag)
            .count() as u32
    }
}
