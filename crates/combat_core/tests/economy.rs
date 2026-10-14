//! Economy: gear stats, perks, sets, vendors, Story Points, loot by zone and role, saves.

mod common;

use combat_core::{
    Command, Content, Event, GearSource, ItemError, Rarity, SLOTS, SaveData, Sim, Slot, SpellId,
    Vec3,
};
use common::*;

const PIXEL_PUSH: SpellId = SpellId(20);
const WHO_TOUCHED_PROD: SpellId = SpellId(32);

fn sim_with(class: &str, tweak: impl FnOnce(&mut Content)) -> (Sim, u64) {
    let mut c = content();
    c.config.spell_hit_pct = 100;
    c.config.white_hit_pct = 100;
    c.config.regen.hp_pct = 0;
    c.config.regen.mana_pct = 0;
    for s in &mut c.spells {
        s.yells.clear();
        s.learn_level = 1;
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, 7);
    let p = sim.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(p, 10);
    (sim, p)
}

fn tick(sim: &mut Sim, n: u32) -> Vec<Event> {
    (0..n).flat_map(|_| sim.tick()).collect()
}

fn me(sim: &Sim, p: u64) -> combat_core::UnitState {
    match sim.unit(p) {
        Some(u) => u.clone(),
        None => panic!("no player"),
    }
}

#[test]
fn rarity_and_item_level_scale_the_budget_and_role_picks_the_stats() {
    let c = content();
    let eco = &c.economy;
    let budget = |r: Rarity, ilvl: u8| {
        eco.gear
            .iter()
            .find(|g| g.rarity == r && g.ilvl == ilvl && g.slot != Slot::Keyboard)
            .map(|g| g.budget())
    };
    // Grey is clearly worse than green, green than blue, blue than purple.
    let tier5: Vec<u64> = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic]
        .iter()
        .filter_map(|r| budget(*r, 10))
        .collect();
    assert_eq!(tier5.len(), 4);
    assert!(tier5.windows(2).all(|w| w[0] < w[1]), "{tier5:?}");
    assert!(budget(Rarity::Rare, 2) < budget(Rarity::Rare, 10));
    let tank = eco
        .gear("Hoodie of Plausible Deniability")
        .map(|g| g.stats().totals);
    let ranged = eco
        .gear("RGB Keyboard That Increases FPS")
        .map(|g| g.stats());
    let (Some(tank), Some(ranged)) = (tank, ranged) else {
        panic!("missing gear");
    };
    assert!(tank.max_hp > 0 && tank.armor_tenths > 0 && tank.crit_tenths == 0);
    assert!(ranged.totals.crit_tenths > 0 && ranged.haste_tenths > 0 && ranged.totals.max_hp == 0);
    // Every world tier offers every rarity up to Epic, so loot never falls back far.
    for tier in 1..=5u8 {
        for r in [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic] {
            assert!(
                eco.gear
                    .iter()
                    .any(|g| g.tier() == tier && g.rarity == r && g.source == GearSource::World),
                "tier {tier} has no {r:?}"
            );
        }
    }
}

#[test]
fn equipping_changes_stats_swaps_through_the_bag_and_unequip_reverts() {
    let (mut sim, p) = sim_with("backend", |_| {});
    let base = me(&sim, p);
    assert!(sim.grant_goods(p, "Hoodie of Plausible Deniability"));
    assert!(sim.grant_goods(p, "Hoodie of Camera-Off Energy"));
    sim.apply_as(
        p,
        Command::Equip {
            item: "Hoodie of Plausible Deniability".into(),
        },
    );
    let ev = tick(&mut sim, 1);
    let geared = me(&sim, p);
    assert!(
        geared.max_hp > base.max_hp,
        "{} vs {}",
        geared.max_hp,
        base.max_hp
    );
    assert_eq!(
        geared.equipped[Slot::Hoodie.index()],
        "Hoodie of Plausible Deniability"
    );
    assert_eq!(geared.bag, vec!["Hoodie of Camera-Off Energy".to_string()]);
    assert!(sim.item_totals().armor_tenths > 0);
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::GearChanged { unit, .. } if *unit == p))
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Bag { unit, .. } if *unit == p))
    );
    // Swap: the old piece goes back where the new one was.
    sim.apply_as(
        p,
        Command::Equip {
            item: "Hoodie of Camera-Off Energy".into(),
        },
    );
    tick(&mut sim, 1);
    assert_eq!(
        me(&sim, p).bag,
        vec!["Hoodie of Plausible Deniability".to_string()]
    );
    sim.apply_as(
        p,
        Command::Unequip {
            slot: Slot::Hoodie.index() as u8,
        },
    );
    tick(&mut sim, 1);
    let bare = me(&sim, p);
    assert_eq!(bare.max_hp, base.max_hp);
    assert_eq!(bare.bag.len(), 2);
    assert!(bare.equipped.iter().all(String::is_empty));
    // Not in the bag: refused with a reason.
    sim.apply_as(
        p,
        Command::Equip {
            item: "The Monorepo".into(),
        },
    );
    let ev = tick(&mut sim, 1);
    assert!(ev.contains(&Event::ItemError {
        reason: ItemError::NotInBag
    }));
}

