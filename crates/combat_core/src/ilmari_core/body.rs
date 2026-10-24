//! P3 of the core, "The Body" (docs/ILMARI_1_0.md D.5, D12): Ilmari builds itself a mech out of
//! the office. The live screen is the body; two Rack Arms stand beside it with a death pact
//! (`PactDef`), a kit of telegraph casts runs on timers, a head appears for a few seconds each
//! minute to cast Quarterly Planning (a kick stops it, otherwise the next casts of the kit all
//! land at once), the stance of P2 adds its own overlay and the Judgment rider locks one member's
//! bar to one spell. The spells are content; this is the state that outlives a cast.

use super::{CoreRt, RemixCast, Stance, Voice};
use crate::content::{Content, SpellId, ticks};
use crate::geom::Vec3;
use crate::sim::Sim;
use crate::targeting::EntityId;
use serde::Deserialize;

fn yes() -> bool {
    true
}

/// One recurring cast of the mech (or, in P4, of Ilmari at the mainframe).
#[derive(Clone, Debug, Deserialize)]
pub struct KitCast {
    pub cast: RemixCast,
    /// After the phase begins.
    pub first_ms: u32,
    pub every_ms: u32,
    /// Skipped while the live screen wears this aura (the cable whip, lost to a double arm kill).
    #[serde(default)]
    pub skip_aura: Option<String>,
    /// Quarterly Planning's volley may pick it (summons and auras stay out of it).
    #[serde(default = "yes")]
    pub volley: bool,
}

