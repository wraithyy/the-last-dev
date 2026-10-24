//! Offers (docs/ILMARI_1_0.md D.3 "The offer", D.9 item 2): the Alignment Consultant's
//! "guardrails", sold as buffs that are secretly debuffs. `Effect::Offer` gives every player in
//! range a pending aura (`ok_aura`, it reads like a buff) for `accept_ms`. Silence is consent: if
//! the window runs out the real aura (`bad_aura`) lands and the boss gains a stack of `Aligned`.
//! Refusing is an action: the `PropKind::OptOut` kiosk cancels the user's own pending offers, and
//! the first refusal of a wave staggers the boss while each one stacks `Pushback` on it. A
//! `Dispel` on an accepted guardrail takes its `Aligned` stack back. Companions are not
//! `Sim::players`, so they are never offered anything. Numbers: docs/FORMULAS.md, Offers.

use crate::content::ticks;
use crate::content::{Content, ContentError};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{AuraEnd, Event};
use serde::Deserialize;

/// `Aligned` and `Pushback` last "until the fight ends"; evade and death clear every aura.
const LONG_MS: u32 = 3_600_000;

fn bad_ms() -> u32 {
    LONG_MS
}

/// What a wave of offers does; the fields of `Effect::Offer`.
#[derive(Clone, Debug, Deserialize)]
pub struct OfferDef {
    /// The pending aura (a benefit on its face; its name carries the fine print on Story).
    pub ok_aura: String,
    /// The guardrail that lands if the window runs out unanswered.
    pub bad_aura: String,
    pub accept_ms: u32,
    /// How long the guardrail lasts (default: the fight).
    #[serde(default = "bad_ms")]
    pub bad_ms: u32,
    /// Stacked on the offering boss for every accepted guardrail (not on Story).
    #[serde(default)]
    pub aligned_aura: Option<String>,
    /// Stacked on the boss for every refusal.
    #[serde(default)]
    pub pushback_aura: Option<String>,
    /// The boss's own stun for the first refusal of a wave.
    #[serde(default)]
    pub stagger_aura: Option<String>,
    #[serde(default)]
    pub stagger_ms: u32,
    /// Tenths of a percent of the boss's max HP each `Aligned` stack heals it, every 5 s.
    #[serde(default)]
    pub heal_tenths: u32,
    /// Judgment's rider: while the boss wears this aura the kiosk's per-player cooldown is `.1` ms.
    #[serde(default)]
    pub rider_kiosk: Option<(String, u32)>,
    /// Each wave moves the Opt-Out Kiosks to the other site ("the policy has been updated"):
    /// props with `alt: true` work only while the room is on the second site.
    #[serde(default)]
    pub relocate: bool,
}

impl OfferDef {
    pub(crate) fn auras(&self) -> Vec<&str> {
        [&self.ok_aura, &self.bad_aura]
            .into_iter()
            .map(String::as_str)
            .chain(self.aligned_aura.as_deref())
            .chain(self.pushback_aura.as_deref())
            .chain(self.stagger_aura.as_deref())
            .chain(self.rider_kiosk.iter().map(|(a, _)| a.as_str()))
            .collect()
    }

    pub(crate) fn validate(&self, c: &Content, spell: &str) -> Result<(), ContentError> {
        let known = self.auras().into_iter().all(|a| c.aura(a).is_some());
        let honest = c.aura(&self.bad_aura).is_some_and(|a| a.harmful);
        if self.accept_ms == 0
            || self.bad_ms == 0
            || self.ok_aura == self.bad_aura
            || !known
            || !honest
            || self.stagger_aura.is_some() != (self.stagger_ms > 0)
        {
            return Err(ContentError::Invalid(format!(
                "spell {spell} offers known auras with a window, a harmful guardrail and a stagger that has both aura and time"
            )));
        }
        Ok(())
    }
}

/// An offer a player has not answered yet.
struct Pending {
    player: EntityId,
    boss: EntityId,
    until: u64,
    def: OfferDef,
}

/// A guardrail a player wears because they accepted (or did not read).
struct Worn {
    player: EntityId,
    boss: EntityId,
    key: String,
    aligned: Option<String>,
}

/// Aligned heals tick this often (5 s).
const HEAL_TICKS: u64 = 150;

#[derive(Default)]
pub(crate) struct OfferState {
    pending: Vec<Pending>,
    worn: Vec<Worn>,
    /// The first refusal of the current wave staggers the boss; the rest only stack Pushback.
    wave_refused: bool,
    /// Guardrails accepted (or not read) in the pull that began at tick `.0`.
    accepted: (u64, u32),
    /// Per offering boss: (aligned aura, heal tenths per stack) and the next heal tick.
    heals: Vec<(EntityId, String, u32)>,
    next_heal: u64,
    /// The kiosks stand on their second site (`OfferDef::relocate`).
    kiosk_alt: bool,
}

impl OfferState {
    /// Guardrails the room has accepted in the pull that began at `since` (0 when it is another).
    pub(crate) fn accepted_in(&self, since: u64) -> u32 {
        if self.accepted.0 == since {
            self.accepted.1
        } else {
            0
        }
    }
}

