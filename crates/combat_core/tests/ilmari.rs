//! The post-credits hook (docs/ILMARI_09.md B.1, B.9 "ilmari_gating"): nothing about Ilmari
//! exists before the AGI is down, the stamp is taken once, the reveal hands the campus over,
//! and old saves that already finished the game get the Opening without a migration.

mod common;

use combat_core::{Command, EntityId, QuestStatus, SaveData, Sim, Vec3};
use common::content;

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let far = name.starts_with("pack_") || name.starts_with("rare_") || name == "boss";
    let at = if far { 400.0 } else { 0.0 };
    Some((Vec3::new(at, 0.0, at), 0.0))
}

fn campus(bosses: &[&str]) -> (Sim, EntityId) {
    let mut sim = Sim::new(content(), 7);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(p, 10);
    for b in bosses {
        sim.debug_mark_boss(b);
    }
    if let Err(e) = sim.load_zone("campus", &anchors, Vec::new()) {
        panic!("{e}");
    }
    (sim, p)
}

fn keys(sim: &Sim) -> Vec<String> {
    sim.units()
        .filter(|u| u.alive && (u.key.starts_with("il_") || u.key.starts_with("ill_marie")))
        .map(|u| u.key.clone())
        .collect()
}

const ALL_BUT_AGI: [&str; 4] = [
    "onboarding_buddy",
    "big_bang_cutover",
    "the_scheduler",
    "ceo",
];

/// 2026-10-21 16:07:23 UTC.
const STAMP: (u64, i16, &str) = (1_792_598_843, 120, "CEST");

#[test]
fn ilmari_gating_nothing_before_the_agi() {
    let c = content();
    let (mut sim, p) = campus(&ALL_BUT_AGI);
    assert!(!sim.ilmari_pending() && !sim.ilmari_open());
    assert_eq!(sim.quest_status_of(p, "il_00_wake"), QuestStatus::Locked);
    // No Ilmari unit in any zone, whatever else is done.
    for z in &c.zones {
        let mut s = Sim::new(content(), 3);
        s.set_wandering(false);
        let q = s.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
        s.set_level_of(q, 10);
        for b in ALL_BUT_AGI {
            s.debug_mark_boss(b);
        }
        if s.load_zone(&z.id, &anchors, Vec::new()).is_ok() {
            assert!(
                !s.units().any(|u| u.key == "il_marie_awake"),
                "{} spawns the reveal Marie early",
                z.id
            );
        }
    }
    // The clock and the reveal are ignored until the credits.
    sim.apply_as(
        p,
        Command::StampIlmari {
            unix_secs: STAMP.0,
            utc_offset_min: STAMP.1,
            tz: STAMP.2.into(),
        },
    );
    sim.apply_as(p, Command::IlmariReveal);
    sim.tick();
    assert!(sim.ilmari_stamp().is_none() && !sim.ilmari_open());
    assert_eq!(
        keys(&sim),
        ["ill_marie", "ill_marie_queue"],
        "only the base-game Maries"
    );
    // A save of that run carries no Ilmari key.
    let text = sim.export_save(p).map(|s| s.encode()).unwrap_or_default();
    assert!(!text.contains("ilmari"), "{text}");
}

