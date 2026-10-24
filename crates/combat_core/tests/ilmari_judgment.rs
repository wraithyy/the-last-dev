//! Judgment Day (docs/ILMARI_1_0.md D.1, task D1): the lead's pick and its quorum, Full-clear and
//! zone rules, the numbers on top of Full, no Async and no companions, one revive per player per
//! pull, the `jd:` lockout key, the per-boss rider and old saves. Wings 5-6 and the core do not
//! exist yet, so Wing 1's zones are renamed into the Judgment range for the fight tests.
#![allow(clippy::expect_used)]

mod common;

use combat_core::companion::DALE;
use combat_core::ilmari::IlmariMode;
use combat_core::judgment::{IlmariModeError, RiderDef};
use combat_core::{Command, Content, EntityId, Event, SaveData, Sim, SpellId, Vec3};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const BOSS: &str = "il_patch_tuesday";
const RIDER: &str = "guardrails";

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, -20.0),
        n if n.starts_with("prop:") => at(1.0, 0.0),
        _ => at(0.0, 0.0),
    }
}

/// Wing 1 moved to `ilmari5/*`, with one rider on its boss, and sturdy players.
fn judgment_content() -> Content {
    let mut c = content();
    for z in &mut c.zones {
        if let Some(n) = z.id.strip_prefix("ilmari1/") {
            z.id = format!("ilmari5/{n}");
        }
    }
    for r in &mut c.raids {
        for z in &mut r.zones {
            if let Some(n) = z.strip_prefix("ilmari1/") {
                *z = format!("ilmari5/{n}");
            }
        }
    }
    c.ilmari.judgment.riders = vec![RiderDef {
        boss: BOSS.into(),
        aura: RIDER.into(),
        on: true,
    }];
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
    c
}

fn room(n: usize, c: Content) -> (Sim, Vec<EntityId>) {
    let classes = ["backend", "devops", "fullstack", "frontend"];
    let mut sim = Sim::new(c, 33);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids = (0..n)
        .map(|k| {
            let p = sim.spawn_player(classes[k % 4], Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, 10);
            p
        })
        .collect();
    (sim, ids)
}

fn load(sim: &mut Sim, zone: &str) {
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{zone}: {e}");
    }
}

fn boss(sim: &Sim) -> EntityId {
    sim.units()
        .find(|u| u.key == BOSS && u.alive)
        .map(|u| u.id)
        .expect("boss")
}

fn mode_events(sim: &mut Sim) -> Vec<IlmariModeError> {
    sim.tick()
        .into_iter()
        .filter_map(|e| match e {
            Event::IlmariRefused { reason } => Some(reason),
            _ => None,
        })
        .collect()
}

fn pick(sim: &mut Sim, who: EntityId, mode: IlmariMode) -> Vec<IlmariModeError> {
    sim.apply_as(who, Command::SetIlmariMode(mode));
    mode_events(sim)
}

/// Judgment picked, loaded, and the first member pulling the boss.
fn judged() -> (Sim, Vec<EntityId>, EntityId) {
    let (mut sim, ids) = room(3, judgment_content());
    sim.debug_ilmari_core_cleared(true);
    assert!(pick(&mut sim, ids[0], IlmariMode::Judgment).is_empty());
    load(&mut sim, "ilmari5/1");
    let b = boss(&sim);
    for &p in &ids {
        sim.apply_as(
            p,
            Command::PlayerMoved {
                pos: Vec3::new(0.0, 0.0, -8.0),
                yaw: 0.0,
                moving: false,
                jumping: false,
            },
        );
    }
    sim.apply_as(
        ids[0],
        Command::SetTarget {
            unit: ids[0],
            target: Some(b),
        },
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: SpellId(1),
        },
    );
    sim.tick();
    (sim, ids, b)
}

#[test]
fn judgment_needs_a_quorum_a_full_clear_and_the_lead() {
    let (mut sim, ids) = room(2, judgment_content());
    sim.debug_ilmari_core_cleared(true);
    assert_eq!(
        pick(&mut sim, ids[0], IlmariMode::Judgment),
        [IlmariModeError::Quorum],
        "two humans are no quorum"
    );
    assert_eq!(
        sim.raid_view().mode_next,
        IlmariMode::Story,
        "no silent switch"
    );
    assert!(
        IlmariModeError::Quorum
            .message()
            .contains("Quorum not reached. This meeting has been rescheduled.")
    );

    let (mut sim, ids) = room(3, judgment_content());
    assert_eq!(
        pick(&mut sim, ids[0], IlmariMode::Judgment),
        [IlmariModeError::NotCleared],
        "Ilmari has not been killed on Full"
    );
    sim.debug_ilmari_core_cleared(true);
    assert_eq!(
        pick(&mut sim, ids[1], IlmariMode::Judgment),
        [IlmariModeError::NotLeader]
    );
    assert!(pick(&mut sim, ids[0], IlmariMode::Judgment).is_empty());
    assert_eq!(sim.raid_view().mode_next, IlmariMode::Judgment);
    // The alias still sets Story and Full.
    sim.apply_as(ids[0], Command::SetIlmariStory { story: true });
    assert!(mode_events(&mut sim).is_empty());
    assert_eq!(sim.raid_view().mode_next, IlmariMode::Story);
    sim.apply_as(ids[0], Command::SetIlmariStory { story: false });
    sim.tick();
    assert_eq!(sim.raid_view().mode_next, IlmariMode::Full);
}

