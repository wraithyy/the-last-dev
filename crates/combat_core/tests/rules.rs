mod common;

use combat_core::{
    AiState, AuraEnd, CastError, Command, Content, ContentError, Event, FixedStepper, SpellId, Vec3,
};
use common::*;
use std::f32::consts::PI;

/// Content tweak: no enemy ability, harmless melee, huge HP (a damage sponge).
fn sponge(c: &mut Content) {
    let m = def_mut(c, "ai_evangelist");
    m.max_hp = 1_000_000_000;
    if let Some(ai) = m.ai.as_mut() {
        ai.ability = None;
        ai.melee_damage = 0;
    }
}

// ---- GCD, haste, timers ----

#[test]
fn gcd_is_1500_ms_and_blocks_other_gcd_spells() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    h.cast(GR);
    h.tick();
    let v = h.sim.cooldown(h.player, CR);
    assert_eq!((v.gcd_total_ms, v.gcd_remaining_ms), (1500, 1500));
    assert_eq!(v.usable, Err(CastError::OnGcd));
    h.run(19);
    h.cast(CR); // ~1.0 s of GCD left: outside the 400 ms queue window
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::OnGcd]);
    h.run(25);
    assert_eq!(h.sim.cooldown(h.player, CR).usable, Ok(()));
}

#[test]
fn haste_shortens_gcd_with_a_1000_ms_floor_and_scales_cast_time() {
    for (haste, gcd) in [(0, 1500), (25, 1200), (100, 1000), (400, 1000)] {
        let mut h = H::new(1, -20.0, |c| {
            def_mut(c, "senior_engineer").haste_pct = haste
        });
        h.cast(GR);
        h.tick();
        assert_eq!(
            h.sim.cooldown(h.player, CR).gcd_total_ms,
            gcd,
            "haste {haste}"
        );
    }
    let mut h = H::new(1, -20.0, |c| def_mut(c, "senior_engineer").haste_pct = 25);
    h.target(h.mob);
    h.cast(CR);
    h.tick();
    assert!(
        h.events()
            .any(|(_, e)| matches!(e, Event::CastStarted { total_ms: 1600, .. }))
    );
}

#[test]
fn gcd_cooldown_cast_and_mana_are_distinct_timers() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    h.cast(CR);
    h.tick();
    let cast = h.unit(h.player).cast;
    assert_eq!(cast.map(|c| c.total_ms), Some(2000));
    assert_eq!(
        h.unit(h.player).mana,
        250,
        "mana is not spent at cast start"
    );
    h.run(46); // GCD (1500 ms) over, cast (2000 ms) still running
    let v = h.sim.cooldown(h.player, CR);
    assert_eq!(v.gcd_remaining_ms, 0);
    assert!(h.unit(h.player).cast.is_some());
    assert_eq!(v.usable, Err(CastError::AlreadyCasting));
    h.run(20);
    let mana = h.unit(h.player).mana;
    assert!(
        (215..=220).contains(&mana),
        "35 spent on completion (plus a little regen), got {mana}"
    );
    // Guardrails: its own 30 s cooldown is independent of the GCD.
    h.cast(GR);
    h.tick();
    h.run(50);
    let v = h.sim.cooldown(h.player, GR);
    assert_eq!(v.gcd_remaining_ms, 0);
    assert!(v.cd_remaining_ms > 28_000 && v.cd_total_ms == 30_000);
    assert_eq!(v.usable, Err(CastError::OnCooldown));
}

