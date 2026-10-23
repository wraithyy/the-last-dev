//! The action recorder (docs/ILMARI_10.md C.3.3, Model Collapse). While an encounter arms it,
//! every real player's casting is logged in a bounded ring and played back by the boss a long
//! delay later as one of six `EchoKind`s. Repeating yourself empowers the echoes, varying your
//! rotation starves them, by an integer entropy formula over the last seconds of the log.
//!
//! Only group members record: the log filters on `Sim::players`, so companions, pets, mirrors and
//! summons never feed it. The log is a pure function of the command stream and the replay is
//! scheduled from it by tick, so there is no new rng and a rerun is bit-identical. The log is
//! never saved and never sent; the client only sees `Event::Echoed` for the ghost.

use crate::content::{Effect, SpellDef, SpellId, ticks};
use crate::geom::Vec3;
use crate::sim::{Hit, Sim};
use crate::targeting::EntityId;
use crate::time::TICK_HZ;
use crate::types::Event;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

/// What a recorded cast becomes when the boss replays it, derived from the spell's first effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EchoKind {
    /// Damage: a circle on the recorded position, scaled by the recorded cast's power.
    Strike,
    /// Heal: the boss heals a sliver.
    Mend,
    /// Absorb: a boss shield.
    Ward,
    /// Interrupt or lockout: a silence around the recorded position.
    Hush,
    /// Summon: a small add.
    Call,
    /// Movement: the boss charges to that spot.
    Dash,
}

impl EchoKind {
    /// The echo a spell becomes, or `None` for casts that do not record (auras, buffs, utility,
    /// consumables: nothing a boss could meaningfully repeat).
    pub fn of(spell: &SpellDef) -> Option<EchoKind> {
        Some(match spell.effect {
            Effect::Damage { .. } => EchoKind::Strike,
            Effect::Heal { .. } | Effect::RestoreHp { .. } => EchoKind::Mend,
            Effect::Absorb { .. } => EchoKind::Ward,
            Effect::Interrupt { .. } | Effect::Lockout { .. } => EchoKind::Hush,
            Effect::Summon { .. } => EchoKind::Call,
            Effect::Charge { .. } | Effect::Teleport { .. } => EchoKind::Dash,
            _ => return None,
        })
    }
}

/// One recorded cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionEntry {
    pub tick: u64,
    pub player: EntityId,
    pub spell: SpellId,
    /// Position relative to the arena centre, centimetres.
    pub rel_x_cm: i32,
    pub rel_z_cm: i32,
    /// Yaw in 1/256 turns.
    pub facing: u8,
    pub kind: EchoKind,
    /// Damage of the cast for `Strike` (the caster's own power at the time), else 0.
    pub power: u32,
}

/// Bounded ring of casts, oldest dropped first.
#[derive(Default)]
pub struct ActionLog {
    entries: VecDeque<ActionEntry>,
}

impl ActionLog {
    /// Entries kept in total.
    pub const CAP_TOTAL: usize = 512;
    /// Entries kept per player inside `PER_PLAYER_WINDOW_TICKS`.
    pub const CAP_PER_PLAYER: usize = 64;
    /// 30 s.
    pub const PER_PLAYER_WINDOW_TICKS: u64 = 30 * TICK_HZ as u64;

