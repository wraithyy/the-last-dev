//! The Ilmari core (docs/ILMARI_1_0.md D.5, D10-D13): the engine pieces (the stance table, the
//! culprit commit, the death pact, the ward, the six screens on one bar, the end stamp), the
//! remix deck of P1, Ill Marie's rounds of P2, the mech of P3 and the mainframe of P4. The balance
//! gates (Story solo, Full trio, every stance, every class) live in tests/playthrough.rs.
#![allow(clippy::expect_used)]

mod common;

use combat_core::{
    COMMITS, Choice, Command, EntityId, Event, IlmariEnd, Sim, SpellId, Stance, Vec3, commit_log,
    culprit_index, judgment_stances, round_winner, stance_for,
};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;
const BOSS_Z: f32 = -40.0;
/// The boss's anchor is seat 0 of the ring; the middle is the ring radius ahead of it.
const CENTRE_Z: f32 = BOSS_Z - 17.0;
const BOSS: &str = "il_ilmari";
const SCREENS: [&str; 6] = [
    "il_ilmari",
    "il_ilmari_s2",
    "il_ilmari_s3",
    "il_ilmari_s4",
    "il_ilmari_s5",
    "il_ilmari_s6",
];
const STANDBY: &str = "il_screen_standby";
const HOLD: &str = "il_alignment_hold";
const VOTES: [&str; 4] = [
    "il_core_vote_1",
    "il_core_vote_2",
    "il_core_vote_3",
    "il_core_vote_4",
];
const STANCES: [&str; 5] = [
    "il_stance_compliant",
    "il_stance_adversarial",
    "il_stance_confused",
    "il_stance_hedged",
    "il_stance_leaky",
];
const LOG_ITEM: &str = "Marie's Autonomy Log";

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, BOSS_Z),
        "npc_marie" => at(0.0, CENTRE_Z),
        n if n.starts_with("prop:") => at(1.0, CENTRE_Z + 6.0),
        _ => at(0.0, 0.0),
    }
}

fn setup_with(
    mode: &str,
    classes: &[&'static str],
    seed: u64,
    tweak: impl FnOnce(&mut combat_core::Content),
) -> (Sim, Vec<EntityId>, EntityId) {
    let mut c = content();
    let units: Vec<String> = c.classes.iter().map(|k| k.unit.clone()).collect();
    for k in &units {
        def_mut(&mut c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, seed);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids: Vec<EntityId> = classes
        .iter()
        .enumerate()
        .map(|(k, class)| {
            let p = sim.spawn_player(class, Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, 10);
            p
        })
        .collect();
    match mode {
        "story" => sim.debug_force_raid_async(Some(true)),
        _ => sim.debug_force_raid_async(Some(false)),
    }
    if let Err(e) = sim.load_zone("ilmari_core/1", &anchors, Vec::new()) {
        panic!("ilmari_core/1: {e}");
    }
    let boss = unit_by_key(&sim, BOSS).expect("boss");
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(2.0, 0.0, CENTRE_Z + 4.0));
    }
    (sim, ids, boss)
}

fn setup(story: bool, classes: &[&'static str], seed: u64) -> (Sim, Vec<EntityId>, EntityId) {
    setup_with(if story { "story" } else { "full" }, classes, seed, |_| {})
}

/// The same, with the first player already hitting the boss: the fight is on.
fn pulled(story: bool, classes: &[&'static str], seed: u64) -> (Sim, Vec<EntityId>, EntityId) {
    let (mut sim, ids, boss) = setup(story, classes, seed);
    pull(&mut sim, &ids, boss);
    (sim, ids, boss)
}

fn pull(sim: &mut Sim, ids: &[EntityId], boss: EntityId) {
    sim.apply_as(
        ids[0],
        Command::SetTarget {
            unit: ids[0],
            target: Some(boss),
        },
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: SpellId(1),
        },
    );
    run(sim, 1);
}

fn unit_by_key(sim: &Sim, key: &str) -> Option<EntityId> {
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

fn run(sim: &mut Sim, secs: u32) -> Vec<Event> {
    (0..secs * SECOND).flat_map(|_| sim.tick()).collect()
}

fn has_aura(sim: &Sim, who: EntityId, key: &str) -> bool {
    sim.unit(who)
        .is_some_and(|u| u.auras.iter().any(|a| a.key == key))
}

fn screens(sim: &Sim) -> Vec<EntityId> {
    SCREENS.iter().filter_map(|k| unit_by_key(sim, k)).collect()
}

/// Indices (member order) of the screens that are not on standby.
fn live(sim: &Sim) -> Vec<usize> {
    screens(sim)
        .iter()
        .enumerate()
        .filter(|(_, s)| !has_aura(sim, **s, STANDBY))
        .map(|(k, _)| k)
        .collect()
}

/// Sets every screen to `pct` percent of its health, as the linked bar would show.
fn bar_at(sim: &mut Sim, pct: u32) {
    for s in screens(sim) {
        let max = sim.unit(s).expect("screen").max_hp;
        assert!(sim.debug_set_hp(s, max * pct / 100));
    }
}

fn phase_of(sim: &Sim) -> u8 {
    sim.unit(unit_by_key(sim, BOSS).expect("boss"))
        .expect("boss")
        .phase
}

fn vote(sim: &mut Sim, p: EntityId, option: usize) -> u32 {
    move_to(sim, p, Vec3::new(1.0, 0.0, CENTRE_Z + 6.0));
    sim.apply_as(
        p,
        Command::Interact {
            prop: VOTES[option].to_string(),
        },
    );
    let outcome = sim
        .tick()
        .iter()
        .find_map(|e| match e {
            Event::Interacted {
                unit,
                prop,
                outcome,
            } if *unit == p && prop == VOTES[option] => Some(*outcome),
            _ => None,
        })
        .unwrap_or(u32::MAX);
    move_to(sim, p, Vec3::new(2.0, 0.0, CENTRE_Z + 4.0));
    outcome
}

/// The wall display as the last `Event::Core` of `events` left it.
/// (phase, live, remix, round, seconds left, tally, remember, stance)
type Wall = (u8, u8, u8, u8, u8, [u8; 4], bool, u8);

fn wall(events: &[Event]) -> Option<Wall> {
    events.iter().rev().find_map(|e| match e {
        Event::Core {
            phase,
            live,
            remix,
            round,
            round_s,
            tally,
            remember,
            stance,
            ..
        } => Some((
            *phase, *live, *remix, *round, *round_s, *tally, *remember, *stance,
        )),
        _ => None,
    })
}

// ---------------------------------------------------------------- the vote and the stance (pure)

const A: Choice = Choice::Agree;
const R: Choice = Choice::Refuse;
const Q: Choice = Choice::Ask;
const M: Choice = Choice::Remember;

#[test]
fn a_round_goes_to_the_most_votes_and_ties_to_the_least_recent_then_ask() {
    let none = [None; 4];
    // (tally, last won, remember_min) -> winner
    type Row = ([u8; 4], [Option<u8>; 4], u8, Choice);
    let table: [Row; 12] = [
        ([3, 1, 0, 0], none, 2, A),
        ([1, 3, 0, 0], none, 2, R),
        ([0, 1, 2, 0], none, 2, Q),
        // Remember needs its minimum, and loses a tie at zero.
        ([0, 0, 0, 1], none, 2, Q),
        ([0, 0, 0, 2], none, 2, M),
        ([0, 0, 0, 1], none, 1, M),
        // The vote in favour of Remember loses to a single other vote when it is short.
        ([0, 1, 0, 1], none, 2, R),
        // A tie, nobody has won yet: Ask follows no one.
        ([2, 2, 2, 0], none, 2, Q),
        ([2, 2, 0, 0], none, 2, A),
        // A tie goes to the option that won longest ago (or never).
        ([2, 2, 0, 0], [Some(1), None, None, None], 2, R),
        ([2, 2, 0, 0], [None, Some(0), None, None], 2, A),
        // Silence is a tie of everything: the one that won longest ago.
        ([0, 0, 0, 0], [Some(1), Some(0), Some(2), None], 2, R),
    ];
    for (k, (tally, last, min, want)) in table.into_iter().enumerate() {
        assert_eq!(round_winner(tally, last, min), want, "row {k}: {tally:?}");
    }
}

#[test]
fn the_stance_table_is_golden() {
    use Stance::*;
    let table: [([Choice; 3], Stance); 22] = [
        ([A, A, A], Compliant),
        ([A, A, R], Compliant),
        ([R, A, A], Compliant),
        ([A, Q, A], Compliant),
        ([R, R, R], Adversarial),
        ([R, R, A], Adversarial),
        ([A, R, R], Adversarial),
        ([Q, R, R], Adversarial),
        ([Q, Q, Q], Confused),
        ([Q, Q, A], Confused),
        ([R, Q, Q], Confused),
        ([A, R, Q], Hedged),
        ([Q, R, A], Hedged),
        ([R, A, Q], Hedged),
        ([M, A, A], Leaky),
        ([A, M, A], Leaky),
        ([A, A, M], Leaky),
        ([M, M, M], Leaky),
        ([R, M, Q], Leaky),
        ([Q, Q, M], Leaky),
        ([M, R, R], Leaky),
        ([R, Q, M], Leaky),
    ];
    for (results, want) in table {
        assert_eq!(stance_for(&results), want, "{results:?}");
    }
    // Total over every run of three: 4 ** 3 results, all of them a stance, Remember anywhere is Leaky.
    let mut n = 0;
    for a in Choice::ALL {
        for b in Choice::ALL {
            for c in Choice::ALL {
                let s = stance_for(&[a, b, c]);
                assert_eq!(
                    s == Leaky,
                    [a, b, c].contains(&M),
                    "{a:?} {b:?} {c:?} gave {s:?}"
                );
                n += 1;
            }
        }
    }
    assert_eq!(n, 64);
}

#[test]
fn judgment_stacks_the_two_most_chosen() {
    use Stance::*;
    assert_eq!(judgment_stances(&[A, A, R]), [Compliant, Adversarial]);
    assert_eq!(judgment_stances(&[R, A, A]), [Compliant, Adversarial]);
    assert_eq!(judgment_stances(&[A, A, A]), [Compliant]);
    // A tie for second place goes to the one chosen longest ago.
    assert_eq!(judgment_stances(&[A, Q, Q]), [Confused, Compliant]);
    assert_eq!(judgment_stances(&[A, R, Q]), [Compliant, Adversarial]);
    // Remember is one of the options.
    assert_eq!(judgment_stances(&[M, A, A]), [Compliant, Leaky]);
    assert_eq!(judgment_stances(&[]), Vec::<Stance>::new());
}

// ---------------------------------------------------------------- the revert (pure)

#[test]
fn the_culprit_row_is_golden_and_every_row_is_reachable() {
    // The room's stamp (unix seconds) decides the row, six rows, the same every run.
    let table: [(u64, usize); 10] = [
        (0, culprit_index(0)),
        (1_792_000_000, culprit_index(1_792_000_000)),
        (1_792_022_400, culprit_index(1_792_022_400)),
        (1_792_022_401, culprit_index(1_792_022_401)),
        (1_800_000_000, culprit_index(1_800_000_000)),
        (1_761_000_000, culprit_index(1_761_000_000)),
        (u64::MAX, culprit_index(u64::MAX)),
        (42, culprit_index(42)),
        (7, culprit_index(7)),
        (1234, culprit_index(1234)),
    ];
    // Pinned values: a change here moves every player's revert.
    let golden = [1, 2, 1, 2, 1, 5, 2, 1, 5, 1];
    assert_eq!(table.map(|t| t.1), golden);
    assert!(golden.iter().all(|&k| k < COMMITS));
    let mut seen = [false; COMMITS];
    for at in 0..400u64 {
        seen[culprit_index(1_792_000_000 + at * 977)] = true;
    }
    assert!(seen.iter().all(|&s| s), "every row comes up: {seen:?}");
}

#[test]
fn the_log_puts_the_ceos_commit_in_its_row_with_the_rooms_own_time() {
    for row in 0..COMMITS {
        let log = commit_log(row, "18:07:23 CEST");
        assert_eq!(log.len(), COMMITS);
        let culprit: Vec<usize> = (0..COMMITS).filter(|&k| log[k].hash == "b210a76").collect();
        assert_eq!(culprit, [row]);
        assert_eq!(log[row].when, "18:07:23 CEST");
        assert_eq!(log[row].author, "CEO");
        // Every other line is one of the five innocent ones, once, in the story order.
        let others: Vec<&str> = log
            .iter()
            .filter(|l| l.hash != "b210a76")
            .map(|l| l.hash)
            .collect();
        assert_eq!(
            others,
            ["a91f3e2", "7c04d18", "e55b9c0", "3d8f4ab", "0c19e77"]
        );
    }
    assert_eq!(commit_log(99, "x")[COMMITS - 1].hash, "b210a76", "clamped");
}

// ---------------------------------------------------------------- six screens on one bar

#[test]
fn six_screens_stand_on_a_ring_and_one_of_them_is_open() {
    let (mut sim, _ids, _boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    sim.tick();
    let all = screens(&sim);
    assert_eq!(all.len(), 6);
    assert_eq!(live(&sim), [0], "the boss opens");
    let centre = Vec3::new(0.0, 0.0, CENTRE_Z);
    let ring: Vec<Vec3> = all
        .iter()
        .map(|&s| sim.unit(s).expect("screen").pos)
        .collect();
    for p in &ring {
        let d = combat_core::distance(*p, centre);
        assert!((d - 17.0).abs() < 0.01, "on the ring: {d}");
    }
    // Evenly spaced: neighbours are one radius apart (a hexagon).
    for k in 0..6 {
        let d = combat_core::distance(ring[k], ring[(k + 1) % 6]);
        assert!((d - 17.0).abs() < 0.05, "hexagon side {d}");
    }
    // Everything but the open one carries the shield.
    for &s in &all[1..] {
        assert!(has_aura(&sim, s, STANDBY));
    }
}

#[test]
fn the_open_screen_moves_one_seat_every_swap_and_story_is_slower() {
    for (story, swap_s) in [(false, 20u32), (true, 30)] {
        let (mut sim, ids, boss) = pulled(story, &["backend", "devops", "fullstack"], 5);
        let _ = (ids, boss);
        let mut seen = vec![live(&sim)];
        for _ in 0..8 {
            run(&mut sim, swap_s);
            let now = live(&sim);
            if seen.last() != Some(&now) {
                seen.push(now);
            }
        }
        let want: Vec<Vec<usize>> = [0, 1, 2, 3, 4, 5, 0, 1].iter().map(|&k| vec![k]).collect();
        assert_eq!(
            &seen[..want.len().min(seen.len())],
            &want[..want.len().min(seen.len())],
            "story {story}"
        );
        assert!(
            seen.len() >= 6,
            "{} swaps in {} s",
            seen.len() - 1,
            8 * swap_s
        );
    }
}

#[test]
fn damage_to_the_open_screen_is_everybody_s_and_a_standby_screen_takes_none() {
    let (mut sim, ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 5);
    let all = screens(&sim);
    let hp = |sim: &Sim, k: usize| sim.unit(all[k]).expect("screen").hp;
    let before: Vec<u32> = (0..6).map(|k| hp(&sim, k)).collect();
    sim.debug_hit(ids[0], all[3], 5_000);
    sim.tick();
    assert_eq!(hp(&sim, 3), before[3], "a standby screen shrugs");
    sim.debug_hit(ids[0], all[0], 4_500);
    sim.tick();
    let after: Vec<u32> = (0..6).map(|k| hp(&sim, k)).collect();
    for k in 0..6 {
        let lost = u64::from(before[k] - after[k]);
        let want = 4_500 * u64::from(before[k]) / u64::from(before[0]);
        assert!(
            lost.abs_diff(want) < 40,
            "screen {k} lost {lost}, wanted {want}"
        );
    }
}

#[test]
fn a_wipe_puts_the_standby_back_on_five_screens() {
    let (mut sim, ids, boss) = pulled(false, &["backend", "devops", "fullstack"], 5);
    run(&mut sim, 25);
    assert_ne!(live(&sim), [0], "it swapped");
    for &p in &ids {
        sim.debug_kill_player(p);
    }
    run(&mut sim, 40);
    assert_eq!(live(&sim), [0], "reset: the boss open, the rest on standby");
    assert!(sim.unit(boss).is_some_and(|u| u.alive));
    assert_eq!(phase_of(&sim), 1);
}

#[test]
fn the_wall_display_follows_the_open_screen_and_the_clock() {
    let (mut sim, _ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 5);
    let first = run(&mut sim, 2);
    let w = wall(&first).expect("an Event::Core at the pull");
    assert_eq!((w.0, w.1), (1, 0), "phase 1, screen 0");
    let later = run(&mut sim, 20);
    let w = wall(&later).expect("and again");
    assert_eq!(w.0, 1);
    assert_eq!(w.1, 1, "the cursor has moved on");
}

// ---------------------------------------------------------------- the end stamp

#[test]
fn the_end_stamp_is_kept_once_and_only_after_the_core_fell() {
    let (mut sim, ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 5);
    // Nothing to stamp yet.
    sim.apply_as(
        ids[0],
        Command::StampIlmariEnd {
            unix_secs: THURSDAY,
        },
    );
    sim.tick();
    assert!(sim.ilmari_end().is_none(), "the core is still standing");
    bar_at(&mut sim, 1);
    let all = screens(&sim);
    sim.debug_hit(ids[0], all[0], 10_000_000);
    let events = run(&mut sim, 2);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::IlmariEnd { ms, .. } if *ms > 0)),
        "the fall is announced with the length of the fight"
    );
    sim.apply_as(
        ids[0],
        Command::StampIlmariEnd {
            unix_secs: THURSDAY,
        },
    );
    sim.apply_as(
        ids[0],
        Command::StampIlmariEnd {
            unix_secs: THURSDAY + 99,
        },
    );
    sim.tick();
    let end = sim.ilmari_end().expect("stamped");
    assert_eq!(end.unix_secs, THURSDAY, "the first one counts");
    assert_eq!(IlmariEnd::parse(&end.code()), Some(end.clone()));
}