#[test]
fn melee_timer_is_independent_of_spell_timers() {
    let mut h = H::new(1, -2.0, |c| {
        def_mut(c, "senior_engineer").max_hp = 10_000;
        if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
            ai.ability = None;
        }
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(130);
    let swings: Vec<u64> = h
        .events()
        .filter(|(_, e)| matches!(e, Event::MeleeSwing { .. }))
        .map(|(t, _)| *t)
        .collect();
    assert_eq!(
        swings,
        vec![2, 62, 122],
        "every 2000 ms regardless of the player's casting"
    );
}

// ---- spell queue ----

#[test]
fn cast_inside_queue_window_is_queued_and_starts_on_first_legal_tick() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    h.cast(CR); // tick 1, completes tick 61
    h.run(39);
    h.cast(RF); // tick 40: 21 ticks (700 ms) left, outside the window
    h.tick();
    assert_eq!(h.failed(RF), vec![CastError::AlreadyCasting]);
    h.run(8);
    h.cast(RF); // tick 49: 12 ticks (400 ms) left, inside the window
    h.tick();
    assert_eq!(
        h.first_tick(|e| matches!(e, Event::CastQueued { spell, .. } if *spell == RF)),
        Some(49)
    );
    assert_eq!(
        h.count(|e| matches!(e, Event::CastStarted { spell, .. } if *spell == RF)),
        0
    );
    h.run(15);
    let ok = h.first_tick(|e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == CR));
    let started = h.first_tick(|e| matches!(e, Event::CastStarted { spell, .. } if *spell == RF));
    assert_eq!(
        (ok, started),
        (Some(61), Some(61)),
        "queued cast starts the tick the cast ends"
    );
}

#[test]
fn queued_cast_is_revalidated_when_it_starts() {
    let mut h = H::new(1, -20.0, |c| {
        def_mut(c, "senior_engineer").spell_power = 100_000
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(49);
    h.cast(RF);
    h.run(20);
    assert!(h.count(|e| matches!(e, Event::CastQueued { .. })) == 1);
    assert_eq!(
        h.failed(RF),
        vec![CastError::Dead],
        "target died before the queued cast could start"
    );

    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    h.cast(CR);
    h.run(49);
    h.cast(RF);
    h.tick();
    h.sim.push(Command::SetTarget {
        unit: h.player,
        target: None,
    });
    h.run(15);
    assert_eq!(h.failed(RF), vec![CastError::NoTarget]);
}

#[test]
fn stop_casting_cancels_the_cast_and_the_queue() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    h.cast(CR);
    h.run(49);
    h.cast(RF);
    h.tick();
    h.sim.push(Command::StopCasting { caster: h.player });
    h.run(30);
    assert_eq!(
        h.count(|e| matches!(e, Event::CastInterrupted { by: None, .. })),
        1
    );
    assert_eq!(
        h.count(|e| matches!(e, Event::CastStarted { spell, .. } if *spell == RF)),
        0
    );
    assert_eq!(h.unit(h.player).mana, 250);
}

// ---- resources ----

#[test]
fn mana_is_spent_on_completion_and_insufficient_mana_fails() {
    let mut h = H::new(1, -20.0, |c| def_mut(c, "senior_engineer").max_mana = 30);
    h.target(h.mob);
    h.cast(CR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::NotEnoughMana]);
    assert_eq!(
        h.sim.cooldown(h.player, CR).usable,
        Err(CastError::NotEnoughMana)
    );
    assert!(h.unit(h.player).cast.is_none());
}

#[test]
fn mana_regenerates_fast_out_of_combat_and_slow_in_combat() {
    let drain = |c: &mut Content| {
        if let Some(s) = c.spells.iter_mut().find(|s| s.id == GR) {
            s.mana = 200;
            s.cooldown_ms = 0;
        }
    };
    let delta = |mob_z: f32, damage: u32| {
        let mut h = H::new(1, mob_z, |c| {
            drain(c);
            if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
                ai.melee_damage = damage;
                ai.ability = None;
            }
        });
        h.cast(GR);
        h.tick();
        let before = h.unit(h.player).mana;
        h.run(150);
        (h.unit(h.player).in_combat, h.unit(h.player).mana - before)
    };
    assert_eq!(
        delta(-40.0, 0),
        (false, 100),
        "100 mana per 5 s out of combat"
    );
    assert_eq!(delta(-2.0, 0), (true, 25), "25 mana per 5 s in combat");
}

// ---- movement ----

#[test]
fn cast_time_spells_fail_while_moving_but_instants_are_allowed() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::default(),
        yaw: 0.0,
        moving: true,
        jumping: false,
    });
    h.cast(CR);
    h.cast(GR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::Moving]);
    assert_eq!(
        h.count(|e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == GR)),
        1
    );
    assert_eq!(CastError::Moving.message(), "Can't do that while moving.");
}

