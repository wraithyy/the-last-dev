//! The interactive office: rides, sprint, props, hidden NPCs and achievements (`world.rs`).

mod common;

use combat_core::{
    AuraEnd, CastError, Command, Content, EntityId, Event, ItemError, SaveData, Sim, SpellId,
    UnitState, Vec3,
};
use common::content;

const SCOOTER: SpellId = SpellId(950);
const SPRINT: SpellId = SpellId(951);

/// Packs far away so nothing pulls; props and hidden NPCs where the test wants them.
fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        // The onboarding dummy is hostile too (it waits to be hit, but not on top of you).
        n if n.starts_with("pack_") || n == "boss" || n == "npc_dummy" => at(400.0, 400.0),
        "prop:coffee_campus" | "prop:printer_pmo" | "prop:vending_open_space" => at(1.0, 0.0),
        "prop:bike_rack" => at(-20.0, 0.0),
        "prop:scooter_1" => at(0.0, 2.0),
        n if n.starts_with("prop:") || n.starts_with("secret:") => at(30.0, 30.0),
        _ => at(0.0, 0.0),
    }
}

fn world(seed: u64, zone: &str, tweak: impl FnOnce(&mut Content)) -> (Sim, EntityId) {
    let mut c = content();
    for s in &mut c.spells {
        s.yells.clear();
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, seed);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    load(&mut sim, zone);
    (sim, p)
}

fn load(sim: &mut Sim, zone: &str) {
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{e}");
    }
}

fn me(sim: &Sim, p: EntityId) -> &UnitState {
    match sim.unit(p) {
        Some(u) => u,
        None => panic!("no player"),
    }
}

fn run(sim: &mut Sim, ticks: u32) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        out.extend(sim.tick());
    }
    out
}

fn has_aura(sim: &Sim, p: EntityId, key: &str) -> bool {
    me(sim, p).auras.iter().any(|a| a.key == key)
}

fn interact(sim: &mut Sim, p: EntityId, prop: &str) -> Vec<Event> {
    sim.apply_as(p, Command::Interact { prop: prop.into() });
    run(sim, 1)
}

fn outcome(ev: &[Event]) -> Option<u32> {
    ev.iter().find_map(|e| match e {
        Event::Interacted { outcome, .. } => Some(*outcome),
        _ => None,
    })
}

#[test]
fn scooter_mounts_after_its_cast_and_speeds_up_out_of_combat() {
    let (mut sim, p) = world(1, "campus", |_| {});
    assert_eq!(sim.move_speed_pct(p), 100);
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SCOOTER,
        },
    );
    run(&mut sim, 30);
    assert!(!has_aura(&sim, p, "e_scooter"), "1.5 s cast, not instant");
    let ev = run(&mut sim, 20);
    assert!(has_aura(&sim, p, "e_scooter"));
    assert_eq!(sim.move_speed_pct(p), 160);
    assert!(
        me(&sim, p)
            .achievements
            .contains(&"scooter_owner".to_string())
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Achievement { key, .. } if key == "scooter_owner"))
    );
    // The mount key again gets off.
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SCOOTER,
        },
    );
    run(&mut sim, 1);
    assert!(!has_aura(&sim, p, "e_scooter"));
    assert_eq!(sim.move_speed_pct(p), 100);
}

#[test]
fn any_other_cast_and_combat_end_the_ride() {
    let (mut sim, p) = world(2, "campus", |_| {});
    assert!(sim.debug_apply_aura(p, "e_scooter", 60_000));
    run(&mut sim, 1);
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SPRINT,
        },
    );
    let ev = run(&mut sim, 1);
    assert!(!has_aura(&sim, p, "e_scooter"));
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::AuraRemoved { key, reason: AuraEnd::Consumed, .. } if key == "e_scooter"
    )));
    // Combat: a robot walks up and starts hitting.
    assert!(sim.debug_apply_aura(p, "e_scooter", 60_000));
    run(&mut sim, 1);
    let robot = match sim.spawn_unit("delivery_robot", Vec3::new(0.0, 0.0, -2.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    run(&mut sim, 5);
    assert!(me(&sim, p).in_combat);
    assert!(!has_aura(&sim, p, "e_scooter"));
    // Mounting mid-fight is refused.
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SCOOTER,
        },
    );
    let ev = run(&mut sim, 1);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::CastFailed {
            reason: CastError::InCombat,
            ..
        }
    )));
    let _ = robot;
}

