//! Class props: only the matching class gets the effect, the others a refusal, Fullstack tries
//! anything clumsily, one-time unlocks persist, and no quest depends on any of them.

mod common;

use combat_core::{
    Command, Content, EntityId, Event, Objective, PROP_DONE, PROP_FULLSTACK, PROP_OK, PROP_REFUSED,
    PropDef, PropKind, SaveData, Sim, Vec3, ms_to_ticks,
};
use common::{Geo, content, geometry};
use std::collections::HashMap;

fn class_props(c: &Content) -> Vec<&PropDef> {
    c.world
        .props
        .iter()
        .filter(|p| p.kind == PropKind::Class)
        .collect()
}

struct W {
    sim: Sim,
    p: EntityId,
    geo: HashMap<String, Geo>,
    zone: String,
}

fn world(zone: &str, class: &str, seed: u64) -> W {
    let mut sim = Sim::new(content(), seed);
    sim.set_wandering(false);
    let p = sim.spawn_player(class, Vec3::new(0.0, 0.0, 0.0), 0.0);
    let geo = geometry();
    let Some(g) = geo.get(zone) else {
        panic!("no geometry for {zone}")
    };
    let anchors = |n: &str| g.anchors.get(n).copied();
    sim.set_zone_terrain(zone, g.terrain.clone());
    if let Err(e) = sim.load_zone(zone, &anchors, g.blockers.clone()) {
        panic!("load_zone {zone}: {e}");
    }
    let mut w = W {
        sim,
        p,
        geo,
        zone: zone.to_string(),
    };
    w.tick(1);
    w
}

impl W {
    fn tick(&mut self, n: u32) -> Vec<Event> {
        let mut out = Vec::new();
        for _ in 0..n {
            out.extend(self.sim.tick());
        }
        out
    }

    fn stand(&mut self, who: EntityId, prop: &str) {
        let Some((pos, yaw)) = self.geo[&self.zone]
            .anchors
            .get(&format!("prop:{prop}"))
            .copied()
        else {
            panic!("{}: prop {prop} is not placed", self.zone)
        };
        self.sim.apply_as(
            who,
            Command::PlayerMoved {
                pos,
                yaw,
                moving: false,
                jumping: false,
            },
        );
        self.tick(1);
    }

    /// Uses `prop` as `who` and returns the outcome it was answered with.
    fn use_as(&mut self, who: EntityId, prop: &str) -> Option<u32> {
        self.sim.apply_as(
            who,
            Command::Interact {
                prop: prop.to_string(),
            },
        );
        self.tick(1).into_iter().find_map(|e| match e {
            Event::Interacted {
                unit,
                prop: k,
                outcome,
            } if unit == who && k == prop => Some(outcome),
            _ => None,
        })
    }

    fn aura(&self, who: EntityId, key: &str) -> bool {
        self.sim
            .unit(who)
            .is_some_and(|u| u.auras.iter().any(|a| a.key == key))
    }

    /// Snapshot of what a prop could change on a player: Kudos, bag size, aura keys.
    fn state(&self, who: EntityId) -> (u32, usize, Vec<String>) {
        let u = self.sim.unit(who).unwrap_or_else(|| panic!("no unit"));
        (
            u.sp,
            u.bag.len(),
            u.auras.iter().map(|a| a.key.clone()).collect(),
        )
    }
}

#[test]
fn every_class_has_at_least_two_placed_props_and_fullstack_may_use_them_all() {
    let c = content();
    let geo = geometry();
    for class in c.classes.iter().filter(|k| k.key != "fullstack") {
        let own: Vec<_> = class_props(&c)
            .into_iter()
            .filter(|p| p.class == class.key)
            .collect();
        assert!(
            own.len() >= 2,
            "{} has {} class props",
            class.key,
            own.len()
        );
        for p in own {
            assert!(
                geo[&p.zone]
                    .anchors
                    .contains_key(&format!("prop:{}", p.key)),
                "{} is not placed in {}",
                p.key,
                p.zone
            );
        }
    }
    assert!(class_props(&c).iter().all(|p| p.class != "fullstack"));
}

#[test]
fn the_wrong_class_gets_a_refusal_and_nothing_changes() {
    let c = content();
    let classes: Vec<String> = c
        .classes
        .iter()
        .filter(|k| k.key != "fullstack")
        .map(|k| k.key.clone())
        .collect();
    for p in class_props(&c) {
        for class in classes.iter().filter(|k| **k != p.class) {
            let mut w = world(&p.zone, class, 3);
            w.stand(w.p, &p.key);
            let before = w.state(w.p);
            let at = w.sim.unit(w.p).map(|u| u.pos);
            let outcome = w.use_as(w.p, &p.key);
            assert!(
                outcome.is_some_and(|o| o & PROP_REFUSED != 0),
                "{} as {class}: {outcome:?}",
                p.key
            );
            assert_eq!(w.state(w.p), before, "{} as {class} changed state", p.key);
            assert_eq!(w.sim.unit(w.p).map(|u| u.pos), at);
            assert!(w.sim.taken_props(w.p).is_empty());
        }
    }
}

