//! The Prologue after the credits (docs/ILMARI_09.md B.2-B.6): the two phases and their toggle,
//! the attunement chain and the world quests played in the real zone geometry, the daily, the
//! token economy and the quartermaster, and the Ilmari item band's "only counts in Ilmari
//! content" rule. Everything runs offline and solo, as guardrail 5 asks.

mod common;

use combat_core::{
    Command, EntityId, Event, ItemError, QuestStatus, SaveData, Sim, Unlock, Vec3, ilmari,
    ms_to_ticks,
};
use common::{Geo, content, def_mut, geometry};
use std::collections::HashMap;

/// 2026-10-21 16:07:23 UTC: the player's own Judgment Day.
const STAMP: (u64, i16, &str) = (1_792_598_843, 120, "CEST");
const DAY_S: u64 = 86_400;

struct W {
    sim: Sim,
    p: EntityId,
    geo: HashMap<String, Geo>,
    log: Vec<Event>,
}

impl W {
    /// A player who survives anything (this is about quests and gear, not balance: the bot
    /// gates in `playthrough.rs` own that), level 10, past the credits and the Opening.
    fn opened() -> W {
        let mut c = content();
        let me = def_mut(&mut c, "senior_engineer");
        me.spell_power = 100_000;
        me.max_hp = 100_000;
        let mut sim = Sim::new(c, 5);
        sim.set_wandering(false);
        let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
        sim.set_player_level(10);
        sim.debug_mark_boss(ilmari::CREDITS_BOSS);
        let mut w = W {
            sim,
            p,
            geo: geometry(),
            log: Vec::new(),
        };
        w.load("campus");
        w.cmd(Command::StampIlmari {
            unix_secs: STAMP.0,
            utc_offset_min: STAMP.1,
            tz: STAMP.2.into(),
        });
        w.cmd(Command::IlmariReveal);
        assert!(w.sim.ilmari_open());
        w
    }

    fn load(&mut self, zone: &str) {
        let Some(g) = self.geo.get(zone) else {
            panic!("no geometry for {zone}")
        };
        let anchors = |n: &str| g.anchors.get(n).copied();
        self.sim.set_zone_terrain(zone, g.terrain.clone());
        if let Err(e) = self.sim.load_zone(zone, &anchors, g.blockers.clone()) {
            panic!("load_zone {zone}: {e}");
        }
        self.tick();
    }

    fn tick(&mut self) {
        let ev = self.sim.tick();
        self.log.extend(ev);
    }

    fn cmd(&mut self, c: Command) {
        self.sim.push(c);
        self.tick();
    }

    fn secs(&mut self, s: u64) {
        for _ in 0..ms_to_ticks(1000) * s {
            self.tick();
            self.sim.debug_set_hp(self.p, u32::MAX);
        }
    }

    fn move_to(&mut self, pos: Vec3) {
        self.cmd(Command::PlayerMoved {
            pos,
            yaw: 0.0,
            moving: false,
            jumping: false,
        });
    }

    fn stand_at(&mut self, anchor: &str) {
        let zone = self.sim.zone().unwrap_or_default().to_string();
        let Some((pos, _)) = self
            .geo
            .get(&zone)
            .and_then(|g| g.anchors.get(anchor))
            .copied()
        else {
            panic!("{zone}: no anchor {anchor}")
        };
        self.move_to(pos);
    }

    fn id_of(&self, key: &str) -> EntityId {
        match self.sim.units().find(|u| u.key == key && u.alive) {
            Some(u) => u.id,
            None => panic!("no living {key}"),
        }
    }

    fn talk(&mut self, key: &str) {
        let id = self.id_of(key);
        let Some(at) = self.sim.unit(id).map(|u| u.pos) else {
            return;
        };
        self.move_to(Vec3::new(at.x + 1.0, 0.0, at.z));
        self.cmd(Command::Talk { unit: id });
    }