    pub fn push(&mut self, e: ActionEntry) {
        let mine = |x: &ActionEntry| {
            x.player == e.player && x.tick + Self::PER_PLAYER_WINDOW_TICKS > e.tick
        };
        if self.entries.iter().filter(|x| mine(x)).count() >= Self::CAP_PER_PLAYER
            && let Some(at) = self.entries.iter().position(mine)
        {
            self.entries.remove(at);
        }
        if self.entries.len() >= Self::CAP_TOTAL {
            self.entries.pop_front();
        }
        self.entries.push_back(e);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ActionEntry> {
        self.entries.iter()
    }
}

/// `c * ln(c)` in millionths for `c` in `0..=32`: with it the entropy of a window of up to 32
/// casts is pure integer arithmetic, so native and web agree to the last digit. (The 33 entries
/// are the spec's fixed table; indexing by count instead of by proportion keeps the error under
/// one per mille without interpolation.)
const CLNC: [i64; 33] = [
    0, 0, 1386294, 3295837, 5545177, 8047190, 10750557, 13621371, 16635532, 19775021, 23025851,
    26376848, 29818880, 33344342, 36946803, 40620753, 44361420, 48164627, 52026692, 55944341,
    59914645, 63934971, 68002934, 72116367, 76273292, 80471896, 84710510, 88987595, 93301726,
    97651579, 102035921, 106453603, 110903549,
];
/// `ln(k)` in millionths for `k` in `2..=8` (indices 0 and 1 unused).
const LNK: [i64; 9] = [
    0, 0, 693147, 1098612, 1386294, 1609438, 1791759, 1945910, 2079442,
];
/// Casts counted in a window: the most recent ones, so the table is enough.
pub const WINDOW_CASTS: usize = 32;
/// Fewer casts than this and the window says nothing (`H` reads 1000).
pub const MIN_CASTS: usize = 6;

/// Normalised Shannon entropy of a window, per mille: `counts` are the casts per distinct spell,
/// `kit` the number of distinct spells the group could be casting (clamped to 2..=8).
pub fn entropy_pm(counts: &[u8], kit: u8) -> u16 {
    let n: usize = counts.iter().map(|&c| usize::from(c)).sum();
    if !(MIN_CASTS..=WINDOW_CASTS).contains(&n) {
        return 1000;
    }
    let sum: i64 = counts.iter().map(|&c| CLNC[usize::from(c)]).sum();
    // (n ln n - sum c ln c) / n is the entropy in nats; divide by ln K.
    let nats_x_n = CLNC[n] - sum;
    let den = n as i64 * LNK[usize::from(kit.clamp(2, 8))];
    ((nats_x_n * 1000 + den / 2) / den).clamp(0, 1000) as u16
}

/// Per-encounter rules, set when the recorder is armed.
#[derive(Clone, Debug)]
pub struct EchoCfg {
    /// The boss that replays, takes the auras and the Underfit damage bonus.
    pub boss: EntityId,
    /// Arena centre, centimetres: logged positions are relative to it.
    pub center_x_cm: i32,
    pub center_z_cm: i32,
    pub story: bool,
    /// Unit an `EchoKind::Call` spawns; `None` skips those echoes.
    pub add_unit: Option<String>,
}

/// What the HUD ("Training loss") and the bot tests read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EchoStatus {
    pub casts: u8,
    pub distinct: u8,
    /// Normalised entropy per mille (1000 = fully varied).
    pub h_pm: u16,
    /// `1000 - H`.
    pub e_pm: u16,
    pub empowered: bool,
    pub underfit: bool,
}

struct Pending {
    due: u64,
    ghost_at: u64,
    announced: bool,
    entry: ActionEntry,
}

#[derive(Default)]
pub(crate) struct EchoState {
    cfg: Option<EchoCfg>,
    log: ActionLog,
    /// Recorded this tick, scheduled in `(tick, player)` order at the end of it.
    staged: Vec<ActionEntry>,
    pending: Vec<Pending>,
    /// Echoes scheduled per replay second, for the cap.
    per_sec: BTreeMap<u64, u8>,
    kit: u8,
    status: Option<EchoStatus>,
    starve_since: Option<u64>,
    underfit_until: u64,
}

/// Replay delay and ghost telegraph, ms.
fn timing(story: bool) -> (u32, u32) {
    if story {
        (45_000, 5_000)
    } else {
        (60_000, 3_000)
    }
}
/// Window the entropy reads, ms.
fn window_ms(story: bool) -> u32 {
    if story { 45_000 } else { 30_000 }
}
/// Echoes replayed per second at most.
const MAX_PER_SEC: u8 = 12;
/// `E >= 500`: the party repeats itself.
const EMPOWER_E: u16 = 500;
/// `H` at or above this for `STARVE_MS` running: the boss starves.
fn starve_h(story: bool) -> u16 {
    if story { 650 } else { 800 }
}
const STARVE_MS: u32 = 12_000;
/// How long Underfit lasts, ms.
const UNDERFIT_MS: u32 = 10_000;
/// Radius of a `Strike` and a `Hush`, metres.
const ECHO_RADIUS_M: f32 = 3.0;
/// `Mend` heals and `Ward` absorbs this share of the boss's max HP, per mille.
const MEND_PM: u64 = 10;
const WARD_PM: u64 = 20;
const OVERFIT_STEP_PM: u64 = 150;

