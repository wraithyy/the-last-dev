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
    assert_eq!(
        keys,
        [
            "fullstack",
            "frontend",
            "backend",
            "devops",
            "qa",
            "data_scientist",
            "security"
        ]
    );
    let mut sim = Sim::new(content(), 1);
    assert_eq!(sim.spawn_player("rockstar", Vec3::default(), 0.0), 0);
    // A room holds five players, so the sixth and seventh class get a sim of their own.
    let mut spare = Sim::new(content(), 1);
    let ids: Vec<EntityId> = keys
        .iter()
        .enumerate()
        .map(|(k, key)| {
            let s = if k < 5 { &mut sim } else { &mut spare };
            s.spawn_player(key, Vec3::default(), 0.0)
        })
        .collect();
    let unit = |k: usize| {
        if k < 5 {
            sim.unit(ids[k])
        } else {
            spare.unit(ids[k])
        }
    };
    let stats: Vec<(String, u32, u32)> = (0..ids.len())
        .filter_map(unit)
        .map(|u| (u.key.clone(), u.max_hp, u.max_mana))
        .collect();
    assert_eq!(
        stats,
        [
            ("senior_engineer".to_string(), 300, 250),
            ("frontend_dev".to_string(), 360, 220),
            ("backend_dev".to_string(), 440, 200),
            ("devops_eng".to_string(), 290, 300),
            ("qa_engineer".to_string(), 320, 260),
            ("data_scientist".to_string(), 320, 280),
            ("security_engineer".to_string(), 330, 240),
        ]
    );
    for (k, (id, key)) in ids.iter().zip(&keys).enumerate() {
        let s = if k < 5 { &sim } else { &spare };
        let class = s.class_of(*id).map(|c| c.key.clone());
        assert_eq!(class.as_deref(), Some(*key));
        // Two spells at level 1 (the Security Engineer starts with five: its combo needs a
        // builder, a finisher and a stealth), each class its own bar of ten.
        let at_start = if *key == "security" { 5 } else { 2 };
        assert_eq!(s.known_spells(*id).len(), at_start, "{key}");
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
    assert_eq!(range("qa_engineer"), Some(25.0));
    assert_eq!(range("data_scientist"), Some(25.0));
    assert_eq!(range("security_engineer"), Some(4.0));
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

// ---- cleansing ----

fn auras_of(sim: &Sim, p: EntityId) -> Vec<String> {
    sim.unit(p)
        .map(|u| u.auras.iter().map(|a| a.key.clone()).collect())
        .unwrap_or_default()
}

fn cleanse_sim(class: &str) -> (Sim, EntityId) {
    let mut c = content();
    for s in &mut c.spells {
        s.learn_level = 1;
        s.yells.clear();
    }
    let mut sim = Sim::new(c, 3);
    let p = sim.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert_ne!(p, 0, "unknown class {class}");
    (sim, p)
}

/// Every class has a self-cleanse: it removes a dispellable debuff, never an undispellable
/// boss mechanic (Budget Frozen is `dispel: Never`), and a second press waits for the cooldown.
#[test]
fn every_class_can_cleanse_itself_but_not_boss_mechanics() {
    let cleanses = [
        ("fullstack", 7),       // Rollback
        ("frontend", 25),       // Dark Mode
        ("backend", 37),        // Retry With Exponential Backoff
        ("devops", 46),         // Feature Flag Off
        ("qa", 56),             // Revert to Last Green
        ("data_scientist", 65), // Confidence Interval
        ("security", 77),       // Incident Response
    ];
    let classes: Vec<String> = content().classes.iter().map(|c| c.key.clone()).collect();
    assert_eq!(classes.len(), cleanses.len(), "a new class needs a cleanse");
    for (class, id) in cleanses {
        let spell = SpellId(id);
        let (mut sim, p) = cleanse_sim(class);
        assert!(sim.debug_apply_aura(p, "budget_frozen", 60_000));
        assert!(sim.debug_apply_aura(p, "code_smell", 60_000));
        sim.apply_as(p, Command::Cast { caster: p, spell });
        for _ in 0..2 {
            sim.tick();
        }
        let left = auras_of(&sim, p);
        assert!(!left.contains(&"code_smell".into()), "{class}: {left:?}");
        assert!(left.contains(&"budget_frozen".into()), "{class}: {left:?}");
        assert!(
            sim.cooldown(p, spell).cd_remaining_ms > 0,
            "{class}: no cooldown"
        );
        // Off the GCD, still on its own cooldown: the new debuff stays.
        for _ in 0..60 {
            sim.tick();
        }
        assert!(sim.debug_apply_aura(p, "code_smell", 60_000));
        assert_eq!(
            sim.cooldown(p, spell).usable,
            Err(CastError::OnCooldown),
            "{class}"
        );
        sim.apply_as(p, Command::Cast { caster: p, spell });
        sim.tick();
        assert!(auras_of(&sim, p).contains(&"code_smell".into()), "{class}");
    }
}

/// The vendor cleanse works for any class, keeps boss mechanics and shares the consumable
/// cooldown.
#[test]
fn the_postmortem_consumable_cleanses_any_class_on_the_shared_cooldown() {
    const ITEM: &str = "Incident Postmortem (Blameless)";
    for class in ["backend", "qa"] {
        let (mut sim, p) = cleanse_sim(class);
        assert!(sim.grant_goods(p, ITEM) && sim.grant_goods(p, ITEM));
        assert!(sim.debug_apply_aura(p, "budget_frozen", 60_000));
        assert!(sim.debug_apply_aura(p, "code_smell", 60_000));
        assert!(sim.debug_apply_aura(p, "review_fatigue", 60_000));
        sim.apply_as(p, Command::UseItem { item: ITEM.into() });
        sim.tick();
        assert_eq!(auras_of(&sim, p), ["budget_frozen"], "{class}");
        assert!(sim.debug_apply_aura(p, "code_smell", 60_000));
        sim.apply_as(p, Command::UseItem { item: ITEM.into() });
        let ev = sim.tick();
        assert!(ev.contains(&Event::ItemError {
            reason: combat_core::ItemError::NotReady
        }));
        assert!(auras_of(&sim, p).contains(&"code_smell".into()));
    }
}

// ---- QA Engineer and Data Scientist ----

/// A level-10 `class` at the origin facing -Z and plain melee `units` (abilities stripped) in
/// a row 20 m ahead, 2 m apart. Hits always land; every spell is learned; no regen, no yells.
fn arena(class: &str, units: &[&str]) -> (Sim, EntityId, Vec<EntityId>) {
    arena_with(class, units, |_| {})
}

fn arena_with(
    class: &str,
    units: &[&str],
    tweak: impl FnOnce(&mut Content),
) -> (Sim, EntityId, Vec<EntityId>) {
    let mut c = content();
    tweak(&mut c);
    c.config.white_hit_pct = 100;
    c.config.spell_hit_pct = 100;
    c.config.regen.hp_pct = 0;
    c.config.regen.mana_pct = 0;
    legacy_ceo(&mut c);
    for s in &mut c.spells {
        s.learn_level = 1;
        s.yells.clear();
    }
    for key in units {
        if let Some(ai) = def_mut(&mut c, key).ai.as_mut() {
            ai.abilities.clear();
        }
    }
    let mut sim = Sim::new(c, 9);
    sim.set_wandering(false);
    let p = sim.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(sim.set_level_of(p, 10));
    let mobs = units
        .iter()
        .enumerate()
        .map(
            |(k, key)| match sim.spawn_unit(key, Vec3::new(k as f32 * 2.0, 0.0, -20.0), 0.0) {
                Ok(id) => id,
                Err(e) => panic!("{e}"),
            },
        )
        .collect();
    (sim, p, mobs)
}

fn cast_at(sim: &mut Sim, p: EntityId, t: EntityId, spell: SpellId) -> Vec<Event> {
    sim.apply_as(
        p,
        Command::SetTarget {
            unit: p,
            target: Some(t),
        },
    );
    sim.apply_as(p, Command::Cast { caster: p, spell });
    sim.tick()
}

fn stacks(sim: &Sim, id: EntityId, key: &str) -> u8 {
    sim.unit(id)
        .and_then(|u| u.auras.iter().find(|a| a.key == key))
        .map_or(0, |a| a.stacks)
}

fn pos_z(sim: &Sim, id: EntityId) -> f32 {
    sim.unit(id).map_or(0.0, |u| u.pos.z)
}

#[test]
fn regression_trap_roots_trash_and_only_slows_a_boss() {
    let (mut sim, p, mobs) = arena("qa", &["sales_rep"]);
    cast_at(&mut sim, p, mobs[0], SpellId(54));
    let at = pos_z(&sim, mobs[0]);
    for _ in 0..60 {
        sim.tick();
    }
    assert_eq!(pos_z(&sim, mobs[0]), at, "a rooted Sales Rep must not walk");
    assert!(stacks(&sim, mobs[0], "regression_trap") > 0);
    // The CEO is a boss: no root, but 40% slower than an untrapped CEO.
    let walked = |trap: bool| {
        let (mut sim, p, mobs) = arena("qa", &["ceo"]);
        let spell = if trap { SpellId(54) } else { SpellId(52) };
        cast_at(&mut sim, p, mobs[0], spell);
        let at = pos_z(&sim, mobs[0]);
        for _ in 0..30 {
            sim.tick();
        }
        pos_z(&sim, mobs[0]) - at
    };
    let (free, trapped) = (walked(false), walked(true));
    assert!(trapped > 0.0, "a boss is never rooted");
    assert!(
        (trapped / free - 0.6).abs() < 0.1,
        "boss slowed to {trapped} of {free}"
    );
}

#[test]
fn bug_reports_stack_to_three() {
    let (mut sim, p, mobs) = arena("qa", &["ceo"]);
    for _ in 0..4 {
        cast_at(&mut sim, p, mobs[0], SpellId(52));
        for _ in 0..46 {
            sim.tick();
        }
    }
    assert_eq!(stacks(&sim, mobs[0], "bug_report"), 3);
}

#[test]
fn cannot_reproduce_closes_every_hit_for_its_duration() {
    let (mut sim, p, mobs) = arena("qa", &["sales_rep"]);
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SpellId(57),
        },
    );
    sim.tick();
    let hp = sim.unit(p).map_or(0, |u| u.hp);
    // Walk into the Sales Rep and let it swing for the 2.5 s window.
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos: Vec3::new(0.0, 0.0, -18.0),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    let mut swung = false;
    for _ in 0..70 {
        swung |= sim
            .tick()
            .iter()
            .any(|e| matches!(e, Event::MeleeSwing { attacker, .. } if *attacker == mobs[0]));
    }
    assert!(swung, "the Sales Rep never swung");
    assert_eq!(sim.unit(p).map_or(0, |u| u.hp), hp);
}

