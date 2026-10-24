//! "Mandatory Fun: Teambuilding", the stealth minigame (frame in `minigame.rs`). Each member
//! sneaks through the garden party on their own: five small objectives, optional canapes, then
//! the exit. People walk fixed loops (`WatcherDef`), pausing at waypoints to look around; a
//! cone in front of each is what they see, blocked by walls and tall props. Standing in a cone
//! fills a suspicion meter (faster up close; the CEO notices at once), running is heard nearby,
//! and a hiding spot (plants, under the buffet, the toilet queue, behind a roll-up banner) hides
//! a player who stands still in it. A full meter is a catch, and the watcher decides what
//! happens: a manager sends you back to the start, an AI enthusiast corners you with a take,
//! the HR Fun Committee walks you to the icebreaker circle, the CEO resets you. Watchers are
//! friendly zone units moved here, so every member sees the same party; runs are per player.
//! Positions come from layout anchors (`tb_start`, `tb_exit`, `tb_icebreaker`, `hide_<n>`,
//! `snack_<n>`, each objective's anchor and `route:<route>:<n>` waypoints), so the server and the
//! client agree on the venue.

use crate::content::{Content, ContentError, ticks};
use crate::geom::{Aabb, Vec3, distance, is_facing, line_of_sight};
use crate::minigame::{GameRun, MinigameView};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::Event;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Eye height of a watcher and chest height of the player for the sight line, so low props
/// (tables, hedges up to the waist) do not hide anyone but tall ones do.
pub(crate) const EYE_Y: f32 = 1.55;
pub(crate) const CHEST_Y: f32 = 1.1;
/// Speed is measured over this many ticks (half a second), smoothing network batching.
const TRAIL_TICKS: usize = 15;
/// A step longer than this between ticks is a teleport, not running.
const TELEPORT_M: f32 = 4.0;
/// Hidden only when nearly still.
const HIDE_MPS: f32 = 1.5;
/// Close in the cone counts this much more than at its edge.
const NEAR_BONUS: f32 = 2.0;
/// Heard but not seen fills at this share of the base rate.
const HEARD_SHARE: f32 = 0.8;
/// Suspicion drains per second while nobody is looking.
const DRAIN_PER_S: f32 = 0.5;
/// Watchers turn their head this far either way while they pause, once per `SCAN_MS`.
const SCAN_RAD: f32 = 0.75;
const SCAN_MS: f32 = 4200.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatcherKind {
    /// "Let's circle back to the beginning": back to the start.
    Manager,
    /// "Let me tell you about AI": cornered for a few seconds with a take.
    Talker,
    /// The HR Fun Committee: a detour through the icebreaker circle.
    Hr,
    /// Rare, sweeping, notices at once: back to the start.
    Ceo,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WatcherDef {
    pub unit: String,
    pub kind: WatcherKind,
    /// Waypoint anchors `route:<route>:<n>` from n = 0, walked as a loop.
    pub route: String,
    pub speed_mps: f32,
    /// Pause at every waypoint (looking left and right).
    pub pause_ms: u32,
    /// Longer pauses at some waypoints: (waypoint, ms).
    #[serde(default)]
    pub pauses: Vec<(usize, u32)>,
    pub range_m: f32,
    pub half_angle_deg: f32,
    /// What it says on a catch, in turn (a talker's take comes from `ai_takes`).
    pub lines: Vec<String>,
    /// Said when it leaves waypoint 0 (the CEO's entrance).
    #[serde(default)]
    pub entrance: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ObjectiveDef {
    pub key: String,
    pub name: String,
    pub anchor: String,
    pub radius_m: f32,
    /// Stand here this long, unseen.
    pub dwell_ms: u32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ScoreDef {
    pub base: u32,
    pub per_snack: u32,
    pub per_catch: u32,
    pub per_second: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TeambuildingDef {
    pub watchers: Vec<WatcherDef>,
    pub objectives: Vec<ObjectiveDef>,
    /// Anchors `snack_1..=snacks`.
    pub snacks: u8,
    /// Anchors `hide_1..=hides`.
    pub hides: u8,
    pub hide_radius_m: f32,
    pub snack_radius_m: f32,
    pub exit_radius_m: f32,
    /// In plain sight at a cone's edge this long is a catch; up close it is three times faster.
    pub notice_ms: u32,
    /// Moving faster than this is running, heard within `noise_m`.
    pub noisy_mps: f32,
    pub noise_m: f32,
    /// After a catch (and its root) nobody can catch you again for this long.
    pub grace_ms: u32,
    /// Stun auras: the AI talker's corner and the icebreaker.
    pub cornered_aura: String,
    pub cornered_ms: u32,
    pub icebreaker_aura: String,
    pub icebreaker_ms: u32,
    /// The bad AI takes, one per talker catch in turn.
    pub ai_takes: Vec<String>,
    pub score: ScoreDef,
}

impl TeambuildingDef {
    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("teambuilding: {m}")));
        // The view carries objectives and collectibles as bit masks.
        if self.objectives.is_empty() || self.objectives.len() > 16 || self.snacks > 32 {
            return bad("1 to 16 objectives, at most 32 collectibles".into());
        }
        for w in &self.watchers {
            if c.unit_def(&w.unit).is_none_or(|u| u.hostile) {
                return bad(format!("watcher {} is no friendly unit", w.unit));
            }
            if w.speed_mps <= 0.0 || w.range_m <= 0.0 || w.lines.is_empty() {
                return bad(format!("watcher {} needs speed, range and lines", w.unit));
            }
        }
        for a in [&self.cornered_aura, &self.icebreaker_aura] {
            if !c.aura(a).is_some_and(|d| d.stun) {
                return bad(format!("{a} is no stun aura"));
            }
        }
        if self.ai_takes.is_empty() || self.notice_ms == 0 {
            return bad("no AI takes or no notice time".into());
        }
        Ok(())
    }
}

/// A loop of waypoints walked at a fixed speed with a pause at each; a pure function of time,
/// so every peer agrees where everyone is.
#[derive(Clone, Debug, Default)]
pub struct Patrol {
    pub points: Vec<Vec3>,
    /// Pause at each waypoint, ms.
    pub pauses: Vec<u32>,
    pub speed_mps: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatrolPose {
    pub pos: Vec3,
    pub yaw: f32,
    pub moving: bool,
    /// Pausing at this waypoint.
    pub at: Option<usize>,
}

fn yaw_to(from: Vec3, to: Vec3) -> f32 {
    (-(to.x - from.x)).atan2(-(to.z - from.z))
}

fn xz(a: Vec3, b: Vec3) -> f32 {
    distance(Vec3::new(a.x, 0.0, a.z), Vec3::new(b.x, 0.0, b.z))
}

impl Patrol {
    fn leg_ms(&self, k: usize) -> u64 {
        let n = self.points.len();
        let d = xz(self.points[k], self.points[(k + 1) % n]);
        (d / self.speed_mps.max(0.1) * 1000.0) as u64
    }

    fn pause(&self, k: usize) -> u64 {
        u64::from(self.pauses.get(k).copied().unwrap_or(0))
    }

    pub fn cycle_ms(&self) -> u64 {
        (0..self.points.len())
            .map(|k| self.pause(k) + self.leg_ms(k))
            .sum()
    }

    /// Where the walker is `t_ms` after starting its pause at waypoint 0.
    pub fn pose(&self, t_ms: u64) -> PatrolPose {
        let n = self.points.len();
        if n == 0 {
            return PatrolPose {
                pos: Vec3::default(),
                yaw: 0.0,
                moving: false,
                at: None,
            };
        }
        let cycle = self.cycle_ms().max(1);
        let mut t = t_ms % cycle;
        for k in 0..n {
            let (a, b) = (self.points[k], self.points[(k + 1) % n]);
            let came_from = self.points[(k + n - 1) % n];
            let pause = self.pause(k);
            if t < pause {
                // Looking ahead along the way it came, sweeping left and right.
                let ahead = if n > 1 { yaw_to(came_from, a) } else { 0.0 };
                let scan = (t as f32 / SCAN_MS * std::f32::consts::TAU).sin() * SCAN_RAD;
                return PatrolPose {
                    pos: a,
                    yaw: ahead + scan,
                    moving: false,
                    at: Some(k),
                };
            }
            t -= pause;
            let leg = self.leg_ms(k);
            if t < leg {
                let f = t as f32 / leg.max(1) as f32;
                return PatrolPose {
                    pos: Vec3::new(a.x + (b.x - a.x) * f, a.y, a.z + (b.z - a.z) * f),
                    yaw: yaw_to(a, b),
                    moving: true,
                    at: None,
                };
            }
            t -= leg;
        }
        PatrolPose {
            pos: self.points[0],
            yaw: 0.0,
            moving: false,
            at: Some(0),
        }
    }
}

/// Distance at which a watcher at `pos` facing `yaw` sees `target`, if it does: inside the
/// cone and with a clear sight line from eye to chest.
pub fn sees(
    pos: Vec3,
    yaw: f32,
    range_m: f32,
    half_angle_deg: f32,
    target: Vec3,
    blockers: &[Aabb],
) -> Option<f32> {
    let d = xz(pos, target);
    if d > range_m || !is_facing(pos, yaw, target, half_angle_deg.to_radians()) {
        return None;
    }
    let eye = Vec3::new(pos.x, pos.y + EYE_Y, pos.z);
    let chest = Vec3::new(target.x, target.y + CHEST_Y, target.z);
    line_of_sight(eye, chest, blockers).then_some(d)
}

/// Time plus collectibles: the base, plus canapes, minus catches and seconds, never below 0.
pub fn score(s: &ScoreDef, elapsed_ms: u32, collected: u8, catches: u8) -> u32 {
    let plus = i64::from(s.base) + i64::from(s.per_snack) * i64::from(collected);
    let minus = i64::from(s.per_catch) * i64::from(catches)
        + i64::from(s.per_second) * i64::from(elapsed_ms / 1000);
    (plus - minus).max(0) as u32
}

pub(crate) struct WatcherRt {
    pub(crate) def: usize,
    /// Unit index of the spawned watcher.
    pub(crate) unit: Option<usize>,
    pub(crate) patrol: Patrol,
    pub(crate) was_moving: bool,
    pub(crate) line: usize,
}

/// The venue's resolved anchors.
#[derive(Default)]
pub(crate) struct Venue {
    pub(crate) start: (Vec3, f32),
    pub(crate) exit: Vec3,
    pub(crate) icebreaker: (Vec3, f32),
    pub(crate) hides: Vec<Vec3>,
    pub(crate) objectives: Vec<Vec3>,
    pub(crate) snacks: Vec<Vec3>,
    pub(crate) watchers: Vec<WatcherRt>,
}

/// Waypoint anchor `n` of `route` (`route:<route>:<n>`).
pub fn route_anchor(route: &str, n: usize) -> String {
    format!("route:{route}:{n}")
}

impl Venue {
    pub(crate) fn resolve(
        def: &TeambuildingDef,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    ) -> Result<Venue, String> {
        let need = |n: &str| anchors(n).ok_or_else(|| format!("no anchor {n}"));
        let numbered = |prefix: &str, n: u8| -> Result<Vec<Vec3>, String> {
            (1..=n)
                .map(|k| need(&format!("{prefix}_{k}")).map(|a| a.0))
                .collect()
        };
        Ok(Venue {
            start: need("tb_start")?,
            exit: need("tb_exit")?.0,
            icebreaker: need("tb_icebreaker")?,
            hides: numbered("hide", def.hides)?,
            objectives: def
                .objectives
                .iter()
                .map(|o| need(&o.anchor).map(|a| a.0))
                .collect::<Result<_, _>>()?,
            snacks: numbered("snack", def.snacks)?,
            watchers: Self::walkers(&def.watchers, anchors)?,
        })
    }

    /// Each walker's loop from its `route:<route>:<n>` anchors.
    pub(crate) fn walkers(
        defs: &[WatcherDef],
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    ) -> Result<Vec<WatcherRt>, String> {
        let mut watchers = Vec::new();
        for (k, w) in defs.iter().enumerate() {
            let mut points = Vec::new();
            while let Some((p, _)) = anchors(&route_anchor(&w.route, points.len())) {
                points.push(p);
            }
            if points.is_empty() {
                return Err(format!("route {} has no waypoints", w.route));
            }
            let pauses = (0..points.len())
                .map(|i| {
                    w.pauses
                        .iter()
                        .find(|(at, _)| *at == i)
                        .map_or(w.pause_ms, |p| p.1)
                })
                .collect();
            watchers.push(WatcherRt {
                def: k,
                unit: None,
                patrol: Patrol {
                    points,
                    pauses,
                    speed_mps: w.speed_mps,
                },
                was_moving: false,
                line: 0,
            });
        }
        Ok(watchers)
    }
}

/// A watcher as a run sees it this tick.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Eye {
    pub(crate) pos: Vec3,
    pub(crate) yaw: f32,
    pub(crate) kind: WatcherKind,
    pub(crate) range_m: f32,
    pub(crate) half_angle_deg: f32,
}

pub(crate) struct StepInput<'a> {
    pub(crate) now: u64,
    pub(crate) elapsed_ms: u32,
    pub(crate) pos: Vec3,
    /// Rooted by a catch (the clock runs, nothing else does).
    pub(crate) stunned: bool,
    pub(crate) eyes: &'a [Eye],
    pub(crate) blockers: &'a [Aabb],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Step {
    Nothing,
    /// Every objective done and at the exit: (score, flawless).
    Finished(u32, bool),
    /// Caught by watcher `k`.
    Caught(usize),
}

