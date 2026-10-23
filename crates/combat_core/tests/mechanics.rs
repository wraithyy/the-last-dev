//! Enemy mechanics: ground telegraphs, must-interrupt casts, frontal shields, dispel types,
//! summoners, enrage timers and the CEO's flight to the elevator.

mod common;

use combat_core::{
    AiAbility, AiTarget, Command, Content, ContentError, DispelType, Event, Sim, SpellId,
    TelegraphShape, Vec3,
};
use common::*;

const ROLLBACK: SpellId = SpellId(7);
const MEETING_ROOM: SpellId = SpellId(602);
const REPLY_ALL: SpellId = SpellId(605);
const SYNERGY: SpellId = SpellId(631);
const EARNINGS: SpellId = SpellId(635);
const SEVERANCE: SpellId = SpellId(637);
const CONE: SpellId = SpellId(608);

struct W {
    sim: Sim,
    p: u64,
    log: Vec<(u64, Event)>,
}

/// Exact numbers: no misses, crits, regen or yells; every spell learned at level 1.
fn world(tweak: impl FnOnce(&mut Content)) -> W {
    let mut c = content();
    c.config.white_hit_pct = 100;
    c.config.spell_hit_pct = 100;
    c.config.regen.hp_pct = 0;
    c.config.regen.mana_pct = 0;
    for s in &mut c.spells {
        s.yells.clear();
        s.learn_level = 1;
    }
    def_mut(&mut c, "senior_engineer").crit_pct = 0;
    tweak(&mut c);
    let mut sim = Sim::new(c, 3);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_player_level(10);
    W {
        sim,
        p,
        log: Vec::new(),
    }
}

/// The unit uses only `spell`, first after `initial_ms`, then never again (unless `again`).
fn only(c: &mut Content, key: &str, spell: SpellId, initial_ms: u32, target: AiTarget) {
    let ai = match def_mut(c, key).ai.as_mut() {
        Some(ai) => ai,
        None => panic!("{key} has no ai"),
    };
    ai.abilities = vec![AiAbility {
        spell,
        cooldown_min_ms: 600_000,
        cooldown_max_ms: 600_000,
        initial_ms: Some((initial_ms, initial_ms)),
        target,
        phases: vec![],
        yells: vec![],
        requires_aura: None,
        mode: Default::default(),
        original_only: false,
    }];
    ai.melee_damage = 0;
    ai.enrage = None;
}

impl W {
    fn spawn(&mut self, key: &str, x: f32, z: f32) -> u64 {
        match self.sim.spawn_unit(key, Vec3::new(x, 0.0, z), 0.0) {
            Ok(id) => id,
            Err(e) => panic!("{e}"),
        }
    }

    fn tick(&mut self) {
        let now = self.sim.now().0 + 1;
        for e in self.sim.tick() {
            self.log.push((now, e));
        }
    }

    fn run(&mut self, n: u32) {
        for _ in 0..n {
            self.tick();
        }
    }

    fn run_until(&mut self, max: u32, pred: impl Fn(&Event) -> bool) -> bool {
        let start = self.log.len();
        for _ in 0..max {
            self.tick();
            if self.log[start..].iter().any(|(_, e)| pred(e)) {
                return true;
            }
        }
        false
    }

    fn move_to(&mut self, who: u64, x: f32, z: f32) {
        self.sim.apply_as(
            who,
            Command::PlayerMoved {
                pos: Vec3::new(x, 0.0, z),
                yaw: 0.0,
                moving: false,
                jumping: false,
            },
        );
    }

    fn cast_at(&mut self, who: u64, target: u64, spell: SpellId) {
        self.sim.apply_as(
            who,
            Command::SetTarget {
                unit: who,
                target: Some(target),
            },
        );
        self.sim.apply_as(who, Command::Cast { caster: who, spell });
    }

    fn damage(&self, target: u64, spell: SpellId) -> Vec<(u64, u32)> {
        self.log
            .iter()
            .filter_map(|(t, e)| match e {
                Event::Damage {
                    target: tg,
                    spell: Some(s),
                    amount,
                    absorbed,
                    ..
                } if *tg == target && *s == spell => Some((*t, amount + absorbed)),
                _ => None,
            })
            .collect()
    }

