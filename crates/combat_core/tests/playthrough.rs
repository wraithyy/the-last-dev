//! Headless playthrough: a scripted "competent player" clears whole zones and does quests.
//!
//! Balance gate: every zone at its max level without dying; the campus quest chain from level 1.
//! Golden: tower/1 from level 1.
//! Regenerate the golden after an intentional change:
//! `UPDATE_GOLDEN=1 cargo test -p combat_core --test playthrough`

mod common;

use combat_core::{
    Command, Event, QuestStatus, Sim, SpellId, TRASH_RESPAWN_MS, TelegraphView, Vec3, ms_to_ticks,
};
use common::*;
use std::fmt::Write as _;
use std::path::PathBuf;

const GOLDEN: &str = "../../content/replays/floor1_seed7.log";
const MAX_TICKS: u64 = 60_000;
const BLAME: SpellId = SpellId(6);
const ROLLBACK: SpellId = SpellId(7);
const CAFFEINE: SpellId = SpellId(8);
const DUCK: SpellId = SpellId(9);
const POSTMORTEM: SpellId = SpellId(10);
const BY_HAND: SpellId = SpellId(11);
/// Casts not worth a kick: self-stuns, the Ethics Review that does nothing, a coin-flip demo.
const NO_KICK: [SpellId; 4] = [SpellId(112), SpellId(150), SpellId(154), SpellId(180)];
/// The AGI's telegraph for its spell reflect window.
const ALIGNMENT: SpellId = SpellId(187);
const PACK_SPACING: f32 = 70.0;
/// Bot running speed while dodging or chasing.
const RUN_MPS: f32 = 7.0;
const PIXEL_PUSH: SpellId = SpellId(20);
const CSS_COMBO: SpellId = SpellId(21);
const IMPORTANT: SpellId = SpellId(22);
const LIGHTHOUSE: SpellId = SpellId(23);
const HOT_RELOAD: SpellId = SpellId(24);
const DARK_MODE: SpellId = SpellId(25);
const RESPONSIVE: SpellId = SpellId(26);
const INFINITE_SCROLL: SpellId = SpellId(27);
const CENTER_DIV: SpellId = SpellId(28);
const NEW_FRAMEWORK: SpellId = SpellId(29);
const BLOCKING_CALL: SpellId = SpellId(30);
const RATE_LIMITER: SpellId = SpellId(31);
const TAUNT: SpellId = SpellId(32);
const CONN_POOL: SpellId = SpellId(33);
const KILL_QUERY: SpellId = SpellId(34);
const DB_LOCK: SpellId = SpellId(35);
const HORIZONTAL: SpellId = SpellId(36);
const RETRY: SpellId = SpellId(37);
const CACHE_INVAL: SpellId = SpellId(38);
const DROP_TABLE: SpellId = SpellId(39);
const KILL_9: SpellId = SpellId(40);
const HOTFIX: SpellId = SpellId(41);
const SIGSTOP: SpellId = SpellId(42);
const AUTO_SCALING: SpellId = SpellId(43);
const CHAOS_MONKEY: SpellId = SpellId(44);
const REVERT: SpellId = SpellId(45);
const FEATURE_FLAG: SpellId = SpellId(46);
const CIRCUIT: SpellId = SpellId(47);
const TERRAFORM: SpellId = SpellId(49);

/// Packs on a straight corridor toward -Z, the boss at the end, NPCs off to the side.
fn anchor(name: &str) -> Option<(Vec3, f32)> {
    let num = |p: &str| name.strip_prefix(p).and_then(|n| n.parse::<f32>().ok());
    if let Some(n) = num("pack_") {
        return Some((Vec3::new(0.0, 0.0, -PACK_SPACING * n), 0.0));
    }
    if let Some(n) = num("npc_") {
        return Some((Vec3::new(40.0, 0.0, -15.0 * n), 0.0));
    }
    (name == "boss").then_some((Vec3::new(0.0, 0.0, -PACK_SPACING * 7.0), 0.0))
}

#[derive(Default, Debug)]
struct Report {
    died: bool,
    cleared: bool,
    rests: u32,
    pulls: u32,
    boss_hp_left: Option<(u32, u32)>,
    /// Lowest player HP % seen during the boss fight (the kill's level-up refills HP).
    boss_min_hp_pct: u32,
    level: u8,
    /// (hp %, mana %, seconds) after each pull.
    after: Vec<(u32, u32, u64)>,
}

struct Bot {
    sim: Sim,
    player: u64,
    pos: Vec3,
    target: Option<u64>,
    log: String,
    report: Report,
    in_boss: bool,
    /// Walked this tick (dodging or chasing): reported to the sim, which cancels cast-time spells.
    moving: bool,
    dodge_goal: Option<Vec3>,
    /// Loot every corpse after a pull (quest runs only; the golden predates looting).
    looting: bool,
    class: &'static str,
    seed: u64,
    /// Lowest HP % seen on any tick (sprint balance).
    low_hp_pct: u32,
    /// Stop drinking when something attacks (sprint adds); off for the golden runs.
    rest_alert: bool,
}

fn yaw_to(from: Vec3, to: Vec3) -> f32 {
    (-(to.x - from.x)).atan2(-(to.z - from.z))
}

impl Bot {
    fn new(zone: &str, level: u8, seed: u64) -> Bot {
        Bot::new_as("fullstack", zone, level, seed)
    }

    fn new_as(class: &'static str, zone: &str, level: u8, seed: u64) -> Bot {
        let mut sim = Sim::new(content(), seed);
        // The balance gate and golden pin fight timing, which wandering would shift.
        sim.set_wandering(false);
        let start = Vec3::new(0.0, 0.0, 20.0);
        let player = sim.spawn_player(class, start, 0.0);
        assert_ne!(player, 0, "unknown class {class}");
        sim.set_player_level(level);
        sim.set_respawn_point(start, 0.0);
        if let Err(e) = sim.load_zone(zone, &anchor, Vec::new()) {
            panic!("load_zone {zone}: {e}");
        }
        Bot {
            sim,
            player,
            pos: start,
            target: None,
            log: format!("# {zone} playthrough, seed {seed}, start level {level}\n"),
            report: Report::default(),
            in_boss: false,
            moving: false,
            dodge_goal: None,
            looting: false,
            class,
            seed,
            low_hp_pct: 100,
            rest_alert: false,
        }
    }

    fn me(&self) -> combat_core::UnitState {
        match self.sim.unit(self.player) {
            Some(u) => u.clone(),
            None => panic!("no player"),
        }
    }

