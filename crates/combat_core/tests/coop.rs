//! Co-op balance: a three-player group (Backend tank, Fullstack DPS, DevOps healer) clears
//! the tower floors pack by pack and must still feel the bosses, while pulling a whole floor at
//! once wipes it. Group scaling, pack growth and linked packs are what make this hold.

mod common;

use combat_core::{
    AiState, Command, Content, EntityId, Sim, SpellId, TelegraphView, UnitState, Vec3,
};
use common::*;

const PACK_SPACING: f32 = 70.0;
const RUN_MPS: f32 = 7.0;
const MAX_TICKS: u64 = 60_000;

const BLAME: SpellId = SpellId(6);
const ROLLBACK: SpellId = SpellId(7);
const BY_HAND: SpellId = SpellId(11);
const BLOCKING_CALL: SpellId = SpellId(30);
const RATE_LIMITER: SpellId = SpellId(31);
const TAUNT: SpellId = SpellId(32);
const CONN_POOL: SpellId = SpellId(33);
const KILL_QUERY: SpellId = SpellId(34);
const HORIZONTAL: SpellId = SpellId(36);
const RETRY: SpellId = SpellId(37);
const CACHE_INVAL: SpellId = SpellId(38);
const DROP_TABLE: SpellId = SpellId(39);
const KILL_9: SpellId = SpellId(40);
const HOTFIX: SpellId = SpellId(41);
const SIGSTOP: SpellId = SpellId(42);
const AUTO_SCALING: SpellId = SpellId(43);
const REVERT: SpellId = SpellId(45);
const FEATURE_FLAG: SpellId = SpellId(46);
const CIRCUIT: SpellId = SpellId(47);

/// Same corridor as the solo playthrough: packs 70 m apart toward -Z, boss at the end.
fn anchor(name: &str) -> Option<(Vec3, f32)> {
    let num = |p: &str| name.strip_prefix(p).and_then(|n| n.parse::<f32>().ok());
    if let Some(n) = num("pack_") {
        return Some((Vec3::new(0.0, 0.0, -PACK_SPACING * n), 0.0));
    }
    if let Some(n) = num("npc_") {
        return Some((Vec3::new(40.0, 0.0, -15.0 * n), 0.0));
    }
    (name == "boss").then_some((Vec3::new(0.0, 0.0, -PACK_SPACING * 7.0), 0.0))
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Tank,
    Dps,
    Healer,
}

struct Member {
    id: EntityId,
    role: Role,
    pos: Vec3,
    moving: bool,
    goal: Option<Vec3>,
}

struct Group {
    sim: Sim,
    m: Vec<Member>,
    /// Where the tank holds the pull; ranged members stand behind it.
    front: Vec3,
    in_boss: bool,
    boss_min_pct: u32,
    wiped: bool,
    deaths: u32,
}

fn pct(u: &UnitState) -> u32 {
    u.hp * 100 / u.max_hp.max(1)
}

impl Group {
    fn new(content: Content, zone: &str, level: u8, seed: u64) -> Group {
        Group::new_at(content, zone, level, seed, &anchor)
    }

    fn new_at(
        content: Content,
        zone: &str,
        level: u8,
        seed: u64,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    ) -> Group {
        let mut sim = Sim::new(content, seed);
        sim.set_wandering(false);
        let start = Vec3::new(0.0, 0.0, 20.0);
        let mut m = Vec::new();
        for (k, (class, role)) in [
            ("backend", Role::Tank),
            ("fullstack", Role::Dps),
            ("devops", Role::Healer),
        ]
        .into_iter()
        .enumerate()
        {
            let pos = Vec3::new(k as f32 * 2.0, 0.0, 20.0);
            let id = sim.spawn_player(class, pos, 0.0);
            assert_ne!(id, 0);
            assert!(sim.set_level_of(id, level));
            m.push(Member {
                id,
                role,
                pos,
                moving: false,
                goal: None,
            });
        }
        sim.set_respawn_point(start, 0.0);
        if let Err(e) = sim.load_zone(zone, anchors, Vec::new()) {
            panic!("load_zone {zone}: {e}");
        }
        Group {
            sim,
            m,
            front: start,
            in_boss: false,
            boss_min_pct: 100,
            wiped: false,
            deaths: 0,
        }
    }

