//! Reorg: a small open-plan office where the org chart changes every round. Each round the
//! colleagues sit at new desks (`desk_<n>` anchors; the partitions between them move too)
//! with new names and roles, and one of them is your manager now. Talking to a colleague
//! (walking up to them) shows their clue, and they are all confused: one knows where your
//! manager sits, one knows their role, one repeats what was true before the reorg, the rest
//! know nothing. "Are you my manager?" settles it; a wrong guess costs points and a moment.
//! The round ends when its clock runs out, or a few seconds after everyone found theirs;
//! then the next reorg reshuffles everything. The org chart is shared by the group (the
//! colleagues are zone units), the search is per player.

use super::{Fx, line, xz};
use crate::content::{Content, ContentError, ticks};
use crate::geom::Vec3;
use crate::minigame::{Card, GameRun, Gauge, MinigameView};
use crate::rng::Rng;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::Event;
use serde::Deserialize;

/// A reply to a guess stays on the card this long.
const REPLY_MS: u32 = 3000;

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ReorgScore {
    pub per_find: u32,
    /// Bonus per second left on the round's clock when you find your manager.
    pub per_second_left: u32,
    pub per_wrong: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReorgDef {
    /// The colleagues: friendly units, one each.
    pub npcs: Vec<String>,
    /// Badges are dealt from these every round.
    pub names: Vec<String>,
    pub roles: Vec<String>,
    /// Desk slot `desk_<n>` as the colleagues describe it ("by the printer").
    pub slots: Vec<String>,
    /// Partition spots `wall_1..=partitions` (the client stands about half each round).
    pub partitions: u8,
    pub rounds: u8,
    pub round_ms: u32,
    /// Once everyone found their manager, the next reorg comes this much later.
    pub settle_ms: u32,
    /// The reorg itself: desks move, nobody answers.
    pub shuffle_ms: u32,
    pub talk_m: f32,
    /// A wrong guess keeps you from asking again this long.
    pub wrong_ms: u32,
    /// Clue templates: `{slot}` is a desk description, `{role}` a role.
    pub place_clues: Vec<String>,
    pub role_clues: Vec<String>,
    /// About the previous org chart (false now).
    pub stale_clues: Vec<String>,
    pub idle_clues: Vec<String>,
    /// What your manager says when you just talk.
    pub manager_lines: Vec<String>,
    pub right: Vec<String>,
    pub wrong: Vec<String>,
    /// To anyone you talk to once you found yours.
    pub settled: String,
    /// One per reorg, in turn.
    pub announcements: Vec<String>,
    pub ask: String,
    pub leave: String,
    pub score: ReorgScore,
}

impl ReorgDef {
    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("reorg: {m}")));
        let n = self.npcs.len();
        if n < 4 || self.names.len() < n || self.roles.len() < n {
            return bad("at least 4 colleagues and a name and role for each".into());
        }
        // The view carries desks and partitions as 16-bit masks.
        if self.slots.len() < n || self.slots.len() > 16 || self.partitions > 16 {
            return bad("a desk slot per colleague, at most 16 slots and partitions".into());
        }
        if let Some(u) = self
            .npcs
            .iter()
            .find(|u| c.unit_def(u).is_none_or(|d| d.hostile))
        {
            return bad(format!("{u} is no friendly unit"));
        }
        let templ = |v: &[String], tag: &str| !v.is_empty() && v.iter().all(|l| l.contains(tag));
        if !templ(&self.place_clues, "{slot}")
            || !templ(&self.stale_clues, "{slot}")
            || !templ(&self.role_clues, "{role}")
        {
            return bad("clue templates need {slot} or {role}".into());
        }
        if self.rounds == 0
            || self.round_ms == 0
            || self.idle_clues.is_empty()
            || self.manager_lines.is_empty()
            || self.right.is_empty()
            || self.wrong.is_empty()
        {
            return bad("rounds, a clock and lines are required".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clue {
    Place,
    Role,
    /// Points at this desk slot, which was true before the reorg.
    Stale(usize),
    Idle,
    Manager,
}

/// The org chart everyone shares.
pub(crate) struct Org {
    pub(crate) round: u8,
    /// The round's full clock ends at `due`; `ends` may come earlier (everyone found theirs).
    due: u64,
    ends: u64,
    /// The desks are moving until here.
    shuffle_until: u64,
    /// Per colleague: desk slot, name, role and clue.
    seat: Vec<usize>,
    name: Vec<usize>,
    role: Vec<usize>,
    clue: Vec<Clue>,
    manager: usize,
    partitions: u32,
    units: Vec<Option<usize>>,
    desks: Vec<(Vec3, f32)>,
    rng: Rng,
    over: bool,
}

fn shuffled(rng: &mut Rng, n: usize) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        v.swap(i, rng.below(i as u32 + 1) as usize);
    }
    v
}

