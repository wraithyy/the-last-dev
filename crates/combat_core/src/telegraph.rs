//! Ground telegraphs: a shape painted on the floor that lands after a delay and only hits
//! the units still inside it (or, with cover, the ones not hiding behind it).

use crate::content::{SpellDef, SpellId, TelegraphAt, ticks};
use crate::effects::dist_xz;
use crate::geom::{Vec3, forward, line_of_sight};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::time::ticks_to_ms;
use crate::types::{AiState, Event};
use serde::{Deserialize, Serialize};

/// Shapes lie on the ground plane; cones and lines open along `facing` (a yaw, see `geom::forward`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TelegraphShape {
    Circle {
        radius_m: f32,
    },
    /// `angle_deg` is the full opening angle.
    Cone {
        radius_m: f32,
        angle_deg: f32,
    },
    /// Starts at the telegraph position and runs `length_m` along the facing.
    Line {
        length_m: f32,
        width_m: f32,
    },
}

impl TelegraphShape {
    pub(crate) fn valid(&self) -> bool {
        let pos = |v: f32| v.is_finite() && v > 0.0;
        match *self {
            TelegraphShape::Circle { radius_m } => pos(radius_m),
            TelegraphShape::Cone {
                radius_m,
                angle_deg,
            } => pos(radius_m) && pos(angle_deg) && angle_deg <= 360.0,
            TelegraphShape::Line { length_m, width_m } => pos(length_m) && pos(width_m),
        }
    }

    /// Whether ground point `p` lies inside the shape placed at `pos` facing `facing`.
    pub fn contains(&self, pos: Vec3, facing: f32, p: Vec3) -> bool {
        let (dx, dz) = (p.x - pos.x, p.z - pos.z);
        let f = forward(facing);
        match *self {
            TelegraphShape::Circle { radius_m } => dx * dx + dz * dz <= radius_m * radius_m,
            TelegraphShape::Cone {
                radius_m,
                angle_deg,
            } => {
                let d = (dx * dx + dz * dz).sqrt();
                if d > radius_m {
                    return false;
                }
                // Standing on the apex counts as inside, like a melee cleave.
                d < 1e-4 || (f.x * dx + f.z * dz) / d >= (angle_deg.to_radians() / 2.0).cos()
            }
            TelegraphShape::Line { length_m, width_m } => {
                let along = f.x * dx + f.z * dz;
                let across = f.z * dx - f.x * dz;
                (0.0..=length_m).contains(&along) && across.abs() <= width_m / 2.0
            }
        }
    }
}

/// A telegraph waiting to land, as the client and bots see it.
#[derive(Clone, Debug, PartialEq)]
pub struct TelegraphView {
    pub id: u32,
    pub source: EntityId,
    pub spell: SpellId,
    pub shape: TelegraphShape,
    pub pos: Vec3,
    pub facing: f32,
    pub remaining_ms: u32,
    pub total_ms: u32,
    /// Shared damage: the units inside split it, so standing in it together is right.
    pub split: bool,
    /// Safe spots (radius `TelegraphDef::cover_radius_m`); empty when there is no cover.
    pub cover: Vec<Vec3>,
    pub cover_radius_m: f32,
}

impl TelegraphView {
    /// True when standing at `p` gets you hit when it lands (split telegraphs: you share it).
    pub fn hits(&self, p: Vec3) -> bool {
        self.shape.contains(self.pos, self.facing, p)
            && !self
                .cover
                .iter()
                .any(|c| dist_xz(*c, p) <= self.cover_radius_m)
    }
}

pub(crate) struct TelegraphRt {
    pub id: u32,
    pub source: EntityId,
    pub spell: SpellId,
    pub shape: TelegraphShape,
    pub pos: Vec3,
    pub facing: f32,
    pub start: u64,
    pub at: u64,
    pub split: bool,
    pub cover: Vec<Vec3>,
    pub cover_radius_m: f32,
    /// Hostility of the source: telegraphs hit the other side only.
    pub hostile: bool,
    /// Firings left after this one and the gap between them (`TelegraphDef::repeat`).
    pub again: u8,
    pub again_ms: u32,
}

/// A split telegraph nobody stood in hits everyone of the other side within this range.
const SPLIT_FALLBACK_M: f32 = 40.0;

