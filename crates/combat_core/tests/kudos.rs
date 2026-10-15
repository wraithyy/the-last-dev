//! Kudos economy balance: expected income per zone of one campaign pass versus vendor prices
//! (docs/FORMULAS.md, Kudos). Print the table with
//! `cargo test -p combat_core --test kudos -- --nocapture`.

mod common;

use combat_core::{Command, Content, Goods, Rarity, Sim, Vec3};
use common::content;

const CAMPAIGN: [&str; 10] = [
    "campus",
    "tower/1",
    "tower/2",
    "tower/3",
    "datacenter/1",
    "datacenter/2",
    "tower/4",
    "tower/5",
    "lab/1",
    "lab/2",
];

/// Expected Kudos one solo player banks in each zone, in campaign order:
/// (zone, band top level, kills, quests, sales of drops, dailies + a seventh of the weekly).
/// Expected values, not rolls: kill Kudos `(level * sp_per_level + level / 2) * mult`, junk and
/// gear drops sold at the vendor's rate, every quest the zone closes, one day of dailies.
fn income(c: &Content) -> Vec<(String, u8, u32, u32, u32, u32)> {
    let eco = &c.economy;
    let mut sim = Sim::new(content(), 3);
    sim.set_wandering(false);
    let any = |_: &str| Some((Vec3::new(-300.0, 0.0, 0.0), 0.0));
    // Average sale value of one gear drop of a tier with the given rarity weights.
    let gear_sale = |tier: u8, weights: [u32; 4]| -> u32 {
        let total: u32 = weights.iter().sum::<u32>().max(1);
        let mut v = 0u64;
        for (k, w) in weights.iter().enumerate() {
            let pool: Vec<u32> = eco
                .gear
                .iter()
                .filter(|g| g.tier() == tier && g.rarity == Rarity::ALL[k])
                .map(|g| g.price())
                .collect();
            if !pool.is_empty() {
                let avg = pool.iter().sum::<u32>() / pool.len() as u32;
                v += u64::from(avg * eco.sell_pct / 100) * u64::from(*w);
            }
        }
        (v / u64::from(total)) as u32
    };
    let junk_avg = if eco.junk.is_empty() {
        0
    } else {
        eco.junk.iter().map(|j| j.price).sum::<u32>() / eco.junk.len() as u32
    };
    let mut out = Vec::new();
    for zone in CAMPAIGN {
        if let Err(e) = sim.load_zone(zone, &any, Vec::new()) {
            panic!("{zone}: {e}");
        }
        let top = c.zone(zone).map_or(1, |z| z.level_range.1);
        let (mut kills, mut sales) = (0u32, 0u32);
        for u in sim.units().filter(|u| u.hostile) {
            let level = u32::from(u.level);
            let elite = eco.elites.contains(&u.key);
            let (mult, gear_pct, weights) = if u.boss {
                (eco.boss_mult, 100, eco.boss_rarity)
            } else if elite {
                (eco.elite_mult, eco.elite_gear_pct, eco.elite_rarity)
            } else {
                (1, eco.trash_gear_pct, eco.trash_rarity)
            };
            kills += (level * eco.sp_per_level + level / 2) * mult.max(1);
            let tier = u.level.div_ceil(2).max(1);
            sales += gear_pct * gear_sale(tier, weights) / 100;
            if !u.boss {
                sales += eco.junk_pct * junk_avg / 100;
            }
        }
        let quests: u32 = c
            .quests
            .iter()
            .filter(|q| q.ender().zone == zone)
            .map(|q| q.reward.xp * eco.quest_sp_pct / 100 + q.reward.sp)
            .sum();
        let lo = c.zone(zone).map_or(1, |z| z.level_range.0);
        let daily =
            c.live.per_day as u32 * c.task_reward(false, lo).1 + c.task_reward(true, lo).1 / 7;
        out.push((zone.to_string(), top, kills, quests, sales, daily));
    }
    out
}

