//! Zones (load, links, unlocks, persistence, graveyard) and quests (accept, progress, turn-in,
//! abandon, chains, loot objectives), plus the new boss mechanics they rely on.

mod common;

use combat_core::{
    AiAbility, AiTarget, Command, Content, EntityId, Event, Objective, QuestStatus, Sim, SpellId,
    TRASH_RESPAWN_MS, Vec3, ms_to_ticks,
};
use common::*;

const GRAVEYARD: Vec3 = Vec3 {
    x: 0.0,
    y: 0.0,
    z: 30.0,
};

/// NPCs off to the side, packs down a corridor toward -Z, the boss at the end.
fn anchor(name: &str) -> Option<(Vec3, f32)> {
    let num = |p: &str| name.strip_prefix(p).and_then(|n| n.parse::<f32>().ok());
    if let Some(n) = num("pack_") {
        return Some((Vec3::new(0.0, 0.0, -70.0 * n), 0.0));
    }
    if let Some(n) = num("npc_") {
        return Some((Vec3::new(40.0, 0.0, -15.0 * n), 0.0));
    }
    match name {
        "boss" => Some((Vec3::new(0.0, 0.0, -630.0), 0.0)),
        "graveyard" => Some((GRAVEYARD, 0.0)),
        _ => None,
    }
}

struct T {
    sim: Sim,
    p: EntityId,
    log: Vec<Event>,
}

fn t_with(level: u8, tweak: impl FnOnce(&mut Content)) -> T {
    let mut c = content();
    c.config.white_hit_pct = 100;
    c.config.spell_hit_pct = 100;
    c.config.regen.hp_pct = 0;
    for s in &mut c.spells {
        s.yells.clear();
    }
    let p = def_mut(&mut c, "senior_engineer");
    p.crit_pct = 0;
    tweak(&mut c);
    let mut sim = Sim::new(c, 3);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 20.0), 0.0);
    sim.set_player_level(level);
    T {
        sim,
        p,
        log: Vec::new(),
    }
}

/// Every spell one-shots; the player survives anything.
fn t(level: u8) -> T {
    t_with(level, |c| {
        let p = def_mut(c, "senior_engineer");
        p.spell_power = 100_000;
        p.max_hp = 100_000;
    })
}

impl T {
    fn tick(&mut self) {
        let ev = self.sim.tick();
        self.log.extend(ev);
    }

    fn run(&mut self, n: u64) {
        for _ in 0..n {
            self.tick();
        }
    }

    fn has(&self, f: impl Fn(&Event) -> bool) -> bool {
        self.log.iter().any(f)
    }

    fn load(&mut self, zone: &str) {
        if let Err(e) = self.sim.load_zone(zone, &anchor, Vec::new()) {
            panic!("load_zone {zone}: {e}");
        }
        self.tick();
    }

    fn move_to(&mut self, pos: Vec3, yaw: f32) {
        self.sim.push(Command::PlayerMoved {
            pos,
            yaw,
            moving: false,
            jumping: false,
        });
        self.tick();
    }

    fn id_of(&self, key: &str) -> EntityId {
        match self.sim.units().find(|u| u.key == key && u.alive) {
            Some(u) => u.id,
            None => panic!("no living {key}"),
        }
    }

    /// Steps 10 m south of the unit, facing it, and Code Reviews it to death.
    fn kill(&mut self, id: EntityId) {
        let Some(at) = self.sim.unit(id).map(|u| u.pos) else {
            panic!("no unit {id}")
        };
        self.move_to(Vec3::new(at.x, 0.0, at.z + 10.0), 0.0);
        self.sim.push(Command::SetTarget {
            unit: self.p,
            target: Some(id),
        });
        self.sim.push(Command::Cast {
            caster: self.p,
            spell: CR,
        });
        for _ in 0..200 {
            self.tick();
            if self.sim.unit(id).is_some_and(|u| !u.alive) {
                return;
            }
        }
        panic!("unit {id} did not die");
    }

    fn talk_to(&mut self, key: &str) {
        let id = self.id_of(key);
        let Some(at) = self.sim.unit(id).map(|u| u.pos) else {
            return;
        };
        self.move_to(Vec3::new(at.x, 0.0, at.z + 2.0), 0.0);
        self.sim.push(Command::Talk { unit: id });
        self.tick();
    }

