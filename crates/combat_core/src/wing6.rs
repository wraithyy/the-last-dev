//! Ilmari Wing 6, the Boardroom in the Sky: the Chief Ilmari Officer (docs/ILMARI_1_0.md D.4). The
//! CEO's table is remade with the pieces a spell cannot hold: the desks that Earnings Call takes
//! away for good, the floor tiles Right-Sizing clears, the inbox with its terminals, the parachute
//! whose landing needs two different interrupters, the severance meter of Offboarding, the
//! invoices and apples of the adds, and Judgment's Heir. The spells themselves are content; this
//! is the state that outlives a cast. Everything is sim state and the combat rng only, so a replay
//! reproduces it exactly. Numbers: docs/FORMULAS.md, The Chief Ilmari Officer.

use crate::burnout::BurnoutCause;
use crate::content::{SpellId, YellKind, ticks};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{AuraEnd, Event};
use serde::Deserialize;

/// Auras that last "until the fight ends": evade and death clear every aura.
const LONG_MS: u32 = 3_600_000;

/// The Heir of the Judgment rider: a copy that only parachutes.
#[derive(Clone, Debug, Deserialize)]
pub struct HeirDef {
    pub unit: String,
    /// Cast by the boss when the Heir joins (a `Summon` of `unit`).
    pub summon: SpellId,
    /// The Heir's own landing: one kick from somebody who did not kick the boss's last landing.
    pub landing: SpellId,
    /// The boss phase (1-based) in which the Heir joins.
    pub phase: u8,
    pub yells: Vec<String>,
}

/// The boardroom's machinery; the encounter of the Chief Ilmari Officer.
#[derive(Clone, Debug, Deserialize)]
pub struct BoardroomDef {
    /// Earnings Call spells by desks left standing (index `desks`, so six entries for five desks).
    pub earnings: Vec<SpellId>,
    pub earnings_phase: u8,
    pub earnings_first_ms: u32,
    pub earnings_ms: u32,
    /// Desks one cast takes away (the rider takes `rider_desks`).
    pub rider_desks: u8,
    pub earnings_yells: Vec<String>,
    /// One spell per floor tile of the 3 x 3 grid (the centre, index 4, is never cleared).
    pub tiles: Vec<SpellId>,
    pub tiles_phase: u8,
    pub tiles_first_ms: u32,
    pub tiles_ms: u32,
    pub tiles_max: u8,
    pub story_tiles_max: u8,
    /// Hiring Freeze: below this boss HP percent two tiles come back, once.
    pub freeze_below_pct: u32,
    pub freeze_yells: Vec<String>,
    pub tile_yells: Vec<String>,
    /// Golden Parachute: the lift-off spell, the rain of Bonus Payments, the aura that makes the
    /// boss untouchable, how long it lasts, and the landing cast that needs the kicks.
    pub parachute: SpellId,
    pub rain: SpellId,
    pub airborne_aura: String,
    pub airborne_ms: u32,
    pub landing: SpellId,
    pub parachute_phase: u8,
    pub parachute_first_ms: u32,
    pub parachute_ms: u32,
    pub pair_window_ms: u32,
    pub parachute_yells: Vec<String>,
    /// The inbox: a message every `inbox_ms`, `window_ms` (`story_window_ms`) to clear it at a
    /// terminal, `notified_aura` on everyone who did not.
    pub inbox_first_ms: u32,
    pub inbox_ms: u32,
    pub window_ms: u32,
    pub story_window_ms: u32,
    pub notified_aura: String,
    pub notified_ms: u32,
    pub inbox_yells: Vec<String>,
    /// Offboarding: at this HP percent the boss sits for `severance_ms` wearing `offboard_aura`;
    /// if the meter fills he heals `heal_pct` and the loop may repeat `sits - 1` more times.
    /// Story skips it.
    pub offboard_below_pct: u32,
    pub offboard_aura: String,
    pub severance_ms: u32,
    pub heal_pct: u32,
    pub sits: u8,
    pub offboard_yells: Vec<String>,
    pub severance_yells: Vec<String>,
    /// Invoiced consultants: unpaid at the deadline they leave and bill the group.
    pub invoice_unit: String,
    pub invoice_ms: u32,
    pub invoice_burnout: u16,
    pub invoice_yells: Vec<String>,
    /// Low-Hanging Fruit: killed, an apple heals its killer `apple_heal_pct`; left alive for
    /// `apple_ms` it gives the boss a stack of `apple_aura`.
    pub apple_unit: String,
    pub apple_ms: u32,
    pub apple_heal_pct: u32,
    pub apple_aura: String,
    /// Judgment's rider: the Heir, and the earnings call that takes `rider_desks` desks.
    #[serde(default)]
    pub rider: Option<String>,
    #[serde(default)]
    pub heir: Option<HeirDef>,
}