#[test]
fn moving_or_jumping_mid_cast_cancels_without_spending_mana() {
    for (moving, jumping) in [(true, false), (false, true)] {
        let mut h = H::new(1, -20.0, |_| {});
        h.target(h.mob);
        h.cast(CR);
        h.run(20);
        h.sim.push(Command::PlayerMoved {
            pos: Vec3::default(),
            yaw: 0.0,
            moving,
            jumping,
        });
        h.tick();
        assert_eq!(
            h.count(|e| matches!(
                e,
                Event::CastInterrupted {
                    by: None,
                    lockout_ms: 0,
                    ..
                }
            )),
            1
        );
        assert!(h.unit(h.player).cast.is_none());
        h.run(60);
        assert_eq!(h.unit(h.player).mana, 250);
        assert!(h.damage_to(h.mob).is_empty());
    }
}

#[test]
fn standing_still_reports_do_not_cancel_the_cast() {
    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    h.cast(CR);
    for _ in 0..70 {
        h.sim.push(Command::PlayerMoved {
            pos: Vec3::default(),
            yaw: 0.0,
            moving: false,
            jumping: false,
        });
        h.tick();
    }
    assert_eq!(h.damage_to(h.mob).len(), 1);
}

// ---- pushback ----

#[test]
fn incoming_damage_pushes_the_cast_back_at_most_twice() {
    let mut h = H::new(1, -2.0, |c| {
        if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
            ai.melee_ms = 500;
            ai.melee_damage = 10;
            ai.ability = None;
        }
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(2); // first swing at tick 2
    assert_eq!(h.unit(h.player).cast.map(|c| c.total_ms), Some(2500));
    h.run(40);
    assert_eq!(h.unit(h.player).cast.map(|c| c.total_ms), Some(3000));
    h.run(60);
    let done = h.first_tick(|e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == CR));
    assert_eq!(
        done,
        Some(91),
        "61 + 2 pushbacks of 15 ticks, third hit ignored"
    );
}

// ---- interrupt and lockout ----

fn caster_mob(c: &mut Content) {
    if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
        ai.ability.iter_mut().for_each(|a| {
            a.cooldown_min_ms = 100;
            a.cooldown_max_ms = 100;
        });
    }
}

#[test]
fn reject_pr_interrupts_and_locks_the_school_for_4_seconds() {
    let mut h = H::new(1, -10.0, caster_mob);
    h.target(h.mob);
    assert!(h.run_until(
        30,
        |e| matches!(e, Event::CastStarted { spell, .. } if *spell == BLAST)
    ));
    h.run(10);
    h.cast(RPR);
    h.tick();
    let at = h.sim.now().0;
    assert!(h.events().any(|(t, e)| *t == at
        && matches!(e, Event::CastInterrupted { by: Some(p), lockout_ms: 4000, .. } if *p == h.player)));
    assert!(h.unit(h.mob).cast.is_none());
    assert_eq!(
        h.count(|e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == BLAST)),
        0
    );
    // Ability is due every 100 ms, so the 4 s lockout is the only thing delaying the next cast.
    assert!(h.run_until(
        200,
        |e| matches!(e, Event::CastStarted { spell, .. } if *spell == BLAST)
    ));
    let next = h.log.last().map(|(t, _)| *t);
    assert_eq!(next, Some(at + 120));
    // Reject PR is off the GCD but has its own 20 s cooldown.
    h.cast(RPR);
    h.tick();
    assert_eq!(h.failed(RPR), vec![CastError::OnCooldown]);
}

#[test]
fn reject_pr_failure_reasons() {
    let mut h = H::new(1, -10.0, |_| {});
    h.target(h.mob);
    h.cast(RPR);
    h.tick();
    assert_eq!(h.failed(RPR), vec![CastError::NothingToInterrupt]);
    assert_eq!(
        CastError::NothingToInterrupt.message(),
        "Nothing to interrupt."
    );

    let mut h = H::new(1, -10.0, |c| {
        caster_mob(c);
        if let Some(s) = c.spells.iter_mut().find(|s| s.id == BLAST) {
            s.interruptible = false;
        }
    });
    h.target(h.mob);
    assert!(h.run_until(30, |e| matches!(e, Event::CastStarted { .. })));
    h.cast(RPR);
    h.tick();
    assert_eq!(h.failed(RPR), vec![CastError::NotInterruptible]);
    assert!(h.unit(h.mob).cast.is_some_and(|c| !c.interruptible));
}

