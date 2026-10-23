//! Ilmari Wing 4, The Cloud (docs/ILMARI_10.md C.3.4, C.3.5, C.5): the Autoscaler's split window,
//! replicas, decommission and cloud bill, the Region Outage's platform sequence, `Failover` and
//! its tolerances, plus the wing's door, locks, titles and drops. The balance gates (Story solo,
//! Full trio) live in tests/playthrough.rs.
#![allow(clippy::expect_used)]

mod common;

use combat_core::companion::{DALE, PRIYA};
use combat_core::{
    Command, Content, EntityId, Event, GearSource, PROP_OK, PlatformKind, PlatformPhase, Sim,
    SpellId, Unlock, Vec3,
};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;
/// Where each synthetic zone puts the boss (and so the hub of the platforms): 30 m north.
const BOSS: (f32, f32) = (0.0, -30.0);

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(BOSS.0, BOSS.1),
        n if n.starts_with("prop:il_rack_") => at(5.0, BOSS.1 + 10.0),
        n if n.starts_with("prop:") => at(1.0, 0.0),
        _ => at(0.0, 0.0),
    }
}

fn sturdy(c: &mut Content) {
    for k in [
        "senior_engineer",
        "backend_dev",
        "devops_eng",
        "frontend_dev",
        "data_scientist",
    ] {
        def_mut(c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
}

fn wing(n: usize, story: bool, seed: u64, tough: bool) -> (Sim, Vec<EntityId>) {
    let classes = ["backend", "devops", "fullstack", "frontend"];
    let mut c = content();
    if tough {
        sturdy(&mut c);
    }
    let mut sim = Sim::new(c, seed);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids: Vec<EntityId> = (0..n)
        .map(|k| {
            let p = sim.spawn_player(
                classes[k % classes.len()],
                Vec3::new(k as f32, 0.0, 0.0),
                0.0,
            );
            sim.set_level_of(p, 10);
            p
        })
        .collect();
    sim.debug_force_raid_async(Some(story));
    (sim, ids)
}

fn load(sim: &mut Sim, zone: &str) {
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{zone}: {e}");
    }
}

fn unit_of(sim: &Sim, key: &str) -> Option<EntityId> {
    sim.units().find(|u| u.key == key && u.alive).map(|u| u.id)
}

fn move_to(sim: &mut Sim, p: EntityId, pos: Vec3) {
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos,
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
}

fn pull(sim: &mut Sim, ids: &[EntityId], t: EntityId, at: Vec3) {
    for &p in ids {
        move_to(sim, p, at);
    }
    sim.apply_as(
        ids[0],
        Command::SetTarget {
            unit: ids[0],
            target: Some(t),
        },
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: SpellId(1),
        },
    );
}

fn run(sim: &mut Sim, secs: u32) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..secs * SECOND {
        out.extend(sim.tick());
    }
    out
}

fn auras(sim: &Sim, id: EntityId) -> Vec<(String, u8)> {
    sim.unit(id)
        .map(|u| u.auras.iter().map(|a| (a.key.clone(), a.stacks)).collect())
        .unwrap_or_default()
}

// ------------------------------------------------------------------ the Autoscaler

/// The Autoscaler pulled by `n` players. The players are very sturdy and cannot be hurt; the boss
/// is topped up between steps by the tests that need it.
fn autoscaler(story: bool, n: usize, seed: u64) -> (Sim, Vec<EntityId>, EntityId) {
    let (mut sim, ids) = wing(n, story, seed, true);
    load(&mut sim, "ilmari4/2");
    let boss = unit_of(&sim, "il_autoscaler").expect("boss");
    pull(&mut sim, &ids, boss, Vec3::new(0.0, 0.0, BOSS.1 + 12.0));
    (sim, ids, boss)
}

fn family(sim: &Sim) -> Vec<(EntityId, u32)> {
    sim.units()
        .filter(|u| u.key == "il_autoscaler" && u.alive)
        .map(|u| (u.id, u.hp))
        .collect()
}

fn max_hp(sim: &Sim, id: EntityId) -> u32 {
    sim.unit(id).expect("unit").max_hp
}

#[test]
fn a_burst_over_the_threshold_splits_and_a_steady_hand_does_not() {
    for (story, n, pct, copies) in [(false, 3, 8_u32, 4_usize), (true, 1, 14, 2)] {
        let (mut sim, ids, boss) = autoscaler(story, n, 1);
        run(&mut sim, 1);
        let max = max_hp(&sim, boss);
        let under = u64::from(max) * u64::from(pct) / 100 - 5;
        // Steady: just below the threshold, again after the window has rolled over: no split.
        for _ in 0..3 {
            sim.debug_hit(ids[0], boss, under as u32);
            run(&mut sim, 6);
            assert_eq!(family(&sim).len(), 1, "story {story}: below the threshold");
        }
        // A burst of the same total in a moment is over it.
        sim.debug_hit(ids[0], boss, under as u32);
        run(&mut sim, 1);
        sim.debug_hit(ids[0], boss, 10);
        run(&mut sim, 1);
        assert_eq!(family(&sim).len(), 2, "story {story}: one replica");
        // Spikes keep coming: the cooldown and then the cap hold.
        for _ in 0..8 {
            let f = family(&sim);
            // The parent is topped up each round, so only the copy cap can stop a split.
            sim.debug_set_hp(f[0].0, max);
            sim.debug_hit(
                ids[0],
                f[0].0,
                (u64::from(max) * u64::from(pct) / 100 + 10) as u32,
            );
            run(&mut sim, 7);
        }
        assert_eq!(
            family(&sim).len(),
            copies,
            "story {story}: at most {copies} copies"
        );
    }
}

