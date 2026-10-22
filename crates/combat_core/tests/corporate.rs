//! The Company Calendar's minigames (`corporate/*.rs`): Performance Review, Reorg, Tool
//! Migration, Hackathon and Open-Space Focus. Per game: the start from the campus, the core
//! rule, scoring and medals, a solo run to the end, the co-op group score, and rewards that
//! land once. The bots below play from the HUD view only, as a player would.

mod common;

use combat_core::corporate::focus::FocusDef;
use combat_core::corporate::migration::MigrationDef;
use combat_core::corporate::reorg::ReorgDef;
use combat_core::corporate::review::{ReviewDef, Tone};
use combat_core::{Command, EntityId, Event, Medal, MinigameResult, MinigameView, Sim, Vec3};
use common::content;

const SECOND: u32 = 30;

type Anchors = Box<dyn Fn(&str) -> Option<(Vec3, f32)>>;

fn at(x: f32, z: f32) -> Option<(Vec3, f32)> {
    Some((Vec3::new(x, 0.0, z), 0.0))
}

/// Campus props and boards next to the player, packs far away.
fn campus(name: &str) -> Option<(Vec3, f32)> {
    match name {
        n if n.starts_with("pack_")
            || n.starts_with("rare_")
            || n.starts_with("event:")
            || n == "boss" =>
        {
            at(400.0, 400.0)
        }
        n if n.starts_with("npc_") || n.starts_with("secret:") || n.starts_with("night_") => {
            at(60.0, 60.0)
        }
        "world_boss" | "graveyard" => at(0.0, -60.0),
        _ => at(1.0, 0.0),
    }
}

fn run(sim: &mut Sim, ticks: u32) -> Vec<Event> {
    (0..ticks).flat_map(|_| sim.tick()).collect()
}

fn on_campus(players: usize) -> (Sim, Vec<EntityId>) {
    let mut sim = Sim::new(content(), 7);
    sim.set_wandering(false);
    let ids: Vec<EntityId> = (0..players)
        .map(|_| sim.spawn_player("fullstack", Vec3::default(), 0.0))
        .collect();
    if let Err(e) = sim.load_zone("campus", &campus, Vec::new()) {
        panic!("{e}");
    }
    run(&mut sim, 1);
    (sim, ids)
}

fn back_to_campus(sim: &mut Sim) {
    if let Err(e) = sim.load_zone("campus", &campus, Vec::new()) {
        panic!("{e}");
    }
    run(sim, 1);
}

/// The lead starts `key`; the host follows the travel into `anchors`.
fn enter(sim: &mut Sim, lead: EntityId, key: &str, anchors: &Anchors) -> Vec<Event> {
    let zone = content()
        .minigame(key)
        .map(|m| m.zone.clone())
        .unwrap_or_else(|| panic!("no minigame {key}"));
    sim.apply_as(lead, Command::StartMinigame { key: key.into() });
    let mut out = Vec::new();
    for _ in 0..5 * SECOND {
        out.extend(sim.tick());
        if sim.minigame_travel() == Some(zone.as_str()) {
            break;
        }
    }
    assert_eq!(sim.minigame_travel(), Some(zone.as_str()));
    if let Err(e) = sim.load_zone(&zone, anchors.as_ref(), Vec::new()) {
        panic!("{e}");
    }
    out.extend(run(sim, 2));
    out
}

fn ended(events: &[Event]) -> Vec<MinigameResult> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::MinigameEnded { result } => Some(result.clone()),
            _ => None,
        })
        .collect()
}

fn notes(events: &[Event], p: EntityId) -> Vec<(String, bool)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::MinigameNote { unit, text, good } if *unit == p => Some((text.clone(), *good)),
            _ => None,
        })
        .collect()
}

fn move_to(sim: &mut Sim, p: EntityId, x: f32, z: f32) {
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos: Vec3::new(x, 0.0, z),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
}

/// Holds `p` at (x, z) for `ticks`.
fn hold(sim: &mut Sim, p: EntityId, x: f32, z: f32, ticks: u32) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        move_to(sim, p, x, z);
        out.extend(sim.tick());
    }
    out
}

fn view(sim: &Sim, p: EntityId) -> MinigameView {
    sim.minigame_view(p)
        .cloned()
        .unwrap_or_else(|| panic!("no view for {p}"))
}

/// Picks option `option` of `p`'s current card and ticks once.
fn pick(sim: &mut Sim, p: EntityId, option: u8) -> Vec<Event> {
    let seq = view(sim, p)
        .card
        .unwrap_or_else(|| panic!("no card for {p}"))
        .seq;
    sim.apply_as(p, Command::MinigamePick { seq, option });
    run(sim, 1)
}

fn bag_count(sim: &Sim, p: EntityId, item: &str) -> usize {
    sim.unit(p)
        .map_or(0, |u| u.bag.iter().filter(|b| *b == item).count())
}

fn has_achievement(sim: &Sim, p: EntityId, key: &str) -> bool {
    sim.unit(p)
        .is_some_and(|u| u.achievements.iter().any(|a| a == key))
}

// ---- Performance Review ----

fn review_def() -> ReviewDef {
    content()
        .minigame("performance_review")
        .and_then(|m| m.review.clone())
        .unwrap_or_else(|| panic!("no review rules"))
}

fn review_venue() -> Anchors {
    Box::new(|n: &str| match n {
        "player_spawn" | "graveyard" => at(0.0, 2.0),
        "npc_manager" => at(0.0, -1.5),
        _ => None,
    })
}

/// The tones that burn the most calendar with the manager staying (search over all answers).
fn best_tones(d: &ReviewDef) -> Vec<Tone> {
    fn go(
        d: &ReviewDef,
        q: usize,
        patience: i32,
        last: Option<Tone>,
        mins: u32,
        best: &mut (u32, Vec<Tone>),
        path: &mut Vec<Tone>,
    ) {
        if q == d.questions.len() {
            if mins > best.0 {
                *best = (mins, path.clone());
            }
            return;
        }
        for a in &d.questions[q].answers {
            let p = (patience - d.cost(a.tone, last)).min(d.patience);
            if p <= 0 {
                continue;
            }
            let m = mins + d.tone(a.tone).map_or(0, |t| t.minutes);
            path.push(a.tone);
            go(d, q + 1, p, Some(a.tone), m, best, path);
            path.pop();
        }
    }
    let mut best = (0, Vec::new());
    go(d, 0, d.patience, None, 0, &mut best, &mut Vec::new());
    assert_eq!(best.0, d.best_minutes());
    best.1
}

