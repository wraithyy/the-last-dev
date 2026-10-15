//! The "First Day" onboarding corner: a healer can heal the burnt-out colleague (and only NPCs
//! authored as `healable`), and the training dummy never starts a fight on its own.

use combat_core::{Command, Content, EntityId, Event, Sim, SpellId, Vec3};

const SRC: &str = include_str!("../../../content/game.ron");
/// DevOps "Hotfix": an `Ally` heal known at level 1.
const HOTFIX: SpellId = SpellId(41);

fn sim() -> Sim {
    match Content::from_ron(SRC) {
        Ok(c) => Sim::new(c, 7),
        Err(e) => panic!("shipped content invalid: {e}"),
    }
}

fn spawn(sim: &mut Sim, key: &str, z: f32) -> EntityId {
    match sim.spawn_unit(key, Vec3::new(0.0, 0.0, z), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("spawn {key}: {e}"),
    }
}

fn heal_lands_on(sim: &mut Sim, healer: EntityId, target: EntityId) -> bool {
    sim.push(Command::SetTarget {
        unit: healer,
        target: Some(target),
    });
    sim.push(Command::Cast {
        caster: healer,
        spell: HOTFIX,
    });
    (0..120).flat_map(|_| sim.tick()).any(
        |e| matches!(e, Event::Heal { source, target: t, .. } if source == healer && t == target),
    )
}

#[test]
fn healers_can_heal_the_colleague_but_not_other_npcs() {
    let mut sim = sim();
    let p = sim.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let colleague = spawn(&mut sim, "burnt_out_colleague", -5.0);
    let intern = spawn(&mut sim, "intern", 5.0);
    sim.debug_set_hp(colleague, 60);
    sim.debug_set_hp(intern, 10);
    assert!(heal_lands_on(&mut sim, p, colleague));
    let hp = sim.unit(colleague).map_or(0, |u| u.hp);
    assert!(hp > 60, "colleague healed to {hp}");
    // Anyone else friendly stays a self-cast, as before.
    assert!(!heal_lands_on(&mut sim, p, intern));
}

#[test]
fn the_training_dummy_waits_to_be_attacked() {
    let mut sim = sim();
    let p = sim.spawn_player("frontend", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let dummy = spawn(&mut sim, "training_dummy", -2.0);
    let events: Vec<Event> = (0..300).flat_map(|_| sim.tick()).collect();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::EnterCombat { unit } if *unit == dummy)),
        "the dummy pulled on its own"
    );
    sim.push(Command::SetTarget {
        unit: p,
        target: Some(dummy),
    });
    sim.push(Command::StartAutoAttack { unit: p });
    let events: Vec<Event> = (0..600).flat_map(|_| sim.tick()).collect();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::CastStarted { caster, spell, .. } if *caster == dummy && *spell == SpellId(1390))),
        "attacked, it plays the training video"
    );
}