    fn quest_cmd(&mut self, cmd: Command) {
        self.sim.push(cmd);
        self.tick();
    }

    fn accept(&mut self, q: &str) {
        self.quest_cmd(Command::AcceptQuest {
            quest: q.to_string(),
        });
    }

    fn turn_in(&mut self, q: &str) {
        self.quest_cmd(Command::TurnInQuest {
            quest: q.to_string(),
        });
    }
}

fn links(sim: &Sim, zone: &str) -> Vec<(String, bool)> {
    sim.zone_links(zone)
        .into_iter()
        .map(|l| (l.to, l.unlocked))
        .collect()
}

// ---- zones ----

#[test]
fn every_zone_has_an_ungated_way_back_to_the_campus() {
    let sim = t(1).sim;
    for z in &sim.content().zones {
        let mut seen = vec![z.id.clone()];
        let mut i = 0;
        while i < seen.len() {
            for l in sim.zone_links(&seen[i]) {
                if l.unlocked && !seen.contains(&l.to) {
                    seen.push(l.to);
                }
            }
            i += 1;
        }
        assert!(
            seen.iter().any(|s| s == "campus"),
            "{} strands the player",
            z.id
        );
    }
}

#[test]
fn load_zone_spawns_the_campus_and_reports_its_links() {
    let mut t = t(3);
    assert!(t.sim.load_zone("nowhere", &anchor, Vec::new()).is_err());
    let no_npc4 = |a: &str| if a == "npc_4" { None } else { anchor(a) };
    assert!(t.sim.load_zone("campus", &no_npc4, Vec::new()).is_err());
    assert_eq!(t.sim.units().count(), 1, "a failed load changes nothing");

    t.load("campus");
    assert_eq!(t.sim.zone(), Some("campus"));
    assert_eq!(t.sim.floor(), None);
    assert!(t.has(|e| matches!(e, Event::ZoneLoaded { zone } if zone == "campus")));
    assert!(!t.has(|e| matches!(e, Event::FloorLoaded { .. })));
    let hostiles = t.sim.units().filter(|u| u.hostile).count();
    assert_eq!(hostiles, 15, "8 packs, no boss");
    for key in ["intern", "facilities_guy", "barista", "smoking_corner_dev"] {
        assert!(t.sim.units().any(|u| u.key == key && !u.hostile), "{key}");
    }
    assert_eq!(
        links(&t.sim, "campus"),
        [
            ("tower/1".to_string(), true),
            ("datacenter/1".to_string(), false),
            ("lab/1".to_string(), false),
        ]
    );
    assert!(t.sim.zone_links("nowhere").is_empty());
}

#[test]
fn clearing_tower_3_unlocks_the_data_center_and_the_next_floor() {
    let mut t = t(10);
    t.load("tower/3");
    assert_eq!(t.sim.floor(), Some(3));
    assert!(t.has(|e| matches!(e, Event::FloorLoaded { floor: 3 })));
    let boss = t.id_of("customer");
    t.kill(boss);
    t.tick();
    let pos = |t: &T, f: &dyn Fn(&Event) -> bool| t.log.iter().position(f);
    let cleared = pos(
        &t,
        &|e| matches!(e, Event::ZoneCleared { zone } if zone == "tower/3"),
    );
    let floor = pos(&t, &|e| matches!(e, Event::FloorCleared { floor: 3 }));
    let dc = pos(
        &t,
        &|e| matches!(e, Event::Unlocked { zone } if zone == "datacenter/1"),
    );
    let next = pos(
        &t,
        &|e| matches!(e, Event::Unlocked { zone } if zone == "tower/4"),
    );
    let loot = pos(
        &t,
        &|e| matches!(e, Event::LootDropped { corpse, .. } if *corpse == boss),
    );
    assert!(loot < cleared && cleared < floor && floor < dc && floor < next);
    assert!(dc.is_some() && next.is_some());
    assert!(t.sim.zone_cleared("tower/3"));
    assert_eq!(
        links(&t.sim, "campus")[1],
        ("datacenter/1".to_string(), true)
    );
    assert_eq!(
        links(&t.sim, "tower/3"),
        [("campus".to_string(), true), ("tower/4".to_string(), true)]
    );
}

