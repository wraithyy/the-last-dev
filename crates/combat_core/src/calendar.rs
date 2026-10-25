//! The Company Calendar: the one board on the campus plaza that lists every side activity,
//! minigames (`minigames`, started with `Command::StartMinigame`) and short quest chains
//! (started by accepting their first quest from anywhere). With `rotation` off, the default,
//! everything is listed all the time; with it on, one minigame and one chain per UTC day.
//! It also takes Jira tickets: using the board (or the Scrum board) closes every `ticket`
//! junk item in the bag for Kudos. Content: `calendar` in `game.ron`.

use crate::content::{Content, ContentError};
use crate::quests::QuestLog;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::Event;
use serde::Deserialize;

/// The board pays this many times a ticket's vendor price: closing tickets is the job.
pub const TICKET_BOARD_MULT: u32 = 2;

/// What an entry pays, shown as icons on its row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum CalendarReward {
    Kudos,
    Rep,
    Title,
    Trophy,
    Pet,
    Gear,
    /// Lowers burnout.
    Relief,
    /// Raises burnout (the Expense Report).
    Burnout,
}

/// A minigame slot. Playable once `minigames` has a `MinigameDef` with the same key; until
/// then the row reads "Coming soon".
#[derive(Clone, Debug, Deserialize)]
pub struct CalendarGame {
    pub key: String,
    pub name: String,
    pub blurb: String,
    pub length: String,
    #[serde(default)]
    pub rewards: Vec<CalendarReward>,
}

/// A quest chain: `quests` in order, each requiring the one before.
#[derive(Clone, Debug, Deserialize)]
pub struct CalendarChain {
    pub key: String,
    pub name: String,
    pub blurb: String,
    pub length: String,
    pub quests: Vec<String>,
    #[serde(default)]
    pub rewards: Vec<CalendarReward>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CalendarDef {
    /// Off: everything is listed. On: one minigame and one chain per UTC day.
    #[serde(default)]
    pub rotation: bool,
    #[serde(default)]
    pub minigames: Vec<CalendarGame>,
    #[serde(default)]
    pub chains: Vec<CalendarChain>,
}

/// Where a player stands with a chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainStatus {
    New,
    /// Some quest of the chain is active or done, the last one is not.
    InProgress,
    Done,
}

impl CalendarChain {
    pub fn status(&self, log: &QuestLog) -> ChainStatus {
        let done = |id: &String| log.completed.contains(id);
        let active = |id: &String| log.active.iter().any(|q| q.id == *id);
        if self.quests.last().is_some_and(done) {
            ChainStatus::Done
        } else if self.quests.iter().any(|q| done(q) || active(q)) {
            ChainStatus::InProgress
        } else {
            ChainStatus::New
        }
    }

    /// The quest the chain is on: the first one not completed.
    pub fn current<'a>(&'a self, log: &QuestLog) -> Option<&'a str> {
        self.quests
            .iter()
            .find(|q| !log.completed.contains(q))
            .map(String::as_str)
    }
}

impl CalendarDef {
    /// What the board lists on UTC day `day`: everything, or with `rotation` one of each.
    pub fn listing(&self, day: u32) -> (Vec<&CalendarGame>, Vec<&CalendarChain>) {
        if !self.rotation {
            return (
                self.minigames.iter().collect(),
                self.chains.iter().collect(),
            );
        }
        let pick = |n: usize| (n > 0).then(|| day as usize % n);
        (
            pick(self.minigames.len())
                .map(|k| vec![&self.minigames[k]])
                .unwrap_or_default(),
            pick(self.chains.len())
                .map(|k| vec![&self.chains[k]])
                .unwrap_or_default(),
        )
    }

    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("calendar: {m}")));
        for g in &self.minigames {
            if g.key.is_empty() || g.key.len() > crate::minigame::MINIGAME_KEY_MAX {
                return bad(format!("bad minigame key {}", g.key));
            }
        }
        for ch in &self.chains {
            if ch.quests.is_empty() {
                return bad(format!("{}: no quests", ch.key));
            }
            for (k, id) in ch.quests.iter().enumerate() {
                let Some(q) = c.quest(id) else {
                    return bad(format!("{}: unknown quest {id}", ch.key));
                };
                // The head may be gated (it only hides the giver's "!": the board starts it at any
                // time); the rest follow the quest before them.
                if let Some(prev) = k.checked_sub(1).map(|p| &ch.quests[p])
                    && q.requires.as_ref() != Some(prev)
                {
                    return bad(format!("{}: {id} must require the quest before it", ch.key));
                }
            }
        }
        for m in &c.minigames {
            if !self.minigames.iter().any(|g| g.key == m.key) {
                return bad(format!("minigame {} is not on the calendar", m.key));
            }
        }
        Ok(())
    }
}

