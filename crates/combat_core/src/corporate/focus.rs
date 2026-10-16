//! Open-Space Focus: hold `goal_ms` of uninterrupted deep work at your desk in the pod (`pod`
//! anchor, within `desk_m`). Colleagues walk fixed loops from the edges of the open space to
//! the pod (Teambuilding's walkers and cones: waypoint 0 far away, waypoint 1 at the pod), and
//! each arrival breaks your focus unless the right defence is up at that moment: headphones
//! for loud calls and alarms, the "In a meeting" status for "got a minute?", ducking under the
//! monitor for the birthday cake. A defence lasts `action_ms` and rests `cooldown_ms`, so it is
//! about timing. A break resets the streak; away from the desk the streak pauses. The walkers
//! are shared, the defence is per player.

use super::{Fx, line, xz};
use crate::content::{Content, ContentError, ticks};
use crate::geom::Vec3;
use crate::minigame::{Card, GameRun, Gauge, MinigameView};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::teambuilding::WatcherDef;
use serde::Deserialize;

/// Arrivals are looked ahead in steps of this many ms.
const LOOK_STEP_MS: u64 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum FocusAction {
    Headphones,
    Status,
    Duck,
}

impl FocusAction {
    pub const ALL: [FocusAction; 3] = [
        FocusAction::Headphones,
        FocusAction::Status,
        FocusAction::Duck,
    ];

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DisturberDef {
    /// The loop: waypoint 0 far away, waypoint 1 at the pod (cone and colour as Teambuilding).
    pub walk: WatcherDef,
    pub name: String,
    pub counter: FocusAction,
    /// Said when it gets you, in turn (the walker's own `lines` are its approach).
    pub interrupt: Vec<String>,
    pub deflected: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct FocusScore {
    pub base: u32,
    pub per_deflect: u32,
    pub per_interrupt: u32,
    pub per_second: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FocusDef {
    pub disturbers: Vec<DisturberDef>,
    pub desk_m: f32,
    pub goal_ms: u32,
    pub action_ms: u32,
    pub cooldown_ms: u32,
    /// Button labels, in `FocusAction::ALL` order.
    pub actions: Vec<String>,
    /// An arrival shows on the card this long before it happens.
    pub warn_ms: u32,
    pub done: String,
    pub away: String,
    pub quiet: String,
    pub score: FocusScore,
}

impl FocusDef {
    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("focus: {m}")));
        if self.disturbers.is_empty() || self.actions.len() != FocusAction::ALL.len() {
            return bad("disturbers and three action labels are required".into());
        }
        for d in &self.disturbers {
            if c.unit_def(&d.walk.unit).is_none_or(|u| u.hostile) {
                return bad(format!("{} is no friendly unit", d.walk.unit));
            }
            if d.walk.speed_mps <= 0.0 || d.interrupt.is_empty() || d.deflected.is_empty() {
                return bad(format!("{} needs a speed and lines", d.name));
            }
        }
        if self.goal_ms == 0 || self.action_ms == 0 {
            return bad("a goal and an action time are required".into());
        }
        Ok(())
    }
}

pub(crate) struct Stage {
    pod: Vec3,
    /// Per disturber: standing at the pod last tick.
    at_pod: Vec<bool>,
    /// Per disturber: ms until it reaches the pod, when within the warning.
    incoming: Vec<Option<u32>>,
}

impl Stage {
    pub(crate) fn resolve(
        d: &FocusDef,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    ) -> Result<Stage, String> {
        Ok(Stage {
            pod: anchors("pod").ok_or("no anchor pod")?.0,
            at_pod: vec![false; d.disturbers.len()],
            incoming: vec![None; d.disturbers.len()],
        })
    }
}

/// One player's attempt at deep work.
#[derive(Default)]
pub(crate) struct Run {
    streak_ms: u32,
    interrupts: u8,
    deflects: u8,
    /// Per action: up until, and resting until (ticks).
    up: [u64; 3],
    rest: [u64; 3],
    at_desk: bool,
    done: bool,
}

