//! The Innovation Lab VR demo: one use puts the mirrored-controls headset on for 15 s, with a
//! cooldown, a little burnout relief and an achievement; a fight blocks it.

mod common;

use combat_core::{Command, EntityId, Event, ItemError, Sim, Vec3};
use common::content;

const SECOND: u32 = 30;

fn world() -> (Sim, EntityId) {
    let mut sim = Sim::new(content(), 7);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let anchors = |name: &str| {
        let far = name.starts_with("pack_") || name == "boss";
        let at = if far { 400.0 } else { 1.0 };
        Some((Vec3::new(at, 0.0, if far { 400.0 } else { 0.0 }), 0.0))
    };
    if let Err(e) = sim.load_zone("lab/1", &anchors, Vec::new()) {
        panic!("{e}");
    }
    (sim, p)
}

fn run(sim: &mut Sim, ticks: u32) -> Vec<Event> {
    (0..ticks).flat_map(|_| sim.tick()).collect()
}

fn use_demo(sim: &mut Sim, p: EntityId) -> Vec<Event> {
    sim.apply_as(
        p,
        Command::Interact {
            prop: "vr_demo".into(),
        },
    );
    run(sim, 1)
}

fn wearing(sim: &Sim, p: EntityId) -> bool {
    sim.unit(p)
        .is_some_and(|u| u.auras.iter().any(|a| a.key == "vr_headset"))
}

#[test]
fn the_headset_goes_on_for_fifteen_seconds_then_waits_out_its_cooldown() {
    let (mut sim, p) = world();
    let ev = use_demo(&mut sim, p);
    assert!(ev.iter().any(|e| matches!(e, Event::Interacted { .. })));
    assert!(wearing(&sim, p));
    assert!(sim.content().reverses_controls(["vr_headset"].into_iter()));
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Achievement { key, .. } if key == "early_adopter"))
    );

    run(&mut sim, 14 * SECOND);
    assert!(wearing(&sim, p), "still on after 14 s");
    run(&mut sim, 2 * SECOND);
    assert!(!wearing(&sim, p), "off after 15 s");

    // Inside the 45 s cooldown a second use does nothing.
    let ev = use_demo(&mut sim, p);
    assert!(!ev.iter().any(|e| matches!(e, Event::Interacted { .. })));
    assert!(!wearing(&sim, p));
    run(&mut sim, 45 * SECOND);
    let ev = use_demo(&mut sim, p);
    assert!(ev.iter().any(|e| matches!(e, Event::Interacted { .. })));
    assert!(wearing(&sim, p));
}

#[test]
fn a_fight_blocks_the_demo() {
    let (mut sim, p) = world();
    assert!(
        sim.spawn_unit("meatproxy", Vec3::new(0.0, 0.0, -2.0), 0.0)
            .is_ok()
    );
    run(&mut sim, 3 * SECOND);
    assert!(sim.unit(p).is_some_and(|u| u.in_combat), "setup: in combat");
    let ev = use_demo(&mut sim, p);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::ItemError {
            reason: ItemError::InCombat
        }
    )));
    assert!(!wearing(&sim, p));
}