impl Sim {
    /// Every telegraph that has not landed yet, oldest first.
    pub fn telegraphs(&self) -> Vec<TelegraphView> {
        self.telegraphs
            .iter()
            .map(|t| TelegraphView {
                id: t.id,
                source: t.source,
                spell: t.spell,
                shape: t.shape,
                pos: t.pos,
                facing: t.facing,
                remaining_ms: ticks_to_ms(t.at.saturating_sub(self.now)) as u32,
                total_ms: ticks_to_ms(t.at - t.start) as u32,
                split: t.split,
                cover: t.cover.clone(),
                cover_radius_m: t.cover_radius_m,
            })
            .collect()
    }

    /// Paints the spell's telegraph(s) instead of applying its effects now. Positions are
    /// fixed at release: dodging means walking out before it lands.
    pub(crate) fn start_telegraphs(&mut self, ci: usize, spell: &SpellDef, target: Option<usize>) {
        let Some(def) = spell.telegraph.as_ref() else {
            return;
        };
        let me = self.units[ci].state.pos;
        let hostile = self.units[ci].state.hostile;
        let (spawn, spawn_yaw) = (self.units[ci].spawn_pos, self.units[ci].spawn_yaw);
        let (sf, sr) = (forward(spawn_yaw), right(spawn_yaw));
        let anchors: Vec<Vec3> = match (def.anchor, def.at) {
            (Some((r, f)), _) => vec![Vec3::new(
                spawn.x + sr.x * r + sf.x * f,
                spawn.y,
                spawn.z + sr.z * r + sf.z * f,
            )],
            (None, TelegraphAt::Caster) => vec![me],
            (None, TelegraphAt::Target) => vec![target.map_or(me, |t| self.units[t].state.pos)],
            // Fighters only: a bystander NPC of the other side never gets a mark.
            (None, TelegraphAt::EachHostile) => (0..self.units.len())
                .filter(|&o| {
                    let u = &self.units[o];
                    u.state.alive
                        && !u.despawned
                        && u.state.hostile != hostile
                        && (u.state.ai.is_some() || self.is_player(u.state.id))
                        && u.state.ai != Some(AiState::Evade)
                        && dist_xz(u.state.pos, me) <= spell.range_m
                })
                .map(|o| self.units[o].state.pos)
                .collect(),
        };
        // Cones and lines start at the caster and open toward the target.
        let aim = target.map_or(me, |t| self.units[t].state.pos);
        let facing = if dist_xz(aim, me) > 1e-3 {
            crate::ai::yaw_toward(me, aim)
        } else {
            self.units[ci].state.yaw
        };
        let cover: Vec<Vec3> = def
            .cover
            .iter()
            .map(|(r, f)| {
                Vec3::new(
                    spawn.x + sr.x * r + sf.x * f,
                    spawn.y,
                    spawn.z + sr.z * r + sf.z * f,
                )
            })
            .collect();
        let side = right(facing);
        let source = Self::id_of(ci);
        for base in anchors {
            let origin = match (def.at, def.shape) {
                (
                    TelegraphAt::Target,
                    TelegraphShape::Cone { .. } | TelegraphShape::Line { .. },
                ) => me,
                _ => base,
            };
            let n = def.count.max(1);
            let gap = (def.gap && n >= 3).then(|| 1 + self.rng.below(u32::from(n) - 2) as u8);
            for k in 0..n {
                if Some(k) == gap {
                    continue;
                }
                let pos = if def.march {
                    let (f, d) = (forward(facing), def.step_m * f32::from(k + 1));
                    Vec3::new(me.x + f.x * d, me.y, me.z + f.z * d)
                } else {
                    let off = (f32::from(k) - f32::from(n - 1) / 2.0) * def.step_m;
                    Vec3::new(origin.x + side.x * off, origin.y, origin.z + side.z * off)
                };
                let delay = def.delay_ms + u32::from(k) * def.step_ms;
                let id = self.next_telegraph;
                self.next_telegraph = self.next_telegraph.wrapping_add(1);
                let at = self.now + ticks(delay);
                self.telegraphs.push(TelegraphRt {
                    id,
                    source,
                    spell: spell.id,
                    shape: def.shape,
                    pos,
                    facing,
                    start: self.now,
                    at,
                    split: def.split,
                    cover: cover.clone(),
                    cover_radius_m: def.cover_radius_m,
                    hostile,
                    again: def.repeat,
                    again_ms: def.repeat_ms,
                });
                self.events.push(Event::TelegraphStarted {
                    id,
                    source,
                    spell: spell.id,
                    shape: def.shape,
                    pos,
                    facing,
                    resolve_ms: ticks_to_ms(at - self.now) as u32,
                    cover: cover.clone(),
                });
            }
        }
    }