/// The Judgment rider of P3: every `every_ms` one member may use one spell only for `lock_ms`.
#[derive(Clone, Debug, Deserialize)]
pub struct HotTakeDef {
    pub first_ms: u32,
    pub every_ms: u32,
    pub lock_ms: u32,
    /// Marker aura on the member (the spell itself is kept by the sim).
    pub aura: String,
    pub yells: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BodyDef {
    /// The Rack Arm unit (it carries the `death_pact`), and how far each stands from the screen.
    pub arm: String,
    pub arm_gap_m: f32,
    pub assemble_yells: Vec<String>,
    pub kit: Vec<KitCast>,
    /// The stance overlay, in `Stance::ALL` order (Hedged adds nothing).
    pub stance_kit: Vec<Vec<KitCast>>,
    /// Confused: every `every_ms` of the whole kit is this percent of what it was.
    pub confused_every_pct: u32,
    /// The head: its unit, the planning spell (and Story's), when the first one starts and how
    /// long a cycle is, how soon a kicked one comes again, and how long the head stays.
    pub head: String,
    pub plan_spell: SpellId,
    pub story_plan_spell: SpellId,
    pub plan_first_ms: u32,
    pub plan_ms: u32,
    pub kick_delay_ms: u32,
    pub head_ms: u32,
    /// Casts the volley lets go of at once; Story splits them in two, `story_split_ms` apart.
    pub volley: u8,
    pub story_split_ms: u32,
    pub plan_yells: Vec<String>,
    pub kicked_yells: Vec<String>,
    pub volley_yells: Vec<String>,
    pub hot_take: HotTakeDef,
    /// Units and auras that go when P3 ends (the arms, the head, the turrets, the cables, the
    /// offers and curses on the party).
    pub clears: Vec<String>,
    pub clear_auras: Vec<String>,
}

impl BodyDef {
    pub(super) fn validate(&self, c: &Content) -> bool {
        let kits = || self.kit.iter().chain(self.stance_kit.iter().flatten());
        let spells = kits()
            .flat_map(|k| std::iter::once(k.cast.spell).chain(k.cast.story))
            .chain([self.plan_spell, self.story_plan_spell]);
        let auras = kits()
            .filter_map(|k| k.skip_aura.as_ref())
            .chain(self.clear_auras.iter())
            .chain(std::iter::once(&self.hot_take.aura));
        let units = [&self.arm, &self.head]
            .into_iter()
            .chain(self.clears.iter());
        spells.into_iter().all(|s| c.spell(s).is_some())
            && auras.into_iter().all(|a| c.aura(a).is_some())
            && units.into_iter().all(|u| c.unit_def(u).is_some())
            && c.unit_def(&self.arm)
                .is_some_and(|u| u.death_pact.is_some())
            && self.stance_kit.len() == Stance::ALL.len()
            && kits().all(|k| k.every_ms > 0)
            && self.plan_ms > 0
            && self.kick_delay_ms > 0
            && self.head_ms > 0
            && self.volley > 0
            && self.hot_take.every_ms > 0
            && self.confused_every_pct > 0
            && !self.assemble_yells.is_empty()
    }
}

/// The head's cast that is under way.
#[derive(Clone)]
struct PlanRt {
    head: EntityId,
    start: u64,
    /// The tick the cast completes at, and the one the head leaves at.
    end: u64,
    gone: u64,
    done: bool,
}

pub(super) struct P3Rt {
    /// The kit and the tick each entry is next due.
    kit: Vec<(KitCast, u64)>,
    plan_at: u64,
    plan: Option<PlanRt>,
    /// Story's second half of a volley.
    late: Vec<(u64, RemixCast)>,
    hot_at: u64,
}

impl Sim {
    /// Phase 3 begins: the stance's kit joins the body's, the arms step out beside the screen.
    pub(super) fn core_enter_p3(&mut self, rt: &mut CoreRt) {
        let cfg = rt.cfg.clone();
        let b = &cfg.body;
        let (now, story) = (self.now, self.raid.story);
        let confused = rt.stances.contains(&(Stance::Confused as u8));
        let mut entries = b.kit.clone();
        for s in &rt.stances {
            if let Some(extra) = b.stance_kit.get(usize::from(*s).saturating_sub(1)) {
                entries.extend(extra.iter().cloned());
            }
        }
        let kit = entries
            .into_iter()
            .filter(|k| !(story && k.cast.full_only))
            .map(|mut k| {
                if confused {
                    k.every_ms = k.every_ms * b.confused_every_pct / 100;
                }
                let due = now + ticks(k.first_ms);
                (k, due)
            })
            .collect();
        if let Some(li) = self.core_live(rt) {
            let at = self.units[li].state.pos;
            let c = self.core_centre(rt);
            let (dx, dz) = (c.x - at.x, c.z - at.z);
            let d = (dx * dx + dz * dz).sqrt().max(0.001);
            let (fx, fz) = (dx / d, dz / d);
            for side in [-1.0_f32, 1.0] {
                let p = Vec3::new(
                    at.x + fx * 4.0 - fz * side * b.arm_gap_m,
                    at.y,
                    at.z + fz * 4.0 + fx * side * b.arm_gap_m,
                );
                self.summon_with(li, p, &b.arm, (1, 2, 0), false);
            }
        }
        rt.p3 = Some(P3Rt {
            kit,
            plan_at: now + ticks(b.plan_first_ms),
            plan: None,
            late: Vec::new(),
            hot_at: now + ticks(b.hot_take.first_ms),
        });
        self.core_say_any(rt, Voice::Screen, &b.assemble_yells, 16_000);
    }

    /// The kit's due casts, from the live screen; a cast-bar spell waits for a busy screen.
    pub(super) fn core_run_kit(&mut self, rt: &CoreRt, kit: &mut [(KitCast, u64)]) {
        let now = self.now;
        let Some(live) = self.core_live(rt) else {
            return;
        };
        for (k, due) in kit.iter_mut() {
            if *due > now {
                continue;
            }
            if k.skip_aura.as_ref().is_some_and(|a| self.has_aura(live, a)) {
                *due = now + ticks(k.every_ms);
                continue;
            }
            *due = if self.core_cast(rt, &k.cast) {
                now + ticks(k.every_ms)
            } else {
                now + 15
            };
        }
    }

    pub(super) fn core_rider(&self, rt: &CoreRt) -> bool {
        rt.cfg
            .rider
            .as_deref()
            .is_some_and(|r| self.index(rt.boss).is_some_and(|b| self.has_aura(b, r)))
    }

