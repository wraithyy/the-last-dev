//! Idle wandering: out-of-combat NPCs stroll near their spawn so the world looks alive.
//!
//! Every unit owns a private SplitMix64 stream (sim seed ^ unit id), so wandering never
//! consumes the combat rng: hit/miss/crit/loot rolls are identical with or without it, only
//! positions differ.

use crate::ai::{step_toward, yaw_toward};
use crate::content::{IdlePose, ticks};
use crate::effects::dist_xz;
use crate::geom::{Aabb, Vec3};
use crate::rng::Rng;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::time::TICK_HZ;
use crate::types::AiState;
use std::f32::consts::{PI, TAU};

const WALK_MPS: f32 = 1.4;
const BOSS_WALK_MPS: f32 = 1.0;
/// Hostile idle radius around the spawn point; bosses only pace.
const HOSTILE_RADIUS_M: f32 = 4.0;
const BOSS_RADIUS_M: f32 = 2.0;
/// Friendly strolls (coffee machine and back).
const FRIENDLY_RADIUS_M: f32 = 6.0;
const MIN_LEG_M: f32 = 1.5;
/// Keeps social-aggro distances sane: pack members never drift farther than this from the anchor.
const PACK_LEASH_M: f32 = 4.0;
/// Wanderers keep this distance to walls and props.
const CLEARANCE_M: f32 = 1.0;
const TURN_RATE_RAD_S: f32 = 6.0;
/// Walking starts once the unit roughly faces its goal, so it never moonwalks sideways.
const FACING_TOLERANCE_RAD: f32 = 0.6;
const HOSTILE_PAUSE_MS: (u32, u32) = (3000, 8000);
const FRIENDLY_PAUSE_MS: (u32, u32) = (6000, 15000);
const PICK_ATTEMPTS: u32 = 6;

pub(crate) struct Wander {
    rng: Rng,
    /// False while the unit is busy (combat, evade, dead, stunned); re-arms the pause on return.
    active: bool,
    next_at: u64,
    goal: Option<Vec3>,
    /// Friendly patrol alternates between an outbound leg and the way home.
    away: bool,
}

impl Wander {
    pub(crate) fn new(seed: u64, id: EntityId) -> Self {
        Self {
            rng: Rng::new(seed ^ id.wrapping_mul(0x9E37_79B9_7F4A_7C15)),
            active: false,
            next_at: 0,
            goal: None,
            away: false,
        }
    }
}

/// Blockers grown by the clearance, so a straight segment test also keeps the margin.
pub(crate) fn padded(blockers: &[Aabb]) -> Vec<Aabb> {
    blockers
        .iter()
        .map(|b| Aabb {
            min: Vec3::new(b.min.x - CLEARANCE_M, b.min.y, b.min.z - CLEARANCE_M),
            max: Vec3::new(b.max.x + CLEARANCE_M, b.max.y, b.max.z + CLEARANCE_M),
        })
        .collect()
}

