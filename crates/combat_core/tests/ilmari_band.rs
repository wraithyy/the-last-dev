//! The Ilmari item band at 1.0 (docs/ILMARI_1_0.md D.7, task D4): Wing 1-4 numbers read from the
//! content, the ilvl 20 budgets, the 16 tier-set pieces and eight Legendaries (data only), and the
//! rewards block (fragments, titles, tokens, the Roomba mount, the robot pet). Guardrail 3: a piece
//! counts as ilvl 14 outside Ilmari content and is in no base-game drop table.
#![allow(clippy::expect_used)]

mod common;

use combat_core::economy::{Rarity, Slot};
use combat_core::ilmari::{self, IlmariMode};
use combat_core::{Content, GearDef, GearSource, Role};
use common::content;

const TOP: u8 = 20;
/// Pieces that were at ilvl 20 before 1.0 and stay there: the Wing 2 review hoodie (a boss drop)
/// and the Resistance notebook (a vendor piece). Everything else at 20 is a D4 piece.
/// The bosses whose tier pieces drop so far (D6, D7); the Chief Ilmari Officer's and Ilmari's wait for D9-D13.
const DROPPING: [&str; 2] = ["il_align_consultant", "il_red_team"];
const GRANDFATHERED: [&str; 2] = [
    "Exceeds Expectations Hoodie",
    "Last Paper Notebook in the Building",
];

fn ilmari_gear(c: &Content) -> impl Iterator<Item = &GearDef> {
    c.economy
        .gear
        .iter()
        .filter(|g| g.source == GearSource::Ilmari)
}

fn rewards_pieces(c: &Content) -> Vec<&str> {
    let r = &c.ilmari.rewards;
    r.tier_sets
        .iter()
        .flat_map(|s| s.pieces.iter().map(|p| p.gear.as_str()))
        .chain(r.legendaries.iter().map(|l| l.gear.as_str()))
        .collect()
}

#[test]
fn the_wings_one_to_four_band_as_authored() {
    let c = content();
    let all: Vec<_> = c.raids.iter().filter(|r| r.ilmari).collect();
    assert_eq!(all.len(), 6, "Wings 1-6 exist, the core is a skeleton zone");
    // Wings 1-4 hold the "three loose pieces per boss" rule; Wing 5 also drops tier pieces (below).
    let wings: Vec<_> = all.into_iter().take(4).collect();
    // (lowest, highest) ilvl of each wing's own drops, read from the content rather than the doc.
    let mut band = Vec::new();
    for w in &wings {
        let bosses = w.bosses(&c);
        let ilvls: Vec<u8> = ilmari_gear(&c)
            .filter(|g| g.drops_from.as_ref().is_some_and(|b| bosses.contains(b)))
            .map(|g| g.ilvl)
            .collect();
        assert_eq!(
            ilvls.len(),
            bosses.len() * 3,
            "{}: three pieces per boss",
            w.name
        );
        band.push((
            *ilvls.iter().min().expect("pieces"),
            *ilvls.iter().max().expect("pieces"),
        ));
    }
    // Wing 1-2 open at 16 and reach 18 (Wing 2 holds one 20); Wings 3-4 sit at 16-18 and 18.
    assert_eq!(band, [(16, 18), (16, 20), (16, 18), (18, 18)], "{band:?}");
    // Odd item levels do not exist in this game (the raid tiers are even), so the doc's "19" is 18.
    assert!(
        c.economy
            .gear
            .iter()
            .all(|g| g.ilvl % 2 == 0 && g.ilvl <= TOP)
    );
    // The only 20s that are not D4's are the two grandfathered ones.
    let mut old: Vec<&str> = ilmari_gear(&c)
        .filter(|g| g.ilvl == TOP && !rewards_pieces(&c).contains(&g.name.as_str()))
        .map(|g| g.name.as_str())
        .collect();
    old.sort_unstable();
    assert_eq!(old, GRANDFATHERED);
}