    fn started(&self) -> Vec<(u64, Event)> {
        self.log
            .iter()
            .filter(|(_, e)| matches!(e, Event::TelegraphStarted { .. }))
            .cloned()
            .collect()
    }
}

#[test]
fn shapes_contain_points_by_radius_angle_and_width() {
    let o = Vec3::new(0.0, 0.0, 0.0);
    let at = |x, z| Vec3::new(x, 0.0, z);
    let circle = TelegraphShape::Circle { radius_m: 4.0 };
    assert!(circle.contains(o, 0.0, at(3.9, 0.0)));
    assert!(!circle.contains(o, 0.0, at(4.1, 0.0)));
    // Yaw 0 faces -Z.
    let cone = TelegraphShape::Cone {
        radius_m: 8.0,
        angle_deg: 90.0,
    };
    assert!(cone.contains(o, 0.0, at(0.0, -7.0)));
    assert!(cone.contains(o, 0.0, at(3.0, -4.0)));
    assert!(
        !cone.contains(o, 0.0, at(5.0, -3.0)),
        "outside the 45 degree half-angle"
    );
    assert!(!cone.contains(o, 0.0, at(0.0, 3.0)), "behind the apex");
    assert!(!cone.contains(o, 0.0, at(0.0, -8.5)), "past the radius");
    let line = TelegraphShape::Line {
        length_m: 10.0,
        width_m: 2.0,
    };
    assert!(line.contains(o, 0.0, at(0.9, -9.0)));
    assert!(!line.contains(o, 0.0, at(1.1, -5.0)));
    assert!(!line.contains(o, 0.0, at(0.0, 1.0)));
    // Facing +X (yaw -90 degrees): the line now runs along +X.
    let east = -std::f32::consts::FRAC_PI_2;
    assert!(line.contains(o, east, at(6.0, 0.5)));
    assert!(!line.contains(o, east, at(-1.0, 0.0)));
}

#[test]
fn telegraph_lands_after_its_delay_and_only_on_who_is_inside() {
    for dodge in [false, true] {
        let mut w = world(|c| only(c, "middle_manager", MEETING_ROOM, 500, AiTarget::Current));
        let mm = w.spawn("middle_manager", 0.0, -5.0);
        assert!(w.run_until(60, |e| matches!(e, Event::TelegraphStarted { .. })));
        let (start, ev) = w.started()[0].clone();
        let Event::TelegraphStarted {
            source,
            spell,
            pos,
            resolve_ms,
            shape,
            id,
            ..
        } = ev
        else {
            unreachable!()
        };
        assert_eq!((source, spell, resolve_ms), (mm, MEETING_ROOM, 2200));
        assert_eq!(shape, TelegraphShape::Circle { radius_m: 4.0 });
        assert!(
            pos.x.abs() < 1e-3 && pos.z.abs() < 1e-3,
            "painted on the player"
        );
        assert_eq!(w.sim.telegraphs().len(), 1);
        if dodge {
            w.move_to(w.p, 6.0, 0.0);
        }
        w.run(70);
        let landed = w
            .log
            .iter()
            .find(
                |(_, e)| matches!(e, Event::TelegraphResolved { id: i, landed: true } if *i == id),
            )
            .map(|(t, _)| *t);
        assert_eq!(landed, Some(start + 66), "2200 ms = 66 ticks");
        let hits = w.damage(w.p, MEETING_ROOM);
        if dodge {
            assert!(hits.is_empty(), "stepped out: {hits:?}");
        } else {
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].0, start + 66);
        }
        assert!(w.sim.telegraphs().is_empty());
    }
}

#[test]
fn cone_telegraph_aims_at_the_target_and_misses_behind_the_caster() {
    for (x, z, hit) in [(0.0, 0.0, true), (2.0, -10.0, false)] {
        let mut w = world(|c| only(c, "crypto_bro", CONE, 500, AiTarget::Current));
        w.spawn("crypto_bro", 0.0, -5.0);
        assert!(w.run_until(60, |e| matches!(e, Event::TelegraphStarted { .. })));
        // Behind the bro (further along -Z) is outside the 80 degree cone aimed at +Z.
        w.move_to(w.p, x, z);
        w.run(60);
        assert_eq!(!w.damage(w.p, CONE).is_empty(), hit, "player at ({x}, {z})");
    }
}

