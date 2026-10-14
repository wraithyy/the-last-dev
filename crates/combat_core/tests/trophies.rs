//! Trophies: looting stores items and applies permanent player bonuses.

mod common;

use combat_core::{Command, Content, ITEM_STACK_CAP, LootEntry, Vec3};
use common::*;

const DECK: &str = "Unread AI Strategy Deck";

/// The CEO dies to one Code Review and always drops `items`; it also one-shots the player
/// if it ever reaches melee, which the second CEO in `die` relies on.
fn h_with_loot(items: &[&str]) -> H {
    let items: Vec<String> = items.iter().map(|s| s.to_string()).collect();
    H::new(1, -20.0, move |c: &mut Content| {
        def_mut(c, "senior_engineer").spell_power = 100_000;
        let ceo = def_mut(c, "ceo");
        ceo.loot = items
            .iter()
            .map(|item| LootEntry {
                item: item.clone(),
                chance_pct: 100,
            })
            .collect();
        if let Some(ai) = ceo.ai.as_mut() {
            ai.abilities.clear();
            ai.melee_damage = 1000;
        }
    })
}

fn kill_and_loot(h: &mut H) {
    h.target(h.mob);
    h.cast(CR);
    h.run(61);
    h.sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, -17.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    h.sim.push(Command::Loot {
        looter: h.player,
        corpse: h.mob,
    });
    h.tick();
}

#[test]
fn loot_adds_trophies_and_applies_bonuses_at_once() {
    let mut h = h_with_loot(&[DECK, "Prompt Fragment", "Recurring Meeting Invite"]);
    kill_and_loot(&mut h);
    let p = h.unit(h.player);
    assert_eq!(p.inventory.len(), 3);
    assert_eq!(p.inventory[0], (DECK.to_string(), 1));
    // Deck +8 HP; current HP scales with the maximum and never drops.
    assert_eq!((p.max_hp, p.hp), (308, 308));
    let t = h.sim.item_totals();
    assert_eq!((t.max_hp, t.spell_power, t.haste_pct), (8, 1, 1));
    assert_eq!(h.sim.inventory().len(), 3);
}

#[test]
fn fractional_crit_and_mana_bonuses_sum_exactly() {
    let mut h = h_with_loot(&[
        "Logo, Slightly Rounder",
        "Tests (Commented Out)",
        "Accepted Suggestion",
    ]);
    kill_and_loot(&mut h);
    let t = h.sim.item_totals();
    assert_eq!((t.crit_tenths, t.max_mana), (10, 4));
    assert_eq!(h.unit(h.player).max_mana, 254);
}

#[test]
fn copies_stack_only_up_to_the_cap() {
    let mut h = h_with_loot(&[DECK, DECK, DECK, DECK, DECK]);
    kill_and_loot(&mut h);
    assert_eq!(h.sim.inventory(), [(DECK.to_string(), 5)]);
    assert_eq!(h.unit(h.player).max_hp, 300 + 8 * ITEM_STACK_CAP);
    assert_eq!(h.sim.item_totals().max_hp, 8 * ITEM_STACK_CAP);
}

#[test]
fn bonuses_survive_floor_loads_respawn_and_level_ups_but_not_a_new_sim() {
    let mut h = h_with_loot(&[DECK]);
    kill_and_loot(&mut h);
    assert_eq!(h.unit(h.player).max_hp, 308);

    assert!(
        h.sim
            .load_floor(
                1,
                // No graveyard: the respawn point stays next to the start, away from the packs;
                // no player_spawn, so the player is not moved onto them either.
                &|a| (a != "graveyard" && a != "player_spawn")
                    .then_some((Vec3::new(0.0, 0.0, -300.0), 0.0)),
                vec![]
            )
            .is_ok()
    );
    h.tick();
    assert_eq!(h.sim.inventory(), [(DECK.to_string(), 1)]);
    assert_eq!(h.unit(h.player).max_hp, 308);

    h.sim.set_player_level(2);
    assert_eq!(
        h.unit(h.player).max_hp,
        308 + h.sim.content().levels[1].hp_bonus
    );

    h.sim.set_player_level(1);
    // A second CEO next to the player kills it in one swing.
    assert!(
        h.sim
            .spawn_unit("ceo", Vec3::new(0.0, 0.0, -19.0), 0.0)
            .is_ok()
    );
    h.run(60);
    assert!(!h.unit(h.player).alive);
    h.sim.push(Command::RespawnPlayer);
    h.tick();
    let p = h.unit(h.player);
    assert!(p.alive);
    assert_eq!((p.max_hp, p.hp), (308, 308));
    assert_eq!(p.inventory, [(DECK.to_string(), 1)]);

    let fresh = fresh_sim();
    assert!(fresh.inventory().is_empty());
    assert!(fresh.item_totals().is_empty());
}

fn fresh_sim() -> combat_core::Sim {
    let mut s = combat_core::Sim::new(content(), 1);
    s.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    s
}

#[test]
fn haste_bonus_shortens_the_gcd() {
    let mut h = h_with_loot(&[
        "Recurring Meeting Invite",
        "Recurring Meeting Invite",
        "Recurring Meeting Invite",
    ]);
    kill_and_loot(&mut h);
    assert_eq!(h.sim.item_totals().haste_pct, 3);
    let content = h.sim.content().clone();
    assert!(content.gcd_ms(3) < content.gcd_ms(0));
}

#[test]
fn every_loot_item_has_a_trophy_entry() {
    let c = content();
    for u in &c.units {
        for l in &u.loot {
            assert!(c.item(&l.item).is_some(), "{} missing from items", l.item);
        }
    }
}

#[test]
fn shipped_item_table_respects_the_content_rules() {
    let c = content();
    assert!(c.items.len() >= 40);
    for it in &c.items {
        assert!(it.description.chars().count() <= combat_core::ITEM_DESC_MAX);
        assert!(!it.bonus.totals(1).is_empty(), "{} has no bonus", it.name);
    }
}

#[test]
fn content_without_an_items_entry_for_a_drop_is_rejected() {
    let broken = SRC.replace(
        "(name: \"Unread AI Strategy Deck\",",
        "(name: \"Renamed Deck\",",
    );
    assert!(matches!(
        Content::from_ron(&broken),
        Err(combat_core::ContentError::Invalid(m)) if m.contains("Unread AI Strategy Deck")
    ));
}