#[test]
fn the_ilvl_20_budgets_follow_the_formula() {
    // budget = rarity quality x (70 + 3 ilvl) % x slot %: at ilvl 20 that is 130 % of the rarity.
    assert_eq!(Rarity::Epic.quality() * 130 / 100, 169);
    assert_eq!(
        Rarity::Legendary.quality() * 130 / 100 * 125 / 100,
        260,
        "a Legendary Keyboard"
    );
    let c = content();
    for name in rewards_pieces(&c) {
        let g = c.economy.gear(name).expect("piece");
        let keyboard = if g.slot == Slot::Keyboard { 125 } else { 100 };
        let want = g.rarity.quality() * 130 / 100 * keyboard / 100;
        assert_eq!(g.budget(), want, "{name}");
        // And it is a real step up from the 18 band and the raid tier at 14.
        let at = |ilvl: u8| {
            let mut x = g.clone();
            x.ilvl = ilvl;
            x.budget()
        };
        assert!(at(20) > at(18) && at(18) > at(14), "{name}");
    }
    // Legendary > Epic set piece > the best Wing 1-4 drop (an Epic at 18), per slot kind.
    let best_wing = ilmari_gear(&c)
        .filter(|g| g.drops_from.is_some() && g.ilvl == 18 && g.slot != Slot::Keyboard)
        .map(GearDef::budget)
        .max()
        .expect("wing pieces");
    assert!(169 > best_wing, "{best_wing}");
}

#[test]
fn all_24_pieces_are_ilvl_20_and_count_as_14_outside_ilmari() {
    let c = content();
    let r = &c.ilmari.rewards;
    assert_eq!(r.top_ilvl, TOP);
    assert_eq!(
        r.tier_sets.iter().map(|s| s.pieces.len()).sum::<usize>(),
        16
    );
    assert_eq!(r.legendaries.len(), 8);
    let roles: Vec<Role> = r.tier_sets.iter().map(|s| s.role).collect();
    for role in [Role::Tank, Role::Healer, Role::Melee, Role::Ranged] {
        assert!(roles.contains(&role), "{role:?}");
    }
    let eco = &c.economy;
    for name in rewards_pieces(&c) {
        let g = eco.gear(name).expect("piece");
        assert_eq!((g.ilvl, &g.source), (TOP, &GearSource::Ilmari), "{name}");
        // A tier piece drops from its own source once that boss is built; a Legendary never drops.
        let source = c
            .ilmari
            .rewards
            .tier_sets
            .iter()
            .flat_map(|s| &s.pieces)
            .find(|p| p.gear == name)
            .map(|p| p.source.as_str())
            .filter(|b| DROPPING.contains(b));
        assert_eq!(
            g.drops_from.as_deref(),
            source,
            "{name}: drops from its source boss once that boss is built, never before"
        );
        let mut capped = g.clone();
        capped.ilvl = combat_core::BASE_ILVL_CAP;
        let worn = [name.to_string()];
        assert_eq!(
            eco.equipment(&worn, false).0,
            capped.stats(),
            "{name} outside Ilmari"
        );
        assert_eq!(
            eco.equipment(&worn, true).0,
            g.stats(),
            "{name} inside Ilmari"
        );
        assert_ne!(capped.stats(), g.stats(), "{name}: the sync matters");
    }
    // A full set outside Ilmari: stats of ilvl 14, no 2-piece, no 4-piece perk.
    for s in &r.tier_sets {
        let worn: Vec<String> = s.pieces.iter().map(|p| p.gear.clone()).collect();
        let (outside, perks_out) = eco.equipment(&worn, false);
        let (inside, perks_in) = eco.equipment(&worn, true);
        assert!(perks_out.is_empty(), "{}", s.set);
        assert!(
            perks_in
                .iter()
                .any(|p| p.source == eco.set(&s.set).expect("set").name)
        );
        assert!(inside.totals != outside.totals, "{}", s.set);
        let mut flat = combat_core::GearStats::default();
        for p in &worn {
            let mut g = eco.gear(p).expect("piece").clone();
            g.ilvl = combat_core::BASE_ILVL_CAP;
            flat.add(&g.stats());
        }
        assert_eq!(outside, flat, "{}: no set bonus outside Ilmari", s.set);
    }
}