impl Org {
    pub(crate) fn new(
        d: &ReorgDef,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
        seed: u64,
        now: u64,
    ) -> Result<Org, String> {
        let desks = (1..=d.slots.len())
            .map(|k| anchors(&format!("desk_{k}")).ok_or(format!("no anchor desk_{k}")))
            .collect::<Result<Vec<_>, _>>()?;
        let n = d.npcs.len();
        let mut org = Org {
            round: 0,
            due: 0,
            ends: 0,
            shuffle_until: now,
            seat: vec![0; n],
            name: vec![0; n],
            role: vec![0; n],
            clue: vec![Clue::Idle; n],
            manager: 0,
            partitions: 0,
            units: vec![None; n],
            desks,
            rng: Rng::new(seed ^ 0x0E60_C4A7),
            over: false,
        };
        org.deal(d, None);
        org.start_clock(d, now);
        Ok(org)
    }

    fn start_clock(&mut self, d: &ReorgDef, from: u64) {
        self.due = from + ticks(d.round_ms);
        self.ends = self.due;
    }

    /// A new org chart: seats, badges, the manager, and who knows what.
    fn deal(&mut self, d: &ReorgDef, before: Option<usize>) {
        let n = d.npcs.len();
        self.seat = shuffled(&mut self.rng, d.slots.len())[..n].to_vec();
        self.name = shuffled(&mut self.rng, d.names.len())[..n].to_vec();
        self.role = shuffled(&mut self.rng, d.roles.len())[..n].to_vec();
        self.manager = self.rng.below(n as u32) as usize;
        let here = self.seat[self.manager];
        // A stale clue points at the old manager's desk, or any desk that is wrong now.
        let stale = before.filter(|s| *s != here).unwrap_or_else(|| {
            let others: Vec<usize> = self.seat.iter().copied().filter(|s| *s != here).collect();
            others[self.rng.below(others.len() as u32) as usize]
        });
        let order: Vec<usize> = shuffled(&mut self.rng, n)
            .into_iter()
            .filter(|k| *k != self.manager)
            .collect();
        self.clue = vec![Clue::Idle; n];
        self.clue[self.manager] = Clue::Manager;
        for (j, k) in order.into_iter().enumerate() {
            self.clue[k] = match j {
                0 => Clue::Place,
                1 => Clue::Role,
                2 => Clue::Stale(stale),
                _ => Clue::Idle,
            };
        }
        self.partitions = (0..u32::from(d.partitions))
            .filter(|_| self.rng.below(2) == 1)
            .fold(0, |m, i| m | 1 << i);
    }

    fn shuffling(&self, now: u64) -> bool {
        now < self.shuffle_until
    }

    /// Bit `i`: desk slot `i` in use; bit `16 + i`: partition `i` stands.
    pub(crate) fn layout(&self) -> u32 {
        self.seat.iter().fold(0, |m, s| m | 1 << s) | self.partitions << 16
    }

    fn badge(&self, d: &ReorgDef, k: usize) -> String {
        format!("{}, {}", d.names[self.name[k]], d.roles[self.role[k]])
    }

    fn clue_text(&self, d: &ReorgDef, k: usize) -> String {
        let pick = usize::from(self.round) * 7 + k;
        let m = self.manager;
        match self.clue[k] {
            Clue::Place => line(&d.place_clues, pick).replace("{slot}", &d.slots[self.seat[m]]),
            Clue::Role => line(&d.role_clues, pick).replace("{role}", &d.roles[self.role[m]]),
            Clue::Stale(s) => line(&d.stale_clues, pick).replace("{slot}", &d.slots[s]),
            Clue::Idle => line(&d.idle_clues, pick),
            Clue::Manager => line(&d.manager_lines, pick),
        }
    }
}

