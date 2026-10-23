//! Ilmari Wing 3, Training Data (docs/ILMARI_10.md C.4): the Scraper Queen's waves, drafts and
//! Mark Confidential, the Hallucination's hazard cycle, Ground Truth and footnotes, Model
//! Collapse's recorder and doubles, plus the wing's door, locks, titles and drops. The balance
//! gates (Story solo, Full trio, the repeater against the varied rotation) live in
//! tests/playthrough.rs.
#![allow(clippy::expect_used)]

mod common;

use combat_core::illusion::{Circle, layout, real_set};
use combat_core::{
    CastError, Command, Content, EntityId, Event, GearSource, PROP_OK, School, Sim, SpellId,
    Unlock, Vec3,
};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;
/// Where each synthetic zone puts the boss: 30 m north of the arrival.
const BOSS: (f32, f32) = (0.0, -30.0);

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    if let Some(k) = name
        .strip_prefix("prop:il_draft_")
        .and_then(|k| k.parse::<f32>().ok())
    {
        let a = k * std::f32::consts::TAU / 5.0;
        return at(BOSS.0 + 9.0 * a.cos(), BOSS.1 + 9.0 * a.sin());
    }
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(BOSS.0, BOSS.1),
        n if n.starts_with("prop:il_truth_board_") => at(5.0, BOSS.1 + 12.0),
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
        move_to(sim, p, Vec3::new(0.0, 0.0, BOSS.1 + 12.0));
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

/// Keeps the boss alive on a huge bar while a mechanic is under test.
fn keep(sim: &mut Sim, key: &str) {
    if let Some(id) = unit_of(sim, key) {
        sim.debug_set_hp(id, 10_000_000);
    }
}

fn run(sim: &mut Sim, secs: u32, boss: &str) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..secs * SECOND {
        out.extend(sim.tick());
        keep(sim, boss);
    }
    out
}

fn interact(sim: &mut Sim, p: EntityId, prop: &str) -> Vec<Event> {
    let spot = sim.prop_spot(prop).expect("prop in the zone");
    move_to(sim, p, spot);
    sim.apply_as(p, Command::Interact { prop: prop.into() });
    sim.tick()
}

fn outcome(events: &[Event], prop: &str) -> Option<u32> {
    events.iter().find_map(|e| match e {
        Event::Interacted {
            prop: k, outcome, ..
        } if k == prop => Some(*outcome),
        _ => None,
    })
}

// ------------------------------------------------------------------ the Scraper Queen

fn queen(story: bool, n: usize, seed: u64) -> (Sim, Vec<EntityId>, EntityId) {
    let classes = ["backend", "devops", "fullstack", "frontend"];
    let (mut sim, ids) = wing(n, story, &classes, seed);
    load(&mut sim, "ilmari3/1");
    let boss = unit_of(&sim, "il_scraper_queen").expect("boss");
    pull(&mut sim, &ids, boss);
    (sim, ids, boss)
}

#[test]
fn the_queen_arms_five_drafts_and_a_wave_comes_in_the_authored_size() {
    for (story, n, clusters, drones) in [(false, 3, 8, 125), (true, 1, 6, 55)] {
        let (mut sim, _, _) = queen(story, n, 1);
        run(&mut sim, 8, "il_scraper_queen");
        assert_eq!(sim.drafts().len(), 5, "five drafts");
        assert!(sim.drafts().iter().all(|d| d.integrity <= 1000));
        let c = sim.swarm_clusters();
        assert_eq!(c.len(), clusters, "story {story}: {c:?}");
        assert!(c.iter().all(|&(_, d)| d <= drones));
        let total: u32 = c.iter().map(|&(_, d)| u32::from(d)).sum();
        assert!(total >= (clusters as u32 - 1) * u32::from(drones));
    }
}