    fn accept(&mut self, giver: &str, q: &str) {
        self.talk(giver);
        self.cmd(Command::AcceptQuest {
            quest: q.to_string(),
        });
        assert_eq!(self.sim.quest_status(q), QuestStatus::Active, "{q}");
    }

    fn turn_in(&mut self, q: &str) {
        let Some(npc) = self.sim.content().quest(q).map(|d| d.ender().npc.clone()) else {
            panic!("no quest {q}")
        };
        self.talk(&npc);
        assert_eq!(
            self.sim.quest_status(q),
            QuestStatus::Ready,
            "{q} not ready"
        );
        self.cmd(Command::TurnInQuest {
            quest: q.to_string(),
            choice: 0,
        });
        assert_eq!(self.sim.quest_status(q), QuestStatus::Done, "{q}");
    }

    fn use_prop(&mut self, prop: &str) {
        self.stand_at(&format!("prop:{prop}"));
        self.cmd(Command::Interact {
            prop: prop.to_string(),
        });
    }

    /// Kills the first living `key` as the player and loots its corpse.
    fn kill_and_loot(&mut self, key: &str) {
        let id = self.id_of(key);
        let Some(at) = self.sim.unit(id).map(|u| u.pos) else {
            return;
        };
        assert!(self.sim.debug_kill(id), "{key} would not die");
        self.tick();
        self.move_to(Vec3::new(at.x + 1.0, 0.0, at.z));
        self.cmd(Command::Loot {
            looter: self.p,
            corpse: id,
        });
    }

    fn tokens(&self) -> u32 {
        self.sim
            .inventory()
            .iter()
            .find(|(n, _)| n == ilmari::TOKEN)
            .map_or(0, |(_, c)| *c)
    }

    fn counts(&self, q: &str) -> Vec<u32> {
        self.sim
            .quests()
            .active
            .iter()
            .find(|a| a.id == q)
            .map(|a| a.counts.clone())
            .unwrap_or_default()
    }
}

fn link_open(sim: &Sim, zone: &str, to: &str) -> bool {
    sim.zone_links(zone)
        .iter()
        .find(|l| l.to == to)
        .is_some_and(|l| l.unlocked)
}

/// The whole Prologue chain, honestly: talk, collect from androids, hold the node, kill the two
/// elites, collect the passes, hand in where the work ends. No debug shortcuts but the kills.
#[test]
fn the_attunement_chain_completes_solo_and_offline() {
    let mut w = W::opened();
    assert_eq!(w.sim.quest_status("il_01_dale"), QuestStatus::Available);
    // The hatch opens with the Opening; the base is two loads away.
    w.load("parking_p3");
    assert!(link_open(&w.sim, "parking_p3", "resistance/1"));
    w.load("resistance/1");
    w.accept("il_dale", "il_01_dale");
    w.talk("il_dale");
    w.turn_in("il_01_dale");
    assert_eq!(w.tokens(), 2);

    w.accept("il_brenda", "il_02_sign_in");
    w.load("campus_occupied");
    // The four clerks and two guards: every android prints a blank badge.
    for key in ["il_android_clerk"; 4]
        .into_iter()
        .chain(["il_android_guard"; 2])
    {
        w.kill_and_loot(key);
    }
    assert_eq!(w.counts("il_02_sign_in"), [6]);
    // Handed in to Dale's field office in the same zone: no walk back to the base.
    w.turn_in("il_02_sign_in");
    assert_eq!(w.tokens(), 6);

    w.accept("il_dale_field", "il_03_node");
    w.use_prop("il_node_dc");
    w.secs(50);
    w.turn_in("il_03_node");

    w.accept("il_dale_field", "il_04_wing1_attune");
    w.kill_and_loot("il_gate_golem");
    w.turn_in("il_04_wing1_attune");

    w.accept("il_dale_field", "il_05_hr_intel");
    for k in 1..=3 {
        w.use_prop(&format!("il_hr_pass_{k}"));
    }
    w.turn_in("il_05_hr_intel");

    w.accept("il_dale_field", "il_06_wing2_attune");
    w.kill_and_loot("il_lobby_marshal");
    w.turn_in("il_06_wing2_attune");

    // 2 + 4 + 6 + 8 + 8 + 10 tokens, all inside the weekly cap.
    assert_eq!(w.tokens(), 38);
    assert_eq!(w.sim.ilmari_tokens_left(), ilmari::TOKEN_WEEK_CAP - 38);
    for q in [
        "il_01_dale",
        "il_02_sign_in",
        "il_03_node",
        "il_04_wing1_attune",
        "il_05_hr_intel",
        "il_06_wing2_attune",
    ] {
        assert_eq!(w.sim.quest_status(q), QuestStatus::Done, "{q}");
    }
    // The wing doors that arrive with the wings will wait on exactly these two quests.
    assert!(
        w.sim
            .content()
            .quest("il_04_wing1_attune")
            .is_some_and(|q| q.main)
    );
}