    fn unit(&self, id: EntityId) -> Option<&UnitState> {
        self.sim.unit(id)
    }

    fn alive(&self, k: usize) -> bool {
        self.unit(self.m[k].id).is_some_and(|u| u.alive)
    }

    fn enemies(&self) -> Vec<UnitState> {
        self.sim
            .units()
            .filter(|u| u.hostile && u.alive && u.in_combat && u.ai != Some(AiState::Evade))
            .cloned()
            .collect()
    }

    fn in_combat(&self) -> bool {
        (0..self.m.len()).any(|k| self.unit(self.m[k].id).is_some_and(|u| u.in_combat))
    }

    fn ready(&self, k: usize, spell: SpellId) -> bool {
        self.sim.cooldown(self.m[k].id, spell).usable.is_ok()
    }

    fn off_cooldown(&self, k: usize, spell: SpellId) -> bool {
        let v = self.sim.cooldown(self.m[k].id, spell);
        v.cd_remaining_ms == 0 && v.usable != Err(combat_core::CastError::NotLearned)
    }

    fn cmd(&mut self, k: usize, c: Command) {
        let id = self.m[k].id;
        self.sim.apply_as(id, c);
    }

    fn target(&mut self, k: usize, t: EntityId) {
        let id = self.m[k].id;
        self.cmd(
            k,
            Command::SetTarget {
                unit: id,
                target: Some(t),
            },
        );
    }

    fn cast(&mut self, k: usize, spell: SpellId) {
        let id = self.m[k].id;
        self.cmd(k, Command::Cast { caster: id, spell });
    }

    fn walk(&mut self, k: usize, goal: Vec3) {
        let p = self.m[k].pos;
        let (dx, dz) = (goal.x - p.x, goal.z - p.z);
        let d = (dx * dx + dz * dz).sqrt();
        if d < 0.05 {
            return;
        }
        let step = (RUN_MPS / 30.0).min(d);
        self.m[k].pos = Vec3::new(p.x + dx / d * step, 0.0, p.z + dz / d * step);
        self.m[k].moving = true;
    }

    /// Out of non-shared telegraphs toward the nearest safe spot (cover included).
    fn dodge(&mut self, k: usize) -> bool {
        let tgs: Vec<TelegraphView> = self
            .sim
            .telegraphs()
            .into_iter()
            .filter(|t| !t.split)
            .collect();
        let bad = |p: Vec3| tgs.iter().any(|t| t.hits(p));
        let pos = self.m[k].pos;
        if !bad(pos) {
            self.m[k].goal = None;
            return false;
        }
        if let Some(g) = self.m[k].goal.filter(|g| !bad(*g)) {
            self.walk(k, g);
            return true;
        }
        let mut cands: Vec<Vec3> = tgs.iter().flat_map(|t| t.cover.clone()).collect();
        for r in 1..=10 {
            for a in 0..16 {
                let ang = a as f32 * std::f32::consts::TAU / 16.0;
                let r = r as f32 * 1.5;
                cands.push(Vec3::new(pos.x + r * ang.cos(), 0.0, pos.z + r * ang.sin()));
            }
        }
        let goal = cands.into_iter().filter(|p| !bad(*p)).min_by(|a, b| {
            combat_core::distance(*a, pos).total_cmp(&combat_core::distance(*b, pos))
        });
        self.m[k].goal = goal;
        match goal {
            Some(g) => {
                self.walk(k, g);
                true
            }
            None => false,
        }
    }

    /// Group target: a must-interrupt caster first, then adds (low max HP), then the nearest.
    fn focus(&self) -> Option<UnitState> {
        let en = self.enemies();
        let c = self.sim.content();
        let kicking = en.iter().find(|u| {
            u.cast
                .is_some_and(|k| k.interruptible && c.spell(k.spell).is_some_and(|s| s.kick))
        });
        if let Some(u) = kicking {
            return Some(u.clone());
        }
        en.into_iter().min_by(|a, b| {
            a.max_hp.cmp(&b.max_hp).then(
                combat_core::distance(a.pos, self.front)
                    .total_cmp(&combat_core::distance(b.pos, self.front)),
            )
        })
    }