impl Sim {
    /// A fresh zone: no offer outlives the room it was made in.
    pub(crate) fn offers_on_load(&mut self) {
        self.offers = OfferState::default();
    }

    /// A joiner is told where the kiosks stand (only when they are not on their first site).
    pub(crate) fn offers_on_join(&mut self, player: EntityId) {
        if self.offers.kiosk_alt {
            self.emit_to(player, Event::Kiosks { alt: true });
        }
    }

    /// The pull ended: the kiosks go back to their first site.
    pub(crate) fn offers_reset_kiosks(&mut self) {
        if self.offers.kiosk_alt {
            self.offers.kiosk_alt = false;
            self.events.push(Event::Kiosks { alt: false });
        }
    }

    /// Guardrails accepted in the pull that began at `since`.
    pub(crate) fn offers_accepted(&self, since: u64) -> u32 {
        self.offers.accepted_in(since)
    }

    /// Whether any wave of offers was made in the pull that began at `since`.
    pub(crate) fn offers_waves_seen(&self, since: u64) -> bool {
        since > 0 && self.offers.accepted.0 == since
    }

    /// `Effect::Offer`: every living player within `range_m` of the boss gets the pending aura.
    pub(crate) fn offer_wave(&mut self, ci: usize, range_m: f32, def: &OfferDef) {
        let boss = Self::id_of(ci);
        let me = self.units[ci].state.pos;
        self.offers.wave_refused = false;
        if def.relocate {
            self.offers.kiosk_alt = !self.offers.kiosk_alt;
            self.events.push(Event::Kiosks {
                alt: self.offers.kiosk_alt,
            });
        }
        let since = self.units[ci].combat_since;
        if self.offers.accepted.0 != since {
            self.offers.accepted = (since, 0);
        }
        for p in self.players.clone() {
            let Some(pi) = self.index(p) else { continue };
            let u = &self.units[pi].state;
            if !u.alive || crate::geom::distance(u.pos, me) > range_m.max(1.0) {
                continue;
            }
            // A new offer replaces an unanswered one of the same kind rather than doubling it.
            if self
                .offers
                .pending
                .iter()
                .any(|o| o.player == p && o.def.ok_aura == def.ok_aura)
            {
                self.offers
                    .pending
                    .retain(|o| !(o.player == p && o.def.ok_aura == def.ok_aura));
                self.remove_aura(pi, &def.ok_aura, AuraEnd::Consumed);
            }
            self.apply_aura(pi, &def.ok_aura, def.accept_ms, None, boss, None);
            self.offers.pending.push(Pending {
                player: p,
                boss,
                until: self.now + ticks(def.accept_ms),
                def: def.clone(),
            });
        }
    }

