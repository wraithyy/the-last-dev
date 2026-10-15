//! Co-op group rules: per-player commands, threat across players, per-player loot, XP split,
//! group HP scaling, leaving mid-fight, routed events and group zone loads.

mod common;

use combat_core::{
    AiState, Audience, CastError, Command, EntityId, Event, GROUP_SHARE_RANGE_M, MAX_PLAYERS, Sim,
    SpellId, Vec3,
};
use common::*;

fn deaths(log: &[(u64, Event)]) -> Vec<EntityId> {
    log.iter()
        .filter_map(|(_, e)| match e {
            Event::Death { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect()
}

/// Ticks with routing; returns everything emitted.
fn run_routed(sim: &mut Sim, n: u32) -> Vec<(Audience, Event)> {
    (0..n).flat_map(|_| sim.tick_routed()).collect()
}

#[test]
fn spawn_player_caps_at_max_players_and_player_stays_the_first() {
    let mut h = H::new(1, -60.0, |_| {});
    for k in 1..MAX_PLAYERS {
        assert_ne!(
            h.sim
                .spawn_player("fullstack", Vec3::new(k as f32, 0.0, 0.0), 0.0),
            0
        );
    }
    assert_eq!(
        h.sim
            .spawn_player("fullstack", Vec3::new(9.0, 0.0, 0.0), 0.0),
        0
    );
    assert_eq!(h.sim.players().len(), MAX_PLAYERS);
    assert_eq!(h.sim.player(), Some(h.player));
}

#[test]
fn enemy_hp_scales_with_group_size_on_join_leave_and_spawn() {
    let mut h = H::new(1, -60.0, |_| {});
    h.sim.set_wandering(false);
    assert_eq!(h.unit(h.mob).max_hp, 900);
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, 0.0), 0.0);
    assert_eq!((h.unit(h.mob).max_hp, h.unit(h.mob).hp), (1395, 1395));
    let p3 = h
        .sim
        .spawn_player("fullstack", Vec3::new(2.0, 0.0, 0.0), 0.0);
    assert_eq!(h.unit(h.mob).max_hp, 1890);
    // Players themselves are not scaled.
    assert_eq!(h.unit(p2).max_hp, h.unit(h.player).max_hp);
    let late = h.sim.spawn_unit("ceo", Vec3::new(0.0, 0.0, -90.0), 0.0);
    assert_eq!(late.map(|id| h.unit(id).max_hp), Ok(1890));
    assert!(h.sim.remove_player(p3));
    assert_eq!(h.unit(h.mob).max_hp, 1395);

    let mut c = H::new(1, -60.0, |c| c.config.group_hp_pct_per_player = 100);
    c.sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, 0.0), 0.0);
    assert_eq!(c.unit(c.mob).max_hp, 1800);
}

#[test]
fn rescale_keeps_the_hp_fraction_of_a_wounded_enemy() {
    let mut h = H::new(1, -20.0, |_| {});
    h.sim.set_wandering(false);
    h.target(h.mob);
    h.cast(CR);
    assert!(h.run_until(120, |e| matches!(e, Event::Damage { .. })));
    let before = h.unit(h.mob).clone();
    h.sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, 0.0), 0.0);
    let after = h.unit(h.mob);
    assert_eq!(after.max_hp, before.max_hp * 155 / 100);
    assert_eq!(after.hp, before.hp * after.max_hp / before.max_hp);
}

