//! Burnout: a per-player meter (per mille) that office life fills and small mercies drain.
//! It grows with meetings (meeting debuffs), Sprint runs, deaths, Crunch-mode nights and long
//! sessions; coffee, holidays (time logged off), Friday afternoons, the rubber duck, office
//! props (each with its own cooldown) and defeating management bring it down. High burnout
//! slows the player, makes them yawn, mutter and reply-all (the sim picks the quips so co-op
//! peers see the same ones) and thins hair faster. Content: `endgame.burnout` in `game.ron`;
//! numbers in docs/FORMULAS.md.

use crate::content::{Content, ContentError, ticks};
use crate::live::{DAY_S, day_index};
use crate::rng::Rng;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::Event;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const BURNOUT_MAX: u16 = 1000;
const MINUTE_MS: u32 = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BurnoutCause {
    Meeting,
    Minigame,
    Death,
    Crunch,
    LongSession,
    CoffeeBreak,
    Holiday,
    Friday,
    Duck,
    /// An office prop (its `Interacted` event comes right before).
    Prop,
    /// Defeated management.
    Kill,
    Debug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuipKind {
    Yawn,
    Mutter,
    ReplyAll,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct BurnoutDef {
    /// Growth, per mille: each meeting debuff landing on you, each Sprint run, each death.
    pub meeting: u16,
    pub meeting_auras: Vec<String>,
    pub minigame: u16,
    pub death: u16,
    /// Per minute at night (Crunch mode) and per minute of a session past the grace time.
    pub crunch_per_min: u16,
    pub session_grace_min: u32,
    pub session_per_min: u16,
    /// Relief, per mille: a Coffee Break (once per cooldown), per minute on a Friday afternoon
    /// (UTC, from 12:00) or with the rubber duck out, per hour logged off past the grace time.
    pub coffee_break: u16,
    pub coffee_break_cd_ms: u32,
    pub friday_per_min: u16,
    pub duck_per_min: u16,
    pub holiday_per_hour: u16,
    pub holiday_grace_min: u32,
    /// Defeating one of `management`; bosses among them take `boss_kill`.
    pub kill: u16,
    pub boss_kill: u16,
    pub management: Vec<String>,
    /// From this much on the player moves `slow_pct` percent slower.
    pub slow_at: u16,
    pub slow_pct: u32,
    /// Quip thresholds; one roll per `quip_every_ms` at `quip_pct` percent.
    pub yawn_at: u16,
    pub mutter_at: u16,
    pub reply_all_at: u16,
    pub quip_every_ms: u32,
    pub quip_pct: u32,
    /// At full burnout hair thins as if HP were this many percent lower.
    pub hair_shift_pct: u32,
    pub yawns: Vec<String>,
    pub mutters: Vec<String>,
    pub reply_all: Vec<String>,
    /// Tooltip line from a burnout level on: (per mille, text), ascending.
    pub tips: Vec<(u16, String)>,
    /// Combat log lines after a management kill ("Therapeutic.").
    pub therapy: Vec<String>,
}

impl BurnoutDef {
    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        if let Some(a) = self.meeting_auras.iter().find(|a| c.aura(a).is_none()) {
            return bad(format!("burnout meeting aura {a} unknown"));
        }
        if let Some(u) = self.management.iter().find(|u| c.unit_def(u).is_none()) {
            return bad(format!("burnout management unit {u} unknown"));
        }
        let lines = [&self.yawns, &self.mutters, &self.reply_all, &self.therapy];
        if lines.iter().any(|l| l.len() > usize::from(u16::MAX)) {
            return bad("too many burnout lines".into());
        }
        if !self.tips.windows(2).all(|w| w[0].0 < w[1].0) || self.slow_pct > 50 {
            return bad("burnout tips ascend; slow_pct <= 50".into());
        }
        Ok(())
    }

    /// Movement speed in percent for this burnout (100 below `slow_at`).
    pub fn speed_pct(&self, burnout: u16) -> u32 {
        if self.slow_at > 0 && burnout >= self.slow_at {
            100 - self.slow_pct
        } else {
            100
        }
    }

    /// HP percent points hair loss runs ahead by at this burnout.
    pub fn hair_shift(&self, burnout: u16) -> f32 {
        self.hair_shift_pct as f32 * f32::from(burnout.min(BURNOUT_MAX)) / f32::from(BURNOUT_MAX)
    }

    /// The tooltip line for this burnout.
    pub fn tip(&self, burnout: u16) -> &str {
        self.tips
            .iter()
            .rev()
            .find(|(at, _)| burnout >= *at)
            .map_or("", |(_, t)| t.as_str())
    }

    pub fn quip_line(&self, kind: QuipKind, line: u16) -> Option<&str> {
        let list = match kind {
            QuipKind::Yawn => &self.yawns,
            QuipKind::Mutter => &self.mutters,
            QuipKind::ReplyAll => &self.reply_all,
        };
        list.get(usize::from(line)).map(String::as_str)
    }

    /// Relief for `away_s` seconds logged off: nothing within the grace time, then
    /// `holiday_per_hour` per full hour.
    pub fn holiday_relief(&self, away_s: u64) -> u16 {
        if away_s < u64::from(self.holiday_grace_min) * 60 {
            return 0;
        }
        (away_s / 3600)
            .saturating_mul(u64::from(self.holiday_per_hour))
            .min(u64::from(BURNOUT_MAX)) as u16
    }
}

/// Monday 0 .. Sunday 6, UTC (1970-01-01 was a Thursday).
pub fn weekday(unix_s: u64) -> u32 {
    (day_index(unix_s) + 3) % 7
}

/// Friday from 12:00 UTC: the office is legally allowed to stop pretending.
pub fn is_friday_afternoon(unix_s: u64) -> bool {
    weekday(unix_s) == 4 && unix_s % DAY_S >= 12 * 3600
}

#[derive(Default)]
struct PlayerBurnout {
    joined: u64,
    coffee_ready: u64,
    /// Prop index -> tick its relief is available again.
    calm_ready: BTreeMap<usize, u64>,
}

pub(crate) struct BurnoutRt {
    rng: Rng,
    players: BTreeMap<EntityId, PlayerBurnout>,
}

impl BurnoutRt {
    /// Own stream, so quips never shift combat or loot rolls.
    pub(crate) fn new(seed: u64) -> Self {
        BurnoutRt {
            rng: Rng::new(seed ^ 0xB0B5_1DE5_0FF1_CE00),
            players: BTreeMap::new(),
        }
    }
}

impl Sim {
    fn burnout_def(&self) -> &BurnoutDef {
        &self.content.endgame.burnout
    }

    pub(crate) fn burnout_on_join(&mut self, player: EntityId) {
        let now = self.now;
        self.burnout.players.insert(
            player,
            PlayerBurnout {
                joined: now,
                ..PlayerBurnout::default()
            },
        );
    }

    pub(crate) fn burnout_on_leave(&mut self, player: EntityId) {
        self.burnout.players.remove(&player);
    }

    /// Moves player `i`'s burnout by `delta` per mille, clamped; emits only on a change.
    pub(crate) fn add_burnout(&mut self, i: usize, delta: i32, cause: BurnoutCause) {
        let id = Self::id_of(i);
        if !self.is_player(id) || delta == 0 {
            return;
        }
        let c = &mut self.units[i].state.career;
        let value = (i32::from(c.burnout) + delta).clamp(0, i32::from(BURNOUT_MAX)) as u16;
        let moved = i32::from(value) - i32::from(c.burnout);
        if moved == 0 {
            return;
        }
        c.burnout = value;
        self.events.push(Event::Burnout {
            unit: id,
            value,
            delta: moved as i16,
            cause,
        });
    }

    /// Debug/QA: sets a player's burnout (event cause `Debug`).
    pub fn debug_set_burnout(&mut self, player: EntityId, value: u16) {
        if let Some(i) = self.index(player) {
            let now = i32::from(self.units[i].state.career.burnout);
            self.add_burnout(
                i,
                i32::from(value.min(BURNOUT_MAX)) - now,
                BurnoutCause::Debug,
            );
        }
    }

    pub(crate) fn burnout_on_death(&mut self, i: usize) {
        let d = i32::from(self.burnout_def().death);
        self.add_burnout(i, d, BurnoutCause::Death);
    }

    pub(crate) fn burnout_on_aura(&mut self, i: usize, key: &str) {
        if self.burnout_def().meeting_auras.iter().any(|a| a == key) {
            let d = i32::from(self.burnout_def().meeting);
            self.add_burnout(i, d, BurnoutCause::Meeting);
        }
    }

    pub(crate) fn burnout_on_sprint_end(&mut self) {
        let d = i32::from(self.burnout_def().minigame);
        for p in self.players.clone() {
            if let Some(i) = self.index(p) {
                self.add_burnout(i, d, BurnoutCause::Minigame);
            }
        }
    }

    pub(crate) fn burnout_on_kill(&mut self, i: usize, key: &str, boss: bool) {
        let def = self.burnout_def();
        if !def.management.iter().any(|m| m == key) {
            return;
        }
        let d = if boss { def.boss_kill } else { def.kill };
        self.add_burnout(i, -i32::from(d), BurnoutCause::Kill);
    }

    /// Prop `k` was used by player `i`: its relief, unless that prop's cooldown still runs.
    pub(crate) fn burnout_on_prop(&mut self, i: usize, k: usize) {
        let Some((amount, cd_ms)) = self.content.world.props[k].calm else {
            return;
        };
        let now = self.now;
        let Some(pb) = self.burnout.players.get_mut(&Self::id_of(i)) else {
            return;
        };
        if pb.calm_ready.get(&k).is_some_and(|t| *t > now) {
            return;
        }
        pb.calm_ready.insert(k, now + ticks(cd_ms));
        self.add_burnout(i, -i32::from(amount), BurnoutCause::Prop);
    }

    /// Ticks left until prop `key`'s relief is ready for `player` (0 = ready or none).
    pub fn calm_ready_in_ms(&self, player: EntityId, key: &str) -> u32 {
        let Some(k) = self.content.world.props.iter().position(|p| p.key == key) else {
            return 0;
        };
        self.burnout
            .players
            .get(&player)
            .and_then(|pb| pb.calm_ready.get(&k))
            .map_or(0, |t| self.remaining_ms(*t))
    }

    /// Holiday on load: the save's burnout minus the relief for the time since it was written.
    pub(crate) fn import_burnout(&mut self, i: usize, burnout: u16, saved_at: u64) {
        self.units[i].state.career.burnout = burnout.min(BURNOUT_MAX);
        if saved_at == 0 || !self.clock_on() {
            return;
        }
        let away = self.unix_s().saturating_sub(saved_at);
        let relief = self.burnout_def().holiday_relief(away);
        self.add_burnout(i, -i32::from(relief), BurnoutCause::Holiday);
    }

    /// Once a second: the Coffee Break; once a minute per player (from when they joined) the
    /// slow drifts; every `quip_every_ms` the quip roll.
    pub(crate) fn update_burnout(&mut self) {
        if !self.now.is_multiple_of(ticks(1000)) {
            return;
        }
        let coffee = self.content.channel_spell().map(|s| s.id);
        let night = self.is_night();
        let friday = self.clock_on() && is_friday_afternoon(self.unix_s());
        let minute = ticks(MINUTE_MS);
        let quip_every = ticks(self.burnout_def().quip_every_ms.max(1000));
        for p in self.players.clone() {
            let Some(i) = self.index(p) else { continue };
            let Some(pb) = self.burnout.players.get(&p) else {
                continue;
            };
            let (joined, coffee_ready) = (pb.joined, pb.coffee_ready);
            let def = &self.content.endgame.burnout;
            let channeling = self.units[i].cast.as_ref().map(|c| c.spell) == coffee;
            if channeling && coffee.is_some() && self.now >= coffee_ready {
                let (d, cd) = (def.coffee_break, def.coffee_break_cd_ms);
                if let Some(pb) = self.burnout.players.get_mut(&p) {
                    pb.coffee_ready = self.now + ticks(cd);
                }
                self.add_burnout(i, -i32::from(d), BurnoutCause::CoffeeBreak);
            }
            let since = self.now.saturating_sub(joined);
            if since > 0 && since.is_multiple_of(minute) {
                let def = &self.content.endgame.burnout;
                let session = since / minute > u64::from(def.session_grace_min);
                let duck = self.units[i].state.pet == crate::pets::RUBBER_DUCK;
                let drifts = [
                    (night, i32::from(def.crunch_per_min), BurnoutCause::Crunch),
                    (
                        session,
                        i32::from(def.session_per_min),
                        BurnoutCause::LongSession,
                    ),
                    (friday, -i32::from(def.friday_per_min), BurnoutCause::Friday),
                    (duck, -i32::from(def.duck_per_min), BurnoutCause::Duck),
                ];
                for (on, d, cause) in drifts {
                    if on {
                        self.add_burnout(i, d, cause);
                    }
                }
            }
            if since > 0 && since.is_multiple_of(quip_every) {
                self.roll_quip(i);
            }
        }
    }

    fn roll_quip(&mut self, i: usize) {
        let s = &self.units[i].state;
        let (b, alive) = (s.career.burnout, s.alive);
        let def = &self.content.endgame.burnout;
        if !alive || def.yawn_at == 0 || b < def.yawn_at {
            return;
        }
        let mut kinds = vec![(QuipKind::Yawn, def.yawns.len())];
        if def.mutter_at > 0 && b >= def.mutter_at {
            kinds.push((QuipKind::Mutter, def.mutters.len()));
        }
        if def.reply_all_at > 0 && b >= def.reply_all_at {
            kinds.push((QuipKind::ReplyAll, def.reply_all.len()));
        }
        kinds.retain(|(_, n)| *n > 0);
        let pct = def.quip_pct;
        if kinds.is_empty() || !self.burnout.rng.chance_pct(pct) {
            return;
        }
        let (kind, n) = kinds[self.burnout.rng.below(kinds.len() as u32) as usize];
        let line = self.burnout.rng.below(n as u32) as u16;
        self.events.push(Event::BurnoutQuip {
            unit: Self::id_of(i),
            kind,
            line,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friday_afternoon_is_utc_friday_from_noon() {
        // 2026-10-16 is a Friday.
        let friday_midnight = 1_792_108_800;
        assert_eq!(weekday(friday_midnight), 4);
        assert!(!is_friday_afternoon(friday_midnight + 11 * 3600));
        assert!(is_friday_afternoon(friday_midnight + 12 * 3600));
        assert!(!is_friday_afternoon(friday_midnight + 36 * 3600));
    }
}