/// One player's sneak.
pub(crate) struct TbRun {
    pub(crate) catches: u8,
    objectives: Vec<bool>,
    snacks: Vec<bool>,
    dwell: Option<(usize, u32)>,
    suspicion: f32,
    grace_until: u64,
    trail: VecDeque<Vec3>,
    speed: f32,
    hidden: bool,
    noisy: bool,
    takes: usize,
}

impl TbRun {
    pub(crate) fn sized(objectives: usize, snacks: usize) -> TbRun {
        TbRun {
            catches: 0,
            objectives: vec![false; objectives],
            snacks: vec![false; snacks],
            dwell: None,
            suspicion: 0.0,
            grace_until: 0,
            trail: VecDeque::new(),
            speed: 0.0,
            hidden: false,
            noisy: false,
            takes: 0,
        }
    }

    pub(crate) fn collected_count(&self) -> u8 {
        self.snacks.iter().filter(|s| **s).count() as u8
    }

    fn all_done(&self) -> bool {
        self.objectives.iter().all(|o| *o)
    }

    pub(crate) fn view(
        &self,
        def: &TeambuildingDef,
        v: &Venue,
        mut base: MinigameView,
    ) -> MinigameView {
        base.objectives = self
            .objectives
            .iter()
            .enumerate()
            .filter(|(_, d)| **d)
            .fold(0, |m, (i, _)| m | (1 << i));
        base.dwell = self.dwell.and_then(|(k, ms)| {
            let need = def.objectives.get(k)?.dwell_ms.max(1);
            Some((k as u8, (ms.min(need) * 100 / need) as u8))
        });
        base.collected = self.collected_count();
        base.taken = self
            .snacks
            .iter()
            .enumerate()
            .take(32)
            .filter(|(_, t)| **t)
            .fold(0, |m, (i, _)| m | (1 << i));
        base.collectibles = v.snacks.len() as u8;
        base.catches = self.catches;
        // Steps of 5 % keep the routed updates few while the meter moves.
        base.suspicion = ((self.suspicion.clamp(0.0, 1.0) * 20.0).round() * 5.0) as u8;
        base.hidden = self.hidden;
        base.noisy = self.noisy;
        base.score = score(&def.score, base.elapsed_ms, base.collected, self.catches);
        base
    }

