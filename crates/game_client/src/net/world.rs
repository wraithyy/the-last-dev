//! `NetWorld`: the client's picture of the server's room, rebuilt from `Snapshot`s (which
//! units exist, where, HP) and `Events` (everything between snapshots). It implements
//! `SimRead`, so the HUD, nameplates, VFX and audio read it exactly like the offline sim.
//!
//! Other units move along a buffer of `Positions` (10 Hz) and snapshot positions, rendered
//! `INTERP_DELAY_S` plus one update interval in the past. The local player is predicted: its
//! body moves at once and `MoveCorrection` shifts it. Aura timers and the server tick count
//! down locally between the server's updates.

use crate::combat::SimRead;
use bevy::prelude::Resource;
use combat_core::{
    ActiveQuest, AiState, AuraView, CastError, CastView, Content, CooldownView, EntityId, Event,
    ItemTotals, Objective, QuestLog, QuestStatus, SpellId, SpellTarget, TICK_HZ, Tick, UnitState,
    Unlock, Vec3, ZoneLinkView,
};
use net_proto::{
    AuraSnap, PlayerInfo, PlayerSnap, QuestSnap, UnitSnap, from_cm, sanitize_name, yaw_from_byte,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// Extra render delay on top of the snapshot interval, to absorb network jitter.
pub const INTERP_DELAY_S: f64 = 0.1;
/// Snapshot positions kept per unit; a few intervals cover any sane render delay.
const INTERP_SAMPLES: usize = 6;
/// Moves remembered for corrections; at 15 Hz this is several seconds of round trip.
const MOVE_HISTORY: usize = 64;
/// Until two `Positions` frames arrive, assume the server's 10 Hz cadence.
const DEFAULT_UPDATE_INTERVAL_S: f64 = 0.1;
/// Gaps longer than this between two `Positions` frames mean nobody moved, not a slow link.
const MAX_UPDATE_GAP_S: f64 = 0.5;
/// The locally advanced tick never runs further than this ahead of the last server frame.
const MAX_TICK_LEAD_S: f64 = 2.0;
/// Until the first Pong: a transatlantic-ish guess, so early cast bars are not far behind.
const DEFAULT_RTT_S: f64 = 0.15;
/// A predicted cast the server never answered (dropped by the rate limiter) is let go after this.
const PREDICTION_TIMEOUT_S: f64 = 2.0;

/// A cast shown before the server confirmed it (see `NetWorld::predict_cast`).
#[derive(Clone, Copy, Debug)]
struct Prediction {
    spell: SpellId,
    at: f64,
    /// The GCD before the prediction started its own, restored if the server refuses.
    gcd_before: (u64, u32),
    bar: bool,
}

pub fn lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    Vec3::new(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.z + (b.z - a.z) * t,
    )
}

/// Blends along the shorter arc, so 350 -> 10 degrees turns 20 degrees, not 340.
pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let d = (b - a).rem_euclid(tau);
    let d = if d > tau / 2.0 { d - tau } else { d };
    a + d * t
}

/// Timestamped positions of one remote unit.
#[derive(Default, Debug)]
pub struct Interp {
    samples: VecDeque<(f64, Vec3, f32)>,
}

impl Interp {
    /// `step` is the expected update interval. A unit that stood still sent nothing for a
    /// while; its old sample is repeated one step before the new one, so it starts walking
    /// from there instead of gliding over the whole silent gap.
    pub fn push(&mut self, t: f64, pos: Vec3, yaw: f32, step: f64) {
        match self.samples.back().copied() {
            Some((last, ..)) if t < last => self.samples.clear(),
            // Two frames handled at the same clock reading: the newer one wins.
            Some((last, ..)) if t == last => {
                self.samples.pop_back();
            }
            Some((last, p, y)) if t - last > step * 1.5 => {
                self.samples.push_back(((t - step).max(last), p, y));
            }
            _ => {}
        }
        self.samples.push_back((t, pos, yaw));
        while self.samples.len() > INTERP_SAMPLES {
            self.samples.pop_front();
        }
    }

    /// A teleport must not glide.
    pub fn reset(&mut self, t: f64, pos: Vec3, yaw: f32) {
        self.samples.clear();
        self.samples.push_back((t, pos, yaw));
    }

    /// Position at render time `t`: held at the ends, never extrapolated.
    pub fn sample(&self, t: f64) -> Option<(Vec3, f32)> {
        let first = self.samples.front()?;
        if t <= first.0 {
            return Some((first.1, first.2));
        }
        for (a, b) in self.samples.iter().zip(self.samples.iter().skip(1)) {
            if t <= b.0 {
                let k = ((t - a.0) / (b.0 - a.0).max(1e-6)) as f32;
                return Some((lerp(a.1, b.1, k), lerp_angle(a.2, b.2, k)));
            }
        }
        self.samples.back().map(|s| (s.1, s.2))
    }
}

/// Client-side prediction bookkeeping: what was reported with which `seq`.
#[derive(Default, Debug)]
pub struct MoveHistory {
    next_seq: u32,
    sent: VecDeque<(u32, Vec3)>,
}

impl MoveHistory {
    pub fn record(&mut self, pos: Vec3) -> u32 {
        self.next_seq = self.next_seq.wrapping_add(1);
        self.sent.push_back((self.next_seq, pos));
        while self.sent.len() > MOVE_HISTORY {
            self.sent.pop_front();
        }
        self.next_seq
    }

    /// The server put move `seq` at `server_pos`. Returns how far to shift the body: the
    /// error of that move, which every later move inherited. Replaying later inputs from the
    /// corrected spot is the same shift as long as no wall is in between; walls are rechecked
    /// by the body's next collision step anyway. Unknown (too old) `seq`s snap fully.
    pub fn correct(&mut self, seq: u32, server_pos: Vec3, current: Vec3) -> Vec3 {
        let sub = |a: Vec3, b: Vec3| Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z);
        let delta = match self.sent.iter().find(|(s, _)| *s == seq) {
            Some((_, sent)) => sub(server_pos, *sent),
            None => sub(server_pos, current),
        };
        self.sent.retain(|(s, _)| s.wrapping_sub(seq) as i32 > 0);
        for (_, p) in &mut self.sent {
            *p = Vec3::new(p.x + delta.x, p.y + delta.y, p.z + delta.z);
        }
        delta
    }
}