impl BoardroomDef {
    pub(crate) fn validate(&self, c: &crate::content::Content, unit: &str) -> bool {
        let spells = self
            .earnings
            .iter()
            .chain(&self.tiles)
            .chain([&self.parachute, &self.rain, &self.landing])
            .chain(self.heir.iter().flat_map(|h| [&h.summon, &h.landing]));
        let auras = [
            &self.airborne_aura,
            &self.notified_aura,
            &self.offboard_aura,
            &self.apple_aura,
        ]
        .into_iter()
        .chain(self.rider.iter());
        let units = [&self.invoice_unit, &self.apple_unit]
            .into_iter()
            .chain(self.heir.iter().map(|h| &h.unit));
        spells.into_iter().all(|s| c.spell(*s).is_some())
            && auras.into_iter().all(|a| c.aura(a).is_some())
            && units.into_iter().all(|u| c.unit_def(u).is_some())
            && self.earnings.len() >= 2
            && self.tiles.len() == 9
            && self.earnings_phase >= 2
            && self.tiles_phase >= 2
            && self.parachute_phase >= 2
            && self.earnings_ms > 0
            && self.tiles_ms > 0
            && self.parachute_ms > 0
            && self.airborne_ms > 0
            && self.inbox_ms > 0
            && self.window_ms > 0
            && self.story_window_ms > 0
            && self.pair_window_ms > 0
            && self.severance_ms > 0
            && self.sits > 0
            && self.rider_desks > 0
            && c.unit_def(unit).is_some()
    }
}

struct BoardroomRt {
    /// Shared, because the step reads it every tick while it also borrows the runtime mutably.
    cfg: std::sync::Arc<BoardroomDef>,
    boss: EntityId,
    desks: u8,
    /// `None` until the boss reaches the phase of the mechanic; then the tick of the next one.
    next_earnings: Option<u64>,
    /// Bit `k` set: tile `k` is cleared.
    tiles: u16,
    next_tile: Option<u64>,
    froze: bool,
    next_inbox: u64,
    inbox_until: Option<u64>,
    next_parachute: Option<u64>,
    airborne_until: Option<u64>,
    /// The landing cast is up (the boss and, under the rider, the Heir).
    landing: bool,
    kicks: Vec<(EntityId, u64)>,
    /// Who kicked the boss's last landing: they may not kick the Heir's.
    last_pair: Vec<EntityId>,
    offboard_until: Option<u64>,
    sits_left: u8,
    heir: Option<EntityId>,
    /// (unit, deadline) of living invoiced consultants and apples.
    bills: Vec<(EntityId, u64)>,
    apples: Vec<(EntityId, u64)>,
    sent: (u8, u16, u16, u16),
}

#[derive(Default)]
pub(crate) struct Wing6State {
    rt: Option<BoardroomRt>,
}

impl BoardroomRt {
    fn desks_per_cast(&self, rider: bool) -> u8 {
        if rider { self.cfg.rider_desks } else { 1 }
    }
}

impl Sim {
    pub(crate) fn wing6_arm(&mut self, bi: usize, cfg: &BoardroomDef) {
        let boss = Self::id_of(bi);
        let desks = (cfg.earnings.len() - 1).min(255) as u8;
        let at = |ms: u32| self.now + ticks(ms);
        let rt = BoardroomRt {
            cfg: std::sync::Arc::new(cfg.clone()),
            boss,
            desks,
            next_earnings: None,
            tiles: 0,
            next_tile: None,
            froze: false,
            next_inbox: at(cfg.inbox_first_ms),
            inbox_until: None,
            next_parachute: None,
            airborne_until: None,
            landing: false,
            kicks: Vec::new(),
            last_pair: Vec::new(),
            offboard_until: None,
            sits_left: if self.raid.story { 0 } else { cfg.sits },
            heir: None,
            bills: Vec::new(),
            apples: Vec::new(),
            sent: (desks, 0, 0, 0),
        };
        self.events.push(Event::Boardroom {
            desks,
            inbox_ms: 0,
            severance_ms: 0,
            tiles: 0,
        });
        self.wing6.rt = Some(rt);
    }