#[test]
fn damage_ends_the_ride() {
    let (mut sim, p) = world(3, "campus", |_| {});
    assert!(sim.debug_apply_aura(p, "e_scooter", 60_000));
    // A damage-over-time tick with no enemy around: damage alone, no combat.
    assert!(sim.debug_apply_aura(p, "thought_leadership", 10_000));
    let ev = run(&mut sim, 95);
    assert!(
        ev.iter().any(
            |e| matches!(e, Event::Damage { target, amount, .. } if *target == p && *amount > 0)
        )
    );
    assert!(!me(&sim, p).in_combat);
    assert!(!has_aura(&sim, p, "e_scooter"));
}

#[test]
fn facilities_forbids_scooters_in_the_tower() {
    let (mut sim, p) = world(4, "campus", |_| {});
    assert!(sim.debug_apply_aura(p, "e_scooter", 60_000));
    run(&mut sim, 1);
    load(&mut sim, "tower/1");
    run(&mut sim, 1);
    assert!(
        !has_aura(&sim, p, "e_scooter"),
        "riding into the office ends the ride"
    );
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SCOOTER,
        },
    );
    let ev = run(&mut sim, 1);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::CastFailed {
            reason: CastError::NoScooters,
            ..
        }
    )));
    assert_eq!(
        CastError::NoScooters.message(),
        "No scooters in the office. (Facilities)"
    );
    // The data center halls allow it.
    load(&mut sim, "datacenter/1");
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SCOOTER,
        },
    );
    run(&mut sim, 50);
    assert!(has_aura(&sim, p, "e_scooter"));
}

#[test]
fn rush_to_standup_is_a_short_sprint_on_a_cooldown() {
    let (mut sim, p) = world(5, "tower/1", |_| {});
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SPRINT,
        },
    );
    run(&mut sim, 1);
    assert_eq!(sim.move_speed_pct(p), 170);
    let cd = sim.cooldown(p, SPRINT);
    assert_eq!(cd.cd_total_ms, 30_000);
    assert_eq!(cd.gcd_remaining_ms, 0, "off the GCD");
    run(&mut sim, 4 * 30);
    assert_eq!(sim.move_speed_pct(p), 100);
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: SPRINT,
        },
    );
    let ev = run(&mut sim, 1);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::CastFailed {
            reason: CastError::OnCooldown,
            ..
        }
    )));
    // Every class knows both, from level 1.
    for class in ["frontend", "backend", "devops"] {
        let mut s2 = Sim::new(content(), 1);
        let q = s2.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
        assert!(s2.cooldown(q, SPRINT).usable.is_ok(), "{class}");
    }
}

#[test]
fn coffee_rolls_are_deterministic_and_sometimes_fail() {
    let roll = |seed: u64| {
        let (mut sim, p) = world(seed, "campus", |_| {});
        let mut outs = Vec::new();
        for _ in 0..40 {
            outs.push(outcome(&interact(&mut sim, p, "coffee_campus")));
            run(&mut sim, 95);
        }
        (outs, sim, p)
    };
    let (a, sim, p) = roll(11);
    let (b, _, _) = roll(11);
    assert_eq!(a, b, "same seed, same brews");
    assert!(
        a.iter().all(Option::is_some),
        "3 s cooldown respected: {a:?}"
    );
    assert!(a.contains(&Some(0)), "10% descaling: {a:?}");
    assert!(a.contains(&Some(1)));
    assert!(has_aura(&sim, p, "fresh_brew"));
    assert!(me(&sim, p).achievements.contains(&"descaling".to_string()));
}

