//! P4 of the core, "Last Commit" (docs/ILMARI_1_0.md D.5, D13): the mech collapses and a COBOL
//! mainframe rises from the hatch. It is the ward: Ilmari's attacks go at it (`AiTarget::Ward`),
//! rewrites walk to it and chew, the party keeps it up for a clock that the rewrites it kills
//! shorten. Then the terminals light, a `git log` is on the wall and the group has to revert the
//! one commit that made Ilmari self-aware: a wrong pick costs, three raise a Force Push, the right
//! one is the player's `git revert` and the fall of the core.

use super::{CoreRt, LONG_MS, RemixCast, Voice, commit_log, culprit_index};
use crate::content::{Content, SpellId, TOKEN_STACK_CAP, ticks};
use crate::geom::Vec3;
use crate::geom::forward;
use crate::ilmari::IlmariMode;
use crate::sim::{Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::types::Event;
use crate::world::PROP_OK;
use serde::Deserialize;
use std::collections::BTreeSet;

/// The terminals each cover two rows of the log: terminal `t` is the one for rows `t` and `t + 3`.
pub const TERMINALS: usize = 3;

#[derive(Clone, Debug, Deserialize)]
pub struct MainframeDef {
    /// The ward's unit and, on Story, the aura that makes it take less.
    pub unit: String,
    pub story_aura: String,
    /// The clock the party keeps it up for.
    pub clock_ms: u32,
    pub story_clock_ms: u32,
    pub judgment_clock_ms: u32,
    /// The rewrites: each one the party kills takes `add_cut_ms` off the clock, `add_cut_max_ms`
    /// at most; a wave comes every `wave_ms` (the rider's is bigger) and starts on the ward.
    pub add: String,
    pub add_cut_ms: u32,
    pub add_cut_max_ms: u32,
    pub wave_first_ms: u32,
    pub wave_ms: u32,
    pub wave_adds: u8,
    pub story_wave_adds: u8,
    pub rider_wave_adds: u8,
    pub add_threat: u32,
    /// The punch cards: each heals the ward `card_heal_pct` of its health, once per `card_cooldown_ms`.
    pub cards: Vec<String>,
    pub card_heal_pct: u32,
    pub card_cooldown_ms: u32,
    /// The terminals, in order.
    pub terminals: Vec<String>,
    /// A wrong pick: the picker's `Merge Conflict` (on Judgment it lasts), the terminal's rest, the
    /// health Ilmari gets back; the Force Push after `wrong_limit` of them, and the protection it
    /// buys the mainframe.
    pub merge_aura: String,
    pub merge_ms: u32,
    pub story_merge_ms: u32,
    pub terminal_down_ms: u32,
    pub wrong_heal_pct: u32,
    pub wrong_limit: u8,
    pub force_spell: SpellId,
    pub force_protect_ms: u32,
    /// The beat between the right pick and the fall.
    pub final_ms: u32,
    pub open_yells: Vec<String>,
    pub hit_yells: Vec<String>,
    pub hit_ms: u32,
    pub revert_yells: Vec<String>,
    /// Judgment only: Marie says what to look for.
    pub marie_hint: String,
    pub wrong_yells: Vec<String>,
    pub force_yells: Vec<String>,
    pub right_yells: Vec<String>,
    pub final_yells: Vec<String>,
    /// `{subject}` is the reverted commit.
    pub reverted: String,
    /// `{player}` is the one who made the pick.
    pub head_line: String,
    pub fallback_name: String,
    pub story_marker: String,
    pub clears: Vec<String>,
}

impl MainframeDef {
    pub(super) fn validate(&self, c: &Content) -> bool {
        let spells = std::iter::once(self.force_spell);
        let units = [&self.unit, &self.add]
            .into_iter()
            .chain(self.clears.iter());
        let auras = [&self.story_aura, &self.merge_aura];
        spells.into_iter().all(|s| c.spell(s).is_some())
            && units.into_iter().all(|u| c.unit_def(u).is_some())
            && auras.into_iter().all(|a| c.aura(a).is_some())
            && c.unit_def(&self.unit).is_some_and(|u| u.ward && !u.hostile)
            && self.terminals.len() == TERMINALS
            && !self.cards.is_empty()
            && self.clock_ms > 0
            && self.story_clock_ms > 0
            && self.judgment_clock_ms > 0
            && self.wave_ms > 0
            && self.wrong_limit > 0
            && self.card_cooldown_ms > 0
            && !self.open_yells.is_empty()
            && !self.right_yells.is_empty()
            && !self.wrong_yells.is_empty()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The clock runs.
    Protect = 1,
    /// The terminals are lit.
    Revert = 2,
    /// The right commit is reverted: the screens roll back.
    Final = 3,
    Done = 4,
}

pub(super) struct P4Rt {
    ward: EntityId,
    stage: Stage,
    /// The tick the protection ends at.
    until: u64,
    cut_ms: u32,
    next_wave: u64,
    waves: u32,
    cards_next: Vec<u64>,
    terms_next: Vec<u64>,
    wrong: u8,
    culprit: usize,
    /// Story: the culprit is marked once anybody picks wrong.
    marked: bool,
    /// Judgment: who has used a terminal (a different member reads each time).
    pickers: BTreeSet<EntityId>,
    final_at: u64,
    picker: Option<EntityId>,
    pub(super) log: Option<Vec<String>>,
    next_hit: u64,
}

impl Sim {
    /// The ward: the mainframe once it is up and standing.
    pub(super) fn core_ward(&self, rt: &CoreRt) -> Option<usize> {
        rt.ward
            .and_then(|w| self.index(w))
            .filter(|&w| self.units[w].state.alive && !self.units[w].despawned)
    }

    /// The mech falls apart and the mainframe rises from the hatch.
    pub(super) fn core_enter_p4(&mut self, rt: &mut CoreRt) {
        let cfg = rt.cfg.clone();
        let m = &cfg.mainframe;
        self.core_clear_body(rt);
        for &s in &rt.screens {
            self.cancel_telegraphs_of(s);
        }
        self.core_set_phase(rt, 4);
        let (now, story) = (self.now, self.raid.story);
        let Some(li) = self.core_live(rt) else { return };
        let live_id = Self::id_of(li);
        let c = self.core_centre(rt);
        let Ok(def) = self.def_index(&m.unit) else {
            return;
        };
        let ward = self.spawn(
            def,
            Vec3::new(c.x, c.y, c.z),
            0.0,
            SpawnOpts {
                level: None,
                pack: None,
                respawns: false,
                summoner: Some(live_id),
                floor_boss: false,
                zone_slot: None,
            },
        );
        rt.ward = Some(ward);
        self.core_start_p4_abilities(rt);
        if let Some(i) = self.index(ward) {
            // A threat entry keeps it "in combat", so it does not regenerate between hits.
            self.add_threat(i, live_id, 1);
            if story {
                self.apply_aura(i, &m.story_aura, LONG_MS, None, ward, None);
            }
        }
        let clock = match (self.raid.mode, story) {
            (IlmariMode::Judgment, _) => m.judgment_clock_ms,
            (_, true) => m.story_clock_ms,
            _ => m.clock_ms,
        };
        rt.p4 = Some(P4Rt {
            ward,
            stage: Stage::Protect,
            until: now + ticks(clock),
            cut_ms: 0,
            next_wave: now + ticks(m.wave_first_ms),
            waves: 0,
            cards_next: vec![0; m.cards.len()],
            terms_next: vec![0; TERMINALS],
            wrong: 0,
            culprit: 0,
            marked: false,
            pickers: BTreeSet::new(),
            final_at: 0,
            picker: None,
            log: None,
            next_hit: now + ticks(m.hit_ms),
        });
        let lines = rt.cfg.p4_yells.clone();
        self.core_say_any(rt, Voice::Screen, &lines, 0);
        self.core_say_any(rt, Voice::Screen, &m.open_yells, 6_000);
    }

    pub(super) fn core_p4_step(&mut self, rt: &mut CoreRt) {
        let Some(mut p4) = rt.p4.take() else { return };
        let cfg = rt.cfg.clone();
        let now = self.now;
        // Ilmari does not go below the floor until the revert.
        if p4.stage != Stage::Done {
            for &s in &rt.screens {
                if let Some(i) = self.index(s).filter(|&i| self.units[i].state.alive) {
                    let st = &mut self.units[i].state;
                    let floor =
                        (u64::from(st.max_hp) * u64::from(cfg.p4_below_pct)).div_ceil(100) as u32;
                    st.hp = st.hp.max(floor);
                }
            }
        }
        match p4.stage {
            Stage::Protect => self.p4_protect(rt, &mut p4),
            Stage::Final if now >= p4.final_at => self.p4_finish(rt, &mut p4),
            _ => {}
        }
        // The mainframe never regenerates by itself.
        if let Some(w) = self.index(p4.ward).filter(|&w| self.units[w].state.alive)
            && self.units[w].threat.is_empty()
            && let Some(li) = self.core_live(rt)
        {
            self.add_threat(w, Self::id_of(li), 1);
        }
        rt.p4 = Some(p4);
    }

    fn p4_protect(&mut self, rt: &mut CoreRt, p4: &mut P4Rt) {
        let cfg = rt.cfg.clone();
        let m = &cfg.mainframe;
        let now = self.now;
        let kills = std::mem::take(&mut self.core.add_kills);
        if kills > 0 {
            let cut = (kills * m.add_cut_ms).min(m.add_cut_max_ms.saturating_sub(p4.cut_ms));
            p4.cut_ms += cut;
            p4.until = p4.until.saturating_sub(ticks(cut));
        }
        if now >= p4.next_wave {
            p4.next_wave = now + ticks(m.wave_ms);
            let n = if self.core_rider(rt) {
                m.rider_wave_adds
            } else if self.raid.story {
                m.story_wave_adds
            } else {
                m.wave_adds
            };
            self.p4_wave(rt, p4, n);
            p4.waves += 1;
        }
        if now >= p4.next_hit {
            p4.next_hit = now + ticks(m.hit_ms);
            self.core_say_any(rt, Voice::Screen, &m.hit_yells, 0);
        }
        if now >= p4.until {
            self.p4_light(rt, p4);
        }
    }

    /// A wave of rewrites from the wall: they head for the mainframe and chew it.
    fn p4_wave(&mut self, rt: &CoreRt, p4: &P4Rt, n: u8) {
        let m = &rt.cfg.mainframe;
        let Some(li) = self.core_live(rt) else { return };
        let live_id = Self::id_of(li);
        let c = self.core_centre(rt);
        let f = forward(p4.waves as f32 * 2.1 + 0.7);
        let at = Vec3::new(c.x + f.x * 15.0, c.y, c.z + f.z * 15.0);
        self.summon_with(li, at, &m.add, (n, 16, 0), true);
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.summoner == Some(live_id)
                && u.state.key == m.add
                && u.state.alive
                && !u.despawned
                && u.threat.is_empty()
            {
                self.add_threat(i, p4.ward, m.add_threat);
                self.set_ai(i, crate::types::AiState::Chase);
            }
        }
    }

    /// The clock ran out: the rewrites pause, the terminals light and the log goes up.
    fn p4_light(&mut self, rt: &mut CoreRt, p4: &mut P4Rt) {
        let cfg = rt.cfg.clone();
        let m = &cfg.mainframe;
        p4.stage = Stage::Revert;
        self.p4_remove_adds(rt);
        // Nothing is thrown while the group reads: the screens freeze on the prompt.
        self.core_hold_screens(rt, true);
        for &s in &rt.screens {
            self.cancel_telegraphs_of(s);
        }
        p4.culprit = culprit_index(self.ilmari.stamp.as_ref().map_or(0, |s| s.unix_secs));
        let lines = self.p4_log(p4, m);
        p4.log = Some(lines.clone());
        self.events.push(Event::CoreLog { lines });
        self.core_say_any(rt, Voice::Screen, &m.revert_yells, 0);
        if self.raid.mode == IlmariMode::Judgment {
            self.core_say(rt, Voice::Marie, &m.marie_hint, 5_000);
        }
    }

    /// The wall's rows: `[terminal] hash subject author when`, the culprit marked on Story once
    /// somebody has picked wrong.
    fn p4_log(&self, p4: &P4Rt, m: &MainframeDef) -> Vec<String> {
        let clock = self
            .ilmari
            .stamp
            .as_ref()
            .map_or_else(|| "the first commit".to_string(), |s| s.clock());
        let mut lines: Vec<String> = commit_log(p4.culprit, &clock)
            .into_iter()
            .enumerate()
            .map(|(row, c)| {
                let mut l = format!(
                    "[{}] {}  {}  {}  {}",
                    row % TERMINALS + 1,
                    c.hash,
                    c.subject,
                    c.author,
                    c.when
                );
                if self.raid.story && p4.marked && row == p4.culprit {
                    l.push_str(&m.story_marker);
                }
                l
            })
            .collect();
        if let (Stage::Final | Stage::Done, Some(p)) = (p4.stage, p4.picker) {
            lines.insert(0, self.p4_head(p, m));
        }
        lines
    }

    /// The new HEAD after the revert, in the picker's name.
    fn p4_head(&self, picker: EntityId, m: &MainframeDef) -> String {
        let name = self
            .index(picker)
            .map(|i| self.units[i].state.name.clone())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| m.fallback_name.clone());
        m.head_line.replace("{player}", &name)
    }

    fn p4_remove_adds(&mut self, rt: &CoreRt) {
        let m = &rt.cfg.mainframe;
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if !u.despawned && u.state.alive && u.state.key == m.add {
                self.despawn(i);
            }
        }
    }

    /// A terminal: the right commit reverts Ilmari, a wrong one costs the group.
    pub(crate) fn use_git_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let key = self.content.world.props[k].key.clone();
        let mut outcome = 0;
        if let Some(mut rt) = self.core.rt.take() {
            outcome = self.p4_pick(&mut rt, player, &key);
            self.core.rt = Some(rt);
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: key,
            outcome,
        });
    }

    fn p4_pick(&mut self, rt: &mut CoreRt, player: EntityId, key: &str) -> u32 {
        let Some(mut p4) = rt.p4.take() else { return 0 };
        let out = self.p4_pick_in(rt, &mut p4, player, key);
        rt.p4 = Some(p4);
        out
    }

    fn p4_pick_in(&mut self, rt: &mut CoreRt, p4: &mut P4Rt, player: EntityId, key: &str) -> u32 {
        let cfg = rt.cfg.clone();
        let m = &cfg.mainframe;
        let now = self.now;
        let Some(t) = m.terminals.iter().position(|x| x == key) else {
            return 0;
        };
        let Some(pi) = self.index(player) else {
            return 0;
        };
        let judgment = self.raid.mode == IlmariMode::Judgment;
        if p4.stage != Stage::Revert
            || self.has_aura(pi, &m.merge_aura)
            || p4.terms_next[t] > now
            || (judgment && p4.pickers.contains(&player))
        {
            return 0;
        }
        p4.pickers.insert(player);
        if t == p4.culprit % TERMINALS {
            p4.stage = Stage::Final;
            p4.final_at = now + ticks(m.final_ms);
            p4.picker = Some(player);
            self.p4_remove_adds(rt);
            let lines = self.p4_log(p4, m);
            p4.log = Some(lines.clone());
            self.events.push(Event::CoreLog { lines });
            self.core_say_any(rt, Voice::Screen, &m.right_yells, 0);
            for (j, y) in m.final_yells.iter().enumerate() {
                self.core_say(rt, Voice::Screen, y, 3_000 * (j as u32 + 1));
            }
            return PROP_OK;
        }
        // A wrong pick reverts one of the two innocent rows this terminal holds.
        let row = t + if p4.culprit < TERMINALS { TERMINALS } else { 0 };
        let clock = self
            .ilmari
            .stamp
            .as_ref()
            .map_or_else(|| "the first commit".to_string(), |s| s.clock());
        let subject = commit_log(p4.culprit, &clock)
            .get(row)
            .map(|c| c.subject)
            .unwrap_or_default();
        let line = m.reverted.replace("{subject}", subject);
        self.core_say(rt, Voice::Screen, &line, 0);
        self.core_say_any(rt, Voice::Screen, &m.wrong_yells, 2_500);
        for &s in &rt.screens {
            if let Some(i) = self.index(s).filter(|&i| self.units[i].state.alive) {
                let by = u64::from(self.units[i].state.max_hp) * u64::from(m.wrong_heal_pct) / 100;
                self.heal(s, i, None, by.min(u64::from(u32::MAX)) as u32);
            }
        }
        let ms = if self.raid.story {
            m.story_merge_ms
        } else if judgment {
            LONG_MS
        } else {
            m.merge_ms
        };
        self.apply_aura(pi, &m.merge_aura, ms, None, Self::id_of(pi), None);
        p4.terms_next[t] = now + ticks(m.terminal_down_ms);
        p4.wrong += 1;
        p4.marked = true;
        let lines = self.p4_log(p4, m);
        p4.log = Some(lines.clone());
        self.events.push(Event::CoreLog { lines });
        if p4.wrong >= m.wrong_limit {
            self.p4_force_push(rt, p4);
        }
        1
    }

    /// Three wrong picks: a room-wide volley and the mainframe has to be kept up for another spell.
    fn p4_force_push(&mut self, rt: &mut CoreRt, p4: &mut P4Rt) {
        let cfg = rt.cfg.clone();
        let m = &cfg.mainframe;
        let now = self.now;
        let c = RemixCast {
            spell: m.force_spell,
            story: None,
            at_player: false,
            cast: false,
            delay_ms: 0,
            full_only: false,
        };
        // Ilmari wakes from the revert prompt to push.
        self.core_hold_screens(rt, false);
        self.core_cast_as(rt, &c, true);
        self.core_say_any(rt, Voice::Screen, &m.force_yells, 1_500);
        p4.stage = Stage::Protect;
        p4.until = now + ticks(m.force_protect_ms);
        p4.wrong = 0;
        p4.next_wave = now + ticks(m.wave_first_ms);
        self.core_start_p4_abilities(rt);
        for d in &mut p4.terms_next {
            *d = 0;
        }
    }

    /// The beat is over: the mainframe and the rewrites go first (a dead ward would wipe the
    /// room), then the core falls to the one who made the pick.
    fn p4_finish(&mut self, rt: &mut CoreRt, p4: &mut P4Rt) {
        p4.stage = Stage::Done;
        self.core_clear_mainframe(rt);
        for &s in &rt.screens {
            self.cancel_telegraphs_of(s);
        }
        let killer = p4.picker;
        if let Some(bi) = self.index(rt.boss).filter(|&b| self.units[b].state.alive) {
            self.die(bi, killer);
        }
    }

    /// The screens freeze (or wake): the P2 hold aura, a stun that stops their casts and swings.
    fn core_hold_screens(&mut self, rt: &CoreRt, on: bool) {
        for &s in &rt.screens {
            let Some(i) = self.index(s).filter(|&i| self.units[i].state.alive) else {
                continue;
            };
            if on {
                self.apply_aura(i, &rt.cfg.hold_aura, LONG_MS, None, s, None);
            } else {
                self.remove_aura(i, &rt.cfg.hold_aura, crate::types::AuraEnd::Expired);
            }
        }
    }

    /// The abilities that belong to P4 (the ones gated to phase 4 on the screens) start their
    /// initial delay now, not at the pull: the same rule `check_phase` keeps for its own phases.
    fn core_start_p4_abilities(&mut self, rt: &CoreRt) {
        for &s in &rt.screens {
            let Some(i) = self.index(s) else { continue };
            let Some(ai) = self.ai_of(i) else { continue };
            for (k, ab) in ai.abilities.iter().enumerate() {
                if !ab.phases.contains(&4) {
                    continue;
                }
                let (lo, hi) = ab
                    .initial_ms
                    .unwrap_or((ab.cooldown_min_ms, ab.cooldown_max_ms));
                let ms = self.rng.range_inclusive(lo, hi);
                if let Some(slot) = self.units[i].ability_next.get_mut(k) {
                    *slot = self.now + ticks(ms);
                }
            }
        }
    }

    /// The mainframe and its rewrites go (the fight ends, or the room resets).
    pub(super) fn core_clear_mainframe(&mut self, rt: &CoreRt) {
        let m = &rt.cfg.mainframe;
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.despawned || !u.state.alive {
                continue;
            }
            if u.state.key == m.unit || u.state.key == m.add || m.clears.contains(&u.state.key) {
                self.despawn(i);
            }
        }
    }

    /// (stage, clock in seconds, the mainframe's health in percent, wrong picks).
    pub(super) fn mainframe_wall(&self, rt: &CoreRt) -> (u8, u8, u8, u8) {
        let Some(p4) = rt.p4.as_ref().filter(|_| rt.phase == 4) else {
            return (0, 0, 0, 0);
        };
        let ward_pct = self.index(p4.ward).map_or(0, |w| {
            let s = &self.units[w].state;
            (u64::from(s.hp) * 100 / u64::from(s.max_hp.max(1))).min(100) as u8
        });
        let clock = if p4.stage == Stage::Protect {
            super::secs_left(p4.until, self.now)
        } else {
            0
        };
        (p4.stage as u8, clock, ward_pct, p4.wrong)
    }

    /// A punch card: the ward heals a little, once per card per cooldown, any class.
    pub(crate) fn use_punch_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let key = self.content.world.props[k].key.clone();
        let now = self.now;
        let mut outcome = 0;
        if let Some(mut rt) = self.core.rt.take() {
            let cfg = rt.cfg.clone();
            let m = &cfg.mainframe;
            let ward = self.core_ward(&rt);
            if let (Some(p4), Some(w), Some(slot)) = (
                rt.p4.as_mut().filter(|p| p.stage == Stage::Protect),
                ward,
                m.cards.iter().position(|c| *c == key),
            ) && p4.cards_next[slot] <= now
            {
                p4.cards_next[slot] = now + ticks(m.card_cooldown_ms);
                let by = u64::from(self.units[w].state.max_hp) * u64::from(m.card_heal_pct) / 100;
                self.heal(player, w, None, by.min(u64::from(u32::MAX)) as u32);
                outcome = PROP_OK;
            }
            self.core.rt = Some(rt);
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: key,
            outcome,
        });
    }

    /// The core fell: every member gets this clear's fragments (Full one, Judgment more, Story none).
    pub(super) fn core_pay_fragments(&mut self) {
        let r = &self.content.ilmari.rewards;
        let n = r.fragments_for(self.raid.mode, self.content.ilmari.judgment.fragment_mult);
        let name = r.fragment.clone();
        if n == 0 {
            return;
        }
        for p in self.players.clone() {
            let Some(pi) = self.index(p) else { continue };
            let held = self.token_count(pi, &name);
            let n = n.min(TOKEN_STACK_CAP.saturating_sub(held));
            if n == 0 {
                continue;
            }
            let inv = &mut self.units[pi].state.inventory;
            match inv.iter_mut().find(|(m, _)| *m == name) {
                Some((_, c)) => *c += n,
                None => inv.push((name.clone(), n)),
            }
            self.bag_event(pi);
        }
    }
}
