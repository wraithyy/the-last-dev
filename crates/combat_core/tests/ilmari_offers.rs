//! Offers (docs/ILMARI_1_0.md D.3 "The offer", D.9 item 2, task D3): the pending aura that reads
//! like a buff, silence as consent, the Opt-Out Kiosk, `Aligned` and `Pushback`, and the dispel.
//! Wing 1's boss stands in as the Consultant (its only ability is the offer, every 20 s).
#![allow(clippy::expect_used)]

mod common;

use combat_core::alignment::OfferDef;
use combat_core::world::{PROP_OK, PropKind};
use combat_core::{Command, Content, Effect, EntityId, Event, Sim, SpellId, Vec3};
use common::*;

const THURSDAY: u64 = 1_792_022_400;
const SECOND: u32 = 30;
const BOSS: &str = "il_patch_tuesday";
const OFFER_SPELL: u32 = 9902;
const OK: &str = "t_safety_net";
const BAD: &str = "t_guardrailed";
const KIOSK: &str = "il_optout_t";

fn anchors(name: &str) -> Option<(Vec3, f32)> {
    let at = |x: f32, z: f32| Some((Vec3::new(x, 0.0, z), 0.0));
    match name {
        n if n.starts_with("pack_") => at(400.0, 400.0),
        "boss" => at(0.0, -20.0),
        n if n.starts_with("prop:") => at(1.0, -8.0),
        _ => at(0.0, 0.0),
    }
}

fn offer() -> OfferDef {
    OfferDef {
        ok_aura: OK.into(),
        bad_aura: BAD.into(),
        accept_ms: 6000,
        bad_ms: 60_000,
        aligned_aura: Some("il_aligned_stack".into()),
        pushback_aura: Some("il_pushback".into()),
        stagger_aura: Some("il_stagger".into()),
        stagger_ms: 3000,
        heal_tenths: 0,
        rider_kiosk: None,
    }
}

fn offer_content(def: OfferDef) -> Content {
    let mut c = content();
    let aura = |c: &Content, from: &str, key: &str, harmful: bool| {
        let mut a = c
            .auras
            .iter()
            .find(|a| a.key == from)
            .expect("aura")
            .clone();
        a.key = key.into();
        a.harmful = harmful;
        a
    };
    let ok = aura(&c, "il_aligned_stack", OK, false);
    let bad = aura(&c, "il_pushback", BAD, true);
    c.auras.extend([ok, bad]);
    // The boss does nothing but offer.
    let mut spell = c.spell(SpellId(950)).expect("scooter spell").clone();
    spell.id = SpellId(OFFER_SPELL);
    spell.cast_ms = 0;
    spell.gcd = false;
    spell.range_m = 60.0;
    spell.yells.clear();
    spell.effect = Effect::Offer(def);
    c.spells.push(spell);
    let ai = def_mut(&mut c, BOSS).ai.as_mut().expect("boss ai");
    let mut ab = ai.abilities[0].clone();
    ab.spell = SpellId(OFFER_SPELL);
    (ab.cooldown_min_ms, ab.cooldown_max_ms) = (20_000, 20_000);
    ab.initial_ms = Some((500, 500));
    ab.phases.clear();
    ab.requires_aura = None;
    ab.yells.clear();
    ab.mode = combat_core::content::AbilityMode::Both;
    ai.abilities = vec![ab];
    ai.phases.clear();
    ai.enrage = None;
    // The kiosk, next to the arrival point.
    let mut prop = c
        .world
        .props
        .iter()
        .find(|p| p.key == "il_label_printer_1")
        .expect("prop")
        .clone();
    prop.key = KIOSK.into();
    prop.zone = "ilmari1/1".into();
    prop.kind = PropKind::OptOut;
    prop.cooldown_ms = 20_000;
    prop.reach_m = 5.0;
    c.world.props.push(prop);
    for k in [
        "senior_engineer",
        "backend_dev",
        "devops_eng",
        "frontend_dev",
    ] {
        def_mut(&mut c, k).max_hp = 10_000_000;
    }
    for s in &mut c.spells {
        s.yells.clear();
    }
    c
}

