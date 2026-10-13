//! Data-driven content: spells, units and balance, parsed from RON.

use crate::time::ms_to_ticks;
use serde::Deserialize;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(transparent)]
pub struct SpellId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum School {
    Physical,
    Arcane,
}

impl School {
    pub(crate) fn index(self) -> usize {
        match self {
            School::Physical => 0,
            School::Arcane => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentError {
    Parse(String),
    Invalid(String),
    UnknownUnit(String),
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentError::Parse(m) => write!(f, "content parse error: {m}"),
            ContentError::Invalid(m) => write!(f, "invalid content: {m}"),
            ContentError::UnknownUnit(k) => write!(f, "unknown unit key: {k}"),
        }
    }
}

impl std::error::Error for ContentError {}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub player_unit: String,
    pub gcd_base_ms: u32,
    pub gcd_floor_ms: u32,
    pub queue_window_ms: u32,
    pub pushback_ms: u32,
    pub pushback_max: u8,
    pub crit_damage_pct: u32,
    pub loot_range_m: f32,
    /// XP needed per level; the HUD bar maximum.
    pub xp_per_level: u32,
    /// Chance for white attacks (player auto attack, enemy melee) to hit.
    pub white_hit_pct: u32,
    /// Chance for damaging spells to hit; self-buffs and interrupts always hit.
    pub spell_hit_pct: u32,
    pub regen: RegenDef,
    /// Minimum gap between two player spell yells; the first eligible cast yells.
    pub player_yell_cooldown_ms: u32,
}

