//! Stay Green: a working day at home (`home/1`), squeezed into `goal_ms`. Your Teams status
//! follows an activity meter: at the laptop (`desk` anchor, within `desk_m`) it rises, away it
//! decays, and below `yellow_at` you are Away. Home chores arrive on a fixed schedule, each with
//! a deadline: walk ones (`Go`: stand at the anchor for its dwell, the laundry and the door),
//! the cat on the keyboard (a pick at the laptop), and the mic (mute for the drill, unmute when
//! the call says you are on mute). Going yellow brings a manager's "quick question?" a moment
//! later, answered from the laptop. The mouse jiggler holds the meter while you are away, but
//! IT scans at fixed times (announced on the card shortly before) and a running jiggler is
//! caught: the meter drops to zero. Everything is per player; the stage only holds anchors.

use super::{Fx, line, xz};
use crate::content::ContentError;
use crate::geom::Vec3;
use crate::minigame::{Card, GameRun, Gauge, MinigameView};
use crate::sim::Sim;
use crate::targeting::EntityId;
use serde::Deserialize;

/// Standing this close to a chore's anchor counts as doing it.
const STATION_M: f32 = 1.6;
/// Card picks, in option order.
const JIGGLER: u8 = 0;
const PET: u8 = 1;
const MIC: u8 = 2;
const REPLY: u8 = 3;
/// Layout bits the venue shows: bits 0..8 a walk station is waiting, then these.
pub const LAYOUT_CAT: u32 = 1 << 8;
pub const LAYOUT_JIGGLER: u32 = 1 << 9;
pub const LAYOUT_MUTED: u32 = 1 << 10;

/// What resolves a chore.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Fix {
    /// Stand at this anchor for this many ms.
    Go(String, u32),
    /// Lift the cat off the keyboard (a pick at the laptop).
    Pet,
    /// Be muted.
    Mute,
    /// Be unmuted.
    Unmute,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChoreDef {
    pub key: String,
    /// The card line while it waits ("Courier at the door").
    pub name: String,
    pub fix: Fix,
    pub deadline_ms: u32,
    /// Said when it starts, done, and missed, in turn.
    pub start: Vec<String>,
    pub done: Vec<String>,
    pub missed: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct StayScore {
    pub per_green_s: u32,
    pub per_chore: u32,
    pub per_miss: u32,
    pub per_detect: u32,
    pub per_reply: u32,
    pub per_ignore: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StayGreenDef {
    pub chores: Vec<ChoreDef>,
    /// (second of the day, chore key).
    pub schedule: Vec<(u32, String)>,
    /// Seconds at which IT looks for jigglers.
    pub it_scans_s: Vec<u32>,
    /// The card warns about a scan this long before.
    pub scan_warn_ms: u32,
    pub desk_m: f32,
    pub goal_ms: u32,
    /// Activity below this is Away (yellow).
    pub yellow_at: u8,
    pub rise_per_s: f32,
    pub decay_per_s: f32,
    pub jiggler_ms: u32,
    pub jiggler_rest_ms: u32,
    /// The manager's question comes this long after you go yellow, and waits `reply_ms`.
    pub ping_ms: u32,
    pub reply_ms: u32,
    /// Button labels: jiggler, lift the cat, mute, unmute, reply.
    pub actions: Vec<String>,
    pub manager: String,
    pub question: Vec<String>,
    pub answered: Vec<String>,
    pub ignored: Vec<String>,
    pub went_yellow: Vec<String>,
    pub scan_warn: String,
    pub scan_clear: Vec<String>,
    pub scan_caught: Vec<String>,
    pub away: String,
    pub quiet: String,
    pub done: String,
    pub score: StayScore,
}

impl StayGreenDef {
    pub(crate) fn validate(&self) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("stay green: {m}")));
        if self.actions.len() != 5 || self.chores.is_empty() || self.goal_ms == 0 {
            return bad("chores, a goal and five action labels are required".into());
        }
        for c in &self.chores {
            if c.start.is_empty() || c.done.is_empty() || c.missed.is_empty() || c.deadline_ms == 0
            {
                return bad(format!("{} needs lines and a deadline", c.key));
            }
        }
        if let Some((_, k)) = self.schedule.iter().find(|(_, k)| self.chore(k).is_none()) {
            return bad(format!("unknown chore {k}"));
        }
        if self.stations().len() > 8 || self.question.is_empty() || self.scan_caught.is_empty() {
            return bad("at most eight stations; manager and IT lines are required".into());
        }
        Ok(())
    }

    fn chore(&self, key: &str) -> Option<usize> {
        self.chores.iter().position(|c| c.key == key)
    }

    /// The walk chores' anchors, each once, in chore order (the layout bits' order).
    pub fn stations(&self) -> Vec<&str> {
        let mut v: Vec<&str> = Vec::new();
        for c in &self.chores {
            if let Fix::Go(a, _) = &c.fix
                && !v.contains(&a.as_str())
            {
                v.push(a);
            }
        }
        v
    }
}