    pub(crate) fn wing6_disarm(&mut self) {
        if let Some(rt) = self.wing6.rt.take() {
            self.cancel_telegraphs_of(rt.boss);
            if rt.sent != (0, 0, 0, 0) {
                self.events.push(Event::Boardroom {
                    desks: 255,
                    inbox_ms: 0,
                    severance_ms: 0,
                    tiles: 0,
                });
            }
        }
    }

    /// A joiner is told the room as it stands.
    pub(crate) fn wing6_on_join(&mut self, player: EntityId) {
        if let Some(rt) = &self.wing6.rt {
            let e = Event::Boardroom {
                desks: rt.sent.0,
                inbox_ms: rt.sent.1,
                severance_ms: rt.sent.2,
                tiles: rt.sent.3,
            };
            self.emit_to(player, e);
        }
    }

    pub(crate) fn update_wing6(&mut self) {
        let Some(mut rt) = self.wing6.rt.take() else {
            return;
        };
        self.wing6_step(&mut rt);
        self.wing6.rt = Some(rt);
    }

    fn rider_up(&self, bi: usize, rt: &BoardroomRt) -> bool {
        rt.cfg
            .rider
            .as_deref()
            .is_some_and(|r| self.has_aura(bi, r))
    }

    fn yell_from(&mut self, bi: usize, lines: &[String], kind: YellKind) {
        if lines.is_empty() {
            return;
        }
        let n = lines.len() as u32;
        let line = lines[self.rng.below(n) as usize].clone();
        self.request_yell_line(bi, kind, Some(line));
    }

    /// Living humans: the group that decides how many kicks the landing needs.
    fn living_players(&self) -> Vec<EntityId> {
        self.players
            .iter()
            .copied()
            .filter(|&p| self.index(p).is_some_and(|i| self.units[i].state.alive))
            .collect()
    }