    fn tick(&mut self) {
        let yaw = self
            .target
            .and_then(|t| self.sim.unit(t))
            .map_or(0.0, |t| yaw_to(self.pos, t.pos));
        self.sim.push(Command::PlayerMoved {
            pos: self.pos,
            yaw,
            moving: std::mem::take(&mut self.moving),
            jumping: false,
        });
        let events = self.sim.tick();
        let now = self.sim.now().0;
        if let Some(me) = self.sim.unit(self.player).filter(|u| u.alive) {
            self.low_hp_pct = self.low_hp_pct.min(me.hp * 100 / me.max_hp.max(1));
        }
        if self.in_boss
            && let Some(me) = self.sim.unit(self.player)
        {
            let pct = me.hp * 100 / me.max_hp.max(1);
            self.report.boss_min_hp_pct = self.report.boss_min_hp_pct.min(pct);
        }
        for e in &events {
            match e {
                Event::Knockback {
                    unit,
                    dir,
                    strength,
                } if *unit == self.player => {
                    self.pos.x += dir.x * strength;
                    self.pos.z += dir.z * strength;
                }
                Event::Teleported { unit, pos } if *unit == self.player => self.pos = *pos,
                Event::Death { unit, .. } if *unit == self.player => self.report.died = true,
                Event::ZoneCleared { .. } => self.report.cleared = true,
                _ => {}
            }
            let _ = writeln!(self.log, "{}", log_line(&self.sim, now, e));
        }
    }

    fn ready(&self, spell: SpellId) -> bool {
        self.sim.cooldown(self.player, spell).usable.is_ok()
    }

    fn cast(&mut self, spell: SpellId) {
        self.sim.push(Command::Cast {
            caster: self.player,
            spell,
        });
    }

    fn set_target(&mut self, t: u64) {
        if self.target != Some(t) {
            self.target = Some(t);
            self.sim.push(Command::SetTarget {
                unit: self.player,
                target: Some(t),
            });
            self.sim
                .push(Command::StartAutoAttack { unit: self.player });
        }
    }

    /// The class's interrupt, and its stun as the fallback on non-bosses.
    fn kick_spells(&self) -> (SpellId, SpellId) {
        match self.class {
            "frontend" => (LIGHTHOUSE, CENTER_DIV),
            "backend" => (KILL_QUERY, DB_LOCK),
            "devops" => (SIGSTOP, SIGSTOP),
            _ => (RPR, DUCK),
        }
    }

    fn off_cooldown(&self, spell: SpellId) -> bool {
        let v = self.sim.cooldown(self.player, spell);
        v.cd_remaining_ms == 0 && v.usable != Err(combat_core::CastError::NotLearned)
    }

    /// An enemy in the fight casting a must-interrupt spell, nearest first.
    fn kick_caster(&self) -> Option<u64> {
        let c = self.sim.content();
        self.sim
            .units()
            .filter(|u| u.hostile && u.alive && u.in_combat)
            .filter(|u| {
                u.cast
                    .is_some_and(|k| k.interruptible && c.spell(k.spell).is_some_and(|s| s.kick))
            })
            .min_by(|a, b| {
                combat_core::distance(a.pos, self.pos)
                    .total_cmp(&combat_core::distance(b.pos, self.pos))
            })
            .map(|u| u.id)
    }

    /// Hostiles fighting the player: a must-interrupt caster while the kick is up, then adds
    /// (low max HP), then the nearest.
    fn pick_target(&self) -> Option<u64> {
        let (kick, stun) = self.kick_spells();
        if let Some(id) = self.kick_caster() {
            let boss = self.sim.unit(id).is_some_and(|u| u.boss);
            if self.off_cooldown(kick) || (!boss && self.off_cooldown(stun)) {
                return Some(id);
            }
        }
        self.sim
            .units()
            .filter(|u| {
                u.hostile && u.alive && u.in_combat && u.ai != Some(combat_core::AiState::Evade)
            })
            .min_by(|a, b| {
                a.max_hp.cmp(&b.max_hp).then(
                    combat_core::distance(a.pos, self.pos)
                        .total_cmp(&combat_core::distance(b.pos, self.pos)),
                )
            })
            .map(|u| u.id)
    }

    /// Walks up to one tick of running speed toward `goal`.
    fn walk(&mut self, goal: Vec3) {
        let (dx, dz) = (goal.x - self.pos.x, goal.z - self.pos.z);
        let d = (dx * dx + dz * dz).sqrt();
        if d < 1e-3 {
            return;
        }
        let step = (RUN_MPS / 30.0).min(d);
        self.pos.x += dx / d * step;
        self.pos.z += dz / d * step;
        self.moving = true;
    }

    /// Steps out of any telegraph that would hit where we stand (the nearest safe spot, cover
    /// included). Shared (split) telegraphs are stood in on purpose. True while walking.
    fn dodge(&mut self) -> bool {
        let tgs: Vec<TelegraphView> = self
            .sim
            .telegraphs()
            .into_iter()
            .filter(|t| !t.split)
            .collect();
        let unsafe_at = |p: Vec3| tgs.iter().any(|t| t.hits(p));
        if !unsafe_at(self.pos) {
            self.dodge_goal = None;
            return false;
        }
        // Keep walking to the spot already picked while it stays safe (no zig-zag).
        if let Some(g) = self.dodge_goal.filter(|g| !unsafe_at(*g)) {
            self.walk(g);
            return true;
        }
        let mut candidates: Vec<Vec3> = tgs.iter().flat_map(|t| t.cover.clone()).collect();
        for k in 1..=10 {
            let r = 1.5 * k as f32;
            for a in 0..16 {
                let ang = a as f32 * std::f32::consts::TAU / 16.0;
                candidates.push(Vec3::new(
                    self.pos.x + r * ang.cos(),
                    0.0,
                    self.pos.z + r * ang.sin(),
                ));
            }
        }
        let goal = candidates
            .into_iter()
            .filter(|p| !unsafe_at(*p))
            .min_by(|a, b| {
                combat_core::distance(*a, self.pos).total_cmp(&combat_core::distance(*b, self.pos))
            });
        self.dodge_goal = goal;
        match goal {
            Some(g) => {
                self.walk(g);
                true
            }
            None => false,
        }
    }

    /// Closes in on a target out of reach (a fleeing boss): melee to 3 m, casters to 24 m.
    fn chase(&mut self) -> bool {
        let Some(t) = self.target.and_then(|t| self.sim.unit(t)).cloned() else {
            return false;
        };
        let melee = matches!(self.class, "frontend" | "backend");
        let (far, near) = if melee { (4.5, 3.0) } else { (27.0, 24.0) };
        let d = combat_core::distance(t.pos, self.pos);
        if !t.alive || d <= far {
            return false;
        }
        let k = (d - near) / d;
        let goal = Vec3::new(
            self.pos.x + (t.pos.x - self.pos.x) * k,
            0.0,
            self.pos.z + (t.pos.z - self.pos.z) * k,
        );
        self.walk(goal);
        true
    }

    /// Kick only what matters: against a unit with must-interrupt casts, save the kick for
    /// those; otherwise any interruptible cast worth a kick.
    fn worth_kick(&self, t: &combat_core::UnitState, spell: SpellId) -> bool {
        if NO_KICK.contains(&spell) {
            return false;
        }
        let c = self.sim.content();
        let kick = |id: SpellId| c.spell(id).is_some_and(|s| s.kick);
        let kicker = |key: &str| {
            c.unit_def(key)
                .and_then(|u| u.ai.as_ref())
                .is_some_and(|ai| ai.abilities.iter().any(|a| kick(a.spell)))
        };
        let pack_kicks = kicker(&t.key)
            || self
                .sim
                .units()
                .any(|u| u.hostile && u.alive && u.in_combat && kicker(&u.key));
        kick(spell) || !pack_kicks
    }