/// Damage of one Code Review against a passive training target, ungeared vs tier-5 Rare gear.
#[test]
fn a_full_set_of_tier_gear_raises_damage_by_a_third_or_more() {
    let run = |gear: &[&str]| -> u32 {
        let (mut sim, p) = sim_with("fullstack", |c| {
            def_mut(c, "ceo").ai = None;
            def_mut(c, "ceo").max_hp = 1_000_000;
            c.config.crit_damage_pct = 100;
        });
        for g in gear {
            assert!(sim.debug_equip(p, g), "{g}");
        }
        let mob = match sim.spawn_unit("ceo", Vec3::new(0.0, 0.0, -10.0), 0.0) {
            Ok(id) => id,
            Err(e) => panic!("{e}"),
        };
        sim.apply_as(
            p,
            Command::SetTarget {
                unit: p,
                target: Some(mob),
            },
        );
        let mut total = 0;
        // Ten seconds of Code Review spam: haste, power and spell power all count.
        for _ in 0..300 {
            sim.apply_as(
                p,
                Command::Cast {
                    caster: p,
                    spell: CR,
                },
            );
            for e in sim.tick() {
                if let Event::Damage { source, amount, .. } = e
                    && source == p
                {
                    total += amount;
                }
            }
            if let Some(u) = sim.unit(p)
                && u.mana < 100
            {
                sim.set_player_mana(10_000);
            }
        }
        total
    };
    let bare = run(&[]);
    let geared = run(&[
        "Full-Stack Overflow: Headset of Context Switching",
        "Rack-Mounted Hoodie",
        "RGB Keyboard That Increases FPS",
        "Stein of Burn Rate",
        "Wellness Program Lanyard",
    ]);
    println!("Code Review 10 s: ungeared {bare}, tier gear {geared}");
    assert!(
        u64::from(geared) * 100 >= u64::from(bare) * 130,
        "gear should be worth 30%+: {bare} -> {geared}"
    );
}

#[test]
fn tank_gear_cuts_damage_taken() {
    let taken = |gear: &[&str]| -> u32 {
        let (mut sim, p) = sim_with("backend", |_| {});
        for g in gear {
            assert!(sim.debug_equip(p, g), "{g}");
        }
        let mob = match sim.spawn_unit("vp_synergy", Vec3::new(0.0, 0.0, -2.0), 0.0) {
            Ok(id) => id,
            Err(e) => panic!("{e}"),
        };
        sim.apply_as(
            p,
            Command::SetTarget {
                unit: p,
                target: Some(mob),
            },
        );
        sim.apply_as(
            p,
            Command::Cast {
                caster: p,
                spell: SpellId(30),
            },
        );
        let mut total = 0;
        for _ in 0..450 {
            for e in sim.tick() {
                if let Event::Damage { target, amount, .. } = e
                    && target == p
                {
                    total += amount;
                }
            }
        }
        total
    };
    let bare = taken(&[]);
    let geared = taken(&[
        "Hoodie of Plausible Deniability",
        "Thought Leader Headset Microphone",
        "Burnout Collection: Lanyard of Last Nerves",
        "Burnout Collection: Mug of Cold Coffee",
    ]);
    println!("VP of Synergy 15 s on a level-10 tank: ungeared {bare}, geared {geared}");
    assert!(geared < bare, "{bare} -> {geared}");
}