impl Sim {
    /// UTC day for the rotation (0 without a world clock: tests and the first frame).
    fn calendar_day(&self) -> u32 {
        crate::live::day_index(self.unix_s())
    }

    /// `quest` opens a chain listed today, so the calendar may hand it out from anywhere.
    pub(crate) fn calendar_starts(&self, quest: &str) -> bool {
        let (_, chains) = self.content.calendar.listing(self.calendar_day());
        chains
            .iter()
            .any(|c| c.quests.first().is_some_and(|q| q == quest))
    }

    /// Closes every Jira ticket in `i`'s bag for `TICKET_BOARD_MULT` times its price.
    pub(crate) fn close_tickets(&mut self, i: usize) {
        let eco = &self.content.economy;
        let bag = &self.units[i].state.bag;
        let tickets: Vec<(usize, u32)> = (0..bag.len())
            .filter_map(|k| {
                let j = eco.junk.iter().find(|j| j.name == bag[k] && j.ticket)?;
                Some((k, j.price.saturating_mul(TICKET_BOARD_MULT)))
            })
            .collect();
        if tickets.is_empty() {
            return;
        }
        let sp: u32 = tickets.iter().map(|t| t.1).sum();
        let s = &mut self.units[i].state;
        for (k, _) in tickets.iter().rev() {
            s.bag.remove(*k);
        }
        s.sp = s.sp.saturating_add(sp);
        let unit: EntityId = Self::id_of(i);
        self.emit_to(
            unit,
            Event::TicketsClosed {
                unit,
                count: tickets.len() as u32,
                sp,
            },
        );
        self.bag_event(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(key: &str) -> CalendarGame {
        CalendarGame {
            key: key.into(),
            name: key.into(),
            blurb: String::new(),
            length: String::new(),
            rewards: Vec::new(),
        }
    }

    fn chain(key: &str, quests: &[&str]) -> CalendarChain {
        CalendarChain {
            key: key.into(),
            name: key.into(),
            blurb: String::new(),
            length: String::new(),
            quests: quests.iter().map(|q| q.to_string()).collect(),
            rewards: Vec::new(),
        }
    }

    #[test]
    fn rotation_off_lists_everything_and_on_lists_one_of_each_per_day() {
        let mut c = CalendarDef {
            rotation: false,
            minigames: vec![game("a"), game("b"), game("c")],
            chains: vec![chain("x", &["x1"]), chain("y", &["y1"])],
        };
        let (g, ch) = c.listing(7);
        assert_eq!((g.len(), ch.len()), (3, 2));
        c.rotation = true;
        let (g, ch) = c.listing(7);
        assert_eq!(g.iter().map(|g| &g.key).collect::<Vec<_>>(), ["b"]);
        assert_eq!(ch.iter().map(|c| &c.key).collect::<Vec<_>>(), ["y"]);
        assert_eq!(c.listing(8).0[0].key, "c");
    }

    #[test]
    fn chain_status_follows_the_log() {
        let ch = chain("x", &["x1", "x2"]);
        let mut log = QuestLog::default();
        assert_eq!(ch.status(&log), ChainStatus::New);
        assert_eq!(ch.current(&log), Some("x1"));
        log.active.push(crate::quests::ActiveQuest {
            id: "x1".into(),
            counts: vec![0],
        });
        assert_eq!(ch.status(&log), ChainStatus::InProgress);
        log.active.clear();
        log.completed.push("x1".into());
        assert_eq!(ch.status(&log), ChainStatus::InProgress);
        assert_eq!(ch.current(&log), Some("x2"));
        log.completed.push("x2".into());
        assert_eq!(ch.status(&log), ChainStatus::Done);
        assert_eq!(ch.current(&log), None);
    }
}