#[test]
fn mobs_follow_top_threat_across_players_and_reset_on_wipe() {
    let mut h = H::new(3, -20.0, |c| {
        let ceo = def_mut(c, "ceo");
        if let Some(ai) = ceo.ai.as_mut() {
            ai.melee_damage = 10_000;
            ai.abilities.clear();
        }
    });
    h.sim.set_wandering(false);
    let p1 = h.player;
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, -5.0), 0.0);
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 4.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    h.target(h.mob);
    h.sim.push(Command::StartAutoAttack { unit: p1 });
    h.sim.apply_as(
        p2,
        Command::SetTarget {
            unit: p2,
            target: Some(h.mob),
        },
    );
    h.sim.apply_as(
        p2,
        Command::Cast {
            caster: p2,
            spell: CR,
        },
    );
    let mob = h.mob;
    assert!(h.run_until(120, |e| matches!(
        e,
        Event::Damage { source, target, spell: Some(_), .. } if *source == p2 && *target == mob
    )));
    h.tick();
    assert_eq!(h.unit(mob).target, Some(p2), "the bigger hit takes aggro");
    assert!(h.run_until(
        600,
        |e| matches!(e, Event::Death { unit, .. } if *unit == p1)
    ));
    assert_eq!(
        deaths(&h.log),
        vec![p2, p1],
        "top threat dies first, then the next"
    );
    // Everyone is dead: a wipe, so the mob evades and heals back home.
    assert!(h.run_until(30, |e| matches!(
        e,
        Event::AiState { unit, state: AiState::Evade } if *unit == mob
    )));
    assert!(h.run_until(900, |e| matches!(
        e,
        Event::AiState { unit, state: AiState::Idle } if *unit == mob
    )));
    assert_eq!(h.unit(mob).hp, h.unit(mob).max_hp);
    assert!(!h.unit(p1).in_combat && !h.unit(p2).in_combat);
    // Respawn is per player, at that player's own respawn point.
    h.sim.apply_as(p2, Command::RespawnPlayer);
    h.tick();
    assert!(h.unit(p2).alive && !h.unit(p1).alive);
    assert_eq!(h.unit(p2).pos, Vec3::new(1.0, 0.0, -5.0));
}

#[test]
fn second_player_aggro_alone_and_leaving_mid_fight_resets_the_mob() {
    let mut h = H::new(4, -60.0, |_| {});
    h.sim.set_wandering(false);
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(0.0, 0.0, -50.0), 0.0);
    let mob = h.mob;
    // Only p2 walks into aggro range; the mob picks p2 though p1 joined first.
    assert!(h.run_until(
        30,
        |e| matches!(e, Event::EnterCombat { unit } if *unit == mob)
    ));
    h.tick();
    assert_eq!(h.unit(mob).target, Some(p2));
    assert!(h.unit(p2).in_combat && !h.unit(h.player).in_combat);
    assert!(h.sim.remove_player(p2));
    assert!(!h.sim.remove_player(p2));
    assert!(h.sim.unit(p2).is_none());
    assert_eq!(h.sim.players(), [h.player]);
    assert_eq!(h.unit(mob).max_hp, 900);
    assert!(h.run_until(5, |e| matches!(
        e,
        Event::AiState { unit, state: AiState::Evade } if *unit == mob
    )));
    assert!(
        h.log
            .iter()
            .any(|(_, e)| *e == Event::Despawned { unit: p2 })
    );
}

#[test]
fn every_member_rolls_own_loot_and_loots_only_their_share() {
    let mut h = H::new(5, -3.0, |c| {
        // Trophy rolls only: gear and junk have their own tests (economy.rs).
        c.economy = Default::default();
        for l in &mut def_mut(c, "ceo").loot {
            l.chance_pct = 100;
        }
    });
    let (p1, mob) = (h.player, h.mob);
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, 0.0), 0.0);
    let p3 = h
        .sim
        .spawn_player("fullstack", Vec3::new(-1.0, 0.0, 0.0), 0.0);
    assert!(h.sim.debug_kill(mob));
    let ev = h.sim.tick_routed();
    let drops: Vec<(Audience, usize)> = ev
        .iter()
        .filter_map(|(a, e)| match e {
            Event::LootDropped { items, .. } => Some((*a, items.len())),
            _ => None,
        })
        .collect();
    let n = def_mut(&mut content(), "ceo").loot.len();
    assert!(n > 0);
    assert_eq!(
        drops,
        vec![
            (Audience::Player(p1), n),
            (Audience::Player(p2), n),
            (Audience::Player(p3), n)
        ]
    );
    // p2 claims to loot as p1; the sim rebinds the looter to the sender.
    h.sim.apply_as(
        p2,
        Command::Loot {
            looter: p1,
            corpse: mob,
        },
    );
    let ev = h.sim.tick_routed();
    assert!(ev.iter().any(|(a, e)| *a == Audience::Player(p2)
        && matches!(e, Event::Looted { looter, .. } if *looter == p2)));
    assert_eq!(h.unit(p2).inventory.len(), n);
    assert!(h.unit(p1).inventory.is_empty());
    assert!(h.unit(mob).lootable, "p1 and p3 still have their rolls");
    // A leaver's share is dropped; the last share empties the corpse.
    assert!(h.sim.remove_player(p3));
    h.sim.apply_as(
        p1,
        Command::Loot {
            looter: p1,
            corpse: mob,
        },
    );
    h.tick();
    assert_eq!(h.unit(p1).inventory.len(), n);
    assert!(!h.unit(mob).lootable);
}