pub(crate) struct Stage {
    desk: Vec3,
    stations: Vec<Vec3>,
    /// The cat (unit index), its bed and its favourite spot on the keyboard.
    cat: Option<(usize, Vec3, Vec3)>,
}

impl Stage {
    pub(crate) fn resolve(
        d: &StayGreenDef,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
        cat: Option<usize>,
    ) -> Result<Stage, String> {
        let at = |n: &str| anchors(n).map(|a| a.0).ok_or(format!("no anchor {n}"));
        Ok(Stage {
            desk: at("desk")?,
            stations: d.stations().into_iter().map(at).collect::<Result<_, _>>()?,
            cat: cat
                .zip(anchors("cat_bed"))
                .zip(anchors("cat_desk"))
                .map(|((i, b), k)| (i, b.0, k.0)),
        })
    }
}

struct Pending {
    chore: usize,
    due_ms: u32,
    dwell_ms: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum Question {
    None,
    /// Asked at this ms.
    Coming(u32),
    /// Unanswered until this ms.
    Asking(u32),
}

/// One player's working day.
pub(crate) struct Run {
    activity: f32,
    green_ms: u32,
    yellows: u8,
    done: u8,
    missed: u8,
    detected: u8,
    replied: u8,
    ignored: u8,
    next: usize,
    scans: usize,
    pending: Vec<Pending>,
    question: Question,
    /// Jiggler on until, and resting until (ms of the day).
    jiggler_until: u32,
    jiggler_rest: u32,
    muted: bool,
    at_desk: bool,
    over: bool,
    /// The day's ms at the last step, so time adds up exactly at any tick rate.
    last_ms: u32,
}

impl Default for Run {
    fn default() -> Self {
        Run {
            activity: 100.0,
            green_ms: 0,
            yellows: 0,
            done: 0,
            missed: 0,
            detected: 0,
            replied: 0,
            ignored: 0,
            next: 0,
            scans: 0,
            pending: Vec::new(),
            question: Question::None,
            jiggler_until: 0,
            jiggler_rest: 0,
            muted: false,
            at_desk: true,
            over: false,
            last_ms: 0,
        }
    }
}

impl Run {
    /// (mistakes, chores done).
    pub(crate) fn tallies(&self) -> (u8, u8) {
        (
            self.missed
                .saturating_add(self.ignored)
                .saturating_add(self.detected),
            self.done,
        )
    }

    fn green(&self, d: &StayGreenDef) -> bool {
        self.activity >= f32::from(d.yellow_at)
    }

    fn jiggling(&self, e: u32) -> bool {
        e < self.jiggler_until
    }

    fn score(&self, d: &StayGreenDef) -> u32 {
        let s = &d.score;
        let n = |c: u8| i64::from(c);
        let plus = i64::from(s.per_green_s) * i64::from(self.green_ms / 1000)
            + i64::from(s.per_chore) * n(self.done)
            + i64::from(s.per_reply) * n(self.replied);
        let minus = i64::from(s.per_miss) * n(self.missed)
            + i64::from(s.per_detect) * n(self.detected)
            + i64::from(s.per_ignore) * n(self.ignored);
        (plus - minus).max(0) as u32
    }

    pub(crate) fn summary(&self, d: &StayGreenDef) -> String {
        format!(
            "Green for {} of {} s. {} chores done, {} missed, jiggler caught {} times. Output: one thumbs-up emoji.",
            self.green_ms / 1000,
            d.goal_ms / 1000,
            self.done,
            self.missed,
            self.detected
        )
    }