#[test]
fn a_split_conserves_the_pool_and_a_cooldown_spaces_the_splits() {
    let (mut sim, ids, boss) = autoscaler(false, 3, 2);
    run(&mut sim, 1);
    let max = max_hp(&sim, boss);
    let before = sim.unit(boss).expect("boss").hp;
    let hit = max / 10;
    sim.debug_hit(ids[0], boss, hit);
    run(&mut sim, 1);
    let f = family(&sim);
    assert_eq!(f.len(), 2);
    let total: u32 = f.iter().map(|x| x.1).sum();
    // The boss may have regenerated or been hit by nothing else: within a couple of points.
    assert!(
        total + hit >= before - 3 && total + hit <= before + 3,
        "pool {total} + hit {hit} against {before}"
    );
    let (parent, child) = (f[0].1, f[1].1);
    assert!(
        child * 100 / (parent + child) >= 39 && child * 100 / (parent + child) <= 41,
        "the child takes 40%: {child} of {}",
        parent + child
    );
    // A second spike inside the 6 s cooldown does not split.
    sim.debug_hit(ids[0], f[0].0, hit);
    run(&mut sim, 2);
    assert_eq!(family(&sim).len(), 2, "cooldown");
    run(&mut sim, 5);
    sim.debug_hit(ids[0], family(&sim)[0].0, hit);
    run(&mut sim, 1);
    assert_eq!(family(&sim).len(), 3, "the cooldown is over");
}

#[test]
fn only_players_feed_the_window() {
    let (mut sim, _ids, boss) = autoscaler(false, 3, 3);
    run(&mut sim, 1);
    let max = max_hp(&sim, boss);
    // The boss hitting itself is not a player: nothing splits however hard.
    sim.debug_hit(boss, boss, max / 5);
    run(&mut sim, 2);
    assert_eq!(family(&sim).len(), 1);
}

#[test]
fn replicas_share_threat_and_follow_the_ability_trim() {
    let (mut sim, ids, boss) = autoscaler(false, 3, 4);
    run(&mut sim, 1);
    let max = max_hp(&sim, boss);
    sim.debug_hit(ids[0], boss, max / 8);
    run(&mut sim, 2);
    let f = family(&sim);
    assert_eq!(f.len(), 2);
    let (a, b) = (f[0].0, f[1].0);
    for id in [a, b] {
        let u = sim.unit(id).expect("unit");
        assert!(u.in_combat, "{id} fights");
        assert!(u.target.is_some());
    }
    // Keep everything alive and watch 70 s: only the original provisions golems.
    let mut summoners = Vec::new();
    for _ in 0..70 {
        for e in run(&mut sim, 1) {
            if let Event::Summoned { summoner, .. } = e {
                summoners.push(summoner);
            }
        }
        for (id, _) in family(&sim) {
            sim.debug_set_hp(id, 10_000);
        }
    }
    assert!(!summoners.is_empty(), "the original provisions");
    assert!(
        summoners.iter().all(|s| *s == boss),
        "replicas never provision: {summoners:?} from {boss}"
    );
}

#[test]
fn a_decommissioned_replica_pays_nothing_and_the_last_one_pays_everything() {
    let (mut sim, ids, boss) = autoscaler(false, 3, 5);
    run(&mut sim, 1);
    let max = max_hp(&sim, boss);
    sim.debug_hit(ids[0], boss, max / 8);
    run(&mut sim, 2);
    let f = family(&sim);
    assert_eq!(f.len(), 2);
    // Take the bill up so the relief shows.
    assert!(sim.debug_set_cloud_bill(300));
    let events = {
        assert!(sim.debug_kill(f[1].0));
        run(&mut sim, 1)
    };
    assert_eq!(family(&sim).len(), 1);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, Event::LootDropped { .. })),
        "no loot from a replica"
    );
    assert!(sim.cloud_bill().is_some_and(|b| (180..=185).contains(&b)));
    assert!(
        !sim.raid_locked("il_autoscaler", true) && !sim.raid_locked("il_autoscaler", false),
        "no lockout yet"
    );
    // The last one is the boss: loot, lockout, zone clear.
    let events = {
        assert!(sim.debug_kill(f[0].0));
        run(&mut sim, 2)
    };
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::LootDropped { .. })),
        "the last one pays"
    );
    assert!(sim.raid_locked("il_autoscaler", false) || sim.raid_locked("il_autoscaler", true));
}