    /// The next AI take for a talker catch.
    pub(crate) fn next_take<'a>(&mut self, def: &'a TeambuildingDef) -> &'a str {
        let t = &def.ai_takes[self.takes % def.ai_takes.len()];
        self.takes += 1;
        t
    }

    /// After a catch: the meter, the current dwell and the speed trail start over, and a
    /// grace period follows the root.
    pub(crate) fn caught(&mut self, now: u64, root_ms: u32, grace_ms: u32) {
        self.catches = self.catches.saturating_add(1);
        self.suspicion = 0.0;
        self.dwell = None;
        self.trail.clear();
        self.grace_until = now + ticks(root_ms.saturating_add(grace_ms));
    }

    pub(crate) fn step(&mut self, def: &TeambuildingDef, v: &Venue, inp: &StepInput) -> Step {
        let dt_ms = 1000 / crate::time::TICK_HZ;
        let dt = dt_ms as f32 / 1000.0;
        if self
            .trail
            .back()
            .is_some_and(|b| xz(*b, inp.pos) > TELEPORT_M)
        {
            self.trail.clear();
        }
        self.trail.push_back(inp.pos);
        while self.trail.len() > TRAIL_TICKS {
            self.trail.pop_front();
        }
        self.speed = match (self.trail.front(), self.trail.len()) {
            (Some(f), n) if n > 1 => xz(*f, inp.pos) / ((n - 1) as f32 * dt),
            _ => 0.0,
        };
        self.noisy = self.speed > def.noisy_mps;
        self.hidden =
            self.speed < HIDE_MPS && v.hides.iter().any(|h| xz(*h, inp.pos) <= def.hide_radius_m);
        if inp.stunned {
            return Step::Nothing;
        }
        for (k, s) in v.snacks.iter().enumerate() {
            if !self.snacks[k] && xz(*s, inp.pos) <= def.snack_radius_m {
                self.snacks[k] = true;
            }
        }
        let here = (0..v.objectives.len()).find(|&k| {
            !self.objectives[k] && xz(v.objectives[k], inp.pos) <= def.objectives[k].radius_m
        });
        self.dwell = here.map(|k| {
            let ms = match self.dwell {
                Some((d, ms)) if d == k => ms + dt_ms,
                _ => dt_ms,
            };
            if ms >= def.objectives[k].dwell_ms {
                self.objectives[k] = true;
            }
            (k, ms)
        });
        if self.dwell.is_some_and(|(k, _)| self.objectives[k]) {
            self.dwell = None;
        }
        if self.all_done() && xz(v.exit, inp.pos) <= def.exit_radius_m {
            let collected = self.collected_count();
            let s = score(&def.score, inp.elapsed_ms, collected, self.catches);
            return Step::Finished(s, self.catches == 0);
        }
        if inp.now < self.grace_until {
            self.suspicion = 0.0;
            return Step::Nothing;
        }
        let base = 1000.0 / def.notice_ms as f32;
        let mut best: Option<(usize, f32)> = None;
        for (k, e) in inp.eyes.iter().enumerate() {
            let seen = if self.hidden {
                None
            } else {
                sees(
                    e.pos,
                    e.yaw,
                    e.range_m,
                    e.half_angle_deg,
                    inp.pos,
                    inp.blockers,
                )
            };
            let rate = match seen {
                Some(_) if e.kind == WatcherKind::Ceo => f32::INFINITY,
                Some(d) => base * (1.0 + NEAR_BONUS * (1.0 - d / e.range_m).max(0.0)),
                None if self.noisy
                    && xz(e.pos, inp.pos) <= def.noise_m
                    && line_of_sight(
                        Vec3::new(e.pos.x, e.pos.y + EYE_Y, e.pos.z),
                        Vec3::new(inp.pos.x, inp.pos.y + CHEST_Y, inp.pos.z),
                        inp.blockers,
                    ) =>
                {
                    base * HEARD_SHARE
                }
                None => 0.0,
            };
            if rate > 0.0 && best.is_none_or(|(_, r)| rate > r) {
                best = Some((k, rate));
            }
        }
        match best {
            Some((k, rate)) => {
                self.suspicion += rate * dt;
                if self.suspicion >= 1.0 {
                    return Step::Caught(k);
                }
            }
            None => self.suspicion = (self.suspicion - DRAIN_PER_S * dt).max(0.0),
        }
        Step::Nothing
    }
}