/// The option on the card that says something of `tone`, if the question offers one.
fn option_of(d: &ReviewDef, v: &MinigameView, tone: Tone) -> Option<u8> {
    let card = v.card.as_ref()?;
    let q = d.questions.iter().find(|q| q.ask == card.text)?;
    let a = q.answers.iter().find(|a| a.tone == tone)?;
    card.options
        .iter()
        .position(|o| *o == a.text)
        .map(|i| i as u8)
}

/// Plays the review answering with `choose` (question index -> tone; `None` stays silent).
fn play_review(
    sim: &mut Sim,
    p: EntityId,
    mut choose: impl FnMut(usize) -> Option<Tone>,
) -> Vec<Event> {
    let d = review_def();
    let mut out = Vec::new();
    for _ in 0..600 * SECOND {
        if !ended(&out).iter().all(|r| r.unit != p) {
            break;
        }
        let Some(v) = sim.minigame_view(p).cloned() else {
            out.extend(run(sim, 1));
            continue;
        };
        let Some(card) = v.card.clone() else {
            out.extend(run(sim, 1));
            continue;
        };
        let asked = d.questions.iter().position(|q| q.ask == card.text);
        let option = match asked {
            Some(q) => match choose(q) {
                Some(t) => option_of(&d, &v, t),
                None => None,
            },
            None => Some(0),
        };
        match option {
            Some(o) => out.extend(pick(sim, p, o)),
            None => out.extend(run(sim, 1)),
        }
    }
    out
}

#[test]
fn review_opens_with_the_manager_and_the_gauge_in_the_middle() {
    let (mut sim, ids) = on_campus(1);
    let events = enter(&mut sim, ids[0], "performance_review", &review_venue());
    assert!(
        events.iter().any(
            |e| matches!(e, Event::MinigameStarted { key, .. } if key == "performance_review")
        )
    );
    let v = view(&sim, ids[0]);
    let card = v.card.clone().unwrap_or_default();
    assert_eq!(card.text, review_def().opener);
    assert_eq!(v.gauges[0].value, 50);
    assert_eq!(v.gauges[0].mark, Some(50));
    assert_eq!(v.gauges[1].value, 100);
}

#[test]
fn whatever_you_say_the_rating_drifts_back_to_three() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "performance_review", &review_venue());
    pick(&mut sim, p, 0);
    let d = review_def();
    let o = option_of(&d, &view(&sim, p), Tone::Buzz).unwrap_or_else(|| panic!("no buzz"));
    pick(&mut sim, p, o);
    let up = view(&sim, p).gauges[0].value;
    assert!(up > 50, "{up}");
    run(&mut sim, 4 * SECOND);
    assert_eq!(view(&sim, p).gauges[0].value, 50);
    // An honest answer pushes it down; it comes back up all the same.
    pick(&mut sim, p, 0);
    if let Some(o) = option_of(&d, &view(&sim, p), Tone::Honest) {
        pick(&mut sim, p, o);
        assert!(view(&sim, p).gauges[0].value < 50);
        run(&mut sim, 4 * SECOND);
        assert_eq!(view(&sim, p).gauges[0].value, 50);
    }
}

#[test]
fn asking_for_the_raise_every_time_runs_out_of_patience_early() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "performance_review", &review_venue());
    let d = review_def();
    let raise_or_stall = |q: usize| {
        let has = |t| d.questions[q].answers.iter().any(|a| a.tone == t);
        Some(if has(Tone::Raise) {
            Tone::Raise
        } else {
            Tone::Stall
        })
    };
    let events = play_review(&mut sim, p, raise_or_stall);
    let r = ended(&events);
    assert_eq!(r.len(), 1);
    // The manager walked out: half the calendar counts, and no Gold whatever the minutes said.
    assert_ne!(r[0].medal, Some(Medal::Gold));
    assert!(r[0].medal.is_some());
    assert!(r[0].summary.contains("3 out of 5"), "{}", r[0].summary);
    assert!(r[0].summary.contains("offline"), "{}", r[0].summary);
    let d = review_def();
    assert_eq!(
        r[0].score,
        u32::from(r[0].collected) * d.per_minute * d.walkout_pct / 100
    );
}

#[test]
fn silence_answers_for_you_and_fills_the_calendar_a_little() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "performance_review", &review_venue());
    pick(&mut sim, p, 0);
    run(&mut sim, 21 * SECOND);
    let v = view(&sim, p);
    assert_eq!(v.catches, 1, "one silence");
    let silence = review_def().tone(Tone::Silence).map_or(0, |t| t.minutes);
    assert_eq!(u32::from(v.collected), silence);
}

#[test]
fn the_best_answers_take_gold_and_the_mug_lands_once() {
    let d = review_def();
    let medals = content()
        .minigame("performance_review")
        .map(|m| m.medals.clone())
        .unwrap_or_default();
    let best = d.best_minutes() * d.per_minute;
    assert!(
        best >= medals[2].min_score,
        "gold {} > best {best}",
        medals[2].min_score
    );
    assert!(medals[1].min_score < medals[2].min_score);
    let tones = best_tones(&d);

    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    sim.debug_set_burnout(p, 500);
    enter(&mut sim, p, "performance_review", &review_venue());
    let events = play_review(&mut sim, p, |q| tones.get(q).copied());
    let r = ended(&events);
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].medal, Some(Medal::Gold), "{:?}", r[0]);
    assert_eq!(r[0].score, best);
    assert_eq!(r[0].items, vec!["Mug: Meets Expectations".to_string()]);
    assert!(has_achievement(&sim, p, "meets_expectations"));
    assert_eq!(
        sim.unit(p).map(|u| u.title.clone()),
        Some("meets_expectations".into())
    );
    assert_eq!(sim.unit(p).map(|u| u.career.burnout), Some(500 - 120));
    assert_eq!(
        sim.minigame_best(p, "performance_review"),
        Some(Medal::Gold)
    );

    back_to_campus(&mut sim);
    enter(&mut sim, p, "performance_review", &review_venue());
    let again = ended(&play_review(&mut sim, p, |q| tones.get(q).copied()));
    assert_eq!(again[0].medal, Some(Medal::Gold));
    assert!(again[0].items.is_empty());
    assert_eq!(bag_count(&sim, p, "Mug: Meets Expectations"), 1);
}

#[test]
fn a_review_group_answers_on_its_own_and_scores_together() {
    let (mut sim, ids) = on_campus(2);
    let (a, b) = (ids[0], ids[1]);
    enter(&mut sim, a, "performance_review", &review_venue());
    let d = review_def();
    let tones = best_tones(&d);
    let first = ended(&play_review(&mut sim, a, |q| tones.get(q).copied()));
    assert_eq!(first.len(), 1);
    // B has not answered anything yet: still on the opener, and no trip back yet.
    assert_eq!(view(&sim, b).card.map(|c| c.text), Some(d.opener.clone()));
    run(&mut sim, 11 * SECOND);
    assert_eq!(sim.minigame_travel(), None);
    let second = ended(&play_review(&mut sim, b, |_| Some(Tone::Agree)));
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].group_score, first[0].score + second[0].score);
    assert!(first[0].score > second[0].score);
}

