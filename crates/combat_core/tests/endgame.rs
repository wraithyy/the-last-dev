//! Endgame backbone: Promotion ranks past the level cap, department reputation with vendor
//! gating, and the burnout meter (growth, drifts, relief and its cooldowns, quips, holidays,
//! saves). `endgame.rs`, `burnout.rs`, docs/FORMULAS.md.

mod common;

use combat_core::{
    BurnoutCause, Command, Content, EntityId, Event, ItemError, QuipKind, SaveData, Sim, Vec3,
};
use common::{content, def_mut};

/// 2026-10-15 00:00 UTC, a Thursday.
const THURSDAY: u64 = 1_792_022_400;
const DAY: u64 = 86_400;
const SECOND: u32 = 30;
const MINUTE: u32 = 60 * SECOND;

/// Packs far away, every prop and board right next to the player.
fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") || n == "boss" || n == "npc_dummy" => at(400.0, 400.0),
        "world_boss" | "graveyard" => at(0.0, -60.0),
        n if n.starts_with("night_") || n.starts_with("npc_") || n.starts_with("secret:") => {
            at(60.0, 60.0)
        }
        "scrum_bot" => at(2.0, 0.0),
        n if n.starts_with("prop:") => at(1.0, 0.0),
        _ => at(0.0, 0.0),
    }
}

fn world(zone: &str, level: u8, tweak: impl FnOnce(&mut Content)) -> (Sim, EntityId) {
    let mut c = content();
    for s in &mut c.spells {
        s.yells.clear();
    }
    tweak(&mut c);
    let mut sim = Sim::new(c, 21);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_level_of(p, level);
    if let Err(e) = sim.load_zone(zone, &anchors, Vec::new()) {
        panic!("{e}");
    }
    (sim, p)
}

fn run(sim: &mut Sim, ticks: u32) -> Vec<Event> {
    (0..ticks).flat_map(|_| sim.tick()).collect()
}

fn spawn(sim: &mut Sim, key: &str) -> EntityId {
    match sim.spawn_unit(key, Vec3::new(0.0, 0.0, -3.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{key}: {e}"),
    }
}

/// Spawns `key` next to the player and kills it as a player kill; the tick's events.
fn kill(sim: &mut Sim, key: &str) -> Vec<Event> {
    let id = spawn(sim, key);
    assert!(sim.debug_kill(id));
    run(sim, 1)
}

fn burnout(sim: &Sim, p: EntityId) -> u16 {
    sim.unit(p).map_or(0, |u| u.career.burnout)
}

fn rep(sim: &Sim, p: EntityId) -> [u32; 6] {
    sim.unit(p).map_or([0; 6], |u| u.career.rep)
}

fn causes(ev: &[Event]) -> Vec<(BurnoutCause, i16)> {
    ev.iter()
        .filter_map(|e| match e {
            Event::Burnout { cause, delta, .. } => Some((*cause, *delta)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_rank_curve_and_paragon_follow_the_formulas() {
    let c = content();
    let e = &c.endgame;
    assert_eq!(e.ranks.len(), 10);
    let curve: Vec<u32> = (0..=10).map(|r| e.rank_xp_to_next(r)).collect();
    assert_eq!(
        curve,
        [
            30000, 36000, 42000, 48000, 54000, 60000, 66000, 72000, 78000, 84000, 0
        ]
    );
    assert_eq!(curve.iter().sum::<u32>(), 570_000);
    let paragon: Vec<u32> = (0..=10).map(|r| e.paragon_pct(r)).collect();
    assert_eq!(paragon, [0, 1, 2, 3, 4, 5, 5, 5, 5, 5, 5]);
    assert_eq!(e.rank_name(1), Some("Senior Staff Engineer"));
    assert_eq!(e.rank_name(0), None);
    // Every rank's title is a real, wearable achievement.
    for k in 1..=10 {
        assert!(c.achievement(&format!("rank_{k}")).is_some(), "rank_{k}");
    }
}

#[test]
fn xp_past_the_cap_fills_promotion_ranks_with_paragon_and_a_title() {
    let (mut sim, p) = world("tower/2", 10, |c| {
        def_mut(c, "yes_man_advisor").xp_reward = 4000;
    });
    let (hp0, max0) = sim.unit(p).map_or((0, 0), |u| (u.hp, u.max_hp));
    let mut ev = Vec::new();
    let mut kills = 0;
    while sim.unit(p).is_some_and(|u| u.career.rank == 0) && kills < 40 {
        ev.extend(kill(&mut sim, "yes_man_advisor"));
        kills += 1;
    }
    let u = sim.unit(p).cloned().expect("player");
    assert_eq!((u.career.rank, u.level, u.xp, u.xp_to_next), (1, 10, 0, 0));
    assert!(kills > 1, "one kill must not be a promotion");
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::RankUp { rank: 1, .. }))
    );
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::RankProgress { gained, .. } if *gained > 0))
    );
    assert!(!ev.iter().any(|e| matches!(e, Event::XpGained { .. })));
    assert!(u.achievements.iter().any(|a| a == "rank_1"));
    assert_eq!(u.title, "rank_1");
    // Paragon: +1% max HP, and the rank-up does not heal.
    assert_eq!(u.max_hp, max0 * 101 / 100);
    assert_eq!(u.hp, hp0);
    assert!(u.career.rank_xp < sim.content().endgame.rank_xp_to_next(1));
}

