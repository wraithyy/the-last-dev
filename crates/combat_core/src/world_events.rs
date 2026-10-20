//! Public world events on the campus ("Free Pizza", "Fire Drill"): every `every_ms` of group
//! time in the zone (the first after `first_ms`) a warning goes out `warn_ms` ahead, then for
//! `duration_ms` waves of a hostile unit come at a layout anchor. Every player who stands within
//! `radius_m` while it runs takes part, no sign-up. It is won when `kills` wave units fall in
//! time, or, with `defend`, when the defended unit is still standing at the end (its death fails
//! it). Winners get a share of their level's XP bar, Kudos per level and the achievements with
//! `Trigger::Event(<key>)`. Like the world boss it needs the world clock.

use crate::content::{Content, ContentError, ticks};
use crate::geom::{Vec3, in_range};
use crate::sim::{Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::types::{AiState, Event};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Most wave units alive at once, as a multiple of `wave_size`, so an ignored event cannot
/// flood the plaza.
const MAX_WAVES_ALIVE: u32 = 3;
/// Threat a wave unit starts with on its target, so it walks past bystanders.
const WAVE_THREAT: u32 = 50;

#[derive(Clone, Debug, Deserialize)]
pub struct WorldEventDef {
    pub key: String,
    pub name: String,
    pub zone: String,
    /// Layout anchor at the centre; a layout without it has no event.
    pub anchor: String,
    /// Friendly zone spawn who announces it (yells).
    pub host: String,
    pub first_ms: u32,
    pub every_ms: u32,
    pub warn_ms: u32,
    pub duration_ms: u32,
    pub radius_m: f32,
    pub wave: String,
    pub wave_size: u32,
    pub wave_every_ms: u32,
    /// Friendly unit spawned at the anchor for the event; the waves go for it.
    #[serde(default)]
    pub defend: Option<String>,
    /// Wave kills that win it; 0 means hold out until the timer ends.
    #[serde(default)]
    pub kills: u32,
    pub xp_pct: u32,
    pub sp_per_level: u32,
    #[serde(default)]
    pub warn_yell: String,
    #[serde(default)]
    pub start_yell: String,
    #[serde(default)]
    pub win_yell: String,
    #[serde(default)]
    pub fail_yell: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldEventPhase {
    /// Starts in `secs`.
    Warning,
    /// Running for `secs`; `need` kills (0 = survive the timer).
    Started,
    /// `count` of `need` kills.
    Progress,
    Won,
    Failed,
}

struct Running {
    ends_at: u64,
    next_wave: u64,
    waves: Vec<EntityId>,
    spawned: u32,
    defend: Option<EntityId>,
    kills: u32,
    joined: BTreeSet<EntityId>,
}

#[derive(Default)]
pub(crate) struct EventRt {
    left_ms: u32,
    spot: Option<(Vec3, f32)>,
    running: Option<Running>,
}

pub(crate) fn validate(c: &Content) -> Result<(), ContentError> {
    for (i, e) in c.live.events.iter().enumerate() {
        let bad = |m: &str| Err(ContentError::Invalid(format!("world event {}: {m}", e.key)));
        if c.live.events[..i].iter().any(|o| o.key == e.key)
            || e.key.len() > crate::world::WORLD_KEY_MAX
        {
            return bad("duplicate or too long key");
        }
        let hosted = c
            .zone(&e.zone)
            .is_some_and(|z| z.friendly.iter().any(|f| f.unit == e.host));
        if !hosted {
            return bad("host must be a friendly spawn of its zone");
        }
        if !c
            .unit_def(&e.wave)
            .is_some_and(|u| u.hostile && u.ai.is_some())
        {
            return bad("wave must be a hostile with AI");
        }
        if e.defend
            .as_ref()
            .is_some_and(|d| c.unit_def(d).is_none_or(|u| u.hostile))
        {
            return bad("defend must be a friendly unit");
        }
        let timed = e.every_ms > e.warn_ms
            && e.first_ms > 0
            && e.duration_ms >= 10_000
            && e.wave_every_ms >= 1000
            && e.wave_size > 0;
        if !timed || e.radius_m.is_nan() || e.radius_m <= 0.0 {
            return bad("needs a schedule, waves and a radius");
        }
    }
    Ok(())
}

impl Sim {
    /// Seconds until each world event of the current zone starts, `None` while it runs.
    pub fn world_event_in(&self, key: &str) -> Option<u32> {
        let k = self.content.live.events.iter().position(|e| e.key == key)?;
        let rt = self.live.events.get(k)?;
        rt.running.is_none().then_some(rt.left_ms / 1000)
    }

    /// Debug/QA: world event `key` starts in `secs` (at least 1; the warning fires on the way
    /// down when `secs` is above it). False for an unknown key.
    pub fn debug_world_event_in(&mut self, key: &str, secs: u32) -> bool {
        let Some(k) = self.content.live.events.iter().position(|e| e.key == key) else {
            return false;
        };
        self.init_world_events();
        let left = secs.saturating_mul(1000).max(1000);
        self.live.events[k].left_ms = left;
        if left <= self.content.live.events[k].warn_ms {
            self.events.push(Event::WorldEvent {
                key: key.to_string(),
                phase: WorldEventPhase::Warning,
                secs: left / 1000,
                count: 0,
                need: 0,
            });
        }
        true
    }

    /// One runtime per event, counting down from `first_ms` (only while the clock is on).
    pub(crate) fn init_world_events(&mut self) {
        if self.live.events.len() != self.content.live.events.len() {
            self.live.events = self
                .content
                .live
                .events
                .iter()
                .map(|e| EventRt {
                    left_ms: e.first_ms,
                    ..EventRt::default()
                })
                .collect();
        }
    }

    /// From `load_live`: the new zone's event spots; a running event ended with the load (its
    /// units are gone), so it counts down again from `every_ms`.
    pub(crate) fn load_world_events(
        &mut self,
        zone: &str,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    ) {
        self.init_world_events();
        for (k, rt) in self.live.events.iter_mut().enumerate() {
            let def = &self.content.live.events[k];
            if rt.running.take().is_some() {
                rt.left_ms = def.every_ms;
            }
            rt.spot = (def.zone == zone).then(|| anchors(&def.anchor)).flatten();
        }
    }

    /// Every tick: wave kills, and the defended unit.
    pub(crate) fn watch_world_events(&mut self) {
        for k in 0..self.live.events.len() {
            let Some(run) = self.live.events[k].running.as_mut() else {
                continue;
            };
            let mut kills = 0;
            let units = &self.units;
            run.waves.retain(|id| {
                let Some(u) = units.get(id.saturating_sub(1) as usize) else {
                    return false;
                };
                if u.despawned {
                    return false;
                }
                if !u.state.alive {
                    kills += 1;
                    return false;
                }
                true
            });
            if kills == 0 {
                continue;
            }
            run.kills += kills;
            let (count, def) = (run.kills, &self.content.live.events[k]);
            if def.kills > 0 {
                let e = Event::WorldEvent {
                    key: def.key.clone(),
                    phase: WorldEventPhase::Progress,
                    secs: 0,
                    count: count.min(def.kills),
                    need: def.kills,
                };
                self.events.push(e);
            }
        }
    }

    /// Once a second with the clock on: countdowns in the event's zone, waves, the end.
    pub(crate) fn world_events_second(&mut self) {
        for k in 0..self.live.events.len() {
            let def = self.content.live.events[k].clone();
            let Some((pos, _)) = self.live.events[k].spot else {
                continue;
            };
            if self.sprint_active() || self.zone.as_deref() != Some(def.zone.as_str()) {
                continue;
            }
            if self.live.events[k].running.is_some() {
                self.run_world_event(k, &def, pos);
                continue;
            }
            let left = self.live.events[k].left_ms.saturating_sub(1000);
            self.live.events[k].left_ms = left;
            if left > 0 {
                if left == def.warn_ms {
                    self.event_yell(&def.host, &def.warn_yell);
                    self.events.push(Event::WorldEvent {
                        key: def.key.clone(),
                        phase: WorldEventPhase::Warning,
                        secs: left / 1000,
                        count: 0,
                        need: 0,
                    });
                }
                continue;
            }
            self.start_world_event(k, &def, pos);
        }
    }

    fn start_world_event(&mut self, k: usize, def: &crate::world_events::WorldEventDef, pos: Vec3) {
        let defend = def.defend.as_ref().and_then(|d| self.def_index(d).ok());
        let defend = defend.map(|d| {
            let yaw = self.live.events[k].spot.map_or(0.0, |s| s.1);
            self.spawn(d, pos, yaw, plain_opts(None))
        });
        self.live.events[k].running = Some(Running {
            ends_at: self.now + ticks(def.duration_ms),
            next_wave: self.now + ticks(2000),
            waves: Vec::new(),
            spawned: 0,
            defend,
            kills: 0,
            joined: BTreeSet::new(),
        });
        self.event_yell(&def.host, &def.start_yell);
        self.events.push(Event::WorldEvent {
            key: def.key.clone(),
            phase: WorldEventPhase::Started,
            secs: def.duration_ms / 1000,
            count: 0,
            need: def.kills,
        });
    }

    fn run_world_event(&mut self, k: usize, def: &crate::world_events::WorldEventDef, pos: Vec3) {
        let near: Vec<EntityId> = self
            .players
            .iter()
            .copied()
            .filter(|p| {
                self.unit(*p)
                    .is_some_and(|u| u.alive && in_range(u.pos, pos, def.radius_m))
            })
            .collect();
        let Some(run) = self.live.events[k].running.as_mut() else {
            return;
        };
        run.joined.extend(near.iter().copied());
        let defend_down = run.defend.is_some_and(|id| {
            self.units
                .get(id.saturating_sub(1) as usize)
                .is_none_or(|u| u.despawned || !u.state.alive)
        });
        let won = (def.kills > 0 && run.kills >= def.kills)
            || (def.kills == 0 && self.now >= run.ends_at && !defend_down);
        if won || defend_down || self.now >= run.ends_at {
            return self.end_world_event(k, def, won);
        }
        let alive = run.waves.len() as u32;
        if self.now < run.next_wave || alive >= def.wave_size * MAX_WAVES_ALIVE {
            return;
        }
        run.next_wave = self.now + ticks(def.wave_every_ms);
        let (defend, first) = (run.defend, run.spawned);
        run.spawned += def.wave_size;
        let Ok(wave) = self.def_index(&def.wave) else {
            return;
        };
        let host = self
            .units
            .iter()
            .position(|u| !u.despawned && !u.state.hostile && u.state.key == def.host)
            .map(Self::id_of);
        let mut ids = Vec::new();
        for n in first..first + def.wave_size {
            // Golden-angle spacing: spread around the anchor without touching the rng.
            let a = n as f32 * 2.399_963;
            let r = def.radius_m * 0.7;
            let at = Vec3::new(pos.x + r * a.cos(), pos.y, pos.z + r * a.sin());
            let level = self.event_level();
            let mut opts = plain_opts(defend.or(host));
            opts.level = Some(level);
            let id = self.spawn(wave, at, crate::ai::yaw_toward(at, pos), opts);
            // Waves go for the defended unit, else for whoever is closest to the anchor.
            let target = defend.or_else(|| {
                near.iter().copied().min_by(|a, b| {
                    let d = |p: &EntityId| {
                        self.unit(*p)
                            .map_or(f32::MAX, |u| crate::geom::distance(u.pos, at))
                    };
                    d(a).total_cmp(&d(b))
                })
            });
            if let (Some(wi), Some(t)) = (self.index(id), target) {
                self.add_threat(wi, t, WAVE_THREAT);
                self.set_ai(wi, AiState::Chase);
            }
            ids.push(id);
        }
        if let Some(run) = self.live.events[k].running.as_mut() {
            run.waves.extend(ids);
        }
    }

    fn end_world_event(&mut self, k: usize, def: &crate::world_events::WorldEventDef, won: bool) {
        let Some(run) = self.live.events[k].running.take() else {
            return;
        };
        self.live.events[k].left_ms = def.every_ms;
        for id in run.waves.iter().chain(run.defend.iter()) {
            if let Some(i) = self.index(*id).filter(|i| !self.units[*i].despawned) {
                self.despawn(i);
            }
        }
        self.event_yell(&def.host, if won { &def.win_yell } else { &def.fail_yell });
        self.events.push(Event::WorldEvent {
            key: def.key.clone(),
            phase: if won {
                WorldEventPhase::Won
            } else {
                WorldEventPhase::Failed
            },
            secs: 0,
            count: run.kills.min(def.kills),
            need: def.kills,
        });
        if !won {
            return;
        }
        for p in run.joined {
            let Some(i) = self.index(p).filter(|_| self.is_player(p)) else {
                continue;
            };
            let level = self.units[i].state.level;
            let xp = u64::from(self.content.xp_to_next(level)) * u64::from(def.xp_pct) / 100;
            self.grant_xp(i, xp as u32);
            let sp = def.sp_per_level * u32::from(level);
            if sp > 0 {
                let s = &mut self.units[i].state;
                s.sp = s.sp.saturating_add(sp);
                self.bag_event(i);
            }
            let key = def.key.clone();
            self.check_achievements(
                p,
                |t| matches!(t, crate::world::Trigger::Event(e) if *e == key),
            );
        }
    }

    /// Waves match the group's best level, like the night shift.
    fn event_level(&self) -> u8 {
        self.players
            .iter()
            .filter_map(|p| self.unit(*p))
            .map(|u| u.level)
            .max()
            .unwrap_or(1)
    }

    fn event_yell(&mut self, host: &str, text: &str) {
        if text.is_empty() {
            return;
        }
        let unit = self
            .units
            .iter()
            .position(|u| !u.despawned && u.state.alive && u.state.key == host)
            .map(Self::id_of);
        if let Some(unit) = unit {
            self.events.push(Event::Yell {
                unit,
                text: text.to_string(),
            });
        }
    }
}

fn plain_opts(summoner: Option<EntityId>) -> SpawnOpts {
    SpawnOpts {
        level: None,
        pack: None,
        respawns: false,
        summoner,
        floor_boss: false,
        zone_slot: None,
    }
}