#[test]
fn the_original_can_go_first_and_a_replica_carries_the_fight() {
    let (mut sim, ids, boss) = autoscaler(false, 3, 6);
    run(&mut sim, 1);
    let max = max_hp(&sim, boss);
    sim.debug_hit(ids[0], boss, max / 8);
    run(&mut sim, 2);
    assert!(sim.debug_kill(boss), "the original is decommissioned");
    run(&mut sim, 1);
    let left = family(&sim);
    assert_eq!(left.len(), 1);
    assert!(sim.cloud_bill().is_some(), "the bill carries on");
    let events = {
        assert!(sim.debug_kill(left[0].0));
        run(&mut sim, 2)
    };
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::LootDropped { .. })),
        "the replica was a floor boss too"
    );
    assert!(sim.raid_locked("il_autoscaler", false) || sim.raid_locked("il_autoscaler", true));
}

#[test]
fn a_wipe_resets_the_family_and_the_bill() {
    let (mut sim, ids, boss) = autoscaler(false, 3, 7);
    run(&mut sim, 1);
    let max = max_hp(&sim, boss);
    sim.debug_hit(ids[0], boss, max / 8);
    run(&mut sim, 2);
    assert_eq!(family(&sim).len(), 2);
    for &p in &ids {
        sim.debug_kill_player(p);
    }
    run(&mut sim, 40);
    let f = family(&sim);
    assert_eq!(f.len(), 1, "one unit walks home: {f:?}");
    assert_eq!(f[0].1, max_hp(&sim, f[0].0), "whole again");
    assert!(sim.cloud_bill().is_none(), "the meter is off");
}

fn bill_after(copies: usize, story: bool, secs: u32) -> u32 {
    let (mut sim, ids, boss) = autoscaler(story, 3, 8);
    run(&mut sim, 1);
    let max = max_hp(&sim, boss);
    for k in 1..copies {
        let f = family(&sim);
        sim.debug_set_hp(f[0].0, max);
        sim.debug_hit(ids[0], f[0].0, max / 5);
        run(&mut sim, 7);
        assert_eq!(family(&sim).len(), k + 1);
    }
    // Keep the family alive and measure the climb from a clean meter.
    for (id, _) in family(&sim) {
        sim.debug_set_hp(id, max / 2);
    }
    assert!(sim.debug_set_cloud_bill(0));
    run(&mut sim, secs);
    sim.cloud_bill().expect("armed")
}

#[test]
fn the_bill_climbs_by_the_rate_table() {
    // 2 / 5.7 / 10.4 / 16 points a second by replica count on Full (a point of slack for the
    // seconds the replicas took to appear), half of that on Story.
    for (copies, rate_x10) in [(1, 20), (2, 57), (3, 104), (4, 160)] {
        let got = bill_after(copies, false, 20);
        let want = rate_x10 * 20 / 10;
        assert!(
            got.abs_diff(want) <= 3,
            "{copies} copies: {got} points against {want}"
        );
    }
    let story = bill_after(2, true, 20);
    assert!(story.abs_diff(57 * 20 / 20) <= 2, "Story is half: {story}");
}

#[test]
fn the_bill_steps_add_overage_and_the_budget_enables_the_invoice() {
    let (mut sim, ids, boss) = autoscaler(false, 3, 9);
    run(&mut sim, 1);
    let overage = |sim: &Sim, id| {
        auras(sim, id)
            .iter()
            .find(|a| a.0 == "il_overage")
            .map_or(0, |a| a.1)
    };
    assert_eq!(overage(&sim, boss), 0);
    for (points, stacks) in [(260, 1), (510, 2), (760, 3)] {
        assert!(sim.debug_set_cloud_bill(points));
        run(&mut sim, 1);
        assert_eq!(overage(&sim, boss), stacks, "at {points}");
    }
    assert!(
        !auras(&sim, boss).iter().any(|a| a.0 == "il_over_budget"),
        "not yet over budget"
    );
    assert!(sim.debug_set_cloud_bill(1000));
    let events = run(&mut sim, 25);
    assert!(auras(&sim, boss).iter().any(|a| a.0 == "il_over_budget"));
    let invoices = events
        .iter()
        .filter(|e| matches!(e, Event::Damage { spell: Some(s), .. } if s.0 == 2566))
        .count();
    assert!(invoices >= 3, "the Invoice lands every 10 s: {invoices}");
    // The meter never passes 1000, and a relief takes the stage away again.
    assert!(sim.cloud_bill().is_some_and(|b| b <= 1000));
    assert!(sim.debug_set_cloud_bill(700));
    run(&mut sim, 1);
    assert!(!auras(&sim, boss).iter().any(|a| a.0 == "il_over_budget"));
    assert_eq!(overage(&sim, boss), 2);
    let _ = ids;
}

