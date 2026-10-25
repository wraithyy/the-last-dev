//! The Company Calendar (E5): the listing and its rotation flag, chain statuses, every quest
//! chain from the board to its last turn-in, Jira tickets as loot and their turn-in, saves.

mod common;

use combat_core::{
    ChainStatus, Command, Content, EntityId, Event, QuestStatus, SaveData, Sim, TICKET_BOARD_MULT,
    Vec3, ms_to_ticks,
};
use common::*;

/// Every NPC far apart on a line, every prop at the origin (where the player starts).
fn anchor(name: &str) -> Option<(Vec3, f32)> {
    if name.starts_with("prop:") || name == "player_spawn" || name == "graveyard" {
        return Some((Vec3::new(0.0, 0.0, 0.0), 0.0));
    }
    // FNV-1a of the name: a stable, distinct spot per anchor, far from the origin.
    let h = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    let x = 200.0 + (h % 4000) as f32;
    Some((Vec3::new(x, 0.0, 300.0), 0.0))
}

struct T {
    sim: Sim,
    p: EntityId,
    log: Vec<Event>,
}

fn t_with(tweak: impl FnOnce(&mut Content)) -> T {
    let mut c = content();
    for u in &mut c.units {
        if u.key == "senior_engineer" {
            u.max_hp = 100_000;
        }
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, 5);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_player_level(2);
    let mut t = T {
        sim,
        p,
        log: Vec::new(),
    };
    t.load("campus");
    t
}

fn t() -> T {
    t_with(|_| {})
}

impl T {
    fn tick(&mut self) {
        let ev = self.sim.tick();
        self.log.extend(ev);
    }

    fn secs(&mut self, s: u64) {
        for _ in 0..ms_to_ticks(1000) * s {
            self.tick();
        }
    }

    fn load(&mut self, zone: &str) {
        if let Err(e) = self.sim.load_zone(zone, &anchor, Vec::new()) {
            panic!("load_zone {zone}: {e}");
        }
        self.tick();
    }

    fn cmd(&mut self, c: Command) {
        self.sim.apply_as(self.p, c);
        self.tick();
    }

    fn go(&mut self, pos: Vec3) {
        self.cmd(Command::PlayerMoved {
            pos,
            yaw: 0.0,
            moving: false,
            jumping: false,
        });
    }

    fn talk_to(&mut self, key: &str) {
        let Some((id, at)) = self
            .sim
            .units()
            .find(|u| u.key == key && u.alive)
            .map(|u| (u.id, u.pos))
        else {
            panic!("no living {key} in {:?}", self.sim.zone())
        };
        self.go(Vec3::new(at.x, 0.0, at.z + 2.0));
        self.cmd(Command::Talk { unit: id });
    }

    fn accept(&mut self, q: &str) {
        self.cmd(Command::AcceptQuest { quest: q.into() });
    }

    fn turn_in(&mut self, q: &str) {
        self.cmd(Command::TurnInQuest {
            quest: q.into(),
            choice: 0,
        });
    }

    fn status(&self, q: &str) -> QuestStatus {
        self.sim.quest_status(q)
    }

    fn me(&self) -> combat_core::UnitState {
        match self.sim.unit(self.p) {
            Some(u) => u.clone(),
            None => panic!("no player"),
        }
    }

    fn chain_status(&self, key: &str) -> ChainStatus {
        let c = self
            .sim
            .content()
            .calendar
            .chains
            .iter()
            .find(|c| c.key == key);
        match c {
            Some(c) => c.status(self.sim.quests()),
            None => panic!("no chain {key}"),
        }
    }

    /// From the board's spot (far from every giver): the calendar hands out the first quest.
    fn start_chain(&mut self, key: &str) {
        let first = self
            .sim
            .content()
            .calendar
            .chains
            .iter()
            .find(|c| c.key == key)
            .and_then(|c| c.quests.first().cloned())
            .unwrap_or_default();
        self.go(Vec3::new(0.0, 0.0, 0.0));
        self.accept(&first);
        assert_eq!(
            self.status(&first),
            QuestStatus::Active,
            "{key} did not start"
        );
    }