#[test]
fn below_the_cap_xp_is_levels_and_the_last_rank_stops_counting() {
    let (mut sim, p) = world("tower/2", 9, |_| {});
    let ev = kill(&mut sim, "yes_man_advisor");
    assert!(ev.iter().any(|e| matches!(e, Event::XpGained { .. })));
    assert!(!ev.iter().any(|e| matches!(e, Event::RankProgress { .. })));
    sim.set_level_of(p, 10);
    sim.debug_set_career(p, 10, 0, [0; 6]);
    let ev = kill(&mut sim, "yes_man_advisor");
    assert!(!ev.iter().any(|e| matches!(e, Event::RankProgress { .. })));
    // The Paragon cap holds at the top rank.
    let max = sim.unit(p).map_or(0, |u| u.max_hp);
    sim.debug_set_career(p, 5, 0, [0; 6]);
    assert_eq!(sim.unit(p).map_or(0, |u| u.max_hp), max);
}

#[test]
fn kills_at_the_cap_earn_the_zone_department_by_kind() {
    let c = content();
    let sales = c.endgame.department_index("Sales").expect("Sales");
    let hr = c.endgame.department_index("HR").expect("HR");
    let (mut sim, p) = world("tower/3", 10, |_| {});
    kill(&mut sim, "sales_rep");
    assert_eq!(rep(&sim, p)[sales], 5);
    // Elites pay more; other departments do not notice.
    kill(&mut sim, "marketing_lead");
    assert_eq!(rep(&sim, p)[sales], 30);
    assert_eq!(rep(&sim, p)[hr], 0);
    let ev = kill(&mut sim, "customer");
    assert_eq!(rep(&sim, p)[sales], 105);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Reputation { dept, total: 105, gained: 75, .. } if usize::from(*dept) == sales
    )));
    // Below the cap kills give no standing.
    let (mut low, q) = world("tower/3", 9, |_| {});
    kill(&mut low, "sales_rep");
    assert_eq!(rep(&low, q), [0; 6]);
}

#[test]
fn tiers_have_thresholds_and_standing_is_capped() {
    let c = content();
    let e = &c.endgame;
    let names: Vec<&str> = e.rep_tiers.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Ignored",
            "Tolerated",
            "Cc'd",
            "Invited",
            "Trusted",
            "Exalted (Gets the Good Monitor)"
        ]
    );
    let tiers: Vec<usize> = [0, 299, 300, 899, 900, 2100, 4199, 4200, 8400, 99_999]
        .iter()
        .map(|r| e.tier_of(*r))
        .collect();
    assert_eq!(tiers, [0, 0, 1, 1, 2, 3, 3, 4, 5, 5]);
    let (mut sim, p) = world("tower/3", 10, |_| {});
    sim.debug_set_career(p, 0, 0, [0, 0, 0, 0, 0, 8390]);
    kill(&mut sim, "customer");
    assert_eq!(rep(&sim, p)[5], 8400);
    let ev = kill(&mut sim, "customer");
    assert!(!ev.iter().any(|e| matches!(e, Event::Reputation { .. })));
}