#[test]
fn reject_pr_works_mid_cast_and_off_gcd() {
    let mut h = H::new(1, -10.0, caster_mob);
    h.target(h.mob);
    h.run_until(30, |e| matches!(e, Event::CastStarted { .. }));
    h.cast(CR);
    h.tick();
    h.cast(RPR);
    h.tick();
    assert!(h.failed(RPR).is_empty());
    assert!(h.unit(h.player).cast.is_some(), "own cast keeps running");
}

// ---- targeting: range, facing, LoS, validity ----

#[test]
fn hostile_spells_check_range_facing_and_line_of_sight() {
    let mut h = H::new(1, -40.0, |_| {});
    h.target(h.mob);
    h.cast(CR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::OutOfRange]);

    let mut h = H::new(1, -20.0, |_| {});
    h.target(h.mob);
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::default(),
        yaw: PI,
        moving: false,
        jumping: false,
    });
    h.cast(CR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::NotFacing]);

    let mut h = H::new(1, -20.0, |_| {});
    h.sim.set_blockers(vec![wall(-10.0)]);
    h.target(h.mob);
    h.cast(CR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::NoLineOfSight]);

    let mut h = H::new(1, -20.0, |_| {});
    h.cast(CR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::NoTarget]);
}

#[test]
fn friendly_unit_cannot_be_attacked() {
    let mut h = H::new(1, -20.0, |_| {});
    let intern = match h.sim.spawn_unit("intern", Vec3::new(0.0, 0.0, -5.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    h.target(intern);
    h.cast(CR);
    h.cast(RPR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::InvalidTarget]);
    assert_eq!(h.failed(RPR), vec![CastError::InvalidTarget]);
    let i = h.unit(intern);
    assert!((i.level, i.hostile, i.ai, i.max_hp > 0) == (1, false, None, true));
    assert!(matches!(
        h.sim.spawn_unit("nope", Vec3::default(), 0.0),
        Err(ContentError::UnknownUnit(_))
    ));
}

// ---- damage, crit, combo, absorb ----

#[test]
fn damage_formulas_match_the_spec_without_crit() {
    let mut h = H::new(1, -20.0, |c| {
        def_mut(c, "senior_engineer").crit_pct = 0;
        def_mut(c, "senior_engineer").max_mana = 1000;
        sponge(c);
    });
    h.target(h.mob);
    h.cast(RF);
    h.run(80);
    h.cast(CR);
    h.run(70);
    h.cast(RF);
    h.run(80);
    let d = h.damage_to(h.mob);
    // 95+0.85*20 = 112; 65+0.70*20 = 79; Refactor on Code Smell: 112*1.35 = 151.
    assert_eq!(
        d,
        vec![
            (112, 0, false, false),
            (79, 0, false, false),
            (151, 0, false, true)
        ]
    );
    assert_eq!(
        h.count(|e| matches!(
            e,
            Event::AuraRemoved {
                reason: AuraEnd::Consumed,
                ..
            }
        )),
        1
    );
}

#[test]
fn crit_deals_150_percent_and_is_seeded() {
    let big_mana = |crit: u32| {
        move |c: &mut Content| {
            def_mut(c, "senior_engineer").crit_pct = crit;
            def_mut(c, "senior_engineer").max_mana = 100_000;
            sponge(c);
        }
    };
    let run = |seed: u64, crit: u32| {
        let mut h = H::new(seed, -20.0, big_mana(crit));
        h.target(h.mob);
        for _ in 0..12 {
            h.cast(CR);
            h.run(62);
        }
        h.damage_to(h.mob)
    };
    assert!(run(1, 100).iter().all(|d| *d == (118, 0, true, false)));
    assert!(run(1, 0).iter().all(|d| *d == (79, 0, false, false)));
    let a = run(7, 50);
    assert_eq!(a, run(7, 50), "same seed, same rolls");
    assert!(a.iter().any(|d| d.2) && a.iter().any(|d| !d.2));
    assert_ne!(run(7, 50), run(8, 50), "another seed rolls differently");
}

#[test]
fn guardrails_absorbs_then_depletes() {
    let mut h = H::new(1, -2.0, |c| {
        if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
            ai.melee_damage = 60;
            ai.ability = None;
        }
    });
    h.cast(GR);
    h.run(3);
    let shield = h.unit(h.player).auras.first().cloned();
    assert_eq!(shield.as_ref().map(|a| a.absorb_remaining), Some(Some(40)));
    h.run(60);
    assert_eq!(
        h.damage_to(h.player),
        vec![(0, 60, false, false), (20, 40, false, false)]
    );
    assert_eq!(
        h.count(|e| matches!(
            e,
            Event::AuraRemoved {
                reason: AuraEnd::Depleted,
                ..
            }
        )),
        1
    );
    assert_eq!(h.unit(h.player).hp, 280);
    assert!(h.unit(h.player).auras.is_empty());
}

