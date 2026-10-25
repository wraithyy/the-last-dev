//! A fresh level-1 player must not face a wall of "!" on the campus, and gating must never
//! strand a quest.

mod common;

use combat_core::{QuestStatus, Sim, Vec3};
use common::content;
use std::collections::BTreeSet;

fn fresh() -> Sim {
    let mut sim = Sim::new(content(), 5);
    sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_player_level(1);
    sim
}

#[test]
fn level_one_campus_offers_stay_small() {
    let mut sim = fresh();
    // The P3 intern's walk is the tutorial; the offers that matter are the ones after it.
    assert!(sim.debug_set_quest("onboarding", QuestStatus::Done));
    let offers: Vec<&str> = sim
        .content()
        .quests
        .iter()
        .filter(|q| q.giver.zone == "campus" && sim.quest_status(&q.id) == QuestStatus::Available)
        .map(|q| q.id.as_str())
        .collect();
    assert!(
        offers.len() <= 6,
        "{} level-1 campus offers: {offers:?}",
        offers.len()
    );
}

/// Every quest can be reached by some level and chain of completed prerequisites.
#[test]
fn every_quest_is_reachable() {
    let c = content();
    let max = c.max_level();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    loop {
        let before = done.len();
        for q in &c.quests {
            if q.level_min <= max && q.requires.as_deref().is_none_or(|r| done.contains(r)) {
                done.insert(&q.id);
            }
        }
        if done.len() == before {
            break;
        }
    }
    let stuck: Vec<&str> = c
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .filter(|i| !done.contains(i) && *i != combat_core::rewards::ENDING_QUEST)
        .collect();
    assert!(stuck.is_empty(), "unreachable quests: {stuck:?}");
}