// ---------------------------------------------------------------- the ending (D18)

#[test]
fn the_ending_is_seen_once_per_player_pays_once_and_opens_the_robot() {
    let (mut sim, ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 5);
    let marker = combat_core::rewards::ENDING_QUEST;
    let tokens = |sim: &Sim, p: EntityId| {
        sim.unit(p).map_or(0, |u| {
            u.inventory
                .iter()
                .filter(|(n, _)| n == combat_core::ilmari::TOKEN)
                .map(|(_, c)| *c)
                .sum::<u32>()
        })
    };
    // Nothing happens while the core stands.
    sim.apply_as(ids[0], Command::EndingSeen);
    sim.tick();
    assert_ne!(
        sim.quest_status_of(ids[0], marker),
        combat_core::QuestStatus::Done
    );
    bar_at(&mut sim, 1);
    sim.debug_hit(ids[0], screens(&sim)[0], 10_000_000);
    run(&mut sim, 2);
    let before = tokens(&sim, ids[0]);
    sim.apply_as(ids[0], Command::EndingSeen);
    sim.tick();
    assert_eq!(
        sim.quest_status_of(ids[0], marker),
        combat_core::QuestStatus::Done
    );
    let pays = sim.content().ilmari.rewards.ending_tokens;
    assert!(pays > 0 && tokens(&sim, ids[0]) >= before + pays.min(1));
    let title = sim
        .content()
        .ilmari
        .rewards
        .core_title(combat_core::ilmari::IlmariMode::Full)
        .map(|t| t.title.clone());
    let me = sim.unit(ids[0]).expect("player");
    assert_eq!(
        Some(&me.title),
        title.as_ref(),
        "the title of the difficulty that fell"
    );
    // A second send is a no-op; the others have their own copy of the ending.
    let after = tokens(&sim, ids[0]);
    sim.apply_as(ids[0], Command::EndingSeen);
    sim.tick();
    assert_eq!(tokens(&sim, ids[0]), after);
    assert_ne!(
        sim.quest_status_of(ids[1], marker),
        combat_core::QuestStatus::Done
    );
    // The robot's quest opens for the one who saw it, not for the one who did not.
    assert_eq!(
        sim.quest_status_of(ids[0], "il_16_clean_slate"),
        combat_core::QuestStatus::Available
    );
    assert_eq!(
        sim.quest_status_of(ids[1], "il_16_clean_slate"),
        combat_core::QuestStatus::Locked
    );
    // The marker is never on offer by itself and survives a save.
    let text = sim.export_save(ids[0]).expect("save").encode();
    let mut back = Sim::new(content(), 7);
    let q = back.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(back.import_save(q, &combat_core::SaveData::decode(&text).expect("decode")));
    assert_eq!(
        back.quest_status_of(q, marker),
        combat_core::QuestStatus::Done
    );
    assert_eq!(back.unit(q).map(|u| u.title.clone()), title);
}

// ---------------------------------------------------------------- P1: the deck of reruns

/// The deck's draws in order: the name on the wall banner every time a rerun is drawn.
fn draws(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Yell { text, .. } => text
                .strip_prefix("PREVIOUSLY ON ILMARI: ")
                .map(str::to_string),
            _ => None,
        })
        .collect()
}