    fn act(&mut self) {
        if self.me().alive && !self.me().stunned && (self.dodge() || self.chase()) {
            return;
        }
        // A must-interrupt cast with the kick on cooldown: drop our own cast for the stun.
        let (kick, stun) = self.kick_spells();
        if let Some(id) = self.kick_caster()
            && self.me().cast.is_some()
            && !self.off_cooldown(kick)
            && self.off_cooldown(stun)
            && self.sim.unit(id).is_some_and(|u| !u.boss)
        {
            self.sim.push(Command::StopCasting {
                caster: self.player,
            });
        }
        match self.class {
            "frontend" => self.act_frontend(),
            "backend" => self.act_backend(),
            "devops" => self.act_devops(),
            _ => self.act_fullstack(),
        }
    }

    /// Shared start of every policy: target pick, then the living target if any.
    fn engage(&mut self) -> Option<(combat_core::UnitState, combat_core::UnitState)> {
        let me = self.me();
        if !me.alive || me.stunned {
            return None;
        }
        if let Some(t) = self.pick_target() {
            self.set_target(t);
        }
        let t = self.target.and_then(|t| self.sim.unit(t)).cloned()?;
        Some((me, t))
    }

    /// Kick an interruptible cast worth kicking with `kick`, else stun a non-boss with `stun`.
    fn try_kick(&mut self, t: &combat_core::UnitState, kick: SpellId, stun: SpellId) -> bool {
        let Some(c) = t.cast else { return false };
        if !c.interruptible || !self.worth_kick(t, c.spell) {
            return false;
        }
        for s in [kick, stun] {
            if (s == kick || !t.boss) && self.ready(s) {
                self.cast(s);
                return true;
            }
        }
        false
    }

    fn busy(&self, me: &combat_core::UnitState) -> bool {
        me.cast.is_some() || self.sim.cooldown(self.player, CR).gcd_remaining_ms > 0
    }

    fn enemies_near(&self, r: f32) -> usize {
        self.sim
            .units()
            .filter(|u| {
                u.hostile && u.alive && u.in_combat && combat_core::distance(u.pos, self.pos) <= r
            })
            .count()
    }

    fn reflecting(t: &combat_core::UnitState) -> bool {
        t.auras.iter().any(|a| a.key == "alignment_training")
            || t.cast.is_some_and(|c| c.spell == ALIGNMENT)
    }

    fn act_frontend(&mut self) {
        let Some((me, t)) = self.engage() else { return };
        let hp_pct = me.hp * 100 / me.max_hp.max(1);
        let deadline = me
            .auras
            .iter()
            .any(|a| a.key == "deadline" && a.remaining_ms < 1500);
        let swarmed = self.enemies_near(6.0) >= 3 && hp_pct < 80;
        if (hp_pct < 45 || deadline || swarmed) && self.ready(DARK_MODE) {
            self.cast(DARK_MODE);
        }
        if Self::reflecting(&t) {
            return;
        }
        if self.try_kick(&t, LIGHTHOUSE, CENTER_DIV) || self.busy(&me) {
            return;
        }
        let d = combat_core::distance(me.pos, t.pos);
        if d > 6.0 && self.ready(HOT_RELOAD) {
            return self.cast(HOT_RELOAD);
        }
        let stacks = t
            .auras
            .iter()
            .find(|a| a.key == "specificity")
            .map_or(0, |a| a.stacks);
        let t_pct = t.hp * 100 / t.max_hp.max(1);
        if self.enemies_near(6.0) >= 3 && self.ready(RESPONSIVE) {
            return self.cast(RESPONSIVE);
        }
        if t.max_hp > 1000 && self.ready(NEW_FRAMEWORK) {
            return self.cast(NEW_FRAMEWORK);
        }
        let scrolled = t.auras.iter().any(|a| a.key == "infinite_scroll");
        if !scrolled && t.hp > 200 && self.ready(INFINITE_SCROLL) {
            return self.cast(INFINITE_SCROLL);
        }
        if (stacks >= 4 || (stacks >= 1 && t_pct < 20)) && self.ready(IMPORTANT) {
            return self.cast(IMPORTANT);
        }
        if self.ready(CSS_COMBO) {
            return self.cast(CSS_COMBO);
        }
        if self.ready(PIXEL_PUSH) {
            self.cast(PIXEL_PUSH);
        }
    }

    fn act_backend(&mut self) {
        let Some((me, t)) = self.engage() else { return };
        let hp_pct = me.hp * 100 / me.max_hp.max(1);
        if hp_pct < 50 && self.ready(HORIZONTAL) {
            self.cast(HORIZONTAL);
        }
        if t.target != Some(self.player) && self.ready(TAUNT) {
            self.cast(TAUNT);
        }
        if self.try_kick(&t, KILL_QUERY, DB_LOCK) || self.busy(&me) {
            return;
        }
        let shielded = me.auras.iter().any(|a| a.key == "rate_limiter");
        if !shielded && hp_pct < 90 && self.ready(RATE_LIMITER) {
            return self.cast(RATE_LIMITER);
        }
        let retrying = me.auras.iter().any(|a| a.key == "retry_backoff");
        if !retrying && hp_pct < 60 && self.ready(RETRY) {
            return self.cast(RETRY);
        }
        if Self::reflecting(&t) {
            return;
        }
        if self.enemies_near(8.0) >= 2 && self.ready(CONN_POOL) {
            return self.cast(CONN_POOL);
        }
        if t.max_hp > 1000 && self.ready(DROP_TABLE) {
            return self.cast(DROP_TABLE);
        }
        if self.ready(CACHE_INVAL) {
            return self.cast(CACHE_INVAL);
        }
        if self.ready(BLOCKING_CALL) {
            self.cast(BLOCKING_CALL);
        }
    }

    fn act_devops(&mut self) {
        let Some((me, t)) = self.engage() else { return };
        let reflecting = Self::reflecting(&t);
        if reflecting
            && me
                .cast
                .is_some_and(|c| c.spell == KILL_9 || c.spell == TERRAFORM)
        {
            self.sim.push(Command::StopCasting {
                caster: self.player,
            });
        }
        if !reflecting && self.try_kick(&t, SIGSTOP, SIGSTOP) {
            return;
        }
        if self.busy(&me) {
            return;
        }
        let hp_pct = me.hp * 100 / me.max_hp.max(1);
        let has = |k: &str| me.auras.iter().any(|a| a.key == k);
        if has("deadline") && self.ready(FEATURE_FLAG) {
            return self.cast(FEATURE_FLAG);
        }
        if hp_pct < 50 && self.ready(REVERT) {
            return self.cast(REVERT);
        }
        if hp_pct < 70 && !has("circuit_breaker") && self.ready(CIRCUIT) {
            return self.cast(CIRCUIT);
        }
        if hp_pct < 85 && !has("auto_scaling") && self.ready(AUTO_SCALING) {
            return self.cast(AUTO_SCALING);
        }
        if hp_pct < 60 && self.ready(HOTFIX) {
            return self.cast(HOTFIX);
        }
        if reflecting {
            return;
        }
        let monkeyed = t.auras.iter().any(|a| a.key == "chaos_monkey");
        if !monkeyed && t.hp > 150 && self.ready(CHAOS_MONKEY) {
            return self.cast(CHAOS_MONKEY);
        }
        if t.max_hp > 1000 && hp_pct > 60 && self.ready(TERRAFORM) {
            return self.cast(TERRAFORM);
        }
        if self.ready(KILL_9) {
            self.cast(KILL_9);
        }
    }

