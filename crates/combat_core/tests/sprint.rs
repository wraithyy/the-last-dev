//! Sprint mode rules: starting, stage travel, the timer and keystone rules, isolation from the
//! campaign, the leaderboard in the save, and every affix on its own.

mod common;

use combat_core::{
    AiState, Command, Content, EntityId, Event, SaveData, Sim, SpellId, SprintError, SprintResult,
    SprintStatus, Vec3,
};
use common::*;

const SPACING: f32 = 70.0;
const TICKS_PER_S: u64 = 30;
const TEAM_BUILDING: SpellId = SpellId(1200);

fn anchor(name: &str) -> Option<(Vec3, f32)> {
    let num = |p: &str| name.strip_prefix(p).and_then(|n| n.parse::<f32>().ok());
    if let Some(n) = num("pack_") {
        return Some((Vec3::new(0.0, 0.0, -SPACING * n), 0.0));
    }
    if let Some(n) = num("npc_") {
        return Some((Vec3::new(60.0, 0.0, -15.0 * n), 0.0));
    }
    // The campus onboarding corner (named, not numbered), far out of every pull.
    if name.starts_with("npc_") {
        return Some((Vec3::new(-400.0, 0.0, 400.0), 0.0));
    }
    (name == "boss").then_some((Vec3::new(0.0, 0.0, -SPACING * 7.0), 0.0))
}

/// Only `affix` exists and it joins at level 1, so tests pick their affix.
fn only(affix: &str) -> Content {
    let mut c = content();
    c.sprint.affixes.retain(|a| a.key == affix);
    assert_eq!(c.sprint.affixes.len(), 1, "{affix}");
    c.sprint.affixes[0].tier = 0;
    c.sprint.affix_levels = vec![1];
    c
}

fn no_affixes() -> Content {
    let mut c = content();
    c.sprint.affixes.clear();
    c
}

struct S {
    sim: Sim,
    p: EntityId,
    log: Vec<Event>,
}