impl Sim {
    fn tb_rules(&self) -> Option<TeambuildingDef> {
        let o = self.minigame.outing.as_ref()?;
        self.content.minigames.get(o.def)?.teambuilding.clone()
    }

    /// The walkers of the current outing: Teambuilding's watchers or Open-Space Focus's
    /// disturbers.
    fn walker_defs(&self) -> Option<Vec<WatcherDef>> {
        let o = self.minigame.outing.as_ref()?;
        let def = self.content.minigames.get(o.def)?;
        match def.rules()? {
            crate::minigame::Rules::Teambuilding(t) => Some(t.watchers.clone()),
            crate::minigame::Rules::Focus(f) => {
                Some(f.disturbers.iter().map(|d| d.walk.clone()).collect())
            }
            _ => None,
        }
    }

    /// Binds each watcher to its spawned unit (the n-th unit of a key to the n-th watcher of
    /// that key) and stands it on its loop.
    pub(crate) fn teambuilding_place_watchers(&mut self) {
        let Some(watchers) = self.walker_defs() else {
            return;
        };
        let mut taken: Vec<usize> = Vec::new();
        let mut binds = Vec::new();
        for (k, w) in watchers.iter().enumerate() {
            let found = (0..self.units.len()).find(|i| {
                let u = &self.units[*i];
                !u.despawned && u.state.key == w.unit && !taken.contains(i)
            });
            if let Some(i) = found {
                taken.push(i);
            }
            binds.push((k, found));
        }
        if let Some(o) = self.minigame.outing.as_mut() {
            for (k, i) in binds {
                o.venue.watchers[k].unit = i;
            }
        }
        self.move_watchers();
    }