/// One player's search.
#[derive(Default)]
pub(crate) struct Run {
    rounds: u8,
    found: u8,
    found_now: bool,
    wrong: u8,
    score: u32,
    /// The colleague you are standing at, and one you said "thanks" to.
    near: Option<usize>,
    dismissed: Option<usize>,
    seq: u16,
    lock_until: u64,
    reply: Option<(String, u64)>,
}

impl Run {
    pub(crate) fn new(rounds: u8) -> Run {
        Run {
            rounds,
            seq: 1,
            ..Run::default()
        }
    }

    pub(crate) fn tallies(&self) -> (u8, u8) {
        (self.wrong, self.found)
    }

    pub(crate) fn summary(&self, _d: &ReorgDef) -> String {
        format!(
            "Found your manager after {} of {} reorgs, {} wrong guesses. You now report to a dotted line.",
            self.found, self.rounds, self.wrong
        )
    }

    fn new_round(&mut self) {
        self.found_now = false;
        self.near = None;
        self.dismissed = None;
        self.reply = None;
        self.lock_until = 0;
        self.seq = self.seq.wrapping_add(1);
    }

    pub(crate) fn view(
        &self,
        d: &ReorgDef,
        org: &Org,
        now: u64,
        mut base: MinigameView,
    ) -> MinigameView {
        let secs = |t: u64| crate::time::ticks_to_ms(t.saturating_sub(now)).div_ceil(1000);
        let shuffling = org.shuffling(now);
        base.lines = vec![
            format!(
                "Reorg {}/{}: find who you report to now",
                org.round + 1,
                d.rounds
            ),
            if shuffling {
                "REORG IN PROGRESS. Desks are moving.".into()
            } else if self.found_now {
                format!("Found. Next reorg in {} s", secs(org.ends))
            } else {
                format!("Next reorg in {} s", secs(org.ends))
            },
            format!("Found {}   Wrong guesses {}", self.found, self.wrong),
        ];
        let left = if shuffling {
            0
        } else {
            org.ends.saturating_sub(now) * 100 / ticks(d.round_ms).max(1)
        };
        base.gauges = vec![Gauge {
            label: "Org chart stability".into(),
            value: left.min(100) as u8,
            mark: None,
        }];
        base.badges = org
            .units
            .iter()
            .enumerate()
            .filter_map(|(k, u)| {
                let text = if shuffling {
                    "(being reorganised)".to_string()
                } else {
                    org.badge(d, k)
                };
                Some((Sim::id_of((*u)?), text))
            })
            .collect();
        base.layout = org.layout();
        base.card = self
            .near
            .filter(|k| !shuffling && !org.over && self.dismissed != Some(*k))
            .map(|k| {
                let reply = self.reply.as_ref().filter(|(_, until)| now < *until);
                let text = match (reply, self.found_now) {
                    (Some((r, _)), _) => r.clone(),
                    (None, true) => d.settled.clone(),
                    (None, false) => org.clue_text(d, k),
                };
                let ask = !self.found_now && reply.is_none() && now >= self.lock_until;
                Card {
                    seq: self.seq,
                    who: org.badge(d, k),
                    text,
                    options: if ask {
                        vec![d.ask.clone(), d.leave.clone()]
                    } else {
                        Vec::new()
                    },
                }
            });
        base.catches = self.wrong;
        base.collected = self.found;
        base.score = self.score;
        base
    }
}

impl Sim {
    /// Seats the colleagues for the first round and announces the reorg.
    pub(crate) fn reorg_begin(&mut self) {
        let Some(o) = self.minigame.outing.as_ref() else {
            return;
        };
        let Some(d) = self.content.minigames[o.def].reorg.as_ref() else {
            return;
        };
        let mut taken: Vec<usize> = Vec::new();
        let mut binds = Vec::new();
        for key in &d.npcs {
            let found = (0..self.units.len()).find(|i| {
                let u = &self.units[*i];
                !u.despawned && u.state.key == *key && !taken.contains(i)
            });
            if let Some(i) = found {
                taken.push(i);
            }
            binds.push(found);
        }
        let first = line(&d.announcements, 0);
        if let Some(o) = self.minigame.outing.as_mut()
            && let crate::corporate::Stage::Reorg(org) = &mut o.stage
        {
            org.units = binds;
        }
        self.reorg_seat();
        for (p, _) in self.open_runs() {
            self.apply_fx(p, vec![Fx::Note(first.clone(), true)]);
        }
    }