#[test]
fn echo_perk_doubles_pixel_push_at_full_hp_and_is_deterministic() {
    let run = || -> (usize, Vec<Event>) {
        let (mut sim, p) = sim_with("frontend", |c| {
            def_mut(c, "ceo").ai = None;
            def_mut(c, "ceo").max_hp = 100_000;
        });
        assert!(sim.debug_equip(p, "Pixel-Perfect Mechanical Keyboard"));
        let mob = match sim.spawn_unit("ceo", Vec3::new(0.0, 0.0, -2.0), 0.0) {
            Ok(id) => id,
            Err(e) => panic!("{e}"),
        };
        tick(&mut sim, 1);
        sim.apply_as(
            p,
            Command::SetTarget {
                unit: p,
                target: Some(mob),
            },
        );
        sim.apply_as(
            p,
            Command::Cast {
                caster: p,
                spell: PIXEL_PUSH,
            },
        );
        let ev = tick(&mut sim, 2);
        let hits = ev
            .iter()
            .filter(|e| matches!(e, Event::Damage { source, spell: Some(s), .. } if *source == p && *s == PIXEL_PUSH))
            .count();
        (hits, ev)
    };
    let (healthy, ev1) = run();
    let (_, ev2) = run();
    assert_eq!(healthy, 2, "Pixel Push hits twice above 80% HP");
    assert!(ev1.iter().any(
        |e| matches!(e, Event::Proc { item, .. } if item == "Pixel-Perfect Mechanical Keyboard")
    ));
    assert_eq!(ev1, ev2, "same seed, same procs");
}

#[test]
fn rider_perk_turns_a_single_taunt_into_an_area_taunt() {
    let (mut sim, p) = sim_with("backend", |_| {});
    assert!(sim.debug_equip(p, "Incident Commander Headset"));
    let mut mobs = Vec::new();
    for x in [-3.0, 0.0, 3.0] {
        match sim.spawn_unit("copilot_drone", Vec3::new(x, 0.0, -4.0), 0.0) {
            Ok(id) => mobs.push(id),
            Err(e) => panic!("{e}"),
        }
    }
    // Someone else holds aggro on all three first.
    let other = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, -8.0), 0.0);
    for &m in &mobs {
        sim.apply_as(
            other,
            Command::SetTarget {
                unit: other,
                target: Some(m),
            },
        );
        sim.apply_as(
            other,
            Command::Cast {
                caster: other,
                spell: SpellId(6),
            },
        );
        tick(&mut sim, 50);
    }
    sim.apply_as(
        p,
        Command::SetTarget {
            unit: p,
            target: Some(mobs[1]),
        },
    );
    sim.apply_as(
        p,
        Command::Cast {
            caster: p,
            spell: WHO_TOUCHED_PROD,
        },
    );
    let ev = tick(&mut sim, 2);
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::Proc { item, .. } if item == "Incident Commander Headset"))
    );
    tick(&mut sim, 5);
    for &m in &mobs {
        assert_eq!(
            sim.unit(m).and_then(|u| u.target),
            Some(p),
            "mob {m} turned to the tank"
        );
    }
}

#[test]
fn set_bonuses_count_pieces() {
    let (mut sim, p) = sim_with("backend", |_| {});
    let base = sim.item_totals();
    assert!(sim.debug_equip(p, "Burnout Collection: Thousand-Yard Beanie"));
    let one = sim.item_totals();
    assert!(sim.debug_equip(p, "Burnout Collection: Hoodie of Quiet Quitting"));
    let two = sim.item_totals();
    // The 2-piece bonus is +40 HP and +3% armour on top of the pieces' own stats.
    let piece = one.max_hp - base.max_hp;
    assert!(two.max_hp >= base.max_hp + 2 * piece + 40, "{two:?}");
    assert!(two.armor_tenths >= one.armor_tenths * 2 + 30);
}

