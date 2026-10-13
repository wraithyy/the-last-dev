//! Rules tests for the tower features: effects, auras, AI rules, progression, floors, NPC talk.

mod common;

use combat_core::{
    AuraEnd, CastError, Command, Content, Event, IdlePose, Sim, SpellId, Vec3, ms_to_ticks,
    ticks_to_ms,
};
use common::*;

const BLAME: SpellId = SpellId(6);
const ROLLBACK: SpellId = SpellId(7);
const CAFFEINE: SpellId = SpellId(8);
const DUCK: SpellId = SpellId(9);
const POSTMORTEM: SpellId = SpellId(10);
const HAND: SpellId = SpellId(11);

/// Real content with exact-outcome settings: no misses, no regen, no player spell yells, no crits.
struct W {
    sim: Sim,
    p: u64,
    log: Vec<(u64, Event)>,
}

fn world(level: u8, tweak: impl FnOnce(&mut Content)) -> W {
    let mut c = content();
    c.config.white_hit_pct = 100;
    c.config.spell_hit_pct = 100;
    c.config.regen.hp_pct = 0;
    c.config.regen.mana_pct = 0;
    for s in &mut c.spells {
        s.yells.clear();
    }
    def_mut(&mut c, "senior_engineer").crit_pct = 0;
    tweak(&mut c);
    let mut sim = Sim::new(c, 1);
    let p = sim.spawn_player(Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_player_level(level);
    W {
        sim,
        p,
        log: Vec::new(),
    }
}

impl W {
    fn spawn(&mut self, key: &str, x: f32, z: f32) -> u64 {
        match self.sim.spawn_unit(key, Vec3::new(x, 0.0, z), 0.0) {
            Ok(id) => id,
            Err(e) => panic!("{e}"),
        }
    }

    fn tick(&mut self) {
        let now = self.sim.now().0 + 1;
        for e in self.sim.tick() {
            self.log.push((now, e));
        }
    }

    fn run(&mut self, n: u32) {
        for _ in 0..n {
            self.tick();
        }
    }

    fn run_until(&mut self, max: u32, pred: impl Fn(&Event) -> bool) -> bool {
        let start = self.log.len();
        for _ in 0..max {
            self.tick();
            if self.log[start..].iter().any(|(_, e)| pred(e)) {
                return true;
            }
        }
        false
    }

    fn attack(&mut self, t: u64, spell: SpellId) {
        self.sim.push(Command::SetTarget {
            unit: self.p,
            target: Some(t),
        });
        self.cast(spell);
    }

    fn cast(&mut self, spell: SpellId) {
        self.sim.push(Command::Cast {
            caster: self.p,
            spell,
        });
    }

    fn u(&self, id: u64) -> combat_core::UnitState {
        match self.sim.unit(id) {
            Some(u) => u.clone(),
            None => panic!("no unit {id}"),
        }
    }

    fn has(&self, pred: impl Fn(&Event) -> bool) -> bool {
        self.log.iter().any(|(_, e)| pred(e))
    }

    fn failed(&self, spell: SpellId) -> Vec<CastError> {
        self.log
            .iter()
            .filter_map(|(_, e)| match e {
                Event::CastFailed {
                    spell: s, reason, ..
                } if *s == spell => Some(*reason),
                _ => None,
            })
            .collect()
    }
}

fn aura_on(target: u64, key: &'static str) -> impl Fn(&Event) -> bool {
    move |e| matches!(e, Event::AuraApplied { target: t, key: k } if *t == target && k == key)
}

fn anchor(name: &str) -> Option<(Vec3, f32)> {
    let num = |p: &str| name.strip_prefix(p).and_then(|n| n.parse::<f32>().ok());
    if let Some(n) = num("pack_") {
        return Some((Vec3::new(0.0, 0.0, -70.0 * n), 0.0));
    }
    if let Some(n) = num("npc_") {
        return Some((Vec3::new(40.0, 0.0, -15.0 * n), 0.0));
    }
    (name == "boss").then_some((Vec3::new(0.0, 0.0, -490.0), 0.0))
}

// ---- crowd control on the player ----

#[test]
fn stun_blocks_player_casts_with_wow_error() {
    let mut w = world(10, |_| {});
    let mm = w.spawn("middle_manager", 0.0, -2.0);
    assert!(w.run_until(300, aura_on(w.p, "quick_sync")));
    assert!(w.u(w.p).stunned);
    w.attack(mm, CR);
    w.tick();
    assert_eq!(w.failed(CR), [CastError::Stunned]);
    assert_eq!(CastError::Stunned.message(), "Can't do that while stunned.");
}

#[test]
fn silence_blocks_player_casts() {
    let mut w = world(10, |_| {});
    let hr = w.spawn("hr_partner", 0.0, -2.0);
    assert!(w.run_until(400, aura_on(w.p, "wellness_survey")));
    assert!(w.u(w.p).silenced);
    w.attack(hr, CR);
    w.tick();
    assert_eq!(w.failed(CR), [CastError::Silenced]);
    assert_eq!(
        CastError::Silenced.message(),
        "Can't do that while silenced."
    );
}

#[test]
fn requirements_change_locks_the_school_of_the_last_spell() {
    let mut w = world(10, |_| {});
    let an = w.spawn("analyst", 0.0, -20.0);
    w.attack(an, CR);
    let lock =
        |e: &Event| matches!(e, Event::CastSucceeded { spell, .. } if *spell == SpellId(117));
    assert!(w.run_until(600, lock));
    w.tick();
    assert_eq!(w.sim.cooldown(w.p, CR).usable, Err(CastError::SchoolLocked));
    assert_ne!(
        w.sim.cooldown(w.p, HAND).usable,
        Err(CastError::SchoolLocked),
        "physical spells stay usable"
    );
}

// ---- progression ----

#[test]
fn unlearned_spells_fail_and_known_spells_follow_level() {
    let mut w = world(1, |_| {});
    assert_eq!(w.sim.known_spells(w.p), [CR, RF]);
    let mp = w.spawn("meatproxy", 0.0, -20.0);
    w.attack(mp, POSTMORTEM);
    w.tick();
    assert_eq!(w.failed(POSTMORTEM), [CastError::NotLearned]);
    w.sim.set_player_level(10);
    assert_eq!(w.sim.known_spells(w.p).len(), 10);
    w.sim.set_player_level(99);
    assert_eq!(w.u(w.p).level, 10);
}

#[test]
fn kill_xp_scales_with_level_difference_and_gray_gives_nothing() {
    let c = content();
    assert_eq!(c.kill_xp(3, 3, 100), 100);
    assert_eq!(c.kill_xp(3, 5, 100), 110);
    assert_eq!(c.kill_xp(3, 9, 100), 120, "capped at +4 levels");
    assert_eq!(c.kill_xp(3, 2, 100), 80);
    assert_eq!(c.kill_xp(3, 1, 100), 60);
    assert_eq!(c.kill_xp(4, 1, 100), 0, "gray");
}

#[test]
fn kill_levels_up_heals_to_full_and_teaches_spells() {
    let mut w = world(1, |c| {
        let d = def_mut(c, "meatproxy");
        d.max_hp = 1;
        d.xp_reward = 500;
    });
    let mp = w.spawn("meatproxy", 0.0, -20.0);
    w.attack(mp, CR);
    assert!(w.run_until(80, |e| matches!(e, Event::LevelUp { .. })));
    let order: Vec<&Event> = w
        .log
        .iter()
        .map(|(_, e)| e)
        .filter(|e| {
            matches!(
                e,
                Event::XpGained { .. } | Event::LevelUp { .. } | Event::SpellLearned { .. }
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            &Event::XpGained {
                unit: w.p,
                amount: 500
            },
            &Event::LevelUp {
                unit: w.p,
                level: 2
            },
            &Event::SpellLearned {
                unit: w.p,
                spell: RPR
            },
        ]
    );
    let p = w.u(w.p);
    assert_eq!((p.level, p.xp, p.xp_to_next), (2, 100, 600));
    assert_eq!((p.hp, p.max_hp, p.max_mana), (335, 335, 290));
}

#[test]
fn gray_kill_gives_no_xp() {
    let mut w = world(5, |c| def_mut(c, "meatproxy").max_hp = 1);
    let mp = w.spawn("meatproxy", 0.0, -20.0);
    w.attack(mp, CR);
    assert!(w.run_until(80, |e| matches!(e, Event::Death { .. })));
    assert!(!w.has(|e| matches!(e, Event::XpGained { .. })));
}

// ---- floors ----

#[test]
fn load_floor_spawns_packs_scaled_by_level_and_reload_despawns() {
    let mut w = world(3, |_| {});
    assert!(w.sim.load_floor(9, &anchor, Vec::new()).is_err());
    let missing = |a: &str| if a == "npc_3" { None } else { anchor(a) };
    assert!(w.sim.load_floor(1, &missing, Vec::new()).is_err());
    assert_eq!(w.sim.units().count(), 1, "a failed load changes nothing");

    assert!(w.sim.load_floor(1, &anchor, Vec::new()).is_ok());
    w.tick();
    assert!(w.has(|e| matches!(e, Event::FloorLoaded { floor: 1 })));
    assert_eq!(w.sim.floor(), Some(1));
    let others: Vec<_> = w.sim.units().filter(|u| u.id != w.p).cloned().collect();
    // 10 pack units, the boss, two IT Support (the intern moved to the campus).
    assert_eq!(others.len(), 10 + 1 + 2);
    let mps: Vec<(u8, u32)> = others
        .iter()
        .filter(|u| u.key == "meatproxy")
        .map(|u| (u.level, u.max_hp))
        .collect();
    assert_eq!(
        mps,
        [(1, 220), (2, 246), (2, 246)],
        "packs 1-2 low level, 3-4 high"
    );
    let boss = others.iter().find(|u| u.key == "vibe_coder").cloned();
    assert!(boss.is_some_and(|b| b.boss && b.level == 2 && b.pack.is_some()));
    let pack2: Vec<_> = others
        .iter()
        .filter(|u| u.key == "copilot_drone")
        .map(|u| u.pack)
        .collect();
    assert_ne!(pack2[0], pack2[1], "pack_1 and pack_2 differ");
    assert_eq!(pack2[1], pack2[2], "pack_2 drones share a pack");
    let it = others.iter().find(|u| u.key == "it_support").cloned();
    assert!(it.is_some_and(|u| u.idle_pose == Some(IdlePose::Chatting) && !u.hostile));

    assert!(w.sim.load_floor(2, &anchor, Vec::new()).is_ok());
    w.tick();
    let despawned = w
        .log
        .iter()
        .filter(|(_, e)| matches!(e, Event::Despawned { .. }))
        .count();
    assert_eq!(despawned, 13);
    assert!(others.iter().all(|u| w.sim.unit(u.id).is_none()));
    assert_eq!(w.u(w.p).level, 3, "the player keeps its level");
}

#[test]
fn pulling_one_pack_member_pulls_the_pack_only() {
    let mut w = world(3, |_| {});
    assert!(w.sim.load_floor(1, &anchor, Vec::new()).is_ok());
    w.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, -50.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    w.tick();
    let pack = |w: &W, n: u32| -> Vec<u64> {
        let Some((at, _)) = anchor(&format!("pack_{n}")) else {
            return Vec::new();
        };
        w.sim
            .units()
            .filter(|u| u.hostile && combat_core::distance(u.pos, at) < 5.0)
            .map(|u| u.id)
            .collect()
    };
    let (p1, p2) = (pack(&w, 1), pack(&w, 2));
    assert_eq!((p1.len(), p2.len()), (2, 3));
    w.attack(p1[0], CR);
    w.run(65);
    assert!(p1.iter().all(|id| w.u(*id).in_combat));
    assert!(p2.iter().all(|id| !w.u(*id).in_combat));
}

#[test]
fn boss_death_clears_the_floor_and_floor_units_never_respawn() {
    let mut w = world(3, |c| def_mut(c, "senior_engineer").spell_power = 100_000);
    assert!(w.sim.load_floor(1, &anchor, Vec::new()).is_ok());
    w.sim.set_respawn_point(Vec3::new(0.0, 0.0, 10.0), 0.0);
    w.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, -470.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    w.tick();
    let boss = w.sim.units().find(|u| u.key == "vibe_coder").map(|u| u.id);
    let Some(boss) = boss else { panic!("no boss") };
    w.attack(boss, CR);
    assert!(w.run_until(80, |e| matches!(e, Event::FloorCleared { floor: 1 })));
    w.run(2500);
    assert!(!w.has(|e| matches!(e, Event::Respawned { .. })));
}

#[test]
fn respawn_point_moves_the_player_respawn() {
    let mut w = world(1, |c| {
        def_mut(c, "senior_engineer").max_hp = 1;
    });
    w.sim.set_respawn_point(Vec3::new(5.0, 0.0, 7.0), 1.0);
    w.spawn("copilot_drone", 0.0, -2.0);
    assert!(w.run_until(200, |e| matches!(e, Event::Death { .. })));
    w.sim.push(Command::RespawnPlayer);
    w.tick();
    assert_eq!(w.u(w.p).pos, Vec3::new(5.0, 0.0, 7.0));
}

// ---- boss mechanics ----

#[test]
fn hp_phase_resolves_its_spell_teleports_and_yells() {
    let mut w = world(1, |c| {
        def_mut(c, "senior_engineer").spell_power = 800;
        def_mut(c, "vp_synergy").max_hp = 1000;
    });
    let vp = w.spawn("vp_synergy", 0.0, -10.0);
    w.attack(vp, CR);
    assert!(w.run_until(150, |e| matches!(e, Event::PhaseChanged { phase: 2, .. })));
    w.tick();
    assert!(w.has(|e| matches!(e, Event::CastSucceeded { caster, spell, .. } if *caster == vp && *spell == SpellId(123))));
    assert!(w.has(|e| matches!(e, Event::Teleported { unit, pos } if *unit == w.p && *pos == Vec3::new(0.0, 0.0, -14.0))));
    assert_eq!(w.u(vp).phase, 2);
    let phase_lines = ["REORG!", "Effective immediately"];
    assert!(w.has(|e| matches!(e, Event::Yell { unit, text } if *unit == vp && phase_lines.iter().any(|l| text.starts_with(l)))));
}

#[test]
fn boss_summons_adds_that_join_the_fight() {
    let mut w = world(2, |_| {});
    let vc = w.spawn("vibe_coder", 0.0, -10.0);
    assert!(w.run_until(400, |e| matches!(e, Event::Summoned { .. })));
    w.tick();
    let adds: Vec<u64> = w
        .log
        .iter()
        .filter_map(|(_, e)| match e {
            Event::Summoned { unit, summoner } if *summoner == vc => Some(*unit),
            _ => None,
        })
        .collect();
    assert_eq!(adds.len(), 2);
    for a in adds {
        let u = w.u(a);
        assert_eq!(
            (u.key.as_str(), u.level, u.in_combat),
            ("unreviewed_pr", 2, true)
        );
    }
}

#[test]
fn rug_pull_emits_knockback_away_from_the_caster() {
    let mut w = world(10, |_| {});
    w.spawn("crypto_bro", 0.0, -3.0);
    let kb = |e: &Event| matches!(e, Event::Knockback { .. });
    assert!(w.run_until(400, kb));
    let found = w.log.iter().find_map(|(_, e)| match e {
        Event::Knockback {
            unit,
            dir,
            strength,
        } => Some((*unit, *dir, *strength)),
        _ => None,
    });
    let Some((unit, dir, strength)) = found else {
        panic!()
    };
    assert_eq!((unit, strength), (w.p, 8.0));
    assert!(dir.z > 0.99, "pushed toward +Z, away from the bro: {dir:?}");
}

#[test]
fn deadline_explodes_unless_rolled_back() {
    let mut w = world(10, |_| {});
    w.spawn("project_manager", 0.0, -3.0);
    assert!(w.run_until(300, aura_on(w.p, "deadline")));
    let boom = |e: &Event| matches!(e, Event::Damage { spell: Some(s), amount: 110, .. } if *s == SpellId(109));
    assert!(w.run_until(250, boom));

    let mut w = world(10, |_| {});
    w.spawn("project_manager", 0.0, -3.0);
    assert!(w.run_until(300, aura_on(w.p, "deadline")));
    w.cast(ROLLBACK);
    w.run(2);
    assert!(w.has(|e| matches!(e, Event::AuraRemoved { key, reason: AuraEnd::Consumed, .. } if key == "deadline")));
    assert!(w.has(|e| matches!(e, Event::Heal { spell: Some(s), .. } if *s == ROLLBACK)));
    w.run(300);
    assert!(!w.has(|e| matches!(e, Event::Damage { spell: Some(s), .. } if *s == SpellId(109))));
}

#[test]
fn postmortem_triples_below_25_percent() {
    let dmg = |w: &W| {
        w.log.iter().find_map(|(_, e)| match e {
            Event::Damage {
                spell: Some(s),
                amount,
                ..
            } if *s == POSTMORTEM => Some(*amount),
            _ => None,
        })
    };
    let mut w = world(10, |c| def_mut(c, "meatproxy").max_hp = 100_000);
    let mp = w.spawn("meatproxy", 0.0, -20.0);
    w.attack(mp, POSTMORTEM);
    w.run(2);
    assert_eq!(dmg(&w), Some(50 + 110 * 60 / 100));

    let mut w = world(10, |c| def_mut(c, "meatproxy").max_hp = 500);
    let mp = w.spawn("meatproxy", 0.0, -20.0);
    w.attack(mp, HAND);
    w.run(125);
    assert!(w.u(mp).hp * 4 < w.u(mp).max_hp);
    w.cast(POSTMORTEM);
    w.run(2);
    assert_eq!(dmg(&w), Some((50 + 110 * 60 / 100) * 3));
}

#[test]
fn git_blame_ticks_and_spreads_to_the_nearest_enemy_on_death() {
    let mut w = world(10, |c| {
        let d = def_mut(c, "meatproxy");
        d.max_hp = 10;
        if let Some(ai) = d.ai.as_mut() {
            ai.chase_speed_mps = 0.0;
        }
    });
    let a = w.spawn("meatproxy", 0.0, -20.0);
    let b = w.spawn("meatproxy", 3.0, -20.0);
    w.attack(a, BLAME);
    assert!(w.run_until(
        100,
        |e| matches!(e, Event::Death { unit, .. } if *unit == a)
    ));
    assert!(w.has(
        |e| matches!(e, Event::Damage { spell: Some(s), target, .. } if *s == BLAME && *target == a)
    ));
    assert!(w.has(aura_on(b, "git_blame")));
}

#[test]
fn meatproxy_forwards_the_last_spell_at_half_damage() {
    let mut w = world(1, |c| def_mut(c, "meatproxy").max_hp = 100_000);
    let mp = w.spawn("meatproxy", 0.0, -20.0);
    w.attack(mp, CR);
    let fwd = move |e: &Event| matches!(e, Event::Damage { source, spell: Some(s), .. } if *source == mp && *s == SpellId(100));
    assert!(w.run_until(500, fwd));
    let amount = w.log.iter().find_map(|(_, e)| match e {
        Event::Damage {
            source,
            spell: Some(s),
            amount,
            ..
        } if *source == mp && *s == SpellId(100) => Some(*amount),
        _ => None,
    });
    assert_eq!(amount, Some(79 / 2));
}

#[test]
fn rubber_duck_interrupts_and_bosses_are_immune() {
    let mut w = world(10, |c| def_mut(c, "meatproxy").max_hp = 100_000);
    // Inside the trash aggro radius so it pulls on its own.
    let mp = w.spawn("meatproxy", 0.0, -5.0);
    let started = move |e: &Event| matches!(e, Event::CastStarted { caster, .. } if *caster == mp);
    assert!(w.run_until(300, started));
    w.attack(mp, DUCK);
    w.tick();
    assert!(w.has(|e| matches!(e, Event::CastInterrupted { caster, by: Some(b), .. } if *caster == mp && *b == w.p)));
    assert!(w.u(mp).stunned);

    let mut w = world(10, |_| {});
    let boss = w.spawn("vibe_coder", 3.0, -15.0);
    w.sim.push(Command::SetTarget {
        unit: w.p,
        target: Some(boss),
    });
    w.run(60);
    assert_eq!(
        w.sim.cooldown(w.p, DUCK).usable,
        Err(CastError::InvalidTarget)
    );
}

#[test]
fn cast_time_and_haste_auras_change_cast_times() {
    let mut w = world(1, |_| {});
    let j = w.spawn("vibe_junior", 0.0, -3.0);
    assert!(w.run_until(200, aura_on(w.p, "review_fatigue")));
    w.attack(j, CR);
    w.tick();
    let total = w.log.iter().rev().find_map(|(_, e)| match e {
        Event::CastStarted {
            caster, total_ms, ..
        } if *caster == w.p => Some(*total_ms),
        _ => None,
    });
    assert_eq!(total, Some(ticks_to_ms(ms_to_ticks(2400)) as u32));

    let mut w = world(10, |_| {});
    let mp = w.spawn("meatproxy", 0.0, -20.0);
    w.cast(CAFFEINE);
    w.attack(mp, CR);
    w.tick();
    let total = w.log.iter().find_map(|(_, e)| match e {
        Event::CastStarted { total_ms, .. } => Some(*total_ms),
        _ => None,
    });
    assert!(total.is_some_and(|t| t < 1600), "{total:?}");
}

#[test]
fn overpromise_stacks_raise_damage_taken() {
    let mut w = world(10, |_| {});
    let sr = w.spawn("sales_rep", 0.0, -2.0);
    let stacks = |w: &W| {
        w.u(w.p)
            .auras
            .iter()
            .find(|a| a.key == "overpromise")
            .map_or(0, |a| a.stacks)
    };
    for _ in 0..900 {
        w.tick();
        if stacks(&w) == 2 {
            break;
        }
    }
    assert_eq!(stacks(&w), 2);
    let from = w.log.len();
    let melee =
        move |e: &Event| matches!(e, Event::Damage { source, spell: None, .. } if *source == sr);
    assert!(w.run_until(100, melee));
    let amount = w.log[from..].iter().find_map(|(_, e)| match e {
        Event::Damage {
            source,
            spell: None,
            amount,
            ..
        } if *source == sr => Some(*amount),
        _ => None,
    });
    // Two stacks: +10% on the Sales Rep's 14 melee (or +15% if a third landed this tick).
    assert!(matches!(amount, Some(15) | Some(16)), "{amount:?}");
}

#[test]
fn unread_spec_stacks_on_repeated_spells() {
    let mut w = world(10, |_| {});
    let an = w.spawn("analyst", 0.0, -20.0);
    w.attack(an, CR);
    assert!(w.run_until(300, aura_on(w.p, "unread_spec")));
    for _ in 0..2 {
        w.run(70);
        w.cast(CR);
    }
    w.run(70);
    let st = w
        .u(w.p)
        .auras
        .iter()
        .find(|a| a.key == "unread_spec")
        .map(|a| a.stacks);
    assert!(st.is_some_and(|s| s >= 2), "{st:?}");
}

#[test]
fn heal_over_time_heals_the_consultant() {
    let mut w = world(10, |_| {});
    let c = w.spawn("consultant", 0.0, -3.0);
    w.attack(c, CR);
    let hot = move |e: &Event| matches!(e, Event::Heal { source, target, spell: Some(s), amount } if *source == c && *target == c && *s == SpellId(111) && *amount > 0);
    assert!(w.run_until(400, hot));
}

#[test]
fn phishing_test_stuns_the_analyst_without_self_threat() {
    let mut w = world(10, |_| {});
    let sa = w.spawn("security_analyst", 0.0, -3.0);
    assert!(w.run_until(400, aura_on(sa, "phished")));
    assert!(w.has(
        |e| matches!(e, Event::Damage { source, target, .. } if *source == sa && *target == sa)
    ));
    let u = w.u(sa);
    assert!(u.stunned);
    assert_eq!(u.target, Some(w.p));
}

#[test]
fn ethics_review_never_does_anything() {
    let mut w = world(10, |_| {});
    let e = w.spawn("ai_ethicist", 0.0, -3.0);
    w.run(40 * 30);
    assert!(w.has(|x| matches!(x, Event::CastStarted { caster, spell, .. } if *caster == e && *spell == SpellId(112))));
    assert!(!w.has(|x| matches!(x, Event::Damage { spell: Some(s), .. } if *s == SpellId(112))));
}

#[test]
fn chatbot_minion_explodes_on_death() {
    let mut w = world(10, |c| def_mut(c, "chatbot_minion").max_hp = 1);
    let cb = w.spawn("chatbot_minion", 0.0, -3.0);
    w.attack(cb, CR);
    assert!(w.run_until(150, |e| matches!(e, Event::Death { .. })));
    assert!(w.has(|e| matches!(e, Event::Damage { source, target, spell: Some(s), .. } if *source == cb && *target == w.p && *s == SpellId(114))));
}

// ---- friendly NPCs ----

#[test]
fn talk_answers_in_range_and_npcs_chat_on_their_own() {
    let mut w = world(1, |_| {});
    let near = w.spawn("it_support", 2.0, -2.0);
    let far = w.spawn("it_support", 0.0, -50.0);
    let mid = w.spawn("it_support", 0.0, -10.0);
    w.sim.push(Command::Talk { unit: near });
    w.sim.push(Command::Talk { unit: far });
    w.tick();
    let talk = content()
        .unit_def("it_support")
        .map(|d| d.yells.talk.clone())
        .unwrap_or_default();
    let yells: Vec<(u64, String)> = w
        .log
        .iter()
        .filter_map(|(_, e)| match e {
            Event::Yell { unit, text } => Some((*unit, text.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(yells, [(near, talk[0].clone())]);
    w.run(41 * 30);
    let count = |w: &W, id: u64| {
        w.log
            .iter()
            .filter(|(_, e)| matches!(e, Event::Yell { unit, .. } if *unit == id))
            .count()
    };
    assert!(count(&w, mid) >= 1, "idle NPCs in earshot chat unprompted");
    assert_eq!(
        count(&w, far),
        0,
        "ambient banter stays silent out of range"
    );
}

#[test]
fn pack_pull_yields_a_single_aggro_yell() {
    let mut w = world(3, |_| {});
    assert!(w.sim.load_floor(1, &anchor, Vec::new()).is_ok());
    w.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, -50.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    w.tick();
    let Some((at, _)) = anchor("pack_1") else {
        panic!("no pack_2 anchor")
    };
    let pack: Vec<u64> = w
        .sim
        .units()
        .filter(|u| u.hostile && combat_core::distance(u.pos, at) < 5.0)
        .map(|u| u.id)
        .collect();
    assert_eq!(pack.len(), 2);
    w.attack(pack[0], CR);
    w.run(65);
    assert!(pack.iter().all(|id| w.u(*id).in_combat));
    let yells = w
        .log
        .iter()
        .filter(|(_, e)| matches!(e, Event::Yell { unit, .. } if pack.contains(unit)))
        .count();
    assert_eq!(yells, 1, "only the first pack member shouts on aggro");
}

#[test]
fn hostile_yells_share_a_global_gap() {
    let mut w = world(3, |c| {
        for k in ["meatproxy", "vibe_junior", "copilot_drone"] {
            def_mut(c, k).yells.aggro = vec!["Hey!".into()];
        }
    });
    let ids = [
        w.spawn("meatproxy", 0.0, -5.0),
        w.spawn("vibe_junior", 3.0, -5.0),
        w.spawn("copilot_drone", -3.0, -5.0),
    ];
    w.run(40);
    let ticks: Vec<u64> = w
        .log
        .iter()
        .filter_map(|(t, e)| match e {
            Event::Yell { unit, .. } if ids.contains(unit) => Some(*t),
            _ => None,
        })
        .collect();
    assert_eq!(
        ticks.len(),
        1,
        "same-tick aggro yells collapse to one: {ticks:?}"
    );
}

#[test]
fn summoned_adds_give_no_xp() {
    let mut w = world(2, |c| def_mut(c, "unreviewed_pr").max_hp = 1);
    let vc = w.spawn("vibe_coder", 0.0, -10.0);
    assert!(w.run_until(400, |e| matches!(e, Event::Summoned { .. })));
    let add = w.log.iter().find_map(|(_, e)| match e {
        Event::Summoned { unit, summoner } if *summoner == vc => Some(*unit),
        _ => None,
    });
    let Some(add) = add else {
        panic!("no add summoned");
    };
    let xp_before = w.u(w.p).xp;
    w.attack(add, CR);
    assert!(w.run_until(
        200,
        |e| matches!(e, Event::Death { unit, .. } if *unit == add)
    ));
    w.tick();
    assert_eq!(w.u(w.p).xp, xp_before);
}

#[test]
fn loading_a_floor_cancels_the_players_cast() {
    let mut w = world(3, |c| def_mut(c, "meatproxy").max_hp = 100_000);
    let mp = w.spawn("meatproxy", 0.0, -20.0);
    w.attack(mp, CR);
    assert!(w.run_until(30, |e| matches!(e, Event::CastStarted { .. })));
    assert!(w.sim.load_floor(1, &anchor, Vec::new()).is_ok());
    w.run(90);
    let p = w.p;
    assert!(
        w.has(|e| matches!(e, Event::CastInterrupted { caster, by: None, .. } if *caster == p))
    );
    assert!(!w.has(|e| matches!(e, Event::CastSucceeded { caster, .. } if *caster == p)));
}

// ---- adds follow their summoner ----

/// Vibe Coder that summons after one second and is soft enough to kill with spells.
fn quick_summoner(c: &mut Content) {
    let vc = def_mut(c, "vibe_coder");
    vc.max_hp = 400;
    if let Some(ai) = vc.ai.as_mut() {
        ai.abilities[0].initial_ms = Some((1000, 1000));
    }
    def_mut(c, "senior_engineer").max_hp = 100_000;
}

fn summoned_by(w: &W, summoner: u64) -> Vec<u64> {
    w.log
        .iter()
        .filter_map(|(_, e)| match e {
            Event::Summoned { unit, summoner: s } if *s == summoner => Some(*unit),
            _ => None,
        })
        .collect()
}

#[test]
fn adds_die_with_their_summoner_without_xp_or_loot() {
    let mut w = world(10, quick_summoner);
    let vc = w.spawn("vibe_coder", 0.0, -10.0);
    assert!(w.run_until(300, |e| matches!(e, Event::Summoned { .. })));
    w.tick();
    let adds = summoned_by(&w, vc);
    assert_eq!(adds.len(), 2);
    assert!(adds.iter().all(|a| w.u(*a).alive));
    for _ in 0..40 {
        w.attack(vc, CR);
        w.run(75);
        if !w.u(vc).alive {
            break;
        }
    }
    assert!(!w.u(vc).alive, "boss should be dead");
    for a in adds {
        let u = w.u(a);
        assert!(
            !u.alive && !u.lootable,
            "add {a} must be dead and unlootable"
        );
        assert!(w.has(|e| matches!(e, Event::Death { unit, killer: None } if *unit == a)));
    }
}

#[test]
fn floor_boss_adds_despawn_when_the_floor_clears() {
    let mut w = world(10, quick_summoner);
    assert!(w.sim.load_floor(1, &anchor, Vec::new()).is_ok());
    let boss = w.sim.units().find(|u| u.key == "vibe_coder").map(|u| u.id);
    let Some(boss) = boss else {
        panic!("no floor boss");
    };
    w.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, -480.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    assert!(w.run_until(300, |e| matches!(e, Event::Summoned { .. })));
    w.tick();
    let adds = summoned_by(&w, boss);
    assert_eq!(adds.len(), 2);
    for _ in 0..40 {
        w.attack(boss, CR);
        w.run(75);
        if !w.u(boss).alive {
            break;
        }
    }
    assert!(w.has(|e| matches!(e, Event::FloorCleared { .. })));
    for a in adds {
        assert!(w.sim.unit(a).is_none(), "add {a} should be despawned");
        assert!(w.has(|e| matches!(e, Event::Despawned { unit } if *unit == a)));
    }
}

#[test]
fn adds_despawn_when_their_summoner_evades() {
    let mut w = world(10, quick_summoner);
    let vc = w.spawn("vibe_coder", 0.0, -10.0);
    assert!(w.run_until(300, |e| matches!(e, Event::Summoned { .. })));
    w.tick();
    let adds = summoned_by(&w, vc);
    assert_eq!(adds.len(), 2);
    // Out of leash range the boss gives up and resets.
    w.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 200.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    assert!(w.run_until(900, |e| matches!(e, Event::Despawned { .. })));
    for a in adds {
        assert!(w.sim.unit(a).is_none(), "add {a} should be gone");
    }
    assert!(w.u(vc).alive);
}

// ---- idle wandering ----

#[test]
fn idle_hostiles_wander_near_spawn_and_keep_clear_of_walls() {
    let wall = combat_core::Aabb {
        min: Vec3::new(2.0, -1.0, -60.0),
        max: Vec3::new(3.0, 5.0, -20.0),
    };
    let mut w = world(1, |_| {});
    w.sim.set_blockers(vec![wall]);
    let mp = w.spawn("meatproxy", 0.0, -40.0);
    let spawn = w.u(mp).pos;
    let (mut far, mut min_wall) = (0.0_f32, f32::MAX);
    for _ in 0..30 * 90 {
        w.tick();
        let p = w.u(mp).pos;
        far = far.max(((p.x - spawn.x).powi(2) + (p.z - spawn.z).powi(2)).sqrt());
        min_wall = min_wall.min(wall.min.x - p.x);
    }
    assert!(far > 1.0, "never wandered (max {far} m)");
    assert!(far <= 4.01, "left its radius ({far} m)");
    assert!(min_wall >= 0.99, "walked within {min_wall} m of the wall");
    assert_eq!(w.u(mp).ai, Some(combat_core::AiState::Idle));
}

#[test]
fn wandering_never_touches_the_combat_rng() {
    let run = |wander: bool| {
        let mut w = world(10, |c| {
            c.config.white_hit_pct = 85;
            c.config.spell_hit_pct = 85;
        });
        w.sim.set_wandering(wander);
        w.spawn("meatproxy", 30.0, -40.0);
        let ceo = w.spawn("ceo", 0.0, -8.0);
        w.attack(ceo, CR);
        w.run(30 * 40);
        w.log
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn pack_members_stay_within_four_metres_of_the_anchor() {
    let mut w = world(1, |_| {});
    assert!(w.sim.load_floor(1, &anchor, Vec::new()).is_ok());
    let start: Vec<_> = w.sim.units().map(|u| (u.id, u.pos)).collect();
    let mut moved = false;
    for _ in 0..30 * 120 {
        w.tick();
        for u in w.sim.units().filter(|u| u.pack.is_some() && u.hostile) {
            let n = (-u.pos.z / 70.0).round();
            let at = Vec3::new(0.0, 0.0, -70.0 * n);
            let d = ((u.pos.x - at.x).powi(2) + (u.pos.z - at.z).powi(2)).sqrt();
            assert!(d <= 4.01, "{} strayed {d} m from its anchor", u.name);
            let from = start.iter().find(|(id, _)| *id == u.id).map(|(_, p)| *p);
            moved |= from.is_some_and(|f| combat_core::distance(f, u.pos) > 0.5);
        }
    }
    assert!(moved, "nobody in any pack moved");
}

#[test]
fn seated_npcs_stay_put_and_standing_ones_stroll_home() {
    let mut w = world(1, |_| {});
    let sleeper = w.spawn("it_support", 10.0, -10.0);
    let stroller = w.spawn("it_support", -10.0, -10.0);
    w.sim.set_idle_pose(sleeper, Some(IdlePose::Sleeping));
    w.sim.set_idle_pose(stroller, None);
    let home = w.u(stroller).pos;
    let mut far = 0.0_f32;
    for _ in 0..30 * 150 {
        w.tick();
        far = far.max(combat_core::distance(home, w.u(stroller).pos));
    }
    assert_eq!(w.u(sleeper).pos, Vec3::new(10.0, 0.0, -10.0));
    assert!(far > 1.0 && far <= 6.01, "stroll reached {far} m");
}