    pub(super) fn core_p3_step(&mut self, rt: &mut CoreRt) {
        let Some(mut p3) = rt.p3.take() else { return };
        let cfg = rt.cfg.clone();
        let b = &cfg.body;
        let now = self.now;
        self.core.hot_take.retain(|h| h.2 > now);
        let Some(li) = self.core_live(rt) else {
            rt.p3 = Some(p3);
            return;
        };
        let live_id = Self::id_of(li);
        self.p3_planning(rt, &mut p3, li);
        // Story's second half of a volley.
        let (due, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut p3.late)
            .into_iter()
            .partition(|(at, _)| *at <= now);
        p3.late = wait;
        for (_, c) in due {
            self.core_cast_as(rt, &c, true);
        }
        self.core_run_kit(rt, &mut p3.kit);
        // The Judgment rider: Hot Take.
        if self.core_rider(rt) && now >= p3.hot_at {
            p3.hot_at = now + ticks(b.hot_take.every_ms);
            let ps = self.core_players();
            if !ps.is_empty() {
                let who = ps[self.rng.below(ps.len() as u32) as usize];
                let bar: Vec<SpellId> = self
                    .index(who)
                    .and_then(|wi| self.class_at(wi))
                    .map(|c| c.action_bar.iter().copied().take(3).collect())
                    .unwrap_or_default();
                if let (Some(wi), false) = (self.index(who), bar.is_empty()) {
                    let spell = bar[self.rng.below(bar.len() as u32) as usize];
                    self.core
                        .hot_take
                        .push((who, spell, now + ticks(b.hot_take.lock_ms)));
                    self.apply_aura(
                        wi,
                        &b.hot_take.aura,
                        b.hot_take.lock_ms,
                        None,
                        live_id,
                        None,
                    );
                    self.core_say_any(rt, Voice::Screen, &b.hot_take.yells, 0);
                }
            }
        }
        rt.p3 = Some(p3);
    }

    /// Quarterly Planning: the head comes up, casts, and is kicked or lets the volley go.
    fn p3_planning(&mut self, rt: &mut CoreRt, p3: &mut P3Rt, li: usize) {
        let cfg = rt.cfg.clone();
        let b = &cfg.body;
        let now = self.now;
        let live_id = Self::id_of(li);
        match p3.plan.clone() {
            None if now >= p3.plan_at => {
                let at = self.units[li].state.pos;
                let c = self.core_centre(rt);
                let (dx, dz) = (c.x - at.x, c.z - at.z);
                let d = (dx * dx + dz * dz).sqrt().max(0.001);
                let spot = Vec3::new(at.x + dx / d * 5.0, at.y, at.z + dz / d * 5.0);
                self.summon_with(li, spot, &b.head, (1, 1, 0), false);
                let head = (0..self.units.len()).find(|&i| {
                    let u = &self.units[i];
                    u.summoner == Some(live_id)
                        && u.state.key == b.head
                        && u.state.alive
                        && !u.despawned
                });
                let id = if self.raid.story {
                    b.story_plan_spell
                } else {
                    b.plan_spell
                };
                match (head, self.content.spell(id).cloned()) {
                    (Some(hi), Some(spell)) => {
                        self.start_cast(hi, &spell, Some(Self::id_of(hi)));
                        let end = self.units[hi]
                            .cast
                            .as_ref()
                            .map_or(now + ticks(spell.cast_ms), |c| c.end);
                        p3.plan = Some(PlanRt {
                            head: Self::id_of(hi),
                            start: now,
                            end,
                            gone: now + ticks(b.head_ms),
                            done: false,
                        });
                        self.core_say_any(rt, Voice::Screen, &b.plan_yells, 0);
                    }
                    _ => p3.plan_at = now + ticks(b.plan_ms),
                }
            }
            Some(mut pl) => {
                let hi = self
                    .index(pl.head)
                    .filter(|&h| self.units[h].state.alive && !self.units[h].despawned);
                if !pl.done && hi.is_none_or(|h| self.units[h].cast.is_none()) {
                    pl.done = true;
                    if hi.is_some() && now + 2 >= pl.end {
                        p3.plan_at = pl.start + ticks(b.plan_ms);
                        self.p3_volley(rt, p3);
                    } else {
                        p3.plan_at = now + ticks(b.kick_delay_ms);
                        self.core_say_any(rt, Voice::Screen, &b.kicked_yells, 0);
                    }
                }
                if now >= pl.gone {
                    if let Some(h) = hi {
                        self.despawn(h);
                    }
                    p3.plan = None;
                } else {
                    p3.plan = Some(pl);
                }
            }
            None => {}
        }
    }