impl Run {
    pub(crate) fn tallies(&self) -> (u8, u8) {
        (self.interrupts, self.deflects)
    }

    fn score(&self, d: &FocusDef, elapsed_ms: u32) -> u32 {
        let s = &d.score;
        let plus = i64::from(s.base) + i64::from(s.per_deflect) * i64::from(self.deflects);
        let minus = i64::from(s.per_interrupt) * i64::from(self.interrupts)
            + i64::from(s.per_second) * i64::from(elapsed_ms / 1000);
        (plus - minus).max(0) as u32
    }

    pub(crate) fn summary(&self, d: &FocusDef) -> String {
        format!(
            "{} s of deep work. Interrupted {} times, deflected {}. You wrote one line of code.",
            d.goal_ms / 1000,
            self.interrupts,
            self.deflects
        )
    }

    fn up(&self, a: FocusAction, now: u64) -> bool {
        now < self.up[a.index()]
    }

    pub(crate) fn view(
        &self,
        d: &FocusDef,
        st: &Stage,
        now: u64,
        mut base: MinigameView,
    ) -> MinigameView {
        let goal = d.goal_ms / 1000;
        base.lines = vec![
            format!(
                "Deep work: {} / {goal} s, uninterrupted",
                self.streak_ms / 1000
            ),
            format!(
                "Interrupted {}   Deflected {}",
                self.interrupts, self.deflects
            ),
        ];
        base.gauges = vec![Gauge {
            label: "Deep work".into(),
            value: (u64::from(self.streak_ms) * 100 / u64::from(d.goal_ms.max(1))).min(100) as u8,
            mark: None,
        }];
        let mut coming: Vec<(u32, &DisturberDef)> = st
            .incoming
            .iter()
            .zip(&d.disturbers)
            .filter_map(|(ms, dd)| Some(((*ms)?, dd)))
            .collect();
        coming.sort_by_key(|c| c.0);
        let text = if !self.at_desk {
            d.away.clone()
        } else if coming.is_empty() {
            d.quiet.clone()
        } else {
            coming
                .iter()
                .take(2)
                .map(|(ms, dd)| format!("Incoming: {} in {} s", dd.name, ms.div_ceil(1000)))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let rest = |a: FocusAction| {
            crate::time::ticks_to_ms(self.rest[a.index()].saturating_sub(now)).div_ceil(1000)
        };
        base.card = (!self.done).then(|| Card {
            seq: 1,
            who: "Your desk".into(),
            text,
            options: FocusAction::ALL
                .iter()
                .map(|a| {
                    let label = &d.actions[a.index()];
                    if self.up(*a, now) {
                        format!("{label}: ON")
                    } else if rest(*a) > 0 {
                        format!("{label} ({} s)", rest(*a))
                    } else {
                        label.clone()
                    }
                })
                .collect(),
        });
        base.catches = self.interrupts;
        base.collected = self.deflects;
        base.score = self.score(d, base.elapsed_ms);
        base
    }
}

impl Sim {
    pub(crate) fn update_focus(&mut self) {
        // The walkers move as Teambuilding's watchers do.
        self.move_watchers();
        let now = self.now;
        let elapsed = self.minigame_elapsed_ms();
        let open = self.open_runs();
        let Some(o) = self.minigame.outing.as_mut() else {
            return;
        };
        let Some(d) = self.content.minigames[o.def].focus.as_ref() else {
            return;
        };
        let crate::corporate::Stage::Focus(st) = &mut o.stage else {
            return;
        };
        let t = u64::from(elapsed);
        let mut arrivals = Vec::new();
        for w in &o.venue.watchers {
            let k = w.def;
            let here = w.patrol.pose(t).at == Some(1);
            if here && !st.at_pod[k] {
                arrivals.push((k, w.unit));
            }
            st.at_pod[k] = here;
            st.incoming[k] = (0..=u64::from(d.warn_ms) / LOOK_STEP_MS)
                .map(|s| s * LOOK_STEP_MS)
                .find(|ms| {
                    w.patrol.pose(t + ms).at == Some(1)
                        && (*ms > 0 || !here)
                        && w.patrol.pose((t + ms).saturating_sub(LOOK_STEP_MS)).at != Some(1)
                })
                .map(|ms| ms as u32)
                .filter(|_| !here);
        }
        let dt = 1000 / crate::time::TICK_HZ;
        let mut out: Vec<(EntityId, Vec<Fx>)> = Vec::new();
        let mut said: Vec<(usize, String)> = Vec::new();
        for (p, pos) in open {
            let Some(GameRun::Focus(run)) = o.runs.get_mut(&p).map(|r| &mut r.game) else {
                continue;
            };
            let mut fx = Vec::new();
            run.at_desk = xz(pos, st.pod) <= d.desk_m;
            for (k, unit) in &arrivals {
                let dd = &d.disturbers[*k];
                if !run.at_desk {
                    continue;
                }
                if run.up(dd.counter, now) {
                    run.deflects = run.deflects.saturating_add(1);
                    let l = line(&dd.deflected, usize::from(run.deflects));
                    fx.push(Fx::Note(format!("Deflected: {}.", dd.name), true));
                    if let Some(u) = unit {
                        said.push((*u, l));
                    }
                } else {
                    run.interrupts = run.interrupts.saturating_add(1);
                    run.streak_ms = 0;
                    let l = line(&dd.interrupt, usize::from(run.interrupts));
                    fx.push(Fx::Note(format!("{}: \"{l}\" Focus lost.", dd.name), false));
                    if let Some(u) = unit {
                        said.push((*u, l));
                    }
                }
            }
            if run.at_desk && !run.done {
                run.streak_ms += dt;
                if run.streak_ms >= d.goal_ms {
                    run.done = true;
                    fx.push(Fx::Note(d.done.clone(), true));
                    fx.push(Fx::Finish(run.score(d, elapsed), run.interrupts == 0));
                }
            }
            if !fx.is_empty() {
                out.push((p, fx));
            }
        }
        // One bubble per arriving colleague is enough, whatever each member did.
        said.sort_by_key(|s| s.0);
        said.dedup_by_key(|s| s.0);
        for (u, l) in said {
            self.request_yell_line(u, crate::content::YellKind::Blast, Some(l));
        }
        for (p, fx) in out {
            self.apply_fx(p, fx);
        }
    }

