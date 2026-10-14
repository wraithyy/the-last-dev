//! Playable classes: per-class stats and bars, threat (the tank holds aggro over DPS), and
//! each class's signature mechanics (Specificity, Hot Reload, taunt, Horizontal Scaling,
//! Rate Limiter, Revert Commit, ally heals, Restart Pod).

mod common;

use combat_core::{CastError, Command, Content, EntityId, Event, Sim, SpellId, Vec3};
use common::*;

const PIXEL_PUSH: SpellId = SpellId(20);
const CSS_COMBO: SpellId = SpellId(21);
const IMPORTANT: SpellId = SpellId(22);
const HOT_RELOAD: SpellId = SpellId(24);
const BLOCKING_CALL: SpellId = SpellId(30);
const RATE_LIMITER: SpellId = SpellId(31);
const TAUNT: SpellId = SpellId(32);
const CONN_POOL: SpellId = SpellId(33);
const HORIZONTAL: SpellId = SpellId(36);
const HOTFIX: SpellId = SpellId(41);
const REVERT: SpellId = SpellId(45);
const BLUE_GREEN: SpellId = SpellId(48);
const RESTART_POD: SpellId = SpellId(50);
const TURN_IT_OFF: SpellId = SpellId(13);

struct T {
    sim: Sim,
    players: Vec<EntityId>,
    mob: EntityId,
    log: Vec<Event>,
}