#[test]
fn training_run_hits_every_second_stacks_overfitting_and_shrugs_off_hits() {
    let (mut sim, p, mobs) = arena("data_scientist", &["ceo"]);
    let mut ev = cast_at(&mut sim, p, mobs[0], SpellId(66));
    for _ in 0..95 {
        ev.extend(sim.tick());
    }
    let hits = ev
        .iter()
        .filter(
            |e| matches!(e, Event::Damage { source, spell: Some(SpellId(66)), .. } if *source == p),
        )
        .count();
    assert_eq!(hits, 3, "two ticks and the completion");
    assert_eq!(stacks(&sim, mobs[0], "overfitting"), 3);
    assert!(
        !ev.iter()
            .any(|e| matches!(e, Event::CastInterrupted { caster, .. } if *caster == p))
    );
}

#[test]
fn cross_validation_copies_dots_with_their_stacks_to_fighting_neighbours() {
    let (mut sim, p, mobs) = arena("data_scientist", &["sales_rep", "sales_rep", "sales_rep"]);
    // Pull all three, then stack Overfitting twice and Data Drift once on the first.
    for &m in &mobs {
        cast_at(&mut sim, p, m, SpellId(61));
        for _ in 0..80 {
            sim.tick();
        }
    }
    for spell in [62, 62] {
        cast_at(&mut sim, p, mobs[0], SpellId(spell));
        for _ in 0..60 {
            sim.tick();
        }
    }
    cast_at(&mut sim, p, mobs[0], SpellId(64));
    for _ in 0..60 {
        sim.tick();
    }
    assert_eq!(stacks(&sim, mobs[0], "overfitting"), 2);
    assert_eq!(stacks(&sim, mobs[0], "data_drift"), 1);
    cast_at(&mut sim, p, mobs[0], SpellId(67));
    for &m in &mobs[1..] {
        assert_eq!(stacks(&sim, m, "overfitting"), 2, "{m}");
        assert_eq!(stacks(&sim, m, "data_drift"), 1, "{m}");
    }
}