    pub(crate) fn focus_pick(&mut self, p: EntityId, seq: u16, option: u8) -> Vec<Fx> {
        let now = self.now;
        let Some(o) = self.minigame.outing.as_mut() else {
            return Vec::new();
        };
        let Some(d) = self.content.minigames[o.def].focus.as_ref() else {
            return Vec::new();
        };
        let (Some(GameRun::Focus(run)), Some(a)) = (
            o.runs.get_mut(&p).map(|r| &mut r.game),
            FocusAction::ALL.get(usize::from(option)),
        ) else {
            return Vec::new();
        };
        let i = a.index();
        if seq == 1 && now >= run.rest[i] && !run.done {
            run.up[i] = now + ticks(d.action_ms);
            run.rest[i] = now + ticks(d.cooldown_ms);
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deflections_add_and_interruptions_and_time_take_away() {
        let d = FocusDef {
            disturbers: Vec::new(),
            desk_m: 3.0,
            goal_ms: 60_000,
            action_ms: 2500,
            cooldown_ms: 4000,
            actions: Vec::new(),
            warn_ms: 6000,
            done: String::new(),
            away: String::new(),
            quiet: String::new(),
            score: FocusScore {
                base: 1500,
                per_deflect: 20,
                per_interrupt: 150,
                per_second: 3,
            },
        };
        let r = Run {
            deflects: 10,
            interrupts: 2,
            ..Run::default()
        };
        assert_eq!(r.score(&d, 100_000), 1500 + 200 - 300 - 300);
        assert_eq!(r.score(&d, 1_000_000), 0);
    }
}