/// Out-of-combat regeneration: every `interval_ms` a living unit that is not in
/// combat gains this percent of its max HP and max mana (rounded down).
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct RegenDef {
    /// 0 disables regeneration entirely.
    pub interval_ms: u32,
    pub hp_pct: u32,
    pub mana_pct: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuraDef {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub harmful: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ApplyAura {
    pub aura: String,
    pub duration_ms: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConsumeAura {
    pub aura: String,
    pub bonus_pct: u32,
}

/// Only the effect kinds the game needs now; deliberately not a scripting language.
#[derive(Clone, Debug, Deserialize)]
pub enum Effect {
    Damage {
        base: u32,
        coefficient_pct: u32,
        apply_aura: Option<ApplyAura>,
        consume_aura: Option<ConsumeAura>,
    },
    Absorb {
        aura: String,
        amount: u32,
        duration_ms: u32,
    },
    Interrupt {
        lockout_ms: u32,
    },
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpellDef {
    pub id: SpellId,
    pub name: String,
    /// Asset key the HUD maps to an image.
    pub icon: String,
    pub school: School,
    pub range_m: f32,
    pub cast_ms: u32,
    pub mana: u32,
    pub gcd: bool,
    pub cooldown_ms: u32,
    pub description: String,
    pub effect: Effect,
    #[serde(default = "yes")]
    pub interruptible: bool,
    /// Lines the caster shouts on release, round-robin; empty means silent.
    #[serde(default)]
    pub yells: Vec<String>,
}

impl SpellDef {
    /// Damage and interrupts need a hostile target; Absorb is self-cast.
    pub fn targets_enemy(&self) -> bool {
        !matches!(self.effect, Effect::Absorb { .. })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct LootEntry {
    pub item: String,
    pub chance_pct: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AiAbility {
    pub spell: SpellId,
    pub cooldown_min_ms: u32,
    pub cooldown_max_ms: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AiDef {
    pub aggro_m: f32,
    pub leash_m: f32,
    pub melee_range_m: f32,
    pub melee_ms: u32,
    pub melee_damage: u32,
    pub chase_speed_mps: f32,
    pub respawn_ms: u32,
    pub ability: Option<AiAbility>,
}

/// Ranged "wand" attack that swings on its own timer while a target is held.
#[derive(Clone, Debug, Deserialize)]
pub struct AutoAttackDef {
    pub name: String,
    pub range_m: f32,
    pub swing_ms: u32,
    pub damage_min: u32,
    pub damage_max: u32,
}

/// Monster-yell lines per trigger; empty lists (the default) mean the unit stays silent.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct YellDef {
    /// Interval between periodic yells while in combat; 0 disables them.
    pub periodic_ms: u32,
    pub aggro: Vec<String>,
    pub periodic: Vec<String>,
    pub blast: Vec<String>,
    pub low_hp: Vec<String>,
    pub player_killed: Vec<String>,
    pub death: Vec<String>,
    pub evade: Vec<String>,
}

/// Declaration order is priority order (lowest wins when several fire in one tick).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum YellKind {
    Death,
    PlayerKilled,
    LowHp,
    Blast,
    Aggro,
    Evade,
    Periodic,
}

impl YellDef {
    pub(crate) fn lines(&self, k: YellKind) -> &[String] {
        match k {
            YellKind::Death => &self.death,
            YellKind::PlayerKilled => &self.player_killed,
            YellKind::LowHp => &self.low_hp,
            YellKind::Blast => &self.blast,
            YellKind::Aggro => &self.aggro,
            YellKind::Evade => &self.evade,
            YellKind::Periodic => &self.periodic,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct UnitDef {
    pub key: String,
    pub name: String,
    pub level: u8,
    pub hostile: bool,
    pub max_hp: u32,
    pub max_mana: u32,
    pub spell_power: u32,
    pub crit_pct: u32,
    pub haste_pct: u32,
    pub mana_regen_per_5s_combat: u32,
    pub xp_reward: u32,
    pub loot: Vec<LootEntry>,
    pub ai: Option<AiDef>,
    #[serde(default)]
    pub auto_attack: Option<AutoAttackDef>,
    #[serde(default)]
    pub yells: YellDef,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Content {
    pub config: Config,
    pub auras: Vec<AuraDef>,
    pub spells: Vec<SpellDef>,
    pub units: Vec<UnitDef>,
    pub action_bar: Vec<SpellId>,
}

impl Content {
    pub fn from_ron(src: &str) -> Result<Content, ContentError> {
        let content: Content =
            ron::from_str(src).map_err(|e| ContentError::Parse(e.to_string()))?;
        content.validate()?;
        Ok(content)
    }

    pub fn spell(&self, id: SpellId) -> Option<&SpellDef> {
        self.spells.iter().find(|s| s.id == id)
    }

    /// Player's keys 1..=4 in order.
    pub fn action_bar(&self) -> &[SpellId] {
        &self.action_bar
    }

    pub fn unit_def(&self, key: &str) -> Option<&UnitDef> {
        self.units.iter().find(|u| u.key == key)
    }

    pub fn aura(&self, key: &str) -> Option<&AuraDef> {
        self.auras.iter().find(|a| a.key == key)
    }

    /// GCD after haste, clamped to `[gcd_floor_ms, gcd_base_ms]`.
    pub fn gcd_ms(&self, haste_pct: u32) -> u32 {
        let c = &self.config;
        let hasted = u64::from(c.gcd_base_ms) * 100 / (100 + u64::from(haste_pct));
        (hasted as u32).clamp(c.gcd_floor_ms.min(c.gcd_base_ms), c.gcd_base_ms)
    }

    /// Cast time after haste (no floor, as in WotLK).
    pub fn cast_ms(&self, spell: &SpellDef, haste_pct: u32) -> u32 {
        (u64::from(spell.cast_ms) * 100 / (100 + u64::from(haste_pct))) as u32
    }

    fn validate(&self) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        for (i, s) in self.spells.iter().enumerate() {
            if self.spells[..i].iter().any(|o| o.id == s.id) {
                return bad(format!("duplicate spell id {}", s.id.0));
            }
            let aura_keys: Vec<&str> = match &s.effect {
                Effect::Damage {
                    apply_aura,
                    consume_aura,
                    ..
                } => apply_aura
                    .iter()
                    .map(|a| a.aura.as_str())
                    .chain(consume_aura.iter().map(|a| a.aura.as_str()))
                    .collect(),
                Effect::Absorb { aura, .. } => vec![aura.as_str()],
                Effect::Interrupt { .. } => vec![],
            };
            if let Some(k) = aura_keys.iter().find(|k| self.aura(k).is_none()) {
                return bad(format!("spell {} references unknown aura {k}", s.name));
            }
        }
        if let Some(id) = self.action_bar.iter().find(|id| self.spell(**id).is_none()) {
            return bad(format!("action bar references unknown spell {}", id.0));
        }
        let c = &self.config;
        if c.white_hit_pct > 100 || c.spell_hit_pct > 100 {
            return bad("hit chance above 100%".to_string());
        }
        if c.regen.hp_pct > 100 || c.regen.mana_pct > 100 {
            return bad("regen above 100%".to_string());
        }
        if c.xp_per_level == 0 {
            return bad("xp_per_level must be positive".to_string());
        }
        if self.unit_def(&self.config.player_unit).is_none() {
            return bad(format!(
                "player unit {} not defined",
                self.config.player_unit
            ));
        }
        for u in &self.units {
            if let Some(aa) = &u.auto_attack
                && (aa.swing_ms == 0 || aa.damage_min > aa.damage_max || aa.range_m <= 0.0)
            {
                return bad(format!("unit {} has an invalid auto attack", u.key));
            }
            if let Some(ab) = u.ai.as_ref().and_then(|a| a.ability.as_ref()) {
                if self.spell(ab.spell).is_none() {
                    return bad(format!("unit {} uses unknown spell {}", u.key, ab.spell.0));
                }
                if ab.cooldown_min_ms > ab.cooldown_max_ms {
                    return bad(format!("unit {} ability cooldown min > max", u.key));
                }
            }
        }
        Ok(())
    }
}

/// Ticks for a content duration.
pub(crate) fn ticks(ms: u32) -> u64 {
    ms_to_ticks(ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = include_str!("../../../content/game.ron");

    fn content() -> Content {
        Content::from_ron(SRC).expect("shipped content must parse")
    }

    #[test]
    fn shipped_content_parses_with_spec_numbers() {
        let c = content();
        let cr = c.spell(SpellId(1)).unwrap();
        assert_eq!((cr.cast_ms, cr.mana, cr.range_m as u32), (2000, 35, 30));
        assert_eq!(c.action_bar().len(), 4);
        assert_eq!(c.config.xp_per_level, 400);
        let icons: Vec<&str> = c
            .action_bar()
            .iter()
            .map(|id| c.spell(*id).unwrap().icon.as_str())
            .collect();
        assert_eq!(
            icons,
            ["code_review", "refactor", "guardrails", "reject_pr"]
        );
        let p = c.unit_def("senior_engineer").unwrap();
        assert_eq!(
            (p.max_hp, p.max_mana, p.spell_power, p.crit_pct),
            (300, 250, 20, 5)
        );
    }

    #[test]
    fn gcd_is_haste_adjusted_and_floored() {
        let c = content();
        assert_eq!(c.gcd_ms(0), 1500);
        assert_eq!(c.gcd_ms(25), 1200);
        assert_eq!(c.gcd_ms(100), 1000);
        assert_eq!(c.gcd_ms(1000), 1000);
    }

    #[test]
    fn rejects_hit_chance_above_100() {
        let broken = SRC.replace("spell_hit_pct: 96", "spell_hit_pct: 101");
        assert!(matches!(
            Content::from_ron(&broken),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_garbage_and_dangling_references() {
        assert!(matches!(
            Content::from_ron("(("),
            Err(ContentError::Parse(_))
        ));
        let broken = SRC.replace("action_bar: [1, 2, 3, 4]", "action_bar: [1, 99]");
        assert!(matches!(
            Content::from_ron(&broken),
            Err(ContentError::Invalid(_))
        ));
    }
}