#[test]
fn killing_the_ceo_unlocks_the_lab_once() {
    let mut t = t(10);
    t.load("tower/5");
    let ceo = t.id_of("ceo");
    t.kill(ceo);
    t.tick();
    let unlocks = |t: &T| {
        t.log
            .iter()
            .filter(|e| matches!(e, Event::Unlocked { zone } if zone == "lab/1"))
            .count()
    };
    assert_eq!(unlocks(&t), 1);
    // A hand-spawned CEO is not a zone boss and unlocks nothing more.
    assert!(
        t.sim
            .spawn_unit("ceo", Vec3::new(0.0, 0.0, -10.0), 0.0)
            .is_ok()
    );
    let again = t.id_of("ceo");
    t.kill(again);
    assert_eq!(unlocks(&t), 1);
}

#[test]
fn a_cleared_boss_stays_dead_and_trash_respawns_only_after_the_timer() {
    let mut t = t(10);
    t.load("tower/1");
    let trash = t.sim.units().filter(|u| u.hostile).count();
    let first = t.id_of("meatproxy");
    t.kill(first);
    let boss = t.id_of("vibe_coder");
    t.kill(boss);
    t.run(30);
    let alive = |t: &T| t.sim.units().filter(|u| u.hostile && u.alive).count();
    let left = alive(&t);

    // Back within the window: the killed trash is still gone, the boss too.
    t.load("campus");
    t.run(30);
    t.log.clear();
    t.load("tower/1");
    assert_eq!(alive(&t), left);
    assert!(!t.sim.units().any(|u| u.key == "vibe_coder"));

    // Leave for longer than the timer: the trash is back, the boss stays dead.
    t.load("campus");
    t.run(ms_to_ticks(TRASH_RESPAWN_MS));
    t.log.clear();
    t.load("tower/1");
    assert_eq!(alive(&t), trash - 1, "all trash, no boss");
    assert!(!t.sim.units().any(|u| u.key == "vibe_coder"));
    assert!(!t.has(|e| matches!(e, Event::ZoneCleared { .. })));
}

#[test]
fn the_player_respawns_at_the_graveyard_and_zone_state_survives_death() {
    let mut t = t_with(1, |c| def_mut(c, "senior_engineer").max_hp = 1);
    t.load("tower/1");
    let killed = t.sim.units().filter(|u| u.hostile).count();
    // Walk into a pack and die.
    let mp = t.id_of("meatproxy");
    let Some(at) = t.sim.unit(mp).map(|u| u.pos) else {
        panic!()
    };
    t.move_to(Vec3::new(at.x, 0.0, at.z + 1.0), 0.0);
    let mut died = false;
    for _ in 0..300 {
        t.tick();
        if t.sim.unit(t.p).is_some_and(|u| !u.alive) {
            died = true;
            break;
        }
    }
    assert!(died);
    t.sim.push(Command::RespawnPlayer);
    t.tick();
    let me = t.sim.unit(t.p).cloned();
    assert!(me.is_some_and(|u| u.alive && u.pos == GRAVEYARD));
    assert_eq!(t.sim.zone(), Some("tower/1"));
    assert_eq!(t.sim.units().filter(|u| u.hostile).count(), killed);
}

// ---- quests ----