#[test]
fn the_right_class_gets_the_effect_and_party_effects_reach_everyone() {
    let c = content();
    for p in class_props(&c) {
        let mut w = world(&p.zone, &p.class, 3);
        let mate_class = if p.class == "qa" { "backend" } else { "qa" };
        let mate = w
            .sim
            .spawn_player(mate_class, Vec3::new(1.0, 0.0, 1.0), 0.0);
        w.stand(w.p, &p.key);
        let (mate_before, me_before) = (w.state(mate), w.state(w.p));
        let outcome = w.use_as(w.p, &p.key);
        assert_eq!(outcome, Some(PROP_OK), "{} as {}", p.key, p.class);
        let me = w.state(w.p);
        for (aura, _) in &p.auras {
            assert!(w.aura(w.p, aura), "{} gives {aura}", p.key);
            assert_eq!(w.aura(mate, aura), p.party, "{} party aura", p.key);
        }
        if !p.items.is_empty() {
            assert_eq!(me.1, me_before.1 + p.items.len(), "{} items", p.key);
        }
        if p.kudos > 0 {
            assert_eq!(me.0, me_before.0 + p.kudos, "{} kudos", p.key);
            let mate_gain = w.state(mate).0 - mate_before.0;
            assert_eq!(
                mate_gain,
                if p.party { p.kudos } else { 0 },
                "{} kudos",
                p.key
            );
        }
        if !p.to.is_empty() {
            let dest = w.sim.prop_spot(&p.to);
            assert_eq!(w.sim.unit(w.p).map(|u| u.pos), dest, "{} shortcut", p.key);
            assert_eq!(
                w.sim.unit(mate).map(|u| u.pos),
                dest,
                "{} party shortcut",
                p.key
            );
        }
        if p.once {
            assert_eq!(w.sim.taken_props(w.p), [p.key.as_str()]);
            w.tick(ms_to_ticks(p.cooldown_ms) as u32 + 1);
            w.stand(w.p, &p.key);
            let again = w.use_as(w.p, &p.key);
            assert_eq!(again, Some(PROP_DONE), "{} is one-time", p.key);
        }
    }
}

#[test]
fn a_one_time_unlock_survives_a_save() {
    let mut w = world("datacenter/2", "backend", 3);
    w.stand(w.p, "be_vending");
    assert_eq!(w.use_as(w.p, "be_vending"), Some(PROP_OK));
    let Some(save) = w.sim.export_save(w.p) else {
        panic!("no save")
    };
    let text = save.encode();
    assert!(text.contains("taken_props=be_vending"), "{text}");
    let mut fresh = world("datacenter/2", "backend", 3);
    let Some(decoded) = SaveData::decode(&text) else {
        panic!("save does not decode")
    };
    assert!(fresh.sim.import_save(fresh.p, &decoded));
    fresh.stand(fresh.p, "be_vending");
    assert_eq!(fresh.use_as(fresh.p, "be_vending"), Some(PROP_DONE));
}

/// Fullstack tries a backend prop until it works: outcomes only depend on the seed, a try
/// can break it, a success pays out and a buff lasts half as long.
fn fullstack_tries(seed: u64) -> Vec<u32> {
    let mut w = world("datacenter/2", "fullstack", seed);
    w.stand(w.p, "be_vending");
    let wait = ms_to_ticks(3 * 1000) as u32 + 2;
    let mut outcomes = Vec::new();
    for _ in 0..12 {
        let Some(o) = w.use_as(w.p, "be_vending") else {
            panic!("no answer")
        };
        outcomes.push(o);
        if o & 3 == PROP_OK {
            let u = w.sim.unit(w.p).unwrap_or_else(|| panic!("no unit"));
            let rush = u.auras.iter().find(|a| a.key == "sugar_rush");
            assert!(rush.is_some_and(|a| a.total_ms <= 30_000), "{rush:?}");
            break;
        }
        w.tick(wait);
    }
    outcomes
}

#[test]
fn fullstack_can_use_any_class_prop_slowly_and_may_break_it() {
    let (mut broke, mut worked) = (0, 0);
    for seed in 1..=20 {
        let a = fullstack_tries(seed);
        assert_eq!(a, fullstack_tries(seed), "seed {seed} is not deterministic");
        assert!(
            a.iter()
                .all(|o| o & PROP_FULLSTACK != 0 && o & PROP_REFUSED == 0)
        );
        broke += a.iter().filter(|o| *o & 3 != PROP_OK).count();
        worked += a.iter().filter(|o| *o & 3 == PROP_OK).count();
    }
    assert!(broke > 0 && worked == 20, "broke {broke}, worked {worked}");
    // Slower: the cooldown is three times the native one, so an immediate retry is ignored.
    let mut w = world("datacenter/2", "fullstack", 1);
    w.stand(w.p, "be_vending");
    let first = w.use_as(w.p, "be_vending");
    assert!(first.is_some());
    w.tick(ms_to_ticks(1500) as u32);
    assert_eq!(w.use_as(w.p, "be_vending"), None);
}

#[test]
fn no_quest_needs_a_class_prop() {
    let c = content();
    let class_keys: Vec<&str> = class_props(&c).iter().map(|p| p.key.as_str()).collect();
    for q in &c.quests {
        for o in &q.objectives {
            let hit = match o {
                Objective::Interact(k, _) => class_keys.contains(&k.as_str()),
                Objective::Sequence { props, .. } => {
                    props.iter().any(|k| class_keys.contains(&k.as_str()))
                }
                Objective::UseOn { target, .. } => class_keys.contains(&target.as_str()),
                Objective::Collect(tag, _) | Objective::Inspect(tag, _) => class_props(&c)
                    .iter()
                    .any(|p| !p.tag.is_empty() && p.tag == *tag),
                _ => false,
            };
            assert!(!hit, "quest {} depends on a class prop: {o:?}", q.id);
        }
    }
}
