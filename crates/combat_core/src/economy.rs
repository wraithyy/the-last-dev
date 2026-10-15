//! Economy: gear (five visible slots, rarity, role-weighted stats, perks, sets), Kudos (`sp`),
//! junk, consumables and vendors. Content lives in `Content::economy`; per-player state lives in
//! `UnitState` (`sp`, `bag`, `equipped`, `buyback`). See docs/FORMULAS.md, Gear.

use crate::content::{
    ContentError, ITEM_DESC_MAX, ItemBonus, ItemTotals, Objective, Role, SpellId, SpellTarget,
    ticks,
};
use crate::geom::in_range;
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{Event, ItemError};
use serde::{Deserialize, Serialize};

/// Equipment slots, in `UnitState::equipped` order.
pub const SLOTS: usize = 5;
/// Longest gear, set or perk text; the tooltip wraps it.
pub const GEAR_TEXT_MAX: usize = 160;
/// Hard cap on the damage reduction gear can give, in tenths of a percent.
pub const ARMOR_CAP_TENTHS: u32 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slot {
    Headgear,
    Hoodie,
    Keyboard,
    Mug,
    Lanyard,
}

impl Slot {
    pub const ALL: [Slot; SLOTS] = [
        Slot::Headgear,
        Slot::Hoodie,
        Slot::Keyboard,
        Slot::Mug,
        Slot::Lanyard,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Slot::Headgear => "Headgear",
            Slot::Hoodie => "Hoodie",
            Slot::Keyboard => "Keyboard",
            Slot::Mug => "Mug",
            Slot::Lanyard => "Lanyard",
        }
    }

    /// The keyboard is the "weapon": it carries a quarter more stats.
    fn budget_pct(self) -> u64 {
        if self == Slot::Keyboard { 125 } else { 100 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
    ];

    /// Dev-speak rarity name shown under the item name.
    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "Works On My Machine",
            Rarity::Uncommon => "Passed CI",
            Rarity::Rare => "Merged",
            Rarity::Epic => "Shipped to Prod",
            Rarity::Legendary => "Approved in Code Review",
        }
    }

    /// Stat budget multiplier in percent.
    pub fn quality(self) -> u64 {
        [40, 70, 100, 130, 160][self as usize]
    }

    fn price_base(self) -> u32 {
        [8, 20, 50, 150, 400][self as usize]
    }
}

/// What a piece of gear (or a 4-piece set) changes about how you play. `spell: None` means
/// every player spell. Item spells (`cast`) are ordinary content spells resolved without mana,
/// cooldown or cast bar; see `Sim::fire_item_spell`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Perk {
    /// Damage of the spell(s) is raised by `pct`.
    SpellDamage { spell: Option<SpellId>, pct: u32 },
    /// Healing of the spell(s) is raised by `pct`.
    SpellHeal { spell: Option<SpellId>, pct: u32 },
    /// The spell's cooldown is `pct` shorter.
    Cooldown { spell: SpellId, pct: u32 },
    /// The spell(s) cost `pct` less mana (negative: more).
    ManaCost { spell: Option<SpellId>, pct: i32 },
    /// A landed damaging hit repeats at `pct` damage with `chance_pct`, while the caster is at
    /// or above `min_hp_pct` HP.
    Echo {
        spell: Option<SpellId>,
        chance_pct: u32,
        pct: u32,
        #[serde(default)]
        min_hp_pct: u32,
    },
    /// After the spell resolves, `cast` also fires at the same target (or around the caster).
    Rider { spell: SpellId, cast: SpellId },
    /// Each successful cast of the spell(s) fires `cast` with `chance_pct`.
    OnCast {
        spell: Option<SpellId>,
        chance_pct: u32,
        cast: SpellId,
        #[serde(default)]
        icd_ms: u32,
    },
    /// Taking damage fires `cast` on yourself with `chance_pct`.
    OnHurt {
        chance_pct: u32,
        cast: SpellId,
        #[serde(default)]
        icd_ms: u32,
    },
    /// Spell damage (of `spell` only, if given) heals you for `pct` of it.
    Leech {
        pct: u32,
        #[serde(default)]
        spell: Option<SpellId>,
    },
    /// Active: the gadget bar fires `cast`, then the item cools down.
    OnUse { cast: SpellId, cooldown_ms: u32 },
    // ---- Career Ladder talents (talents.rs); gear may use them too ----
    /// The spell's cast time changes by `pct` percent (-100: instant).
    CastTime { spell: Option<SpellId>, pct: i32 },
    /// Cast-time spells start and keep casting while moving (channels excluded).
    CastWhileMoving { spell: Option<SpellId> },
    /// The spell also lands on up to `count` more units of its target's side within `radius_m`
    /// of the target, at `pct` of the damage or healing. Enemies must already be fighting;
    /// heals pick injured allies only.
    Splash {
        spell: SpellId,
        radius_m: f32,
        count: u8,
        pct: u32,
    },
    /// Your killing blow resets `reset`'s cooldown and/or fires `cast`.
    OnKill {
        #[serde(default)]
        reset: Option<SpellId>,
        #[serde(default)]
        cast: Option<SpellId>,
    },
    /// Each cast of `on` (any spell for `None`) resets `reset`'s cooldown with `chance_pct`.
    Reset {
        on: Option<SpellId>,
        chance_pct: u32,
        reset: SpellId,
    },
    /// The bar spell casts `with` instead (its cost, cast time and cooldown); perks that name
    /// `spell` apply to `with` as well.
    Replace { spell: SpellId, with: SpellId },
    /// Periodic ticks of the auras the spell applies are `pct` stronger.
    Periodic { spell: SpellId, pct: u32 },
    /// Damage of the spell(s) refunds `pct` of it as mana.
    ManaReturn { spell: Option<SpellId>, pct: u32 },
    /// Mana costs are paid in HP instead: `pct` HP per 100 mana (never below 1 HP).
    HpCost { pct: u32 },
    /// All damage taken changes by `pct` percent.
    DamageTaken { pct: i32 },
    /// Damage against targets at or below `below_pct` HP is `pct` higher.
    Execute { below_pct: u32, pct: u32 },
    /// Healing on targets at or below `below_pct` HP is `pct` higher.
    Triage { below_pct: u32, pct: u32 },
    /// While you are at or below `below_pct` HP, your damage and healing are `pct` higher.
    Desperate { below_pct: u32, pct: u32 },
    /// Kudos from your kills are `pct` higher (your own share only).
    KudosFind { pct: u32 },
}