/// The side quests of the field post, one of each objective kind.
#[test]
fn the_world_quests_play_out_in_the_occupied_campus() {
    let mut w = W::opened();
    for q in ["il_01_dale", "il_02_sign_in"] {
        w.sim.debug_set_quest(q, QuestStatus::Done);
    }
    w.load("campus_occupied");

    w.accept("il_priya", "il_wq_checkpoint");
    for _ in 0..8 {
        w.kill_and_loot("il_android_guard");
    }
    w.turn_in("il_wq_checkpoint");

    w.accept("il_priya", "il_wq_mug");
    for k in 1..=4 {
        w.use_prop(&format!("il_mug_proof_{k}"));
    }
    w.turn_in("il_wq_mug");

    w.accept("il_rupert", "il_wq_cable");
    for k in 1..=3 {
        w.use_prop(&format!("il_cable_{k}"));
    }
    w.turn_in("il_wq_cable");

    w.accept("il_rupert", "il_wq_vending");
    w.stand_at("il_vending_spot");
    w.secs(95);
    w.turn_in("il_wq_vending");
    assert_eq!(w.tokens(), 4 + 3 + 3 + 4);

    // Brenda walks to the Data Center gate while the player stays within her escort range.
    w.accept("il_rupert", "il_wq_brenda");
    for _ in 0..200 {
        let Some(b) = w
            .sim
            .units()
            .find(|u| u.key == "il_brenda_field" && u.alive)
            .map(|u| u.pos)
        else {
            break;
        };
        w.move_to(Vec3::new(b.x, 0.0, b.z + 2.0));
        w.secs(1);
        if w.counts("il_wq_brenda").first() == Some(&100)
            || w.sim.quest_status("il_wq_brenda") == QuestStatus::Ready
        {
            break;
        }
    }
    w.turn_in("il_wq_brenda");
    assert_eq!(w.tokens(), 4 + 3 + 3 + 4 + 5);
}