    fn wing6_step(&mut self, rt: &mut BoardroomRt) {
        let boss = rt.boss;
        let Some(bi) = self.index(boss).filter(|&b| self.units[b].state.alive) else {
            return;
        };
        let story = self.raid.story;
        let phase = self.units[bi].state.phase;
        let rider = self.rider_up(bi, rt);
        let busy = self.units[bi].cast.is_some();
        let sitting = rt.offboard_until.is_some();
        let calm = !sitting && rt.airborne_until.is_none();
        let cfg = rt.cfg.clone();

        // Judgment's Heir joins with the second act.
        if let Some(h) = cfg.heir.as_ref()
            && rider
            && !story
            && rt.heir.is_none()
            && phase >= h.phase
            && let Some(spell) = self.content.spell(h.summon).cloned()
        {
            self.resolve(bi, &spell, Some(boss));
            rt.heir = self
                .units
                .iter()
                .position(|u| {
                    u.summoner == Some(boss)
                        && u.state.alive
                        && self.content.units[u.def].key == h.unit
                })
                .map(Self::id_of);
            self.yell_from(bi, &h.yells, YellKind::Phase);
        }

        self.wing6_adds(rt, bi);

        // Earnings Call, Live: the cover is whatever desks are left, and each call takes one away.
        let now = self.now;
        if phase >= cfg.earnings_phase && rt.next_earnings.is_none() {
            rt.next_earnings = Some(now + ticks(cfg.earnings_first_ms));
        }
        if phase >= cfg.tiles_phase && rt.next_tile.is_none() {
            rt.next_tile = Some(now + ticks(cfg.tiles_first_ms));
        }
        if phase >= cfg.parachute_phase && rt.next_parachute.is_none() {
            rt.next_parachute = Some(now + ticks(cfg.parachute_first_ms));
        }
        if calm && rt.next_earnings.is_some_and(|t| now >= t) && !busy {
            let k = usize::from(rt.desks).min(cfg.earnings.len() - 1);
            if let Some(spell) = self.content.spell(cfg.earnings[k]).cloned() {
                self.resolve(bi, &spell, Some(boss));
                self.yell_from(bi, &cfg.earnings_yells, YellKind::Blast);
            }
            rt.desks = rt.desks.saturating_sub(rt.desks_per_cast(rider));
            rt.next_earnings = Some(self.now + ticks(cfg.earnings_ms));
        }

        // Right-Sizing: floor tiles are cleared for good, never the centre one; the freeze
        // brings two back once.
        let max = u32::from(if story {
            cfg.story_tiles_max
        } else {
            cfg.tiles_max
        });
        if calm && rt.next_tile.is_some_and(|t| now >= t) && !busy && rt.tiles.count_ones() < max {
            let free: Vec<usize> = (0..9)
                .filter(|&k| k != 4 && rt.tiles & (1 << k) == 0)
                .collect();
            if !free.is_empty() {
                let k = free[self.rng.below(free.len() as u32) as usize];
                rt.tiles |= 1 << k;
                if let Some(spell) = self.content.spell(cfg.tiles[k]).cloned() {
                    self.resolve(bi, &spell, Some(boss));
                }
                self.yell_from(bi, &cfg.tile_yells, YellKind::Blast);
            }
            rt.next_tile = Some(self.now + ticks(cfg.tiles_ms));
        }
        let s = &self.units[bi].state;
        if !rt.froze && s.hp * 100 <= s.max_hp * cfg.freeze_below_pct && rt.tiles != 0 {
            rt.froze = true;
            for _ in 0..2 {
                if let Some(k) = (0..9).find(|&k| rt.tiles & (1 << k) != 0) {
                    rt.tiles &= !(1 << k);
                    self.wing6_drop_tile(rt, cfg.tiles[k]);
                }
            }
            self.yell_from(bi, &cfg.freeze_yells, YellKind::Phase);
        }

        // The inbox: a message, a few seconds, a terminal; the party that misses it is Notified.
        match rt.inbox_until {
            None if self.now >= rt.next_inbox && !sitting => {
                let ms = if story {
                    cfg.story_window_ms
                } else {
                    cfg.window_ms
                };
                rt.inbox_until = Some(self.now + ticks(ms));
                self.yell_from(bi, &cfg.inbox_yells, YellKind::Blast);
            }
            Some(until) if self.now >= until => {
                rt.inbox_until = None;
                rt.next_inbox = self.now + ticks(cfg.inbox_ms);
                for p in self.living_players() {
                    if let Some(i) = self.index(p) {
                        self.apply_aura(i, &cfg.notified_aura, cfg.notified_ms, None, boss, None);
                    }
                }
            }
            _ => {}
        }

        self.wing6_parachute(rt, bi, rider);

        // Offboarding: he sits, the severance fills; the last 5% before it does ends him.
        let s = &self.units[bi].state;
        let low = s.hp * 100 <= s.max_hp * cfg.offboard_below_pct;
        match rt.offboard_until {
            None if low && rt.sits_left > 0 && calm && !busy => {
                rt.offboard_until = Some(self.now + ticks(cfg.severance_ms));
                self.apply_aura(bi, &cfg.offboard_aura, cfg.severance_ms, None, boss, None);
                self.yell_from(bi, &cfg.offboard_yells, YellKind::Phase);
            }
            Some(until) if self.now >= until => {
                rt.offboard_until = None;
                rt.sits_left -= 1;
                let by = self.units[bi].state.max_hp * cfg.heal_pct / 100;
                self.heal(boss, bi, None, by);
                self.yell_from(bi, &cfg.severance_yells, YellKind::Phase);
            }
            _ => {}
        }

        self.wing6_sync(rt);
    }