/// The new classes save and load like the old ones: class, level and Career Ladder picks
/// survive the text round trip into a fresh sim of the same class.
#[test]
fn new_class_saves_round_trip_with_their_talents() {
    for class in ["qa", "data_scientist"] {
        let mut sim = Sim::new(content(), 4);
        let p = sim.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
        assert!(sim.set_level_of(p, 10));
        for row in 0..combat_core::TALENT_ROWS as u8 {
            sim.apply_as(p, Command::PickTalent { row, choice: 2 });
        }
        sim.tick();
        let Some(save) = sim.export_save(p) else {
            panic!("{class}: no save")
        };
        let Some(back) = combat_core::SaveData::decode(&save.encode()) else {
            panic!("{class}: save does not decode")
        };
        assert_eq!(back.class, class);
        let mut fresh = Sim::new(content(), 5);
        let q = fresh.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
        assert!(fresh.import_save(q, &back), "{class}");
        let me = fresh.unit(q).cloned();
        assert_eq!(me.as_ref().map(|u| u.level), Some(10), "{class}");
        assert_eq!(
            me.map(|u| u.talents),
            Some([2; combat_core::TALENT_ROWS]),
            "{class}"
        );
    }
}

// ---- Security Engineer ----

const SHOULDER_SURF: SpellId = SpellId(71);
const ZERO_DAY: SpellId = SpellId(73);
const PRIV_ESC: SpellId = SpellId(74);
const TAILGATING: SpellId = SpellId(75);
const SOCIAL_ENG: SpellId = SpellId(78);