/// "Disconnect a node" is offered again every UTC day and pays tokens each time.
#[test]
fn the_daily_node_comes_back_with_the_date() {
    let mut w = W::opened();
    for q in ["il_01_dale", "il_02_sign_in", "il_03_node"] {
        w.sim.debug_set_quest(q, QuestStatus::Done);
    }
    w.sim.set_clock(40_000 * DAY_S + 3_600);
    w.load("campus_occupied");
    w.accept("il_dale_field", "il_daily_node");
    w.use_prop("il_node_dc");
    w.secs(65);
    w.turn_in("il_daily_node");
    assert_eq!(w.tokens(), 3);
    // Done for today: Dale has nothing more to offer until midnight UTC.
    assert_eq!(w.sim.quest_status("il_daily_node"), QuestStatus::Done);

    // A save keeps the day; a fresh session on the same day agrees, the next day resets it.
    let text = w
        .sim
        .export_save(w.p)
        .map(|s| s.encode())
        .unwrap_or_default();
    assert!(text.contains("quest_days=il_daily_node*40000\n"), "{text}");
    let mut back = Sim::new(content(), 9);
    back.set_clock(40_000 * DAY_S + 7_200);
    let r = back.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(back.import_save(r, &SaveData::decode(&text).expect("decode")));
    assert_eq!(back.quest_status_of(r, "il_daily_node"), QuestStatus::Done);
    back.set_clock(40_001 * DAY_S + 60);
    assert_eq!(
        back.quest_status_of(r, "il_daily_node"),
        QuestStatus::Available
    );

    w.sim.set_clock(40_001 * DAY_S + 60);
    assert_eq!(w.sim.quest_status("il_daily_node"), QuestStatus::Available);
    w.accept("il_dale_field", "il_daily_node");
    w.use_prop("il_node_dc");
    w.secs(65);
    w.turn_in("il_daily_node");
    assert_eq!(w.tokens(), 6);
}

/// The stack holds 99, the week 200: the rest of a reward is lost, and next week it resets.
#[test]
fn tokens_stop_at_the_stack_cap_and_the_weekly_cap() {
    let mut w = W::opened();
    w.sim.set_clock(2_800 * 7 * DAY_S + 3 * DAY_S);
    assert_eq!(
        w.sim.grant_ilmari_tokens(w.p, 150),
        99,
        "the stack holds 99"
    );
    assert_eq!(w.tokens(), 99);
    assert_eq!(w.sim.grant_ilmari_tokens(w.p, 10), 0);
    assert_eq!(w.sim.ilmari_tokens_left(), 101);
    // Looting copies (a quest prop that is also a token) never passes the stack cap either.
    for _ in 0..120 {
        w.sim.grant_trophy("Blank Badge");
    }
    let badges = w
        .sim
        .inventory()
        .iter()
        .find(|(n, _)| n == "Blank Badge")
        .map_or(0, |(_, c)| *c);
    assert_eq!(badges, 99);
    // Spending frees stack room but not weekly allowance.
    w.load("resistance/1");
    w.talk("il_brenda");
    let brenda = w.id_of("il_brenda");
    w.cmd(Command::Buy {
        vendor: brenda,
        item: "Emergency Coffee, Unbranded".into(),
    });
    assert_eq!(w.tokens(), 98);
    assert_eq!(
        w.sim.grant_ilmari_tokens(w.p, 100),
        1,
        "one slot free in the stack"
    );
    assert_eq!(w.sim.ilmari_tokens_left(), 100);
    // The next ISO week starts a fresh allowance.
    w.sim.set_clock(2_801 * 7 * DAY_S + 3 * DAY_S);
    assert_eq!(w.sim.ilmari_tokens_left(), ilmari::TOKEN_WEEK_CAP);
    // And a save carries the counter, so reloading cannot refill the week.
    let text = w
        .sim
        .export_save(w.p)
        .map(|s| s.encode())
        .unwrap_or_default();
    assert!(text.contains("ilmari_tok=2800,"), "{text}");
}