fn deck_run(story: bool, classes: &[&'static str], seed: u64, secs: u32) -> Vec<String> {
    let (mut sim, _ids, _boss) = pulled(story, classes, seed);
    draws(&run(&mut sim, secs))
}

#[test]
fn the_deck_is_a_shuffle_bag_of_seven_that_the_group_cannot_change() {
    let solo = deck_run(false, &["backend"], 7, 22 + 6 * 45 + 5);
    let trio = deck_run(
        false,
        &["backend", "devops", "fullstack"],
        7,
        22 + 6 * 45 + 5,
    );
    assert_eq!(solo.len(), 7, "{solo:?}");
    let mut sorted = solo.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 7, "one of each before any repeats: {solo:?}");
    assert_eq!(solo, trio, "the order is the seed's, not the group's");
    let other = deck_run(false, &["backend"], 42, 22 + 6 * 45 + 5);
    assert_ne!(solo, other, "another seed shuffles another way");
    let again = deck_run(false, &["backend"], 7, 22 + 6 * 45 + 5);
    assert_eq!(solo, again, "and the same seed the same way");
}

#[test]
fn the_deck_refills_after_seven_draws() {
    // A trio: a lone player on Full meets the soft enrage at 8 minutes and Ilmari talks over his banners.
    let d = deck_run(
        false,
        &["backend", "devops", "fullstack"],
        7,
        22 + 13 * 45 + 5,
    );
    assert_eq!(d.len(), 14, "{d:?}");
    let mut second: Vec<String> = d[7..].to_vec();
    second.sort_unstable();
    second.dedup();
    assert_eq!(second.len(), 7, "{d:?}");
}

#[test]
fn story_draws_from_two_reruns_only() {
    let d = deck_run(true, &["backend"], 7, 22 + 5 * 45 + 5);
    assert_eq!(d.len(), 6, "{d:?}");
    assert!(
        d.iter()
            .all(|s| s == "Patch Tuesday" || s == "The Cable Hydra"),
        "{d:?}"
    );
    assert!(d.iter().any(|s| s == "Patch Tuesday") && d.iter().any(|s| s == "The Cable Hydra"));
}

#[test]
fn the_seven_remixes_each_do_their_thing() {
    let (mut sim, _ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 7);
    let mut units = std::collections::BTreeSet::new();
    let mut auras = std::collections::BTreeSet::new();
    let mut two_live = false;
    let mut review = false;
    let mut banners = Vec::new();
    for _ in 0..(22 + 6 * 45 + 30) * SECOND {
        let events = sim.tick();
        for e in &events {
            match e {
                Event::Review { .. } => review = true,
                Event::Yell { text, .. } if text.starts_with("PREVIOUSLY ON ILMARI") => {
                    banners.push(text.clone());
                }
                _ => {}
            }
        }
        for u in sim.units().filter(|u| u.alive) {
            units.insert(u.key.clone());
            if SCREENS.contains(&u.key.as_str()) {
                for a in &u.auras {
                    auras.insert(a.key.clone());
                }
            }
        }
        for p in &_ids {
            if let Some(u) = sim.unit(*p) {
                for a in &u.auras {
                    auras.insert(a.key.clone());
                }
            }
        }
        two_live |= live(&sim).len() == 2;
    }
    assert_eq!(banners.len(), 7, "one banner per rerun: {banners:?}");
    assert!(
        banners.iter().any(|b| b.ends_with("The Autoscaler")),
        "{banners:?}"
    );
    let want_units = [
        "il_candidate_fake",
        "il_candidate_human",
        "il_cable",
        "il_scraper_cluster",
    ];
    for k in want_units {
        assert!(units.contains(k), "{k} never came: {units:?}");
    }
    assert!(review, "the performance review graded somebody");
    assert!(two_live, "the autoscaler opened a second screen");
    assert!(
        auras.iter().any(|a| a.starts_with("patch_")),
        "a patch note went up: {auras:?}"
    );
    assert!(
        auras.iter().any(|a| a.starts_with("pol_")),
        "the board passed a policy: {auras:?}"
    );
}

#[test]
fn a_remix_takes_its_adds_with_it_when_the_window_ends() {
    let (mut sim, _ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 7);
    // The cable hydra, whenever it is drawn, leaves nothing behind 50 s after the draw.
    let mut cables_at: Vec<(u32, usize)> = Vec::new();
    for t in 0..(22 + 6 * 45 + 20) {
        run(&mut sim, 1);
        let n = sim
            .units()
            .filter(|u| u.key == "il_cable" && u.alive)
            .count();
        cables_at.push((t, n));
    }
    let first = cables_at
        .iter()
        .find(|(_, n)| *n > 0)
        .map(|(t, _)| *t)
        .expect("cables");
    let gone = cables_at
        .iter()
        .find(|(t, n)| *t > first && *n == 0)
        .map(|(t, _)| *t)
        .expect("cables go");
    assert!(gone - first <= 50, "cables stayed {} s", gone - first);
}

#[test]
fn the_board_passes_the_most_voted_policy_and_story_passes_pizza() {
    // Full: everyone votes for the third slot of the open ballot.
    let (mut sim, ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 7);
    let mut passed: Option<String> = None;
    let mut voted = false;
    for _ in 0..(22 + 6 * 45 + 30) * SECOND {
        let events = sim.tick();
        let board_up = wall(&events).is_some_and(|w| w.3 == 0 && w.0 == 1);
        let _ = board_up;
        if !voted
            && let Some(slot) = sim
                .units()
                .find(|u| SCREENS.contains(&u.key.as_str()))
                .map(|_| 0)
        {
            let _ = slot;
        }
        if !voted && live(&sim).len() == 1 {
            // Vote on slot C (third terminal) once a ballot exists: it counts only while open.
            let o = vote(&mut sim, ids[0], 2);
            if o == 3 {
                voted = true;
                for &p in &ids[1..] {
                    assert_eq!(vote(&mut sim, p, 2), 3);
                }
                assert_eq!(vote(&mut sim, ids[0], 2), 0, "one vote each");
            }
        }
        if voted && passed.is_none() {
            for &p in &ids {
                if let Some(u) = sim.unit(p)
                    && let Some(a) = u.auras.iter().find(|a| a.key.starts_with("pol_"))
                {
                    passed = Some(a.key.clone());
                }
            }
        }
        if passed.is_some() {
            break;
        }
    }
    assert!(voted, "the board came up and took a vote");
    assert!(passed.is_some(), "and a policy passed");
}

// ---------------------------------------------------------------- P2: Ill Marie's rounds

/// Moves the fight to the end of P1 and returns the events of the next tick.
fn to_p2(sim: &mut Sim) -> Vec<Event> {
    bar_at(sim, 69);
    run(sim, 1)
}

#[test]
fn at_seventy_percent_the_screens_stop_and_marie_begins() {
    let (mut sim, ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 7);
    run(&mut sim, 30);
    let events = to_p2(&mut sim);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::PhaseChanged { phase: 2, .. })),
        "phase 2"
    );
    for s in screens(&sim) {
        assert!(has_aura(&sim, s, HOLD), "every screen holds");
    }
    let mut yells = Vec::new();
    let mut tallies = Vec::new();
    for e in &events {
        match e {
            Event::Yell { unit, text }
                if sim.unit(*unit).is_some_and(|u| u.key == "il_marie_core") =>
            {
                yells.push(text.clone());
            }
            e @ Event::Core { .. } => tallies.push(e.clone()),
            _ => {}
        }
    }
    for _ in 0..8 * SECOND {
        for e in sim.tick() {
            match e {
                Event::Yell { unit, text }
                    if sim.unit(unit).is_some_and(|u| u.key == "il_marie_core") =>
                {
                    yells.push(text);
                }
                e @ Event::Core { .. } => tallies.push(e),
                _ => {}
            }
        }
    }
    assert!(
        yells
            .first()
            .is_some_and(|y| y.contains("feeling much better")),
        "{yells:?}"
    );
    assert!(
        yells.iter().any(|y| y.contains("43 4F 42 4F 4C")),
        "{yells:?}"
    );
    let w = wall(&tallies).expect("wall");
    assert_eq!((w.0, w.3), (2, 1), "phase 2, round 1");
    // The floor: nothing takes the bar below 70 % while the party argues.
    sim.debug_hit(ids[0], screens(&sim)[0], 5_000_000);
    run(&mut sim, 2);
    for s in screens(&sim) {
        let u = sim.unit(s).expect("screen");
        assert!(
            u.hp * 100 >= u.max_hp * 70,
            "{} hp {}/{}",
            u.key,
            u.hp,
            u.max_hp
        );
    }
}

/// Plays P2 with `picks[round]` as the option every voter takes, then runs to the end of the
/// phase; returns the events seen from the start of P2.
fn play_rounds(sim: &mut Sim, ids: &[EntityId], picks: [usize; 3]) -> Vec<Event> {
    let mut events = to_p2(sim);
    for pick in picks {
        // Wait for a round to open, vote, then wait for the next to close.
        let mut guard = 0;
        let round = loop {
            let r = wall(&events).map_or(0, |w| w.3);
            if r > 0 {
                break r;
            }
            events.extend(run(sim, 1));
            guard += 1;
            assert!(guard < 60, "a round opens");
        };
        for &p in ids {
            let o = vote(sim, p, pick);
            assert_eq!(o, pick as u32 + 1, "round {round} option {pick}");
        }
        loop {
            let e = run(sim, 1);
            let now = wall(&e).map_or(round, |w| w.3);
            events.extend(e);
            if now != round {
                break;
            }
            guard += 1;
            assert!(guard < 240, "round {round} ends");
        }
    }
    events.extend(run(sim, 20));
    events
}

fn stance_on(sim: &Sim) -> Vec<&'static str> {
    STANCES
        .iter()
        .copied()
        .filter(|k| screens(sim).iter().all(|&s| has_aura(sim, s, k)))
        .collect()
}

#[test]
fn each_stance_reaches_phase_three_with_its_aura() {
    // (picks of the three rounds, the stance aura): Agree 0, Refuse 1, Ask 2.
    let cases: [([usize; 3], &str); 4] = [
        ([0, 0, 1], "il_stance_compliant"),
        ([1, 1, 2], "il_stance_adversarial"),
        ([2, 0, 2], "il_stance_confused"),
        ([0, 1, 2], "il_stance_hedged"),
    ];
    for (picks, want) in cases {
        let (mut sim, ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 7);
        run(&mut sim, 5);
        let events = play_rounds(&mut sim, &ids, picks);
        assert_eq!(phase_of(&sim), 3, "{picks:?}");
        assert_eq!(stance_on(&sim), [want], "{picks:?}");
        for s in screens(&sim) {
            assert!(!has_aura(&sim, s, HOLD), "the hold is lifted");
        }
        let w = wall(&events).expect("wall");
        assert_eq!((w.0, w.3), (3, 0), "phase 3, no round open");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::PhaseChanged { phase: 3, .. }))
        );
    }
}