#[test]
fn a_kill_or_a_reset_takes_the_swarm_and_the_drafts_with_it() {
    let (mut sim, _, boss) = queen(false, 3, 2);
    run(&mut sim, 8, "il_scraper_queen");
    assert!(!sim.swarm_clusters().is_empty());
    assert!(sim.debug_kill(boss));
    sim.tick();
    sim.tick();
    assert!(sim.swarm_clusters().is_empty(), "clusters despawn");
    assert!(sim.drafts().is_empty(), "drafts are forgotten");
    assert!(sim.wing3_boss().is_none());
}

#[test]
fn a_lost_draft_heals_the_queen_teaches_her_and_three_wipe_the_room_on_full() {
    let (mut sim, ids, boss) = queen(false, 3, 3);
    run(&mut sim, 2, "il_scraper_queen");
    let max = sim.unit(boss).expect("boss").max_hp;
    sim.debug_set_hp(boss, max / 2);
    assert!(sim.debug_set_draft(0, 0));
    sim.tick();
    sim.tick();
    let hp = sim.unit(boss).expect("boss").hp;
    assert!(hp >= max / 2 + max / 25, "5% heal: {hp} of {max}");
    assert_eq!(
        sim.unit(boss).and_then(|u| u
            .auras
            .iter()
            .find(|a| a.key == "il_well_trained")
            .map(|a| a.stacks)),
        Some(1)
    );
    assert!(ids.iter().all(|&p| sim.unit(p).is_some_and(|u| u.alive)));
    assert!(sim.debug_set_draft(1, 0));
    sim.tick();
    assert_eq!(
        sim.unit(boss).and_then(|u| u
            .auras
            .iter()
            .find(|a| a.key == "il_well_trained")
            .map(|a| a.stacks)),
        Some(2)
    );
    assert!(ids.iter().all(|&p| sim.unit(p).is_some_and(|u| u.alive)));
    // The third one is the one that counts.
    assert!(sim.debug_set_draft(2, 0));
    for _ in 0..5 {
        sim.tick();
    }
    assert!(
        ids.iter().all(|&p| sim.unit(p).is_some_and(|u| !u.alive)),
        "we learned enough: the room wipes"
    );
}

#[test]
fn story_needs_all_five_lost_to_wipe() {
    let (mut sim, ids, _) = queen(true, 1, 4);
    run(&mut sim, 2, "il_scraper_queen");
    for k in 0..4 {
        assert!(sim.debug_set_draft(k, 0));
    }
    for _ in 0..5 {
        sim.tick();
    }
    assert!(
        sim.unit(ids[0]).is_some_and(|u| u.alive),
        "four of five: fine"
    );
    assert!(sim.debug_set_draft(4, 0));
    for _ in 0..5 {
        sim.tick();
    }
    assert!(
        sim.unit(ids[0]).is_some_and(|u| !u.alive),
        "all five: wiped"
    );
}

#[test]
fn mark_confidential_is_for_any_class_pauses_the_bites_and_waits_forty_seconds_per_draft() {
    let (mut sim, ids, _) = queen(false, 3, 5);
    run(&mut sim, 1, "il_scraper_queen");
    // One cluster on top of the first draft, eating.
    let at = sim.drafts()[0].pos;
    let c = sim
        .spawn_swarm("il_scraper_cluster", Vec3::new(at.x + 1.0, 0.0, at.z), 100)
        .expect("cluster");
    let ev = interact(&mut sim, ids[1], "il_draft_1");
    assert_eq!(
        outcome(&ev, "il_draft_1"),
        Some(PROP_OK),
        "a healer may too"
    );
    let before = sim.drafts()[0].integrity;
    run(&mut sim, 6, "il_scraper_queen");
    assert_eq!(sim.drafts()[0].integrity, before, "no bites while marked");
    // A second player right away: the draft's own cooldown, shared by everyone.
    let ev = interact(&mut sim, ids[2], "il_draft_1");
    assert_eq!(outcome(&ev, "il_draft_1"), Some(0));
    run(&mut sim, 7, "il_scraper_queen");
    assert!(
        sim.drafts()[0].integrity < before,
        "the 12 s ran out, the bites are back"
    );
    // Forty seconds after the first mark it works again; another draft always did.
    let ev = interact(&mut sim, ids[0], "il_draft_2");
    assert_eq!(outcome(&ev, "il_draft_2"), Some(PROP_OK));
    run(&mut sim, 30, "il_scraper_queen");
    let ev = interact(&mut sim, ids[0], "il_draft_1");
    assert_eq!(outcome(&ev, "il_draft_1"), Some(PROP_OK));
    let _ = c;
}