#[test]
fn split_telegraph_shares_damage_and_punishes_nobody_showing_up() {
    let full = {
        let mut w = world(|c| only(c, "middle_manager", SYNERGY, 300, AiTarget::Current));
        w.spawn("middle_manager", 0.0, -5.0);
        w.run(150);
        let d = w.damage(w.p, SYNERGY);
        assert_eq!(d.len(), 1, "solo: the whole circle on the one player");
        d[0].1
    };
    // Two players stacked: each takes half.
    let mut w = world(|c| only(c, "middle_manager", SYNERGY, 300, AiTarget::Current));
    let p2 = w
        .sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, 0.0), 0.0);
    w.spawn("middle_manager", 0.0, -5.0);
    w.run(150);
    let (a, b) = (w.damage(w.p, SYNERGY), w.damage(p2, SYNERGY));
    assert_eq!((a.len(), b.len()), (1, 1));
    // Half each, then the co-op damage bonus (+30% for the second member).
    let half = full * 130 / 100 / 2;
    assert!(a[0].1.abs_diff(half) <= 1, "{} vs {half}", a[0].1);
    assert_eq!(a[0].1, b[0].1);
    // Both leave the circle: everyone nearby takes it in full.
    let mut w = world(|c| only(c, "middle_manager", SYNERGY, 300, AiTarget::Current));
    let p2 = w
        .sim
        .spawn_player("fullstack", Vec3::new(1.0, 0.0, 0.0), 0.0);
    w.spawn("middle_manager", 0.0, -5.0);
    assert!(w.run_until(60, |e| matches!(e, Event::TelegraphStarted { .. })));
    w.move_to(w.p, 9.0, 0.0);
    w.move_to(p2, -9.0, 0.0);
    w.run(120);
    let whole = full * 130 / 100;
    assert!(w.damage(w.p, SYNERGY)[0].1.abs_diff(whole) <= 1);
    assert!(w.damage(p2, SYNERGY)[0].1.abs_diff(whole) <= 1);
}

#[test]
fn earnings_call_spares_players_behind_cover() {
    for hide in [false, true] {
        let mut w = world(|c| only(c, "ceo", EARNINGS, 300, AiTarget::Caster));
        w.spawn("ceo", 0.0, -8.0);
        assert!(w.run_until(60, |e| matches!(e, Event::TelegraphStarted { .. })));
        let cover = w.sim.telegraphs()[0].cover.clone();
        assert_eq!(cover.len(), 3);
        // Spawn yaw 0 faces -Z: (right -6, forward 6) is at (-6, -14).
        assert!((cover[0].x + 6.0).abs() < 1e-3 && (cover[0].z + 14.0).abs() < 1e-3);
        let (x, z) = if hide {
            (cover[2].x, cover[2].z)
        } else {
            (8.0, 0.0)
        };
        w.move_to(w.p, x, z);
        w.run(170);
        assert_eq!(w.damage(w.p, EARNINGS).is_empty(), hide);
    }
}

#[test]
fn interrupting_a_must_interrupt_cast_prevents_the_hit() {
    for kick in [false, true] {
        let mut w = world(|c| only(c, "sales_rep", REPLY_ALL, 300, AiTarget::Current));
        let rep = w.spawn("sales_rep", 0.0, -5.0);
        assert!(w.sim.content().spell(REPLY_ALL).is_some_and(|s| s.kick));
        assert!(w.run_until(
            60,
            |e| matches!(e, Event::CastStarted { spell, .. } if *spell == REPLY_ALL)
        ));
        if kick {
            w.cast_at(w.p, rep, RPR);
        }
        w.run(120);
        let hits = w.damage(w.p, REPLY_ALL);
        assert_eq!(hits.is_empty(), kick, "{hits:?}");
        let interrupted = w.log.iter().any(|(_, e)| {
            matches!(e, Event::CastInterrupted { spell, by: Some(_), .. } if *spell == REPLY_ALL)
        });
        assert_eq!(interrupted, kick);
    }
}