/// Snapshot cadence estimate, smoothed so one late frame does not jolt every unit.
fn smooth_interval(old: f64, measured: f64) -> f64 {
    let m = measured.clamp(0.03, 2.0);
    old + (m - old) * 0.3
}

fn objective_need(o: &Objective) -> u32 {
    match o {
        Objective::Kill(_, n) | Objective::Loot(_, n) => *n,
        Objective::Talk(_) | Objective::ClearZone(_) => 1,
    }
}

fn ms_to_ticks(ms: u32) -> u64 {
    u64::from(ms) * u64::from(TICK_HZ) / 1000
}

#[derive(Resource)]
pub struct NetWorld {
    content: Content,
    pub you: EntityId,
    pub room: String,
    /// Display names by player id, already sanitised by the server.
    pub names: BTreeMap<EntityId, String>,
    players: Vec<EntityId>,
    units: BTreeMap<EntityId, UnitState>,
    interp: HashMap<EntityId, Interp>,
    update_interval: f64,
    last_positions_at: Option<f64>,
    /// Last server tick and when it arrived, for advancing `tick` between frames.
    tick_at: (u64, f64),
    last_advance: Option<f64>,
    /// Smoothed round trip to the server from Ping/Pong, seconds.
    rtt_s: f64,
    quests: QuestLog,
    cleared: BTreeSet<String>,
    bosses_killed: BTreeSet<String>,
    unlocked: BTreeSet<String>,
    tick: u64,
    cooldowns: HashMap<SpellId, (u64, u32)>,
    gcd: (u64, u32),
    /// Server events not yet handed to the presentation step.
    inbox: Vec<Event>,
    pub moves: MoveHistory,
    /// Body shift from `MoveCorrection`s, taken by the next tick.
    correction: Option<Vec3>,
    /// What the last `SetTarget` asked for, so it is sent once per change.
    pub target_sent: Option<Option<EntityId>>,
    pub move_ticks: u32,
    /// Last movement report (pos, yaw, moving, jumping): an idle player sends nothing.
    pub last_move: Option<(Vec3, f32, bool, bool)>,
    prediction: Option<Prediction>,
    /// Corpses holding this player's own loot roll (from `LootDropped`, which only this
    /// player receives); the snapshot's lootable flag is about anybody's roll.
    my_loot: BTreeSet<EntityId>,
}

impl NetWorld {
    pub fn new(content: Content, you: EntityId, room: String, players: &[PlayerInfo]) -> Self {
        let mut w = NetWorld {
            content,
            you,
            room,
            names: BTreeMap::new(),
            players: Vec::new(),
            units: BTreeMap::new(),
            interp: HashMap::new(),
            update_interval: DEFAULT_UPDATE_INTERVAL_S,
            last_positions_at: None,
            tick_at: (0, 0.0),
            last_advance: None,
            rtt_s: DEFAULT_RTT_S,
            quests: QuestLog::default(),
            cleared: BTreeSet::new(),
            bosses_killed: BTreeSet::new(),
            unlocked: BTreeSet::new(),
            tick: 0,
            cooldowns: HashMap::new(),
            gcd: (0, 0),
            inbox: Vec::new(),
            moves: MoveHistory::default(),
            correction: None,
            target_sent: None,
            move_ticks: 0,
            last_move: None,
            prediction: None,
            my_loot: BTreeSet::new(),
        };
        for p in players {
            w.add_player(p);
        }
        if !w.players.contains(&you) {
            w.players.insert(0, you);
        }
        w
    }

    /// The `Welcome` state snapshots do not carry: this player's quest log and the zones the
    /// group cleared or opened before the join.
    pub fn apply_welcome(&mut self, quests: &QuestSnap, cleared: &[String], unlocked: &[String]) {
        self.quests = quests.to_log();
        self.cleared.extend(cleared.iter().cloned());
        self.unlocked.extend(unlocked.iter().cloned());
    }

    fn server_tick(&mut self, tick: u64, now: f64) {
        if tick >= self.tick_at.0 {
            self.tick_at = (tick, now);
        }
        self.tick = self.tick.max(tick);
    }

    /// Replaces a unit's auras with the server's list (timers included).
    fn set_auras(&mut self, id: EntityId, list: &[AuraSnap]) {
        let content = &self.content;
        let Some(u) = self.units.get_mut(&id) else {
            return;
        };
        u.auras = list.iter().filter_map(|a| a.to_view(content)).collect();
        let has = |f: fn(&combat_core::AuraDef) -> bool| {
            u.auras.iter().any(|a| content.aura(&a.key).is_some_and(f))
        };
        u.stunned = has(|d| d.stun);
        u.silenced = has(|d| d.silence);
    }

    pub fn apply_auras(&mut self, units: &[(EntityId, Vec<AuraSnap>)]) {
        for (id, list) in units {
            self.set_auras(*id, list);
        }
    }

    /// Authoritative cooldowns of this player's spells after a cast (server tick `tick`).
    pub fn apply_cooldowns(&mut self, tick: u64, gcd: (u32, u32), spells: &[(SpellId, u32, u32)]) {
        self.gcd = (tick + ms_to_ticks(gcd.0), gcd.1);
        self.cooldowns = spells
            .iter()
            .map(|(s, left, total)| (*s, (tick + ms_to_ticks(*left), *total)))
            .collect();
    }