    /// Whether unit `i` walks a minigame loop (wandering leaves it alone).
    pub(crate) fn is_watcher(&self, i: usize) -> bool {
        self.minigame
            .outing
            .as_ref()
            .is_some_and(|o| o.venue.watchers.iter().any(|w| w.unit == Some(i)))
    }

    pub(crate) fn move_watchers(&mut self) {
        let t = u64::from(self.minigame_elapsed_ms());
        let Some(watchers) = self.walker_defs() else {
            return;
        };
        let Some(o) = self.minigame.outing.as_mut() else {
            return;
        };
        let mut entrances = Vec::new();
        for w in &mut o.venue.watchers {
            let Some(i) = w.unit else { continue };
            let pose = w.patrol.pose(t);
            if pose.moving && !w.was_moving && t > 0 {
                let left_zero = w.patrol.pose(t.saturating_sub(40)).at == Some(0);
                if let (true, Some(line)) = (left_zero, &watchers[w.def].entrance) {
                    entrances.push((i, line.clone()));
                }
            }
            w.was_moving = pose.moving;
            let u = &mut self.units[i];
            u.state.pos = pose.pos;
            u.state.yaw = pose.yaw;
            u.moving = pose.moving;
        }
        for (i, line) in entrances {
            self.request_yell_line(i, crate::content::YellKind::Blast, Some(line));
        }
    }