#[test]
fn no_base_game_source_ever_hands_one_out() {
    let c = content();
    let pieces = rewards_pieces(&c);
    for name in &pieces {
        let g = c.economy.gear(name).expect("piece");
        assert_eq!(g.source, GearSource::Ilmari, "{name}");
    }
    // No vendor lists or sells it (a token vendor's shelf included).
    for v in &c.economy.vendors {
        for item in v
            .stock
            .iter()
            .chain(&v.rotating)
            .chain(v.rep.iter().map(|r| &r.item))
            .chain(v.token_prices.iter().map(|(n, _)| n))
        {
            assert!(!pieces.contains(&item.as_str()), "{} sells {item}", v.title);
        }
    }
    // Nothing above the band exists and nothing reaches the base world's pools by rarity/tier:
    // World, Raid, Legacy, Sprint and named-boss pieces are never ilvl 20.
    for g in &c.economy.gear {
        if g.source != GearSource::Ilmari {
            assert!(
                g.ilvl <= combat_core::BASE_ILVL_CAP,
                "{} leaks ilvl {}",
                g.name,
                g.ilvl
            );
        }
    }
    // The sets are carried by Ilmari pieces only.
    for s in &c.ilmari.rewards.tier_sets {
        assert!(
            c.economy
                .gear
                .iter()
                .filter(|g| g.set.as_deref() == Some(s.set.as_str()))
                .all(|g| g.source == GearSource::Ilmari && g.ilvl == TOP),
            "{}",
            s.set
        );
    }
    // No `raid.sets` list (the raid tier's filter) names a 1.0 set.
    for r in &c.raids {
        for set in &r.sets {
            assert!(
                !c.ilmari.rewards.tier_sets.iter().any(|s| &s.set == set),
                "{set}"
            );
        }
    }
}

#[test]
fn the_rewards_block_is_what_the_doc_promises() {
    let c = content();
    let r = &c.ilmari.rewards;
    let jd = &c.ilmari.judgment;
    // One Legendary per class, each a Keyboard for its class's role.
    assert_eq!(r.legendaries.len(), c.classes.len());
    for cl in &c.classes {
        let l = r
            .legendaries
            .iter()
            .find(|l| l.class == cl.key)
            .expect("legendary");
        let g = c.economy.gear(&l.gear).expect("gear");
        assert_eq!(
            (g.slot, g.rarity, g.role),
            (Slot::Keyboard, Rarity::Legendary, Some(cl.role))
        );
        assert!(l.effect.chars().count() <= 240, "{}", l.gear);
    }
    // Fragments: five of them, none on Story, one on Full, two on Judgment.
    assert_eq!(r.fragments_needed, 5);
    let pay = |m| r.fragments_for(m, jd.fragment_mult);
    assert_eq!(
        [
            pay(IlmariMode::Story),
            pay(IlmariMode::Full),
            pay(IlmariMode::Judgment)
        ],
        [0, 1, 2]
    );
    // Titles: unique, wire-safe, and the wing titles are there.
    assert_eq!(r.titles.len(), 13);
    for key in [
        "hard_reset",
        "top_floor",
        "revert_committed",
        "last_human_standing",
    ] {
        assert!(r.titles.iter().any(|t| t.key == key), "{key}");
    }
    assert!(r.titles.iter().all(|t| t.title.len() <= 24));
    // The mount rides on the e-scooter's own speed and builds from eight parts.
    let ride = c.aura("roomba_mk9").expect("roomba");
    let scooter = c.aura("e_scooter").expect("scooter");
    assert_eq!(
        (ride.speed_pct, ride.mount, ride.harmful),
        (scooter.speed_pct, true, false)
    );
    assert_eq!(r.mount.as_ref().map(|m| m.parts), Some(8));
    // The pet is no companion and no base-game pet; the journal never lists it.
    assert!(c.pets.iter().all(|p| p.key != "clean_slate"));
    assert!(r.pet.as_ref().is_some_and(|p| p.lines.len() >= 3));
    // Cosmetics cost tokens and fit under the weekly cap.
    assert!(
        r.cosmetics
            .iter()
            .all(|k| k.tokens > 0 && k.tokens <= ilmari::TOKEN_WEEK_CAP)
    );
    // Tokens are power-free items, so they can never leak a bonus into the base game.
    for name in [r.fragment.as_str(), "Roomba Part (Salvaged)", ilmari::TOKEN] {
        assert_eq!(
            c.item(name).map(|i| i.kind),
            Some(combat_core::ItemKind::Token),
            "{name}"
        );
    }
}