/// Three players (Fullstack pulling, DevOps, Backend) pulling the offering boss; `story` forces the difficulty.
fn fight(story: bool, def: OfferDef) -> (Sim, Vec<EntityId>, EntityId) {
    let mut sim = Sim::new(offer_content(def), 33);
    sim.set_wandering(false);
    sim.set_clock(THURSDAY);
    let ids: Vec<EntityId> = ["fullstack", "devops", "backend"]
        .iter()
        .enumerate()
        .map(|(k, class)| {
            let p = sim.spawn_player(class, Vec3::new(k as f32, 0.0, 0.0), 0.0);
            sim.set_level_of(p, 10);
            p
        })
        .collect();
    sim.debug_force_raid_async(Some(story));
    if let Err(e) = sim.load_zone("ilmari1/1", &anchors, Vec::new()) {
        panic!("{e}");
    }
    let boss = sim
        .units()
        .find(|u| u.key == BOSS)
        .map(|u| u.id)
        .expect("boss");
    for &p in &ids {
        sim.apply_as(
            p,
            Command::PlayerMoved {
                pos: Vec3::new(p as f32, 0.0, -8.0),
                yaw: 0.0,
                moving: false,
                jumping: false,
            },
        );
    }
    sim.apply_as(
        ids[0],
        Command::SetTarget {
            unit: ids[0],
            target: Some(boss),
        },
    );
    sim.apply_as(
        ids[0],
        Command::Cast {
            caster: ids[0],
            spell: SpellId(1),
        },
    );
    // Code Review lands after 2 s and the first offer 0.5 s after the pull.
    run(&mut sim, 2600);
    (sim, ids, boss)
}

fn run(sim: &mut Sim, ms: u32) -> Vec<Event> {
    (0..ms * SECOND / 1000).flat_map(|_| sim.tick()).collect()
}

fn stacks(sim: &Sim, id: EntityId, key: &str) -> u8 {
    sim.unit(id)
        .and_then(|u| u.auras.iter().find(|a| a.key == key).map(|a| a.stacks))
        .unwrap_or(0)
}

fn applied(events: &[Event], key: &str, on: EntityId) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::AuraApplied { target, key: k } if *target == on && k == key))
        .count()
}

fn use_kiosk(sim: &mut Sim, p: EntityId) -> Vec<u32> {
    sim.apply_as(p, Command::Interact { prop: KIOSK.into() });
    sim.tick()
        .into_iter()
        .filter_map(|e| match e {
            Event::Interacted {
                unit,
                prop,
                outcome,
            } if unit == p && prop == KIOSK => Some(outcome),
            _ => None,
        })
        .collect()
}

#[test]
fn an_offer_looks_like_a_buff_and_silence_is_consent() {
    let (mut sim, ids, boss) = fight(false, offer());
    run(&mut sim, 1500);
    for &p in &ids {
        assert_eq!(stacks(&sim, p, OK), 1, "the pending offer is on {p}");
        assert_eq!(stacks(&sim, p, BAD), 0, "nothing is real yet");
    }
    assert_eq!(stacks(&sim, boss, "il_aligned_stack"), 0);
    // The window is 6 s; nobody reads the terms.
    run(&mut sim, 6500);
    for &p in &ids {
        assert_eq!(stacks(&sim, p, OK), 0, "the offer is gone");
        assert_eq!(stacks(&sim, p, BAD), 1, "the guardrail landed on {p}");
    }
    assert_eq!(
        stacks(&sim, boss, "il_aligned_stack"),
        3,
        "accepting powers the boss"
    );
}

#[test]
fn refusing_at_the_kiosk_cancels_your_offer_staggers_once_and_stacks_pushback() {
    let (mut sim, ids, boss) = fight(false, offer());
    run(&mut sim, 1500);
    assert_eq!(use_kiosk(&mut sim, ids[0]), [PROP_OK]);
    assert_eq!(
        stacks(&sim, ids[0], OK),
        0,
        "the pending offer was cancelled"
    );
    assert_eq!(stacks(&sim, boss, "il_pushback"), 1);
    assert_eq!(
        stacks(&sim, boss, "il_stagger"),
        1,
        "the first refusal staggers him"
    );
    // A second refusal stacks Pushback and does not stagger again.
    let second = (0..3)
        .flat_map(|_| {
            sim.apply_as(ids[2], Command::Interact { prop: KIOSK.into() });
            sim.tick()
        })
        .collect::<Vec<_>>();
    assert_eq!(stacks(&sim, boss, "il_pushback"), 2);
    assert_eq!(applied(&second, "il_stagger", boss), 0);
    // Nothing to cancel: the kiosk reports it and the cooldown is not burnt.
    assert_eq!(use_kiosk(&mut sim, ids[0]), [0]);

    let after = run(&mut sim, 6500);
    assert_eq!(stacks(&sim, ids[0], BAD), 0, "refusing works");
    assert_eq!(stacks(&sim, ids[2], BAD), 0);
    assert_eq!(
        stacks(&sim, ids[1], BAD),
        1,
        "the one who did not read it wears it"
    );
    assert_eq!(
        stacks(&sim, boss, "il_aligned_stack"),
        1,
        "only the accepted one feeds him"
    );
    assert_eq!(applied(&after, BAD, ids[0]), 0);
}