fn near_vendor(sim: &mut Sim, p: u64, key: &str) -> u64 {
    let v = match sim.spawn_unit(key, Vec3::new(0.0, 0.0, -3.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    let _ = p;
    v
}

#[test]
fn buying_selling_and_buyback_check_funds_range_and_stock() {
    let (mut sim, p) = sim_with("fullstack", |_| {});
    let v = near_vendor(&mut sim, p, "procurement_officer");
    let buy = |sim: &mut Sim, item: &str| {
        sim.apply_as(
            p,
            Command::Buy {
                vendor: v,
                item: item.into(),
            },
        );
        tick(sim, 1)
    };
    let ev = buy(&mut sim, "Ibuprofen");
    assert!(ev.contains(&Event::ItemError {
        reason: ItemError::NotEnoughSp
    }));
    sim.grant_sp(p, 100);
    buy(&mut sim, "Ibuprofen");
    let s = me(&sim, p);
    assert_eq!((s.sp, s.bag.clone()), (88, vec!["Ibuprofen".to_string()]));
    let ev = buy(&mut sim, "The Monorepo");
    assert!(ev.contains(&Event::ItemError {
        reason: ItemError::NotForSale
    }));
    // Sell gear for a quarter of its price, junk for all of it; buy back at the sell price.
    assert!(sim.grant_goods(p, "Expired Free Pizza Coupon"));
    sim.apply_as(
        p,
        Command::Sell {
            vendor: v,
            item: "Expired Free Pizza Coupon".into(),
        },
    );
    tick(&mut sim, 1);
    assert_eq!(me(&sim, p).sp, 91);
    sim.apply_as(
        p,
        Command::Buyback {
            vendor: v,
            item: "Expired Free Pizza Coupon".into(),
        },
    );
    tick(&mut sim, 1);
    let s = me(&sim, p);
    assert_eq!(s.sp, 88);
    assert!(s.buyback.is_empty() && s.bag.len() == 2);
    // Too far away: nothing happens but an error.
    sim.apply_as(
        p,
        Command::PlayerMoved {
            pos: Vec3::new(40.0, 0.0, 0.0),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    let ev = buy(&mut sim, "Ibuprofen");
    assert!(ev.contains(&Event::ItemError {
        reason: ItemError::TooFar
    }));
    assert_eq!(me(&sim, p).sp, 88);
}

#[test]
fn bag_capacity_is_enforced() {
    let (mut sim, p) = sim_with("fullstack", |c| c.economy.bag_size = 2);
    let v = near_vendor(&mut sim, p, "procurement_officer");
    sim.grant_sp(p, 1000);
    assert!(sim.grant_goods(p, "Ibuprofen"));
    assert!(sim.grant_goods(p, "Ibuprofen"));
    assert!(!sim.grant_goods(p, "Ibuprofen"));
    sim.apply_as(
        p,
        Command::Buy {
            vendor: v,
            item: "Cold Brew".into(),
        },
    );
    let ev = tick(&mut sim, 1);
    assert!(ev.contains(&Event::ItemError {
        reason: ItemError::BagFull
    }));
    assert_eq!(me(&sim, p).sp, 1000);
}

#[test]
fn consumables_share_a_cooldown_and_on_use_gear_fires_its_spell() {
    let (mut sim, p) = sim_with("fullstack", |_| {});
    sim.set_player_mana(0);
    assert!(sim.grant_goods(p, "Energy Drink"));
    assert!(sim.grant_goods(p, "Cold Brew"));
    sim.apply_as(
        p,
        Command::UseItem {
            item: "Energy Drink".into(),
        },
    );
    tick(&mut sim, 1);
    let s = me(&sim, p);
    assert!(s.mana > 0 && s.bag == vec!["Cold Brew".to_string()]);
    sim.apply_as(
        p,
        Command::UseItem {
            item: "Cold Brew".into(),
        },
    );
    let ev = tick(&mut sim, 1);
    assert!(ev.contains(&Event::ItemError {
        reason: ItemError::NotReady
    }));
    tick(&mut sim, 30 * 30);
    sim.apply_as(
        p,
        Command::UseItem {
            item: "Cold Brew".into(),
        },
    );
    tick(&mut sim, 1);
    assert!(me(&sim, p).auras.iter().any(|a| a.key == "cold_brew"));

    assert!(sim.debug_equip(p, "Out-of-Office Autoreply Mug"));
    sim.apply_as(
        p,
        Command::UseItem {
            item: "Out-of-Office Autoreply Mug".into(),
        },
    );
    let ev = tick(&mut sim, 1);
    assert!(me(&sim, p).auras.iter().any(|a| a.key == "out_of_office"));
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::ItemCooldown { ms: 120_000, .. }))
    );
    assert_eq!(
        sim.item_cooldown(p, "Out-of-Office Autoreply Mug").1,
        120_000
    );
}

/// Kills every pack of a zone with `debug_kill` and returns the gear that dropped.
fn zone_drops(c: &Content, zone: &str, class: &str, seed: u64) -> (Vec<String>, u32) {
    let mut sim = Sim::new(c.clone(), seed);
    let p = sim.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
    let anchors = |_: &str| Some((Vec3::new(0.0, 0.0, 0.0), 0.0));
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{e}");
    }
    let mut items = Vec::new();
    let mut sp = 0;
    for _ in 0..3 {
        let ids: Vec<u64> = sim
            .units()
            .filter(|u| u.hostile && u.alive)
            .map(|u| u.id)
            .collect();
        for id in ids {
            sim.debug_kill(id);
        }
        for e in sim.tick() {
            if let Event::LootDropped {
                items: it, sp: s, ..
            } = e
            {
                items.extend(it);
                sp += s;
            }
        }
        let _ = p;
    }
    (items, sp)
}

