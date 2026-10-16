//! Hackathon: a 24-hour countdown in fast time (one hour every `hour_ms`). The demo builds
//! itself, as fast as your focus allows; focus drains all the time and faster in the small
//! hours. Pizza boxes (`pizza_<n>`) and energy drinks (`drink_<n>`) around the room refill
//! it when you walk over them (not when you are already full) and come back after a while;
//! pizza goes cold late in the night. At hour 24 you walk onto the stage (`stage_<n>`) and
//! the demo crashes, whatever the bar said. The medal measures how far the demo got, plus a
//! little for what you ate.

use super::{Fx, xz};
use crate::content::{ContentError, ticks};
use crate::geom::Vec3;
use crate::minigame::{Card, GameRun, Gauge, MinigameView};
use crate::sim::Sim;
use crate::targeting::EntityId;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct HackathonScore {
    pub per_percent: u32,
    pub per_snack: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HackathonDef {
    pub hours: u32,
    /// Real milliseconds per hackathon hour.
    pub hour_ms: u32,
    /// The clock at the start (Saturday, 0..24).
    pub start_hour: u32,
    pub focus_start: u32,
    /// Focus lost per second, tenths.
    pub drain: u32,
    /// Hours since the start in which the drain doubles: [from, to).
    pub night: (u32, u32),
    /// Demo percent per second at full focus, hundredths.
    pub progress: u32,
    pub pizzas: u8,
    pub drinks: u8,
    pub pizza_focus: u32,
    /// Pizza from this hour on is cold and gives `cold_focus`.
    pub cold_after: u32,
    pub cold_focus: u32,
    pub drink_focus: u32,
    /// At this focus or above you walk past food.
    pub full: u32,
    pub respawn_ms: u32,
    pub reach_m: f32,
    /// (hour since the start, line), shown once each.
    pub notes: Vec<(u32, String)>,
    /// Who hosts the stage (a friendly unit), and its line when the demo dies.
    pub mc: String,
    pub mc_line: String,
    /// The demo card; `{progress}` is the bar.
    pub demo: String,
    pub bow: String,
    pub demo_ms: u32,
    pub score: HackathonScore,
}

impl HackathonDef {
    pub(crate) fn validate(&self) -> Result<(), ContentError> {
        let bad = |m: &str| Err(ContentError::Invalid(format!("hackathon: {m}")));
        if self.hours == 0 || self.hour_ms == 0 || self.demo_ms == 0 {
            return bad("hours, an hour length and a demo time are required");
        }
        // The view carries the snack spots as a 32-bit mask.
        if self.pizzas == 0 || self.drinks == 0 || usize::from(self.pizzas + self.drinks) > 32 {
            return bad("1 to 32 snack spots");
        }
        if self.full > 100 || self.focus_start > 100 {
            return bad("focus is 0..=100");
        }
        Ok(())
    }

    /// "Sat 14:30" for `ms` into the run.
    pub fn clock(&self, ms: u32) -> String {
        let minutes = u64::from(ms) * 60 / u64::from(self.hour_ms.max(1));
        let total = u64::from(self.start_hour) * 60 + minutes;
        let day = if total < 24 * 60 { "Sat" } else { "Sun" };
        format!("{day} {:02}:{:02}", total / 60 % 24, total % 60)
    }

    pub fn length_ms(&self) -> u32 {
        self.hours.saturating_mul(self.hour_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Snack {
    Pizza,
    Drink,
}

pub(crate) struct Stage {
    pub(crate) spots: Vec<(Vec3, Snack)>,
    stage: Vec<(Vec3, f32)>,
    mc: Option<usize>,
}

impl Stage {
    pub(crate) fn resolve(
        d: &HackathonDef,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    ) -> Result<Stage, String> {
        let need = |n: String| anchors(&n).ok_or(format!("no anchor {n}"));
        let mut spots = Vec::new();
        for k in 1..=d.pizzas {
            spots.push((need(format!("pizza_{k}"))?.0, Snack::Pizza));
        }
        for k in 1..=d.drinks {
            spots.push((need(format!("drink_{k}"))?.0, Snack::Drink));
        }
        let mut stage = Vec::new();
        while let Some(a) = anchors(&format!("stage_{}", stage.len() + 1)) {
            stage.push(a);
        }
        if stage.is_empty() {
            return Err("no anchor stage_1".into());
        }
        Ok(Stage {
            spots,
            stage,
            mc: None,
        })
    }
}

/// One player's 24 hours.
pub(crate) struct Run {
    focus: f32,
    /// Percent.
    progress: f32,
    pizzas: u8,
    drinks: u8,
    /// Per snack spot: free again from this tick.
    ready: Vec<u64>,
    notes: usize,
    /// On stage: the card's deadline.
    demo: Option<u64>,
    seq: u16,
    /// Ran out of focus at some point.
    crashed: bool,
}

impl Run {
    pub(crate) fn new(spots: usize, focus: u32) -> Run {
        Run {
            focus: focus as f32,
            progress: 0.0,
            pizzas: 0,
            drinks: 0,
            ready: vec![0; spots],
            notes: 0,
            demo: None,
            seq: 1,
            crashed: false,
        }
    }

    pub(crate) fn tallies(&self) -> (u8, u8) {
        (
            u8::from(self.crashed),
            self.pizzas.saturating_add(self.drinks),
        )
    }

    fn percent(&self) -> u32 {
        self.progress.clamp(0.0, 100.0) as u32
    }

    fn score(&self, d: &HackathonDef) -> u32 {
        self.percent() * d.score.per_percent
            + u32::from(self.pizzas.saturating_add(self.drinks)) * d.score.per_snack
    }

    pub(crate) fn summary(&self, _d: &HackathonDef) -> String {
        format!(
            "Demo {}% built, {} pizza, {} energy drinks. It crashed on stage, as is tradition.",
            self.percent(),
            self.pizzas,
            self.drinks
        )
    }

    pub(crate) fn view(
        &self,
        d: &HackathonDef,
        st: &Stage,
        now: u64,
        mut base: MinigameView,
    ) -> MinigameView {
        let left_ms = d.length_ms().saturating_sub(base.elapsed_ms);
        base.lines = vec![
            format!(
                "{}   {} h left",
                d.clock(base.elapsed_ms.min(d.length_ms())),
                left_ms.div_ceil(d.hour_ms.max(1))
            ),
            format!(
                "Pizza {}   Energy drinks {}   (walk over them)",
                self.pizzas, self.drinks
            ),
        ];
        base.gauges = vec![
            Gauge {
                label: format!("Focus {}", self.focus.round() as u32),
                value: self.focus.clamp(0.0, 100.0) as u8,
                mark: None,
            },
            Gauge {
                label: format!("Demo progress {}%", self.percent()),
                value: self.percent() as u8,
                mark: None,
            },
        ];
        base.collectibles = st.spots.len() as u8;
        base.taken = self
            .ready
            .iter()
            .enumerate()
            .take(32)
            .filter(|(_, r)| now < **r)
            .fold(0, |m, (i, _)| m | 1 << i);
        base.collected = self.pizzas.saturating_add(self.drinks);
        base.catches = u8::from(self.crashed);
        base.card = self.demo.map(|_| Card {
            seq: self.seq,
            who: "Demo Day".into(),
            text: d.demo.replace("{progress}", &self.percent().to_string()),
            options: vec![d.bow.clone()],
        });
        base.score = self.score(d);
        base
    }

    /// One tick at `pos`; `elapsed_ms` into the run.
    fn step(
        &mut self,
        d: &HackathonDef,
        st: &Stage,
        k: usize,
        pos: Vec3,
        now: u64,
        elapsed_ms: u32,
    ) -> Vec<Fx> {
        let mut fx = Vec::new();
        if let Some(until) = self.demo {
            if now >= until {
                fx.push(self.finish(d));
            }
            return fx;
        }
        if elapsed_ms >= d.length_ms() {
            self.demo = Some(now + ticks(d.demo_ms));
            self.seq = self.seq.wrapping_add(1);
            let (at, yaw) = st.stage[k % st.stage.len()];
            fx.push(Fx::Teleport(at, yaw));
            fx.push(Fx::Note(
                format!("Demo time. The bar says {}%. The bar lies.", self.percent()),
                true,
            ));
            if let Some(mc) = st.mc {
                fx.push(Fx::Say(mc, d.mc_line.clone()));
            }
            return fx;
        }
        let dt = 1.0 / crate::time::TICK_HZ as f32;
        let hour = elapsed_ms / d.hour_ms.max(1);
        let night = (d.night.0..d.night.1).contains(&hour);
        let drain = d.drain as f32 / 10.0 * if night { 2.0 } else { 1.0 };
        self.focus = (self.focus - drain * dt).max(0.0);
        if self.focus <= 0.0 {
            self.crashed = true;
        }
        self.progress += d.progress as f32 / 100.0 * self.focus / 100.0 * dt;
        for (i, (spot, kind)) in st.spots.iter().enumerate() {
            if now < self.ready[i] || xz(*spot, pos) > d.reach_m || self.focus >= d.full as f32 {
                continue;
            }
            self.ready[i] = now + ticks(d.respawn_ms);
            let (gain, what) = match kind {
                Snack::Pizza if hour >= d.cold_after => (d.cold_focus, "Cold pizza"),
                Snack::Pizza => (d.pizza_focus, "Pizza"),
                Snack::Drink => (d.drink_focus, "Energy drink"),
            };
            match kind {
                Snack::Pizza => self.pizzas = self.pizzas.saturating_add(1),
                Snack::Drink => self.drinks = self.drinks.saturating_add(1),
            }
            self.focus = (self.focus + gain as f32).min(100.0);
            fx.push(Fx::Note(format!("{what}: +{gain} focus"), true));
        }
        while let Some((h, line)) = d.notes.get(self.notes) {
            if hour < *h {
                break;
            }
            self.notes += 1;
            fx.push(Fx::Note(format!("{}  {line}", d.clock(elapsed_ms)), true));
        }
        fx
    }

    fn finish(&self, d: &HackathonDef) -> Fx {
        Fx::Finish(self.score(d), !self.crashed)
    }
}

impl Sim {
    pub(crate) fn update_hackathon(&mut self) {
        let now = self.now;
        let elapsed = self.minigame_elapsed_ms();
        let open = self.open_runs();
        let Some(o) = self.minigame.outing.as_mut() else {
            return;
        };
        let Some(d) = self.content.minigames[o.def].hackathon.as_ref() else {
            return;
        };
        let crate::corporate::Stage::Hackathon(st) = &mut o.stage else {
            return;
        };
        if st.mc.is_none() {
            st.mc = (0..self.units.len()).find(|i| {
                let u = &self.units[*i];
                !u.despawned && u.state.key == d.mc
            });
        }
        let mut out = Vec::new();
        for (k, (p, pos)) in open.into_iter().enumerate() {
            let Some(GameRun::Hackathon(run)) = o.runs.get_mut(&p).map(|r| &mut r.game) else {
                continue;
            };
            let fx = run.step(d, st, k, pos, now, elapsed);
            if !fx.is_empty() {
                out.push((p, fx));
            }
        }
        for (p, fx) in out {
            self.apply_fx(p, fx);
        }
    }

    pub(crate) fn hackathon_pick(&mut self, p: EntityId, seq: u16, _option: u8) -> Vec<Fx> {
        let Some(o) = self.minigame.outing.as_mut() else {
            return Vec::new();
        };
        let Some(d) = self.content.minigames[o.def].hackathon.as_ref() else {
            return Vec::new();
        };
        match o.runs.get_mut(&p).map(|r| &mut r.game) {
            Some(GameRun::Hackathon(run)) if run.demo.is_some() && seq == run.seq => {
                vec![run.finish(d)]
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_runs_from_saturday_morning_into_sunday() {
        let d = HackathonDef {
            hours: 24,
            hour_ms: 10_000,
            start_hour: 10,
            focus_start: 60,
            drain: 10,
            night: (16, 20),
            progress: 50,
            pizzas: 1,
            drinks: 1,
            pizza_focus: 20,
            cold_after: 12,
            cold_focus: 10,
            drink_focus: 35,
            full: 90,
            respawn_ms: 1000,
            reach_m: 1.5,
            notes: Vec::new(),
            mc: String::new(),
            mc_line: String::new(),
            demo: String::new(),
            bow: String::new(),
            demo_ms: 1000,
            score: HackathonScore {
                per_percent: 10,
                per_snack: 5,
            },
        };
        assert_eq!(d.clock(0), "Sat 10:00");
        assert_eq!(d.clock(45_000), "Sat 14:30");
        assert_eq!(d.clock(150_000), "Sun 01:00");
        assert_eq!(d.length_ms(), 240_000);
    }
}
