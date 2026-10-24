//! Resistance companions (docs/ILMARI_09.md B.7): Dale (tank), Priya (healer) and Rupert (damage)
//! can come along into an Ilmari wing on Story, so a solo player has company.
//!
//! A companion is an ordinary friendly unit (`UnitDef::companion`, no `ai`), not a group member:
//! it never joins `Sim::players`, so every mechanic that looks at players (the Recruiter's
//! candidates, the mirrors, the review's grades, managed out, the tethers) ignores it with no
//! special case. The brain here is a pure function of sim state, with no rng, so replays stay
//! bit-identical, and it runs inside the tick, so a server runs the same code.
//!
//! The loadout (`picks`) is chosen at the base's sign-up sheet and lives per player; the units are
//! spawned when a wing loads on Story and are gone with the zone. Full never gets them: it is the
//! group fight, and `companion_group_pct` makes every companion on Story count as part of a
//! group member for the enemy HP and damage scaling.

use crate::content::{CompanionDef, CompanionRole, SpellId, YellKind};
use crate::effects::dist_xz;
use crate::floors::pack_offsets;
use crate::geom::Vec3;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::time::TICK_HZ;
use crate::types::{AiState, Event};
use std::collections::BTreeMap;

/// The base's sign-up sheet: talk to it to cycle the loadout.
pub const ROSTER: &str = "il_roster";
pub const DALE: &str = "il_comp_dale";
pub const PRIYA: &str = "il_comp_priya";
pub const RUPERT: &str = "il_comp_rupert";
/// Companions one player may bring.
pub const MAX_PER_PLAYER: usize = 2;
/// Companions in one room, whoever brought them: a full room is a group, not an escort.
pub const MAX_PER_ROOM: usize = 2;

/// What the sheet cycles through, with the line it answers; pairs only, because the doc's
/// "pick two" is the point of the choice.
const PRESETS: [(&[&str], &str); 4] = [
    (
        &[],
        "Roster cleared. You are your own team now. HR calls that 'ownership'.",
    ),
    (
        &[DALE, PRIYA],
        "Dale and Priya signed. Tank and healer. The paperwork is laminated.",
    ),
    (
        &[DALE, RUPERT],
        "Dale and Rupert signed. Tank and damage. Rupert initialled twice, once in the wrong box.",
    ),
    (
        &[PRIYA, RUPERT],
        "Priya and Rupert signed. Healer and damage. Nobody is holding aggro. Bold.",
    ),
];
const FULL_LINE: &str =
    "Full is a real fight. Nobody gets lent out. Set the wing door to Story, then come back.";

const RUN_MPS: f32 = 6.5;
/// Gap a companion keeps to its owner while nothing is fighting; the second one stands further.
const FOLLOW_M: f32 = 4.0;
const FOLLOW_STEP_M: f32 = 1.5;
/// Past this the owner outran it (a door, a mount, a bot): it appears next to them instead.
const SNAP_M: f32 = 40.0;
/// Enemies this close to the owner are the group's business.
const ENGAGE_M: f32 = 45.0;
/// A dead companion is back this long after the owner leaves combat, free of charge.
const REVIVE_MS: u32 = 8_000;
const HEAL_BELOW_PCT: u64 = 70;
const DEFENSE_BELOW_PCT: u64 = 50;
/// Rupert stands this far from his target: out of melee, inside his bolt's range.
const RANGED_STAND_M: f32 = 10.0;
const MELEE_STAND_M: f32 = 2.4;

#[derive(Default)]
pub(crate) struct CompanionState {
    /// Loadout per player: companion unit keys, at most `MAX_PER_PLAYER`.
    pub(crate) picks: BTreeMap<EntityId, Vec<String>>,
    /// (owner, unit) of the companions out in the loaded zone, in spawn order.
    pub(crate) active: Vec<(EntityId, EntityId)>,
}

/// The highest-threat entry, lowest id on a tie (the rule `ai.rs` fights by).
fn top_threat(threat: &BTreeMap<EntityId, u32>) -> Option<EntityId> {
    threat
        .iter()
        .fold(None, |best: Option<(EntityId, u32)>, (id, t)| match best {
            Some((_, bt)) if bt >= *t => best,
            _ => Some((*id, *t)),
        })
        .map(|(id, _)| id)
}