#[test]
fn story_overage_is_gentler_and_has_no_invoice() {
    let (mut sim, _, boss) = autoscaler(true, 1, 10);
    run(&mut sim, 1);
    assert!(sim.debug_set_cloud_bill(1000));
    let events = run(&mut sim, 25);
    let a = auras(&sim, boss);
    assert!(
        a.iter().any(|x| x.0 == "il_overage_story" && x.1 == 3),
        "{a:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Damage { spell: Some(s), .. } if s.0 == 2566)),
        "Budget Exceeded is skipped on Story"
    );
}

#[test]
fn a_reserved_instance_rack_halves_the_rate_for_thirty_seconds() {
    let (mut sim, ids, _) = autoscaler(false, 3, 11);
    run(&mut sim, 1);
    assert!(sim.debug_set_cloud_bill(0));
    let spot = sim.prop_spot("il_rack_1").expect("rack");
    move_to(&mut sim, ids[0], spot);
    sim.apply_as(
        ids[0],
        Command::Interact {
            prop: "il_rack_1".into(),
        },
    );
    let events = sim.tick();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Interacted { prop, outcome, .. } if prop == "il_rack_1" && *outcome == PROP_OK
    )));
    run(&mut sim, 20);
    let reserved = sim.cloud_bill().expect("armed");
    assert!(
        reserved.abs_diff(20) <= 2,
        "half of 2/s for 20 s: {reserved}"
    );
    // Past the 30 s the full rate is back.
    run(&mut sim, 20);
    let after = sim.cloud_bill().expect("armed");
    assert!(
        after.abs_diff(reserved + 10 + 20) <= 3,
        "10 s halved then 10 s full: {after} after {reserved}"
    );
    // A second use waits for its cooldown.
    sim.apply_as(
        ids[0],
        Command::Interact {
            prop: "il_rack_1".into(),
        },
    );
    let events = sim.tick();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Interacted { prop, outcome, .. } if prop == "il_rack_1" && *outcome == 0
    )));
}

#[test]
fn bill_events_go_out_on_tenths_and_a_joiner_gets_the_meter() {
    let (mut sim, ids, _) = autoscaler(false, 3, 12);
    let events = run(&mut sim, 60);
    let sent: Vec<u16> = events
        .iter()
        .filter_map(|e| match e {
            Event::CloudBill { bill, .. } => Some(*bill),
            _ => None,
        })
        .collect();
    assert!(!sent.is_empty() && sent.len() <= 15, "frames: {sent:?}");
    assert!(sent.windows(2).all(|w| w[0] <= w[1]));
    let late = sim.spawn_player("backend", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let events = sim.tick();
    let mine: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e, Event::CloudBill { .. }))
        .collect();
    assert!(!mine.is_empty(), "the joiner sees the bill");
    let _ = (ids, late);
}

// ------------------------------------------------------------------ the Region Outage

fn outage(story: bool, n: usize, seed: u64, tough: bool) -> (Sim, Vec<EntityId>, EntityId) {
    let (mut sim, ids) = wing(n, story, seed, tough);
    load(&mut sim, "ilmari4/3");
    let boss = unit_of(&sim, "il_region_outage").expect("boss");
    // The hub is where the boss hovers: the pull happens standing on it.
    pull(&mut sim, &ids, boss, Vec3::new(2.0, 0.0, BOSS.1));
    // The encounter arms on the first tick of the fight.
    sim.tick();
    (sim, ids, boss)
}

fn phase_of(sim: &Sim, id: &str) -> PlatformPhase {
    sim.platform_phases()
        .into_iter()
        .find(|p| p.0 == id)
        .map(|p| p.1)
        .expect("platform")
}

fn keep_boss(sim: &mut Sim, boss: EntityId) {
    sim.debug_set_hp(boss, 100_000_000);
}

fn run_keep(sim: &mut Sim, secs: u32, boss: EntityId) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..secs * SECOND {
        out.extend(sim.tick());
        keep_boss(sim, boss);
    }
    out
}

#[test]
fn us_east_1_is_always_first_and_the_rest_follow_the_seed() {
    let mut orders = std::collections::BTreeSet::new();
    for seed in 1..=24 {
        let (sim, _, _) = outage(false, 3, seed, true);
        let order = sim.platform_order();
        assert_eq!(order.len(), 5);
        assert_eq!(order[0], "us-east-1", "seed {seed}: {order:?}");
        let mut sorted = order.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 5, "every region once");
        // The same seed, the same tour.
        let (again, _, _) = outage(false, 3, seed, true);
        assert_eq!(again.platform_order(), order);
        orders.insert(order);
    }
    assert!(orders.len() > 4, "the seed shuffles the rest: {orders:?}");
}