/// Brenda takes tokens and only tokens, two band pieces a week.
#[test]
fn the_quartermaster_sells_two_band_pieces_a_week_for_tokens() {
    let mut w = W::opened();
    w.load("resistance/1");
    let brenda = w.id_of("il_brenda");
    w.talk("il_brenda");
    let eco = w.sim.content().economy.clone();
    let shelf = eco
        .vendor_for("il_brenda")
        .expect("Brenda has a shop")
        .clone();
    assert_eq!(shelf.token.as_deref(), Some(ilmari::TOKEN));
    let week = u64::from(shelf.rotate_ms);
    let stock = |ms: u64| eco.stock(&shelf, ms);
    let pieces: Vec<String> = stock(0)
        .into_iter()
        .filter(|n| eco.gear(n).is_some())
        .collect();
    assert_eq!(pieces.len(), 2, "two pieces on the shelf in any week");
    assert!(
        stock(week)
            .iter()
            .any(|n| !pieces.contains(n) && eco.gear(n).is_some()),
        "the shelf turns over weekly"
    );
    let piece = pieces[0].clone();
    let cost = shelf.token_price(&piece).expect("priced in tokens");

    // Kudos do not buy it, and short on tokens nothing is sold.
    w.sim.grant_sp(w.p, 1_000_000);
    w.log.clear();
    w.cmd(Command::Buy {
        vendor: brenda,
        item: piece.clone(),
    });
    assert!(
        w.log.iter().any(|e| matches!(e, Event::ItemError { reason, .. } if *reason == ItemError::NotEnoughTokens)),
        "{:?}",
        w.log
    );
    assert!(!w.sim.units().any(|u| u.id == w.p && u.bag.contains(&piece)));

    assert_eq!(w.sim.grant_ilmari_tokens(w.p, cost), cost);
    let sp = w
        .sim
        .player()
        .and_then(|p| w.sim.unit(p))
        .map_or(0, |u| u.sp);
    w.cmd(Command::Buy {
        vendor: brenda,
        item: piece.clone(),
    });
    let me = w.sim.unit(w.p).cloned().expect("player");
    assert!(me.bag.contains(&piece), "{:?}", me.bag);
    assert_eq!(w.tokens(), 0);
    assert_eq!(me.sp, sp, "tokens paid, no Kudos taken");
    // An item she does not stock is not for sale, whatever it costs.
    w.log.clear();
    w.cmd(Command::Buy {
        vendor: brenda,
        item: "Energy Drink".into(),
    });
    assert!(
        w.log.iter().any(
            |e| matches!(e, Event::ItemError { reason, .. } if *reason == ItemError::NotForSale)
        ),
        "{:?}",
        w.log
    );
}

fn totals_of(w: &W) -> combat_core::ItemTotals {
    w.sim.item_totals()
}

/// Guardrail 3: an Ilmari piece is the same as an item-level-14 piece anywhere but Ilmari
/// content, shows full strength inside it, and nothing else in the game changes.
#[test]
fn the_item_band_counts_in_full_only_in_ilmari_content() {
    const PIECE: &str = "Dale's Hoodie (Laminated)";
    let c = content();
    let g = c.economy.gear(PIECE).expect("band piece").clone();
    assert!(g.ilvl > combat_core::BASE_ILVL_CAP);

    // The reference: the same piece authored at the base cap, from a source that never syncs.
    let mut capped = content();
    for x in &mut capped.economy.gear {
        if x.name == PIECE {
            x.ilvl = combat_core::BASE_ILVL_CAP;
            x.source = combat_core::GearSource::World;
        }
    }
    let wear = |c: combat_core::Content, zone: &str| {
        let mut w = W::opened();
        w.sim = {
            let mut sim = Sim::new(c, 5);
            sim.set_wandering(false);
            w.p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
            sim.set_player_level(10);
            sim
        };
        w.load(zone);
        assert!(w.sim.debug_equip(w.p, PIECE));
        w.tick();
        w
    };
    let outside = wear(content(), "campus");
    let reference = wear(capped.clone(), "campus");
    assert_eq!(
        totals_of(&outside),
        totals_of(&reference),
        "synced to ilvl 14"
    );
    assert!(!totals_of(&outside).is_empty());

    // Inside, full strength; crossing the border re-sums without a gear change.
    let mut w = wear(content(), "campus");
    let before = (totals_of(&w), w.sim.unit(w.p).map(|u| u.max_hp));
    w.load("resistance/1");
    let inside = (totals_of(&w), w.sim.unit(w.p).map(|u| u.max_hp));
    assert!(inside.0.max_hp > before.0.max_hp || inside.0.power_tenths > before.0.power_tenths);
    assert!(inside.1 > before.1, "max HP follows the zone");
    w.load("campus_occupied");
    assert_eq!(totals_of(&w), inside.0);
    w.load("parking_p3");
    assert_eq!((totals_of(&w), w.sim.unit(w.p).map(|u| u.max_hp)), before);
    // The sheet shows what the sim uses, outside and inside.
    let eco = &w.sim.content().economy;
    let worn = vec![PIECE.to_string()];
    let (outer, _) = eco.equipment(&worn, false);
    let (full, _) = eco.equipment(&worn, true);
    assert_eq!(outer.totals, totals_of(&reference));
    assert!(full.totals.max_hp > outer.totals.max_hp);

    // Legacy and raid pieces never move, in or out.
    for piece in c.economy.gear.iter().filter(|g| {
        matches!(
            g.source,
            combat_core::GearSource::Legacy | combat_core::GearSource::Raid
        )
    }) {
        let worn = vec![piece.name.clone()];
        assert_eq!(
            eco.equipment(&worn, false).0,
            eco.equipment(&worn, true).0,
            "{} must not care about the zone",
            piece.name
        );
    }
}