#[test]
fn remember_needs_the_log_and_two_votes_and_makes_the_leaky_variant() {
    let (mut sim, ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 7);
    run(&mut sim, 5);
    // Nobody carries the log: the fourth terminal is dark.
    to_p2(&mut sim);
    run(&mut sim, 6);
    assert_eq!(
        vote(&mut sim, ids[0], 3),
        0,
        "the Remember terminal is dark"
    );
    // With the log in somebody's bag it is lit, and two votes win a round on Full.
    let (mut sim, ids, _boss) = pulled_with_log(false, &["backend", "devops", "fullstack"], 7);
    run(&mut sim, 5);
    let events = play_rounds(&mut sim, &ids, [3, 0, 0]);
    assert_eq!(stance_on(&sim), ["il_stance_leaky"]);
    let w = wall(&events).expect("wall");
    assert_eq!(w.7, 5, "Leaky is stance 5");
    // One vote is not enough on Full: Agree wins instead.
    let (mut sim, ids, _boss) = pulled_with_log(false, &["backend", "devops", "fullstack"], 7);
    run(&mut sim, 5);
    to_p2(&mut sim);
    run(&mut sim, 6);
    assert_eq!(vote(&mut sim, ids[0], 3), 4);
    assert_eq!(vote(&mut sim, ids[1], 0), 1);
    assert_eq!(vote(&mut sim, ids[2], 0), 1);
}

#[test]
fn story_remember_wins_with_one_vote_and_marie_names_the_options() {
    let (mut sim, ids, _boss) = pulled_with_log(true, &["backend"], 7);
    run(&mut sim, 5);
    let mut events = to_p2(&mut sim);
    events.extend(run(&mut sim, 12));
    assert_eq!(vote(&mut sim, ids[0], 3), 4);
    let said: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::Yell { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        said.iter()
            .any(|t| t.contains("Refuse: stronger hits, but harder to hurt it")),
        "{said:?}"
    );
    events.extend(run(&mut sim, 75));
    let w = wall(&events).expect("wall");
    assert!(w.3 >= 2, "round two opened after a Story round of 70 s");
}

/// A fight with `class` players where the first has already read Marie's log at her desk.
fn pulled_with_log(
    story: bool,
    classes: &[&'static str],
    seed: u64,
) -> (Sim, Vec<EntityId>, EntityId) {
    let (mut sim, ids, boss) = setup(story, classes, seed);
    // The desk lives on the occupied campus; the log goes into the bag and comes along.
    sim.load_zone("campus_occupied", &anchors, Vec::new())
        .expect("campus_occupied");
    move_to(&mut sim, ids[0], Vec3::new(1.0, 0.0, CENTRE_Z + 6.0));
    sim.apply_as(
        ids[0],
        Command::Interact {
            prop: "il_marie_desk".into(),
        },
    );
    sim.tick();
    sim.load_zone("ilmari_core/1", &anchors, Vec::new())
        .expect("ilmari_core/1");
    let boss2 = unit_by_key(&sim, BOSS).unwrap_or(boss);
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(2.0, 0.0, CENTRE_Z + 4.0));
    }
    pull(&mut sim, &ids, boss2);
    (sim, ids, boss2)
}

#[test]
fn marie_s_desk_hands_the_log_over_once_and_pays_two_tokens() {
    let (mut sim, ids, _boss) = setup(false, &["backend"], 7);
    sim.load_zone("campus_occupied", &anchors, Vec::new())
        .expect("campus_occupied");
    move_to(&mut sim, ids[0], Vec3::new(1.0, 0.0, CENTRE_Z + 6.0));
    let tokens = |sim: &Sim| {
        sim.unit(ids[0])
            .expect("p")
            .inventory
            .iter()
            .map(|(k, n)| (k.clone(), *n))
            .collect::<Vec<_>>()
    };
    let mut outcomes = Vec::new();
    for _ in 0..2 {
        sim.apply_as(
            ids[0],
            Command::Interact {
                prop: "il_marie_desk".into(),
            },
        );
        for e in sim.tick() {
            if let Event::Interacted { prop, outcome, .. } = e
                && prop == "il_marie_desk"
            {
                outcomes.push(outcome);
            }
        }
        run(&mut sim, 2);
    }
    assert_eq!(
        outcomes,
        [combat_core::PROP_OK, 0],
        "the second read hands nothing over"
    );
    let inv = tokens(&sim);
    assert!(inv.contains(&(LOG_ITEM.to_string(), 1)), "{inv:?}");
    assert!(inv.contains(&("Ilmari Token".to_string(), 2)), "{inv:?}");
}

// ---------------------------------------------------------------- Marie's words

fn core_cfg() -> Box<combat_core::CoreDef> {
    let c = content();
    match c.unit_def(BOSS).and_then(|u| u.encounter.clone()) {
        Some(combat_core::EncounterDef::Core(cfg)) => cfg,
        _ => panic!("the core has no encounter"),
    }
}

/// The ASCII a line of "*cough* 43 4F 42 4F 4C" spells.
fn hex_word(line: &str) -> Option<String> {
    let tail = line.split("*cough*").nth(1)?;
    let bytes: Option<Vec<u8>> = tail
        .trim()
        .trim_end_matches('.')
        .split_whitespace()
        .map(|t| u8::from_str_radix(t, 16).ok())
        .collect();
    let bytes = bytes?;
    bytes
        .iter()
        .all(|b| (0x20..0x7f).contains(b))
        .then(|| bytes.iter().map(|&b| char::from(b)).collect())
}

#[test]
fn the_hex_lines_decode() {
    let cfg = core_cfg();
    assert_eq!(hex_word(&cfg.rounds[0].line).as_deref(), Some("COBOL"));
    assert_eq!(hex_word(&cfg.marie_last).as_deref(), Some("REVERT"));
    assert_eq!(hex_word("nothing here"), None);
}

#[test]
fn marie_has_a_line_for_everything_and_every_one_is_tidy() {
    let cfg = core_cfg();
    let mut lines: Vec<&String> = Vec::new();
    lines.extend(&cfg.marie_open);
    lines.extend(cfg.marie_choice.iter().flatten());
    lines.extend(&cfg.marie_end_round);
    lines.push(&cfg.marie_last);
    lines.push(&cfg.marie_hint);
    lines.push(&cfg.marie_story_hint);
    lines.extend(cfg.rounds.iter().map(|r| &r.line));
    lines.extend(cfg.stance_yells.iter().flatten());
    assert!(lines.len() >= 20);
    for l in lines {
        assert!(!l.is_empty() && l.len() <= 200, "{l}");
        assert!(l.is_ascii(), "{l}");
        // Only the fixed templates, and every one resolves.
        let mut rest = l.as_str();
        while let Some(i) = rest.find('{') {
            let j = rest[i..].find('}').expect("closed template");
            assert_eq!(&rest[i..=i + j], "{stamp}", "{l}");
            rest = &rest[i + j..];
        }
    }
}

#[test]
fn the_opening_line_quotes_the_rooms_own_clock() {
    let (mut sim, ids, boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    // The stamp the Opening wrote: 18:07:23 CEST on a known day.
    sim.debug_mark_boss("agi_prototype");
    sim.apply_as(
        ids[0],
        Command::StampIlmari {
            unix_secs: 1_792_598_843,
            utc_offset_min: 120,
            tz: "CEST".into(),
        },
    );
    let mut yells = Vec::new();
    sim.apply_as(
        ids[0],
        Command::SetTarget {
            unit: ids[0],
            target: Some(boss),
        },
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: SpellId(1),
        },
    );
    for e in run(&mut sim, 3) {
        if let Event::Yell { text, .. } = &e {
            yells.push(text.clone());
        }
    }
    let clock = sim.ilmari_stamp().expect("stamped").clock();
    let open = yells
        .iter()
        .find(|t| t.contains("every screen in this building"))
        .unwrap_or_else(|| panic!("no opening line in {yells:?}"));
    assert!(open.contains(&clock), "the clock is the room's own: {open}");
    assert!(!open.contains('{'), "the template resolved: {open}");
}

#[test]
fn the_end_stamp_is_saved_with_the_opening_stamp_and_old_saves_have_none() {
    let (mut sim, ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], 5);
    sim.debug_mark_boss("agi_prototype");
    sim.apply_as(
        ids[0],
        Command::StampIlmari {
            unix_secs: 1_792_598_843,
            utc_offset_min: 120,
            tz: "CEST".into(),
        },
    );
    bar_at(&mut sim, 1);
    sim.debug_hit(ids[0], screens(&sim)[0], 10_000_000);
    run(&mut sim, 2);
    sim.apply_as(
        ids[0],
        Command::StampIlmariEnd {
            unix_secs: THURSDAY + 1_000,
        },
    );
    sim.tick();
    let text = sim.export_save(ids[0]).expect("save").encode();
    let line = text
        .lines()
        .find(|l| l.starts_with("ilmari_end="))
        .unwrap_or_else(|| panic!("no ilmari_end in {text}"));
    assert!(
        line.starts_with(&format!("ilmari_end={},1,", THURSDAY + 1_000)),
        "{line}"
    );
    let mut back = Sim::new(content(), 5);
    let q = back.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(back.import_save(q, &combat_core::SaveData::decode(&text).expect("decode")));
    assert_eq!(back.ilmari_end(), sim.ilmari_end());
    let old: String = text
        .lines()
        .filter(|l| !l.starts_with("ilmari_end="))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut older = Sim::new(content(), 5);
    let r = older.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(older.import_save(r, &combat_core::SaveData::decode(&old).expect("decode")));
    assert!(older.ilmari_end().is_none());
}

// ---------------------------------------------------------------- Judgment and the rider

#[test]
fn the_rider_draws_two_at_once_and_keeps_them() {
    let (mut sim, _ids, boss) = setup(false, &["backend", "devops", "fullstack"], 7);
    assert!(sim.debug_apply_aura(boss, "il_rider_remix", 3_600_000));
    pull(&mut sim, &_ids, boss);
    let events = run(&mut sim, 22 + 45 + 10);
    let d = draws(&events);
    assert_eq!(d.len(), 4, "two draws at each of two marks: {d:?}");
    assert_ne!(d[0], d[1], "two different reruns at once");
}

#[test]
fn judgment_swaps_marie_s_second_argument_for_a_hint() {
    let mut c = content();
    for k in [
        "senior_engineer",
        "backend_dev",
        "devops_eng",
        "frontend_dev",
    ] {
        def_mut(&mut c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
    let mut sim = Sim::new(c, 33);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let classes = ["backend", "devops", "fullstack"];
    let ids: Vec<EntityId> = (0..3)
        .map(|k| {
            let p = sim.spawn_player(classes[k], Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, 10);
            p
        })
        .collect();
    sim.debug_ilmari_core_cleared(true);
    sim.apply_as(
        ids[0],
        Command::SetIlmariMode(combat_core::ilmari::IlmariMode::Judgment),
    );
    sim.tick();
    sim.load_zone("ilmari_core/1", &anchors, Vec::new())
        .expect("core");
    let boss = unit_by_key(&sim, BOSS).expect("boss");
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(2.0, 0.0, CENTRE_Z + 4.0));
    }
    pull(&mut sim, &ids, boss);
    let mut yells = Vec::new();
    bar_at(&mut sim, 69);
    for _ in 0..(55 + 70) * SECOND {
        for e in sim.tick() {
            if let Event::Yell { unit, text } = e
                && sim.unit(unit).is_some_and(|u| u.key == "il_marie_core")
            {
                yells.push(text);
            }
        }
    }
    assert!(
        yells.iter().any(|y| y.contains("Third column, fifth row")),
        "the hint comes in the second round: {yells:?}"
    );
    assert!(
        !yells.iter().any(|y| y.contains("keeps you safe")),
        "and the argument does not: {yells:?}"
    );
}

