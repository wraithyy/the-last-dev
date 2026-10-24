//! Ilmari Wing 3, Training Data (docs/ILMARI_10.md C.4): the glue between the three engines of
//! `swarm.rs`, `illusion.rs` and `echo.rs` and the three bosses that use them. A boss with an
//! `EncounterDef` arms its machinery when it is pulled and tears it down when it resets or dies;
//! the mechanics themselves are ordinary spell effects (`SwarmWave`, `Confabulate`, `Blink`,
//! `Footnotes`) so the bosses stay data in `game.ron`. Everything here runs on sim state and the
//! combat rng: a replay reproduces it exactly.

use crate::content::{EncounterDef, YellKind};
use crate::echo::EchoCfg;
use crate::geom::Vec3;
use crate::illusion::IllusionArena;
use crate::sim::{Hit, Sim};
use crate::targeting::EntityId;
use crate::types::Event;
use crate::world::PROP_OK;

/// Drafts are re-sent about once a second, and only when something changed.
const DRAFT_SYNC_TICKS: u64 = 30;

#[derive(Default)]
pub(crate) struct Wing3State {
    /// The boss whose encounter is armed.
    pub(crate) boss: Option<EntityId>,
    /// Prop keys of the drafts, in `EncounterDef::Drafts::props` order.
    draft_props: Vec<String>,
    lost: u8,
    /// What was last sent, to skip identical frames.
    sent: Vec<(u16, bool)>,
    next_sync: u64,
}

impl Sim {
    /// Arms a pulled Wing 3 boss and disarms one that reset or died.
    pub(crate) fn update_wing3(&mut self) {
        let fighting = (0..self.units.len()).find(|&i| {
            let u = &self.units[i];
            !u.despawned
                && u.state.alive
                && u.state.hostile
                && u.state.in_combat
                && self.content.units[u.def].encounter.is_some()
        });
        let id = fighting.map(Self::id_of);
        if id != self.wing3.boss {
            // A split boss whose original was decommissioned fights on in a replica: same fight.
            let same_family = match (fighting, self.wing3.boss.and_then(|b| self.index(b))) {
                (Some(f), Some(b)) => {
                    self.units[f].def == self.units[b].def
                        && self.content.units[self.units[f].def].split.is_some()
                }
                _ => false,
            };
            if same_family {
                self.wing3.boss = id;
            } else {
                self.wing3_disarm();
                if let Some(i) = fighting {
                    self.wing3_arm(i);
                }
            }
        }
        if let Some(i) = fighting {
            self.wing3_tick(i);
        }
    }

    fn wing3_arm(&mut self, bi: usize) {
        let Some(enc) = self.content.units[self.units[bi].def].encounter.clone() else {
            return;
        };
        let boss = Self::id_of(bi);
        self.wing3.boss = Some(boss);
        match enc {
            EncounterDef::Drafts { props, .. } => {
                self.drafts_clear();
                self.wing3.draft_props.clear();
                for key in props {
                    // A zone that lacks a draft prop simply has fewer drafts to lose.
                    if let Some(at) = self.prop_spot(&key) {
                        self.add_draft(at);
                        self.wing3.draft_props.push(key);
                    }
                }
                self.wing3.lost = 0;
                self.wing3.sent.clear();
                self.wing3.next_sync = self.now;
            }
            EncounterDef::Illusion => {}
            e @ (EncounterDef::Cloud(_) | EncounterDef::Outage(_)) => self.wing4_arm(bi, &e),
            EncounterDef::Squads(cfg) => self.wing5_arm(bi, &cfg),
            EncounterDef::Echo {
                add_unit,
                strike_pct,
            } => {
                let at = self.units[bi].spawn_pos;
                self.echo_start(EchoCfg {
                    boss,
                    center_x_cm: (at.x * 100.0).round() as i32,
                    center_z_cm: (at.z * 100.0).round() as i32,
                    story: self.raid.story,
                    add_unit,
                    strike_pct,
                });
            }
        }
    }