fn wrap_pi(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

impl Sim {
    pub(crate) fn update_wander(&mut self) {
        if !self.wandering {
            return;
        }
        for i in 0..self.units.len() {
            let Some((radius, speed, pause)) = self.wander_profile(i) else {
                let w = &mut self.units[i].wander;
                w.active = false;
                w.goal = None;
                w.away = false;
                continue;
            };
            if !self.units[i].wander.active {
                let ms = self.units[i].wander.rng.range_inclusive(pause.0, pause.1);
                let w = &mut self.units[i].wander;
                w.active = true;
                w.next_at = self.now + ticks(ms);
            }
            match self.units[i].wander.goal {
                Some(goal) => self.walk_to(i, goal, speed, pause),
                None if self.now >= self.units[i].wander.next_at => {
                    self.pick_goal(i, radius, pause);
                }
                None => {}
            }
        }
    }

    /// `(radius, speed, pause range)` for units allowed to wander right now.
    fn wander_profile(&self, i: usize) -> Option<(f32, f32, (u32, u32))> {
        let u = &self.units[i];
        if u.despawned
            || !u.state.alive
            || u.summoner.is_some()
            || self.is_player(u.state.id)
            || self.companion_owner(u.state.id).is_some()
            || self.is_watcher(i)
            || self.escorted(i)
            || self.has_aura_flag(i, |a| a.stun)
        {
            return None;
        }
        if u.state.hostile {
            let idle = u.state.ai == Some(AiState::Idle) && u.threat.is_empty();
            if !idle {
                return None;
            }
            return Some(if u.state.boss {
                (BOSS_RADIUS_M, BOSS_WALK_MPS, HOSTILE_PAUSE_MS)
            } else {
                (HOSTILE_RADIUS_M, WALK_MPS, HOSTILE_PAUSE_MS)
            });
        }
        let seated = matches!(
            u.state.idle_pose,
            Some(IdlePose::Sleeping | IdlePose::Sitting)
        );
        (!seated && u.state.ai.is_none()).then_some((
            FRIENDLY_RADIUS_M,
            WALK_MPS,
            FRIENDLY_PAUSE_MS,
        ))
    }

    fn walk_to(&mut self, i: usize, goal: Vec3, speed: f32, pause: (u32, u32)) {
        let pos = self.units[i].state.pos;
        let Some((target, _)) = self.nav_target(i, goal) else {
            // The route vanished (blockers changed): give up this stroll and pause.
            let ms = self.units[i].wander.rng.range_inclusive(pause.0, pause.1);
            self.units[i].nav.reset();
            let w = &mut self.units[i].wander;
            w.goal = None;
            w.next_at = self.now + ticks(ms);
            return;
        };
        let heading = yaw_toward(pos, target);
        let turn = wrap_pi(heading - self.units[i].state.yaw);
        let max_turn = TURN_RATE_RAD_S / TICK_HZ as f32;
        self.units[i].state.yaw =
            wrap_pi(self.units[i].state.yaw + turn.clamp(-max_turn, max_turn));
        if turn.abs() > FACING_TOLERANCE_RAD {
            return;
        }
        let step = speed / TICK_HZ as f32;
        if dist_xz(pos, goal) <= step {
            let ms = self.units[i].wander.rng.range_inclusive(pause.0, pause.1);
            let w = &mut self.units[i].wander;
            w.goal = None;
            w.next_at = self.now + ticks(ms);
            self.units[i].state.pos = Vec3::new(goal.x, pos.y, goal.z);
        } else {
            self.units[i].state.pos = step_toward(pos, target, step, 0.0);
        }
    }

    fn pick_goal(&mut self, i: usize, radius: f32, pause: (u32, u32)) {
        let home = self.units[i].spawn_pos;
        let friendly = !self.units[i].state.hostile;
        if friendly && self.units[i].wander.away {
            let w = &mut self.units[i].wander;
            w.goal = Some(home);
            w.away = false;
            return;
        }
        let anchor = self.pack_anchor(i);
        let pos = self.units[i].state.pos;
        for _ in 0..PICK_ATTEMPTS {
            let rng = &mut self.units[i].wander.rng;
            let angle = rng.below(3600) as f32 / 3600.0 * TAU;
            let min_leg = MIN_LEG_M.min(radius * 0.5);
            let d = min_leg + rng.below(1001) as f32 / 1000.0 * (radius - min_leg);
            let cand = Vec3::new(home.x + angle.cos() * d, home.y, home.z + angle.sin() * d);
            let near_anchor = anchor.is_none_or(|a| dist_xz(cand, a) <= PACK_LEASH_M);
            if near_anchor && self.los_padded.clear(pos, cand) {
                let w = &mut self.units[i].wander;
                w.goal = Some(cand);
                w.away = friendly;
                return;
            }
        }
        // No clear spot (tight corner): wait a full pause before trying again.
        let ms = self.units[i].wander.rng.range_inclusive(pause.0, pause.1);
        self.units[i].wander.next_at = self.now + ticks(ms);
    }

    /// Slot 0 of a pack is the anchor itself; it is the first unit spawned with that pack id.
    fn pack_anchor(&self, i: usize) -> Option<Vec3> {
        let pack = self.units[i].state.pack?;
        self.units
            .iter()
            .find(|u| u.state.pack == Some(pack))
            .map(|u| u.spawn_pos)
    }
}