#[test]
fn drafts_are_sent_to_the_clients_and_to_a_joiner() {
    let (mut sim, ids, _) = queen(false, 3, 6);
    let ev = run(&mut sim, 3, "il_scraper_queen");
    let sent: Vec<&Event> = ev
        .iter()
        .filter(|e| matches!(e, Event::Drafts { .. }))
        .collect();
    assert!(!sent.is_empty(), "the drafts are announced");
    let Some(Event::Drafts { integrity, marked }) = sent.last() else {
        panic!("a Drafts event")
    };
    assert_eq!((integrity.len(), marked.len()), (5, 5));
    let _ = ids;
}

#[test]
fn the_proxies_blink_between_three_alcoves_and_an_interrupt_stops_the_shield() {
    let (mut sim, _, boss) = queen(false, 3, 7);
    let max = sim.unit(boss).expect("boss").max_hp;
    // Phase two: below half.
    sim.debug_set_hp(boss, max * 40 / 100);
    let home = sim.unit(boss).expect("boss").pos;
    let mut seen = Vec::new();
    for _ in 0..70 * SECOND {
        sim.tick();
        sim.debug_set_hp(boss, max * 40 / 100);
        let u = sim.unit(boss).expect("boss");
        let moved = (u.pos.x - home.x).hypot(u.pos.z - home.z);
        if moved > 8.0
            && !seen
                .iter()
                .any(|p: &Vec3| (p.x - u.pos.x).hypot(p.z - u.pos.z) < 3.0)
        {
            seen.push(u.pos);
        }
    }
    assert!(seen.len() >= 2, "she appeared at several alcoves: {seen:?}");
    assert!(
        seen.iter()
            .all(|p| (p.x - home.x).hypot(p.z - home.z) < 20.0),
        "all of them inside the pit"
    );
}

// ------------------------------------------------------------------ the Hallucination

fn hallucination(story: bool, n: usize, seed: u64) -> (Sim, Vec<EntityId>, EntityId) {
    let classes = ["backend", "devops", "fullstack", "frontend"];
    let (mut sim, ids) = wing(n, story, &classes, seed);
    load(&mut sim, "ilmari3/2");
    let boss = unit_of(&sim, "il_hallucination").expect("boss");
    pull(&mut sim, &ids, boss);
    (sim, ids, boss)
}

fn cycles(events: &[Event]) -> Vec<(u32, u64, combat_core::illusion::IllusionArena)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::IllusionCycle { cycle, salt, arena } => Some((*cycle, *salt, *arena)),
            _ => None,
        })
        .collect()
}

#[test]
fn confabulate_starts_a_cycle_every_twenty_seconds_and_story_every_twenty_four() {
    let (mut sim, _, _) = hallucination(false, 3, 1);
    let full = cycles(&run(&mut sim, 70, "il_hallucination"));
    assert_eq!(full.len(), 4, "Full: {full:?}");
    let (mut sim, _, _) = hallucination(true, 1, 1);
    let story = cycles(&run(&mut sim, 66, "il_hallucination"));
    assert_eq!(story.len(), 3, "Story: {story:?}");
    // The grid is the 3 x 3 of 8 m tiles round the boss's spawn point.
    assert_eq!(full[0].2.tile_cm, 800);
    assert_eq!((full[0].2.x_cm, full[0].2.z_cm), (0, -3000));
}

