//! The dungeon "The Onboarding" (docs/DUNGEONS.md, content `raids`): its rooms and door, the daily
//! boss lockout with a locked boss that still spawns, the quest chain from the entrance clerk to
//! the exit hire, and the XP ladder (full, a quarter, nothing) with its gate under half a level.
//! The bot playthroughs (a trio on Normal, one player on Async) live in tests/coop.rs.

mod common;

use combat_core::{Command, Content, EntityId, Event, SaveData, Sim, Unlock, Vec3};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const DAY: u64 = 86_400;
const ROOMS: [&str; 3] = ["onboarding/1", "onboarding/2", "onboarding/3"];
const BOSSES: [&str; 3] = [
    "compliance_trainer",
    "laptop_provisioner",
    "onboarding_buddy",
];
/// Level-3 players: the dungeon's door level, and the level-3 XP bar of docs/FORMULAS.md.
const LEVEL: u8 = 3;
const BAR: u32 = 1500;

/// Packs far away, the boss 20 m north of the arrival, the room's NPCs and props beside it.
fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, -20.0),
        n if n.starts_with("npc_") => at(2.0, 2.0),
        n if n.starts_with("prop:") => at(1.0, 0.0),
        n if n.starts_with("rare_") || n.starts_with("event:") || n.starts_with("night_") => None,
        _ => at(0.0, 0.0),
    }
}

fn run(sim: &mut Sim, ticks: u32) -> Vec<Event> {
    (0..ticks).flat_map(|_| sim.tick()).collect()
}

/// `n` players of `level` on the campus, sturdy and dressed in nothing.
fn group(n: usize, level: u8, tweak: impl FnOnce(&mut Content)) -> (Sim, Vec<EntityId>) {
    let mut c = content();
    for s in &mut c.spells {
        s.yells.clear();
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, 5);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let classes = ["fullstack", "backend", "devops", "frontend"];
    let ids: Vec<EntityId> = (0..n)
        .map(|k| {
            let p = sim.spawn_player(classes[k % 4], Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, level);
            p
        })
        .collect();
    load(&mut sim, "campus");
    (sim, ids)
}

fn load(sim: &mut Sim, zone: &str) {
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{zone}: {e}");
    }
    run(sim, 1);
}

fn alive(sim: &Sim, key: &str) -> Vec<EntityId> {
    sim.units()
        .filter(|u| u.key == key && u.alive)
        .map(|u| u.id)
        .collect()
}