#[test]
fn the_world_boss_pays_every_department() {
    let (mut sim, p) = world("campus", 10, |_| {});
    let ev = kill(&mut sim, "the_board");
    assert_eq!(rep(&sim, p), [150; 6]);
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, Event::Reputation { gained: 150, .. }))
            .count(),
        6
    );
}

#[test]
fn dailies_go_to_the_least_impressed_department() {
    let (mut sim, p) = world("campus", 4, |_| {});
    sim.set_clock(THURSDAY + 12 * 3600);
    assert!(sim.debug_join_standup(p));
    run(&mut sim, 2);
    let board = sim.task_board(p);
    let dailies: Vec<_> = board.tasks.iter().filter(|t| !t.weekly).cloned().collect();
    assert!(dailies.len() >= 2);
    for (k, d) in dailies.iter().take(2).enumerate() {
        let counts: Vec<u32> = d.goals.iter().map(|g| g.need).collect();
        assert!(sim.debug_task_counts(p, &d.key, &counts));
        run(&mut sim, SECOND);
        let mut want = [0; 6];
        for w in want.iter_mut().take(k + 1) {
            *w = 80;
        }
        assert_eq!(rep(&sim, p), want, "after daily {k}");
    }
}

#[test]
fn rep_gated_vendor_items_need_the_tier() {
    let c = content();
    let (mut sim, p) = world("campus", 10, |_| {});
    let v = spawn(&mut sim, "hardware_broker");
    let item = "CODEOWNERS Mechanical Keyboard".to_string();
    let vendor = c.economy.vendor_for("hardware_broker").expect("vendor");
    assert!(c.economy.stock(vendor, 0).contains(&item), "always listed");
    assert_eq!(c.economy.rep_gate(vendor, &item, &c), Some((0, 2)));
    sim.grant_sp(p, 100_000);
    let buy = Command::Buy {
        vendor: v,
        item: item.clone(),
    };
    sim.apply_as(p, buy.clone());
    let ev = run(&mut sim, 1);
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::ItemError {
            reason: ItemError::RepTooLow
        }
    )));
    // Tolerated is not enough, Cc'd is.
    sim.debug_set_career(p, 0, 0, [899, 0, 0, 0, 0, 0]);
    sim.apply_as(p, buy.clone());
    run(&mut sim, 1);
    assert!(!sim.unit(p).is_some_and(|u| u.bag.contains(&item)));
    sim.debug_set_career(p, 0, 0, [900, 0, 0, 0, 0, 0]);
    sim.apply_as(p, buy);
    run(&mut sim, 1);
    assert!(sim.unit(p).is_some_and(|u| u.bag.contains(&item)));
}

#[test]
fn deaths_meetings_and_sprints_grow_burnout() {
    let (mut sim, p) = world("tower/2", 5, |_| {});
    assert!(sim.debug_apply_aura(p, "standup", 2000));
    let ev = run(&mut sim, 1);
    assert_eq!(causes(&ev), [(BurnoutCause::Meeting, 20)]);
    // A harmful aura that is not a meeting does nothing.
    assert!(sim.debug_apply_aura(p, "overpromise", 2000));
    assert!(causes(&run(&mut sim, 1)).is_empty());
    assert!(sim.debug_kill_player(p));
    let ev = run(&mut sim, 1);
    assert_eq!(causes(&ev), [(BurnoutCause::Death, 80)]);
    assert_eq!(burnout(&sim, p), 100);
}

