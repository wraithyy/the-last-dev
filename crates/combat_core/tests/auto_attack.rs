//! Player auto attack ("Nitpick") and miss/resist rules.

mod common;

use combat_core::{Command, Event, Vec3};
use common::*;

fn start(h: &mut H) {
    h.sim.push(Command::StartAutoAttack { unit: h.player });
}

fn player_swings(h: &H) -> Vec<u64> {
    let p = h.player;
    h.events()
        .filter(|(_, e)| matches!(e, Event::MeleeSwing { attacker, .. } if *attacker == p))
        .map(|(t, _)| *t)
        .collect()
}

fn toggles(h: &H) -> Vec<bool> {
    h.events()
        .filter_map(|(_, e)| match e {
            Event::AutoAttackToggled { on, .. } => Some(*on),
            _ => None,
        })
        .collect()
}

fn is_miss(e: &Event) -> bool {
    matches!(e, Event::Miss { .. })
}

#[test]
fn swings_every_1800_ms_and_exposes_timer() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    start(&mut h);
    h.tick();
    assert!(h.unit(h.player).auto_attack);
    // 1800 ms = 54 ticks; first swing is immediate on a fresh timer.
    assert!(h.unit(h.player).swing_remaining_ms > 1700);
    h.run(250);
    let swings = player_swings(&h);
    assert_eq!(swings[0], 1);
    assert!(swings.len() >= 5);
    assert!(swings.windows(2).all(|w| w[1] - w[0] == 54), "{swings:?}");
    let dmg = h.damage_to(h.mob);
    assert!(
        dmg.iter()
            .all(|(a, _, crit, _)| *crit || (9..=14).contains(a))
    );
    assert_eq!(toggles(&h), [true]);
}

#[test]
fn swing_damage_is_threat_and_starts_combat() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    start(&mut h);
    h.run(2);
    assert!(h.unit(h.mob).in_combat);
    assert!(h.unit(h.player).in_combat);
}

#[test]
fn stop_command_disables_and_swings_cease() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    start(&mut h);
    h.run(10);
    h.sim.push(Command::StopAutoAttack { unit: h.player });
    h.run(1);
    assert!(!h.unit(h.player).auto_attack);
    assert_eq!(h.unit(h.player).swing_remaining_ms, 0);
    let n = player_swings(&h).len();
    h.run(200);
    assert_eq!(player_swings(&h).len(), n);
    assert_eq!(toggles(&h), [true, false]);
    // Stopping twice emits nothing more.
    h.sim.push(Command::StopAutoAttack { unit: h.player });
    h.run(1);
    assert_eq!(toggles(&h), [true, false]);
}

#[test]
fn target_change_or_clear_disables_but_same_target_does_not() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    start(&mut h);
    h.run(5);
    h.target(h.mob);
    h.run(1);
    assert!(h.unit(h.player).auto_attack);
    h.sim.push(Command::SetTarget {
        unit: h.player,
        target: None,
    });
    h.run(1);
    assert!(!h.unit(h.player).auto_attack);

    let mut h = H::new(1, -20.0, |_| {});
    let intern = h.sim.spawn_unit("intern", Vec3::new(3.0, 0.0, -3.0), 0.0);
    let intern = intern.unwrap_or_else(|e| panic!("{e}"));
    h.target(h.mob);
    start(&mut h);
    h.run(3);
    h.target(intern);
    h.run(1);
    assert!(!h.unit(h.player).auto_attack);
    assert_eq!(toggles(&h), [true, false]);
}

#[test]
fn target_death_disables() {
    let mut h = H::new(1, -20.0, |c| def_mut(c, "ceo").max_hp = 5);
    h.target(h.mob);
    start(&mut h);
    assert!(h.run_until(10, |e| matches!(e, Event::Death { .. })));
    assert!(!h.unit(h.player).auto_attack);
    assert_eq!(toggles(&h), [true, false]);
    // A dead target cannot be re-engaged.
    start(&mut h);
    h.run(2);
    assert_eq!(toggles(&h), [true, false]);
}

#[test]
fn attacker_death_disables_and_respawn_stays_off() {
    let mut h = H::new(1, -5.0, |c| {
        def_mut(c, "senior_engineer").max_hp = 10;
        def_mut(c, "ceo").max_hp = 100_000;
    });
    h.target(h.mob);
    start(&mut h);
    let p = h.player;
    assert!(h.run_until(
        400,
        |e| matches!(e, Event::Death { unit, .. } if *unit == p)
    ));
    assert!(!h.unit(p).auto_attack);
    assert_eq!(toggles(&h), [true, false]);
    h.sim.push(Command::RespawnPlayer);
    h.run(60);
    assert!(!h.unit(p).auto_attack);
    assert_eq!(toggles(&h), [true, false]);
}