// ---- Reorg ----

fn reorg_def() -> ReorgDef {
    content()
        .minigame("reorg")
        .and_then(|m| m.reorg.clone())
        .unwrap_or_else(|| panic!("no reorg rules"))
}

fn desk(k: usize) -> (f32, f32) {
    (k as f32 * 10.0, -10.0)
}

fn reorg_venue() -> Anchors {
    Box::new(|n: &str| match n {
        "player_spawn" | "graveyard" => at(0.0, 0.0),
        n => {
            let k: usize = n.strip_prefix("desk_")?.parse().ok()?;
            let (x, z) = desk(k);
            at(x, z)
        }
    })
}

/// What a filled-in template says in place of its tag.
fn fill<'a>(templates: &[String], tag: &str, text: &'a str) -> Option<&'a str> {
    templates.iter().find_map(|t| {
        let (pre, post) = t.split_once(tag)?;
        text.strip_prefix(pre)?.strip_suffix(post)
    })
}

/// Visits every used desk, reads the clues and asks the colleague they point at. Returns the
/// events, or `None` if no clue gave the manager away.
fn solve_round(sim: &mut Sim, p: EntityId, d: &ReorgDef) -> Option<Vec<Event>> {
    let mut out = Vec::new();
    // Wait out a reorg in progress.
    for _ in 0..10 * SECOND {
        if !view(sim, p).lines[1].contains("IN PROGRESS") {
            break;
        }
        out.extend(run(sim, 1));
    }
    let layout = view(sim, p).layout;
    let mut seen = Vec::new();
    for k in (0..16).filter(|k| layout & (1 << k) != 0) {
        let (x, z) = desk(k + 1);
        out.extend(hold(sim, p, x, z, 2));
        let card = view(sim, p).card?;
        seen.push((k, card.who.clone(), card.text.clone()));
    }
    let target = seen.iter().find_map(|(_, _, text)| {
        if let Some(slot) = fill(&d.place_clues, "{slot}", text) {
            return d.slots.iter().position(|s| s == slot);
        }
        let role = fill(&d.role_clues, "{role}", text)?;
        seen.iter()
            .find(|(_, who, _)| who.ends_with(&format!(", {role}")))
            .map(|(k, _, _)| *k)
    })?;
    let (x, z) = desk(target + 1);
    out.extend(hold(sim, p, x, z, 2));
    out.extend(pick(sim, p, 0));
    Some(out)
}

#[test]
fn reorg_badges_everyone_and_seats_them_at_desks() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    let events = enter(&mut sim, p, "reorg", &reorg_venue());
    assert!(!notes(&events, p).is_empty(), "the reorg is announced");
    let v = view(&sim, p);
    let d = reorg_def();
    assert_eq!(v.badges.len(), d.npcs.len());
    assert!(v.badges.iter().all(|(_, b)| b.contains(", ")));
    assert_eq!((v.layout & 0xFFFF).count_ones() as usize, d.npcs.len());
    assert!(v.lines[0].starts_with("Reorg 1/5"), "{:?}", v.lines);
    // Walking up to a colleague shows their clue and the question.
    let k = (0..16).find(|k| v.layout & (1 << k) != 0).unwrap_or(0);
    let (x, z) = desk(k + 1);
    hold(&mut sim, p, x, z, 2);
    let card = view(&sim, p).card.unwrap_or_default();
    assert_eq!(card.options, vec![d.ask.clone(), d.leave.clone()]);
    // "Thanks, I guess" puts the card away until you walk off.
    pick(&mut sim, p, 1);
    assert!(view(&sim, p).card.is_none());
}

#[test]
fn a_wrong_guess_costs_points_and_a_moment() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "reorg", &reorg_venue());
    let d = reorg_def();
    // Solve once for the score, then try everyone else in the next round until a wrong one.
    let first = solve_round(&mut sim, p, &d).unwrap_or_else(|| panic!("unsolvable"));
    assert!(
        notes(&first, p)
            .iter()
            .any(|n| n.1 && n.0.starts_with("Found"))
    );
    let score = view(&sim, p).score;
    assert!(score >= d.score.per_find);
    run(&mut sim, (d.settle_ms / 1000 + 5) * SECOND);
    assert!(view(&sim, p).lines[0].starts_with("Reorg 2/5"));
    let layout = view(&sim, p).layout;
    for k in (0..16).filter(|k| layout & (1 << k) != 0) {
        let (x, z) = desk(k + 1);
        hold(&mut sim, p, x, z, 2);
        // Your manager is as confused as anyone, but only they say one of these.
        let text = view(&sim, p).card.map(|c| c.text).unwrap_or_default();
        if d.manager_lines.contains(&text) {
            continue;
        }
        let events = pick(&mut sim, p, 0);
        if notes(&events, p).iter().any(|n| !n.1) {
            let v = view(&sim, p);
            assert_eq!(v.catches, 1);
            assert_eq!(v.score, score - d.score.per_wrong);
            assert!(v.card.is_some_and(|c| c.options.is_empty()), "locked");
            run(&mut sim, d.wrong_ms / 1000 * SECOND + SECOND);
            assert_eq!(view(&sim, p).card.map(|c| c.options.len()), Some(2));
            return;
        }
    }
    panic!("never guessed wrong");
}

#[test]
fn a_round_nobody_solves_ends_on_its_clock_and_the_org_chart_changes() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "reorg", &reorg_venue());
    let d = reorg_def();
    let before = view(&sim, p).badges;
    run(&mut sim, (d.round_ms / 1000 + 1) * SECOND);
    let v = view(&sim, p);
    assert!(v.lines[0].starts_with("Reorg 2/5"));
    assert!(v.lines[1].contains("IN PROGRESS"));
    run(&mut sim, (d.shuffle_ms / 1000 + 1) * SECOND);
    assert_ne!(view(&sim, p).badges, before);
}