    /// 10 Hz movement of the units that moved; the snapshot keeps everything else.
    pub fn apply_positions(&mut self, tick: u64, units: &[(EntityId, [i32; 3], u8)], now: f64) {
        if let Some(last) = self.last_positions_at
            && now - last < MAX_UPDATE_GAP_S
        {
            self.update_interval = smooth_interval(self.update_interval, now - last);
        }
        self.last_positions_at = Some(now);
        self.server_tick(tick, now);
        let step = self.update_interval;
        for (id, cm, facing) in units {
            if *id == self.you || !self.units.contains_key(id) {
                continue;
            }
            self.interp.entry(*id).or_default().push(
                now,
                from_cm(*cm),
                yaw_from_byte(*facing),
                step,
            );
        }
    }

    pub fn add_player(&mut self, p: &PlayerInfo) {
        // A modified or compromised server must not feed arbitrary text into the HUD.
        let name = sanitize_name(&p.name, u32::try_from(p.id).unwrap_or(0));
        if let Some(u) = self.units.get_mut(&p.id) {
            u.name.clone_from(&name);
        }
        self.names.insert(p.id, name);
        if !self.players.contains(&p.id) {
            self.players.push(p.id);
        }
    }

    pub fn remove_player(&mut self, id: EntityId) {
        self.players.retain(|p| *p != id);
        self.names.remove(&id);
        self.units.remove(&id);
        self.interp.remove(&id);
    }