    /// Turns `q` in at its giver, then takes `next` there.
    fn hand_in(&mut self, giver: &str, q: &str, next: Option<&str>) {
        self.talk_to(giver);
        assert_eq!(self.status(q), QuestStatus::Ready, "{q}");
        self.turn_in(q);
        assert_eq!(self.status(q), QuestStatus::Done, "{q}");
        if let Some(n) = next {
            self.accept(n);
            assert_eq!(self.status(n), QuestStatus::Active, "{n}");
        }
    }

    fn has_title(&self, key: &str) -> bool {
        self.me().achievements.iter().any(|a| a == key)
    }

    fn rep(&self, dept: &str) -> u32 {
        let d = self.sim.content().endgame.department_index(dept);
        d.map_or(0, |d| self.me().career.rep[d])
    }
}

#[test]
fn the_calendar_lists_every_minigame_and_chain_with_rotation_off() {
    let c = content();
    assert!(!c.calendar.rotation, "rotation must ship off");
    for day in [0, 1, 20_000, 20_001] {
        let (games, chains) = c.calendar.listing(day);
        assert_eq!(games.len(), c.calendar.minigames.len());
        assert_eq!(chains.len(), c.calendar.chains.len());
    }
    let names: Vec<&str> = c
        .calendar
        .minigames
        .iter()
        .map(|g| g.name.as_str())
        .collect();
    for want in [
        "Mandatory Fun: Teambuilding",
        "Performance Review",
        "Reorg",
        "Tool Migration",
        "Hackathon",
        "Open-Space Focus",
        "Stay Green",
    ] {
        assert!(names.contains(&want), "{want} missing from {names:?}");
    }
    // Teambuilding is playable; every other slot is "Coming soon" until its MinigameDef lands.
    assert!(c.minigame("teambuilding").is_some());
    let chains: Vec<&str> = c.calendar.chains.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        chains,
        [
            "expense_report",
            "exit_interview",
            "mandatory_training",
            "return_to_office",
            "town_hall",
            "prod_incident",
            "flaky_tests"
        ]
    );
    for g in &c.calendar.minigames {
        assert!(
            !g.blurb.is_empty() && g.length.starts_with('~'),
            "{}",
            g.key
        );
    }
    for ch in &c.calendar.chains {
        assert!(
            !ch.blurb.is_empty() && ch.length.starts_with('~'),
            "{}",
            ch.key
        );
        assert!(!ch.rewards.is_empty(), "{}", ch.key);
    }
}

#[test]
fn rotation_on_lists_one_of_each_and_only_today_starts_remotely() {
    let mut t = t_with(|c| c.calendar.rotation = true);
    let day = 20_000_u32;
    t.sim.set_clock(u64::from(day) * 86_400 + 3_600);
    t.tick();
    let (games, chains) = t.sim.content().calendar.listing(day);
    assert_eq!((games.len(), chains.len()), (1, 1));
    let today = chains[0].quests[0].clone();
    let other = t
        .sim
        .content()
        .calendar
        .chains
        .iter()
        .find(|c| c.quests[0] != today)
        .map(|c| c.quests[0].clone())
        .unwrap_or_default();
    t.accept(&other);
    // Gated heads read Locked at the giver, but the point is that nothing was handed out.
    assert_ne!(t.status(&other), QuestStatus::Active);
    t.accept(&today);
    assert_eq!(t.status(&today), QuestStatus::Active);
}

#[test]
fn only_a_chain_start_is_handed_out_remotely() {
    let mut t = t();
    // An ordinary quest still needs its giver.
    t.accept("onboarding");
    assert_eq!(t.status("onboarding"), QuestStatus::Available);
    t.start_chain("expense_report");
    assert_eq!(t.chain_status("expense_report"), ChainStatus::InProgress);
    assert_eq!(t.chain_status("town_hall"), ChainStatus::New);
    // The second step is not a start: done remotely it stays where it is.
    t.talk_to("barista");
    t.go(Vec3::new(0.0, 0.0, 0.0));
    t.turn_in("er_receipt");
    assert_eq!(t.status("er_receipt"), QuestStatus::Ready);
    t.hand_in("expense_controller", "er_receipt", None);
    t.go(Vec3::new(0.0, 0.0, 0.0));
    t.accept("er_approvals");
    assert_eq!(t.status("er_approvals"), QuestStatus::Available);
}