/// The two phases are only zone ids: the door picks the world and round trips leave the base
/// campus exactly as it was.
#[test]
fn the_phase_toggle_round_trips_and_the_normal_campus_stays_put() {
    let mut w = W::opened();
    let keys = |w: &W| {
        let mut k: Vec<String> = w
            .sim
            .units()
            .filter(|u| u.alive)
            .map(|u| u.key.clone())
            .collect();
        k.sort();
        k
    };
    w.load("campus");
    let before = keys(&w);
    assert!(!before.iter().any(|k| k.starts_with("il_")), "{before:?}");

    for hop in [
        "parking_p3",
        "resistance/1",
        "campus_occupied",
        "resistance/1",
        "parking_p3",
        "campus",
    ] {
        w.load(hop);
        assert_eq!(w.sim.zone(), Some(hop));
        let ilmari_units = keys(&w).iter().filter(|k| k.starts_with("il_")).count();
        assert_eq!(
            ilmari_units > 0,
            ilmari::is_ilmari_zone(hop),
            "{hop}: Ilmari units exactly in Ilmari zones"
        );
    }
    assert_eq!(
        keys(&w),
        before,
        "the normal campus is unchanged after the trip"
    );

    // The links of the phases: every hop above is a real door, both ways, and the occupied
    // campus offers the old one too.
    assert!(link_open(&w.sim, "resistance/1", "campus_occupied"));
    assert!(link_open(&w.sim, "resistance/1", "parking_p3"));
    assert!(link_open(&w.sim, "campus_occupied", "resistance/1"));
    assert!(link_open(&w.sim, "campus_occupied", "campus"));
    let occupied = w
        .sim
        .content()
        .zone("campus_occupied")
        .cloned()
        .expect("zone");
    assert!(
        occupied
            .links
            .iter()
            .all(|l| ["resistance/1", "campus"].contains(&l.to.as_str())),
        "the closed buildings have no doors in this phase"
    );
    // No normal zone links into the occupation except the booth hatch.
    let c = w.sim.content();
    for z in c.zones.iter().filter(|z| !ilmari::is_ilmari_zone(&z.id)) {
        for l in z.links.iter().filter(|l| ilmari::is_ilmari_zone(&l.to)) {
            assert_eq!(
                (z.id.as_str(), l.to.as_str()),
                ("parking_p3", "resistance/1")
            );
            assert_eq!(l.requires, Some(Unlock::QuestDone("il_00_wake".into())));
        }
    }
}