impl Sim {
    /// Companion units a player has out right now.
    pub fn companions_of(&self, owner: EntityId) -> Vec<EntityId> {
        self.companions
            .active
            .iter()
            .filter(|(o, _)| *o == owner)
            .map(|(_, u)| *u)
            .collect()
    }

    /// The player's loadout, as companion unit keys.
    pub fn companion_picks(&self, owner: EntityId) -> Vec<String> {
        self.companions
            .picks
            .get(&owner)
            .cloned()
            .unwrap_or_default()
    }

    /// Whom `unit` fights for, when it is a companion.
    pub(crate) fn companion_owner(&self, unit: EntityId) -> Option<EntityId> {
        self.companions
            .active
            .iter()
            .find(|(_, u)| *u == unit)
            .map(|(o, _)| *o)
    }

    /// Replaces the loadout with the known companion keys of `keys`, deduplicated and capped.
    /// Used by the save, the roster and the bots; a wire client has no command for it.
    pub fn set_companion_picks(&mut self, owner: EntityId, keys: &[String]) {
        let mut picks: Vec<String> = Vec::new();
        for k in keys {
            let known = self
                .content
                .unit_def(k)
                .is_some_and(|d| d.companion.is_some());
            if known && !picks.contains(k) && picks.len() < MAX_PER_PLAYER {
                picks.push(k.clone());
            }
        }
        if picks.is_empty() {
            self.companions.picks.remove(&owner);
        } else {
            self.companions.picks.insert(owner, picks);
        }
    }

    /// Talking to the sign-up sheet: the next preset, or a refusal on Full. True when `key` is
    /// the sheet, so the generic talk line stays quiet.
    pub(crate) fn companion_roster_talk(&mut self, player: EntityId, ti: usize, key: &str) -> bool {
        if key != ROSTER {
            return false;
        }
        let line = if self.wing_story() {
            let have = self.companion_picks(player);
            let at = PRESETS.iter().position(|(keys, _)| {
                keys.len() == have.len() && keys.iter().zip(&have).all(|(a, b)| a == b)
            });
            let (keys, line) = PRESETS[at.map_or(0, |k| (k + 1) % PRESETS.len())];
            let keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
            self.set_companion_picks(player, &keys);
            line
        } else {
            FULL_LINE
        };
        self.request_yell_line(ti, YellKind::Talk, Some(line.to_string()));
        true
    }

    /// Before a zone loads: the old companions are about to despawn with everything else.
    pub(crate) fn companion_on_load(&mut self) {
        self.companions.active.clear();
    }

    /// A player leaves: their companions go with them.
    pub(crate) fn companion_on_leave(&mut self, owner: EntityId) {
        self.companions.picks.remove(&owner);
        let gone: Vec<EntityId> = self.companions_of(owner);
        if gone.is_empty() {
            return;
        }
        for id in &gone {
            if let Some(i) = self.index(*id) {
                self.despawn(i);
            }
        }
        self.companions.active.retain(|(o, _)| *o != owner);
        self.rescale_group_hp();
    }

    /// After a wing loads on Story: every player's picks step out beside them, up to the room's cap.
    pub(crate) fn companions_after_load(&mut self) {
        if !self.raid.story || !self.cur_raid_is_ilmari() {
            return;
        }
        let ring = pack_offsets(MAX_PER_ROOM + 1);
        for owner in self.players.clone() {
            for key in self.companion_picks(owner) {
                let slot = self.companions.active.len();
                let (Some(oi), Ok(def)) = (self.index(owner), self.def_index(&key)) else {
                    continue;
                };
                if slot >= MAX_PER_ROOM {
                    return self.rescale_group_hp();
                }
                let o = &self.units[oi].state;
                let (dx, dz) = ring[slot + 1];
                let pos = Vec3::new(o.pos.x + dx, o.pos.y, o.pos.z + dz);
                let id = self.spawn(
                    def,
                    pos,
                    o.yaw,
                    crate::sim::SpawnOpts {
                        level: None,
                        pack: None,
                        respawns: false,
                        summoner: None,
                        floor_boss: false,
                        zone_slot: None,
                    },
                );
                self.companions.active.push((owner, id));
            }
        }
        if !self.companions.active.is_empty() {
            self.rescale_group_hp();
        }
    }