#[test]
fn a_region_degrades_goes_dark_restores_and_the_tour_goes_on() {
    let (mut sim, _, boss) = outage(false, 3, 1, true);
    // Everything is live until the first turn at 14 s.
    run_keep(&mut sim, 13, boss);
    assert!(
        sim.platform_phases()
            .iter()
            .all(|p| p.1 == PlatformPhase::Live)
    );
    run_keep(&mut sim, 2, boss);
    assert_eq!(phase_of(&sim, "us-east-1"), PlatformPhase::Degraded);
    run_keep(&mut sim, 4, boss);
    assert_eq!(phase_of(&sim, "us-east-1"), PlatformPhase::Dark);
    run_keep(&mut sim, 30, boss);
    assert_eq!(phase_of(&sim, "us-east-1"), PlatformPhase::Restoring);
    run_keep(&mut sim, 4, boss);
    assert_eq!(phase_of(&sim, "us-east-1"), PlatformPhase::Live);
    // The next region began at one period (37 s) after the first: the second of the tour.
    let second = sim.platform_order()[1].clone();
    assert_ne!(phase_of(&sim, &second), PlatformPhase::Live, "{second}");
    // The hub and the bridges never go dark, whatever the time.
    for _ in 0..300 {
        run_keep(&mut sim, 1, boss);
        for (id, ph) in sim.platform_phases() {
            if id == "hub" || id.starts_with("bridge") {
                assert_eq!(ph, PlatformPhase::Live, "{id}");
            }
        }
    }
}

#[test]
fn the_warning_comes_ahead_and_story_names_the_whole_order() {
    for (story, ahead_s, order_len) in [(false, 8, 0), (true, 20, 5)] {
        let (mut sim, ids) = wing(if story { 1 } else { 3 }, story, 2, true);
        load(&mut sim, "ilmari4/3");
        let boss = unit_of(&sim, "il_region_outage").expect("boss");
        pull(&mut sim, &ids, boss, Vec3::new(2.0, 0.0, BOSS.1));
        // (tick, event) from the arming on.
        let mut seen: Vec<(u32, Event)> = Vec::new();
        for k in 0..40 * SECOND {
            for e in sim.tick() {
                seen.push((k, e));
            }
            keep_boss(&mut sim, boss);
        }
        let warns: Vec<(u32, u8, u64, usize)> = seen
            .iter()
            .filter_map(|(k, e)| match e {
                Event::PlatformWarn { idx, at_ms, order } if *idx != u8::MAX => {
                    Some((*k, *idx, *at_ms, order.len()))
                }
                _ => None,
            })
            .collect();
        let first = warns.first().expect("a warning");
        assert_eq!(first.3, order_len, "story {story}: the order list");
        let degrade = seen.iter().find_map(|(k, e)| match e {
            Event::PlatformState {
                idx,
                state: PlatformPhase::Degraded,
                at_ms,
            } if *idx == first.1 => Some((*k, *at_ms)),
            _ => None,
        });
        let (dk, dms) = degrade.expect("the warned region degrades");
        assert!(
            dms.abs_diff(first.2) <= 100,
            "story {story}: warned for {} ms, degraded at {dms} ms",
            first.2
        );
        // Full warns 8 s ahead (or from the first tick if it is nearer); Story 20 s.
        let lead = (dk.saturating_sub(first.0)) / SECOND;
        assert!(
            lead <= ahead_s && lead + 2 >= ahead_s.min(14),
            "story {story}: {lead} s ahead"
        );
    }
}

/// Failover hits of `p` in `events`: direct damage without a spell of at least `floor` points
/// (the boss's melee and abilities are far below the percent hits of 10 million HP players).
fn failover_hits(events: &[Event], p: EntityId, floor: u32) -> Vec<u32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Damage {
                target,
                spell: None,
                amount,
                ..
            } if *target == p && *amount >= floor => Some(*amount),
            _ => None,
        })
        .collect()
}

#[test]
fn off_the_grid_costs_a_hit_and_a_teleport_and_never_the_last_point() {
    let (mut sim, ids, boss) = outage(false, 3, 3, true);
    // Everyone stands on the hub until us-east-1 goes dark; one player waits on it.
    let p = ids[1];
    move_to(&mut sim, p, Vec3::new(BOSS.0 - 28.0, 0.0, BOSS.1));
    let max = sim.unit(p).expect("p").max_hp;
    let early = run_keep(&mut sim, 13, boss);
    assert!(
        failover_hits(&early, p, max / 20).is_empty(),
        "nothing before the turn"
    );
    // 14 s degrade, 18 s dark: grace is a second, so by 20 s the teleport has happened.
    let events = run_keep(&mut sim, 7, boss);
    assert_eq!(
        failover_hits(&events, p, max / 20),
        vec![max / 10],
        "one Failover of 10%"
    );
    let pos = events
        .iter()
        .find_map(|e| match e {
            Event::Teleported { unit, pos } if *unit == p => Some(*pos),
            _ => None,
        })
        .expect("teleported");
    let live = sim
        .platform_spots()
        .into_iter()
        .filter(|s| s.2 != PlatformKind::Bridge && s.3 == PlatformPhase::Live)
        .any(|s| combat_core::distance(s.1, pos) <= 8.0);
    assert!(live, "landed on a live platform: {pos:?}");
    assert_eq!(sim.unit(p).expect("p").pos, pos);
    // And it is the only one: once on the hub nothing else happens to that player.
    let later = run_keep(&mut sim, 10, boss);
    assert!(failover_hits(&later, p, max / 20).is_empty());
}

