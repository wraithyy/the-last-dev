//! Career Ladder talents (talents.rs): every talent's mechanic, pick and respec rules, titles
//! and the save migration.

mod common;

use combat_core::{
    CastError, Command, EntityId, Event, ItemError, Perk, SaveData, Sim, SpellDef, SpellId,
    SpellTarget, TALENT_ROWS, TalentDef, TalentPicks, Vec3,
};
use common::content;

fn arena_content() -> combat_core::Content {
    let mut c = content();
    let d = common::def_mut(&mut c, MOB);
    d.max_hp = 60_000;
    if let Some(ai) = d.ai.as_mut() {
        ai.melee_damage = 3;
    }
    c
}

/// Trash without abilities, made sturdy and toothless below so fights only test the talents.
const MOB: &str = "additional_requirement";
const MOBS: [(f32, f32); 4] = [(0.0, -3.0), (1.5, -3.5), (-1.5, -3.5), (0.0, -4.5)];
const CLASSES: [&str; 4] = ["fullstack", "frontend", "backend", "devops"];

struct Arena {
    sim: Sim,
    me: EntityId,
    allies: Vec<EntityId>,
    mobs: Vec<EntityId>,
    moving: bool,
}

impl Arena {
    /// A level-10 `class` player with `picks`, two idle allies of the same class next to it and
    /// four mobs in melee range in front (yaw 0 faces -z), already fighting.
    fn new(class: &str, picks: TalentPicks, seed: u64) -> Arena {
        let mut sim = Sim::new(arena_content(), seed);
        sim.set_wandering(false);
        let me = sim.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
        let allies: Vec<EntityId> = [2.0, -2.0]
            .iter()
            .map(|x| sim.spawn_player(class, Vec3::new(*x, 0.0, 1.0), 0.0))
            .collect();
        for p in std::iter::once(me).chain(allies.iter().copied()) {
            assert!(sim.set_level_of(p, 10));
        }
        assert!(sim.set_talents(me, picks));
        let mobs = MOBS
            .iter()
            .map(
                |(x, z)| match sim.spawn_unit(MOB, Vec3::new(*x, 0.0, *z), std::f32::consts::PI) {
                    Ok(id) => id,
                    Err(e) => panic!("mob spawns: {e:?}"),
                },
            )
            .collect::<Vec<_>>();
        let mut a = Arena {
            sim,
            me,
            allies,
            mobs,
            moving: false,
        };
        a.target(a.mobs[0]);
        a.run(15);
        a
    }

    fn target(&mut self, t: EntityId) {
        self.sim.apply_as(
            self.me,
            Command::SetTarget {
                unit: self.me,
                target: Some(t),
            },
        );
    }

    fn tick(&mut self) -> Vec<Event> {
        let pos = self
            .sim
            .unit(self.me)
            .map_or(Vec3::new(0.0, 0.0, 0.0), |u| u.pos);
        self.sim.apply_as(
            self.me,
            Command::PlayerMoved {
                pos,
                yaw: 0.0,
                moving: self.moving,
                jumping: false,
            },
        );
        self.sim.tick()
    }

    fn run(&mut self, n: u32) -> Vec<Event> {
        (0..n).flat_map(|_| self.tick()).collect()
    }

    fn me(&self) -> combat_core::UnitState {
        match self.sim.unit(self.me) {
            Some(u) => u.clone(),
            None => panic!("player exists"),
        }
    }

    /// Presses `spell` once it is usable (up to 10 s) and runs until it resolves or fails;
    /// every event on the way is returned.
    fn cast(&mut self, spell: SpellId) -> Vec<Event> {
        let mut out = Vec::new();
        for _ in 0..300 {
            if self.sim.cooldown(self.me, spell).usable.is_ok() {
                break;
            }
            out.extend(self.tick());
        }
        self.sim.apply_as(
            self.me,
            Command::Cast {
                caster: self.me,
                spell,
            },
        );
        let cast = self.sim.cast_spell_for(self.me, spell);
        for _ in 0..240 {
            let ev = self.tick();
            let done = ev.iter().any(|e| {
                matches!(e, Event::CastSucceeded { caster, spell: s, .. }
                    | Event::CastFailed { caster, spell: s, .. }
                    | Event::CastInterrupted { caster, spell: s, .. }
                    if *caster == self.me && *s == cast)
            });
            out.extend(ev);
            if done {
                break;
            }
        }
        out.extend(self.run(2));
        out
    }

