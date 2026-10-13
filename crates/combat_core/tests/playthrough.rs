//! Headless playthrough: a scripted "competent player" clears whole zones and does quests.
//!
//! Balance gate: every zone at its max level without dying; the campus quest chain from level 1.
//! Golden: tower/1 from level 1.
//! Regenerate the golden after an intentional change:
//! `UPDATE_GOLDEN=1 cargo test -p combat_core --test playthrough`

mod common;

use combat_core::{Command, Event, QuestStatus, Sim, SpellId, TRASH_RESPAWN_MS, Vec3, ms_to_ticks};
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
    /// Loot every corpse after a pull (quest runs only; the golden predates looting).
    looting: bool,
}

fn yaw_to(from: Vec3, to: Vec3) -> f32 {
    (-(to.x - from.x)).atan2(-(to.z - from.z))
}

impl Bot {
    fn new(zone: &str, level: u8, seed: u64) -> Bot {
        let mut sim = Sim::new(content(), seed);
        // The balance gate and golden pin fight timing, which wandering would shift.
        sim.set_wandering(false);
        let start = Vec3::new(0.0, 0.0, 20.0);
        let player = sim.spawn_player(start, 0.0);
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
            looting: false,
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
            moving: false,
            jumping: false,
        });
        let events = self.sim.tick();
        let now = self.sim.now().0;
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

    /// Hostiles fighting the player: adds (low max HP) first, then the nearest.
    fn pick_target(&self) -> Option<u64> {
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

    fn act(&mut self) {
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
            && !NO_KICK.contains(&c.spell)
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
        while self.sim.now().0 - start < 90 || self.me().in_combat {
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
            self.tick();
        }
    }

    /// Walks to 20 m in front of the anchor, opens on the nearest unit, fights, rests.
    fn pull(&mut self, at: Vec3, boss: bool) {
        self.in_boss = boss;
        if boss {
            self.report.boss_min_hp_pct = 100;
        }
        self.pos = Vec3::new(at.x, 0.0, at.z + 20.0);
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
                let giver = self.sim.content().quest(q).map(|d| d.giver.npc.clone());
                if giver.as_deref() != Some(f.unit.as_str()) {
                    continue;
                }
                let quest = q.to_string();
                match self.sim.quest_status(q) {
                    QuestStatus::Ready => self.sim.push(Command::TurnInQuest { quest }),
                    QuestStatus::Available => self.sim.push(Command::AcceptQuest { quest }),
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

/// Campus from level 1: visit every giver, clear the packs looting everything, hand in, and
/// leave and return after the trash respawn timer until the campus chain is done.
#[test]
fn campus_quest_chain_from_level_one() {
    // Human in the Loop is left out: its MeatProxies are spread over the campus and the tower.
    const CAMPUS: [&str; 5] = [
        "onboarding",
        "badge_access_denied",
        "coffee_run",
        "fire_exit_strategy",
        "content_moderation",
    ];
    for seed in [7, 42, 1234] {
        let mut bot = Bot::new("campus", 1, seed);
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
            "seed {seed}: campus chain in {rounds} clears, level {}, {} trophies, {} rests / {} pulls",
            me.level,
            me.inventory.len(),
            bot.report.rests,
            bot.report.pulls
        );
        assert!(!bot.report.died, "seed {seed}: died on the campus");
        assert!(left.is_empty(), "seed {seed}: unfinished {left:?}");
        assert!(
            rounds <= 3,
            "seed {seed}: {rounds} clears is too much farming"
        );
        // The campus is the tutorial: a fresh hire leaves it at level 3 and walks into tower/1
        // (levels 1-2) slightly over-levelled, never at 4+ with the floor turned grey.
        assert_eq!(
            me.level, 3,
            "seed {seed}: the campus should take a fresh hire to level 3"
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