#[test]
fn coffee_has_a_per_player_cooldown_and_a_reach() {
    let (mut sim, p) = world(6, "campus", |_| {});
    assert!(outcome(&interact(&mut sim, p, "coffee_campus")).is_some());
    assert!(outcome(&interact(&mut sim, p, "coffee_campus")).is_none());
    // Too far: an error, no roll.
    let (mut sim, p) = world(6, "campus", |_| {});
    sim.push(Command::PlayerMoved {
        pos: Vec3::new(20.0, 0.0, 0.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    let ev = interact(&mut sim, p, "coffee_campus");
    assert!(outcome(&ev).is_none());
    assert!(ev.contains(&Event::ItemError {
        reason: ItemError::TooFar
    }));
    // A prop of another zone does nothing.
    assert!(outcome(&interact(&mut sim, p, "printer_pmo")).is_none());
}

#[test]
fn vending_costs_story_points_and_pays_out_from_its_list() {
    let (mut sim, p) = world(7, "tower/1", |_| {});
    let ev = interact(&mut sim, p, "vending_open_space");
    assert!(ev.contains(&Event::ItemError {
        reason: ItemError::NotEnoughSp
    }));
    sim.grant_sp(p, 200);
    let mut seen = Vec::new();
    for _ in 0..30 {
        let o = outcome(&interact(&mut sim, p, "vending_open_space"));
        seen.push(o.unwrap_or(99));
        run(&mut sim, 61);
    }
    assert_eq!(me(&sim, p).sp, 200 - 30 * 5);
    assert!(seen.iter().all(|o| *o <= 5), "{seen:?}");
    let items = seen.iter().filter(|o| **o >= 4).count();
    let bag = me(&sim, p).bag.len();
    assert_eq!(bag, items, "outcomes 4 and 5 are bag items");
    assert!(seen.contains(&0));
    assert!(me(&sim, p).achievements.contains(&"sunk_cost".to_string()));
}

#[test]
fn ten_printer_clicks_in_a_row_earn_the_secret_and_the_trophy() {
    let (mut sim, p) = world(8, "tower/4", |_| {});
    for _ in 0..9 {
        interact(&mut sim, p, "printer_pmo");
        run(&mut sim, 20);
    }
    // A long pause breaks the streak.
    run(&mut sim, 11 * 30);
    for _ in 0..9 {
        interact(&mut sim, p, "printer_pmo");
        run(&mut sim, 20);
    }
    assert!(
        !me(&sim, p)
            .achievements
            .contains(&"pc_load_letter".to_string())
    );
    let ev = interact(&mut sim, p, "printer_pmo");
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Achievement { key, .. } if key == "pc_load_letter"))
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::TitleChanged { title, .. } if title == "pc_load_letter"))
    );
    let u = me(&sim, p);
    assert_eq!(u.title, "pc_load_letter");
    assert!(
        u.inventory
            .iter()
            .any(|(n, _)| n == "PC LOAD LETTER Survivor")
    );
}