#[test]
fn the_real_hazards_hurt_whoever_stands_in_them_and_a_fake_never_does() {
    let (mut sim, ids, _) = hallucination(false, 2, 2);
    let ev = run(&mut sim, 9, "il_hallucination");
    let (cycle, salt, arena) = *cycles(&ev).first().expect("a cycle in 5 s");
    let real = real_set(salt, &arena);
    let cm = |v: i64| v as f32 / 100.0;
    // Player 0 on the middle of a real circle, player 1 on a fake of their own that no real
    // hazard covers.
    let c = real.circles[0];
    // A fake centre when one sits clear of the truth, else any safe spot of the grid.
    let mine = layout(salt, ids[1], cycle, &arena);
    let safe = mine
        .fakes
        .iter()
        .map(|f| (f.x_cm, f.z_cm))
        .chain((-12..=12).flat_map(|i| {
            (-12..=12).map(move |j| (i * 100 + arena.x_cm as i64, j * 100 + arena.z_cm as i64))
        }))
        .find(|&(x, z)| !real.hits(&arena, cm(x), cm(z), 1.0))
        .expect("a spot the truth does not reach");
    let fake = Circle {
        x_cm: safe.0,
        z_cm: safe.1,
        r_cm: 0,
    };
    // Move after the cast ends (the 3 s cast of the next cycle is not what we test).
    let (hp0, hp1) = (
        sim.unit(ids[0]).expect("p0").hp,
        sim.unit(ids[1]).expect("p1").hp,
    );
    let mut damage = Vec::new();
    for _ in 0..8 * SECOND {
        move_to(&mut sim, ids[0], Vec3::new(cm(c.x_cm), 0.0, cm(c.z_cm)));
        move_to(
            &mut sim,
            ids[1],
            Vec3::new(cm(fake.x_cm), 0.0, cm(fake.z_cm)),
        );
        for e in sim.tick() {
            if let Event::Damage {
                source,
                target,
                spell: None,
                amount,
                school: School::Arcane,
                ..
            } = e
                && ids.contains(&target)
                && Some(source) == unit_of(&sim, "il_hallucination")
            {
                damage.push((target, amount));
            }
        }
        keep(&mut sim, "il_hallucination");
    }
    assert!(
        damage.iter().any(|d| d.0 == ids[0]),
        "the real circle landed on its player: {damage:?} (hp {hp0} -> {:?})",
        sim.unit(ids[0]).map(|u| u.hp)
    );
    assert!(
        damage.iter().all(|d| d.0 != ids[1]),
        "a fake never hurts: {damage:?} ({hp1})"
    );
}

#[test]
fn fakes_differ_per_player_and_never_reach_the_sim() {
    let (mut sim, ids, _) = hallucination(false, 3, 3);
    let ev = run(&mut sim, 9, "il_hallucination");
    let (cycle, salt, arena) = *cycles(&ev).first().expect("a cycle");
    let a = layout(salt, ids[0], cycle, &arena);
    let b = layout(salt, ids[1], cycle, &arena);
    let c = layout(salt, ids[2], cycle, &arena);
    assert!(a != b && b != c && a != c, "nobody sees the same fakes");
    // The sim holds one truth: the real set, the same for every player.
    assert_eq!(sim.illusion_real(), Some(real_set(salt, &arena)));
    assert_eq!(sim.illusion_cycle().map(|c| c.1), Some(salt));
}

