//! Out-of-combat regeneration and player spell yells.

mod common;

use combat_core::{Command, Content, Event, Vec3};
use common::*;

fn regen_on(c: &mut Content) {
    c.config.regen.interval_ms = 2000;
    c.config.regen.hp_pct = 4;
    c.config.regen.mana_pct = 5;
}

/// A one-HP CEO dies to the first Code Review, after biting the player a few times.
fn fight_then_win() -> (H, u64) {
    let mut h = H::new(1, -5.0, |c| {
        regen_on(c);
        let d = def_mut(c, "ceo");
        d.max_hp = 1;
        if let Some(ai) = d.ai.as_mut() {
            ai.melee_damage = 20;
            ai.melee_ms = 500;
            // A respawned CEO would re-aggro and end the out-of-combat window.
            ai.respawn_ms = 10_000_000;
            ai.abilities.clear();
        }
    });
    h.target(h.mob);
    h.cast(CR);
    let player = h.player;
    assert!(h.run_until(
        400,
        |e| matches!(e, Event::LeaveCombat { unit } if *unit == player)
    ));
    let left = h
        .first_tick(|e| matches!(e, Event::LeaveCombat { unit } if *unit == h.player))
        .unwrap_or(0);
    (h, left)
}

#[test]
fn regen_starts_one_interval_after_leaving_combat() {
    let (mut h, _) = fight_then_win();
    let (hp, mana) = (h.unit(h.player).hp, h.unit(h.player).mana);
    assert!(hp < 300, "the CEO must have landed a hit");
    h.run(59);
    assert_eq!((h.unit(h.player).hp, h.unit(h.player).mana), (hp, mana));
    h.tick();
    assert_eq!(h.unit(h.player).hp, hp + 12, "4% of 300");
    assert_eq!(h.unit(h.player).mana, mana + 12, "5% of 250, rounded down");
    h.run(59);
    assert_eq!(h.unit(h.player).hp, hp + 12);
    h.tick();
    assert_eq!(h.unit(h.player).hp, hp + 24);
}

#[test]
fn regen_caps_at_max() {
    let (mut h, _) = fight_then_win();
    h.run(30 * 60);
    let p = h.unit(h.player);
    assert_eq!((p.hp, p.mana), (p.max_hp, p.max_mana));
}