    pub(crate) fn update_teambuilding(&mut self) {
        let Some(rules) = self.tb_rules() else {
            return;
        };
        self.move_watchers();
        let elapsed_ms = self.minigame_elapsed_ms();
        let Some(o) = self.minigame.outing.as_ref() else {
            return;
        };
        let eyes: Vec<Eye> = o
            .venue
            .watchers
            .iter()
            .filter_map(|w| {
                let u = &self.units[w.unit?].state;
                let d = &rules.watchers[w.def];
                Some(Eye {
                    pos: u.pos,
                    yaw: u.yaw,
                    kind: d.kind,
                    range_m: d.range_m,
                    half_angle_deg: d.half_angle_deg,
                })
            })
            .collect();
        let eye_watcher: Vec<usize> = o
            .venue
            .watchers
            .iter()
            .enumerate()
            .filter(|(_, w)| w.unit.is_some())
            .map(|(k, _)| k)
            .collect();
        let players: Vec<EntityId> = o
            .runs
            .iter()
            .filter(|(_, r)| r.over.is_none())
            .map(|(p, _)| *p)
            .collect();
        let mut outcomes = Vec::new();
        for p in players {
            let Some(i) = self.index(p) else { continue };
            let s = &self.units[i].state;
            if !s.alive {
                continue;
            }
            let inp = StepInput {
                now: self.now,
                elapsed_ms,
                pos: s.pos,
                stunned: s.stunned,
                eyes: &eyes,
                blockers: &self.blockers,
            };
            let Some(o) = self.minigame.outing.as_mut() else {
                return;
            };
            let Some(run) = o.runs.get_mut(&p) else {
                continue;
            };
            let GameRun::Teambuilding(t) = &mut run.game else {
                continue;
            };
            outcomes.push((p, t.step(&rules, &o.venue, &inp)));
        }
        for (p, step) in outcomes {
            match step {
                Step::Nothing => {}
                Step::Finished(score, flawless) => self.end_run(p, Some((score, flawless))),
                Step::Caught(k) => {
                    if let Some(&w) = eye_watcher.get(k) {
                        self.tb_catch(p, w, &rules);
                    }
                }
            }
        }
    }

