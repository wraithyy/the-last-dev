//! Ilmari Wing 2, Human Resources (docs/ILMARI_09.md B.5): the HR Business Partner's mirrors, the
//! Culture Committee's vote and ballot terminals, Performance Review's grades and the HR Portal,
//! plus the wing's door, locks, tokens and drops. Balance (Story solo, Full trio) lives in
//! tests/playthrough.rs.
#![allow(clippy::expect_used)]

mod common;

use combat_core::{Command, Content, EntityId, Event, GearSource, Sim, SpellId, Unlock, Vec3};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;

/// Packs and props out of the way; the boss 20 m north of the arrival, terminals at hand.
fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, -20.0),
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
    ] {
        def_mut(c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
}

fn wing(n: usize, story: bool, classes: &[&str], seed: u64) -> (Sim, Vec<EntityId>) {
    let mut sim = Sim::new(
        {
            let mut c = content();
            sturdy(&mut c);
            c
        },
        seed,
    );
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

fn pull(sim: &mut Sim, ids: &[EntityId], t: EntityId) {
    for &p in ids {
        move_to(sim, p, Vec3::new(0.0, 0.0, -8.0));
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

fn auras(sim: &Sim, id: EntityId) -> Vec<String> {
    sim.unit(id)
        .map(|u| u.auras.iter().map(|a| a.key.clone()).collect())
        .unwrap_or_default()
}

/// Keeps every member of a linked panel (or one boss) alive on the shared bar.
fn keep(sim: &mut Sim, keys: &[&str]) {
    for k in keys {
        for id in sim
            .units()
            .filter(|u| u.key == *k && u.alive)
            .map(|u| u.id)
            .collect::<Vec<_>>()
        {
            sim.debug_set_hp(id, 10_000_000);
        }
    }
}

fn interact(sim: &mut Sim, p: EntityId, prop: &str) {
    move_to(sim, p, Vec3::new(0.0, 0.0, 0.0));
    sim.apply_as(p, Command::Interact { prop: prop.into() });
}

// ------------------------------------------------------------------ the mirror

/// Everything the mirrors did over `secs` of the Partner fight: (tick, mirror key, copied player).
fn watch_mirrors(story: bool, secs: u32, seed: u64) -> Vec<(u64, String, EntityId)> {
    let classes = ["backend", "devops", "fullstack", "frontend"];
    let (mut sim, ids) = wing(if story { 1 } else { 4 }, story, &classes, seed);
    load(&mut sim, "ilmari2/1");
    let boss = unit_of(&sim, "il_hrbp").expect("boss");
    pull(&mut sim, &ids, boss);
    let mut seen = Vec::new();
    for _ in 0..secs * SECOND {
        let events = sim.tick();
        for e in events {
            if let Event::Mirrored { mirror, player } = e {
                let key = sim.unit(mirror).map(|u| u.key.clone()).unwrap_or_default();
                seen.push((sim.now().0, key, player));
            }
        }
        keep(&mut sim, &["il_hrbp"]);
    }
    seen
}

#[test]
fn the_partner_copies_a_player_of_each_role_with_their_kit_and_the_same_seed_picks_the_same_one() {
    let a = watch_mirrors(false, 130, 11);
    let b = watch_mirrors(false, 130, 11);
    assert_eq!(a, b, "a replay copies the same people");
    assert!(a.len() >= 3, "a copy every 40 s: {a:?}");
    let classes = ["backend", "devops", "fullstack", "frontend"];
    for (_, key, player) in &a {
        let class = classes[(*player as usize - 1) % 4];
        let want = match class {
            "backend" => "il_mirror_tank",
            "devops" => "il_mirror_healer",
            _ => "il_mirror_dps",
        };
        assert_eq!(key, want, "{class}'s copy has {class}'s role");
    }
    let who: std::collections::BTreeSet<_> = a.iter().map(|m| m.2).collect();
    assert!(who.len() >= 2, "the pick moves around: {who:?}");
}

#[test]
fn the_copied_player_hits_weaker_until_the_copy_is_found_or_clocks_out() {
    let classes = ["backend", "devops", "fullstack", "frontend"];
    let (mut sim, ids) = wing(4, false, &classes, 5);
    load(&mut sim, "ilmari2/1");
    let boss = unit_of(&sim, "il_hrbp").expect("boss");
    pull(&mut sim, &ids, boss);
    let mut pair = None;
    for _ in 0..14 * SECOND {
        for e in sim.tick() {
            if let Event::Mirrored { mirror, player } = e {
                pair = Some((mirror, player));
            }
        }
        keep(&mut sim, &["il_hrbp"]);
        if pair.is_some() {
            break;
        }
    }
    let (mirror, player) = pair.expect("a copy within 14 s");
    assert!(auras(&sim, player).contains(&"mirrored".to_string()));
    assert!(
        auras(&sim, player)
            .iter()
            .filter(|a| *a == "mirrored")
            .count()
            == 1
    );
    // Finding the copy lifts the marker at once.
    assert!(sim.debug_kill(mirror));
    sim.tick();
    sim.tick();
    assert!(!auras(&sim, player).contains(&"mirrored".to_string()));
    // Left alone, the next one clocks out after 28 s: copy and marker both go.
    let mut next = None;
    for _ in 0..60 * SECOND {
        for e in sim.tick() {
            if let Event::Mirrored { mirror, player } = e {
                next = Some((mirror, player, sim.now().0));
            }
        }
        keep(&mut sim, &["il_hrbp"]);
        if next.is_some() {
            break;
        }
    }
    let (mirror, player, born) = next.expect("a second copy");
    for _ in 0..27 * SECOND {
        sim.tick();
        keep(&mut sim, &["il_hrbp"]);
    }
    assert!(
        sim.unit(mirror).is_some_and(|u| u.alive),
        "27 s in: still here"
    );
    for _ in 0..3 * SECOND {
        sim.tick();
        keep(&mut sim, &["il_hrbp"]);
    }
    assert!(
        sim.unit(mirror).is_none_or(|u| !u.alive),
        "28 s: clocked out (born at {born})"
    );
    assert!(!auras(&sim, player).contains(&"mirrored".to_string()));
}

#[test]
fn story_copies_one_player_at_a_time_for_forty_seconds_and_full_phase_two_copies_two() {
    let (mut sim, ids) = wing(1, true, &["fullstack"], 3);
    load(&mut sim, "ilmari2/1");
    let boss = unit_of(&sim, "il_hrbp").expect("boss");
    pull(&mut sim, &ids, boss);
    let mut left = 0;
    for _ in 0..12 * SECOND {
        sim.tick();
        keep(&mut sim, &["il_hrbp"]);
        if let Some(a) = sim
            .unit(ids[0])
            .and_then(|u| u.auras.iter().find(|a| a.key == "mirrored").cloned())
        {
            left = a.remaining_ms;
        }
    }
    assert!(
        (30_000..=40_000).contains(&left),
        "Story: 40 s copy ({left} ms left)"
    );
    // Full, phase two: two copies at once.
    let classes = ["backend", "devops", "fullstack", "frontend"];
    let (mut sim, ids) = wing(4, false, &classes, 9);
    load(&mut sim, "ilmari2/1");
    let boss = unit_of(&sim, "il_hrbp").expect("boss");
    pull(&mut sim, &ids, boss);
    let max_hp = sim.unit(boss).expect("boss").max_hp;
    let mut most = 0;
    for _ in 0..30 * SECOND {
        sim.tick();
        // Below half: phase two.
        sim.debug_set_hp(boss, max_hp * 40 / 100);
        let live = sim
            .units()
            .filter(|u| u.key.starts_with("il_mirror_") && u.alive)
            .count();
        most = most.max(live);
    }
    assert_eq!(most, 2, "two copies at once in phase two");
}

// ------------------------------------------------------------------ the vote

fn committee(story: bool, n: usize, seed: u64) -> (Sim, Vec<EntityId>) {
    let classes = ["backend", "devops", "fullstack", "frontend"];
    let (mut sim, ids) = wing(n, story, &classes, seed);
    load(&mut sim, "ilmari2/2");
    let boss = unit_of(&sim, "il_cc_cpo").expect("boss");
    pull(&mut sim, &ids, boss);
    for _ in 0..2 * SECOND {
        sim.tick();
        keep(&mut sim, &["il_cc_cpo", "il_cc_culture", "il_cc_vibes"]);
    }
    (sim, ids)
}

fn run(sim: &mut Sim, secs: u32) {
    for _ in 0..secs * SECOND {
        sim.tick();
        keep(sim, &["il_cc_cpo", "il_cc_culture", "il_cc_vibes"]);
    }
}

#[test]
fn the_committee_is_three_members_on_one_bar_with_one_open() {
    let (sim, _) = committee(false, 4, 1);
    let members: Vec<_> = ["il_cc_cpo", "il_cc_culture", "il_cc_vibes"]
        .iter()
        .map(|k| unit_of(&sim, k).expect(k))
        .collect();
    let shielded = members
        .iter()
        .filter(|m| auras(&sim, **m).contains(&"committee_quorum".to_string()))
        .count();
    assert_eq!(shielded, 2, "one open, two shielded");
}

#[test]
fn the_slot_with_the_most_ballots_passes_and_silence_picks_the_first() {
    let (mut sim, ids) = committee(false, 4, 2);
    let view = sim.raid_view();
    assert_eq!(view.vote.len(), 3, "three of five on the ballot");
    assert_eq!(view.ballots, vec![0, 0, 0]);
    assert!(view.vote.iter().all(|p| p.starts_with("pol_")));
    let mut unique = view.vote.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 3, "three different policies");
    let offered = view.vote.clone();
    // Slot C three times, slot A once (each player has a 3 s cooldown on a terminal).
    interact(&mut sim, ids[0], "il_ballot_c");
    interact(&mut sim, ids[1], "il_ballot_a");
    sim.tick();
    for _ in 0..2 {
        run(&mut sim, 4);
        interact(&mut sim, ids[0], "il_ballot_c");
        sim.tick();
    }
    assert_eq!(sim.raid_view().ballots, vec![1, 0, 3]);
    // The swap closes the vote 30 s after the pull.
    run(&mut sim, 30);
    let view = sim.raid_view();
    assert_eq!(view.passed, offered[2], "slot C passed");
    for &p in &ids {
        assert!(auras(&sim, p).contains(&offered[2]), "everyone wears it");
        let left = sim
            .unit(p)
            .and_then(|u| u.auras.iter().find(|a| a.key == offered[2]).cloned())
            .map_or(0, |a| a.remaining_ms);
        assert!(left > 0 && left <= 20_000, "20 s of it at most: {left}");
    }
    assert_eq!(view.ballots, vec![0, 0, 0], "a fresh ballot");
    // Nobody votes the second time: the first listed slot passes.
    let second = view.vote.clone();
    run(&mut sim, 30);
    assert_eq!(sim.raid_view().passed, second[0], "silence picks the first");
}

#[test]
fn a_tie_goes_to_the_earlier_slot_and_the_terminal_has_a_three_second_cooldown() {
    let (mut sim, ids) = committee(false, 4, 4);
    let offered = sim.raid_view().vote.clone();
    interact(&mut sim, ids[0], "il_ballot_b");
    sim.tick();
    interact(&mut sim, ids[0], "il_ballot_b");
    sim.tick();
    assert_eq!(
        sim.raid_view().ballots,
        vec![0, 1, 0],
        "the second use waits"
    );
    interact(&mut sim, ids[1], "il_ballot_a");
    sim.tick();
    assert_eq!(sim.raid_view().ballots, vec![1, 1, 0]);
    run(&mut sim, 30);
    assert_eq!(
        sim.raid_view().passed,
        offered[0],
        "1 : 1 goes to the first"
    );
}

#[test]
fn story_skips_the_ballot_and_passes_pizza_friday_every_time() {
    let (mut sim, ids) = committee(true, 1, 6);
    assert!(sim.raid_view().vote.is_empty(), "no ballot on Story");
    run(&mut sim, 30);
    assert_eq!(sim.raid_view().passed, "pol_pizza");
    assert!(auras(&sim, ids[0]).contains(&"pol_pizza".to_string()));
    run(&mut sim, 30);
    assert_eq!(sim.raid_view().passed, "pol_pizza");
}

#[test]
fn every_policy_aura_does_what_the_wall_board_says() {
    let c = content();
    let get = |k: &str| c.aura(k).unwrap_or_else(|| panic!("{k}"));
    assert_eq!(get("pol_rto").stealth_slow_pct, 30);
    assert_eq!(get("pol_open_plan").damage_taken_pct, 25);
    assert_eq!(get("pol_efficiency").cast_time_pct, 30);
    assert_eq!(get("pol_unlimited_pto").max_hp_pct, -20);
    let pizza = get("pol_pizza");
    assert_eq!(pizza.haste_pct, 10);
    assert!(pizza.periodic.is_some(), "it comes with strings");
    let vote = c
        .raid_of("ilmari2/2")
        .and_then(|r| r.twins.as_ref())
        .and_then(|t| t.vote.as_ref())
        .expect("vote");
    assert_eq!(vote.policies.len(), 5);
    assert_eq!((vote.show, vote.duration_ms), (3, 20_000));
}

// ------------------------------------------------------------------ the review

fn review_ready(story: bool, n: usize, seed: u64) -> (Sim, Vec<EntityId>, EntityId) {
    // Damage dealers only: a tank's soaking and a healer's healing would blur who is "lowest".
    let classes = ["fullstack", "frontend"];
    let (mut sim, ids) = wing(n, story, &classes, seed);
    load(&mut sim, "ilmari2/3");
    let boss = unit_of(&sim, "il_perf_review").expect("boss");
    pull(&mut sim, &ids, boss);
    (sim, ids, boss)
}

/// Runs until the first review lands, every `(player, damage)` of `hits` landing on the boss each
/// second: (grades, worst, best).
fn first_review(
    sim: &mut Sim,
    boss: EntityId,
    hits: &[(EntityId, u32)],
) -> (Vec<(EntityId, u32)>, Option<EntityId>, Option<EntityId>) {
    for t in 0..40 * SECOND {
        if t % SECOND == 0 {
            for &(p, dmg) in hits {
                sim.debug_hit(p, boss, dmg);
            }
        }
        for e in sim.tick() {
            if let Event::Review {
                grades,
                worst,
                best,
            } = e
            {
                return (grades, worst, best);
            }
        }
        sim.debug_set_hp(boss, 10_000_000);
    }
    panic!("no review in 40 s");
}

#[test]
fn the_lowest_grade_is_managed_out_and_the_highest_exceeds_expectations() {
    let (mut sim, ids, boss) = review_ready(false, 3, 8);
    // The first player hits hard, the second lightly, the third not at all: the third grades
    // zero and is managed out, the first is far above the baseline.
    let (grades, worst, best) = first_review(&mut sim, boss, &[(ids[0], 900), (ids[1], 60)]);
    assert_eq!(grades.len(), 3);
    let grade = |p: EntityId| grades.iter().find(|g| g.0 == p).map_or(0, |g| g.1);
    assert_eq!(grade(ids[2]), 0, "{grades:?}");
    assert!(
        grade(ids[0]) > 100 && grade(ids[0]) > grade(ids[1]),
        "{grades:?}"
    );
    assert_eq!(worst, Some(ids[2]), "{grades:?}");
    assert_eq!(best, Some(ids[0]));
    sim.tick();
    assert!(auras(&sim, ids[2]).contains(&"managed_out".to_string()));
    assert!(!auras(&sim, ids[0]).contains(&"managed_out".to_string()));
    assert!(auras(&sim, ids[0]).contains(&"employee_of_the_month".to_string()));
}

#[test]
fn nobody_is_managed_out_on_story_unless_they_did_almost_nothing() {
    // Story, two active players: nobody is below a quarter of their baseline.
    let (mut sim, ids, boss) = review_ready(true, 2, 8);
    let (grades, worst, _) = first_review(&mut sim, boss, &[(ids[0], 900), (ids[1], 900)]);
    assert!(grades.iter().all(|g| g.1 >= 25), "{grades:?}");
    assert_eq!(worst, None, "nobody is worst when everybody is fine");
    // Story, a lone idle player: grade zero, managed out in the mild way.
    let (mut sim, ids, boss) = review_ready(true, 1, 8);
    let (_, worst, _) = first_review(&mut sim, boss, &[]);
    assert_eq!(worst, Some(ids[0]));
    sim.tick();
    let held = auras(&sim, ids[0]);
    assert!(held.contains(&"managed_out_story".to_string()), "{held:?}");
    assert!(!held.contains(&"managed_out".to_string()));
}

#[test]
fn the_portal_clears_managed_out_and_waits_fifteen_seconds() {
    let (mut sim, ids, _) = review_ready(false, 1, 8);
    for aura in ["managed_out", "managed_out_story"] {
        assert!(sim.debug_apply_aura(ids[0], aura, 600_000));
    }
    interact(&mut sim, ids[0], "il_hr_portal_1");
    sim.tick();
    let held = auras(&sim, ids[0]);
    assert!(
        !held.iter().any(|a| a.starts_with("managed_out")),
        "{held:?}"
    );
    // A second appeal inside the cooldown does nothing; after it, it works again.
    assert!(sim.debug_apply_aura(ids[0], "managed_out", 600_000));
    interact(&mut sim, ids[0], "il_hr_portal_1");
    sim.tick();
    assert!(auras(&sim, ids[0]).contains(&"managed_out".to_string()));
    for _ in 0..15 * SECOND {
        sim.tick();
    }
    interact(&mut sim, ids[0], "il_hr_portal_1");
    sim.tick();
    assert!(!auras(&sim, ids[0]).contains(&"managed_out".to_string()));
}

#[test]
fn the_review_formula_and_baselines_are_in_the_content() {
    let c = content();
    for id in [2480, 2481] {
        let s = c.spell(SpellId(id)).expect("review spell");
        let review = s.effects().find_map(|e| match e {
            combat_core::content::Effect::Review {
                fine_pct,
                baselines,
                ..
            } => Some((*fine_pct, baselines.len())),
            _ => None,
        });
        let (fine, roles) = review.expect("a review effect");
        assert_eq!(roles, 4, "a baseline per role");
        assert_eq!(fine, if id == 2480 { 85 } else { 25 });
    }
}

// ------------------------------------------------------------------ the wing

#[test]
fn the_wing_door_waits_for_the_marshal_and_each_wing_boss_has_three_own_pieces() {
    let c = content();
    let door = c
        .zone("campus_occupied")
        .and_then(|z| z.links.iter().find(|l| l.to == "ilmari2/1"))
        .expect("door");
    assert!(matches!(&door.requires, Some(Unlock::QuestDone(q)) if q == "il_06_wing2_attune"));
    let hr = c.raid_of("ilmari2/1").expect("wing");
    assert!(hr.ilmari && !hr.is_dungeon());
    assert_eq!(
        hr.bosses(&c),
        vec!["il_hrbp", "il_cc_cpo", "il_perf_review"]
    );
    // il_05 waits for Wing 1, il_w2_done for the marshal.
    let q = |id: &str| c.quest(id).unwrap_or_else(|| panic!("{id}"));
    assert_eq!(q("il_05_hr_intel").requires.as_deref(), Some("il_w1_done"));
    assert_eq!(
        q("il_06_wing2_attune").requires.as_deref(),
        Some("il_05_hr_intel")
    );
    assert_eq!(
        q("il_w2_done").requires.as_deref(),
        Some("il_06_wing2_attune")
    );
    for boss in hr.bosses(&c) {
        let own: Vec<_> = c
            .economy
            .gear
            .iter()
            .filter(|g| g.drops_from.as_deref() == Some(boss.as_str()))
            .collect();
        assert_eq!(own.len(), 3, "{boss}");
        assert!(own.iter().all(|g| g.source == GearSource::Ilmari));
        assert!(own.iter().all(|g| (16..=20).contains(&g.ilvl)), "{boss}");
    }
}

#[test]
fn the_wing_locks_per_difficulty_pays_tokens_and_drops_only_its_own_pieces() {
    let c = content();
    for (story, tokens) in [(true, 7), (false, 14)] {
        let classes = ["backend", "fullstack", "frontend"];
        let (mut sim, ids) = wing(if story { 1 } else { 3 }, story, &classes, 33);
        load(&mut sim, "ilmari2/1");
        let boss = unit_of(&sim, "il_hrbp").expect("boss");
        pull(&mut sim, &ids, boss);
        assert!(sim.debug_kill(boss));
        let mut pieces = Vec::new();
        for e in sim.tick() {
            if let Event::LootDropped { items, .. } = e {
                pieces.extend(items);
            }
        }
        assert!(sim.raid_locked("il_hrbp", story));
        assert!(
            !sim.raid_locked("il_hrbp", !story),
            "each difficulty locks apart"
        );
        let held = sim
            .inventory()
            .iter()
            .find(|(n, _)| n == combat_core::ilmari::TOKEN)
            .map_or(0, |(_, n)| *n);
        assert_eq!(held, tokens, "tokens, half on Story");
        for p in &pieces {
            let g = c
                .economy
                .gear(p)
                .unwrap_or_else(|| panic!("{p} is not gear"));
            assert_eq!(g.source, GearSource::Ilmari, "{p}");
            assert_eq!(g.drops_from.as_deref(), Some("il_hrbp"), "{p}");
        }
        if !story {
            assert!(pieces.len() >= 2, "Full rolls two a member: {pieces:?}");
        }
    }
}

#[test]
fn the_review_closes_the_wing_with_a_title_for_each_difficulty() {
    for (story, key) in [(true, "il_w2_review_story"), (false, "il_w2_review_full")] {
        let classes = ["backend", "fullstack", "frontend"];
        let (mut sim, ids) = wing(if story { 1 } else { 3 }, story, &classes, 33);
        load(&mut sim, "ilmari2/3");
        let boss = unit_of(&sim, "il_perf_review").expect("boss");
        pull(&mut sim, &ids, boss);
        assert!(sim.debug_kill(boss));
        sim.tick();
        sim.tick();
        let earned = |k: &str| {
            sim.unit(ids[0])
                .is_some_and(|u| u.achievements.contains(&k.to_string()))
        };
        assert!(earned(key), "{key}");
        let other = if story {
            "il_w2_review_full"
        } else {
            "il_w2_review_story"
        };
        assert!(!earned(other), "{other} is the other difficulty's");
    }
}
