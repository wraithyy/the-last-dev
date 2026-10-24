//! Ilmari Wing 6, the Boardroom in the Sky (docs/ILMARI_1_0.md D.4, D9): the Chief Ilmari
//! Officer's desks, the landing that needs two kickers, the inbox, the invoices and apples, the
//! floor tiles, the severance meter and the Heir, plus the wing's drops and locks. The balance
//! gates (Story solo, Full trio) live in tests/playthrough.rs.
#![allow(clippy::expect_used)]

mod common;

use combat_core::{
    BurnoutCause, Command, EntityId, Event, GearSource, PROP_OK, Sim, SpellId, Vec3,
};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;
const BOSS_Z: f32 = -30.0;
const BOSS: &str = "il_cio";
/// Three classes whose interrupt reaches 25 m and is no stun (a boss refuses those): Reject PR,
/// Severity: Blocker and Early Stopping.
const KICKERS: [&str; 3] = ["fullstack", "qa", "data_scientist"];
const KICKS: [SpellId; 3] = [SpellId(4), SpellId(53), SpellId(63)];
const NON_COMPETE: SpellId = SpellId(2684);
const HEIR_LANDING: SpellId = SpellId(2685);

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, BOSS_Z),
        n if n.starts_with("prop:") => at(1.0, 0.0),
        _ => at(0.0, 0.0),
    }
}