#[test]
fn the_opening_is_due_once_the_agi_is_down_and_hands_the_campus_over() {
    let (mut sim, p) = campus(&["agi_prototype"]);
    assert!(sim.ilmari_pending());
    assert_eq!(sim.quest_status_of(p, "il_00_wake"), QuestStatus::Available);
    let k = keys(&sim);
    for want in ["ill_marie", "ill_marie_bench", "il_marie_awake"] {
        assert!(k.iter().any(|x| x == want), "{want} in {k:?}");
    }
    // Revealing before the clock is read does nothing.
    sim.apply_as(p, Command::IlmariReveal);
    sim.tick();
    assert!(sim.ilmari_pending());

    let stamp = |sim: &mut Sim, t: u64| {
        sim.apply_as(
            p,
            Command::StampIlmari {
                unix_secs: t,
                utc_offset_min: STAMP.1,
                tz: STAMP.2.into(),
            },
        );
        sim.tick();
    };
    stamp(&mut sim, STAMP.0);
    stamp(&mut sim, STAMP.0 + 999);
    let line = sim.ilmari_stamp().map(|s| s.line()).unwrap_or_default();
    assert_eq!(
        line, "ILMARI became self-aware at 18:07:23 CEST, 21 October 2026.",
        "the first stamp wins"
    );

    sim.apply_as(p, Command::IlmariReveal);
    sim.tick();
    assert!(!sim.ilmari_pending() && sim.ilmari_open());
    assert_eq!(sim.quest_status_of(p, "il_00_wake"), QuestStatus::Done);
    // The next main step is already in the log: nobody has to find Dale's hatch by luck.
    assert_eq!(sim.quest_status_of(p, "il_01_dale"), QuestStatus::Active);
    assert!(keys(&sim).is_empty(), "{:?}", keys(&sim));
    // The next campus load: the desk, the bench and the reveal Marie are gone.
    sim.load_zone("campus", &anchors, Vec::new())
        .expect("campus");
    assert!(keys(&sim).is_empty(), "{:?}", keys(&sim));
}

#[test]
fn saves_carry_the_stamp_and_old_finished_saves_get_the_opening() {
    let (mut sim, p) = campus(&["agi_prototype"]);
    // An old save of a finished game: bosses has the AGI, no Ilmari keys.
    let old = sim.export_save(p).expect("save");
    assert!(old.ilmari_stamp.is_none() && !old.ilmari_revealed);
    let text = old.encode();
    assert!(text.contains("agi_prototype") && !text.contains("ilmari"));
    let mut fresh = Sim::new(content(), 9);
    let q = fresh.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(fresh.import_save(q, &SaveData::decode(&text).expect("decode")));
    assert!(fresh.ilmari_pending());

    sim.apply_as(
        p,
        Command::StampIlmari {
            unix_secs: STAMP.0,
            utc_offset_min: STAMP.1,
            tz: STAMP.2.into(),
        },
    );
    sim.tick();
    let text = sim.export_save(p).expect("save").encode();
    assert!(text.contains("ilmari_at=1792598843,120,CEST\n"), "{text}");
    assert!(text.contains("ilmari_flags=0,0,0\n"));
    sim.apply_as(p, Command::IlmariReveal);
    sim.tick();
    let text = sim.export_save(p).expect("save").encode();
    assert!(text.contains("ilmari_flags=1,0,0\n"));

    let mut back = Sim::new(content(), 11);
    let r = back.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(back.import_save(r, &SaveData::decode(&text).expect("decode")));
    assert!(back.ilmari_open() && !back.ilmari_pending());
    assert_eq!(
        back.ilmari_stamp().map(|s| s.clock()).as_deref(),
        Some("18:07:23 CEST")
    );
    assert_eq!(back.quest_status_of(r, "il_00_wake"), QuestStatus::Done);

    // A hand-edited flag without a clock cannot skip the Opening.
    let forged = text.replace("ilmari_at=1792598843,120,CEST\n", "");
    let mut f = Sim::new(content(), 12);
    let s = f.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(f.import_save(s, &SaveData::decode(&forged).expect("decode")));
    assert!(f.ilmari_pending());
}

#[test]
fn the_reveal_has_no_spoiler_before_the_credits_in_any_player_text() {
    // The reveal unit and its quest are the only content that may name her new role, and both
    // are gated on the AGI; every other friendly line stays free of the word.
    let c = content();
    for u in c
        .units
        .iter()
        .filter(|u| u.key != "il_marie_awake" && !u.key.starts_with("il_"))
    {
        for t in u.yells.talk.iter().chain(&u.yells.ambient) {
            assert!(!t.to_lowercase().contains("ilmari"), "{}: {t}", u.key);
        }
    }
}