#[test]
fn a_dispel_cancels_an_accepted_guardrail_and_its_aligned_stack() {
    let (mut sim, ids, boss) = fight(false, offer());
    run(&mut sim, 7500);
    assert_eq!(stacks(&sim, boss, "il_aligned_stack"), 3);
    // The DevOps player's Feature Flag Off (spell 46) on the Backend player.
    sim.apply_as(
        ids[1],
        Command::SetTarget {
            unit: ids[1],
            target: Some(ids[2]),
        },
    );
    sim.apply_as(
        ids[1],
        Command::Cast {
            caster: ids[1],
            spell: SpellId(46),
        },
    );
    sim.tick();
    assert_eq!(stacks(&sim, ids[2], BAD), 0, "dispelled");
    assert_eq!(stacks(&sim, ids[1], BAD), 1);
    assert_eq!(
        stacks(&sim, boss, "il_aligned_stack"),
        2,
        "the stack went with it"
    );
}

#[test]
fn story_offers_feed_the_boss_nothing() {
    let (mut sim, ids, boss) = fight(true, offer());
    run(&mut sim, 7500);
    for &p in &ids {
        assert_eq!(stacks(&sim, p, BAD), 1);
    }
    assert_eq!(stacks(&sim, boss, "il_aligned_stack"), 0, "Story is honest");
}

#[test]
fn a_new_wave_replaces_an_unanswered_offer_and_the_dead_are_not_offered() {
    let mut def = offer();
    def.accept_ms = 30_000;
    let (mut sim, ids, boss) = fight(false, def);
    run(&mut sim, 1500);
    assert!(sim.debug_kill_player(ids[2]));
    run(&mut sim, 30_000);
    // The second wave (20 s after the first) replaced the first window rather than doubling it.
    assert_eq!(stacks(&sim, ids[0], OK), 1, "one pending offer, not two");
    assert_eq!(
        stacks(&sim, ids[0], BAD),
        0,
        "the replaced window never converted"
    );
    assert_eq!(stacks(&sim, ids[2], OK), 0);
    assert_eq!(stacks(&sim, ids[2], BAD), 0, "a dead player wears nothing");
    let _ = boss;
}

#[test]
fn the_same_offers_twice_give_the_same_log() {
    let log = || {
        let (mut sim, ids, _) = fight(false, offer());
        let mut out = Vec::new();
        for t in 0..14 * SECOND {
            if t == 3 * SECOND {
                sim.apply_as(ids[1], Command::Interact { prop: KIOSK.into() });
            }
            for e in sim.tick() {
                out.push(log_line(&sim, u64::from(t), &e));
            }
        }
        out
    };
    let a = log();
    assert!(!a.is_empty());
    assert_eq!(a, log());
}

#[test]
fn offer_content_validates() {
    let ron = include_str!("../../../content/game.ron");
    assert!(Content::from_ron(ron).is_ok());
    // The shipped auras exist and read like the doc: Aligned +8% per stack, Pushback +6% to 5 stacks.
    let c = content();
    let al = c.aura("il_aligned_stack").expect("aligned");
    assert_eq!((al.damage_done_pct, al.max_stacks), (8, 10));
    let pb = c.aura("il_pushback").expect("pushback");
    assert_eq!(
        (pb.damage_taken_pct, pb.max_stacks, pb.harmful),
        (6, 5, true)
    );
    assert!(c.aura("il_stagger").is_some_and(|a| a.stun));
    // A spell offering a beneficial "guardrail" is refused (the real aura must be harmful).
    let spell = |bad: &str| {
        format!(
            "(\n            id: 9990, name: \"Offer\", icon: \"x\", school: Arcane,\n            range_m: 60.0, cast_ms: 0, mana: 0, gcd: false, cooldown_ms: 0, target: Caster,\n            description: \"d\",\n            effect: Offer((ok_aura: \"il_aligned_stack\", bad_aura: \"{bad}\", accept_ms: 6000)),\n        ),\n        (\n            id: 950,"
        )
    };
    let with = |bad: &str| ron.replacen("(\n            id: 950,", &spell(bad), 1);
    assert!(Content::from_ron(&with("il_pushback")).is_ok());
    assert!(
        Content::from_ron(&with("il_aligned_stack")).is_err(),
        "benefit as guardrail"
    );
    assert!(Content::from_ron(&with("no_such_aura")).is_err());
}
