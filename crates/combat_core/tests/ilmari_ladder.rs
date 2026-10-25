//! The Ilmari ladder (docs/ILMARI_1_0.md D.10, task D16): the core's two weekly boards, the Async
//! tab, the week roll, the crew string through a save, the weekly Directive, the rank titles and
//! the pay. Rows come from the sim only: these tests kill the core's boss and read what the sim wrote.
#![allow(clippy::expect_used)]

mod common;

use combat_core::ilmari::IlmariMode;
use combat_core::ladder::{LadderBoard, directive_index};
use combat_core::sprint::{LADDER_LABEL_MAX, LADDER_SIZE, LadderEntry};
use combat_core::{Command, Content, EntityId, Event, MAX_PLAYERS, SaveData, Sim, SpellId, Vec3};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const DAY: u64 = 86_400;
const BOSS: &str = "il_ilmari";
const BOSS_Z: f32 = -40.0;
const CLASSES: [&str; 3] = ["backend", "devops", "fullstack"];

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, BOSS_Z),
        "npc_marie" => at(0.0, BOSS_Z - 17.0),
        n if n.starts_with("prop:") => at(1.0, BOSS_Z - 11.0),
        _ => at(0.0, 0.0),
    }
}

/// A room of `n` sturdy players in the core on `mode`, the fight just pulled.
fn core_room(mode: IlmariMode, n: usize) -> (Sim, Vec<EntityId>) {
    let mut c = content();
    let units: Vec<String> = c.classes.iter().map(|k| k.unit.clone()).collect();
    for k in &units {
        def_mut(&mut c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
    let mut sim = Sim::new(c, 33);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids: Vec<EntityId> = (0..n)
        .map(|k| {
            let p = sim.spawn_player(CLASSES[k % 3], Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, 10);
            sim.set_ladder_name(p, ["Ada", "Linus", "Grace"][k % 3]);
            p
        })
        .collect();
    sim.debug_ilmari_core_cleared(true);
    sim.apply_as(ids[0], Command::SetIlmariMode(mode));
    sim.tick();
    sim.load_zone("ilmari_core/1", &anchors, Vec::new())
        .expect("ilmari_core/1");
    for &p in &ids {
        sim.apply_as(
            p,
            Command::PlayerMoved {
                pos: Vec3::new(2.0, 0.0, BOSS_Z - 13.0),
                yaw: 0.0,
                moving: false,
                jumping: false,
            },
        );
    }
    let boss = boss_of(&sim);
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
    run(&mut sim, 1);
    (sim, ids)
}

fn run(sim: &mut Sim, ticks: u32) {
    for _ in 0..ticks {
        sim.tick();
    }
}

fn boss_of(sim: &Sim) -> EntityId {
    sim.units()
        .find(|u| u.key == BOSS && u.alive)
        .map(|u| u.id)
        .expect("boss")
}

/// The fight lasts `secs` seconds of the sim's clock, then the core falls. The events of the kill.
fn kill_after(sim: &mut Sim, secs: u32) -> Vec<Event> {
    run(sim, secs * 30);
    let boss = boss_of(sim);
    assert!(sim.debug_kill(boss));
    sim.tick()
}

fn board(sim: &Sim, b: LadderBoard) -> Vec<LadderEntry> {
    sim.ilmari_ladder_view(b).entries
}

fn fresh_with(save: &SaveData) -> Sim {
    let mut sim = Sim::new(content(), 5);
    sim.set_clock(THURSDAY);
    let q = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(sim.import_save(q, save));
    sim
}

#[test]
fn a_full_kill_writes_the_full_board_and_a_judgment_kill_the_judgment_board_and_story_none() {
    for (mode, n, want) in [
        (IlmariMode::Full, 3, Some(LadderBoard::IlmariFull)),
        (IlmariMode::Judgment, 3, Some(LadderBoard::IlmariJudgment)),
        (IlmariMode::Story, 1, None),
    ] {
        let (mut sim, _ids) = core_room(mode, n);
        let ev = kill_after(&mut sim, 40);
        for b in LadderBoard::ILMARI {
            let rows = board(&sim, b);
            if Some(b) == want {
                assert_eq!(rows.len(), 1, "{mode:?} on {b:?}");
                let e = &rows[0];
                assert!(
                    e.time_ms.abs_diff(40_000) < 2_500,
                    "the sim's clock from the pull: {}",
                    e.time_ms
                );
                assert_eq!(e.deaths, 0);
                assert!(!e.is_async, "a group row");
                assert_eq!(
                    e.crew,
                    [
                        "Ada (Backend Engineer)",
                        "Linus (DevOps Engineer)",
                        "Grace (Fullstack Engineer)"
                    ]
                );
                assert!(
                    ev.iter()
                        .any(|x| matches!(x, Event::Ladder { board, .. } if *board == b)),
                    "the board is announced"
                );
            } else {
                assert!(rows.is_empty(), "{mode:?} leaves {b:?} alone");
            }
        }
        if want.is_none() {
            assert!(
                !ev.iter().any(|x| matches!(x, Event::Ladder { .. })),
                "Story has no ladder"
            );
        }
    }
}

#[test]
fn deaths_in_the_pull_are_counted_and_break_ties_then_the_earlier_row_wins() {
    let (mut sim, ids) = core_room(IlmariMode::Full, 3);
    run(&mut sim, 60);
    assert!(sim.debug_kill_player(ids[2]));
    kill_after(&mut sim, 10);
    let rows = board(&sim, LadderBoard::IlmariFull);
    assert_eq!(rows[0].deaths, 1, "one fell in the pull");
    // Same time: fewer deaths first, then whoever came first.
    let b = LadderBoard::IlmariFull;
    let t = rows[0].time_ms;
    sim.debug_ilmari_ladder_add(b, t, 0, &["Clean Crew"], false);
    sim.debug_ilmari_ladder_add(b, t, 0, &["Later Clean Crew"], false);
    let order: Vec<_> = board(&sim, b)
        .iter()
        .map(|e| (e.deaths, e.crew[0].clone()))
        .collect();
    assert_eq!(order[0], (0, "Clean Crew".to_string()));
    assert_eq!(order[1], (0, "Later Clean Crew".to_string()));
    assert_eq!(order[2].0, 1);
}

#[test]
fn a_lone_full_run_lands_on_the_async_tab_and_never_pushes_a_group_row_off() {
    let (mut sim, _ids) = core_room(IlmariMode::Full, 1);
    kill_after(&mut sim, 20);
    let rows = board(&sim, LadderBoard::IlmariFull);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_async, "fewer than three on Full: the Async tab");
    // A full tab of async rows and a few group rows: both keep their own ten.
    let b = LadderBoard::IlmariFull;
    for k in 0..(LADDER_SIZE as u32 + 4) {
        sim.debug_ilmari_ladder_add(b, 100_000 + k, 0, &["Solo"], true);
    }
    for k in 0..3 {
        sim.debug_ilmari_ladder_add(b, 900_000 + k, 2, &["Crew A", "Crew B", "Crew C"], false);
    }
    let rows = board(&sim, b);
    let asyncs = rows.iter().filter(|e| e.is_async).count();
    assert_eq!(asyncs, LADDER_SIZE, "the Async tab is bounded");
    assert_eq!(
        rows.iter().filter(|e| !e.is_async).count(),
        3,
        "slow group rows survive faster solo rows"
    );
}

#[test]
fn a_new_week_empties_the_boards_and_the_directive_follows_the_week_seed() {
    let (mut sim, _ids) = core_room(IlmariMode::Judgment, 3);
    kill_after(&mut sim, 15);
    let before = sim.ilmari_ladder_view(LadderBoard::IlmariJudgment);
    assert_eq!(before.entries.len(), 1);
    assert!(
        !before.directive.is_empty(),
        "the board names the Directive"
    );
    sim.set_clock(THURSDAY + 7 * DAY);
    let after = sim.ilmari_ladder_view(LadderBoard::IlmariJudgment);
    assert!(after.entries.is_empty(), "a new week, a new board");
    assert_eq!(after.week, before.week + 1);
    // A row of the new week replaces the stale rows instead of joining them.
    sim.debug_ilmari_ladder_add(LadderBoard::IlmariJudgment, 5, 0, &["Next Week"], false);
    assert_eq!(board(&sim, LadderBoard::IlmariJudgment).len(), 1);

    // The Directive is a pure function of the week and the table, and the table comes up in turn.
    let n = sim.content().ilmari.ladder.directives.len();
    assert_eq!(n, 4);
    let picks: Vec<usize> = (2_900..2_920)
        .map(|w| directive_index(w, n).expect("table"))
        .collect();
    assert_eq!(
        picks,
        (2_900..2_920)
            .map(|w| directive_index(w, n).expect("table"))
            .collect::<Vec<_>>()
    );
    assert!(
        (0..n).all(|k| picks.contains(&k)),
        "every policy comes up inside twenty weeks: {picks:?}"
    );
    assert_eq!(directive_index(1, 0), None);
}

#[test]
fn the_directive_is_worn_through_a_ranked_fight_and_not_on_story() {
    for (mode, n, worn) in [
        (IlmariMode::Full, 3, true),
        (IlmariMode::Judgment, 3, true),
        (IlmariMode::Story, 1, false),
    ] {
        let (mut sim, ids) = core_room(mode, n);
        run(&mut sim, 30 * 20);
        let (aura, name) = sim.ilmari_directive().expect("a directive");
        for &p in &ids {
            let has = sim
                .unit(p)
                .expect("player")
                .auras
                .iter()
                .any(|a| a.key == aura);
            assert_eq!(has, worn, "{mode:?} wears {name}");
        }
    }
}

#[test]
fn the_boards_and_their_crews_survive_a_save_and_old_saves_load_empty() {
    let (mut sim, ids) = core_room(IlmariMode::Full, 3);
    kill_after(&mut sim, 25);
    let b = LadderBoard::IlmariJudgment;
    sim.debug_ilmari_ladder_add(
        b,
        700_000,
        3,
        &["A|B~C+D*E (Backend Engineer)", "Grace (DevOps Engineer)"],
        false,
    );
    let want_full = sim.ilmari_ladder_view(LadderBoard::IlmariFull);
    let want_jd = sim.ilmari_ladder_view(b);
    assert!(
        want_jd.entries[0].crew[0]
            .chars()
            .all(|c| !"|~+*".contains(c)),
        "separators never reach a row"
    );
    let text = sim.export_save(ids[0]).expect("save").encode();
    assert_eq!(
        text.lines()
            .filter(|l| l.starts_with("ilmari_ladder="))
            .count(),
        2,
        "one line per board"
    );
    let fresh = fresh_with(&SaveData::decode(&text).expect("decode"));
    assert_eq!(
        board(&fresh, LadderBoard::IlmariFull),
        want_full.entries,
        "the crew string and the Async flag round-trip"
    );
    assert_eq!(board(&fresh, b), want_jd.entries);
    // A save without the lines (any build before D16) loads two empty boards.
    let old: String = text
        .lines()
        .filter(|l| !l.starts_with("ilmari_ladder="))
        .map(|l| format!("{l}\n"))
        .collect();
    let empty = fresh_with(&SaveData::decode(&old).expect("decode"));
    for b in LadderBoard::ILMARI {
        assert!(board(&empty, b).is_empty());
    }
}

#[test]
fn a_hand_edited_save_cannot_overfill_a_board() {
    let (mut sim, ids) = core_room(IlmariMode::Full, 3);
    kill_after(&mut sim, 5);
    let text = sim.export_save(ids[0]).expect("save").encode();
    let crew = vec!["x".repeat(500); 40].join("+");
    let rows: Vec<String> = (0..50).map(|k| format!("{}~0~{crew}", 1000 + k)).collect();
    let week = combat_core::live::week_index(THURSDAY);
    let mut forged: String = text
        .lines()
        .filter(|l| !l.starts_with("ilmari_ladder="))
        .map(|l| format!("{l}\n"))
        .collect();
    forged.push_str(&format!("ilmari_ladder={week}*1*{}\n", rows.join("|")));
    let fresh = fresh_with(&SaveData::decode(&forged).expect("decode"));
    let rows = board(&fresh, LadderBoard::IlmariFull);
    assert_eq!(rows.len(), LADDER_SIZE);
    for e in &rows {
        assert!(e.crew.len() <= MAX_PLAYERS);
        assert!(e.crew.iter().all(|c| c.chars().count() <= LADDER_LABEL_MAX));
    }
}

#[test]
fn a_ranked_kill_pays_the_entry_tokens_and_the_top_three_get_their_rank_title() {
    let tokens = |sim: &Sim, p: EntityId| {
        sim.unit(p)
            .and_then(|u| {
                u.inventory
                    .iter()
                    .find(|(n, _)| n == combat_core::ilmari::TOKEN)
                    .map(|(_, c)| *c)
            })
            .unwrap_or(0)
    };
    let (mut sim, ids) = core_room(IlmariMode::Full, 3);
    let pay = sim.content().ilmari.ladder.entry_tokens;
    assert_eq!(pay, 15);
    let before: Vec<u32> = ids.iter().map(|&p| tokens(&sim, p)).collect();
    kill_after(&mut sim, 30);
    for (k, &p) in ids.iter().enumerate() {
        assert!(
            tokens(&sim, p) >= before[k] + pay,
            "15 tokens for every member of a ranked kill"
        );
        let got = &sim.unit(p).expect("player").achievements;
        assert!(got.iter().any(|a| a == "il_ladder_1"), "first: {got:?}");
    }
    // A faster crew was already on the board: second place, its own title.
    let (mut again, ids2) = core_room(IlmariMode::Full, 3);
    again.debug_ilmari_ladder_add(LadderBoard::IlmariFull, 1, 0, &["Faster Crew"], false);
    kill_after(&mut again, 30);
    for &p in &ids2 {
        let got = &again.unit(p).expect("player").achievements;
        assert!(got.iter().any(|a| a == "il_ladder_2"), "second: {got:?}");
        assert!(!got.iter().any(|a| a == "il_ladder_1"));
    }
    // Async rows are paid but never ranked.
    let (mut solo, ids3) = core_room(IlmariMode::Full, 1);
    kill_after(&mut solo, 30);
    let got = &solo.unit(ids3[0]).expect("player").achievements;
    assert!(!got.iter().any(|a| a.starts_with("il_ladder_")), "{got:?}");
}

#[test]
fn ladder_content_refuses_a_bad_directive_and_an_overpaid_entry() {
    let ron = include_str!("../../../content/game.ron");
    assert!(Content::from_ron(ron).is_ok());
    for (from, to) in [
        (
            "directives: [\"dir_rto\",",
            "directives: [\"no_such_policy\",",
        ),
        ("entry_tokens: 15", "entry_tokens: 201"),
    ] {
        assert!(ron.contains(from), "{from}");
        assert!(
            Content::from_ron(&ron.replacen(from, to, 1)).is_err(),
            "{from}"
        );
    }
}