    pub(crate) fn view(&self, d: &StayGreenDef, mut base: MinigameView) -> MinigameView {
        let e = base.elapsed_ms;
        let green = self.green(d);
        base.lines = vec![
            format!(
                "Status: {}   green {} / {} s",
                if green { "Available" } else { "Away" },
                self.green_ms / 1000,
                d.goal_ms / 1000
            ),
            format!(
                "Chores {} done, {} missed   IT flags {}",
                self.done, self.missed, self.detected
            ),
        ];
        let secs = |until: u32| until.saturating_sub(e).div_ceil(1000);
        base.gauges = vec![Gauge {
            label: if green {
                "Teams: Available".into()
            } else {
                "Teams: Away".into()
            },
            value: self.activity.clamp(0.0, 100.0) as u8,
            mark: Some(d.yellow_at),
        }];
        if let Some(p) = self.pending.iter().find(|p| p.dwell_ms > 0)
            && let Fix::Go(_, need) = &d.chores[p.chore].fix
        {
            base.gauges.push(Gauge {
                label: d.chores[p.chore].name.clone(),
                value: (u64::from(p.dwell_ms) * 100 / u64::from((*need).max(1))).min(100) as u8,
                mark: None,
            });
        }
        let mut text: Vec<String> = Vec::new();
        if !self.at_desk {
            text.push(d.away.clone());
        }
        if let Question::Asking(due) = self.question {
            text.push(format!(
                "{}: \"Quick question?\" {} s",
                d.manager,
                secs(due)
            ));
        }
        if let Some(s) = d.it_scans_s.get(self.scans).map(|s| s * 1000)
            && s.saturating_sub(e) <= d.scan_warn_ms
        {
            text.push(format!("{} in {} s", d.scan_warn, secs(s)));
        }
        for p in &self.pending {
            text.push(format!("{}: {} s", d.chores[p.chore].name, secs(p.due_ms)));
        }
        if text.is_empty() {
            text.push(d.quiet.clone());
        }
        text.truncate(3);
        let jiggler = if self.jiggling(e) {
            format!("{}: ON {} s", d.actions[0], secs(self.jiggler_until))
        } else if e < self.jiggler_rest {
            format!("{} ({} s)", d.actions[0], secs(self.jiggler_rest))
        } else {
            d.actions[0].clone()
        };
        base.card = (!self.over).then(|| Card {
            seq: 1,
            who: "Your laptop".into(),
            text: text.join("\n"),
            options: vec![
                jiggler,
                d.actions[1].clone(),
                d.actions[if self.muted { 3 } else { 2 }].clone(),
                d.actions[4].clone(),
            ],
        });
        let mut layout = 0;
        for p in &self.pending {
            if let Fix::Go(a, _) = &d.chores[p.chore].fix
                && let Some(k) = d.stations().iter().position(|s| s == a)
            {
                layout |= 1 << k;
            }
            if d.chores[p.chore].fix == Fix::Pet {
                layout |= LAYOUT_CAT;
            }
        }
        if self.jiggling(e) {
            layout |= LAYOUT_JIGGLER;
        }
        if self.muted {
            layout |= LAYOUT_MUTED;
        }
        base.layout = layout;
        let (catches, collected) = self.tallies();
        base.catches = catches;
        base.collected = collected;
        base.score = self.score(d);
        base
    }