    /// Invoices and apples: scan the boss's adds, start their clocks, settle the expired ones.
    fn wing6_adds(&mut self, rt: &mut BoardroomRt, bi: usize) {
        let boss = rt.boss;
        let cfg = rt.cfg.clone();
        for (key, is_bill) in [(&cfg.invoice_unit, true), (&cfg.apple_unit, false)] {
            let ms = if is_bill {
                cfg.invoice_ms
            } else {
                cfg.apple_ms
            };
            let live: Vec<EntityId> = self
                .units
                .iter()
                .enumerate()
                .filter(|(_, u)| {
                    !u.despawned
                        && u.state.alive
                        && u.summoner == Some(boss)
                        && self.content.units[u.def].key == *key
                })
                .map(|(i, _)| Self::id_of(i))
                .collect();
            let now = self.now;
            let list = if is_bill {
                &mut rt.bills
            } else {
                &mut rt.apples
            };
            list.retain(|(id, _)| live.contains(id));
            for id in &live {
                if !list.iter().any(|(k, _)| k == id) {
                    list.push((*id, now + ticks(ms)));
                }
            }
            let due: Vec<EntityId> = list
                .iter()
                .filter(|(_, at)| *at <= now)
                .map(|(id, _)| *id)
                .collect();
            list.retain(|(_, at)| *at > now);
            for id in due {
                let Some(i) = self.index(id) else { continue };
                self.despawn(i);
                if is_bill {
                    let by = i32::from(cfg.invoice_burnout);
                    for p in self.living_players() {
                        if let Some(pi) = self.index(p) {
                            self.add_burnout(pi, by, BurnoutCause::Collateral);
                        }
                    }
                    self.yell_from(bi, &cfg.invoice_yells, YellKind::Blast);
                } else {
                    self.apply_aura(bi, &cfg.apple_aura, LONG_MS, None, boss, None);
                }
            }
        }
    }

    /// Golden Parachute: up, untouchable, a rain of payments; down with a landing that needs the
    /// kicks (the Heir lands a few seconds after, on one kick from a fresh pair of hands).
    fn wing6_parachute(&mut self, rt: &mut BoardroomRt, bi: usize, rider: bool) {
        let (boss, cfg) = (rt.boss, rt.cfg.clone());
        let busy = self.units[bi].cast.is_some();
        match rt.airborne_until {
            None if rt.next_parachute.is_some_and(|t| self.now >= t)
                && rt.offboard_until.is_none()
                && !busy =>
            {
                let ms = cfg.parachute_ms;
                rt.airborne_until = Some(self.now + ticks(cfg.airborne_ms));
                rt.next_parachute = Some(self.now + ticks(ms));
                rt.landing = false;
                for s in [cfg.parachute, cfg.rain] {
                    if let Some(spell) = self.content.spell(s).cloned() {
                        self.resolve(bi, &spell, Some(boss));
                    }
                }
                if let Some(h) = rt.heir.and_then(|h| self.index(h)) {
                    self.apply_aura(h, &cfg.airborne_aura, cfg.airborne_ms, None, boss, None);
                }
                self.yell_from(bi, &cfg.parachute_yells, YellKind::Blast);
            }
            Some(until) if self.now >= until && !rt.landing => {
                rt.landing = true;
                if let Some(spell) = self.content.spell(cfg.landing).cloned() {
                    self.remove_aura(bi, &cfg.airborne_aura, AuraEnd::Expired);
                    self.start_cast(bi, &spell, Some(boss));
                }
                if let (Some(h), Some(hs)) = (
                    rt.heir.and_then(|h| self.index(h)),
                    cfg.heir
                        .as_ref()
                        .and_then(|h| self.content.spell(h.landing).cloned()),
                ) && rider
                {
                    self.remove_aura(h, &cfg.airborne_aura, AuraEnd::Expired);
                    let hid = Self::id_of(h);
                    self.start_cast(h, &hs, Some(hid));
                }
            }
            Some(_) if rt.landing => {
                let heir_busy = rt
                    .heir
                    .and_then(|h| self.index(h))
                    .is_some_and(|h| self.units[h].cast.is_some());
                if !busy && !heir_busy {
                    rt.airborne_until = None;
                    rt.landing = false;
                    rt.kicks.clear();
                }
            }
            _ => {}
        }
    }

