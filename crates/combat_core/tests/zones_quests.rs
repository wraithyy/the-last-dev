//! Zones (load, links, unlocks, persistence, graveyard) and quests (accept, progress, turn-in,
//! abandon, chains, loot objectives), plus the new boss mechanics they rely on.

mod common;

use combat_core::{
    AiAbility, AiTarget, Command, Content, EntityId, Event, Objective, QuestStatus, Role, Sim,
    SpellId, TRASH_RESPAWN_MS, Vec3, ms_to_ticks,
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
    // The campus onboarding corner (named, not numbered), far out of every pull.
    if name.starts_with("npc_") {
        return Some((Vec3::new(-400.0, 0.0, 400.0), 0.0));
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

    /// New hires get the onboarding ticket from the intern in Car Park P3, then walk up to
    /// the campus.
    fn onboard(&mut self) {
        self.load("parking_p3");
        self.talk_to("intern");
        self.accept("onboarding");
        self.load("campus");
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

    /// Marks the chain of quests in front of `q` as done for the first player.
    fn finish_prerequisites(&mut self, q: &str) {
        let mut chain = Vec::new();
        let mut at = self.sim.content().quest(q).and_then(|d| d.requires.clone());
        while let Some(id) = at {
            at = self
                .sim
                .content()
                .quest(&id)
                .and_then(|d| d.requires.clone());
            chain.push(id);
        }
        for id in chain.into_iter().rev() {
            self.sim.debug_set_quest(&id, QuestStatus::Done);
        }
    }

    fn accept(&mut self, q: &str) {
        self.finish_prerequisites(q);
        self.quest_cmd(Command::AcceptQuest {
            quest: q.to_string(),
        });
    }

    /// Walks to a quest's separate turn-in NPC when they stand in the loaded zone (zone
    /// turn-ins wait at the far end, not at the giver), then hands it in.
    fn turn_in(&mut self, q: &str) {
        let ender = self
            .sim
            .content()
            .quest(q)
            .and_then(|d| d.turn_in.as_ref().map(|t| t.npc.clone()));
        if let Some(npc) = ender.filter(|k| self.sim.units().any(|u| u.key == *k && u.alive)) {
            self.talk_to(&npc);
        }
        self.quest_cmd(Command::TurnInQuest {
            quest: q.to_string(),
            choice: 0,
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
        // The Body Leasing office has no enemies and opens its only door with its last ticket;
        // that chain can always be finished, so nobody is stranded.
        if z.packs.is_empty() && z.boss.is_none() {
            continue;
        }
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
    // The rares need anchors this layout lacks; the district packs all spawn.
    let districts: usize = content().zone("campus").map_or(0, |z| {
        z.packs
            .iter()
            .filter(|p| p.level.is_some())
            .map(|p| p.units.len())
            .sum()
    });
    assert_eq!(
        hostiles,
        15 + districts,
        "8 packs, no boss, the districts (the onboarding dummy lives in Car Park P3)"
    );
    for key in ["intern", "facilities_guy", "barista", "smoking_corner_dev"] {
        assert!(t.sim.units().any(|u| u.key == key && !u.hostile), "{key}");
    }
    assert_eq!(
        links(&t.sim, "campus"),
        [
            ("parking_p3".to_string(), true),
            ("tower/1".to_string(), true),
            ("datacenter/1".to_string(), false),
            ("lab/1".to_string(), false),
            ("client/1".to_string(), false),
            ("offsite/1".to_string(), false),
            ("vibecon/1".to_string(), false),
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
        links(&t.sim, "campus")[2],
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
    assert_eq!(t.sim.quest_status("escalation_path"), QuestStatus::Locked);
    assert_eq!(t.sim.quest_status("mandatory_fun"), QuestStatus::Locked);

    // Too far from the intern: ignored.
    t.accept("onboarding");
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Available);
    // The campus intern has nothing to give a new hire: the ticket comes from P3.
    t.talk_to("intern");
    t.accept("onboarding");
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Available);

    t.load("parking_p3");
    t.talk_to("intern");
    assert!(
        t.has(|e| matches!(e, Event::QuestOffered { quests, .. } if quests.iter().any(|q| q == "onboarding")))
    );
    t.accept("onboarding");
    assert!(t.has(|e| matches!(e, Event::QuestAccepted { quest } if quest == "onboarding")));
    t.load("campus");
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
    // Ready quests are turned in at the intern, not here.
    t.quest_cmd(Command::TurnInQuest {
        quest: "onboarding".into(),
        choice: 0,
    });
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
        .position(|e| matches!(e, Event::XpGained { amount: 60, .. }));
    assert!(done.is_some() && done < xp);
    assert_eq!(t.sim.quest_status("onboarding"), QuestStatus::Done);
    assert_eq!(t.sim.quests().completed, ["onboarding"]);
    assert!(t.sim.quests().active.is_empty());
    assert_eq!(t.sim.unit(t.p).map(|u| u.xp), Some(60));
    // The chain opens; level-gated quests stay locked.
    assert_eq!(
        t.sim.quest_status("badge_access_denied"),
        QuestStatus::Available
    );
    assert_eq!(
        t.sim.quest_status("escalation_path"),
        QuestStatus::Available
    );
    assert_eq!(t.sim.quest_status("mandatory_fun"), QuestStatus::Locked);
    t.sim.set_player_level(2);
    assert_eq!(t.sim.quest_status("mandatory_fun"), QuestStatus::Available);
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
    // Progress is gone with the quest, but the kill itself is remembered (retro credit).
    assert_eq!(t.sim.quests().active[0].counts, [1]);
    assert_eq!(t.sim.quests().kills.get("linkedin_influencer"), Some(&1));
}

#[test]
fn kill_credit_counts_only_matching_units_and_caps_at_need() {
    let mut t = t(5);
    t.load("campus");
    t.talk_to("facilities_guy");
    // Fire Exit Strategy needs onboarding first (so no prerequisite shortcut here).
    t.quest_cmd(Command::AcceptQuest {
        quest: "fire_exit_strategy".into(),
    });
    assert!(t.sim.quests().active.is_empty());
    t.onboard();
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
    t.load("tower/1");
    t.talk_to("helpdesk_lead");
    t.log.clear();
    t.accept("code_freeze");
    assert!(t.has(|e| matches!(e, Event::QuestReady { quest } if quest == "code_freeze")));
    t.turn_in("code_freeze");
    assert_eq!(t.sim.quest_status("code_freeze"), QuestStatus::Done);
}

#[test]
fn quest_content_is_validated() {
    let bad = SRC.replace(
        "objectives: [Kill(\"gpu_cultist\", 4),",
        "objectives: [Kill(\"intern\", 4),",
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

/// A boss fight next to a quest NPC (the playtest: "the CEO killed the woman I hand tickets in
/// to"): enemy AoE must leave anyone a quest depends on untouched, and still hurt a bystander
/// no quest cares about.
#[test]
fn enemy_aoe_never_kills_a_quest_npc() {
    let mut t = t_with(1, |c| {
        def_mut(c, "senior_engineer").max_hp = 100_000;
        def_mut(c, "helpdesk_lead").max_hp = 1;
        def_mut(c, "meatproxy").spell_power = 5_000;
        // Responsive Layout: damages everything on the other side within 6 m of the caster.
        only_ability(c, "meatproxy", 26, AiTarget::Caster);
    });
    t.load("tower/1");
    assert!(content().is_quest_npc("helpdesk_lead"));
    let lead = t.id_of("helpdesk_lead");
    let Some(at) = t.sim.unit(lead).map(|u| u.pos) else {
        panic!("no helpdesk lead")
    };
    let Ok(boss) = t
        .sim
        .spawn_unit("meatproxy", Vec3::new(at.x + 1.0, 0.0, at.z), 0.0)
    else {
        panic!("spawn")
    };
    t.move_to(Vec3::new(at.x + 4.0, 0.0, at.z), 0.0);
    t.sim.push(Command::SetTarget {
        unit: t.p,
        target: Some(boss),
    });
    t.sim.push(Command::StartAutoAttack { unit: t.p });
    let mut cast = false;
    for _ in 0..200 {
        t.tick();
        cast |= t.has(|e| matches!(e, Event::Damage { target, .. } if *target == t.p));
    }
    assert!(
        cast,
        "the AoE must have landed on the player for this test to mean anything"
    );
    let Some(lead) = t.sim.unit(lead) else {
        panic!("lead gone")
    };
    assert!(
        lead.alive && lead.hp == lead.max_hp,
        "quest NPC was hurt: {} {}/{}",
        lead.alive,
        lead.hp,
        lead.max_hp
    );
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

// ---- quest placement, levels, retro credit, quest drops, rewards ----

/// "datacenter/2" -> "datacenter": halls of one building share the giver at its entrance.
fn area(zone: &str) -> &str {
    zone.split('/').next().unwrap_or(zone)
}

/// Campaign order of the zones (`playthrough.rs` WORLD_ORDER); 0 for zones off the path.
fn campaign_rank(zone: &str) -> usize {
    const ORDER: [&str; 11] = [
        "parking_p3",
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
    ORDER.iter().position(|z| *z == zone).map_or(0, |k| k + 1)
}

#[test]
fn every_quest_is_given_where_its_work_happens() {
    let c = content();
    assert!(c.quests.len() >= 40, "{} quests", c.quests.len());
    for q in &c.quests {
        if q.is_breadcrumb() {
            // A breadcrumb points at the next zone's giver, who also takes it.
            let to = &q.ender().zone;
            // Open-world breadcrumbs lead from one campus district's giver to the next.
            if !q.open_world {
                assert_ne!(*to, q.giver.zone, "{}: breadcrumb to its own zone", q.id);
            }
            for o in &q.objectives {
                assert_eq!(c.objective_zones(o), [to.as_str()], "{}", q.id);
            }
            continue;
        }
        // Work is handed in where it ends: the giver's zone, or a later one on the campaign
        // path (the story steps and timed deliveries carried forward).
        let ender = q.ender().zone.as_str();
        assert!(
            ender == q.giver.zone || campaign_rank(ender) > campaign_rank(&q.giver.zone),
            "{}: turned in at {ender}, given in {}",
            q.id,
            q.giver.zone
        );
        // Company Calendar chains are side trips from the campus: their work may sit in any
        // zone open from level 1 (the Open Space).
        let calendar = c.calendar.chains.iter().any(|ch| ch.quests.contains(&q.id));
        for o in &q.objectives {
            let zones = c.objective_zones(o);
            if calendar {
                assert!(
                    zones.iter().any(|z| ["campus", "tower/1"].contains(z)),
                    "{}: {o:?} happens in {zones:?}",
                    q.id
                );
                continue;
            }
            let here = zones.iter().any(|z| {
                *z == q.giver.zone
                    || *z == ender
                    || area(z) == area(&q.giver.zone) && z != &"campus"
            });
            assert!(
                here,
                "{}: {o:?} happens in {zones:?}, given in {}",
                q.id, q.giver.zone
            );
        }
    }
    // Every zone with packs has local work, and each zone's givers stand in it. The raid is
    // weekly endgame content above the cap: its work is the lockout, not tickets; a minigame's
    // venue has its own objectives.
    let minigame_zone = |id: &str| c.minigames.iter().any(|m| m.zone == id);
    for z in c
        .zones
        .iter()
        .filter(|z| c.raid_of(&z.id).is_none() && !minigame_zone(&z.id))
    {
        let n = c
            .quests
            .iter()
            .filter(|q| !q.is_breadcrumb())
            .filter(|q| {
                q.objectives
                    .iter()
                    .any(|o| c.objective_zones(o).contains(&z.id.as_str()))
            })
            .count();
        assert!(n >= 3, "{}: only {n} quests work there", z.id);
    }
}

#[test]
fn quest_levels_fit_their_zone() {
    let c = content();
    for q in &c.quests {
        if q.open_world {
            continue;
        }
        let zone = if q.is_breadcrumb() {
            &q.ender().zone
        } else {
            &q.giver.zone
        };
        let Some((lo, hi)) = c.zone(zone).map(|z| z.level_range) else {
            panic!("{}: no zone {zone}", q.id)
        };
        // Available on arrival (a level of slack for the walk), never above the zone.
        let top = if q.is_breadcrumb() { lo } else { hi };
        assert!(
            q.level_min + 1 >= lo && q.level_min <= top,
            "{}: level {} for {zone} ({lo}-{hi})",
            q.id,
            q.level_min
        );
    }
}

#[test]
fn kills_before_accepting_count_and_reach_remote_logs_as_progress() {
    let mut t = t(5);
    t.load("tower/1");
    let proxies: Vec<EntityId> = t
        .sim
        .units()
        .filter(|u| u.key == "meatproxy")
        .map(|u| u.id)
        .collect();
    assert_eq!(proxies.len(), 3);
    for id in proxies {
        assert!(t.sim.debug_kill(id));
    }
    t.tick();
    t.talk_to("helpdesk_lead");
    t.log.clear();
    t.accept("secrets_in_the_repo");
    // The keys still have to be looted, but the three kills are credited on accept.
    assert_eq!(
        t.sim.quest_status("secrets_in_the_repo"),
        QuestStatus::Active
    );
    assert_eq!(t.sim.quests().active[0].counts, [3, 0]);
    let accepted = t.log.iter().position(
        |e| matches!(e, Event::QuestAccepted { quest } if quest == "secrets_in_the_repo"),
    );
    let progress = t.log.iter().position(|e| {
        matches!(e, Event::QuestProgress { quest, count: 3, need: 3, objective: 0, .. } if quest == "secrets_in_the_repo")
    });
    assert!(accepted.is_some() && accepted < progress, "{:?}", t.log);
}

#[test]
fn quest_items_drop_only_while_the_quest_wants_them() {
    let mut t = t_with(5, |c| {
        let p = def_mut(c, "senior_engineer");
        p.spell_power = 100_000;
        p.max_hp = 100_000;
        for l in &mut def_mut(c, "vibe_junior").loot {
            if l.quest.is_some() {
                l.chance_pct = 100;
            }
        }
    });
    t.load("tower/1");
    let drops = |t: &T| {
        t.log.iter().any(|e| {
            matches!(e, Event::LootDropped { items, .. } if items.iter().any(|i| i == "Hardcoded API Key"))
        })
    };
    let juniors: Vec<EntityId> = t
        .sim
        .units()
        .filter(|u| u.key == "vibe_junior")
        .map(|u| u.id)
        .collect();
    assert!(t.sim.debug_kill(juniors[0]));
    t.tick();
    assert!(!drops(&t), "no quest, no key");
    t.talk_to("helpdesk_lead");
    t.accept("secrets_in_the_repo");
    t.log.clear();
    assert!(t.sim.debug_kill(juniors[1]));
    t.tick();
    assert!(drops(&t), "on the quest the key drops");
}

#[test]
fn gear_rewards_offer_a_role_choice_and_the_pick_lands_in_the_bag() {
    let c = content();
    for q in c.quests.iter().filter(|q| q.reward.gear_tier > 0) {
        for role in [Role::Tank, Role::Healer, Role::Melee, Role::Ranged] {
            let choices = c.quest_gear_choices(q, Some(role));
            assert!(
                (2..=3).contains(&choices.len()),
                "{} for {role:?}: {choices:?}",
                q.id
            );
            for g in &choices {
                let gear = c.economy.gear(g);
                assert!(gear.is_some_and(|g| g.role.is_none() || g.role == Some(role)));
            }
        }
    }
    let mut t = t(5);
    t.load("tower/1");
    let boss = t.id_of("vibe_coder");
    assert!(t.sim.debug_kill(boss));
    t.tick();
    t.talk_to("helpdesk_lead");
    t.accept("code_freeze");
    let Some(def) = t.sim.content().quest("code_freeze").cloned() else {
        panic!("no code_freeze")
    };
    let choices = t.sim.content().quest_gear_choices(&def, Some(Role::Ranged));
    let bag = t.sim.unit(t.p).map_or(0, |u| u.bag.len());
    t.talk_to("reviewer_t1");
    t.quest_cmd(Command::TurnInQuest {
        quest: "code_freeze".into(),
        choice: 1,
    });
    let Some(me) = t.sim.unit(t.p) else { panic!() };
    assert_eq!(me.bag.len(), bag + 1);
    assert_eq!(me.bag.last(), choices.get(1));
}

#[test]
fn a_gear_reward_waits_for_a_free_bag_slot() {
    let mut t = t(5);
    t.load("tower/1");
    let boss = t.id_of("vibe_coder");
    assert!(t.sim.debug_kill(boss));
    t.tick();
    while t.sim.grant_goods(t.p, "Ibuprofen") {}
    t.talk_to("helpdesk_lead");
    t.accept("code_freeze");
    t.turn_in("code_freeze");
    assert_eq!(t.sim.quest_status("code_freeze"), QuestStatus::Ready);
    assert!(t.has(|e| matches!(e, Event::ItemError { .. })));
}

#[test]
fn a_breadcrumb_is_turned_in_at_the_next_zones_giver() {
    let mut t = t(5);
    t.load("campus");
    t.onboard();
    t.talk_to("facilities_guy");
    t.talk_to("intern");
    t.turn_in("onboarding");
    t.accept("escalation_path");
    assert_eq!(t.sim.quest_status("escalation_path"), QuestStatus::Active);
    // The intern cannot take it back.
    t.turn_in("escalation_path");
    assert_eq!(t.sim.quest_status("escalation_path"), QuestStatus::Active);
    t.load("tower/1");
    t.log.clear();
    t.talk_to("helpdesk_lead");
    assert!(t.has(|e| matches!(
        e,
        Event::QuestOffered { quests, .. } if quests.iter().any(|q| q == "escalation_path")
    )));
    t.turn_in("escalation_path");
    assert_eq!(t.sim.quest_status("escalation_path"), QuestStatus::Done);
}

#[test]
fn saves_keep_open_quests_and_kill_tallies() {
    let mut t = t(5);
    t.load("tower/1");
    let proxy = t.id_of("meatproxy");
    assert!(t.sim.debug_kill(proxy));
    t.tick();
    t.talk_to("helpdesk_lead");
    t.accept("autocomplete_pest_control");
    let Some(save) = t.sim.export_save(t.p) else {
        panic!("no save")
    };
    let Some(back) = combat_core::SaveData::decode(&save.encode()) else {
        panic!("decode")
    };
    assert_eq!(back.kills, [("meatproxy".to_string(), 1)]);
    let mut fresh = Sim::new(content(), 1);
    let p = fresh.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(fresh.import_save(p, &back));
    assert_eq!(fresh.quests().active[0].id, "autocomplete_pest_control");
    assert_eq!(fresh.quests().kills.get("meatproxy"), Some(&1));
}

/// The XP budget of one clear per zone in campaign order, quests included (breadcrumbs pay out
/// where they end): the level you arrive with should match each zone's band, and the level you
/// leave with the curve's design (docs/FORMULAS.md, Levels and XP): one level per zone early,
/// later levels spanning more content, the cap inside the Innovation Lab.
#[test]
fn quest_and_kill_xp_track_the_zone_levels() {
    const ORDER: [(&str, u8, u8); 10] = [
        ("campus", 3, 3),
        ("tower/1", 3, 4),
        ("tower/2", 4, 5),
        ("tower/3", 5, 6),
        ("datacenter/1", 6, 7),
        ("datacenter/2", 7, 8),
        ("tower/4", 8, 9),
        ("tower/5", 9, 10),
        ("lab/1", 10, 10),
        ("lab/2", 10, 10),
    ];
    // The campus districts level on their own band (`campus_districts_level_a_fresh_hire`).
    let c = campaign_content();
    let mut sim = Sim::new(c.clone(), 1);
    sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let (mut level, mut xp) = (1u8, 0u32);
    let gain = |level: &mut u8, xp: &mut u32, amount: u32| {
        *xp += amount;
        while c.xp_to_next(*level) > 0 && *xp >= c.xp_to_next(*level) {
            *xp -= c.xp_to_next(*level);
            *level += 1;
        }
    };
    let mut report = Vec::new();
    let mut bad = Vec::new();
    for (zone, leave_lo, leave_hi) in ORDER {
        let arrive = level;
        if let Err(e) = sim.load_zone(zone, &anchor, Vec::new()) {
            panic!("{zone}: {e}");
        }
        let mobs: Vec<(u8, u32)> = sim
            .units()
            .filter(|u| u.hostile)
            .map(|u| (u.level, u.xp))
            .collect();
        let mut kills = 0;
        for (ml, reward) in mobs {
            let amount = c.kill_xp(level, ml, reward);
            kills += amount;
            gain(&mut level, &mut xp, amount);
        }
        let mut quests = 0;
        for q in &c.quests {
            let at = if q.is_breadcrumb() {
                &q.ender().zone
            } else {
                &q.giver.zone
            };
            if at == zone {
                // Taken after the zone's kills at the latest; the curve must allow it by then.
                if q.level_min > level {
                    bad.push(zone);
                }
                quests += c.quest_xp(q);
                gain(&mut level, &mut xp, c.quest_xp(q));
            }
        }
        let (lo, hi) = c.zone(zone).map_or((1, 1), |z| z.level_range);
        report.push(format!(
            "{zone} ({lo}-{hi}): {arrive} -> {level} (kills {kills} XP, quests {quests} XP)"
        ));
        if arrive + 1 < lo || arrive > hi + 1 || !(leave_lo..=leave_hi).contains(&level) {
            bad.push(zone);
        }
    }
    println!("{}", report.join("\n"));
    assert!(
        bad.is_empty(),
        "off-band arrivals {bad:?}:\n{}",
        report.join("\n")
    );
    assert_eq!(level, c.max_level(), "the campaign ends at the cap");
}

#[test]
fn group_members_keep_their_own_logs_and_share_kill_credit() {
    let mut t = t(5);
    t.load("tower/1");
    let p2 = t.sim.spawn_player("devops", Vec3::new(1.0, 0.0, 20.0), 0.0);
    t.talk_to("helpdesk_lead");
    t.accept("passwords_on_post_its");
    // The second member stands next to the first and takes a different ticket.
    let Some(at) = t.sim.unit(t.p).map(|u| u.pos) else {
        panic!()
    };
    t.sim.push(Command::PlayerMoved {
        pos: Vec3::new(at.x + 1.0, 0.0, at.z),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    t.sim.apply_as(
        p2,
        Command::PlayerMoved {
            pos: Vec3::new(at.x + 1.0, 0.0, at.z),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    t.sim
        .debug_set_quest_of(p2, "autocomplete_pest_control", QuestStatus::Done);
    t.sim.apply_as(
        p2,
        Command::AcceptQuest {
            quest: "secrets_in_the_repo".into(),
        },
    );
    t.tick();
    assert_eq!(t.sim.quests_of(p2).active[0].id, "secrets_in_the_repo");
    assert_eq!(t.sim.quests_of(t.p).active[0].id, "passwords_on_post_its");
    // A MeatProxy killed by the first member near the second counts for the second's ticket.
    let drone = t.id_of("meatproxy");
    let Some(dpos) = t.sim.unit(drone).map(|u| u.pos) else {
        panic!()
    };
    t.sim.apply_as(
        p2,
        Command::PlayerMoved {
            pos: Vec3::new(dpos.x + 2.0, 0.0, dpos.z + 2.0),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    t.kill(drone);
    assert_eq!(t.sim.quests_of(p2).active[0].counts, [1, 0]);
    assert_eq!(t.sim.quests_of(t.p).active[0].counts, [0]);
    assert_eq!(t.sim.quests_of(p2).kills.get("meatproxy"), Some(&1));
}

// ---- corpse lifetime ----

fn despawned(t: &T, id: EntityId) -> bool {
    t.has(|e| matches!(e, Event::Despawned { unit } if *unit == id))
}

impl T {
    /// Kills a living `key` and returns its id with the death tick.
    fn kill_key(&mut self, key: &str) -> (EntityId, u64) {
        let id = self.id_of(key);
        self.kill(id);
        (id, self.sim.now().0)
    }

    fn loot(&mut self, looter: EntityId, corpse: EntityId) {
        self.sim.apply_as(looter, Command::Loot { looter, corpse });
        self.tick();
        assert!(
            self.has(|e| matches!(e, Event::Looted { looter: l, corpse: c, .. } if *l == looter && *c == corpse)),
            "{looter} could not loot {corpse}"
        );
    }

    /// Runs until tick `at` (absolute).
    fn run_to(&mut self, at: u64) {
        while self.sim.now().0 < at {
            self.tick();
        }
    }
}

#[test]
fn a_looted_corpse_despawns_30_s_after_the_loot_and_its_kill_still_counts() {
    let mut t = t(5);
    t.load("tower/1");
    let (mp, _) = t.kill_key("meatproxy");
    let Some(at) = t.sim.unit(mp).map(|u| u.pos) else {
        panic!("no corpse")
    };
    t.move_to(Vec3::new(at.x, 0.0, at.z + 1.0), 0.0);
    t.sim.push(Command::SetTarget {
        unit: t.p,
        target: Some(mp),
    });
    t.loot(t.p, mp);
    let emptied = t.sim.now().0;
    t.run_to(emptied + ms_to_ticks(combat_core::CORPSE_EMPTY_MS) - 1);
    assert!(t.sim.unit(mp).is_some_and(|u| !u.lootable && !u.alive));
    t.tick();
    assert!(t.sim.unit(mp).is_none() && despawned(&t, mp));
    assert!(
        t.sim.units().all(|u| u.target != Some(mp)),
        "targets drop it"
    );
    assert_eq!(t.sim.quests().kills.get("meatproxy"), Some(&1));
}

#[test]
fn an_unlooted_corpse_keeps_its_loot_for_three_minutes_then_goes() {
    let mut t = t(5);
    t.load("tower/1");
    let (mp, died) = t.kill_key("meatproxy");
    let last = died + ms_to_ticks(combat_core::CORPSE_LOOT_MS);
    t.run_to(died + ms_to_ticks(combat_core::CORPSE_EMPTY_MS) + 30);
    assert!(
        t.sim.unit(mp).is_some_and(|u| u.lootable),
        "still holds loot"
    );
    t.run_to(last - 1);
    assert!(t.sim.unit(mp).is_some_and(|u| u.lootable));
    t.tick();
    assert!(
        t.sim.unit(mp).is_none() && despawned(&t, mp),
        "loot is lost"
    );
}

/// Per-player rolls: the corpse stays while any member still has one, and the short timer
/// starts when the last roll is taken (or its owner leaves).
#[test]
fn a_corpse_stays_while_any_group_member_still_has_loot_on_it() {
    let mut t = t_with(5, |c| {
        for k in ["senior_engineer", "devops_eng", "backend_dev"] {
            let u = def_mut(c, k);
            u.spell_power = 100_000;
            u.max_hp = 100_000;
        }
    });
    t.load("tower/1");
    let p2 = t.sim.spawn_player("devops", Vec3::new(0.0, 0.0, 20.0), 0.0);
    let p3 = t
        .sim
        .spawn_player("backend", Vec3::new(0.0, 0.0, 20.0), 0.0);
    let (mp, died) = t.kill_key("meatproxy");
    let Some(at) = t.sim.unit(mp).map(|u| u.pos) else {
        panic!("no corpse")
    };
    let pos = Vec3::new(at.x, 0.0, at.z + 1.0);
    for p in [p2, p3] {
        let (yaw, moving, jumping) = (0.0, false, false);
        t.sim.apply_as(
            p,
            Command::PlayerMoved {
                pos,
                yaw,
                moving,
                jumping,
            },
        );
    }
    t.move_to(pos, 0.0);
    t.loot(t.p, mp);
    t.run_to(died + ms_to_ticks(combat_core::CORPSE_EMPTY_MS) + 30);
    assert!(t.sim.unit(mp).is_some(), "p2 and p3 still have rolls on it");
    t.loot(p2, mp);
    // The last roll leaves with its owner: from here the corpse is empty for everyone.
    assert!(t.sim.remove_player(p3));
    let emptied = t.sim.now().0;
    t.run_to(emptied + ms_to_ticks(combat_core::CORPSE_EMPTY_MS) - 1);
    assert!(t.sim.unit(mp).is_some_and(|u| !u.lootable));
    t.tick();
    assert!(t.sim.unit(mp).is_none());
}

/// Boss corpses are set pieces: they stay until the zone unloads, loot or not.
#[test]
fn a_boss_corpse_stays_until_the_zone_is_left() {
    let mut t = t(10);
    t.load("tower/1");
    let boss = t.id_of("vibe_coder");
    assert!(t.sim.debug_kill(boss));
    t.run(ms_to_ticks(combat_core::CORPSE_LOOT_MS) + 30);
    assert!(t.sim.unit(boss).is_some_and(|u| !u.alive));
    t.load("campus");
    assert!(t.sim.unit(boss).is_none());
}

#[test]
fn an_item_an_active_quest_wants_cannot_be_discarded() {
    const JUNK: &str = "Expired Free Pizza Coupon";
    let mut picked = None;
    let mut t = t_with(10, |c| {
        picked = c
            .quests
            .iter_mut()
            .find(|q| q.giver.zone == "campus" && q.requires.is_none() && q.level_min <= 10)
            .map(|q| {
                q.objectives = vec![Objective::Loot(JUNK.into(), 1)];
                (q.id.clone(), q.giver.npc.clone())
            });
    });
    let Some((quest, giver)) = picked else {
        panic!("no campus quest")
    };
    t.load("campus");
    t.talk_to(&giver);
    t.accept(&quest);
    assert_eq!(t.sim.quests_of(t.p).active.len(), 1);
    assert!(t.sim.grant_goods(t.p, JUNK));
    t.quest_cmd(Command::Discard { slot: 0 });
    t.quest_cmd(Command::DiscardJunk);
    let bag = t.sim.unit(t.p).map(|u| u.bag.clone());
    assert_eq!(bag, Some(vec![JUNK.to_string()]), "protected while active");
    assert!(t.has(|e| matches!(
        e,
        Event::ItemError {
            reason: combat_core::ItemError::Protected
        }
    )));
}

// ---- Body Leasing easter egg ----

/// Runs `secs` sim seconds.
fn wait_secs(t: &mut T, secs: u64) {
    t.run(ms_to_ticks(1000) * secs);
}

fn counts(t: &T, quest: &str) -> Vec<u32> {
    t.sim
        .quests()
        .active
        .iter()
        .find(|q| q.id == quest)
        .map_or_else(Vec::new, |q| q.counts.clone())
}

#[test]
fn body_leasing_chain_runs_from_the_account_manager_to_the_title() {
    let mut t = t(1);
    t.load("campus");
    let am = t.id_of("bl_account_manager");
    // Closed until the allocation ticket is taken, and reachable from the campus only then.
    assert!(links(&t.sim, "campus").contains(&("client/1".to_string(), false)));
    let at = t.sim.unit(am).map(|u| u.pos).unwrap_or_default();
    t.move_to(Vec3::new(at.x, 0.0, at.z + 2.0), 0.0);
    t.accept("bl_allocation");
    assert_eq!(t.sim.quest_status("bl_allocation"), QuestStatus::Active);
    assert!(t.has(|e| matches!(e, Event::Unlocked { zone } if zone == "client/1")));
    assert!(links(&t.sim, "campus").contains(&("client/1".to_string(), true)));

    t.load("client/1");
    // Nothing to fight: the zone has only friendly units.
    assert!(t.sim.units().all(|u| !u.hostile));
    t.talk_to("client_receptionist");
    assert_eq!(t.sim.quest_status("bl_allocation"), QuestStatus::Ready);
    t.turn_in("bl_allocation");
    assert_eq!(
        t.sim.quest_status("bl_access_rights"),
        QuestStatus::Available
    );

    // The wait is slow on purpose: 75 s, one second per count, then ready.
    t.accept("bl_access_rights");
    wait_secs(&mut t, 74);
    assert_eq!(t.sim.quest_status("bl_access_rights"), QuestStatus::Active);
    assert!(counts(&t, "bl_access_rights")[0] >= 73);
    wait_secs(&mut t, 2);
    assert_eq!(t.sim.quest_status("bl_access_rights"), QuestStatus::Ready);
    t.turn_in("bl_access_rights");

    // Muted meeting: accepted at the reception, closed at the lead in the back room, who
    // also hands out the timesheet: nothing sends you back to reception.
    t.talk_to("client_receptionist");
    t.accept("bl_meeting");
    t.talk_to("client_delivery_lead");
    assert_eq!(t.sim.quest_status("bl_meeting"), QuestStatus::Ready);
    t.quest_cmd(Command::TurnInQuest {
        quest: "bl_meeting".into(),
        choice: 0,
    });
    assert_eq!(t.sim.quest_status("bl_meeting"), QuestStatus::Done);

    // The only way out is gated by the timesheet.
    assert_eq!(links(&t.sim, "client/1"), [("campus".to_string(), false)]);
    t.accept("bl_timesheet");
    wait_secs(&mut t, 21);
    assert_eq!(t.sim.quest_status("bl_timesheet"), QuestStatus::Ready);
    assert_eq!(links(&t.sim, "client/1"), [("campus".to_string(), false)]);
    t.turn_in("bl_timesheet");
    assert_eq!(t.sim.quest_status("bl_timesheet"), QuestStatus::Done);
    assert_eq!(links(&t.sim, "client/1"), [("campus".to_string(), true)]);

    // The reward: the achievement and its title.
    assert!(t.has(|e| matches!(e, Event::Achievement { key, .. } if key == "billable")));
    let me = t
        .sim
        .unit(t.p)
        .map(|u| (u.title.clone(), u.achievements.clone()));
    let (title, achievements) = me.unwrap_or_default();
    assert_eq!(title, "billable");
    assert!(achievements.iter().any(|a| a == "billable"));
    let def = t
        .sim
        .content()
        .world
        .achievements
        .iter()
        .find(|a| a.key == "billable");
    assert_eq!(def.map(|a| a.title.as_str()), Some("External Resource"));
    assert_eq!(
        def.map(|a| a.text.as_str()),
        Some("Nothing happened. You billed it anyway.")
    );
}

#[test]
fn body_leasing_waits_only_count_inside_the_client_office() {
    let mut t = t(1);
    t.debug_to_access_rights();
    wait_secs(&mut t, 5);
    let in_zone = counts(&t, "bl_access_rights")[0];
    assert!(in_zone >= 4, "the bar moved {in_zone}");
    t.load("campus");
    wait_secs(&mut t, 10);
    assert_eq!(counts(&t, "bl_access_rights")[0], in_zone);
}

#[test]
fn body_leasing_quests_cannot_be_skipped() {
    let mut t = t(1);
    t.load("client/1");
    t.talk_to("client_receptionist");
    // No allocation ticket, no access rights, and the later tickets stay locked behind the chain.
    for q in ["bl_access_rights", "bl_meeting", "bl_timesheet"] {
        assert_eq!(t.sim.quest_status(q), QuestStatus::Locked, "{q}");
    }
}

impl T {
    /// Loads the office with the allocation ticket done and the access-rights ticket accepted.
    fn debug_to_access_rights(&mut self) {
        self.load("client/1");
        assert!(self.sim.debug_set_quest("bl_allocation", QuestStatus::Done));
        self.talk_to("client_receptionist");
        self.accept("bl_access_rights");
    }
}

// ---- loot quests are finishable solo, fresh or cleared ----

impl T {
    /// Kills every living hostile trash unit of the zone (the boss stays, so the zone is not
    /// "cleared" but its trash is all dead, like after a thorough run).
    fn wipe_trash(&mut self) {
        loop {
            let next = self
                .sim
                .units()
                .find(|u| u.alive && u.hostile && u.key != "vibe_coder" && !u.boss)
                .map(|u| u.id);
            match next {
                Some(id) => self.kill(id),
                None => return,
            }
        }
    }

    /// Accepts `quest` from `giver`, then kills and loots living sources until it is ready.
    fn loot_quest_solo(&mut self, giver: &str, quest: &str, item: &str) {
        self.talk_to(giver);
        self.accept(quest);
        let need = match self
            .sim
            .content()
            .quest(quest)
            .map(|q| q.objectives[0].clone())
        {
            Some(Objective::Loot(_, n)) => n,
            other => panic!("not a loot quest: {other:?}"),
        };
        for _ in 0..40 {
            if self.sim.quests().active[0].counts[0] >= need {
                break;
            }
            let src = self
                .sim
                .units()
                .find(|u| {
                    u.alive
                        && u.hostile
                        && self
                            .sim
                            .content()
                            .unit_def(&u.key)
                            .is_some_and(|d| d.loot.iter().any(|l| l.item == item))
                })
                .map(|u| u.id);
            let Some(id) = src else {
                panic!(
                    "{quest}: no living source left at {:?}",
                    self.sim.quests().active[0].counts
                )
            };
            self.kill(id);
            let Some(at) = self.sim.unit(id).map(|u| u.pos) else {
                panic!()
            };
            self.move_to(Vec3::new(at.x, 0.0, at.z + 1.0), 0.0);
            self.sim.push(Command::Loot {
                looter: self.p,
                corpse: id,
            });
            self.tick();
        }
        // Merged tickets also ask for kills of their own (the loot sources are already dead).
        let kills: Vec<String> = self
            .sim
            .content()
            .quest(quest)
            .map(|q| {
                q.objectives
                    .iter()
                    .filter_map(|o| match o {
                        Objective::Kill(k, _) => Some(k.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        for key in kills {
            while self.sim.quest_status(quest) != QuestStatus::Ready {
                let Some(id) = self
                    .sim
                    .units()
                    .find(|u| u.alive && u.hostile && u.key == key)
                    .map(|u| u.id)
                else {
                    break;
                };
                self.kill(id);
            }
        }
        assert_eq!(self.sim.quests().active[0].counts[0], need, "{quest} stuck");
        self.talk_to(giver);
        self.turn_in(quest);
        assert!(self.sim.quests().completed.iter().any(|c| c == quest));
    }
}

fn solo_loot_case(zone: &str, giver: &str, quest: &str, item: &str, cleared: bool) {
    // Default drop rules and a level-appropriate fighter: no tweaks to chances, so this only
    // passes when the shipped drops are guaranteed.
    let mut t = t(6);
    t.load(zone);
    if cleared {
        t.wipe_trash();
    }
    t.loot_quest_solo(giver, quest, item);
}

#[test]
fn quota_attainment_finishes_solo_in_a_fresh_zone() {
    solo_loot_case(
        "tower/3",
        "office_manager",
        "quota_attainment",
        "Verbal Commitment (Notarized)",
        false,
    );
}

#[test]
fn quota_attainment_finishes_solo_in_a_cleared_zone() {
    solo_loot_case(
        "tower/3",
        "office_manager",
        "quota_attainment",
        "Verbal Commitment (Notarized)",
        true,
    );
}

#[test]
fn talking_stick_finishes_solo_in_a_fresh_zone() {
    solo_loot_case(
        "tower/2",
        "facilities_manager",
        "talking_stick",
        "Stand-up Talking Stick",
        false,
    );
}

#[test]
fn talking_stick_finishes_solo_in_a_cleared_zone() {
    solo_loot_case(
        "tower/2",
        "facilities_manager",
        "talking_stick",
        "Stand-up Talking Stick",
        true,
    );
}

/// Level of the district units an open-world objective sends you to: the fixed level of the
/// campus pack, or the authored level of a campus rare.
fn district_level(c: &combat_core::Content, unit: &str) -> Option<u8> {
    let z = c.zone("campus")?;
    let pack = z
        .packs
        .iter()
        .filter(|p| p.units.iter().any(|u| u == unit))
        .filter_map(|p| p.level)
        .min();
    let rare = z
        .rares
        .iter()
        .filter(|r| r.unit == unit)
        .filter_map(|r| c.unit_def(&r.unit).map(|d| d.level))
        .min();
    pack.or(rare)
}

/// Open-world quests are offered at the level of the district they send you to: never above
/// it, at most two below it.
#[test]
fn open_world_quests_match_their_district_band() {
    let c = content();
    let mut bad = Vec::new();
    for q in c.quests.iter().filter(|q| q.open_world) {
        for o in &q.objectives {
            let unit = match o {
                Objective::Kill(k, _) | Objective::KillClean(k, _) => Some(k.clone()),
                Objective::Loot(it, _) => c
                    .units
                    .iter()
                    .find(|u| {
                        u.loot
                            .iter()
                            .any(|l| l.item == *it && l.quest.as_deref() == Some(q.id.as_str()))
                    })
                    .map(|u| u.key.clone()),
                _ => None,
            };
            let Some(level) = unit.as_deref().and_then(|u| district_level(&c, u)) else {
                continue;
            };
            if q.level_min > level || q.level_min + 2 < level {
                bad.push(format!(
                    "{}: level {} for district level {level}",
                    q.id, q.level_min
                ));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The districts are a leveling path of their own: a fresh hire who works only the district
/// packs and the open-world quests up to level 5 reaches 4 to 6 (WotLK-style: about one level
/// per district), and the level-10 spots stay out of reach of that budget.
#[test]
fn campus_districts_level_a_fresh_hire() {
    let c = content();
    let Some(campus) = c.zone("campus") else {
        panic!("no campus")
    };
    let mut mobs: Vec<(u8, u32)> = Vec::new();
    for p in campus.packs.iter().filter(|p| p.level.is_some()) {
        let level = p.level.unwrap_or(1);
        for u in &p.units {
            let Some(d) = c.unit_def(u) else {
                panic!("no unit {u}")
            };
            // Authored XP scales by `mob_scaling.xp_pct_per_level` from the authored level.
            let pct = 100 + i32::from(level) * 10 - i32::from(d.level) * 10;
            let xp = (i64::from(d.xp_reward) * i64::from(pct.max(10)) / 100) as u32;
            let xp = xp * c.xp_reward_pct(level) / 100;
            mobs.push((level, xp));
        }
    }
    mobs.sort();
    let (mut level, mut xp) = (1u8, 0u32);
    let gain = |level: &mut u8, xp: &mut u32, amount: u32| {
        *xp += amount;
        while c.xp_to_next(*level) > 0 && *xp >= c.xp_to_next(*level) {
            *xp -= c.xp_to_next(*level);
            *level += 1;
        }
    };
    let mut kills = 0;
    let mut quests = 0;
    for (ml, reward) in &mobs {
        let a = c.kill_xp(level, *ml, *reward);
        kills += a;
        gain(&mut level, &mut xp, a);
    }
    for q in c.quests.iter().filter(|q| q.open_world && q.level_min <= 5) {
        quests += c.quest_xp(q);
        gain(&mut level, &mut xp, c.quest_xp(q));
    }
    println!("districts: 1 -> {level} (kills {kills} XP, quests {quests} XP)");
    assert!(
        (4..=6).contains(&level),
        "districts take a fresh hire to {level}"
    );
    let ten: Vec<&str> = c
        .quests
        .iter()
        .filter(|q| q.open_world && q.level_min > 5)
        .map(|q| q.id.as_str())
        .collect();
    assert!(ten.len() >= 2, "level-10 spots: {ten:?}");
}