#[test]
fn frontal_shield_reduces_hits_from_the_front_only() {
    let mut w = world(|c| {
        let u = def_mut(c, "defensive_stakeholder");
        if let Some(ai) = u.ai.as_mut() {
            ai.abilities.clear();
            ai.melee_damage = 0;
        }
        u.max_hp = 100_000;
    });
    let back = w
        .sim
        .spawn_player("fullstack", Vec3::new(0.0, 0.0, -10.0), 0.0);
    assert!(w.sim.set_level_of(back, 10));
    let ds = w.spawn("defensive_stakeholder", 0.0, -5.0);
    // The front player pulls with a big hit so the stakeholder faces (and keeps facing) it.
    w.cast_at(w.p, ds, CR);
    w.run(70);
    // Two cut hits from the front leave the stakeholder facing the front player for the rear hit.
    assert!(
        w.sim.unit(ds).is_some_and(|u| u.yaw.abs() > 3.0),
        "faces +Z"
    );
    w.cast_at(w.p, ds, CR);
    w.run(70);
    // The rear player faces +Z, toward the stakeholder's back.
    w.sim.apply_as(
        back,
        Command::PlayerMoved {
            pos: Vec3::new(0.0, 0.0, -10.0),
            yaw: std::f32::consts::PI,
            moving: false,
            jumping: false,
        },
    );
    w.cast_at(back, ds, CR);
    w.run(70);
    let hits: Vec<(u64, u64, u32)> = w
        .log
        .iter()
        .filter_map(|(t, e)| match e {
            Event::Damage {
                source,
                target,
                spell: Some(s),
                amount,
                ..
            } if *target == ds && *s == CR => Some((*t, *source, *amount)),
            _ => None,
        })
        .collect();
    let front: Vec<u32> = hits.iter().filter(|h| h.1 == w.p).map(|h| h.2).collect();
    let rear: Vec<u32> = hits.iter().filter(|h| h.1 == back).map(|h| h.2).collect();
    assert_eq!((front.len(), rear.len()), (2, 1), "{hits:?}");
    // Same spell, same stats: the front hit is cut by 60%.
    assert_eq!(front[0], front[1]);
    assert_eq!(front[0], rear[0] * 40 / 100, "{front:?} vs {rear:?}");
}

#[test]
fn dispel_skips_never_auras_and_removes_magic_ones() {
    let mut w = world(|c| {
        let mut never = match c.aura("root_revoked").cloned() {
            Some(a) => a,
            None => panic!("no root_revoked"),
        };
        never.key = "test_never".into();
        never.dispel = DispelType::Never;
        c.auras.push(never);
    });
    assert_eq!(
        w.sim.content().aura("root_revoked").map(|a| a.dispel),
        Some(DispelType::Magic)
    );
    assert!(w.sim.debug_apply_aura(w.p, "root_revoked", 20_000));
    // Applied last, so a newest-first dispel would take it if it could.
    assert!(w.sim.debug_apply_aura(w.p, "test_never", 20_000));
    w.tick();
    w.sim.push(Command::Cast {
        caster: w.p,
        spell: ROLLBACK,
    });
    w.run(3);
    let keys: Vec<String> = w
        .sim
        .unit(w.p)
        .map(|u| u.auras.iter().map(|a| a.key.clone()).collect())
        .unwrap_or_default();
    assert_eq!(keys, ["test_never"]);
}

#[test]
fn summoner_spawns_adds_up_to_its_cap_until_killed() {
    let mut w = world(|c| {
        if let Some(ai) = def_mut(c, "agent_orchestrator").ai.as_mut() {
            ai.melee_damage = 0;
        }
        if let Some(ai) = def_mut(c, "copilot_drone").ai.as_mut() {
            ai.melee_damage = 0;
        }
    });
    let orch = w.spawn("agent_orchestrator", 0.0, -5.0);
    w.run(40 * 30);
    let summoned = |w: &W| {
        w.log
            .iter()
            .filter(|(_, e)| matches!(e, Event::Summoned { summoner, .. } if *summoner == orch))
            .count()
    };
    let alive = |w: &W| {
        w.sim
            .units()
            .filter(|u| u.key == "copilot_drone" && u.alive)
            .count()
    };
    assert_eq!(summoned(&w), 3, "max_alive 3");
    assert_eq!(alive(&w), 3);
    assert!(w.sim.debug_kill(orch));
    w.run(20 * 30);
    assert_eq!(summoned(&w), 3, "no more after the summoner died");
    assert_eq!(alive(&w), 0, "the sub-agents die with the orchestrator");
}