    /// The fight ended one way or another: the drafts, their eaters, the hazard cycle and the
    /// recorder go with it, so the next pull starts clean.
    fn wing3_disarm(&mut self) {
        if self.wing3.boss.take().is_none() {
            return;
        }
        self.drafts_clear();
        self.wing3.draft_props.clear();
        self.wing3.sent.clear();
        self.echo_stop();
        self.illusion_end();
        self.wing4_disarm();
        self.wing5_disarm();
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if !u.despawned && self.content.units[u.def].swarm.is_some() {
                self.despawn(i);
            }
        }
        self.events.push(Event::Drafts {
            integrity: Vec::new(),
            marked: Vec::new(),
        });
    }

    fn wing3_tick(&mut self, bi: usize) {
        let Some(EncounterDef::Drafts {
            fail_lost,
            story_fail_lost,
            heal_pct,
            lost_aura,
            lost_yells,
            fail_yells,
            ..
        }) = self.content.units[self.units[bi].def].encounter.clone()
        else {
            return;
        };
        let boss = Self::id_of(bi);
        for _ in self.drafts_newly_lost() {
            self.wing3.lost = self.wing3.lost.saturating_add(1);
            let heal = u64::from(self.units[bi].state.max_hp) * u64::from(heal_pct) / 100;
            self.heal(boss, bi, None, heal.min(u64::from(u32::MAX)) as u32);
            if let Some(a) = &lost_aura {
                self.apply_aura(bi, a, 3_600_000, None, boss, None);
            }
            if !lost_yells.is_empty() {
                let line = lost_yells[self.rng.below(lost_yells.len() as u32) as usize].clone();
                self.request_yell_line(bi, YellKind::Blast, Some(line));
            }
        }
        let limit = if self.raid.story {
            story_fail_lost
        } else {
            fail_lost
        };
        if self.wing3.lost >= limit {
            // "We learned enough": the room wipes. The boss then resets and the drafts with it.
            if !fail_yells.is_empty() {
                let line = fail_yells[self.rng.below(fail_yells.len() as u32) as usize].clone();
                self.request_yell_line(bi, YellKind::Blast, Some(line));
            }
            self.wing3.lost = 0;
            self.wipe_room(boss);
            return;
        }
        let status = self.draft_status();
        let changed = status != self.wing3.sent;
        if changed && self.now >= self.wing3.next_sync {
            self.wing3.next_sync = self.now + DRAFT_SYNC_TICKS;
            self.wing3.sent = status.clone();
            self.events.push(Event::Drafts {
                integrity: status.iter().map(|s| s.0).collect(),
                marked: status.iter().map(|s| s.1).collect(),
            });
        }
    }

    /// Every living friendly unit takes a hit that finishes it: players, and companions with them.
    fn wipe_room(&mut self, boss: EntityId) {
        let doomed: Vec<(usize, u32)> = (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                !u.despawned && u.state.alive && !u.state.hostile
            })
            .map(|i| (i, self.units[i].state.max_hp.saturating_mul(4)))
            .collect();
        for (i, amount) in doomed {
            self.deal_damage(
                boss,
                Self::id_of(i),
                Hit {
                    spell: None,
                    amount,
                    crit: false,
                    combo: false,
                    school: crate::content::School::Arcane,
                    dot: false,
                },
            );
        }
    }

    /// A joiner sees the drafts as they stand.
    pub(crate) fn wing3_on_join(&mut self, player: EntityId) {
        if self.wing3.boss.is_none() || self.wing3.draft_props.is_empty() {
            return;
        }
        let status = self.draft_status();
        self.emit_to(
            player,
            Event::Drafts {
                integrity: status.iter().map(|s| s.0).collect(),
                marked: status.iter().map(|s| s.1).collect(),
            },
        );
    }

    /// The draft prop's index in the armed encounter's list.
    fn draft_index(&self, key: &str) -> Option<usize> {
        self.wing3.draft_props.iter().position(|k| k == key)
    }

    /// Mark Confidential: anyone, any class. Outcome `PROP_OK` when it took, 0 when no swarm is
    /// at the drafts, the draft is gone or its 40 s are not up yet.
    pub(crate) fn use_draft_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        let enc = self
            .wing3
            .boss
            .and_then(|b| self.index(b))
            .and_then(|b| self.content.units[self.units[b].def].encounter.clone());
        let mut outcome = 0;
        if let (
            Some(EncounterDef::Drafts {
                mark_ms,
                mark_cooldown_ms,
                saved_yells,
                ..
            }),
            Some(d),
        ) = (enc, self.draft_index(&def.key))
            && self.draft_mark(d, mark_ms, mark_cooldown_ms)
        {
            outcome = PROP_OK;
            self.wing3.next_sync = self.now;
            if let Some(bi) = self.wing3.boss.and_then(|b| self.index(b))
                && !saved_yells.is_empty()
            {
                let line = saved_yells[self.rng.below(saved_yells.len() as u32) as usize].clone();
                self.request_yell_line(bi, YellKind::Blast, Some(line));
            }
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key,
            outcome,
        });
    }

    /// A Ground Truth board: shows the user which hazards are real (`il_grounded`, read by the
    /// client) while a cycle runs. Per-player cooldown from the prop.
    pub(crate) fn use_truth_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        let mut outcome = 0;
        // An empty click must not burn the 25 s: the cooldown starts only on a real answer.
        if self.illusion_cycle().is_some()
            && self.prop_ready(player, k, def.cooldown_ms)
            && let Some((aura, ms)) = def.auras.first().cloned()
        {
            self.apply_aura(pi, &aura, ms, None, player, None);
            outcome = PROP_OK;
            if let Some(bi) = (0..self.units.len()).find(|&i| {
                let u = &self.units[i];
                u.state.alive
                    && u.state.hostile
                    && matches!(
                        self.content.units[u.def].encounter,
                        Some(EncounterDef::Illusion)
                    )
            }) {
                self.request_yell(bi, YellKind::Talk);
            }
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key,
            outcome,
        });
    }

    // ---- effects ----

    /// `Effect::SwarmWave`: clusters out of the vents, on a ring round the caster's spawn point
    /// (starting at a random vent) or in a column from one vent.
    #[allow(clippy::too_many_arguments)] // the effect's fields
    pub(crate) fn swarm_wave(
        &mut self,
        ci: usize,
        unit: &str,
        clusters: u8,
        drones: u16,
        ring_m: f32,
        column: bool,
        max_alive: u8,
    ) {
        if max_alive > 0 && self.swarm_clusters().len() >= usize::from(max_alive) {
            return;
        }
        let home = self.units[ci].spawn_pos;
        let n = usize::from(clusters.max(1));
        let start = self.rng.below(360) as f32;
        for k in 0..n {
            let at = if column {
                // One vent, clusters in a line walking toward the middle.
                let a = start.to_radians();
                let (dx, dz) = (a.cos(), a.sin());
                let back = ring_m + 3.0 * k as f32;
                Vec3::new(home.x + dx * back, home.y, home.z + dz * back)
            } else {
                let a = (start + 360.0 * k as f32 / n as f32).to_radians();
                Vec3::new(home.x + ring_m * a.cos(), home.y, home.z + ring_m * a.sin())
            };
            let _ = self.spawn_swarm(unit, at, drones);
        }
    }

    /// `Effect::Confabulate`: a new hazard cycle on the 3 x 3 grid round the caster's spawn point.
    pub(crate) fn confabulate(&mut self, ci: usize, tile_m: f32, damage: u32, warn_ms: u32) {
        let home = self.units[ci].spawn_pos;
        let arena = IllusionArena {
            x_cm: (home.x * 100.0).round() as i32,
            z_cm: (home.z * 100.0).round() as i32,
            tile_cm: (tile_m * 100.0).round().max(1.0) as u32,
        };
        // The cycle's hits are authored numbers: scale them like any other hit of this boss.
        let amount = self.power(ci, damage, 0).min(u64::from(u32::MAX)) as u32;
        self.illusion_begin(arena, Self::id_of(ci), amount, warn_ms);
    }

    /// `Effect::Blink`: the caster jumps to one of the offsets from its spawn point.
    pub(crate) fn blink(&mut self, ci: usize, spots: &[(f32, f32)]) {
        let (home, yaw) = (self.units[ci].spawn_pos, self.units[ci].spawn_yaw);
        let (right, fwd) = spots[self.rng.below(spots.len() as u32) as usize];
        // Same frame as `TelegraphDef::cover`: right and forward of the spawn facing.
        let (sf, sr) = (crate::geom::forward(yaw), crate::telegraph::right(yaw));
        let pos = Vec3::new(
            home.x + sr.x * right + sf.x * fwd,
            home.y,
            home.z + sr.z * right + sf.z * fwd,
        );
        self.units[ci].state.pos = pos;
        self.units[ci].moving = false;
        self.events.push(Event::Teleported {
            unit: Self::id_of(ci),
            pos,
        });
    }

    /// `Effect::Footnotes`: `count` footnotes beside the caster, one of them the real citation.
    pub(crate) fn footnotes(&mut self, ci: usize, fake: &str, real: &str, count: u8) {
        let n = count.max(1);
        let pick = self.rng.below(u32::from(n)) as u8;
        let at = self.units[ci].state.pos;
        for k in 0..n {
            let key = if k == pick { real } else { fake };
            self.summon(ci, at, key, 1, u8::MAX, k);
        }
    }

    /// Debug/test: the armed boss of the encounter machinery, if any.
    pub fn wing3_boss(&self) -> Option<EntityId> {
        self.wing3.boss
    }
}