#[test]
fn kill_xp_is_split_with_group_bonus_among_members_in_range() {
    let xp_of = |log: &[(Audience, Event)], p: EntityId| -> Vec<u32> {
        log.iter()
            .filter_map(|(a, e)| match e {
                Event::XpGained { unit, amount } if *unit == p => {
                    assert_eq!(*a, Audience::Player(p));
                    Some(*amount)
                }
                _ => None,
            })
            .collect()
    };
    // Solo: the full 120 (legacy CEO, same level).
    let mut h = H::new(6, -30.0, |_| {});
    assert!(h.sim.debug_kill(h.mob));
    assert_eq!(xp_of(&h.sim.tick_routed(), h.player), vec![120]);

    // Three in range: 120 * 116% / 3 = 46 each.
    let mut h = H::new(6, -30.0, |_| {});
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, 0.0), 0.0);
    let p3 = h
        .sim
        .spawn_player("fullstack", Vec3::new(2.0, 0.0, 0.0), 0.0);
    assert!(h.sim.debug_kill(h.mob));
    let ev = h.sim.tick_routed();
    for p in [h.player, p2, p3] {
        assert_eq!(xp_of(&ev, p), vec![46]);
    }

    // One member out of range: two share 120 * 100% / 2 = 60, the third gets nothing.
    let mut h = H::new(6, -30.0, |_| {});
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, 0.0), 0.0);
    let far = h.sim.spawn_player(
        "fullstack",
        Vec3::new(0.0, 0.0, GROUP_SHARE_RANGE_M + 70.0),
        0.0,
    );
    assert!(h.sim.debug_kill(h.mob));
    let ev = h.sim.tick_routed();
    assert_eq!(xp_of(&ev, h.player), vec![60]);
    assert_eq!(xp_of(&ev, p2), vec![60]);
    assert!(xp_of(&ev, far).is_empty());
}

#[test]
fn commands_act_per_player_and_failures_route_to_the_caster() {
    let mut h = H::new(7, -60.0, |_| {});
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, 0.0), 0.0);
    h.sim.apply_as(
        p2,
        Command::PlayerMoved {
            pos: Vec3::new(5.0, 0.0, 5.0),
            yaw: 1.0,
            moving: false,
            jumping: false,
        },
    );
    // No target: CastFailed for p2 only. A non-player id is ignored entirely.
    h.sim.apply_as(
        p2,
        Command::Cast {
            caster: h.player,
            spell: CR,
        },
    );
    h.sim.apply_as(h.mob, Command::RespawnPlayer);
    let ev = run_routed(&mut h.sim, 1);
    assert_eq!(h.unit(p2).pos, Vec3::new(5.0, 0.0, 5.0));
    assert_eq!(h.unit(h.player).pos, Vec3::new(0.0, 0.0, 0.0));
    assert!(ev.iter().any(|(a, e)| *a == Audience::Player(p2)
        && matches!(e, Event::CastFailed { caster, .. } if *caster == p2)));
    assert!(
        !ev.iter()
            .any(|(_, e)| matches!(e, Event::CastFailed { caster, .. } if *caster == h.player))
    );
    // Shared events stay broadcast.
    h.target(h.mob);
    h.sim.push(Command::StartAutoAttack { unit: h.player });
    let ev = run_routed(&mut h.sim, 1);
    assert!(
        ev.iter()
            .any(|(a, e)| *a == Audience::All && matches!(e, Event::AutoAttackToggled { .. }))
    );
}

#[test]
fn zone_load_moves_the_whole_group_to_the_spawn_anchor() {
    let spawn = Vec3::new(10.0, 0.0, 10.0);
    let anchor = move |name: &str| -> Option<(Vec3, f32)> {
        let num = |p: &str| name.strip_prefix(p).and_then(|n| n.parse::<f32>().ok());
        if let Some(n) = num("pack_") {
            return Some((Vec3::new(0.0, 0.0, -70.0 * n), 0.0));
        }
        if let Some(n) = num("npc_") {
            return Some((Vec3::new(40.0, 0.0, -15.0 * n), 0.0));
        }
        // The campus onboarding corner (named, not numbered), far out of every pull.
        if name.starts_with("npc_") {
            return Some((Vec3::new(-400.0, 0.0, 400.0), 0.0));
        }
        match name {
            "boss" => Some((Vec3::new(0.0, 0.0, -630.0), 0.0)),
            "player_spawn" => Some((spawn, 1.5)),
            _ => None,
        }
    };
    let mut h = H::new(8, -60.0, |_| {});
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(100.0, 0.0, 0.0), 0.0);
    assert!(h.sim.load_zone("tower/1", &anchor, Vec::new()).is_ok());
    let (a, b) = (h.unit(h.player).clone(), h.unit(p2).clone());
    assert_eq!(a.pos, spawn);
    assert!((b.pos.x - spawn.x).abs() + (b.pos.z - spawn.z).abs() > 1.0);
    assert!((b.pos.x - spawn.x).abs() < 4.0 && (b.pos.z - spawn.z).abs() < 4.0);
    assert_eq!((a.yaw, b.yaw), (1.5, 1.5));
    assert!(h.sim.unit(h.mob).is_none(), "old units despawn");
}