#[test]
fn wings_one_to_four_have_no_judgment_day() {
    let (mut sim, ids) = room(3, content());
    sim.debug_ilmari_core_cleared(true);
    // Picked at the base, it loads as Full in a Wing 1-4 zone ...
    assert!(pick(&mut sim, ids[0], IlmariMode::Judgment).is_empty());
    load(&mut sim, "ilmari1/1");
    assert_eq!(sim.ilmari_mode(), IlmariMode::Full);
    assert!(!sim.raid_async());
    // ... and cannot be picked from inside one.
    assert!(
        pick(&mut sim, ids[0], IlmariMode::Full).is_empty(),
        "Full is always fine"
    );
    assert_eq!(
        pick(&mut sim, ids[0], IlmariMode::Judgment),
        [IlmariModeError::WrongZone]
    );
}

#[test]
fn judgment_sits_on_full_has_no_async_and_keys_its_lockout() {
    let (mut full, ids) = room(3, judgment_content());
    full.apply_as(ids[0], Command::SetIlmariMode(IlmariMode::Full));
    full.tick();
    load(&mut full, "ilmari5/1");
    let full_hp = full.unit(boss(&full)).expect("boss").max_hp;
    assert_eq!(full.ilmari_mode(), IlmariMode::Full);
    assert!(
        full.unit(boss(&full))
            .expect("boss")
            .auras
            .iter()
            .all(|a| a.key != RIDER)
    );

    let (mut sim, ids, b) = judged();
    let view = sim.raid_view();
    assert_eq!(view.mode, IlmariMode::Judgment);
    assert!(
        !sim.raid_async() && !view.story && !view.async_mode,
        "no Async scaling"
    );
    let hp = sim.unit(b).expect("boss").max_hp;
    assert_eq!(
        u64::from(hp),
        u64::from(full_hp) * 130 / 100,
        "HP x1.30 on Full"
    );
    assert!(
        sim.unit(b)
            .expect("boss")
            .auras
            .iter()
            .any(|a| a.key == RIDER),
        "the rider is on the boss"
    );
    assert_eq!(view.key(BOSS), "jd:il_patch_tuesday");

    sim.debug_kill(b);
    sim.tick();
    let locked = sim.raid_view().locked;
    assert!(
        locked.contains(&"jd:il_patch_tuesday".to_string()),
        "{locked:?}"
    );
    assert!(sim.raid_locked(BOSS, false));
    assert!(
        !locked
            .iter()
            .any(|k| k == BOSS || k == "story:il_patch_tuesday")
    );
    // A Full run of the same week is not locked by it.
    let _ = ids;
}

#[test]
fn riders_switch_individually() {
    let mut c = judgment_content();
    c.ilmari.judgment.riders[0].on = false;
    let (mut sim, ids) = room(3, c);
    sim.debug_ilmari_core_cleared(true);
    assert!(pick(&mut sim, ids[0], IlmariMode::Judgment).is_empty());
    load(&mut sim, "ilmari5/1");
    sim.tick();
    assert!(
        sim.unit(boss(&sim))
            .expect("boss")
            .auras
            .iter()
            .all(|a| a.key != RIDER)
    );
}

#[test]
fn judgment_has_no_companions() {
    let (mut sim, ids) = room(3, judgment_content());
    sim.debug_ilmari_core_cleared(true);
    sim.set_companion_picks(ids[0], &[DALE.to_string()]);
    assert!(pick(&mut sim, ids[0], IlmariMode::Judgment).is_empty());
    load(&mut sim, "ilmari5/1");
    assert!(sim.companions_of(ids[0]).is_empty());
    // Story on the same wing does bring them.
    let (mut story, sids) = room(1, judgment_content());
    story.set_companion_picks(sids[0], &[DALE.to_string()]);
    story.apply_as(sids[0], Command::SetIlmariMode(IlmariMode::Story));
    story.tick();
    load(&mut story, "ilmari5/1");
    assert!(!story.companions_of(sids[0]).is_empty());
}

