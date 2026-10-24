//! Companions: cosmetic pets that follow their owner. They add flavour, never combat power;
//! the one mechanical effect is the Roomba sweeping the owner's grey junk off nearby corpses.
//! A pet is learned by using its carrier (a consumable sold by vendors or dropped by enemies),
//! or granted outright (Golden Unicorn for delivering a high Sprint). Content: `pets` in
//! `game.ron`.

use crate::content::{Content, ticks};
use crate::effects::dist_xz;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{Event, ItemError, UnitState};
use serde::Deserialize;

/// The pet that sweeps up junk.
pub const ROOMBA: &str = "roomba";
/// The pet that gives real advice.
pub const RUBBER_DUCK: &str = "rubber_duck";
/// Corpses within this distance of the owner get swept.
pub const ROOMBA_RANGE_M: f32 = 12.0;
/// How often the Roomba makes a pass.
pub const ROOMBA_PERIOD_MS: u32 = 1_000;

#[derive(Clone, Debug, Deserialize)]
pub struct PetDrop {
    pub unit: String,
    pub chance_pct: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PetDef {
    pub key: String,
    pub name: String,
    /// The consumable that teaches this pet when used ("" for granted-only pets).
    #[serde(default)]
    pub carrier: String,
    /// Where to get it, for the pet journal.
    pub source: String,
    pub blurb: String,
    /// Kills of these units roll the carrier into each looter's own share.
    #[serde(default)]
    pub drops: Vec<PetDrop>,
    /// Granted to every member delivering a Sprint at this level or higher.
    #[serde(default)]
    pub sprint_level: Option<u8>,
    /// Left out of the journal until owned (the Ilmari reward pet, guardrail 1).
    #[serde(default)]
    pub hidden: bool,
    /// Things it says now and then (the client picks them).
    #[serde(default)]
    pub lines: Vec<String>,
}

/// Unique keys, carriers that are consumables, drops from real units.
pub(crate) fn validate(c: &Content) -> Result<(), crate::content::ContentError> {
    let bad = |m: String| Err(crate::content::ContentError::Invalid(m));
    for (i, p) in c.pets.iter().enumerate() {
        if c.pets[..i].iter().any(|o| o.key == p.key) || p.key.is_empty() {
            return bad(format!("duplicate or empty pet {}", p.key));
        }
        if !p.carrier.is_empty() && c.economy.consumable(&p.carrier).is_none() {
            return bad(format!(
                "pet {} carrier {} is no consumable",
                p.key, p.carrier
            ));
        }
        if let Some(d) = p.drops.iter().find(|d| c.unit_def(&d.unit).is_none()) {
            return bad(format!("pet {} drops from unknown unit {}", p.key, d.unit));
        }
        if !p.drops.is_empty() && p.carrier.is_empty() {
            return bad(format!("pet {} drops without a carrier", p.key));
        }
    }
    Ok(())
}

impl Content {
    pub fn pet(&self, key: &str) -> Option<&PetDef> {
        self.pets.iter().find(|p| p.key == key)
    }

    /// The pet a carrier consumable teaches.
    pub fn pet_for_carrier(&self, item: &str) -> Option<&PetDef> {
        self.pets
            .iter()
            .find(|p| !p.carrier.is_empty() && p.carrier == item)
    }
}

impl Sim {
    /// Adds `key` to the player's collection; false for unknown pets, non-players or a pet
    /// already owned.
    pub fn grant_pet(&mut self, player: EntityId, key: &str) -> bool {
        let Some(i) = self.index(player).filter(|_| self.is_player(player)) else {
            return false;
        };
        if self.content.pet(key).is_none() || self.units[i].state.pets.iter().any(|p| p == key) {
            return false;
        }
        self.units[i].state.pets.push(key.to_string());
        self.emit_to(
            player,
            Event::PetLearned {
                unit: player,
                pet: key.to_string(),
            },
        );
        true
    }

    /// Owned pets of a player, oldest first.
    pub fn pets_of(&self, player: EntityId) -> &[String] {
        self.unit(player).map_or(&[][..], |u| &u.pets)
    }

    /// "" dismisses. Unknown or unowned pets are ignored.
    pub(crate) fn summon_pet(&mut self, player: EntityId, key: &str) {
        let Some(i) = self.index(player).filter(|_| self.is_player(player)) else {
            return;
        };
        let s = &self.units[i].state;
        if s.pet == key || !(key.is_empty() || s.pets.iter().any(|p| p == key)) {
            return;
        }
        self.units[i].state.pet = key.to_string();
        self.events.push(Event::PetChanged {
            unit: player,
            pet: key.to_string(),
        });
    }

    /// Using a carrier: learn the pet (summoning it right away), or refuse if already owned.
    /// `None` when `item` is not a carrier.
    pub(crate) fn use_carrier(&mut self, player: EntityId, item: &str) -> Option<bool> {
        let key = self.content.pet_for_carrier(item)?.key.clone();
        if !self.grant_pet(player, &key) {
            self.item_error(player, ItemError::NotEligible);
            return Some(false);
        }
        self.summon_pet(player, &key);
        Some(true)
    }