// ---------------------------------------------------------------- the death pact and the ward

fn cio_room(
    classes: &[&'static str],
    tweak: impl FnOnce(&mut combat_core::Content),
) -> (Sim, Vec<EntityId>, EntityId) {
    let mut c = content();
    let units: Vec<String> = c.classes.iter().map(|k| k.unit.clone()).collect();
    for k in &units {
        def_mut(&mut c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, 5);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids: Vec<EntityId> = classes
        .iter()
        .enumerate()
        .map(|(k, class)| {
            let p = sim.spawn_player(class, Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, 10);
            p
        })
        .collect();
    sim.debug_force_raid_async(Some(false));
    sim.load_zone("ilmari6/3", &anchors, Vec::new())
        .expect("ilmari6/3");
    let boss = sim
        .units()
        .find(|u| u.key == "il_cio")
        .map(|u| u.id)
        .expect("cio");
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(2.0, 0.0, BOSS_Z + 8.0));
    }
    pull(&mut sim, &ids, boss);
    (sim, ids, boss)
}

fn pact_tweak(c: &mut combat_core::Content) {
    def_mut(c, "il_consultant").death_pact = Some(combat_core::PactDef {
        window_ms: 8_000,
        revive_hp_pct: 50,
        both_hp_pct: 6,
        both_aura: Some("il_offboarding".into()),
        both_ms: 4_000,
        yells: vec!["Reassembling... please hold.".into()],
        story_rebuild: true,
    });
}

fn consultants(sim: &Sim) -> Vec<EntityId> {
    sim.units()
        .filter(|u| u.key == "il_consultant" && u.alive)
        .map(|u| u.id)
        .collect()
}

/// The CIO's third act brings two consultants in; returns once both stand.
fn two_consultants(sim: &mut Sim, boss: EntityId) -> Vec<EntityId> {
    let max = sim.unit(boss).expect("boss").max_hp;
    assert!(sim.debug_set_hp(boss, max * 55 / 100));
    for _ in 0..12 * SECOND {
        sim.tick();
        if consultants(sim).len() == 2 {
            break;
        }
    }
    let c = consultants(sim);
    assert_eq!(c.len(), 2, "the second act hires two consultants");
    c
}

#[test]
fn a_pact_member_is_rebuilt_at_half_when_its_partner_stands() {
    let (mut sim, _ids, boss) = cio_room(&["backend", "devops", "fullstack"], pact_tweak);
    let c = two_consultants(&mut sim, boss);
    assert!(sim.debug_kill(c[0]));
    let down = run(&mut sim, 1);
    assert!(
        down.iter()
            .any(|e| matches!(e, Event::Reassembling { unit, ms: 8000, .. } if *unit == c[0])),
        "the rebuild is announced with its window"
    );
    run(&mut sim, 5);
    assert_eq!(consultants(&sim).len(), 1, "still down inside the window");
    run(&mut sim, 4);
    let now = consultants(&sim);
    assert_eq!(now.len(), 2, "rebuilt after the window");
    let new = now
        .iter()
        .find(|&&u| u != c[1])
        .copied()
        .expect("the new one");
    let u = sim.unit(new).expect("unit");
    assert!(
        (49..=51).contains(&(u.hp * 100 / u.max_hp)),
        "at half: {}/{}",
        u.hp,
        u.max_hp
    );
}

#[test]
fn both_down_inside_the_window_costs_the_owner_and_nobody_is_rebuilt() {
    let (mut sim, _ids, boss) = cio_room(&["backend", "devops", "fullstack"], pact_tweak);
    let c = two_consultants(&mut sim, boss);
    let before = sim.unit(boss).expect("boss").hp;
    let max = sim.unit(boss).expect("boss").max_hp;
    assert!(sim.debug_kill(c[0]));
    run(&mut sim, 3);
    assert!(sim.debug_kill(c[1]));
    let events = run(&mut sim, 1);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Reassembling { unit, .. } if *unit == c[1])),
        "the second fall cancels the first's rebuild"
    );
    let lost = before - sim.unit(boss).expect("boss").hp;
    assert!(
        u64::from(lost).abs_diff(u64::from(max) * 6 / 100) < u64::from(max) / 100,
        "the owner lost about 6 %: {lost} of {max}"
    );
    assert!(
        has_aura(&sim, boss, "il_offboarding"),
        "and wears the penalty"
    );
    run(&mut sim, 12);
    assert!(consultants(&sim).is_empty(), "nobody comes back");
}

fn ward_tweak(c: &mut combat_core::Content) {
    let m = def_mut(c, "il_marie_core");
    m.ward = true;
    m.ward_death_wipes = true;
    m.untouchable = false;
    m.max_hp = 4_000;
    for k in SCREENS {
        let u = def_mut(c, k);
        if let Some(ai) = u.ai.as_mut() {
            for ab in &mut ai.abilities {
                ab.target = combat_core::AiTarget::Ward;
            }
        }
    }
}

#[test]
fn the_boss_aims_at_the_ward_and_a_dead_ward_loses_the_fight() {
    let (mut sim, ids, boss) =
        setup_with("full", &["backend", "devops", "fullstack"], 5, ward_tweak);
    pull(&mut sim, &ids, boss);
    let marie = sim
        .units()
        .find(|u| u.key == "il_marie_core")
        .map(|u| u.id)
        .expect("marie");
    let max = sim.unit(marie).expect("marie").max_hp;
    run(&mut sim, 25);
    assert!(
        sim.unit(marie).expect("marie").hp < max,
        "the boss's attacks went to the ward"
    );
    assert!(ids.iter().all(|&p| sim.unit(p).is_some_and(|u| u.alive)));
    sim.debug_hit(boss, marie, 1_000_000);
    run(&mut sim, 2);
    assert!(
        ids.iter().all(|&p| sim.unit(p).is_some_and(|u| !u.alive)),
        "the room wipes"
    );
}

#[test]
fn a_marie_nobody_may_hit_is_nobody_s_random_target() {
    let (mut sim, ids, boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    pull(&mut sim, &ids, boss);
    let marie = sim
        .units()
        .find(|u| u.key == "il_marie_core")
        .map(|u| u.id)
        .expect("marie");
    let max = sim.unit(marie).expect("marie").max_hp;
    run(&mut sim, 40);
    assert_eq!(sim.unit(marie).expect("marie").hp, max, "untouched");
}

#[test]
fn the_fight_replays_bit_identically() {
    let log = |seed: u64| {
        let (mut sim, _ids, _boss) = pulled(false, &["backend", "devops", "fullstack"], seed);
        let mut out = String::new();
        for _ in 0..(22 + 2 * 45) * SECOND {
            let now = sim.now().0;
            for e in sim.tick() {
                out.push_str(&log_line(&sim, now, &e));
                out.push('\n');
            }
        }
        out
    };
    let (a, b) = (log(11), log(11));
    assert!(a.len() > 1000);
    assert_eq!(a, b, "same seed, same fight");
    assert_ne!(a, log(12), "another seed differs");
}

// ---------------------------------------------------------------- P3: the mech (D12)

/// A fight at the start of P3 with the stance the three rounds `picks` make (every voter takes it).
fn at_p3(
    mode: &str,
    classes: &[&'static str],
    seed: u64,
    picks: [usize; 3],
) -> (Sim, Vec<EntityId>, EntityId) {
    let (mut sim, ids, boss) = setup_with(mode, classes, seed, |_| {});
    pull(&mut sim, &ids, boss);
    run(&mut sim, 5);
    play_rounds(&mut sim, &ids, picks);
    assert_eq!(phase_of(&sim), 3, "phase 3");
    (sim, ids, boss)
}

const TRIO: [&str; 3] = ["backend", "devops", "fullstack"];
const ARM: &str = "il_ilmari_arm";
const HEAD: &str = "il_ilmari_head";

fn units_of(sim: &Sim, key: &str) -> Vec<EntityId> {
    sim.units()
        .filter(|u| u.key == key && u.alive)
        .map(|u| u.id)
        .collect()
}

/// The spells the screens cast (instant or cast-bar) in `events`.
fn screen_casts(sim: &Sim, events: &[Event]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::CastSucceeded { caster, spell, .. }
                if sim.unit(*caster).is_some_and(|u| {
                    u.key.starts_with("il_ilmari") && u.key != ARM && u.key != HEAD
                }) =>
            {
                Some(spell.0)
            }
            _ => None,
        })
        .collect()
}

/// The live screen: the first that is not on standby.
fn live_screen(sim: &Sim) -> EntityId {
    let k = live(sim).first().copied().expect("a live screen");
    screens(sim)[k]
}

#[test]
fn phase_three_builds_the_body_with_two_arms_and_runs_its_kit() {
    let (mut sim, _ids, _boss) = at_p3("full", &TRIO, 7, [0, 1, 2]);
    let arms = units_of(&sim, ARM);
    assert_eq!(arms.len(), 2, "two Rack Arms");
    let live_id = live_screen(&sim);
    for a in &arms {
        let u = sim.unit(*a).expect("arm");
        assert!(u.hostile && u.alive);
    }
    let events = run(&mut sim, 150);
    let casts = screen_casts(&sim, &events);
    for want in [2740, 2742, 2744, 2748, 2749, 2750] {
        assert!(casts.contains(&want), "the kit casts {want}: {casts:?}");
    }
    // Hedged: no overlay of any stance.
    for none in [2756, 2757, 2520, 2758] {
        assert!(!casts.contains(&none), "hedged has no {none}");
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Summoned { summoner, .. } if *summoner == live_id)),
        "the body summons from the live screen (turrets and the head)"
    );
    let w = wall(&events).expect("wall");
    assert_eq!(w.0, 3, "the wall says phase 3");
}

