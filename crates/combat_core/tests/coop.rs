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
    // The campus onboarding corner (named, not numbered), far out of every pull.
    if name.starts_with("npc_") {
        return Some((Vec3::new(-400.0, 0.0, 400.0), 0.0));
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
        let ev = self.sim.tick();
        if std::env::var_os("WOW_RAID_LOG").is_some() {
            let now = self.sim.now().0;
            for e in &ev {
                println!("{}", log_line(&self.sim, now, e));
            }
        }
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
        // A lone member has nobody to open for it: it walks into aggro range itself.
        let solo = self.m.len() == 1;
        for k in 0..self.m.len() {
            let back = match self.m[k].role {
                Role::Tank => 4.0,
                _ if solo => 5.0,
                _ => 22.0,
            };
            self.m[k].pos = Vec3::new(at.x + k as f32 * 1.5, 0.0, at.z + back);
        }
        self.tick();
        self.fight();
    }

    /// Clears the floor pack by pack, then the boss; the report is (lowest group HP % in the
    /// boss fight, cleared, most members dead at once).
    fn clear(&mut self, zone: &str) -> (u32, bool, u32) {
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

// ---- sprint mode ----

impl Group {
    /// Every member in the best World Rare of `tier` for its role, slot by slot.
    fn gear_up(&mut self, tier: u8) {
        let eco = self.sim.content().economy.clone();
        for k in 0..self.m.len() {
            let id = self.m[k].id;
            let role = self.sim.class_of(id).map(|c| c.role);
            for slot in combat_core::Slot::ALL {
                let best = eco
                    .gear
                    .iter()
                    .filter(|g| {
                        g.slot == slot
                            && g.tier() == tier
                            && g.rarity == combat_core::Rarity::Rare
                            && g.source == combat_core::GearSource::World
                    })
                    .max_by_key(|g| eco.score(g, role));
                if let Some(g) = best {
                    self.sim.debug_equip(id, &g.name);
                }
            }
        }
    }

    /// A whole sprint with the group as host: trash first, the boss last, respawn after wipes.
    fn sprint(
        &mut self,
        building: &str,
        level: u8,
    ) -> (Option<combat_core::SprintResult>, u32, u32) {
        let lead = self.m[0].id;
        self.sim.apply_as(
            lead,
            Command::StartSprint {
                building: building.into(),
                level,
            },
        );
        self.tick();
        let deadline = self.sim.now().0 + 40 * 60 * 30;
        while self.sim.sprint_last_result().is_none() && self.sim.now().0 < deadline {
            if let Some(z) = self.sim.sprint_travel().map(str::to_string) {
                if let Err(e) = self.sim.load_zone(&z, &anchor, Vec::new()) {
                    panic!("load_zone {z}: {e}");
                }
                self.tick();
                continue;
            }
            let dead: Vec<usize> = (0..self.m.len()).filter(|&k| !self.alive(k)).collect();
            if !dead.is_empty() && !self.in_combat() {
                for k in dead {
                    let id = self.m[k].id;
                    self.sim.apply_as(id, Command::RespawnPlayer);
                }
                self.tick();
                for k in 0..self.m.len() {
                    if let Some(u) = self.unit(self.m[k].id) {
                        self.m[k].pos = u.pos;
                    }
                }
                self.wiped = false;
                continue;
            }
            let left: Vec<UnitState> = self
                .sim
                .units()
                .filter(|u| u.hostile && u.alive && u.ai.is_some())
                .cloned()
                .collect();
            let evading = left.iter().any(|u| u.ai == Some(AiState::Evade));
            let front = self.front;
            let next = left
                .iter()
                .filter(|u| u.ai != Some(AiState::Evade))
                .min_by(|a, b| {
                    a.boss.cmp(&b.boss).then(
                        combat_core::distance(a.pos, front)
                            .total_cmp(&combat_core::distance(b.pos, front)),
                    )
                })
                .filter(|u| !u.boss || !evading)
                .map(|u| u.pos);
            match next {
                Some(at) => {
                    self.pull(at);
                    self.rest();
                }
                None => self.tick(),
            }
        }
        let st = self.sim.sprint_status().unwrap_or_default();
        let last = self.sim.sprint_last_result();
        (
            last.map(|l| l.0),
            last.map_or(st.time_ms(), |l| l.1),
            st.par_ms,
        )
    }
}

/// The three-bot group (tank, DPS, healer) at level 10 in tier-5 gear delivers sprint level 5
/// (two affixes) in every building.
#[test]
fn a_group_delivers_sprint_level_five() {
    let mut failures = Vec::new();
    for (seed, building) in [(7, "tower"), (42, "datacenter"), (1234, "lab")] {
        let mut g = Group::new(content(), "campus", 10, seed);
        g.gear_up(5);
        let (result, time, par) = g.sprint(building, 5);
        let affixes = g.sim.sprint_status().map(|s| s.affixes).unwrap_or_default();
        println!(
            "group {building} +5 seed {seed}: {result:?} in {time} ms (par {par}), affixes {affixes:?}, most dead at once {}",
            g.deaths
        );
        if result != Some(combat_core::SprintResult::Delivered) {
            failures.push(format!("{building} seed {seed}: {result:?} {time}/{par}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- the campus world boss ----

/// The campus with its packs out of the way and the Board's spot 40 m north.
fn lawn(name: &str) -> Option<(Vec3, f32)> {
    match name {
        n if n.starts_with("pack_") || n.starts_with("night_") => {
            Some((Vec3::new(500.0, 0.0, 500.0), 0.0))
        }
        "world_boss" => Some((Vec3::new(0.0, 0.0, -40.0), 0.0)),
        n if n.starts_with("npc_") || n == "scrum_bot" => Some((Vec3::new(-60.0, 0.0, 60.0), 0.0)),
        _ => None,
    }
}

/// Geared level-10 members (`keep` of tank, DPS, healer) meet the Board on the lawn. Returns
/// (killed, fight seconds, most dead at once).
fn board_meeting(keep: &[Role], seed: u64) -> (bool, u64, u32) {
    let mut g = Group::new_at(content(), "campus", 10, seed, &lawn);
    for k in (0..g.m.len()).rev() {
        if !keep.contains(&g.m[k].role) {
            let id = g.m[k].id;
            assert!(g.sim.remove_player(id));
            g.m.remove(k);
        }
    }
    g.gear_up(5);
    // 2026-10-15 12:00 UTC: daytime, so no night shift joins in.
    g.sim.set_clock(1_792_065_600);
    assert!(g.sim.debug_world_boss_in(2));
    for _ in 0..90 {
        g.tick();
    }
    let Some(boss) = g.sim.world_boss_unit() else {
        panic!("the Board did not show up")
    };
    let at = g.unit(boss).map_or(Vec3::new(0.0, 0.0, -40.0), |u| u.pos);
    let start = g.sim.now().0;
    g.in_boss = true;
    // A lone DPS has no tank to open: it walks into aggro range itself.
    let solo = g.m.len() == 1;
    g.front = at;
    for k in 0..g.m.len() {
        let back = match g.m[k].role {
            Role::Tank => 4.0,
            _ if solo => 10.0,
            _ => 22.0,
        };
        g.m[k].pos = Vec3::new(at.x + k as f32 * 1.5, 0.0, at.z + back);
    }
    g.tick();
    g.fight();
    let secs = (g.sim.now().0 - start) / 30;
    let killed = g.unit(boss).is_none_or(|u| !u.alive) && !g.wiped;
    (killed, secs, g.deaths)
}

#[test]
fn three_players_beat_the_board_and_one_does_not_trivially() {
    let mut failures = Vec::new();
    let mut group_secs = Vec::new();
    for seed in [7, 42, 1234] {
        let (killed, secs, deaths) = board_meeting(&[Role::Tank, Role::Dps, Role::Healer], seed);
        println!(
            "Board Meeting, group of 3, seed {seed}: killed {killed} in {secs} s, most dead at once {deaths}"
        );
        if !killed {
            failures.push(format!("group seed {seed}: no kill in {secs} s"));
        }
        group_secs.push(secs);
    }
    for seed in [7, 42, 1234] {
        let (killed, secs, deaths) = board_meeting(&[Role::Dps], seed);
        println!(
            "Board Meeting, solo DPS, seed {seed}: killed {killed} in {secs} s, died {deaths}"
        );
        // Solo is possible in principle, but never quick: a kill must take far longer than a
        // group's.
        let quickest = group_secs.iter().copied().max().unwrap_or(0);
        if killed && secs < quickest * 2 {
            failures.push(format!("solo seed {seed}: trivial kill in {secs} s"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- Legacy Mode ----

const LEGACY_FLOORS: [&str; 9] = [
    "tower/1",
    "tower/2",
    "tower/3",
    "tower/4",
    "tower/5",
    "datacenter/1",
    "datacenter/2",
    "lab/1",
    "lab/2",
];

struct LegacyRun {
    cleared: bool,
    secs: u64,
    /// Lowest HP % any member reached on the floor.
    low: u32,
    deaths: u32,
}

/// Level-10 members (`keep` of tank, DPS, healer) in the best tier-5 World Rares switch the
/// group to Legacy Mode on campus, take the door to `zone` and clear it pack by pack, boss
/// last. Cleared means every enemy on the floor (affix adds included) is dead and nobody wiped.
fn legacy_clear(zone: &str, keep: &[Role], seed: u64) -> LegacyRun {
    let mut g = Group::new(content(), "campus", 10, seed);
    for k in (0..g.m.len()).rev() {
        if !keep.contains(&g.m[k].role) {
            let id = g.m[k].id;
            assert!(g.sim.remove_player(id));
            g.m.remove(k);
        }
    }
    g.gear_up(5);
    let lead = g.m[0].id;
    g.sim.apply_as(lead, Command::SetLegacy { on: true });
    g.tick();
    assert!(g.sim.legacy_on(), "the lead switched to Legacy");
    if let Err(e) = g.sim.load_zone(zone, &anchor, Vec::new()) {
        panic!("load_zone {zone}: {e}");
    }
    assert_eq!(g.sim.legacy_zone(), Some(zone));
    let start = g.sim.now().0;
    // Track the lowest HP over the whole floor, not just the boss.
    g.in_boss = true;
    let (low, _, deaths) = g.clear(zone);
    // Stragglers: affix adds and anything a pull left behind.
    for _ in 0..8 {
        let next = g
            .sim
            .units()
            .find(|u| u.hostile && u.alive && u.ai.is_some())
            .map(|u| u.pos);
        match next {
            Some(at) if !g.wiped => {
                g.pull(at);
                g.rest();
            }
            _ => break,
        }
    }
    let left: Vec<String> = g
        .sim
        .units()
        .filter(|u| u.hostile && u.alive && u.ai.is_some())
        .map(|u| format!("{} {:?} at {:?}", u.key, u.ai, u.pos))
        .collect();
    let left = left.len();
    LegacyRun {
        cleared: left == 0 && !g.wiped,
        secs: (g.sim.now().0 - start) / 30,
        low: low.min(g.boss_min_pct),
        deaths: deaths.max(g.deaths),
    }
}

/// The E2 balance gate (docs/FORMULAS.md, Legacy Mode): three level-10 bots in Normal-tier
/// (tier-5 Rare) gear clear every Legacy floor on every seed, while a lone DPS in the same gear
/// either dies or only scrapes through, below 25% HP at some point.
#[test]
fn three_players_clear_every_legacy_floor_and_one_does_not_trivially() {
    let mut failures = Vec::new();
    let trio = [Role::Tank, Role::Dps, Role::Healer];
    for zone in LEGACY_FLOORS {
        for seed in [7u64, 42, 1234] {
            let g = legacy_clear(zone, &trio, seed);
            let s = legacy_clear(zone, &[Role::Dps], seed);
            println!(
                "Legacy {zone} seed {seed}: group cleared {} in {} s (low {}%, most dead {}); solo DPS cleared {} in {} s (low {}%, died {})",
                g.cleared, g.secs, g.low, g.deaths, s.cleared, s.secs, s.low, s.deaths
            );
            if !g.cleared {
                failures.push(format!("{zone}: the group did not clear it"));
            }
            // Trivial: a solo clear that never got dangerous. (Time alone cannot tell: enemies lose
            // the group HP bonus solo, so a lone DPS is only ~1.2-1.5x slower when it survives.)
            if s.cleared && s.low >= 25 {
                failures.push(format!(
                    "{zone}: solo cleared it in {} s at {}% lowest (group {} s)",
                    s.secs, s.low, g.secs
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- the raid: All-Hands Offsite ----

const RAID: [&str; 4] = ["offsite/1", "offsite/2", "offsite/3", "offsite/4"];

impl Group {
    /// Every member in the best Legacy piece (ilvl 12) of each slot for its role.
    fn gear_up_legacy(&mut self) {
        let eco = self.sim.content().economy.clone();
        for k in 0..self.m.len() {
            let id = self.m[k].id;
            let role = self.sim.class_of(id).map(|c| c.role);
            for slot in combat_core::Slot::ALL {
                let best = eco
                    .gear
                    .iter()
                    .filter(|g| g.slot == slot && g.source == combat_core::GearSource::Legacy)
                    .max_by_key(|g| eco.score(g, role));
                if let Some(g) = best {
                    self.sim.debug_equip(id, &g.name);
                }
            }
        }
    }

    /// Raid play on top of `act`: a tethered member closes in on its partner first.
    fn mind_tethers(&mut self) -> Vec<usize> {
        let view = self.sim.raid_view();
        let range = f32::from(view.tether_m.max(1)) - 2.0;
        let mut busy = Vec::new();
        for k in 0..self.m.len() {
            let me = self.m[k].id;
            let partner = view.tethers.iter().find_map(|&(a, b)| {
                if a == me {
                    Some(b)
                } else if b == me {
                    Some(a)
                } else {
                    None
                }
            });
            let Some(at) = partner.and_then(|p| {
                // Other members move in this tick too: aim at where they will be.
                self.m
                    .iter()
                    .find(|m| m.id == p)
                    .map(|m| m.pos)
                    .or_else(|| self.unit(p).map(|u| u.pos))
            }) else {
                continue;
            };
            if self.alive(k) && combat_core::distance(at, self.m[k].pos) > range {
                self.walk(k, at);
                busy.push(k);
            }
        }
        busy
    }
}

struct RaidRun {
    bosses: u32,
    secs: u64,
    low: u32,
    deaths: u32,
}

/// Level-10 members (`keep` of tank, DPS, healer) in the best Legacy gear walk the four raid
/// rooms in order on the difficulty `async_mode`, packs one by one, boss last; the dead are
/// released and everyone rests between rooms. Stops at the first wipe.
fn raid_run(keep: &[Role], async_mode: bool, seed: u64) -> RaidRun {
    let mut g = Group::new(content(), "campus", 10, seed);
    for k in (0..g.m.len()).rev() {
        if !keep.contains(&g.m[k].role) {
            let id = g.m[k].id;
            assert!(g.sim.remove_player(id));
            g.m.remove(k);
        }
    }
    g.gear_up_legacy();
    g.sim.debug_force_raid_async(Some(async_mode));
    let start = g.sim.now().0;
    let mut bosses = 0;
    let mut low = 100;
    g.in_boss = true;
    for zone in RAID {
        if let Err(e) = g.sim.load_zone(zone, &anchor, Vec::new()) {
            panic!("load_zone {zone}: {e}");
        }
        assert_eq!(g.sim.raid_view().async_mode, async_mode);
        for k in 0..g.m.len() {
            if let Some(u) = g.unit(g.m[k].id) {
                g.m[k].pos = u.pos;
            }
        }
        g.rest();
        g.boss_min_pct = 100;
        let t0 = g.sim.now().0;
        let (_, cleared, _) = g.raid_clear(zone);
        low = low.min(g.boss_min_pct);
        println!(
            "  {zone}: cleared {cleared} in {} s, low {}%, most dead {}",
            (g.sim.now().0 - t0) / 30,
            g.boss_min_pct,
            g.deaths
        );
        if !cleared {
            break;
        }
        bosses += 1;
        let dead: Vec<usize> = (0..g.m.len()).filter(|&k| !g.alive(k)).collect();
        for k in dead {
            let id = g.m[k].id;
            g.sim.apply_as(id, Command::RespawnPlayer);
        }
        g.tick();
    }
    RaidRun {
        bosses,
        secs: (g.sim.now().0 - start) / 30,
        low,
        deaths: g.deaths,
    }
}

impl Group {
    fn raid_step(&mut self) {
        let busy = self.mind_tethers();
        let focus = self.raid_focus();
        for k in 0..self.m.len() {
            if busy.contains(&k) || !self.alive(k) {
                continue;
            }
            if self.unit(self.m[k].id).is_some_and(|u| u.stunned) || self.dodge(k) {
                continue;
            }
            let Some(t) = focus.clone() else { continue };
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
        self.tick();
    }

    /// `focus`, except that a shielded Synergy Twin is never the target while its partner is
    /// open (linked HP: hitting the open one is the whole fight).
    fn raid_focus(&self) -> Option<UnitState> {
        let f = self.focus()?;
        if !f.auras.iter().any(|a| a.key == "synergy_shield") {
            return Some(f);
        }
        self.enemies()
            .into_iter()
            .find(|u| u.boss && !u.auras.iter().any(|a| a.key == "synergy_shield"))
            .or(Some(f))
    }

    fn raid_fight(&mut self) {
        let start = self.sim.now().0;
        while self.sim.now().0 - start < 120 || self.in_combat() {
            if self.wiped || self.sim.now().0 > MAX_TICKS * 3 {
                return;
            }
            self.raid_step();
        }
    }

    fn raid_pull(&mut self, at: Vec3) {
        self.front = at;
        let solo = self.m.len() == 1;
        for k in 0..self.m.len() {
            let back = match self.m[k].role {
                Role::Tank => 4.0,
                _ if solo => 5.0,
                _ => 22.0,
            };
            self.m[k].pos = Vec3::new(at.x + k as f32 * 1.5, 0.0, at.z + back);
        }
        self.tick();
        self.raid_fight();
    }

    /// `clear` with the raid's step: packs, then the boss; cleared = the room's boss died.
    fn raid_clear(&mut self, zone: &str) -> (u32, bool, u32) {
        let packs = self.sim.content().zone(zone).map_or(0, |z| z.packs.len());
        for n in 1..=packs {
            if let Some((at, _)) = anchor(&format!("pack_{n}")) {
                self.raid_pull(at);
                if self.wiped {
                    return (0, false, self.deaths);
                }
                self.rest();
            }
        }
        if let Some((at, _)) = anchor("boss") {
            self.raid_pull(at);
        }
        let boss = self
            .sim
            .content()
            .zone(zone)
            .and_then(|z| z.boss.as_ref())
            .map(|b| b.unit.clone())
            .unwrap_or_default();
        let dead = self.sim.raid_locked(&boss, self.sim.raid_async());
        (self.boss_min_pct, dead && !self.wiped, self.deaths)
    }
}

/// The E3 balance gate (docs/FORMULAS.md, Raid): three level-10 bots in Legacy gear clear all
/// four encounters on Normal; one bot clears Async Attendance; one bot on Normal either fails or
/// only scrapes through (below 25% HP at some point).
#[test]
fn the_raid_takes_a_group_and_async_attendance_takes_one() {
    let mut failures = Vec::new();
    let trio = [Role::Tank, Role::Dps, Role::Healer];
    for seed in [7u64, 42, 1234] {
        let g = raid_run(&trio, false, seed);
        let a = raid_run(&[Role::Dps], true, seed);
        let s = raid_run(&[Role::Dps], false, seed);
        println!(
            "Raid seed {seed}: trio Normal {}/4 bosses in {} s (low {}%, most dead {}); solo Async {}/4 in {} s (low {}%, died {}); solo Normal {}/4 in {} s (low {}%, died {})",
            g.bosses,
            g.secs,
            g.low,
            g.deaths,
            a.bosses,
            a.secs,
            a.low,
            a.deaths,
            s.bosses,
            s.secs,
            s.low,
            s.deaths
        );
        if g.bosses < 4 {
            failures.push(format!(
                "seed {seed}: the trio stopped after {} bosses",
                g.bosses
            ));
        }
        if a.bosses < 4 {
            failures.push(format!(
                "seed {seed}: solo Async stopped after {} bosses",
                a.bosses
            ));
        }
        if s.bosses == 4 && s.low >= 25 {
            failures.push(format!(
                "seed {seed}: solo Normal cleared it at {}% lowest",
                s.low
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