fn no_crit(c: &mut Content) {
    def_mut(c, "security_engineer").crit_pct = 0;
}

fn move_to(sim: &mut Sim, p: EntityId, z: f32, yaw: f32) {
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos: Vec3::new(0.0, 0.0, z),
            yaw,
            moving: false,
            jumping: false,
        },
    );
    sim.tick();
}

fn has_aura(sim: &Sim, id: EntityId, key: &str) -> bool {
    stacks(sim, id, key) > 0
}

fn damage_of(ev: &[Event], p: EntityId, spell: SpellId) -> u32 {
    ev.iter()
        .find_map(|e| match e {
            Event::Damage {
                source,
                spell: Some(s),
                amount,
                ..
            } if *source == p && *s == spell => Some(*amount),
            _ => None,
        })
        .unwrap_or(0)
}

/// An idle Sales Rep (aggro range 7 m) 20 m ahead notices a player at 6 m, but not one in
/// Tailgating, until the player attacks.
#[test]
fn tailgating_hides_from_idle_enemies_until_you_attack() {
    let notices = |stealth: bool| {
        let (mut sim, p, mobs) = arena_with("security", &["sales_rep"], no_crit);
        if stealth {
            sim.apply_as(
                p,
                Command::Cast {
                    caster: p,
                    spell: TAILGATING,
                },
            );
            sim.tick();
        }
        move_to(&mut sim, p, -14.0, 0.0);
        for _ in 0..30 {
            sim.tick();
        }
        sim.unit(mobs[0]).and_then(|u| u.target).is_some()
            || sim
                .unit(mobs[0])
                .is_some_and(|u| u.ai != Some(combat_core::AiState::Idle))
    };
    assert!(notices(false), "a visible player at 6 m must be noticed");
    assert!(!notices(true), "a stealthed player at 6 m must not be");

    let (mut sim, p, mobs) = arena_with("security", &["sales_rep"], no_crit);
    cast_at(&mut sim, p, mobs[0], TAILGATING);
    move_to(&mut sim, p, -16.0, 0.0);
    assert!(has_aura(&sim, p, "tailgating"));
    // Four metres away it still does not notice; the first hit gives the player away.
    for _ in 0..30 {
        sim.tick();
    }
    assert!(has_aura(&sim, p, "tailgating"));
    cast_at(&mut sim, p, mobs[0], SHOULDER_SURF);
    assert!(!has_aura(&sim, p, "tailgating"), "attacking ends stealth");
}

#[test]
fn stealth_is_slower_to_walk_and_taking_damage_ends_it() {
    let c = content();
    assert_eq!(c.speed_pct(["tailgating"]), 70);
    assert_eq!(c.speed_pct(std::iter::empty()), 100);
    let (mut sim, p, _mobs) = arena_with("security", &["sales_rep"], no_crit);
    assert!(sim.debug_apply_aura(p, "tailgating", 60_000));
    assert_eq!(sim.move_speed_pct(p), 70);
    // Closer than the stealth floor (1.5 m) the Sales Rep notices and swings: the hit ends it.
    move_to(&mut sim, p, -19.0, 0.0);
    let mut hurt = false;
    for _ in 0..120 {
        hurt |= sim
            .tick()
            .iter()
            .any(|e| matches!(e, Event::Damage { target, .. } if *target == p));
    }
    assert!(hurt, "the Sales Rep never hit the player");
    assert!(!has_aura(&sim, p, "tailgating"), "damage ends stealth");
}