#[test]
fn loot_follows_the_zone_tier_the_role_and_bosses_always_drop_gear() {
    // Role preference forced on, so the expectation is exact rather than statistical.
    let mut c = content();
    c.economy.own_role_pct = 100;
    let eco = &c.economy;
    let tank = Some(combat_core::Role::Tank);
    for (zone, tier) in [("tower/1", 1u8), ("tower/3", 3), ("tower/5", 5)] {
        let mut gear = Vec::new();
        let mut sp = 0;
        for seed in 0..12 {
            let (items, s) = zone_drops(&c, zone, "backend", seed);
            sp += s;
            gear.extend(items.into_iter().filter_map(|n| eco.gear(&n).cloned()));
        }
        let world: Vec<_> = gear
            .iter()
            .filter(|g| g.source == GearSource::World)
            .collect();
        assert!(!world.is_empty() && sp > 0, "{zone}");
        for g in &world {
            assert_eq!(g.tier(), tier, "{zone}: {} is ilvl {}", g.name, g.ilvl);
            // Another role's piece only when the rarity has nothing for tanks in this tier.
            let fits = |o: &&combat_core::GearDef| {
                o.source == GearSource::World
                    && o.tier() == tier
                    && o.rarity == g.rarity
                    && (o.role.is_none() || o.role == tank)
            };
            assert!(
                g.role.is_none() || g.role == tank || !eco.gear.iter().any(|o| fits(&o)),
                "{zone}: {} is for {:?} although tank pieces of that rarity exist",
                g.name,
                g.role
            );
        }
    }
    // The boss of tower/1 drops a piece every time.
    for seed in 0..10 {
        let mut sim = Sim::new(content(), seed);
        sim.spawn_player("frontend", Vec3::new(0.0, 0.0, 0.0), 0.0);
        let anchors = |_: &str| Some((Vec3::new(0.0, 0.0, 0.0), 0.0));
        if let Err(e) = sim.load_zone("tower/1", &anchors, Vec::new()) {
            panic!("{e}");
        }
        let Some(boss) = sim.units().find(|u| u.boss).map(|u| u.id) else {
            panic!("no boss");
        };
        sim.debug_kill(boss);
        let drop = sim.tick().into_iter().find_map(|e| match e {
            Event::LootDropped { items, sp, .. } => Some((items, sp)),
            _ => None,
        });
        let Some((items, sp)) = drop else {
            panic!("no drop")
        };
        assert!(
            items.iter().any(|n| eco.gear(n).is_some()),
            "seed {seed}: {items:?}"
        );
        assert!(sp >= 24, "boss SP {sp}");
    }
}