/// Players of `classes` side by side at z = 0 facing -Z, a sturdy CEO 3 m ahead that only
/// melees for `melee` damage. Hits always land, no regen, no yells.
fn setup(classes: &[&str], melee: u32, tweak: impl FnOnce(&mut Content)) -> T {
    let mut c = content();
    c.config.white_hit_pct = 100;
    c.config.spell_hit_pct = 100;
    c.config.regen.hp_pct = 0;
    c.config.regen.mana_pct = 0;
    for s in &mut c.spells {
        s.yells.clear();
    }
    legacy_ceo(&mut c);
    let ceo = def_mut(&mut c, "ceo");
    ceo.max_hp = 50_000;
    if let Some(ai) = ceo.ai.as_mut() {
        ai.melee_damage = melee;
        ai.abilities.clear();
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, 5);
    sim.set_wandering(false);
    let players: Vec<EntityId> = classes
        .iter()
        .enumerate()
        .map(|(k, class)| {
            let id = sim.spawn_player(class, Vec3::new(k as f32 * 1.5, 0.0, 0.0), 0.0);
            assert_ne!(id, 0, "{class}");
            assert!(sim.set_level_of(id, 10));
            id
        })
        .collect();
    let mob = match sim.spawn_unit("ceo", Vec3::new(0.0, 0.0, -3.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    T {
        sim,
        players,
        mob,
        log: Vec::new(),
    }
}

impl T {
    fn tick(&mut self) {
        self.log.extend(self.sim.tick());
    }

    fn run(&mut self, n: u32) {
        for _ in 0..n {
            self.tick();
        }
    }

    fn target(&mut self, p: EntityId, t: EntityId) {
        self.sim.apply_as(
            p,
            Command::SetTarget {
                unit: p,
                target: Some(t),
            },
        );
    }

    fn cast(&mut self, p: EntityId, spell: SpellId) {
        self.sim.apply_as(p, Command::Cast { caster: p, spell });
    }

    /// Casts `spell` if it is usable right now; true when pushed.
    fn press(&mut self, p: EntityId, spell: SpellId) -> bool {
        let ok = self.sim.cooldown(p, spell).usable.is_ok();
        if ok {
            self.cast(p, spell);
        }
        ok
    }

    fn hp(&self, id: EntityId) -> (u32, u32) {
        self.sim.unit(id).map_or((0, 0), |u| (u.hp, u.max_hp))
    }

    fn mob_target(&self) -> Option<EntityId> {
        self.sim.unit(self.mob).and_then(|u| u.target)
    }

    fn failures(&self, spell: SpellId) -> Vec<CastError> {
        self.log
            .iter()
            .filter_map(|e| match e {
                Event::CastFailed {
                    spell: s, reason, ..
                } if *s == spell => Some(*reason),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn classes_spawn_with_their_own_stats_bars_and_auto_attacks() {
    let c = content();
    let keys: Vec<&str> = c.classes.iter().map(|k| k.key.as_str()).collect();
    assert_eq!(keys, ["fullstack", "frontend", "backend", "devops"]);
    let mut sim = Sim::new(content(), 1);
    assert_eq!(sim.spawn_player("rockstar", Vec3::default(), 0.0), 0);
    let ids: Vec<EntityId> = keys
        .iter()
        .map(|k| sim.spawn_player(k, Vec3::default(), 0.0))
        .collect();
    let stats: Vec<(String, u32, u32)> = ids
        .iter()
        .filter_map(|id| sim.unit(*id))
        .map(|u| (u.key.clone(), u.max_hp, u.max_mana))
        .collect();
    assert_eq!(
        stats,
        [
            ("senior_engineer".to_string(), 300, 250),
            ("frontend_dev".to_string(), 360, 220),
            ("backend_dev".to_string(), 440, 200),
            ("devops_eng".to_string(), 290, 300),
        ]
    );
    for (id, key) in ids.iter().zip(&keys) {
        let class = sim.class_of(*id).map(|c| c.key.clone());
        assert_eq!(class.as_deref(), Some(*key));
        // Two spells at level 1, each class its own bar of ten.
        assert_eq!(sim.known_spells(*id).len(), 2, "{key}");
        assert_eq!(c.class(key).map(|k| k.action_bar.len()), Some(10));
    }
    // Melee classes melee, the casters wand.
    let range = |unit: &str| {
        c.unit_def(unit)
            .and_then(|u| u.auto_attack.as_ref())
            .map(|a| a.range_m)
    };
    assert_eq!(range("frontend_dev"), Some(4.0));
    assert_eq!(range("backend_dev"), Some(4.0));
    assert_eq!(range("devops_eng"), Some(25.0));
    // The tank's level HP grows 60% faster: 440 + 315 * 1.6 at level 10.
    assert!(sim.set_level_of(ids[2], 10));
    assert_eq!(sim.unit(ids[2]).map(|u| u.max_hp), Some(440 + 504));
}

#[test]
fn players_cannot_cast_another_classes_spells() {
    let mut t = setup(&["frontend", "devops"], 1, |_| {});
    let (fe, dev) = (t.players[0], t.players[1]);
    t.target(fe, t.mob);
    t.cast(fe, CR);
    t.cast(fe, HOTFIX);
    t.cast(dev, TURN_IT_OFF);
    t.run(2);
    assert_eq!(t.failures(CR), [CastError::NotLearned]);
    assert_eq!(t.failures(HOTFIX), [CastError::NotLearned]);
    assert_eq!(t.failures(TURN_IT_OFF), [CastError::NotLearned]);
    let res = |key: &str| t.sim.content().resurrect_for_unit(key).map(|s| s.id);
    assert_eq!(res("devops_eng"), Some(RESTART_POD));
    assert_eq!(res("backend_dev"), Some(TURN_IT_OFF));
}

/// Tank pulls, the Fullstack starts nuking 2 s later; over 30 s the mob never leaves the tank.
/// The same fight with the tank's threat modifier at 100% loses aggro to the DPS.
#[test]
fn backend_holds_aggro_over_a_fullstack_dps() {
    let fight = |threat_pct: u32| -> (usize, u32, u32) {
        let mut t = setup(&["backend", "fullstack"], 1, |c| {
            if let Some(k) = c.classes.iter_mut().find(|k| k.key == "backend") {
                k.threat_pct = threat_pct;
            }
        });
        let (tank, dps) = (t.players[0], t.players[1]);
        t.target(tank, t.mob);
        t.target(dps, t.mob);
        t.sim
            .apply_as(tank, Command::StartAutoAttack { unit: tank });
        let mut lost = 0;
        let mut dps_damage = 0;
        let mut tank_damage = 0;
        for k in 0..900 {
            t.press(tank, BLOCKING_CALL);
            if k > 60 && !t.press(dps, RF) {
                t.press(dps, CR);
            }
            let start = t.log.len();
            t.tick();
            for e in &t.log[start..] {
                if let Event::Damage { source, amount, .. } = e {
                    if *source == dps {
                        dps_damage += amount;
                    } else if *source == tank {
                        tank_damage += amount;
                    }
                }
            }
            if k > 60 && t.mob_target() != Some(tank) {
                lost += 1;
            }
        }
        (lost, tank_damage, dps_damage)
    };
    let (lost, tank_dmg, dps_dmg) = fight(350);
    assert!(
        dps_dmg > tank_dmg,
        "the DPS must out-damage the tank: {dps_dmg} vs {tank_dmg}"
    );
    assert_eq!(lost, 0, "tank lost aggro for {lost} ticks");
    let (lost_flat, _, _) = fight(100);
    assert!(
        lost_flat > 0,
        "without the modifier the DPS should pull aggro"
    );
}

#[test]
fn who_touched_prod_takes_aggro_at_once_and_connection_pool_threatens_everyone() {
    let mut t = setup(&["fullstack", "backend"], 1, |_| {});
    let (dps, tank) = (t.players[0], t.players[1]);
    t.target(dps, t.mob);
    t.cast(dps, CR);
    t.run(80);
    assert_eq!(t.mob_target(), Some(dps));
    t.target(tank, t.mob);
    t.cast(tank, TAUNT);
    t.run(2);
    assert_eq!(t.mob_target(), Some(tank));

    // Connection Pool: every enemy within 8 m starts on the tank.
    let mut t = setup(&["backend"], 1, |_| {});
    let tank = t.players[0];
    let extra = match t.sim.spawn_unit("ceo", Vec3::new(4.0, 0.0, -2.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    t.sim.push(Command::SetTarget {
        unit: tank,
        target: Some(t.mob),
    });
    t.cast(tank, CONN_POOL);
    t.run(2);
    for m in [t.mob, extra] {
        assert_eq!(t.sim.unit(m).and_then(|u| u.target), Some(tank));
    }
}

#[test]
fn specificity_stacks_and_important_cashes_them_in() {
    let mut t = setup(&["frontend"], 0, |_| {});
    let fe = t.players[0];
    t.target(fe, t.mob);
    let stacks = |t: &T| {
        t.sim
            .unit(t.mob)
            .and_then(|u| {
                u.auras
                    .iter()
                    .find(|a| a.key == "specificity")
                    .map(|a| a.stacks)
            })
            .unwrap_or(0)
    };
    for spell in [PIXEL_PUSH, CSS_COMBO, PIXEL_PUSH] {
        t.cast(fe, spell);
        t.run(46);
    }
    assert_eq!(stacks(&t), 4);
    let before = t.log.len();
    t.cast(fe, IMPORTANT);
    t.run(2);
    assert_eq!(stacks(&t), 0);
    let hit = t.log[before..].iter().find_map(|e| match e {
        Event::Damage {
            spell: Some(s),
            amount,
            combo,
            crit,
            ..
        } if *s == IMPORTANT => Some((*amount, *combo, *crit)),
        _ => None,
    });
    // 30 + 50% of 110 spell power = 85, +60% per stack: x3.4.
    let Some((amount, combo, crit)) = hit else {
        panic!("no !important hit")
    };
    assert!(combo);
    let base = 85 * 340 / 100;
    assert_eq!(amount, if crit { base * 150 / 100 } else { base });
}

#[test]
fn hot_reload_dashes_to_the_target() {
    let mut t = setup(&["frontend"], 0, |_| {});
    let fe = t.players[0];
    let far = match t.sim.spawn_unit("ceo", Vec3::new(0.0, 0.0, -20.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    t.target(fe, far);
    t.cast(fe, HOT_RELOAD);
    t.run(2);
    let landed = t.log.iter().find_map(|e| match e {
        Event::Teleported { unit, pos } if *unit == fe => Some(*pos),
        _ => None,
    });
    let Some(pos) = landed else { panic!("no dash") };
    assert!((pos.z + 18.0).abs() < 0.01, "{pos:?}");
    assert_eq!(t.sim.unit(fe).map(|u| u.pos), Some(pos));
    assert!(t.log.iter().any(
        |e| matches!(e, Event::Damage { source, spell: Some(s), .. } if *source == fe && *s == HOT_RELOAD)
    ));
}

#[test]
fn horizontal_scaling_raises_max_hp_and_rate_limiter_soaks() {
    let mut t = setup(&["backend"], 100, |_| {});
    let tank = t.players[0];
    let (hp0, max0) = t.hp(tank);
    assert_eq!(hp0, max0);
    t.cast(tank, HORIZONTAL);
    t.run(2);
    let (hp1, max1) = t.hp(tank);
    assert_eq!(max1, max0 * 130 / 100);
    assert_eq!(hp1, max1, "the new HP is granted, not just the cap");
    t.run(15 * 30);
    let (hp2, max2) = t.hp(tank);
    assert_eq!(max2, max0);
    assert!(hp2 <= max2);

    // Rate Limiter: a 100 hit (80 after the tank's armour) is absorbed, then -20% more.
    let mut t = setup(&["backend"], 100, |_| {});
    let tank = t.players[0];
    t.cast(tank, RATE_LIMITER);
    t.target(tank, t.mob);
    t.sim
        .apply_as(tank, Command::StartAutoAttack { unit: tank });
    t.run(240);
    let hits: Vec<(u32, u32)> = t
        .log
        .iter()
        .filter_map(|e| match e {
            Event::Damage {
                target,
                amount,
                absorbed,
                ..
            } if *target == tank => Some((*amount, *absorbed)),
            _ => None,
        })
        .collect();
    assert_eq!(hits.first(), Some(&(0, 60)), "{hits:?}");
    assert_eq!(hits.get(1), Some(&(0, 60)), "{hits:?}");
}

#[test]
fn revert_commit_restores_the_hp_of_five_seconds_ago() {
    let mut t = setup(&["devops"], 40, |_| {});
    let dev = t.players[0];
    t.target(dev, t.mob);
    t.sim.apply_as(dev, Command::StartAutoAttack { unit: dev });
    t.run(30);
    let (max, _) = (t.hp(dev).1, ());
    // Let the CEO chew for a while, sampling the history second by second.
    let mut samples = Vec::new();
    for _ in 0..8 {
        t.run(30);
        samples.push(t.hp(dev).0);
    }
    let now = t.hp(dev).0;
    assert!(now < max);
    t.cast(dev, REVERT);
    t.run(1);
    let healed = t.log.iter().rev().find_map(|e| match e {
        Event::Heal {
            spell: Some(s),
            amount,
            ..
        } if *s == REVERT => Some(*amount),
        _ => None,
    });
    // The history entry from 5 s ago is the sample taken five seconds before `now`.
    let then = samples[samples.len() - 6];
    assert_eq!(healed, Some(then.saturating_sub(now)));
}

#[test]
fn devops_heals_allies_and_the_group_and_restarts_dead_pods() {
    let mut t = setup(&["devops", "backend", "frontend"], 0, |_| {});
    let (dev, tank, fe) = (t.players[0], t.players[1], t.players[2]);
    assert!(t.sim.debug_apply_aura(tank, "deadline", 1));
    t.run(3);
    let hurt = t.hp(tank);
    assert!(hurt.0 < hurt.1);
    // Hotfix on the targeted teammate.
    t.target(dev, tank);
    t.cast(dev, HOTFIX);
    t.run(50);
    assert!(t.log.iter().any(|e| matches!(
        e,
        Event::Heal { source, target, spell: Some(s), amount } if *source == dev && *target == tank && *s == HOTFIX && *amount > 0
    )));
    // Blue-Green Deploy heals everyone around, the healer included.
    for p in [dev, fe] {
        assert!(t.sim.debug_apply_aura(p, "deadline", 1));
    }
    t.run(80);
    t.cast(dev, BLUE_GREEN);
    t.run(80);
    let healed: Vec<EntityId> = t
        .log
        .iter()
        .filter_map(|e| match e {
            Event::Heal {
                target,
                spell: Some(s),
                amount,
                ..
            } if *s == BLUE_GREEN && *amount > 0 => Some(*target),
            _ => None,
        })
        .collect();
    for p in [dev, fe] {
        assert!(healed.contains(&p), "{healed:?}");
    }
    // Restart Pod: 3 s cast, 50% HP, out of combat only (the CEO goes first).
    assert!(t.sim.debug_kill_player(fe));
    assert!(t.sim.debug_kill(t.mob));
    t.run(2);
    t.target(dev, fe);
    t.cast(dev, RESTART_POD);
    t.run(3 * 30 + 2);
    let fe_state = t.sim.unit(fe).map(|u| (u.alive, u.hp, u.max_hp));
    let Some((alive, hp, max)) = fe_state else {
        panic!("no frontend")
    };
    assert!(alive);
    assert_eq!(hp, max / 2);
}
