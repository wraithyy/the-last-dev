//! Shared harness for combat_core integration tests.
#![allow(dead_code)] // each test binary uses a different subset of the harness

use combat_core::{
    Aabb, CastError, Command, Content, EntityId, Event, Sim, SpellId, UnitDef, UnitState, Vec3,
};

pub const SRC: &str = include_str!("../../../content/game.ron");
pub const CR: SpellId = SpellId(1);
pub const RF: SpellId = SpellId(2);
pub const GR: SpellId = SpellId(3);
pub const RPR: SpellId = SpellId(4);
pub const BLAST: SpellId = SpellId(5);

pub fn content() -> Content {
    match Content::from_ron(SRC) {
        Ok(c) => c,
        Err(e) => panic!("shipped content invalid: {e}"),
    }
}

pub fn def_mut<'a>(c: &'a mut Content, key: &str) -> &'a mut UnitDef {
    match c.units.iter_mut().find(|u| u.key == key) {
        Some(u) => u,
        None => panic!("no unit {key}"),
    }
}

pub struct H {
    pub sim: Sim,
    pub player: EntityId,
    pub mob: EntityId,
    /// (tick, event) in emission order.
    pub log: Vec<(u64, Event)>,
}

impl H {
    /// Player at the origin facing -Z; CEO at `(0, 0, mob_z)`.
    pub fn new(seed: u64, mob_z: f32, tweak: impl FnOnce(&mut Content)) -> H {
        let mut c = content();
        // Rules tests assert exact outcomes; miss tests opt in through `tweak`.
        c.config.white_hit_pct = 100;
        c.config.spell_hit_pct = 100;
        // Regen and player yells have dedicated tests; keep exact-HP/mana and event-count tests stable.
        c.config.regen.hp_pct = 0;
        c.config.regen.mana_pct = 0;
        for s in &mut c.spells {
            s.yells.clear();
        }
        tweak(&mut c);
        let mut sim = Sim::new(c, seed);
        let player = sim.spawn_player(Vec3::new(0.0, 0.0, 0.0), 0.0);
        let mob = match sim.spawn_unit("ceo", Vec3::new(0.0, 0.0, mob_z), 0.0) {
            Ok(id) => id,
            Err(e) => panic!("spawn failed: {e}"),
        };
        H {
            sim,
            player,
            mob,
            log: Vec::new(),
        }
    }

    pub fn tick(&mut self) {
        let now = self.sim.now().0 + 1;
        for e in self.sim.tick() {
            self.log.push((now, e));
        }
    }

    pub fn run(&mut self, n: u32) {
        for _ in 0..n {
            self.tick();
        }
    }

    /// Ticks until `pred` matches a newly emitted event; false after `max` ticks.
    pub fn run_until(&mut self, max: u32, pred: impl Fn(&Event) -> bool) -> bool {
        let start = self.log.len();
        for _ in 0..max {
            self.tick();
            if self.log[start..].iter().any(|(_, e)| pred(e)) {
                return true;
            }
        }
        false
    }

    pub fn target(&mut self, t: EntityId) {
        self.sim.push(Command::SetTarget {
            unit: self.player,
            target: Some(t),
        });
    }

    pub fn cast(&mut self, spell: SpellId) {
        self.sim.push(Command::Cast {
            caster: self.player,
            spell,
        });
    }

    pub fn unit(&self, id: EntityId) -> &UnitState {
        match self.sim.unit(id) {
            Some(u) => u,
            None => panic!("no unit {id}"),
        }
    }

    pub fn events(&self) -> impl Iterator<Item = &(u64, Event)> {
        self.log.iter()
    }

    pub fn first_tick(&self, pred: impl Fn(&Event) -> bool) -> Option<u64> {
        self.log.iter().find(|(_, e)| pred(e)).map(|(t, _)| *t)
    }

    pub fn count(&self, pred: impl Fn(&Event) -> bool) -> usize {
        self.log.iter().filter(|(_, e)| pred(e)).count()
    }

    pub fn failed(&self, spell: SpellId) -> Vec<CastError> {
        self.log
            .iter()
            .filter_map(|(_, e)| match e {
                Event::CastFailed {
                    spell: s, reason, ..
                } if *s == spell => Some(*reason),
                _ => None,
            })
            .collect()
    }

    pub fn damage_to(&self, target: EntityId) -> Vec<(u32, u32, bool, bool)> {
        self.log
            .iter()
            .filter_map(|(_, e)| match e {
                Event::Damage {
                    target: t,
                    amount,
                    absorbed,
                    crit,
                    combo,
                    ..
                } if *t == target => Some((*amount, *absorbed, *crit, *combo)),
                _ => None,
            })
            .collect()
    }
}

pub fn wall(z: f32) -> Aabb {
    Aabb {
        min: Vec3::new(-5.0, -1.0, z - 0.5),
        max: Vec3::new(5.0, 10.0, z + 0.5),
    }
}