impl S {
    fn new(c: Content, level: u8) -> S {
        let mut sim = Sim::new(c, 3);
        sim.set_wandering(false);
        let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 20.0), 0.0);
        sim.set_player_level(level);
        sim.set_respawn_point(Vec3::new(0.0, 0.0, 20.0), 0.0);
        if let Err(e) = sim.load_zone("campus", &anchor, Vec::new()) {
            panic!("{e}");
        }
        S {
            sim,
            p,
            log: Vec::new(),
        }
    }

    /// One tick; the test plays the host and follows `sprint_travel`.
    fn tick(&mut self) {
        self.log.extend(self.sim.tick());
        if let Some(z) = self.sim.sprint_travel().map(str::to_string)
            && let Err(e) = self.sim.load_zone(&z, &anchor, Vec::new())
        {
            panic!("{e}");
        }
    }

    fn secs(&mut self, s: u64) {
        for _ in 0..s * TICKS_PER_S {
            self.tick();
        }
    }

    fn start(&mut self, building: &str, level: u8) {
        self.sim.push(Command::StartSprint {
            building: building.into(),
            level,
        });
        self.tick();
    }

    /// Starts and waits for the first stage.
    fn start_in(&mut self, building: &str, level: u8) -> SprintStatus {
        self.start(building, level);
        let first = self.zones()[0].clone();
        for _ in 0..10 * TICKS_PER_S {
            if self.sim.zone() == Some(first.as_str()) {
                break;
            }
            self.tick();
        }
        assert_eq!(self.sim.zone(), Some(first.as_str()));
        // Views (auras) sync at the end of a tick.
        self.tick();
        self.status()
    }

    fn zones(&self) -> Vec<String> {
        self.log
            .iter()
            .find_map(|e| match e {
                Event::SprintStarted { zones, .. } => Some(zones.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn status(&self) -> SprintStatus {
        match self.sim.sprint_status() {
            Some(s) => s,
            None => panic!("no sprint"),
        }
    }

    fn hostiles(&self) -> Vec<combat_core::UnitState> {
        self.sim
            .units()
            .filter(|u| u.hostile && u.alive && u.ai.is_some())
            .cloned()
            .collect()
    }

    /// Kills every enemy of the loaded stage, boss last.
    fn clear_stage(&mut self) {
        let mut ids: Vec<(bool, EntityId)> =
            self.hostiles().iter().map(|u| (u.boss, u.id)).collect();
        ids.sort();
        for (_, id) in ids {
            self.sim.debug_kill(id);
        }
        self.tick();
    }

    fn walk_to(&mut self, pos: Vec3) {
        self.sim.push(Command::PlayerMoved {
            pos,
            yaw: 0.0,
            moving: false,
            jumping: false,
        });
        self.tick();
    }

    fn affix_events(&self, key: &str) -> usize {
        self.log
            .iter()
            .filter(|e| matches!(e, Event::SprintAffix { affix, .. } if affix == key))
            .count()
    }

    fn ended(&self) -> Option<(SprintResult, u32, i8)> {
        self.sim.sprint_last_result()
    }
}

#[test]
fn start_is_validated() {
    let refused = |s: &S| {
        s.log.iter().rev().find_map(|e| match e {
            Event::SprintRefused { reason } => Some(*reason),
            _ => None,
        })
    };
    let mut s = S::new(no_affixes(), 1);
    s.start("tower", 1);
    assert_eq!(refused(&s), Some(SprintError::Locked));
    s.sim.set_player_level(10);
    s.start("moon", 1);
    assert_eq!(refused(&s), Some(SprintError::UnknownBuilding));
    s.start("tower", 0);
    assert_eq!(refused(&s), Some(SprintError::BadLevel));
    s.start("tower", 21);
    assert_eq!(refused(&s), Some(SprintError::BadLevel));
    // A second member is not the leader.
    let other = s.sim.spawn_player("devops", Vec3::new(2.0, 0.0, 20.0), 0.0);
    s.sim.apply_as(
        other,
        Command::StartSprint {
            building: "lab".into(),
            level: 1,
        },
    );
    s.tick();
    assert_eq!(refused(&s), Some(SprintError::NotLeader));
    assert!(!s.sim.sprint_active());
    // Outside the campus.
    let mut t = S::new(no_affixes(), 10);
    if let Err(e) = t.sim.load_zone("tower/2", &anchor, Vec::new()) {
        panic!("{e}");
    }
    t.start("lab", 1);
    assert_eq!(refused(&t), Some(SprintError::NotInCampus));
    // Unlocked by the CEO kill even below level 6.
    let mut u = S::new(no_affixes(), 3);
    if let Err(e) = u.sim.load_zone("tower/5", &anchor, Vec::new()) {
        panic!("{e}");
    }
    let ceo = u
        .hostiles()
        .into_iter()
        .find(|h| h.key == "ceo")
        .map(|h| h.id);
    assert!(ceo.is_some_and(|c| u.sim.debug_kill(c)));
    if let Err(e) = u.sim.load_zone("campus", &anchor, Vec::new()) {
        panic!("{e}");
    }
    u.start("lab", 1);
    assert!(u.sim.sprint_active(), "{:?}", refused(&u));
}

#[test]
fn a_run_travels_its_stages_scales_enemies_and_delivers() {
    let mut s = S::new(no_affixes(), 10);
    let st = s.start_in("tower", 5);
    let zones = s.zones();
    assert_eq!(zones.len(), 3);
    assert!(
        zones.windows(2).all(|w| w[0] < w[1]),
        "floors go up: {zones:?}"
    );
    assert_eq!((st.stage, st.stages, st.bosses_total), (1, 3, 3));
    // Every enemy is at least level 10 and has the level's +32% HP.
    for h in s.hostiles() {
        assert!(h.level >= 10, "{} level {}", h.key, h.level);
    }
    let boss = |x: &S| x.hostiles().into_iter().find(|h| h.boss).map(|h| h.max_hp);
    let Some(scaled) = boss(&s) else {
        panic!("no boss");
    };
    // Same session seed and run number: the same floors at level 1, so bosses compare.
    let mut lvl1 = S::new(no_affixes(), 10);
    lvl1.start_in("tower", 1);
    assert_eq!(lvl1.zones(), zones);
    let Some(base) = boss(&lvl1) else {
        panic!("no boss")
    };
    assert_eq!(scaled, (u64::from(base) * 132 / 100) as u32);
    // Doors stay shut except the way out.
    for l in s.sim.zone_links(&zones[0]) {
        assert_eq!(l.unlocked, l.to == "campus", "{l:?}");
    }
    for z in &zones {
        assert_eq!(s.sim.zone(), Some(z.as_str()));
        s.clear_stage();
        s.secs(5);
    }
    let Some((result, time, upgrade)) = s.ended() else {
        panic!("no end: {:?}", s.sim.sprint_status());
    };
    assert_eq!(result, SprintResult::Delivered);
    assert!(time < 30_000, "{time}");
    assert_eq!(upgrade, 3);
    assert_eq!(s.sim.sprint_key(s.p, "tower"), 8);
    assert_eq!(s.sim.sprint_records(s.p).len(), 1);
    // The chest: SP and gear in the bag.
    let chest = s.log.iter().find_map(|e| match e {
        Event::SprintEnded { summary, .. } => Some(summary.clone()),
        _ => None,
    });
    let Some(chest) = chest else { panic!() };
    assert_eq!(chest.rewards.len(), 1);
    assert_eq!(chest.rewards[0].sp, 200);
    assert!(chest.rewards[0].items.len() >= 2);
    assert_eq!(chest.next_level, 8);
    // Then the group is sent home.
    s.secs(21);
    assert_eq!(s.sim.zone(), Some("campus"));
    assert!(s.sim.sprint_status().is_none());
}

#[test]
fn late_runs_and_deaths_carry_over() {
    let mut s = S::new(no_affixes(), 10);
    let st = s.start_in("lab", 3);
    assert_eq!(st.par_ms, 624_000);
    assert!(s.sim.debug_kill_player(s.p));
    s.tick();
    assert_eq!(s.status().deaths, 1);
    assert_eq!(s.status().penalty_ms, 5_000);
    s.sim.push(Command::RespawnPlayer);
    s.tick();
    // Past par.
    s.secs(625);
    for _ in 0..2 {
        s.clear_stage();
        s.secs(5);
    }
    let Some((result, time, upgrade)) = s.ended() else {
        panic!("no end")
    };
    assert_eq!(result, SprintResult::CarriedOver);
    assert!(time > 624_000);
    assert_eq!(upgrade, 0);
    assert_eq!(s.sim.sprint_key(s.p, "lab"), 2);
    assert!(s.sim.sprint_records(s.p).is_empty(), "only deliveries rank");
}

#[test]
fn skipped_scope_is_carried_over_even_on_time() {
    let mut s = S::new(no_affixes(), 10);
    s.start_in("datacenter", 1);
    // Only the bosses: the trash stays, so story points never reach 100%.
    for _ in 0..2 {
        let boss = s.hostiles().into_iter().find(|h| h.boss).map(|h| h.id);
        assert!(boss.is_some_and(|b| s.sim.debug_kill(b)));
        s.secs(5);
    }
    assert!(s.sim.sprint_active(), "trash left: the sprint goes on");
    assert!(s.status().forces_pct < 100);
    s.clear_stage();
    s.secs(2);
    assert_eq!(s.ended().map(|e| e.0), Some(SprintResult::CarriedOver));
}

#[test]
fn the_run_does_not_touch_campaign_progress_and_leaving_abandons() {
    let mut s = S::new(no_affixes(), 10);
    if let Err(e) = s.sim.load_zone("tower/1", &anchor, Vec::new()) {
        panic!("{e}");
    }
    s.clear_stage();
    assert!(s.sim.zone_cleared("tower/1"));
    if let Err(e) = s.sim.load_zone("campus", &anchor, Vec::new()) {
        panic!("{e}");
    }
    s.start_in("lab", 1);
    s.log.retain(|e| matches!(e, Event::SprintStarted { .. }));
    assert!(!s.sim.zone_cleared("tower/1"), "the run has its own world");
    // The save written mid-run keeps the campaign.
    let Some(save) = s.sim.export_save(s.p) else {
        panic!()
    };
    assert_eq!(save.cleared, vec!["tower/1".to_string()]);
    s.clear_stage();
    s.secs(5);
    assert!(s.sim.zone() == Some("lab/2"));
    let lab2 = s.hostiles().into_iter().find(|h| h.boss).map(|h| h.id);
    assert!(lab2.is_some_and(|b| s.sim.debug_kill(b)));
    s.tick();
    // Walking out abandons.
    if let Err(e) = s.sim.load_zone("campus", &anchor, Vec::new()) {
        panic!("{e}");
    }
    s.tick();
    assert_eq!(s.ended().map(|e| e.0), Some(SprintResult::Abandoned));
    assert!(!s.sim.sprint_active());
    assert!(s.sim.zone_cleared("tower/1"));
    assert!(
        !s.sim.zone_cleared("lab/2"),
        "the run's kills stay in the run"
    );
    assert!(
        !s.log.iter().any(|e| matches!(e, Event::Unlocked { .. })),
        "no unlock banners from a run"
    );
    // Abandoning counts as carried over for the next level.
    assert_eq!(s.sim.sprint_key(s.p, "lab"), 1);
}

#[test]
fn keystones_and_the_leaderboard_survive_the_save() {
    let mut s = S::new(no_affixes(), 10);
    s.start_in("lab", 2);
    for _ in 0..2 {
        s.clear_stage();
        s.secs(5);
    }
    assert_eq!(s.ended().map(|e| e.0), Some(SprintResult::Delivered));
    let Some(save) = s.sim.export_save(s.p) else {
        panic!()
    };
    assert_eq!(save.sprint_keys, vec![("lab".to_string(), 5)]);
    assert_eq!(save.sprint_records.len(), 1);
    let mut rec = save.clone();
    rec.sprint_records[0].name = "Ana|Lyst".into();
    let text = rec.encode();
    let Some(back) = SaveData::decode(&text) else {
        panic!("{text}")
    };
    assert_eq!(back.sprint_keys, save.sprint_keys);
    assert_eq!(back.sprint_records[0].name, "AnaLyst");
    assert_eq!(back.sprint_records[0].level, 2);
    assert_eq!(back.sprint_records[0].building, "lab");
    assert_eq!(back.sprint_records[0].class, "fullstack");
    let mut fresh = Sim::new(content(), 9);
    let p = fresh.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(fresh.import_save(p, &back));
    assert_eq!(fresh.sprint_key(p, "lab"), 5);
    assert_eq!(fresh.sprint_records(p), &back.sprint_records[..]);
}

// ---- affixes ----

/// Walks into pack 1 so the fight starts.
fn pull(s: &mut S) {
    let at = Vec3::new(0.0, 0.0, -SPACING + 3.0);
    s.walk_to(at);
    s.secs(1);
    assert!(s.sim.unit(s.p).is_some_and(|u| u.in_combat));
}

#[test]
fn ai_assisted_hits_you_with_your_own_last_spell() {
    let mut s = S::new(only("ai_assisted"), 10);
    s.start_in("tower", 1);
    pull(&mut s);
    let t = s.hostiles()[0].id;
    s.sim.push(Command::SetTarget {
        unit: s.p,
        target: Some(t),
    });
    s.sim.push(Command::Cast {
        caster: s.p,
        spell: CR,
    });
    s.secs(16);
    assert!(s.affix_events("ai_assisted") >= 1);
    let p = s.p;
    let copied = s.log.iter().any(|e| {
        matches!(e, Event::Damage { target, spell: Some(sp), source, .. }
            if *target == p && *sp == CR && *source != p)
    });
    assert!(copied, "an enemy cast the player's own spell back at them");
}

#[test]
fn reorg_moves_idle_packs() {
    let mut s = S::new(only("reorg"), 10);
    s.start_in("tower", 1);
    let packs = |s: &S| {
        let mut v: Vec<(u32, i32)> = s
            .hostiles()
            .iter()
            .filter(|u| !u.boss)
            .filter_map(|u| u.pack.map(|p| (p, u.pos.z.round() as i32)))
            .collect();
        v.sort();
        v.dedup_by_key(|x| x.0);
        v
    };
    let before = packs(&s);
    s.secs(91);
    let after = packs(&s);
    assert_eq!(before.len(), after.len());
    assert!(
        before.iter().zip(&after).all(|(a, b)| a.1 != b.1),
        "every idle pack moved: {before:?} -> {after:?}"
    );
    let mut spots: Vec<i32> = before.iter().map(|x| x.1).collect();
    let mut now: Vec<i32> = after.iter().map(|x| x.1).collect();
    spots.sort();
    now.sort();
    assert_eq!(spots, now, "they swap the same spots");
    assert_eq!(s.affix_events("reorg"), 1);
}

#[test]
fn scope_creep_adds_a_requirement_per_finished_pack() {
    let mut s = S::new(only("scope_creep"), 10);
    s.start_in("tower", 1);
    let pack = s
        .hostiles()
        .into_iter()
        .find(|u| !u.boss)
        .and_then(|u| u.pack);
    let members: Vec<EntityId> = s
        .hostiles()
        .iter()
        .filter(|u| u.pack == pack)
        .map(|u| u.id)
        .collect();
    for (k, id) in members.iter().enumerate() {
        s.sim.debug_kill(*id);
        s.tick();
        let adds = s
            .hostiles()
            .iter()
            .filter(|u| u.key == "additional_requirement")
            .count();
        assert_eq!(
            adds,
            usize::from(k + 1 == members.len()),
            "only after the last"
        );
    }
    s.tick();
    let add = s
        .hostiles()
        .into_iter()
        .find(|u| u.key == "additional_requirement");
    assert!(add.is_some_and(|a| a.level >= 10 && a.target == Some(s.p)));
    // Killing the add itself does not spawn another.
    let id = s
        .hostiles()
        .into_iter()
        .find(|u| u.key == "additional_requirement")
        .map(|u| u.id);
    assert!(id.is_some_and(|i| s.sim.debug_kill(i)));
    s.tick();
    assert_eq!(s.affix_events("scope_creep"), 1);
}

#[test]
fn mandatory_fun_paints_a_shared_circle_on_a_fighter() {
    let mut s = S::new(only("mandatory_fun"), 10);
    s.start_in("tower", 1);
    pull(&mut s);
    s.secs(31);
    let p = s.sim.unit(s.p).map(|u| u.pos);
    let painted = s.log.iter().any(|e| {
        matches!(e, Event::TelegraphStarted { spell, pos, .. }
            if *spell == TEAM_BUILDING && p.is_some_and(|p| combat_core::distance(*pos, p) < 3.0))
    });
    assert!(painted || s.affix_events("mandatory_fun") > 0);
    assert!(
        s.sim
            .telegraphs()
            .iter()
            .chain(std::iter::empty())
            .all(|t| t.spell != TEAM_BUILDING || t.split)
    );
}

#[test]
fn technical_debt_stacks_with_deaths() {
    let mut s = S::new(only("technical_debt"), 10);
    s.start_in("tower", 1);
    for k in 1..=2u8 {
        assert!(s.sim.debug_kill_player(s.p));
        s.tick();
        assert_eq!(s.status().tech_debt, k);
        s.sim.push(Command::RespawnPlayer);
        s.tick();
    }
    assert_eq!(s.affix_events("technical_debt"), 2);
    // The same pull hurts more with debt: average enemy hit against the old level.
    let hits = |c: Content, deaths: u32| {
        let mut x = S::new(c, 10);
        x.start_in("tower", 1);
        for _ in 0..deaths {
            x.sim.debug_kill_player(x.p);
            x.tick();
            x.sim.push(Command::RespawnPlayer);
            x.tick();
        }
        x.log.clear();
        pull(&mut x);
        x.secs(8);
        let p = x.p;
        let v: Vec<u32> = x
            .log
            .iter()
            .filter_map(|e| match e {
                Event::Damage { target, amount, .. } if *target == p => Some(*amount),
                _ => None,
            })
            .collect();
        v.iter().sum::<u32>() / v.len().max(1) as u32
    };
    let (clean, indebted) = (
        hits(only("technical_debt"), 0),
        hits(only("technical_debt"), 3),
    );
    assert!(
        indebted > clean,
        "3 deaths of debt must raise enemy damage: {clean} -> {indebted}"
    );
}

#[test]
fn hotfix_friday_heals_each_enemy_once() {
    let mut c = only("hotfix_friday");
    c.config.regen.hp_pct = 0;
    let mut s = S::new(c, 10);
    s.start_in("tower", 1);
    let u = s.hostiles()[0].clone();
    assert!(s.sim.debug_set_hp(u.id, u.max_hp / 5));
    s.secs(1);
    let hp = s.sim.unit(u.id).map_or(0, |x| x.hp);
    assert_eq!(hp, u.max_hp / 5 + u.max_hp * 30 / 100);
    assert!(s.sim.debug_set_hp(u.id, u.max_hp / 5));
    s.secs(1);
    assert_eq!(s.sim.unit(u.id).map_or(0, |x| x.hp), u.max_hp / 5);
    assert_eq!(s.affix_events("hotfix_friday"), 1);
}

#[test]
fn budget_freeze_pays_loot_not_story_points() {
    let mut s = S::new(only("budget_freeze"), 10);
    s.start_in("lab", 1);
    let ids: Vec<EntityId> = s.hostiles().iter().map(|u| u.id).collect();
    for id in ids {
        s.sim.debug_kill(id);
    }
    s.tick();
    let sp: u32 = s
        .log
        .iter()
        .filter_map(|e| match e {
            Event::LootDropped { sp, .. } => Some(*sp),
            _ => None,
        })
        .sum();
    assert_eq!(sp, 0);
    s.secs(5);
    s.clear_stage();
    s.secs(2);
    let chest = s.log.iter().find_map(|e| match e {
        Event::SprintEnded { summary, .. } => Some(summary.rewards[0].clone()),
        _ => None,
    });
    let Some(chest) = chest else { panic!("no end") };
    assert_eq!(chest.sp, 0);
    assert_eq!(chest.items.len(), 2, "one extra piece for the freeze");
}

#[test]
fn burnout_shaves_max_hp_until_an_elite_dies() {
    let mut s = S::new(only("burnout"), 10);
    s.start_in("tower", 1);
    let full = s.sim.unit(s.p).map_or(0, |u| u.max_hp);
    s.secs(121);
    let me = s.sim.unit(s.p).cloned();
    let Some(me) = me else { panic!() };
    let stacks = me
        .auras
        .iter()
        .find(|a| a.key == "burnout")
        .map_or(0, |a| a.stacks);
    assert_eq!(stacks, 2);
    assert_eq!(me.max_hp, full * 98 / 100);
    let boss = s.hostiles().into_iter().find(|u| u.boss).map(|u| u.id);
    assert!(boss.is_some_and(|b| s.sim.debug_kill(b)));
    s.tick();
    let me = s.sim.unit(s.p).cloned();
    assert!(me.is_some_and(|u| u.max_hp == full && !u.auras.iter().any(|a| a.key == "burnout")));
}

#[test]
fn pair_programming_splits_damage_with_the_partner() {
    let mut s = S::new(only("pair_programming"), 10);
    s.start_in("tower", 2);
    let paired: Vec<combat_core::UnitState> = s
        .hostiles()
        .into_iter()
        .filter(|u| u.auras.iter().any(|a| a.key == "pair_programming"))
        .collect();
    assert!(paired.len() >= 4, "two per pack");
    let a = paired[0].clone();
    let b = paired
        .iter()
        .find(|u| u.pack == a.pack && u.id != a.id)
        .cloned();
    let Some(b) = b else { panic!("no partner") };
    s.walk_to(Vec3::new(a.pos.x, 0.0, a.pos.z + 25.0));
    s.sim.push(Command::SetTarget {
        unit: s.p,
        target: Some(a.id),
    });
    s.sim.push(Command::Cast {
        caster: s.p,
        spell: CR,
    });
    s.secs(3);
    let hits = |id: EntityId| {
        s.log
            .iter()
            .filter_map(|e| match e {
                Event::Damage { target, amount, .. } if *target == id => Some(*amount),
                _ => None,
            })
            .sum::<u32>()
    };
    let (ha, hb) = (hits(a.id), hits(b.id));
    assert!(ha > 0 && hb > 0, "{ha} {hb}");
    assert!(
        ha.abs_diff(hb) <= ha / 4 + 2,
        "split about evenly: {ha} vs {hb}"
    );
}

#[test]
fn legacy_code_is_invulnerable_until_interrupted() {
    let mut s = S::new(only("legacy_code"), 10);
    s.start_in("tower", 1);
    let mono = s
        .hostiles()
        .into_iter()
        .find(|u| u.key == "legacy_monolith");
    let Some(mono) = mono else {
        panic!("no Do Not Touch elite")
    };
    assert!(mono.auras.iter().any(|a| a.key == "do_not_touch"));
    s.walk_to(Vec3::new(mono.pos.x, 0.0, mono.pos.z + 6.0));
    s.sim.push(Command::SetTarget {
        unit: s.p,
        target: Some(mono.id),
    });
    s.sim.push(Command::StartAutoAttack { unit: s.p });
    s.secs(2);
    assert_eq!(
        s.sim.unit(mono.id).map(|u| u.hp),
        Some(mono.max_hp),
        "untouchable"
    );
    let mut kicked = false;
    for _ in 0..12 * TICKS_PER_S {
        s.tick();
        let casting = s.sim.unit(mono.id).is_some_and(|u| u.cast.is_some());
        if casting {
            s.sim.push(Command::SetTarget {
                unit: s.p,
                target: Some(mono.id),
            });
            s.sim.push(Command::Cast {
                caster: s.p,
                spell: RPR,
            });
            s.tick();
            kicked = true;
            break;
        }
    }
    assert!(kicked, "it never cast");
    s.secs(1);
    let m = s.sim.unit(mono.id).cloned();
    assert!(m.is_some_and(
        |m| !m.auras.iter().any(|a| a.key == "do_not_touch") && m.ai != Some(AiState::Dead)
    ));
    assert_eq!(s.affix_events("legacy_code"), 1);
    s.secs(4);
    assert!(s.sim.unit(mono.id).is_some_and(|u| u.hp < u.max_hp));
}

fn kill_drops(log: &[Event]) -> (usize, usize, u32) {
    log.iter().fold((0, 0, 0), |(n, i, sp), e| match e {
        Event::LootDropped { items, sp: s, .. } => (n + 1, i + items.len(), sp + s),
        _ => (n, i, sp),
    })
}

fn sp_of(s: &S) -> u32 {
    s.sim.unit(s.p).map_or(0, |u| u.sp)
}

#[test]
fn sprint_kills_leave_lootable_quest_free_corpses() {
    let c = no_affixes();
    let quest_items: Vec<String> = c
        .units
        .iter()
        .flat_map(|u| u.loot.iter())
        .filter(|l| l.quest.is_some())
        .map(|l| l.item.clone())
        .collect();
    let mut s = S::new(c, 10);
    s.start_in("tower", 5);
    let trash: Vec<EntityId> = s
        .hostiles()
        .iter()
        .filter(|u| !u.boss)
        .map(|u| u.id)
        .collect();
    assert!(!trash.is_empty());
    for id in trash {
        s.sim.debug_kill(id);
    }
    s.tick();
    let (n, _, sp) = kill_drops(&s.log);
    assert!(n > 0 && sp > 0, "kills drop during the run");
    for e in &s.log {
        if let Event::LootDropped { items, .. } = e {
            assert!(items.iter().all(|i| !quest_items.contains(i)), "{items:?}");
        }
    }
    let Some(corpse) = s
        .sim
        .units()
        .find(|u| !u.alive && u.lootable && u.hostile)
        .map(|u| (u.id, u.pos))
    else {
        panic!("no lootable corpse");
    };
    s.walk_to(corpse.1);
    let before = sp_of(&s);
    s.sim.push(Command::Loot {
        looter: s.p,
        corpse: corpse.0,
    });
    s.tick();
    assert!(sp_of(&s) > before, "the corpse paid out");
    assert!(
        s.log
            .iter()
            .any(|e| matches!(e, Event::Looted { corpse: c, .. } if *c == corpse.0))
    );
}

#[test]
fn unclaimed_kill_loot_follows_the_group_to_the_next_floor() {
    let mut s = S::new(no_affixes(), 10);
    // Co-op: every member keeps their own share.
    let mate = s
        .sim
        .spawn_player("fullstack", Vec3::new(2.0, 0.0, 20.0), 0.0);
    s.start_in("tower", 1);
    let mate_sp = |s: &S| s.sim.unit(mate).map_or(0, |u| u.sp);
    let before = sp_of(&s) + mate_sp(&s);
    s.clear_stage();
    let (_, _, dropped) = kill_drops(&s.log);
    assert!(dropped > 0);
    let first = s.zones()[0].clone();
    for _ in 0..10 * TICKS_PER_S {
        if s.sim.zone() != Some(first.as_str()) {
            break;
        }
        s.tick();
    }
    assert_ne!(s.sim.zone(), Some(first.as_str()));
    s.tick();
    assert!(mate_sp(&s) > 0 && sp_of(&s) > 0);
    assert_eq!(
        sp_of(&s) + mate_sp(&s) - before,
        dropped,
        "nothing left behind on the floor"
    );
}

/// Same seed with and without the sprint cut: kills pay at most `kill_loot_pct` of a
/// normal zone's Kudos and the chest still comes on top.
#[test]
fn kill_loot_is_trimmed_and_the_chest_still_pays() {
    let run = |pct: u32| {
        let mut c = no_affixes();
        c.sprint.kill_loot_pct = pct;
        let mut s = S::new(c, 10);
        s.start_in("tower", 5);
        for _ in s.zones() {
            s.clear_stage();
            s.secs(5);
        }
        let chest = s.log.iter().find_map(|e| match e {
            Event::SprintEnded { summary, .. } => Some(summary.rewards[0].clone()),
            _ => None,
        });
        let Some(chest) = chest else { panic!("no end") };
        (kill_drops(&s.log), chest)
    };
    let ((n, _, full), _) = run(100);
    let pct = content().sprint.kill_loot_pct;
    assert!(pct < 100, "sprint kills pay less than a zone");
    let ((m, items, cut), chest) = run(pct);
    assert_eq!(n, m);
    // Gear rolls fork the loot stream between the two runs, hence the slack.
    assert!(cut > 0 && cut * 100 <= full * (pct + 15), "{cut} vs {full}");
    assert!(items <= 2 * m, "{items} items from {m} kills");
    assert_eq!(chest.sp, 200);
    assert!(chest.items.len() >= 2);
}