fn setup(story: bool, classes: &[&'static str], seed: u64) -> (Sim, Vec<EntityId>, EntityId) {
    let mut c = content();
    let units: Vec<String> = c.classes.iter().map(|k| k.unit.clone()).collect();
    for k in &units {
        def_mut(&mut c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
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
    sim.debug_force_raid_async(Some(story));
    if let Err(e) = sim.load_zone("ilmari6/3", &anchors, Vec::new()) {
        panic!("ilmari6/3: {e}");
    }
    let boss = sim
        .units()
        .find(|u| u.key == BOSS)
        .map(|u| u.id)
        .expect("boss");
    for &p in &ids {
        move_to(&mut sim, p, Vec3::new(2.0, 0.0, BOSS_Z + 8.0));
    }
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
    (sim, ids, boss)
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

/// Sets the boss to `pct` percent of its health; the next tick moves it into that phase.
fn boss_at(sim: &mut Sim, boss: EntityId, pct: u32) {
    let max = sim.unit(boss).expect("boss").max_hp;
    assert!(sim.debug_set_hp(boss, max * pct / 100));
}

fn kick(sim: &mut Sim, who: EntityId, spell: SpellId, boss: EntityId) {
    // Casts need a facing target, and the boss walks.
    let (from, to) = (
        sim.unit(who).expect("player").pos,
        sim.unit(boss).expect("boss").pos,
    );
    sim.apply_as(
        who,
        Command::PlayerMoved {
            pos: from,
            yaw: (from.x - to.x).atan2(from.z - to.z),
            moving: false,
            jumping: false,
        },
    );
    sim.apply_as(
        who,
        Command::SetTarget {
            unit: who,
            target: Some(boss),
        },
    );
    sim.apply_as(who, Command::Cast { caster: who, spell });
}

/// Ticks until `done` matches an event, at most `secs`; every event seen is returned.
fn until(sim: &mut Sim, secs: u32, done: impl Fn(&Event) -> bool) -> (bool, Vec<Event>) {
    let mut seen = Vec::new();
    for _ in 0..secs * SECOND {
        let events = sim.tick();
        let hit = events.iter().any(&done);
        seen.extend(events);
        if hit {
            return (true, seen);
        }
    }
    (false, seen)
}

fn landing_cast(spell: SpellId, boss: EntityId) -> impl Fn(&Event) -> bool {
    move |e| matches!(e, Event::CastStarted { caster, spell: s, .. } if *caster == boss && *s == spell)
}

fn has_aura(sim: &Sim, who: EntityId, key: &str) -> bool {
    sim.unit(who)
        .is_some_and(|u| u.auras.iter().any(|a| a.key == key))
}

/// Covers of each Earnings Call telegraph in the order they were painted.
fn earnings_covers(events: &[Event]) -> Vec<usize> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::TelegraphStarted { spell, cover, .. } if (2688..=2693).contains(&spell.0) => {
                Some(cover.len())
            }
            _ => None,
        })
        .collect()
}

fn desk_counts(events: &[Event]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for e in events {
        if let Event::Boardroom { desks, .. } = e
            && out.last() != Some(desks)
        {
            out.push(*desks);
        }
    }
    out
}

// ---------------------------------------------------------------- the desks

#[test]
fn every_earnings_call_takes_a_desk_away_for_good() {
    let (mut sim, _ids, boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    let mut events = run(&mut sim, 1);
    boss_at(&mut sim, boss, 50);
    events.extend(run(&mut sim, 6 * 24 + 8));
    assert_eq!(
        earnings_covers(&events),
        [5, 4, 3, 2, 1, 0],
        "the cover shrinks with every call"
    );
    assert_eq!(
        desk_counts(&events),
        [5, 4, 3, 2, 1, 0],
        "and the clients are told"
    );
}

#[test]
fn the_succession_rider_takes_two_desks_per_call() {
    let (mut sim, _ids, boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    assert!(sim.debug_apply_aura(boss, "il_rider_succession", 3_600_000));
    run(&mut sim, 1);
    boss_at(&mut sim, boss, 50);
    let events = run(&mut sim, 5 * 24 + 8);
    assert_eq!(earnings_covers(&events)[..4], [5, 3, 1, 0]);
}

// ---------------------------------------------------------------- the landing

/// Puts the boss in its third act and runs to the landing cast; returns with it on the bar.
fn to_the_landing(sim: &mut Sim, boss: EntityId) {
    boss_at(sim, boss, 20);
    let (seen, _) = until(sim, 80, landing_cast(NON_COMPETE, boss));
    assert!(seen, "the boss lands within 80 s of its third act");
    assert!(sim.unit(boss).is_some_and(|u| u.cast.is_some()));
}

#[test]
fn two_different_kickers_inside_the_window_stop_the_landing() {
    let (mut sim, ids, boss) = setup(false, &KICKERS, 9);
    to_the_landing(&mut sim, boss);
    kick(&mut sim, ids[0], KICKS[0], boss);
    let first = run(&mut sim, 1);
    assert!(
        first.iter().any(|e| matches!(
            e,
            Event::PairKick {
                have: 1,
                need: 2,
                ..
            }
        )),
        "the first kick is counted, not applied"
    );
    assert!(
        sim.unit(boss).is_some_and(|u| u.cast.is_some()),
        "one kick is not enough"
    );
    kick(&mut sim, ids[1], KICKS[1], boss);
    let second = run(&mut sim, 1);
    assert!(
        second.iter().any(
            |e| matches!(e, Event::CastInterrupted { caster, spell, .. } if *caster == boss && *spell == NON_COMPETE)
        ),
        "the second kicker completes the pair"
    );
    let rest = run(&mut sim, 5);
    assert!(
        !rest
            .iter()
            .any(|e| matches!(e, Event::AuraApplied { key, .. } if key == "il_noncompete")),
        "nothing lands"
    );
}

#[test]
fn a_second_kick_after_the_window_is_too_late() {
    let (mut sim, ids, boss) = setup(false, &KICKERS, 9);
    to_the_landing(&mut sim, boss);
    kick(&mut sim, ids[0], KICKS[0], boss);
    let mut events = run(&mut sim, 2);
    assert!(
        sim.unit(boss).is_some_and(|u| u.cast.is_some()),
        "2 s on, still casting"
    );
    kick(&mut sim, ids[1], KICKS[1], boss);
    events.extend(run(&mut sim, 5));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::CastInterrupted { spell, .. } if *spell == NON_COMPETE)),
        "1.5 s are over: the second kick starts a new pair"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::AuraApplied { key, .. } if key == "il_noncompete")),
        "the landing lands"
    );
}

#[test]
fn the_boss_is_untouchable_while_he_hangs_in_the_parachute() {
    let (mut sim, ids, boss) = setup(false, &KICKERS, 9);
    boss_at(&mut sim, boss, 20);
    let mut up = false;
    for _ in 0..40 * SECOND {
        sim.tick();
        if has_aura(&sim, boss, "il_airborne") {
            up = true;
            break;
        }
    }
    assert!(up, "he lifts off in the third act");
    let before = sim.unit(boss).expect("boss").hp;
    sim.debug_hit(ids[0], boss, 5_000);
    run(&mut sim, 1);
    assert_eq!(
        sim.unit(boss).expect("boss").hp,
        before,
        "nothing touches him up there"
    );
}