#[test]
fn a_ground_truth_board_grounds_its_user_for_eight_seconds_and_waits_twenty_five() {
    let (mut sim, ids, _) = hallucination(false, 2, 4);
    // No cycle yet: the board has nothing to check.
    let ev = interact(&mut sim, ids[0], "il_truth_board_1");
    assert_eq!(outcome(&ev, "il_truth_board_1"), Some(0));
    assert!(!auras(&sim, ids[0]).contains(&"il_grounded".to_string()));
    // The first cycle opens about 7 s in (4 s opener plus the 3 s cast).
    for _ in 0..12 * SECOND {
        sim.tick();
        keep(&mut sim, "il_hallucination");
        if sim.illusion_cycle().is_some() {
            break;
        }
    }
    assert!(sim.illusion_cycle().is_some());
    let ev = interact(&mut sim, ids[0], "il_truth_board_1");
    assert_eq!(outcome(&ev, "il_truth_board_1"), Some(PROP_OK));
    let left = sim
        .unit(ids[0])
        .and_then(|u| {
            u.auras
                .iter()
                .find(|a| a.key == "il_grounded")
                .map(|a| a.remaining_ms)
        })
        .expect("grounded");
    assert!(left > 7000 && left <= 8000, "{left}");
    assert!(
        !auras(&sim, ids[1]).contains(&"il_grounded".to_string()),
        "only the user"
    );
    // A second use inside the 25 s does nothing; the other board is the other player's.
    run(&mut sim, 9, "il_hallucination");
    assert!(!auras(&sim, ids[0]).contains(&"il_grounded".to_string()));
    let ev = interact(&mut sim, ids[0], "il_truth_board_1");
    assert_ne!(
        outcome(&ev, "il_truth_board_1"),
        Some(PROP_OK),
        "still cooling down"
    );
}

#[test]
fn the_data_scientist_grounds_the_group_within_twenty_metres_and_only_in_a_wing() {
    let (mut sim, ids) = wing(3, false, &["data_scientist", "backend", "frontend"], 5);
    let ds = ids[0];
    let c = sim.content().clone();
    let class = c
        .classes
        .iter()
        .find(|k| k.key == "data_scientist")
        .expect("class");
    let ground_truth = class.utility.expect("a utility spell");
    assert_eq!(ground_truth, SpellId(2500));
    // Out in the campus it does not work.
    sim.apply_as(
        ds,
        Command::Cast {
            caster: ds,
            spell: ground_truth,
        },
    );
    let ev = sim.tick();
    assert!(
        ev.iter().any(|e| matches!(
            e,
            Event::CastFailed {
                reason: CastError::NotHere,
                ..
            }
        )),
        "{ev:?}"
    );
    load(&mut sim, "ilmari3/2");
    move_to(&mut sim, ds, Vec3::new(0.0, 0.0, 0.0));
    move_to(&mut sim, ids[1], Vec3::new(10.0, 0.0, 0.0));
    move_to(&mut sim, ids[2], Vec3::new(30.0, 0.0, 0.0));
    sim.apply_as(
        ds,
        Command::Cast {
            caster: ds,
            spell: ground_truth,
        },
    );
    for _ in 0..3 {
        sim.tick();
    }
    assert!(auras(&sim, ds).contains(&"il_grounded".to_string()));
    assert!(
        auras(&sim, ids[1]).contains(&"il_grounded".to_string()),
        "10 m"
    );
    assert!(
        !auras(&sim, ids[2]).contains(&"il_grounded".to_string()),
        "30 m is out of range"
    );
}

#[test]
fn source_needed_makes_three_footnotes_one_real_and_citing_it_hurts_the_boss() {
    let (mut sim, ids, boss) = hallucination(false, 3, 6);
    let max = sim.unit(boss).expect("boss").max_hp;
    let mut footnotes = Vec::new();
    for _ in 0..40 * SECOND {
        sim.tick();
        sim.debug_set_hp(boss, max * 50 / 100);
        footnotes = sim
            .units()
            .filter(|u| u.key.starts_with("il_footnote") && u.alive)
            .map(|u| (u.id, u.key.clone(), u.pos))
            .collect();
        if footnotes.len() == 3 {
            break;
        }
    }
    assert_eq!(footnotes.len(), 3, "three footnotes in phase two");
    let real: Vec<_> = footnotes
        .iter()
        .filter(|f| f.1 == "il_footnote_real")
        .collect();
    assert_eq!(real.len(), 1, "exactly one is the citation");
    // A fake retracts on whoever stands by it.
    let fake = footnotes
        .iter()
        .find(|f| f.1 == "il_footnote")
        .expect("a fake");
    move_to(&mut sim, ids[0], Vec3::new(fake.2.x + 1.0, 0.0, fake.2.z));
    let hp = sim.unit(ids[0]).expect("p").hp;
    assert!(sim.debug_kill(fake.0));
    sim.tick();
    sim.tick();
    assert!(sim.unit(ids[0]).expect("p").hp < hp, "the retraction hit");
    // The real one is verified: the boss takes more damage from then on.
    assert!(sim.debug_kill(real[0].0));
    sim.tick();
    sim.tick();
    assert!(auras(&sim, boss).contains(&"il_cited".to_string()));
}

