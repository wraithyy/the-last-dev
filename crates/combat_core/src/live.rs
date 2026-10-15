//! The live world: a wall clock (the UTC date for daily and weekly tasks, and a 24-minute
//! office day), "Crunch mode" nights on the campus with their own enemies, and the campus
//! world boss "Board Meeting". Daily and weekly tasks are in `dailies.rs`; their per-player
//! state lives in `LiveRt` too, so the sim carries one field for all of it.
//!
//! The sim cannot read a clock: the host anchors one with `Sim::set_clock` (the device clock
//! offline, the server's online) and time then advances with sim ticks, so everything stays
//! deterministic for a given anchor. Without an anchor (tests, golden replays) the world clock
//! is off: no nights, no world boss, and the date is day 0.

use crate::content::{Content, ContentError, ticks};
use crate::geom::Vec3;
use crate::sim::{Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::time::TICK_HZ;
use crate::types::Event;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One office day lasts 24 real minutes: a real second is a game minute.
pub const GAME_DAY_S: u64 = 1440;
/// Game minute of the day when Crunch mode starts (18:00) and ends (06:00).
pub const DUSK_MIN: u32 = 18 * 60;
pub const DAWN_MIN: u32 = 6 * 60;
/// "Go home" counts when clocking out within this many game minutes after 18:00 (real seconds).
pub const GO_HOME_WINDOW_MIN: u32 = 10;
const DAY_S: u64 = 86_400;

/// Game minute of the day (0..1440) at a unix time.
pub fn game_minute(unix_s: u64) -> u32 {
    (unix_s % GAME_DAY_S) as u32
}

pub fn is_night_minute(minute: u32) -> bool {
    !(DAWN_MIN..DUSK_MIN).contains(&minute)
}

/// 0 by day, 1 at night, eased over an hour around dusk (17:15-18:15) and dawn (05:45-06:45),
/// so the client can light the campus smoothly from the same clock the sim uses.
pub fn darkness(minute: f32) -> f32 {
    let m = minute.rem_euclid(GAME_DAY_S as f32);
    let x = if m >= 720.0 {
        (m - 1035.0) / 60.0
    } else {
        1.0 - (m - 345.0) / 60.0
    };
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Days since 1970-01-01 (UTC): the daily rotation.
pub fn day_index(unix_s: u64) -> u32 {
    (unix_s / DAY_S) as u32
}

/// ISO weeks since the Monday before the epoch (1970-01-01 was a Thursday): the weekly rotation.
pub fn week_index(unix_s: u64) -> u32 {
    ((unix_s / DAY_S + 3) / 7) as u32
}

/// Seconds until the dailies reset (00:00 UTC).
pub fn secs_to_day_reset(unix_s: u64) -> u64 {
    DAY_S - unix_s % DAY_S
}

/// Seconds until the weekly goal resets (Monday 00:00 UTC).
pub fn secs_to_week_reset(unix_s: u64) -> u64 {
    let next = (u64::from(week_index(unix_s)) + 1) * 7 - 3;
    (next * DAY_S).saturating_sub(unix_s)
}

fn default_every() -> u32 {
    1_800_000
}

fn default_warn() -> u32 {
    300_000
}

fn default_idle() -> u32 {
    600_000
}

/// The campus world boss: spawns every `every_ms` of time spent in `zone` (warning `warn_ms`
/// before), leaves if nobody engages it within `idle_ms`. The kill grants achievements with
/// `Trigger::Event(<unit key>)` to every group member.
#[derive(Clone, Debug, Deserialize)]
pub struct WorldBossDef {
    pub unit: String,
    pub zone: String,
    pub anchor: String,
    pub level: u8,
    #[serde(default = "default_every")]
    pub every_ms: u32,
    #[serde(default = "default_warn")]
    pub warn_ms: u32,
    #[serde(default = "default_idle")]
    pub idle_ms: u32,
    #[serde(default)]
    pub spawn_yell: String,
    #[serde(default)]
    pub cancel_yell: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NightSpawn {
    pub anchor: String,
    pub unit: String,
}

/// Crunch mode: units at layout anchors of `zone` between 18:00 and 06:00, and friendly NPCs
/// whose ambient chatter changes for the night.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NightDef {
    #[serde(default)]
    pub zone: String,
    #[serde(default)]
    pub spawns: Vec<NightSpawn>,
    /// A killed night unit comes back after this, if it is still night.
    #[serde(default)]
    pub respawn_ms: u32,
    /// (friendly unit key, night lines).
    #[serde(default)]
    pub ambient: Vec<(String, Vec<String>)>,
}

fn default_per_day() -> usize {
    3
}

/// `content.live`: the task board, daily/weekly templates and rewards, the world boss and the
/// night. Absent in minimal test content.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct LiveDef {
    #[serde(default)]
    pub board: Option<crate::dailies::BoardDef>,
    #[serde(default = "default_per_day")]
    pub per_day: usize,
    /// Rewards: percent of the level's XP bar, and Story Points per level.
    #[serde(default)]
    pub daily_xp_pct: u32,
    #[serde(default)]
    pub daily_sp_per_level: u32,
    #[serde(default)]
    pub weekly_xp_pct: u32,
    #[serde(default)]
    pub weekly_sp_per_level: u32,
    #[serde(default)]
    pub dailies: Vec<crate::dailies::TaskDef>,
    #[serde(default)]
    pub weeklies: Vec<crate::dailies::TaskDef>,
    #[serde(default)]
    pub boss: Option<WorldBossDef>,
    #[serde(default)]
    pub night: NightDef,
}

impl LiveDef {
    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        validate_world(c)?;
        crate::dailies::validate_tasks(c)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldBossState {
    /// `secs` until it spawns.
    Warning,
    Spawned,
    /// Nobody engaged it in time; it left.
    Cancelled,
    Defeated,
}

pub(crate) struct NightSpot {
    def: usize,
    pos: Vec3,
    yaw: f32,
    unit: Option<EntityId>,
    respawn_at: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct BossRt {
    pub id: EntityId,
    spawned_at: u64,
    engaged: bool,
}

#[derive(Default)]
pub(crate) struct LiveRt {
    /// (unix seconds, tick) of the anchor; `None` while the world clock is off.
    clock: Option<(u64, u64)>,
    /// Keeps the night shift away while a new player is in the onboarding.
    hold_night: bool,
    boss_left_ms: u32,
    boss_spot: Option<(Vec3, f32)>,
    pub(crate) boss: Option<BossRt>,
    night_spots: Vec<NightSpot>,
    /// Next night line per friendly unit index, and its line cursor.
    night_lines: BTreeMap<usize, (u64, usize)>,
    /// Per-player daily and weekly tasks (`dailies.rs`).
    pub(crate) tasks: BTreeMap<EntityId, crate::dailies::PlayerTasks>,
    pub(crate) board_npc: Option<EntityId>,
}

impl Content {
    pub fn world_boss(&self) -> Option<&WorldBossDef> {
        self.live.boss.as_ref()
    }

    /// True for a Crunch-mode night unit.
    pub fn is_night_unit(&self, key: &str) -> bool {
        self.live.night.spawns.iter().any(|s| s.unit == key)
    }
}

fn validate_world(c: &Content) -> Result<(), ContentError> {
    let bad = |m: String| Err(ContentError::Invalid(m));
    let live = &c.live;
    if let Some(b) = &live.boss {
        let boss = c.unit_def(&b.unit).is_some_and(|u| u.hostile && u.boss);
        if !boss || c.zone(&b.zone).is_none() || b.warn_ms >= b.every_ms || b.level == 0 {
            return bad(format!(
                "world boss {} needs a hostile boss unit, a zone and warn < every",
                b.unit
            ));
        }
    }
    let n = &live.night;
    if !n.spawns.is_empty() && c.zone(&n.zone).is_none() {
        return bad(format!("night zone {} unknown", n.zone));
    }
    for s in &n.spawns {
        if !c
            .unit_def(&s.unit)
            .is_some_and(|u| u.hostile && u.ai.is_some())
        {
            return bad(format!("night unit {} must be a hostile with AI", s.unit));
        }
    }
    for (u, lines) in &n.ambient {
        if c.unit_def(u).is_none_or(|u| u.hostile) || lines.is_empty() {
            return bad(format!("night ambient {u} needs a friendly unit and lines"));
        }
    }
    Ok(())
}

impl Sim {
    /// Anchors the world clock: `unix_s` is now. Turns on nights and the world boss (its
    /// countdown starts on the first call) and tells everyone (`Event::Clock`).
    pub fn set_clock(&mut self, unix_s: u64) {
        if self.live.clock.is_none() {
            self.live.boss_left_ms = self.content.world_boss().map_or(0, |b| b.every_ms);
        }
        self.live.clock = Some((unix_s, self.now));
        self.events.push(Event::Clock { unix_s });
    }

    /// Unix seconds now: the anchor plus sim time (sim time alone while the clock is off).
    pub fn unix_s(&self) -> u64 {
        let (at, tick) = self.live.clock.unwrap_or((0, 0));
        at + self.now.saturating_sub(tick) / u64::from(TICK_HZ)
    }

    /// `unix_s` with the fraction of the current second, for smooth client lighting.
    pub fn unix_secs_f64(&self) -> f64 {
        let (at, tick) = self.live.clock.unwrap_or((0, 0));
        at as f64 + self.now.saturating_sub(tick) as f64 / f64::from(TICK_HZ)
    }

    pub fn clock_on(&self) -> bool {
        self.live.clock.is_some()
    }

    /// Game minute of the office day (0..1440).
    pub fn game_minute(&self) -> u32 {
        game_minute(self.unix_s())
    }

    /// Crunch mode: only with the world clock on.
    pub fn is_night(&self) -> bool {
        self.clock_on() && !self.live.hold_night && is_night_minute(self.game_minute())
    }

    pub fn is_night_held(&self) -> bool {
        self.live.hold_night
    }

    pub fn set_hold_night(&mut self, hold: bool) {
        self.live.hold_night = hold;
    }

    /// Seconds until the world boss spawns while the group stays in its zone; `None` while it
    /// is up or there is none.
    pub fn world_boss_in(&self) -> Option<u32> {
        self.content.world_boss()?;
        if self.live.boss.is_some() {
            return None;
        }
        Some(self.live.boss_left_ms / 1000)
    }

    /// The living world boss, if it is up.
    pub fn world_boss_unit(&self) -> Option<EntityId> {
        self.live.boss.map(|b| b.id)
    }

    /// Debug/QA: the world boss countdown is set to `secs` (the warning fires on the way down
    /// when `secs` is above it). False without a world boss.
    pub fn debug_world_boss_in(&mut self, secs: u32) -> bool {
        if self.content.world_boss().is_none() {
            return false;
        }
        self.live.boss_left_ms = secs.saturating_mul(1000).max(1000);
        if self.live.boss_left_ms <= self.content.world_boss().map_or(0, |b| b.warn_ms) {
            let secs = self.live.boss_left_ms / 1000;
            self.events.push(Event::WorldBoss {
                state: WorldBossState::Warning,
                secs,
                unit: 0,
            });
        }
        true
    }

    /// A joining member learns the clock.
    pub(crate) fn live_on_join(&mut self, player: EntityId) {
        if self.live.clock.is_some() {
            let unix_s = self.unix_s();
            self.emit_to(player, Event::Clock { unix_s });
        }
    }

    /// Called by `load_zone` after its units exist: the scrum master, the world boss spot and
    /// the night anchors of the new zone. A world boss left behind is gone (the load despawned
    /// it) and the countdown starts over.
    pub(crate) fn load_live(&mut self, zone: &str, anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>) {
        if self.live.boss.take().is_some() {
            self.live.boss_left_ms = self.content.world_boss().map_or(0, |b| b.every_ms);
        }
        self.live.boss_spot = self
            .content
            .world_boss()
            .filter(|b| b.zone == zone)
            .and_then(|b| anchors(&b.anchor));
        self.live.night_spots.clear();
        self.live.night_lines.clear();
        if self.content.live.night.zone == zone {
            let spots: Vec<NightSpot> = self
                .content
                .live
                .night
                .spawns
                .iter()
                .filter_map(|s| {
                    let (pos, yaw) = anchors(&s.anchor)?;
                    Some(NightSpot {
                        def: self.def_index(&s.unit).ok()?,
                        pos,
                        yaw,
                        unit: None,
                        respawn_at: 0,
                    })
                })
                .collect();
            self.live.night_spots = spots;
        }
        self.load_board(zone, anchors);
    }

    /// Once per tick: task credit from this tick's events; once a second the clock-driven parts.
    pub(crate) fn update_live(&mut self) {
        self.scan_task_events();
        self.watch_world_boss();
        if !self.now.is_multiple_of(ticks(1000)) {
            return;
        }
        self.quest_waits();
        self.tasks_second();
        if self.live.clock.is_none() {
            return;
        }
        self.update_night();
        self.world_boss_second();
    }

    /// `Command::GoHome`: clocking out at 18:00 sharp is an achievement.
    pub(crate) fn go_home(&mut self, player: EntityId) {
        let m = self.game_minute();
        if self.clock_on() && (DUSK_MIN..DUSK_MIN + GO_HOME_WINDOW_MIN).contains(&m) {
            self.check_achievements(
                player,
                |t| matches!(t, crate::world::Trigger::Event(k) if k == "go_home"),
            );
        }
    }

    fn group_level(&self) -> u8 {
        self.players
            .iter()
            .filter_map(|p| self.unit(*p))
            .map(|u| u.level)
            .max()
            .unwrap_or(1)
    }

    fn update_night(&mut self) {
        let night = self.is_night();
        let level = self.group_level();
        let respawn = ticks(self.content.live.night.respawn_ms.max(1000));
        let mut spawn = Vec::new();
        let mut leave = Vec::new();
        for (k, s) in self.live.night_spots.iter_mut().enumerate() {
            if let Some(id) = s.unit {
                let i = id.saturating_sub(1) as usize;
                let Some(u) = self.units.get(i) else {
                    s.unit = None;
                    continue;
                };
                if u.despawned || !u.state.alive {
                    s.unit = None;
                    s.respawn_at = self.now + respawn;
                } else if !night && !u.state.in_combat && u.threat.is_empty() {
                    // Dawn: the night shift clocks out, unless it is busy.
                    s.unit = None;
                    leave.push(i);
                }
            } else if night && self.now >= s.respawn_at {
                spawn.push(k);
            }
        }
        for i in leave {
            self.despawn(i);
        }
        for k in spawn {
            let (def, pos, yaw) = {
                let s = &self.live.night_spots[k];
                (s.def, s.pos, s.yaw)
            };
            let opts = SpawnOpts {
                level: Some(level),
                pack: None,
                respawns: false,
                summoner: None,
                floor_boss: false,
                zone_slot: None,
            };
            let id = self.spawn(def, pos, yaw, opts);
            self.live.night_spots[k].unit = Some(id);
        }
        self.night_chatter(night);
    }

    /// Night lines for the friendly NPCs listed in `night.ambient`; their day chatter waits.
    fn night_chatter(&mut self, night: bool) {
        let lines = self.content.live.night.ambient.clone();
        if lines.is_empty() {
            return;
        }
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.despawned || !u.state.alive || u.state.hostile {
                continue;
            }
            let Some((_, night_lines)) = lines.iter().find(|(k, _)| *k == u.state.key) else {
                continue;
            };
            if !night {
                self.live.night_lines.remove(&i);
                continue;
            }
            let every = ticks(self.content.units[u.def].yells.ambient_ms.max(15_000));
            // Keep the day chatter from firing while the night lines run.
            self.units[i].ambient_next = self.units[i].ambient_next.max(self.now + every);
            let now = self.now;
            let (next, cursor) = self
                .live
                .night_lines
                .entry(i)
                .or_insert((now + ticks(2000) * (i as u64 % 4 + 1), i));
            if now < *next {
                continue;
            }
            *next = now + every;
            let text = night_lines[*cursor % night_lines.len()].clone();
            *cursor += 1;
            let near = self
                .players
                .iter()
                .filter_map(|p| self.unit(*p))
                .any(|p| crate::geom::in_range(p.pos, self.units[i].state.pos, 15.0));
            if near {
                self.events.push(Event::Yell {
                    unit: Sim::id_of(i),
                    text,
                });
            }
        }
    }

    /// Every tick: a world boss that died or engaged.
    fn watch_world_boss(&mut self) {
        let Some(b) = self.live.boss else { return };
        let Some(i) = self.index(b.id) else { return };
        let u = &self.units[i];
        if u.despawned {
            self.live.boss = None;
            self.live.boss_left_ms = self.content.world_boss().map_or(0, |d| d.every_ms);
            return;
        }
        if !u.state.alive {
            self.live.boss = None;
            self.live.boss_left_ms = self.content.world_boss().map_or(0, |d| d.every_ms);
            self.events.push(Event::WorldBoss {
                state: WorldBossState::Defeated,
                secs: 0,
                unit: b.id,
            });
            let key = u.state.key.clone();
            for p in self.players.clone() {
                if self.unit(p).is_some() {
                    self.check_achievements(
                        p,
                        |t| matches!(t, crate::world::Trigger::Event(k) if *k == key),
                    );
                }
            }
            return;
        }
        if !b.engaged && (u.state.in_combat || !u.threat.is_empty()) {
            self.live.boss = Some(BossRt { engaged: true, ..b });
        }
    }

    fn world_boss_second(&mut self) {
        let Some(def) = self.content.world_boss().cloned() else {
            return;
        };
        if let Some(b) = self.live.boss {
            if !b.engaged && self.now >= b.spawned_at + ticks(def.idle_ms) {
                if let Some(i) = self.index(b.id) {
                    if !def.cancel_yell.is_empty() {
                        self.events.push(Event::Yell {
                            unit: b.id,
                            text: def.cancel_yell.clone(),
                        });
                    }
                    self.despawn(i);
                }
                self.live.boss = None;
                self.live.boss_left_ms = def.every_ms;
                self.events.push(Event::WorldBoss {
                    state: WorldBossState::Cancelled,
                    secs: 0,
                    unit: b.id,
                });
            }
            return;
        }
        let Some((pos, yaw)) = self.live.boss_spot else {
            return;
        };
        if self.sprint_active() || self.zone.as_deref() != Some(def.zone.as_str()) {
            return;
        }
        let left = self.live.boss_left_ms.saturating_sub(1000);
        self.live.boss_left_ms = left;
        if left > 0 {
            if left == def.warn_ms || left == 60_000 {
                self.events.push(Event::WorldBoss {
                    state: WorldBossState::Warning,
                    secs: left / 1000,
                    unit: 0,
                });
            }
            return;
        }
        let Ok(d) = self.def_index(&def.unit) else {
            return;
        };
        let pack = self.next_pack;
        self.next_pack += 1;
        let opts = SpawnOpts {
            level: Some(def.level),
            pack: Some(pack),
            respawns: false,
            summoner: None,
            floor_boss: false,
            zone_slot: None,
        };
        let id = self.spawn(d, pos, yaw, opts);
        self.live.boss = Some(BossRt {
            id,
            spawned_at: self.now,
            engaged: false,
        });
        self.events.push(Event::WorldBoss {
            state: WorldBossState::Spawned,
            secs: def.idle_ms / 1000,
            unit: id,
        });
        if !def.spawn_yell.is_empty() {
            self.events.push(Event::Yell {
                unit: id,
                text: def.spawn_yell,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_math() {
        // 2026-10-15 12:00 UTC is a Thursday.
        let t = 1_792_065_600;
        assert_eq!(day_index(t), 20_741);
        assert_eq!(secs_to_day_reset(t), 12 * 3600);
        // Next Monday 2026-10-19 00:00 UTC.
        assert_eq!(secs_to_week_reset(t), 3 * 86_400 + 12 * 3600);
        assert_eq!(week_index(t + secs_to_week_reset(t)), week_index(t) + 1);
        assert_eq!(week_index(t + secs_to_week_reset(t) - 1), week_index(t));
    }

    #[test]
    fn night_and_darkness() {
        assert!(!is_night_minute(DUSK_MIN - 1));
        assert!(is_night_minute(DUSK_MIN));
        assert!(is_night_minute(DAWN_MIN - 1));
        assert!(!is_night_minute(DAWN_MIN));
        assert_eq!(darkness(12.0 * 60.0), 0.0);
        assert_eq!(darkness(0.0), 1.0);
        assert!(darkness(18.0 * 60.0) > 0.5 && darkness(18.0 * 60.0) < 1.0);
        assert!(darkness(6.0 * 60.0) > 0.5 && darkness(6.0 * 60.0) < 1.0);
        assert_eq!(darkness(7.0 * 60.0), 0.0);
        // Continuous across midnight and noon.
        assert!((darkness(1439.9) - darkness(0.0)).abs() < 0.01);
        assert!((darkness(719.9) - darkness(720.0)).abs() < 0.01);
    }
}
