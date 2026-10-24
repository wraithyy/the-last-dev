//! Ilmari Wing 5, the Alignment Lab's second boss, Red Team Blue Team (docs/ILMARI_1_0.md D.3). The
//! fight is a tabletop exercise: two squads built from the party's roster (`Effect::SummonSquad`)
//! and three Blue sensors with vision cones (`sight.rs`). Nobody shoots until somebody is seen:
//! the boss and the Red squad stand by (`SquadsDef::standby_aura`), and the exercise starts at the
//! first incident, when the last sensor falls, when somebody hits a Red unit, or on schedule. At
//! the merge phase every unit turns "purple" (the units' own abilities read the aura: Red learns
//! to heal, Blue to hit). The Judgment rider starts the exercise live and punishes a sighting
//! with a teleport into Red's middle. Everything is sim state and the combat rng only, so a
//! replay reproduces it exactly. Numbers: docs/FORMULAS.md, Red Team Blue Team.

use crate::content::{EncounterDef, SpellId, YellKind};
use crate::geom::Vec3;
use crate::sight::SquadSide;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{AiState, AuraEnd, Event};
use serde::Deserialize;

/// Standby and the purple aura last "until the fight ends": evade and death clear every aura.
const LONG_MS: u32 = 3_600_000;
/// Both squads must fall this close together in the merge phase for the "Purple" title.
const PURPLE_WINDOW_MS: u64 = 10_000;

/// The exercise of Red Team Blue Team; the encounter of the boss.
#[derive(Clone, Debug, Deserialize)]
pub struct SquadsDef {
    /// On every Red unit until the exercise starts (a stun: they do not act).
    pub standby_aura: String,
    /// On the boss until the exercise starts (a stun and less damage taken).
    pub boss_aura: String,
    /// On the boss and every squad unit from the merge on.
    pub purple_aura: String,
    /// The boss phase (1-based) of the merge.
    pub merge_phase: u8,
    /// Cast by the boss when the exercise arms: the squads and the sensors take their places
    /// (`story_setup` on Story: one Red squad and the cameras).
    pub setup: SpellId,
    pub story_setup: SpellId,
    /// The exercise starts by itself this long after the pull.
    pub start_ms: u32,
    pub story_start_ms: u32,
    /// Mirrored onto both squads when the boss wears it (Budget Approved).
    #[serde(default)]
    pub enrage_aura: Option<String>,
    /// Judgment's rider: live from the start, no honeypots, a sighting teleports.
    #[serde(default)]
    pub rider: Option<String>,
    #[serde(default)]
    pub rider_teleport_ms: u32,
    /// Cast by the boss at a player an incident named (the marked circle).
    #[serde(default)]
    pub incident_spell: Option<SpellId>,
    #[serde(default)]
    pub start_yells: Vec<String>,
    #[serde(default)]
    pub incident_yells: Vec<String>,
    #[serde(default)]
    pub purple_yells: Vec<String>,
}

impl SquadsDef {
    pub(crate) fn validate(&self, c: &crate::content::Content, unit: &str) -> bool {
        let auras = [&self.standby_aura, &self.boss_aura, &self.purple_aura]
            .into_iter()
            .chain(self.enrage_aura.iter())
            .chain(self.rider.iter());
        auras.into_iter().all(|a| c.aura(a).is_some())
            && self.merge_phase >= 2
            && self.start_ms > 0
            && self.story_start_ms > 0
            && self.incident_spell.is_none_or(|s| c.spell(s).is_some())
            && c.spell(self.setup).is_some()
            && c.spell(self.story_setup).is_some()
            && (self.rider.is_none() || self.rider_teleport_ms > 0)
            && c.unit_def(unit).is_some()
    }
}

/// Gap between two incidents that count against the same player.
const INCIDENT_GAP_MS: u32 = 10_000;