    fn act(&mut self) {
        let focus = self.focus();
        for k in 0..self.m.len() {
            if !self.alive(k) || self.unit(self.m[k].id).is_some_and(|u| u.stunned) {
                continue;
            }
            if self.dodge(k) {
                continue;
            }
            let Some(t) = focus.clone() else { continue };
            // Melee stays on the target, ranged keeps 18-24 m.
            let (far, near) = match self.m[k].role {
                Role::Tank => (4.0, 2.5),
                _ => (26.0, 20.0),
            };
            let d = combat_core::distance(t.pos, self.m[k].pos);
            if d > far {
                let p = self.m[k].pos;
                let f = (d - near) / d;
                self.walk(
                    k,
                    Vec3::new(p.x + (t.pos.x - p.x) * f, 0.0, p.z + (t.pos.z - p.z) * f),
                );
                continue;
            }
            match self.m[k].role {
                Role::Tank => self.tank(k, &t),
                Role::Dps => self.dps(k, &t),
                Role::Healer => self.heal(k, &t),
            }
        }
    }

    fn busy(&self, k: usize) -> bool {
        self.unit(self.m[k].id).is_some_and(|u| u.cast.is_some())
            || self.sim.cooldown(self.m[k].id, CR).gcd_remaining_ms > 0
    }

    fn kick(&mut self, k: usize, t: &UnitState, spell: SpellId) -> bool {
        let c = self.sim.content();
        let must = t
            .cast
            .is_some_and(|x| x.interruptible && c.spell(x.spell).is_some_and(|s| s.kick));
        if must && self.off_cooldown(k, spell) {
            self.target(k, t.id);
            self.cast(k, spell);
            return true;
        }
        false
    }

    fn tank(&mut self, k: usize, t: &UnitState) {
        let me = self.m[k].id;
        let hp = self.unit(me).map_or(100, pct);
        self.target(k, t.id);
        if self.kick(k, t, KILL_QUERY) {
            return;
        }
        // Taunt whatever is beating on someone else.
        let loose = self
            .enemies()
            .into_iter()
            .find(|u| u.target.is_some_and(|x| x != me));
        if let Some(l) = loose
            && self.off_cooldown(k, TAUNT)
        {
            self.target(k, l.id);
            self.cast(k, TAUNT);
            return;
        }
        if hp < 50 && self.ready(k, HORIZONTAL) {
            self.cast(k, HORIZONTAL);
        }
        if self.busy(k) {
            return;
        }
        let shielded = self
            .unit(me)
            .is_some_and(|u| u.auras.iter().any(|a| a.key == "rate_limiter"));
        let near = self
            .enemies()
            .iter()
            .filter(|u| combat_core::distance(u.pos, self.m[k].pos) <= 8.0)
            .count();
        for (ok, s) in [
            (!shielded && hp < 90, RATE_LIMITER),
            (hp < 60, RETRY),
            (near >= 2, CONN_POOL),
            (t.max_hp > 1000, DROP_TABLE),
            (true, CACHE_INVAL),
            (true, BLOCKING_CALL),
        ] {
            if ok && self.ready(k, s) {
                return self.cast(k, s);
            }
        }
    }

    fn dps(&mut self, k: usize, t: &UnitState) {
        self.target(k, t.id);
        if self.kick(k, t, RPR) {
            return;
        }
        if self.busy(k) {
            return;
        }
        let hp = self.unit(self.m[k].id).map_or(100, pct);
        let blamed = t.auras.iter().any(|a| a.key == "git_blame");
        let smell = t.auras.iter().any(|a| a.key == "code_smell");
        for (ok, s) in [
            (hp < 50, ROLLBACK),
            (!blamed && t.hp > 150, BLAME),
            (t.max_hp > 1000, BY_HAND),
            (smell, RF),
            (true, CR),
        ] {
            if ok && self.ready(k, s) {
                return self.cast(k, s);
            }
        }
    }