#[test]
fn no_percent_regen_in_combat() {
    let mut h = H::new(1, -2.0, |c| {
        regen_on(c);
        def_mut(c, "senior_engineer").max_hp = 100_000;
        let d = def_mut(c, "ceo");
        d.max_hp = 1_000_000;
        if let Some(ai) = d.ai.as_mut() {
            ai.melee_damage = 1;
            ai.abilities.clear();
        }
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(10);
    assert!(h.unit(h.player).in_combat);
    let mut prev = h.unit(h.player).hp;
    for _ in 0..600 {
        h.tick();
        let hp = h.unit(h.player).hp;
        assert!(hp <= prev, "hp rose in combat: {prev} -> {hp}");
        prev = hp;
    }
}

#[test]
fn regen_is_off_when_interval_is_zero() {
    let mut h = H::new(1, -40.0, |c| {
        regen_on(c);
        c.config.regen.interval_ms = 0;
    });
    h.cast(GR);
    h.run(600);
    assert_eq!(h.unit(h.player).mana, 250 - 40);
}

// ---- player spell yells ----

fn yells_of(h: &H, unit: u64) -> Vec<String> {
    h.events()
        .filter_map(|(_, e)| match e {
            Event::Yell { unit: u, text } if *u == unit => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn give_yells(c: &mut Content, spell: combat_core::SpellId, lines: &[&str]) {
    if let Some(s) = c.spells.iter_mut().find(|s| s.id == spell) {
        s.yells = lines.iter().map(|l| (*l).to_string()).collect();
        s.mana = 0;
        s.cooldown_ms = 0;
    }
}

#[test]
fn instant_yells_on_release_and_cooldown_gates_the_next() {
    let mut h = H::new(1, -40.0, |c| give_yells(c, GR, &["a", "b"]));
    // 50-tick spacing clears the GCD; casts land on ticks 1, 51, ... 1201.
    for _ in 0..25 {
        h.cast(GR);
        h.run(50);
    }
    // 20000 ms = 600 ticks: yells at 1, 601 and 1201, round-robin.
    assert_eq!(yells_of(&h, h.player), ["a", "b", "a"]);
    let ticks: Vec<u64> = h
        .events()
        .filter(|(_, e)| matches!(e, Event::Yell { unit, .. } if *unit == h.player))
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(ticks, [1, 601, 1201]);
}

#[test]
fn cast_time_spell_yells_on_completion_not_on_start() {
    let mut h = H::new(1, -20.0, |c| give_yells(c, CR, &["now"]));
    h.target(h.mob);
    h.cast(CR);
    h.run(30);
    assert!(yells_of(&h, h.player).is_empty());
    h.run(40);
    assert_eq!(yells_of(&h, h.player), ["now"]);
}

#[test]
fn round_robin_is_per_spell() {
    let mut h = H::new(1, -20.0, |c| {
        c.config.player_yell_cooldown_ms = 0;
        give_yells(c, GR, &["g1", "g2"]);
        give_yells(c, CR, &["c1", "c2"]);
    });
    h.target(h.mob);
    for s in [GR, CR, GR, CR] {
        h.cast(s);
        h.run(100);
    }
    assert_eq!(yells_of(&h, h.player), ["g1", "c1", "g2", "c2"]);
}

#[test]
fn no_yell_for_auto_attack_failed_or_interrupted_casts_or_enemy_spells() {
    let mut h = H::new(1, -20.0, |c| {
        c.config.player_yell_cooldown_ms = 0;
        give_yells(c, CR, &["c"]);
        give_yells(c, BLAST, &["enemy"]);
        if let Some(s) = c.spells.iter_mut().find(|s| s.id == CR) {
            s.mana = 999;
        }
    });
    h.target(h.mob);
    h.sim.push(Command::StartAutoAttack { unit: h.player });
    h.cast(CR); // not enough mana
    h.run(100);
    assert_eq!(h.failed(CR), vec![combat_core::CastError::NotEnoughMana]);
    assert!(h.count(|e| matches!(e, Event::MeleeSwing { .. } | Event::Damage { .. })) > 0);

    // Interrupted: a cast cancelled by movement never releases.
    let mut h2 = H::new(1, -20.0, |c| give_yells(c, CR, &["c"]));
    h2.target(h2.mob);
    h2.cast(CR);
    h2.run(10);
    h2.sim.push(Command::PlayerMoved {
        pos: Vec3::default(),
        yaw: 0.0,
        moving: true,
        jumping: false,
    });
    h2.run(100);
    assert_eq!(h2.count(|e| matches!(e, Event::CastInterrupted { .. })), 1);

    // Enemy casts never use the player-yell path.
    assert!(h.run_until(
        1500,
        |e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == BLAST)
    ));
    for hh in [&h, &h2] {
        assert!(yells_of(hh, hh.player).is_empty());
        assert!(!yells_of(hh, hh.mob).iter().any(|t| t == "enemy"));
    }
}

#[test]
fn spell_yells_do_not_perturb_the_rng_stream() {
    let run = |with_yells: bool| {
        let mut h = H::new(7, -20.0, |c| {
            c.config.white_hit_pct = 50;
            c.config.spell_hit_pct = 50;
            def_mut(c, "senior_engineer").crit_pct = 50;
            // Same mana in both runs; only the yell lines differ.
            give_yells(c, CR, if with_yells { &["c"] } else { &[] });
        });
        h.target(h.mob);
        for _ in 0..8 {
            h.cast(CR);
            h.run(70);
        }
        h.log
            .into_iter()
            .filter(|(_, e)| !matches!(e, Event::Yell { unit, .. } if *unit == 1))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(true), run(false));
}