#[test]
fn the_content_validation_refuses_a_broken_band() {
    let ron = include_str!("../../../content/game.ron");
    assert!(Content::from_ron(ron).is_ok());
    let first_set_piece = "(name: \"Constraint Lanyard (Non-Negotiable)\", icon: \"gear_idcard\", slot: Lanyard, rarity: Epic, ilvl: 20,";
    let legend = "(name: \"Runs Since 1974\", icon: \"gear_keyboard\", slot: Keyboard, rarity: Legendary, ilvl: 20,";
    assert!(ron.contains(first_set_piece) && ron.contains(legend));
    let broken = |from: &str, to: &str| ron.replacen(from, to, 1);
    for (what, text) in [
        (
            "a set piece below the top level",
            broken(
                first_set_piece,
                &first_set_piece.replace("ilvl: 20", "ilvl: 18"),
            ),
        ),
        (
            "a set piece above the top level",
            broken(
                first_set_piece,
                &first_set_piece.replace("ilvl: 20", "ilvl: 22"),
            ),
        ),
        (
            "a Rare set piece",
            broken(first_set_piece, &first_set_piece.replace("Epic", "Rare")),
        ),
        (
            "a Legendary that is not a Keyboard",
            broken(legend, &legend.replace("Keyboard", "Mug")),
        ),
        (
            "a Legendary at ilvl 18",
            broken(legend, &legend.replace("ilvl: 20", "ilvl: 18")),
        ),
        (
            "a missing Legendary class",
            broken(
                "(class: \"cobol\", gear: \"Runs Since 1974\",",
                "(class: \"nobody\", gear: \"Runs Since 1974\",",
            ),
        ),
        (
            "a set piece that is a World drop",
            broken(
                "set: Some(\"hard_constraints\"), source: Ilmari",
                "set: Some(\"hard_constraints\"), source: World",
            ),
        ),
        (
            "a title the wire would eat",
            broken(
                "(key: \"purple\", title: \"Purple\"",
                "(key: \"purple\", title: \"Purple (Team)\"",
            ),
        ),
        ("a free cosmetic", broken("tokens: 60", "tokens: 0")),
        (
            "a mount on another speed",
            broken(
                "name: \"Refurbished Roomba Mk. IX\", icon: \"scooter_helmet\", harmful: false,\n            speed_pct: 60",
                "name: \"Refurbished Roomba Mk. IX\", icon: \"scooter_helmet\", harmful: false,\n            speed_pct: 90",
            ),
        ),
        (
            "a fragment that carries a bonus",
            broken(
                "fragment: \"Legendary Fragment\"",
                "fragment: \"Laptop Box (Empty)\"",
            ),
        ),
    ] {
        assert!(Content::from_ron(&text).is_err(), "{what}");
    }
}