#[test]
fn fire_alarm_pulls_the_whole_floor() {
    let (mut sim, p) = world(9, "tower/2", |_| {});
    // Stand at the alarm.
    sim.push(Command::PlayerMoved {
        pos: Vec3::new(30.0, 0.0, 30.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    sim.tick();
    let hostiles = sim.units().filter(|u| u.hostile && u.alive).count() as u32;
    assert!(hostiles > 3);
    let ev = interact(&mut sim, p, "fire_alarm");
    assert_eq!(outcome(&ev), Some(hostiles));
    run(&mut sim, 2);
    assert!(
        sim.units()
            .filter(|u| u.hostile && u.alive)
            .all(|u| u.in_combat)
    );
    assert!(me(&sim, p).achievements.contains(&"fire_drill".to_string()));
}

#[test]
fn borrowing_a_parked_scooter_mounts_at_once_and_hides_it_for_everyone() {
    let mut c = content();
    for s in &mut c.spells {
        s.yells.clear();
    }
    let mut sim = Sim::new(c, 10);
    sim.set_wandering(false);
    let a = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let b = sim.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    load(&mut sim, "campus");
    for (p, x) in [(a, 0.0), (b, 0.5)] {
        sim.apply_as(
            p,
            Command::PlayerMoved {
                pos: Vec3::new(x, 0.0, 1.0),
                yaw: 0.0,
                moving: false,
                jumping: false,
            },
        );
    }
    sim.tick();
    let ev = interact(&mut sim, a, "scooter_1");
    assert_eq!(outcome(&ev), Some(1));
    assert!(has_aura(&sim, a, "e_scooter"));
    sim.apply_as(
        b,
        Command::Interact {
            prop: "scooter_1".into(),
        },
    );
    assert!(outcome(&run(&mut sim, 1)).is_none(), "already gone");
    run(&mut sim, 60 * 30);
    sim.apply_as(
        b,
        Command::Interact {
            prop: "scooter_1".into(),
        },
    );
    assert_eq!(outcome(&run(&mut sim, 1)), Some(1), "back after a minute");
    assert!(has_aura(&sim, b, "e_scooter"));
}

#[test]
fn standing_by_the_lonely_bike_for_a_minute_counts() {
    let (mut sim, p) = world(12, "campus", |_| {});
    sim.push(Command::PlayerMoved {
        pos: Vec3::new(-20.0, 0.0, 1.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    run(&mut sim, 59 * 30);
    assert!(
        !me(&sim, p)
            .achievements
            .contains(&"last_cyclist".to_string())
    );
    run(&mut sim, 31);
    assert!(
        me(&sim, p)
            .achievements
            .contains(&"last_cyclist".to_string())
    );
}

#[test]
fn hidden_npcs_pay_once_and_titles_can_be_changed() {
    let (mut sim, p) = world(13, "campus", |_| {});
    let intern = sim
        .units()
        .find(|u| u.key == "docs_intern")
        .map(|u| u.id)
        .unwrap_or(0);
    assert!(intern > 0, "the hidden intern spawns at its anchor");
    sim.push(Command::PlayerMoved {
        pos: Vec3::new(30.0, 0.0, 31.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    sim.tick();
    for _ in 0..2 {
        sim.apply_as(p, Command::Talk { unit: intern });
        run(&mut sim, 1);
    }
    let u = me(&sim, p);
    assert_eq!(u.sp, 15, "the reward is paid once");
    assert_eq!(u.title, "rtfm");
    sim.apply_as(
        p,
        Command::SetTitle {
            achievement: "pc_load_letter".into(),
        },
    );
    run(&mut sim, 1);
    assert_eq!(me(&sim, p).title, "rtfm", "only unlocked titles");
    sim.apply_as(
        p,
        Command::SetTitle {
            achievement: String::new(),
        },
    );
    let ev = run(&mut sim, 1);
    assert_eq!(me(&sim, p).title, "");
    assert!(ev.contains(&Event::TitleChanged {
        unit: p,
        title: String::new()
    }));
}

#[test]
fn achievements_and_title_survive_a_save_roundtrip() {
    let (mut sim, p) = world(14, "campus", |_| {});
    sim.apply_as(
        p,
        Command::Interact {
            prop: "days_sign_campus".into(),
        },
    );
    sim.push(Command::PlayerMoved {
        pos: Vec3::new(30.0, 0.0, 30.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    run(&mut sim, 2);
    sim.apply_as(
        p,
        Command::Interact {
            prop: "days_sign_campus".into(),
        },
    );
    run(&mut sim, 1);
    assert_eq!(me(&sim, p).achievements, vec!["zero_days".to_string()]);
    let Some(save) = sim.export_save(p) else {
        panic!("no save")
    };
    let text = save.encode();
    let Some(back) = SaveData::decode(&(text + "achievements=zero_days|no_such_thing\n")) else {
        panic!("decode failed")
    };
    let mut fresh = Sim::new(content(), 1);
    let q = fresh.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(fresh.import_save(q, &back));
    let u = me(&fresh, q);
    assert_eq!(
        u.achievements,
        vec!["zero_days".to_string()],
        "unknown keys dropped"
    );
    assert_eq!(u.title, "zero_days");
}

#[test]
fn eight_secrets_survive_the_reorg() {
    let (mut sim, p) = world(15, "tower/4", |_| {});
    for _ in 0..10 {
        interact(&mut sim, p, "printer_pmo");
        run(&mut sim, 20);
    }
    assert!(
        !me(&sim, p)
            .achievements
            .contains(&"survived_reorg".to_string())
    );
    let n = me(&sim, p).achievements.len();
    assert!(n < 8);
    // Pad up to seven more through a save, then one more unlock tips it over.
    assert_eq!(n, 1);
    let keys = [
        "rtfm",
        "mandatory_fun",
        "manually_tested",
        "descaling",
        "deprecated",
        "bus_factor",
    ];
    let mut save = sim.export_save(p).unwrap_or_default();
    save.achievements.extend(keys.iter().map(|k| k.to_string()));
    assert!(sim.import_save(p, &save));
    assert_eq!(me(&sim, p).achievements.len(), 7);
    sim.push(Command::PlayerMoved {
        pos: Vec3::new(30.0, 0.0, 30.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    run(&mut sim, 1);
    let ev = interact(&mut sim, p, "days_sign_pmo");
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Achievement { key, .. } if key == "survived_reorg"))
    );
    assert!(me(&sim, p).sp >= 50);
}