#[test]
fn five_reorgs_solved_fast_and_clean_take_gold_and_the_title_once() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "reorg", &reorg_venue());
    let d = reorg_def();
    let mut events = Vec::new();
    for round in 0..d.rounds {
        events.extend(solve_round(&mut sim, p, &d).unwrap_or_else(|| panic!("round {round}")));
        for _ in 0..(d.settle_ms + d.shuffle_ms) / 1000 * SECOND + SECOND {
            events.extend(hold(&mut sim, p, 0.0, 0.0, 1));
            if !ended(&events).is_empty() {
                break;
            }
        }
    }
    let r = ended(&events);
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!((r[0].collected, r[0].catches), (d.rounds, 0));
    assert_eq!(r[0].medal, Some(Medal::Gold), "{:?}", r[0]);
    assert!(has_achievement(&sim, p, "dotted_line"));
    let sp = sim.unit(p).map_or(0, |u| u.sp);
    back_to_campus(&mut sim);
    enter(&mut sim, p, "reorg", &reorg_venue());
    let mut again = Vec::new();
    for _ in 0..d.rounds {
        again.extend(solve_round(&mut sim, p, &d).unwrap_or_default());
        for _ in 0..(d.settle_ms + d.shuffle_ms) / 1000 * SECOND + SECOND {
            again.extend(hold(&mut sim, p, 0.0, 0.0, 1));
            if !ended(&again).is_empty() {
                break;
            }
        }
    }
    assert_eq!(ended(&again)[0].medal, Some(Medal::Gold));
    // The title's Kudos came once.
    assert_eq!(sim.unit(p).map_or(0, |u| u.sp), sp);
}

#[test]
fn an_idle_reorg_ends_without_a_medal() {
    let (mut sim, ids) = on_campus(1);
    enter(&mut sim, ids[0], "reorg", &reorg_venue());
    let d = reorg_def();
    let total = u32::from(d.rounds) * (d.round_ms + d.shuffle_ms) / 1000 + 2;
    let r = ended(&run(&mut sim, total * SECOND));
    assert_eq!(r.len(), 1);
    assert!(
        !r[0].quit && r[0].medal.is_none() && r[0].score == 0,
        "{:?}",
        r[0]
    );
}

#[test]
fn a_reorg_group_shares_the_org_chart_and_adds_up() {
    let (mut sim, ids) = on_campus(2);
    let (a, b) = (ids[0], ids[1]);
    enter(&mut sim, a, "reorg", &reorg_venue());
    let d = reorg_def();
    assert_eq!(view(&sim, a).badges, view(&sim, b).badges);
    solve_round(&mut sim, a, &d).unwrap_or_else(|| panic!("a"));
    // A found theirs; the round waits for B.
    run(&mut sim, (d.settle_ms / 1000 + 2) * SECOND);
    assert!(view(&sim, a).lines[0].starts_with("Reorg 1/5"));
    solve_round(&mut sim, b, &d).unwrap_or_else(|| panic!("b"));
    let mut events = Vec::new();
    while ended(&events).len() < 2 {
        events.extend(run(&mut sim, SECOND));
        assert!(events.len() < 1_000_000);
        if sim.minigame_view(a).is_none() && sim.minigame_view(b).is_none() {
            break;
        }
    }
    let r = ended(&events);
    assert_eq!(r.len(), 2);
    assert_eq!(r[1].group_score, r[0].score + r[1].score);
    assert!(r.iter().all(|x| x.collected == 1));
}

// ---- Tool Migration ----

fn migration_def() -> MigrationDef {
    content()
        .minigame("tool_migration")
        .and_then(|m| m.migration.clone())
        .unwrap_or_else(|| panic!("no migration rules"))
}

const OLD: (f32, f32) = (-5.0, 0.0);
const NEW: (f32, f32) = (5.0, 0.0);

fn migration_venue() -> Anchors {
    Box::new(|n: &str| match n {
        "player_spawn" | "graveyard" => at(0.0, 3.0),
        "term_old" => at(OLD.0, OLD.1),
        "term_new" => at(NEW.0, NEW.1),
        "npc_champion" => at(0.0, -4.0),
        _ => None,
    })
}

/// The right option for the field on the card, from the ticket's text.
fn right_field(d: &MigrationDef, text: &str) -> Option<u8> {
    let id: u32 = text
        .strip_prefix('#')?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    let t = d.tickets.iter().find(|t| t.id == id)?;
    let (_, rest) = text.split_once("Mandatory field ")?;
    let n: usize = rest.split('/').next()?.parse().ok()?;
    t.fields.get(n - 1).map(|f| f.1)
}

/// Migrates tickets until the twist (or `limit` tickets), picking right unless `wrong_at`
/// says which ticket gets one wrong pick first.
fn migrate(sim: &mut Sim, p: EntityId, limit: usize, wrong_at: Option<usize>) -> Vec<Event> {
    let d = migration_def();
    let mut out = Vec::new();
    for t in 0..limit {
        out.extend(hold(sim, p, OLD.0, OLD.1, 2));
        if view(sim, p).card.is_some_and(|c| c.who == d.twist_from) {
            break;
        }
        out.extend(pick(sim, p, 0));
        out.extend(hold(sim, p, NEW.0, NEW.1, 2));
        let mut wrong = wrong_at == Some(t);
        for _ in 0..200 {
            let Some(card) = view(sim, p).card else { break };
            if card.who == d.twist_from {
                break;
            }
            if card.options.is_empty() {
                // Validating, or nothing left to paste.
                if card.text.contains("Validating") {
                    out.extend(run(sim, SECOND));
                    continue;
                }
                break;
            }
            let right = right_field(&d, &card.text).unwrap_or(0);
            let o = if wrong {
                wrong = false;
                (right + 1) % card.options.len() as u8
            } else {
                right
            };
            out.extend(pick(sim, p, o));
        }
    }
    out
}

#[test]
fn a_ticket_is_exported_at_the_old_tool_and_filled_in_at_the_new_one() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "tool_migration", &migration_venue());
    assert!(view(&sim, p).card.is_none(), "no terminal in reach");
    let d = migration_def();
    hold(&mut sim, p, OLD.0, OLD.1, 2);
    let card = view(&sim, p).card.unwrap_or_default();
    assert!(card.text.contains(&d.tickets[0].text), "{}", card.text);
    assert_eq!(card.options, vec![d.export.clone()]);
    let events = migrate(&mut sim, p, 1, None);
    assert!(
        notes(&events, p)
            .iter()
            .any(|n| n.1 && n.0.contains("migrated"))
    );
    let v = view(&sim, p);
    assert_eq!((v.collected, v.catches), (1, 0));
    assert!(v.lines[0].contains("1/20"));
}

#[test]
fn a_wrong_field_is_a_validation_error_with_a_spinner() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "tool_migration", &migration_venue());
    hold(&mut sim, p, OLD.0, OLD.1, 2);
    pick(&mut sim, p, 0);
    hold(&mut sim, p, NEW.0, NEW.1, 2);
    let d = migration_def();
    let card = view(&sim, p).card.unwrap_or_default();
    let right = right_field(&d, &card.text).unwrap_or_else(|| panic!("{}", card.text));
    let events = pick(&mut sim, p, (right + 1) % 4);
    assert!(notes(&events, p).iter().any(|n| !n.1));
    let v = view(&sim, p);
    assert_eq!(v.catches, 1);
    assert!(
        v.card
            .is_some_and(|c| c.options.is_empty() && c.text.contains("Validating"))
    );
    // The spinner ignores picks.
    sim.apply_as(
        p,
        Command::MinigamePick {
            seq: 0,
            option: right,
        },
    );
    run(&mut sim, d.error_ms / 1000 * SECOND + 2);
    assert_eq!(view(&sim, p).collected, 0);
    assert_eq!(view(&sim, p).card.map(|c| c.options.len()), Some(4));
}

