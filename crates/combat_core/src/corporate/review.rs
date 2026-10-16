//! Performance Review: a dialogue "boss fight" with your manager. Each round the manager asks
//! a question and you pick one of up to four answers. Every answer is one kind of thing to
//! say (`Tone`): it burns minutes of the manager's calendar, costs (or restores) their
//! patience and pushes the "Meets Expectations" rating, which drifts back to 3 of 5 whatever
//! you say. Saying the same kind of thing twice in a row costs more patience. Run out of
//! patience and the manager ends the meeting early. The medal measures the calendar you
//! burned (Gold also wants the manager to sit through to the verdict). Every ending is the
//! same verdict: 3 out of 5, no budget for raises.

use super::{Fx, line};
use crate::content::{Content, ContentError, ticks};
use crate::minigame::{Card, GameRun, Gauge, MinigameView};
use crate::sim::Sim;
use crate::targeting::EntityId;
use serde::Deserialize;

/// The rating is kept in thousandths of a star; three stars is the middle it returns to.
const MIDDLE: i32 = 3000;
const STAR: i32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Tone {
    /// What actually happened.
    Honest,
    /// Synergy, alignment, at scale.
    Buzz,
    /// A clarifying question about the question.
    Stall,
    /// Whatever the manager said.
    Agree,
    /// The raise.
    Raise,
    /// No answer in time: the manager fills the silence.
    Silence,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToneDef {
    pub tone: Tone,
    /// Minutes of the manager's calendar it burns.
    pub minutes: u32,
    /// Patience it costs; negative restores.
    pub patience: i32,
    /// Push on the rating, tenths of a star.
    pub gauge: i32,
    /// The manager's replies, in turn.
    pub replies: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AnswerDef {
    pub text: String,
    pub tone: Tone,
}

#[derive(Clone, Debug, Deserialize)]
pub struct QuestionDef {
    pub ask: String,
    pub answers: Vec<AnswerDef>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReviewDef {
    /// The friendly unit across the table (it says the replies).
    pub manager: String,
    /// Its name on the card.
    pub who: String,
    pub opener: String,
    pub questions: Vec<QuestionDef>,
    pub tones: Vec<ToneDef>,
    pub patience: i32,
    /// The same tone twice in a row costs this percent of its patience (a restore halves).
    pub repeat_pct: u32,
    pub repeat_lines: Vec<String>,
    /// Time to answer before the silence answers for you.
    pub answer_ms: u32,
    /// A reply stays this long (or until "Continue").
    pub reply_ms: u32,
    /// The rating drifts back to the middle this fast, tenths of a star per second.
    pub drift: u32,
    /// The meeting's hard stop in minutes, for the header.
    pub hard_stop: u32,
    /// Said when patience runs out.
    pub walkout: String,
    /// The ending, every time; `{minutes}` is the calendar burned.
    pub verdict: String,
    /// The only answer to it.
    pub thanks: String,
    pub verdict_ms: u32,
    pub per_minute: u32,
    /// After a walkout only this percent of the calendar counts ("that was a quick sync").
    pub walkout_pct: u32,
}

impl ReviewDef {
    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("review: {m}")));
        if c.unit_def(&self.manager).is_none_or(|u| u.hostile) {
            return bad(format!("{} is no friendly unit", self.manager));
        }
        if self.questions.is_empty() || self.patience <= 0 || self.answer_ms == 0 {
            return bad("needs questions, patience and an answer time".into());
        }
        for q in &self.questions {
            if q.answers.len() < 2 || q.answers.len() > crate::minigame::CARD_OPTIONS_MAX {
                return bad(format!("2 to 4 answers: {}", q.ask));
            }
            if let Some(a) = q.answers.iter().find(|a| self.tone(a.tone).is_none()) {
                return bad(format!("no tone {:?}", a.tone));
            }
        }
        if self.tone(Tone::Silence).is_none() {
            return bad("no Silence tone".into());
        }
        Ok(())
    }

    pub fn tone(&self, t: Tone) -> Option<&ToneDef> {
        self.tones.iter().find(|d| d.tone == t)
    }

    /// Patience a tone costs after `last`: a repeat costs `repeat_pct` percent, or restores half.
    pub fn cost(&self, t: Tone, last: Option<Tone>) -> i32 {
        let base = self.tone(t).map_or(0, |d| d.patience);
        match (last == Some(t), base > 0) {
            (false, _) => base,
            (true, true) => base * self.repeat_pct as i32 / 100,
            (true, false) => base / 2,
        }
    }

    /// The most minutes any sequence of answers burns with the manager staying to the verdict
    /// (dynamic programming over patience and the last tone); for balance tests and docs.
    pub fn best_minutes(&self) -> u32 {
        use std::collections::BTreeMap;
        // (patience, last tone) -> best minutes so far.
        let mut states: BTreeMap<(i32, Option<u8>), u32> = BTreeMap::new();
        states.insert((self.patience, None), 0);
        for q in &self.questions {
            let mut next: BTreeMap<(i32, Option<u8>), u32> = BTreeMap::new();
            for ((pat, last), mins) in &states {
                let last_tone = last.and_then(|k| self.tones.get(usize::from(k)).map(|d| d.tone));
                for a in &q.answers {
                    let Some(k) = self.tones.iter().position(|d| d.tone == a.tone) else {
                        continue;
                    };
                    let p = (*pat - self.cost(a.tone, last_tone)).min(self.patience);
                    if p <= 0 {
                        continue;
                    }
                    let m = mins + self.tones[k].minutes;
                    let e = next.entry((p, Some(k as u8))).or_insert(0);
                    *e = (*e).max(m);
                }
            }
            states = next;
        }
        states.values().copied().max().unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
enum Phase {
    Opener {
        until: u64,
    },
    Ask {
        q: usize,
        until: u64,
    },
    Reply {
        next: Option<usize>,
        until: u64,
        text: String,
    },
    Verdict {
        until: u64,
    },
}

pub(crate) struct Stage {
    pub(crate) manager: Option<usize>,
}

/// One player's review.
pub(crate) struct Run {
    phase: Phase,
    seq: u16,
    minutes: u32,
    patience: i32,
    /// Thousandths of a star.
    rating: i32,
    last: Option<Tone>,
    /// Answer order per question (shuffled per player so positions mean nothing).
    order: Vec<Vec<u8>>,
    answered: u32,
    silences: u8,
    walked_out: bool,
}

impl Run {
    pub(crate) fn new(
        answers: &[usize],
        patience: i32,
        seed: u64,
        now: u64,
        opener_ms: u32,
    ) -> Run {
        let mut rng = crate::rng::Rng::new(seed ^ 0x5EED_0F10);
        let order = answers
            .iter()
            .map(|n| {
                let mut o: Vec<u8> = (0..*n as u8).collect();
                for i in (1..o.len()).rev() {
                    o.swap(i, rng.below(i as u32 + 1) as usize);
                }
                o
            })
            .collect();
        Run {
            phase: Phase::Opener {
                until: now + ticks(opener_ms),
            },
            seq: 1,
            minutes: 0,
            patience,
            rating: MIDDLE,
            last: None,
            order,
            answered: 0,
            silences: 0,
            walked_out: false,
        }
    }

    pub(crate) fn tallies(&self) -> (u8, u8) {
        (self.silences, self.minutes.min(255) as u8)
    }

    fn score(&self, d: &ReviewDef) -> u32 {
        let full = self.minutes.saturating_mul(d.per_minute);
        if self.walked_out {
            full * d.walkout_pct / 100
        } else {
            full
        }
    }

    pub(crate) fn summary(&self, d: &ReviewDef) -> String {
        let walked = if self.walked_out {
            " They took the rest offline, so it was logged as a quick sync."
        } else {
            ""
        };
        format!(
            "Burned {} min of your manager's calendar (hard stop: {}).{walked} Rating: 3 out of 5.",
            self.minutes, d.hard_stop
        )
    }

    fn ask(&mut self, d: &ReviewDef, q: usize, now: u64) {
        self.seq = self.seq.wrapping_add(1);
        self.phase = Phase::Ask {
            q,
            until: now + ticks(d.answer_ms),
        };
    }

    fn verdict(&mut self, d: &ReviewDef, now: u64) {
        self.seq = self.seq.wrapping_add(1);
        self.phase = Phase::Verdict {
            until: now + ticks(d.verdict_ms),
        };
    }

    /// Answers question `q` with `tone`; the manager's reply comes back to be said aloud.
    fn answer(&mut self, d: &ReviewDef, q: usize, tone: Tone, now: u64) -> String {
        let cost = d.cost(tone, self.last);
        let repeat = self.last == Some(tone);
        let td = d.tone(tone);
        self.patience = (self.patience - cost).min(d.patience);
        self.minutes += td.map_or(0, |t| t.minutes);
        self.rating = (self.rating + td.map_or(0, |t| t.gauge) * 100).clamp(STAR, 5 * STAR);
        if tone == Tone::Silence {
            self.silences = self.silences.saturating_add(1);
        }
        let k = self.answered as usize;
        self.answered += 1;
        self.last = Some(tone);
        let mut text = if repeat {
            line(&d.repeat_lines, k)
        } else {
            td.map_or(String::new(), |t| line(&t.replies, k))
        };
        let next = if self.patience <= 0 {
            self.walked_out = true;
            text = format!("{text} {}", d.walkout);
            None
        } else {
            (q + 1 < d.questions.len()).then_some(q + 1)
        };
        self.seq = self.seq.wrapping_add(1);
        self.phase = Phase::Reply {
            next,
            until: now + ticks(d.reply_ms),
            text: text.clone(),
        };
        text
    }

    fn advance(&mut self, d: &ReviewDef, next: Option<usize>, now: u64) {
        match next {
            Some(q) => self.ask(d, q, now),
            None => self.verdict(d, now),
        }
    }

    /// One tick: the rating drifts home and timers run out. Returns the manager's line to say.
    fn step(&mut self, d: &ReviewDef, now: u64) -> (Option<String>, bool) {
        let drift = (d.drift as i32 * 100 / crate::time::TICK_HZ as i32).max(1);
        self.rating = if self.rating > MIDDLE {
            (self.rating - drift).max(MIDDLE)
        } else {
            (self.rating + drift).min(MIDDLE)
        };
        match self.phase.clone() {
            Phase::Opener { until } if now >= until => self.ask(d, 0, now),
            Phase::Ask { q, until } if now >= until => {
                return (Some(self.answer(d, q, Tone::Silence, now)), false);
            }
            Phase::Reply { next, until, .. } if now >= until => self.advance(d, next, now),
            Phase::Verdict { until } if now >= until => return (None, true),
            _ => {}
        }
        (None, false)
    }

    /// Returns the line to say and whether the run is over.
    fn pick(&mut self, d: &ReviewDef, seq: u16, option: u8, now: u64) -> (Option<String>, bool) {
        if seq != self.seq {
            return (None, false);
        }
        match self.phase.clone() {
            Phase::Opener { .. } => self.ask(d, 0, now),
            Phase::Ask { q, .. } => {
                let Some(&k) = self.order.get(q).and_then(|o| o.get(usize::from(option))) else {
                    return (None, false);
                };
                let Some(a) = d
                    .questions
                    .get(q)
                    .and_then(|q| q.answers.get(usize::from(k)))
                else {
                    return (None, false);
                };
                return (Some(self.answer(d, q, a.tone, now)), false);
            }
            Phase::Reply { next, .. } => self.advance(d, next, now),
            Phase::Verdict { .. } => return (None, true),
        }
        (None, false)
    }

    fn finish(&self, d: &ReviewDef) -> Fx {
        Fx::Finish(self.score(d), !self.walked_out)
    }

    pub(crate) fn view(&self, d: &ReviewDef, now: u64, mut base: MinigameView) -> MinigameView {
        let n = d.questions.len();
        let secs = |until: u64| crate::time::ticks_to_ms(until.saturating_sub(now)).div_ceil(1000);
        let (q_line, card) = match &self.phase {
            Phase::Opener { .. } => (
                "The meeting starts".to_string(),
                Card {
                    seq: self.seq,
                    who: d.who.clone(),
                    text: d.opener.clone(),
                    options: vec!["Sounds good".into()],
                },
            ),
            Phase::Ask { q, until } => (
                format!(
                    "Question {}/{n}: answer in {} s, or the silence answers",
                    q + 1,
                    secs(*until)
                ),
                Card {
                    seq: self.seq,
                    who: d.who.clone(),
                    text: d.questions[*q].ask.clone(),
                    options: self.order[*q]
                        .iter()
                        .filter_map(|k| d.questions[*q].answers.get(usize::from(*k)))
                        .map(|a| a.text.clone())
                        .collect(),
                },
            ),
            Phase::Reply { text, next, .. } => (
                match next {
                    Some(q) => format!("Question {}/{n} next", q + 1),
                    None => "The verdict is coming".to_string(),
                },
                Card {
                    seq: self.seq,
                    who: d.who.clone(),
                    text: text.clone(),
                    options: vec!["Continue".into()],
                },
            ),
            Phase::Verdict { .. } => (
                "The verdict".to_string(),
                Card {
                    seq: self.seq,
                    who: d.who.clone(),
                    text: d.verdict.replace("{minutes}", &self.minutes.to_string()),
                    options: vec![d.thanks.clone()],
                },
            ),
        };
        base.lines = vec![
            q_line,
            format!(
                "Calendar burned: {} min (hard stop at {})",
                self.minutes, d.hard_stop
            ),
        ];
        let stars = self.rating as f32 / STAR as f32;
        base.gauges = vec![
            Gauge {
                label: format!("Meets Expectations: {stars:.1} of 5"),
                value: ((self.rating - STAR) * 100 / (4 * STAR)).clamp(0, 100) as u8,
                mark: Some(50),
            },
            Gauge {
                label: "Manager's patience".into(),
                value: (self.patience * 100 / d.patience.max(1)).clamp(0, 100) as u8,
                mark: None,
            },
        ];
        base.card = Some(card);
        base.catches = self.silences;
        base.collected = self.minutes.min(255) as u8;
        base.score = self.score(d);
        base
    }
}

impl Sim {
    fn review_parts(&self) -> Option<(usize, Option<usize>)> {
        let o = self.minigame.outing.as_ref()?;
        let crate::corporate::Stage::Review(s) = &o.stage else {
            return None;
        };
        Some((o.def, s.manager))
    }

    pub(crate) fn update_review(&mut self) {
        let Some((def, manager)) = self.review_parts() else {
            return;
        };
        let now = self.now;
        let Some(d) = self.content.minigames[def].review.as_ref() else {
            return;
        };
        let mut out = Vec::new();
        if let Some(o) = self.minigame.outing.as_mut() {
            for (p, r) in o.runs.iter_mut() {
                if r.over.is_some() {
                    continue;
                }
                let GameRun::Review(run) = &mut r.game else {
                    continue;
                };
                let (say, done) = run.step(d, now);
                let mut fx = Vec::new();
                if let (Some(line), Some(m)) = (say, manager) {
                    fx.push(Fx::Say(m, line));
                }
                if done {
                    fx.push(run.finish(d));
                }
                if !fx.is_empty() {
                    out.push((*p, fx));
                }
            }
        }
        for (p, fx) in out {
            self.apply_fx(p, fx);
        }
    }

    pub(crate) fn review_pick(&mut self, p: EntityId, seq: u16, option: u8) -> Vec<Fx> {
        let Some((def, manager)) = self.review_parts() else {
            return Vec::new();
        };
        let now = self.now;
        let Some(d) = self.content.minigames[def].review.as_ref() else {
            return Vec::new();
        };
        let Some(GameRun::Review(run)) = self
            .minigame
            .outing
            .as_mut()
            .and_then(|o| o.runs.get_mut(&p))
            .map(|r| &mut r.game)
        else {
            return Vec::new();
        };
        let (say, done) = run.pick(d, seq, option, now);
        let mut fx = Vec::new();
        if let (Some(line), Some(m)) = (say, manager) {
            fx.push(Fx::Say(m, line));
        }
        if done {
            fx.push(run.finish(d));
        }
        fx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> ReviewDef {
        let tone = |tone, minutes, patience, gauge| ToneDef {
            tone,
            minutes,
            patience,
            gauge,
            replies: vec![format!("{tone:?}")],
        };
        let a = |text: &str, tone| AnswerDef {
            text: text.into(),
            tone,
        };
        ReviewDef {
            manager: "m".into(),
            who: "Manager".into(),
            opener: "Hi".into(),
            questions: vec![
                QuestionDef {
                    ask: "Q1".into(),
                    answers: vec![a("buzz", Tone::Buzz), a("agree", Tone::Agree)],
                };
                3
            ],
            tones: vec![
                tone(Tone::Buzz, 6, 40, 6),
                tone(Tone::Agree, 1, -20, 0),
                tone(Tone::Silence, 4, 15, 0),
            ],
            patience: 100,
            repeat_pct: 150,
            repeat_lines: vec!["Again?".into()],
            answer_ms: 1000,
            reply_ms: 1000,
            drift: 5,
            hard_stop: 30,
            walkout: "Bye.".into(),
            verdict: "3 of 5 after {minutes} min".into(),
            thanks: "Thanks".into(),
            verdict_ms: 1000,
            per_minute: 10,
            walkout_pct: 50,
        }
    }

    #[test]
    fn a_repeat_costs_more_and_a_restore_less() {
        let d = def();
        assert_eq!(d.cost(Tone::Buzz, None), 40);
        assert_eq!(d.cost(Tone::Buzz, Some(Tone::Buzz)), 60);
        assert_eq!(d.cost(Tone::Agree, Some(Tone::Agree)), -10);
    }

    #[test]
    fn the_best_line_alternates_instead_of_repeating() {
        // Buzz, Buzz walks out on the second (40 + 60); Buzz, Agree, Buzz stays: 6 + 1 + 6.
        assert_eq!(def().best_minutes(), 13);
    }

    #[test]
    fn the_rating_always_drifts_back_to_three_stars() {
        let d = def();
        let mut r = Run::new(&[2, 2, 2], 100, 1, 0, 0);
        r.step(&d, 1);
        r.answer(&d, 0, Tone::Buzz, 1);
        assert!(r.rating > MIDDLE);
        for t in 2..200 {
            r.step(&d, t);
        }
        assert_eq!(r.rating, MIDDLE);
    }
}