#[test]
fn failover_never_kills_even_at_one_hit_point() {
    let (mut sim, ids, boss) = outage(false, 3, 4, true);
    let p = ids[2];
    move_to(&mut sim, p, Vec3::new(BOSS.0 - 28.0, 0.0, BOSS.1));
    sim.debug_set_hp(p, 1);
    let events = run_keep(&mut sim, 22, boss);
    assert!(sim.unit(p).is_some_and(|u| u.alive && u.hp >= 1));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Teleported { unit, .. } if *unit == p)),
        "teleported all the same"
    );
}

#[test]
fn story_failover_costs_five_percent() {
    let (mut sim, ids, boss) = outage(true, 1, 5, true);
    let p = ids[0];
    let max = sim.unit(p).expect("p").max_hp;
    move_to(&mut sim, p, Vec3::new(BOSS.0 - 28.0, 0.0, BOSS.1));
    let events = run_keep(&mut sim, 22, boss);
    assert_eq!(failover_hits(&events, p, max / 40), vec![max / 20]);
}

#[test]
fn the_margin_and_the_grace_forgive_lag() {
    let (mut sim, ids, boss) = outage(false, 3, 6, true);
    let p = ids[1];
    let max = sim.unit(p).expect("p").max_hp;
    // 1.4 m past the hub's edge (9 m): inside the 1.5 m margin, for as long as it likes.
    move_to(&mut sim, p, Vec3::new(BOSS.0 + 10.4, 0.0, BOSS.1));
    let events = run_keep(&mut sim, 10, boss);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Teleported { unit, .. } if *unit == p)),
        "inside the margin"
    );
    // Well outside for less than the 1 s grace, then back: nothing.
    let mut blips = Vec::new();
    for _ in 0..4 {
        move_to(&mut sim, p, Vec3::new(BOSS.0, 0.0, BOSS.1 + 16.0));
        for _ in 0..20 {
            blips.extend(sim.tick());
            keep_boss(&mut sim, boss);
        }
        move_to(&mut sim, p, Vec3::new(BOSS.0 + 2.0, 0.0, BOSS.1));
        blips.extend(run_keep(&mut sim, 2, boss));
    }
    assert!(
        failover_hits(&blips, p, max / 20).is_empty(),
        "no hit for a blip"
    );
    assert!(
        !blips
            .iter()
            .any(|e| matches!(e, Event::Teleported { unit, .. } if *unit == p))
    );
    // Staying out does cost it.
    move_to(&mut sim, p, Vec3::new(BOSS.0, 0.0, BOSS.1 + 16.0));
    let events = run_keep(&mut sim, 3, boss);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Teleported { unit, .. } if *unit == p))
    );
}

#[test]
fn bridges_count_as_grid_and_a_dark_region_does_not() {
    let (mut sim, ids, boss) = outage(false, 3, 7, true);
    let p = ids[1];
    // Standing on bridge 1 (between the hub and us-east-1) while us-east-1 goes dark is fine.
    move_to(&mut sim, p, Vec3::new(BOSS.0 - 17.0, 0.0, BOSS.1));
    let events = run_keep(&mut sim, 30, boss);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Teleported { unit, .. } if *unit == p)),
        "a bridge never goes dark"
    );
}

#[test]
fn full_cascades_two_at_a_time_below_two_thirds_and_story_never_does() {
    for story in [false, true] {
        let (mut sim, _, boss) = outage(story, if story { 1 } else { 3 }, 8, true);
        let max = sim.unit(boss).expect("boss").max_hp;
        // Below 66% the period halves on Full: count the regions that were non-live at once.
        let mut worst = 0;
        for _ in 0..160 {
            sim.debug_set_hp(boss, max / 2);
            run(&mut sim, 1);
            let down = sim
                .platform_phases()
                .iter()
                .filter(|p| p.1 != PlatformPhase::Live)
                .count();
            worst = worst.max(down);
        }
        if story {
            assert_eq!(worst, 1, "Story: one at a time");
        } else {
            assert!(worst >= 2, "Full cascades: {worst}");
        }
    }
}

