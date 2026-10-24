//! Sight cones for hostile units and squads built from the roster (docs/ILMARI_1_0.md D.3, D.9
//! items 3 and 4): Wing 5's Red Team Blue Team is a stealth fight and a mirror of the party.
//!
//! `UnitDef::sight` gives a hostile unit a cone (`SightDef`) that fills a per-player suspicion
//! meter with the Teambuilding vision code (`teambuilding::sees`: range, half angle, a clear line
//! from eye to chest, so tall props hide you and low ones do not). A player in stealth is never
//! seen or heard, running within `noise_m` is heard through walls' absence, and a sneaking pace
//! halves the fill rate. At 1.0 the sensor raises an incident: the unit's `detected_aura` goes on
//! the player and `Event::Incident` names both. Companions are not `Sim::players`, so they are
//! never checked; a unit's patrol is a pure function of the clock, as in Teambuilding.
//! `Effect::SummonSquad` reads the living humans' roles (the `SummonMirror` role read) and
//! summons one unit per role present, padded to a minimum body count. Numbers: docs/FORMULAS.md.

use crate::content::{Content, ContentError, Role};
use crate::geom::{Vec3, distance, line_of_sight};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::teambuilding::{CHEST_Y, EYE_Y, Patrol, sees};
use crate::time::{TICK_HZ, ticks_to_ms};
use crate::types::{AiState, Event};
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};

/// Close in the cone counts this much more than at its edge (Teambuilding's figure).
const NEAR_BONUS: f32 = 2.0;
/// Heard but not seen fills at this share of the base rate.
const HEARD_SHARE: f32 = 0.8;
/// A player at sneak pace fills at this share of the rate.
const SNEAK_SHARE: f32 = 0.5;
/// Suspicion drains per second while nobody is looking.
const DRAIN_PER_S: f32 = 0.5;
/// Speed is measured over this many ticks (half a second).
const TRAIL_TICKS: usize = 15;
/// A step longer than this between ticks is a teleport, not running.
const TELEPORT_M: f32 = 4.0;

fn noisy_mps() -> f32 {
    5.0
}
fn noise_m() -> f32 {
    7.0
}
fn sneak_mps() -> f32 {
    3.0
}
fn patrol_mps() -> f32 {
    1.6
}

/// A hostile unit's eyes.
#[derive(Clone, Debug, Deserialize)]
pub struct SightDef {
    pub range_m: f32,
    pub half_angle_deg: f32,
    /// A player in plain sight at the cone's edge fills the meter in this long; up close it is
    /// three times faster.
    pub notice_ms: u32,
    /// Corners of the loop it walks while idle, as (x, z) offsets from where it spawned; empty:
    /// it stands and looks along its spawn yaw.
    #[serde(default)]
    pub patrol: Vec<(f32, f32)>,
    #[serde(default = "patrol_mps")]
    pub patrol_mps: f32,
    /// Pause at each corner.
    #[serde(default)]
    pub pause_ms: u32,
    /// The `Detected` aura an incident puts on the player, and for how long.
    pub detected_aura: String,
    pub detected_ms: u32,
    /// Faster than this is running, heard within `noise_m`.
    #[serde(default = "noisy_mps")]
    pub noisy_mps: f32,
    #[serde(default = "noise_m")]
    pub noise_m: f32,
    /// At or below this pace a player is sneaking.
    #[serde(default = "sneak_mps")]
    pub sneak_mps: f32,
}

impl SightDef {
    pub(crate) fn validate(&self, c: &Content, unit: &str) -> Result<(), ContentError> {
        let ok = self.range_m.is_finite()
            && self.range_m > 0.0
            && self.half_angle_deg > 0.0
            && self.half_angle_deg <= 180.0
            && self.notice_ms > 0
            && self.detected_ms > 0
            && c.aura(&self.detected_aura).is_some()
            && self.patrol_mps > 0.0
            && self.noisy_mps > self.sneak_mps
            && (self.patrol.is_empty() || self.patrol.len() >= 2);
        if ok {
            Ok(())
        } else {
            Err(ContentError::Invalid(format!(
                "unit {unit} sight needs a range, an angle, a notice time, a known Detected aura and a patrol of 0 or 2+ corners"
            )))
        }
    }
}

/// Which side of the arena a squad stands on: Red to the caster's right, Blue to its left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum SquadSide {
    Red,
    Blue,
}