#[test]
fn twenty_tickets_later_the_company_migrates_back_and_a_clean_run_takes_gold_once() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "tool_migration", &migration_venue());
    let d = migration_def();
    migrate(&mut sim, p, 30, None);
    let card = view(&sim, p).card.unwrap_or_default();
    assert_eq!(card.who, d.twist_from);
    assert!(card.text.contains("migrate back"));
    let r = ended(&pick(&mut sim, p, 0));
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].collected, r[0].catches), (20, 0));
    assert_eq!(r[0].medal, Some(Medal::Gold), "{:?}", r[0]);
    assert_eq!(r[0].items, vec!["Migration Champion Lanyard".to_string()]);
    assert!(has_achievement(&sim, p, "migrated_twice"));
    assert!(r[0].summary.contains("migrating back"));

    back_to_campus(&mut sim);
    enter(&mut sim, p, "tool_migration", &migration_venue());
    migrate(&mut sim, p, 30, None);
    let again = ended(&pick(&mut sim, p, 0));
    assert!(again[0].items.is_empty());
    assert_eq!(bag_count(&sim, p, "Migration Champion Lanyard"), 1);
}

#[test]
fn one_validation_error_costs_gold_and_points() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "tool_migration", &migration_venue());
    migrate(&mut sim, p, 30, Some(3));
    let r = ended(&pick(&mut sim, p, 0));
    assert_eq!(r[0].catches, 1);
    assert_eq!(r[0].medal, Some(Medal::Silver), "{:?}", r[0]);
    let d = migration_def();
    assert_eq!(
        r[0].score,
        d.score.base - d.score.per_mistake - d.score.per_second * (r[0].time_ms / 1000)
    );
}

#[test]
fn a_migration_group_works_its_own_tickets_and_adds_up() {
    let (mut sim, ids) = on_campus(2);
    let (a, b) = (ids[0], ids[1]);
    enter(&mut sim, a, "tool_migration", &migration_venue());
    migrate(&mut sim, a, 30, None);
    let first = ended(&pick(&mut sim, a, 0));
    assert_eq!(view(&sim, b).collected, 0);
    migrate(&mut sim, b, 30, Some(0));
    let second = ended(&pick(&mut sim, b, 0));
    assert_eq!(second[0].group_score, first[0].score + second[0].score);
}

// ---- Hackathon ----

fn hackathon_venue() -> Anchors {
    Box::new(|n: &str| match n {
        "player_spawn" | "graveyard" => at(0.0, 0.0),
        "stage_1" => at(0.0, -20.0),
        "stage_2" => at(2.0, -20.0),
        "npc_mc" => at(0.0, -22.0),
        "npc_hacker_1" => at(-8.0, 8.0),
        "npc_hacker_2" => at(8.0, 8.0),
        n => {
            if let Some(k) = n.strip_prefix("pizza_") {
                let k: f32 = k.parse().ok()?;
                return at(-12.0 + 4.0 * k, 6.0);
            }
            let k: f32 = n.strip_prefix("drink_")?.parse().ok()?;
            at(-10.0 + 4.0 * k, -6.0)
        }
    })
}

fn snack_spot(k: usize) -> (f32, f32) {
    if k < 6 {
        (-12.0 + 4.0 * (k as f32 + 1.0), 6.0)
    } else {
        (-10.0 + 4.0 * (k as f32 - 5.0), -6.0)
    }
}

fn focus_of(v: &MinigameView) -> u8 {
    v.gauges[0].value
}

#[test]
fn focus_drains_and_food_refills_it_unless_you_are_full() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "hackathon", &hackathon_venue());
    let start = focus_of(&view(&sim, p));
    hold(&mut sim, p, 0.0, 0.0, 10 * SECOND);
    let low = focus_of(&view(&sim, p));
    assert!(low + 8 <= start && low + 12 >= start, "{start} -> {low}");
    let (x, z) = snack_spot(6);
    let events = hold(&mut sim, p, x, z, 2);
    assert!(
        notes(&events, p)
            .iter()
            .any(|n| n.0.starts_with("Energy drink"))
    );
    let v = view(&sim, p);
    assert!(focus_of(&v) >= 85);
    assert_ne!(v.taken & (1 << 6), 0);
    // Full: the next drink stays where it is.
    let (x, z) = snack_spot(7);
    hold(&mut sim, p, x, z, 2);
    assert_eq!(view(&sim, p).taken & (1 << 7), 0);
}

#[test]
fn at_hour_24_the_demo_crashes_on_stage_and_everyone_gets_the_trophy_once() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "hackathon", &hackathon_venue());
    let events = run(&mut sim, 241 * SECOND);
    assert!(events.iter().any(|e| matches!(e,
        Event::Teleported { unit, pos } if *unit == p && pos.z == -20.0)));
    let card = view(&sim, p).card.unwrap_or_default();
    assert!(card.text.contains("beach ball"), "{}", card.text);
    let r = ended(&pick(&mut sim, p, 0));
    assert_eq!(r.len(), 1);
    // Idle all night: Bronze, and still the plastic trophy.
    assert_eq!(r[0].medal, Some(Medal::Bronze), "{:?}", r[0]);
    let trophies = |s: &Sim| {
        s.unit(p).map_or(0, |u| {
            u.inventory
                .iter()
                .filter(|(n, _)| n == "Hackathon Trophy (Plastic)")
                .map(|(_, c)| *c)
                .sum::<u32>()
        })
    };
    assert_eq!(trophies(&sim), 1);
    assert!(has_achievement(&sim, p, "hackathon_demo"));
    back_to_campus(&mut sim);
    enter(&mut sim, p, "hackathon", &hackathon_venue());
    run(&mut sim, 241 * SECOND);
    pick(&mut sim, p, 0);
    assert_eq!(trophies(&sim), 1);
}

/// Eats whenever focus is below the threshold, from the nearest free spot.
fn feed(sim: &mut Sim, p: EntityId) -> Vec<Event> {
    let mut out = Vec::new();
    let mut spot: Option<usize> = None;
    for _ in 0..260 * SECOND {
        let Some(v) = sim.minigame_view(p).cloned() else {
            out.extend(run(sim, 1));
            continue;
        };
        if v.card.is_some() {
            out.extend(pick(sim, p, 0));
            break;
        }
        if spot.is_some_and(|k| v.taken & (1 << k) != 0) {
            spot = None;
        }
        if spot.is_none() && focus_of(&v) < 80 {
            spot = (0..10)
                .filter(|k| v.taken & (1 << k) == 0)
                .min_by_key(|k| (snack_spot(*k).0.abs() + snack_spot(*k).1.abs()) as u32);
        }
        let (x, z) = spot.map_or((0.0, 0.0), snack_spot);
        out.extend(hold(sim, p, x, z, 1));
    }
    out
}