    fn heal(&mut self, k: usize, t: &UnitState) {
        if self.kick(k, t, SIGSTOP) {
            return;
        }
        if self.busy(k) {
            return;
        }
        let hurt = (0..self.m.len())
            .filter(|&j| self.alive(j))
            .map(|j| {
                let u = self.unit(self.m[j].id).cloned();
                (j, u.as_ref().map_or(100, pct), u)
            })
            .min_by_key(|(_, p, _)| *p);
        if let Some((j, p, Some(u))) = hurt {
            let has = |key: &str| u.auras.iter().any(|a| a.key == key);
            let tank = self.m[j].role == Role::Tank;
            let deadline = has("deadline");
            let plan = [
                (deadline, FEATURE_FLAG),
                (p < 40, REVERT),
                (p < 65 && tank && !has("circuit_breaker"), CIRCUIT),
                (p < 85 && !has("auto_scaling"), AUTO_SCALING),
                (p < 60, HOTFIX),
            ];
            for (ok, s) in plan {
                if ok && self.off_cooldown(k, s) {
                    let id = self.m[j].id;
                    self.target(k, id);
                    self.cast(k, s);
                    self.target(k, t.id);
                    return;
                }
            }
        }
        self.target(k, t.id);
        if self.ready(k, KILL_9) {
            self.cast(k, KILL_9);
        }
    }

    fn tick(&mut self) {
        let focus = self.focus().map(|u| u.pos);
        for k in 0..self.m.len() {
            let p = self.m[k].pos;
            let yaw = focus.map_or(0.0, |t| (-(t.x - p.x)).atan2(-(t.z - p.z)));
            let moving = std::mem::take(&mut self.m[k].moving);
            let id = self.m[k].id;
            // Movement first, so this tick's casts see this tick's facing.
            self.sim.apply_as(
                id,
                Command::PlayerMoved {
                    pos: p,
                    yaw,
                    moving,
                    jumping: false,
                },
            );
        }
        let _ = self.sim.tick();
        if self.in_boss {
            for k in 0..self.m.len() {
                if let Some(u) = self.unit(self.m[k].id) {
                    self.boss_min_pct = self.boss_min_pct.min(pct(u));
                }
            }
        }
        let dead = (0..self.m.len()).filter(|&k| !self.alive(k)).count();
        self.deaths = self.deaths.max(dead as u32);
        if dead == self.m.len() {
            self.wiped = true;
        }
    }

    fn step(&mut self) {
        self.act();
        self.tick();
    }

    fn fight(&mut self) {
        let start = self.sim.now().0;
        while self.sim.now().0 - start < 120 || self.in_combat() {
            if self.wiped || self.sim.now().0 > MAX_TICKS {
                return;
            }
            self.step();
        }
    }

    fn rest(&mut self) {
        let full = |g: &Group| {
            (0..g.m.len()).all(|k| {
                g.unit(g.m[k].id)
                    .is_none_or(|u| !u.alive || (pct(u) >= 95 && u.mana * 100 >= u.max_mana * 90))
            })
        };
        while !full(self) && self.sim.now().0 < MAX_TICKS && !self.in_combat() {
            self.tick();
        }
    }

    /// Tank opens from 4 m, the others hang back 20 m.
    fn pull(&mut self, at: Vec3) {
        self.front = at;
        for k in 0..self.m.len() {
            let back = if self.m[k].role == Role::Tank {
                4.0
            } else {
                22.0
            };
            self.m[k].pos = Vec3::new(at.x + k as f32 * 1.5, 0.0, at.z + back);
        }
        self.tick();
        self.fight();
    }

    /// Clears the floor pack by pack, then the boss; the report is (lowest group HP % in the
    /// boss fight, cleared, most members dead at once).
    fn clear(mut self, zone: &str) -> (u32, bool, u32) {
        let packs = self.sim.content().zone(zone).map_or(0, |z| z.packs.len());
        for n in 1..=packs {
            if let Some((at, _)) = anchor(&format!("pack_{n}")) {
                self.pull(at);
                if self.wiped {
                    return (0, false, self.deaths);
                }
                self.rest();
            }
        }
        if let Some((at, _)) = anchor("boss") {
            self.in_boss = true;
            self.pull(at);
        }
        let cleared = self.sim.zone_cleared(zone) && !self.wiped;
        (self.boss_min_pct, cleared, self.deaths)
    }
}