#[test]
fn companions_stand_in_the_truth_when_the_hazards_land() {
    use combat_core::companion::{DALE, RUPERT};
    let (mut sim, ids) = wing(1, true, &["fullstack"], 8);
    sim.set_companion_picks(ids[0], &[DALE.to_string(), RUPERT.to_string()]);
    load(&mut sim, "ilmari3/2");
    let boss = unit_of(&sim, "il_hallucination").expect("boss");
    pull(&mut sim, &ids, boss);
    let comps = sim.companions_of(ids[0]);
    assert_eq!(comps.len(), 2);
    let mut landed = 0;
    for _ in 0..110 * SECOND {
        for e in sim.tick() {
            if let Event::Damage {
                target,
                spell: None,
                school: School::Arcane,
                ..
            } = e
                && comps.contains(&target)
            {
                landed += 1;
            }
        }
        keep(&mut sim, "il_hallucination");
        sim.debug_set_hp(ids[0], u32::MAX / 2);
    }
    assert_eq!(
        landed, 0,
        "companions read the sim and never stand in a real hazard"
    );
}

// ------------------------------------------------------------------ Model Collapse

fn collapse(story: bool, n: usize, seed: u64) -> (Sim, Vec<EntityId>, EntityId) {
    let classes = ["fullstack", "backend", "devops", "frontend"];
    let (mut sim, ids) = wing(n, story, &classes, seed);
    load(&mut sim, "ilmari3/3");
    let boss = unit_of(&sim, "il_model_collapse").expect("boss");
    pull(&mut sim, &ids, boss);
    (sim, ids, boss)
}

#[test]
fn the_recorder_is_armed_by_the_pull_and_only_players_feed_it() {
    use combat_core::companion::{DALE, RUPERT};
    let (mut sim, ids) = wing(1, true, &["fullstack"], 9);
    sim.set_companion_picks(ids[0], &[DALE.to_string(), RUPERT.to_string()]);
    load(&mut sim, "ilmari3/3");
    let boss = unit_of(&sim, "il_model_collapse").expect("boss");
    assert!(sim.wing3_boss().is_none(), "nothing armed before the pull");
    pull(&mut sim, &ids, boss);
    run(&mut sim, 20, "il_model_collapse");
    assert_eq!(sim.wing3_boss(), Some(boss));
    let log = sim.echo_log();
    assert!(!log.is_empty(), "the player's casts are recorded");
    assert!(
        log.iter().all(|e| ids.contains(&e.player)),
        "companions never feed the log"
    );
    // A wipe or a kill clears it.
    assert!(sim.debug_kill(boss));
    sim.tick();
    sim.tick();
    assert!(sim.echo_log().is_empty());
}