    fn act_fullstack(&mut self) {
        let me = self.me();
        if !me.alive || me.stunned {
            return;
        }
        if let Some(t) = self.pick_target() {
            self.set_target(t);
        }
        let Some(t) = self.target.and_then(|t| self.sim.unit(t)).cloned() else {
            return;
        };
        // Spell reflect (or its telegraph): stop casting at it and let Nitpick do the work.
        let reflecting = t.auras.iter().any(|a| a.key == "alignment_training")
            || t.cast.is_some_and(|c| c.spell == ALIGNMENT);
        if reflecting {
            if me.cast.is_some() {
                self.sim.push(Command::StopCasting {
                    caster: self.player,
                });
            }
            let hp_pct = me.hp * 100 / me.max_hp.max(1);
            if hp_pct < 50 && self.ready(ROLLBACK) {
                self.cast(ROLLBACK);
            }
            return;
        }
        if let Some(c) = t.cast
            && c.interruptible
            && self.worth_kick(&t, c.spell)
        {
            if self.ready(RPR) {
                return self.cast(RPR);
            }
            if !t.boss && self.ready(DUCK) {
                return self.cast(DUCK);
            }
        }
        if me.cast.is_some() || self.sim.cooldown(self.player, CR).gcd_remaining_ms > 0 {
            return;
        }
        let hp_pct = me.hp * 100 / me.max_hp.max(1);
        let deadline = me.auras.iter().any(|a| a.key == "deadline");
        if (hp_pct < 50 || deadline) && self.ready(ROLLBACK) {
            return self.cast(ROLLBACK);
        }
        if hp_pct < 75 && self.ready(GR) {
            return self.cast(GR);
        }
        if self.in_boss && self.ready(CAFFEINE) {
            return self.cast(CAFFEINE);
        }
        let t_pct = t.hp * 100 / t.max_hp.max(1);
        if t_pct < 25 && self.ready(POSTMORTEM) {
            return self.cast(POSTMORTEM);
        }
        let blamed = t.auras.iter().any(|a| a.key == "git_blame");
        if !blamed && t.hp > 150 && self.ready(BLAME) {
            return self.cast(BLAME);
        }
        if t.max_hp > 1000 && t_pct > 25 && hp_pct > 60 && self.ready(BY_HAND) {
            return self.cast(BY_HAND);
        }
        let smell = t.auras.iter().any(|a| a.key == "code_smell");
        if smell && self.ready(RF) {
            self.cast(RF);
        } else if self.ready(CR) {
            self.cast(CR);
        }
    }

    fn fight_until_calm(&mut self) {
        let start = self.sim.now().0;
        // An opener still in flight (a cast from range) counts: its hit is what pulls.
        while self.sim.now().0 - start < 90 || self.me().in_combat || self.me().cast.is_some() {
            if self.sim.now().0 > MAX_TICKS || self.report.died {
                return;
            }
            self.act();
            self.tick();
        }
        self.target = None;
    }

    /// Drinks (out-of-combat regen) only below 60% HP or 50% mana, then up to 95%.
    /// Before the boss it always tops off, which does not count as a rest.
    fn rest(&mut self, before_boss: bool) {
        let low = |u: &combat_core::UnitState| {
            u.hp * 100 < u.max_hp * 60 || u.mana * 100 < u.max_mana * 50
        };
        let full = |u: &combat_core::UnitState| {
            u.hp * 100 >= u.max_hp * 95 && u.mana * 100 >= u.max_mana * 95
        };
        if !before_boss && !low(&self.me()) {
            return;
        }
        if !before_boss {
            self.report.rests += 1;
        }
        while !full(&self.me()) && !self.report.died && self.sim.now().0 < MAX_TICKS {
            if self.rest_alert && self.me().in_combat {
                return self.fight_until_calm();
            }
            self.tick();
        }
    }

    /// Walks to 20 m in front of the anchor, opens on the nearest unit, fights, rests.
    fn pull(&mut self, at: Vec3, boss: bool) {
        self.in_boss = boss;
        if boss {
            self.report.boss_min_hp_pct = 100;
        }
        // Melee walks up into the pack's aggro radius; casters open from 20 m.
        let gap = if matches!(self.class, "frontend" | "backend") {
            4.0
        } else {
            20.0
        };
        self.pos = Vec3::new(at.x, 0.0, at.z + gap);
        self.tick();
        let first = self
            .sim
            .units()
            .filter(|u| u.hostile && u.alive && combat_core::distance(u.pos, at) < 6.0)
            .min_by(|a, b| {
                combat_core::distance(a.pos, self.pos)
                    .total_cmp(&combat_core::distance(b.pos, self.pos))
            })
            .map(|u| u.id);
        let Some(first) = first else { return };
        self.report.pulls += 1;
        self.set_target(first);
        self.tick();
        let t0 = self.sim.now().0;
        self.fight_until_calm();
        let me = self.me();
        self.report.after.push((
            me.hp * 100 / me.max_hp.max(1),
            me.mana * 100 / me.max_mana.max(1),
            (self.sim.now().0 - t0) / 30,
        ));
        if self.looting {
            self.loot_near(at);
        }
        if boss {
            self.report.boss_hp_left = Some((self.me().hp, self.me().max_hp));
        } else {
            self.rest(false);
        }
    }

    fn loot_near(&mut self, at: Vec3) {
        let corpses: Vec<(u64, Vec3)> = self
            .sim
            .units()
            .filter(|u| u.lootable && combat_core::distance(u.pos, at) < 40.0)
            .map(|u| (u.id, u.pos))
            .collect();
        for (id, pos) in corpses {
            self.pos = Vec3::new(pos.x, 0.0, pos.z + 1.0);
            self.tick();
            self.sim.push(Command::Loot {
                looter: self.player,
                corpse: id,
            });
            self.tick();
        }
    }

    /// Talks to every friendly NPC of the zone, turns in what is ready and accepts what is
    /// available among `quests`.
    fn visit_givers(&mut self, zone: &str, quests: &[&str]) {
        let Some(z) = self.sim.content().zone(zone).cloned() else {
            return;
        };
        for f in &z.friendly {
            let Some((at, _)) = anchor(&f.anchor) else {
                continue;
            };
            self.pos = Vec3::new(at.x, 0.0, at.z + 2.0);
            self.tick();
            let Some(npc) = self.sim.units().find(|u| u.key == f.unit).map(|u| u.id) else {
                continue;
            };
            self.sim.push(Command::Talk { unit: npc });
            self.tick();
            for q in quests {
                let Some(def) = self.sim.content().quest(q).cloned() else {
                    continue;
                };
                let quest = q.to_string();
                match self.sim.quest_status(q) {
                    QuestStatus::Ready if def.ender().npc == f.unit => {
                        self.sim.push(Command::TurnInQuest { quest, choice: 0 })
                    }
                    QuestStatus::Available if def.giver.npc == f.unit => {
                        self.sim.push(Command::AcceptQuest { quest })
                    }
                    _ => continue,
                }
                self.tick();
            }
        }
    }