#[test]
fn looting_moves_goods_to_the_bag_and_sp_to_the_wallet() {
    let mut h = H::new(1, -20.0, |c| {
        def_mut(c, "senior_engineer").spell_power = 100_000;
        def_mut(c, "ceo").boss = true;
    });
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
    let looted = h.events().find_map(|(_, e)| match e {
        Event::Looted { items, sp, .. } => Some((items.clone(), *sp)),
        _ => None,
    });
    let Some((items, sp)) = looted else {
        panic!("nothing looted")
    };
    let p = h.unit(h.player);
    assert_eq!(p.sp, sp);
    assert!(sp > 0);
    let goods: Vec<&String> = items
        .iter()
        .filter(|n| h.sim.content().economy.goods(n).is_some())
        .collect();
    assert!(!goods.is_empty(), "a boss always drops gear: {items:?}");
    assert!(goods.iter().all(|g| p.bag.contains(g)));
}

#[test]
fn quest_turn_in_pays_story_points() {
    let mut c = content();
    c.economy.quest_sp_pct = 50;
    let mut sim = Sim::new(c, 3);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let anchors = |_: &str| Some((Vec3::new(0.0, 0.0, 0.0), 0.0));
    if let Err(e) = sim.load_zone("campus", &anchors, Vec::new()) {
        panic!("{e}");
    }
    let Some(q) = sim.content().quests.first().cloned() else {
        panic!("no quests")
    };
    assert!(sim.debug_set_quest(&q.id, combat_core::QuestStatus::Ready));
    sim.apply_as(
        p,
        Command::TurnInQuest {
            quest: q.id.clone(),
        },
    );
    sim.tick();
    assert_eq!(me(&sim, p).sp, q.reward.xp / 2);
}

#[test]
fn saves_roundtrip_and_tolerate_bad_data() {
    let (mut sim, p) = sim_with("devops", |_| {});
    sim.grant_sp(p, 321);
    assert!(sim.grant_goods(p, "Ibuprofen"));
    assert!(sim.grant_goods(p, "Hoodie of Shareholder Value"));
    assert!(sim.debug_equip(p, "The Pager That Goes Off at 3 a.m."));
    assert!(sim.grant_trophy("Prompt Fragment"));
    tick(&mut sim, 1);
    let Some(save) = sim.export_save(p) else {
        panic!("no save")
    };
    let text = save.encode();
    let Some(back) = SaveData::decode(&text) else {
        panic!("decode failed:\n{text}")
    };
    assert_eq!(back, save);

    let mut fresh = Sim::new(content(), 9);
    let q = fresh.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(fresh.import_save(q, &back));
    let (a, b) = (me(&sim, p), me(&fresh, q));
    assert_eq!(
        (a.level, a.sp, &a.bag, &a.equipped, &a.inventory),
        (b.level, b.sp, &b.bag, &b.equipped, &b.inventory)
    );
    assert_eq!((a.max_hp, a.max_mana), (b.max_hp, b.max_mana));
    assert_eq!(b.hp, b.max_hp);

    // Garbage, other versions and unknown names never break a load.
    assert!(SaveData::decode("").is_none());
    assert!(SaveData::decode("hello\nworld").is_none());
    assert!(SaveData::decode(&text.replace("version=1", "version=99")).is_none());
    let mangled = text
        .replace("level=10", "level=banana")
        .replace("Ibuprofen", "Ibuprofen|Nonexistent Thing")
        .replace("sp=321", "sp=-5");
    let Some(m) = SaveData::decode(&mangled) else {
        panic!("tolerant decode failed")
    };
    let mut again = Sim::new(content(), 9);
    let r = again.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(again.import_save(r, &m));
    let c = me(&again, r);
    assert_eq!((c.level, c.sp), (1, 0));
    assert!(c.bag.iter().all(|n| n != "Nonexistent Thing"));
    assert_eq!(c.equipped.len(), SLOTS);
}

#[test]
fn every_item_text_is_written() {
    let c = content();
    for g in &c.economy.gear {
        assert!(g.description.len() > 30, "{} needs a real joke", g.name);
    }
    for (role, key) in [
        (combat_core::Role::Ranged, "fullstack"),
        (combat_core::Role::Melee, "frontend"),
        (combat_core::Role::Tank, "backend"),
        (combat_core::Role::Healer, "devops"),
    ] {
        let defining = c
            .economy
            .gear
            .iter()
            .filter(|g| g.role == Some(role) && g.rarity >= Rarity::Epic && g.perk.is_some())
            .count();
        assert!(
            defining >= 4,
            "{key}: only {defining} build-defining Epic/Legendary items"
        );
    }
}