#[test]
fn onboarding_accept_progress_ready_turn_in() {
    let mut t = t_with(1, |_| {});
    t.load("campus");
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Available);
    assert_eq!(
        t.sim.quest_status("badge_access_denied"),
        QuestStatus::Locked
    );
    assert_eq!(t.sim.quest_status("code_freeze"), QuestStatus::Locked);

    // Too far from the intern: ignored.
    t.accept("onboarding");
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Available);

    t.talk_to("intern");
    assert!(
        t.has(|e| matches!(e, Event::QuestOffered { quests, .. } if quests == &["onboarding"]))
    );
    t.accept("onboarding");
    assert!(t.has(|e| matches!(e, Event::QuestAccepted { quest } if quest == "onboarding")));
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Active);
    // Turning in early does nothing.
    t.turn_in("onboarding");
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Active);

    t.talk_to("facilities_guy");
    assert!(t.has(|e| matches!(
        e,
        Event::QuestProgress { quest, objective: 0, count: 1, need: 1 } if quest == "onboarding"
    )));
    assert!(t.has(|e| matches!(e, Event::QuestReady { quest } if quest == "onboarding")));
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Ready);
    // Ready quests are turned in at the giver, not here.
    t.turn_in("onboarding");
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Ready);

    t.log.clear();
    t.talk_to("intern");
    assert!(
        t.has(|e| matches!(e, Event::QuestOffered { quests, .. } if quests == &["onboarding"]))
    );
    t.turn_in("onboarding");
    let done = t
        .log
        .iter()
        .position(|e| matches!(e, Event::QuestCompleted { quest } if quest == "onboarding"));
    let xp = t
        .log
        .iter()
        .position(|e| matches!(e, Event::XpGained { amount: 40, .. }));
    assert!(done.is_some() && done < xp);
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Done);
    assert_eq!(t.sim.quests().completed, ["onboarding"]);
    assert!(t.sim.quests().active.is_empty());
    assert_eq!(t.sim.unit(t.p).map(|u| u.xp), Some(40));
    // The chain opens; level-gated quests stay locked.
    assert_eq!(
        t.sim.quest_status("badge_access_denied"),
        QuestStatus::Available
    );
    assert_eq!(t.sim.quest_status("code_freeze"), QuestStatus::Locked);
    t.sim.set_player_level(2);
    assert_eq!(t.sim.quest_status("code_freeze"), QuestStatus::Available);
}

#[test]
fn abandon_drops_progress_and_the_quest_can_be_taken_again() {
    let mut t = t(5);
    t.load("campus");
    t.talk_to("smoking_corner_dev");
    t.accept("content_moderation");
    let inf = t.id_of("linkedin_influencer");
    t.kill(inf);
    assert_eq!(t.sim.quests().active[0].counts, [1]);
    t.quest_cmd(Command::AbandonQuest {
        quest: "content_moderation".to_string(),
    });
    assert_eq!(
        t.sim.quest_status("content_moderation"),
        QuestStatus::Available
    );
    t.talk_to("smoking_corner_dev");
    t.accept("content_moderation");
    assert_eq!(t.sim.quests().active[0].counts, [0]);
}

#[test]
fn kill_credit_counts_only_matching_units_and_caps_at_need() {
    let mut t = t(5);
    t.load("campus");
    t.talk_to("facilities_guy");
    // Fire Exit Strategy needs onboarding first.
    t.accept("fire_exit_strategy");
    assert!(t.sim.quests().active.is_empty());
    t.talk_to("intern");
    t.accept("onboarding");
    t.talk_to("facilities_guy");
    t.talk_to("intern");
    t.turn_in("onboarding");
    t.talk_to("facilities_guy");
    t.accept("fire_exit_strategy");
    t.log.clear();
    let robot = t.id_of("delivery_robot");
    t.kill(robot);
    assert!(!t.has(|e| matches!(e, Event::QuestProgress { .. })));
    loop {
        let next = t
            .sim
            .units()
            .find(|u| u.key == "scooter_exec" && u.alive)
            .map(|u| u.id);
        let Some(s) = next else { break };
        t.kill(s);
    }
    let progress = t
        .log
        .iter()
        .filter(|e| matches!(e, Event::QuestProgress { .. }))
        .count();
    assert_eq!(progress, 3, "three scooters in the campus, need 3");
    assert_eq!(t.sim.quest_status("fire_exit_strategy"), QuestStatus::Ready);
    let readies = t
        .log
        .iter()
        .filter(|e| matches!(e, Event::QuestReady { .. }))
        .count();
    assert_eq!(readies, 1);
}

