//! Resistance companions (docs/ILMARI_09.md B.7): the sign-up sheet, who comes along and who does
//! not, the role behaviours, the scaling rule, the Recruiter's humans, the room cap and the save.
//! Whether Story is still a fair fight with company lives in tests/playthrough.rs.
#![allow(clippy::expect_used)]

mod common;

use combat_core::companion::{DALE, MAX_PER_ROOM, PRIYA, ROSTER, RUPERT};
use combat_core::{Command, Content, EntityId, Event, SaveData, Sim, SpellId, Vec3};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, -20.0),
        n if n.starts_with("prop:") => at(1.0, 0.0),
        _ => at(0.0, 0.0),
    }
}

/// `n` level-10 players of the same sturdy build as the wing tests, so only the companions can die.
fn room(n: usize, story: bool, seed: u64) -> (Sim, Vec<EntityId>) {
    let mut c = content();
    for k in [
        "senior_engineer",
        "backend_dev",
        "devops_eng",
        "frontend_dev",
    ] {
        def_mut(&mut c, k).max_hp = 10_000_000;
    }
    let mut sim = Sim::new(c, seed);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids: Vec<EntityId> = (0..n)
        .map(|k| {
            let p = sim.spawn_player("fullstack", Vec3::new(k as f32, 0.0, 0.0), 0.0);
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

fn keys(k: &[&str]) -> Vec<String> {
    k.iter().map(|s| s.to_string()).collect()
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

/// The owner opens on `t` with Code Review from 12 m; the companions follow their own rules.
fn open_on(sim: &mut Sim, owner: EntityId, t: EntityId) {
    move_to(sim, owner, Vec3::new(0.0, 0.0, -8.0));
    sim.apply_as(
        owner,
        Command::SetTarget {
            unit: owner,
            target: Some(t),
        },
    );
    sim.apply_as(
        owner,
        Command::Cast {
            caster: owner,
            spell: SpellId(1),
        },
    );
}

fn companion_of(sim: &Sim, owner: EntityId, key: &str) -> EntityId {
    sim.companions_of(owner)
        .into_iter()
        .find(|c| sim.unit(*c).is_some_and(|u| u.key == key))
        .expect("companion out")
}

/// Standing at the sheet in the base: `Talk` is the whole interface.
fn talk_to_sheet(sim: &mut Sim, p: EntityId) -> Vec<String> {
    let sheet = unit_of(sim, ROSTER).expect("the sheet hangs in the base");
    sim.apply_as(p, Command::Talk { unit: sheet });
    sim.tick()
        .into_iter()
        .filter_map(|e| match e {
            Event::Yell { unit, text } if unit == sheet => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn the_sign_up_sheet_cycles_pairs_and_full_refuses() {
    let (mut sim, ids) = room(1, true, 5);
    let p = ids[0];
    load(&mut sim, "resistance/1");
    let mut seen = Vec::new();
    for _ in 0..5 {
        let said = talk_to_sheet(&mut sim, p);
        assert_eq!(said.len(), 1, "one answer a talk: {said:?}");
        seen.push(sim.companion_picks(p));
    }
    assert_eq!(
        seen,
        vec![
            keys(&[DALE, PRIYA]),
            keys(&[DALE, RUPERT]),
            keys(&[PRIYA, RUPERT]),
            keys(&[]),
            keys(&[DALE, PRIYA]),
        ]
    );
    // Full is the group fight: the sheet says so and the loadout stays as it was.
    sim.debug_force_raid_async(Some(false));
    let said = talk_to_sheet(&mut sim, p);
    assert!(said[0].contains("Full is a real fight"), "{said:?}");
    assert_eq!(sim.companion_picks(p), keys(&[DALE, PRIYA]));
}

#[test]
fn companions_walk_into_a_story_wing_and_never_into_full() {
    let (mut sim, ids) = room(1, true, 6);
    let p = ids[0];
    sim.set_companion_picks(p, &keys(&[DALE, RUPERT]));
    load(&mut sim, "ilmari1/1");
    let out = sim.companions_of(p);
    assert_eq!(out.len(), 2);
    for c in &out {
        let u = sim.unit(*c).expect("unit");
        assert!(u.alive && !u.hostile, "{u:?}");
        assert!(combat_core::distance(u.pos, sim.unit(p).expect("me").pos) < 6.0);
    }
    // The next room brings them along; the base and the open campus do not.
    load(&mut sim, "ilmari1/2");
    assert_eq!(sim.companions_of(p).len(), 2);
    load(&mut sim, "resistance/1");
    assert!(sim.companions_of(p).is_empty());

    sim.debug_force_raid_async(Some(false));
    load(&mut sim, "ilmari1/1");
    assert!(
        sim.companions_of(p).is_empty(),
        "Full is the group difficulty"
    );
    assert_eq!(
        sim.companion_picks(p),
        keys(&[DALE, RUPERT]),
        "kept for Story"
    );
}

#[test]
fn a_player_brings_two_and_a_room_holds_two() {
    let (mut sim, ids) = room(1, true, 7);
    sim.set_companion_picks(ids[0], &keys(&[DALE, PRIYA, RUPERT, DALE, "il_roster"]));
    assert_eq!(sim.companion_picks(ids[0]), keys(&[DALE, PRIYA]));

    let (mut sim, ids) = room(2, true, 8);
    for p in &ids {
        sim.set_companion_picks(*p, &keys(&[DALE, PRIYA]));
    }
    load(&mut sim, "ilmari1/1");
    let total: usize = ids.iter().map(|p| sim.companions_of(*p).len()).sum();
    assert_eq!(total, MAX_PER_ROOM, "two in the room, whoever brought them");
    assert_eq!(
        sim.companions_of(ids[0]).len(),
        2,
        "join order breaks the tie"
    );

    // Leaving takes the owner's companions along and eases the scaling back.
    let boss_hp = |sim: &Sim| {
        sim.units()
            .find(|u| u.key == "il_patch_tuesday")
            .map_or(0, |u| u.max_hp)
    };
    let with = boss_hp(&sim);
    assert!(sim.remove_player(ids[0]));
    assert!(sim.companions_of(ids[0]).is_empty());
    assert!(
        boss_hp(&sim) < with,
        "no companions, no group in the numbers"
    );
}

#[test]
fn each_companion_counts_a_share_of_a_player_for_the_enemy_scaling_and_full_has_none() {
    let boss_hp = |picks: &[&str], story: bool| {
        let (mut sim, ids) = room(1, story, 9);
        sim.set_companion_picks(ids[0], &keys(picks));
        load(&mut sim, "ilmari1/1");
        sim.units()
            .find(|u| u.key == "il_patch_tuesday")
            .expect("boss")
            .max_hp
    };
    let (alone, one, two) = (
        boss_hp(&[], true),
        boss_hp(&[DALE], true),
        boss_hp(&[DALE, PRIYA], true),
    );
    let c = content().config.clone();
    let pct = |n: u32| 100 + c.group_hp_pct_per_player * c.companion_group_pct * n / 100;
    let near = |a: u32, b: u32| a.abs_diff(b) <= b / 100 + 1;
    assert!(near(one, alone * pct(1) / 100), "{alone} {one}");
    assert!(near(two, alone * pct(2) / 100), "{alone} {two}");
    assert!(
        one > alone && two > one,
        "company never makes a boss weaker"
    );
    // Full ignores the loadout: nobody comes, so nothing scales.
    assert_eq!(boss_hp(&[DALE, PRIYA], false), boss_hp(&[], false));
}

/// Fights the Recruiter for `secs` with the owner opening on him and the boss kept alive.
fn recruiter_fight(seed: u64, secs: u32) -> (Sim, EntityId, Vec<Event>) {
    let (mut sim, ids) = room(1, true, seed);
    let p = ids[0];
    sim.set_companion_picks(p, &keys(&[DALE, RUPERT]));
    load(&mut sim, "ilmari1/2");
    let boss = unit_of(&sim, "il_recruiter").expect("boss");
    open_on(&mut sim, p, boss);
    let mut log = Vec::new();
    for _ in 0..secs * SECOND {
        log.extend(sim.tick());
        sim.debug_set_hp(boss, 10_000_000);
    }
    (sim, p, log)
}

#[test]
fn companions_never_hit_the_recruiters_humans_and_do_fight() {
    let (sim, p, log) = recruiter_fight(10, 50);
    let mine = sim.companions_of(p);
    let humans: Vec<EntityId> = sim
        .units()
        .filter(|u| u.key == "il_candidate_human")
        .map(|u| u.id)
        .collect();
    assert!(!humans.is_empty(), "the candidates came");
    let by_company = |e: &Event| match e {
        Event::Damage { source, target, .. } => mine.contains(source).then_some(*target),
        _ => None,
    };
    let hits: Vec<EntityId> = log.iter().filter_map(by_company).collect();
    assert!(hits.len() > 10, "they fight: {} hits", hits.len());
    assert!(
        hits.iter().all(|t| !humans.contains(t)),
        "a person was hit by a companion"
    );
    // A person who walks out of the room dies with no killer; nobody's hand killed one.
    assert!(!log.iter().any(|e| matches!(
        e,
        Event::Death { unit, killer: Some(_) } if humans.contains(unit)
    )));
    // A companion's kill is the owner's, so no death names a companion as the killer.
    assert!(!log.iter().any(|e| matches!(
        e,
        Event::Death { killer: Some(k), .. } if mine.contains(k)
    )));
}

#[test]
fn a_replay_with_companions_is_bit_identical() {
    let fingerprint = |seed| {
        let (sim, p, log) = recruiter_fight(seed, 30);
        let pos: Vec<Vec3> = sim
            .companions_of(p)
            .iter()
            .map(|c| sim.unit(*c).expect("unit").pos)
            .collect();
        format!("{log:?}{pos:?}")
    };
    assert_eq!(fingerprint(11), fingerprint(11));
    assert_ne!(fingerprint(11), fingerprint(12), "the seed still matters");
}

#[test]
fn the_tank_holds_the_boss_the_healer_heals_and_the_damage_dealer_kicks() {
    let (mut sim, ids) = room(1, true, 13);
    let p = ids[0];
    sim.set_companion_picks(p, &keys(&[DALE, PRIYA]));
    load(&mut sim, "ilmari1/1");
    let (dale, priya) = (companion_of(&sim, p, DALE), companion_of(&sim, p, PRIYA));
    let boss = unit_of(&sim, "il_patch_tuesday").expect("boss");
    open_on(&mut sim, p, boss);
    let mut taunted = false;
    let mut healed_owner = false;
    for t in 0..40 * SECOND {
        if t == 12 * SECOND {
            // Priya heals whoever is below 70%: the owner is the lowest.
            sim.debug_set_hp(p, 4_000_000);
        }
        for e in sim.tick() {
            match e {
                Event::CastSucceeded { caster, spell, .. } if caster == dale && spell.0 == 2491 => {
                    taunted = true;
                }
                Event::Heal { source, target, .. } if source == priya && target == p => {
                    healed_owner = true;
                }
                _ => {}
            }
        }
        sim.debug_set_hp(boss, 10_000_000);
    }
    assert!(taunted, "Dale asked for the boss's attention");
    assert!(healed_owner, "Priya healed the owner");
    assert!(
        sim.unit(dale).expect("dale").hp < sim.unit(dale).expect("dale").max_hp
            || sim.unit(boss).expect("boss").target == Some(dale),
        "the boss turned on the tank"
    );
}

#[test]
fn the_damage_dealer_interrupts_a_must_stop_cast() {
    let (mut sim, ids) = room(1, true, 14);
    let p = ids[0];
    sim.set_companion_picks(p, &keys(&[DALE, RUPERT]));
    load(&mut sim, "ilmari2/1");
    let rupert = companion_of(&sim, p, RUPERT);
    // The Stakeholder's "Quick Question" is a must-interrupt cast.
    let foe = sim
        .spawn_unit("il_stakeholder", Vec3::new(0.0, 0.0, -10.0), 0.0)
        .expect("stakeholder");
    open_on(&mut sim, p, foe);
    let mut kicked = false;
    for _ in 0..40 * SECOND {
        for e in sim.tick() {
            if let Event::CastInterrupted { by: Some(by), .. } = e
                && by == rupert
            {
                kicked = true;
            }
        }
        sim.debug_set_hp(foe, 10_000_000);
    }
    assert!(kicked, "Rupert used Hole Punch on a kick cast");
}

#[test]
fn companions_step_out_of_ground_telegraphs() {
    let (mut sim, ids) = room(1, true, 15);
    let p = ids[0];
    sim.set_companion_picks(p, &keys(&[DALE, RUPERT]));
    load(&mut sim, "ilmari2/1");
    let mine = sim.companions_of(p);
    // A turret's Toner Volley lands a small circle on its target, which is Dale once he taunts.
    let foe = sim
        .spawn_unit("il_printer_turret", Vec3::new(0.0, 0.0, -10.0), 0.0)
        .expect("turret");
    open_on(&mut sim, p, foe);
    let (mut volleys, mut standing) = (0, 0);
    for _ in 0..60 * SECOND {
        sim.tick();
        sim.debug_set_hp(foe, 10_000_000);
        for t in sim.telegraphs() {
            volleys += 1;
            // A quarter second before it lands nobody of the company may be inside.
            if t.remaining_ms < 250
                && mine
                    .iter()
                    .any(|c| sim.unit(*c).is_some_and(|u| u.alive && t.hits(u.pos)))
            {
                standing += 1;
            }
        }
    }
    assert!(volleys > 0, "the turret painted the floor");
    assert_eq!(standing, 0, "a companion stood in a landing telegraph");
}

#[test]
fn a_fallen_companion_comes_back_when_the_fight_is_over_for_free() {
    let (mut sim, ids) = room(1, true, 16);
    let p = ids[0];
    sim.set_companion_picks(p, &keys(&[DALE, PRIYA]));
    load(&mut sim, "ilmari1/1");
    let dale = companion_of(&sim, p, DALE);
    assert!(sim.debug_kill(dale));
    sim.tick();
    for _ in 0..4 * SECOND {
        sim.tick();
    }
    assert!(
        !sim.unit(dale).expect("dale").alive,
        "not mid-pull, not at once"
    );
    let mut back = false;
    for _ in 0..12 * SECOND {
        back |= sim
            .tick()
            .iter()
            .any(|e| matches!(e, Event::Respawned { unit } if *unit == dale));
    }
    let d = sim.unit(dale).expect("dale");
    assert!(back && d.alive && d.hp == d.max_hp, "{d:?}");
}

#[test]
fn the_loadout_survives_a_save_and_an_old_save_has_none() {
    let (mut sim, ids) = room(1, true, 17);
    let p = ids[0];
    sim.set_companion_picks(p, &keys(&[PRIYA, RUPERT]));
    let text = sim.export_save(p).expect("save").encode();
    assert!(
        text.contains("companions=il_comp_priya|il_comp_rupert"),
        "{text}"
    );
    let mut back = Sim::new(content(), 18);
    let r = back.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(back.import_save(r, &SaveData::decode(&text).expect("decode")));
    assert_eq!(back.companion_picks(r), keys(&[PRIYA, RUPERT]));

    // Unknown or doctored keys are dropped, and a save without the key stays clean.
    let doctored = text.replace("il_comp_rupert", "il_hunter_bot");
    let mut other = Sim::new(content(), 19);
    let o = other.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(other.import_save(o, &SaveData::decode(&doctored).expect("decode")));
    assert_eq!(other.companion_picks(o), keys(&[PRIYA]));
    let (plain, _) = {
        let (s, i) = room(1, true, 20);
        (s.export_save(i[0]).expect("save").encode(), ())
    };
    assert!(!plain.contains("companions"), "{plain}");
}

#[test]
fn companion_units_are_friendly_and_use_known_spells() {
    let c: Content = content();
    for k in [DALE, PRIYA, RUPERT] {
        let d = c.unit_def(k).expect("unit");
        let def = d.companion.as_ref().expect("companion");
        assert!(!d.hostile && d.ai.is_none() && d.healable, "{k}");
        assert!(c.spell(def.main).is_some(), "{k}");
    }
}

/// Wing 5 (D8): the Consultant and the Red Team read people, not companions. A companion is
/// never offered a guardrail, never marked by a cone, and the exercise starts only on the
/// owner's account; the fight replays bit for bit with company.
fn wing_five_fight(
    zone: &str,
    boss_key: &str,
    seed: u64,
    secs: u32,
) -> (Sim, EntityId, Vec<Event>) {
    let (mut sim, ids) = room(1, true, seed);
    let p = ids[0];
    sim.set_companion_picks(p, &keys(&[DALE, RUPERT]));
    load(&mut sim, zone);
    let boss = unit_of(&sim, boss_key).expect("boss");
    open_on(&mut sim, p, boss);
    let mut log = Vec::new();
    for _ in 0..secs * SECOND {
        log.extend(sim.tick());
        sim.debug_set_hp(boss, 10_000_000);
    }
    (sim, p, log)
}

#[test]
fn companions_are_never_offered_or_detected_in_wing_five() {
    for (zone, boss) in [
        ("ilmari5/2", "il_align_consultant"),
        ("ilmari5/3", "il_red_team"),
    ] {
        let (sim, p, log) = wing_five_fight(zone, boss, 21, 60);
        let mine = sim.companions_of(p);
        assert_eq!(mine.len(), 2, "{boss}: company came");
        let bad = |key: &str| {
            key.starts_with("il_ok") || key.starts_with("il_detected") || key == "il_aligned_stack"
        };
        assert!(
            !log.iter().any(|e| matches!(
                e,
                Event::AuraApplied { target, key } if mine.contains(target) && bad(key)
            )),
            "{boss}: a companion was offered or marked"
        );
    }
}

#[test]
fn a_wing_five_replay_with_companions_is_bit_identical() {
    let fingerprint = |zone: &str, boss: &str| {
        let (sim, p, log) = wing_five_fight(zone, boss, 22, 40);
        let pos: Vec<Vec3> = sim
            .companions_of(p)
            .iter()
            .map(|c| sim.unit(*c).expect("unit").pos)
            .collect();
        format!("{log:?}{pos:?}")
    };
    for (zone, boss) in [
        ("ilmari5/2", "il_align_consultant"),
        ("ilmari5/3", "il_red_team"),
    ] {
        assert_eq!(fingerprint(zone, boss), fingerprint(zone, boss), "{boss}");
    }
}

/// Wing 6 (D9): the inbox and the Notified debuff read people, not companions, and the fight
/// with company replays bit for bit, desks, tiles and parachute included.
#[test]
fn companions_are_never_notified_and_a_wing_six_replay_is_bit_identical() {
    let fingerprint = |boss_hp_pct: u32| {
        let (mut sim, ids) = room(1, true, 23);
        let p = ids[0];
        sim.set_companion_picks(p, &keys(&[DALE, RUPERT]));
        load(&mut sim, "ilmari6/3");
        let boss = unit_of(&sim, "il_cio").expect("boss");
        open_on(&mut sim, p, boss);
        let mut log = Vec::new();
        for k in 0..70 * SECOND {
            log.extend(sim.tick());
            if k == 10 * SECOND {
                let max = sim.unit(boss).expect("boss").max_hp;
                sim.debug_set_hp(boss, max * boss_hp_pct / 100);
            }
        }
        let mine = sim.companions_of(p);
        assert_eq!(mine.len(), 2, "company came");
        assert!(
            !log.iter().any(|e| matches!(
                e,
                Event::AuraApplied { target, key } if mine.contains(target) && key == "il_notified"
            )),
            "a companion was notified"
        );
        let pos: Vec<Vec3> = mine
            .iter()
            .map(|c| sim.unit(*c).expect("unit").pos)
            .collect();
        format!("{log:?}{pos:?}")
    };
    for pct in [50, 20] {
        assert_eq!(fingerprint(pct), fingerprint(pct), "at {pct}%");
    }
}

/// The core (D10, D11): Ilmari's screens are not Story's problem for a companion. They never
/// vote (Ill Marie's rounds decide by the people in the room, and silence takes the tie rule),
/// they are never put on standby or held, and the fight with company, through the end of the
/// remixes and every round of P2, replays bit for bit.
#[test]
fn companions_abstain_in_the_core_and_a_replay_with_company_is_bit_identical() {
    let run_core = || {
        let (mut sim, ids) = room(1, true, 24);
        let p = ids[0];
        sim.set_companion_picks(p, &keys(&[DALE, PRIYA]));
        load(&mut sim, "ilmari_core/1");
        let boss = unit_of(&sim, "il_ilmari").expect("boss");
        open_on(&mut sim, p, boss);
        let mut log = Vec::new();
        for k in 0..(40 + 3 * 70 + 40) * SECOND {
            log.extend(sim.tick());
            if k == 30 * SECOND {
                for s in sim
                    .units()
                    .filter(|u| u.key.starts_with("il_ilmari") && u.alive)
                    .map(|u| u.id)
                    .collect::<Vec<_>>()
                {
                    let max = sim.unit(s).expect("screen").max_hp;
                    sim.debug_set_hp(s, max * 69 / 100);
                }
            }
        }
        let mine = sim.companions_of(p);
        assert_eq!(mine.len(), 2, "company came");
        assert!(
            !log.iter().any(|e| matches!(
                e,
                Event::AuraApplied { target, key }
                    if mine.contains(target)
                        && (key == "il_screen_standby" || key == "il_alignment_hold" || key.starts_with("il_stance"))
            )),
            "a companion was put on standby, held or given a stance"
        );
        // Nobody voted: every round ended on the tie rule alone.
        assert!(
            log.iter().all(|e| !matches!(
                e,
                Event::Core { tally, .. } if tally.iter().any(|n| *n > 0)
            )),
            "a companion cast a vote"
        );
        assert!(
            log.iter()
                .any(|e| matches!(e, Event::PhaseChanged { phase: 3, .. })),
            "the rounds ran out on their own"
        );
        format!("{log:?}")
    };
    assert_eq!(run_core(), run_core());
}

// ---- The core's P3 and P4 (docs/ILMARI_1_0.md D.6, D14): company heals the Ward, taunts the
// rewrites and kills them; none of them touches a terminal or a punch card.

fn set_bar(sim: &mut Sim, pct: u32) {
    let all: Vec<EntityId> = sim
        .units()
        .filter(|u| u.key.starts_with("il_ilmari") && u.alive)
        .map(|u| u.id)
        .collect();
    for s in all {
        let max = sim.unit(s).expect("screen").max_hp;
        sim.debug_set_hp(s, max * pct / 100);
    }
}

/// The core on Story with company, brought to the first seconds of P4 (the bar at 49 % ends the
/// mech on Story). Nobody votes: the rounds end on the tie rule alone.
fn core_p4_with_company(picks: &[&str]) -> (Sim, EntityId, Vec<Event>) {
    let (mut sim, ids) = room(1, true, 24);
    let p = ids[0];
    sim.set_companion_picks(p, &keys(picks));
    load(&mut sim, "ilmari_core/1");
    let boss = unit_of(&sim, "il_ilmari").expect("boss");
    open_on(&mut sim, p, boss);
    let mut log = Vec::new();
    for k in 0..(30 + 3 * 70 + 40) * SECOND {
        log.extend(sim.tick());
        if k == 30 * SECOND {
            set_bar(&mut sim, 69);
        }
        if k == (30 + 3 * 70 + 30) * SECOND {
            set_bar(&mut sim, 49);
        }
    }
    (sim, p, log)
}

#[test]
fn priya_heals_the_mainframe_and_dale_takes_the_rewrites_off_it() {
    let (mut sim, p, _log) = core_p4_with_company(&[DALE, PRIYA]);
    let ward = unit_of(&sim, "il_core_mainframe").expect("the mainframe rose");
    let (dale, priya) = (companion_of(&sim, p, DALE), companion_of(&sim, p, PRIYA));
    sim.debug_set_hp(ward, 9_000);
    let mut healed = false;
    let mut taken = false;
    for _ in 0..60 * SECOND {
        for e in sim.tick() {
            if let Event::Heal { source, target, .. } = e
                && source == priya
                && target == ward
            {
                healed = true;
            }
        }
        taken |= sim
            .units()
            .any(|u| u.key == "il_modern_replacement" && u.alive && u.target == Some(dale));
    }
    assert!(healed, "Priya heals the mainframe below 70 %");
    assert!(taken, "Dale taunts a rewrite off it");
    assert!(
        sim.unit(ward).is_some_and(|u| u.alive),
        "and the mainframe is still up"
    );
}

#[test]
fn rupert_kills_the_rewrites_that_chew_the_mainframe() {
    let (mut sim, _p, _log) = core_p4_with_company(&[DALE, RUPERT]);
    let ward = unit_of(&sim, "il_core_mainframe").expect("the mainframe rose");
    let mut died = 0;
    for _ in 0..70 * SECOND {
        for e in sim.tick() {
            if let Event::Death { unit, .. } = e
                && sim
                    .unit(unit)
                    .is_some_and(|u| u.key == "il_modern_replacement")
            {
                died += 1;
            }
        }
    }
    assert!(died >= 3, "the damage dealer kills the rewrites: {died}");
    assert!(
        sim.unit(ward).is_some_and(|u| u.alive),
        "the mainframe holds"
    );
}

#[test]
fn company_never_touches_a_terminal_or_a_card_and_p3_and_p4_replay_bit_identically() {
    let run_it = || {
        let (mut sim, p, mut log) = core_p4_with_company(&[DALE, PRIYA]);
        for _ in 0..90 * SECOND {
            log.extend(sim.tick());
        }
        let mine = sim.companions_of(p);
        assert_eq!(mine.len(), 2, "company came");
        assert!(
            !log.iter().any(|e| matches!(
                e,
                Event::Interacted { unit, .. } if mine.contains(unit)
            )),
            "a companion used a prop"
        );
        assert!(
            log.iter()
                .any(|e| matches!(e, Event::PhaseChanged { phase: 4, .. })),
            "P4 began"
        );
        format!("{log:?}")
    };
    assert_eq!(run_it(), run_it());
}