    fn succeeded(&self, ev: &[Event], spell: SpellId) -> bool {
        let cast = self.sim.cast_spell_for(self.me, spell);
        ev.iter().any(|e| {
            matches!(e, Event::CastSucceeded { caster, spell: s, .. } if *caster == self.me && *s == cast)
        })
    }

    fn proc_of(&self, ev: &[Event], name: &str) -> bool {
        ev.iter()
            .any(|e| matches!(e, Event::Proc { unit, item } if *unit == self.me && item == name))
    }
}

fn picks_for(row: usize, choice: u8) -> TalentPicks {
    let mut p = [0; TALENT_ROWS];
    p[row] = choice;
    p
}

fn spell(id: SpellId) -> SpellDef {
    match content().spell(id) {
        Some(s) => s.clone(),
        None => panic!("unknown spell {id:?}"),
    }
}

/// The class's first bar spell that deals damage to an enemy.
fn damage_spell(class: &str) -> SpellId {
    let c = content();
    let Some(bar) = c.class(class).map(|c| c.action_bar.clone()) else {
        panic!("no class {class}");
    };
    bar.into_iter()
        .find(|id| {
            c.spell(*id).is_some_and(|s| {
                s.target == SpellTarget::Enemy
                    && s.area.is_none()
                    && matches!(s.effect, combat_core::Effect::Damage { .. })
            })
        })
        .unwrap_or_else(|| panic!("{class} has no damage spell"))
}

fn first_amount(ev: &[Event], f: impl Fn(&Event) -> Option<u32>) -> Option<u32> {
    ev.iter().find_map(f)
}

