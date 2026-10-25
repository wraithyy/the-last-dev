//! The interactive quest objectives (Collect, Inspect, Sequence, UseOn, Deliver, Escort) run in
//! the real zone geometry, and the turn-in flow: a zone's tickets close where its work ends.

mod common;

use combat_core::{
    Command, Content, EntityId, Event, Objective, QUEST_PROP_WRONG_ORDER, QuestStatus, Sim, Vec3,
    ms_to_ticks,
};
use common::{Geo, content, def_mut, geometry};

/// Campaign zones in world order; a later index is further along.
const CAMPAIGN: [&str; 9] = [
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

/// A turn-in counts as "where the work ends" this close to the boss or the way on.
const END_RADIUS_M: f32 = 30.0;

struct W {
    sim: Sim,
    p: EntityId,
    geo: std::collections::HashMap<String, Geo>,
    log: Vec<Event>,
}

fn world_with(zone: &str, level: u8, tweak: impl FnOnce(&mut Content)) -> W {
    let mut c = content();
    let me = def_mut(&mut c, "senior_engineer");
    me.spell_power = 100_000;
    me.max_hp = 100_000;
    tweak(&mut c);
    let mut sim = Sim::new(c, 5);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_player_level(level);
    let mut w = W {
        sim,
        p,
        geo: geometry(),
        log: Vec::new(),
    };
    w.load(zone);
    w
}

fn world(zone: &str, level: u8) -> W {
    world_with(zone, level, |_| {})
}

impl W {
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

    fn secs(&mut self, s: u64) {
        for _ in 0..ms_to_ticks(1000) * s {
            self.tick();
        }
        self.sim.debug_set_hp(self.p, u32::MAX);
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

    /// Stands on a layout anchor of the loaded zone, facing the way it faces.
    fn stand_at(&mut self, anchor: &str) {
        let zone = self.sim.zone().unwrap_or_default().to_string();
        let Some((pos, yaw)) = self
            .geo
            .get(&zone)
            .and_then(|g| g.anchors.get(anchor))
            .copied()
        else {
            panic!("{zone}: no anchor {anchor}")
        };
        self.move_to(pos, yaw);
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
        self.move_to(Vec3::new(at.x + 1.0, 0.0, at.z), 0.0);
        self.sim.push(Command::Talk { unit: id });
        self.tick();
    }

    fn cmd(&mut self, c: Command) {
        self.sim.push(c);
        self.tick();
    }

    /// Marks the chain of quests in front of `q` as done, so a test can take it directly.
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

    fn accept(&mut self, giver: &str, q: &str) {
        self.finish_prerequisites(q);
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

#[test]
fn pickups_count_once_per_player_and_only_while_a_quest_wants_them() {
    let mut w = world("tower/1", 2);
    w.log.clear();
    w.use_prop("t1_password_1");
    assert!(
        !w.log
            .iter()
            .any(|e| matches!(e, Event::Interacted { prop, .. } if prop == "t1_password_1")),
        "nothing to pick up without the quest"
    );
    w.accept("helpdesk_lead", "passwords_on_post_its");
    w.use_prop("t1_password_1");
    w.use_prop("t1_password_1");
    assert_eq!(
        w.counts("passwords_on_post_its"),
        [1],
        "one pickup per prop"
    );
    // The same prop is still there for a second player: pickups respawn per player.
    let p2 = w.sim.spawn_player("devops", Vec3::new(0.0, 0.0, 0.0), 0.0);
    w.sim.set_level_of(p2, 2);
    let giver = w.id_of("helpdesk_lead");
    let at = w.sim.unit(giver).map(|u| u.pos).unwrap_or_default();
    let moved = |pos| Command::PlayerMoved {
        pos,
        yaw: 0.0,
        moving: false,
        jumping: false,
    };
    w.sim.apply_as(p2, moved(Vec3::new(at.x + 1.0, 0.0, at.z)));
    w.sim.apply_as(
        p2,
        Command::AcceptQuest {
            quest: "passwords_on_post_its".into(),
        },
    );
    let spot = w
        .sim
        .prop_spot("t1_password_1")
        .unwrap_or(Vec3::new(0.0, 0.0, 0.0));
    w.sim.apply_as(p2, moved(spot));
    w.sim.apply_as(
        p2,
        Command::Interact {
            prop: "t1_password_1".into(),
        },
    );
    w.tick();
    let p2_counts = w.sim.quests_of(p2).active.first().map(|q| q.counts.clone());
    assert_eq!(p2_counts, Some(vec![1]));
    for k in 2..=4 {
        w.use_prop(&format!("t1_password_{k}"));
    }
    assert_eq!(
        w.sim.quest_status("passwords_on_post_its"),
        QuestStatus::Ready
    );
    w.turn_in("passwords_on_post_its");
}

#[test]
fn inspect_counts_standing_at_the_spot_facing_it_or_on_use() {
    let mut w = world("tower/2", 4);
    w.accept("facilities_manager", "fire_code_violations");
    let Some((pos, yaw)) = w
        .geo
        .get("tower/2")
        .and_then(|g| g.anchors.get("prop:t2_firecode_1"))
        .copied()
    else {
        panic!("no firecode spot")
    };
    w.move_to(pos, yaw + std::f32::consts::PI);
    w.secs(2);
    assert_eq!(
        w.counts("fire_code_violations"),
        [0],
        "back turned: no photo"
    );
    w.move_to(pos, yaw);
    w.secs(2);
    assert_eq!(w.counts("fire_code_violations"), [1]);
    w.use_prop("t2_firecode_2");
    assert_eq!(
        w.counts("fire_code_violations"),
        [2],
        "F takes the photo too"
    );
    for k in 3..=4 {
        w.stand_at(&format!("prop:t2_firecode_{k}"));
        w.secs(2);
    }
    assert_eq!(
        w.sim.quest_status("fire_code_violations"),
        QuestStatus::Ready
    );
    w.turn_in("fire_code_violations");
}

#[test]
fn a_sequence_starts_over_on_a_skipped_step_and_completes_in_order() {
    let mut w = world("tower/1", 2);
    w.accept("helpdesk_lead", "have_you_tried_restarting");
    w.use_prop("t1_switch_a");
    assert_eq!(w.counts("have_you_tried_restarting"), [1]);
    w.log.clear();
    w.use_prop("t1_switch_c");
    assert_eq!(
        w.counts("have_you_tried_restarting"),
        [0],
        "C before B resets"
    );
    assert!(w.log.iter().any(|e| matches!(
        e,
        Event::Interacted { prop, outcome, .. }
            if prop == "t1_switch_c" && *outcome == QUEST_PROP_WRONG_ORDER
    )));
    for s in ["t1_switch_a", "t1_switch_b", "t1_switch_c"] {
        w.use_prop(s);
    }
    assert_eq!(
        w.sim.quest_status("have_you_tried_restarting"),
        QuestStatus::Ready
    );
    w.turn_in("have_you_tried_restarting");
}

#[test]
fn use_on_takes_distinct_devices_and_enemies_in_reach() {
    let mut w = world("tower/2", 4);
    w.accept("facilities_manager", "meeting_room_reclamation");
    w.use_prop("t2_room_panel_1");
    w.use_prop("t2_room_panel_1");
    assert_eq!(w.counts("meeting_room_reclamation"), [1], "each panel once");
    w.use_prop("t2_room_panel_2");
    w.use_prop("t2_room_panel_3");
    assert_eq!(
        w.sim.quest_status("meeting_room_reclamation"),
        QuestStatus::Ready
    );
    w.turn_in("meeting_room_reclamation");

    // Enemies take the item through F (a talk command) in reach, each one once.
    let mut w = world("tower/1", 2);
    w.accept("helpdesk_lead", "autocomplete_pest_control");
    let drones: Vec<(EntityId, Vec3)> = w
        .sim
        .units()
        .filter(|u| u.key == "copilot_drone" && u.alive)
        .map(|u| (u.id, u.pos))
        .collect();
    assert!(drones.len() >= 3);
    let hand = |w: &mut W, (id, at): (EntityId, Vec3)| {
        let now = w.sim.unit(id).map_or(at, |u| u.pos);
        w.move_to(Vec3::new(now.x + 1.0, 0.0, now.z), 0.0);
        w.cmd(Command::Talk { unit: id });
    };
    hand(&mut w, drones[0]);
    hand(&mut w, drones[0]);
    assert_eq!(w.counts("autocomplete_pest_control"), [1]);
    hand(&mut w, drones[1]);
    hand(&mut w, drones[2]);
    assert_eq!(
        w.sim.quest_status("autocomplete_pest_control"),
        QuestStatus::Ready
    );
}

#[test]
fn a_timed_delivery_fails_when_the_coffee_goes_cold_and_counts_on_time() {
    let mut w = world("datacenter/1", 7);
    w.accept("reviewer_dc1", "coffee_for_the_noc");
    assert!(
        w.sim
            .deliver_ticks_left(w.p, "coffee_for_the_noc")
            .is_some()
    );
    w.log.clear();
    w.secs(91);
    assert!(
        w.log
            .iter()
            .any(|e| matches!(e, Event::QuestFailed { quest } if quest == "coffee_for_the_noc"))
    );
    assert_eq!(
        w.sim.quest_status("coffee_for_the_noc"),
        QuestStatus::Available
    );
    // Again, and straight through the door this time.
    w.accept("reviewer_dc1", "coffee_for_the_noc");
    w.secs(10);
    w.load("datacenter/2");
    w.talk("noc_operator");
    assert_eq!(w.sim.quest_status("coffee_for_the_noc"), QuestStatus::Ready);
    w.turn_in("coffee_for_the_noc");
}

fn escort(w: &mut W, unit: &str, wave: &str, quest: &str, max_s: u64) -> (u32, f32) {
    let start = w.sim.units().find(|u| u.key == unit).map(|u| u.pos);
    let mut waves = 0;
    for _ in 0..max_s {
        let Some(at) = w
            .sim
            .units()
            .find(|u| u.key == unit && u.alive)
            .map(|u| u.pos)
        else {
            break;
        };
        w.move_to(Vec3::new(at.x + 1.0, 0.0, at.z + 1.0), 0.0);
        w.secs(1);
        let adds: Vec<EntityId> = w
            .sim
            .units()
            .filter(|u| u.key == wave && u.alive)
            .map(|u| u.id)
            .collect();
        waves += adds.len() as u32;
        for a in adds {
            w.sim.debug_kill(a);
        }
        if w.sim.quest_status(quest) == QuestStatus::Ready {
            break;
        }
    }
    let end = w.sim.units().find(|u| u.key == unit).map(|u| u.pos);
    let walked = match (start, end) {
        (Some(a), Some(b)) => ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt(),
        _ => 0.0,
    };
    (waves, walked)
}

#[test]
fn an_escort_walks_with_its_guard_through_waves_to_the_far_end() {
    let mut w = world("tower/2", 4);
    w.accept("facilities_manager", "performance_improvement_plan");
    // Nobody near: Dave waits.
    w.stand_at("prop:t2_commit_note_1");
    let before = w
        .sim
        .units()
        .find(|u| u.key == "pip_colleague")
        .map(|u| u.pos);
    w.secs(3);
    let after = w
        .sim
        .units()
        .find(|u| u.key == "pip_colleague")
        .map(|u| u.pos);
    assert_eq!(before, after, "an unguarded escort stays put");
    let (waves, walked) = escort(
        &mut w,
        "pip_colleague",
        "rogue_calendar_invite",
        "performance_improvement_plan",
        150,
    );
    assert!(waves > 0, "calendar invites came for him");
    assert!(walked > 40.0, "walked {walked:.1} m");
    assert_eq!(
        w.sim.quest_status("performance_improvement_plan"),
        QuestStatus::Ready
    );
    w.turn_in("performance_improvement_plan");
}

#[test]
fn a_fallen_escort_starts_over_from_its_spawn() {
    let mut w = world_with("datacenter/2", 8, |c| {
        def_mut(c, "tape_robot").max_hp = 1;
    });
    // The story step comes from Hall A's reviewer; forcing it active skips the chain.
    assert!(
        w.sim
            .debug_set_quest("story_last_backup", QuestStatus::Active)
    );
    let spawn = w.sim.units().find(|u| u.key == "tape_robot").map(|u| u.pos);
    let mut progressed = false;
    let mut reset = false;
    for _ in 0..90 {
        let Some(at) = w
            .sim
            .units()
            .find(|u| u.key == "tape_robot" && u.alive)
            .map(|u| u.pos)
        else {
            w.secs(1);
            continue;
        };
        w.move_to(Vec3::new(at.x + 1.0, 0.0, at.z + 1.0), 0.0);
        w.secs(1);
        let c = w.counts("story_last_backup").first().copied().unwrap_or(0);
        progressed |= c > 0;
        if progressed && c == 0 {
            reset = true;
            break;
        }
    }
    assert!(
        progressed && reset,
        "progress, then a reset when TAPE-9000 fell"
    );
    // Back at the start; the guard is still next to it, so it may have taken a step or two.
    let back = w
        .sim
        .units()
        .find(|u| u.key == "tape_robot" && u.alive)
        .map(|u| u.pos);
    let gap = match (back, spawn) {
        (Some(a), Some(b)) => ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt(),
        _ => f32::INFINITY,
    };
    assert!(
        gap < 3.0,
        "it stands up again at the start ({gap:.1} m off)"
    );
}

/// Where the turn-in NPC of `q` stands: (zone, anchor position).
fn ender_spot(
    c: &Content,
    geo: &std::collections::HashMap<String, Geo>,
    q: &str,
) -> (String, Vec3) {
    let Some(def) = c.quest(q) else {
        panic!("no quest {q}")
    };
    let e = def.ender();
    let anchor = c
        .zone(&e.zone)
        .and_then(|z| z.friendly.iter().find(|f| f.unit == e.npc))
        .map(|f| f.anchor.clone());
    let pos = anchor
        .and_then(|a| geo.get(&e.zone).and_then(|g| g.anchors.get(&a)))
        .map(|a| a.0);
    let Some(pos) = pos else {
        panic!("{q}: {} has no anchor in {}", e.npc, e.zone)
    };
    (e.zone.clone(), pos)
}

/// The far end of a campaign zone: the boss, the exit elevator and doors to later zones.
fn zone_ends(geo: &Geo, zone: &str) -> Vec<Vec3> {
    let rank = |z: &str| CAMPAIGN.iter().position(|c| *c == z);
    let here = rank(zone);
    geo.anchors
        .iter()
        .filter(|(n, _)| {
            *n == "boss"
                || *n == "elevator_exit"
                || n.strip_prefix("door:")
                    .is_some_and(|to| rank(to).is_some() && rank(to) > here)
        })
        .map(|(_, a)| a.0)
        .collect()
}

#[test]
fn no_campaign_ticket_sends_you_back_to_the_zone_entrance() {
    let c = content();
    let geo = geometry();
    let rank = |z: &str| CAMPAIGN.iter().position(|c| *c == z);
    let mut bad = Vec::new();
    for q in c.quests.iter().filter(|q| rank(&q.giver.zone).is_some()) {
        let (zone, at) = ender_spot(&c, &geo, &q.id);
        if rank(&zone) > rank(&q.giver.zone) {
            continue;
        }
        let Some(g) = geo.get(&zone) else {
            panic!("no geometry for {zone}")
        };
        let near = zone_ends(g, &zone)
            .iter()
            .map(|e| ((e.x - at.x).powi(2) + (e.z - at.z).powi(2)).sqrt())
            .fold(f32::INFINITY, f32::min);
        if near > END_RADIUS_M {
            bad.push(format!(
                "{}: turned in at {} in {zone}, {near:.0} m from the zone's end",
                q.id,
                q.ender().npc
            ));
        }
        // Handing in work before the boss must not pull it.
        let boss = c.zone(&zone).and_then(|z| z.boss.as_ref());
        let aggro = boss
            .and_then(|b| c.unit_def(&b.unit))
            .and_then(|u| u.ai.as_ref())
            .map_or(0.0, |ai| ai.aggro_m);
        if let Some(b) = boss.and_then(|b| g.anchors.get(&b.anchor)) {
            let d = ((b.0.x - at.x).powi(2) + (b.0.z - at.z).powi(2)).sqrt();
            if d <= aggro {
                bad.push(format!("{}: turn-in {d:.0} m from the boss pulls it", q.id));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "tickets that send you back:\n{}",
        bad.join("\n")
    );
}

/// Kill and loot only: the tickets the playtesters called "drop quests".
fn drop_quest(q: &combat_core::QuestDef) -> bool {
    q.objectives.iter().all(|o| {
        matches!(
            o,
            Objective::Kill(..) | Objective::Loot(..) | Objective::KillClean(..)
        )
    })
}

#[test]
fn campaign_zones_mix_objective_kinds_and_keep_drop_quests_a_minority() {
    let c = content();
    for zone in CAMPAIGN {
        let qs: Vec<_> = c
            .quests
            .iter()
            .filter(|q| q.giver.zone == zone && !q.is_breadcrumb())
            .collect();
        let drops = qs.iter().filter(|q| drop_quest(q)).count();
        let mut kinds: Vec<std::mem::Discriminant<Objective>> = qs
            .iter()
            .flat_map(|q| q.objectives.iter().map(std::mem::discriminant))
            .collect();
        kinds.sort_by_key(|d| format!("{d:?}"));
        kinds.dedup();
        assert!(
            drops * 3 <= qs.len(),
            "{zone}: {drops} of {} tickets are drop quests",
            qs.len()
        );
        assert!(
            kinds.len() >= 3,
            "{zone}: only {} objective kinds",
            kinds.len()
        );
    }
}

#[test]
fn the_hybrid_worker_badge_goes_to_the_enforcer_who_then_counts_desks() {
    let mut w = world("campus", 2);
    w.accept("hybrid_worker", "coffee_badging");
    w.talk("rto_enforcer");
    assert_eq!(w.sim.quest_status("coffee_badging"), QuestStatus::Ready);
    w.turn_in("coffee_badging");
    w.accept("rto_enforcer", "desk_census");
    for k in 1..=3 {
        w.stand_at(&format!("prop:rto_desk_{k}"));
        w.secs(2);
    }
    assert_eq!(w.sim.quest_status("desk_census"), QuestStatus::Ready);
    w.turn_in("desk_census");
}

#[test]
fn every_quest_npc_is_protected_and_no_giver_is_a_defended_unit() {
    let c = content();
    let mortal: Vec<&str> = c
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter())
        .filter_map(|o| match o {
            Objective::Defend { unit, .. } | Objective::Escort { unit, .. } => Some(unit.as_str()),
            _ => None,
        })
        .collect();
    for q in &c.quests {
        let mut npcs = vec![q.giver.npc.as_str(), q.ender().npc.as_str()];
        for o in &q.objectives {
            match o {
                Objective::Talk(k) => npcs.push(k),
                Objective::Deliver { npc, .. } => npcs.push(npc),
                _ => {}
            }
        }
        for npc in npcs {
            assert!(
                !mortal.contains(&npc),
                "{}: {npc} gives, takes or is talked to and can also die",
                q.id
            );
            assert!(c.is_quest_npc(npc), "{}: {npc} is not protected", q.id);
        }
    }
}

#[test]
fn main_story_never_waits_on_side_quests_and_chains_keep_their_levels() {
    let c = content();
    for q in &c.quests {
        let Some(r) = q.requires.as_deref().and_then(|r| c.quest(r)) else {
            continue;
        };
        assert!(
            r.level_min <= q.level_min,
            "{} (level {}) needs {} (level {})",
            q.id,
            q.level_min,
            r.id,
            r.level_min
        );
        assert!(
            !q.main || r.main,
            "main quest {} waits on side quest {}",
            q.id,
            r.id
        );
    }
    // The story is one connected line: every main quest except the first has a main prerequisite
    // or is the head of its floor's chain.
    assert!(c.quests.iter().filter(|q| q.main).count() >= 20);
    // A floor never offers more than three side quests at once (no prerequisite, level reached).
    for zone in CAMPAIGN {
        let offered: Vec<&str> = c
            .quests
            .iter()
            .filter(|q| q.giver.zone == zone && !q.main && q.requires.is_none())
            .map(|q| q.id.as_str())
            .collect();
        assert!(offered.len() <= 3, "{zone} offers {offered:?} at once");
    }
}

#[test]
fn a_taken_pickup_stays_taken_across_a_save_and_is_reported() {
    let mut w = world("tower/1", 2);
    w.accept("helpdesk_lead", "passwords_on_post_its");
    w.use_prop("t1_password_1");
    assert_eq!(w.counts("passwords_on_post_its"), [1]);
    assert_eq!(w.sim.taken_props(w.p), ["t1_password_1"]);
    let Some(save) = w.sim.export_save(w.p) else {
        panic!("no save")
    };
    let text = save.encode();
    assert!(text.contains("taken_props=t1_password_1"), "{text}");
    // A new session: the pickup is still used up, and pressing F on it says so.
    let mut fresh = world("tower/1", 2);
    let Some(decoded) = combat_core::SaveData::decode(&text) else {
        panic!("save does not decode")
    };
    assert!(fresh.sim.import_save(fresh.p, &decoded));
    assert_eq!(fresh.sim.taken_props(fresh.p), ["t1_password_1"]);
    fresh.log.clear();
    fresh.use_prop("t1_password_1");
    assert!(fresh.log.iter().any(|e| matches!(
        e,
        Event::Interacted { prop, outcome, .. }
            if prop == "t1_password_1" && *outcome == combat_core::QUEST_PROP_TAKEN
    )));
}

#[test]
fn every_quest_item_has_its_own_icon() {
    let c = content();
    let mut seen: Vec<(String, &str)> = Vec::new();
    for q in &c.quests {
        for o in &q.objectives {
            let Objective::Loot(item, _) = o else {
                continue;
            };
            let Some(def) = c.item(item) else {
                panic!("{}: unknown item {item}", q.id)
            };
            if let Some((_, other)) = seen.iter().find(|(i, _)| *i == def.icon) {
                if other != item {
                    panic!("{item} and {other} share the icon {}", def.icon);
                }
            } else {
                seen.push((def.icon.clone(), item.as_str()));
            }
        }
    }
}