    /// How many group members the companions add to the enemy scaling, in percent of one.
    pub(crate) fn companion_scaling_pct(&self) -> u32 {
        self.companions.active.len() as u32 * self.content.config.companion_group_pct
    }

    pub(crate) fn update_companions(&mut self) {
        for k in 0..self.companions.active.len() {
            let (owner, id) = self.companions.active[k];
            if let Some(ci) = self.index(id) {
                self.companion_think(owner, ci, k);
            }
        }
    }

    fn companion_think(&mut self, owner: EntityId, ci: usize, slot: usize) {
        let Some(oi) = self.index(owner).filter(|&o| !self.units[o].despawned) else {
            return;
        };
        let Some(def) = self.content.units[self.units[ci].def].companion.clone() else {
            return;
        };
        self.units[ci].moving = false;
        if !self.units[ci].state.alive {
            return self.companion_revive(ci, oi);
        }
        if !self.units[oi].state.alive || self.has_aura_flag(ci, |a| a.stun) {
            return;
        }
        let (me, home) = (self.units[ci].state.pos, self.units[oi].state.pos);
        if dist_xz(me, home) > SNAP_M {
            let (dx, dz) = pack_offsets(MAX_PER_ROOM + 1)[slot + 1];
            self.units[ci].state.pos = Vec3::new(home.x + dx, home.y, home.z + dz);
            self.units[ci].nav.reset();
            return;
        }
        // The Region Outage's room: never stranded on a platform that is going dark or left behind.
        if self.companion_platform_snap(ci, oi) {
            let (dx, dz) = pack_offsets(MAX_PER_ROOM + 1)[slot + 1];
            self.units[ci].state.pos = Vec3::new(home.x + dx, home.y, home.z + dz);
            self.units[ci].nav.reset();
            return;
        }
        if self.companion_dodge(ci) {
            return;
        }
        if self.units[ci].cast.is_some() {
            return;
        }
        match def.role {
            CompanionRole::Healer => {
                if !self.companion_heal(ci, oi, &def) {
                    self.companion_follow(ci, oi, slot);
                }
            }
            CompanionRole::Tank => self.companion_tank(ci, oi, slot, &def),
            CompanionRole::Damage => self.companion_damage(ci, oi, slot, &def),
        }
    }

    /// Back on its feet beside the owner once the fight is over; the "checkpoint" of a wing is
    /// the end of the pull, and it costs nothing.
    fn companion_revive(&mut self, ci: usize, oi: usize) {
        let due = self.units[ci].died_at + crate::content::ticks(REVIVE_MS);
        if self.now < due || self.units[oi].state.in_combat || !self.units[oi].state.alive {
            return;
        }
        let at = self.units[oi].state.pos;
        let u = &mut self.units[ci];
        u.state.alive = true;
        u.state.hp = u.state.max_hp;
        u.state.pos = Vec3::new(at.x + 1.5, at.y, at.z + 1.5);
        u.cooldowns.clear();
        u.nav.reset();
        let unit = Self::id_of(ci);
        self.events.push(Event::Respawned { unit });
    }

    /// An enemy a companion may hit: hostile with a brain, awake, and not a bystander the
    /// Recruiter's humans stand for (`death_burnout`: the sheet's rule, nobody hits them).
    fn companion_foe(&self, o: usize) -> bool {
        let u = &self.units[o];
        let d = &self.content.units[u.def];
        u.state.alive
            && !u.despawned
            && u.state.hostile
            && d.ai.is_some()
            && d.death_burnout == 0
            && u.state.ai != Some(AiState::Evade)
    }