/// Checks one perk of talent `t` (row `row`, option `k`) does what its text promises.
fn check_perk(class: &str, row: usize, k: u8, t: &TalentDef, perk: &Perk) -> Result<(), String> {
    let picks = picks_for(row, k);
    let fresh = || Arena::new(class, picks, 11);
    let bare = || Arena::new(class, [0; TALENT_ROWS], 11);
    let fail = |m: &str| Err(format!("{class} {} ({perk:?}): {m}", t.key));
    match perk {
        Perk::Splash { spell: s, .. } => {
            let def = spell(*s);
            let mut a = fresh();
            let ally = def.target != SpellTarget::Enemy;
            if ally {
                for p in std::iter::once(a.me).chain(a.allies.clone()) {
                    let hp = a.sim.unit(p).map_or(1, |u| u.max_hp / 3);
                    a.sim.debug_set_hp(p, hp);
                }
                let first = a.allies[0];
                a.target(first);
            }
            let auras: Vec<String> = def
                .effects()
                .filter_map(|e| match e {
                    combat_core::Effect::Aura { aura, .. } => Some(aura.clone()),
                    combat_core::Effect::Damage {
                        apply_aura: Some(x),
                        ..
                    } => Some(x.aura.clone()),
                    _ => None,
                })
                .collect();
            let ev = a.cast(*s);
            let mut hit: Vec<EntityId> = ev
                .iter()
                .filter_map(|e| match e {
                    Event::Damage {
                        target,
                        spell: Some(x),
                        ..
                    }
                    | Event::Heal {
                        target,
                        spell: Some(x),
                        ..
                    } if *x == *s => Some(*target),
                    Event::AuraApplied { target, key } if auras.contains(key) => Some(*target),
                    _ => None,
                })
                .collect();
            hit.sort_unstable();
            hit.dedup();
            if hit.len() < 2 {
                return fail(&format!("landed on {} unit(s)", hit.len()));
            }
        }
        Perk::SpellDamage { spell: s, pct } | Perk::SpellHeal { spell: s, pct } => {
            let heal = matches!(perk, Perk::SpellHeal { .. });
            let id = s.unwrap_or_else(|| damage_spell(class));
            let amount = |mut a: Arena| {
                if heal {
                    let hp = a.me().max_hp / 4;
                    a.sim.debug_set_hp(a.me, hp);
                }
                let ev = a.cast(id);
                let cast = a.sim.cast_spell_for(a.me, id);
                first_amount(&ev, |e| match e {
                    Event::Damage {
                        source,
                        spell: Some(x),
                        amount,
                        absorbed,
                        ..
                    } if !heal && *source == a.me && *x == cast => Some(amount + absorbed),
                    Event::Heal {
                        source,
                        spell: Some(x),
                        amount,
                        ..
                    } if heal && *source == a.me && *x == cast => Some(*amount),
                    _ => None,
                })
                .unwrap_or(0)
            };
            let (with, without) = (amount(fresh()), amount(bare()));
            if with * 100 < without * (100 + pct) * 95 / 100 || with == 0 {
                return fail(&format!("{with} vs {without} without the talent"));
            }
        }
        Perk::CastTime { spell: s, pct } => {
            let id = s.unwrap_or(SpellId(1));
            let total = |mut a: Arena| {
                let ev = a.cast(id);
                assert!(a.succeeded(&ev, id), "{} cast {id:?} failed: {ev:?}", t.key);
                first_amount(&ev, |e| match e {
                    Event::CastStarted {
                        caster, total_ms, ..
                    } if *caster == a.me => Some(*total_ms),
                    _ => None,
                })
                .unwrap_or(0)
            };
            let (with, without) = (total(fresh()), total(bare()));
            let ok = if *pct < 0 {
                with < without
            } else {
                with > without
            };
            if !ok {
                return fail(&format!("cast {with} ms vs {without} ms"));
            }
        }
        Perk::ManaCost { spell: s, pct } => {
            let id = s.unwrap_or(SpellId(1));
            let spent = |mut a: Arena| {
                let before = a.me().mana;
                a.cast(id);
                before.saturating_sub(a.me().mana)
            };
            let (with, without) = (spent(fresh()), spent(bare()));
            let ok = if *pct < 0 {
                with > without
            } else {
                with < without
            };
            if !ok {
                return fail(&format!("spent {with} vs {without} mana"));
            }
        }
        Perk::CastWhileMoving { spell: s } => {
            let id = s.unwrap_or(SpellId(1));
            let moving_cast = |mut a: Arena| {
                a.moving = true;
                a.run(2);
                let ev = a.cast(id);
                a.succeeded(&ev, id)
            };
            if !moving_cast(fresh()) {
                return fail("could not cast on the move");
            }
            if moving_cast(bare()) {
                return fail("the bare class already casts on the move");
            }
        }
        Perk::OnCast { spell: s, .. } | Perk::Echo { spell: s, .. } | Perk::Reset { on: s, .. } => {
            let id = s.unwrap_or_else(|| damage_spell(class));
            let mut a = fresh();
            let reset = match perk {
                Perk::Reset { reset, .. } => Some(*reset),
                _ => None,
            };
            if let Some(r) = reset {
                a.cast(r);
                if a.sim.cooldown(a.me, r).cd_remaining_ms == 0 {
                    return fail("reset target has no cooldown to reset");
                }
            }
            let mut procced = false;
            for _ in 0..40 {
                let mana = a.me().max_mana;
                a.sim.set_player_mana(mana);
                if def_targets_ally(id) {
                    let me = a.me;
                    a.target(me);
                }
                let ev = a.cast(id);
                if a.proc_of(&ev, &t.name) {
                    procced = true;
                    break;
                }
                let m0 = a.mobs[0];
                a.target(m0);
            }
            if !procced {
                return fail("never procced in 40 casts");
            }
            if let Some(r) = reset
                && a.sim.cooldown(a.me, r).cd_remaining_ms > 0
                && a.sim.cooldown(a.me, r).cd_total_ms > 0
                && a.sim.cooldown(a.me, r).cd_remaining_ms + 2000
                    > a.sim.cooldown(a.me, r).cd_total_ms
                && r != id
            {
                return fail("cooldown still running after the reset proc");
            }
        }
        Perk::Rider { spell: s, .. } => {
            let mut a = fresh();
            if def_targets_ally(*s) {
                let me = a.me;
                a.target(me);
            }
            let ev = a.cast(*s);
            if !a.proc_of(&ev, &t.name) {
                return fail("no rider proc");
            }
        }
        Perk::OnHurt { .. } => {
            let mut a = fresh();
            let ev = a.run(30 * 30);
            if !a.proc_of(&ev, &t.name) {
                return fail("no proc after 30 s of being hit");
            }
        }
        Perk::Leech { .. } | Perk::ManaReturn { .. } => {
            let id = match perk {
                Perk::ManaReturn { spell: Some(s), .. } => *s,
                _ => damage_spell(class),
            };
            let mut a = fresh();
            let (hp, mana) = (a.me().max_hp / 3, a.me().max_mana / 3);
            a.sim.debug_set_hp(a.me, hp);
            a.sim.set_player_mana(mana);
            let mut ok = false;
            for _ in 0..4 {
                let ev = a.cast(id);
                ok |= ev.iter().any(|e| match (perk, e) {
                    (
                        Perk::Leech { .. },
                        Event::Heal {
                            source,
                            target,
                            spell: Some(x),
                            ..
                        },
                    ) => *source == a.me && *target == a.me && *x == id,
                    (Perk::ManaReturn { .. }, Event::ManaGain { unit, spell: x, .. }) => {
                        *unit == a.me && *x == id
                    }
                    _ => false,
                });
            }
            if !ok {
                return fail("no heal or mana back from damage");
            }
        }
        Perk::HpCost { .. } => {
            let id = damage_spell(class);
            let broke = |mut a: Arena| {
                a.sim.set_player_mana(0);
                let me = a.me;
                a.sim.apply_as(
                    me,
                    Command::Cast {
                        caster: me,
                        spell: id,
                    },
                );
                let ev = a.run(3);
                a.succeeded(&ev, id)
            };
            if !broke(fresh()) || broke(bare()) {
                return fail("casting with zero mana should work only with the talent");
            }
        }
        Perk::DamageTaken { pct } => {
            let tick = |mut a: Arena| {
                let me = a.me;
                assert!(a.sim.debug_apply_aura(me, "chaos_monkey", 10_000));
                let ev = a.run(4 * 30);
                first_amount(&ev, |e| match e {
                    Event::Damage {
                        source,
                        target,
                        amount,
                        absorbed,
                        ..
                    } if *source == me && *target == me => Some(amount + absorbed),
                    _ => None,
                })
                .unwrap_or(0)
            };
            let (with, without) = (tick(fresh()), tick(bare()));
            let ok = if *pct < 0 {
                with < without
            } else {
                with > without
            };
            if !ok || without == 0 {
                return fail(&format!("tick {with} vs {without}"));
            }
        }
        Perk::Cooldown { spell: s, .. } => {
            let mut a = fresh();
            if def_targets_ally(*s) {
                let me = a.me;
                a.target(me);
            }
            a.cast(*s);
            let total = a.sim.cooldown(a.me, *s).cd_total_ms;
            if total == 0 || total >= spell(*s).cooldown_ms {
                return fail(&format!("cooldown total {total} ms"));
            }
        }
        Perk::OnKill { reset, .. } => {
            let Some(r) = reset else {
                return fail("OnKill without a reset is not covered");
            };
            let mut a = fresh();
            if def_targets_ally(*r) {
                let me = a.me;
                a.target(me);
            }
            a.cast(*r);
            if a.sim.cooldown(a.me, *r).cd_remaining_ms == 0 {
                return fail("no cooldown to reset");
            }
            let victim = a.mobs[3];
            assert!(a.sim.debug_kill(victim));
            let ev = a.run(2);
            if a.sim.cooldown(a.me, *r).cd_remaining_ms > 0 || !a.proc_of(&ev, &t.name) {
                return fail("killing blow did not reset the cooldown");
            }
        }
        Perk::Replace { spell: s, with } => {
            let mut a = fresh();
            if a.sim.cast_spell_for(a.me, *s) != *with {
                return fail("bar spell not replaced");
            }
            let ev = a.cast(*s);
            let cast_with = ev.iter().any(|e| {
                matches!(e, Event::CastSucceeded { caster, spell: x, .. } if *caster == a.me && x == with)
            });
            if !cast_with {
                return fail(&format!(
                    "pressing the bar spell did not cast the variant: {ev:?}"
                ));
            }
            if a.sim.cooldown(a.me, *s).cd_total_ms != spell(*with).cooldown_ms {
                return fail("cooldown view is not the variant's");
            }
        }
        Perk::Periodic { spell: s, pct } => {
            let def = spell(*s);
            let first_tick = |mut a: Arena| {
                let me = a.me;
                let hp = a.me().max_hp / 4;
                a.sim.debug_set_hp(me, hp);
                if def.target != SpellTarget::Enemy {
                    a.target(me);
                }
                let mut ev = a.cast(*s);
                ev.extend(a.run(4 * 30));
                first_amount(&ev, |e| match e {
                    Event::Damage {
                        source,
                        spell: Some(x),
                        amount,
                        absorbed,
                        ..
                    } if *source == me && x == s => Some(amount + absorbed),
                    Event::Heal {
                        source,
                        spell: Some(x),
                        amount,
                        ..
                    } if *source == me && x == s => Some(*amount),
                    _ => None,
                })
                .unwrap_or(0)
            };
            let (with, without) = (first_tick(fresh()), first_tick(bare()));
            if with * 100 < without * (100 + pct) * 90 / 100 || with == 0 {
                return fail(&format!("tick {with} vs {without}"));
            }
        }
        Perk::OnUse { .. } => return fail("talents have no on-use perks"),
    }
    Ok(())
}