    pub fn name_of(&self, id: EntityId) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    pub fn take_inbox(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.inbox)
    }

    pub fn take_correction(&mut self) -> Option<Vec3> {
        self.correction.take()
    }

    /// Server-side emotes become yells: same bubble, same combat-log line.
    pub fn push_presentation(&mut self, ev: Event) {
        self.inbox.push(ev);
    }

    pub fn correct_move(&mut self, seq: u32, server_pos: Vec3, current: Vec3) {
        let d = self.moves.correct(seq, server_pos, current);
        let acc = self.correction.unwrap_or_default();
        self.correction = Some(Vec3::new(acc.x + d.x, acc.y + d.y, acc.z + d.z));
    }

    fn fresh_unit(&self, s: &UnitSnap) -> Option<UnitState> {
        let d = self.content.units.get(usize::from(s.key_idx))?;
        let name = self
            .names
            .get(&s.id)
            .cloned()
            .unwrap_or_else(|| d.name.clone());
        Some(UnitState {
            id: s.id,
            key: d.key.clone(),
            name,
            level: s.level,
            hostile: d.hostile,
            pos: s.pos(),
            yaw: s.yaw(),
            hp: s.hp,
            max_hp: s.max_hp,
            mana: d.max_mana,
            max_mana: d.max_mana,
            alive: true,
            in_combat: false,
            target: None,
            cast: None,
            auras: Vec::new(),
            ai: d.ai.as_ref().map(|_| AiState::Idle),
            lootable: false,
            respawn_in_ms: None,
            xp: if d.hostile { d.xp_reward } else { 0 },
            auto_attack: false,
            swing_remaining_ms: 0,
            xp_to_next: 0,
            stunned: false,
            silenced: false,
            boss: d.boss,
            pack: None,
            phase: 1,
            idle_pose: d.idle_pose,
            inventory: Vec::new(),
        })
    }

    /// A snapshot lists every unit of the zone, so anything missing from it is gone.
    pub fn apply_snapshot(
        &mut self,
        tick: u64,
        units: &[UnitSnap],
        players: &[PlayerSnap],
        now: f64,
    ) {
        self.server_tick(tick, now);
        let step = self.update_interval;
        let listed: BTreeSet<EntityId> = units.iter().map(|s| s.id).collect();
        self.units.retain(|id, _| listed.contains(id));
        self.interp.retain(|id, _| listed.contains(id));
        for s in units {
            if !self.units.contains_key(&s.id) {
                let Some(fresh) = self.fresh_unit(s) else {
                    continue;
                };
                self.units.insert(s.id, fresh);
                self.interp
                    .entry(s.id)
                    .or_default()
                    .reset(now, s.pos(), s.yaw());
            } else {
                self.interp
                    .entry(s.id)
                    .or_default()
                    .push(now, s.pos(), s.yaw(), step);
            }
            self.set_auras(s.id, &s.auras);
            let Some(u) = self.units.get_mut(&s.id) else {
                continue;
            };
            u.level = s.level;
            u.hp = s.hp;
            u.max_hp = s.max_hp;
            u.target = s.target;
            u.alive = s.has(UnitSnap::ALIVE);
            u.in_combat = s.has(UnitSnap::IN_COMBAT);
            u.stunned = s.has(UnitSnap::STUNNED);
            if !s.has(UnitSnap::LOOTABLE) || u.alive {
                self.my_loot.remove(&s.id);
            }
            u.lootable = !u.alive && self.my_loot.contains(&s.id);
            u.auto_attack = s.has(UnitSnap::AUTO_ATTACK);
            // This snapshot may predate the server receiving our cast.
            let predicted = s.id == self.you && self.prediction.is_some_and(|p| p.bar);
            if !s.has(UnitSnap::CASTING) && !predicted {
                u.cast = None;
            }
            if s.has(UnitSnap::EVADING) {
                u.ai = Some(AiState::Evade);
            }
            if s.id == self.you {
                // Prediction owns the body; this is only the server's last word for respawns.
                u.pos = s.pos();
                if self.target_sent.is_some_and(|t| t != s.target) {
                    // The server changed it (death, despawn): resend if the player still wants one.
                    self.target_sent = Some(s.target);
                }
            }
        }
        for p in players {
            if let Some(u) = self.units.get_mut(&p.id) {
                u.mana = p.mana;
                u.max_mana = p.max_mana;
                u.xp = p.xp;
                u.xp_to_next = p.xp_to_next;
            }
        }
    }

    /// Moves every remote unit to its interpolated position for render time `now`, and runs
    /// the clocks the server only corrects now and then: aura timers and the tick that
    /// cooldowns count against.
    pub fn advance(&mut self, now: f64) {
        let since = (now - self.tick_at.1).clamp(0.0, MAX_TICK_LEAD_S);
        self.tick = self
            .tick
            .max(self.tick_at.0 + (since * f64::from(TICK_HZ)) as u64);
        let dt_ms = self
            .last_advance
            .map_or(0, |last| ((now - last).max(0.0) * 1000.0) as u32);
        self.last_advance = Some(now);
        if let Some(p) = self.prediction
            && now - p.at > PREDICTION_TIMEOUT_S + self.rtt_s
        {
            self.drop_prediction(p);
        }
        if dt_ms > 0 {
            for u in self.units.values_mut() {
                for a in u.auras.iter_mut().filter(|a| a.total_ms > 0) {
                    a.remaining_ms = a.remaining_ms.saturating_sub(dt_ms);
                }
                // Casts only get start and end events; the bar fills locally in between.
                if let Some(c) = u.cast.as_mut() {
                    c.elapsed_ms = (c.elapsed_ms + dt_ms).min(c.total_ms);
                }
            }
        }
        let t = now - (INTERP_DELAY_S + self.update_interval);
        for (id, interp) in &self.interp {
            if *id == self.you {
                continue;
            }
            if let (Some(u), Some((pos, yaw))) = (self.units.get_mut(id), interp.sample(t)) {
                u.pos = pos;
                u.yaw = yaw;
            }
        }
    }

    /// Client-side prediction for this player's own cast: when the local picture says the
    /// spell can go (learned, off cooldown and GCD, affordable, standing still for a cast
    /// time, a plausible target), the cast bar and the GCD sweep start now instead of one
    /// round trip later. The server's `CastStarted` takes over; its `CastFailed` undoes it.
    pub fn predict_cast(
        &mut self,
        spell: SpellId,
        moving: bool,
        target: Option<EntityId>,
        now: f64,
    ) {
        let you = self.you;
        let Some(d) = self.content.spell(spell) else {
            return;
        };
        if self.cooldown(you, spell).usable.is_err() || self.prediction.is_some() {
            return;
        }
        let Some(me) = self.units.get(&you) else {
            return;
        };
        let target_ok = match d.target {
            SpellTarget::Enemy => target
                .and_then(|t| self.units.get(&t))
                .is_some_and(|t| t.hostile && t.alive),
            SpellTarget::DeadAlly => {
                !me.in_combat
                    && target
                        .filter(|t| *t != you && self.players.contains(t))
                        .and_then(|t| self.units.get(&t))
                        .is_some_and(|t| !t.alive)
            }
            SpellTarget::Caster | SpellTarget::Ally => true,
        };
        let busy = me.cast.is_some() && d.gcd;
        if !target_ok || busy || me.silenced {
            return;
        }
        let bar = d.cast_ms > 0 && !moving && (d.channel.is_none() || me.mana < me.max_mana);
        if d.cast_ms > 0 && !bar {
            return;
        }
        let gcd_before = self.gcd;
        if d.gcd {
            let gcd = self.content.gcd_ms(0);
            self.gcd = (self.tick + ms_to_ticks(gcd), gcd);
        }
        if bar {
            let total_ms = match d.channel {
                Some(_) => d.cast_ms,
                None => self.content.cast_ms(d, 0),
            };
            let interruptible = d.interruptible;
            if let Some(u) = self.units.get_mut(&you) {
                u.cast = Some(CastView {
                    spell,
                    elapsed_ms: 0,
                    total_ms,
                    interruptible,
                });
            }
        }
        if bar || d.gcd {
            self.prediction = Some(Prediction {
                spell,
                at: now,
                gcd_before,
                bar,
            });
        }
    }

    fn drop_prediction(&mut self, p: Prediction) {
        self.prediction = None;
        self.gcd = p.gcd_before;
        if p.bar
            && let Some(u) = self.units.get_mut(&self.you)
            && u.cast.is_some_and(|c| c.spell == p.spell)
        {
            u.cast = None;
        }
    }

    /// `sent_ms` is the client clock (ms) echoed back in `Pong`.
    pub fn record_pong(&mut self, sent_ms: u64, now: f64) {
        let rtt = now - sent_ms as f64 / 1000.0;
        if (0.0..5.0).contains(&rtt) {
            self.rtt_s = 0.7 * self.rtt_s + 0.3 * rtt;
        }
    }

    pub fn receive_events(&mut self, tick: u64, events: Vec<Event>, now: f64) {
        self.server_tick(tick, now);
        for ev in &events {
            self.apply_event(ev, now);
        }
        self.inbox.extend(events);
    }

    fn apply_event(&mut self, ev: &Event, now: f64) {
        let you = self.you;
        let content = &self.content;
        match ev {
            Event::Damage { target, amount, .. } => {
                if let Some(u) = self.units.get_mut(target) {
                    u.hp = u.hp.saturating_sub(*amount);
                }
            }
            Event::Heal { target, amount, .. } => {
                if let Some(u) = self.units.get_mut(target) {
                    u.hp = (u.hp + amount).min(u.max_hp);
                }
            }
            Event::ManaGain { unit, amount, .. } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.mana = (u.mana + amount).min(u.max_mana);
                }
            }
            Event::CastStarted {
                caster,
                spell,
                total_ms,
            } => {
                let def = content.spell(*spell);
                let confirmed =
                    *caster == you && self.prediction.take_if(|p| p.spell == *spell).is_some();
                if let Some(u) = self.units.get_mut(caster) {
                    // The server started it about one way of the round trip ago; a predicted
                    // bar is already further along and must not jump back.
                    let mut late_ms = (self.rtt_s * 500.0) as u32;
                    if confirmed && let Some(c) = u.cast.filter(|c| c.spell == *spell) {
                        late_ms = late_ms.max(c.elapsed_ms);
                    }
                    u.cast = Some(CastView {
                        spell: *spell,
                        elapsed_ms: late_ms.min(*total_ms),
                        total_ms: *total_ms,
                        interruptible: def.is_none_or(|d| d.interruptible),
                    });
                }
                if *caster == you && def.is_some_and(|d| d.gcd) {
                    let gcd = content.gcd_ms(0);
                    self.gcd = (self.tick + ms_to_ticks(gcd), gcd);
                }
            }
            Event::CastFailed { caster, spell, .. } if *caster == you => {
                if let Some(p) = self.prediction.filter(|p| p.spell == *spell) {
                    self.drop_prediction(p);
                }
            }
            Event::CastSucceeded { caster, spell, .. } => {
                if *caster == you {
                    self.prediction.take_if(|p| p.spell == *spell);
                }
                if let Some(u) = self.units.get_mut(caster)
                    && u.cast.is_some_and(|c| c.spell == *spell)
                {
                    u.cast = None;
                }
                if *caster == you
                    && let Some(d) = content.spell(*spell)
                {
                    if d.cooldown_ms > 0 {
                        self.cooldowns.insert(
                            *spell,
                            (self.tick + ms_to_ticks(d.cooldown_ms), d.cooldown_ms),
                        );
                    }
                    if d.gcd && d.cast_ms == 0 {
                        let gcd = content.gcd_ms(0);
                        self.gcd = (self.tick + ms_to_ticks(gcd), gcd);
                    }
                    if let Some(u) = self.units.get_mut(&you) {
                        u.mana = u.mana.saturating_sub(d.mana);
                    }
                }
            }
            Event::CastInterrupted { caster, .. } => {
                if let Some(u) = self.units.get_mut(caster) {
                    u.cast = None;
                }
            }
            Event::AuraApplied { target, key } => {
                let Some(def) = content.aura(key) else {
                    return;
                };
                let Some(u) = self.units.get_mut(target) else {
                    return;
                };
                match u.auras.iter_mut().find(|a| a.key == *key) {
                    Some(a) => a.stacks = (a.stacks + 1).min(def.max_stacks.max(1)),
                    None => u.auras.push(AuraView {
                        key: key.clone(),
                        name: def.name.clone(),
                        icon: def.icon.clone(),
                        harmful: def.harmful,
                        // The event carries no duration; the HUD shows the aura without a timer.
                        remaining_ms: 0,
                        total_ms: 0,
                        absorb_remaining: None,
                        stacks: 1,
                    }),
                }
                if def.stun {
                    u.stunned = true;
                }
                if def.silence {
                    u.silenced = true;
                }
            }
            Event::AuraRemoved { target, key, .. } => {
                if let Some(u) = self.units.get_mut(target) {
                    u.auras.retain(|a| a.key != *key);
                    let has = |f: fn(&combat_core::AuraDef) -> bool| {
                        u.auras.iter().any(|a| content.aura(&a.key).is_some_and(f))
                    };
                    u.stunned = has(|d| d.stun);
                    u.silenced = has(|d| d.silence);
                }
            }
            Event::EnterCombat { unit } | Event::LeaveCombat { unit } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.in_combat = matches!(ev, Event::EnterCombat { .. });
                }
            }
            Event::AiState { unit, state } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.ai = Some(*state);
                }
            }
            Event::AutoAttackToggled { unit, on } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.auto_attack = *on;
                }
            }
            Event::Death { unit, .. } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.alive = false;
                    u.hp = 0;
                    u.cast = None;
                    u.auras.clear();
                    u.in_combat = false;
                    u.auto_attack = false;
                    // Our own roll, if any, arrives as `LootDropped` right after.
                    u.lootable = false;
                    if u.boss {
                        self.bosses_killed.insert(u.key.clone());
                    }
                }
            }
            Event::Respawned { unit } => {
                self.my_loot.remove(unit);
                if let Some(u) = self.units.get_mut(unit) {
                    u.alive = true;
                    u.hp = u.max_hp;
                    u.mana = u.max_mana;
                    u.lootable = false;
                }
            }
            Event::Resurrected { unit, hp, mana, .. } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.alive = true;
                    u.hp = (*hp).min(u.max_hp);
                    u.mana = (*mana).min(u.max_mana);
                    u.lootable = false;
                }
            }
            Event::LootDropped { corpse, .. } => {
                self.my_loot.insert(*corpse);
                if let Some(c) = self.units.get_mut(corpse) {
                    c.lootable = !c.alive;
                }
            }
            Event::XpGained { unit, amount } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.xp += amount;
                }
            }
            Event::LevelUp { unit, level } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.level = *level;
                    u.xp = 0;
                    u.xp_to_next = content.xp_to_next(*level);
                }
            }
            Event::Looted {
                looter,
                corpse,
                items,
            } => {
                if *looter == you {
                    self.my_loot.remove(corpse);
                    for item in items {
                        self.add_item(item);
                    }
                    // This client's roll is claimed; the corpse may stay lootable for others.
                    if let Some(c) = self.units.get_mut(corpse) {
                        c.lootable = false;
                    }
                }
            }
            Event::Teleported { unit, pos } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.pos = *pos;
                    let yaw = u.yaw;
                    self.interp.entry(*unit).or_default().reset(now, *pos, yaw);
                }
            }
            Event::PhaseChanged { unit, phase } => {
                if let Some(u) = self.units.get_mut(unit) {
                    u.phase = *phase;
                }
            }
            Event::Despawned { unit } => {
                self.my_loot.remove(unit);
                self.units.remove(unit);
                self.interp.remove(unit);
            }
            Event::ZoneCleared { zone } => {
                self.cleared.insert(zone.clone());
            }
            Event::Unlocked { zone } => {
                self.unlocked.insert(zone.clone());
            }
            Event::QuestAccepted { quest } => {
                if let Some(def) = content.quest(quest)
                    && !self.quests.active.iter().any(|q| q.id == *quest)
                {
                    self.quests.active.push(ActiveQuest {
                        id: quest.clone(),
                        counts: vec![0; def.objectives.len()],
                    });
                }
            }
            Event::QuestProgress {
                quest,
                objective,
                count,
                ..
            } => {
                if let Some(c) = self
                    .quests
                    .active
                    .iter_mut()
                    .find(|q| q.id == *quest)
                    .and_then(|q| q.counts.get_mut(*objective))
                {
                    *c = *count;
                }
            }
            Event::QuestCompleted { quest } => {
                self.quests.active.retain(|q| q.id != *quest);
                if !self.quests.completed.contains(quest) {
                    self.quests.completed.push(quest.clone());
                }
                // The reward trophy is granted silently, like `grant_trophy`.
                if let Some(t) = content.quest(quest).and_then(|q| q.reward.trophy.clone()) {
                    self.add_item(&t);
                }
            }
            _ => {}
        }
    }

    fn add_item(&mut self, name: &str) {
        let Some(u) = self.units.get_mut(&self.you) else {
            return;
        };
        match u.inventory.iter_mut().find(|(n, _)| n == name) {
            Some((_, c)) => *c += 1,
            None => u.inventory.push((name.to_string(), 1)),
        }
    }

    fn unlock_met(&self, r: &Unlock) -> bool {
        match r {
            Unlock::ZoneCleared(z) => self.cleared.contains(z),
            Unlock::BossKilled(k) => self.bosses_killed.contains(k),
            Unlock::QuestDone(q) => self.quests.completed.contains(q),
        }
    }

    fn remaining_ms(&self, end: u64) -> u32 {
        let ticks = end.saturating_sub(self.tick);
        u32::try_from(ticks * 1000 / u64::from(TICK_HZ)).unwrap_or(u32::MAX)
    }
}