#[test]
fn expense_report_costs_burnout_and_pays_three_kudos() {
    let mut t = t();
    t.start_chain("expense_report");
    t.talk_to("barista");
    t.hand_in("expense_controller", "er_receipt", Some("er_approvals"));
    for npc in [
        "intern",
        "facilities_guy",
        "procurement_officer",
        "bl_account_manager",
    ] {
        t.talk_to(npc);
    }
    t.hand_in(
        "expense_controller",
        "er_approvals",
        Some("er_justification"),
    );
    t.secs(46);
    let before = t.me();
    t.hand_in("expense_controller", "er_justification", None);
    let after = t.me();
    assert_eq!(after.career.burnout, before.career.burnout + 80);
    assert!(after.sp >= before.sp + 3);
    assert!(t.has_title("reimbursed_in_spirit"));
    assert!(
        after
            .inventory
            .iter()
            .any(|(n, _)| n == "Approved Coffee Receipt (Napkin)")
    );
    assert!(t.log.iter().any(|e| matches!(
        e,
        Event::Burnout {
            delta: 80,
            cause: combat_core::BurnoutCause::Quest,
            ..
        }
    )));
    assert_eq!(t.chain_status("expense_report"), ChainStatus::Done);
}

#[test]
fn exit_interview_relieves_burnout() {
    let mut t = t();
    t.sim.debug_set_burnout(t.p, 500);
    t.start_chain("exit_interview");
    t.talk_to("hr_onboarding_bot");
    t.hand_in("leaving_colleague", "ex_last_day", Some("ex_handover"));
    t.talk_to("smoking_corner_dev");
    t.hand_in("leaving_colleague", "ex_handover", Some("ex_interview"));
    t.secs(36);
    t.hand_in("leaving_colleague", "ex_interview", None);
    assert_eq!(t.me().career.burnout, 380);
    assert_eq!(t.rep("HR"), 40);
    assert!(t.has_title("said_it_out_loud"));
    assert_eq!(t.chain_status("exit_interview"), ChainStatus::Done);
}

#[test]
fn mandatory_training_ends_at_it_support_and_pays_security() {
    let mut t = t();
    t.start_chain("mandatory_training");
    t.secs(51);
    t.hand_in("security_trainer", "mt_slides", Some("mt_report_phish"));
    t.load("tower/1");
    t.talk_to("it_support");
    assert_eq!(t.status("mt_report_phish"), QuestStatus::Ready);
    t.load("campus");
    t.hand_in("security_trainer", "mt_report_phish", None);
    assert_eq!(t.rep("Security"), 60);
    assert!(t.has_title("did_not_click"));
    assert_eq!(t.chain_status("mandatory_training"), ChainStatus::Done);
}

#[test]
fn return_to_office_waits_in_the_open_space_only() {
    let mut t = t();
    t.start_chain("return_to_office");
    t.talk_to("facilities_guy");
    t.hand_in("rto_coordinator", "rto_commute", Some("rto_collaborate"));
    // Waiting on the campus does not count: the team is in the tower.
    t.secs(5);
    assert_eq!(t.sim.quests().active[0].counts, vec![0]);
    t.load("tower/1");
    t.secs(41);
    assert_eq!(t.status("rto_collaborate"), QuestStatus::Ready);
    t.load("campus");
    t.hand_in("rto_coordinator", "rto_collaborate", Some("rto_survey"));
    t.talk_to("burnt_out_colleague");
    t.hand_in("rto_coordinator", "rto_survey", None);
    assert_eq!(t.rep("HR"), 50);
    assert_eq!(t.chain_status("return_to_office"), ChainStatus::Done);
}

#[test]
fn town_hall_answers_one_question() {
    let mut t = t();
    t.start_chain("town_hall");
    t.talk_to("intern");
    t.hand_in("comms_lead", "th_question", Some("th_stream"));
    t.secs(46);
    t.hand_in("comms_lead", "th_stream", Some("th_qa"));
    t.talk_to("burnt_out_colleague");
    t.hand_in("comms_lead", "th_qa", None);
    assert_eq!(t.rep("Marketing"), 60);
    assert!(t.has_title("pre_approved_question"));
    assert_eq!(t.chain_status("town_hall"), ChainStatus::Done);
}

fn tickets(c: &Content) -> Vec<(String, u32)> {
    c.economy
        .junk
        .iter()
        .filter(|j| j.ticket)
        .map(|j| (j.name.clone(), j.price))
        .collect()
}