impl Sim {
    /// Arms the recorder for an encounter; clears the log. The distinct-spell count `K` comes
    /// from the group's action bars.
    pub fn echo_start(&mut self, cfg: EchoCfg) {
        let mut seen: Vec<SpellId> = Vec::new();
        for &p in &self.players {
            let Some(i) = self.index(p) else { continue };
            for &s in self.content.bar_for_unit(&self.units[i].state.key) {
                let recordable = self
                    .content
                    .spell(s)
                    .is_some_and(|d| EchoKind::of(d).is_some());
                if recordable && !seen.contains(&s) {
                    seen.push(s);
                }
            }
        }
        self.echo = EchoState {
            kit: seen.len().clamp(2, 8) as u8,
            cfg: Some(cfg),
            ..Default::default()
        };
    }

    /// Disarms and forgets everything (a wipe, the boss's death, an encounter reset or a zone
    /// change).
    pub fn echo_stop(&mut self) {
        self.echo = EchoState::default();
    }

    pub fn echo_log(&self) -> &ActionLog {
        &self.echo.log
    }

    pub fn echo_status(&self) -> Option<EchoStatus> {
        self.echo.status
    }

    /// While true the boss may not start a new echo wave (content gate for the Starved window).
    pub fn echo_underfit(&self) -> bool {
        self.echo.cfg.is_some() && self.now < self.echo.underfit_until
    }

    /// Echoes waiting to land (staged ones included).
    pub fn echo_pending(&self) -> usize {
        self.echo.pending.len() + self.echo.staged.len()
    }

    /// Hook of `resolve`: a finished cast. Only group members record, only while armed.
    pub(crate) fn echo_record(&mut self, ci: usize, spell: &SpellDef) {
        let id = Self::id_of(ci);
        let Some(cfg) = self.echo.cfg.as_ref() else {
            return;
        };
        if !self.is_player(id) {
            return;
        }
        let Some(kind) = EchoKind::of(spell) else {
            return;
        };
        let power = match spell.effect {
            Effect::Damage {
                base,
                coefficient_pct,
                ..
            } => self
                .power(ci, base, coefficient_pct)
                .min(u64::from(u32::MAX)) as u32,
            _ => 0,
        };
        let s = &self.units[ci].state;
        let (cx, cz) = (cfg.center_x_cm, cfg.center_z_cm);
        let turns = (f64::from(s.yaw) / std::f64::consts::TAU * 256.0).floor() as i64;
        self.echo.staged.push(ActionEntry {
            tick: self.now,
            player: id,
            spell: spell.id,
            rel_x_cm: (s.pos.x * 100.0).round() as i32 - cx,
            rel_z_cm: (s.pos.z * 100.0).round() as i32 - cz,
            facing: turns.rem_euclid(256) as u8,
            kind,
            power,
        });
    }

    pub(crate) fn update_echo(&mut self) {
        let Some(cfg) = self.echo.cfg.clone() else {
            return;
        };
        self.echo_stage(&cfg);
        if self.now.is_multiple_of(u64::from(TICK_HZ)) {
            self.echo_evaluate(&cfg);
        }
        self.echo_replay(&cfg);
    }

    /// Moves this tick's casts into the log and the replay schedule, in `(tick, player)` order
    /// and under the per-second cap, so which echoes are dropped never depends on command order.
    fn echo_stage(&mut self, cfg: &EchoCfg) {
        if self.echo.staged.is_empty() {
            return;
        }
        let mut staged = std::mem::take(&mut self.echo.staged);
        staged.sort_by_key(|e| (e.tick, e.player));
        let (delay, tele) = timing(cfg.story);
        for e in staged {
            self.echo.log.push(e);
            // Story: no silences and no adds.
            if cfg.story && matches!(e.kind, EchoKind::Hush | EchoKind::Call) {
                continue;
            }
            let due = e.tick + ticks(delay);
            let n = self
                .echo
                .per_sec
                .entry(due / u64::from(TICK_HZ))
                .or_insert(0);
            if *n >= MAX_PER_SEC {
                continue;
            }
            *n += 1;
            self.echo.pending.push(Pending {
                due,
                ghost_at: due - ticks(tele),
                announced: false,
                entry: e,
            });
        }
    }