    /// Watcher `w` caught player `p`: the line, the outcome (start, root or icebreaker) and
    /// the event everyone sees.
    fn tb_catch(&mut self, p: EntityId, w: usize, rules: &TeambuildingDef) {
        let Some(pi) = self.index(p) else { return };
        let Some((wi, def_k, line_k)) = self.minigame.outing.as_mut().and_then(|o| {
            let rt = &mut o.venue.watchers[w];
            rt.line += 1;
            Some((rt.unit?, rt.def, rt.line - 1))
        }) else {
            return;
        };
        let wd = &rules.watchers[def_k];
        let said = wd.lines[line_k % wd.lines.len()].clone();
        let (start, ice) = match self.minigame.outing.as_ref() {
            Some(o) => (o.venue.start, o.venue.icebreaker),
            None => return,
        };
        let (root, line) = match wd.kind {
            WatcherKind::Manager | WatcherKind::Ceo => {
                self.tb_teleport(pi, start);
                (0, said.clone())
            }
            WatcherKind::Hr => {
                self.tb_teleport(pi, ice);
                self.apply_aura(
                    pi,
                    &rules.icebreaker_aura,
                    rules.icebreaker_ms,
                    None,
                    p,
                    None,
                );
                (rules.icebreaker_ms, said.clone())
            }
            WatcherKind::Talker => {
                self.apply_aura(pi, &rules.cornered_aura, rules.cornered_ms, None, p, None);
                let take = match self
                    .minigame
                    .outing
                    .as_mut()
                    .and_then(|o| o.runs.get_mut(&p))
                    .map(|r| &mut r.game)
                {
                    Some(GameRun::Teambuilding(t)) => t.next_take(rules).to_string(),
                    _ => String::new(),
                };
                (rules.cornered_ms, take)
            }
        };
        let now = self.now;
        if let Some(r) = self
            .minigame
            .outing
            .as_mut()
            .and_then(|o| o.runs.get_mut(&p))
            && let GameRun::Teambuilding(t) = &mut r.game
        {
            t.caught(now, root, rules.grace_ms);
        }
        self.request_yell_line(wi, crate::content::YellKind::Blast, Some(said));
        self.events.push(Event::MinigameCaught {
            unit: p,
            by: Self::id_of(wi),
            kind: wd.kind,
            line,
        });
    }