    fn clear_packs(&mut self, zone: &str) {
        let packs = self.sim.content().zone(zone).map_or(0, |z| z.packs.len());
        for k in 1..=packs {
            if self.report.died {
                return;
            }
            if let Some((at, _)) = anchor(&format!("pack_{k}")) {
                self.pull(at, false);
            }
        }
    }

    /// Walks out to another zone and back once the trash respawn timer has run out.
    fn leave_and_return(&mut self, zone: &str, elsewhere: &str) {
        for id in [elsewhere, zone] {
            if let Err(e) = self.sim.load_zone(id, &anchor, Vec::new()) {
                panic!("load_zone {id}: {e}");
            }
            self.tick();
            if id == elsewhere {
                for _ in 0..ms_to_ticks(TRASH_RESPAWN_MS) {
                    self.tick();
                }
            }
        }
    }

    fn run(mut self, zone: &str) -> (Report, String) {
        let def = self.sim.content().zone(zone).cloned();
        let packs = def.as_ref().map_or(0, |z| z.packs.len());
        let has_boss = def.is_some_and(|z| z.boss.is_some());
        for k in 1..=packs {
            if self.report.died {
                break;
            }
            if let Some((at, _)) = anchor(&format!("pack_{k}")) {
                self.pull(at, false);
            }
        }
        if !self.report.died
            && has_boss
            && let Some((at, _)) = anchor("boss")
        {
            self.rest(true);
            self.pull(at, true);
        }
        self.report.level = self.me().level;
        // Tuning aid: `WOW_BOT_DUMP=<dir>` keeps every run's full event log.
        if let Some(dir) = std::env::var_os("WOW_BOT_DUMP") {
            let name = format!(
                "{}_{}_{}.log",
                self.class,
                zone.replace('/', "-"),
                self.seed
            );
            let _ = std::fs::write(PathBuf::from(dir).join(name), &self.log);
        }
        (self.report, self.log)
    }
}