    /// Lands every due telegraph on the units inside it at this moment.
    pub(crate) fn update_telegraphs(&mut self) {
        let mut k = 0;
        while k < self.telegraphs.len() {
            if self.telegraphs[k].at > self.now {
                k += 1;
                continue;
            }
            let t = self.telegraphs.remove(k);
            self.land_telegraph(t);
        }
    }

    fn land_telegraph(&mut self, t: TelegraphRt) {
        let (Some(ci), Some(spell)) = (self.index(t.source), self.content.spell(t.spell).cloned())
        else {
            return;
        };
        let view = TelegraphView {
            id: t.id,
            source: t.source,
            spell: t.spell,
            shape: t.shape,
            pos: t.pos,
            facing: t.facing,
            remaining_ms: 0,
            total_ms: 0,
            split: t.split,
            cover: t.cover.clone(),
            cover_radius_m: t.cover_radius_m,
        };
        let has_cover = !t.cover.is_empty();
        let enemies: Vec<usize> = (0..self.units.len())
            .filter(|&o| {
                let u = &self.units[o];
                u.state.alive
                    && !u.despawned
                    && u.state.hostile != t.hostile
                    && u.state.ai != Some(AiState::Evade)
            })
            .collect();
        let mut hit: Vec<usize> = enemies
            .iter()
            .copied()
            .filter(|&o| {
                let p = self.units[o].state.pos;
                // With cover, a real wall between the source and you is as good as a desk.
                view.hits(p) && (!has_cover || line_of_sight(t.pos, p, &self.blockers))
            })
            .collect();
        let share = t.split && hit.len() > 1;
        let spell = if share {
            split_spell(&spell, hit.len() as u32)
        } else {
            spell
        };
        // Nobody showed up to the synergy session: everyone nearby gets it in full.
        if t.split && hit.is_empty() {
            hit = enemies
                .into_iter()
                .filter(|&o| dist_xz(self.units[o].state.pos, t.pos) <= SPLIT_FALLBACK_M)
                .collect();
        }
        self.events.push(Event::TelegraphResolved {
            id: t.id,
            landed: true,
        });
        // Standing in a stack on purpose is not getting hit.
        if !t.split {
            for &o in &hit {
                self.note_telegraph_hit(Self::id_of(o));
            }
        }
        for e in spell.effects() {
            if e.caster_centric() {
                self.apply_effect(ci, &spell, e, None);
            } else {
                for &o in &hit {
                    self.apply_effect(ci, &spell, e, Some(o));
                }
            }
        }
        // The next firing is painted at once: the floor shows where it will land.
        if t.again > 0 && self.units[ci].state.alive {
            let id = self.next_telegraph;
            self.next_telegraph = self.next_telegraph.wrapping_add(1);
            let at = self.now + ticks(t.again_ms);
            self.events.push(Event::TelegraphStarted {
                id,
                source: t.source,
                spell: t.spell,
                shape: t.shape,
                pos: t.pos,
                facing: t.facing,
                resolve_ms: t.again_ms,
                cover: t.cover.clone(),
            });
            self.telegraphs.push(TelegraphRt {
                id,
                start: self.now,
                at,
                again: t.again - 1,
                ..t
            });
        }
    }

    /// Drops the unit's pending telegraphs (evade, despawn): the encounter is over.
    pub(crate) fn cancel_telegraphs_of(&mut self, source: EntityId) {
        let mut gone = Vec::new();
        self.telegraphs.retain(|t| {
            let keep = t.source != source;
            if !keep {
                gone.push(t.id);
            }
            keep
        });
        for id in gone {
            self.events
                .push(Event::TelegraphResolved { id, landed: false });
        }
    }
}

/// Right-hand side of a yaw on the ground plane (yaw 0 faces -Z, so right is +X).
pub(crate) fn right(yaw: f32) -> Vec3 {
    Vec3::new(yaw.cos(), 0.0, -yaw.sin())
}

/// The same spell with its direct damage divided among `n` units.
fn split_spell(spell: &SpellDef, n: u32) -> SpellDef {
    let mut s = spell.clone();
    let div = |e: &mut crate::content::Effect| {
        if let crate::content::Effect::Damage {
            base,
            coefficient_pct,
            ..
        } = e
        {
            *base = (*base / n).max(1);
            *coefficient_pct /= n;
        }
    };
    div(&mut s.effect);
    s.extra_effects.iter_mut().for_each(div);
    s
}