    /// The core's mainframe (or any unit authored as a ward) while it stands.
    fn companion_ward(&self) -> Option<usize> {
        (0..self.units.len()).find(|&o| {
            let u = &self.units[o];
            u.state.alive && !u.despawned && self.content.units[u.def].ward
        })
    }

    /// Whether `o` is chewing on a ward: the rewrites of P4 are fought for it, not for the owner.
    fn companion_at_ward(&self, o: usize) -> bool {
        let u = &self.units[o];
        top_threat(&u.threat).is_some_and(|t| {
            self.index(t)
                .is_some_and(|ti| self.content.units[self.units[ti].def].ward)
        })
    }

    /// The enemies fighting this owner's side near them, plus whatever the owner has targeted.
    fn companion_foes(&self, oi: usize) -> Vec<usize> {
        let owner = Self::id_of(oi);
        let at = self.units[oi].state.pos;
        let target = self.units[oi].state.target;
        (0..self.units.len())
            .filter(|&o| {
                self.companion_foe(o) && {
                    let u = &self.units[o];
                    dist_xz(u.state.pos, at) <= ENGAGE_M
                        && (target == Some(Self::id_of(o))
                            || self.companion_at_ward(o)
                            // A swarm cluster has no threat table until somebody hits it: it
                            // is a foe by walking at the drafts.
                            || self.content.units[u.def].swarm.is_some()
                            || u.threat.contains_key(&owner)
                            || self
                                .companions
                                .active
                                .iter()
                                .any(|(_, c)| u.threat.contains_key(c)))
                }
            })
            .collect()
    }

    fn companion_nearest(&self, ci: usize, of: &[usize]) -> Option<usize> {
        let me = self.units[ci].state.pos;
        of.iter().copied().min_by(|&a, &b| {
            let (da, db) = (
                dist_xz(self.units[a].state.pos, me),
                dist_xz(self.units[b].state.pos, me),
            );
            da.total_cmp(&db).then(a.cmp(&b))
        })
    }

    fn companion_tank(&mut self, ci: usize, oi: usize, slot: usize, def: &CompanionDef) {
        let (me, owner) = (Self::id_of(ci), Self::id_of(oi));
        let foes = self.companion_foes(oi);
        // Whatever is chewing on the owner comes first, then the owner's own target.
        let loose: Vec<usize> = foes
            .iter()
            .copied()
            .filter(|&o| {
                top_threat(&self.units[o].threat) == Some(owner) || self.companion_at_ward(o)
            })
            .collect();
        let target = self.companion_nearest(ci, &loose).or_else(|| {
            let t = self.units[oi].state.target;
            foes.iter().copied().find(|&o| Some(Self::id_of(o)) == t)
        });
        let Some(t) = target.or_else(|| self.companion_nearest(ci, &foes)) else {
            return self.companion_follow(ci, oi, slot);
        };
        let tid = Self::id_of(t);
        self.face(ci, t);
        let hp_pct = u64::from(self.units[ci].state.hp) * 100
            / u64::from(self.units[ci].state.max_hp.max(1));
        if hp_pct < DEFENSE_BELOW_PCT
            && let Some(s) = def.defense
            && self.companion_cast(ci, s, me)
        {
            return;
        }
        let holds = top_threat(&self.units[t].threat) == Some(me);
        if !holds
            && let Some(s) = def.utility
            && self.companion_cast(ci, s, tid)
        {
            return;
        }
        if !self.companion_cast(ci, def.main, tid) {
            self.companion_walk(ci, self.units[t].state.pos, MELEE_STAND_M);
        }
    }