    /// One tick of the day at `pos`, `e` ms in.
    fn step(&mut self, d: &StayGreenDef, st: &Stage, pos: Vec3, e: u32) -> Vec<Fx> {
        let dt = e.saturating_sub(self.last_ms);
        self.last_ms = e;
        let mut fx = Vec::new();
        let n = |c: u8| usize::from(c);
        // Before the scans: a caught jiggler drops the meter, and that counts as going yellow.
        let was_green = self.green(d);
        self.at_desk = xz(pos, st.desk) <= d.desk_m;
        while let Some((at, key)) = d.schedule.get(self.next)
            && e >= at * 1000
        {
            self.next += 1;
            let Some(k) = d.chore(key) else { continue };
            let c = &d.chores[k];
            // "You're on mute" only happens to people who are on mute.
            if c.fix == Fix::Unmute && !self.muted {
                continue;
            }
            self.pending.push(Pending {
                chore: k,
                due_ms: e + c.deadline_ms,
                dwell_ms: 0,
            });
            let nth = d.schedule[..self.next]
                .iter()
                .filter(|s| s.1 == *key)
                .count();
            fx.push(Fx::Note(line(&c.start, nth - 1), false));
        }
        while let Some(at) = d.it_scans_s.get(self.scans)
            && e >= at * 1000
        {
            self.scans += 1;
            if self.jiggling(e) {
                self.detected = self.detected.saturating_add(1);
                self.jiggler_until = e;
                self.activity = 0.0;
                fx.push(Fx::Note(line(&d.scan_caught, n(self.detected) - 1), false));
            } else {
                fx.push(Fx::Note(line(&d.scan_clear, self.scans - 1), true));
            }
        }
        let mut keep = Vec::new();
        for mut p in std::mem::take(&mut self.pending) {
            let c = &d.chores[p.chore];
            let solved = match &c.fix {
                Fix::Go(a, need) => {
                    let k = d.stations().iter().position(|s| s == a);
                    if k.and_then(|k| st.stations.get(k))
                        .is_some_and(|s| xz(pos, *s) <= STATION_M)
                    {
                        p.dwell_ms += dt;
                    } else {
                        p.dwell_ms = 0;
                    }
                    p.dwell_ms >= *need
                }
                Fix::Pet => false,
                Fix::Mute => self.muted,
                Fix::Unmute => !self.muted,
            };
            if solved {
                self.done = self.done.saturating_add(1);
                fx.push(Fx::Note(line(&c.done, n(self.done) - 1), true));
            } else if e >= p.due_ms {
                self.missed = self.missed.saturating_add(1);
                fx.push(Fx::Note(line(&c.missed, n(self.missed) - 1), false));
            } else {
                keep.push(p);
            }
        }
        self.pending = keep;
        let secs = dt as f32 / 1000.0;
        if self.at_desk {
            self.activity += d.rise_per_s * secs;
        } else if !self.jiggling(e) {
            self.activity -= d.decay_per_s * secs;
        }
        self.activity = self.activity.clamp(0.0, 100.0);
        if self.green(d) {
            self.green_ms += dt;
        } else if was_green {
            self.yellows = self.yellows.saturating_add(1);
            fx.push(Fx::Note(line(&d.went_yellow, n(self.yellows) - 1), false));
            if self.question == Question::None {
                self.question = Question::Coming(e + d.ping_ms);
            }
        }
        match self.question {
            Question::Coming(t) if e >= t => {
                self.question = Question::Asking(e + d.reply_ms);
                let q = line(&d.question, n(self.yellows).saturating_sub(1));
                fx.push(Fx::Note(format!("{}: \"{q}\"", d.manager), false));
            }
            Question::Asking(due) if e >= due => {
                self.question = Question::None;
                self.ignored = self.ignored.saturating_add(1);
                fx.push(Fx::Note(line(&d.ignored, n(self.ignored) - 1), false));
            }
            _ => {}
        }
        if e >= d.goal_ms && !self.over {
            self.over = true;
            fx.push(Fx::Note(d.done.clone(), true));
            fx.push(Fx::Finish(self.score(d), self.yellows == 0));
        }
        fx
    }

