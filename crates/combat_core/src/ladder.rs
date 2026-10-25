//! The Ilmari ladder (docs/ILMARI_1_0.md D.10): the weekly boards of the core fight on Full and on
//! Judgment Day, and the weekly Directive.
//!
//! Everything here is sim state. A row is written by the sim when the core falls (the sim's own
//! clock from the pull to the revert, the deaths the sim counted, the room's members), never by a
//! command, so nobody can claim a time. A board is the room's own list (offline: the save's), a
//! new ISO week empties it, and nothing is global. Story has no ladder.
//!
//! The Directive is one of the Culture Committee's policies (`ilmari.ladder.directives`), picked
//! from the week seed and worn by every member for the whole ranked fight.

use crate::content::{Content, ContentError};
use crate::ilmari::{IlmariMode, TOKEN_WEEK_CAP};
use crate::rng::Rng;
use crate::sim::Sim;
use crate::sprint::{LadderEntry, LadderView, ladder_insert, ladder_label, ladder_restored};
use crate::targeting::EntityId;
use crate::types::Event;
use serde::{Deserialize, Serialize};

/// Which board a `LadderView` is: the Weekly Sprint's, or the core on one of its ranked difficulties.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LadderBoard {
    #[default]
    Sprint,
    IlmariFull,
    IlmariJudgment,
}

impl LadderBoard {
    /// The two Ilmari boards.
    pub const ILMARI: [LadderBoard; 2] = [LadderBoard::IlmariFull, LadderBoard::IlmariJudgment];

    /// The board a core kill on `mode` lands on (Story has none).
    pub fn of_mode(mode: IlmariMode) -> Option<LadderBoard> {
        match mode {
            IlmariMode::Story => None,
            IlmariMode::Full => Some(LadderBoard::IlmariFull),
            IlmariMode::Judgment => Some(LadderBoard::IlmariJudgment),
        }
    }

    /// The difficulty field of the `ilmari_ladder` save line (1 Full, 2 Judgment).
    pub fn code(self) -> u8 {
        match self {
            LadderBoard::Sprint => 0,
            LadderBoard::IlmariFull => 1,
            LadderBoard::IlmariJudgment => 2,
        }
    }

    pub fn from_code(c: &str) -> Option<LadderBoard> {
        match c {
            "1" => Some(LadderBoard::IlmariFull),
            "2" => Some(LadderBoard::IlmariJudgment),
            _ => None,
        }
    }
}

/// The ladder's content (`ilmari.ladder`): the Directive table and the weekly pay.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LadderDef {
    /// Policy auras of the Culture Committee's table; the week seed picks one.
    pub directives: Vec<String>,
    /// Ilmari tokens for every member of a ranked kill (inside the weekly token cap).
    pub entry_tokens: u32,
}

impl LadderDef {
    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        if let Some(d) = self.directives.iter().find(|d| c.aura(d).is_none()) {
            return bad(format!("ladder directive {d} is not an aura"));
        }
        if self.entry_tokens > TOKEN_WEEK_CAP {
            return bad("ladder entry tokens sit under the weekly token cap".into());
        }
        Ok(())
    }
}

/// The two boards of the room (offline: of the save).
#[derive(Default)]
pub(crate) struct IlmariLadders {
    pub(crate) full: LadderView,
    pub(crate) judgment: LadderView,
}

impl IlmariLadders {
    fn of(&self, board: LadderBoard) -> Option<&LadderView> {
        match board {
            LadderBoard::Sprint => None,
            LadderBoard::IlmariFull => Some(&self.full),
            LadderBoard::IlmariJudgment => Some(&self.judgment),
        }
    }

    fn of_mut(&mut self, board: LadderBoard) -> Option<&mut LadderView> {
        match board {
            LadderBoard::Sprint => None,
            LadderBoard::IlmariFull => Some(&mut self.full),
            LadderBoard::IlmariJudgment => Some(&mut self.judgment),
        }
    }
}