struct SquadsRt {
    cfg: SquadsDef,
    boss: EntityId,
    pulled: u64,
    released: bool,
    seen_sensors: bool,
    merged: bool,
    /// Incidents before the merge: "Silent Alarm" wants none.
    incidents_p1: u32,
    last_teleport: Option<u64>,
    /// Per player: the tick of the last incident that counted. A cone that keeps seeing a marked
    /// player refills the meter every few hundred ms; one incident per `INCIDENT_GAP_MS` is the
    /// rule, or the incident circle would stack into a wipe.
    last_incident: Vec<(EntityId, u64)>,
    /// When each squad's last living unit fell in the merge phase.
    red_gone: Option<u64>,
    blue_gone: Option<u64>,
    /// What the clients were last told: (standing by, sensors left, merged).
    sent: (bool, u8, bool),
}

#[derive(Default)]
pub(crate) struct Wing5State {
    rt: Option<SquadsRt>,
}

impl Sim {
    pub(crate) fn wing5_arm(&mut self, bi: usize, cfg: &SquadsDef) {
        let boss = Self::id_of(bi);
        let live = cfg
            .rider
            .as_ref()
            .is_some_and(|r| self.rider_on(bi, r) && !self.raid.story);
        let mut rt = SquadsRt {
            cfg: cfg.clone(),
            boss,
            pulled: self.now,
            released: live,
            seen_sensors: false,
            merged: false,
            incidents_p1: 0,
            last_teleport: None,
            last_incident: Vec::new(),
            red_gone: None,
            blue_gone: None,
            sent: (false, 0, false),
        };
        if !live {
            self.apply_aura(bi, &cfg.boss_aura, LONG_MS, None, boss, None);
        }
        rt.sent = (!live, 0, false);
        self.events.push(Event::Exercise {
            standby: !live,
            sensors: 0,
            merged: false,
        });
        // The squads stand where the setup spell puts them; Red is told to wait on the next tick.
        let setup = if self.raid.story {
            cfg.story_setup
        } else {
            cfg.setup
        };
        if let Some(spell) = self.content.spell(setup).cloned() {
            self.resolve(bi, &spell, Some(boss));
        }
        if !live {
            for i in self.squad_idx(SquadSide::Red) {
                self.apply_aura(i, &cfg.standby_aura, LONG_MS, None, boss, None);
            }
        }
        self.wing5.rt = Some(rt);
    }

    pub(crate) fn wing5_disarm(&mut self) {
        if let Some(rt) = self.wing5.rt.take()
            && rt.sent != (false, 0, false)
        {
            self.events.push(Event::Exercise {
                standby: false,
                sensors: 0,
                merged: false,
            });
        }
        self.sight.incidents.clear();
    }

    /// A joiner is told the exercise as it stands.
    pub(crate) fn wing5_on_join(&mut self, player: EntityId) {
        if let Some(rt) = &self.wing5.rt {
            let e = Event::Exercise {
                standby: rt.sent.0,
                sensors: rt.sent.1,
                merged: rt.sent.2,
            };
            self.emit_to(player, e);
        }
    }

    fn rider_on(&self, bi: usize, rider: &str) -> bool {
        self.has_aura(bi, rider)
    }