/// Kills `key` with the debug hook (it counts as a player kill) and returns the loot of the
/// first group member as (items, events).
fn kill(sim: &mut Sim, key: &str) -> Vec<Event> {
    let ids = alive(sim, key);
    assert!(!ids.is_empty(), "no living {key}");
    let mut log = Vec::new();
    for id in ids {
        let at = sim.unit(id).map_or(Vec3::new(0.0, 0.0, 0.0), |u| u.pos);
        assert!(sim.debug_kill(id));
        log.extend(run(sim, 1));
        // Everyone loots the corpse (quest items only count once picked up), then goes home.
        for p in sim.players().to_vec() {
            move_to(sim, p, at);
            sim.apply_as(
                p,
                Command::Loot {
                    looter: p,
                    corpse: id,
                },
            );
            log.extend(run(sim, 1));
            move_to(sim, p, Vec3::new(0.0, 0.0, 0.0));
        }
    }
    log
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

fn dropped(log: &[Event]) -> Vec<String> {
    log.iter()
        .flat_map(|e| match e {
            Event::LootDropped { items, .. } => items.clone(),
            _ => Vec::new(),
        })
        .collect()
}

fn xp_of(sim: &Sim, p: EntityId) -> u32 {
    sim.unit(p).map_or(0, |u| u.xp)
}

fn gear_of(boss: &str) -> Vec<String> {
    content()
        .economy
        .gear
        .iter()
        .filter(|g| matches!(&g.source, combat_core::GearSource::Boss(k) if k == boss))
        .map(|g| g.name.clone())
        .collect()
}

#[test]
fn the_onboarding_is_three_rooms_three_bosses_and_a_door_at_level_three() {
    let c = content();
    let r = c
        .raids
        .iter()
        .find(|r| r.name == "The Onboarding")
        .expect("dungeon");
    assert!(r.is_dungeon());
    assert_eq!(r.zones, ROOMS);
    assert_eq!(r.normal_min_players, 3);
    assert_eq!(r.bosses(&c), BOSSES);
    let mut trash = 0;
    for (i, z) in ROOMS.iter().enumerate() {
        let def = c.zone(z).expect("room");
        assert_eq!(def.level_range, (3, 4));
        assert_eq!(def.packs.len(), 2, "{z}: two packs per room");
        for p in &def.packs {
            assert!((2..=3).contains(&p.units.len()), "{z}: packs of 2-3");
            trash += p.units.len();
            for u in &p.units {
                assert_eq!(c.unit_def(u).map(|d| d.level), Some(3), "{u}");
            }
        }
        let boss = c.unit_def(BOSSES[i]).expect("boss");
        assert!(boss.boss && boss.level == 4);
        // Each room opens over the previous boss; the way back (and the last room's exit) is free.
        let forward = def
            .links
            .iter()
            .find(|l| ROOMS.get(i + 1) == Some(&l.to.as_str()));
        if i < 2 {
            assert!(
                matches!(forward.and_then(|l| l.requires.as_ref()), Some(Unlock::BossKilled(k)) if k == BOSSES[i]),
                "{z}"
            );
        } else {
            assert!(forward.is_none());
        }
        let back = if i == 0 { "campus" } else { ROOMS[i - 1] };
        assert!(
            def.links
                .iter()
                .any(|l| l.to == back && l.requires.is_none()),
            "{z}"
        );
        if i == 2 {
            assert!(
                def.links
                    .iter()
                    .any(|l| l.to == "campus" && l.requires.is_none()),
                "exit door"
            );
        }
    }
    assert_eq!(trash, 16, "five types, sixteen units");

    let door = |sim: &Sim| {
        sim.zone_links("campus")
            .into_iter()
            .find(|l| l.to == "onboarding/1")
            .is_some_and(|l| l.unlocked)
    };
    let (sim, _) = group(2, 2, |_| {});
    assert!(!door(&sim), "level 2 stays outside");
    let (mut sim, ids) = group(2, LEVEL, |_| {});
    assert!(door(&sim));
    sim.set_level_of(ids[1], 2);
    assert!(!door(&sim), "everyone needs the band's lower level");
    sim.set_level_of(ids[1], LEVEL);
    load(&mut sim, "onboarding/1");
    assert!(
        sim.raid_view().async_mode,
        "two players attend asynchronously"
    );
    // Async cuts the boss to 40% (a solo player pays no group scaling on top).
    let (mut solo, _) = group(1, LEVEL, |_| {});
    load(&mut solo, "onboarding/1");
    let boss = alive(&solo, "compliance_trainer")[0];
    assert_eq!(solo.unit(boss).map(|u| u.max_hp), Some(2400 * 40 / 100));
    let (mut trio, _) = group(3, LEVEL, |_| {});
    load(&mut trio, "onboarding/1");
    assert!(!trio.raid_view().async_mode, "three is a full programme");
}

#[test]
fn a_locked_boss_still_spawns_and_drops_its_quest_item_but_no_gear() {
    let (mut sim, ids) = group(3, LEVEL, |_| {});
    let p = ids[0];
    load(&mut sim, "onboarding/2");
    let gear = gear_of("laptop_provisioner");
    assert_eq!(gear.len(), 3);
    let first = dropped(&kill(&mut sim, "laptop_provisioner"));
    assert!(
        first.iter().any(|i| gear.contains(i)),
        "Normal drops its gear: {first:?}"
    );
    assert!(
        !first.iter().any(|i| i == "Laptop Box (Empty)"),
        "no ticket, no box: {first:?}"
    );
    assert!(sim.raid_locked("laptop_provisioner", false));
    // Take the ticket, then the same room the same day: the boss is there, fights, and drops only
    // the quest item.
    load(&mut sim, "onboarding/1");
    let clerk = alive(&sim, "welcome_desk_clerk")[0];
    sim.apply_as(p, Command::Talk { unit: clerk });
    sim.apply_as(
        p,
        Command::AcceptQuest {
            quest: "onb_laptop".into(),
        },
    );
    run(&mut sim, 1);
    load(&mut sim, "onboarding/2");
    assert_eq!(
        alive(&sim, "laptop_provisioner").len(),
        1,
        "a locked dungeon boss spawns"
    );
    let again = dropped(&kill(&mut sim, "laptop_provisioner"));
    assert!(
        !again.iter().any(|i| gear.contains(i)),
        "locked: no gear {again:?}"
    );
    assert!(
        again.iter().any(|i| i == "Laptop Box (Empty)"),
        "the quest item always drops {again:?}"
    );
    // The next UTC day it pays again.
    sim.set_clock(THURSDAY + DAY);
    assert!(!sim.raid_locked("laptop_provisioner", false));
    load(&mut sim, "onboarding/1");
    load(&mut sim, "onboarding/2");
    let next = dropped(&kill(&mut sim, "laptop_provisioner"));
    assert!(
        next.iter().any(|i| gear.contains(i)),
        "unlocked again: {next:?}"
    );
}

#[test]
fn the_dungeon_lock_is_daily_and_the_save_keeps_it() {
    let (mut sim, ids) = group(3, LEVEL, |_| {});
    load(&mut sim, "onboarding/1");
    kill(&mut sim, "compliance_trainer");
    assert!(sim.raid_locked("compliance_trainer", false));
    assert!(
        !sim.raid_locked("compliance_trainer", true),
        "Async is a lock of its own"
    );
    sim.set_clock(THURSDAY + DAY - 1);
    assert!(sim.raid_locked("compliance_trainer", false), "same UTC day");
    sim.set_clock(THURSDAY + DAY);
    assert!(
        !sim.raid_locked("compliance_trainer", false),
        "the next day index unlocks"
    );
    sim.set_clock(THURSDAY);
    load(&mut sim, "onboarding/3");
    kill(&mut sim, "onboarding_buddy");
    let save = sim.export_save(ids[0]).expect("save");
    let back = SaveData::decode(&save.encode()).expect("decode");
    assert!(
        back.raid_locks.is_empty(),
        "a dungeon never touches the weekly lock"
    );
    assert!(
        back.dungeon_locks
            .contains(&"compliance_trainer".to_string())
    );
    assert!(back.dungeon_locks.contains(&"onboarding_buddy".to_string()));
    assert!(
        back.dungeon_locks
            .iter()
            .any(|k| k.starts_with("clear:onboarding/1#")),
        "{:?}",
        back.dungeon_locks
    );
    let (mut fresh, q) = group(3, LEVEL, |_| {});
    assert!(fresh.import_save(q[0], &back));
    assert!(fresh.raid_locked("compliance_trainer", false));
}

/// The whole run through the debug kill: every pack and boss of the three rooms, `done` after
/// each room. Quests are accepted at the clerk and handed in to the exit hire by `players`.
fn clear_dungeon(sim: &mut Sim, players: &[EntityId], quests: bool) {
    for (i, room) in ROOMS.iter().enumerate() {
        load(sim, room);
        if quests && i == 0 {
            let clerk = alive(sim, "welcome_desk_clerk")[0];
            for &p in players {
                sim.apply_as(p, Command::Talk { unit: clerk });
                for q in ["onb_welcome", "onb_laptop", "onb_buddy"] {
                    sim.apply_as(p, Command::AcceptQuest { quest: q.into() });
                }
            }
            run(sim, 1);
        }
        if quests && i == 1 {
            for &p in players {
                sim.apply_as(
                    p,
                    Command::Interact {
                        prop: "onb_ticket_printer".into(),
                    },
                );
            }
        }
        let keys: Vec<String> = sim
            .units()
            .filter(|u| u.hostile && u.alive && !u.boss)
            .map(|u| u.key.clone())
            .collect();
        for k in keys {
            if !alive(sim, &k).is_empty() {
                kill(sim, &k);
            }
        }
        kill(sim, BOSSES[i]);
    }
    if quests {
        let hire = alive(sim, "tired_new_hire")[0];
        for &p in players {
            sim.apply_as(p, Command::Talk { unit: hire });
            run(sim, 1);
            for q in ["onb_welcome", "onb_laptop", "onb_buddy"] {
                sim.apply_as(
                    p,
                    Command::TurnInQuest {
                        quest: q.into(),
                        choice: 0,
                    },
                );
            }
        }
        run(sim, 1);
    }
}

#[test]
fn the_quest_chain_runs_from_the_clerk_to_the_exit_hire() {
    let (mut sim, ids) = group(1, LEVEL, |_| {});
    let p = ids[0];
    let c = content();
    for q in ["onb_welcome", "onb_laptop", "onb_buddy"] {
        let def = c.quest(q).expect("quest");
        assert_eq!(def.giver.npc, "welcome_desk_clerk");
        assert_eq!(def.giver.zone, "onboarding/1");
        let end = def.turn_in.as_ref().expect("turn-in at the exit");
        assert_eq!(
            (end.npc.as_str(), end.zone.as_str()),
            ("tired_new_hire", "onboarding/3")
        );
        assert!(def.requires.is_none(), "the set runs in parallel");
    }
    assert!(c.quest("onb_welcome").is_some_and(|q| q.main));
    clear_dungeon(&mut sim, &[p], true);
    let done = &sim.quests_of(p).completed;
    for q in ["onb_welcome", "onb_laptop", "onb_buddy"] {
        assert!(done.iter().any(|d| d == q), "{q} not done: {done:?}");
    }
    assert!(sim.quests_of(p).active.is_empty());
}

#[test]
fn dungeon_xp_stays_under_half_a_level() {
    // Solo, Async: the full first run with every quest.
    let (mut sim, ids) = group(1, LEVEL, |_| {});
    clear_dungeon(&mut sim, &ids, true);
    let gained = xp_of(&sim, ids[0]);
    let level = sim.unit(ids[0]).map_or(0, |u| u.level);
    println!(
        "solo first run: {gained} XP of {BAR} ({}%)",
        gained * 100 / BAR
    );
    assert_eq!(
        level, LEVEL,
        "a fresh level-3 player must not reach level 4"
    );
    assert!(gained > BAR / 4, "worth doing: {gained}");
    assert!(gained < BAR / 2, "under half a level: {gained}");
    // A four on Normal: kill XP is shared, quest XP is in full.
    let (mut four, ids) = group(4, LEVEL, |_| {});
    clear_dungeon(&mut four, &ids, true);
    for &p in &ids {
        let (xp, lvl) = (xp_of(&four, p), four.unit(p).map_or(0, |u| u.level));
        println!("four, one member: {xp} XP of {BAR} ({}%)", xp * 100 / BAR);
        assert_eq!(lvl, LEVEL);
        assert!(xp < BAR / 2, "under half a level: {xp}");
    }
}

#[test]
fn repeat_clears_the_same_day_pay_a_quarter_then_nothing() {
    let (mut sim, ids) = group(1, LEVEL, |_| {});
    let p = ids[0];
    let trainer_xp = |sim: &mut Sim| {
        load(sim, "onboarding/1");
        let before = xp_of(sim, p);
        kill(sim, "compliance_trainer");
        xp_of(sim, p) - before
    };
    let finale = |sim: &mut Sim| {
        load(sim, "onboarding/3");
        kill(sim, "onboarding_buddy");
    };
    let first = trainer_xp(&mut sim);
    assert!(first > 40, "{first}");
    finale(&mut sim);
    let second = trainer_xp(&mut sim);
    assert_eq!(
        second,
        first * 25 / 100,
        "the second clear of the day pays 25% ({first} -> {second})"
    );
    finale(&mut sim);
    assert_eq!(trainer_xp(&mut sim), 0, "the third and later pay nothing");
    // The next UTC day starts over.
    sim.set_clock(THURSDAY + DAY);
    assert_eq!(trainer_xp(&mut sim), first);
}

#[test]
fn the_box_the_laptop_shipped_in_goes_off_with_a_telegraph_when_the_provisioner_dies() {
    let (mut sim, _) = group(3, LEVEL, |_| {});
    load(&mut sim, "onboarding/2");
    let mut log = kill(&mut sim, "laptop_provisioner");
    log.extend(run(&mut sim, 90));
    let started = log
        .iter()
        .any(|e| matches!(e, Event::TelegraphStarted { .. }));
    let landed = log
        .iter()
        .any(|e| matches!(e, Event::TelegraphResolved { .. }));
    assert!(
        started && landed,
        "ring telegraph on death: {started} {landed}"
    );
}