#[test]
fn a_well_fed_hacker_builds_the_demo_far_enough_for_gold() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "hackathon", &hackathon_venue());
    let events = feed(&mut sim, p);
    assert!(
        notes(&events, p)
            .iter()
            .any(|n| n.0.starts_with("Cold pizza"))
    );
    let r = ended(&events);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].medal, Some(Medal::Gold), "{:?}", r[0]);
    assert_eq!(
        r[0].items,
        vec!["Hackathon Hoodie (Wrong Size)".to_string()]
    );
    assert!(r[0].summary.contains("crashed"));
}

#[test]
fn a_hackathon_team_eats_on_its_own_and_scores_together() {
    let (mut sim, ids) = on_campus(2);
    let (a, b) = (ids[0], ids[1]);
    enter(&mut sim, a, "hackathon", &hackathon_venue());
    // A eats the first pizza; it is still there for B.
    let (x, z) = snack_spot(0);
    hold(&mut sim, a, 0.0, 0.0, 3 * SECOND);
    hold(&mut sim, a, x, z, 2);
    assert_ne!(view(&sim, a).taken & 1, 0);
    assert_eq!(view(&sim, b).taken & 1, 0);
    let events = run(&mut sim, 245 * SECOND);
    let mut events = events;
    events.extend(pick(&mut sim, a, 0));
    events.extend(pick(&mut sim, b, 0));
    let r = ended(&events);
    assert_eq!(r.len(), 2);
    assert_eq!(r[1].group_score, r[0].score + r[1].score);
}

// ---- Open-Space Focus ----

fn focus_def() -> FocusDef {
    content()
        .minigame("open_space_focus")
        .and_then(|m| m.focus.clone())
        .unwrap_or_else(|| panic!("no focus rules"))
}

fn focus_venue() -> Anchors {
    Box::new(|n: &str| match n {
        "player_spawn" | "graveyard" | "pod" => at(0.0, 0.0),
        "route:colleague:0" => at(-12.0, 0.0),
        "route:colleague:1" => at(-1.6, 0.0),
        "route:caller:0" => at(12.0, 0.0),
        "route:caller:1" => at(1.6, 0.0),
        "route:cake:0" => at(0.0, 12.0),
        "route:cake:1" => at(0.0, 1.6),
        "route:warden:0" => at(0.0, -12.0),
        "route:warden:1" => at(0.0, -1.6),
        _ => None,
    })
}

/// Raises the right defence once an arrival on `p`'s card is at most `lead_s` seconds away.
fn guard(sim: &mut Sim, p: EntityId, lead_s: u32) {
    let d = focus_def();
    let Some(c) = sim.minigame_view(p).and_then(|v| v.card.clone()) else {
        return;
    };
    for l in c.text.lines() {
        let Some((name, secs)) = l
            .strip_prefix("Incoming: ")
            .and_then(|rest| rest.rsplit_once(" in "))
        else {
            continue;
        };
        let secs: u32 = secs.trim_end_matches(" s").parse().unwrap_or(99);
        let Some(dd) = d.disturbers.iter().find(|x| x.name == name) else {
            continue;
        };
        let a = dd.counter as u8;
        let ready = c.options[usize::from(a)] == d.actions[usize::from(a)];
        if secs <= lead_s && ready {
            sim.apply_as(
                p,
                Command::MinigamePick {
                    seq: c.seq,
                    option: a,
                },
            );
        }
    }
}

/// At the desk, defending, for at most `ticks` or until the run ends.
fn defend(sim: &mut Sim, p: EntityId, ticks: u32, lead_s: u32) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        if !ended(&out).is_empty() {
            break;
        }
        guard(sim, p, lead_s);
        out.extend(hold(sim, p, 0.0, 0.5, 1));
    }
    out
}

#[test]
fn an_unguarded_arrival_breaks_focus_and_resets_the_streak() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "open_space_focus", &focus_venue());
    let events = hold(&mut sim, p, 0.0, 0.5, 14 * SECOND);
    let n = notes(&events, p);
    assert!(
        n.iter().any(|x| !x.1 && x.0.contains("Focus lost")),
        "{n:?}"
    );
    let v = view(&sim, p);
    assert_eq!(v.catches, 1);
    assert!(v.lines[0].starts_with("Deep work: 1 /") || v.lines[0].starts_with("Deep work: 0 /"));
    assert!(
        events.iter().any(|e| matches!(e, Event::Yell { .. })),
        "the colleague speaks"
    );
}

#[test]
fn the_right_defence_at_the_right_moment_deflects_and_the_wrong_one_does_not() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "open_space_focus", &focus_venue());
    let events = defend(&mut sim, p, 14 * SECOND, 2);
    assert!(
        notes(&events, p)
            .iter()
            .any(|x| x.1 && x.0.starts_with("Deflected"))
    );
    assert_eq!(view(&sim, p).catches, 0);

    // Headphones do nothing against "got a minute?".
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "open_space_focus", &focus_venue());
    hold(&mut sim, p, 0.0, 0.5, 11 * SECOND);
    sim.apply_as(p, Command::MinigamePick { seq: 1, option: 0 });
    hold(&mut sim, p, 0.0, 0.5, 3 * SECOND);
    assert_eq!(view(&sim, p).catches, 1);
}

#[test]
fn a_defence_raised_too_early_wears_off_and_rests() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "open_space_focus", &focus_venue());
    sim.apply_as(p, Command::MinigamePick { seq: 1, option: 1 });
    run(&mut sim, 1);
    let d = focus_def();
    let label = view(&sim, p)
        .card
        .map(|c| c.options[1].clone())
        .unwrap_or_default();
    assert!(label.ends_with(": ON"), "{label}");
    hold(&mut sim, p, 0.0, 0.5, 3 * SECOND);
    let resting = view(&sim, p)
        .card
        .map(|c| c.options[1].clone())
        .unwrap_or_default();
    assert!(
        resting.starts_with(&d.actions[1]) && resting.ends_with(" s)"),
        "{resting}"
    );
    hold(&mut sim, p, 0.0, 0.5, 11 * SECOND);
    assert_eq!(view(&sim, p).catches, 1, "the early status was long gone");
}