/// Total price of every vendor item (fixed and rotating shelves) a level-`level` player may buy.
fn stock_price(c: &Content, level: u8) -> u32 {
    let mut names: Vec<&String> = Vec::new();
    for v in &c.economy.vendors {
        for n in v.stock.iter().chain(&v.rotating) {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    names
        .iter()
        .filter_map(|n| c.economy.goods(n))
        .filter(|g| matches!(g, Goods::Gear(d) if d.required_level() <= level))
        .map(|g| g.price())
        .sum()
}

/// Cheapest Rare of a tier: the "notable upgrade" a player saves up for.
fn rare_price(c: &Content, tier: u8) -> u32 {
    c.economy
        .gear
        .iter()
        .filter(|g| g.tier() == tier && g.rarity == Rarity::Rare)
        .map(|g| g.price())
        .min()
        .unwrap_or(0)
}

/// The balance gate (docs/FORMULAS.md, Kudos): after any zone of one pass, everything earned so
/// far (spending nothing) stays under a third of the gear a vendor would sell at that level, and
/// the tier's Rare costs about one to two zones of income.
#[test]
fn kudos_never_buy_the_whole_stock_and_upgrades_take_zones() {
    let c = content();
    let rows = income(&c);
    let mut total = 0;
    println!(
        "| zone | band top | kills | quests | sales | dailies | zone total | cumulative | stock at band top | share | tier Rare | zones per Rare |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut bad = Vec::new();
    for (zone, top, kills, quests, sales, daily) in &rows {
        let sum = kills + quests + sales + daily;
        total += sum;
        let stock = stock_price(&c, *top);
        let rare = rare_price(&c, top.div_ceil(2));
        let zones = rare as f32 / sum.max(1) as f32;
        println!(
            "| {zone} | {top} | {kills} | {quests} | {sales} | {daily} | {sum} | {total} | {stock} | {}% | {rare} | {zones:.1} |",
            total * 100 / stock.max(1)
        );
        if total * 100 >= stock * 35 {
            bad.push(format!("{zone}: earned {total} of a {stock} stock"));
        }
        if !(0.8..=2.5).contains(&zones) {
            bad.push(format!(
                "{zone}: the tier Rare ({rare}) is {zones:.1} zones of income"
            ));
        }
    }
    let s = &c.sprint.reward;
    println!(
        "sprint chest per delivered run: +2 {}, +5 {}, +10 {}",
        2 * s.sp_per_level,
        5 * s.sp_per_level,
        10 * s.sp_per_level
    );
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Epics are the long goal: the priciest vendor piece is several zones of top-level income.
#[test]
fn epics_are_steep_and_vendors_gate_by_level() {
    let c = content();
    let eco = &c.economy;
    let epic = eco
        .gear
        .iter()
        .filter(|g| g.rarity == Rarity::Epic && g.tier() == 4)
        .map(|g| g.price())
        .min()
        .unwrap_or(0);
    assert!(epic >= 3 * rare_price(&c, 4), "epic {epic}");
    // Legacy and raid pieces sit above the cap and are never sold (tests/items.rs: drops only).
    let top = eco.campaign_top_tier();
    for g in eco.gear.iter().filter(|g| g.tier() <= top) {
        assert!(
            g.required_level() >= 1 && g.required_level() <= c.max_level(),
            "{}",
            g.name
        );
    }
}

fn errors(sim: &mut Sim) -> Vec<combat_core::ItemError> {
    sim.tick()
        .iter()
        .filter_map(|e| match e {
            combat_core::Event::ItemError { reason } => Some(*reason),
            _ => None,
        })
        .collect()
}

/// Vendors refuse gear above the buyer's level, whatever the wallet says.
#[test]
fn vendors_check_the_level_requirement() {
    let c = content();
    let mut sim = Sim::new(content(), 5);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let v = match sim.spawn_unit("procurement_officer", Vec3::new(0.0, 0.0, -3.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    let vendor = c.economy.vendor_for("procurement_officer").expect("vendor");
    let now = combat_core::ticks_to_ms(sim.now().0);
    let item = c
        .economy
        .stock(vendor, now)
        .into_iter()
        .find(|n| c.economy.gear(n).is_some_and(|g| g.required_level() > 1))
        .expect("a level-gated piece on the shelf");
    let need = c.economy.gear(&item).map_or(1, |g| g.required_level());
    sim.grant_sp(p, 100_000);
    let buy = Command::Buy {
        vendor: v,
        item: item.clone(),
    };
    sim.apply_as(p, buy.clone());
    assert_eq!(errors(&mut sim), [combat_core::ItemError::LevelTooLow]);
    sim.set_level_of(p, need);
    sim.apply_as(p, buy);
    assert!(errors(&mut sim).is_empty());
    assert!(sim.unit(p).is_some_and(|u| u.bag.contains(&item)));
}

/// Each Performance Review costs more than the last: 150, 300, 450 at level 10.
#[test]
fn respec_cost_grows_per_review() {
    let c = content();
    let mut sim = Sim::new(content(), 5);
    let me = sim.spawn_player("backend", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(me, 10);
    let mut paid = Vec::new();
    for k in 0..3 {
        sim.apply_as(me, Command::PickTalent { row: 0, choice: 1 });
        sim.tick();
        let cost = c.talents.respec_cost(10, k);
        sim.grant_sp(me, cost);
        sim.apply_as(me, Command::ResetTalents);
        assert!(errors(&mut sim).is_empty());
        assert_eq!(sim.unit(me).map(|u| (u.sp, u.respecs)), Some((0, k + 1)));
        paid.push(cost);
    }
    assert_eq!(paid, [150, 300, 450]);
    // The count survives a save round trip.
    let save = sim.export_save(me).expect("save");
    assert_eq!(save.respecs, 3);
    let back = combat_core::SaveData::decode(&save.encode()).expect("decode");
    assert_eq!(back.respecs, 3);
}