static NO_ITEMS: [(String, u32); 0] = [];

impl SimRead for NetWorld {
    fn content(&self) -> &Content {
        &self.content
    }
    fn unit(&self, id: EntityId) -> Option<&UnitState> {
        self.units.get(&id)
    }
    fn units(&self) -> Box<dyn Iterator<Item = &UnitState> + '_> {
        Box::new(self.units.values())
    }
    fn player(&self) -> Option<EntityId> {
        Some(self.you)
    }
    fn players(&self) -> &[EntityId] {
        &self.players
    }
    /// From the server's `Cooldowns` after each cast (estimated from content until it
    /// arrives); the HUD greys and sweeps slots with it, the server still decides.
    fn cooldown(&self, caster: EntityId, spell: SpellId) -> CooldownView {
        let def = self.content.spell(spell);
        let (cd_end, cd_total) = self.cooldowns.get(&spell).copied().unwrap_or((0, 0));
        let gcd_on = def.is_some_and(|d| d.gcd);
        let (gcd_remaining_ms, gcd_total_ms) = if gcd_on {
            (self.remaining_ms(self.gcd.0), self.gcd.1)
        } else {
            (0, 0)
        };
        let cd_remaining_ms = self.remaining_ms(cd_end);
        let me = self.units.get(&caster);
        let usable = match (def, me) {
            (None, _) | (_, None) => Err(CastError::InvalidTarget),
            (Some(d), Some(u)) => {
                if caster != self.you {
                    Ok(())
                } else if d.learn_level > u.level {
                    Err(CastError::NotLearned)
                } else if !u.alive {
                    Err(CastError::Dead)
                } else if u.stunned {
                    Err(CastError::Stunned)
                } else if cd_remaining_ms > 0 {
                    Err(CastError::OnCooldown)
                } else if gcd_remaining_ms > 0 {
                    Err(CastError::OnGcd)
                } else if u.mana < d.mana {
                    Err(CastError::NotEnoughMana)
                } else {
                    Ok(())
                }
            }
        };
        CooldownView {
            gcd_remaining_ms,
            gcd_total_ms,
            cd_remaining_ms,
            cd_total_ms: cd_total,
            usable,
        }
    }
    fn quest_status(&self, quest: &str) -> QuestStatus {
        let Some(def) = self.content.quest(quest) else {
            return QuestStatus::Locked;
        };
        if self.quests.completed.iter().any(|c| c == quest) {
            return QuestStatus::Done;
        }
        if let Some(q) = self.quests.active.iter().find(|q| q.id == quest) {
            let ready = def
                .objectives
                .iter()
                .zip(&q.counts)
                .all(|(o, c)| *c >= objective_need(o));
            return if ready {
                QuestStatus::Ready
            } else {
                QuestStatus::Active
            };
        }
        let level = self.units.get(&self.you).map_or(0, |u| u.level);
        let chain = def
            .requires
            .as_ref()
            .is_none_or(|r| self.quests.completed.contains(r));
        if chain && level >= def.level_min {
            QuestStatus::Available
        } else {
            QuestStatus::Locked
        }
    }
    fn quests(&self) -> &QuestLog {
        &self.quests
    }
    fn inventory(&self) -> &[(String, u32)] {
        self.units
            .get(&self.you)
            .map_or(&NO_ITEMS[..], |u| u.inventory.as_slice())
    }
    fn item_totals(&self) -> ItemTotals {
        let mut t = ItemTotals::default();
        for (name, n) in self.inventory() {
            if let Some(item) = self.content.item(name) {
                t.add(&item.bonus.totals(*n));
            }
        }
        t
    }
    fn zone_cleared(&self, zone: &str) -> bool {
        self.cleared.contains(zone)
    }
    fn zone_links(&self, zone: &str) -> Vec<ZoneLinkView> {
        self.content.zone(zone).map_or_else(Vec::new, |z| {
            z.links
                .iter()
                .map(|l| ZoneLinkView {
                    anchor: l.anchor.clone(),
                    to: l.to.clone(),
                    unlocked: l
                        .requires
                        .as_ref()
                        .is_none_or(|r| self.unlocked.contains(&l.to) || self.unlock_met(r)),
                })
                .collect()
        })
    }
    fn known_spells(&self, unit: EntityId) -> Vec<SpellId> {
        let level = self.units.get(&unit).map_or(1, |u| u.level);
        self.content
            .action_bar()
            .iter()
            .copied()
            .filter(|id| {
                self.content
                    .spell(*id)
                    .is_some_and(|s| s.learn_level <= level)
            })
            .collect()
    }
    fn now(&self) -> Tick {
        Tick(self.tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_names_are_sanitised_again_on_arrival() {
        let content = Content::from_ron(include_str!("../../../../content/game.ron")).unwrap();
        let mut w = NetWorld::new(content, 1, "R".into(), &[]);
        for (id, raw, want) in [(7, "Bob", "Bob"), (8, "<b>evil\u{202e}</b>", "Intern-8")] {
            w.add_player(&PlayerInfo {
                id,
                name: raw.into(),
            });
            assert_eq!(w.name_of(id), Some(want));
        }
    }

    #[test]
    fn cast_bars_fill_between_server_events_and_start_late_by_half_the_ping() {
        let content = Content::from_ron(include_str!("../../../../content/game.ron")).unwrap();
        let mut w = NetWorld::new(content, 1, "R".into(), &[]);
        let unit = UnitSnap {
            id: 1,
            key_idx: 0,
            level: 1,
            pos_cm: [0, 0, 0],
            facing: 0,
            hp: 100,
            max_hp: 100,
            target: None,
            flags: UnitSnap::ALIVE,
            auras: Vec::new(),
        };
        w.apply_snapshot(1, &[unit], &[], 0.0);
        w.record_pong(0, 0.2);
        let rtt = w.rtt_s;
        w.receive_events(
            2,
            vec![Event::CastStarted {
                caster: 1,
                spell: SpellId(1),
                total_ms: 2000,
            }],
            0.0,
        );
        let elapsed = |w: &NetWorld| w.units[&1].cast.map(|c| c.elapsed_ms);
        let late = (rtt * 500.0) as u32;
        assert!(late > 0 && late < 200, "{late}");
        assert_eq!(elapsed(&w), Some(late));
        w.advance(0.0);
        w.advance(1.0);
        assert_eq!(elapsed(&w), Some(late + 1000));
        w.advance(5.0);
        assert_eq!(elapsed(&w), Some(2000));
    }

    /// You (id 1, the player def) plus `others` as (id, unit key, flags).
    fn world_with(others: &[(EntityId, &str, u8)]) -> NetWorld {
        let content = Content::from_ron(include_str!("../../../../content/game.ron")).unwrap();
        let idx = |k: &str| content.units.iter().position(|u| u.key == k).unwrap() as u16;
        let snap = |id, key: &str, flags| UnitSnap {
            id,
            key_idx: idx(key),
            level: 1,
            pos_cm: [0, 0, 0],
            facing: 0,
            hp: 100,
            max_hp: 100,
            target: None,
            flags,
            auras: Vec::new(),
        };
        let mut units = vec![snap(1, "senior_engineer", UnitSnap::ALIVE)];
        units.extend(others.iter().map(|(id, k, f)| snap(*id, k, *f)));
        let mut w = NetWorld::new(content, 1, "R".into(), &[]);
        w.apply_snapshot(1, &units, &[], 0.0);
        w
    }

    #[test]
    fn own_cast_is_predicted_at_once_and_handed_to_the_server() {
        let cr = SpellId(1);
        let mut w = world_with(&[(2, "meatproxy", UnitSnap::ALIVE)]);
        let bar = |w: &NetWorld| w.units[&1].cast.map(|c| (c.spell, c.elapsed_ms));
        // Moving, or no enemy target: no prediction.
        w.predict_cast(cr, true, Some(2), 0.0);
        w.predict_cast(cr, false, None, 0.0);
        assert_eq!(bar(&w), None);
        assert_eq!(w.cooldown(1, cr).gcd_remaining_ms, 0);
        w.predict_cast(cr, false, Some(2), 0.0);
        assert_eq!(bar(&w), Some((cr, 0)));
        assert!(w.cooldown(1, cr).gcd_remaining_ms > 0);
        // A snapshot taken before the server saw the command does not wipe the bar.
        let me = w.units[&1].clone();
        let snap = UnitSnap::from_state(&me, &w.content).unwrap();
        let snap = UnitSnap {
            flags: UnitSnap::ALIVE,
            ..snap
        };
        w.apply_snapshot(2, &[snap], &[], 0.1);
        w.advance(0.0);
        w.advance(0.3);
        assert_eq!(bar(&w), Some((cr, 300)));
        // The server's start keeps the bar where it is (no jump back) and takes over.
        let started = Event::CastStarted {
            caster: 1,
            spell: cr,
            total_ms: 2000,
        };
        w.receive_events(3, vec![started], 0.3);
        assert_eq!(bar(&w), Some((cr, 300)));
        assert!(w.prediction.is_none());

        // A refusal removes the bar and gives the GCD back.
        let mut w = world_with(&[(2, "meatproxy", UnitSnap::ALIVE)]);
        w.predict_cast(cr, false, Some(2), 0.0);
        let failed = Event::CastFailed {
            caster: 1,
            spell: cr,
            reason: CastError::OutOfRange,
        };
        w.receive_events(2, vec![failed], 0.1);
        assert_eq!(bar(&w), None);
        assert_eq!(w.cooldown(1, cr).gcd_remaining_ms, 0);
        // An answer that never comes times out.
        w.predict_cast(cr, false, Some(2), 0.2);
        w.advance(0.2);
        w.advance(5.0);
        assert_eq!(bar(&w), None);
    }

    #[test]
    fn loot_markers_follow_this_players_roll_not_the_corpse_flag() {
        let dead_lootable = UnitSnap::LOOTABLE;
        let mut w = world_with(&[
            (2, "meatproxy", UnitSnap::ALIVE),
            (3, "meatproxy", UnitSnap::ALIVE),
        ]);
        let death = |unit| Event::Death {
            unit,
            killer: Some(1),
        };
        w.receive_events(
            2,
            vec![
                death(2),
                death(3),
                Event::LootDropped {
                    corpse: 2,
                    items: vec![],
                },
            ],
            0.0,
        );
        assert!(w.units[&2].lootable);
        // Unit 3's roll went to someone else: the corpse flag alone shows nothing here.
        w.apply_snapshot(
            3,
            &snaps_of(&w, &[(2, dead_lootable), (3, dead_lootable)]),
            &[],
            0.1,
        );
        assert!(w.units[&2].lootable && !w.units[&3].lootable);
        w.receive_events(
            4,
            vec![Event::Looted {
                looter: 1,
                corpse: 2,
                items: vec![],
            }],
            0.2,
        );
        // Still lootable for a teammate, but no longer for us.
        w.apply_snapshot(5, &snaps_of(&w, &[(2, dead_lootable)]), &[], 0.3);
        assert!(!w.units[&2].lootable);
    }

    fn snaps_of(w: &NetWorld, flags: &[(EntityId, u8)]) -> Vec<UnitSnap> {
        let mut out: Vec<UnitSnap> = w
            .units
            .values()
            .filter_map(|u| UnitSnap::from_state(u, &w.content))
            .collect();
        for s in &mut out {
            if let Some((_, f)) = flags.iter().find(|(id, _)| *id == s.id) {
                s.flags = *f;
            }
        }
        out
    }

    #[test]
    fn resurrected_teammate_comes_back_with_the_servers_hp_and_mana() {
        let mut w = world_with(&[(2, "senior_engineer", UnitSnap::ALIVE)]);
        w.receive_events(
            2,
            vec![Event::Death {
                unit: 2,
                killer: None,
            }],
            0.0,
        );
        assert!(!w.units[&2].alive && !w.units[&2].lootable);
        w.receive_events(
            3,
            vec![Event::Resurrected {
                unit: 2,
                by: 1,
                hp: 30,
                mana: 12,
            }],
            0.1,
        );
        let u = &w.units[&2];
        assert!(u.alive && u.hp == 30 && u.mana == 12);
    }

    fn v(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 0.0, z)
    }

    #[test]
    fn interpolation_blends_holds_ends_and_resets() {
        let mut i = Interp::default();
        assert!(i.sample(0.0).is_none());
        i.push(1.0, v(0.0, 0.0), 0.0, 1.0);
        i.push(2.0, v(10.0, 0.0), 1.0, 1.0);
        let Some((p, yaw)) = i.sample(1.5) else {
            panic!("no sample")
        };
        assert!((p.x - 5.0).abs() < 1e-4 && (yaw - 0.5).abs() < 1e-4);
        assert_eq!(i.sample(0.5).map(|s| s.0.x), Some(0.0));
        assert_eq!(i.sample(9.0).map(|s| s.0.x), Some(10.0));
        i.reset(3.0, v(-4.0, 0.0), 0.0);
        assert_eq!(i.sample(2.5).map(|s| s.0.x), Some(-4.0));
        // An out-of-order stamp restarts the buffer instead of producing a backwards segment.
        i.push(1.0, v(7.0, 0.0), 0.0, 1.0);
        assert_eq!(i.sample(5.0).map(|s| s.0.x), Some(7.0));
    }

    #[test]
    fn a_unit_that_starts_walking_after_a_pause_does_not_glide() {
        let mut i = Interp::default();
        i.push(0.0, v(0.0, 0.0), 0.0, 0.1);
        // Silent for 5 s, then a step at 10 Hz.
        i.push(5.0, v(0.7, 0.0), 0.0, 0.1);
        i.push(5.1, v(1.4, 0.0), 0.0, 0.1);
        assert_eq!(i.sample(4.85).map(|s| s.0.x), Some(0.0));
        let Some((mid, _)) = i.sample(4.95) else {
            panic!("no sample")
        };
        assert!((mid.x - 0.35).abs() < 1e-4, "{mid:?}");
        assert!((i.sample(5.05).map_or(0.0, |s| s.0.x) - 1.05).abs() < 1e-4);
    }

    #[test]
    fn angles_take_the_short_way() {
        let a = 350f32.to_radians();
        let b = 10f32.to_radians();
        let mid = lerp_angle(a, b, 0.5).rem_euclid(std::f32::consts::TAU);
        assert!(
            !(0.01..=std::f32::consts::TAU - 0.01).contains(&mid),
            "{mid}"
        );
    }

    #[test]
    fn correction_shifts_by_the_error_of_that_move() {
        let mut h = MoveHistory::default();
        let s1 = h.record(v(1.0, 0.0));
        let s2 = h.record(v(2.0, 0.0));
        let _s3 = h.record(v(3.0, 0.0));
        // The server stopped move s1 at x=0.5: the body (now at 3) moves back by 0.5.
        let d = h.correct(s1, v(0.5, 0.0), v(3.0, 0.0));
        assert!((d.x + 0.5).abs() < 1e-5);
        // Later history inherited the shift; s1 is gone, so a second correction of s2 is exact.
        let d2 = h.correct(s2, v(1.5, 0.0), v(2.5, 0.0));
        assert!(d2.x.abs() < 1e-5, "{d2:?}");
        // Unknown seq: snap to the server position.
        let d3 = h.correct(999, v(10.0, 0.0), v(2.5, 0.0));
        assert!((d3.x - 7.5).abs() < 1e-5);
    }

    #[test]
    fn snapshot_interval_is_smoothed_and_clamped() {
        let s = smooth_interval(1.0, 0.1);
        assert!(s < 1.0 && s > 0.1);
        assert!(smooth_interval(1.0, 100.0) <= 2.0);
    }
}