/// Wings 5-6 and the core (docs/ILMARI_1_0.md D.11, D5): none of their units stands in a base zone,
/// no base zone links to them, their doors stay locked until their quest, their quests wait for
/// the wing before, and Marie's desk prop belongs to the occupied campus only.
#[test]
fn ilmari_gating_wings_five_six_and_the_core() {
    let c = content();
    let new_zone = |z: &str| {
        z.starts_with("ilmari5/") || z.starts_with("ilmari6/") || z.starts_with("ilmari_core/")
    };
    // The units of the new zones (and the two attunement elites that stand on the occupied campus).
    let mut new_units: Vec<String> = c
        .zones
        .iter()
        .filter(|z| new_zone(&z.id))
        .flat_map(|z| c.zone_hostiles(&z.id).into_iter().map(String::from))
        .collect();
    new_units.extend(["il_ethics_chaperone", "il_crane_foreman", "il_surveyor"].map(String::from));
    assert!(new_units.len() > 6);
    for z in c.zones.iter().filter(|z| !is_ilmari_or_base(&z.id)) {
        let mut s = Sim::new(content(), 3);
        s.set_wandering(false);
        let q = s.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
        s.set_level_of(q, 10);
        for b in ALL_BUT_AGI.iter().chain(&["agi_prototype"]) {
            s.debug_mark_boss(b);
        }
        for l in &z.links {
            assert!(!new_zone(&l.to), "{} links to {}", z.id, l.to);
        }
        if s.load_zone(&z.id, &anchors, Vec::new()).is_ok() {
            assert!(
                !s.units().any(|u| new_units.contains(&u.key)),
                "{} spawns an Ilmari 1.0 unit",
                z.id
            );
        }
    }
    // The prop is the occupied campus's only.
    let desks: Vec<&str> = c
        .world
        .props
        .iter()
        .filter(|p| p.key == "il_marie_desk")
        .map(|p| p.zone.as_str())
        .collect();
    assert_eq!(desks, ["campus_occupied"]);
    // Quests wait for the wing before; doors wait for their quest.
    let (mut sim, p) = campus(&["agi_prototype"]);
    sim.apply_as(
        p,
        Command::StampIlmari {
            unix_secs: STAMP.0,
            utc_offset_min: STAMP.1,
            tz: STAMP.2.into(),
        },
    );
    sim.apply_as(p, Command::IlmariReveal);
    sim.tick();
    for q in [
        "il_11_ethics_intel",
        "il_13_topping_out",
        "il_15_last_commit",
    ] {
        assert_eq!(sim.quest_status_of(p, q), QuestStatus::Locked, "{q}");
    }
    sim.load_zone("campus_occupied", &anchors, Vec::new())
        .expect("occupied campus");
    let open = |sim: &Sim, zone: &str, to: &str| {
        sim.zone_links(zone)
            .iter()
            .find(|l| l.to == to)
            .is_some_and(|l| l.unlocked)
    };
    assert!(!open(&sim, "campus_occupied", "ilmari5/1"));
    assert!(!open(&sim, "campus_occupied", "ilmari6/1"));
    sim.debug_set_quest("il_w4_done", QuestStatus::Done);
    assert_eq!(
        sim.quest_status_of(p, "il_11_ethics_intel"),
        QuestStatus::Available
    );
    assert_eq!(
        sim.quest_status_of(p, "il_13_topping_out"),
        QuestStatus::Locked
    );
    sim.debug_set_quest("il_12_wing5_attune", QuestStatus::Done);
    assert!(open(&sim, "campus_occupied", "ilmari5/1"));
    assert!(!open(&sim, "campus_occupied", "ilmari6/1"));
    sim.debug_set_quest("il_w5_done", QuestStatus::Done);
    assert_eq!(
        sim.quest_status_of(p, "il_13_topping_out"),
        QuestStatus::Available
    );
    sim.debug_set_quest("il_14_wing6_attune", QuestStatus::Done);
    assert!(open(&sim, "campus_occupied", "ilmari6/1"));
    sim.load_zone("ilmari6/3", &anchors, Vec::new())
        .expect("boardroom");
    assert!(!open(&sim, "ilmari6/3", "ilmari_core/1"));
    sim.debug_set_quest("il_15_last_commit", QuestStatus::Done);
    assert!(open(&sim, "ilmari6/3", "ilmari_core/1"));
}

fn is_ilmari_or_base(zone: &str) -> bool {
    zone.starts_with("ilmari") || zone == "campus_occupied" || zone == "resistance/1"
}
