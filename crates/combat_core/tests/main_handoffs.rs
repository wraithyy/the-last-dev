//! Main quests chain across zones; the player must never be left without a pointer to the next
//! step. A successor is fine when its giver stands where the previous quest ended (a breadcrumb's
//! turn-in), or when it is handed over unasked with a first objective that leads to the giver's
//! zone, and in both cases the way there is open with only what the player has done by then.

mod common;

use combat_core::{Content, Objective, Unlock};
use common::content;
use std::collections::BTreeSet;

/// Whether `r` holds for a player who finished the quests in `done`. World progress (zone
/// clears, bosses, levels) is ordinary play and counts as met; quest gates are the hidden kind.
fn met(r: &Unlock, done: &BTreeSet<&str>) -> bool {
    match r {
        Unlock::QuestDone(q) | Unlock::QuestStarted(q) => done.contains(q.as_str()),
        Unlock::AnyOf(any) => any.iter().any(|u| met(u, done)),
        _ => true,
    }
}

fn reachable(c: &Content, from: &str, to: &str, done: &BTreeSet<&str>) -> bool {
    let mut seen = vec![from];
    let mut i = 0;
    while i < seen.len() {
        let z = seen[i];
        i += 1;
        if z == to {
            return true;
        }
        for l in c.zone(z).into_iter().flat_map(|z| &z.links) {
            if l.requires.as_ref().is_none_or(|r| met(r, done)) && !seen.contains(&l.to.as_str()) {
                seen.push(&l.to);
            }
        }
    }
    false
}

#[test]
fn every_main_successor_is_in_reach_and_pointed_at() {
    let c = content();
    let mut checked = 0;
    for s in c.quests.iter().filter(|q| q.main) {
        let Some(prev) = s.requires.as_deref().and_then(|r| c.quest(r)) else {
            continue;
        };
        if !prev.main {
            continue;
        }
        checked += 1;
        // Everything up the chain is done by the time the successor is offered.
        let mut done = BTreeSet::new();
        let mut at = Some(prev);
        while let Some(q) = at {
            done.insert(q.id.as_str());
            at = q.requires.as_deref().and_then(|r| c.quest(r));
        }
        let (from, to) = (prev.ender().zone.as_str(), s.giver.zone.as_str());
        if from != to {
            let pointed = matches!(
                s.objectives.first(),
                Some(Objective::Wait { zone, .. }) if zone == to
            );
            assert!(
                pointed,
                "{} ends in {from} but {} is given in {to} and nothing leads there",
                prev.id, s.id
            );
            assert!(
                reachable(&c, from, to, &done),
                "{}: no open route {from} -> {to} once {} is done",
                s.id,
                prev.id
            );
        }
    }
    assert!(checked > 30, "only {checked} main successors found");
}