    fn pick(&mut self, d: &StayGreenDef, option: u8, e: u32) -> Vec<Fx> {
        if self.over {
            return Vec::new();
        }
        if !self.at_desk {
            return vec![Fx::Note(d.away.clone(), false)];
        }
        match option {
            JIGGLER if e >= self.jiggler_rest => {
                self.jiggler_until = e + d.jiggler_ms;
                self.jiggler_rest = e + d.jiggler_rest_ms;
            }
            PET => {
                if let Some(i) = self
                    .pending
                    .iter()
                    .position(|p| d.chores[p.chore].fix == Fix::Pet)
                {
                    let c = &d.chores[self.pending.remove(i).chore];
                    self.done = self.done.saturating_add(1);
                    return vec![Fx::Note(line(&c.done, usize::from(self.done) - 1), true)];
                }
            }
            MIC => self.muted = !self.muted,
            REPLY => {
                if let Question::Asking(_) = self.question {
                    self.question = Question::None;
                    self.replied = self.replied.saturating_add(1);
                    let l = line(&d.answered, usize::from(self.replied) - 1);
                    return vec![Fx::Note(l, true)];
                }
            }
            _ => {}
        }
        Vec::new()
    }
}

impl Sim {
    pub(crate) fn update_stay_green(&mut self) {
        let e = self.minigame_elapsed_ms();
        let open = self.open_runs();
        let Some(o) = self.minigame.outing.as_mut() else {
            return;
        };
        let Some(d) = self.content.minigames[o.def].stay_green.as_ref() else {
            return;
        };
        let crate::corporate::Stage::StayGreen(st) = &o.stage else {
            return;
        };
        let mut out: Vec<(EntityId, Vec<Fx>)> = Vec::new();
        for (p, pos) in open {
            let Some(GameRun::StayGreen(run)) = o.runs.get_mut(&p).map(|r| &mut r.game) else {
                continue;
            };
            let fx = run.step(d, st, pos, e);
            if !fx.is_empty() {
                out.push((p, fx));
            }
        }
        // One cat for the whole flat: on the keyboard while anyone still has it there.
        let on_keys = o.runs.values().any(|r| match &r.game {
            GameRun::StayGreen(run) => {
                r.over.is_none()
                    && run
                        .pending
                        .iter()
                        .any(|p| d.chores[p.chore].fix == Fix::Pet)
            }
            _ => false,
        });
        if let Some((i, bed, keys)) = st.cat
            && let Some(u) = self.units.get_mut(i)
        {
            u.state.pos = if on_keys { keys } else { bed };
        }
        for (p, fx) in out {
            self.apply_fx(p, fx);
        }
    }

    pub(crate) fn stay_green_pick(&mut self, p: EntityId, seq: u16, option: u8) -> Vec<Fx> {
        let e = self.minigame_elapsed_ms();
        let Some(o) = self.minigame.outing.as_mut() else {
            return Vec::new();
        };
        let Some(d) = self.content.minigames[o.def].stay_green.as_ref() else {
            return Vec::new();
        };
        match o.runs.get_mut(&p).map(|r| &mut r.game) {
            Some(GameRun::StayGreen(run)) if seq == 1 => run.pick(d, option, e),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> StayGreenDef {
        let chore = |key: &str, fix: Fix| ChoreDef {
            key: key.into(),
            name: key.into(),
            fix,
            deadline_ms: 5000,
            start: vec!["s".into()],
            done: vec!["d".into()],
            missed: vec!["m".into()],
        };
        StayGreenDef {
            chores: vec![
                chore("laundry", Fix::Go("laundry".into(), 1000)),
                chore("cat", Fix::Pet),
                chore("door", Fix::Go("door".into(), 500)),
                chore("again", Fix::Go("laundry".into(), 500)),
            ],
            schedule: vec![(1, "cat".into())],
            it_scans_s: vec![],
            scan_warn_ms: 5000,
            desk_m: 1.5,
            goal_ms: 10_000,
            yellow_at: 50,
            rise_per_s: 30.0,
            decay_per_s: 10.0,
            jiggler_ms: 5000,
            jiggler_rest_ms: 8000,
            ping_ms: 1000,
            reply_ms: 3000,
            actions: vec!["j".into(), "c".into(), "m".into(), "u".into(), "r".into()],
            manager: "M".into(),
            question: vec!["q".into()],
            answered: vec!["a".into()],
            ignored: vec!["i".into()],
            went_yellow: vec!["y".into()],
            scan_warn: "w".into(),
            scan_clear: vec!["ok".into()],
            scan_caught: vec!["caught".into()],
            away: "away".into(),
            quiet: "quiet".into(),
            done: "done".into(),
            score: StayScore {
                per_green_s: 6,
                per_chore: 40,
                per_miss: 100,
                per_detect: 150,
                per_reply: 20,
                per_ignore: 150,
            },
        }
    }

    #[test]
    fn stations_are_unique_in_chore_order_and_the_score_floors_at_zero() {
        let d = def();
        assert_eq!(d.stations(), vec!["laundry", "door"]);
        assert!(d.validate().is_ok());
        let r = Run {
            green_ms: 100_500,
            done: 3,
            replied: 1,
            missed: 1,
            ..Run::default()
        };
        assert_eq!(r.score(&d), 600 + 120 + 20 - 100);
        let r = Run {
            detected: 9,
            ..Run::default()
        };
        assert_eq!(r.score(&d), 0);
    }
}
