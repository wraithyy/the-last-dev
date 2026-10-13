mod common;

use combat_core::{Command, Event, Vec3};
use common::*;

/// Mob yells as `(tick, text)` in emission order.
fn yells(h: &H) -> Vec<(u64, String)> {
    h.events()
        .filter_map(|(t, e)| match e {
            Event::Yell { unit, text } if *unit == h.mob => Some((*t, text.clone())),
            _ => None,
        })
        .collect()
}

fn calm(c: &mut combat_core::Content) {
    if let Some(ai) = def_mut(c, "ceo").ai.as_mut() {
        ai.abilities.clear();
        ai.melee_damage = 0;
    }
}

fn lines(f: impl Fn(&combat_core::YellDef) -> Vec<String>) -> Vec<String> {
    f(&content()
        .units
        .iter()
        .find(|u| u.key == "ceo")
        .map(|u| u.yells.clone())
        .unwrap_or_default())
}

#[test]
fn aggro_yell_follows_enter_combat_in_the_same_tick() {
    let mut h = H::new(1, -10.0, calm);
    h.run(2);
    let y = yells(&h);
    assert_eq!(y.len(), 1);
    assert_eq!(y[0].1, lines(|d| d.aggro.clone())[0]);
    let enter = h.first_tick(|e| matches!(e, Event::EnterCombat { unit } if *unit == h.mob));
    assert_eq!(enter, Some(y[0].0));
}

#[test]
fn periodic_cadence_and_round_robin() {
    let mut h = H::new(1, -10.0, calm);
    let periodic = lines(|d| d.periodic.clone());
    let n = periodic.len() * 2 + 1;
    h.run(270 * n as u32 + 5);
    let y = yells(&h);
    assert_eq!(y.len(), n + 1, "aggro + n periodic");
    for (k, w) in y.windows(2).enumerate() {
        assert_eq!(w[1].0 - w[0].0, 270);
        assert_eq!(w[1].1, periodic[k % periodic.len()]);
    }
}

#[test]
fn higher_priority_yell_resets_periodic_timer_and_one_per_tick() {
    let mut h = H::new(1, -10.0, |c| {
        calm(c);
        def_mut(c, "ceo").yells.periodic_ms = 100; // 3 ticks
    });
    h.run(20);
    let y = yells(&h);
    assert_eq!(y[0].1, lines(|d| d.aggro.clone())[0]);
    assert_eq!(y[1].0 - y[0].0, 3);
    let mut ticks: Vec<u64> = y.iter().map(|(t, _)| *t).collect();
    ticks.dedup();
    assert_eq!(ticks.len(), y.len(), "at most one yell per tick");
}

#[test]
fn low_hp_once_then_death() {
    let mut h = H::new(1, -10.0, |c| {
        calm(c);
        def_mut(c, "ceo").max_hp = 300;
        def_mut(c, "ceo").yells.periodic_ms = 0;
        def_mut(c, "senior_engineer").max_mana = 1000;
    });
    h.target(h.mob);
    for _ in 0..600 {
        h.cast(CR);
        h.tick();
    }
    let y = yells(&h);
    let low = lines(|d| d.low_hp.clone());
    let death = lines(|d| d.death.clone());
    assert_eq!(y.iter().filter(|(_, t)| low.contains(t)).count(), 1);
    assert_eq!(y.iter().filter(|(_, t)| death.contains(t)).count(), 1);
    assert!(death.contains(&y[y.len() - 1].1), "death yell is last");
}

#[test]
fn player_killed_yell() {
    let mut h = H::new(1, -3.0, |c| {
        if let Some(ai) = def_mut(c, "ceo").ai.as_mut() {
            ai.abilities.clear();
            ai.melee_damage = 10_000;
        }
    });
    assert!(h.run_until(300, |e| matches!(e, Event::Death { .. })));
    h.run(2);
    let pk = lines(|d| d.player_killed.clone());
    assert!(yells(&h).iter().any(|(_, t)| pk.contains(t)));
}

#[test]
fn blast_yell_on_cast_start() {
    let mut h = H::new(1, -10.0, |c| {
        def_mut(c, "ceo").yells.periodic_ms = 0;
    });
    assert!(h.run_until(500, |e| matches!(e, Event::CastStarted { .. })));
    h.run(1);
    let t = h.first_tick(|e| matches!(e, Event::CastStarted { caster, .. } if *caster == h.mob));
    let blast = lines(|d| d.blast.clone());
    let y = yells(&h);
    assert!(y.iter().any(|(yt, s)| Some(*yt) == t && blast.contains(s)));
}

#[test]
fn evade_yell_on_leash() {
    let mut h = H::new(1, -10.0, calm);
    h.run(5);
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 150.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    h.run(300);
    let ev = lines(|d| d.evade.clone());
    assert!(yells(&h).iter().any(|(_, t)| ev.contains(t)));
}

#[test]
fn silent_units_never_yell() {
    let h = H::new(1, -10.0, |_| {});
    assert_eq!(
        h.count(|e| matches!(e, Event::Yell { unit, .. } if *unit == h.player)),
        0
    );
}