    fn companion_damage(&mut self, ci: usize, oi: usize, slot: usize, def: &CompanionDef) {
        let foes = self.companion_foes(oi);
        if foes.is_empty() {
            return self.companion_follow(ci, oi, slot);
        }
        // A cast that must be stopped always is, before anything else.
        if let Some(kick) = def.utility {
            let must: Vec<usize> = foes
                .iter()
                .copied()
                .filter(|&o| {
                    self.units[o].cast.as_ref().is_some_and(|c| {
                        c.interruptible && self.content.spell(c.spell).is_some_and(|s| s.kick)
                    })
                })
                .collect();
            if let Some(t) = self.companion_nearest(ci, &must) {
                self.face(ci, t);
                if self.companion_cast(ci, kick, Self::id_of(t)) {
                    return;
                }
            }
        }
        // A cluster on its way to a draft comes before everything else: the one nearest a draft
        // that still stands (drafts are the owner's to mark, the fight for them is shared).
        if let Some(c) = self.companion_swarm_pick(&foes)
            && self.companion_attack(ci, c, def)
        {
            return;
        }
        // Adds die first; then the owner's target; then whatever is nearest.
        let adds: Vec<usize> = foes
            .iter()
            .copied()
            .filter(|&o| self.units[o].summoner.is_some())
            .collect();
        let owner_target = self.units[oi].state.target;
        let t = self
            .companion_nearest(ci, &adds)
            .or_else(|| {
                foes.iter()
                    .copied()
                    .find(|&o| Some(Self::id_of(o)) == owner_target)
            })
            .or_else(|| self.companion_nearest(ci, &foes));
        let Some(t) = t else { return };
        self.companion_attack(ci, t, def);
    }

    /// Casts the main attack at `t`, or walks to the stand-off distance. False when it did
    /// neither (nothing usable and already standing there).
    fn companion_attack(&mut self, ci: usize, t: usize, def: &CompanionDef) -> bool {
        self.face(ci, t);
        if self.companion_cast(ci, def.main, Self::id_of(t)) {
            return true;
        }
        let range = self.content.spell(def.main).map_or(0.0, |s| s.range_m);
        let stand = RANGED_STAND_M.min(range - 2.0).max(MELEE_STAND_M);
        let blocked = dist_xz(self.units[ci].state.pos, self.units[t].state.pos) <= stand;
        // Within the stand-off but still blocked means no line of sight: get closer.
        self.companion_walk(
            ci,
            self.units[t].state.pos,
            if blocked { 1.0 } else { stand },
        );
        true
    }

    /// The swarm cluster closest to a draft that still stands (lowest id on a tie).
    fn companion_swarm_pick(&self, foes: &[usize]) -> Option<usize> {
        let live: Vec<Vec3> = self
            .drafts()
            .iter()
            .filter(|d| d.integrity > 0)
            .map(|d| d.pos)
            .collect();
        if live.is_empty() {
            return None;
        }
        let near = |o: usize| {
            live.iter()
                .map(|d| dist_xz(*d, self.units[o].state.pos))
                .fold(f32::MAX, f32::min)
        };
        foes.iter()
            .copied()
            .filter(|&o| self.content.units[self.units[o].def].swarm.is_some())
            .min_by(|&a, &b| near(a).total_cmp(&near(b)).then(a.cmp(&b)))
    }

    /// Heals the most hurt of the owner's side below the threshold; false when nobody needs it.
    fn companion_heal(&mut self, ci: usize, oi: usize, def: &CompanionDef) -> bool {
        // The mainframe comes before the party: losing it loses the fight.
        if let Some(w) = self.companion_ward().filter(|&w| {
            let s = &self.units[w].state;
            u64::from(s.hp) * 100 < u64::from(s.max_hp) * HEAL_BELOW_PCT
        }) {
            self.face(ci, w);
            if self.companion_cast(ci, def.main, Self::id_of(w)) {
                return true;
            }
        }
        let mut side: Vec<usize> = vec![oi];
        side.extend(
            self.companions
                .active
                .iter()
                .filter_map(|(_, c)| self.index(*c)),
        );
        let hurt = side
            .into_iter()
            .filter(|&o| {
                let s = &self.units[o].state;
                s.alive && u64::from(s.hp) * 100 < u64::from(s.max_hp) * HEAL_BELOW_PCT
            })
            .min_by_key(|&o| {
                let s = &self.units[o].state;
                (u64::from(s.hp) * 1000 / u64::from(s.max_hp.max(1)), o)
            });
        let Some(t) = hurt else { return false };
        if t != ci {
            self.face(ci, t);
        }
        self.companion_cast(ci, def.main, Self::id_of(t))
    }

