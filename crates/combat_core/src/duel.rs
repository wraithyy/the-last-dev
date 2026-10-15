//! Code Review Duel: opt-in 1v1 between two group members. Asked, accepted, a short
//! countdown inside an arena circle, then the pair (and only the pair) can hurt each other at
//! reduced damage until one drops to 1 HP. Leaving the circle forfeits; nobody dies.

use crate::content::ticks;
use crate::geom::Vec3;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{AuraEnd, Event};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Players must stand this close to ask.
pub const DUEL_REQUEST_RANGE_M: f32 = 20.0;
/// Arena radius around the midpoint of the two players at acceptance.
pub const DUEL_RADIUS_M: f32 = 20.0;
/// Unanswered requests lapse after this.
pub const DUEL_ASK_MS: u32 = 30_000;
pub const DUEL_COUNTDOWN_MS: u32 = 3_000;
/// Minimum gap between two requests from the same player.
pub const DUEL_REQUEST_GAP_MS: u32 = 5_000;
/// Damage between duelists, in percent of the PvE number.
pub const PVP_DAMAGE_PCT: u32 = 50;
/// Crowd control on a duelist lasts 100%, 50%, 25%, then not at all, until this long passes
/// without a new one.
pub const DR_RESET_MS: u32 = 18_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuelOutcome {
    /// The winner brought the other to 1 HP.
    Won,
    /// The loser left the arena circle or the room.
    Forfeit,
    Declined,
    TimedOut,
    /// Called off: someone was pulled into a real fight, died, or a sprint started.
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuelError {
    /// Not another living member of the group.
    NoSuchPlayer,
    TooFar,
    /// One of you is fighting something real.
    InCombat,
    InSprint,
    /// One of you already has a duel or a request open.
    Busy,
    TooSoon,
}

impl DuelError {
    pub fn message(self) -> &'static str {
        match self {
            DuelError::NoSuchPlayer => "Code reviews need a reviewer who is actually here.",
            DuelError::TooFar => "Walk over to their desk first. Remote reviews are for PRs.",
            DuelError::InCombat => "Finish the incident first. Then argue about tabs vs spaces.",
            DuelError::InSprint => "No code reviews during a sprint. Ship first, review never.",
            DuelError::Busy => "They already have a review in progress.",
            DuelError::TooSoon => {
                "You just asked. Give them a minute to pretend they didn't see it."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Asked { until: u64 },
    Countdown { until: u64 },
    Fight,
}

#[derive(Clone, Debug)]
pub(crate) struct Duel {
    a: EntityId,
    b: EntityId,
    phase: Phase,
    center: Vec3,
    comments: u32,
    /// Per side (`a`, `b`): crowd control applications so far and when that count resets.
    dr: [(u8, u64); 2],
}

impl Duel {
    fn has(&self, id: EntityId) -> bool {
        self.a == id || self.b == id
    }
    fn other(&self, id: EntityId) -> EntityId {
        if self.a == id { self.b } else { self.a }
    }
}

#[derive(Default)]
pub(crate) struct DuelState {
    list: Vec<Duel>,
    /// Earliest tick of each player's next request.
    pub(crate) next_request: BTreeMap<EntityId, u64>,
}

/// Damage between duelists.
pub(crate) fn pvp_amount(amount: u32) -> u32 {
    (u64::from(amount) * u64::from(PVP_DAMAGE_PCT) / 100) as u32
}

impl Sim {
    fn duel_of(&self, id: EntityId) -> Option<usize> {
        self.duels.list.iter().position(|d| d.has(id))
    }

    /// Fighting the unit at `j` right now (not merely asked or counting down).
    pub(crate) fn duel_foes(&self, i: usize, j: usize) -> bool {
        let (a, b) = (Self::id_of(i), Self::id_of(j));
        a != b
            && self
                .duels
                .list
                .iter()
                .any(|d| d.phase == Phase::Fight && d.has(a) && d.has(b))
    }

    /// Different sides, or duel opponents.
    pub(crate) fn hostile_to(&self, i: usize, j: usize) -> bool {
        self.units[i].state.hostile != self.units[j].state.hostile || self.duel_foes(i, j)
    }

    /// Heals and buffs cannot cross into or out of an accepted duel.
    pub(crate) fn duel_blocks_help(&self, i: usize, j: usize) -> bool {
        let engaged = |k: usize| {
            self.duels
                .list
                .iter()
                .any(|d| d.has(Self::id_of(k)) && !matches!(d.phase, Phase::Asked { .. }))
        };
        i != j && (engaged(i) || engaged(j))
    }

    /// A living group member that no enemy has on its threat table.
    fn duel_ready(&self, id: EntityId) -> Result<usize, crate::duel::DuelError> {
        let i = self
            .index(id)
            .filter(|&i| self.is_player(id) && self.units[i].state.alive)
            .ok_or(DuelError::NoSuchPlayer)?;
        let pulled = self
            .units
            .iter()
            .any(|o| o.state.alive && o.threat.contains_key(&id));
        if pulled || self.now < self.units[i].combat_lock_until {
            return Err(DuelError::InCombat);
        }
        Ok(i)
    }

    pub(crate) fn duel_request(&mut self, from: EntityId, to: EntityId) {
        if let Err(reason) = self.try_duel_request(from, to) {
            self.emit_to(from, Event::DuelRefused { reason });
        }
    }

    fn try_duel_request(&mut self, from: EntityId, to: EntityId) -> Result<(), DuelError> {
        if self
            .duels
            .next_request
            .get(&from)
            .is_some_and(|&t| self.now < t)
        {
            return Err(DuelError::TooSoon);
        }
        if from == to {
            return Err(DuelError::NoSuchPlayer);
        }
        let fi = self.duel_ready(from)?;
        let ti = self.duel_ready(to)?;
        if self.sprint_active() {
            return Err(DuelError::InSprint);
        }
        if self.duel_of(from).is_some() || self.duel_of(to).is_some() {
            return Err(DuelError::Busy);
        }
        let (pf, pt) = (self.units[fi].state.pos, self.units[ti].state.pos);
        if !crate::geom::in_range(pf, pt, DUEL_REQUEST_RANGE_M) {
            return Err(DuelError::TooFar);
        }
        self.duels
            .next_request
            .insert(from, self.now + ticks(DUEL_REQUEST_GAP_MS));
        self.duels.list.push(Duel {
            a: from,
            b: to,
            phase: Phase::Asked {
                until: self.now + ticks(DUEL_ASK_MS),
            },
            center: Vec3::default(),
            comments: 0,
            dr: [(0, 0); 2],
        });
        for p in [from, to] {
            self.emit_to(p, Event::DuelRequested { from, to });
        }
        Ok(())
    }

    /// Only the asked player answers; anything else is ignored.
    pub(crate) fn duel_respond(&mut self, actor: EntityId, accept: bool) {
        let Some(k) = self
            .duels
            .list
            .iter()
            .position(|d| d.b == actor && matches!(d.phase, Phase::Asked { .. }))
        else {
            return;
        };
        if !accept {
            return self.end_duel(k, None, crate::duel::DuelOutcome::Declined);
        }
        let (a, b) = (self.duels.list[k].a, self.duels.list[k].b);
        // Conditions may have changed while the prompt was open.
        let ready = self.duel_ready(a).and_then(|_| self.duel_ready(b));
        let (Ok(bi), Some(ai), false) = (ready, self.index(a), self.sprint_active()) else {
            return self.end_duel(k, None, DuelOutcome::Cancelled);
        };
        let (pa, pb) = (self.units[ai].state.pos, self.units[bi].state.pos);
        let center = Vec3::new(
            (pa.x + pb.x) / 2.0,
            (pa.y + pb.y) / 2.0,
            (pa.z + pb.z) / 2.0,
        );
        let d = &mut self.duels.list[k];
        d.center = center;
        d.phase = Phase::Countdown {
            until: self.now + ticks(DUEL_COUNTDOWN_MS),
        };
        self.events.push(Event::DuelCountdown {
            a,
            b,
            center,
            radius_m: DUEL_RADIUS_M,
            ms: DUEL_COUNTDOWN_MS,
        });
    }

    /// Request timeouts, countdowns, the arena boundary and real fights interrupting.
    pub(crate) fn update_duels(&mut self) {
        let mut k = 0;
        while k < self.duels.list.len() {
            let d = self.duels.list[k].clone();
            let ended = match d.phase {
                Phase::Asked { until } => {
                    (self.now >= until).then_some((None, DuelOutcome::TimedOut))
                }
                Phase::Countdown { .. } | Phase::Fight => self.duel_check(&d),
            };
            if let Some((winner, outcome)) = ended {
                self.end_duel(k, winner, outcome);
                continue;
            }
            match d.phase {
                Phase::Countdown { until } if self.now >= until => {
                    self.duels.list[k].phase = Phase::Fight;
                    for (me, foe) in [(d.a, d.b), (d.b, d.a)] {
                        if let Some(i) = self.index(me) {
                            self.units[i].state.duel_foe = Some(foe);
                        }
                    }
                    self.events.push(Event::DuelBegan { a: d.a, b: d.b });
                }
                Phase::Fight => {
                    // Fighting counts as combat: no out-of-combat regen or coffee mid-duel.
                    for p in [d.a, d.b] {
                        if let Some(i) = self.index(p) {
                            self.units[i].combat_lock_until = self.now + 2;
                        }
                    }
                }
                _ => {}
            }
            k += 1;
        }
    }

    /// An accepted duel that must end now: (winner, outcome).
    fn duel_check(&self, d: &Duel) -> Option<(Option<EntityId>, DuelOutcome)> {
        if self.sprint_active() {
            return Some((None, DuelOutcome::Cancelled));
        }
        for p in [d.a, d.b] {
            let Some(i) = self.index(p).filter(|_| self.is_player(p)) else {
                return Some((Some(d.other(p)), DuelOutcome::Forfeit));
            };
            let pulled = self
                .units
                .iter()
                .any(|o| o.state.alive && o.threat.contains_key(&p));
            if pulled || !self.units[i].state.alive {
                return Some((None, DuelOutcome::Cancelled));
            }
            if crate::effects::dist_xz(self.units[i].state.pos, d.center) > DUEL_RADIUS_M {
                return Some((Some(d.other(p)), DuelOutcome::Forfeit));
            }
        }
        None
    }

    fn end_duel(&mut self, k: usize, winner: Option<EntityId>, outcome: DuelOutcome) {
        let d = self.duels.list.remove(k);
        let mut streak = 0;
        for p in [d.a, d.b] {
            let Some(i) = self.index(p) else { continue };
            let foe = d.other(p);
            let u = &mut self.units[i].state;
            u.duel_foe = None;
            if winner == Some(p) {
                u.duel_streak += 1;
                streak = u.duel_streak;
            } else if winner.is_some() {
                u.duel_streak = 0;
            }
            if u.target == Some(foe) {
                self.set_auto_attack(i, false);
            }
            // The opponent's debuffs go with the duel.
            let harmful: Vec<String> = self.units[i]
                .auras
                .iter()
                .filter(|a| a.source == foe)
                .filter(|a| self.content.aura(&a.key).is_some_and(|x| x.harmful))
                .map(|a| a.key.clone())
                .collect();
            for key in harmful {
                self.remove_aura(i, &key, AuraEnd::Expired);
            }
        }
        self.events.push(Event::DuelEnded {
            a: d.a,
            b: d.b,
            winner,
            outcome,
            comments: d.comments,
            streak,
        });
    }

    /// One more landed hit for the duel `ti` is in.
    pub(crate) fn duel_hit(&mut self, ti: usize) {
        if let Some(k) = self.duel_of(Self::id_of(ti)) {
            self.duels.list[k].comments += 1;
        }
    }

    /// `winner` brought `loser` to 1 HP.
    pub(crate) fn duel_won(&mut self, winner: EntityId, loser: EntityId) {
        if let Some(k) = self.duel_of(loser) {
            self.end_duel(k, Some(winner), DuelOutcome::Won);
        }
    }

    /// The unit died (to something else): its duel or request is off.
    pub(crate) fn duel_cancel_for(&mut self, id: EntityId) {
        if let Some(k) = self.duel_of(id) {
            self.end_duel(k, None, DuelOutcome::Cancelled);
        }
    }

    /// The player left the room: an accepted duel is forfeit, a request just ends.
    pub(crate) fn duel_left(&mut self, id: EntityId) {
        let Some(k) = self.duel_of(id) else { return };
        let d = &self.duels.list[k];
        if matches!(d.phase, Phase::Asked { .. }) {
            self.end_duel(k, None, DuelOutcome::Cancelled);
        } else {
            let other = d.other(id);
            self.end_duel(k, Some(other), DuelOutcome::Forfeit);
        }
    }

    /// Diminishing returns on crowd control landing on duelist `ti`: the duration to apply,
    /// 0 when immune.
    pub(crate) fn duel_cc_ms(&mut self, ti: usize, ms: u32) -> u32 {
        let id = Self::id_of(ti);
        let now = self.now;
        let Some(d) = self.duels.list.iter_mut().find(|d| d.has(id)) else {
            return ms;
        };
        let side = usize::from(d.b == id);
        let (count, reset) = &mut d.dr[side];
        if now >= *reset {
            *count = 0;
        }
        let out = match *count {
            0 => ms,
            1 => ms / 2,
            2 => ms / 4,
            _ => 0,
        };
        *count = count.saturating_add(1);
        *reset = now + ticks(DR_RESET_MS);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pvp_damage_is_halved() {
        assert_eq!(pvp_amount(101), 50);
        assert_eq!(pvp_amount(0), 0);
    }

    #[test]
    fn crowd_control_on_a_duelist_diminishes_then_resets() {
        let Ok(c) = crate::Content::from_ron(include_str!("../../../content/game.ron")) else {
            panic!("content");
        };
        let mut sim = Sim::new(c, 3);
        let a = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
        let b = sim.spawn_player("backend", Vec3::new(0.0, 0.0, -6.0), 0.0);
        sim.apply_as(a, crate::Command::DuelRequest { target: b });
        sim.tick();
        sim.apply_as(b, crate::Command::DuelRespond { accept: true });
        for _ in 0..100 {
            sim.tick();
        }
        let bi = sim.index(b).unwrap_or(0);
        assert!(sim.duel_foes(0, bi));
        let got: Vec<u32> = (0..5).map(|_| sim.duel_cc_ms(bi, 4000)).collect();
        assert_eq!(got, vec![4000, 2000, 1000, 0, 0]);
        // The attacker's own counter is separate.
        assert_eq!(sim.duel_cc_ms(0, 4000), 4000);
        for _ in 0..ticks(DR_RESET_MS) {
            sim.tick();
        }
        assert_eq!(sim.duel_cc_ms(bi, 4000), 4000);
    }
}