const ROLLBACK: SpellId = SpellId(7);
const REZ: SpellId = SpellId(13);

fn fails_of(ev: &[(Audience, Event)], who: EntityId) -> Vec<CastError> {
    ev.iter()
        .filter_map(|(_, e)| match e {
            Event::CastFailed { caster, reason, .. } if *caster == who => Some(*reason),
            _ => None,
        })
        .collect()
}

fn cast_as(sim: &mut Sim, who: EntityId, target: Option<EntityId>, spell: SpellId) {
    sim.apply_as(who, Command::SetTarget { unit: who, target });
    sim.apply_as(who, Command::Cast { caster: who, spell });
}

#[test]
fn helpful_spells_land_on_a_targeted_teammate_or_fall_back_to_self() {
    let mut h = H::new(1, -60.0, |_| {});
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(10.0, 0.0, 0.0), 0.0);
    assert!(h.sim.debug_apply_aura(p2, "code_smell", 10_000));
    run_routed(&mut h.sim, 1);
    cast_as(&mut h.sim, h.player, Some(p2), GR);
    let ev = run_routed(&mut h.sim, 2);
    assert!(ev.iter().any(|(_, e)| matches!(e,
        Event::CastSucceeded { spell, target, .. } if *spell == GR && *target == Some(p2))));
    assert!(h.unit(p2).auras.iter().any(|a| a.key == "guardrails"));
    assert!(!h.unit(h.player).auras.iter().any(|a| a.key == "guardrails"));
    // Rollback's dispel takes the teammate's debuff, not the caster's.
    run_routed(&mut h.sim, 60);
    cast_as(&mut h.sim, h.player, Some(p2), ROLLBACK);
    let ev = run_routed(&mut h.sim, 2);
    assert!(ev.iter().any(|(_, e)| matches!(e,
        Event::AuraRemoved { target, key, .. } if *target == p2 && key == "code_smell")));
    // An enemy (or no) target self-casts, exactly like single-player.
    let mut s = H::new(1, -60.0, |_| {});
    let p2 = s
        .sim
        .spawn_player("fullstack", Vec3::new(10.0, 0.0, 0.0), 0.0);
    cast_as(&mut s.sim, s.player, Some(s.mob), GR);
    run_routed(&mut s.sim, 2);
    assert!(s.unit(s.player).auras.iter().any(|a| a.key == "guardrails"));
    assert!(!s.unit(p2).auras.iter().any(|a| a.key == "guardrails"));
}

