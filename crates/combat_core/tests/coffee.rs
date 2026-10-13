mod common;

use combat_core::{CastError, Command, Event, SpellId, Vec3};
use common::*;

const COFFEE: SpellId = SpellId(12);

fn is_gain(e: &Event) -> bool {
    matches!(e, Event::ManaGain { .. })
}

/// Player at 50/250 mana (Guardrails costs 200 here), GCD already over, nothing in aggro range.
fn drained() -> H {
    let mut h = H::new(1, -40.0, |c| {
        for s in c.spells.iter_mut().filter(|s| s.id == GR) {
            s.mana = 200;
        }
    });
    h.cast(GR);
    h.run(60);
    assert_eq!(h.unit(h.player).mana, 50);
    h.log.clear();
    h
}

fn moved(h: &mut H, moving: bool, jumping: bool) {
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 0.0),
        yaw: 0.0,
        moving,
        jumping,
    });
}

fn interrupted(h: &H) -> usize {
    h.count(|e| matches!(e, Event::CastInterrupted { spell, .. } if *spell == COFFEE))
}

#[test]
fn restores_8_percent_per_second_for_six_seconds_and_not_hp() {
    let mut h = drained();
    let hp = h.unit(h.player).hp;
    h.cast(COFFEE);
    h.run(1);
    assert!(matches!(
        h.log.iter().find(|(_, e)| matches!(e, Event::CastStarted { .. })),
        Some((_, Event::CastStarted { spell, total_ms: 6000, .. })) if *spell == COFFEE
    ));
    h.run(200);
    let gains: Vec<u32> = h
        .events()
        .filter_map(|(_, e)| match e {
            Event::ManaGain { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(gains, vec![20; 6]);
    assert_eq!(h.unit(h.player).mana, 170);
    assert_eq!(h.unit(h.player).hp, hp, "coffee is not food");
    assert_eq!(
        h.count(|e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == COFFEE)),
        1
    );
    assert_eq!(interrupted(&h), 0);
    // Ticks land one second apart; the last on the tick the channel ends.
    let ticks: Vec<u64> = h
        .events()
        .filter(|(_, e)| is_gain(e))
        .map(|(t, _)| *t)
        .collect();
    assert!(ticks.windows(2).all(|w| w[1] - w[0] == 30));
}

#[test]
fn a_full_bar_ends_the_break_early() {
    let mut h = H::new(1, -40.0, |c| {
        for s in c.spells.iter_mut().filter(|s| s.id == GR) {
            s.mana = 30;
        }
    });
    h.cast(GR);
    h.run(60);
    h.cast(COFFEE);
    h.run(120);
    // 220/250: one tick of 20 leaves 10 missing, the second is clamped.
    let gains: Vec<u32> = h
        .events()
        .filter_map(|(_, e)| match e {
            Event::ManaGain { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(gains, vec![20, 10]);
    assert_eq!(h.unit(h.player).mana, 250);
    assert_eq!(
        h.count(|e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == COFFEE)),
        1
    );
    assert!(h.unit(h.player).cast.is_none());
}

#[test]
fn unusable_at_full_mana() {
    let mut h = H::new(1, -40.0, |_| {});
    assert_eq!(
        h.sim.cooldown(h.player, COFFEE).usable,
        Err(CastError::FullMana)
    );
    h.cast(COFFEE);
    h.run(2);
    assert_eq!(h.failed(COFFEE), vec![CastError::FullMana]);
    assert_eq!(
        CastError::FullMana.message(),
        "You are already at full mana."
    );
}

#[test]
fn moving_or_jumping_cancels_and_starts_the_cooldown() {
    for jumping in [false, true] {
        let mut h = drained();
        h.cast(COFFEE);
        h.run(40); // one tick landed
        moved(&mut h, !jumping, jumping);
        h.run(2);
        assert_eq!(interrupted(&h), 1);
        assert_eq!(h.count(is_gain), 1);
        let v = h.sim.cooldown(h.player, COFFEE);
        assert_eq!(v.usable, Err(CastError::OnCooldown));
        assert!(v.cd_remaining_ms > 7000);
    }
}

#[test]
fn cannot_start_while_moving() {
    let mut h = drained();
    moved(&mut h, true, false);
    h.cast(COFFEE);
    h.run(2);
    assert_eq!(h.failed(COFFEE), vec![CastError::Moving]);
}

#[test]
fn casting_another_spell_cancels() {
    let mut h = drained();
    h.cast(COFFEE);
    h.run(60); // past the GCD of the coffee itself
    h.cast(SpellId(8)); // On-Call Caffeine: off-GCD instant, so only the channel rule is under test
    h.run(3);
    assert_eq!(interrupted(&h), 1);
    assert_eq!(
        h.count(|e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == COFFEE)),
        0
    );
}

#[test]
fn pressing_coffee_again_does_not_cancel_it() {
    let mut h = drained();
    h.cast(COFFEE);
    h.run(60);
    h.cast(COFFEE);
    h.run(2);
    assert_eq!(interrupted(&h), 0);
    assert_eq!(h.failed(COFFEE), vec![CastError::AlreadyCasting]);
}

#[test]
fn damage_cancels() {
    // The CEO stands in aggro range and melees the player. Guardrails would absorb the hits
    // (absorbed damage does not break a channel), so it only serves as the mana sink here.
    let mut h = H::new(1, -3.0, |c| {
        for s in c.spells.iter_mut().filter(|s| s.id == GR) {
            s.mana = 200;
            s.effect = combat_core::Effect::Nothing;
        }
    });
    h.cast(GR);
    h.run(60);
    h.cast(COFFEE);
    assert!(h.run_until(
        300,
        |e| matches!(e, Event::CastInterrupted { spell, .. } if *spell == COFFEE)
    ));
    let player = h.player;
    let hit = h
        .events()
        .find(|(t, e)| *t > 61 && matches!(e, Event::Damage { target, .. } if *target == player))
        .map(|(t, _)| *t);
    let cut = h.first_tick(|e| matches!(e, Event::CastInterrupted { .. }));
    assert_eq!(hit, cut);
}

#[test]
fn cooldown_is_8_seconds_after_the_break_ends() {
    let mut h = drained();
    h.cast(COFFEE);
    h.run(185); // 6 s channel done (180 ticks) plus a little
    assert_eq!(
        h.count(|e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == COFFEE)),
        1
    );
    let v = h.sim.cooldown(h.player, COFFEE);
    assert_eq!(v.cd_total_ms, 8000);
    assert_eq!(v.usable, Err(CastError::OnCooldown));
    h.run(240);
    assert_eq!(h.sim.cooldown(h.player, COFFEE).usable, Ok(()));
}

#[test]
fn yells_once_at_the_start_and_respects_the_player_yell_gap() {
    let mut h = H::new(1, -40.0, |c| {
        for s in c.spells.iter_mut().filter(|s| s.id == GR) {
            s.mana = 200;
        }
        for s in c.spells.iter_mut().filter(|s| s.id == COFFEE) {
            s.yells = vec!["Coffee first.".into()];
        }
    });
    h.cast(GR);
    h.run(60);
    h.cast(COFFEE);
    h.run(2);
    let yells = |h: &H| h.count(|e| matches!(e, Event::Yell { unit, .. } if *unit == h.player));
    assert_eq!(yells(&h), 1);
    let (cs, y) = (
        h.first_tick(|e| matches!(e, Event::CastStarted { .. })),
        h.first_tick(|e| matches!(e, Event::Yell { .. })),
    );
    assert_eq!(cs, y);
    h.run(400);
    h.cast(COFFEE);
    h.run(2);
    assert_eq!(yells(&h), 1, "second break inside 20 s stays quiet");
}