#[test]
fn crunch_nights_and_long_sessions_drift_up_once_a_minute() {
    let (mut sim, p) = world("campus", 5, |c| c.endgame.burnout.session_grace_min = 1);
    // 2026-10-15 at 18:30 game time: night (a game day is 24 real minutes).
    let day0 = THURSDAY - THURSDAY % combat_core::live::GAME_DAY_S;
    sim.set_clock(day0 + u64::from(combat_core::live::DUSK_MIN) + 30);
    sim.set_hold_night(false);
    let ev = run(&mut sim, MINUTE);
    assert_eq!(causes(&ev), [(BurnoutCause::Crunch, 10)]);
    let ev = run(&mut sim, MINUTE);
    let c = causes(&ev);
    assert!(c.contains(&(BurnoutCause::LongSession, 5)), "{c:?}");
    assert_eq!(burnout(&sim, p), 25);
}

#[test]
fn friday_afternoons_and_the_rubber_duck_drift_down() {
    let (mut sim, p) = world("campus", 5, |_| {});
    sim.debug_set_burnout(p, 500);
    // Friday 13:00 UTC; the game clock is in the daytime then.
    sim.set_clock(THURSDAY + DAY + 13 * 3600);
    let ev = run(&mut sim, MINUTE);
    assert!(causes(&ev).contains(&(BurnoutCause::Friday, -20)), "{ev:?}");
    assert!(sim.grant_pet(p, "rubber_duck"));
    sim.apply_as(
        p,
        Command::SummonPet {
            pet: "rubber_duck".into(),
        },
    );
    let ev = run(&mut sim, MINUTE);
    let c = causes(&ev);
    assert!(c.contains(&(BurnoutCause::Duck, -10)), "{c:?}");
    assert!(c.contains(&(BurnoutCause::Friday, -20)), "{c:?}");
    assert_eq!(burnout(&sim, p), 450);
}

#[test]
fn office_props_relieve_burnout_once_per_their_own_cooldown() {
    let (mut sim, p) = world("tower/1", 5, |_| {});
    sim.debug_set_burnout(p, 600);
    run(&mut sim, 1);
    let use_prop = |sim: &mut Sim, key: &str| {
        sim.apply_as(p, Command::Interact { prop: key.into() });
        run(sim, 1)
    };
    let ev = use_prop(&mut sim, "foosball_open_space");
    assert!(ev.iter().any(|e| matches!(e, Event::Interacted { .. })));
    assert_eq!(causes(&ev), [(BurnoutCause::Prop, -120)]);
    // The table can be played again at once, but the relief waits for its own cooldown.
    run(&mut sim, 2 * SECOND);
    let ev = use_prop(&mut sim, "foosball_open_space");
    assert!(ev.iter().any(|e| matches!(e, Event::Interacted { .. })));
    assert!(causes(&ev).is_empty());
    assert!(sim.calm_ready_in_ms(p, "foosball_open_space") > 170_000);
    // Each prop keeps its own clock.
    let ev = use_prop(&mut sim, "beanbag_open_space");
    assert_eq!(causes(&ev), [(BurnoutCause::Prop, -150)]);
    run(&mut sim, 180 * SECOND);
    let ev = use_prop(&mut sim, "foosball_open_space");
    assert_eq!(causes(&ev), [(BurnoutCause::Prop, -120)]);
    assert_eq!(burnout(&sim, p), 600 - 120 - 150 - 120);
}

#[test]
fn a_coffee_break_helps_once_per_cooldown() {
    let (mut sim, p) = world("campus", 5, |_| {});
    sim.debug_set_burnout(p, 500);
    let coffee = sim.content().channel_spell().map(|s| s.id).expect("coffee");
    let brew = |sim: &mut Sim| {
        sim.set_player_mana(1);
        sim.apply_as(
            p,
            Command::Cast {
                caster: p,
                spell: coffee,
            },
        );
        run(sim, 3 * SECOND)
    };
    assert!(causes(&brew(&mut sim)).contains(&(BurnoutCause::CoffeeBreak, -30)));
    sim.apply_as(p, Command::StopCasting { caster: p });
    run(&mut sim, SECOND);
    assert!(!causes(&brew(&mut sim)).contains(&(BurnoutCause::CoffeeBreak, -30)));
}