/// Several seeds so the verdict does not hinge on one lucky crit/miss/loot stream. Every zone
/// with packs, at its max level; zones without a boss only need to be survived.
#[test]
fn every_zone_is_cleared_at_its_max_level() {
    let mut failures = Vec::new();
    let mut lowest: Vec<(String, u32)> = Vec::new();
    for z in content().zones {
        let mut low = 100;
        for seed in [7, 42, 1234] {
            let max = z.level_range.1;
            let (r, log) = Bot::new(&z.id, max, seed).run(&z.id);
            println!(
                "seed {seed} {} at level {max}: boss fight min HP {}%, HP at kill {:?}, rests {}/{} pulls, after each pull {:?}",
                z.id, r.boss_min_hp_pct, r.boss_hp_left, r.rests, r.pulls, r.after
            );
            if z.boss.is_some() {
                low = low.min(r.boss_min_hp_pct);
            }
            if r.died || (z.boss.is_some() && !r.cleared) {
                let tail: Vec<&str> = log.lines().rev().take(60).collect();
                failures.push(format!(
                    "seed {seed} {}: {r:?}\n{}",
                    z.id,
                    tail.into_iter().rev().collect::<Vec<_>>().join("\n")
                ));
            }
            // "Competent player clears trash without resting every pull."
            if r.rests >= r.pulls {
                failures.push(format!(
                    "seed {seed} {}: rested after every pull ({r:?})",
                    z.id
                ));
            }
        }
        lowest.push((z.id.clone(), low));
    }
    println!("lowest boss-fight HP % per zone over 3 seeds: {lowest:?}");
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every zone at its max level for one class over three seeds; returns failures and the
/// lowest boss-fight HP % per zone.
fn clear_zones(class: &'static str, zones: &[&str]) -> (Vec<String>, Vec<(String, u32)>) {
    let mut failures = Vec::new();
    let mut lowest = Vec::new();
    for zid in zones {
        let Some(z) = content().zone(zid).cloned() else {
            panic!("no zone {zid}");
        };
        let mut low = 100;
        for seed in [7, 42, 1234] {
            let max = z.level_range.1;
            let (r, log) = Bot::new_as(class, &z.id, max, seed).run(&z.id);
            println!(
                "{class} seed {seed} {} at level {max}: boss min HP {}%, rests {}/{} pulls, after each pull {:?}",
                z.id, r.boss_min_hp_pct, r.rests, r.pulls, r.after
            );
            if z.boss.is_some() {
                low = low.min(r.boss_min_hp_pct);
            }
            if r.died || (z.boss.is_some() && !r.cleared) {
                let tail: Vec<&str> = log.lines().rev().take(40).collect();
                failures.push(format!(
                    "{class} seed {seed} {}: {r:?}\n{}",
                    z.id,
                    tail.into_iter().rev().collect::<Vec<_>>().join("\n")
                ));
            }
            if r.rests >= r.pulls {
                failures.push(format!(
                    "{class} seed {seed} {}: rested after every pull",
                    z.id
                ));
            }
        }
        lowest.push((z.id.clone(), low));
    }
    (failures, lowest)
}

/// Frontend, Backend and DevOps clear every zone solo at its max level, like the Fullstack
/// gate above (the campus chain from level 1 is covered per class below). `WOW_CLASS_ZONES`
/// (comma list) narrows it while tuning.
#[test]
fn other_classes_clear_every_zone() {
    let zones: Vec<String> = std::env::var("WOW_CLASS_ZONES").map_or_else(
        |_| content().zones.into_iter().map(|z| z.id).collect(),
        |v| v.split(',').map(String::from).collect(),
    );
    let zones: Vec<&str> = zones.iter().map(String::as_str).collect();
    let mut failures = Vec::new();
    for class in ["frontend", "backend", "devops"] {
        let (f, lowest) = clear_zones(class, &zones);
        println!("{class}: lowest boss-fight HP % per zone over 3 seeds: {lowest:?}");
        failures.extend(f);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn campus_quest_chain_from_level_one() {
    campus_chain("fullstack");
}

#[test]
fn campus_quest_chain_from_level_one_other_classes() {
    for class in ["frontend", "backend", "devops"] {
        campus_chain(class);
    }
}

/// Campus from level 1: visit every giver, clear the packs looting everything, hand in, and
/// leave and return after the trash respawn timer until the campus chain is done.
fn campus_chain(class: &'static str) {
    // Human in the Loop is left out: its MeatProxies are spread over the campus and the tower.
    const CAMPUS: [&str; 5] = [
        "onboarding",
        "badge_access_denied",
        "coffee_run",
        "fire_exit_strategy",
        "content_moderation",
    ];
    for seed in [7, 42, 1234] {
        let mut bot = Bot::new_as(class, "campus", 1, seed);
        bot.looting = true;
        let mut rounds = 0;
        while rounds < 6 && !bot.report.died {
            rounds += 1;
            bot.visit_givers("campus", &CAMPUS);
            bot.clear_packs("campus");
            bot.visit_givers("campus", &CAMPUS);
            if CAMPUS
                .iter()
                .all(|q| bot.sim.quest_status(q) == QuestStatus::Done)
            {
                break;
            }
            bot.leave_and_return("campus", "tower/1");
        }
        let left: Vec<&str> = CAMPUS
            .iter()
            .copied()
            .filter(|q| bot.sim.quest_status(q) != QuestStatus::Done)
            .collect();
        let me = bot.me();
        println!(
            "{class} seed {seed}: campus chain in {rounds} clears, level {}, {} trophies, {} rests / {} pulls",
            me.level,
            me.inventory.len(),
            bot.report.rests,
            bot.report.pulls
        );
        if bot.report.died {
            let tail: Vec<&str> = bot.log.lines().rev().take(60).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            panic!(
                "{class} seed {seed}: died on the campus\n{}",
                tail.join("\n")
            );
        }
        assert!(left.is_empty(), "{class} seed {seed}: unfinished {left:?}");
        assert!(
            rounds <= 3,
            "seed {seed}: {rounds} clears is too much farming"
        );
        // The campus is the tutorial: a fresh hire leaves it at level 3 and walks into tower/1
        // (levels 1-2) slightly over-levelled. A bad quest-drop streak (telegraph dodges and
        // kicks shift every class's rng stream) can need the third clear allowed above, which
        // is worth one extra level; never more.
        let levels = 3..=4;
        assert!(
            levels.contains(&me.level),
            "{class} seed {seed}: the campus should take a fresh hire to level 3 (got {})",
            me.level
        );
    }
}

#[test]
fn floor_one_from_level_one_matches_golden() {
    let (r, log) = Bot::new("tower/1", 1, 7).run("tower/1");
    assert!(r.cleared && !r.died, "{r:?}");
    assert!(r.level >= 2, "floor 1 should level the player: {r:?}");
    for beat in [
        "floor 1 loaded",
        "reaches level 2",
        "learns Reject PR",
        "MeatProxy casts Forwarded Reply",
        "summons Unreviewed PR",
        "The Vibe Coder dies",
        "floor 1 cleared",
    ] {
        assert!(log.contains(beat), "missing beat {beat:?}");
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GOLDEN);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        match std::fs::write(&path, &log) {
            Ok(()) => return,
            Err(e) => panic!("cannot write golden {}: {e}", path.display()),
        }
    }
    let golden = match std::fs::read_to_string(&path) {
        Ok(g) => g,
        Err(e) => panic!(
            "missing golden {} ({e}); run with UPDATE_GOLDEN=1",
            path.display()
        ),
    };
    assert!(
        log == golden,
        "floor 1 log drifted; if intended, rerun with UPDATE_GOLDEN=1"
    );
}

// ---- Economy: gear across the campaign (economy.rs, docs/FORMULAS.md Gear) ----

/// Zones in campaign order; each is played at its max level.
const CAMPAIGN: [&str; 10] = [
    "campus",
    "tower/1",
    "tower/2",
    "tower/3",
    "tower/4",
    "datacenter/1",
    "datacenter/2",
    "tower/5",
    "lab/1",
    "lab/2",
];

impl Bot {
    fn role(&self) -> Option<combat_core::Role> {
        self.sim.class_of(self.player).map(|c| c.role)
    }

    /// The simple bot rule: wear whatever in the bag scores higher for the class's role.
    fn equip_upgrades(&mut self) {
        let role = self.role();
        let me = self.me();
        let eco = &self.sim.content().economy;
        let score = |n: &str| eco.gear(n).map_or(0, |g| eco.score(g, role));
        let mut picks: Vec<String> = Vec::new();
        for name in &me.bag {
            let Some(g) = eco.gear(name) else { continue };
            let worn = &me.equipped[g.slot.index()];
            let rival = picks
                .iter()
                .find(|p| eco.gear(p).is_some_and(|o| o.slot == g.slot))
                .cloned();
            let best = rival.as_deref().map_or_else(|| score(worn), score);
            if score(name) > best {
                picks.retain(|p| eco.gear(p).is_none_or(|o| o.slot != g.slot));
                picks.push(name.clone());
            }
        }
        for item in picks {
            self.sim.apply_as(self.player, Command::Equip { item });
            self.tick();
        }
    }

    /// Between zones: a vendor appears next to the bot (as if visited). It sells the junk and
    /// every piece that is no upgrade, then buys the best affordable upgrade per slot.
    fn visit_vendor(&mut self, zone: &str) {
        let key = if zone.starts_with("tower/") || zone == "campus" {
            "procurement_officer"
        } else {
            "hardware_broker"
        };
        let Ok(v) = self.sim.spawn_unit(key, self.pos, 0.0) else {
            return;
        };
        self.tick();
        let role = self.role();
        let me = self.me();
        let eco = self.sim.content().economy.clone();
        let worn = |slot: usize| {
            eco.gear(&me.equipped[slot])
                .map_or(0, |w| eco.score(w, role))
        };
        let keep = |n: &str| match eco.gear(n) {
            Some(g) => eco.score(g, role) > worn(g.slot.index()),
            None => eco.consumable(n).is_some(),
        };
        for item in me
            .bag
            .iter()
            .filter(|n| !keep(n))
            .cloned()
            .collect::<Vec<_>>()
        {
            self.sim
                .apply_as(self.player, Command::Sell { vendor: v, item });
            self.tick();
        }
        let now_ms = combat_core::ticks_to_ms(self.sim.now().0);
        let Some(def) = eco.vendor_for(key) else {
            return;
        };
        let mut stock: Vec<&combat_core::GearDef> = eco
            .stock(def, now_ms)
            .iter()
            .filter_map(|n| eco.gear(n))
            .filter(|g| eco.score(g, role) > worn(g.slot.index()))
            .collect();
        stock.sort_by_key(|g| std::cmp::Reverse(eco.score(g, role)));
        let mut bought: Vec<combat_core::Slot> = Vec::new();
        for g in stock {
            if bought.contains(&g.slot) || self.me().sp < g.price() {
                continue;
            }
            bought.push(g.slot);
            let item = g.name.clone();
            self.sim
                .apply_as(self.player, Command::Buy { vendor: v, item });
            self.tick();
        }
        self.equip_upgrades();
    }

    /// Debug: the best World Rare of `tier` for the role in every slot (a "well-geared" player).
    fn wear_tier_gear(&mut self, tier: u8) {
        let role = self.role();
        let eco = self.sim.content().economy.clone();
        for slot in combat_core::Slot::ALL {
            let best = eco
                .gear
                .iter()
                .filter(|g| {
                    g.slot == slot
                        && g.tier() == tier
                        && g.rarity == combat_core::Rarity::Rare
                        && g.source == combat_core::GearSource::World
                })
                .max_by_key(|g| eco.score(g, role));
            if let Some(g) = best {
                self.sim.debug_equip(self.player, &g.name);
            }
        }
    }

    /// `run` without consuming the bot: packs, then the boss, equipping drops after each pull.
    fn run_geared(&mut self, zone: &str) {
        let def = self.sim.content().zone(zone).cloned();
        let packs = def.as_ref().map_or(0, |z| z.packs.len());
        let has_boss = def.is_some_and(|z| z.boss.is_some());
        for k in 1..=packs {
            if self.report.died {
                return;
            }
            if let Some((at, _)) = anchor(&format!("pack_{k}")) {
                self.pull(at, false);
                self.equip_upgrades();
            }
        }
        if has_boss && let Some((at, _)) = anchor("boss") {
            self.rest(true);
            self.pull(at, true);
            self.equip_upgrades();
        }
    }
}

/// (zone, boss-fight min HP %, boss-fight seconds, died) per zone of one campaign.
type ZoneLine = (String, u32, u64, bool);

/// The whole campaign for one class and seed, carrying the character (gear, bag, SP) from zone
/// to zone through a save, as the client does across sessions.
/// Also returns how many quests the bot closed on the way.
fn campaign(class: &'static str, seed: u64) -> (Vec<ZoneLine>, combat_core::UnitState, usize) {
    campaign_build(class, seed, 0)
}

/// `campaign` with Career Ladder option `build` (1..=3) taken in every row as soon as the level
/// allows (through the same command the talent window sends); 0 takes no talents.
fn campaign_build(
    class: &'static str,
    seed: u64,
    build: u8,
) -> (Vec<ZoneLine>, combat_core::UnitState, usize) {
    let mut save: Option<combat_core::SaveData> = None;
    let mut lines = Vec::new();
    let mut last = None;
    let quests: Vec<String> = content().quests.iter().map(|q| q.id.clone()).collect();
    let quests: Vec<&str> = quests.iter().map(String::as_str).collect();
    for zone in CAMPAIGN {
        let max = content().zone(zone).map_or(1, |z| z.level_range.1);
        let mut bot = Bot::new_as(class, zone, max, seed);
        bot.looting = true;
        if let Some(s) = &save {
            assert!(bot.sim.import_save(bot.player, s));
            bot.sim.set_player_level(max);
            bot.visit_vendor(zone);
        }
        if build > 0 {
            for row in 0..combat_core::TALENT_ROWS as u8 {
                bot.sim
                    .apply_as(bot.player, Command::PickTalent { row, choice: build });
            }
            bot.tick();
        }
        // Quests on the way: hand in breadcrumbs and pick up the zone's work on arrival,
        // close what is done after the clear (twice, for chains that open on turn-in).
        bot.visit_givers(zone, &quests);
        bot.run_geared(zone);
        bot.visit_givers(zone, &quests);
        bot.visit_givers(zone, &quests);
        let secs = bot.report.after.last().map_or(0, |a| a.2);
        lines.push((
            zone.to_string(),
            bot.report.boss_min_hp_pct,
            secs,
            bot.report.died,
        ));
        save = bot.sim.export_save(bot.player);
        last = Some(bot.me());
        if bot.report.died {
            break;
        }
    }
    let Some(me) = last else {
        panic!("empty campaign")
    };
    let done = save.map_or(0, |s| s.quests_done.len());
    (lines, me, done)
}

/// Every class clears the campaign on three seeds while equipping what drops.
#[test]
fn every_class_clears_the_campaign_equipping_drops() {
    let mut failures = Vec::new();
    for class in ["fullstack", "frontend", "backend", "devops"] {
        let mut lowest: Vec<(String, u32)> = Vec::new();
        for seed in [7, 42, 1234] {
            let (lines, me, done) = campaign(class, seed);
            println!("{class} seed {seed}: {lines:?}, {done} quests closed");
            // One pass, no farming or backtracking: most quests still close on the way.
            let total = content().quests.len();
            assert!(
                done * 5 >= total * 3,
                "{class} seed {seed}: only {done}/{total} quests closed on the way"
            );
            for (zone, min, _, died) in &lines {
                if *died {
                    failures.push(format!("{class} seed {seed}: died in {zone}"));
                }
                if content().zone(zone).is_some_and(|z| z.boss.is_none()) {
                    continue;
                }
                match lowest.iter_mut().find(|(z, _)| z == zone) {
                    Some((_, m)) => *m = (*m).min(*min),
                    None => lowest.push((zone.clone(), *min)),
                }
            }
            let worn: Vec<&str> = me.equipped.iter().map(String::as_str).collect();
            println!(
                "{class} seed {seed}: {} SP, {} in bag, wearing {worn:?}",
                me.sp,
                me.bag.len()
            );
            assert!(me.sp > 0, "{class}: no Story Points over a whole campaign");
            assert!(
                me.equipped.iter().filter(|e| !e.is_empty()).count() >= 3,
                "{class} seed {seed}: barely any gear after the campaign: {worn:?}"
            );
        }
        println!(
            "{class}: lowest boss-fight HP % per zone over 3 seeds (geared by drops): {lowest:?}"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The difference gear makes: each class fights the tier's bosses at max level fresh and in a
/// full set of the tier's Rare pieces for its role. DPS and healers must kill clearly faster;
/// the tank must end its fights clearly healthier (or faster).
#[test]
fn tier_gear_makes_boss_fights_clearly_easier() {
    let zones = [
        ("tower/2", 2u8),
        ("tower/3", 3),
        ("tower/4", 4),
        ("tower/5", 5),
    ];
    let mut table = Vec::new();
    let mut failures = Vec::new();
    for class in ["fullstack", "frontend", "backend", "devops"] {
        let (mut bare_secs, mut geared_secs, mut bare_hp, mut geared_hp) = (0u64, 0u64, 0u64, 0u64);
        for (zone, tier) in zones {
            let max = content().zone(zone).map_or(1, |z| z.level_range.1);
            for seed in [7, 42, 1234] {
                let (b, _) = Bot::new_as(class, zone, max, seed).run(zone);
                let mut g = Bot::new_as(class, zone, max, seed);
                g.wear_tier_gear(tier);
                let (gr, _) = g.run(zone);
                let secs = |r: &Report| r.after.last().map_or(0, |a| a.2);
                bare_secs += secs(&b);
                geared_secs += secs(&gr);
                bare_hp += u64::from(b.boss_min_hp_pct);
                geared_hp += u64::from(gr.boss_min_hp_pct);
                table.push(format!(
                    "{class:9} {zone:8} seed {seed:4}: fresh {:3}s min HP {:3}% | geared {:3}s min HP {:3}%",
                    secs(&b),
                    b.boss_min_hp_pct,
                    secs(&gr),
                    gr.boss_min_hp_pct
                ));
            }
        }
        let pct = geared_secs * 100 / bare_secs.max(1);
        let n = (zones.len() * 3) as u64;
        table.push(format!(
            "{class}: boss time with tier gear = {pct}% of fresh; average boss-fight min HP {}% -> {}%",
            bare_hp / n,
            geared_hp / n
        ));
        let ok = if class == "backend" {
            pct <= 85 || geared_hp >= bare_hp + 10 * n
        } else {
            pct <= 85
        };
        if !ok {
            failures.push(format!("{class}: gear barely mattered ({pct}% time)"));
        }
    }
    println!("{}", table.join("\n"));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- sprint mode balance ----

/// How one sprint went for the balance tests (all of it shows in the printed table).
#[derive(Debug)]
#[allow(dead_code)] // some fields are only read through Debug
struct SprintRun {
    result: Option<combat_core::SprintResult>,
    time_ms: u32,
    par_ms: u32,
    low_hp_pct: u32,
    deaths: u16,
    forces_pct: u8,
    affixes: Vec<String>,
}

impl Bot {
    /// A whole sprint with the bot as host: it loads each stage when the sim asks, pulls the
    /// nearest enemy until the stage is empty and takes the respawn when it dies.
    fn sprint(&mut self, building: &str, level: u8) -> SprintRun {
        self.sim.push(Command::StartSprint {
            building: building.into(),
            level,
        });
        self.tick();
        let deadline = self.sim.now().0 + 40 * 60 * 30;
        while self.sim.sprint_last_result().is_none() && self.sim.now().0 < deadline {
            if let Some(z) = self.sim.sprint_travel().map(str::to_string) {
                if let Err(e) = self.sim.load_zone(&z, &anchor, Vec::new()) {
                    panic!("load_zone {z}: {e}");
                }
                self.tick();
                continue;
            }
            if self.report.died || !self.me().alive {
                self.sim.push(Command::RespawnPlayer);
                self.tick();
                self.pos = self.me().pos;
                self.report.died = false;
                continue;
            }
            let pos = self.pos;
            // Trash first (the scope), the boss last; an evading pack is waited for.
            let left: Vec<combat_core::UnitState> = self
                .sim
                .units()
                .filter(|u| u.hostile && u.alive && u.ai.is_some())
                .cloned()
                .collect();
            let evading = left
                .iter()
                .any(|u| u.ai == Some(combat_core::AiState::Evade));
            let next = left
                .iter()
                .filter(|u| u.ai != Some(combat_core::AiState::Evade))
                .min_by(|a, b| {
                    a.boss.cmp(&b.boss).then(
                        combat_core::distance(a.pos, pos)
                            .total_cmp(&combat_core::distance(b.pos, pos)),
                    )
                })
                .filter(|u| !u.boss || !evading)
                .map(|u| (u.pos, u.boss));
            match next {
                Some((at, boss)) => {
                    if boss {
                        self.rest(true);
                    }
                    self.pull(at, boss);
                }
                None => self.tick(),
            }
        }
        let st = self.sim.sprint_status().unwrap_or_default();
        let last = self.sim.sprint_last_result();
        SprintRun {
            result: last.map(|l| l.0),
            time_ms: last.map_or(st.time_ms(), |l| l.1),
            par_ms: st.par_ms,
            low_hp_pct: self.low_hp_pct,
            deaths: st.deaths,
            forces_pct: st.forces_pct,
            affixes: st.affixes.clone(),
        }
    }
}

/// A level-10 character of `class` in the campus in a full tier-5 Rare set, ready for sprints.
fn sprinter(class: &'static str, seed: u64) -> Bot {
    let mut b = Bot::new_as(class, "campus", 10, seed);
    b.wear_tier_gear(5);
    b.low_hp_pct = 100;
    b.rest_alert = true;
    b
}

const SPRINT_CLASSES: [&str; 4] = ["fullstack", "frontend", "backend", "devops"];

/// Every class solo delivers sprint levels 1-3 within par over three seeds (one building per
/// seed, so all three buildings are covered).
#[test]
fn every_class_delivers_low_sprints_solo() {
    let mut failures = Vec::new();
    for class in SPRINT_CLASSES {
        for (seed, building) in [(7, "tower"), (42, "datacenter"), (1234, "lab")] {
            for level in 1..=3 {
                let r = sprinter(class, seed).sprint(building, level);
                println!("{class:9} {building:10} +{level} seed {seed:4}: {r:?}");
                if r.result != Some(combat_core::SprintResult::Delivered) {
                    failures.push(format!("{class} {building} +{level} seed {seed}: {r:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Level 10 (four affixes, +72% HP and damage) must not be a walk in the park: someone drops
/// under half HP or the clock gets close to par.
#[test]
fn sprint_level_ten_is_hard() {
    let mut failures = Vec::new();
    for class in SPRINT_CLASSES {
        let r = sprinter(class, 7).sprint("tower", 10);
        println!("{class:9} tower +10: {r:?}");
        let near_par = u64::from(r.time_ms) * 100 >= u64::from(r.par_ms) * 75;
        if r.low_hp_pct >= 50 && !near_par && r.deaths == 0 {
            failures.push(format!("{class}: +10 was trivial: {r:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Career Ladder balance: every class clears the campaign on three seeds with each of its three
/// builds (option 1, 2 or 3 in every row). No build may make boss fights more than ~25% faster
/// than the strongest class does without talents.
#[test]
fn every_talent_build_clears_the_campaign() {
    let boss_secs = |lines: &[ZoneLine]| -> u64 {
        lines
            .iter()
            .filter(|(z, ..)| content().zone(z).is_some_and(|z| z.boss.is_some()))
            .map(|l| l.2)
            .sum()
    };
    let classes = ["fullstack", "frontend", "backend", "devops"];
    let seeds = [7, 42, 1234];
    let mut base: Vec<(&str, u64)> = Vec::new();
    for class in classes {
        let secs = seeds
            .iter()
            .map(|s| boss_secs(&campaign(class, *s).0))
            .sum();
        base.push((class, secs));
    }
    let fastest = base.iter().map(|b| b.1).min().unwrap_or(1).max(1);
    let mut failures = Vec::new();
    let mut table = vec![format!("no talents, boss seconds over 3 seeds: {base:?}")];
    for class in classes {
        let own = base.iter().find(|b| b.0 == class).map_or(1, |b| b.1);
        for build in 1..=3u8 {
            let mut lowest: Vec<(String, u32)> = Vec::new();
            let mut secs = 0;
            for seed in seeds {
                let (lines, me, _) = campaign_build(class, seed, build);
                assert_eq!(
                    me.talents,
                    [build; combat_core::TALENT_ROWS],
                    "{class} build {build}: picks did not stick"
                );
                secs += boss_secs(&lines);
                for (zone, min, _, died) in &lines {
                    if *died {
                        failures.push(format!("{class} build {build} seed {seed}: died in {zone}"));
                    }
                    if content().zone(zone).is_some_and(|z| z.boss.is_none()) {
                        continue;
                    }
                    match lowest.iter_mut().find(|(z, _)| z == zone) {
                        Some((_, m)) => *m = (*m).min(*min),
                        None => lowest.push((zone.clone(), *min)),
                    }
                }
            }
            let floor = lowest.iter().map(|l| l.1).min().unwrap_or(0);
            table.push(format!(
                "{class:9} build {build}: boss time {}% of own bare, {}% of fastest class; lowest boss HP {floor}% {lowest:?}",
                secs * 100 / own.max(1),
                secs * 100 / fastest
            ));
            if secs * 100 < fastest * 80 {
                failures.push(format!(
                    "{class} build {build}: boss fights {}% of the fastest bare class (cap 80%)",
                    secs * 100 / fastest
                ));
            }
        }
    }
    println!("{}", table.join("\n"));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
