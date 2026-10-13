//! Data-driven content: spells, auras, units, levels, floors and balance, parsed from RON.

use crate::time::ms_to_ticks;
use serde::Deserialize;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(transparent)]
pub struct SpellId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Deserialize)]
pub enum School {
    Physical,
    #[default]
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
    /// Level-1 XP bar maximum (equals `levels[0].xp_to_next`); use `Content::xp_to_next`.
    pub xp_per_level: u32,
    /// Chance for white attacks (player auto attack, enemy melee) to hit.
    pub white_hit_pct: u32,
    /// Chance for damaging spells to hit; heals, auras and interrupts always land.
    pub spell_hit_pct: u32,
    pub regen: RegenDef,
    /// Minimum gap between two player spell yells; the first eligible cast yells.
    pub player_yell_cooldown_ms: u32,
    pub mob_scaling: MobScaling,
    pub xp_rules: XpRules,
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

/// A unit spawned above (or below) its authored level scales by these percents per level.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct MobScaling {
    pub hp_pct_per_level: u32,
    pub damage_pct_per_level: u32,
    pub xp_pct_per_level: u32,
}

/// Classic-style kill XP adjustment by level difference (see FORMULAS.md).
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct XpRules {
    pub above_pct_per_level: u32,
    pub above_max_levels: u32,
    pub below_pct_per_level: u32,
    /// A mob this many levels below the player (or more) is gray: 0 XP.
    pub gray_level_diff: u32,
}

/// Periodic part of an aura: damage (DoT) or heal (HoT), snapshotted when applied.
#[derive(Clone, Debug, Deserialize)]
pub struct PeriodicDef {
    pub interval_ms: u32,
    pub base: u32,
    #[serde(default)]
    pub coefficient_pct: u32,
    #[serde(default)]
    pub heal: bool,
}