    /// Counts per distinct spell over the entropy window, and how often each one was cast.
    fn echo_window(&self, cfg: &EchoCfg) -> BTreeMap<SpellId, u8> {
        let floor = self.now.saturating_sub(ticks(window_ms(cfg.story)));
        let recent: Vec<&ActionEntry> = self.echo.log.iter().filter(|e| e.tick > floor).collect();
        let skip = recent.len().saturating_sub(WINDOW_CASTS);
        let mut counts = BTreeMap::new();
        for e in &recent[skip..] {
            *counts.entry(e.spell).or_insert(0u8) += 1;
        }
        counts
    }

    fn echo_evaluate(&mut self, cfg: &EchoCfg) {
        let counts = self.echo_window(cfg);
        let list: Vec<u8> = counts.values().copied().collect();
        let n: usize = list.iter().map(|&c| usize::from(c)).sum();
        let kit = self.echo.kit;
        let h = entropy_pm(&list, kit);
        let e = 1000 - h;
        let enough = n >= MIN_CASTS;
        let empowered = enough && e >= EMPOWER_E;
        // A small kit can still starve the boss: it needs min(5, K - 1) distinct spells.
        let varied = enough
            && h >= starve_h(cfg.story)
            && counts.len() >= usize::from(5.min(kit.saturating_sub(1)));
        self.echo.starve_since = match (varied, self.echo.starve_since) {
            (true, since) => Some(since.unwrap_or(self.now)),
            (false, _) => None,
        };
        let boss = self.index(cfg.boss);
        if let Some(since) = self.echo.starve_since
            && self.now - since >= ticks(STARVE_MS)
        {
            self.echo.starve_since = None;
            self.echo.underfit_until = self.now + ticks(UNDERFIT_MS);
            // Whatever was about to land is dropped: a free damage window.
            self.echo.pending.clear();
            if let Some(b) = boss {
                self.apply_aura(b, "il_underfit", UNDERFIT_MS, None, cfg.boss, None);
            }
        }
        if empowered && let Some(b) = boss {
            // One stack per second the party keeps repeating itself; the aura runs out after.
            self.apply_aura(b, "il_overfitting", 4000, None, cfg.boss, None);
        }
        self.echo.status = Some(EchoStatus {
            casts: n.min(255) as u8,
            distinct: counts.len() as u8,
            h_pm: h,
            e_pm: e,
            empowered,
            underfit: self.now < self.echo.underfit_until,
        });
    }

    fn echo_replay(&mut self, cfg: &EchoCfg) {
        if self.echo.pending.is_empty() {
            return;
        }
        let now = self.now;
        let (_, tele) = timing(cfg.story);
        let mut landing: Vec<ActionEntry> = Vec::new();
        for p in &mut self.echo.pending {
            if !p.announced && now >= p.ghost_at {
                p.announced = true;
                let e = p.entry;
                self.events.push(Event::Echoed {
                    as_player: e.player,
                    kind: e.kind,
                    at_ms: tele,
                    at_cm: [cfg.center_x_cm + e.rel_x_cm, cfg.center_z_cm + e.rel_z_cm],
                });
            }
            if now >= p.due {
                landing.push(p.entry);
            }
        }
        self.echo.pending.retain(|p| p.due > now);
        // A pending queue can outlive the boss (a wipe cleared nothing yet): echoes only land
        // while it is alive.
        if self
            .index(cfg.boss)
            .is_none_or(|b| !self.units[b].state.alive)
        {
            return;
        }
        landing.sort_by_key(|e| (e.tick, e.player));
        let counts = self.echo_window(cfg);
        let status = self.echo.status;
        for e in landing {
            self.echo_land(cfg, &e, &counts, status);
        }
    }