#[test]
fn a_recorded_cast_comes_back_after_the_delay_with_a_ghost_three_seconds_before() {
    for (story, delay, warn) in [(false, 60, 3), (true, 45, 5)] {
        let n = if story { 1 } else { 3 };
        let (mut sim, ids, _) = collapse(story, n, 10);
        let mut first_cast: Option<u64> = None;
        let mut ghost: Option<(u64, EntityId)> = None;
        for _ in 0..(delay + 12) * SECOND {
            let t = sim.now().0;
            for e in sim.tick() {
                match e {
                    Event::CastStarted { caster, .. } if ids.contains(&caster) => {
                        first_cast.get_or_insert(t);
                    }
                    Event::Echoed { as_player, .. } => {
                        ghost.get_or_insert((t, as_player));
                    }
                    _ => {}
                }
            }
            keep(&mut sim, "il_model_collapse");
        }
        let (t, who) = ghost.expect("an echo was announced");
        let first = first_cast.expect("someone cast");
        assert!(ids.contains(&who));
        let after = (t - first) / u64::from(SECOND);
        assert!(
            after >= u64::from(delay - warn - 1) && after <= u64::from(delay - warn + 6),
            "story {story}: the ghost appears {after} s after the first cast"
        );
    }
}

#[test]
fn synthetic_data_copies_a_player_by_name_and_the_copy_clocks_out() {
    let (mut sim, ids, _) = collapse(false, 3, 11);
    let mut pair = None;
    for _ in 0..30 * SECOND {
        for e in sim.tick() {
            if let Event::Mirrored { mirror, player } = e {
                pair = Some((mirror, player));
            }
        }
        keep(&mut sim, "il_model_collapse");
        if pair.is_some() {
            break;
        }
    }
    let (double, player) = pair.expect("a copy within 30 s");
    assert!(ids.contains(&player));
    assert_eq!(
        sim.unit(double).map(|u| u.key.clone()).as_deref(),
        Some("il_synth_double")
    );
    assert!(auras(&sim, player).contains(&"mirrored".to_string()));
    run(&mut sim, 23, "il_model_collapse");
    assert!(
        sim.unit(double).is_none_or(|u| !u.alive),
        "22 s: clocked out"
    );
}

// ------------------------------------------------------------------ determinism

fn fight_log(zone: &str, boss_key: &str, story: bool, seed: u64) -> String {
    let classes = ["backend", "devops", "fullstack", "frontend"];
    let n = if story { 1 } else { 3 };
    let (mut sim, ids) = wing(n, story, &classes, seed);
    load(&mut sim, zone);
    let boss = unit_of(&sim, boss_key).expect("boss");
    pull(&mut sim, &ids, boss);
    let mut log = String::new();
    for _ in 0..80 * SECOND {
        let t = sim.now().0;
        for e in sim.tick() {
            log.push_str(&log_line(&sim, t, &e));
            log.push('\n');
        }
        keep(&mut sim, boss_key);
    }
    log
}

#[test]
fn the_three_fights_replay_bit_identically() {
    for (zone, boss) in [
        ("ilmari3/1", "il_scraper_queen"),
        ("ilmari3/2", "il_hallucination"),
        ("ilmari3/3", "il_model_collapse"),
    ] {
        for story in [false, true] {
            let a = fight_log(zone, boss, story, 21);
            let b = fight_log(zone, boss, story, 21);
            assert!(a.len() > 500, "{boss}: a real fight");
            assert!(a == b, "{boss} story {story}: two runs differ");
        }
    }
}

// ------------------------------------------------------------------ the wing

