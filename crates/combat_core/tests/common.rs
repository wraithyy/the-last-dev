//! Shared harness for combat_core integration tests.
#![allow(dead_code)] // each test binary uses a different subset of the harness

use combat_core::{
    Aabb, CastError, Command, Content, Effect, EntityId, Event, Sim, SpellId, UnitDef, UnitState,
    Vec3,
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

/// The campaign path alone: the campus districts are optional open-world content on their own
/// level band (packs with a fixed level, the campus rares, `open_world` quests), so campaign
/// pacing tests leave them out.
pub fn without_districts(c: &mut Content) {
    for z in &mut c.zones {
        z.packs.retain(|p| p.level.is_none());
        if z.id == "campus" {
            z.rares.clear();
        }
    }
    c.quests.retain(|q| !q.open_world);
}

/// `content()` without the campus districts (`without_districts`).
pub fn campaign_content() -> Content {
    let mut c = content();
    without_districts(&mut c);
    c
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
            // Rules tests exercise every spell from level 1; progression has its own tests.
            s.learn_level = 1;
        }
        legacy_ceo(&mut c);
        tweak(&mut c);
        let mut sim = Sim::new(c, seed);
        let player = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
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

/// Rules tests predate floors and levels: pin the CEO to its original single-ability
/// encounter (900 HP, 14 melee, 45 Blast, same level as the player, no phases).
pub fn legacy_ceo(c: &mut Content) {
    let ceo = def_mut(c, "ceo");
    ceo.level = 1;
    ceo.max_hp = 900;
    ceo.xp_reward = 120;
    if let Some(ai) = ceo.ai.as_mut() {
        ai.melee_damage = 14;
        ai.leash_m = 35.0;
        ai.respawn_ms = 15000;
        ai.abilities.retain(|a| a.spell == BLAST);
        ai.phases.clear();
        ai.flee = None;
        ai.enrage = None;
    }
    // The shipped CEO always drops the deck (a quest needs it); the legacy encounter rolled 35%.
    for l in ceo
        .loot
        .iter_mut()
        .filter(|l| l.item == "Unread AI Strategy Deck")
    {
        l.chance_pct = 35;
    }
    for s in c.spells.iter_mut().filter(|s| s.id == BLAST) {
        if let Effect::Damage { base, .. } = &mut s.effect {
            *base = 45;
        }
    }
}

pub fn unit_name(sim: &Sim, id: u64) -> String {
    sim.unit(id)
        .map_or_else(|| format!("#{id}"), |u| u.name.clone())
}

pub fn spell_name(sim: &Sim, id: SpellId) -> String {
    sim.content()
        .spell(id)
        .map_or_else(|| format!("spell{}", id.0), |s| s.name.clone())
}

pub fn log_line(sim: &Sim, tick: u64, e: &Event) -> String {
    let t = |id: u64| unit_name(sim, id);
    let body = match e {
        Event::CastStarted {
            caster,
            spell: s,
            total_ms,
        } => {
            format!(
                "{} begins {} ({total_ms} ms)",
                t(*caster),
                spell_name(sim, *s)
            )
        }
        Event::CastSucceeded {
            caster, spell: s, ..
        } => format!("{} casts {}", t(*caster), spell_name(sim, *s)),
        Event::CastFailed {
            caster,
            spell: s,
            reason,
        } => {
            format!(
                "{} failed {}: {}",
                t(*caster),
                spell_name(sim, *s),
                reason.message()
            )
        }
        Event::CastInterrupted {
            caster,
            spell: s,
            by,
            lockout_ms,
        } => format!(
            "{} {} interrupted by {} (lockout {lockout_ms} ms)",
            t(*caster),
            spell_name(sim, *s),
            by.map_or_else(|| "movement/cancel".to_string(), t)
        ),
        Event::CastQueued { caster, spell: s } => {
            format!("{} queues {}", t(*caster), spell_name(sim, *s))
        }
        Event::Damage {
            source,
            target,
            spell: s,
            amount,
            absorbed,
            crit,
            combo,
            ..
        } => format!(
            "{} hits {} with {} for {amount} (absorbed {absorbed}){}{}",
            t(*source),
            t(*target),
            s.map_or_else(|| "melee".to_string(), |s| spell_name(sim, s)),
            if *crit { " CRIT" } else { "" },
            if *combo { " COMBO" } else { "" },
        ),
        Event::MeleeSwing { attacker, target } => {
            format!("{} swings at {}", t(*attacker), t(*target))
        }
        Event::Miss {
            source,
            target,
            spell: s,
        } => format!(
            "{} misses {} with {}",
            t(*source),
            t(*target),
            s.map_or_else(|| "melee".to_string(), |s| spell_name(sim, s)),
        ),
        Event::AutoAttackToggled { unit, on } => format!(
            "{} auto attack {}",
            t(*unit),
            if *on { "on" } else { "off" }
        ),
        Event::AuraApplied { target, key } => format!("{} gains {key}", t(*target)),
        Event::AuraRemoved {
            target,
            key,
            reason,
        } => format!("{} loses {key} ({reason:?})", t(*target)),
        Event::EnterCombat { unit } => format!("{} enters combat", t(*unit)),
        Event::LeaveCombat { unit } => format!("{} leaves combat", t(*unit)),
        Event::AiState { unit, state } => format!("{} ai {state:?}", t(*unit)),
        Event::Death { unit, killer } => {
            format!(
                "{} dies (killer {})",
                t(*unit),
                killer.map_or_else(|| "none".into(), t)
            )
        }
        Event::XpGained { unit, amount } => format!("{} gains {amount} xp", t(*unit)),
        Event::LootDropped { corpse, items, sp } => {
            format!("{} drops {items:?} and {sp} SP", t(*corpse))
        }
        Event::Looted {
            looter,
            corpse,
            items,
            sp,
        } => format!("{} loots {} {items:?} and {sp} SP", t(*looter), t(*corpse)),
        Event::Respawned { unit } => format!("{} respawns", t(*unit)),
        Event::Resurrected { unit, by, hp, mana } => {
            format!("{} resurrects {} ({hp} HP, {mana} mana)", t(*by), t(*unit))
        }
        Event::Yell { unit, text } => format!("{} yells: {text}", t(*unit)),
        Event::Heal {
            source,
            target,
            spell: s,
            amount,
        } => format!(
            "{} heals {} with {} for {amount}",
            t(*source),
            t(*target),
            s.map_or_else(|| "?".to_string(), |s| spell_name(sim, s)),
        ),
        Event::ManaGain {
            unit,
            spell,
            amount,
        } => format!(
            "{} restores {amount} mana with {}",
            t(*unit),
            spell_name(sim, *spell)
        ),
        Event::Knockback {
            unit,
            dir,
            strength,
        } => format!(
            "{} knocked back {strength:.1} m toward ({:.2}, {:.2})",
            t(*unit),
            dir.x,
            dir.z
        ),
        Event::Teleported { unit, pos } => {
            format!("{} teleported to ({:.1}, {:.1})", t(*unit), pos.x, pos.z)
        }
        Event::LevelUp { unit, level } => format!("{} reaches level {level}", t(*unit)),
        Event::SpellLearned { unit, spell: s } => {
            format!("{} learns {}", t(*unit), spell_name(sim, *s))
        }
        Event::PhaseChanged { unit, phase } => format!("{} enters phase {phase}", t(*unit)),
        Event::Summoned { unit, summoner } => {
            format!("{} summons {} #{unit}", t(*summoner), t(*unit))
        }
        Event::Mirrored { mirror, player } => {
            format!("{} copies {}", t(*mirror), t(*player))
        }
        Event::Review {
            grades,
            worst,
            best,
        } => format!("review {grades:?} worst {worst:?} best {best:?}"),
        Event::Despawned { unit } => format!("#{unit} despawns"),
        Event::FloorLoaded { floor } => format!("floor {floor} loaded"),
        Event::FloorCleared { floor } => format!("floor {floor} cleared"),
        Event::ZoneLoaded { zone } => format!("zone {zone} loaded"),
        Event::ZoneCleared { zone } => format!("zone {zone} cleared"),
        Event::Unlocked { zone } => format!("{zone} unlocked"),
        Event::QuestOffered { npc, quests } => format!("{} offers {quests:?}", t(*npc)),
        Event::QuestAccepted { quest } => format!("quest {quest} accepted"),
        Event::QuestProgress {
            quest,
            objective,
            count,
            need,
        } => format!("quest {quest} objective {objective}: {count}/{need}"),
        Event::QuestReady { quest } => format!("quest {quest} ready"),
        Event::QuestCompleted { quest } => format!("quest {quest} completed"),
        Event::QuestFailed { quest } => format!("quest {quest} failed"),
        Event::TelegraphStarted {
            id,
            source,
            spell: s,
            shape,
            pos,
            resolve_ms,
            ..
        } => format!(
            "{} telegraphs {} #{id} {shape:?} at ({:.1}, {:.1}) in {resolve_ms} ms",
            t(*source),
            spell_name(sim, *s),
            pos.x,
            pos.z
        ),
        Event::TelegraphResolved { id, landed } => {
            format!(
                "telegraph #{id} {}",
                if *landed { "lands" } else { "fizzles" }
            )
        }
        Event::Bag { unit, sp, bag, .. } => {
            format!("{} bag: {} items, {sp} SP", t(*unit), bag.len())
        }
        Event::GearChanged { unit, equipped } => format!("{} wears {equipped:?}", t(*unit)),
        Event::Proc { unit, item } => format!("{} procs {item}", t(*unit)),
        Event::ItemError { reason } => format!("item error: {}", reason.message()),
        Event::ItemCooldown { item, ms } => format!("{item} cools down for {ms} ms"),
        Event::Interacted {
            unit,
            prop,
            outcome,
        } => format!("{} uses {prop} ({outcome})", t(*unit)),
        Event::Achievement { unit, key } => format!("{} earns {key}", t(*unit)),
        Event::TitleChanged { unit, title } => format!("{} wears title {title:?}", t(*unit)),
        Event::TalentsChanged { unit, talents } => format!("{} talents {talents:?}", t(*unit)),
        Event::SprintStarted { status, zones } => {
            format!(
                "sprint {} +{} starts: {zones:?}",
                status.building, status.level
            )
        }
        Event::SprintProgress { status } => format!(
            "sprint {} ms, {}% burned, {} bosses left",
            status.time_ms(),
            status.forces_pct,
            status.bosses_left
        ),
        Event::SprintEnded {
            result,
            time_ms,
            upgrade,
            ..
        } => format!("sprint ends {result:?} in {time_ms} ms, upgrade {upgrade}"),
        Event::SprintAffix { affix, text, .. } => format!("affix {affix}: {text}"),
        Event::SprintRefused { reason } => format!("sprint refused: {}", reason.message()),
        Event::Clock { unix_s } => format!("clock {unix_s}"),
        Event::DailyBoard { board, open } => format!(
            "board day {} open {open}: {} tasks",
            board.day,
            board.tasks.len()
        ),
        Event::TaskCompleted {
            key,
            weekly,
            xp,
            sp,
        } => format!("task {key} done (weekly {weekly}): {xp} XP, {sp} SP"),
        Event::WorldBoss { state, secs, unit } => {
            format!("world boss {state:?} {secs} s ({})", t(*unit))
        }
        Event::WorldEvent {
            key,
            phase,
            secs,
            count,
            need,
        } => format!("world event {key} {phase:?} {secs} s {count}/{need}"),
        Event::PetChanged { unit, pet } => format!("{} pet: {pet}", t(*unit)),
        Event::PetLearned { unit, pet } => format!("{} learns pet {pet}", t(*unit)),
        Event::PetLooted { unit, items, .. } => {
            format!("{}'s pet sweeps up {}", t(*unit), items.join(", "))
        }
        Event::DuelRequested { from, to } => format!("{} asks {} for a duel", t(*from), t(*to)),
        Event::DuelCountdown { a, b, .. } => format!("duel {} vs {} counts down", t(*a), t(*b)),
        Event::DuelBegan { a, b } => format!("duel {} vs {} begins", t(*a), t(*b)),
        Event::DuelEnded { outcome, .. } => format!("duel over: {outcome:?}"),
        Event::DuelRefused { reason } => format!("duel refused: {}", reason.message()),
        Event::RankProgress { unit, rank, xp, .. } => {
            format!("{} rank {rank}, {xp} XP to the next", t(*unit))
        }
        Event::RankUp { unit, rank } => format!("{} promoted to rank {rank}", t(*unit)),
        Event::Reputation {
            unit, dept, total, ..
        } => format!("{} dept {dept} standing {total}", t(*unit)),
        Event::Burnout {
            unit, value, cause, ..
        } => format!("{} burnout {value} ({cause:?})", t(*unit)),
        Event::BurnoutQuip { unit, kind, line } => format!("{} {kind:?} #{line}", t(*unit)),
        Event::TicketsClosed { unit, count, sp } => {
            format!("{} closed {count} Jira tickets for {sp} Kudos", t(*unit))
        }
        Event::Legacy { view } => format!(
            "legacy {} in {:?}, week {} locked {:?}",
            view.on, view.zone, view.week, view.locked
        ),
        Event::LegacyRefused { reason } => format!("legacy refused: {}", reason.message()),
        Event::Raid { view } => format!(
            "raid {:?} async {} week {} locked {:?} tethers {:?}",
            view.zone, view.async_mode, view.week, view.locked, view.tethers
        ),
        Event::Ladder { view } => format!(
            "ladder week {}: {:?}",
            view.week,
            view.entries.iter().map(|e| e.time_ms).collect::<Vec<_>>()
        ),
        Event::MinigameStarted { key, limit_ms } => {
            format!("minigame {key} starts ({limit_ms} ms)")
        }
        Event::Minigame { unit, view } => format!(
            "{} minigame: objectives {:b} snacks {} catches {} suspicion {}",
            t(*unit),
            view.objectives,
            view.collected,
            view.catches,
            view.suspicion
        ),
        Event::MinigameCaught {
            unit,
            by,
            kind,
            line,
        } => format!("{} caught by {} ({kind:?}): {line}", t(*unit), t(*by)),
        Event::MinigameNote { unit, text, good } => {
            format!("{} minigame note ({good}): {text}", t(*unit))
        }
        Event::MinigameEnded { result } => format!(
            "{} minigame over: {:?} score {} group {}",
            t(result.unit),
            result.medal,
            result.score,
            result.group_score
        ),
    };
    format!("[{tick:05}] {body}")
}

pub fn wall(z: f32) -> Aabb {
    Aabb {
        min: Vec3::new(-5.0, -1.0, z - 0.5),
        max: Vec3::new(5.0, 10.0, z + 0.5),
    }
}

/// One zone of `content/zone_geometry.ron` (the client's exported layouts): anchors and walls.
pub struct Geo {
    pub anchors: std::collections::HashMap<String, (Vec3, f32)>,
    pub blockers: Vec<Aabb>,
    pub terrain: combat_core::Terrain,
}

#[derive(serde::Deserialize)]
struct RawGeo {
    id: String,
    anchors: Vec<(String, [f32; 3], f32)>,
    blockers: Vec<([f32; 3], [f32; 3])>,
    #[serde(default)]
    terrain: combat_core::Terrain,
}

/// Every zone of the shipped geometry, by id.
pub fn geometry() -> std::collections::HashMap<String, Geo> {
    let raw: Vec<RawGeo> = match ron::from_str(include_str!("../../../content/zone_geometry.ron")) {
        Ok(r) => r,
        Err(e) => panic!("zone_geometry.ron: {e}"),
    };
    let v = |p: [f32; 3]| Vec3::new(p[0], p[1], p[2]);
    raw.into_iter()
        .map(|z| {
            let geo = Geo {
                anchors: z
                    .anchors
                    .into_iter()
                    .map(|(n, p, yaw)| (n, (v(p), yaw)))
                    .collect(),
                blockers: z
                    .blockers
                    .into_iter()
                    .map(|(min, max)| Aabb {
                        min: v(min),
                        max: v(max),
                    })
                    .collect(),
                terrain: z.terrain,
            };
            (z.id, geo)
        })
        .collect()
}