/// A real floor's packs sit within a room or two of each other, not 70 m apart: a ring of packs
/// 14 m around one spot, each beyond the link radius of its neighbours.
fn compact(name: &str) -> Option<(Vec3, f32)> {
    if let Some(n) = name
        .strip_prefix("pack_")
        .and_then(|n| n.parse::<f32>().ok())
    {
        let a = n * std::f32::consts::TAU / 6.0;
        return Some((Vec3::new(14.0 * a.cos(), 0.0, -60.0 + 14.0 * a.sin()), 0.0));
    }
    anchor(name)
}

const FLOORS: [&str; 5] = ["tower/1", "tower/2", "tower/3", "tower/4", "tower/5"];

/// The pre-rework co-op numbers, for the before/after report: HP +40%, damage +15% per extra
/// member, no pack growth, no linked packs, authored trash damage.
fn old_scaling(c: &mut Content) {
    c.config.group_hp_pct_per_player = 40;
    c.config.group_damage_pct_per_player = 15;
    c.config.group_pack_growth_players = 0;
    c.config.pack_link_m = 0.0;
    c.config.trash_damage_pct = 100;
    c.config.group_boss_damage_pct_per_player = 0;
}

#[test]
fn three_players_clear_the_tower_but_feel_every_boss() {
    let mut failures = Vec::new();
    let mut table = Vec::new();
    for zone in FLOORS {
        let level = content().zone(zone).map_or(1, |z| z.level_range.1);
        let mut now_low = 100;
        let mut old_low = 100;
        for seed in [7, 42, 1234] {
            let (low, cleared, deaths) = Group::new(content(), zone, level, seed).clear(zone);
            let mut old = content();
            old_scaling(&mut old);
            let (before, ..) = Group::new(old, zone, level, seed).clear(zone);
            println!(
                "{zone} seed {seed}: group boss-fight low {low}% (old scaling {before}%), cleared {cleared}, most dead at once {deaths}"
            );
            now_low = now_low.min(low);
            old_low = old_low.min(before);
            if !cleared {
                failures.push(format!("{zone} seed {seed}: the group did not clear it"));
            }
        }
        // Over three seeds, at least one boss pull must drop someone below 60%.
        if now_low >= 60 {
            failures.push(format!(
                "{zone}: the boss never got the group under 60% (lowest {now_low}%)"
            ));
        }
        table.push((zone, old_low, now_low));
    }
    println!("co-op lowest boss-fight HP % (zone, old scaling, now): {table:?}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn three_players_pulling_a_whole_floor_wipe() {
    for zone in ["tower/2", "tower/3", "tower/4"] {
        let (pulled, wiped) = floor_pull(content(), zone);
        let mut old = content();
        old_scaling(&mut old);
        let (old_pulled, old_wiped) = floor_pull(old, zone);
        println!(
            "{zone}: whole-floor pull of {pulled} enemies, wiped {wiped} (old scaling: {old_pulled} enemies, wiped {old_wiped})"
        );
        assert!(pulled >= 10, "{zone}: only {pulled} enemies joined");
        assert!(
            wiped,
            "{zone}: three players survived pulling the whole floor"
        );
    }
}

/// The tank drags every pack of the compact floor onto the group: (enemies pulled, wiped).
fn floor_pull(c: Content, zone: &str) -> (usize, bool) {
    {
        let level = content().zone(zone).map_or(1, |z| z.level_range.1);
        let mut g = Group::new_at(c, zone, level, 7, &compact);
        let packs = g.sim.content().zone(zone).map_or(0, |z| z.packs.len());
        // The tank sprints through every pack; the others follow behind it.
        for n in 1..=packs {
            let Some((at, _)) = compact(&format!("pack_{n}")) else {
                continue;
            };
            for k in 0..g.m.len() {
                g.m[k].pos = Vec3::new(at.x + k as f32, 0.0, at.z + 2.0);
            }
            g.tick();
        }
        let pulled = g.enemies().len();
        g.front = g.m[0].pos;
        let start = g.sim.now().0;
        while !g.wiped && g.sim.now().0 - start < 30 * 120 {
            g.step();
        }
        (pulled, g.wiped)
    }
}

#[test]
fn packs_grow_with_the_group_and_linked_packs_come_together() {
    let count = |players: usize| {
        let mut sim = Sim::new(content(), 1);
        sim.set_wandering(false);
        for k in 0..players {
            sim.spawn_player("fullstack", Vec3::new(k as f32, 0.0, 20.0), 0.0);
        }
        if let Err(e) = sim.load_zone("tower/2", &anchor, Vec::new()) {
            panic!("{e}");
        }
        sim.units().filter(|u| u.hostile && !u.boss).count()
    };
    let solo = count(1);
    let packs = content().zone("tower/2").map_or(0, |z| z.packs.len());
    assert_eq!(count(2), solo, "growth starts at the third member");
    assert_eq!(count(3), solo + packs, "one extra member per pack");
    assert_eq!(count(5), solo + 2 * packs);

    // A member joining after the load grows the idle packs on the spot.
    let mut sim = Sim::new(content(), 1);
    sim.set_wandering(false);
    sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 20.0), 0.0);
    sim.spawn_player("fullstack", Vec3::new(1.0, 0.0, 20.0), 0.0);
    if let Err(e) = sim.load_zone("tower/2", &anchor, Vec::new()) {
        panic!("{e}");
    }
    sim.spawn_player("fullstack", Vec3::new(2.0, 0.0, 20.0), 0.0);
    assert_eq!(
        sim.units().filter(|u| u.hostile && !u.boss).count(),
        solo + packs
    );

    // Two packs 6 m apart: pulling one brings the other.
    let close = |name: &str| match name {
        "pack_1" => Some((Vec3::new(0.0, 0.0, -20.0), 0.0)),
        "pack_2" => Some((Vec3::new(4.0, 0.0, -24.0), 0.0)),
        other => anchor(other),
    };
    let mut sim = Sim::new(content(), 1);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, -12.0), 0.0);
    sim.set_player_level(4);
    if let Err(e) = sim.load_zone("tower/2", &close, Vec::new()) {
        panic!("{e}");
    }
    sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, -14.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    let _ = p;
    for _ in 0..10 {
        let _ = sim.tick();
    }
    let fighting: Vec<Option<u32>> = sim
        .units()
        .filter(|u| u.hostile && u.in_combat)
        .map(|u| u.pack)
        .collect();
    let mut ids = fighting.clone();
    ids.sort();
    ids.dedup();
    assert!(ids.len() >= 2, "linked packs should join: {fighting:?}");
}

