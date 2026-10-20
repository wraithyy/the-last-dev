//! Car Park P3, the starter zone: every ticket there is finishable by one level-1 player in the
//! shipped layout, the tutorial's quest is handed out there, the gate leads to the campus, and
//! units keep to the ground on the exit ramp.

mod common;

use combat_core::{Command, EntityId, Event, QuestStatus, Sim, Vec3, ms_to_ticks};
use common::{Geo, content, def_mut, geometry};

const ZONE: &str = "parking_p3";

struct W {
    sim: Sim,
    p: EntityId,
    geo: std::collections::HashMap<String, Geo>,
    log: Vec<Event>,
}

impl W {
    fn new() -> W {
        let mut c = content();
        // Strong enough that the test is about the tickets, not the fights (balance is the
        // playthrough bot's job, which clears P3 solo with every class).
        let me = def_mut(&mut c, "senior_engineer");
        me.max_hp = 100_000;
        let mut sim = Sim::new(c, 9);
        sim.set_wandering(false);
        let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
        let mut w = W {
            sim,
            p,
            geo: geometry(),
            log: Vec::new(),
        };
        w.load(ZONE);
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

    fn move_to(&mut self, pos: Vec3) {
        self.cmd(Command::PlayerMoved {
            pos,
            yaw: 0.0,
            moving: false,
            jumping: false,
        });
    }

    fn anchor(&self, name: &str) -> Vec3 {
        let zone = self.sim.zone().unwrap_or_default().to_string();
        match self.geo.get(&zone).and_then(|g| g.anchors.get(name)) {
            Some((p, _)) => *p,
            None => panic!("{zone}: no anchor {name}"),
        }
    }

    fn talk(&mut self, key: &str) {
        let Some((id, at)) = self
            .sim
            .units()
            .find(|u| u.key == key && u.alive)
            .map(|u| (u.id, u.pos))
        else {
            panic!("no {key}")
        };
        self.move_to(Vec3::new(at.x + 1.0, at.y, at.z));
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
        self.cmd(Command::TurnInQuest {
            quest: q.to_string(),
            choice: 0,
        });
        assert_eq!(self.sim.quest_status(q), QuestStatus::Done, "{q}");
    }

    /// A player kill of every living `key`, each corpse looted on the spot.
    fn kill_and_loot(&mut self, key: &str, n: usize) {
        let ids: Vec<EntityId> = self
            .sim
            .units()
            .filter(|u| u.alive && u.key == key)
            .map(|u| u.id)
            .take(n)
            .collect();
        assert_eq!(ids.len(), n, "{n} {key} in P3");
        for id in ids {
            let at = self.sim.unit(id).map(|u| u.pos).unwrap_or_default();
            self.move_to(Vec3::new(at.x + 1.0, at.y, at.z));
            assert!(self.sim.debug_kill(id));
            self.tick();
            self.cmd(Command::Loot {
                looter: self.p,
                corpse: id,
            });
        }
    }

    fn use_prop(&mut self, prop: &str) {
        let at = self.anchor(&format!("prop:{prop}"));
        self.move_to(at);
        self.cmd(Command::Interact {
            prop: prop.to_string(),
        });
    }

    fn secs(&mut self, s: u64) {
        for _ in 0..ms_to_ticks(1000) * s {
            self.tick();
        }
    }
}

#[test]
fn every_starter_ticket_is_finishable_solo_and_the_gate_leads_on() {
    let mut w = W::new();
    assert_eq!(w.sim.zone(), Some(ZONE));
    // The tutorial's own ticket first, from the HR bot beside your car.
    w.accept("hr_onboarding_bot", "mandatory_compliance");
    w.kill_and_loot("training_dummy", 1);
    w.turn_in("mandatory_compliance");

    w.accept("badge_clerk", "p3_badge_photo");
    w.use_prop("badge_photo_booth");
    w.secs(16);
    w.turn_in("p3_badge_photo");

    // The vibecoder's hotfix is interact-and-wait only: no kills, giver and prop side by side.
    w.accept("p3_vibecoder", "p3_hotfix_hatchback");
    w.use_prop("p3_prod_lunchbox");
    w.secs(13);
    w.turn_in("p3_hotfix_hatchback");

    w.accept("badge_clerk", "p3_feedback_loop");
    w.accept("p3_attendant", "p3_pay_and_display");
    w.accept("lanyard_vendor", "p3_lost_lanyards");
    w.kill_and_loot("survey_ambusher", 3);
    w.kill_and_loot("clamp_drone", 4);
    for k in 1..=3 {
        w.use_prop(&format!("p3_lanyard_{k}"));
    }
    for q in ["p3_feedback_loop", "p3_pay_and_display", "p3_lost_lanyards"] {
        assert_eq!(w.sim.quest_status(q), QuestStatus::Ready, "{q}");
        w.turn_in(q);
    }

    w.accept("p3_attendant", "p3_raise_the_barrier");
    w.kill_and_loot("exit_barrier", 1);
    w.turn_in("p3_raise_the_barrier");

    // The breadcrumb: the intern's ticket, closed on the campus.
    w.accept("intern", "onboarding");
    assert!(
        w.sim
            .zone_links(ZONE)
            .iter()
            .any(|l| l.to == "campus" && l.unlocked),
        "the gate is open"
    );
    w.load("campus");
    w.talk("facilities_guy");
    assert_eq!(w.sim.quest_status("onboarding"), QuestStatus::Ready);
    w.turn_in("onboarding");
    let me = w.sim.unit(w.p).map_or(0, |u| u.level);
    assert!(me >= 2, "P3 should leave a new hire at level 2, got {me}");
}

#[test]
fn units_stay_on_the_exit_ramp_ground() {
    let mut w = W::new();
    let g = &w.geo[ZONE];
    let boss = w.anchor("boss");
    // The barrier stands on the raised deck at the top of the ramp.
    assert!(boss.y > 2.0, "deck height {}", boss.y);
    assert!((g.terrain.height(boss.x, boss.z) - boss.y).abs() < 1e-3);
    w.secs(1);
    let Some((id, pos)) = w
        .sim
        .units()
        .find(|u| u.key == "exit_barrier")
        .map(|u| (u.id, u.pos))
    else {
        panic!("no barrier")
    };
    assert!((pos.y - w.sim.ground_at(pos.x, pos.z)).abs() < 1e-3);
    // Pull it down the ramp: every step keeps it on the slope.
    let foot = Vec3::new(0.0, 0.0, -48.0);
    w.move_to(foot);
    w.cmd(Command::SetTarget {
        unit: w.p,
        target: Some(id),
    });
    let bar = w.sim.content().bar_for_unit("senior_engineer");
    let Some(&spell) = bar.first() else {
        panic!("no class spell")
    };
    w.cmd(Command::Cast { caster: w.p, spell });
    let mut on_slope = false;
    for _ in 0..ms_to_ticks(8000) {
        w.tick();
        let Some(b) = w.sim.unit(id) else { break };
        let ground = w.sim.ground_at(b.pos.x, b.pos.z);
        assert!(
            (b.pos.y - ground).abs() < 1e-3,
            "{:?} vs ground {ground}",
            b.pos
        );
        on_slope |= ground > 0.3 && ground < 2.2;
    }
    assert!(on_slope, "the barrier never walked down the ramp");
}