#[test]
fn guardrails_expires_after_10_seconds() {
    let mut h = H::new(1, -40.0, |_| {});
    h.cast(GR);
    h.run(310);
    let applied =
        h.first_tick(|e| matches!(e, Event::AuraApplied { key, .. } if key == "guardrails"));
    let gone = h.first_tick(|e| {
        matches!(
            e,
            Event::AuraRemoved {
                reason: AuraEnd::Expired,
                ..
            }
        )
    });
    assert_eq!((applied, gone), (Some(1), Some(301)));
}

#[test]
fn code_smell_applies_for_12_seconds_and_refresh_does_not_stack() {
    let mut h = H::new(1, -20.0, |c| {
        def_mut(c, "senior_engineer").max_mana = 1000;
        sponge(c);
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(62);
    h.cast(CR);
    h.run(62);
    let smell = h
        .unit(h.mob)
        .auras
        .iter()
        .filter(|a| a.key == "code_smell")
        .count();
    assert_eq!(smell, 1);
    assert!(
        h.unit(h.mob)
            .auras
            .iter()
            .all(|a| a.harmful && a.total_ms == 12_000)
    );
}

// ---- aggro, leash, evade ----

#[test]
fn aggro_triggers_inside_12_m_only() {
    let mut h = H::new(1, -11.0, |_| {});
    h.run(2);
    assert_eq!(h.unit(h.mob).ai, Some(AiState::Chase));
    let mut h = H::new(1, -13.0, |_| {});
    h.run(10);
    assert_eq!(h.unit(h.mob).ai, Some(AiState::Idle));
    assert!(!h.unit(h.mob).in_combat);
}

#[test]
fn chase_faces_target_and_stops_in_melee_range() {
    let mut h = H::new(1, -11.0, sponge);
    h.run(90);
    let m = h.unit(h.mob);
    let d = (m.pos.x.powi(2) + m.pos.z.powi(2)).sqrt();
    assert!(d <= 2.5 && d > 2.0, "stopped at {d}");
    assert_eq!(m.ai, Some(AiState::Attack));
    assert!(
        m.yaw.abs() < 1e-3 || (m.yaw - PI).abs() > 1.0,
        "faces the player (+Z)"
    );
    assert!((combat_core::forward(m.yaw).z - 1.0).abs() < 1e-3);
}

#[test]
fn chase_speed_is_about_6_m_per_s() {
    let mut h = H::new(1, -12.0, sponge);
    h.run(31); // aggro on tick 1, then 30 ticks of movement
    let travelled = 12.0 + h.unit(h.mob).pos.z;
    assert!((travelled - 6.0).abs() < 0.2, "moved {travelled} m in 1 s");
}

#[test]
fn leash_makes_the_enemy_evade_heal_and_ignore_damage() {
    let mut h = H::new(1, -20.0, |c| {
        def_mut(c, "senior_engineer").max_mana = 1000;
        if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
            ai.ability = None;
        }
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(62);
    let max_hp = h.unit(h.mob).max_hp;
    assert!(h.unit(h.mob).hp < max_hp && h.unit(h.mob).in_combat);
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 150.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    assert!(h.run_until(600, |e| matches!(
        e,
        Event::AiState {
            state: AiState::Evade,
            ..
        }
    )));
    let m = h.unit(h.mob);
    let from_spawn = (m.pos.z + 20.0).abs();
    assert!(
        from_spawn > 35.0 && from_spawn < 36.5,
        "evade began {from_spawn} m from spawn"
    );
    assert!(!m.in_combat && m.target.is_none());

    // An evading mob is untargetable: the cast is refused, nothing is dealt.
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 30.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    h.cast(CR);
    let before = h.damage_to(h.mob).len();
    h.run(80);
    assert_eq!(h.failed(CR), vec![CastError::InvalidTarget]);
    assert_eq!(h.damage_to(h.mob).len(), before);
    assert_eq!(h.unit(h.mob).ai, Some(AiState::Evade));

    assert!(h.run_until(600, |e| matches!(
        e,
        Event::AiState {
            state: AiState::Idle,
            ..
        }
    )));
    let m = h.unit(h.mob);
    assert_eq!((m.hp, m.pos), (max_hp, Vec3::new(0.0, 0.0, -20.0)));
    assert!(m.auras.is_empty(), "debuffs drop on reset");
}

// ---- death, loot, respawn ----

fn one_shot(c: &mut Content) {
    def_mut(c, "senior_engineer").spell_power = 100_000;
    if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
        ai.ability = None;
    }
}

#[test]
fn death_grants_xp_drops_loot_and_respawns_after_15_seconds() {
    let mut h = H::new(1, -20.0, |c| {
        one_shot(c);
        def_mut(c, "ai_evangelist").loot[0].chance_pct = 100;
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(61);
    assert!(h.events().any(|(_, e)| matches!(e, Event::Death { unit, killer } if *unit == h.mob && *killer == Some(h.player))));
    assert!(
        h.events()
            .any(|(_, e)| matches!(e, Event::XpGained { amount: 120, unit } if *unit == h.player))
    );
    assert!(h.events().any(
        |(_, e)| matches!(e, Event::LootDropped { items, .. } if items == &["Prompt Fragment"])
    ));
    let m = h.unit(h.mob);
    assert!(
        (m.alive, m.lootable, m.ai, m.respawn_in_ms)
            == (false, true, Some(AiState::Dead), Some(15_000))
    );
    assert_eq!(h.unit(h.player).xp, 120);
    assert!(!h.unit(h.player).in_combat);
    assert_eq!(
        h.unit(h.player).target,
        Some(h.mob),
        "SetTarget/target on a corpse is kept for looting"
    );

    h.cast(CR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::Dead]);

    // Out of loot range first, then walk up.
    h.sim.push(Command::Loot {
        looter: h.player,
        corpse: h.mob,
    });
    h.tick();
    assert_eq!(h.count(|e| matches!(e, Event::Looted { .. })), 0);
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, -17.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    h.sim.push(Command::Loot {
        looter: h.player,
        corpse: h.mob,
    });
    h.sim.push(Command::Loot {
        looter: h.player,
        corpse: h.mob,
    });
    h.tick();
    assert_eq!(
        h.count(|e| matches!(e, Event::Looted { items, .. } if items == &["Prompt Fragment"])),
        1
    );
    assert!(!h.unit(h.mob).lootable);

    assert!(h.run_until(500, |e| matches!(e, Event::Respawned { .. })));
    let died = h.first_tick(|e| matches!(e, Event::Death { .. }));
    let back = h.first_tick(|e| matches!(e, Event::Respawned { .. }));
    assert_eq!(back.zip(died).map(|(b, d)| b - d), Some(450));
    let m = h.unit(h.mob);
    assert!((m.alive, m.hp, m.lootable, m.ai) == (true, m.max_hp, false, Some(AiState::Idle)));
    assert_eq!(m.pos, Vec3::new(0.0, 0.0, -20.0));
}

#[test]
fn loot_drop_follows_configured_probability() {
    for (chance, expect) in [(0, 0), (100, 1)] {
        let mut h = H::new(3, -20.0, |c| {
            one_shot(c);
            def_mut(c, "ai_evangelist").loot[0].chance_pct = chance;
        });
        h.target(h.mob);
        h.cast(CR);
        h.run(61);
        let n = h.log.iter().find_map(|(_, e)| match e {
            Event::LootDropped { items, .. } => Some(items.len()),
            _ => None,
        });
        assert_eq!(n, Some(expect));
    }
}

#[test]
fn player_death_resets_the_encounter_and_respawn_restores_the_player() {
    let mut h = H::new(1, -2.0, |c| {
        if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
            ai.melee_damage = 1000;
            ai.ability = None;
        }
    });
    h.target(h.mob);
    h.run(5);
    assert!(
        h.events()
            .any(|(_, e)| matches!(e, Event::Death { unit, .. } if *unit == h.player))
    );
    assert!(!h.unit(h.player).alive);
    assert_eq!(
        h.unit(h.mob).ai,
        Some(AiState::Idle),
        "mob reset after the player died"
    );
    assert!(h.events().any(|(_, e)| matches!(
        e,
        Event::AiState {
            state: AiState::Evade,
            ..
        }
    )));

    h.cast(CR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::Dead]);

    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 50.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    h.sim.push(Command::RespawnPlayer);
    h.tick();
    let p = h.unit(h.player);
    assert!((p.alive, p.hp, p.mana, p.pos) == (true, 300, 250, Vec3::new(0.0, 0.0, 0.0)));
    assert!(
        h.events()
            .any(|(_, e)| matches!(e, Event::Respawned { unit } if *unit == h.player))
    );
}

// ---- messages ----

#[test]
fn cast_error_messages_are_wow_style() {
    use CastError::*;
    let all = [
        (OutOfRange, "Out of range."),
        (NotEnoughMana, "Not enough mana."),
        (NotFacing, "You must be facing your target."),
        (NoLineOfSight, "Target not in line of sight."),
        (OnCooldown, "Ability is not ready yet."),
        (Moving, "Can't do that while moving."),
        (InvalidTarget, "Invalid target."),
        (NothingToInterrupt, "Nothing to interrupt."),
        (NotInterruptible, "That spell can't be interrupted."),
    ];
    for (e, m) in all {
        assert_eq!(e.message(), m);
    }
    for e in [NoTarget, Dead, OnGcd, AlreadyCasting, SchoolLocked] {
        assert!(!e.message().is_empty());
    }
}

// ---- 30 Hz invariance ----

fn drive(sim: &mut combat_core::Sim, player: u64, mob: u64) {
    match sim.now().0 {
        1 => sim.push(Command::SetTarget {
            unit: player,
            target: Some(mob),
        }),
        2 => sim.push(Command::Cast {
            caster: player,
            spell: CR,
        }),
        90 => sim.push(Command::Cast {
            caster: player,
            spell: GR,
        }),
        100 => sim.push(Command::Cast {
            caster: player,
            spell: RF,
        }),
        _ => {}
    }
}

fn final_state(frames_us: &[u64]) -> (String, u64) {
    let mut sim = combat_core::Sim::new(content(), 99);
    let player = sim.spawn_player(Vec3::default(), 0.0);
    let mob = match sim.spawn_unit("ai_evangelist", Vec3::new(0.0, 0.0, -15.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    let mut stepper = FixedStepper::new();
    let mut log = String::new();
    let mut i = 0;
    while sim.now().0 < 600 {
        let n = stepper.advance(frames_us[i % frames_us.len()]);
        i += 1;
        for _ in 0..n {
            if sim.now().0 >= 600 {
                break;
            }
            drive(&mut sim, player, mob);
            for e in sim.tick() {
                log.push_str(&format!("{} {e:?}\n", sim.now().0));
            }
        }
    }
    let units: Vec<String> = sim.units().map(|u| format!("{u:?}")).collect();
    (format!("{log}{}", units.join("\n")), sim.now().0)
}

#[test]
fn simulation_is_invariant_to_render_frame_rate() {
    let at_60 = final_state(&[16_667]);
    let at_144 = final_state(&[6_944]);
    let uneven = final_state(&[16_000, 33_000, 7_000, 41_000, 5_000, 12_345, 20_000]);
    assert_eq!(at_60.1, 600);
    assert!(at_60.0.contains("Damage"), "scenario actually fights");
    assert_eq!(at_60, at_144);
    assert_eq!(at_60, uneven);
}

#[test]
fn unknown_spell_ids_are_ignored() {
    let mut h = H::new(1, -20.0, |_| {});
    h.cast(SpellId(999));
    h.tick();
    assert!(h.log.is_empty());
}

// ---- review regressions ----

#[test]
fn evading_target_is_invalid_and_completing_cast_spends_no_mana() {
    let mut h = H::new(1, -20.0, |c| {
        def_mut(c, "senior_engineer").max_mana = 1000;
        if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
            ai.ability = None;
        }
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(62);
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 150.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    assert!(h.run_until(600, |e| matches!(
        e,
        Event::AiState {
            state: AiState::Evade,
            ..
        }
    )));
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 30.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    h.tick();
    let mana = h.unit(h.player).mana;
    h.cast(CR);
    h.tick();
    assert_eq!(h.failed(CR), vec![CastError::InvalidTarget]);
    assert!(h.unit(h.player).mana >= mana);

    // Cast started on a fresh pull, target starts evading before completion.
    let mut h = H::new(1, -20.0, |c| {
        def_mut(c, "senior_engineer").max_mana = 1000;
        if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
            ai.ability = None;
            ai.leash_m = 10.0;
        }
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(61); // hit lands; the mob engages and will exceed the 10 m leash in ~50 ticks
    h.cast(RF);
    h.tick();
    let mana = h.unit(h.player).mana;
    assert!(h.run_until(
        120,
        |e| matches!(e, Event::CastFailed { spell, .. } if *spell == RF)
    ));
    assert_eq!(h.failed(RF), vec![CastError::InvalidTarget]);
    assert!(
        h.unit(h.player).mana >= mana,
        "no mana spent on the failed completion"
    );
    assert_eq!(
        h.count(|e| matches!(e, Event::CastSucceeded { spell, .. } if *spell == RF)),
        0
    );
}

#[test]
fn one_hit_depletes_every_exhausted_absorb_aura() {
    let mut h = H::new(1, -2.0, |c| {
        c.auras.push(combat_core::AuraDef {
            key: "extra".into(),
            name: "Extra".into(),
            icon: "extra".into(),
            harmful: false,
        });
        if let Some(s) = c.spells.iter_mut().find(|s| s.id == GR) {
            s.effect = combat_core::Effect::Absorb {
                aura: "guardrails".into(),
                amount: 10,
                duration_ms: 10_000,
            };
        }
        if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
            ai.melee_damage = 60;
            ai.ability = None;
        }
    });
    // Two shields with different keys: a second Guardrails-like spell via id 6.
    let mut c = h.sim.content().clone();
    c.spells.push(combat_core::SpellDef {
        id: SpellId(6),
        effect: combat_core::Effect::Absorb {
            aura: "extra".into(),
            amount: 10,
            duration_ms: 10_000,
        },
        gcd: false,
        cooldown_ms: 0,
        mana: 0,
        ..c.spells[2].clone()
    });
    let mut sim = combat_core::Sim::new(c, 1);
    let player = sim.spawn_player(Vec3::default(), 0.0);
    let _mob = sim.spawn_unit("ai_evangelist", Vec3::new(0.0, 0.0, -2.0), 0.0);
    sim.push(Command::Cast {
        caster: player,
        spell: GR,
    });
    sim.push(Command::Cast {
        caster: player,
        spell: SpellId(6),
    });
    let mut events = Vec::new();
    for _ in 0..5 {
        events.extend(sim.tick());
    }
    let depleted = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::AuraRemoved {
                    reason: AuraEnd::Depleted,
                    ..
                }
            )
        })
        .count();
    assert_eq!(depleted, 2);
    h.run(1);
}

#[test]
fn death_of_a_casting_unit_emits_cast_interrupted() {
    let mut h = H::new(1, -2.0, |c| {
        if let Some(ai) = def_mut(c, "ai_evangelist").ai.as_mut() {
            ai.melee_damage = 1000;
            ai.ability = None;
        }
    });
    h.target(h.mob);
    h.cast(CR);
    h.run(5);
    assert_eq!(
        h.count(
            |e| matches!(e, Event::CastInterrupted { caster, by: None, .. } if *caster == h.player)
        ),
        1
    );
    assert!(h.unit(h.player).cast.is_none());
}