/// Nothing before the credits: no Ilmari unit in any base zone, the hatch is locked, the
/// Prologue quests cannot be offered, the base is not reachable.
#[test]
fn the_prologue_does_not_exist_before_the_credits() {
    let c = content();
    let mut sim = Sim::new(c.clone(), 3);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(p, 10);
    for b in [
        "onboarding_buddy",
        "big_bang_cutover",
        "the_scheduler",
        "ceo",
    ] {
        sim.debug_mark_boss(b);
    }
    assert!(!sim.ilmari_pending() && !sim.ilmari_open());
    assert!(!link_open(&sim, "parking_p3", "resistance/1"));
    for q in c.quests.iter().filter(|q| q.id.starts_with("il_")) {
        assert_ne!(
            sim.quest_status_of(p, &q.id),
            QuestStatus::Available,
            "{}",
            q.id
        );
    }
    let geo = geometry();
    for z in c.zones.iter().filter(|z| !ilmari::is_ilmari_zone(&z.id)) {
        let g = &geo[&z.id];
        let anchors = |n: &str| g.anchors.get(n).copied();
        let mut s = Sim::new(c.clone(), 3);
        s.set_wandering(false);
        let q = s.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
        s.set_level_of(q, 10);
        for b in [
            "onboarding_buddy",
            "big_bang_cutover",
            "the_scheduler",
            "ceo",
        ] {
            s.debug_mark_boss(b);
        }
        s.set_zone_terrain(&z.id, g.terrain.clone());
        if s.load_zone(&z.id, &anchors, g.blockers.clone()).is_ok() {
            assert!(
                !s.units().any(|u| u.key.starts_with("il_")),
                "{} spawns Ilmari units before the credits",
                z.id
            );
        }
    }
    // Ilmari tokens and the band are not obtainable either: no base unit drops or sells them.
    let il_units: Vec<&str> = c
        .units
        .iter()
        .filter(|u| u.key.starts_with("il_") && u.key != "il_marie_awake")
        .map(|u| u.key.as_str())
        .collect();
    for z in c.zones.iter().filter(|z| !ilmari::is_ilmari_zone(&z.id)) {
        let spawned = z
            .packs
            .iter()
            .flat_map(|p| p.units.iter())
            .chain(z.friendly.iter().map(|f| &f.unit))
            .chain(z.rares.iter().map(|r| &r.unit))
            .chain(z.boss.iter().map(|b| &b.unit));
        for u in spawned {
            assert!(!il_units.contains(&u.as_str()), "{}: {u}", z.id);
        }
    }
    assert!(
        c.units
            .iter()
            .filter(|u| !u.key.starts_with("il_"))
            .all(|u| u
                .loot
                .iter()
                .all(|l| l.item != ilmari::TOKEN && l.item != "Blank Badge"))
    );
}

/// Old saves load, the new optional keys round trip, and a save with neither stays byte-clean.
#[test]
fn saves_carry_tokens_and_days_and_old_saves_load() {
    let mut w = W::opened();
    w.sim.grant_ilmari_tokens(w.p, 7);
    let save = w.sim.export_save(w.p).expect("save");
    let text = save.encode();
    assert!(text.contains("ilmari_tok="), "{text}");
    let mut back = Sim::new(content(), 11);
    let r = back.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(back.import_save(r, &SaveData::decode(&text).expect("decode")));
    assert_eq!(back.ilmari_tokens_left(), w.sim.ilmari_tokens_left());
    assert!(back.ilmari_open());

    // The save of a run that never saw Ilmari has none of the new keys, and loads as before.
    let mut old = Sim::new(content(), 12);
    let o = old.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let plain = old.export_save(o).expect("save").encode();
    assert!(
        !plain.contains("ilmari") && !plain.contains("quest_days"),
        "{plain}"
    );
    let mut fresh = Sim::new(content(), 13);
    let f = fresh.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(fresh.import_save(f, &SaveData::decode(&plain).expect("decode")));
    assert_eq!(fresh.ilmari_tokens_left(), ilmari::TOKEN_WEEK_CAP);
}