#[test]
fn defeating_management_is_therapeutic_and_bosses_more_so() {
    let (mut sim, p) = world("tower/2", 5, |_| {});
    sim.debug_set_burnout(p, 800);
    run(&mut sim, 1);
    assert_eq!(
        causes(&kill(&mut sim, "middle_manager")),
        [(BurnoutCause::Kill, -50)]
    );
    assert!(causes(&kill(&mut sim, "meatproxy")).is_empty());
    assert_eq!(
        causes(&kill(&mut sim, "vp_synergy")),
        [(BurnoutCause::Kill, -150)]
    );
    assert_eq!(burnout(&sim, p), 600);
}

#[test]
fn burnt_out_players_yawn_mutter_and_reply_all() {
    let (mut sim, p) = world("campus", 5, |c| c.endgame.burnout.quip_pct = 100);
    sim.debug_set_burnout(p, 100);
    let ev = run(&mut sim, 3 * MINUTE);
    assert!(!ev.iter().any(|e| matches!(e, Event::BurnoutQuip { .. })));
    sim.debug_set_burnout(p, 900);
    let ev = run(&mut sim, 20 * MINUTE);
    let quips: Vec<(QuipKind, u16)> = ev
        .iter()
        .filter_map(|e| match e {
            Event::BurnoutQuip { kind, line, .. } => Some((*kind, *line)),
            _ => None,
        })
        .collect();
    assert!(quips.len() >= 20, "{quips:?}");
    let def = &sim.content().endgame.burnout;
    for (k, l) in &quips {
        assert!(def.quip_line(*k, *l).is_some());
    }
    for kind in [QuipKind::Yawn, QuipKind::Mutter, QuipKind::ReplyAll] {
        assert!(quips.iter().any(|(k, _)| *k == kind), "{kind:?}");
    }
}

#[test]
fn effects_scale_with_burnout() {
    let c = content();
    let b = &c.endgame.burnout;
    assert_eq!((b.speed_pct(749), b.speed_pct(750)), (100, 90));
    assert_eq!(b.hair_shift(0), 0.0);
    assert_eq!(b.hair_shift(1000), 25.0);
    assert_eq!(b.tip(720), "Have you tried a wellness webinar?");
    assert!(!b.tip(0).is_empty());
}

#[test]
fn time_logged_off_is_a_holiday_and_progress_survives_the_save() {
    let (mut sim, p) = world("tower/2", 10, |_| {});
    sim.set_clock(THURSDAY);
    sim.debug_set_career(p, 3, 1234, [10, 20, 30, 40, 50, 60]);
    sim.debug_set_burnout(p, 600);
    let save = sim.export_save(p).expect("save");
    assert_eq!(
        (
            save.rank,
            save.rank_xp,
            save.rep,
            save.burnout,
            save.burnout_at
        ),
        (3, 1234, [10, 20, 30, 40, 50, 60], 600, THURSDAY)
    );
    let back = SaveData::decode(&save.encode()).expect("decode");
    assert_eq!(back, save);
    let load = |unix: Option<u64>| {
        let mut s = Sim::new(content(), 3);
        if let Some(t) = unix {
            s.set_clock(t);
        }
        let q = s.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
        assert!(s.import_save(q, &back));
        let u = s.unit(q).cloned().expect("player");
        (u.career, s.tick())
    };
    // Five hours away: -50%.
    let (career, ev) = load(Some(THURSDAY + 5 * 3600 + 10));
    assert_eq!(
        (career.rank, career.rank_xp, career.rep, career.burnout),
        (3, 1234, back.rep, 100)
    );
    assert_eq!(causes(&ev), [(BurnoutCause::Holiday, -500)]);
    // A quick restart is no holiday, and without a clock nobody knows.
    assert_eq!(load(Some(THURSDAY + 600)).0.burnout, 600);
    assert_eq!(load(None).0.burnout, 600);
    // Older saves have no endgame keys at all.
    let old = SaveData::decode("the-last-dev save\nversion=3\nclass=devops\nlevel=10\n")
        .expect("old save");
    assert_eq!((old.rank, old.rep, old.burnout), (0, [0; 6], 0));
}