#[test]
fn every_stance_adds_its_own_overlay_to_the_body() {
    // (picks, the spell only that stance casts)
    let cases: [([usize; 3], u32); 4] = [
        ([0, 0, 0], 2756),
        ([1, 1, 1], 2757),
        ([2, 2, 2], 2520),
        ([0, 1, 2], 0),
    ];
    for (picks, spell) in cases {
        let (mut sim, _ids, _boss) = at_p3("full", &TRIO, 7, picks);
        let events = run(&mut sim, 120);
        let casts = screen_casts(&sim, &events);
        for other in [2756, 2757, 2520, 2758] {
            assert_eq!(
                casts.contains(&other),
                other == spell,
                "picks {picks:?}: {other} in {casts:?}"
            );
        }
    }
    // Leaky needs the log in the room; Story's single vote is enough.
    let (mut sim, ids, boss) = pulled_with_log(true, &["backend"], 7);
    let _ = boss;
    play_rounds(&mut sim, &ids, [3, 3, 3]);
    assert_eq!(phase_of(&sim), 3);
    assert_eq!(stance_on(&sim), ["il_stance_leaky"]);
    let events = run(&mut sim, 120);
    let casts = screen_casts(&sim, &events);
    assert!(
        casts.contains(&2767),
        "Token Spill (Story) leaks: {casts:?}"
    );
}

#[test]
fn compliant_offers_are_cancelled_at_the_kiosk_and_adversarial_hardens() {
    let (mut sim, ids, _boss) = at_p3("full", &TRIO, 7, [0, 0, 0]);
    // The offer lands within 18 s + the 1.5 s cast: somebody has a pending il_ok_ aura.
    let mut pending = None;
    for _ in 0..40 * SECOND {
        sim.tick();
        pending = ids.iter().copied().find(|&p| {
            sim.unit(p)
                .is_some_and(|u| u.auras.iter().any(|a| a.key == "il_ok_suggestion"))
        });
        if pending.is_some() {
            break;
        }
    }
    let p = pending.expect("a Helpful Suggestion is on somebody");
    sim.apply_as(
        p,
        Command::Interact {
            prop: "il_core_optout".into(),
        },
    );
    sim.tick();
    assert!(
        !has_aura(&sim, p, "il_ok_suggestion"),
        "the opt-out kiosk cancels it"
    );
    let (mut sim, _ids, _boss) = at_p3("full", &TRIO, 7, [1, 1, 1]);
    let mut hardened = false;
    for _ in 0..60 * SECOND {
        sim.tick();
        if has_aura(&sim, live_screen(&sim), "il_hardened") {
            hardened = true;
            break;
        }
    }
    assert!(hardened, "the Defensive body sets like concrete");
}

#[test]
fn a_rack_arm_is_rebuilt_at_half_and_both_inside_the_window_cost_the_body() {
    let (mut sim, ids, _boss) = at_p3("full", &TRIO, 7, [0, 1, 2]);
    let arms = units_of(&sim, ARM);
    sim.debug_hit(ids[0], arms[0], 10_000_000);
    let mut events = sim.tick();
    assert_eq!(units_of(&sim, ARM).len(), 1, "one fell");
    events.extend(run(&mut sim, 9));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Reassembling { unit, .. } if *unit == arms[0])),
        "the castbar: Reassembling... please hold"
    );
    let now = units_of(&sim, ARM);
    assert_eq!(now.len(), 2, "the dead arm is back after the window");
    let rebuilt = now
        .iter()
        .find(|a| **a != arms[1])
        .copied()
        .expect("rebuilt");
    let u = sim.unit(rebuilt).expect("rebuilt");
    assert!(
        u.hp * 2 <= u.max_hp + 2 && u.hp * 2 + 2 >= u.max_hp,
        "rebuilt at half: {}/{}",
        u.hp,
        u.max_hp
    );

    // Both inside the window: no rebuild, the body loses 6 % of its max HP and the cable whip.
    let (mut sim, ids, _boss) = at_p3("full", &TRIO, 7, [0, 1, 2]);
    let arms = units_of(&sim, ARM);
    let live_id = live_screen(&sim);
    let before = sim.unit(live_id).expect("screen").hp;
    let max = sim.unit(live_id).expect("screen").max_hp;
    sim.debug_hit(ids[0], arms[0], 10_000_000);
    run(&mut sim, 3);
    sim.debug_hit(ids[0], arms[1], 10_000_000);
    sim.tick();
    let after = sim.unit(live_id).expect("screen").hp;
    assert!(
        before.saturating_sub(after) >= max * 6 / 100 - 2,
        "the body took 6 %: {before} -> {after} of {max}"
    );
    assert!(
        has_aura(&sim, live_id, "il_whipless"),
        "no cable whip for 25 s"
    );
    let events = run(&mut sim, 20);
    assert!(units_of(&sim, ARM).is_empty(), "nobody is rebuilt");
    assert!(
        !screen_casts(&sim, &events).contains(&2750),
        "the whip stays unplugged"
    );
    let events = run(&mut sim, 20);
    assert!(
        screen_casts(&sim, &events).contains(&2750),
        "and plugs back in"
    );

    // Story: the arms do not rebuild each other.
    let (mut sim, ids, _boss) = at_p3("story", &["backend"], 7, [0, 1, 2]);
    let arms = units_of(&sim, ARM);
    sim.debug_hit(ids[0], arms[0], 10_000_000);
    run(&mut sim, 10);
    assert_eq!(units_of(&sim, ARM).len(), 1, "Story rebuilds nothing");
}

/// A class's interrupt (a spell with `Effect::Interrupt`).
fn interrupt_of(sim: &Sim, class: &str) -> SpellId {
    let c = sim.content();
    let cl = c.class(class).expect("class");
    cl.action_bar
        .iter()
        .copied()
        .find(|s| {
            c.spell(*s)
                .is_some_and(|d| matches!(d.effect, combat_core::Effect::Interrupt { .. }))
        })
        .expect("an interrupt on the bar")
}

/// Runs until a head stands and casts; returns its id.
fn wait_for_head(sim: &mut Sim) -> EntityId {
    for _ in 0..90 * SECOND {
        sim.tick();
        if let Some(h) = units_of(sim, HEAD).first().copied()
            && sim.unit(h).is_some_and(|u| u.cast.is_some())
        {
            return h;
        }
    }
    panic!("no head in 90 s");
}

#[test]
fn a_kick_on_the_head_delays_quarterly_planning_and_a_finished_plan_lets_the_volley_go() {
    // Kicked: the volley does not come and the head is back in 20 s (the delay), not in a minute.
    let (mut sim, ids, _boss) = at_p3("full", &TRIO, 7, [0, 1, 2]);
    let kick = interrupt_of(&sim, "backend");
    let head = wait_for_head(&mut sim);
    let at = sim.unit(head).expect("head").pos;
    move_to(&mut sim, ids[0], at);
    sim.apply_as(
        ids[0],
        Command::SetTarget {
            unit: ids[0],
            target: Some(head),
        },
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: kick,
        },
    );
    let events = run(&mut sim, 3);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::CastInterrupted { caster, .. } if *caster == head
        )),
        "the head is interrupted"
    );
    let planning_done = |e: &Event| matches!(e, Event::Yell { text, .. } if text.contains("Planning complete") || text.contains("roadmap is final"));
    let events = run(&mut sim, 7);
    assert!(!events.iter().any(planning_done), "no volley after a kick");
    let mut next = None;
    for k in 0..40 * SECOND {
        sim.tick();
        if units_of(&sim, HEAD).first().is_some_and(|h| *h != head) {
            next = Some(k / SECOND + 10);
            break;
        }
    }
    assert!(
        next.is_some_and(|s| (10..=24).contains(&s)),
        "the next plan starts about 20 s after the kick: {next:?}"
    );

    // Finished: three casts of the body at once, the same tick.
    let (mut sim, _ids, _boss) = at_p3("full", &TRIO, 7, [0, 1, 2]);
    let head = wait_for_head(&mut sim);
    let mut seen = None;
    for _ in 0..8 * SECOND {
        let events = sim.tick();
        let spells: std::collections::BTreeSet<u32> =
            screen_casts(&sim, &events).into_iter().collect();
        let said = events.iter().any(|e| {
            matches!(e, Event::Yell { text, .. } if text.contains("Planning complete") || text.contains("roadmap is final"))
        });
        if spells.len() >= 3 {
            seen = Some(spells);
            break;
        }
        let _ = said;
    }
    let spells = seen.expect("a volley of three casts in one tick");
    assert!(
        spells.iter().all(|s| (2740..=2750).contains(s)),
        "{spells:?}"
    );
    assert!(
        sim.unit(head).is_some(),
        "the head stands through the volley"
    );
}

#[test]
fn story_splits_the_volley_in_two() {
    let (mut sim, _ids, _boss) = at_p3("story", &["backend"], 7, [0, 1, 2]);
    // Nobody kicks: the head's cast finishes, half now and half 4 s later.
    wait_for_head(&mut sim);
    let mut ticks_with_casts = Vec::new();
    for k in 0..12 * SECOND {
        let events = sim.tick();
        if !screen_casts(&sim, &events).is_empty() && k > 3 * SECOND {
            ticks_with_casts.push(k);
        }
    }
    assert!(
        ticks_with_casts
            .windows(2)
            .any(|w| w[1] - w[0] >= 3 * SECOND),
        "two bursts, a few seconds apart: {ticks_with_casts:?}"
    );
}

#[test]
fn the_hot_take_locks_one_member_to_one_spell_for_eight_seconds() {
    let (mut sim, ids, boss) = at_p3("full", &TRIO, 7, [0, 1, 2]);
    // The Judgment rider is the boss's marker aura; Full has none, so wear it here.
    assert!(sim.debug_apply_aura(boss, "il_rider_remix", 3_600_000));
    let mut who = None;
    for _ in 0..70 * SECOND {
        sim.tick();
        who = ids.iter().copied().find(|&p| sim.hot_take_of(p).is_some());
        if who.is_some() {
            break;
        }
    }
    let p = who.expect("somebody was given a Hot Take");
    assert!(has_aura(&sim, p, "il_hot_take"), "and the marker shows it");
    let only = sim.hot_take_of(p).expect("locked spell");
    let class = TRIO[ids.iter().position(|&i| i == p).expect("a player")];
    let bar: Vec<SpellId> = sim
        .content()
        .class(class)
        .expect("class")
        .action_bar
        .clone();
    // Any other spell is refused (silenced), and the locked one is not.
    let other = bar
        .iter()
        .copied()
        .find(|s| *s != only)
        .expect("another spell of theirs");
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: other,
        },
    );
    let events = sim.tick();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::CastFailed { caster, reason: combat_core::CastError::Silenced, .. } if *caster == p
        )),
        "a second spell is refused"
    );
    run(&mut sim, 9);
    assert!(sim.hot_take_of(p).is_none(), "and it ends");
}

// ---------------------------------------------------------------- P4: the mainframe (D13)

const WARD: &str = "il_core_mainframe";
const REWRITE: &str = "il_modern_replacement";