    fn echo_land(
        &mut self,
        cfg: &EchoCfg,
        e: &ActionEntry,
        counts: &BTreeMap<SpellId, u8>,
        status: Option<EchoStatus>,
    ) {
        let Some(b) = self.index(cfg.boss) else {
            return;
        };
        let at = Vec3::new(
            (cfg.center_x_cm + e.rel_x_cm) as f32 / 100.0,
            self.units[b].state.pos.y,
            (cfg.center_z_cm + e.rel_z_cm) as f32 / 100.0,
        );
        match e.kind {
            EchoKind::Strike => {
                let mut pm: u64 = 1000;
                if let Some(s) = status.filter(|s| s.empowered) {
                    pm = pm * (1000 + 800 * u64::from(s.e_pm) / 1000) / 1000;
                    let seen = u64::from(counts.get(&e.spell).copied().unwrap_or(1));
                    let cap = if cfg.story { 1500 } else { 2500 };
                    let overfit = (1000 + OVERFIT_STEP_PM * seen.saturating_sub(1)).min(cap);
                    pm = pm * overfit / 1000;
                }
                let amount = (u64::from(e.power) * pm / 1000).min(u64::from(u32::MAX)) as u32;
                for i in self.echo_friendlies(at, ECHO_RADIUS_M) {
                    self.deal_damage(
                        cfg.boss,
                        Self::id_of(i),
                        Hit {
                            spell: Some(e.spell),
                            amount,
                            crit: false,
                            combo: false,
                            school: crate::content::School::Arcane,
                            dot: false,
                        },
                    );
                }
            }
            EchoKind::Mend => {
                let amount = u64::from(self.units[b].state.max_hp) * MEND_PM / 1000;
                self.heal(cfg.boss, b, Some(e.spell), amount as u32);
            }
            EchoKind::Ward => {
                let amount = (u64::from(self.units[b].state.max_hp) * WARD_PM / 1000) as u32;
                self.apply_aura(
                    b,
                    "il_echo_ward",
                    8000,
                    Some(amount),
                    cfg.boss,
                    Some(e.spell),
                );
            }
            EchoKind::Hush => {
                for i in self.echo_friendlies(at, ECHO_RADIUS_M * 2.0) {
                    self.apply_aura(i, "il_cold_outreach", 3000, None, cfg.boss, Some(e.spell));
                }
            }
            EchoKind::Call => {
                if let Some(unit) = cfg.add_unit.clone() {
                    let _ = self.spawn_unit(&unit, at, 0.0);
                }
            }
            EchoKind::Dash => {
                self.units[b].state.pos = at;
                self.events.push(Event::Teleported {
                    unit: cfg.boss,
                    pos: at,
                });
            }
        }
    }