#[test]
fn story_needs_one_kick_for_the_landing() {
    let (mut sim, ids, boss) = setup(true, &KICKERS[..1], 9);
    to_the_landing(&mut sim, boss);
    kick(&mut sim, ids[0], KICKS[0], boss);
    let events = run(&mut sim, 1);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::CastInterrupted { spell, .. } if *spell == NON_COMPETE))
    );
    assert!(!events.iter().any(|e| matches!(e, Event::PairKick { .. })));
}

#[test]
fn the_heirs_landing_falls_to_one_kicker() {
    let (mut sim, ids, boss) = setup(false, &KICKERS, 9);
    assert!(sim.debug_apply_aura(boss, "il_rider_succession", 3_600_000));
    run(&mut sim, 1);
    boss_at(&mut sim, boss, 20);
    let heir = {
        run(&mut sim, 2);
        sim.units()
            .find(|u| u.key == "il_cio_heir" && u.alive)
            .map(|u| u.id)
            .expect("the Heir joins in the second act")
    };
    let (seen, _) = until(&mut sim, 80, landing_cast(HEIR_LANDING, heir));
    assert!(seen, "the Heir lands with the CIO");
    kick(&mut sim, ids[2], KICKS[2], heir);
    let events = run(&mut sim, 1);
    assert!(
        events.iter().any(
            |e| matches!(e, Event::CastInterrupted { caster, spell, .. } if *caster == heir && *spell == HEIR_LANDING)
        ),
        "one kicker from the sidelines is enough for the Heir"
    );
}

// ---------------------------------------------------------------- the inbox

fn inbox_events(events: &[Event]) -> Vec<u16> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Boardroom { inbox_ms, .. } => Some(*inbox_ms),
            _ => None,
        })
        .collect()
}

#[test]
fn a_terminal_clears_the_message_for_the_whole_party() {
    let (mut sim, ids, _boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    let (ping, _) = until(
        &mut sim,
        40,
        |e| matches!(e, Event::Boardroom { inbox_ms, .. } if *inbox_ms > 0),
    );
    assert!(ping, "a message pings within 40 s");
    move_to(&mut sim, ids[1], Vec3::new(1.0, 0.0, 0.0));
    sim.apply_as(
        ids[1],
        Command::Interact {
            prop: "il_inbox_2".into(),
        },
    );
    let events = run(&mut sim, 1);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Interacted { unit, outcome, .. } if *unit == ids[1] && *outcome == PROP_OK
    )));
    assert_eq!(
        inbox_events(&events).last(),
        Some(&0),
        "the pending message is gone"
    );
    run(&mut sim, 8);
    for &p in &ids {
        assert!(!has_aura(&sim, p, "il_notified"), "nobody is notified");
    }
}

#[test]
fn an_unread_message_notifies_everybody() {
    let (mut sim, ids, _boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    let (ping, _) = until(
        &mut sim,
        40,
        |e| matches!(e, Event::Boardroom { inbox_ms, .. } if *inbox_ms > 0),
    );
    assert!(ping);
    run(&mut sim, 7);
    for &p in &ids {
        assert!(has_aura(&sim, p, "il_notified"), "the 6 s window is over");
    }
}

#[test]
fn a_terminal_with_nothing_pending_does_nothing() {
    let (mut sim, ids, _boss) = setup(false, &["backend"], 5);
    run(&mut sim, 2);
    move_to(&mut sim, ids[0], Vec3::new(1.0, 0.0, 0.0));
    sim.apply_as(
        ids[0],
        Command::Interact {
            prop: "il_inbox_1".into(),
        },
    );
    let events = run(&mut sim, 1);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Interacted { outcome, .. } if *outcome == 0
    )));
}

// ---------------------------------------------------------------- invoices, apples, tiles