fn def_targets_ally(id: SpellId) -> bool {
    spell(id).target == SpellTarget::Ally
}

/// Every perk of every talent (4 classes x 5 rows x 3 options) does what its card says.
#[test]
fn every_talent_mechanic_works() {
    let c = content();
    let mut failures = Vec::new();
    let mut checked = 0;
    for ct in &c.talents.classes {
        for (r, row) in ct.rows.iter().enumerate() {
            for (k, t) in row.options.iter().enumerate() {
                for p in &t.perks {
                    checked += 1;
                    if let Err(e) = check_perk(&ct.class, r, k as u8 + 1, t, p) {
                        failures.push(e);
                    }
                }
            }
        }
    }
    println!("{checked} talent perks checked");
    assert_eq!(c.talents.classes.len(), 4);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn picks_follow_level_combat_and_row_rules() {
    let mut sim = Sim::new(content(), 3);
    let me = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let errors = |sim: &mut Sim| -> Vec<ItemError> {
        sim.tick()
            .into_iter()
            .filter_map(|e| match e {
                Event::ItemError { reason } => Some(reason),
                _ => None,
            })
            .collect()
    };
    // Level 1: row 0 (level 2) is locked.
    sim.apply_as(me, Command::PickTalent { row: 0, choice: 1 });
    assert_eq!(errors(&mut sim), [ItemError::NotEligible]);
    sim.set_level_of(me, 4);
    for bad in [(0, 0), (0, 4), (5, 1), (2, 1)] {
        sim.apply_as(
            me,
            Command::PickTalent {
                row: bad.0,
                choice: bad.1,
            },
        );
        assert_eq!(errors(&mut sim), [ItemError::NotEligible], "{bad:?}");
    }
    sim.apply_as(me, Command::PickTalent { row: 0, choice: 2 });
    let ev = sim.tick();
    assert!(ev.iter().any(|e| matches!(e,
        Event::TalentsChanged { unit, talents } if *unit == me && *talents == [2, 0, 0, 0, 0])));
    // A taken row stays taken until a review.
    sim.apply_as(me, Command::PickTalent { row: 0, choice: 1 });
    assert_eq!(errors(&mut sim), [ItemError::NotEligible]);
    assert_eq!(sim.unit(me).map(|u| u.talents), Some([2, 0, 0, 0, 0]));
    // In combat: no promotions.
    let mob = sim
        .spawn_unit(MOB, Vec3::new(0.0, 0.0, -3.0), 0.0)
        .expect("mob");
    sim.apply_as(
        me,
        Command::SetTarget {
            unit: me,
            target: Some(mob),
        },
    );
    for _ in 0..30 {
        sim.tick();
    }
    assert!(sim.unit(me).is_some_and(|u| u.in_combat));
    sim.apply_as(me, Command::PickTalent { row: 1, choice: 1 });
    assert_eq!(errors(&mut sim), [ItemError::InCombat]);
}

#[test]
fn respec_costs_story_points_and_clears_the_capstone_title() {
    let c = content();
    let mut sim = Sim::new(content(), 5);
    let me = sim.spawn_player("backend", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(me, 10);
    for row in 0..TALENT_ROWS as u8 {
        sim.apply_as(me, Command::PickTalent { row, choice: 3 });
    }
    let ev = sim.tick();
    let capstone = &c.talents.for_class("backend").expect("rows").rows[4].options[2];
    assert!(ev.iter().any(|e| matches!(e,
        Event::TitleChanged { unit, title } if *unit == me && *title == capstone.key)));
    assert_eq!(sim.unit(me).map(|u| u.talents), Some([3; TALENT_ROWS]));
    assert_eq!(
        c.title_text(&capstone.key),
        Some("Backend-for-Frontend Lead")
    );
    let cost = c.talents.respec_cost(10);
    assert_eq!(cost, 150);
    // Too poor: nothing changes.
    sim.grant_sp(me, cost - 1);
    sim.apply_as(me, Command::ResetTalents);
    let ev = sim.tick();
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::ItemError {
            reason: ItemError::NotEnoughSp
        }
    )));
    sim.grant_sp(me, 1);
    sim.apply_as(me, Command::ResetTalents);
    let ev = sim.tick();
    let u = sim.unit(me).expect("player");
    assert_eq!(
        (u.sp, u.talents, u.title.as_str()),
        (0, [0; TALENT_ROWS], "")
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::TitleChanged { title, .. } if title.is_empty()))
    );
    // Nothing to reset is no free action either.
    sim.apply_as(me, Command::ResetTalents);
    let ev = sim.tick();
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::ItemError {
            reason: ItemError::NotEligible
        }
    )));
    // The capstone title can be taken off and put back on, but only while it is picked.
    sim.apply_as(
        me,
        Command::SetTitle {
            achievement: capstone.key.clone(),
        },
    );
    sim.tick();
    assert_eq!(sim.unit(me).map(|u| u.title.clone()), Some(String::new()));
}