#[test]
fn a_kited_reset_keeps_the_player_in_combat_for_a_while() {
    let mut sim = Sim::new(content(), 1);
    sim.set_wandering(false);
    let p = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    let mob = match sim.spawn_unit("scrum_master", Vec3::new(0.0, 0.0, -5.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    for _ in 0..30 {
        let _ = sim.tick();
    }
    assert!(sim.unit(p).is_some_and(|u| u.in_combat));
    // Run far beyond the leash: the scrum master evades home.
    sim.push(Command::PlayerMoved {
        pos: Vec3::new(0.0, 0.0, 200.0),
        yaw: 0.0,
        moving: false,
        jumping: false,
    });
    let mut evaded_at = None;
    for t in 0..600 {
        let _ = sim.tick();
        if evaded_at.is_none() && sim.unit(mob).is_some_and(|u| u.ai == Some(AiState::Evade)) {
            evaded_at = Some(t);
        }
        if let Some(e) = evaded_at
            && t == e + 60
        {
            assert!(
                sim.unit(p).is_some_and(|u| u.in_combat),
                "2 s after the reset: still locked in combat"
            );
        }
        if let Some(e) = evaded_at
            && t == e + 200
        {
            assert!(
                sim.unit(p).is_some_and(|u| !u.in_combat),
                "the lock runs out after 5 s"
            );
        }
    }
    assert!(evaded_at.is_some());
    assert!(
        sim.unit(mob).is_some_and(|u| u.hp == u.max_hp),
        "healed to full at home"
    );
}