#[test]
fn start_without_valid_target_does_nothing() {
    let mut h = H::new(1, -20.0, |_| {});
    start(&mut h);
    h.run(2);
    let intern = h.sim.spawn_unit("intern", Vec3::new(3.0, 0.0, -3.0), 0.0);
    h.target(intern.unwrap_or_else(|e| panic!("{e}")));
    start(&mut h);
    h.run(2);
    assert!(!h.unit(h.player).auto_attack);
    assert!(h.log.is_empty(), "no events expected: {:?}", h.log);
}

#[test]
fn ready_swing_waits_for_cast_to_end() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    h.cast(CR);
    start(&mut h);
    assert!(h.run_until(100, |e| matches!(e, Event::CastSucceeded { .. })));
    let done = h.first_tick(|e| matches!(e, Event::CastSucceeded { .. }));
    assert_eq!(player_swings(&h), vec![done.unwrap_or(0)]);
}

#[test]
fn out_of_range_waits_without_errors_then_swings_when_close() {
    let mut h = H::new(1, -40.0, |_| {});
    h.target(h.mob);
    start(&mut h);
    h.run(60);
    assert!(player_swings(&h).is_empty());
    assert!(h.unit(h.player).auto_attack);
    assert_eq!(toggles(&h), [true]);
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, -20.0),
        yaw: 0.0,
        moving: true,
        jumping: false,
    });
    h.run(3);
    assert_eq!(player_swings(&h).len(), 1, "moving must not block swings");
}

#[test]
fn zero_hit_chance_makes_every_swing_miss() {
    let mut h = H::new(1, -20.0, |c| c.config.white_hit_pct = 0);
    h.target(h.mob);
    start(&mut h);
    h.run(200);
    let swings = player_swings(&h).len();
    assert!(swings >= 3);
    let misses = h.count(|e| matches!(e, Event::Miss { source, spell: None, .. } if *source == 1));
    assert_eq!(misses, swings);
    assert!(h.damage_to(h.mob).is_empty());
    assert!(!h.unit(h.mob).in_combat, "a miss generates no threat");
}

#[test]
fn full_hit_chance_never_misses() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    start(&mut h);
    h.run(300);
    assert_eq!(h.count(is_miss), 0);
}

#[test]
fn missed_spell_spends_mana_and_applies_no_debuff() {
    let mut h = H::new(1, -20.0, |c| c.config.spell_hit_pct = 0);
    h.target(h.mob);
    h.cast(CR);
    assert!(h.run_until(100, is_miss));
    assert!(matches!(
        h.log.last(),
        Some((_, Event::Miss { spell: Some(s), .. })) if *s == CR
    ));
    let p = h.unit(h.player);
    assert!((214..=216).contains(&p.mana), "mana {}", p.mana);
    assert_eq!(h.unit(h.mob).hp, 900);
    assert!(h.unit(h.mob).auras.is_empty());
    assert_eq!(h.count(|e| matches!(e, Event::AuraApplied { .. })), 0);
    assert!(!h.unit(h.mob).in_combat);
}

#[test]
fn guardrails_never_miss() {
    let mut h = H::new(1, -20.0, |c| c.config.spell_hit_pct = 0);
    h.cast(GR);
    h.run(3);
    assert_eq!(h.count(is_miss), 0);
    assert!(h.unit(h.player).auras.iter().any(|a| a.key == "guardrails"));
}

#[test]
fn reject_pr_never_misses_and_enemy_blast_can_be_resisted() {
    let mut h = H::new(1, -8.0, |c| {
        c.config.spell_hit_pct = 0;
        if let Some(ab) = def_mut(c, "ceo")
            .ai
            .as_mut()
            .and_then(|a| a.abilities.first_mut())
        {
            ab.cooldown_min_ms = 100;
            ab.cooldown_max_ms = 100;
        }
    });
    h.target(h.mob);
    start(&mut h);
    let mob = h.mob;
    assert!(h.run_until(120, |e| {
        matches!(e, Event::CastStarted { caster, .. } if *caster == mob)
    }));
    h.sim.push(Command::Cast {
        caster: h.player,
        spell: RPR,
    });
    assert!(h.run_until(5, |e| matches!(e, Event::CastInterrupted { .. })));
    assert!(
        !h.events()
            .any(|(_, e)| matches!(e, Event::Miss { spell: Some(s), .. } if *s == RPR))
    );

    // Let the next Blast land: with 0% spell hit it is resisted, not dealt.
    assert!(h.run_until(
        400,
        |e| matches!(e, Event::Miss { source, spell: Some(_), .. } if *source == mob)
    ));
    assert!(
        !h.events()
            .any(|(_, e)| matches!(e, Event::Damage { spell: Some(s), .. } if *s == BLAST))
    );
}