impl Perk {
    /// Every spell the perk names (validation and tooltips).
    pub fn spells(&self) -> Vec<SpellId> {
        match self {
            Perk::SpellDamage { spell, .. }
            | Perk::SpellHeal { spell, .. }
            | Perk::ManaCost { spell, .. }
            | Perk::Echo { spell, .. }
            | Perk::Leech { spell, .. }
            | Perk::CastTime { spell, .. }
            | Perk::CastWhileMoving { spell }
            | Perk::ManaReturn { spell, .. } => spell.iter().copied().collect(),
            Perk::Cooldown { spell, .. }
            | Perk::Splash { spell, .. }
            | Perk::Periodic { spell, .. } => vec![*spell],
            Perk::Rider { spell, cast } => vec![*spell, *cast],
            Perk::OnCast { spell, cast, .. } => spell.iter().copied().chain([*cast]).collect(),
            Perk::OnHurt { cast, .. } | Perk::OnUse { cast, .. } => vec![*cast],
            Perk::OnKill { reset, cast } => reset.iter().chain(cast.iter()).copied().collect(),
            Perk::Reset { on, reset, .. } => on.iter().copied().chain([*reset]).collect(),
            Perk::Replace { spell, with } => vec![*spell, *with],
            Perk::HpCost { .. }
            | Perk::DamageTaken { .. }
            | Perk::Execute { .. }
            | Perk::Triage { .. }
            | Perk::Desperate { .. }
            | Perk::KudosFind { .. } => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum GearSource {
    /// Random drops from mobs of the item's tier.
    #[default]
    World,
    /// Only vendors sell it.
    Vendor,
    /// This unit (key) drops it now and then: a boss at `legendary_pct` on top of its
    /// guaranteed piece, any other unit (elites, the night shift) at `named_pct`.
    Boss(String),
    /// Only the end-of-sprint chest hands it out (top tier like the rest of the chest).
    Sprint,
    /// Only Legacy Mode floors drop it (`legacy.rs`), whatever its item level.
    Legacy,
}

/// How the piece looks on the character: a tint and a model variant per slot.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct GearVisual {
    pub color: (f32, f32, f32),
    #[serde(default)]
    pub style: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GearDef {
    pub name: String,
    pub icon: String,
    pub slot: Slot,
    pub rarity: Rarity,
    /// Item level; tier = ceil(ilvl / 2) follows the zone level bands.
    pub ilvl: u8,
    /// Whose stat package it carries; `None` is a generalist mix.
    #[serde(default)]
    pub role: Option<Role>,
    #[serde(default)]
    pub set: Option<String>,
    #[serde(default)]
    pub perk: Option<Perk>,
    #[serde(default)]
    pub perk_text: String,
    #[serde(default)]
    pub source: GearSource,
    pub visual: GearVisual,
    /// Vendor price in SP; 0 derives it from rarity and item level.
    #[serde(default)]
    pub price: u32,
    pub description: String,
}

/// Per-slot stats at budget 100: max HP, mana, spell power, crit (tenths), haste (tenths),
/// mana per 5 s, armour (tenths), power (tenths). Also what a role values when comparing gear.
pub fn role_package(role: Option<Role>) -> [u64; 8] {
    match role {
        Some(Role::Ranged) => [0, 15, 6, 12, 20, 0, 0, 35],
        Some(Role::Melee) => [30, 0, 6, 18, 0, 0, 0, 35],
        Some(Role::Tank) => [70, 5, 0, 0, 0, 0, 35, 20],
        Some(Role::Healer) => [0, 35, 6, 0, 20, 8, 0, 35],
        None => [30, 18, 4, 0, 0, 0, 0, 25],
    }
}

/// Gear stats with haste still in tenths; `Sim::refresh_item_bonus` folds them into totals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GearStats {
    pub totals: ItemTotals,
    pub haste_tenths: u32,
}

impl GearStats {
    pub fn add(&mut self, o: &GearStats) {
        self.totals.add(&o.totals);
        self.haste_tenths += o.haste_tenths;
    }
}

impl GearDef {
    pub fn tier(&self) -> u8 {
        self.ilvl.div_ceil(2).max(1)
    }

    /// Stat budget: rarity quality times an item-level factor (70% at ilvl 0, 100% at 10),
    /// keyboards +25%. See docs/FORMULAS.md, Gear.
    pub fn budget(&self) -> u64 {
        let ilvl_pct = 70 + 3 * u64::from(self.ilvl);
        self.rarity.quality() * ilvl_pct * self.slot.budget_pct() / 10_000
    }

    pub fn stats(&self) -> GearStats {
        let b = self.budget();
        let p = role_package(self.role).map(|v| (v * b / 100) as u32);
        GearStats {
            totals: ItemTotals {
                max_hp: p[0],
                max_mana: p[1],
                spell_power: p[2],
                crit_tenths: p[3],
                haste_pct: 0,
                mana_regen: p[5],
                armor_tenths: p[6],
                power_tenths: p[7],
            },
            haste_tenths: p[4],
        }
    }

    pub fn price(&self) -> u32 {
        if self.price > 0 {
            return self.price;
        }
        // Linear in ilvl like zone income, so a tier's Rare costs one to two zones of play.
        self.rarity.price_base() * (3 * u32::from(self.ilvl) + 2)
    }

    /// Level a vendor wants before selling the piece: the bottom of its tier's zone band
    /// (ilvl - 1), one more for Epic and up. Drops ignore it.
    pub fn required_level(&self) -> u8 {
        let epic = u8::from(self.rarity >= Rarity::Epic);
        (self.ilvl.saturating_sub(1) + epic).max(1)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SetDef {
    pub key: String,
    pub name: String,
    pub two: ItemBonus,
    pub two_text: String,
    pub four: Perk,
    pub four_text: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JunkDef {
    pub name: String,
    pub icon: String,
    pub description: String,
    pub price: u32,
    /// Drops only from mobs of this gear tier; 0 drops anywhere.
    #[serde(default)]
    pub tier: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConsumableDef {
    pub name: String,
    pub icon: String,
    pub description: String,
    pub price: u32,
    /// Restores this percent of max mana.
    #[serde(default)]
    pub mana_pct: u32,
    /// Item spell fired on yourself (heal, dispel, buff).
    #[serde(default)]
    pub cast: Option<SpellId>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VendorDef {
    /// Friendly unit key that runs the shop.
    pub unit: String,
    /// Window title.
    pub title: String,
    pub greeting: String,
    pub stock: Vec<String>,
    /// `rotate_count` of these are on sale per `rotate_ms` window of sim time.
    #[serde(default)]
    pub rotating: Vec<String>,
    #[serde(default)]
    pub rotate_count: usize,
    #[serde(default)]
    pub rotate_ms: u32,
    /// Always listed, sold only to players a department likes enough.
    #[serde(default)]
    pub rep: Vec<RepGoods>,
}

/// A vendor item gated by department standing.
#[derive(Clone, Debug, Deserialize)]
pub struct RepGoods {
    pub item: String,
    /// `DepartmentDef::name`.
    pub dept: String,
    /// Index into `EndgameDef::rep_tiers`.
    pub tier: u8,
}

/// Anything that can sit in a bag.
#[derive(Clone, Copy, Debug)]
pub enum Goods<'a> {
    Gear(&'a GearDef),
    Junk(&'a JunkDef),
    Consumable(&'a ConsumableDef),
}

impl Goods<'_> {
    pub fn name(&self) -> &str {
        match self {
            Goods::Gear(g) => &g.name,
            Goods::Junk(j) => &j.name,
            Goods::Consumable(c) => &c.name,
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            Goods::Gear(g) => &g.icon,
            Goods::Junk(j) => &j.icon,
            Goods::Consumable(c) => &c.icon,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Goods::Gear(g) => &g.description,
            Goods::Junk(j) => &j.description,
            Goods::Consumable(c) => &c.description,
        }
    }

    pub fn price(&self) -> u32 {
        match self {
            Goods::Gear(g) => g.price(),
            Goods::Junk(j) => j.price,
            Goods::Consumable(c) => c.price,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Economy {
    pub bag_size: usize,
    pub buyback_size: usize,
    /// Gear and consumables sell for this percent of their price; junk for all of it.
    pub sell_pct: u32,
    pub vendor_range_m: f32,
    /// Kill SP: `level * sp_per_level + U[0, level]`, times the elite or boss multiplier.
    pub sp_per_level: u32,
    pub elite_mult: u32,
    pub boss_mult: u32,
    /// Unit keys that count as elites (mini-bosses) for drops.
    pub elites: Vec<String>,
    pub trash_gear_pct: u32,
    pub elite_gear_pct: u32,
    pub junk_pct: u32,
    /// Common, Uncommon, Rare, Epic weights.
    pub trash_rarity: [u32; 4],
    pub elite_rarity: [u32; 4],
    pub boss_rarity: [u32; 4],
    /// A boss with a `Boss` item drops it with this chance, on top of its guaranteed piece.
    pub legendary_pct: u32,
    /// A non-boss unit with a `Boss` item (elites, night shift) drops it with this chance.
    pub named_pct: u32,
    /// Chance a gear drop is picked from the looter's own role (else any role).
    pub own_role_pct: u32,
    /// Quest turn-in SP in percent of the quest XP.
    pub quest_sp_pct: u32,
    /// Shared cooldown of consumables.
    pub consumable_cd_ms: u32,
    pub gear: Vec<GearDef>,
    pub sets: Vec<SetDef>,
    pub junk: Vec<JunkDef>,
    pub consumables: Vec<ConsumableDef>,
    pub vendors: Vec<VendorDef>,
}

impl Economy {
    pub fn gear(&self, name: &str) -> Option<&GearDef> {
        self.gear.iter().find(|g| g.name == name)
    }

    /// The highest tier outside Legacy Mode: the Sprint chest's.
    pub fn campaign_top_tier(&self) -> u8 {
        self.gear
            .iter()
            .filter(|g| g.source != GearSource::Legacy)
            .map(GearDef::tier)
            .max()
            .unwrap_or(1)
    }

    pub fn set(&self, key: &str) -> Option<&SetDef> {
        self.sets.iter().find(|s| s.key == key)
    }

    pub fn goods(&self, name: &str) -> Option<Goods<'_>> {
        if let Some(g) = self.gear(name) {
            return Some(Goods::Gear(g));
        }
        if let Some(j) = self.junk.iter().find(|j| j.name == name) {
            return Some(Goods::Junk(j));
        }
        self.consumables
            .iter()
            .find(|c| c.name == name)
            .map(Goods::Consumable)
    }

    pub fn consumable(&self, name: &str) -> Option<&ConsumableDef> {
        self.consumables.iter().find(|c| c.name == name)
    }

    pub fn vendor_for(&self, unit_key: &str) -> Option<&VendorDef> {
        self.vendors.iter().find(|v| v.unit == unit_key)
    }

    /// What the vendor sells at sim time `now_ms`: the fixed stock plus the rotating window.
    pub fn stock(&self, v: &VendorDef, now_ms: u64) -> Vec<String> {
        let mut out = v.stock.clone();
        let n = v.rotating.len();
        if n > 0 && v.rotate_count > 0 && v.rotate_ms > 0 {
            let window = (now_ms / u64::from(v.rotate_ms)) as usize;
            let start = window * v.rotate_count % n;
            out.extend((0..v.rotate_count.min(n)).map(|k| v.rotating[(start + k) % n].clone()));
        }
        out.extend(v.rep.iter().map(|r| r.item.clone()));
        out
    }

    /// The (department index, tier) an item of this vendor needs; `None` when ungated.
    pub fn rep_gate(
        &self,
        v: &VendorDef,
        item: &str,
        content: &crate::Content,
    ) -> Option<(usize, u8)> {
        let r = v.rep.iter().find(|r| r.item == item)?;
        Some((content.endgame.department_index(&r.dept)?, r.tier))
    }

    pub fn sell_price(&self, name: &str) -> Option<u32> {
        Some(match self.goods(name)? {
            Goods::Junk(j) => j.price,
            g => (g.price() * self.sell_pct / 100).max(1),
        })
    }

    /// Equipped pieces of `set` among the equipped names.
    pub fn set_count(&self, set: &str, equipped: &[String]) -> usize {
        equipped
            .iter()
            .filter_map(|n| self.gear(n))
            .filter(|g| g.set.as_deref() == Some(set))
            .count()
    }

    /// Stats of the worn pieces plus set bonuses, and the perks they grant (the client shows
    /// the same numbers the sim uses).
    pub fn equipment(&self, equipped: &[String]) -> (GearStats, Vec<ActivePerk>) {
        let mut sum = GearStats::default();
        let mut perks = Vec::new();
        let mut sets: Vec<&str> = Vec::new();
        for g in equipped.iter().filter_map(|n| self.gear(n)) {
            sum.add(&g.stats());
            if let Some(p) = &g.perk {
                perks.push(ActivePerk {
                    source: g.name.clone(),
                    perk: p.clone(),
                });
            }
            if let Some(s) = g.set.as_deref()
                && !sets.contains(&s)
            {
                sets.push(s);
            }
        }
        for key in sets {
            let Some(set) = self.set(key) else { continue };
            let n = self.set_count(key, equipped);
            if n >= 2 {
                sum.totals.add(&set.two.totals(1));
            }
            if n >= 4 {
                perks.push(ActivePerk {
                    source: set.name.clone(),
                    perk: set.four.clone(),
                });
            }
        }
        (sum, perks)
    }

    /// Trophies plus equipment, as `Sim::item_totals` reports them.
    pub fn player_totals(
        &self,
        content: &crate::content::Content,
        trophies: &[(String, u32)],
        equipped: &[String],
    ) -> ItemTotals {
        let mut t = ItemTotals::default();
        for (name, n) in trophies {
            if let Some(item) = content.item(name) {
                t.add(&item.bonus.totals(*n));
            }
        }
        let (gear, _) = self.equipment(equipped);
        t.add(&gear.totals);
        t.haste_pct += gear.haste_tenths / 10;
        t.armor_tenths = t.armor_tenths.min(ARMOR_CAP_TENTHS);
        t
    }

    /// A rough per-role desirability used by bots and the upgrade arrow: the stat budget, full
    /// for the role's own package, 70% for generalist pieces, 35% for another role's.
    pub fn score(&self, g: &GearDef, role: Option<Role>) -> u64 {
        let fit = match (g.role, role) {
            (Some(a), Some(b)) if a == b => 100,
            (None, _) => 70,
            _ => 35,
        };
        let perk = if g.perk.is_some() { 115 } else { 100 };
        g.budget() * fit * perk / 100
    }

    pub(crate) fn validate(&self, content: &crate::content::Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        let names: Vec<&str> = content
            .items
            .iter()
            .map(|i| i.name.as_str())
            .chain(self.gear.iter().map(|g| g.name.as_str()))
            .chain(self.junk.iter().map(|j| j.name.as_str()))
            .chain(self.consumables.iter().map(|c| c.name.as_str()))
            .collect();
        for (i, n) in names.iter().enumerate() {
            if names[..i].contains(n) {
                return bad(format!("duplicate item name {n}"));
            }
            // The save file uses these as separators.
            if n.is_empty() || n.contains(['|', '*', '\n', '=']) || n.len() > 64 {
                return bad(format!(
                    "item name {n:?} is empty, too long or has | * = or a newline"
                ));
            }
        }
        let spell_ok = |id: SpellId| content.spell(id).is_some();
        let perk_ok = |p: &Perk| p.spells().into_iter().all(spell_ok);
        for g in &self.gear {
            if g.description.chars().count() > GEAR_TEXT_MAX
                || g.perk_text.chars().count() > GEAR_TEXT_MAX
                || g.ilvl == 0
            {
                return bad(format!("gear {} text too long or ilvl 0", g.name));
            }
            if g.perk.as_ref().is_some_and(|p| !perk_ok(p))
                || g.perk.is_some() == g.perk_text.is_empty()
            {
                return bad(format!(
                    "gear {} perk needs known spells and a text",
                    g.name
                ));
            }
            if let Some(s) = &g.set
                && self.set(s).is_none()
            {
                return bad(format!("gear {} has unknown set {s}", g.name));
            }
            if let GearSource::Boss(u) = &g.source
                && content.unit_def(u).is_none()
            {
                return bad(format!("gear {} drops from unknown unit {u}", g.name));
            }
        }
        for s in &self.sets {
            if !perk_ok(&s.four)
                || self
                    .gear
                    .iter()
                    .filter(|g| g.set.as_ref() == Some(&s.key))
                    .count()
                    < 4
            {
                return bad(format!(
                    "set {} needs 4 pieces and known perk spells",
                    s.key
                ));
            }
        }
        for j in &self.junk {
            if j.description.chars().count() > ITEM_DESC_MAX + 30 || j.tier > 5 {
                return bad(format!(
                    "junk {} description too long or tier above 5",
                    j.name
                ));
            }
        }
        for c in &self.consumables {
            if c.cast.is_some_and(|id| !spell_ok(id)) {
                return bad(format!("consumable {} casts an unknown spell", c.name));
            }
        }
        for v in &self.vendors {
            if content.unit_def(&v.unit).is_none_or(|u| u.hostile) {
                return bad(format!("vendor {} must be a friendly unit", v.unit));
            }
            if let Some(n) = v
                .stock
                .iter()
                .chain(&v.rotating)
                .find(|n| !matches!(self.goods(n), Some(Goods::Gear(_) | Goods::Consumable(_))))
            {
                return bad(format!("vendor {} sells unknown {n}", v.unit));
            }
            for r in &v.rep {
                let known = matches!(
                    self.goods(&r.item),
                    Some(Goods::Gear(_) | Goods::Consumable(_))
                );
                let dept = content.endgame.department_index(&r.dept).is_some();
                if !known || !dept || usize::from(r.tier) >= content.endgame.rep_tiers.len() {
                    return bad(format!("vendor {} rep item {} is bad", v.unit, r.item));
                }
            }
        }
        if let Some(e) = self.elites.iter().find(|e| content.unit_def(e).is_none()) {
            return bad(format!("unknown elite {e}"));
        }
        Ok(())
    }
}

/// One equipped perk with the item (or set name) that grants it.
#[derive(Clone, Debug)]
pub struct ActivePerk {
    pub source: String,
    pub perk: Perk,
}

/// Cost, cast time and cooldown math over one player's perks, shared by the sim and the HUD
/// (which builds it from the talent picks alone).
pub struct PerkView<'a>(pub &'a [ActivePerk]);

impl PerkView<'_> {
    /// What a press of `spell` casts: its `Replace` variant or itself.
    pub fn replaced(&self, spell: SpellId) -> SpellId {
        self.0
            .iter()
            .find_map(|p| match p.perk {
                Perk::Replace { spell: s, with } if s == spell => Some(with),
                _ => None,
            })
            .unwrap_or(spell)
    }

    /// A perk naming `named` applies to a cast of `cast`: the same spell, or the variant that
    /// replaces it.
    pub fn hits(&self, named: SpellId, cast: SpellId) -> bool {
        named == cast || self.replaced(named) == cast
    }

    fn hits_opt(&self, named: Option<SpellId>, cast: SpellId) -> bool {
        named.is_none_or(|s| self.hits(s, cast))
    }

    /// HP paid instead of `mana` under an `HpCost` perk; 0 without one.
    pub fn hp_cost(&self, mana: u32) -> u32 {
        let pct: u32 = self
            .0
            .iter()
            .map(|p| match p.perk {
                Perk::HpCost { pct } => pct,
                _ => 0,
            })
            .sum();
        (u64::from(mana) * u64::from(pct) / 100) as u32
    }

    /// Mana the cast spends (0 when an `HpCost` perk pays it in HP).
    pub fn mana_cost(&self, spell: SpellId, mana: u32) -> u32 {
        if self.hp_cost(mana) > 0 {
            return 0;
        }
        let pct: i32 = self
            .0
            .iter()
            .map(|p| match &p.perk {
                Perk::ManaCost { spell: s, pct } if self.hits_opt(*s, spell) => *pct,
                _ => 0,
            })
            .sum();
        (i64::from(mana) * i64::from((100 - pct).max(0)) / 100) as u32
    }

    /// Cooldown after `Cooldown` perks (at most 80% shorter).
    pub fn cooldown(&self, spell: SpellId, ms: u32) -> u32 {
        let pct: u32 = self
            .0
            .iter()
            .map(|p| match &p.perk {
                Perk::Cooldown { spell: s, pct } if self.hits(*s, spell) => *pct,
                _ => 0,
            })
            .sum();
        (u64::from(ms) * u64::from(100u32.saturating_sub(pct.min(80))) / 100) as u32
    }

    /// Cast-time change in percent from `CastTime` perks.
    pub fn cast_time_pct(&self, spell: SpellId) -> i64 {
        self.0
            .iter()
            .map(|p| match &p.perk {
                Perk::CastTime { spell: s, pct } if self.hits_opt(*s, spell) => i64::from(*pct),
                _ => 0,
            })
            .sum()
    }
}

impl Sim {
    pub(crate) fn economy(&self) -> &Economy {
        &self.content.economy
    }

    pub(crate) fn bag_event(&mut self, i: usize) {
        let s = &self.units[i].state;
        let e = Event::Bag {
            unit: s.id,
            sp: s.sp,
            bag: s.bag.clone(),
            equipped: s.equipped.clone(),
            buyback: s.buyback.clone(),
        };
        self.emit_to(s.id, e);
    }

    pub(crate) fn item_error(&mut self, pid: EntityId, reason: ItemError) {
        self.emit_to(pid, Event::ItemError { reason });
    }

    /// Gear totals with set bonuses, and the perks they grant.
    pub(crate) fn gear_totals(&self, i: usize) -> (GearStats, Vec<ActivePerk>) {
        self.economy().equipment(&self.units[i].state.equipped)
    }

    // ---- perk hooks (called from effects and casting) ----

    fn perks(&self, i: usize) -> &[ActivePerk] {
        &self.units[i].perks
    }

    pub(crate) fn perk_hits(&self, ci: usize, named: SpellId, cast: SpellId) -> bool {
        self.perk_view(ci).hits(named, cast)
    }

    fn perk_hits_opt(&self, ci: usize, named: Option<SpellId>, cast: SpellId) -> bool {
        named.is_none_or(|s| self.perk_hits(ci, s, cast))
    }

    /// Spell damage or healing on unit `ti` after the caster's gear perks (and a splash's
    /// reduced share).
    pub(crate) fn perk_amount(
        &self,
        ci: usize,
        ti: usize,
        spell: SpellId,
        amount: u64,
        heal: bool,
    ) -> u64 {
        let low = |i: usize, below: u32| {
            let u = &self.units[i].state;
            u64::from(u.hp) * 100 <= u64::from(u.max_hp) * u64::from(below)
        };
        let pct: u64 = self
            .perks(ci)
            .iter()
            .map(|p| match &p.perk {
                Perk::Execute { below_pct, pct } if !heal && low(ti, *below_pct) => u64::from(*pct),
                Perk::Triage { below_pct, pct } if heal && low(ti, *below_pct) => u64::from(*pct),
                Perk::Desperate { below_pct, pct } if low(ci, *below_pct) => u64::from(*pct),
                Perk::SpellDamage { spell: s, pct }
                    if !heal && self.perk_hits_opt(ci, *s, spell) =>
                {
                    u64::from(*pct)
                }
                Perk::SpellHeal { spell: s, pct } if heal && self.perk_hits_opt(ci, *s, spell) => {
                    u64::from(*pct)
                }
                _ => 0,
            })
            .sum();
        amount * (100 + pct) / 100 * u64::from(self.units[ci].splash_pct) / 100
    }

    fn perk_view(&self, ci: usize) -> PerkView<'_> {
        PerkView(&self.units[ci].perks)
    }

    pub(crate) fn perk_mana_cost(&self, ci: usize, spell: SpellId, mana: u32) -> u32 {
        self.perk_view(ci).mana_cost(spell, mana)
    }

    /// HP paid instead of `mana` under an `HpCost` perk; 0 without one.
    pub(crate) fn perk_hp_cost(&self, ci: usize, mana: u32) -> u32 {
        self.perk_view(ci).hp_cost(mana)
    }

    pub(crate) fn perk_cooldown(&self, ci: usize, spell: SpellId, ms: u32) -> u32 {
        self.perk_view(ci).cooldown(spell, ms)
    }

    /// Cast-time change in percent from `CastTime` perks.
    pub(crate) fn perk_cast_time_pct(&self, ci: usize, spell: SpellId) -> i64 {
        self.perk_view(ci).cast_time_pct(spell)
    }

    /// A `CastWhileMoving` perk covers this (non-channel) spell.
    pub(crate) fn perk_moving_ok(&self, ci: usize, spell: &crate::content::SpellDef) -> bool {
        spell.channel.is_none()
            && self.perks(ci).iter().any(|p| {
                matches!(&p.perk, Perk::CastWhileMoving { spell: s } if self.perk_hits_opt(ci, *s, spell.id))
            })
    }

    /// Kudos bonus in percent from the looter's `KudosFind` perks.
    pub(crate) fn perk_kudos_pct(&self, i: usize) -> u32 {
        self.perks(i)
            .iter()
            .map(|p| match p.perk {
                Perk::KudosFind { pct } => pct,
                _ => 0,
            })
            .sum()
    }

    /// Damage-taken change in percent from the bearer's `DamageTaken` perks.
    pub(crate) fn perk_damage_taken_pct(&self, ti: usize) -> i64 {
        self.perks(ti)
            .iter()
            .map(|p| match p.perk {
                Perk::DamageTaken { pct } => i64::from(pct),
                _ => 0,
            })
            .sum()
    }

    /// Periodic tick strength change from `Periodic` perks of the aura's caster.
    pub(crate) fn perk_periodic_pct(&self, si: usize, spell: Option<SpellId>) -> u64 {
        let Some(spell) = spell else { return 0 };
        self.perks(si)
            .iter()
            .map(|p| match &p.perk {
                Perk::Periodic { spell: s, pct } if self.perk_hits(si, *s, spell) => {
                    u64::from(*pct)
                }
                _ => 0,
            })
            .sum()
    }

    /// Extra targets of a single-target spell from a `Splash` perk: up to `count` living units
    /// on the primary target's side near it, nearest first, with the share in percent.
    pub(crate) fn perk_splash(
        &self,
        ci: usize,
        spell: &crate::content::SpellDef,
        primary: usize,
    ) -> Option<(Vec<usize>, u32)> {
        let (radius, count, pct) = self.perks(ci).iter().find_map(|p| match &p.perk {
            Perk::Splash {
                spell: s,
                radius_m,
                count,
                pct,
            } if self.perk_hits(ci, *s, spell.id) => Some((*radius_m, *count, *pct)),
            _ => None,
        })?;
        let at = self.units[primary].state.pos;
        let side = self.units[primary].state.hostile;
        let heal = !side && spell.target != SpellTarget::Enemy;
        let mut near: Vec<(f32, usize)> = (0..self.units.len())
            .filter(|&o| {
                let u = &self.units[o];
                o != primary
                    && u.state.alive
                    && !u.despawned
                    && u.state.hostile == side
                    && u.state.ai != Some(crate::types::AiState::Evade)
                    && (!side || u.state.in_combat)
                    && (!heal || u.state.hp < u.state.max_hp)
            })
            .map(|o| (crate::effects::dist_xz(self.units[o].state.pos, at), o))
            .filter(|(d, _)| *d <= radius)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let picked: Vec<usize> = near
            .into_iter()
            .take(usize::from(count))
            .map(|(_, o)| o)
            .collect();
        (!picked.is_empty()).then_some((picked, pct))
    }

    /// After a landed damaging spell hit of `amount` (before modifiers): echoes, leech and
    /// mana refunds.
    pub(crate) fn perk_after_hit(
        &mut self,
        ci: usize,
        ti: usize,
        spell: &crate::content::SpellDef,
        amount: u32,
    ) {
        if self.units[ci].perks.is_empty() {
            return;
        }
        let perks = self.units[ci].perks.clone();
        for p in perks {
            match p.perk {
                Perk::Echo {
                    spell: s,
                    chance_pct,
                    pct,
                    min_hp_pct,
                } if self.perk_hits_opt(ci, s, spell.id) => {
                    let me = &self.units[ci].state;
                    let healthy =
                        u64::from(me.hp) * 100 >= u64::from(me.max_hp) * u64::from(min_hp_pct);
                    if !healthy || !self.units[ti].state.alive || !self.rng.chance_pct(chance_pct) {
                        continue;
                    }
                    self.events.push(Event::Proc {
                        unit: Self::id_of(ci),
                        item: p.source.clone(),
                    });
                    let hit = crate::sim::Hit {
                        spell: Some(spell.id),
                        amount: (u64::from(amount) * u64::from(pct) / 100) as u32,
                        crit: false,
                        combo: false,
                        school: spell.school,
                        dot: false,
                    };
                    self.deal_damage(Self::id_of(ci), Self::id_of(ti), hit);
                }
                Perk::Leech { pct, spell: s } if self.perk_hits_opt(ci, s, spell.id) => {
                    let heal = (u64::from(amount) * u64::from(pct) / 100) as u32;
                    if heal > 0 {
                        self.heal(Self::id_of(ci), ci, Some(spell.id), heal);
                    }
                }
                Perk::ManaReturn { spell: s, pct } if self.perk_hits_opt(ci, s, spell.id) => {
                    let u = &mut self.units[ci].state;
                    let gain = (u64::from(amount) * u64::from(pct) / 100) as u32;
                    let gain = gain.min(u.max_mana.saturating_sub(u.mana));
                    u.mana += gain;
                    if gain > 0 {
                        self.events.push(Event::ManaGain {
                            unit: Self::id_of(ci),
                            spell: spell.id,
                            amount: gain,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    /// Clears a spell's cooldown (and its talent variant's) for unit `i`.
    fn reset_cooldown(&mut self, i: usize, spell: SpellId) {
        let with = self.effective_spell(i, spell);
        let u = &mut self.units[i];
        u.cooldowns.remove(&spell);
        u.cooldowns.remove(&with);
    }

    /// Unit `ki` landed a killing blow: `OnKill` resets and procs.
    pub(crate) fn perk_on_kill(&mut self, ki: usize) {
        if self.units[ki].perks.is_empty() || !self.units[ki].state.alive {
            return;
        }
        let perks = self.units[ki].perks.clone();
        for p in perks {
            let Perk::OnKill { reset, cast } = p.perk else {
                continue;
            };
            if let Some(r) = reset {
                let cooling = [r, self.effective_spell(ki, r)].iter().any(|s| {
                    self.units[ki]
                        .cooldowns
                        .get(s)
                        .is_some_and(|c| c.0 > self.now)
                });
                if cooling {
                    self.reset_cooldown(ki, r);
                    // fire_item_spell announces itself; a bare reset needs its own proc line.
                    if cast.is_none() {
                        self.events.push(Event::Proc {
                            unit: Self::id_of(ki),
                            item: p.source.clone(),
                        });
                    }
                }
            }
            if let Some(c) = cast {
                self.fire_item_spell(ki, c, None, &p.source);
            }
        }
    }

    fn perk_ready(&mut self, ci: usize, key: &str, icd_ms: u32) -> bool {
        let k = format!("icd:{key}");
        let now = self.now;
        if self.units[ci].item_cd.get(&k).is_some_and(|&t| t > now) {
            return false;
        }
        if icd_ms > 0 {
            self.units[ci].item_cd.insert(k, now + ticks(icd_ms));
        }
        true
    }

    /// After a player spell resolved: riders and on-cast procs.
    pub(crate) fn perk_after_cast(&mut self, ci: usize, spell: SpellId, target: Option<EntityId>) {
        if self.units[ci].perks.is_empty() {
            return;
        }
        let ti = target.and_then(|t| self.index(t));
        let perks = self.units[ci].perks.clone();
        for p in perks {
            match p.perk {
                Perk::Rider { spell: s, cast } if self.perk_hits(ci, s, spell) => {
                    self.fire_item_spell(ci, cast, ti, &p.source);
                }
                Perk::Reset {
                    on,
                    chance_pct,
                    reset,
                } if self.perk_hits_opt(ci, on, spell) => {
                    if self.rng.chance_pct(chance_pct) {
                        self.events.push(Event::Proc {
                            unit: Self::id_of(ci),
                            item: p.source.clone(),
                        });
                        self.reset_cooldown(ci, reset);
                    }
                }
                Perk::OnCast {
                    spell: s,
                    chance_pct,
                    cast,
                    icd_ms,
                } if self.perk_hits_opt(ci, s, spell) => {
                    let key = p.source.clone();
                    if self.perk_ready_peek(ci, &key) && self.rng.chance_pct(chance_pct) {
                        self.perk_ready(ci, &key, icd_ms);
                        self.fire_item_spell(ci, cast, ti, &p.source);
                    }
                }
                _ => {}
            }
        }
    }

    fn perk_ready_peek(&self, ci: usize, key: &str) -> bool {
        let k = format!("icd:{key}");
        self.units[ci]
            .item_cd
            .get(&k)
            .is_none_or(|&t| t <= self.now)
    }

    /// A player took damage: on-hurt procs.
    pub(crate) fn perk_on_hurt(&mut self, ti: usize) {
        if self.units[ti].perks.is_empty() || !self.units[ti].state.alive {
            return;
        }
        let perks = self.units[ti].perks.clone();
        for p in perks {
            if let Perk::OnHurt {
                chance_pct,
                cast,
                icd_ms,
            } = p.perk
                && self.perk_ready_peek(ti, &p.source)
                && self.rng.chance_pct(chance_pct)
            {
                self.perk_ready(ti, &p.source, icd_ms);
                self.fire_item_spell(ti, cast, Some(ti), &p.source);
            }
        }
    }

    /// Resolves an item spell's effects for unit `ci` without mana, cooldown or cast events:
    /// area spells hit around the caster, enemy spells the given or current target, ally
    /// spells the given friendly target or the caster.
    pub(crate) fn fire_item_spell(
        &mut self,
        ci: usize,
        cast: SpellId,
        hint: Option<usize>,
        source: &str,
    ) {
        let Some(spell) = self.content.spell(cast).cloned() else {
            return;
        };
        let me = &self.units[ci].state;
        let hostile_ok = |i: usize| {
            let u = &self.units[i].state;
            u.alive && u.hostile != me.hostile
        };
        let targets: Vec<usize> = match (spell.area, spell.target) {
            (Some(area), _) => self.area_targets(ci, area),
            (None, SpellTarget::Enemy) => hint
                .filter(|&i| hostile_ok(i))
                .or_else(|| {
                    me.target
                        .and_then(|t| self.index(t))
                        .filter(|&i| hostile_ok(i))
                })
                .into_iter()
                .collect(),
            (None, SpellTarget::Ally) => {
                let friend = hint.filter(|&i| {
                    let u = &self.units[i].state;
                    u.alive && u.hostile == me.hostile
                });
                vec![friend.unwrap_or(ci)]
            }
            (None, _) => vec![ci],
        };
        if targets.is_empty() && !spell.effects().any(|e| e.caster_centric()) {
            return;
        }
        self.events.push(Event::Proc {
            unit: Self::id_of(ci),
            item: source.to_string(),
        });
        for e in spell.effects() {
            if e.caster_centric() {
                self.apply_effect(ci, &spell, e, None);
            } else {
                for &ti in &targets {
                    self.apply_effect(ci, &spell, e, Some(ti));
                }
            }
        }
    }

    // ---- commands ----

    fn near_vendor(&self, pi: usize, vendor: EntityId) -> Option<&VendorDef> {
        let vi = self.index(vendor)?;
        let v = &self.units[vi];
        let range = self
            .economy()
            .vendor_range_m
            .max(self.content.config.loot_range_m);
        let ok = v.state.alive
            && !v.despawned
            && !v.state.hostile
            && in_range(self.units[pi].state.pos, v.state.pos, range);
        if !ok {
            return None;
        }
        self.economy().vendor_for(&v.state.key)
    }

    pub(crate) fn bag_full(&self, pi: usize) -> bool {
        self.units[pi].state.bag.len() >= self.economy().bag_size.max(1)
    }

    pub(crate) fn equip(&mut self, pid: EntityId, item: &str) {
        let Some(pi) = self.index(pid) else { return };
        let Some(g) = self.economy().gear(item) else {
            return self.item_error(pid, ItemError::CannotEquip);
        };
        let slot = g.slot.index();
        let Some(k) = self.units[pi].state.bag.iter().position(|n| n == item) else {
            return self.item_error(pid, ItemError::NotInBag);
        };
        if !self.units[pi].state.alive {
            return self.item_error(pid, ItemError::Dead);
        }
        let s = &mut self.units[pi].state;
        let old = std::mem::replace(&mut s.equipped[slot], item.to_string());
        if old.is_empty() {
            s.bag.remove(k);
        } else {
            s.bag[k] = old;
        }
        self.gear_changed(pi);
    }

    pub(crate) fn unequip(&mut self, pid: EntityId, slot: u8) {
        let Some(pi) = self.index(pid) else { return };
        let slot = usize::from(slot);
        if slot >= SLOTS || self.units[pi].state.equipped[slot].is_empty() {
            return;
        }
        if self.bag_full(pi) {
            return self.item_error(pid, ItemError::BagFull);
        }
        let s = &mut self.units[pi].state;
        let old = std::mem::take(&mut s.equipped[slot]);
        s.bag.push(old);
        self.gear_changed(pi);
    }

    /// Stats, perks, visuals (broadcast) and the owner's bag after an equipment change.
    fn gear_changed(&mut self, pi: usize) {
        let (old_hp, old_mana) = (self.units[pi].state.max_hp, self.units[pi].state.max_mana);
        self.refresh_item_bonus(pi);
        let s = &mut self.units[pi].state;
        // Equipping never heals and unequipping never kills: keep the HP fraction.
        let keep = |cur: u32, old: u32, new: u32| {
            if old == 0 {
                cur.min(new)
            } else {
                ((u64::from(cur) * u64::from(new) / u64::from(old)) as u32).clamp(1, new.max(1))
            }
        };
        if s.alive {
            s.hp = keep(s.hp, old_hp, s.max_hp);
        }
        s.mana = keep(s.mana, old_mana, s.max_mana).min(s.max_mana);
        let e = Event::GearChanged {
            unit: s.id,
            equipped: s.equipped.clone(),
        };
        self.events.push(e);
        self.bag_event(pi);
    }

    pub(crate) fn buy(&mut self, pid: EntityId, vendor: EntityId, item: &str) {
        let Some(pi) = self.index(pid) else { return };
        let Some(v) = self.near_vendor(pi, vendor) else {
            return self.item_error(pid, ItemError::TooFar);
        };
        let now_ms = crate::time::ticks_to_ms(self.now);
        if !self.economy().stock(v, now_ms).iter().any(|n| n == item) {
            return self.item_error(pid, ItemError::NotForSale);
        }
        let price = self.economy().goods(item).map_or(u32::MAX, |g| g.price());
        let need = self.economy().gear(item).map_or(1, GearDef::required_level);
        if self.units[pi].state.level < need {
            return self.item_error(pid, ItemError::LevelTooLow);
        }
        if let Some((dept, tier)) = self.economy().rep_gate(v, item, &self.content) {
            let rep = self.units[pi].state.career.rep[dept];
            if self.content.endgame.tier_of(rep) < usize::from(tier) {
                return self.item_error(pid, ItemError::RepTooLow);
            }
        }
        if self.units[pi].state.sp < price {
            return self.item_error(pid, ItemError::NotEnoughSp);
        }
        if self.bag_full(pi) {
            return self.item_error(pid, ItemError::BagFull);
        }
        let s = &mut self.units[pi].state;
        s.sp -= price;
        s.bag.push(item.to_string());
        self.bag_event(pi);
    }

    pub(crate) fn sell(&mut self, pid: EntityId, vendor: EntityId, item: &str) {
        let Some(pi) = self.index(pid) else { return };
        if self.near_vendor(pi, vendor).is_none() {
            return self.item_error(pid, ItemError::TooFar);
        }
        let Some(k) = self.units[pi].state.bag.iter().position(|n| n == item) else {
            return self.item_error(pid, ItemError::NotInBag);
        };
        self.sell_slot(pi, k);
        self.bag_event(pi);
    }

    /// Every junk item in the bag goes through the same path as a single sale.
    pub(crate) fn sell_junk(&mut self, pid: EntityId, vendor: EntityId) {
        let Some(pi) = self.index(pid) else { return };
        if self.near_vendor(pi, vendor).is_none() {
            return self.item_error(pid, ItemError::TooFar);
        }
        let junk = self.junk_slots(pi);
        for &k in junk.iter().rev() {
            self.sell_slot(pi, k);
        }
        if !junk.is_empty() {
            self.bag_event(pi);
        }
    }

    /// Callers have checked that `k` is in range; the caller sends the bag event.
    fn sell_slot(&mut self, pi: usize, k: usize) {
        let price = self
            .economy()
            .sell_price(&self.units[pi].state.bag[k])
            .unwrap_or(0);
        let cap = self.economy().buyback_size.max(1);
        let s = &mut self.units[pi].state;
        let name = s.bag.remove(k);
        s.sp = s.sp.saturating_add(price);
        s.buyback.push(name);
        if s.buyback.len() > cap {
            s.buyback.remove(0);
        }
    }

    /// Bag indexes of junk, ascending.
    fn junk_slots(&self, pi: usize) -> Vec<usize> {
        let eco = self.economy();
        let bag = &self.units[pi].state.bag;
        (0..bag.len())
            .filter(|&k| matches!(eco.goods(&bag[k]), Some(Goods::Junk(_))))
            .collect()
    }

    /// Trophies and the loot objective of an active quest are never thrown away. Equipped
    /// pieces are not in the bag at all, so they cannot be reached by slot.
    fn discard_protected(&self, pid: EntityId, item: &str) -> bool {
        if self.content.item(item).is_some() {
            return true;
        }
        self.quests_of(pid).active.iter().any(|q| {
            self.content.quest(&q.id).is_some_and(|def| {
                def.objectives
                    .iter()
                    .any(|o| matches!(o, Objective::Loot(it, _) if it == item))
            })
        })
    }

    /// Destroys the bag item at `slot`. A slot past the end is ignored.
    pub(crate) fn discard(&mut self, pid: EntityId, slot: u16) {
        let Some(pi) = self.index(pid) else { return };
        let k = usize::from(slot);
        let Some(name) = self.units[pi].state.bag.get(k) else {
            return;
        };
        if self.discard_protected(pid, name) {
            return self.item_error(pid, ItemError::Protected);
        }
        self.units[pi].state.bag.remove(k);
        self.bag_event(pi);
    }

    /// Destroys every junk item in the bag except one an active quest still wants.
    pub(crate) fn discard_junk(&mut self, pid: EntityId) {
        let Some(pi) = self.index(pid) else { return };
        let junk: Vec<usize> = self
            .junk_slots(pi)
            .into_iter()
            .filter(|&k| !self.discard_protected(pid, &self.units[pi].state.bag[k]))
            .collect();
        for &k in junk.iter().rev() {
            self.units[pi].state.bag.remove(k);
        }
        if !junk.is_empty() {
            self.bag_event(pi);
        }
    }

    pub(crate) fn buyback(&mut self, pid: EntityId, vendor: EntityId, item: &str) {
        let Some(pi) = self.index(pid) else { return };
        if self.near_vendor(pi, vendor).is_none() {
            return self.item_error(pid, ItemError::TooFar);
        }
        let Some(k) = self.units[pi].state.buyback.iter().rposition(|n| n == item) else {
            return self.item_error(pid, ItemError::NotForSale);
        };
        let price = self.economy().sell_price(item).unwrap_or(0);
        if self.units[pi].state.sp < price {
            return self.item_error(pid, ItemError::NotEnoughSp);
        }
        if self.bag_full(pi) {
            return self.item_error(pid, ItemError::BagFull);
        }
        let s = &mut self.units[pi].state;
        s.sp -= price;
        let name = s.buyback.remove(k);
        s.bag.push(name);
        self.bag_event(pi);
    }

    /// Consumables from the bag, or the on-use perk of an equipped piece.
    pub(crate) fn use_item(&mut self, pid: EntityId, item: &str) {
        let Some(pi) = self.index(pid) else { return };
        if !self.units[pi].state.alive {
            return self.item_error(pid, ItemError::Dead);
        }
        if self.units[pi].state.stunned {
            return self.item_error(pid, ItemError::NotReady);
        }
        if let Some(c) = self.economy().consumable(item).cloned() {
            let Some(k) = self.units[pi].state.bag.iter().position(|n| n == item) else {
                return self.item_error(pid, ItemError::NotInBag);
            };
            // A pet carrier is opened, not drunk: no shared cooldown, kept if already owned.
            if let Some(learned) = self.use_carrier(pid, item) {
                if learned {
                    self.units[pi].state.bag.remove(k);
                    self.bag_event(pi);
                }
                return;
            }
            if !self.item_cd_ready(pi, "consumable") {
                return self.item_error(pid, ItemError::NotReady);
            }
            let cd = self.economy().consumable_cd_ms;
            self.units[pi]
                .item_cd
                .insert("consumable".into(), self.now + ticks(cd));
            self.units[pi].state.bag.remove(k);
            self.emit_to(
                pid,
                Event::ItemCooldown {
                    item: "consumable".into(),
                    ms: cd,
                },
            );
            let s = &mut self.units[pi].state;
            s.mana = (s.mana + (u64::from(s.max_mana) * u64::from(c.mana_pct) / 100) as u32)
                .min(s.max_mana);
            if let Some(cast) = c.cast {
                self.fire_item_spell(pi, cast, Some(pi), &c.name);
            } else {
                self.events.push(Event::Proc {
                    unit: pid,
                    item: c.name.clone(),
                });
            }
            return self.bag_event(pi);
        }
        let on_use = self.units[pi].perks.iter().find_map(|p| match p.perk {
            Perk::OnUse { cast, cooldown_ms } if p.source == item => Some((cast, cooldown_ms)),
            _ => None,
        });
        let Some((cast, cd)) = on_use else {
            return self.item_error(pid, ItemError::CannotUse);
        };
        if !self.item_cd_ready(pi, item) {
            return self.item_error(pid, ItemError::NotReady);
        }
        self.units[pi]
            .item_cd
            .insert(item.to_string(), self.now + ticks(cd));
        let target = self.units[pi].state.target.and_then(|t| self.index(t));
        self.fire_item_spell(pi, cast, target, item);
        let id = self.units[pi].state.id;
        self.emit_to(
            id,
            Event::ItemCooldown {
                item: item.to_string(),
                ms: cd,
            },
        );
    }

    fn item_cd_ready(&self, pi: usize, key: &str) -> bool {
        self.units[pi]
            .item_cd
            .get(key)
            .is_none_or(|&t| t <= self.now)
    }

    /// Remaining and total cooldown of an on-use item or the consumable group (`"consumable"`).
    pub fn item_cooldown(&self, player: EntityId, key: &str) -> (u32, u32) {
        let Some(pi) = self.index(player) else {
            return (0, 0);
        };
        let total = if key == "consumable" {
            self.economy().consumable_cd_ms
        } else {
            self.units[pi]
                .perks
                .iter()
                .find_map(|p| match p.perk {
                    Perk::OnUse { cooldown_ms, .. } if p.source == key => Some(cooldown_ms),
                    _ => None,
                })
                .unwrap_or(0)
        };
        let left = self.units[pi]
            .item_cd
            .get(key)
            .map_or(0, |&t| self.remaining_ms(t));
        (left, total)
    }

    // ---- loot ----

    /// Story Points, gear and junk one group member rolls on a kill (own rng stream, so combat
    /// replays do not shift when loot tables change). Summons give nothing.
    pub(crate) fn roll_economy_loot(
        &mut self,
        mob: usize,
        role: Option<Role>,
    ) -> (Vec<String>, u32) {
        let freeze = self.sprint_has(crate::sprint::Affix::BudgetFreeze);
        let scale = self.sprint_kill_loot_pct();
        let u = &self.units[mob];
        let d = &self.content.units[u.def];
        if !d.hostile || u.summoner.is_some() || d.ai.is_none() {
            return (Vec::new(), 0);
        }
        let eco = &self.content.economy;
        let level = u32::from(u.state.level);
        let elite = eco.elites.contains(&d.key);
        let boss = d.boss;
        // Legacy floors drop the Legacy tier; their boss pieces are the weekly-locked bonus.
        let legacy = self.legacy.fx.is_some();
        let rolls = match (legacy, boss) {
            (true, true) if self.legacy_locked(&d.key) => 0,
            (true, true) => self.content.legacy.boss_items.max(1),
            _ => 1,
        };
        let mult = if boss {
            eco.boss_mult
        } else if elite {
            eco.elite_mult
        } else {
            1
        };
        let rng = &mut self.loot_rng;
        let sp = (level * eco.sp_per_level + rng.below(level + 1)) * mult.max(1);
        // Sprint Budget Freeze: no Story Points, but the loot budget doubles.
        let (sp, loot_mult) = if freeze { (0, 2) } else { (sp, 1) };
        let sp = sp * scale / 100;
        let mut items = Vec::new();
        let gear_pct = if boss {
            100
        } else if elite {
            eco.elite_gear_pct
        } else {
            eco.trash_gear_pct
        };
        let gear_pct = (gear_pct * loot_mult * scale / 100).min(100);
        let tier = u.state.level.div_ceil(2).max(1);
        for _ in 0..rolls {
            if !rng.chance_pct(gear_pct) {
                continue;
            }
            let weights = if boss {
                eco.boss_rarity
            } else if elite {
                eco.elite_rarity
            } else {
                eco.trash_rarity
            };
            let total: u32 = weights.iter().sum();
            let mut roll = rng.below(total.max(1));
            let mut rarity = Rarity::Common;
            for (k, w) in weights.iter().enumerate() {
                if roll < *w {
                    rarity = Rarity::ALL[k];
                    break;
                }
                roll -= w;
            }
            let own = rng.chance_pct(eco.own_role_pct);
            let pool = |r: Rarity, own_only: bool| -> Vec<&GearDef> {
                eco.gear
                    .iter()
                    .filter(|g| {
                        (if legacy {
                            g.source == GearSource::Legacy
                        } else {
                            g.source == GearSource::World && g.tier() == tier
                        }) && g.rarity == r
                            && (!own_only || g.role.is_none() || g.role == role)
                    })
                    .collect()
            };
            // Fall back to lower rarities, then to the whole tier.
            let mut picked = None;
            for r in Rarity::ALL[..=rarity as usize].iter().rev() {
                let mut p = pool(*r, own && role.is_some());
                if p.is_empty() {
                    p = pool(*r, false);
                }
                if !p.is_empty() {
                    let k = rng.below(p.len() as u32) as usize;
                    picked = Some(p[k].name.clone());
                    break;
                }
            }
            items.extend(picked);
        }
        let named: Vec<&GearDef> = eco
            .gear
            .iter()
            .filter(|g| matches!(&g.source, GearSource::Boss(k) if *k == d.key))
            .collect();
        let named_pct = if boss {
            eco.legendary_pct
        } else {
            eco.named_pct * scale / 100
        };
        if !legacy && !named.is_empty() && rng.chance_pct(named_pct) {
            let own: Vec<&&GearDef> = named.iter().filter(|g| g.role == role).collect();
            let k = if own.is_empty() {
                named[rng.below(named.len() as u32) as usize]
            } else {
                own[rng.below(own.len() as u32) as usize]
            };
            items.push(k.name.clone());
        }
        // Junk of the mob's tier, or tier-free junk.
        let junk: Vec<&JunkDef> = eco
            .junk
            .iter()
            .filter(|j| j.tier == 0 || j.tier == tier)
            .collect();
        if !boss && !junk.is_empty() && rng.chance_pct(eco.junk_pct * scale / 100) {
            let k = rng.below(junk.len() as u32) as usize;
            items.push(junk[k].name.clone());
        }
        (items, sp)
    }

    /// Puts looted goods into the bag; returns what did not fit.
    pub(crate) fn stow(&mut self, pi: usize, items: Vec<String>) -> Vec<String> {
        let cap = self.economy().bag_size.max(1);
        let mut left = Vec::new();
        for it in items {
            let s = &mut self.units[pi].state;
            if s.bag.len() < cap {
                s.bag.push(it);
            } else {
                left.push(it);
            }
        }
        left
    }

    /// Quest turn-in SP: `quest_sp_pct` of the quest XP plus the quest's own SP.
    pub(crate) fn quest_sp(&mut self, pi: usize, xp: u32, extra: u32) {
        let sp = (u64::from(xp) * u64::from(self.economy().quest_sp_pct) / 100) as u32 + extra;
        if sp > 0 {
            self.units[pi].state.sp = self.units[pi].state.sp.saturating_add(sp);
            self.bag_event(pi);
        }
    }

    /// Debug/test: puts goods into a player's bag (`player` 0 = the first). False for unknown
    /// goods, a full bag or no player.
    pub fn grant_goods(&mut self, player: EntityId, name: &str) -> bool {
        let p = if player == 0 {
            self.player()
        } else {
            Some(player)
        };
        let Some(pi) = p.filter(|p| self.is_player(*p)).and_then(|p| self.index(p)) else {
            return false;
        };
        if self.economy().goods(name).is_none() || self.bag_full(pi) {
            return false;
        }
        self.units[pi].state.bag.push(name.to_string());
        self.bag_event(pi);
        true
    }

    /// Debug/test: adds Story Points to a player (0 = the first).
    pub fn grant_sp(&mut self, player: EntityId, sp: u32) {
        let p = if player == 0 {
            self.player()
        } else {
            Some(player)
        };
        if let Some(pi) = p.filter(|p| self.is_player(*p)).and_then(|p| self.index(p)) {
            self.units[pi].state.sp = self.units[pi].state.sp.saturating_add(sp);
            self.bag_event(pi);
        }
    }

    /// Debug/test: equips gear directly (bypassing the bag), as if equipped from it.
    pub fn debug_equip(&mut self, player: EntityId, name: &str) -> bool {
        let p = if player == 0 {
            self.player()
        } else {
            Some(player)
        };
        let Some(pi) = p.filter(|p| self.is_player(*p)).and_then(|p| self.index(p)) else {
            return false;
        };
        let Some(slot) = self.economy().gear(name).map(|g| g.slot.index()) else {
            return false;
        };
        self.units[pi].state.equipped[slot] = name.to_string();
        self.gear_changed(pi);
        true
    }
}