#[test]
fn loot_objectives_count_carried_copies_and_new_pickups() {
    let mut t = t_with(5, |c| {
        let p = def_mut(c, "senior_engineer");
        p.spell_power = 100_000;
        p.max_hp = 100_000;
        for l in &mut def_mut(c, "unpaid_ambassador").loot {
            l.chance_pct = 100;
        }
    });
    t.load("campus");
    assert!(t.sim.grant_trophy("Oat Milk Prompt"));
    t.talk_to("barista");
    t.accept("coffee_run");
    assert_eq!(t.sim.quests().active[0].counts, [1]);
    let amb = t.id_of("unpaid_ambassador");
    t.kill(amb);
    let Some(at) = t.sim.unit(amb).map(|u| u.pos) else {
        panic!()
    };
    t.move_to(Vec3::new(at.x, 0.0, at.z + 1.0), 0.0);
    t.sim.push(Command::Loot {
        looter: t.p,
        corpse: amb,
    });
    t.tick();
    let looted = t.log.iter().position(|e| matches!(e, Event::Looted { .. }));
    let progress = t.log.iter().position(|e| {
        matches!(e, Event::QuestProgress { quest, count: 2, need: 3, .. } if quest == "coffee_run")
    });
    assert!(looted.is_some() && looted < progress);
}

#[test]
fn clear_zone_objectives_count_a_zone_cleared_before_accepting() {
    let mut t = t(5);
    t.load("tower/1");
    let boss = t.id_of("vibe_coder");
    t.kill(boss);
    t.load("campus");
    t.talk_to("intern");
    t.accept("onboarding");
    t.talk_to("facilities_guy");
    t.talk_to("intern");
    t.turn_in("onboarding");
    t.log.clear();
    t.accept("code_freeze");
    assert!(t.has(|e| matches!(e, Event::QuestReady { quest } if quest == "code_freeze")));
    t.turn_in("code_freeze");
    assert_eq!(t.sim.quest_status("code_freeze"), QuestStatus::Done);
}

#[test]
fn quest_content_is_validated() {
    let bad = SRC.replace(
        "objectives: [Kill(\"gpu_cultist\", 6)]",
        "objectives: [Kill(\"intern\", 6)]",
    );
    assert!(Content::from_ron(&bad).is_err(), "kill a friendly");
    let bad = SRC.replace(
        "(anchor: \"door:lab/2\", to: \"lab/2\")",
        "(anchor: \"door:lab/2\", to: \"lab/9\")",
    );
    assert!(Content::from_ron(&bad).is_err(), "link to a missing zone");
    let bad = SRC.replace(
        "requires: Some(BossKilled(\"ceo\"))",
        "requires: Some(BossKilled(\"intern\"))",
    );
    assert!(Content::from_ron(&bad).is_err(), "unlock by a non-boss");
    let c = content();
    assert!(c.quests.len() >= 12);
    let objectives = c.quests.iter().flat_map(|q| q.objectives.iter());
    assert!(objectives.clone().any(|o| matches!(o, Objective::Loot(..))));
    assert!(objectives.clone().any(|o| matches!(o, Objective::Talk(..))));
    assert!(
        objectives
            .clone()
            .any(|o| matches!(o, Objective::ClearZone(..)))
    );
}

#[test]
fn a_quest_run_is_deterministic() {
    let run = || {
        let mut t = t(5);
        t.load("campus");
        t.talk_to("smoking_corner_dev");
        t.accept("content_moderation");
        t.talk_to("barista");
        t.accept("coffee_run");
        for k in 1..=8 {
            let Some((at, _)) = anchor(&format!("pack_{k}")) else {
                continue;
            };
            let ids: Vec<EntityId> = t
                .sim
                .units()
                .filter(|u| u.hostile && u.alive && combat_core::distance(u.pos, at) < 6.0)
                .map(|u| u.id)
                .collect();
            for id in ids {
                if t.sim.unit(id).is_some_and(|u| u.alive) {
                    t.kill(id);
                }
            }
        }
        format!("{:?}", t.log)
    };
    assert_eq!(run(), run());
}

// ---- new mechanics ----

/// Strips a unit down to one ability that fires 100 ms after the pull.
fn only_ability(c: &mut Content, key: &str, spell: u32, target: AiTarget) {
    if let Some(ai) = def_mut(c, key).ai.as_mut() {
        ai.abilities = vec![AiAbility {
            spell: SpellId(spell),
            cooldown_min_ms: 60_000,
            cooldown_max_ms: 60_000,
            initial_ms: Some((100, 100)),
            target,
            phases: Vec::new(),
            yells: Vec::new(),
        }];
        ai.phases.clear();
        ai.melee_damage = 0;
    }
}

