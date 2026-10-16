//! Quests: a log per player, advanced by sim outcomes (the player's kills, loot pickups, talking,
//! zone clears). No rng is involved. Kills are also counted per unit key for the whole run, so a
//! kill quest accepted after the work was done is ready at once (retro credit), like a cleared
//! zone or carried items.

use crate::content::{Content, Objective, QuestDef, QuestGiver, Role};
use crate::geom::in_range;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{Event, ItemError};
use std::collections::BTreeMap;

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

    /// Adds one to every matching objective of `pid`'s active quests.
    pub(crate) fn quest_credit(&mut self, pid: EntityId, hit: impl Fn(&Objective) -> bool) {
        let Some(log) = self.quest_logs.get_mut(&pid) else {
            return;
        };
        for q in &mut log.active {
            let Some(def) = self.content.quests.iter().find(|d| d.id == q.id) else {
                continue;
            };
            let was_ready = is_ready(def, q);
            for (k, o) in def.objectives.iter().enumerate() {
                let need = o.need();
                if let Some(c) = q.counts.get_mut(k)
                    && *c < need
                    && hit(o)
                {
                    *c += 1;
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
                        // Fixed spawns can be hostile too (the campus training dummy).
                        || z.friendly.iter().any(|f| f.unit == key)
                })
                .map(|z| z.id.as_str())
                .collect::<Vec<_>>()
        };
        match o {
            Objective::Kill(k, _) => spawns(k),
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
        }
    }
}