#[test]
fn the_wing_door_waits_for_the_scraper_and_each_boss_has_three_own_pieces() {
    let c = content();
    let door = c
        .zone("campus_occupied")
        .and_then(|z| z.links.iter().find(|l| l.to == "ilmari3/1"))
        .expect("door");
    assert!(matches!(&door.requires, Some(Unlock::QuestDone(q)) if q == "il_08_wing3_attune"));
    let w3 = c.raid_of("ilmari3/1").expect("wing");
    assert!(w3.ilmari && !w3.is_dungeon());
    assert_eq!(
        w3.bosses(&c),
        vec!["il_scraper_queen", "il_hallucination", "il_model_collapse"]
    );
    let q = |id: &str| c.quest(id).unwrap_or_else(|| panic!("{id}"));
    assert_eq!(q("il_07_lab_intel").requires.as_deref(), Some("il_w2_done"));
    assert_eq!(
        q("il_08_wing3_attune").requires.as_deref(),
        Some("il_07_lab_intel")
    );
    assert_eq!(
        q("il_w3_done").requires.as_deref(),
        Some("il_08_wing3_attune")
    );
    for boss in w3.bosses(&c) {
        let own: Vec<_> = c
            .economy
            .gear
            .iter()
            .filter(|g| g.drops_from.as_deref() == Some(boss.as_str()))
            .collect();
        assert_eq!(own.len(), 3, "{boss}");
        assert!(own.iter().all(|g| g.source == GearSource::Ilmari));
        // Wing 3 tops out at 18: 20 stays reserved for the 1.0 core.
        assert!(own.iter().all(|g| (16..=18).contains(&g.ilvl)), "{boss}");
    }
}

#[test]
fn every_class_has_a_recordable_kit_and_only_the_data_scientist_has_ground_truth() {
    let c = content();
    for class in &c.classes {
        let has = class.utility.is_some();
        assert_eq!(has, class.key == "data_scientist", "{}", class.key);
    }
    let gt = c.spell(SpellId(2500)).expect("Ground Truth");
    assert!(gt.ilmari_only);
}

#[test]
fn the_wing_locks_per_difficulty_and_titles_follow_the_bosses() {
    for (story, key_full, boss_zone, boss) in [
        (true, "il_w3_queen", "ilmari3/1", "il_scraper_queen"),
        (false, "il_w3_queen", "ilmari3/1", "il_scraper_queen"),
        (true, "il_w3_hallucination", "ilmari3/2", "il_hallucination"),
        (true, "il_w3_collapse", "ilmari3/3", "il_model_collapse"),
        (false, "il_w3_finetuned", "ilmari3/3", "il_model_collapse"),
    ] {
        let classes = ["backend", "fullstack", "frontend"];
        let (mut sim, ids) = wing(if story { 1 } else { 3 }, story, &classes, 33);
        load(&mut sim, boss_zone);
        let id = unit_of(&sim, boss).expect("boss");
        pull(&mut sim, &ids, id);
        assert!(sim.debug_kill(id));
        sim.tick();
        sim.tick();
        assert!(sim.raid_locked(boss, story));
        assert!(
            !sim.raid_locked(boss, !story),
            "each difficulty locks apart"
        );
        assert!(
            sim.unit(ids[0])
                .is_some_and(|u| u.achievements.contains(&key_full.to_string())),
            "{key_full} (story {story})"
        );
    }
    // The wing's last title is Full's alone.
    let (mut sim, ids) = wing(1, true, &["fullstack"], 34);
    load(&mut sim, "ilmari3/3");
    let id = unit_of(&sim, "il_model_collapse").expect("boss");
    pull(&mut sim, &ids, id);
    assert!(sim.debug_kill(id));
    sim.tick();
    sim.tick();
    assert!(
        !sim.unit(ids[0])
            .is_some_and(|u| u.achievements.contains(&"il_w3_finetuned".to_string()))
    );
}

#[test]
fn a_boss_drops_only_its_own_pieces_and_pays_tokens() {
    let c = content();
    for (story, tokens) in [(true, 8), (false, 16)] {
        let classes = ["backend", "fullstack", "frontend"];
        let (mut sim, ids) = wing(if story { 1 } else { 3 }, story, &classes, 35);
        load(&mut sim, "ilmari3/2");
        let boss = unit_of(&sim, "il_hallucination").expect("boss");
        pull(&mut sim, &ids, boss);
        assert!(sim.debug_kill(boss));
        let mut pieces = Vec::new();
        for e in sim.tick() {
            if let Event::LootDropped { items, .. } = e {
                pieces.extend(items);
            }
        }
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
            assert_eq!(g.drops_from.as_deref(), Some("il_hallucination"), "{p}");
        }
    }
}