    fn wing6_drop_tile(&mut self, rt: &BoardroomRt, spell: SpellId) {
        let mut gone = Vec::new();
        self.telegraphs.retain(|t| {
            let keep = !(t.source == rt.boss && t.spell == spell);
            if !keep {
                gone.push(t.id);
            }
            keep
        });
        for id in gone {
            self.events
                .push(Event::TelegraphResolved { id, landed: false });
        }
    }

    fn wing6_sync(&mut self, rt: &mut BoardroomRt) {
        let left = |until: Option<u64>, now: u64| {
            until.map_or(0, |u| {
                (crate::time::ticks_to_ms(u.saturating_sub(now)) as u32).min(60_000) as u16
            })
        };
        // Whole seconds only, or every tick would be a new event.
        let round = |ms: u16| ms.div_ceil(1000) * 1000;
        let state = (
            rt.desks,
            round(left(rt.inbox_until, self.now)),
            round(left(rt.offboard_until, self.now)),
            rt.tiles,
        );
        if state != rt.sent {
            rt.sent = state;
            self.events.push(Event::Boardroom {
                desks: state.0,
                inbox_ms: state.1,
                severance_ms: state.2,
                tiles: state.3,
            });
        }
    }

    /// A kick on a cast that needs the pair. True when the cast goes on (the kick is held: its
    /// partner has not come in the window, or the Heir's kicker already kicked the boss).
    pub(crate) fn wing6_hold_interrupt(&mut self, ti: usize, by: EntityId) -> bool {
        let Some(mut rt) = self.wing6.rt.take() else {
            return false;
        };
        let held = self.wing6_kick(&mut rt, ti, by);
        self.wing6.rt = Some(rt);
        held
    }

    fn wing6_kick(&mut self, rt: &mut BoardroomRt, ti: usize, by: EntityId) -> bool {
        let Some(cast) = self.units[ti].cast.as_ref().map(|c| c.spell) else {
            return false;
        };
        let who = Self::id_of(ti);
        let is_heir =
            Some(who) == rt.heir && rt.cfg.heir.as_ref().is_some_and(|h| h.landing == cast);
        if !is_heir && cast != rt.cfg.landing {
            return false;
        }
        let need: u8 = if is_heir {
            1
        } else if !self.raid.story {
            // Full is the group difficulty: even a lone player faces the pair.
            2
        } else {
            1
        };
        if is_heir && rt.last_pair.contains(&by) {
            self.events.push(Event::PairKick {
                boss: who,
                by,
                have: 0,
                need,
            });
            return true;
        }
        if need == 1 {
            if !is_heir {
                rt.last_pair = vec![by];
            }
            return false;
        }
        let (now, window) = (self.now, ticks(rt.cfg.pair_window_ms));
        rt.kicks.retain(|(_, at)| now < *at + window);
        if let Some((other, _)) = rt.kicks.iter().find(|(p, _)| *p != by).copied() {
            rt.last_pair = vec![other, by];
            rt.kicks.clear();
            return false;
        }
        rt.kicks.retain(|(p, _)| *p != by);
        rt.kicks.push((by, now));
        self.events.push(Event::PairKick {
            boss: who,
            by,
            have: 1,
            need,
        });
        true
    }

    /// An apple killed by a player heals them.
    pub(crate) fn wing6_on_death(&mut self, ti: usize, killer: Option<EntityId>) {
        let Some(rt) = &self.wing6.rt else { return };
        let key = &self.content.units[self.units[ti].def].key;
        if *key != rt.cfg.apple_unit || self.units[ti].summoner != Some(rt.boss) {
            return;
        }
        let pct = rt.cfg.apple_heal_pct;
        let boss = rt.boss;
        let Some(k) = killer
            .filter(|&k| self.is_player(k))
            .and_then(|k| self.index(k))
        else {
            return;
        };
        let by = self.units[k].state.max_hp * pct / 100;
        self.heal(boss, k, None, by);
    }

    /// An inbox terminal: any class, one clear for the whole party.
    pub(crate) fn use_inbox_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        let mut outcome = 0;
        if let Some(rt) = &mut self.wing6.rt
            && rt.inbox_until.is_some()
        {
            rt.inbox_until = None;
            rt.next_inbox = self.now + ticks(rt.cfg.inbox_ms);
            outcome = crate::world::PROP_OK;
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key,
            outcome,
        });
    }
}