fn directive_seed(week: u32) -> u64 {
    0x11AA_D1EC_0000 ^ u64::from(week).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// The Directive of ISO week `week`: an index into `directives` (a seeded draw, so the same week is
/// the same policy for every room and every machine); `None` without a table.
pub fn directive_index(week: u32, directives: usize) -> Option<usize> {
    (directives > 0).then(|| Rng::new(directive_seed(week)).below(directives as u32) as usize)
}

/// Ranks that earn a title the first time they are reached (top three of a group board).
pub const RANK_TITLES: usize = 3;

impl Sim {
    /// This week's Directive: (policy aura key, its name), by the sim's clock. A sim that was never
    /// told the date (the headless bots) has no week, so no Directive.
    pub fn ilmari_directive(&self) -> Option<(String, String)> {
        if !self.clock_on() {
            return None;
        }
        let table = &self.content.ilmari.ladder.directives;
        let key = table.get(directive_index(self.legacy_week(), table.len())?)?;
        let name = self
            .content
            .aura(key)
            .map_or(key.clone(), |a| a.name.clone());
        Some((key.clone(), name))
    }

    /// The board `board` for the current week (empty once the week turned), with the Directive's name.
    pub fn ilmari_ladder_view(&self, board: LadderBoard) -> LadderView {
        let week = self.legacy_week();
        let Some(l) = self.ilmari.ladders.of(board) else {
            return self.ladder_view();
        };
        LadderView {
            week,
            entries: if l.week == week {
                l.entries.clone()
            } else {
                Vec::new()
            },
            directive: self.ilmari_directive().map(|d| d.1).unwrap_or_default(),
        }
    }

    /// A joiner sees both Ilmari boards of the room (when they have rows).
    pub(crate) fn ilmari_ladders_on_join(&mut self, player: EntityId) {
        for board in LadderBoard::ILMARI {
            let view = self.ilmari_ladder_view(board);
            if !view.entries.is_empty() {
                self.emit_to(player, Event::Ladder { board, view });
            }
        }
    }

    /// The core fell on `mode` after `ms` of the sim's clock and `deaths` deaths: a Full or Judgment
    /// kill writes a row (Full under fewer than three members is the Async tab's), pays every member
    /// the entry tokens and gives the top three of a group board their rank title.
    pub(crate) fn ladder_core_kill(&mut self, mode: IlmariMode, ms: u32, deaths: u16) {
        let Some(board) = LadderBoard::of_mode(mode) else {
            return;
        };
        let min = self
            .cur_raid()
            .map_or(3, |r| usize::from(r.normal_min_players));
        let is_async = mode == IlmariMode::Full && self.players.len() < min;
        let crew: Vec<String> = self
            .players
            .clone()
            .into_iter()
            .take(crate::sim::MAX_PLAYERS)
            .map(|p| {
                let class = self
                    .index(p)
                    .and_then(|i| self.class_at(i))
                    .map(|c| c.key.clone())
                    .unwrap_or_default();
                self.crew_label(p, &class)
            })
            .map(|c| ladder_label(&c, crate::sprint::LADDER_LABEL_MAX))
            .collect();
        let entry = LadderEntry {
            time_ms: ms,
            deaths,
            crew,
            is_async,
        };
        let week = self.legacy_week();
        let Some(l) = self.ilmari.ladders.of_mut(board) else {
            return;
        };
        // Rank among the rows of its own tab, before the row is in (ties go to the earlier row).
        let rank = if l.week == week {
            l.entries
                .iter()
                .filter(|e| e.is_async == is_async)
                .filter(|e| (e.time_ms, e.deaths) <= (ms, deaths))
                .count()
                + 1
        } else {
            1
        };
        ladder_insert(l, week, entry);
        let view = self.ilmari_ladder_view(board);
        self.events.push(Event::Ladder { board, view });
        let tokens = self.content.ilmari.ladder.entry_tokens;
        for p in self.players.clone() {
            self.grant_ilmari_tokens(p, tokens);
            if !is_async && rank <= RANK_TITLES {
                let key = format!("ladder:rank{rank}");
                self.check_achievements(
                    p,
                    |t| matches!(t, crate::world::Trigger::Event(k) if *k == key),
                );
            }
        }
    }

    /// Every few seconds of a ranked core fight the Directive is put back on every living member
    /// (its aura keeps its own effect; only the duration is the fight's refresh).
    pub(crate) fn core_directive(&mut self, boss: EntityId, force: bool) {
        if self.raid.mode == IlmariMode::Story
            || !(force
                || self
                    .now
                    .is_multiple_of(crate::content::ticks(DIRECTIVE_EVERY_MS)))
        {
            return;
        }
        let Some((aura, _)) = self.ilmari_directive() else {
            return;
        };
        for p in self.players.clone() {
            if let Some(i) = self.index(p).filter(|&i| self.units[i].state.alive) {
                self.apply_aura(i, &aura, DIRECTIVE_MS, None, boss, None);
            }
        }
    }

    /// The saved boards: (board, week, rows) for each board that has rows.
    pub(crate) fn ilmari_ladders_saved(&self) -> Vec<(LadderBoard, LadderView)> {
        LadderBoard::ILMARI
            .into_iter()
            .filter_map(|b| self.ilmari.ladders.of(b).map(|l| (b, l.clone())))
            .filter(|(_, l)| !l.entries.is_empty())
            .collect()
    }

    /// Restores a saved board (bounded, sorted, labels sanitised).
    pub(crate) fn set_ilmari_ladder(
        &mut self,
        board: LadderBoard,
        week: u32,
        entries: Vec<LadderEntry>,
    ) {
        if let Some(l) = self.ilmari.ladders.of_mut(board) {
            *l = ladder_restored(week, entries);
        }
    }

    /// Debug/QA: a row on this week's `board` without a fight (screenshots, tests).
    pub fn debug_ilmari_ladder_add(
        &mut self,
        board: LadderBoard,
        time_ms: u32,
        deaths: u16,
        crew: &[&str],
        is_async: bool,
    ) {
        let week = self.legacy_week();
        let crew = crew
            .iter()
            .map(|c| ladder_label(c, crate::sprint::LADDER_LABEL_MAX))
            .collect();
        if let Some(l) = self.ilmari.ladders.of_mut(board) {
            ladder_insert(
                l,
                week,
                LadderEntry {
                    time_ms,
                    deaths,
                    crew,
                    is_async,
                },
            );
            let view = self.ilmari_ladder_view(board);
            self.events.push(Event::Ladder { board, view });
        }
    }
}

/// The Directive is put back this often and lasts a little longer than that.
const DIRECTIVE_EVERY_MS: u32 = 5_000;
const DIRECTIVE_MS: u32 = 12_000;