    /// Carriers this kill rolls into `owner`'s share. Rolled on a stream of their own, keyed by
    /// corpse, tick and owner, so pets never shift combat or loot replays.
    pub(crate) fn pet_drops(&self, ti: usize, owner: Option<EntityId>) -> Vec<String> {
        let Some(owner) = owner else {
            return Vec::new();
        };
        let key = &self.content.units[self.units[ti].def].key;
        let mut out = Vec::new();
        for p in &self.content.pets {
            for d in p.drops.iter().filter(|d| d.unit == *key) {
                let salt = (Self::id_of(ti) << 32) ^ (owner << 16) ^ self.now;
                let mut rng = crate::rng::Rng::new(self.seed ^ 0x9E7_D0C5 ^ salt);
                if rng.chance_pct(d.chance_pct) {
                    out.push(p.carrier.clone());
                }
            }
        }
        out
    }

    /// Golden Unicorn and friends: pets for delivering a Sprint at `level`.
    pub(crate) fn sprint_pets(&mut self, player: EntityId, level: u8) {
        let keys: Vec<String> = self
            .content
            .pets
            .iter()
            .filter(|p| p.sprint_level.is_some_and(|l| level >= l))
            .map(|p| p.key.clone())
            .collect();
        for k in keys {
            self.grant_pet(player, &k);
        }
    }

    /// Debug/test: adds `item` to every loot share on a corpse. False if there are none.
    pub fn debug_add_loot(&mut self, corpse: EntityId, item: &str) -> bool {
        let Some(ci) = self.index(corpse) else {
            return false;
        };
        for l in &mut self.units[ci].loot {
            l.items.push(item.to_string());
        }
        !self.units[ci].loot.is_empty()
    }

    /// Debug/test: the items of `owner`'s share on a corpse.
    pub fn debug_loot_of(&self, corpse: EntityId, owner: EntityId) -> Vec<String> {
        self.index(corpse)
            .and_then(|ci| {
                self.units[ci]
                    .loot
                    .iter()
                    .find(|l| l.owner == Some(owner))
                    .map(|l| l.items.clone())
            })
            .unwrap_or_default()
    }

    /// The Roomba's pass: grey junk from the owner's own share on corpses in range.
    pub(crate) fn update_pets(&mut self) {
        if !self.now.is_multiple_of(ticks(ROOMBA_PERIOD_MS).max(1)) {
            return;
        }
        for p in self.players.clone() {
            let Some(pi) = self.index(p) else { continue };
            let u = &self.units[pi].state;
            if u.pet != ROOMBA || !u.alive {
                continue;
            }
            let at = u.pos;
            for ci in 0..self.units.len() {
                let c = &self.units[ci];
                if c.despawned || c.state.alive || !c.state.lootable {
                    continue;
                }
                if dist_xz(c.state.pos, at) > ROOMBA_RANGE_M {
                    continue;
                }
                self.sweep(pi, ci);
            }
        }
    }

    fn sweep(&mut self, pi: usize, ci: usize) {
        let owner = Self::id_of(pi);
        let Some(k) = self.units[ci]
            .loot
            .iter()
            .position(|l| l.owner == Some(owner))
        else {
            return;
        };
        let eco = &self.content.economy;
        let free = eco
            .bag_size
            .max(1)
            .saturating_sub(self.units[pi].state.bag.len());
        let share = &mut self.units[ci].loot[k];
        let mut junk = Vec::new();
        share.items.retain(|n| {
            let take = junk.len() < free && eco.junk.iter().any(|j| j.name == *n);
            if take {
                junk.push(n.clone());
            }
            !take
        });
        if junk.is_empty() {
            return;
        }
        let emptied = share.items.is_empty() && share.sp == 0;
        if emptied {
            self.units[ci].loot.remove(k);
        }
        self.units[pi].state.bag.extend(junk.iter().cloned());
        self.units[ci].state.lootable = !self.units[ci].loot.is_empty();
        self.refresh_corpse(ci);
        self.bag_event(pi);
        self.emit_to(
            owner,
            Event::PetLooted {
                unit: owner,
                corpse: Self::id_of(ci),
                items: junk,
                emptied,
            },
        );
    }
}

/// The Rubber Duck's advice for the fight `me` is in, read off the current state: the most
/// urgent thing first. `None` out of combat.
pub fn duck_tip(content: &Content, me: &UnitState, target: Option<&UnitState>) -> Option<String> {
    if !me.alive || !me.in_combat {
        return None;
    }
    let foe = target.filter(|t| t.alive && t.hostile);
    if let Some((t, c)) = foe.and_then(|t| t.cast.map(|c| (t, c)))
        && c.interruptible
    {
        let spell = content.spell(c.spell).map_or("that", |s| s.name.as_str());
        return Some(format!(
            "Interrupt the {}. {spell} is not going to cast itself... yet.",
            t.name
        ));
    }
    if me.max_hp > 0 && me.hp * 100 < me.max_hp * 35 {
        return Some(format!(
            "You're at {}% HP. Heal, step back, or update your LinkedIn.",
            me.hp * 100 / me.max_hp
        ));
    }
    if let Some(a) = me.auras.iter().find(|a| a.harmful) {
        return Some(format!("You still have {} on you. Get rid of it.", a.name));
    }
    if me.max_mana > 0 && me.mana * 100 < me.max_mana * 20 {
        return Some("Low mana. Finish this, then Coffee Break. Not the other way round.".into());
    }
    match foe {
        Some(t) if t.max_hp > 0 && t.hp * 100 < t.max_hp * 20 => {
            Some(format!("{} is almost done. Like the sprint. Push.", t.name))
        }
        Some(t) if t.boss => Some(format!(
            "{} hits harder in later phases. Save your cooldowns.",
            t.name
        )),
        Some(t) => Some(format!(
            "Explain the {} to me out loud. Slowly. Then hit it.",
            t.name
        )),
        None => Some("You have no target. Press Tab. That is the whole tip.".into()),
    }
}