#[test]
fn enrage_turns_on_after_its_timer_and_resets_with_the_fight() {
    let mut w = world(|c| {
        let u = def_mut(c, "chief_happiness_officer");
        u.max_hp = 100_000;
        if let Some(ai) = u.ai.as_mut() {
            ai.abilities.clear();
            ai.melee_damage = 1;
        }
    });
    let cho = w.spawn("chief_happiness_officer", 0.0, -3.0);
    let enraged = |w: &W| {
        w.sim
            .unit(cho)
            .is_some_and(|u| u.auras.iter().any(|a| a.key == "happiness_quota"))
    };
    w.cast_at(w.p, cho, CR);
    w.run(34 * 30);
    assert!(!enraged(&w), "not before 35 s");
    w.run(2 * 30);
    assert!(enraged(&w), "on after 35 s in combat");
    assert!(
        w.sim
            .content()
            .aura("happiness_quota")
            .is_some_and(|a| a.dispel == DispelType::Never)
    );
}

#[test]
fn ceo_flees_to_the_elevator_and_a_kick_stops_the_severance_heal() {
    for kick in [false, true] {
        let mut w = world(|c| {
            let u = def_mut(c, "ceo");
            u.max_hp = 100_000;
            if let Some(ai) = u.ai.as_mut() {
                ai.abilities.clear();
                ai.melee_damage = 0;
                if let Some(f) = ai.flee.as_mut() {
                    f.phase = 1;
                }
            }
        });
        let ceo = w.spawn("ceo", 0.0, -5.0);
        w.cast_at(w.p, ceo, CR);
        // Exit (right 8, forward 2) from a spawn at (0, -5) facing -Z: (8, -7).
        assert!(w.run_until(30 * 30, |e| {
            matches!(e, Event::CastStarted { spell, .. } if *spell == SEVERANCE)
        }));
        let at = w
            .sim
            .unit(ceo)
            .map(|u| u.pos)
            .unwrap_or(Vec3::new(0.0, 0.0, 0.0));
        assert!(
            (at.x - 8.0).abs() < 0.1 && (at.z + 7.0).abs() < 0.1,
            "{at:?}"
        );
        if kick {
            // Face him where he stopped.
            w.sim.push(Command::PlayerMoved {
                pos: Vec3::new(0.0, 0.0, 0.0),
                yaw: (-at.x).atan2(-at.z),
                moving: false,
                jumping: false,
            });
            w.cast_at(w.p, ceo, RPR);
        }
        w.run(4 * 30);
        let healed = w.log.iter().any(|(_, e)| {
            matches!(e, Event::Heal { source, spell: Some(s), .. } if *source == ceo && *s == SEVERANCE)
        });
        assert_eq!(healed, !kick);
    }
}

#[test]
fn consultants_shield_the_ceo_while_alive() {
    let mut w = world(|c| {
        let u = def_mut(c, "ceo");
        if let Some(ai) = u.ai.as_mut() {
            ai.abilities.clear();
            ai.melee_damage = 0;
            ai.flee = None;
            ai.phases.truncate(1);
            // Any hit enters phase 2.
            ai.phases[0].below_hp_pct = 99;
        }
        if let Some(ai) = def_mut(c, "strategy_consultant").ai.as_mut() {
            ai.melee_damage = 0;
        }
    });
    let ceo = w.spawn("ceo", 0.0, -5.0);
    w.cast_at(w.p, ceo, CR);
    w.run(90);
    let consultants: Vec<u64> = w
        .sim
        .units()
        .filter(|u| u.key == "strategy_consultant" && u.alive)
        .map(|u| u.id)
        .collect();
    assert_eq!(consultants.len(), 2);
    let shielded = |w: &W| {
        w.sim
            .unit(ceo)
            .is_some_and(|u| u.auras.iter().any(|a| a.key == "consulting_retainer"))
    };
    assert!(shielded(&w));
    for c in consultants {
        assert!(w.sim.debug_kill(c));
    }
    w.run(8 * 30);
    assert!(!shielded(&w), "the retainer lapses once nobody bills it");
}

#[test]
fn content_rejects_degenerate_telegraphs() {
    let broken = SRC.replace(
        "shape: Circle(radius_m: 3.5), delay_ms: 2000, label: \"Package drop zone: sign here\"",
        "shape: Circle(radius_m: 0.0), delay_ms: 2000, label: \"Package drop zone: sign here\"",
    );
    assert_ne!(broken, SRC);
    assert!(matches!(
        Content::from_ron(&broken),
        Err(ContentError::Invalid(_))
    ));
}
