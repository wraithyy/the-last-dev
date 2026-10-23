//! Per-player illusions (docs/ILMARI_10.md C.3.2): the pure layout, its constraints, and the
//! proof that it is cosmetic (a stubbed layout leaves the sim hash untouched).
#![allow(clippy::expect_used)]

mod common;

use combat_core::companion::{DALE, RUPERT};
use combat_core::illusion::{
    CIRCLES, GRID, IllusionArena, IllusionLayout, TILES, layout, real_set,
};
use combat_core::{Command, EntityId, Event, Sim, Vec3};
use common::*;

const ARENA: IllusionArena = IllusionArena {
    x_cm: 0,
    z_cm: 0,
    tile_cm: 800,
};

/// FNV-1a over a Debug string: a stable digest that does not depend on std's hasher.
fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

#[test]
fn layout_is_deterministic_and_frozen() {
    let a = layout(0xDEAD_BEEF, 3, 7, &ARENA);
    assert_eq!(a, layout(0xDEAD_BEEF, 3, 7, &ARENA));
    // Frozen: native and web must agree, and a refactor must not move it silently.
    assert_eq!(
        format!("{a:?}"),
        "IllusionLayout { fakes: [Circle { x_cm: -1198, z_cm: -753, r_cm: 272 }, Circle { x_cm: 625, z_cm: -50, r_cm: 288 }, Circle { x_cm: -407, z_cm: 527, r_cm: 320 }], believed_safe: 217 }"
    );
    assert_eq!(fnv(&format!("{a:?}")), 2_493_068_321_208_510_550);
}

#[test]
fn no_two_players_share_a_layout_over_twenty_cycles() {
    for cycle in 1..=20u32 {
        let salt = u64::from(cycle) * 0x9E37_79B9;
        let all: Vec<IllusionLayout> = (1..=5).map(|p| layout(salt, p, cycle, &ARENA)).collect();
        for i in 0..all.len() {
            for j in i + 1..all.len() {
                assert_ne!(all[i], all[j], "cycle {cycle}: players {i} and {j}");
            }
        }
    }
}

#[test]
fn a_players_layout_does_not_depend_on_who_else_is_in_the_room() {
    // `layout` takes no group argument at all; this pins that no hidden state sneaks in.
    let alone = layout(99, 2, 4, &ARENA);
    let _crowd: Vec<_> = (1..=5).map(|p| layout(99, p, 4, &ARENA)).collect();
    assert_eq!(alone, layout(99, 2, 4, &ARENA));
}

#[test]
fn the_constraints_hold_for_a_thousand_salts() {
    let mut cases = 0;
    for salt in 0..1000u64 {
        let salt = salt.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ salt;
        let real = real_set(salt, &ARENA);
        let safe: Vec<usize> = (0..TILES).filter(|k| real.overflow >> k & 1 == 0).collect();
        assert!((3..=5).contains(&real.overflow.count_ones()));
        // The truth has a way through: a safe tile no real circle covers.
        let free = |k: usize, circles: &[combat_core::illusion::Circle]| {
            let (x, z) = ARENA.tile_center(k);
            !circles
                .iter()
                .any(|c| (x - c.x_cm).pow(2) + (z - c.z_cm).pow(2) <= c.r_cm.pow(2))
        };
        assert!(safe.iter().any(|&k| free(k, &real.circles)), "salt {salt}");
        for player in [1, 2, 7] {
            let l = layout(salt, player, 3, &ARENA);
            cases += 1;
            // Same count as the reals.
            assert_eq!(l.fakes.len(), CIRCLES);
            // Never coincident with a real one.
            for f in &l.fakes {
                for r in &real.circles {
                    assert!(f != r, "salt {salt}");
                    assert!((f.x_cm, f.z_cm) != (r.x_cm, r.z_cm), "salt {salt}");
                }
            }
            // Solvable in the player's own view and in the truth.
            let all: Vec<_> = real.circles.iter().chain(l.fakes.iter()).copied().collect();
            assert!(
                safe.iter().any(|&k| free(k, &all)),
                "salt {salt} player {player}"
            );
            // Believed-safe tiles: at least one really safe, at least one really not.
            let right = (l.believed_safe & !real.overflow).count_ones();
            let wrong = (l.believed_safe & real.overflow).count_ones();
            assert!(
                right >= 1 && wrong >= 1,
                "salt {salt}: {right} right, {wrong} wrong"
            );
            assert!(l.believed_safe < 1 << TILES);
        }
    }
    assert_eq!(cases, 3000);
    assert_eq!(GRID * GRID, TILES);
}

fn stub(_: u64, _: EntityId, _: u32, _: &IllusionArena) -> IllusionLayout {
    IllusionLayout {
        fakes: real_set(0, &ARENA).circles,
        believed_safe: 0,
    }
}