#[test]
fn alignment_training_reflects_spells_but_not_auto_attacks() {
    let mut t = t_with(10, |c| {
        def_mut(c, "senior_engineer").max_hp = 100_000;
        only_ability(c, "agi_prototype", 187, AiTarget::Caster);
    });
    let Ok(agi) = t
        .sim
        .spawn_unit("agi_prototype", Vec3::new(0.0, 0.0, 0.0), 0.0)
    else {
        panic!("spawn")
    };
    t.move_to(Vec3::new(0.0, 0.0, 15.0), 0.0);
    t.sim.push(Command::SetTarget {
        unit: t.p,
        target: Some(agi),
    });
    t.sim.push(Command::StartAutoAttack { unit: t.p });
    let mut reflecting = false;
    for _ in 0..120 {
        t.tick();
        if t.sim
            .unit(agi)
            .is_some_and(|u| u.auras.iter().any(|a| a.key == "alignment_training"))
        {
            reflecting = true;
            break;
        }
    }
    assert!(reflecting, "the telegraphed cast ends in the reflect aura");
    t.log.clear();
    t.sim.push(Command::Cast {
        caster: t.p,
        spell: CR,
    });
    t.run(80);
    assert!(t.has(|e| matches!(
        e,
        Event::Damage { source, target, spell: Some(s), amount, .. }
            if *source == agi && *target == t.p && *s == CR && *amount > 0
    )));
    assert!(!t.has(|e| matches!(
        e,
        Event::Damage { target, spell: Some(_), .. } if *target == agi
    )));
    assert!(t.has(|e| matches!(
        e,
        Event::Damage { source, target, spell: None, .. } if *source == t.p && *target == agi
    )));
}

#[test]
fn context_window_exceeded_drains_mana() {
    let mut t = t_with(7, |c| {
        def_mut(c, "senior_engineer").mana_regen_per_5s_combat = 0;
        only_ability(c, "token_counter", 172, AiTarget::Current);
    });
    let Ok(tc) = t
        .sim
        .spawn_unit("token_counter", Vec3::new(0.0, 0.0, 0.0), 0.0)
    else {
        panic!("spawn")
    };
    t.move_to(Vec3::new(0.0, 0.0, 5.0), 0.0);
    let max = t.sim.unit(t.p).map_or(0, |u| u.max_mana);
    let before = t.sim.unit(t.p).map_or(0, |u| u.mana);
    assert!(t.sim.unit(tc).is_some_and(|u| u.in_combat));
    t.run(10);
    let after = t.sim.unit(t.p).map_or(0, |u| u.mana);
    assert_eq!(before - after, max * 15 / 100);
}

#[test]
fn debug_set_quest_forces_each_status_silently() {
    let mut t = t(1);
    t.load("campus");
    assert!(!t.sim.debug_set_quest("nope", QuestStatus::Done));
    assert!(
        t.sim
            .debug_set_quest("badge_access_denied", QuestStatus::Active)
    );
    assert_eq!(
        t.sim.quest_status("badge_access_denied"),
        QuestStatus::Active
    );
    assert!(
        t.sim
            .debug_set_quest("badge_access_denied", QuestStatus::Ready)
    );
    assert_eq!(
        t.sim.quest_status("badge_access_denied"),
        QuestStatus::Ready
    );
    assert!(t.sim.debug_set_quest("onboarding", QuestStatus::Done));
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Done);
    assert_eq!(t.sim.quests().active.len(), 1);
    assert!(
        t.sim
            .debug_set_quest("badge_access_denied", QuestStatus::Available)
    );
    assert_eq!(
        t.sim.quest_status("badge_access_denied"),
        QuestStatus::Available
    );
    t.tick();
    assert!(!t.has(|e| matches!(
        e,
        Event::QuestAccepted { .. } | Event::QuestCompleted { .. }
    )));
}

#[test]
fn debug_kill_counts_as_a_player_kill() {
    let mut t = t(10);
    t.load("lab/2");
    let boss = t.id_of("agi_prototype");
    assert!(!t.sim.debug_kill(t.p), "the player is not hostile");
    assert!(t.sim.debug_kill(boss));
    assert!(!t.sim.debug_kill(boss), "already dead");
    t.tick();
    assert!(t.has(
        |e| matches!(e, Event::Death { unit, killer } if *unit == boss && *killer == Some(t.p))
    ));
    assert!(t.sim.zone_cleared("lab/2"));
}
