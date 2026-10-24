//! What Ilmari 1.0 pays out (docs/ILMARI_1_0.md D.7), as data: the ilvl 20 tier sets, the eight
//! class Legendaries, Legendary Fragments, the titles, the Roomba mech mount, the reformed
//! cleaning-robot pet and the token cosmetics. Nothing here is reachable yet: the gear pieces
//! have no `drops_from` until their bosses exist (D5-D13), and titles, mount and pet live in this
//! block rather than in `achievements` / `pets` so that no journal of the base game lists them
//! before the credits (guardrail 1). `validate` is the band: every piece sits at the top Ilmari
//! item level, so it is exactly the ilvl-14 figure outside Ilmari content and the full one inside
//! (`Economy::equipment`), and none of it is in a base-game drop table.

use crate::content::{Content, ContentError, ItemKind, Role};
use crate::economy::{BASE_ILVL_CAP, GearSource, Rarity, Slot};
use serde::Deserialize;

/// Longest title that survives the wire's name sanitiser (`net_proto::MAX_TITLE_CHARS`).
const TITLE_MAX: usize = 24;

/// One piece of a role's tier set and the boss whose drop table it will join.
#[derive(Clone, Debug, Deserialize)]
pub struct TierPieceDef {
    /// The boss unit key that will drop it (the unit exists from D5 on; checked then).
    pub source: String,
    /// The `GearDef::name`.
    pub gear: String,
}

/// A tier set: one per role group, four pieces, one per source.
#[derive(Clone, Debug, Deserialize)]
pub struct TierSetDef {
    /// The `Economy::sets` key.
    pub set: String,
    pub role: Role,
    pub pieces: Vec<TierPieceDef>,
}