/// The unit a squad fields for each role present.
#[derive(Clone, Debug, Deserialize)]
pub struct SquadRoles {
    pub tank: String,
    pub healer: String,
    pub melee: String,
    pub ranged: String,
}

impl SquadRoles {
    pub(crate) fn key(&self, r: Role) -> &str {
        match r {
            Role::Tank => &self.tank,
            Role::Healer => &self.healer,
            Role::Melee => &self.melee,
            Role::Ranged => &self.ranged,
        }
    }

    pub(crate) fn keys(&self) -> [&str; 4] {
        [&self.tank, &self.healer, &self.melee, &self.ranged]
    }
}

/// How many units a squad has for a set of roles: one per distinct role, never fewer than
/// `min_bodies` (a solo Story player still meets a squad of three).
pub fn squad_size(roles: &[Role], min_bodies: u8) -> usize {
    let mut seen: Vec<Role> = Vec::new();
    for r in roles {
        if !seen.contains(r) {
            seen.push(*r);
        }
    }
    seen.len().max(usize::from(min_bodies))
}

/// One player's sneak: pace and suspicion.
#[derive(Default)]
struct Run {
    trail: VecDeque<Vec3>,
    speed: f32,
    suspicion: f32,
}

#[derive(Default)]
pub(crate) struct SightState {
    runs: BTreeMap<EntityId, Run>,
    /// Summoned squad members and their side.
    pub(crate) squads: Vec<(EntityId, SquadSide)>,
    /// Squad members that wear a player's name: (unit, the player it copies).
    pub(crate) owners: Vec<(EntityId, EntityId)>,
    /// Incidents raised since `wing5` last looked: (player, sensor).
    pub(crate) incidents: Vec<(EntityId, EntityId)>,
    /// A honeypot is turning every cone toward `.1` until tick `.0`.
    honeypot: Option<(u64, Vec3)>,
    /// Honeypots share one cooldown, ready again at this tick.
    honeypot_ready: u64,
}

/// How long every cone looks at a honeypot.
pub const HONEYPOT_MS: u32 = 8_000;

fn xz(a: Vec3, b: Vec3) -> f32 {
    distance(Vec3::new(a.x, 0.0, a.z), Vec3::new(b.x, 0.0, b.z))
}

impl Sim {
    /// A fresh zone: nobody is suspected and no squad stands.
    pub(crate) fn sight_on_load(&mut self) {
        self.sight = SightState::default();
    }

    fn sight_of(&self, i: usize) -> Option<&SightDef> {
        self.content.units[self.units[i].def].sight.as_ref()
    }

    /// Unit `i` walks a sight patrol while idle (wandering leaves it alone).
    pub(crate) fn is_patroller(&self, i: usize) -> bool {
        self.sight_of(i).is_some_and(|s| !s.patrol.is_empty())
    }

    /// A player's suspicion meter in percent (the client draws it).
    pub fn sight_suspicion(&self, player: EntityId) -> u8 {
        self.sight
            .runs
            .get(&player)
            .map_or(0, |r| (r.suspicion.clamp(0.0, 1.0) * 100.0).round() as u8)
    }

    /// Whom `unit` fights for, if it was summoned by `Effect::SummonSquad`.
    pub fn squad_side(&self, unit: EntityId) -> Option<SquadSide> {
        self.sight
            .squads
            .iter()
            .find(|(u, _)| *u == unit)
            .map(|(_, s)| *s)
    }

    /// The living members of one squad.
    pub fn squad_units(&self, side: SquadSide) -> Vec<EntityId> {
        self.sight
            .squads
            .iter()
            .filter(|(u, s)| *s == side && self.unit(*u).is_some_and(|x| x.alive))
            .map(|(u, _)| *u)
            .collect()
    }