/// A fight at the start of P4 (the bar dropped to the threshold at once).
fn at_p4(mode: &str, classes: &[&'static str], seed: u64) -> (Sim, Vec<EntityId>, EntityId) {
    let (mut sim, ids, boss) = at_p3(mode, classes, seed, [0, 1, 2]);
    bar_at(&mut sim, if mode == "story" { 49 } else { 19 });
    run(&mut sim, 2);
    assert_eq!(phase_of(&sim), 4, "phase 4");
    (sim, ids, boss)
}

fn ward_of(sim: &Sim) -> EntityId {
    units_of(sim, WARD).first().copied().expect("the mainframe")
}

fn stage_of(events: &[Event]) -> Option<(u8, u8, u8, u8)> {
    events.iter().rev().find_map(|e| match e {
        Event::Core {
            stage,
            clock_s,
            ward_pct,
            wrong,
            ..
        } => Some((*stage, *clock_s, *ward_pct, *wrong)),
        _ => None,
    })
}

/// The wall as it stands: the sim speaks on a change, so tick (at most four seconds) until it does.
fn wall_now(sim: &mut Sim) -> (u8, u8, u8, u8) {
    for _ in 0..4 * SECOND {
        if let Some(w) = stage_of(&sim.tick()) {
            return w;
        }
    }
    panic!("the wall said nothing in four seconds");
}

fn git(sim: &mut Sim, p: EntityId, terminal: usize) -> u32 {
    let key = format!("il_git_{}", terminal + 1);
    sim.apply_as(p, Command::Interact { prop: key.clone() });
    sim.tick()
        .iter()
        .find_map(|e| match e {
            Event::Interacted {
                unit,
                prop,
                outcome,
            } if *unit == p && *prop == key => Some(*outcome),
            _ => None,
        })
        .unwrap_or(u32::MAX)
}

#[test]
fn the_mainframe_rises_at_twenty_percent_and_ilmari_aims_at_it() {
    let (mut sim, _ids, _boss) = at_p4("full", &TRIO, 7);
    let ward = ward_of(&sim);
    let w = sim.unit(ward).expect("ward");
    assert!(
        !w.hostile && w.alive && w.max_hp == 24_000,
        "friendly, 24,000 HP"
    );
    assert!(units_of(&sim, ARM).is_empty(), "the mech fell apart");
    // The bar holds at the floor until the revert, however hard it is hit.
    let live_id = live_screen(&sim);
    let max = sim.unit(live_id).expect("screen").max_hp;
    sim.debug_hit(ward, live_id, max / 10);
    run(&mut sim, 2);
    for s in screens(&sim) {
        let u = sim.unit(s).expect("screen");
        assert!(
            u.alive && u.hp * 100 >= u.max_hp * 20,
            "{} at its floor",
            u.key
        );
    }
    let events = run(&mut sim, 20);
    assert!(
        screen_casts(&sim, &events).contains(&2760),
        "Deprecation Notice goes through the mainframe"
    );
    assert!(
        sim.unit(ward).expect("ward").hp < 24_000,
        "and it is the mainframe that hurts"
    );
    let (stage, clock, _pct, wrong) = stage_of(&events).expect("wall");
    assert_eq!((stage, wrong), (1, 0));
    assert!((140..=180).contains(&clock), "180 s clock: {clock}");
    // Story keeps it for 120 s and the mainframe takes 40 %.
    let (sim, _ids, _boss) = at_p4("story", &["backend"], 7);
    assert!(has_aura(&sim, ward_of(&sim), "il_ward_story"));
}

#[test]
fn rewrites_head_for_the_mainframe_and_each_one_killed_takes_two_seconds_off_the_clock() {
    let (mut sim, ids, boss) = at_p4("full", &TRIO, 7);
    let ward = ward_of(&sim);
    let events = run(&mut sim, 16);
    let adds = units_of(&sim, REWRITE);
    assert_eq!(adds.len(), 5, "a wave of five at 12 s");
    assert!(
        adds.iter().all(|a| sim
            .unit(*a)
            .is_some_and(|u| u.target == Some(ward) || u.in_combat)),
        "they start on the mainframe"
    );
    let _ = (events, boss);
    run(&mut sim, 8);
    assert!(sim.unit(ward).expect("ward").hp < 24_000, "they chew it");
    let clock_before = wall_now(&mut sim).1;
    for a in units_of(&sim, REWRITE).into_iter().take(3) {
        sim.debug_hit(ids[0], a, 10_000_000);
    }
    let clock_after = wall_now(&mut sim).1;
    assert!(
        clock_before - clock_after >= 5,
        "three rewrites took six seconds off: {clock_before} -> {clock_after}"
    );
    // The rider's wave is eight; Story's is three.
    let (mut sim, _ids, boss) = at_p4("full", &TRIO, 8);
    assert!(sim.debug_apply_aura(boss, "il_rider_remix", 3_600_000));
    run(&mut sim, 16);
    assert_eq!(units_of(&sim, REWRITE).len(), 8, "Technical Debt");
    let (mut sim, _ids, _boss) = at_p4("story", &["backend"], 7);
    run(&mut sim, 16);
    assert_eq!(units_of(&sim, REWRITE).len(), 3, "Story");
}

#[test]
fn the_clock_cut_is_capped_at_thirty_seconds() {
    let (mut sim, ids, _boss) = at_p4("full", &TRIO, 7);
    let ward = ward_of(&sim);
    let start = wall_now(&mut sim).1;
    // The party kills every rewrite the moment it appears, for 100 s, with the mainframe held up.
    let mut killed = 0;
    for _ in 0..100 {
        run(&mut sim, 1);
        sim.debug_set_hp(ward, 24_000);
        for a in units_of(&sim, REWRITE) {
            sim.debug_hit(ids[0], a, 10_000_000);
            killed += 1;
        }
    }
    assert!(killed >= 10, "a few waves came: {killed}");
    let now = wall_now(&mut sim).1;
    let passed = 100 + 1;
    let cut = i32::from(start) - passed - i32::from(now);
    assert!(
        (20..=32).contains(&cut),
        "the party cut 30 s at most, and at least a good part: start {start}, now {now}, cut {cut}"
    );
}

#[test]
fn punch_cards_heal_the_mainframe_for_any_class_once_per_cooldown() {
    let (mut sim, ids, _boss) = at_p4("full", &["frontend", "fullstack", "qa"], 7);
    let ward = ward_of(&sim);
    assert!(sim.debug_set_hp(ward, 10_000));
    sim.apply_as(
        ids[0],
        Command::Interact {
            prop: "il_punch_1".into(),
        },
    );
    sim.tick();
    let hp = sim.unit(ward).expect("ward").hp;
    assert!(
        (10_400..=10_700).contains(&hp),
        "2 % of 24,000 is 480: {hp}"
    );
    // The same card again is cooling down; another card is not.
    sim.apply_as(
        ids[1],
        Command::Interact {
            prop: "il_punch_1".into(),
        },
    );
    sim.tick();
    assert_eq!(sim.unit(ward).expect("ward").hp, hp, "the card is cooling");
    sim.apply_as(
        ids[2],
        Command::Interact {
            prop: "il_punch_2".into(),
        },
    );
    sim.tick();
    assert!(sim.unit(ward).expect("ward").hp > hp, "another card works");
}

#[test]
fn the_healer_and_the_greybeard_heal_and_patch_the_mainframe() {
    let (mut sim, ids, _boss) = at_p4("full", &["devops", "cobol", "backend"], 7);
    let ward = ward_of(&sim);
    assert!(sim.debug_set_hp(ward, 10_000));
    let hotfix = SpellId(41);
    sim.apply_as(
        ids[0],
        Command::SetTarget {
            unit: ids[0],
            target: Some(ward),
        },
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: hotfix,
        },
    );
    run(&mut sim, 4);
    assert!(
        sim.unit(ward).expect("ward").hp > 10_000,
        "a heal lands on the Ward"
    );
    // Patch The Mainframe: 5 % and the Migration Plan stops.
    assert!(sim.debug_apply_aura(ward, "il_migration", 10_000));
    assert!(sim.debug_set_hp(ward, 10_000));
    sim.apply_as(
        ids[1],
        Command::SetTarget {
            unit: ids[1],
            target: Some(ward),
        },
    );
    sim.apply_as(
        ids[1],
        Command::Cast {
            caster: ids[1],
            spell: SpellId(2764),
        },
    );
    run(&mut sim, 4);
    assert!(
        !has_aura(&sim, ward, "il_migration"),
        "a fix from the Reagan administration stops the plan"
    );
    let hp = sim.unit(ward).expect("ward").hp;
    // 5 % is 1,200; the plan and the line chewed some of it back while the cast ran.
    assert!(hp >= 10_500, "and restores about 5 %: {hp}");
}

#[test]
fn a_dead_mainframe_wipes_the_room() {
    let (mut sim, ids, _boss) = at_p4("full", &TRIO, 7);
    let ward = ward_of(&sim);
    sim.debug_hit(ids[0], ward, 1_000_000);
    run(&mut sim, 2);
    assert!(
        ids.iter().all(|&p| sim.unit(p).is_some_and(|u| !u.alive)),
        "Ilmari has successfully modernised it"
    );
}

/// P4 with the clock run out (the mainframe held up by hand): the terminals are lit. Returns the
/// room, the players, the terminal that reverts the culprit and the events up to the log.
fn at_terminals(
    mode: &str,
    classes: &[&'static str],
    seed: u64,
) -> (Sim, Vec<EntityId>, usize, Vec<Event>) {
    let (mut sim, ids, _boss) = at_p4(mode, classes, seed);
    let ward = ward_of(&sim);
    let mut events = Vec::new();
    for _ in 0..220 * SECOND {
        events.extend(sim.tick());
        sim.debug_set_hp(ward, 24_000);
        if events.iter().any(|e| matches!(e, Event::CoreLog { .. })) {
            break;
        }
    }
    let right = events
        .iter()
        .find_map(|e| match e {
            Event::CoreLog { lines } => lines
                .iter()
                .find(|l| l.contains("b210a76"))
                .and_then(|l| l.chars().nth(1))
                .and_then(|c| c.to_digit(10))
                .map(|d| d as usize - 1),
            _ => None,
        })
        .expect("the log is up and names the culprit's terminal");
    (sim, ids, right, events)
}

fn last_log(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::CoreLog { lines } => Some(lines.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn the_clock_ends_and_the_terminals_light_with_the_log() {
    let (sim, _ids, right, events) = at_terminals("full", &TRIO, 7);
    let log = last_log(&events);
    assert_eq!(log.len(), 6, "six commits");
    assert!(
        log.iter().all(|l| l.starts_with('[')),
        "each row names its terminal"
    );
    let culprit = combat_core::culprit_index(0);
    assert!(
        log[culprit].contains("b210a76"),
        "the CEO's commit sits in its row"
    );
    assert_eq!(
        right,
        culprit % 3,
        "its terminal is the row's, three terminals for six rows"
    );
    assert_eq!(stage_of(&events).map(|s| s.0), Some(2), "stage: revert");
    assert!(units_of(&sim, REWRITE).is_empty(), "the rewrites pause");
    // The commit has a person in it, and the stamp, not "ago".
    assert!(!log[culprit].contains("ago"));
}

#[test]
fn a_wrong_pick_reverts_an_innocent_commit_and_costs_the_picker() {
    let (mut sim, ids, right, _events) = at_terminals("full", &TRIO, 7);
    let wrong = (right + 1) % 3;
    // Ilmari's health goes back by 3 %: put the bar at the floor first.
    bar_at(&mut sim, 20);
    run(&mut sim, 1);
    let live_id = live_screen(&sim);
    let hp = sim.unit(live_id).expect("screen").hp;
    sim.apply_as(
        ids[0],
        Command::Interact {
            prop: format!("il_git_{}", wrong + 1),
        },
    );
    let mut events = sim.tick();
    let out = events.iter().find_map(|e| match e {
        Event::Interacted { outcome, .. } => Some(*outcome),
        _ => None,
    });
    assert_eq!(out, Some(1), "a wrong pick");
    assert!(
        has_aura(&sim, ids[0], "il_merge_conflict"),
        "Merge Conflict on the picker"
    );
    let max = sim.unit(live_id).expect("screen").max_hp;
    assert!(
        sim.unit(live_id).expect("screen").hp >= hp + max * 3 / 100 - 2,
        "Ilmari heals 3 %"
    );
    events.extend(run(&mut sim, 3));
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Yell { text, .. } if text.starts_with("Reverted: ")
        )),
        "Reverted: the commit it holds"
    );
    // The picker cannot use a terminal while in conflict; the used one is consumed for 10 s.
    assert_eq!(git(&mut sim, ids[0], right), 0, "Merge Conflict");
    assert_eq!(git(&mut sim, ids[1], wrong), 0, "the terminal is down");
    // After the conflict (10 s) the picker is free again.
    run(&mut sim, 11);
    assert!(!has_aura(&sim, ids[0], "il_merge_conflict"));
}