#[test]
fn an_uninterrupted_health_check_brings_the_next_outage_ten_seconds_early() {
    let (mut sim, _, boss) = outage(false, 3, 9, true);
    let mut prev = sim.outage_next_in_ms().expect("armed");
    let mut seen = None;
    for _ in 0..90 * SECOND {
        let events = sim.tick();
        keep_boss(&mut sim, boss);
        let next = sim.outage_next_in_ms().expect("armed");
        if events.iter().any(|e| {
            matches!(e, Event::Damage { spell: Some(s), source, .. } if s.0 == 2573 && *source == boss)
        }) {
            seen = Some((prev, next));
            break;
        }
        prev = next;
    }
    let (before, after) = seen.expect("the Health Check landed");
    // The clock ran a tick; the rest of the drop is the ten seconds (or what was left of the wait).
    assert!(
        before.saturating_sub(after) + 100 >= before.min(10_000),
        "the next outage moved from {before} ms to {after} ms"
    );
}

#[test]
fn a_joiner_sees_the_platforms_as_they_stand() {
    let (mut sim, _, boss) = outage(false, 3, 10, true);
    run_keep(&mut sim, 20, boss);
    let late = sim.spawn_player("backend", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let events = sim.tick();
    let mut seen: Vec<(u8, PlatformPhase)> = events
        .iter()
        .filter_map(|e| match e {
            Event::PlatformState { idx, state, .. } => Some((*idx, *state)),
            _ => None,
        })
        .collect();
    seen.sort_by_key(|s| s.0);
    assert_eq!(seen.len(), 11, "every platform: {seen:?}");
    let want: Vec<PlatformPhase> = sim.platform_phases().iter().map(|p| p.1).collect();
    assert_eq!(seen.iter().map(|s| s.1).collect::<Vec<_>>(), want);
    assert!(
        seen.iter()
            .any(|s| s.1 == PlatformPhase::Degraded || s.1 == PlatformPhase::Dark),
        "the sequence is running"
    );
    let _ = late;
}

#[test]
fn a_wipe_puts_every_platform_back() {
    let (mut sim, ids, boss) = outage(false, 3, 11, true);
    run_keep(&mut sim, 20, boss);
    for &p in &ids {
        sim.debug_kill_player(p);
    }
    let events = run(&mut sim, 30);
    assert!(sim.platform_phases().is_empty(), "disarmed");
    assert!(events.iter().any(|e| matches!(
        e,
        Event::PlatformState {
            state: PlatformPhase::Live,
            ..
        }
    )));
}

// ------------------------------------------------------------------ companions

#[test]
fn a_companion_never_stays_on_a_dark_platform_or_behind_its_owner() {
    let (mut sim, ids) = wing(1, true, 12, true);
    let p = ids[0];
    sim.set_companion_picks(p, &[DALE.to_string(), PRIYA.to_string()]);
    load(&mut sim, "ilmari4/3");
    sim.tick();
    let boss = unit_of(&sim, "il_region_outage").expect("boss");
    pull(&mut sim, &ids, boss, Vec3::new(2.0, 0.0, BOSS.1));
    let comps = sim.companions_of(p);
    assert_eq!(comps.len(), 2);
    // The companions are sent to us-east-1; when it degrades they join the owner on the hub.
    for &c in &comps {
        sim.debug_teleport_unit(c, Vec3::new(BOSS.0 - 28.0, 0.0, BOSS.1));
    }
    run_keep(&mut sim, 16, boss);
    for &c in &comps {
        let at = sim.unit(c).expect("companion").pos;
        assert!(
            combat_core::distance(at, Vec3::new(BOSS.0, 0.0, BOSS.1)) <= 12.0,
            "snapped to the owner: {at:?}"
        );
    }
}

// ------------------------------------------------------------------ the wing around them

#[test]
fn the_wing_door_waits_for_the_warden_and_each_boss_has_three_own_pieces() {
    let c = content();
    let door = c
        .zone("campus_occupied")
        .and_then(|z| z.links.iter().find(|l| l.to == "ilmari4/1"))
        .expect("door");
    assert!(matches!(&door.requires, Some(Unlock::QuestDone(q)) if q == "il_10_wing4_attune"));
    let w4 = c.raid_of("ilmari4/1").expect("wing");
    assert!(w4.ilmari && w4.zones.len() == 3);
    let q = |id: &str| c.quests.iter().find(|q| q.id == id).expect("quest");
    assert_eq!(
        q("il_09_roof_intel").requires.as_deref(),
        Some("il_w3_done")
    );
    assert_eq!(
        q("il_10_wing4_attune").requires.as_deref(),
        Some("il_09_roof_intel")
    );
    assert_eq!(
        q("il_w4_done").requires.as_deref(),
        Some("il_10_wing4_attune")
    );
    for boss in ["il_autoscaler", "il_region_outage"] {
        let own: Vec<_> = c
            .economy
            .gear
            .iter()
            .filter(|g| g.drops_from.as_deref() == Some(boss))
            .collect();
        assert_eq!(own.len(), 3, "{boss}");
        for g in own {
            assert_eq!(g.source, GearSource::Ilmari);
            assert!(
                g.ilvl == 18,
                "{}: ilvl {} (20 stays reserved)",
                g.name,
                g.ilvl
            );
            assert!(g.description.len() <= 110, "{} tooltip", g.name);
        }
    }
    // The first room is a trash gauntlet: it opens onto the second without a boss to beat.
    let z1 = c.zone("ilmari4/1").expect("zone");
    assert!(z1.boss.is_none());
    assert!(
        z1.links
            .iter()
            .any(|l| l.to == "ilmari4/2" && l.requires.is_none())
    );
    let z2 = c.zone("ilmari4/2").expect("zone");
    assert!(z2.links.iter().any(|l| l.to == "ilmari4/3"
        && matches!(&l.requires, Some(Unlock::BossKilled(b)) if b == "il_autoscaler")));
}

#[test]
fn the_platform_layout_is_consistent() {
    let c = content();
    let z = c.zone("ilmari4/3").expect("zone");
    let regions: Vec<&str> = z
        .platforms
        .iter()
        .filter(|p| p.kind == PlatformKind::Region)
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(regions.len(), 5);
    assert!(regions.contains(&"us-east-1"));
    // The hub is under the boss: the origin is on a platform and no region overlaps another.
    let hub = z
        .platforms
        .iter()
        .find(|p| p.kind == PlatformKind::Hub)
        .expect("hub");
    assert!(hub.dist_outside(0.0, 0.0) == 0.0);
    for (i, a) in z.platforms.iter().enumerate() {
        for b in &z.platforms[..i] {
            let both = a.kind == PlatformKind::Region && b.kind == PlatformKind::Region;
            if both {
                let (dx, dz) = (a.x - b.x, a.z - b.z);
                assert!(
                    (dx * dx + dz * dz).sqrt() > 16.0,
                    "{} and {} overlap",
                    a.id,
                    b.id
                );
            }
        }
    }
    // Every region is reached from the hub by a bridge: a point halfway is on a platform.
    for r in z
        .platforms
        .iter()
        .filter(|p| p.kind == PlatformKind::Region)
    {
        let (mx, mz) = (r.x * 0.5, r.z * 0.5);
        assert!(
            z.platforms.iter().any(|p| p.dist_outside(mx, mz) == 0.0),
            "{} has a bridge",
            r.id
        );
    }
}

#[test]
fn the_wing_locks_per_difficulty_and_titles_follow_the_bosses() {
    for (story, ach, zone, boss) in [
        (true, "il_w4_autoscaler", "ilmari4/2", "il_autoscaler"),
        (false, "il_w4_autoscaler", "ilmari4/2", "il_autoscaler"),
        (true, "il_w4_outage", "ilmari4/3", "il_region_outage"),
        (false, "il_w4_fivenines", "ilmari4/3", "il_region_outage"),
    ] {
        let (mut sim, ids) = wing(if story { 1 } else { 3 }, story, 13, true);
        load(&mut sim, zone);
        let id = unit_of(&sim, boss).expect("boss");
        pull(&mut sim, &ids, id, Vec3::new(2.0, 0.0, BOSS.1));
        run(&mut sim, 1);
        assert!(sim.debug_kill(id));
        for _ in 0..10 {
            sim.tick();
        }
        assert!(sim.raid_locked(boss, story), "locked on its difficulty");
        assert!(
            !sim.raid_locked(boss, !story),
            "the other difficulty is not"
        );
        let has = sim
            .unit(ids[0])
            .is_some_and(|u| u.achievements.contains(&ach.to_string()));
        assert!(has, "{ach} after {boss} (story {story})");
    }
}

fn fight_log(zone: &str, boss_key: &str, story: bool, seed: u64) -> String {
    let (mut sim, ids) = wing(if story { 1 } else { 3 }, story, seed, true);
    load(&mut sim, zone);
    let boss = unit_of(&sim, boss_key).expect("boss");
    pull(&mut sim, &ids, boss, Vec3::new(2.0, 0.0, BOSS.1));
    let mut log = String::new();
    for k in 0..60 * SECOND {
        for e in sim.tick() {
            log.push_str(&log_line(&sim, u64::from(k), &e));
            log.push('\n');
        }
        // A burst now and then, so the Autoscaler splits and the bill moves.
        if k % (9 * SECOND) == 4 * SECOND {
            let first = unit_of(&sim, boss_key);
            if let Some(first) = first {
                let max = sim.unit(first).map_or(1000, |u| u.max_hp);
                sim.debug_hit(ids[0], first, max / 9);
            }
        }
    }
    log
}

#[test]
fn both_fights_replay_bit_identically() {
    for (zone, boss) in [
        ("ilmari4/2", "il_autoscaler"),
        ("ilmari4/3", "il_region_outage"),
    ] {
        for story in [false, true] {
            let a = fight_log(zone, boss, story, 3);
            let b = fight_log(zone, boss, story, 3);
            assert!(!a.is_empty());
            assert_eq!(a, b, "{boss} story {story}");
        }
    }
}