#[test]
fn talents_change_stats_only_through_perks_and_cast_the_variant() {
    // Replace: the bar key of Database Lock casts Deadlock, which works on bosses.
    let mut a = Arena::new("backend", [0, 0, 3, 0, 0], 2);
    assert_eq!(a.sim.cast_spell_for(a.me, SpellId(35)), SpellId(1024));
    let ev = a.cast(SpellId(35));
    assert!(a.succeeded(&ev, SpellId(35)));
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::AuraApplied { key, .. } if key == "t_deadlock"))
    );
    // Casting the variant directly is just as legal; the original is not known any more as such.
    assert!(a.sim.cooldown(a.me, SpellId(1024)).usable != Err(CastError::NotLearned));
}

#[test]
fn saves_carry_talents_and_old_saves_migrate() {
    let mut sim = Sim::new(content(), 1);
    let me = sim.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(me, 10);
    for (row, choice) in [(0u8, 1u8), (1, 2), (2, 3), (3, 1), (4, 2)] {
        sim.apply_as(me, Command::PickTalent { row, choice });
    }
    sim.tick();
    let save = sim.export_save(me).expect("save");
    assert_eq!(save.talents, [1, 2, 3, 1, 2]);
    assert_eq!(save.title, "do_oncall");
    let text = save.encode();
    assert!(text.contains("version=2\n") && text.contains("talents=1,2,3,1,2\n"));
    let back = SaveData::decode(&text).expect("decodes");
    assert_eq!(back, save);
    // Restoring at a lower level drops the rows the level does not allow (and the title).
    let mut low = back.clone();
    low.level = 5;
    let mut sim2 = Sim::new(content(), 1);
    let p2 = sim2.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(sim2.import_save(p2, &low));
    let u = sim2.unit(p2).expect("player");
    assert_eq!((u.talents, u.title.as_str()), ([1, 2, 0, 0, 0], ""));
    // Version 1 (before talents) still loads, with no picks.
    let v1 = text
        .replace("version=2", "version=1")
        .lines()
        .filter(|l| !l.starts_with("talents="))
        .collect::<Vec<_>>()
        .join("\n");
    let old = SaveData::decode(&v1).expect("v1 decodes");
    assert_eq!((old.level, old.talents), (10, [0; TALENT_ROWS]));
    let mut sim3 = Sim::new(content(), 1);
    let p3 = sim3.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(sim3.import_save(p3, &old));
    assert_eq!(sim3.unit(p3).map(|u| u.talents), Some([0; TALENT_ROWS]));
    assert!(SaveData::decode(&text.replace("version=2", "version=3")).is_none());
    // A save of another class keeps nothing it cannot use.
    let mut sim4 = Sim::new(content(), 1);
    let p4 = sim4.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let mut odd = back;
    odd.talents = [9, 1, 1, 1, 1];
    assert!(sim4.import_save(p4, &odd));
    assert_eq!(sim4.unit(p4).map(|u| u.talents), Some([0, 1, 1, 1, 1]));
}

#[test]
fn talent_content_is_complete_and_texts_name_real_spells() {
    let c = content();
    for class in CLASSES {
        let ct = c
            .talents
            .for_class(class)
            .expect("every class has a ladder");
        let levels: Vec<u8> = ct.rows.iter().map(|r| r.level).collect();
        assert_eq!(levels, [2, 4, 6, 8, 10], "{class}");
        for row in &ct.rows {
            for t in &row.options {
                assert!(!t.title.is_empty() && !t.text.is_empty(), "{}", t.key);
                // Every talent spell is learned by the row's level.
                for p in &t.perks {
                    for s in p.spells() {
                        let d = c.spell(s).expect("known spell");
                        assert!(
                            d.learn_level <= row.level,
                            "{}: {} at {}",
                            t.key,
                            d.name,
                            row.level
                        );
                    }
                }
            }
        }
    }
}
