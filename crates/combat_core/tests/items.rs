//! Item catalogue gates: stat budgets per item level and rarity, perk magnitudes, every item
//! reachable from some source, tier coverage, sets, quest choices; plus the gear-only perks
//! (Execute, Triage, Desperate, KudosFind), named non-boss drops, tiered junk and daily items.

mod common;

use combat_core::{
    Command, Content, Event, GearDef, GearSource, Goods, Perk, Rarity, Role, Sim, Slot, SpellId,
    Vec3,
};
use common::*;

const HOTFIX: SpellId = SpellId(41);

fn hostile_levels(c: &Content) -> Vec<u8> {
    let mut v: Vec<u8> = c
        .zones
        .iter()
        .flat_map(|z| z.level_range.0..=z.level_range.1)
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Stats follow the documented budget, item power climbs with item level inside every rarity,
/// and only campus vendor cosmetics carry an authored (cheaper) price.
#[test]
fn every_gear_piece_follows_the_stat_budget() {
    let c = content();
    let eco = &c.economy;
    // Dungeons are low-level and drop tier pieces of their own band (checked below), not the raid tier.
    let raid_bosses: Vec<String> = c
        .raids
        .iter()
        .filter(|r| !r.is_dungeon() && !r.ilmari)
        .flat_map(|r| r.bosses(&c))
        .collect();
    assert_eq!(
        raid_bosses.len(),
        9,
        "four offsite and five VibeCon encounters"
    );
    let mut bad = Vec::new();
    for g in &eco.gear {
        let keyboard = if g.slot == Slot::Keyboard { 125 } else { 100 };
        let want = g.rarity.quality() * (70 + 3 * u64::from(g.ilvl)) * keyboard / 10_000;
        if g.budget() != want {
            bad.push(format!("{}: budget {} != {want}", g.name, g.budget()));
        }
        // Tier levels 2..=10; Legacy Mode's tier sits one tier above the cap, the raid's two.
        let raid = g.source == GearSource::Raid
            || matches!(&g.source, GearSource::Boss(k) if raid_bosses.contains(k));
        // The Ilmari band (16-20) sits above everything else and is the only band an authored
        // price may buy back cheaply: its vendor pays in tokens and Kudos are only the sale price.
        let top = if g.source == GearSource::Ilmari {
            20
        } else if g.source == GearSource::Legacy {
            12
        } else if raid {
            14
        } else {
            10
        };
        if g.ilvl == 0 || g.ilvl > top || g.ilvl % 2 == 1 {
            bad.push(format!("{}: ilvl {} is not a tier level", g.name, g.ilvl));
        }
        if (g.source == GearSource::Legacy) != (g.ilvl == 12) {
            bad.push(format!("{}: ilvl 12 is the Legacy tier only", g.name));
        }
        if raid != (g.ilvl == 14) {
            bad.push(format!("{}: ilvl 14 is the raid tier only", g.name));
        }
        let s = g.stats();
        let total = u64::from(s.totals.max_hp)
            + u64::from(s.totals.max_mana)
            + u64::from(s.totals.spell_power)
            + u64::from(s.totals.crit_tenths)
            + u64::from(s.haste_tenths)
            + u64::from(s.totals.mana_regen)
            + u64::from(s.totals.armor_tenths)
            + u64::from(s.totals.power_tenths);
        // Each role package sums to about 77-130 points per 100 budget (before flooring).
        if total < want * 70 / 100 || total > want * 135 / 100 {
            bad.push(format!("{}: stat total {total} for budget {want}", g.name));
        }
        let cheap_vendor = g.source == GearSource::Vendor && g.ilvl <= 2 && g.price <= 60;
        if g.price > 0 && !cheap_vendor && g.source != GearSource::Ilmari {
            bad.push(format!("{}: authored price {}", g.name, g.price));
        }
        if g.rarity == Rarity::Legendary && g.source == GearSource::World {
            bad.push(format!("{}: Legendaries are named drops", g.name));
        }
    }
    for r in Rarity::ALL {
        let mut by_ilvl: Vec<(u8, u64)> = eco
            .gear
            .iter()
            .filter(|g| g.rarity == r && g.slot != Slot::Keyboard)
            .map(|g| (g.ilvl, g.budget()))
            .collect();
        by_ilvl.sort_unstable();
        by_ilvl.dedup();
        if by_ilvl.windows(2).any(|w| w[0].1 >= w[1].1) {
            bad.push(format!(
                "{r:?}: budget does not climb with ilvl: {by_ilvl:?}"
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Percent perks stay inside a cap per rarity, gadgets cool down for at least 45 s and chance
/// procs stay modest, so no single piece outclasses its tier.
#[test]
fn perks_stay_inside_their_rarity_caps() {
    let c = content();
    let cap = |r: Rarity| match r {
        Rarity::Common | Rarity::Uncommon => 0,
        Rarity::Rare => 25,
        Rarity::Epic => 30,
        Rarity::Legendary => 50,
    };
    let mut bad = Vec::new();
    for g in &c.economy.gear {
        let Some(p) = &g.perk else { continue };
        let pct = match p {
            Perk::SpellDamage { pct, .. }
            | Perk::SpellHeal { pct, .. }
            | Perk::Cooldown { pct, .. }
            | Perk::Periodic { pct, .. }
            | Perk::ManaReturn { pct, .. }
            | Perk::Execute { pct, .. }
            | Perk::Triage { pct, .. }
            | Perk::Desperate { pct, .. }
            | Perk::KudosFind { pct }
            | Perk::Leech { pct, .. } => *pct,
            Perk::ManaCost { pct, .. } | Perk::CastTime { pct, .. } => pct.unsigned_abs(),
            Perk::DamageTaken { pct } => pct.unsigned_abs() * 3,
            Perk::Echo {
                chance_pct,
                pct,
                min_hp_pct,
                ..
            } => chance_pct * pct / 100 * (100 - (*min_hp_pct).min(90)) / 100,
            Perk::OnCast { chance_pct, .. }
            | Perk::OnHurt { chance_pct, .. }
            | Perk::Reset { chance_pct, .. } => *chance_pct,
            Perk::OnUse { cooldown_ms, .. } => {
                if *cooldown_ms < 45_000 {
                    bad.push(format!("{}: gadget cooldown {cooldown_ms} ms", g.name));
                }
                0
            }
            _ => 0,
        };
        if pct > cap(g.rarity) {
            bad.push(format!("{} ({:?}): perk {pct} over cap", g.name, g.rarity));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Every gear piece, junk item, trophy and consumable can be obtained somewhere.
#[test]
fn every_item_is_reachable_from_some_source() {
    let c = content();
    let eco = &c.economy;
    let levels = hostile_levels(&c);
    let tiers: Vec<u8> = levels.iter().map(|l| l.div_ceil(2).max(1)).collect();
    let shelves: Vec<&String> = eco
        .vendors
        .iter()
        .flat_map(|v| {
            v.stock
                .iter()
                .chain(&v.rotating)
                .chain(v.rep.iter().map(|r| &r.item))
        })
        .collect();
    let top = eco.campaign_top_tier();
    // Ilmari 1.0's Legendaries, fragments, Roomba parts and the tier pieces of the bosses that are not built yet
    // (Ilmari) are data until D9-D18 attach their sources (docs/ILMARI_1_0.md D4);
    // `tests/ilmari_band.rs` guards them instead. A tier piece of a built boss has to drop from it: the strict check.
    let r = &c.ilmari.rewards;
    let pending: Vec<&str> = r
        .tier_sets
        .iter()
        .flat_map(|s| s.pieces.iter().map(|p| p.gear.as_str()))
        .chain(r.legendaries.iter().map(|l| l.gear.as_str()))
        .chain([r.fragment.as_str()])
        .chain(r.mount.iter().map(|m| m.part_item.as_str()))
        .filter(|n| eco.gear(n).is_none_or(|g| g.drops_from.is_none()))
        .collect();
    let mut bad = Vec::new();
    for g in &eco.gear {
        let ok = match &g.source {
            GearSource::World => tiers.contains(&g.tier()),
            GearSource::Vendor => shelves.contains(&&g.name),
            GearSource::Boss(k) => c.unit_def(k).is_some_and(|u| u.hostile && u.ai.is_some()),
            GearSource::Sprint => g.tier() == top,
            // Drops only: above the cap, so no vendor level gate could sell it.
            GearSource::Legacy => {
                !c.legacy.zones.is_empty() && g.tier() > top && !shelves.contains(&&g.name)
            }
            GearSource::Raid => {
                !c.raids.is_empty() && g.tier() > top && !shelves.contains(&&g.name)
            }
            // Brenda's shelf until the wings drop their own: above the cap, sold for tokens.
            GearSource::Ilmari => {
                g.ilvl > combat_core::BASE_ILVL_CAP
                    && (shelves.contains(&&g.name)
                        || g.drops_from.is_some()
                        || pending.contains(&g.name.as_str()))
            }
            // Medal rewards only: some medal must hand it out, and no vendor sells it.
            GearSource::Minigame => {
                !shelves.contains(&&g.name)
                    && c.minigames
                        .iter()
                        .flat_map(|m| &m.medals)
                        .any(|d| d.items.contains(&g.name))
            }
        };
        if !ok {
            bad.push(format!("gear {} ({:?})", g.name, g.source));
        }
    }
    for j in &eco.junk {
        if j.tier != 0 && !tiers.contains(&j.tier) {
            bad.push(format!("junk {} (tier {})", j.name, j.tier));
        }
    }
    let trophy_sources: Vec<&String> = c
        .units
        .iter()
        .flat_map(|u| u.loot.iter().map(|l| &l.item))
        .chain(c.quests.iter().filter_map(|q| q.reward.trophy.as_ref()))
        .chain(
            c.world
                .achievements
                .iter()
                .filter_map(|a| a.trophy.as_ref()),
        )
        .collect();
    // Tokens also come from quest rewards (`tokens`), not only from loot.
    let paid = c.quests.iter().any(|q| q.reward.tokens > 0);
    for t in &c.items {
        let token_paid = paid && t.name == combat_core::ilmari::TOKEN;
        if !trophy_sources.contains(&&t.name) && !token_paid && !pending.contains(&t.name.as_str())
        {
            bad.push(format!("trophy {}", t.name));
        }
    }
    let task_items: Vec<&String> = c
        .live
        .dailies
        .iter()
        .chain(&c.live.weeklies)
        .filter_map(|t| t.item.as_ref())
        .collect();
    for k in &task_items {
        if eco.goods(k).is_none() {
            bad.push(format!("task item {k} is not goods"));
        }
    }
    for cons in &eco.consumables {
        let pet = c.pets.iter().any(|p| p.carrier == cons.name);
        if !shelves.contains(&&cons.name) && !pet && !task_items.contains(&&cons.name) {
            bad.push(format!("consumable {}", cons.name));
        }
    }
    assert!(bad.is_empty(), "unreachable:\n{}", bad.join("\n"));
}

/// Each tier's world pool has every slot, every role and every rarity up to Epic, so drops
/// and quest choices always have something to offer.
#[test]
fn every_tier_covers_slots_roles_and_rarities() {
    let c = content();
    let mut bad = Vec::new();
    for tier in 1..=5u8 {
        let pool: Vec<&GearDef> = c
            .economy
            .gear
            .iter()
            .filter(|g| g.source == GearSource::World && g.tier() == tier)
            .collect();
        if pool.len() < 30 {
            bad.push(format!("tier {tier}: only {} world pieces", pool.len()));
        }
        for s in Slot::ALL {
            if !pool.iter().any(|g| g.slot == s) {
                bad.push(format!("tier {tier}: no {s:?}"));
            }
        }
        for r in [Role::Ranged, Role::Melee, Role::Tank, Role::Healer] {
            for rar in [Rarity::Uncommon, Rarity::Rare, Rarity::Epic] {
                if !pool
                    .iter()
                    .any(|g| g.rarity == rar && (g.role == Some(r) || g.role.is_none()))
                {
                    bad.push(format!("tier {tier}: no {rar:?} for {r:?}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn sets_have_four_pieces_in_different_slots_of_one_item_level() {
    let c = content();
    assert!(c.economy.sets.len() >= 14, "{} sets", c.economy.sets.len());
    for s in &c.economy.sets {
        let pieces: Vec<&GearDef> = c
            .economy
            .gear
            .iter()
            .filter(|g| g.set.as_deref() == Some(s.key.as_str()))
            .collect();
        assert!(pieces.len() >= 4, "{}", s.key);
        let mut slots: Vec<usize> = pieces.iter().map(|g| g.slot.index()).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(
            slots.len(),
            pieces.len(),
            "{}: two pieces share a slot",
            s.key
        );
        assert!(
            pieces.iter().all(|g| g.ilvl == pieces[0].ilvl),
            "{}: mixed item levels",
            s.key
        );
    }
}

/// "Choose one" rewards never offer two pieces of a slot while other slots exist, and quests
/// of one tier do not all offer the same pieces.
#[test]
fn quest_choices_have_distinct_slots_and_vary_between_quests() {
    let c = content();
    for tier in 1..=5u8 {
        let quests: Vec<_> = c
            .quests
            .iter()
            .filter(|q| q.reward.gear_tier == tier)
            .collect();
        for role in [Role::Ranged, Role::Melee, Role::Tank, Role::Healer] {
            let mut sets = Vec::new();
            for q in &quests {
                let choices = c.quest_gear_choices(q, Some(role));
                let mut slots: Vec<usize> = choices
                    .iter()
                    .filter_map(|n| c.economy.gear(n))
                    .map(|g| g.slot.index())
                    .collect();
                slots.sort_unstable();
                slots.dedup();
                assert_eq!(slots.len(), choices.len(), "{} {role:?}: {choices:?}", q.id);
                sets.push(choices);
            }
            sets.sort();
            sets.dedup();
            if quests.len() >= 3 {
                assert!(
                    sets.len() >= 2,
                    "tier {tier} {role:?}: every quest offers {sets:?}"
                );
            }
        }
    }
}

// ---- gear-only perks ----

/// A test-only piece carrying `perk` (generalist package, so no crit from gear).
fn with_perk(c: &mut Content, perk: Perk) -> String {
    let mut g = c.economy.gear[0].clone();
    g.name = "Test Perk Piece".into();
    g.role = None;
    g.perk = Some(perk);
    g.perk_text = "test".into();
    c.economy.gear.push(g);
    "Test Perk Piece".into()
}

fn arena(class: &str, perk: Option<Perk>) -> (Sim, u64, u64) {
    let mut c = content();
    c.config.spell_hit_pct = 100;
    c.config.regen.hp_pct = 0;
    c.config.regen.mana_pct = 0;
    for s in &mut c.spells {
        s.yells.clear();
        s.learn_level = 1;
    }
    for u in &mut c.units {
        u.crit_pct = 0;
    }
    def_mut(&mut c, "ceo").ai = None;
    def_mut(&mut c, "ceo").max_hp = 100_000;
    // The bare run wears the same piece with a no-op perk, so only the perk differs.
    let item = with_perk(&mut c, perk.unwrap_or(Perk::DamageTaken { pct: 0 }));
    let mut sim = Sim::new(c, 7);
    let p = sim.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(p, 10);
    assert!(sim.debug_equip(p, &item));
    let mob = match sim.spawn_unit("ceo", Vec3::new(0.0, 0.0, -4.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    sim.tick();
    (sim, p, mob)
}

/// First amount of `spell` cast by `p` on `target` (damage or heal) within 4 s.
fn first_amount(sim: &mut Sim, p: u64, target: u64, spell: SpellId) -> u32 {
    sim.apply_as(
        p,
        Command::SetTarget {
            unit: p,
            target: Some(target),
        },
    );
    sim.apply_as(p, Command::Cast { caster: p, spell });
    for _ in 0..120 {
        for e in sim.tick() {
            match e {
                Event::Damage {
                    source,
                    spell: Some(s),
                    amount,
                    absorbed,
                    ..
                } if source == p && s == spell => return amount + absorbed,
                Event::Heal {
                    source,
                    spell: Some(s),
                    amount,
                    ..
                } if source == p && s == spell => return amount,
                _ => {}
            }
        }
    }
    panic!("no {spell:?}")
}

fn set_hp_pct(sim: &mut Sim, id: u64, pct: u32) {
    let max = sim.unit(id).map_or(1, |u| u.max_hp);
    assert!(sim.debug_set_hp(id, max * pct / 100));
}

#[test]
fn execute_raises_damage_only_against_low_targets() {
    let perk = || {
        Some(Perk::Execute {
            below_pct: 30,
            pct: 20,
        })
    };
    let hit = |perk: Option<Perk>, hp: u32| {
        let (mut sim, p, mob) = arena("fullstack", perk);
        set_hp_pct(&mut sim, mob, hp);
        first_amount(&mut sim, p, mob, CR)
    };
    let base = hit(None, 20);
    assert_eq!(hit(perk(), 20), base * 120 / 100);
    assert_eq!(hit(perk(), 90), hit(None, 90));
}

#[test]
fn desperate_raises_damage_while_you_are_low() {
    let perk = || {
        Some(Perk::Desperate {
            below_pct: 40,
            pct: 25,
        })
    };
    let hit = |perk: Option<Perk>, my_hp: u32| {
        let (mut sim, p, mob) = arena("fullstack", perk);
        set_hp_pct(&mut sim, p, my_hp);
        first_amount(&mut sim, p, mob, CR)
    };
    assert_eq!(hit(perk(), 30), hit(None, 30) * 125 / 100);
    assert_eq!(hit(perk(), 80), hit(None, 80));
}

#[test]
fn triage_raises_healing_on_low_targets() {
    let perk = || {
        Some(Perk::Triage {
            below_pct: 35,
            pct: 30,
        })
    };
    let heal = |perk: Option<Perk>, hp: u32| {
        let (mut sim, p, _) = arena("devops", perk);
        set_hp_pct(&mut sim, p, hp);
        first_amount(&mut sim, p, p, HOTFIX)
    };
    // Healing is rounded at several steps; the bonus lands within a point or two.
    let (with, without) = (heal(perk(), 20), heal(None, 20));
    assert!(
        with.abs_diff(without * 130 / 100) <= 4,
        "{with} vs {without}"
    );
    assert_eq!(heal(perk(), 50), heal(None, 50));
}

#[test]
fn kudos_find_raises_the_kill_kudos_of_the_wearer() {
    let kill = |perk: Option<Perk>| {
        let (mut sim, _, _) = arena("fullstack", perk);
        let mob = match sim.spawn_unit("copilot_drone", Vec3::new(4.0, 0.0, 0.0), 0.0) {
            Ok(id) => id,
            Err(e) => panic!("{e}"),
        };
        assert!(sim.debug_kill(mob));
        sim.tick()
            .into_iter()
            .find_map(|e| match e {
                Event::LootDropped { sp, .. } => Some(sp),
                _ => None,
            })
            .unwrap_or(0)
    };
    let base = kill(None);
    assert!(base > 0);
    assert_eq!(kill(Some(Perk::KudosFind { pct: 25 })), base * 125 / 100);
}

// ---- sources ----

fn kill_drops(c: Content, unit: &str, seed: u64) -> Vec<String> {
    let mut sim = Sim::new(c, seed);
    sim.spawn_player("backend", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let mob = match sim.spawn_unit(unit, Vec3::new(0.0, 0.0, -4.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    assert!(sim.debug_kill(mob));
    sim.tick()
        .into_iter()
        .find_map(|e| match e {
            Event::LootDropped { items, .. } => Some(items),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn the_night_shift_drops_its_named_pieces() {
    let mut c = content();
    c.economy.named_pct = 100;
    let named: Vec<String> = c
        .economy
        .gear
        .iter()
        .filter(|g| g.source == GearSource::Boss("deadline_wraith".into()))
        .map(|g| g.name.clone())
        .collect();
    assert!(named.len() >= 2);
    for seed in 0..5 {
        let items = kill_drops(c.clone(), "deadline_wraith", seed);
        assert!(items.iter().any(|i| named.contains(i)), "{items:?}");
    }
    // Off by default for a unit without named pieces: nothing named appears.
    let items = kill_drops(c, "copilot_drone", 1);
    assert!(items.iter().all(
        |i| !matches!(content().economy.gear(i), Some(g) if matches!(g.source, GearSource::Boss(_)))
    ));
}

#[test]
fn junk_drops_come_from_the_mob_tier_or_anywhere() {
    let mut c = content();
    c.economy.junk_pct = 100;
    c.economy.trash_gear_pct = 0;
    for (unit, tier) in [("copilot_drone", 1u8), ("prompt_whisperer", 5)] {
        let mut seen = Vec::new();
        for seed in 0..40 {
            for i in kill_drops(c.clone(), unit, seed) {
                if let Some(Goods::Junk(j)) = c.economy.goods(&i) {
                    assert!(
                        j.tier == 0 || j.tier == tier,
                        "{unit}: {} tier {}",
                        j.name,
                        j.tier
                    );
                    seen.push(j.tier);
                }
            }
        }
        assert!(
            seen.contains(&tier),
            "{unit}: no tier-{tier} junk in 40 kills"
        );
    }
}

#[test]
fn a_finished_daily_hands_out_its_item() {
    let mut c = content();
    let Some(daily) = c.live.dailies.iter().find(|t| t.item.is_some()).cloned() else {
        panic!("no daily with an item")
    };
    c.live.dailies.retain(|t| t.key == daily.key);
    c.live.per_day = 1;
    let mut sim = Sim::new(c, 5);
    let p = sim.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(sim.debug_join_standup(p));
    let board = sim.task_board(p);
    let Some(task) = board.tasks.iter().find(|t| t.key == daily.key) else {
        panic!("{board:?}")
    };
    let full: Vec<u32> = task.goals.iter().map(|g| g.need).collect();
    assert!(sim.debug_task_counts(p, &daily.key, &full));
    for _ in 0..60 {
        sim.tick();
    }
    let bag = sim.unit(p).map(|u| u.bag.clone()).unwrap_or_default();
    assert!(bag.contains(&daily.item.unwrap_or_default()), "{bag:?}");
}

/// Every visual style has a model in the client (game_client economy/outfit.rs), so no piece
/// silently falls back to another look.
#[test]
fn visual_styles_exist_for_their_slot() {
    let c = content();
    let max = |s: Slot| match s {
        Slot::Headgear => 16,
        Slot::Hoodie => 10,
        Slot::Mug => 7,
        Slot::Lanyard => 9,
        Slot::Keyboard => 6,
    };
    let bad: Vec<String> = c
        .economy
        .gear
        .iter()
        .filter(|g| g.visual.style > max(g.slot))
        .map(|g| format!("{}: style {}", g.name, g.visual.style))
        .collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