#[test]
fn unpaid_consultants_leave_and_bill_the_group() {
    let (mut sim, ids, boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    run(&mut sim, 1);
    boss_at(&mut sim, boss, 50);
    run(&mut sim, 4);
    let consultants = |sim: &Sim| {
        sim.units()
            .filter(|u| u.key == "il_consultant" && u.alive)
            .count()
    };
    assert_eq!(consultants(&sim), 2, "two invoices on Full");
    let events = run(&mut sim, 23);
    assert_eq!(consultants(&sim), 0, "the invoices ran out");
    let billed = events
        .iter()
        .filter(|e| matches!(e, Event::Burnout { unit, delta: 120, cause: BurnoutCause::Collateral, .. } if ids.contains(unit)))
        .count();
    assert!(billed >= ids.len(), "every player is billed 12%: {billed}");
}

#[test]
fn story_hires_one_consultant() {
    let (mut sim, _ids, boss) = setup(true, &["backend"], 5);
    run(&mut sim, 1);
    boss_at(&mut sim, boss, 50);
    run(&mut sim, 4);
    assert_eq!(
        sim.units()
            .filter(|u| u.key == "il_consultant" && u.alive)
            .count(),
        1
    );
}

#[test]
fn killing_an_apple_heals_its_killer_two_percent() {
    let (mut sim, ids, boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    run(&mut sim, 1);
    boss_at(&mut sim, boss, 50);
    run(&mut sim, 17);
    let apple = sim
        .units()
        .find(|u| u.key == "il_apple" && u.alive)
        .map(|u| u.id)
        .expect("apples come with the second act");
    let max = sim.unit(ids[0]).expect("player").max_hp;
    assert!(sim.debug_set_hp(ids[0], max / 2));
    sim.debug_hit(ids[0], apple, 1_000_000);
    let events = run(&mut sim, 1);
    let healed: u32 = events
        .iter()
        .filter_map(|e| match e {
            Event::Heal { target, amount, .. } if *target == ids[0] => Some(*amount),
            _ => None,
        })
        .sum();
    assert_eq!(healed, max * 2 / 100);
}

#[test]
fn an_apple_left_alone_makes_the_boss_faster() {
    let (mut sim, _ids, boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    run(&mut sim, 1);
    boss_at(&mut sim, boss, 50);
    run(&mut sim, 32);
    assert!(
        has_aura(&sim, boss, "il_ripe"),
        "three ripe apples, +3% haste each"
    );
}

#[test]
fn right_sizing_clears_tiles_for_good_and_the_freeze_gives_two_back() {
    let (mut sim, _ids, boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    run(&mut sim, 1);
    boss_at(&mut sim, boss, 20);
    let mut tiles = 0u16;
    for e in run(&mut sim, 45) {
        if let Event::Boardroom { tiles: t, .. } = e {
            tiles = t;
        }
    }
    assert!(tiles.count_ones() >= 3, "tiles go: {tiles:#b}");
    assert_eq!(tiles & (1 << 4), 0, "the centre tile is never cleared");
    boss_at(&mut sim, boss, 9);
    let mut after = tiles;
    for e in run(&mut sim, 1) {
        if let Event::Boardroom { tiles: t, .. } = e {
            after = t;
        }
    }
    assert_eq!(
        after.count_ones() + 2,
        tiles.count_ones(),
        "a Hiring Freeze restores two"
    );
}

// ---------------------------------------------------------------- offboarding

#[test]
fn offboarding_sits_him_down_and_a_full_meter_heals_him() {
    let (mut sim, _ids, boss) = setup(false, &["backend", "devops", "fullstack"], 5);
    run(&mut sim, 1);
    let max = sim.unit(boss).expect("boss").max_hp;
    assert!(sim.debug_set_hp(boss, max * 4 / 100));
    run(&mut sim, 1);
    assert!(has_aura(&sim, boss, "il_offboarding"), "he sits");
    let events = run(&mut sim, 10);
    let hp = sim.unit(boss).expect("boss").hp;
    assert!(
        hp >= max * 15 / 100,
        "the severance package healed him: {hp}/{max}"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Boardroom {
                severance_ms: 0,
                ..
            }
        )),
        "the meter ran out"
    );
}

#[test]
fn story_skips_offboarding() {
    let (mut sim, _ids, boss) = setup(true, &["backend"], 5);
    run(&mut sim, 1);
    let max = sim.unit(boss).expect("boss").max_hp;
    assert!(sim.debug_set_hp(boss, max * 4 / 100));
    run(&mut sim, 2);
    assert!(!has_aura(&sim, boss, "il_offboarding"));
}

// ---------------------------------------------------------------- the rider, replays, content

#[test]
fn the_heir_joins_only_under_the_rider_and_never_on_story() {
    for (story, rider, want) in [(false, true, 1), (false, false, 0), (true, true, 0)] {
        let classes: &[&'static str] = if story {
            &["backend"]
        } else {
            &["backend", "devops", "fullstack"]
        };
        let (mut sim, _ids, boss) = setup(story, classes, 5);
        if rider {
            assert!(sim.debug_apply_aura(boss, "il_rider_succession", 3_600_000));
        }
        run(&mut sim, 1);
        boss_at(&mut sim, boss, 50);
        run(&mut sim, 2);
        let heirs = sim
            .units()
            .filter(|u| u.key == "il_cio_heir" && u.alive)
            .count();
        assert_eq!(heirs, want, "story {story}, rider {rider}");
    }
}

fn fight_log(story: bool, seed: u64) -> String {
    let classes: &[&'static str] = if story {
        &["fullstack"]
    } else {
        &["backend", "fullstack", "frontend"]
    };
    let (mut sim, _ids, boss) = setup(story, classes, seed);
    let mut log = String::new();
    for k in 0..90 * SECOND {
        if k == 15 * SECOND {
            boss_at(&mut sim, boss, 55);
        }
        if k == 45 * SECOND {
            boss_at(&mut sim, boss, 20);
        }
        for e in sim.tick() {
            log.push_str(&log_line(&sim, u64::from(k), &e));
            log.push('\n');
        }
    }
    log
}

#[test]
fn the_fight_replays_bit_identically() {
    for story in [false, true] {
        let a = fight_log(story, 3);
        let b = fight_log(story, 3);
        assert!(a.contains("boardroom"));
        assert_eq!(a, b, "story {story}");
    }
}

#[test]
fn the_wing_door_drops_and_riders_follow_the_content() {
    let c = content();
    let w6 = c.raid_of("ilmari6/1").expect("wing");
    assert!(w6.ilmari && w6.zones.len() == 3);
    let own: Vec<_> = c
        .economy
        .gear
        .iter()
        .filter(|g| g.drops_from.as_deref() == Some(BOSS))
        .collect();
    assert_eq!(own.len(), 8, "4 loose pieces and the 4 set pieces");
    assert!(own.iter().all(|g| g.source == GearSource::Ilmari));
    assert_eq!(own.iter().filter(|g| g.ilvl == 18).count(), 4);
    assert_eq!(own.iter().filter(|g| g.ilvl == 20).count(), 4);
    assert!(
        c.ilmari
            .judgment
            .riders
            .iter()
            .any(|r| r.boss == BOSS && r.aura == "il_rider_succession")
    );
    let terminals = c
        .world
        .props
        .iter()
        .filter(|p| p.zone == "ilmari6/3" && p.key.starts_with("il_inbox_"))
        .count();
    assert_eq!(terminals, 4);
    // Every pack of the wing is made of units that exist and fight.
    for z in ["ilmari6/1", "ilmari6/2", "ilmari6/3"] {
        let zone = c.zone(z).expect("zone");
        assert_eq!(zone.packs.len(), 3, "{z}");
        for u in zone.packs.iter().flat_map(|p| &p.units) {
            assert!(
                c.unit_def(u).is_some_and(|d| d.hostile && d.ai.is_some()),
                "{u}"
            );
        }
    }
}

#[test]
fn killing_the_cio_locks_its_own_difficulty() {
    for story in [true, false] {
        let classes: &[&'static str] = if story {
            &["fullstack"]
        } else {
            &["backend", "devops", "fullstack"]
        };
        let (mut sim, _ids, id) = setup(story, classes, 13);
        run(&mut sim, 1);
        assert!(sim.debug_kill(id));
        run(&mut sim, 1);
        assert!(sim.raid_locked(BOSS, story), "locked on its own difficulty");
        assert!(!sim.raid_locked(BOSS, !story), "the other one is free");
    }
}
