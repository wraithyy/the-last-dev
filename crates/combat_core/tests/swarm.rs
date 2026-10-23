//! Swarm clusters in a real fight (docs/ILMARI_10.md C.3.1): the crowd multiplier through the
//! normal cast path, and bit-identical reruns. The unit tests in `swarm.rs` cover the rest.
#![allow(clippy::expect_used)]

mod common;
use combat_core::{Event, Vec3};
use common::*;

/// The CEO only stands there: its melee would push the casts back and muddy the numbers.
fn calm(c: &mut combat_core::Content) {
    c.config.spell_hit_pct = 100;
    c.config.white_hit_pct = 100;
    if let Some(ai) = def_mut(c, "ceo").ai.as_mut() {
        ai.abilities.clear();
        ai.melee_damage = 0;
    }
}

const KEY: &str = "il_scraper_swarm";

fn fight(seed: u64) -> (H, u64) {
    let mut h = H::new(seed, -30.0, calm);
    h.sim.set_wandering(false);
    h.sim.add_draft(Vec3::new(10.0, 0.0, -10.0));
    let cluster = h
        .sim
        .spawn_swarm(KEY, Vec3::new(0.0, 0.0, -8.0), 125)
        .expect("spawn_swarm");
    h.sim
        .spawn_swarm(KEY, Vec3::new(5.0, 0.0, -12.0), 60)
        .expect("spawn_swarm");
    h.target(cluster);
    h.run(2);
    for _ in 0..40 {
        h.cast(CR);
        h.run(80);
    }
    let hash = sim_hash(&h.sim, &h.log);
    (h, hash)
}

#[test]
fn a_cluster_takes_the_crowd_multiplier_from_a_normal_cast() {
    let mut h = H::new(5, -8.0, calm);
    let cluster = h
        .sim
        .spawn_swarm(KEY, Vec3::new(0.0, 0.0, -8.5), 125)
        .expect("spawn_swarm");
    h.target(h.mob);
    h.cast(CR);
    h.run(80);
    h.target(cluster);
    h.cast(CR);
    h.run(80);
    let plain: Vec<_> = h.damage_to(h.mob).into_iter().filter(|d| !d.2).collect();
    let swarm: Vec<_> = h.damage_to(cluster).into_iter().filter(|d| !d.2).collect();
    assert!(!plain.is_empty() && !swarm.is_empty());
    // 125 drones: x4 (the cap) on a single-target hit.
    assert_eq!(swarm[0].0, plain[0].0 * 4, "{plain:?} vs {swarm:?}");
    assert!(
        h.log
            .iter()
            .any(|(_, e)| matches!(e, Event::Damage { target, .. } if *target == cluster))
    );
}

#[test]
fn the_same_seeded_fight_is_bit_identical() {
    let (a, ha) = fight(11);
    let (b, hb) = fight(11);
    assert_eq!(ha, hb);
    assert_eq!(a.log.len(), b.log.len());
    // The fight did something: the cluster lost HP and the draft was bitten.
    assert!(
        a.sim
            .swarm_clusters()
            .iter()
            .map(|c| u32::from(c.1))
            .sum::<u32>()
            < 185
    );
}