#[test]
fn one_revive_per_player_per_pull() {
    let (mut sim, ids, b) = judged();
    let victim = ids[1];
    let respawn = |sim: &mut Sim| {
        sim.apply_as(victim, Command::RespawnPlayer);
        sim.tick()
    };
    assert!(sim.unit(b).expect("boss").in_combat, "the pull is on");
    assert!(sim.debug_kill_player(victim));
    sim.tick();
    respawn(&mut sim);
    assert!(
        sim.unit(victim).expect("player").alive,
        "the first revive is free"
    );
    assert_eq!(sim.raid_view().spent, [victim], "the party frame shows it");

    assert!(sim.debug_kill_player(victim));
    let events = respawn(&mut sim);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::IlmariRefused {
                reason: IlmariModeError::Redundancy
            }
        )),
        "{events:?}"
    );
    assert!(
        !sim.unit(victim).expect("player").alive,
        "the second stays down"
    );
    // Another player still has theirs.
    assert!(sim.debug_kill_player(ids[2]));
    sim.tick();
    sim.apply_as(ids[2], Command::RespawnPlayer);
    sim.tick();
    assert!(sim.unit(ids[2]).expect("player").alive);

    // The pull ends with the boss: revives come back, the dead can get up.
    sim.debug_kill(b);
    sim.tick();
    assert!(sim.raid_view().spent.is_empty());
    respawn(&mut sim);
    assert!(sim.unit(victim).expect("player").alive);
}

#[test]
fn the_judgment_block_pays_three_pieces_and_two_fragments() {
    let c = judgment_content();
    let jd = &c.ilmari.judgment;
    assert_eq!((jd.enrage_pct, jd.boss_items, jd.fragment_mult), (80, 3, 2));
    // Full pays the wing's own `boss_items`; Judgment the content block's. (The enrage factor is
    // read by `check_enrage` and tuned per boss with the bosses in D6-D13.)
    let wing = c.raids.iter().find(|r| r.ilmari).expect("wing");
    assert_eq!(wing.boss_items, 2);
    let rewards = &c.ilmari.rewards;
    assert_eq!(
        rewards.fragments_for(IlmariMode::Story, jd.fragment_mult),
        0
    );
    assert_eq!(rewards.fragments_for(IlmariMode::Full, jd.fragment_mult), 1);
    assert_eq!(
        rewards.fragments_for(IlmariMode::Judgment, jd.fragment_mult),
        2
    );
}

#[test]
fn the_full_clear_flag_saves_and_old_saves_load_uncleared() {
    let stamp = |sim: &mut Sim, p: EntityId| {
        sim.debug_mark_boss("agi_prototype");
        sim.apply_as(
            p,
            Command::StampIlmari {
                unix_secs: 1_792_598_843,
                utc_offset_min: 120,
                tz: "CEST".into(),
            },
        );
        sim.tick();
    };
    let (mut sim, ids) = room(1, content());
    stamp(&mut sim, ids[0]);
    let text = sim.export_save(ids[0]).expect("save").encode();
    assert!(text.contains("ilmari_flags=0,0,0\n"), "{text}");
    assert!(!sim.ilmari_full_cleared());

    sim.debug_ilmari_core_cleared(true);
    let text = sim.export_save(ids[0]).expect("save").encode();
    assert!(text.contains("ilmari_flags=0,1,0\n"), "{text}");
    let mut back = Sim::new(content(), 5);
    let q = back.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(back.import_save(q, &SaveData::decode(&text).expect("decode")));
    assert!(back.ilmari_full_cleared());

    // A 0.10 save: flags have no second field's worth of meaning, the old text has no key at all.
    let old = text.replace("ilmari_flags=0,1,0\n", "ilmari_flags=1,0,0\n");
    let mut older = Sim::new(content(), 5);
    let r = older.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(older.import_save(r, &SaveData::decode(&old).expect("decode")));
    assert!(!older.ilmari_full_cleared());
}

#[test]
fn judgment_content_validates_and_the_default_block_is_the_documented_one() {
    assert!(Content::from_ron(include_str!("../../../content/game.ron")).is_ok());
    let c = content();
    let jd = &c.ilmari.judgment;
    assert_eq!(
        (
            jd.hp_pct,
            jd.damage_pct,
            jd.enrage_pct,
            jd.min_humans,
            jd.boss_items
        ),
        (130, 125, 80, 3, 3)
    );
    // Numbers below Full, a quorum below three or a rider on a non-boss are refused.
    let ron = include_str!("../../../content/game.ron");
    for (from, to) in [
        (
            "hp_pct: 130, damage_pct: 125",
            "hp_pct: 90, damage_pct: 125",
        ),
        ("min_humans: 3, boss_items", "min_humans: 2, boss_items"),
        ("aura: \"il_rider_training\"", "aura: \"no_such_aura\""),
    ] {
        assert!(ron.contains(from), "{from}");
        assert!(
            Content::from_ron(&ron.replacen(from, to, 1)).is_err(),
            "{from}"
        );
    }
}
