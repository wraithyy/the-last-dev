//! The action recorder (docs/ILMARI_10.md C.3.3) against the real content: kits, companions and
//! determinism. The formula, the ring and the replay timing are unit-tested in `echo.rs`.
#![allow(clippy::expect_used)]

mod common;

use combat_core::companion::{DALE, RUPERT};
use combat_core::echo::{EchoCfg, EchoKind};
use combat_core::{Command, EntityId, Event, Sim, SpellId, Vec3};
use common::*;

#[test]
fn every_class_has_at_least_four_distinct_recordable_spells() {
    let c = content();
    for class in &c.classes {
        let kinds: std::collections::BTreeSet<_> = class
            .action_bar
            .iter()
            .filter_map(|s| c.spell(*s))
            .filter(|s| EchoKind::of(s).is_some())
            .map(|s| s.id)
            .collect();
        assert!(kinds.len() >= 4, "{}: {kinds:?}", class.key);
    }
}

/// A Story room with two companions at Wing 1's boss: the recorder is armed with that boss and
/// the owner opens on it, so the companions fight too.
fn story_fight(seed: u64) -> (Sim, EntityId, Vec<EntityId>, Vec<(u64, Event)>) {
    let mut c = content();
    def_mut(&mut c, "senior_engineer").max_hp = 10_000_000;
    let mut sim = Sim::new(c, seed);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(p, 10);
    sim.debug_force_raid_async(Some(true));
    sim.set_companion_picks(p, &[DALE.to_string(), RUPERT.to_string()]);
    let anchors = |n: &str| {
        Some(match n {
            "boss" => (Vec3::new(0.0, 0.0, -20.0), 0.0),
            n if n.starts_with("pack_") => (Vec3::new(400.0, 0.0, 400.0), 0.0),
            _ => (Vec3::new(0.0, 0.0, 0.0), 0.0),
        })
    };
    sim.load_zone("ilmari1/1", &anchors, Vec::new())
        .expect("zone");
    let boss = sim
        .units()
        .find(|u| u.key == "il_patch_tuesday")
        .expect("boss")
        .id;
    sim.echo_start(EchoCfg {
        boss,
        center_x_cm: 0,
        center_z_cm: -1000,
        story: true,
        add_unit: None,
    });
    let comps = sim.companions_of(p);
    let mut log = Vec::new();
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos: Vec3::new(0.0, 0.0, -8.0),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    sim.apply_as(
        p,
        Command::SetTarget {
            unit: p,
            target: Some(boss),
        },
    );
    for t in 0..60 * 30u64 {
        if t % 90 == 0 {
            sim.apply_as(
                p,
                Command::Cast {
                    caster: p,
                    spell: SpellId(1),
                },
            );
        }
        for e in sim.tick() {
            log.push((sim.now().0, e));
        }
    }
    (sim, p, comps, log)
}

#[test]
fn companions_never_feed_the_log() {
    let (sim, p, comps, log) = story_fight(4);
    let companion_casts = log
        .iter()
        .filter(|(_, e)| matches!(e, Event::CastSucceeded { caster, .. } if comps.contains(caster)))
        .count();
    assert!(
        companion_casts > 3,
        "the companions fought: {companion_casts} casts"
    );
    let player_casts = log
        .iter()
        .filter(|(_, e)| matches!(e, Event::CastSucceeded { caster, .. } if *caster == p))
        .count();
    assert!(player_casts > 3);
    assert_eq!(
        sim.echo_log().len(),
        player_casts,
        "only the player was recorded"
    );
    assert!(sim.echo_log().iter().all(|e| e.player == p));
}

#[test]
fn a_recorded_run_is_bit_identical() {
    let (a, _, _, la) = story_fight(8);
    let (b, _, _, lb) = story_fight(8);
    assert_eq!(sim_hash(&a, &la), sim_hash(&b, &lb));
    let echoes = |l: &[(u64, Event)]| {
        l.iter()
            .filter(|(_, e)| matches!(e, Event::Echoed { .. }))
            .count()
    };
    // Story replays after 45 s: the ghosts of the first casts have shown up by the end.
    assert!(echoes(&la) > 0);
    assert_eq!(echoes(&la), echoes(&lb));
}