    /// Living friendly units (players and companions both stand in the truth) near `at`.
    fn echo_friendlies(&self, at: Vec3, radius: f32) -> Vec<usize> {
        (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                !u.despawned
                    && u.state.alive
                    && !u.state.hostile
                    && crate::effects::dist_xz(u.state.pos, at) <= radius
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Content;

    fn entry(tick: u64, player: EntityId, spell: u16) -> ActionEntry {
        ActionEntry {
            tick,
            player,
            spell: SpellId(u32::from(spell)),
            rel_x_cm: 0,
            rel_z_cm: 0,
            facing: 0,
            kind: EchoKind::Strike,
            power: 100,
        }
    }

    /// A fullstack player standing on the arena centre and a far-away boss, recorder armed.
    fn armed(story: bool) -> (Sim, EntityId, EntityId) {
        let c = Content::from_ron(include_str!("../../../content/game.ron")).expect("content");
        let mut s = Sim::new(c, 3);
        s.set_wandering(false);
        let p = s.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
        let boss = s
            .spawn_unit("ceo", Vec3::new(0.0, 0.0, -90.0), 0.0)
            .expect("boss");
        s.echo_start(EchoCfg {
            boss,
            center_x_cm: 0,
            center_z_cm: 0,
            story,
            add_unit: None,
        });
        s.echo.kit = 6;
        (s, p, boss)
    }

    fn run(s: &mut Sim, secs: u32) -> Vec<Event> {
        (0..secs * TICK_HZ).flat_map(|_| s.tick()).collect()
    }

    /// One cast per second of `spell(second)` for `secs` seconds, then the sim moves on.
    fn feed(s: &mut Sim, p: EntityId, secs: u32, spell: impl Fn(u32) -> u16) -> Vec<Event> {
        let mut out = Vec::new();
        for t in 0..secs {
            let now = s.now + 1;
            s.echo.staged.push(entry(now, p, spell(t)));
            out.extend(run(s, 1));
        }
        out
    }

    fn f64_entropy(counts: &[u8], kit: u8) -> f64 {
        let n: f64 = counts.iter().map(|&c| f64::from(c)).sum();
        let h: f64 = counts
            .iter()
            .map(|&c| {
                let p = f64::from(c) / n;
                -p * p.ln()
            })
            .sum();
        h / f64::from(kit).ln() * 1000.0
    }

    #[test]
    fn entropy_golden_numbers() {
        assert_eq!(entropy_pm(&[10], 8), 0, "all one spell");
        assert_eq!(entropy_pm(&[2; 8], 8), 1000, "uniform over the kit");
        assert_eq!(entropy_pm(&[8, 2], 2), 722);
        assert_eq!(entropy_pm(&[3, 3, 3], 3), 1000);
        assert_eq!(entropy_pm(&[5], 8), 1000, "too few casts say nothing");
    }

    #[test]
    fn the_integer_table_tracks_f64_within_one_unit() {
        let mut checked = 0;
        // Every window of 6..=32 casts split over up to four spells, kits 2..=8.
        for n in 6u8..=32 {
            for a in 0..=n {
                for b in 0..=(n - a) {
                    for c in 0..=(n - a - b) {
                        let d = n - a - b - c;
                        let counts: Vec<u8> = [a, b, c, d].into_iter().filter(|&x| x > 0).collect();
                        for kit in [2u8, 4, 8] {
                            let want = f64_entropy(&counts, kit).clamp(0.0, 1000.0);
                            let got = f64::from(entropy_pm(&counts, kit));
                            assert!(
                                (got - want).abs() <= 1.0,
                                "{counts:?} k{kit}: {got} vs {want}"
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 10_000);
    }

    #[test]
    fn the_log_is_bounded_in_total_and_per_player() {
        let mut log = ActionLog::default();
        for t in 0..2000u64 {
            log.push(entry(t, 1 + t % 20, 1));
            assert!(log.len() <= ActionLog::CAP_TOTAL);
        }
        let mut log = ActionLog::default();
        for t in 0..300u64 {
            log.push(entry(t, 1, (t % 5) as u16));
        }
        // 300 ticks fit one 30 s window: one player keeps at most 64.
        assert_eq!(log.len(), ActionLog::CAP_PER_PLAYER);
        assert_eq!(
            log.iter().next().map(|e| e.tick),
            Some(236),
            "oldest dropped first"
        );
    }

    #[test]
    fn replay_comes_a_minute_later_with_a_three_second_ghost() {
        let (mut s, p, _) = armed(false);
        let out = feed(&mut s, p, 1, |_| 1);
        assert!(out.iter().all(|e| !matches!(e, Event::Echoed { .. })));
        let mut ghost_at = None;
        let mut hit_at = None;
        for t in 0..64 * TICK_HZ {
            for e in s.tick() {
                match e {
                    Event::Echoed { kind, at_ms, .. } => {
                        assert_eq!((kind, at_ms), (EchoKind::Strike, 3000));
                        ghost_at = Some(t);
                    }
                    Event::Damage {
                        spell: Some(SpellId(1)),
                        source,
                        ..
                    } if source != p => {
                        hit_at = hit_at.or(Some(t));
                    }
                    _ => {}
                }
            }
        }
        let (g, h) = (ghost_at.expect("ghost"), hit_at.expect("landed"));
        // Recorded at tick 1 (the staged entry took the next tick); delay 60 s, ghost 3 s before.
        assert!((56 * TICK_HZ..58 * TICK_HZ).contains(&g), "ghost {g}");
        assert!((59 * TICK_HZ..60 * TICK_HZ + 2).contains(&h), "hit {h}");
    }

    #[test]
    fn at_most_twelve_echoes_land_per_second_and_the_rest_drop_in_player_order() {
        let (mut s, p, _) = armed(false);
        for k in 0..30u64 {
            s.echo.staged.push(entry(1, p + k, 1));
        }
        s.tick();
        assert_eq!(s.echo.pending.len(), 12);
        let kept: Vec<_> = s.echo.pending.iter().map(|x| x.entry.player).collect();
        assert_eq!(kept, (0..12).map(|k| p + k).collect::<Vec<_>>());
    }

    #[test]
    fn the_recorder_ignores_everything_that_is_not_a_group_member() {
        let (mut s, p, boss) = armed(false);
        let spell = s.content.spell(SpellId(1)).expect("spell").clone();
        let (pi, bi) = (s.index(p).unwrap(), s.index(boss).unwrap());
        s.echo_record(bi, &spell);
        assert!(s.echo.staged.is_empty(), "a non-member is not recorded");
        s.echo_record(pi, &spell);
        assert_eq!(s.echo.staged.len(), 1);
        let mut idle = Sim::new(s.content.clone(), 1);
        let q = idle.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
        let qi = idle.index(q).unwrap();
        idle.echo_record(qi, &spell);
        assert!(
            idle.echo.staged.is_empty(),
            "nothing records until it is armed"
        );
    }

    #[test]
    fn a_repeating_party_empowers_the_echoes_a_varied_one_does_not() {
        let strike = |s: &mut Sim, p: EntityId, spells: &dyn Fn(u32) -> u16| {
            let mut out = feed(s, p, 40, spells);
            // Read right after the casting: the window empties once the casting stops.
            let status = s.echo_status().expect("status");
            out.extend(run(s, 30));
            let dmg = out
                .iter()
                .filter_map(|e| match e {
                    Event::Damage {
                        spell: Some(_),
                        source,
                        amount,
                        ..
                    } if *source != p => Some(*amount),
                    _ => None,
                })
                .collect::<Vec<u32>>();
            (dmg, status)
        };
        let (mut a, pa, boss) = armed(false);
        let (rep, st) = strike(&mut a, pa, &|_| 1);
        assert!(st.empowered && st.e_pm > 900, "{st:?}");
        let _ = boss;
        // Power x (1 + 0.8 E) x capped overfit (x2.5): 4.5 times the recorded power at E = 1000.
        let power = a.echo.log.iter().next().map(|e| e.power).unwrap_or(0);
        assert!(
            power == 0 || rep.is_empty() || rep.iter().all(|&x| x <= power * 5),
            "{rep:?}"
        );
        assert!(rep.iter().any(|&x| x >= 300), "{rep:?}");

        // Five spells in turn over a kit of six: entropy is high but not high enough to starve
        // (needs 12 s running), so the first stretch replays at baseline.
        let (mut b, pb, _) = armed(false);
        let (var, st) = strike(&mut b, pb, &|t| (t % 5) as u16 + 1);
        assert!(!st.empowered, "{st:?}");
        assert!(var.iter().all(|&x| x <= 100), "{var:?}");
    }

    #[test]
    fn starving_the_boss_opens_a_free_damage_window_and_closes_it() {
        let (mut s, p, boss) = armed(false);
        // Six spells in rotation over a kit of six: H = 1000 once 6 casts are in.
        let mut began = None;
        for t in 0..30u32 {
            feed(&mut s, p, 1, |_| (t % 6) as u16 + 1);
            if s.echo_underfit() && began.is_none() {
                began = Some(s.now);
                let bi = s.index(boss).unwrap();
                assert!(s.units[bi].auras.iter().any(|a| a.key == "il_underfit"));
            }
        }
        let began = began.expect("12 s of variety starved it");
        assert!(
            s.echo.pending.iter().all(|x| x.entry.tick >= began - 2),
            "echoes queued before the window were dropped"
        );
        // 10 s later the window is shut again.
        run(&mut s, 11);
        assert!(!s.echo_underfit());
    }

    #[test]
    fn story_drops_hush_and_call_and_caps_overfitting() {
        let (mut s, p, _) = armed(true);
        let mut e = entry(1, p, 1);
        e.kind = EchoKind::Hush;
        s.echo.staged.push(e);
        e.kind = EchoKind::Call;
        s.echo.staged.push(e);
        e.kind = EchoKind::Mend;
        s.echo.staged.push(e);
        s.tick();
        assert_eq!(s.echo.pending.len(), 1);
        assert_eq!(s.echo.log.len(), 3, "still logged, just never replayed");
        let (delay, tele) = timing(true);
        assert_eq!((delay, tele), (45_000, 5_000));
    }
}