    /// Windows that ran out convert: the guardrail lands and the boss grows. Offers whose player
    /// died or whose boss is gone just lapse.
    pub(crate) fn update_offers(&mut self) {
        self.aligned_heal();
        if self.offers.pending.is_empty() && self.offers.worn.is_empty() {
            return;
        }
        let now = self.now;
        let (due, keep): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut self.offers.pending)
            .into_iter()
            .partition(|o| o.until <= now);
        self.offers.pending = keep;
        for o in due {
            let (Some(pi), Some(bi)) = (self.index(o.player), self.index(o.boss)) else {
                continue;
            };
            self.remove_aura(pi, &o.def.ok_aura, AuraEnd::Consumed);
            if !self.units[pi].state.alive || !self.units[bi].state.alive {
                continue;
            }
            self.apply_aura(pi, &o.def.bad_aura, o.def.bad_ms, None, o.boss, None);
            let since = self.units[bi].combat_since;
            if self.offers.accepted.0 != since {
                self.offers.accepted = (since, 0);
            }
            self.offers.accepted.1 += 1;
            if let (Some(a), true) = (&o.def.aligned_aura, o.def.heal_tenths > 0)
                && !self.offers.heals.iter().any(|(b, _, _)| *b == o.boss)
            {
                self.offers
                    .heals
                    .push((o.boss, a.clone(), o.def.heal_tenths));
            }
            // Story's offers are honest: nothing is fed to the boss.
            if let Some(a) = o.def.aligned_aura.as_deref().filter(|_| !self.raid.story) {
                self.apply_aura(bi, a, LONG_MS, None, o.boss, None);
            }
            self.offers.worn.push(Worn {
                player: o.player,
                boss: o.boss,
                key: o.def.bad_aura.clone(),
                aligned: o.def.aligned_aura.clone(),
            });
        }
        // A guardrail that is no longer on its wearer was dispelled, ran out or died with them.
        let worn = std::mem::take(&mut self.offers.worn);
        for w in worn {
            let on = self
                .index(w.player)
                .is_some_and(|pi| self.has_aura(pi, &w.key));
            if on {
                self.offers.worn.push(w);
            }
        }
    }

    /// Every accepted guardrail feeds the offering boss: it heals a share of its max HP per
    /// `Aligned` stack every 5 s (Story feeds it nothing, so the stacks never exist there).
    fn aligned_heal(&mut self) {
        if self.offers.heals.is_empty() {
            return;
        }
        if self.offers.next_heal == 0 {
            self.offers.next_heal = self.now + HEAL_TICKS;
        }
        if self.now < self.offers.next_heal {
            return;
        }
        self.offers.next_heal = self.now + HEAL_TICKS;
        for (boss, aligned, tenths) in self.offers.heals.clone() {
            let Some(bi) = self.index(boss).filter(|&b| self.units[b].state.alive) else {
                continue;
            };
            let stacks = self.units[bi]
                .auras
                .iter()
                .find(|a| a.key == aligned)
                .map_or(0, |a| u64::from(a.stacks));
            let amount = u64::from(self.units[bi].state.max_hp) * u64::from(tenths) * stacks / 1000;
            if amount > 0 {
                self.heal(boss, bi, None, amount as u32);
            }
        }
    }

    /// `Effect::ClearOffers`: the pending offers of every living player within `radius_m` of the
    /// caster lapse without a refusal.
    pub(crate) fn offers_clear_near(&mut self, ci: usize, radius_m: f32) {
        let me = self.units[ci].state.pos;
        for p in self.players.clone() {
            let Some(pi) = self.index(p) else { continue };
            let u = &self.units[pi].state;
            if !u.alive || crate::geom::distance(u.pos, me) > radius_m {
                continue;
            }
            let (mine, rest): (Vec<Pending>, Vec<Pending>) =
                std::mem::take(&mut self.offers.pending)
                    .into_iter()
                    .partition(|o| o.player == p);
            self.offers.pending = rest;
            for o in mine {
                self.remove_aura(pi, &o.def.ok_aura, AuraEnd::Consumed);
            }
        }
    }

    /// The opt-out kiosk: cancels the player's own pending offers. Each refusal stacks Pushback
    /// on the offering boss; the first of the wave also staggers it. Returns how many it cancelled.
    pub(crate) fn offers_cancel(&mut self, player: EntityId) -> usize {
        let (mine, rest): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut self.offers.pending)
            .into_iter()
            .partition(|o| o.player == player);
        self.offers.pending = rest;
        let n = mine.len();
        for o in mine {
            if let Some(pi) = self.index(o.player) {
                self.remove_aura(pi, &o.def.ok_aura, AuraEnd::Consumed);
            }
            let Some(bi) = self.index(o.boss).filter(|&b| self.units[b].state.alive) else {
                continue;
            };
            if let Some(a) = &o.def.pushback_aura {
                self.apply_aura(bi, a, LONG_MS, None, player, None);
            }
            if !self.offers.wave_refused
                && let Some(a) = &o.def.stagger_aura
            {
                self.offers.wave_refused = true;
                self.apply_aura(bi, a, o.def.stagger_ms, None, o.boss, None);
            }
        }
        n
    }

    /// Hook of `remove_aura`: a dispelled guardrail takes its `Aligned` stack back. Running out
    /// or dying with the wearer only ends the record.
    pub(crate) fn offers_on_removed(&mut self, ti: usize, key: &str, reason: AuraEnd) {
        let player = Self::id_of(ti);
        let Some(k) = self
            .offers
            .worn
            .iter()
            .position(|w| w.player == player && w.key == key)
        else {
            return;
        };
        let w = self.offers.worn.remove(k);
        if reason != AuraEnd::Consumed {
            return;
        }
        let (Some(bi), Some(aligned)) = (self.index(w.boss), w.aligned) else {
            return;
        };
        let stacks = self.units[bi]
            .auras
            .iter()
            .find(|a| a.key == aligned)
            .map(|a| a.stacks);
        match stacks {
            Some(s) if s > 1 => {
                if let Some(a) = self.units[bi].auras.iter_mut().find(|a| a.key == aligned) {
                    a.stacks = s - 1;
                }
            }
            Some(_) => {
                self.remove_aura(bi, &aligned, AuraEnd::Consumed);
            }
            None => {}
        }
    }

    /// The `PropKind::OptOut` kiosk: a cooldown per player, then the cancel. Anyone can use it.
    pub(crate) fn use_optout_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        let mut outcome = 0;
        let rider = self
            .offers
            .pending
            .iter()
            .find(|o| o.player == player)
            .and_then(|o| {
                let (aura, ms) = o.def.rider_kiosk.as_ref()?;
                let bi = self.index(o.boss)?;
                self.has_aura(bi, aura).then_some(*ms)
            });
        let cooldown = rider.map_or(def.cooldown_ms, |ms| ms.max(def.cooldown_ms));
        let mine = self.offers.pending.iter().any(|o| o.player == player);
        // A kiosk of the other site is switched off.
        if mine
            && def.alt == self.offers.kiosk_alt
            && self.prop_ready(player, k, cooldown)
            && self.offers_cancel(player) > 0
        {
            outcome = crate::world::PROP_OK;
        }
        self.events.push(crate::types::Event::Interacted {
            unit: player,
            prop: def.key,
            outcome,
        });
    }
}