    fn face(&mut self, ci: usize, t: usize) {
        let (me, them) = (self.units[ci].state.pos, self.units[t].state.pos);
        self.units[ci].state.yaw = crate::ai::yaw_toward(me, them);
    }

    /// One validated cast; false (and no event) when the spell is not usable right now, so the
    /// brain falls through to walking instead of spamming failed-cast events.
    fn companion_cast(&mut self, ci: usize, spell: SpellId, target: EntityId) -> bool {
        let Some(spell) = self.content.spell(spell).cloned() else {
            return false;
        };
        self.units[ci].state.target = Some(target);
        let Ok(t) = self.validate(ci, &spell) else {
            return false;
        };
        if self.busy(ci, &spell).is_some() {
            return false;
        }
        self.start_cast(ci, &spell, t);
        true
    }

    fn companion_follow(&mut self, ci: usize, oi: usize, slot: usize) {
        let gap = FOLLOW_M + FOLLOW_STEP_M * slot as f32;
        self.companion_walk(ci, self.units[oi].state.pos, gap);
        self.units[ci].state.target = None;
    }

    fn companion_walk(&mut self, ci: usize, goal: Vec3, stop: f32) {
        let me = self.units[ci].state.pos;
        if dist_xz(me, goal) <= stop {
            return;
        }
        let Some((wp, last)) = self.nav_target(ci, goal) else {
            return;
        };
        let step = RUN_MPS / TICK_HZ as f32;
        self.units[ci].state.yaw = crate::ai::yaw_toward(me, wp);
        let next = crate::ai::step_toward(me, wp, step, if last { stop } else { 0.0 });
        // A safe companion never walks back into a pending hazard it just left (it would be
        // standing in it when the warning ends); the dodge itself only steps to safe spots.
        if self.telegraph_danger(next) && !self.telegraph_danger(me) {
            return;
        }
        self.units[ci].state.pos = next;
        self.units[ci].moving = true;
    }

    /// Whether standing at `p` gets you hit by a pending enemy telegraph. Shared (split) ones
    /// are meant to be stood in together, so they never count.
    fn telegraph_danger(&self, p: Vec3) -> bool {
        self.telegraphs.iter().any(|t| {
            t.hostile
                && !t.split
                && t.shape.contains(t.pos, t.facing, p)
                && !t.cover.iter().any(|c| dist_xz(*c, p) <= t.cover_radius_m)
        }) || self
            .illusion_pending()
            .is_some_and(|(real, arena)| real.hits(&arena, p.x, p.z, 0.0))
            // The Hallucination's fakes are cosmetic and never reach the sim: a companion only
            // ever sees the truth. The recorder's ghosts count like telegraphs.
            || self
                .echo_ghosts()
                .iter()
                .any(|(at, r)| dist_xz(*at, p) <= *r)
    }

    /// Steps out of a ground telegraph toward the nearest safe spot (a metre of margin, a clear
    /// walk). True while it is dodging, which takes priority over everything else.
    fn companion_dodge(&mut self, ci: usize) -> bool {
        let me = self.units[ci].state.pos;
        if !self.telegraph_danger(me) {
            return false;
        }
        let safe = |s: &Sim, p: Vec3| {
            [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
                .iter()
                .all(|(dx, dz)| !s.telegraph_danger(Vec3::new(p.x + dx, p.y, p.z + dz)))
                && s.los_padded.clear(me, p)
        };
        let spot = [2.0_f32, 3.5, 5.0, 7.0, 9.0, 12.0]
            .iter()
            .flat_map(|r| (0..16).map(move |k| (*r, k as f32 * std::f32::consts::TAU / 16.0)))
            .map(|(r, a)| Vec3::new(me.x + r * a.sin(), me.y, me.z - r * a.cos()))
            .find(|p| safe(self, *p));
        let Some(spot) = spot else { return false };
        // A cast-time spell cannot survive the step.
        if self.units[ci].cast.is_some() {
            self.cancel_cast(ci, None, 0);
        }
        self.companion_walk(ci, spot, 0.3);
        true
    }
}