/// Casting Tailgating mid-fight drops trash from the threat table; a boss stays on you.
#[test]
fn tailgating_in_a_fight_is_a_vanish_for_trash_but_not_for_bosses() {
    let (mut sim, p, mobs) = arena_with("security", &["sales_rep", "ceo"], no_crit);
    move_to(&mut sim, p, -16.0, 0.0);
    cast_at(&mut sim, p, mobs[0], SHOULDER_SURF);
    cast_at(&mut sim, p, mobs[1], SHOULDER_SURF);
    for _ in 0..20 {
        sim.tick();
    }
    assert!(sim.unit(mobs[0]).is_some_and(|u| u.in_combat));
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: TAILGATING,
        },
    );
    for _ in 0..10 {
        sim.tick();
    }
    let rep = sim.unit(mobs[0]);
    assert!(
        rep.is_some_and(|u| u.target.is_none() || u.ai == Some(combat_core::AiState::Evade)),
        "trash should lose the player: {rep:?}"
    );
    assert_eq!(sim.unit(mobs[1]).and_then(|u| u.target), Some(p));
}

#[test]
fn zero_day_hits_harder_from_stealth_and_from_behind() {
    // The mob faces -Z: standing at +Z of it is behind it, at -Z in front.
    let hit = |stealth: bool, behind: bool| {
        let (mut sim, p, mobs) = arena_with("security", &["sales_rep"], no_crit);
        if stealth {
            sim.apply_as(
                p,
                Command::Cast {
                    caster: p,
                    spell: TAILGATING,
                },
            );
            sim.tick();
        }
        if behind {
            move_to(&mut sim, p, -16.0, 0.0);
        } else {
            move_to(&mut sim, p, -24.0, std::f32::consts::PI);
        }
        let ev = cast_at(&mut sim, p, mobs[0], ZERO_DAY);
        damage_of(&ev, p, ZERO_DAY)
    };
    let (front, behind, stealth_front, both) = (
        hit(false, false),
        hit(false, true),
        hit(true, false),
        hit(true, true),
    );
    assert!(front > 0);
    let near =
        |got: u32, pct: u32| (i64::from(got) - i64::from(front) * i64::from(pct) / 100).abs() <= 2;
    assert!(near(behind, 150), "behind {behind} vs front {front}");
    assert!(
        near(stealth_front, 250),
        "stealth {stealth_front} vs front {front}"
    );
    assert!(near(both, 300), "both {both} vs front {front}");
}

/// Zero-Day builds two points, Shoulder Surf one; five is the cap and Privilege Escalation
/// spends them all for 35% more damage each.
#[test]
fn exploit_chain_builds_to_five_and_privilege_escalation_spends_it() {
    let (mut sim, p, mobs) = arena_with("security", &["ceo"], no_crit);
    move_to(&mut sim, p, -16.0, 0.0);
    let plain = damage_of(&cast_at(&mut sim, p, mobs[0], PRIV_ESC), p, PRIV_ESC);
    assert_eq!(stacks(&sim, mobs[0], "exploit_chain"), 0);
    for _ in 0..50 {
        sim.tick();
    }
    cast_at(&mut sim, p, mobs[0], ZERO_DAY);
    assert_eq!(stacks(&sim, mobs[0], "exploit_chain"), 2);
    for _ in 0..4 {
        for _ in 0..50 {
            sim.tick();
        }
        cast_at(&mut sim, p, mobs[0], SHOULDER_SURF);
    }
    assert_eq!(stacks(&sim, mobs[0], "exploit_chain"), 5, "capped at five");
    for _ in 0..50 {
        sim.tick();
    }
    let full = damage_of(&cast_at(&mut sim, p, mobs[0], PRIV_ESC), p, PRIV_ESC);
    assert_eq!(stacks(&sim, mobs[0], "exploit_chain"), 0);
    let want = plain * 275 / 100;
    assert!(
        (i64::from(full) - i64::from(want)).abs() <= 3,
        "five points {full}, expected about {want} ({plain} bare)"
    );
}

#[test]
fn social_engineering_stuns_trash_until_hit_and_only_interrupts_a_boss() {
    let (mut sim, p, mobs) = arena_with("security", &["sales_rep", "ceo"], no_crit);
    move_to(&mut sim, p, -16.0, 0.0);
    cast_at(&mut sim, p, mobs[0], SOCIAL_ENG);
    assert!(sim.unit(mobs[0]).is_some_and(|u| u.stunned));
    for _ in 0..50 {
        sim.tick();
    }
    assert!(sim.unit(mobs[0]).is_some_and(|u| u.stunned), "still lost");
    cast_at(&mut sim, p, mobs[0], SHOULDER_SURF);
    assert!(
        sim.unit(mobs[0]).is_some_and(|u| !u.stunned),
        "a hit wakes it up"
    );
    for _ in 0..80 {
        sim.tick();
    }
    cast_at(&mut sim, p, mobs[1], SOCIAL_ENG);
    assert!(
        sim.unit(mobs[1]).is_some_and(|u| !u.stunned),
        "bosses are immune to the stun"
    );
}