    pub(crate) fn update_sight(&mut self) {
        let eyes: Vec<usize> = (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                u.state.hostile && u.state.alive && !u.despawned && self.sight_of(i).is_some()
            })
            .collect();
        if eyes.is_empty() {
            if !self.sight.runs.is_empty() {
                self.sight.runs.clear();
            }
            return;
        }
        self.walk_patrols(&eyes);
        let dt = 1.0 / TICK_HZ as f32;
        for p in self.players.clone() {
            let Some(pi) = self.index(p) else { continue };
            if !self.units[pi].state.alive {
                self.sight.runs.remove(&p);
                continue;
            }
            let pos = self.units[pi].state.pos;
            let hidden = self.is_stealthed(pi);
            let run = self.sight.runs.entry(p).or_default();
            if run.trail.back().is_some_and(|b| xz(*b, pos) > TELEPORT_M) {
                run.trail.clear();
            }
            run.trail.push_back(pos);
            while run.trail.len() > TRAIL_TICKS {
                run.trail.pop_front();
            }
            run.speed = match (run.trail.front(), run.trail.len()) {
                (Some(f), n) if n > 1 => xz(*f, pos) / ((n - 1) as f32 * dt),
                _ => 0.0,
            };
            let speed = run.speed;
            let mut best: Option<(usize, f32)> = None;
            for &e in &eyes {
                let Some(def) = self.sight_of(e) else {
                    continue;
                };
                if hidden || self.has_aura_flag(e, |a| a.stun) {
                    continue;
                }
                let u = &self.units[e].state;
                let base = 1000.0 / def.notice_ms as f32;
                let rate = match sees(
                    u.pos,
                    u.yaw,
                    def.range_m,
                    def.half_angle_deg,
                    pos,
                    &self.blockers,
                ) {
                    Some(d) => {
                        let r = base * (1.0 + NEAR_BONUS * (1.0 - d / def.range_m).max(0.0));
                        if speed <= def.sneak_mps {
                            r * SNEAK_SHARE
                        } else {
                            r
                        }
                    }
                    None if speed > def.noisy_mps
                        && xz(u.pos, pos) <= def.noise_m
                        && line_of_sight(
                            Vec3::new(u.pos.x, u.pos.y + EYE_Y, u.pos.z),
                            Vec3::new(pos.x, pos.y + CHEST_Y, pos.z),
                            &self.blockers,
                        ) =>
                    {
                        base * HEARD_SHARE
                    }
                    None => 0.0,
                };
                if rate > 0.0 && best.is_none_or(|(_, r)| rate > r) {
                    best = Some((e, rate));
                }
            }
            let Some(run) = self.sight.runs.get_mut(&p) else {
                continue;
            };
            match best {
                Some((e, rate)) => {
                    run.suspicion += rate * dt;
                    if run.suspicion >= 1.0 {
                        run.suspicion = 0.0;
                        self.raise_incident(pi, e);
                    }
                }
                None => run.suspicion = (run.suspicion - DRAIN_PER_S * dt).max(0.0),
            }
        }
    }

    fn raise_incident(&mut self, pi: usize, sensor: usize) {
        let Some((aura, ms)) = self
            .sight_of(sensor)
            .map(|s| (s.detected_aura.clone(), s.detected_ms))
        else {
            return;
        };
        let by = Self::id_of(sensor);
        self.apply_aura(pi, &aura, ms, None, by, None);
        self.sight.incidents.push((Self::id_of(pi), by));
        self.events.push(Event::Incident {
            player: Self::id_of(pi),
            by,
        });
    }

    /// A honeypot prop was used: every cone turns to it for `HONEYPOT_MS`. False while the shared
    /// cooldown (`cooldown_ms`) runs or nothing has eyes to fool.
    pub(crate) fn honeypot_use(&mut self, at: Vec3, cooldown_ms: u32) -> bool {
        let sensors = self
            .units
            .iter()
            .any(|u| u.state.alive && u.state.hostile && self.content.units[u.def].sight.is_some());
        if !sensors || self.now < self.sight.honeypot_ready {
            return false;
        }
        self.sight.honeypot = Some((self.now + crate::content::ticks(HONEYPOT_MS), at));
        self.sight.honeypot_ready = self.now + crate::content::ticks(cooldown_ms);
        true
    }

    /// The honeypot's spot while one is lit.
    pub fn honeypot_active(&self) -> Option<Vec3> {
        self.sight
            .honeypot
            .filter(|(until, _)| *until > self.now)
            .map(|(_, at)| at)
    }

    /// Idle sensors with a patrol walk it; a sensor with a target is left to the AI.
    fn walk_patrols(&mut self, eyes: &[usize]) {
        let t_ms = ticks_to_ms(self.now);
        let lure = self.honeypot_active();
        for &i in eyes {
            let u = &self.units[i];
            if !matches!(u.state.ai, None | Some(AiState::Idle)) || !u.threat.is_empty() {
                continue;
            }
            // Every cone is on the honeypot; the clock's patrol picks up again where it is.
            if let Some(at) = lure {
                let from = u.state.pos;
                self.units[i].state.yaw = crate::ai::yaw_toward(from, at);
                continue;
            }
            let Some(def) = self.sight_of(i).filter(|s| !s.patrol.is_empty()) else {
                continue;
            };
            let home = u.spawn_pos;
            let patrol = Patrol {
                points: def
                    .patrol
                    .iter()
                    .map(|(x, z)| Vec3::new(home.x + x, home.y, home.z + z))
                    .collect(),
                pauses: vec![def.pause_ms; def.patrol.len()],
                speed_mps: def.patrol_mps,
            };
            let pose = patrol.pose(t_ms);
            let u = &mut self.units[i];
            u.state.pos = pose.pos;
            u.state.yaw = pose.yaw;
            u.moving = pose.moving;
        }
    }

    /// `Effect::SummonSquad`: one unit per role among the living humans, `filler` units up to
    /// `min_bodies`, on the caster's right (Red) or left (Blue).
    pub(crate) fn summon_squad(
        &mut self,
        ci: usize,
        per_role: &SquadRoles,
        filler: &str,
        side: SquadSide,
        min_bodies: u8,
        idle: bool,
    ) {
        let roles: Vec<Role> = self
            .players
            .iter()
            .filter_map(|p| {
                let i = self.index(*p).filter(|&i| self.units[i].state.alive)?;
                self.class_at(i).map(|c| c.role)
            })
            .collect();
        let present: Vec<Role> = [Role::Tank, Role::Healer, Role::Melee, Role::Ranged]
            .into_iter()
            .filter(|r| roles.contains(r))
            .collect();
        let mut keys: Vec<String> = present
            .iter()
            .map(|&r| per_role.key(r).to_string())
            .collect();
        while keys.len() < squad_size(&roles, min_bodies) {
            keys.push(filler.to_string());
        }
        let (pos, yaw) = (self.units[ci].state.pos, self.units[ci].state.yaw);
        let right = crate::telegraph::right(yaw);
        let off = if side == SquadSide::Red { 6.0 } else { -6.0 };
        let at = Vec3::new(pos.x + right.x * off, pos.y, pos.z + right.z * off);
        for (k, key) in keys.iter().enumerate() {
            let before = self.units.len();
            self.summon_with(ci, at, key, (1, u8::MAX, k as u8), idle);
            for i in before..self.units.len() {
                self.sight.squads.push((Self::id_of(i), side));
                // A unit built for a role wears the name of the first living player in it (a
                // filler body copies nobody); the client glitches the plate like Wing 2's.
                let owner = present.get(k).and_then(|&role| {
                    self.players.iter().copied().find(|&p| {
                        self.index(p).is_some_and(|pi| {
                            self.units[pi].state.alive
                                && self.class_at(pi).is_some_and(|c| c.role == role)
                        })
                    })
                });
                if let Some(player) = owner {
                    let unit = Self::id_of(i);
                    self.sight.owners.push((unit, player));
                    self.events.push(Event::Mirrored {
                        mirror: unit,
                        player,
                    });
                }
            }
        }
    }

    /// Players with a living copy among the squads (the `mirrored` damage cut lasts while one stands).
    pub(crate) fn squad_copied_players(&self) -> Vec<(EntityId, bool)> {
        let mut out: Vec<(EntityId, bool)> = Vec::new();
        for &(unit, player) in &self.sight.owners {
            let alive = self.unit(unit).is_some_and(|u| u.alive);
            match out.iter_mut().find(|(p, _)| *p == player) {
                Some((_, a)) => *a |= alive,
                None => out.push((player, alive)),
            }
        }
        out
    }
}

impl SquadRoles {
    pub(crate) fn validate(
        &self,
        c: &Content,
        filler: &str,
        spell: &str,
    ) -> Result<(), ContentError> {
        if self
            .keys()
            .into_iter()
            .chain([filler])
            .any(|k| c.unit_def(k).is_none())
        {
            return Err(ContentError::Invalid(format!(
                "spell {spell} summons a squad of unknown units"
            )));
        }
        Ok(())
    }
}