/// A class's Legendary: a Keyboard the class forges from fragments. The effect is one sentence
/// (the sim implements it as a capped aura with the boss content that needs it).
#[derive(Clone, Debug, Deserialize)]
pub struct LegendaryDef {
    /// The `ClassDef::key` that can forge it.
    pub class: String,
    pub gear: String,
    pub effect: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TitleDef {
    pub key: String,
    /// What the player wears; only letters, digits, `-`, `_` and spaces fit the wire.
    pub title: String,
    pub how: String,
}

/// A token-priced cosmetic at Brenda's (all difficulties, never power).
#[derive(Clone, Debug, Deserialize)]
pub struct CosmeticDef {
    pub key: String,
    pub name: String,
    pub tokens: u32,
}

/// The Roomba mech: `parts` of `part_item` assemble it at Dale's workbench (`quest`). The ride is
/// the e-scooter's own speed aura, so it is cosmetic only and works in the base game.
#[derive(Clone, Debug, Deserialize)]
pub struct MountDef {
    pub key: String,
    pub name: String,
    pub aura: String,
    pub part_item: String,
    pub parts: u32,
    pub quest: String,
}

/// The reformed cleaning robot: a cosmetic pet (`pets.rs` from D18), never a companion.
#[derive(Clone, Debug, Deserialize)]
pub struct PetRewardDef {
    pub key: String,
    pub name: String,
    pub quest: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RewardsDef {
    /// The only item level above the wings' own band: sets and Legendaries sit exactly here.
    pub top_ilvl: u8,
    /// The `ItemKind::Token` item a Legendary is forged from.
    pub fragment: String,
    pub fragments_needed: u32,
    /// Fragments per clear of the core on Full; Judgment multiplies it (`JudgmentDef`), Story pays none.
    pub fragments_full: u32,
    pub tier_sets: Vec<TierSetDef>,
    pub legendaries: Vec<LegendaryDef>,
    pub titles: Vec<TitleDef>,
    pub cosmetics: Vec<CosmeticDef>,
    pub mount: Option<MountDef>,
    pub pet: Option<PetRewardDef>,
}

impl RewardsDef {
    /// Fragments one core clear pays on `mode`.
    pub fn fragments_for(&self, mode: crate::ilmari::IlmariMode, jd_mult: u32) -> u32 {
        match mode {
            crate::ilmari::IlmariMode::Story => 0,
            crate::ilmari::IlmariMode::Full => self.fragments_full,
            crate::ilmari::IlmariMode::Judgment => self.fragments_full * jd_mult,
        }
    }

    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        // Minimal test content has no rewards at all.
        if self.top_ilvl == 0 && self.tier_sets.is_empty() && self.legendaries.is_empty() {
            return Ok(());
        }
        let bad = |m: String| Err(ContentError::Invalid(format!("ilmari rewards: {m}")));
        // The raid tier's item levels are even, and the wings' band stops below the top.
        if self.top_ilvl <= BASE_ILVL_CAP || self.top_ilvl % 2 == 1 {
            return bad(format!(
                "top item level {} is not a tier level above the base cap",
                self.top_ilvl
            ));
        }
        let gear = |name: &str| c.economy.gear(name);
        for role in [Role::Tank, Role::Healer, Role::Melee, Role::Ranged] {
            if self.tier_sets.iter().filter(|s| s.role == role).count() != 1 {
                return bad(format!("needs exactly one tier set for {role:?}"));
            }
        }
        for s in &self.tier_sets {
            if c.economy.set(&s.set).is_none() || s.pieces.len() != 4 {
                return bad(format!("set {} must be a known set of four pieces", s.set));
            }
            let mut slots = Vec::new();
            for p in &s.pieces {
                let Some(g) = gear(&p.gear) else {
                    return bad(format!("set {} lists unknown piece {}", s.set, p.gear));
                };
                if g.source != GearSource::Ilmari
                    || g.ilvl != self.top_ilvl
                    || g.rarity != Rarity::Epic
                    || g.role != Some(s.role)
                    || g.set.as_deref() != Some(s.set.as_str())
                    || slots.contains(&g.slot)
                {
                    return bad(format!(
                        "{} must be an Epic Ilmari piece of ilvl {} for {:?}, in set {}, in its own slot",
                        g.name, self.top_ilvl, s.role, s.set
                    ));
                }
                // A tier piece that drops at all drops from its own source boss.
                if g.drops_from.as_deref().is_some_and(|b| b != p.source) {
                    return bad(format!(
                        "{} must drop from its source {}, not {:?}",
                        g.name, p.source, g.drops_from
                    ));
                }
                slots.push(g.slot);
            }
            // Nothing else may carry the set, or the 4-piece could be finished from elsewhere.
            let carried = c
                .economy
                .gear
                .iter()
                .filter(|g| g.set.as_deref() == Some(s.set.as_str()))
                .count();
            if carried != 4 {
                return bad(format!("set {} has {carried} pieces, not four", s.set));
            }
        }
        if self.legendaries.len() != c.classes.len() {
            return bad("needs one Legendary per class".into());
        }
        for cl in &c.classes {
            let Some(l) = self.legendaries.iter().find(|l| l.class == cl.key) else {
                return bad(format!("class {} has no Legendary", cl.key));
            };
            let Some(g) = gear(&l.gear) else {
                return bad(format!("Legendary {} is no gear", l.gear));
            };
            if g.source != GearSource::Ilmari
                || g.ilvl != self.top_ilvl
                || g.rarity != Rarity::Legendary
                || g.slot != Slot::Keyboard
                || g.role != Some(cl.role)
                || g.drops_from.is_some()
                || l.effect.is_empty()
                || l.effect.chars().count() > 240
            {
                return bad(format!(
                    "{} must be an undropped Legendary Keyboard of ilvl {} for {} with a short effect",
                    l.gear, self.top_ilvl, cl.key
                ));
            }
        }
        if c.item(&self.fragment)
            .is_none_or(|i| i.kind != ItemKind::Token)
            || self.fragments_needed == 0
            || self.fragments_full == 0
        {
            return bad("fragments need a token item, a price and a Full payout".into());
        }
        for (i, t) in self.titles.iter().enumerate() {
            let ok = !t.title.is_empty()
                && t.title.len() <= TITLE_MAX
                && !t.title.starts_with(' ')
                && !t.title.contains("  ")
                && t.title
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_'));
            if !ok || t.key.is_empty() || self.titles[..i].iter().any(|o| o.key == t.key) {
                return bad(format!(
                    "title {} is empty, duplicate or does not fit the wire",
                    t.key
                ));
            }
        }
        for (i, k) in self.cosmetics.iter().enumerate() {
            if k.tokens == 0 || self.cosmetics[..i].iter().any(|o| o.key == k.key) {
                return bad(format!("cosmetic {} is free or duplicate", k.key));
            }
        }
        if let Some(m) = &self.mount {
            let (ride, scooter) = (
                c.aura(&m.aura),
                c.world
                    .spells
                    .iter()
                    .filter_map(|s| c.spell(*s))
                    .find_map(|s| c.mount_of(s))
                    .and_then(|(a, _)| c.aura(a)),
            );
            let same = matches!((ride, scooter), (Some(r), Some(s))
                if r.mount && !r.harmful && r.speed_pct == s.speed_pct);
            if !same
                || m.parts == 0
                || c.item(&m.part_item)
                    .is_none_or(|i| i.kind != ItemKind::Token)
            {
                return bad(
                    "the Roomba rides on the scooter's speed and builds from token parts".into(),
                );
            }
        }
        if let Some(p) = &self.pet
            && (p.lines.is_empty() || p.key.is_empty() || c.pets.iter().any(|o| o.key == p.key))
        {
            return bad("the robot pet needs lines and a key no base pet uses".into());
        }
        Ok(())
    }
}