#[test]
fn away_from_the_desk_nothing_counts() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "open_space_focus", &focus_venue());
    let events = hold(&mut sim, p, 20.0, 20.0, 16 * SECOND);
    assert!(notes(&events, p).is_empty());
    let v = view(&sim, p);
    assert!(v.lines[0].starts_with("Deep work: 0 /"));
    assert_eq!(v.card.map(|c| c.text), Some(focus_def().away));
}

#[test]
fn sixty_clean_seconds_take_gold_and_the_headphones_once() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "open_space_focus", &focus_venue());
    let events = defend(&mut sim, p, 120 * SECOND, 2);
    let r = ended(&events);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].catches, 0, "{:?}", notes(&events, p));
    assert!(r[0].collected >= 5, "{:?}", r[0]);
    assert_eq!(r[0].medal, Some(Medal::Gold), "{:?}", r[0]);
    assert_eq!(
        r[0].items,
        vec!["Noise-Cancelling Headphones (Decorative)".to_string()]
    );
    assert!(has_achievement(&sim, p, "deep_work"));
    back_to_campus(&mut sim);
    enter(&mut sim, p, "open_space_focus", &focus_venue());
    let again = ended(&defend(&mut sim, p, 120 * SECOND, 2));
    assert!(again[0].items.is_empty());
    assert_eq!(
        bag_count(&sim, p, "Noise-Cancelling Headphones (Decorative)"),
        1
    );
}

#[test]
fn an_open_space_group_defends_on_its_own_and_adds_up() {
    let (mut sim, ids) = on_campus(2);
    let (a, b) = (ids[0], ids[1]);
    enter(&mut sim, a, "open_space_focus", &focus_venue());
    // A defends from the start; B only after the first colleague got to them.
    let mut events = Vec::new();
    for t in 0..290 * SECOND {
        if ended(&events).len() == 2 {
            break;
        }
        guard(&mut sim, a, 2);
        if t > 20 * SECOND {
            guard(&mut sim, b, 2);
        }
        move_to(&mut sim, a, 0.0, 0.5);
        move_to(&mut sim, b, 0.5, 0.0);
        events.extend(sim.tick());
    }
    let r = ended(&events);
    assert_eq!(r.len(), 2, "{:?}", notes(&events, b));
    let (ra, rb) = if r[0].unit == a {
        (&r[0], &r[1])
    } else {
        (&r[1], &r[0])
    };
    assert!(!ra.quit && !rb.quit);
    assert!(ra.score > rb.score);
    assert!(rb.catches > 0 && ra.catches == 0);
    assert_eq!(r[1].group_score, r[0].score + r[1].score);
}

// ---- Stay Green ----

use combat_core::corporate::stay_green::{LAYOUT_CAT, LAYOUT_JIGGLER, LAYOUT_MUTED};

const DESK: (f32, f32) = (0.0, 0.0);
const LAUNDRY: (f32, f32) = (6.0, 0.0);
const DOOR: (f32, f32) = (-10.0, 0.0);

fn home_venue() -> Anchors {
    Box::new(|n: &str| match n {
        "player_spawn" | "graveyard" => at(0.0, 0.5),
        "desk" => at(DESK.0, DESK.1),
        "laundry" => at(LAUNDRY.0, LAUNDRY.1),
        "door" => at(DOOR.0, DOOR.1),
        "cat_bed" => at(3.0, 3.0),
        "cat_desk" => at(0.3, -0.3),
        // Ill Marie's bed, well away from the laptop the chores are about.
        "npc_ill_marie_call" => at(-3.0, 3.0),
        _ => None,
    })
}

fn green_view(sim: &Sim, p: EntityId) -> (u8, String) {
    let v = view(sim, p);
    let g = v.gauges.first().cloned().unwrap_or_default();
    (g.value, g.label)
}

/// A diligent home worker reading only the HUD: desk chores from the laptop, walk chores in
/// person (the bits say which station waits), no jiggler.
fn diligent(sim: &mut Sim, p: EntityId) -> Vec<Event> {
    let Some(v) = sim.minigame_view(p).cloned() else {
        return run(sim, 1);
    };
    let text = v.card.as_ref().map(|c| c.text.clone()).unwrap_or_default();
    let muted = v.layout & LAYOUT_MUTED != 0;
    let mut out = Vec::new();
    let tap = |sim: &mut Sim, option: u8| {
        sim.apply_as(p, Command::MinigamePick { seq: 1, option });
    };
    move_to(sim, p, DESK.0, DESK.1);
    if v.layout & LAYOUT_CAT != 0 {
        tap(sim, 1);
    }
    if (text.contains("drilling") && !muted) || (text.contains("on mute") && muted) {
        tap(sim, 2);
    }
    if text.contains("Quick question") {
        tap(sim, 3);
    }
    let station = [LAUNDRY, DOOR]
        .into_iter()
        .enumerate()
        .find(|(k, _)| v.layout & (1 << k) != 0);
    if let Some((_, (x, z))) = station {
        move_to(sim, p, x, z);
    }
    out.extend(sim.tick());
    out
}

fn play(sim: &mut Sim, p: EntityId, ticks: u32) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        if !ended(&out).is_empty() {
            break;
        }
        out.extend(diligent(sim, p));
    }
    out
}

#[test]
fn stay_green_starts_green_with_the_laptop_card() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    let events = enter(&mut sim, p, "stay_green", &home_venue());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::MinigameStarted { key, .. } if key == "stay_green"))
    );
    hold(&mut sim, p, 0.0, 0.0, SECOND);
    let v = view(&sim, p);
    let card = v.card.clone().unwrap_or_default();
    assert_eq!(card.options.len(), 4);
    assert!(card.options[0].starts_with("Mouse jiggler"), "{card:?}");
    assert_eq!(card.options[2], "Mute");
    assert!(v.lines[0].starts_with("Status: Available"), "{:?}", v.lines);
    assert_eq!(green_view(&sim, p), (100, "Teams: Available".into()));
}

#[test]
fn away_from_the_laptop_the_status_decays_and_the_manager_asks() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "stay_green", &home_venue());
    let events = hold(&mut sim, p, 20.0, 20.0, 6 * SECOND);
    let (value, label) = green_view(&sim, p);
    assert!(value < 50, "{value}");
    assert_eq!(label, "Teams: Away");
    assert!(
        notes(&events, p)
            .iter()
            .any(|n| n.0.contains("status turns yellow")),
        "{:?}",
        notes(&events, p)
    );
    // The question comes a moment later and waits for a reply nobody gives.
    let events = hold(&mut sim, p, 20.0, 20.0, 4 * SECOND);
    assert!(
        notes(&events, p)
            .iter()
            .any(|n| n.0.starts_with("Your manager: \"Quick question"))
    );
    let events = hold(&mut sim, p, 20.0, 20.0, 11 * SECOND);
    assert!(!notes(&events, p).is_empty());
    assert!(view(&sim, p).catches >= 1, "the ignored question counts");
    // Back at the laptop the meter climbs again.
    hold(&mut sim, p, 0.0, 0.0, 3 * SECOND);
    assert_eq!(green_view(&sim, p).1, "Teams: Available");
}