#[test]
fn ally_spells_check_range_line_of_sight_and_life() {
    let mut h = H::new(1, -60.0, |_| {});
    let far = h
        .sim
        .spawn_player("fullstack", Vec3::new(40.0, 0.0, 0.0), 0.0);
    let walled = h
        .sim
        .spawn_player("fullstack", Vec3::new(0.0, 0.0, 10.0), 0.0);
    h.sim.set_blockers(vec![combat_core::Aabb {
        min: Vec3::new(-3.0, 0.0, 4.0),
        max: Vec3::new(3.0, 3.0, 5.0),
    }]);
    cast_as(&mut h.sim, h.player, Some(far), GR);
    let ev = run_routed(&mut h.sim, 1);
    assert_eq!(fails_of(&ev, h.player), vec![CastError::OutOfRange]);
    cast_as(&mut h.sim, h.player, Some(walled), GR);
    let ev = run_routed(&mut h.sim, 1);
    assert_eq!(fails_of(&ev, h.player), vec![CastError::NoLineOfSight]);
    assert!(h.sim.debug_kill_player(far));
    h.sim.apply_as(
        far,
        Command::PlayerMoved {
            pos: Vec3::new(5.0, 0.0, 0.0),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    cast_as(&mut h.sim, h.player, Some(far), GR);
    let ev = run_routed(&mut h.sim, 1);
    assert_eq!(fails_of(&ev, h.player), vec![CastError::Dead]);
}

#[test]
fn resurrect_revives_a_dead_teammate_where_they_fell_out_of_combat_only() {
    let mut h = H::new(1, -20.0, |_| {});
    h.sim.set_wandering(false);
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(6.0, 0.0, 0.0), 0.0);
    // Living, self, enemy and nothing are all refused.
    for (t, want) in [
        (Some(p2), CastError::TargetNotDead),
        (Some(h.player), CastError::InvalidTarget),
        (Some(h.mob), CastError::InvalidTarget),
        (None, CastError::NoTarget),
    ] {
        cast_as(&mut h.sim, h.player, t, REZ);
        let ev = run_routed(&mut h.sim, 1);
        assert_eq!(fails_of(&ev, h.player), vec![want], "target {t:?}");
    }
    assert!(h.sim.debug_kill_player(p2));
    run_routed(&mut h.sim, 1);
    // In combat: no.
    h.target(h.mob);
    h.cast(CR);
    assert!(h.run_until(120, |e| matches!(e, Event::EnterCombat { .. })));
    cast_as(&mut h.sim, h.player, Some(p2), REZ);
    let ev = run_routed(&mut h.sim, 1);
    assert_eq!(fails_of(&ev, h.player), vec![CastError::InCombat]);
    // Out of combat it works after the full cast, with 30% HP and 20% mana, in place.
    let mut h = H::new(1, -60.0, |_| {});
    let p2 = h
        .sim
        .spawn_player("fullstack", Vec3::new(6.0, 0.0, 0.0), 0.0);
    assert!(h.sim.debug_kill_player(p2));
    run_routed(&mut h.sim, 1);
    cast_as(&mut h.sim, h.player, Some(p2), REZ);
    let ev = run_routed(&mut h.sim, 2);
    assert!(ev.iter().any(|(_, e)| matches!(e,
        Event::CastStarted { spell, total_ms: 6000, .. } if *spell == REZ)));
    let ev = run_routed(&mut h.sim, 180);
    let (max_hp, max_mana) = (h.unit(p2).max_hp, h.unit(p2).max_mana);
    assert!(ev.iter().any(|(a, e)| *a == Audience::All && matches!(e,
        Event::Resurrected { unit, by, hp, mana }
            if *unit == p2 && *by == h.player && *hp == max_hp * 30 / 100 && *mana == max_mana * 20 / 100)));
    let u = h.unit(p2);
    assert!(u.alive && u.hp == max_hp * 30 / 100);
    assert_eq!(u.pos, Vec3::new(6.0, 0.0, 0.0));
    // A dead caster or a teammate who released meanwhile fails at completion.
    assert!(h.sim.debug_kill_player(p2));
    run_routed(&mut h.sim, 1);
    cast_as(&mut h.sim, h.player, Some(p2), REZ);
    run_routed(&mut h.sim, 30);
    h.sim.apply_as(p2, Command::RespawnPlayer);
    let ev = run_routed(&mut h.sim, 180);
    assert_eq!(fails_of(&ev, h.player), vec![CastError::TargetNotDead]);
    assert!(
        !ev.iter()
            .any(|(_, e)| matches!(e, Event::Resurrected { .. }))
    );
}

#[test]
fn enemy_damage_scales_with_group_size() {
    let first_melee = |players: usize| {
        let mut h = H::new(1, -2.0, |c| {
            // The CEO is a boss: leave out the extra boss slice to see the plain group factor.
            c.config.group_boss_damage_pct_per_player = 0;
            if let Some(ai) = def_mut(c, "ceo").ai.as_mut() {
                ai.abilities.clear();
            }
        });
        h.sim.set_wandering(false);
        for k in 1..players {
            h.sim
                .spawn_player("fullstack", Vec3::new(50.0 + k as f32, 0.0, 0.0), 0.0);
        }
        h.target(h.mob);
        h.cast(CR);
        let ev = run_routed(&mut h.sim, 300);
        ev.into_iter()
            .find_map(|(_, e)| match e {
                Event::Damage {
                    source,
                    spell: None,
                    amount,
                    absorbed,
                    ..
                } if source != h.player => Some(amount + absorbed),
                _ => None,
            })
            .unwrap_or(0)
    };
    let solo = first_melee(1);
    assert!(solo > 0);
    assert_eq!(first_melee(2), solo * 130 / 100);
    assert_eq!(first_melee(3), solo * 160 / 100);
}