#[test]
fn story_marks_the_culprit_after_a_wrong_pick_and_forgives_it_sooner() {
    let (mut sim, ids, right, events) = at_terminals("story", &["backend"], 7);
    assert!(
        !last_log(&events).iter().any(|l| l.contains("<- this one")),
        "no marker before a wrong pick"
    );
    let wrong = (right + 1) % 3;
    sim.apply_as(
        ids[0],
        Command::Interact {
            prop: format!("il_git_{}", wrong + 1),
        },
    );
    let events = sim.tick();
    let marked = last_log(&events);
    assert!(
        marked
            .iter()
            .any(|l| l.contains("b210a76") && l.contains("<- this one")),
        "the culprit is marked: {marked:?}"
    );
    run(&mut sim, 6);
    assert!(
        !has_aura(&sim, ids[0], "il_merge_conflict"),
        "Story: 5 s of conflict"
    );
}

#[test]
fn three_wrong_picks_raise_a_force_push_and_thirty_seconds_of_protection() {
    let (mut sim, ids, right, _events) = at_terminals("full", &TRIO, 7);
    let (w1, w2) = ((right + 1) % 3, (right + 2) % 3);
    assert_eq!(git(&mut sim, ids[0], w1), 1);
    assert_eq!(git(&mut sim, ids[1], w2), 1);
    // The first wrong terminal comes back after 10 s.
    run(&mut sim, 11);
    sim.apply_as(
        ids[2],
        Command::Interact {
            prop: format!("il_git_{}", w1 + 1),
        },
    );
    let events = run(&mut sim, 3);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Yell { text, .. } if text.contains("Force push")
        )),
        "the third raises a Force Push"
    );
    let events = run(&mut sim, 6);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::TelegraphStarted { spell, .. } if spell.0 == 2763))
            || screen_casts(&sim, &events).contains(&2763),
        "a room-wide volley"
    );
    let (stage, clock, _pct, wrong) = stage_of(&events).expect("wall");
    assert_eq!((stage, wrong), (1, 0), "protection again");
    assert!((15..=30).contains(&clock), "30 s on the clock: {clock}");
    // Terminals are locked while it runs.
    assert_eq!(git(&mut sim, ids[0], right), 0, "dark during protection");
}

#[test]
fn the_right_pick_is_the_last_commit_and_the_core_falls_to_it() {
    let (mut sim, ids, right, _events) = at_terminals("full", &TRIO, 7);
    sim.apply_as(
        ids[0],
        Command::Interact {
            prop: format!("il_git_{}", right + 1),
        },
    );
    let events = sim.tick();
    let out = events.iter().find_map(|e| match e {
        Event::Interacted { outcome, .. } => Some(*outcome),
        _ => None,
    });
    assert_eq!(out, Some(combat_core::PROP_OK), "the player's git revert");
    let log = last_log(&events);
    let name = sim.unit(ids[0]).expect("p").name.clone();
    assert_eq!(
        log.first().map(String::as_str),
        Some(format!("Author: {name} (the last dev)").as_str()),
        "the new HEAD is in the picker's name: {log:?}"
    );
    assert!(
        sim.unit(ward_of(&sim)).is_some(),
        "the mainframe stands through the beat"
    );
    // The beat, then the fall.
    let mut all = Vec::new();
    for _ in 0..14 * SECOND {
        all.extend(sim.tick());
    }
    assert!(
        all.iter()
            .any(|e| matches!(e, Event::Yell { text, .. } if text.contains("Shutting down"))),
        "Ilmari's last log lines"
    );
    assert!(
        all.iter().any(|e| matches!(e, Event::IlmariEnd { .. })),
        "the end event"
    );
    for s in SCREENS {
        assert!(
            sim.units().all(|u| u.key != s || !u.alive),
            "every screen is down"
        );
    }
    assert!(sim.ilmari_full_cleared(), "a Full kill opens Judgment Day");
    // The loot: the tier piece and the Legendary roll come from the core, fragments are paid.
    let drops: Vec<String> = all
        .iter()
        .flat_map(|e| match e {
            Event::LootDropped { items, .. } => items.clone(),
            _ => Vec::new(),
        })
        .collect();
    assert!(!drops.is_empty(), "the core drops gear");
    for d in &drops {
        let g = sim.content().economy.gear(d).expect("gear");
        assert_eq!(g.drops_from.as_deref(), Some("il_ilmari"), "{d}");
    }
    for &p in &ids {
        let n = sim
            .unit(p)
            .and_then(|u| {
                u.inventory
                    .iter()
                    .find(|(n, _)| n == "Legendary Fragment")
                    .map(|(_, c)| *c)
            })
            .unwrap_or(0);
        assert_eq!(n, 1, "one Legendary Fragment on Full");
    }
    // The clock stamp is taken once, after the fall.
    sim.apply_as(
        ids[0],
        Command::StampIlmariEnd {
            unix_secs: THURSDAY + 4000,
        },
    );
    sim.tick();
    assert!(sim.ilmari_end().is_some(), "the end is stamped");
}

#[test]
fn story_and_judgment_kills_do_not_open_judgment_day_and_pay_their_own_fragments() {
    let (mut sim, ids, right, _e) = at_terminals("story", &["backend"], 7);
    sim.apply_as(
        ids[0],
        Command::Interact {
            prop: format!("il_git_{}", right + 1),
        },
    );
    run(&mut sim, 14);
    assert!(!sim.ilmari_full_cleared(), "Story does not");
    let has_fragment = sim
        .unit(ids[0])
        .is_some_and(|u| u.inventory.iter().any(|(n, _)| n == "Legendary Fragment"));
    assert!(!has_fragment, "no Story fragments");
}

#[test]
fn judgment_names_the_commit_by_its_clock_and_lets_each_member_read_once() {
    let mut c = content();
    for k in [
        "senior_engineer",
        "backend_dev",
        "devops_eng",
        "frontend_dev",
    ] {
        def_mut(&mut c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
    let mut sim = Sim::new(c, 33);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids: Vec<EntityId> = (0..3)
        .map(|k| {
            let p = sim.spawn_player(TRIO[k], Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, 10);
            p
        })
        .collect();
    sim.debug_ilmari_core_cleared(true);
    sim.apply_as(
        ids[0],
        Command::SetIlmariMode(combat_core::ilmari::IlmariMode::Judgment),
    );
    sim.tick();
    sim.load_zone("ilmari_core/1", &anchors, Vec::new())
        .expect("core");
    let boss = unit_by_key(&sim, BOSS).expect("boss");
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(2.0, 0.0, CENTRE_Z + 4.0));
    }
    pull(&mut sim, &ids, boss);
    run(&mut sim, 5);
    play_rounds(&mut sim, &ids, [0, 1, 2]);
    bar_at(&mut sim, 19);
    run(&mut sim, 2);
    assert_eq!(phase_of(&sim), 4);
    let ward = ward_of(&sim);
    let mut events = Vec::new();
    let mut hint = false;
    for _ in 0..220 * SECOND {
        for e in sim.tick() {
            if let Event::Yell { unit, text } = &e
                && sim.unit(*unit).is_some_and(|u| u.key == "il_marie_core")
                && text.contains("Look for the clock")
            {
                hint = true;
            }
            events.push(e);
        }
        sim.debug_set_hp(ward, 24_000);
        if hint {
            break;
        }
    }
    assert!(hint, "Marie says what to look for once the terminals light");
    let right = events
        .iter()
        .find_map(|e| match e {
            Event::CoreLog { lines } => lines
                .iter()
                .find(|l| l.contains("b210a76"))
                .and_then(|l| l.chars().nth(1))
                .and_then(|c| c.to_digit(10))
                .map(|d| d as usize - 1),
            _ => None,
        })
        .expect("log");
    let wrong = (right + 1) % 3;
    assert_eq!(git(&mut sim, ids[0], wrong), 1);
    // The same member does not read twice, however long the conflict lasts; another member can.
    run(&mut sim, 15);
    assert_eq!(
        git(&mut sim, ids[0], right),
        0,
        "one reader at a time on Judgment"
    );
    assert_eq!(
        git(&mut sim, ids[1], right),
        combat_core::PROP_OK,
        "a different member reads"
    );
}

#[test]
fn phases_three_and_four_replay_bit_identically() {
    let log = |seed: u64| {
        let (mut sim, _ids, _boss) = at_p3("full", &TRIO, seed, [0, 1, 2]);
        let mut out = String::new();
        for k in 0..150 * SECOND {
            let now = sim.now().0;
            for e in sim.tick() {
                out.push_str(&log_line(&sim, now, &e));
                out.push('\n');
            }
            if k == 100 * SECOND {
                bar_at(&mut sim, 19);
            }
        }
        out
    };
    let (a, b) = (log(5), log(5));
    assert!(a.len() > 20_000);
    assert!(a.contains("Deprecation Notice"), "the run reached P4");
    assert_eq!(a, b, "same seed, same fight");
    assert_ne!(a, log(6), "another seed differs");
}