/// A calm fight (a CEO that stands there) with a second player, cycles every four seconds, and a
/// "client" that asks for each player's illusion every tick with the given layout function.
fn run_with(f: combat_core::illusion::LayoutFn, seed: u64) -> (u64, Vec<u64>) {
    let mut h = H::new(seed, -30.0, |c| {
        if let Some(ai) = def_mut(c, "ceo").ai.as_mut() {
            ai.abilities.clear();
            ai.melee_damage = 0;
        }
    });
    let second = h.sim.spawn_player("backend", Vec3::new(2.0, 0.0, 0.0), 0.0);
    let (a, b) = (h.player, second);
    let mut salts = Vec::new();
    for t in 0..1800u32 {
        if t % 120 == 0 {
            h.sim.illusion_begin(ARENA, h.mob, 25, 2000);
        }
        h.tick();
        for p in [a, b] {
            // The client reading; it must not be able to change anything.
            let _ = h.sim.illusion_for(p, f);
        }
        if let Some((_, salt, _)) = h.sim.illusion_cycle()
            && salts.last() != Some(&salt)
        {
            salts.push(salt);
        }
    }
    (sim_hash(&h.sim, &h.log), salts)
}

#[test]
fn the_illusion_layer_is_cosmetic_a_stubbed_layout_gives_the_same_sim_hash() {
    let (real, salts) = run_with(layout, 21);
    let (stubbed, salts2) = run_with(stub, 21);
    assert_eq!(real, stubbed);
    assert_eq!(salts, salts2);
    assert_eq!(salts.len(), 15);
    // And the run itself is a pure function of the seed.
    assert_eq!(real, run_with(layout, 21).0);
}

#[test]
fn one_draw_per_cycle_whatever_the_group_size() {
    let salts = |players: usize| {
        let mut sim = Sim::new(content(), 5);
        for k in 0..players {
            sim.spawn_player("fullstack", Vec3::new(k as f32, 0.0, 0.0), 0.0);
        }
        (0..6)
            .map(|_| {
                sim.illusion_begin(ARENA, 0, 0, 1000);
                sim.illusion_cycle().expect("cycle").1
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(salts(1), salts(4));
}

#[test]
fn only_the_real_set_hurts() {
    let mut h = H::new(2, -30.0, |c| {
        if let Some(ai) = def_mut(c, "ceo").ai.as_mut() {
            ai.abilities.clear();
            ai.melee_damage = 0;
        }
    });
    let cycle = h.sim.illusion_begin(ARENA, h.mob, 500, 1000);
    assert_eq!(cycle, 1);
    let real = h.sim.illusion_real().expect("real");
    let at = |k: usize| {
        let (x, z) = ARENA.tile_center(k);
        // A tile corner away from the circles' usual centres is not needed: pick the exact
        // centre of a tile and keep only tiles no real circle covers.
        Vec3::new(x as f32 / 100.0, 0.0, z as f32 / 100.0)
    };
    let free_of_circles = |k: usize| {
        let (x, z) = ARENA.tile_center(k);
        !real
            .circles
            .iter()
            .any(|c| (x - c.x_cm).pow(2) + (z - c.z_cm).pow(2) <= c.r_cm.pow(2))
    };
    let safe = (0..TILES)
        .find(|&k| real.overflow >> k & 1 == 0 && free_of_circles(k))
        .expect("safe");
    let bad = (0..TILES)
        .find(|&k| real.overflow >> k & 1 == 1)
        .expect("overflow");
    let second = h.sim.spawn_player("backend", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let place = |h: &mut H, p: EntityId, pos: Vec3| {
        h.sim.apply_as(
            p,
            Command::PlayerMoved {
                pos,
                yaw: 0.0,
                moving: false,
                jumping: false,
            },
        );
    };
    let first = h.player;
    place(&mut h, first, at(safe));
    place(&mut h, second, at(bad));
    h.run(40);
    let hp = |h: &H, p| (h.unit(p).hp, h.unit(p).max_hp);
    assert_eq!(hp(&h, h.player).0, hp(&h, h.player).1, "safe tile");
    assert!(hp(&h, second).0 < hp(&h, second).1, "overflow tile");
    assert!(
        h.events()
            .any(|(_, e)| matches!(e, Event::IllusionCycle { cycle: 1, .. }))
    );
}

#[test]
fn companions_are_never_shown_illusions_and_stand_in_the_truth() {
    let mut c = content();
    def_mut(&mut c, "senior_engineer").max_hp = 10_000_000;
    let mut sim = Sim::new(c, 9);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(p, 10);
    sim.debug_force_raid_async(Some(true));
    sim.set_companion_picks(p, &[DALE.to_string(), RUPERT.to_string()]);
    sim.load_zone(
        "ilmari1/1",
        &|_| Some((Vec3::new(0.0, 0.0, 0.0), 0.0)),
        Vec::new(),
    )
    .expect("zone");
    let comps = sim.companions_of(p);
    assert_eq!(comps.len(), 2);
    sim.illusion_begin(ARENA, 0, 0, 1000);
    assert!(sim.illusion_for(p, layout).is_some());
    for c in comps {
        assert!(sim.illusion_for(c, layout).is_none());
    }
}
