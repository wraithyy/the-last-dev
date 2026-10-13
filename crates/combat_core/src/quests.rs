//! Quests: a log per player, advanced by sim outcomes (the player's kills, loot pickups, talking,
//! zone clears). No rng is involved.

use crate::content::{Objective, QuestDef};
use crate::geom::in_range;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::Event;

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
}

static EMPTY_LOG: QuestLog = QuestLog {
    active: Vec::new(),
    completed: Vec::new(),
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

    /// A living player within talk range of a living giver of the quest, in the giver's zone.
    fn near_giver(&self, player: EntityId, def: &QuestDef) -> bool {
        let Some(p) = self.unit(player).filter(|p| p.alive) else {
            return false;
        };
        if self.zone() != Some(def.giver.zone.as_str()) {
            return false;
        }
        let range = self.content.config.loot_range_m + GIVER_REACH_SLACK_M;
        self.units().any(|u| {
            u.alive && !u.hostile && u.key == def.giver.npc && in_range(u.pos, p.pos, range)
        })
    }

    pub(crate) fn accept_quest(&mut self, pid: EntityId, id: &str) {
        let Some(def) = self.content.quest(id).cloned() else {
            return;
        };
        if self.quest_status_of(pid, id) != QuestStatus::Available || !self.near_giver(pid, &def) {
            return;
        }
        // A zone cleared or items carried before accepting count at once (a dead boss stays dead).
        let counts = def
            .objectives
            .iter()
            .map(|o| match o {
                Objective::ClearZone(z) if self.world.cleared.contains(z) => 1,
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
            counts,
        };
        let ready = is_ready(&def, &q);
        self.quest_logs.entry(pid).or_default().active.push(q);
        self.emit_to(
            pid,
            Event::QuestAccepted {
                quest: id.to_string(),
            },
        );
        if ready {
            self.emit_to(
                pid,
                Event::QuestReady {
                    quest: id.to_string(),
                },
            );
        }
    }

    pub(crate) fn turn_in_quest(&mut self, pid: EntityId, id: &str) {
        let Some(def) = self.content.quest(id).cloned() else {
            return;
        };
        let Some(pi) = self.index(pid) else {
            return;
        };
        if self.quest_status_of(pid, id) != QuestStatus::Ready || !self.near_giver(pid, &def) {
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
        self.grant_xp(pi, def.reward.xp);
        self.check_unlocks();
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
            .filter(|q| q.giver.npc == *key)
            .filter(|q| {
                matches!(
                    self.quest_status_of(pid, &q.id),
                    QuestStatus::Available | QuestStatus::Ready
                )
            })
            .map(|q| q.id.clone())
            .collect();
        if !quests.is_empty() {
            let npc = Self::id_of(npc);
            self.emit_to(pid, Event::QuestOffered { npc, quests });
        }
    }
}