#[test]
fn chores_arrive_on_schedule_and_are_done_or_missed() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "stay_green", &home_venue());
    hold(&mut sim, p, 0.0, 0.0, 13 * SECOND);
    let v = view(&sim, p);
    assert!(
        v.layout & LAYOUT_CAT != 0,
        "the cat sits on the keyboard at 12 s"
    );
    let cat = sim
        .units()
        .find(|u| u.key == "home_cat")
        .map(|u| (u.pos.x, u.pos.z));
    assert_eq!(cat, Some((0.3, -0.3)), "the cat moved onto the desk");
    // Lifting the cat from across the flat does nothing.
    move_to(&mut sim, p, 20.0, 20.0);
    run(&mut sim, 1);
    let events = pick(&mut sim, p, 1);
    assert!(notes(&events, p).iter().any(|n| !n.1));
    let events = hold(&mut sim, p, 0.0, 0.0, 1);
    assert!(events.is_empty() || view(&sim, p).collected == 0);
    let events = pick(&mut sim, p, 1);
    assert!(notes(&events, p).iter().any(|n| n.1));
    assert_eq!(view(&sim, p).collected, 1);
    // At 46 s the laundry (25 s) and the drill (45 s) both wait; muting solves the drill.
    hold(&mut sim, p, 0.0, 0.0, 33 * SECOND);
    let v = view(&sim, p);
    let text = v.card.as_ref().map(|c| c.text.clone()).unwrap_or_default();
    assert!(
        text.contains("drilling") && text.contains("laundry"),
        "{text}"
    );
    assert!(v.layout & 1 != 0, "the laundry station is marked");
    pick(&mut sim, p, 2);
    run(&mut sim, 1);
    let v = view(&sim, p);
    assert!(v.layout & LAYOUT_MUTED != 0);
    assert_eq!(
        v.card.map(|c| c.options[2].clone()).as_deref(),
        Some("Unmute")
    );
    assert_eq!(view(&sim, p).collected, 2);
    // Ignoring the laundry past its 30 s deadline misses it.
    let events = hold(&mut sim, p, 0.0, 0.0, 10 * SECOND);
    assert!(
        notes(&events, p)
            .iter()
            .any(|n| !n.1 && n.0.contains("laundry")),
        "{:?}",
        notes(&events, p)
    );
    assert_eq!(view(&sim, p).catches, 1);
}

#[test]
fn the_jiggler_holds_the_status_until_it_scans() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "stay_green", &home_venue());
    hold(&mut sim, p, 0.0, 0.0, 57 * SECOND);
    let v = view(&sim, p);
    assert!(
        v.card
            .as_ref()
            .is_some_and(|c| c.text.contains("IT security scan in")),
        "the 62 s scan is announced: {:?}",
        v.card
    );
    pick(&mut sim, p, 0);
    assert!(view(&sim, p).layout & LAYOUT_JIGGLER != 0);
    hold(&mut sim, p, 20.0, 20.0, 4 * SECOND);
    assert_eq!(
        green_view(&sim, p).0,
        100,
        "the jiggler holds the meter away"
    );
    let events = hold(&mut sim, p, 20.0, 20.0, 3 * SECOND);
    let n = notes(&events, p);
    assert!(n.iter().any(|x| !x.1 && x.0.starts_with("IT:")), "{n:?}");
    let v = view(&sim, p);
    assert!(v.lines[1].ends_with("IT flags 1"), "{:?}", v.lines);
    assert_eq!(green_view(&sim, p).1, "Teams: Away");
    assert!(v.layout & LAYOUT_JIGGLER == 0);
    assert!(n.iter().any(|x| x.0.contains("yellow")), "{n:?}");
}

#[test]
fn a_clean_green_day_takes_gold_the_title_and_the_hoodie_once() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "stay_green", &home_venue());
    let events = play(&mut sim, p, 200 * SECOND);
    let r = ended(&events);
    assert_eq!(r.len(), 1, "{:?}", notes(&events, p));
    assert_eq!(r[0].catches, 0, "{:?}", notes(&events, p));
    assert!(r[0].collected >= 10, "{:?}", r[0]);
    assert_eq!(r[0].medal, Some(Medal::Gold), "{:?}", r[0]);
    assert!(
        r[0].summary.starts_with("Green for 180 of 180 s"),
        "{}",
        r[0].summary
    );
    assert_eq!(
        r[0].items,
        vec!["Webcam Hoodie (Sweatpants Implied)".to_string()]
    );
    assert!(has_achievement(&sim, p, "status_green"));
    back_to_campus(&mut sim);
    enter(&mut sim, p, "stay_green", &home_venue());
    let again = ended(&play(&mut sim, p, 200 * SECOND));
    assert_eq!(again[0].medal, Some(Medal::Gold));
    assert!(again[0].items.is_empty());
    assert_eq!(bag_count(&sim, p, "Webcam Hoodie (Sweatpants Implied)"), 1);
}

#[test]
fn a_day_of_ignored_chores_still_ends_with_a_lower_medal() {
    let (mut sim, ids) = on_campus(1);
    let p = ids[0];
    enter(&mut sim, p, "stay_green", &home_venue());
    let r = ended(&hold(&mut sim, p, 0.0, 0.0, 200 * SECOND));
    assert_eq!(r.len(), 1);
    assert!(r[0].catches >= 8, "{:?}", r[0]);
    assert!(r[0].medal.is_some_and(|m| m < Medal::Gold), "{:?}", r[0]);
}

#[test]
fn a_home_office_group_adds_up() {
    let (mut sim, ids) = on_campus(2);
    let (a, b) = (ids[0], ids[1]);
    enter(&mut sim, a, "stay_green", &home_venue());
    // A does the chores; B sits at the laptop and ignores the flat.
    let mut events = Vec::new();
    for _ in 0..200 * SECOND {
        if ended(&events).len() == 2 {
            break;
        }
        move_to(&mut sim, b, 0.5, 0.0);
        events.extend(diligent(&mut sim, a));
    }
    let r = ended(&events);
    assert_eq!(r.len(), 2);
    let (ra, rb) = if r[0].unit == a {
        (&r[0], &r[1])
    } else {
        (&r[1], &r[0])
    };
    assert!(!ra.quit && !rb.quit);
    assert!(ra.score > rb.score, "{} vs {}", ra.score, rb.score);
    assert!(rb.catches > ra.catches);
    assert_eq!(r[1].group_score, r[0].score + r[1].score);
}