#[test]
fn campus_corpses_drop_jira_tickets() {
    let c = content();
    let names: Vec<String> = tickets(&c).into_iter().map(|t| t.0).collect();
    assert_eq!(
        names,
        [
            "Jira Ticket: Blocker",
            "Jira Ticket: Won't Fix",
            "Jira Ticket: Duplicate"
        ]
    );
    // Tickets are tier-free junk: any trash anywhere can drop one.
    assert!(
        c.economy
            .junk
            .iter()
            .filter(|j| j.ticket)
            .all(|j| j.tier == 0)
    );
    let mut sim = Sim::new(c, 11);
    let _p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let all = |_: &str| Some((Vec3::new(0.0, 0.0, 0.0), 0.0));
    let mut dropped = 0;
    for _ in 0..40 {
        if let Err(e) = sim.load_zone("campus", &all, Vec::new()) {
            panic!("{e}");
        }
        let ids: Vec<u64> = sim
            .units()
            .filter(|u| u.hostile && u.alive)
            .map(|u| u.id)
            .collect();
        for id in ids {
            sim.debug_kill(id);
        }
        for e in sim.tick() {
            if let Event::LootDropped { items, .. } = e {
                dropped += items.iter().filter(|i| names.contains(i)).count();
            }
        }
    }
    assert!(dropped > 0, "no ticket in 40 campus clears");
}

#[test]
fn the_board_and_the_calendar_close_tickets_for_kudos() {
    let mut t = t();
    let list = tickets(t.sim.content());
    for (n, _) in &list {
        assert!(t.sim.grant_goods(t.p, n));
    }
    assert!(t.sim.grant_goods(t.p, "Ibuprofen"));
    let sp0 = t.me().sp;
    t.cmd(Command::Interact {
        prop: "company_calendar".into(),
    });
    let want: u32 = list.iter().map(|t| t.1 * TICKET_BOARD_MULT).sum();
    let me = t.me();
    assert_eq!(me.sp, sp0 + want);
    assert_eq!(me.bag, ["Ibuprofen"]);
    assert!(t.log.iter().any(|e| matches!(
        e,
        Event::TicketsClosed { count: 3, sp, .. } if *sp == want
    )));
    // The Scrum board takes them too; with no tickets nothing is said.
    assert!(t.sim.grant_goods(t.p, &list[0].0));
    t.secs(3);
    t.log.clear();
    t.cmd(Command::Interact {
        prop: "scrum_board".into(),
    });
    assert!(t.me().bag == ["Ibuprofen"]);
    t.secs(3);
    t.log.clear();
    t.cmd(Command::Interact {
        prop: "scrum_board".into(),
    });
    assert!(
        !t.log
            .iter()
            .any(|e| matches!(e, Event::TicketsClosed { .. }))
    );
}

#[test]
fn chains_and_tickets_survive_a_save() {
    let mut t = t();
    t.start_chain("town_hall");
    t.talk_to("intern");
    t.hand_in("comms_lead", "th_question", Some("th_stream"));
    t.secs(10);
    assert!(t.sim.grant_goods(t.p, "Jira Ticket: Won't Fix"));
    t.tick();
    let Some(save) = t.sim.export_save(t.p) else {
        panic!("no save")
    };
    let Some(back) = SaveData::decode(&save.encode()) else {
        panic!("decode")
    };
    assert_eq!(back, save);
    let mut fresh = Sim::new(content(), 9);
    let q = fresh.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    assert!(fresh.import_save(q, &back));
    let ch = fresh
        .content()
        .calendar
        .chains
        .iter()
        .find(|c| c.key == "town_hall")
        .map(|c| {
            (
                c.status(fresh.quests()),
                c.current(fresh.quests()).map(String::from),
            )
        });
    assert_eq!(
        ch,
        Some((ChainStatus::InProgress, Some("th_stream".to_string())))
    );
    assert!(
        fresh
            .quests()
            .active
            .iter()
            .any(|a| a.id == "th_stream" && a.counts[0] >= 9)
    );
    assert!(
        fresh
            .unit(q)
            .is_some_and(|u| u.bag.iter().any(|b| b == "Jira Ticket: Won't Fix"))
    );
}