    /// Moves every colleague to their desk of this round.
    fn reorg_seat(&mut self) {
        let Some(o) = self.minigame.outing.as_ref() else {
            return;
        };
        let crate::corporate::Stage::Reorg(org) = &o.stage else {
            return;
        };
        let moves: Vec<(usize, Vec3, f32)> = org
            .units
            .iter()
            .zip(&org.seat)
            .filter_map(|(u, s)| Some(((*u)?, org.desks[*s].0, org.desks[*s].1)))
            .collect();
        for (i, pos, yaw) in moves {
            let s = &mut self.units[i].state;
            s.pos = pos;
            s.yaw = yaw;
            self.events.push(Event::Teleported {
                unit: Sim::id_of(i),
                pos,
            });
        }
    }

    pub(crate) fn update_reorg(&mut self) {
        let now = self.now;
        let open = self.open_runs();
        let Some(o) = self.minigame.outing.as_mut() else {
            return;
        };
        let Some(d) = self.content.minigames[o.def].reorg.as_ref() else {
            return;
        };
        let crate::corporate::Stage::Reorg(org) = &mut o.stage else {
            return;
        };
        if org.over {
            return;
        }
        let mut finish = false;
        let mut reseat = false;
        let mut announce = None;
        if !org.shuffling(now) {
            let all_found = o
                .runs
                .values()
                .filter(|r| r.over.is_none())
                .all(|r| matches!(&r.game, GameRun::Reorg(x) if x.found_now));
            if all_found {
                org.ends = org.ends.min(now + ticks(d.settle_ms));
            }
            if now >= org.ends {
                if org.round + 1 >= d.rounds {
                    org.over = true;
                    finish = true;
                } else {
                    org.round += 1;
                    let before = org.seat[org.manager];
                    org.deal(d, Some(before));
                    org.shuffle_until = now + ticks(d.shuffle_ms);
                    org.start_clock(d, org.shuffle_until);
                    reseat = true;
                    announce = Some(line(&d.announcements, usize::from(org.round)));
                    for r in o.runs.values_mut() {
                        if let GameRun::Reorg(x) = &mut r.game {
                            x.new_round();
                        }
                    }
                }
            }
        }
        let mut out: Vec<(EntityId, Vec<Fx>)> = Vec::new();
        for (p, pos) in open {
            let Some(GameRun::Reorg(run)) = o.runs.get_mut(&p).map(|r| &mut r.game) else {
                continue;
            };
            let near = (0..org.seat.len())
                .map(|k| (k, xz(org.desks[org.seat[k]].0, pos)))
                .filter(|(_, dist)| *dist <= d.talk_m)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(k, _)| k)
                .filter(|_| !org.shuffling(now));
            if near != run.near {
                run.near = near;
                run.reply = None;
                run.seq = run.seq.wrapping_add(1);
                if near.is_none() {
                    run.dismissed = None;
                }
            }
            let mut fx = Vec::new();
            if let Some(a) = &announce {
                fx.push(Fx::Note(a.clone(), true));
            }
            if finish {
                fx.push(Fx::Finish(run.score, run.wrong == 0));
            }
            if !fx.is_empty() {
                out.push((p, fx));
            }
        }
        if reseat {
            self.reorg_seat();
        }
        for (p, fx) in out {
            self.apply_fx(p, fx);
        }
    }

    pub(crate) fn reorg_pick(&mut self, p: EntityId, seq: u16, option: u8) -> Vec<Fx> {
        let now = self.now;
        let Some(o) = self.minigame.outing.as_mut() else {
            return Vec::new();
        };
        let Some(d) = self.content.minigames[o.def].reorg.as_ref() else {
            return Vec::new();
        };
        let crate::corporate::Stage::Reorg(org) = &mut o.stage else {
            return Vec::new();
        };
        let Some(GameRun::Reorg(run)) = o.runs.get_mut(&p).map(|r| &mut r.game) else {
            return Vec::new();
        };
        let Some(k) = run
            .near
            .filter(|_| seq == run.seq && !org.shuffling(now) && !org.over)
        else {
            return Vec::new();
        };
        let unit = org.units[k];
        let mut fx = Vec::new();
        match option {
            1 => run.dismissed = Some(k),
            0 if !run.found_now && now >= run.lock_until => {
                let pick = usize::from(org.round) + usize::from(run.wrong);
                let said = if k == org.manager {
                    run.found_now = true;
                    run.found += 1;
                    let left = crate::time::ticks_to_ms(org.due.saturating_sub(now)) / 1000;
                    run.score += d.score.per_find + d.score.per_second_left * left as u32;
                    let s = line(&d.right, pick);
                    fx.push(Fx::Note(
                        format!("Found: {} is your manager. For now.", org.badge(d, k)),
                        true,
                    ));
                    s
                } else {
                    run.wrong = run.wrong.saturating_add(1);
                    run.score = run.score.saturating_sub(d.score.per_wrong);
                    run.lock_until = now + ticks(d.wrong_ms);
                    let s = line(&d.wrong, pick);
                    fx.push(Fx::Note(
                        format!("Wrong desk. -{} points.", d.score.per_wrong),
                        false,
                    ));
                    s
                };
                run.reply = Some((said.clone(), now + ticks(REPLY_MS)));
                if let Some(i) = unit {
                    fx.push(Fx::Say(i, said));
                }
            }
            _ => return fx,
        }
        run.seq = run.seq.wrapping_add(1);
        fx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> ReorgDef {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        ReorgDef {
            npcs: s(&["a", "b", "c", "d", "e"]),
            names: s(&["N1", "N2", "N3", "N4", "N5"]),
            roles: s(&["R1", "R2", "R3", "R4", "R5"]),
            slots: s(&["S1", "S2", "S3", "S4", "S5", "S6", "S7"]),
            partitions: 6,
            rounds: 3,
            round_ms: 1000,
            settle_ms: 100,
            shuffle_ms: 100,
            talk_m: 2.0,
            wrong_ms: 100,
            place_clues: s(&["at {slot}"]),
            role_clues: s(&["the {role}"]),
            stale_clues: s(&["was {slot}"]),
            idle_clues: s(&["no idea"]),
            manager_lines: s(&["me?"]),
            right: s(&["yes"]),
            wrong: s(&["no"]),
            settled: "done".into(),
            announcements: s(&["reorg"]),
            ask: "ask".into(),
            leave: "leave".into(),
            score: ReorgScore {
                per_find: 100,
                per_second_left: 1,
                per_wrong: 10,
            },
        }
    }

    fn org(seed: u64) -> Org {
        let d = def();
        let at = |n: &str| {
            let k: f32 = n.strip_prefix("desk_")?.parse().ok()?;
            Some((Vec3::new(k * 5.0, 0.0, 0.0), 0.0))
        };
        Org::new(&d, &at, seed, 0).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn every_org_chart_has_one_true_place_and_role_clue_and_a_false_stale_one() {
        let d = def();
        for seed in 0..50 {
            let mut o = org(seed);
            for _ in 0..4 {
                let before = o.seat[o.manager];
                o.deal(&d, Some(before));
                let m = o.manager;
                let count = |c: Clue| o.clue.iter().filter(|x| **x == c).count();
                assert_eq!((count(Clue::Place), count(Clue::Role)), (1, 1));
                assert_eq!(o.clue[m], Clue::Manager);
                let stale = o.clue.iter().find_map(|c| match c {
                    Clue::Stale(s) => Some(*s),
                    _ => None,
                });
                assert!(stale.is_some_and(|s| s != o.seat[m]));
                // Desks are unique, so the place clue names exactly one colleague.
                let mut seats = o.seat.clone();
                seats.sort_unstable();
                seats.dedup();
                assert_eq!(seats.len(), d.npcs.len());
                assert!(o.clue_text(&d, (m + 1) % 5).len() > 2);
            }
        }
    }

    #[test]
    fn the_layout_marks_the_used_desks_and_the_standing_partitions() {
        let o = org(3);
        let l = o.layout();
        assert_eq!((l & 0xFFFF).count_ones() as usize, def().npcs.len());
        assert_eq!(l >> 16, o.partitions);
    }
}