    /// Living units of `key`'s kind that stand watch: summoned by the boss, with eyes.
    fn sensors_of(&self, boss: EntityId) -> Vec<usize> {
        (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                !u.despawned
                    && u.state.alive
                    && u.summoner == Some(boss)
                    && self.content.units[u.def].sight.is_some()
            })
            .collect()
    }

    fn squad_idx(&self, side: SquadSide) -> Vec<usize> {
        self.squad_units(side)
            .into_iter()
            .filter_map(|u| self.index(u))
            .collect()
    }

    pub(crate) fn update_wing5(&mut self) {
        let Some(mut rt) = self.wing5.rt.take() else {
            self.sight.incidents.clear();
            return;
        };
        self.wing5_step(&mut rt);
        self.wing5.rt = Some(rt);
    }

    fn wing5_step(&mut self, rt: &mut SquadsRt) {
        let (boss, cfg) = (rt.boss, rt.cfg.clone());
        let Some(bi) = self.index(boss).filter(|&b| self.units[b].state.alive) else {
            return;
        };
        let story = self.raid.story;
        let red = self.squad_idx(SquadSide::Red);
        let blue = self.squad_idx(SquadSide::Blue);
        let sensors = self.sensors_of(boss);
        let merged_now = self.units[bi].state.phase >= cfg.merge_phase;
        let incidents: Vec<(EntityId, EntityId)> = std::mem::take(&mut self.sight.incidents);
        rt.seen_sensors |= !sensors.is_empty();
        let mut released = rt.released;

        // A sighting: counted for the title, and (off Story) it starts the exercise, marks the
        // player and, under the rider, throws them into Red's circle.
        for (player, _by) in &incidents {
            let gap = crate::content::ticks(INCIDENT_GAP_MS);
            let fresh = match rt.last_incident.iter_mut().find(|(p, _)| p == player) {
                Some((_, at)) if self.now < *at + gap => false,
                Some((_, at)) => {
                    *at = self.now;
                    true
                }
                None => {
                    rt.last_incident.push((*player, self.now));
                    true
                }
            };
            if !fresh {
                continue;
            }
            if !merged_now {
                rt.incidents_p1 += 1;
            }
            if story {
                continue;
            }
            if let Some(spell) = cfg
                .incident_spell
                .and_then(|s| self.content.spell(s))
                .cloned()
            {
                self.resolve(bi, &spell, Some(*player));
            }
            if !cfg.incident_yells.is_empty() {
                let n = cfg.incident_yells.len() as u32;
                let line = cfg.incident_yells[self.rng.below(n) as usize].clone();
                self.request_yell_line(bi, YellKind::Blast, Some(line));
            }
            self.wing5_teleport(bi, *player, rt, &red);
            released = true;
        }

        let start_ms = if story {
            cfg.story_start_ms
        } else {
            cfg.start_ms
        };
        let hurt = red.iter().any(|&i| {
            let s = &self.units[i].state;
            s.hp < s.max_hp
        });
        let sensors_down = rt.seen_sensors && sensors.is_empty();
        if !released
            && (sensors_down
                || hurt
                || merged_now
                || self.now >= rt.pulled + crate::content::ticks(start_ms))
        {
            released = true;
        }
        if released && !rt.released {
            self.wing5_release(bi, &cfg, &red, &blue);
        }
        rt.released = released;
        if !released {
            for &i in &red {
                if !self.has_aura(i, &cfg.standby_aura) {
                    self.apply_aura(i, &cfg.standby_aura, LONG_MS, None, boss, None);
                }
            }
        }

        // The merge: everyone turns purple, and the rest of the fight is a fight.
        if merged_now && !rt.merged {
            rt.merged = true;
            if let Some(line) = cfg.purple_yells.first().cloned() {
                self.request_yell_line(bi, YellKind::Phase, Some(line));
            }
        }
        if rt.merged {
            for i in red.iter().chain(&blue).copied().chain([bi]) {
                if !self.has_aura(i, &cfg.purple_aura) {
                    self.apply_aura(i, &cfg.purple_aura, LONG_MS, None, boss, None);
                }
            }
            if red.is_empty() && rt.red_gone.is_none() {
                rt.red_gone = Some(self.now);
            }
            if blue.is_empty() && rt.blue_gone.is_none() {
                rt.blue_gone = Some(self.now);
            }
        }
        if let Some(e) = &cfg.enrage_aura
            && self.has_aura(bi, e)
        {
            for &i in red.iter().chain(&blue) {
                if !self.has_aura(i, e) {
                    self.apply_aura(i, e, LONG_MS, None, boss, None);
                }
            }
        }
        let state = (!rt.released, sensors.len().min(255) as u8, rt.merged);
        if rt.sent != state {
            rt.sent = state;
            self.events.push(Event::Exercise {
                standby: state.0,
                sensors: state.1,
                merged: state.2,
            });
        }
    }

    /// The exercise starts: Red and the boss wake, Blue stops standing guard and joins in.
    fn wing5_release(&mut self, bi: usize, cfg: &SquadsDef, red: &[usize], blue: &[usize]) {
        self.remove_aura(bi, &cfg.boss_aura, AuraEnd::Expired);
        for &i in red {
            self.remove_aura(i, &cfg.standby_aura, AuraEnd::Expired);
        }
        // Blue was summoned idle: it joins the fight on whoever the boss is fighting.
        let target = self.units[bi]
            .threat
            .iter()
            .max_by_key(|(id, t)| (**t, std::cmp::Reverse(**id)))
            .map(|(id, _)| *id);
        if let Some(t) = target {
            for &i in red.iter().chain(blue) {
                if self.units[i].threat.is_empty() {
                    self.add_threat(i, t, 1);
                    self.set_ai(i, AiState::Chase);
                }
            }
        }
        if !cfg.start_yells.is_empty() {
            let n = cfg.start_yells.len() as u32;
            let line = cfg.start_yells[self.rng.below(n) as usize].clone();
            self.request_yell_line(bi, YellKind::Blast, Some(line));
        }
    }

    /// Judgment's rider: the first sighting each minute throws the player into Red's middle.
    fn wing5_teleport(&mut self, bi: usize, player: EntityId, rt: &mut SquadsRt, red: &[usize]) {
        let Some(rider) = rt.cfg.rider.as_deref() else {
            return;
        };
        if !self.rider_on(bi, rider) {
            return;
        }
        let gap = crate::content::ticks(rt.cfg.rider_teleport_ms);
        if rt.last_teleport.is_some_and(|t| self.now < t + gap) {
            return;
        }
        rt.last_teleport = Some(self.now);
        let Some(pi) = self.index(player).filter(|&p| self.units[p].state.alive) else {
            return;
        };
        let (mut x, mut z, mut n) = (0.0_f32, 0.0_f32, 0.0_f32);
        for &i in red {
            let p = self.units[i].state.pos;
            (x, z, n) = (x + p.x, z + p.z, n + 1.0);
        }
        let from = self.units[bi].state.pos;
        let pos = if n > 0.0 {
            Vec3::new(x / n, from.y, z / n)
        } else {
            from
        };
        self.units[pi].state.pos = pos;
        self.cancel_cast(pi, None, 0);
        self.events.push(Event::Teleported { unit: player, pos });
    }

    /// A honeypot: any class, one cooldown for the whole group, and never under the rider.
    pub(crate) fn use_honeypot_prop(&mut self, pi: usize, k: usize, spot: Vec3) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        let rider = self.wing5.rt.as_ref().is_some_and(|rt| {
            let (rider, bi) = (rt.cfg.rider.clone(), self.index(rt.boss));
            matches!((rider, bi), (Some(r), Some(b)) if self.has_aura(b, &r))
        });
        let mut outcome = 0;
        if !rider && self.prop_ready(player, k, 1000) && self.honeypot_use(spot, def.cooldown_ms) {
            outcome = crate::world::PROP_OK;
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key,
            outcome,
        });
    }

    /// Achievement keys of a boss kill beyond `raid:<boss>`: the Consultant's clear with no
    /// guardrail accepted, and Red Team Blue Team's silent phase and merged finish. `pulled` is
    /// the tick the fight began.
    pub(crate) fn wing5_clean_keys(&mut self, boss: &str, pulled: u64) -> Vec<String> {
        let mut keys = Vec::new();
        let squads = self
            .content
            .units
            .iter()
            .any(|u| u.key == boss && matches!(u.encounter, Some(EncounterDef::Squads(_))));
        if squads {
            if let Some(rt) = &self.wing5.rt {
                if rt.incidents_p1 == 0 {
                    keys.push(format!("raid:silent:{boss}"));
                }
                let window = crate::content::ticks(PURPLE_WINDOW_MS as u32);
                if let (false, Some(r), Some(b)) = (self.raid.story, rt.red_gone, rt.blue_gone)
                    && r.abs_diff(b) <= window
                {
                    keys.push(format!("raid:purple:{boss}"));
                }
            }
        } else if !self.raid.story
            && self.offers_waves_seen(pulled)
            && self.offers_accepted(pulled) == 0
        {
            keys.push(format!("raid:clean:{boss}"));
        }
        keys
    }
}
