//! Pets (summon, save, Roomba sweeps) and Code Review Duels (request, countdown, 1 HP end,
//! forfeits, who may hurt or heal whom).

mod common;

use combat_core::{
    CastError, Command, DuelError, DuelOutcome, EntityId, Event, SaveData, Sim, SpellId, Vec3,
};
use common::{CR, content, def_mut, legacy_ceo};
use std::f32::consts::PI;

const GUARDRAILS: SpellId = SpellId(3);

struct T {
    sim: Sim,
    a: EntityId,
    b: EntityId,
    c: EntityId,
    log: Vec<Event>,
}

/// A at the origin facing -Z, B 10 m ahead facing A, C off to the side; a legacy CEO far away.
fn three() -> T {
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
    legacy_ceo(&mut c);
    let mut sim = Sim::new(c, 7);
    sim.set_wandering(false);
    let a = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let b = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, -10.0), PI);
    let c = sim.spawn_player("fullstack", Vec3::new(4.0, 0.0, -5.0), 0.0);
    T {
        sim,
        a,
        b,
        c,
        log: Vec::new(),
    }
}

impl T {
    fn run(&mut self, n: u32) {
        for _ in 0..n {
            let ev = self.sim.tick();
            self.log.extend(ev);
        }
    }
    fn act(&mut self, p: EntityId, cmd: Command) {
        self.sim.apply_as(p, cmd);
        self.run(1);
    }
    fn hp(&self, p: EntityId) -> u32 {
        self.sim.unit(p).map_or(0, |u| u.hp)
    }
    fn ended(&self) -> Vec<(Option<EntityId>, DuelOutcome, u32)> {
        self.log
            .iter()
            .filter_map(|e| match e {
                Event::DuelEnded {
                    winner,
                    outcome,
                    streak,
                    ..
                } => Some((*winner, *outcome, *streak)),
                _ => None,
            })
            .collect()
    }
    fn refused(&self) -> Vec<DuelError> {
        self.log
            .iter()
            .filter_map(|e| match e {
                Event::DuelRefused { reason } => Some(*reason),
                _ => None,
            })
            .collect()
    }
    /// Request, accept and sit through the countdown.
    fn duel(&mut self) {
        self.act(self.a, Command::DuelRequest { target: self.b });
        self.act(self.b, Command::DuelRespond { accept: true });
        self.run(95);
        assert!(
            self.log
                .iter()
                .any(|e| matches!(e, Event::DuelBegan { .. })),
            "duel never began"
        );
    }
    fn cast(&mut self, who: EntityId, target: EntityId, spell: SpellId) {
        self.sim.apply_as(
            who,
            Command::SetTarget {
                unit: who,
                target: Some(target),
            },
        );
        self.sim.apply_as(who, Command::Cast { caster: who, spell });
        self.run(80);
    }
    fn failures(&self, who: EntityId) -> Vec<CastError> {
        self.log
            .iter()
            .filter_map(|e| match e {
                Event::CastFailed { caster, reason, .. } if *caster == who => Some(*reason),
                _ => None,
            })
            .collect()
    }
    fn damage(&self, from: EntityId, to: EntityId) -> Vec<u32> {
        self.log
            .iter()
            .filter_map(|e| match e {
                Event::Damage {
                    source,
                    target,
                    amount,
                    ..
                } if *source == from && *target == to => Some(*amount),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn request_accept_countdown_then_fight() {
    let mut t = three();
    t.act(t.a, Command::DuelRequest { target: t.b });
    assert!(
        t.log
            .iter()
            .any(|e| matches!(e, Event::DuelRequested { from, to } if *from == t.a && *to == t.b))
    );
    // Asked is not accepted: still friends.
    t.cast(t.a, t.b, CR);
    assert!(t.failures(t.a).contains(&CastError::InvalidTarget));
    t.act(t.b, Command::DuelRespond { accept: true });
    let countdown = t
        .log
        .iter()
        .find_map(|e| match e {
            Event::DuelCountdown { center, ms, .. } => Some((*center, *ms)),
            _ => None,
        })
        .expect("countdown");
    assert_eq!(countdown.1, combat_core::duel::DUEL_COUNTDOWN_MS);
    assert!((countdown.0.z + 5.0).abs() < 0.01, "arena between the two");
    t.run(95);
    assert!(t.log.iter().any(|e| matches!(e, Event::DuelBegan { .. })));
    assert_eq!(t.sim.unit(t.a).and_then(|u| u.duel_foe), Some(t.b));
    assert_eq!(t.sim.unit(t.b).and_then(|u| u.duel_foe), Some(t.a));
}

#[test]
fn decline_and_timeout_end_the_request() {
    let mut t = three();
    t.act(t.a, Command::DuelRequest { target: t.b });
    // Only the asked player can answer.
    t.act(t.c, Command::DuelRespond { accept: true });
    assert!(t.ended().is_empty());
    t.act(t.b, Command::DuelRespond { accept: false });
    assert_eq!(t.ended(), vec![(None, DuelOutcome::Declined, 0)]);

    let mut t = three();
    t.act(t.a, Command::DuelRequest { target: t.b });
    t.run(30 * 30 + 5);
    assert_eq!(t.ended(), vec![(None, DuelOutcome::TimedOut, 0)]);
    // Too late to accept now.
    t.act(t.b, Command::DuelRespond { accept: true });
    assert!(
        !t.log
            .iter()
            .any(|e| matches!(e, Event::DuelCountdown { .. }))
    );
}

#[test]
fn requests_are_validated_and_rate_limited() {
    let mut t = three();
    t.act(t.a, Command::DuelRequest { target: t.a });
    t.act(t.a, Command::DuelRequest { target: 9_999 });
    assert_eq!(
        t.refused(),
        vec![DuelError::NoSuchPlayer, DuelError::NoSuchPlayer]
    );
    t.log.clear();
    t.act(t.a, Command::DuelRequest { target: t.b });
    t.act(t.b, Command::DuelRespond { accept: false });
    t.act(t.a, Command::DuelRequest { target: t.b });
    assert_eq!(t.refused(), vec![DuelError::TooSoon]);
    t.run(5 * 30);
    t.log.clear();
    // C is busy being asked by B.
    t.act(t.b, Command::DuelRequest { target: t.c });
    t.act(t.a, Command::DuelRequest { target: t.c });
    assert_eq!(t.refused(), vec![DuelError::Busy]);
    t.act(t.c, Command::DuelRespond { accept: false });
    t.log.clear();
    t.sim.apply_as(
        t.a,
        Command::PlayerMoved {
            pos: Vec3::new(0.0, 0.0, 40.0),
            yaw: 0.0,
            moving: false,
            jumping: false,
        },
    );
    t.run(5 * 30);
    t.act(t.a, Command::DuelRequest { target: t.c });
    assert!(t.refused().contains(&DuelError::TooFar));
}

#[test]
fn nobody_duels_while_pulled_by_an_enemy() {
    let mut t = three();
    let ceo = t
        .sim
        .spawn_unit("ceo", Vec3::new(0.0, 0.0, 8.0), PI)
        .expect("ceo");
    t.sim.apply_as(
        t.a,
        Command::PlayerMoved {
            pos: Vec3::new(0.0, 0.0, 0.0),
            yaw: PI,
            moving: false,
            jumping: false,
        },
    );
    t.cast(t.a, ceo, CR);
    t.act(t.a, Command::DuelRequest { target: t.b });
    assert_eq!(t.refused(), vec![DuelError::InCombat]);
}

#[test]
fn only_the_pair_trades_blows_at_half_damage_and_it_ends_at_one_hp() {
    // Baseline: the same first Code Review on a CEO.
    let mut base = three();
    let ceo = base
        .sim
        .spawn_unit("ceo", Vec3::new(0.0, 0.0, -10.0), 0.0)
        .expect("ceo");
    base.cast(base.a, ceo, CR);
    let pve = base.damage(base.a, ceo)[0];

    let mut t = three();
    t.duel();
    t.cast(t.a, t.b, CR);
    let pvp = t.damage(t.a, t.b);
    assert_eq!(pvp, vec![pve * combat_core::duel::PVP_DAMAGE_PCT / 100]);
    // Outsiders can neither hurt nor help the duelists.
    t.cast(t.c, t.b, CR);
    assert!(t.failures(t.c).contains(&CastError::InvalidTarget));
    assert!(t.damage(t.c, t.b).is_empty());
    t.log.clear();
    t.cast(t.c, t.a, GUARDRAILS);
    assert!(t.failures(t.c).contains(&CastError::InvalidTarget));

    // The finishing blow leaves 1 HP and a winner.
    assert!(t.sim.debug_set_hp(t.b, 5));
    t.log.clear();
    t.cast(t.a, t.b, CR);
    assert_eq!(t.hp(t.b), 1);
    assert!(t.sim.unit(t.b).is_some_and(|u| u.alive));
    assert_eq!(t.ended(), vec![(Some(t.a), DuelOutcome::Won, 1)]);
    assert!(
        !t.log
            .iter()
            .any(|e| matches!(e, Event::Death { unit, .. } if *unit == t.b))
    );
    assert_eq!(t.sim.unit(t.a).map(|u| u.duel_streak), Some(1));
    assert_eq!(t.sim.unit(t.b).and_then(|u| u.duel_foe), None);
    // Friends again.
    t.log.clear();
    t.cast(t.a, t.b, CR);
    assert!(t.failures(t.a).contains(&CastError::InvalidTarget));
}

#[test]
fn leaving_the_circle_or_the_room_forfeits() {
    let mut t = three();
    t.duel();
    t.sim.apply_as(
        t.b,
        Command::PlayerMoved {
            pos: Vec3::new(0.0, 0.0, -40.0),
            yaw: 0.0,
            moving: true,
            jumping: false,
        },
    );
    t.run(2);
    assert_eq!(t.ended(), vec![(Some(t.a), DuelOutcome::Forfeit, 1)]);

    let mut t = three();
    t.duel();
    assert!(t.sim.remove_player(t.a));
    assert_eq!(
        t.ended(),
        Vec::new(),
        "ended inside remove_player, not a tick"
    );
    t.log.extend(t.sim.tick());
    assert_eq!(t.ended(), vec![(Some(t.b), DuelOutcome::Forfeit, 1)]);
}

#[test]
fn summoned_pet_is_owned_saved_and_visible() {
    let mut t = three();
    // Not owned yet: nothing happens.
    t.act(
        t.a,
        Command::SummonPet {
            pet: "rubber_duck".into(),
        },
    );
    assert_eq!(t.sim.unit(t.a).map(|u| u.pet.as_str()), Some(""));
    assert!(!t.sim.grant_pet(t.a, "no_such_pet"));
    // Learned from its carrier, bought like any consumable.
    assert!(t.sim.grant_goods(t.a, "Boxed Rubber Duck"));
    t.act(
        t.a,
        Command::UseItem {
            item: "Boxed Rubber Duck".into(),
        },
    );
    let u = t.sim.unit(t.a).expect("a");
    assert_eq!(u.pets, vec!["rubber_duck".to_string()]);
    assert_eq!(u.pet, "rubber_duck");
    assert!(!u.bag.iter().any(|n| n == "Boxed Rubber Duck"));
    assert!(t.log.iter().any(
        |e| matches!(e, Event::PetChanged { unit, pet } if *unit == t.a && pet == "rubber_duck")
    ));

    let save = t.sim.export_save(t.a).expect("save");
    let back = SaveData::decode(&save.encode()).expect("decode");
    assert_eq!(back.pets, vec!["rubber_duck".to_string()]);
    assert_eq!(back.pet, "rubber_duck");
    let mut fresh = Sim::new(content(), 1);
    let p = fresh.spawn_player("fullstack", Vec3::default(), 0.0);
    assert!(fresh.import_save(p, &back));
    assert_eq!(fresh.unit(p).map(|u| u.pet.as_str()), Some("rubber_duck"));
    // A save naming a pet the content lost loads without it.
    let mut odd = back.clone();
    odd.pets.push("dodo".into());
    odd.pet = "dodo".into();
    assert!(fresh.import_save(p, &odd));
    assert_eq!(fresh.unit(p).map(|u| u.pet.as_str()), Some(""));

    t.act(t.a, Command::SummonPet { pet: String::new() });
    assert_eq!(t.sim.unit(t.a).map(|u| u.pet.as_str()), Some(""));
}

#[test]
fn roomba_sweeps_only_its_owners_junk_in_range() {
    let mut t = three();
    let junk = t.sim.content().economy.junk[0].name.clone();
    assert!(t.sim.grant_pet(t.a, "roomba"));
    t.act(
        t.a,
        Command::SummonPet {
            pet: "roomba".into(),
        },
    );
    let near = t
        .sim
        .spawn_unit("ceo", Vec3::new(0.0, 0.0, 6.0), 0.0)
        .expect("ceo");
    let far = t
        .sim
        .spawn_unit("ceo", Vec3::new(0.0, 0.0, 40.0), 0.0)
        .expect("ceo");
    for id in [near, far] {
        assert!(t.sim.debug_kill(id));
    }
    t.run(1);
    // Plant a junk item in every share so the test does not depend on loot rolls.
    for id in [near, far] {
        assert!(t.sim.debug_add_loot(id, &junk));
    }
    let bag_before = |t: &T, p| t.sim.unit(p).map_or(0, |u| u.bag.len());
    let (a0, b0) = (bag_before(&t, t.a), bag_before(&t, t.b));
    t.log.clear();
    t.run(31);
    let swept: Vec<(EntityId, Vec<String>)> = t
        .log
        .iter()
        .filter_map(|e| match e {
            Event::PetLooted {
                unit,
                corpse,
                items,
                ..
            } => {
                assert_eq!(*unit, t.a);
                Some((*corpse, items.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(swept, vec![(near, vec![junk.clone()])]);
    assert_eq!(bag_before(&t, t.a), a0 + 1);
    // B's roll on the same corpse is untouched.
    assert_eq!(bag_before(&t, t.b), b0);
    assert!(t.sim.debug_loot_of(near, t.b).contains(&junk));
    assert!(!t.sim.debug_loot_of(near, t.a).contains(&junk));
    assert!(t.sim.debug_loot_of(far, t.a).contains(&junk));
}

#[test]
fn duck_tips_read_the_fight() {
    let mut t = three();
    let ceo = t
        .sim
        .spawn_unit("ceo", Vec3::new(0.0, 0.0, 8.0), PI)
        .expect("ceo");
    let me = t.sim.unit(t.a).cloned().expect("a");
    assert_eq!(combat_core::duck_tip(t.sim.content(), &me, None), None);
    let mut fighting = me.clone();
    fighting.in_combat = true;
    fighting.hp = fighting.max_hp / 10;
    let boss = t.sim.unit(ceo).cloned();
    let tip = combat_core::duck_tip(t.sim.content(), &fighting, boss.as_ref()).expect("tip");
    assert!(tip.contains("HP"), "{tip}");
    let mut casting = boss.clone().expect("ceo");
    casting.cast = Some(combat_core::types::CastView {
        spell: SpellId(5),
        elapsed_ms: 0,
        total_ms: 2000,
        interruptible: true,
    });
    let tip = combat_core::duck_tip(t.sim.content(), &fighting, Some(&casting)).expect("tip");
    assert!(tip.starts_with("Interrupt the CEO"), "{tip}");
}

#[test]
fn pets_cost_no_combat_power() {
    let mut t = three();
    assert!(t.sim.grant_pet(t.a, "golden_unicorn"));
    let before = t.sim.unit(t.a).map(|u| (u.max_hp, u.max_mana));
    t.act(
        t.a,
        Command::SummonPet {
            pet: "golden_unicorn".into(),
        },
    );
    assert_eq!(t.sim.unit(t.a).map(|u| (u.max_hp, u.max_mana)), before);
}