fn one() -> u8 {
    1
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuraDef {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub harmful: bool,
    /// No casting, moving, melee or auto attack; applying it interrupts the current cast.
    #[serde(default)]
    pub stun: bool,
    /// No spellcasting (auto attack still works).
    #[serde(default)]
    pub silence: bool,
    #[serde(default)]
    pub periodic: Option<PeriodicDef>,
    /// School of periodic and expiry damage.
    #[serde(default)]
    pub school: School,
    /// Per stack, added to cast times (+20 = 20% longer); negative shortens.
    #[serde(default)]
    pub cast_time_pct: i32,
    /// Per stack, on damage the bearer takes.
    #[serde(default)]
    pub damage_taken_pct: i32,
    /// Per stack, on damage the bearer deals.
    #[serde(default)]
    pub damage_done_pct: i32,
    #[serde(default)]
    pub haste_pct: u32,
    #[serde(default = "one")]
    pub max_stacks: u8,
    /// When the bearer dies, a copy jumps to the nearest enemy of the caster within this radius.
    #[serde(default)]
    pub spread_radius_m: f32,
    /// Damage dealt to the bearer when the aura runs out naturally (not when dispelled).
    #[serde(default)]
    pub expire_damage: u32,
    /// The bearer gains a stack whenever it casts the same spell twice in a row.
    #[serde(default)]
    pub stack_on_repeat_cast: bool,
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

/// Bonus damage while the target is at or below `below_pct` HP.
#[derive(Clone, Debug, Deserialize)]
pub struct Execute {
    pub below_pct: u32,
    pub bonus_pct: u32,
}

/// A small closed set of effect kinds; deliberately not a scripting language.
/// Each effect acts on the spell's target unless noted as caster-centric.
#[derive(Clone, Debug, Deserialize)]
pub enum Effect {
    Damage {
        base: u32,
        coefficient_pct: u32,
        #[serde(default)]
        apply_aura: Option<ApplyAura>,
        #[serde(default)]
        consume_aura: Option<ConsumeAura>,
        #[serde(default)]
        execute: Option<Execute>,
    },
    Absorb {
        aura: String,
        amount: u32,
        duration_ms: u32,
    },
    Interrupt {
        lockout_ms: u32,
    },
    Heal {
        base: u32,
        coefficient_pct: u32,
    },
    Aura {
        aura: String,
        duration_ms: u32,
    },
    /// Applies one aura picked by the rng.
    RandomAura {
        auras: Vec<String>,
        duration_ms: u32,
    },
    /// Removes up to `count` harmful auras, newest first.
    Dispel {
        #[serde(default = "one")]
        count: u8,
    },
    /// Locks a school; `None` locks the school of the target's last cast spell.
    Lockout {
        lockout_ms: u32,
        #[serde(default)]
        school: Option<School>,
    },
    /// Pushes the target away from the caster (the client applies it to the player).
    Knockback {
        strength_m: f32,
    },
    /// Moves the target in front of the caster's spawn point.
    Teleport {
        distance_m: f32,
    },
    /// Caster-centric: spawns adds next to the caster.
    Summon {
        unit: String,
        count: u8,
        max_alive: u8,
    },
    /// Caster-centric: every threat entry of the caster and its allies in range drops to 1.
    ResetAllyThreat {
        radius_m: f32,
    },
    /// Caster-centric: swaps the top two entries of the caster's threat table.
    ThreatSwap,
    /// Repeats the last damaging spell the player landed on the caster, at `pct` damage.
    ForwardLastSpell {
        pct: u32,
    },
    Nothing,
}

impl Effect {
    pub(crate) fn caster_centric(&self) -> bool {
        matches!(
            self,
            Effect::Summon { .. } | Effect::ResetAllyThreat { .. } | Effect::ThreatSwap
        )
    }

    fn auras(&self) -> Vec<&str> {
        match self {
            Effect::Damage {
                apply_aura,
                consume_aura,
                ..
            } => apply_aura
                .iter()
                .map(|a| a.aura.as_str())
                .chain(consume_aura.iter().map(|a| a.aura.as_str()))
                .collect(),
            Effect::Absorb { aura, .. } | Effect::Aura { aura, .. } => vec![aura.as_str()],
            Effect::RandomAura { auras, .. } => auras.iter().map(String::as_str).collect(),
            _ => vec![],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Deserialize)]
pub enum SpellTarget {
    /// The caster's hostile target (player spells validate range, facing and LoS).
    #[default]
    Enemy,
    Caster,
}

/// Effects hit every matching living unit within `radius_m` of the caster instead of one target.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Area {
    pub radius_m: f32,
    /// true: the caster's side (including itself); false: the opposing side.
    pub allies: bool,
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
    /// Resolved after `effect`, in order.
    #[serde(default)]
    pub extra_effects: Vec<Effect>,
    #[serde(default)]
    pub target: SpellTarget,
    #[serde(default)]
    pub area: Option<Area>,
    #[serde(default = "yes")]
    pub interruptible: bool,
    /// Player level that unlocks it (action bar spells only).
    #[serde(default = "one")]
    pub learn_level: u8,
    /// Lines the caster shouts on release, round-robin; empty means silent.
    #[serde(default)]
    pub yells: Vec<String>,
}

impl SpellDef {
    pub fn targets_enemy(&self) -> bool {
        self.target == SpellTarget::Enemy
    }

    pub fn effects(&self) -> impl Iterator<Item = &Effect> {
        std::iter::once(&self.effect).chain(self.extra_effects.iter())
    }

    /// Applies a stun aura (Rubber Duck): bosses are immune.
    pub(crate) fn stuns(&self, content: &Content) -> bool {
        self.effects().any(|e| {
            e.auras()
                .iter()
                .any(|k| content.aura(k).is_some_and(|a| a.stun))
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct LootEntry {
    pub item: String,
    pub chance_pct: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Deserialize)]
pub enum AiTarget {
    /// The current (highest-threat) target.
    #[default]
    Current,
    Caster,
    /// The most injured ally (by HP percent) in range, including itself; skipped while all are full.
    AllyLowestHp,
    RandomHostile,
}

/// One rule of the enemy ability interpreter.
#[derive(Clone, Debug, Deserialize)]
pub struct AiAbility {
    pub spell: SpellId,
    pub cooldown_min_ms: u32,
    pub cooldown_max_ms: u32,
    /// Delay after the pull before the first use; defaults to the cooldown range.
    #[serde(default)]
    pub initial_ms: Option<(u32, u32)>,
    #[serde(default)]
    pub target: AiTarget,
    /// Phases (1-based) in which it is used; empty means all.
    #[serde(default)]
    pub phases: Vec<u8>,
    /// Shouted when the ability starts; empty falls back to the unit's `blast` lines.
    #[serde(default)]
    pub yells: Vec<String>,
}

/// Entered once when HP drops to `below_hp_pct` or less; phases are numbered from 2.
#[derive(Clone, Debug, Deserialize)]
pub struct PhaseDef {
    pub below_hp_pct: u32,
    /// Resolved instantly on entry.
    #[serde(default)]
    pub spell: Option<SpellId>,
    #[serde(default)]
    pub target: AiTarget,
    #[serde(default)]
    pub yells: Vec<String>,
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
    #[serde(default)]
    pub abilities: Vec<AiAbility>,
    #[serde(default)]
    pub phases: Vec<PhaseDef>,
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
    /// Answer to `Command::Talk` (friendly units).
    pub talk: Vec<String>,
    /// Out-of-combat chatter every `ambient_ms` (staggered per unit); 0 disables it.
    pub ambient_ms: u32,
    pub ambient: Vec<String>,
}

/// Declaration order is priority order (lowest wins when several fire in one tick).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum YellKind {
    Death,
    PlayerKilled,
    Phase,
    LowHp,
    Blast,
    Aggro,
    Evade,
    Talk,
    Periodic,
    Ambient,
}

pub(crate) const YELL_KINDS: usize = 10;

impl YellDef {
    pub(crate) fn lines(&self, k: YellKind) -> &[String] {
        match k {
            YellKind::Death => &self.death,
            YellKind::PlayerKilled => &self.player_killed,
            YellKind::Phase => &[],
            YellKind::LowHp => &self.low_hp,
            YellKind::Blast => &self.blast,
            YellKind::Aggro => &self.aggro,
            YellKind::Evade => &self.evade,
            YellKind::Talk => &self.talk,
            YellKind::Periodic => &self.periodic,
            YellKind::Ambient => &self.ambient,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct UnitDef {
    pub key: String,
    pub name: String,
    /// Nameplate subtitle, e.g. "<People & Culture & Chatbots>".
    #[serde(default)]
    pub title: String,
    /// Authored level; stats below are for this level.
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
    /// Immune to stuns, shown with a boss frame.
    #[serde(default)]
    pub boss: bool,
    /// Resolved by the corpse on death (Chatbot Minion explosion).
    #[serde(default)]
    pub death_spell: Option<SpellId>,
    /// Presentation hint for idle friendly NPCs.
    #[serde(default)]
    pub idle_pose: Option<IdlePose>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum IdlePose {
    Sleeping,
    Chatting,
    Sitting,
}

/// Player growth for one level; index 0 is level 1. Bonuses are cumulative over the
/// player unit's authored stats.
#[derive(Clone, Debug, Deserialize)]
pub struct LevelDef {
    /// XP to reach the next level; 0 on the last level.
    pub xp_to_next: u32,
    pub hp_bonus: u32,
    pub mana_bonus: u32,
    pub spell_power_bonus: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PackDef {
    pub anchor: String,
    pub units: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FloorSpawn {
    pub anchor: String,
    pub unit: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FloorDef {
    pub id: u8,
    pub name: String,
    pub level_range: (u8, u8),
    pub packs: Vec<PackDef>,
    pub boss: FloorSpawn,
    #[serde(default)]
    pub friendly: Vec<FloorSpawn>,
}

pub const MAX_ACTION_BAR: usize = 10;

#[derive(Clone, Debug, Deserialize)]
pub struct Content {
    pub config: Config,
    pub auras: Vec<AuraDef>,
    pub spells: Vec<SpellDef>,
    pub units: Vec<UnitDef>,
    pub action_bar: Vec<SpellId>,
    pub levels: Vec<LevelDef>,
    pub floors: Vec<FloorDef>,
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

    /// The player's full bar in key order (1..=0, up to 10); see `Sim::known_spells`.
    pub fn action_bar(&self) -> &[SpellId] {
        &self.action_bar
    }

    pub fn unit_def(&self, key: &str) -> Option<&UnitDef> {
        self.units.iter().find(|u| u.key == key)
    }

    pub fn aura(&self, key: &str) -> Option<&AuraDef> {
        self.auras.iter().find(|a| a.key == key)
    }

    pub fn floor(&self, id: u8) -> Option<&FloorDef> {
        self.floors.iter().find(|f| f.id == id)
    }

    pub fn max_level(&self) -> u8 {
        self.levels.len() as u8
    }

    /// XP needed to leave `level`; 0 at the level cap (or beyond the table).
    pub fn xp_to_next(&self, level: u8) -> u32 {
        level
            .checked_sub(1)
            .and_then(|i| self.levels.get(usize::from(i)))
            .map_or(0, |l| l.xp_to_next)
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
        for (i, a) in self.auras.iter().enumerate() {
            if self.auras[..i].iter().any(|o| o.key == a.key) {
                return bad(format!("duplicate aura {}", a.key));
            }
            if a.max_stacks == 0 || a.periodic.as_ref().is_some_and(|p| p.interval_ms == 0) {
                return bad(format!("aura {} has zero stacks or interval", a.key));
            }
        }
        for (i, s) in self.spells.iter().enumerate() {
            if self.spells[..i].iter().any(|o| o.id == s.id) {
                return bad(format!("duplicate spell id {}", s.id.0));
            }
            for e in s.effects() {
                if let Some(k) = e.auras().into_iter().find(|k| self.aura(k).is_none()) {
                    return bad(format!("spell {} references unknown aura {k}", s.name));
                }
                if let Effect::Summon { unit, .. } = e
                    && self.unit_def(unit).is_none()
                {
                    return bad(format!("spell {} summons unknown unit {unit}", s.name));
                }
                if let Effect::RandomAura { auras, .. } = e
                    && auras.is_empty()
                {
                    return bad(format!("spell {} has an empty RandomAura", s.name));
                }
            }
            if usize::from(s.learn_level) > self.levels.len() || s.learn_level == 0 {
                return bad(format!("spell {} learn_level out of range", s.name));
            }
        }
        if self.action_bar.len() > MAX_ACTION_BAR {
            return bad(format!("action bar longer than {MAX_ACTION_BAR}"));
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
        if self.levels.is_empty() || self.levels.len() > usize::from(u8::MAX) {
            return bad("levels must list 1..=255 entries".to_string());
        }
        if self.levels[0].xp_to_next != c.xp_per_level {
            return bad("xp_per_level must equal levels[0].xp_to_next".to_string());
        }
        let (last, rest) = self.levels.split_last().unwrap_or((&self.levels[0], &[]));
        if last.xp_to_next != 0 || rest.iter().any(|l| l.xp_to_next == 0) {
            return bad("only the last level has xp_to_next 0".to_string());
        }
        if self.unit_def(&self.config.player_unit).is_none() {
            return bad(format!(
                "player unit {} not defined",
                self.config.player_unit
            ));
        }
        for (i, u) in self.units.iter().enumerate() {
            if self.units[..i].iter().any(|o| o.key == u.key) {
                return bad(format!("duplicate unit {}", u.key));
            }
            if let Some(aa) = &u.auto_attack
                && (aa.swing_ms == 0 || aa.damage_min > aa.damage_max || aa.range_m <= 0.0)
            {
                return bad(format!("unit {} has an invalid auto attack", u.key));
            }
            if let Some(id) = u.death_spell
                && self.spell(id).is_none()
            {
                return bad(format!("unit {} death spell {} unknown", u.key, id.0));
            }
            let Some(ai) = &u.ai else { continue };
            for ab in &ai.abilities {
                if self.spell(ab.spell).is_none() {
                    return bad(format!("unit {} uses unknown spell {}", u.key, ab.spell.0));
                }
                let init_bad = ab.initial_ms.is_some_and(|(lo, hi)| lo > hi);
                if ab.cooldown_min_ms > ab.cooldown_max_ms || init_bad {
                    return bad(format!("unit {} ability cooldown min > max", u.key));
                }
            }
            let mut prev = 100;
            for p in &ai.phases {
                if p.below_hp_pct >= prev {
                    return bad(format!("unit {} phases must descend below 100%", u.key));
                }
                prev = p.below_hp_pct;
                if let Some(id) = p.spell
                    && self.spell(id).is_none()
                {
                    return bad(format!("unit {} phase spell {} unknown", u.key, id.0));
                }
            }
        }
        for (i, f) in self.floors.iter().enumerate() {
            if self.floors[..i].iter().any(|o| o.id == f.id) {
                return bad(format!("duplicate floor {}", f.id));
            }
            if f.level_range.0 > f.level_range.1 || f.level_range.0 == 0 {
                return bad(format!("floor {} has a bad level range", f.id));
            }
            let keys = f
                .packs
                .iter()
                .flat_map(|p| p.units.iter())
                .chain(std::iter::once(&f.boss.unit))
                .chain(f.friendly.iter().map(|s| &s.unit));
            for k in keys {
                if self.unit_def(k).is_none() || *k == self.config.player_unit {
                    return bad(format!("floor {} spawns invalid unit {k}", f.id));
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
        assert_eq!(c.action_bar().len(), 10);
        assert_eq!(c.config.xp_per_level, 400);
        assert_eq!(c.max_level(), 10);
        let icons: Vec<&str> = c
            .action_bar()
            .iter()
            .take(4)
            .map(|id| c.spell(*id).unwrap().icon.as_str())
            .collect();
        assert_eq!(
            icons,
            ["code_review", "refactor", "guardrails", "reject_pr"]
        );
        let learn: Vec<u8> = c
            .action_bar()
            .iter()
            .map(|id| c.spell(*id).unwrap().learn_level)
            .collect();
        assert_eq!(learn, [1, 1, 3, 2, 4, 5, 6, 7, 8, 9]);
        let p = c.unit_def("senior_engineer").unwrap();
        assert_eq!(
            (p.max_hp, p.max_mana, p.spell_power, p.crit_pct),
            (300, 250, 20, 5)
        );
        assert_eq!(c.floors.len(), 5);
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
        let broken = SRC.replace(
            "action_bar: [1, 2, 3, 4, 6, 7, 8, 9, 10, 11]",
            "action_bar: [1, 99]",
        );
        assert!(matches!(
            Content::from_ron(&broken),
            Err(ContentError::Invalid(_))
        ));
        let broken = SRC.replace("unit: \"vibe_coder\"", "unit: \"nobody\"");
        assert!(matches!(
            Content::from_ron(&broken),
            Err(ContentError::Invalid(_))
        ));
    }
}
