//! The Company Calendar's minigames (E5 task 5.1), on the `minigame.rs` frame: Performance
//! Review (`review`), Reorg (`reorg`), Tool Migration (`migration`), Hackathon (`hackathon`)
//! and Open-Space Focus (`focus`). Each member plays their own run and the group score adds
//! up, as in Teambuilding. Decisions go through one choice card (`Card`, answered with
//! `Command::MinigamePick`), progress through the header's lines and gauges, and moments
//! through routed notes; the venues are small zones with friendly units.

pub mod focus;
pub mod hackathon;
pub mod migration;
pub mod reorg;
pub mod review;

use crate::geom::Vec3;
use crate::minigame::{GameRun, MinigameView, Rules};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::teambuilding::Venue;
use crate::types::Event;

/// What every run of an outing shares.
#[derive(Default)]
pub(crate) enum Stage {
    #[default]
    None,
    Review(review::Stage),
    Reorg(Box<reorg::Org>),
    Migration(migration::Stage),
    Hackathon(hackathon::Stage),
    Focus(focus::Stage),
}

/// What a game asks the frame to do for one run.
pub(crate) enum Fx {
    /// A routed note on the player's screen.
    Note(String, bool),
    /// A speech bubble over unit `index`.
    Say(usize, String),
    /// The run is over: (score, flawless).
    Finish(u32, bool),
    /// Move the player (the hackathon's walk onto the stage).
    Teleport(Vec3, f32),
}

/// The outing being set up: its shared state and how a member's run starts.
pub(crate) struct Setup {
    pub(crate) venue: Venue,
    pub(crate) stage: Stage,
    make: Box<dyn Fn(EntityId) -> GameRun>,
}

impl Setup {
    pub(crate) fn new_run(&self, p: EntityId) -> GameRun {
        (self.make)(p)
    }
}

/// Line `k` of a list, cycling; "" for an empty list.
pub(crate) fn line(lines: &[String], k: usize) -> String {
    if lines.is_empty() {
        return String::new();
    }
    lines[k % lines.len()].clone()
}