    fn tb_teleport(&mut self, pi: usize, (pos, yaw): (Vec3, f32)) {
        let s = &mut self.units[pi].state;
        s.pos = pos;
        s.yaw = yaw;
        self.events.push(Event::Teleported {
            unit: Self::id_of(pi),
            pos,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Patrol {
        Patrol {
            points: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 10.0),
                Vec3::new(0.0, 0.0, 10.0),
            ],
            pauses: vec![1000, 0, 0, 0],
            speed_mps: 2.0,
        }
    }

    #[test]
    fn a_patrol_pauses_walks_its_legs_and_loops() {
        let p = square();
        assert_eq!(p.cycle_ms(), 1000 + 4 * 5000);
        let a = p.pose(500);
        assert_eq!((a.at, a.moving), (Some(0), false));
        // Half way along the first leg, heading +X (yaw -90 degrees).
        let b = p.pose(1000 + 2500);
        assert!(b.moving && (b.pos.x - 5.0).abs() < 1e-3 && b.pos.z.abs() < 1e-3);
        assert!((b.yaw + std::f32::consts::FRAC_PI_2).abs() < 1e-3);
        let c = p.pose(p.cycle_ms() + 1000 + 2500);
        assert_eq!(b, c);
    }

    #[test]
    fn the_cone_sees_ahead_not_behind_nor_through_walls() {
        let at = Vec3::default();
        // Yaw 0 faces -Z.
        assert!(sees(at, 0.0, 8.0, 35.0, Vec3::new(0.0, 0.0, -5.0), &[]).is_some());
        assert!(sees(at, 0.0, 8.0, 35.0, Vec3::new(0.0, 0.0, 5.0), &[]).is_none());
        assert!(sees(at, 0.0, 8.0, 35.0, Vec3::new(0.0, 0.0, -9.0), &[]).is_none());
        assert!(sees(at, 0.0, 8.0, 35.0, Vec3::new(5.0, 0.0, -2.0), &[]).is_none());
        let wall = Aabb {
            min: Vec3::new(-2.0, 0.0, -3.1),
            max: Vec3::new(2.0, 3.0, -2.9),
        };
        assert!(sees(at, 0.0, 8.0, 35.0, Vec3::new(0.0, 0.0, -5.0), &[wall]).is_none());
        // A waist-high table does not hide anyone standing behind it.
        let table = Aabb {
            min: Vec3::new(-2.0, 0.0, -3.1),
            max: Vec3::new(2.0, 0.8, -2.9),
        };
        assert!(sees(at, 0.0, 8.0, 35.0, Vec3::new(0.0, 0.0, -5.0), &[table]).is_some());
    }

    #[test]
    fn the_score_is_time_plus_collectibles() {
        let s = ScoreDef {
            base: 2400,
            per_snack: 50,
            per_catch: 250,
            per_second: 3,
        };
        assert_eq!(score(&s, 300_000, 8, 0), 2400 + 400 - 900);
        assert_eq!(score(&s, 300_000, 8, 1), 2400 + 400 - 900 - 250);
        assert_eq!(score(&s, 3_000_000, 0, 5), 0);
    }
}