    /// The planning is done: the kit's next casts, the ones due soonest, all land together
    /// (Story lets half go now and half a few seconds later).
    fn p3_volley(&mut self, rt: &mut CoreRt, p3: &mut P3Rt) {
        let cfg = rt.cfg.clone();
        let b = &cfg.body;
        let now = self.now;
        let Some(live) = self.core_live(rt) else {
            return;
        };
        let mut order: Vec<usize> = (0..p3.kit.len())
            .filter(|&k| {
                let c = &p3.kit[k].0;
                c.volley
                    && !c.cast.cast
                    && !c.skip_aura.as_ref().is_some_and(|a| self.has_aura(live, a))
            })
            .collect();
        order.sort_by_key(|&k| (p3.kit[k].1, k));
        order.truncate(usize::from(b.volley));
        let now_n = if self.raid.story {
            order.len().div_ceil(2)
        } else {
            order.len()
        };
        for (j, &k) in order.iter().enumerate() {
            let c = p3.kit[k].0.clone();
            if j < now_n {
                self.core_cast_as(rt, &c.cast, true);
            } else {
                p3.late
                    .push((now + ticks(b.story_split_ms), c.cast.clone()));
            }
            p3.kit[k].1 = now + ticks(c.every_ms);
        }
        self.core_say_any(rt, Voice::Screen, &b.volley_yells, 1_500);
    }

    /// The arms, the head and everything else the mech brought go; so do the curses on the party.
    pub(super) fn core_clear_body(&mut self, rt: &CoreRt) {
        let b = &rt.cfg.body;
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.despawned || !u.state.alive {
                continue;
            }
            let of_us = u
                .summoner
                .is_some_and(|s| rt.screens.contains(&s) || s == rt.boss);
            if of_us && b.clears.contains(&u.state.key) {
                self.despawn(i);
            }
        }
        for p in self.players.clone() {
            if let Some(pi) = self.index(p) {
                for a in &b.clear_auras {
                    self.remove_aura(pi, a, crate::types::AuraEnd::Expired);
                }
            }
        }
        for &s in &rt.screens {
            if let Some(i) = self.index(s) {
                for a in &b.clear_auras {
                    self.remove_aura(i, a, crate::types::AuraEnd::Expired);
                }
            }
        }
        self.core.pacts.clear();
        self.core.hot_take.clear();
    }

    /// (seconds to the next Quarterly Planning while none is on, the Rack Arms standing).
    pub(super) fn body_wall(&self, rt: &CoreRt) -> (u8, u8) {
        let Some(p3) = rt.p3.as_ref().filter(|_| rt.phase == 3) else {
            return (0, 0);
        };
        let plan_s = if p3.plan.is_none() {
            super::secs_left(p3.plan_at, self.now)
        } else {
            0
        };
        let arms = self
            .units
            .iter()
            .filter(|u| u.state.alive && !u.despawned && u.state.key == rt.cfg.body.arm)
            .count()
            .min(255) as u8;
        (plan_s, arms)
    }

    /// Whether a Hot Take keeps `player` from casting `spell` right now.
    pub(crate) fn core_hot_take_blocks(&self, player: EntityId, spell: SpellId) -> bool {
        self.core
            .hot_take
            .iter()
            .any(|(p, only, until)| *p == player && *only != spell && *until > self.now)
    }

    /// The one spell a Hot Take leaves `player` (the client names it on the bar, D17).
    pub fn hot_take_of(&self, player: EntityId) -> Option<SpellId> {
        self.core
            .hot_take
            .iter()
            .find(|(p, _, until)| *p == player && *until > self.now)
            .map(|(_, s, _)| *s)
    }
}