/// Flat distance.
pub(crate) fn xz(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// A run's HUD view for the corporate games.
pub(crate) fn view(
    game: &GameRun,
    rules: Rules,
    stage: &Stage,
    now: u64,
    base: MinigameView,
) -> MinigameView {
    match (game, rules, stage) {
        (GameRun::Review(r), Rules::Review(d), _) => r.view(d, now, base),
        (GameRun::Reorg(r), Rules::Reorg(d), Stage::Reorg(o)) => r.view(d, o, now, base),
        (GameRun::Migration(r), Rules::Migration(d), _) => r.view(d, now, base),
        (GameRun::Hackathon(r), Rules::Hackathon(d), Stage::Hackathon(s)) => {
            r.view(d, s, now, base)
        }
        (GameRun::Focus(r), Rules::Focus(d), Stage::Focus(s)) => r.view(d, s, now, base),
        _ => base,
    }
}

impl Sim {
    /// Shared state and run factory for minigame `def` in the venue described by `anchors`.
    pub(crate) fn corporate_setup(
        &self,
        def: usize,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    ) -> Result<Setup, String> {
        let d = &self.content.minigames[def];
        let seed = self.seed ^ self.now.rotate_left(17);
        let none = |make: Box<dyn Fn(EntityId) -> GameRun>, stage| Setup {
            venue: Venue::default(),
            stage,
            make,
        };
        Ok(match d.rules().ok_or("no rules")? {
            Rules::Teambuilding(t) => {
                let venue = Venue::resolve(t, anchors)?;
                let objectives = venue.objectives.len();
                let snacks = venue.snacks.len();
                Setup {
                    venue,
                    stage: Stage::None,
                    make: Box::new(move |_| {
                        GameRun::Teambuilding(crate::teambuilding::TbRun::sized(objectives, snacks))
                    }),
                }
            }
            Rules::Review(r) => {
                let n = r
                    .questions
                    .iter()
                    .map(|q| q.answers.len())
                    .collect::<Vec<_>>();
                let now = self.now;
                let stage = review::Stage {
                    manager: self.first_unit(&r.manager),
                };
                let (patience, opener_ms) = (r.patience, r.reply_ms.saturating_mul(2));
                none(
                    Box::new(move |p| {
                        GameRun::Review(review::Run::new(&n, patience, seed ^ p, now, opener_ms))
                    }),
                    Stage::Review(stage),
                )
            }
            Rules::Reorg(r) => {
                let org = reorg::Org::new(r, anchors, seed, self.now)?;
                let rounds = r.rounds;
                none(
                    Box::new(move |_| GameRun::Reorg(reorg::Run::new(rounds))),
                    Stage::Reorg(Box::new(org)),
                )
            }
            Rules::Migration(r) => {
                let stage = migration::Stage::resolve(r, anchors)?;
                none(
                    Box::new(|_| GameRun::Migration(migration::Run::default())),
                    Stage::Migration(stage),
                )
            }
            Rules::Hackathon(r) => {
                let stage = hackathon::Stage::resolve(r, anchors)?;
                let (spots, focus) = (stage.spots.len(), r.focus_start);
                none(
                    Box::new(move |_| GameRun::Hackathon(hackathon::Run::new(spots, focus))),
                    Stage::Hackathon(stage),
                )
            }
            Rules::Focus(r) => {
                let walk: Vec<_> = r.disturbers.iter().map(|d| d.walk.clone()).collect();
                let stage = focus::Stage::resolve(r, anchors)?;
                Setup {
                    venue: Venue {
                        watchers: Venue::walkers(&walk, anchors)?,
                        ..Venue::default()
                    },
                    stage: Stage::Focus(stage),
                    make: Box::new(|_| GameRun::Focus(focus::Run::default())),
                }
            }
        })
    }

    /// Index of the first live unit with `key` in the zone.
    pub(crate) fn first_unit(&self, key: &str) -> Option<usize> {
        (0..self.units.len()).find(|i| {
            let u = &self.units[*i];
            !u.despawned && u.state.key == key
        })
    }

    /// Once the runs exist: the reorg seats everyone for the first round.
    pub(crate) fn corporate_after_start(&mut self) {
        let Some(o) = self.minigame.outing.as_ref() else {
            return;
        };
        if let Some(Rules::Reorg(_)) = self.content.minigames[o.def].rules() {
            self.reorg_begin();
        }
    }

    pub(crate) fn update_corporate(&mut self) {
        let Some(o) = self.minigame.outing.as_ref() else {
            return;
        };
        match self.content.minigames[o.def].rules() {
            Some(Rules::Review(_)) => self.update_review(),
            Some(Rules::Reorg(_)) => self.update_reorg(),
            Some(Rules::Migration(_)) => self.update_migration(),
            Some(Rules::Hackathon(_)) => self.update_hackathon(),
            Some(Rules::Focus(_)) => self.update_focus(),
            Some(Rules::Teambuilding(_)) | None => {}
        }
    }

    /// `Command::MinigamePick`: the asker answers their run's card.
    pub(crate) fn pick_minigame(&mut self, actor: EntityId, seq: u16, option: u8) {
        let Some(o) = self.minigame.outing.as_ref() else {
            return;
        };
        if self.zone.as_deref() != Some(o.zone.as_str())
            || o.runs.get(&actor).is_none_or(|r| r.over.is_some())
        {
            return;
        }
        let fx = match self.content.minigames[o.def].rules() {
            Some(Rules::Review(_)) => self.review_pick(actor, seq, option),
            Some(Rules::Reorg(_)) => self.reorg_pick(actor, seq, option),
            Some(Rules::Migration(_)) => self.migration_pick(actor, seq, option),
            Some(Rules::Hackathon(_)) => self.hackathon_pick(actor, seq, option),
            Some(Rules::Focus(_)) => self.focus_pick(actor, seq, option),
            Some(Rules::Teambuilding(_)) | None => Vec::new(),
        };
        self.apply_fx(actor, fx);
    }

    /// The finished run's line for the medal screen.
    pub(crate) fn corporate_summary(&self, player: EntityId) -> String {
        let Some(o) = self.minigame.outing.as_ref() else {
            return String::new();
        };
        let Some(r) = o.runs.get(&player) else {
            return String::new();
        };
        let def = &self.content.minigames[o.def];
        match (&r.game, def.rules()) {
            (GameRun::Review(r), Some(Rules::Review(d))) => r.summary(d),
            (GameRun::Reorg(r), Some(Rules::Reorg(d))) => r.summary(d),
            (GameRun::Migration(r), Some(Rules::Migration(d))) => r.summary(d),
            (GameRun::Hackathon(r), Some(Rules::Hackathon(d))) => r.summary(d),
            (GameRun::Focus(r), Some(Rules::Focus(d))) => r.summary(d),
            _ => String::new(),
        }
    }

    /// Positions of the members whose run is still going (alive ones only).
    pub(crate) fn open_runs(&self) -> Vec<(EntityId, Vec3)> {
        let Some(o) = self.minigame.outing.as_ref() else {
            return Vec::new();
        };
        o.runs
            .iter()
            .filter(|(_, r)| r.over.is_none())
            .filter_map(|(p, _)| {
                let u = &self.units[self.index(*p)?].state;
                u.alive.then_some((*p, u.pos))
            })
            .collect()
    }

    pub(crate) fn apply_fx(&mut self, p: EntityId, fx: Vec<Fx>) {
        for f in fx {
            match f {
                Fx::Note(text, good) => self.emit_to(
                    p,
                    Event::MinigameNote {
                        unit: p,
                        text,
                        good,
                    },
                ),
                Fx::Say(i, line) => {
                    if i < self.units.len() {
                        self.request_yell_line(i, crate::content::YellKind::Blast, Some(line));
                    }
                }
                Fx::Finish(score, flawless) => self.end_run(p, Some((score, flawless))),
                Fx::Teleport(pos, yaw) => {
                    if let Some(i) = self.index(p) {
                        let s = &mut self.units[i].state;
                        s.pos = pos;
                        s.yaw = yaw;
                        self.events.push(Event::Teleported { unit: p, pos });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_cycle_and_an_empty_list_is_silent() {
        let l = vec!["a".to_string(), "b".to_string()];
        assert_eq!(line(&l, 3), "b");
        assert_eq!(line(&[], 3), "");
    }
}
