//! Data-driven content: spells, auras, units, levels, zones, quests and balance, parsed from RON.

use crate::telegraph::TelegraphShape;
use crate::time::ms_to_ticks;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpellId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
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
    /// Extra enemy max HP per group member beyond the first, in percent (FORMULAS.md).
    #[serde(default = "default_group_hp_pct")]
    pub group_hp_pct_per_player: u32,
    /// Extra enemy damage per group member beyond the first, in percent (FORMULAS.md).
    #[serde(default)]
    pub group_damage_pct_per_player: u32,
    /// Co-op: extra boss damage per group member beyond the first, on top of the group damage.
    #[serde(default)]
    pub group_boss_damage_pct_per_player: u32,
    /// Ilmari Story: each companion counts as this percent of a group member in the three
    /// group scalings above, so bringing company never makes a boss easier on paper.
    #[serde(default)]
    pub companion_group_pct: u32,
    /// Co-op: every this many players beyond the first, each trash pack spawns one more
    /// member (0 = off; FORMULAS.md, Co-op group).
    #[serde(default)]
    pub group_pack_growth_players: u32,
    /// Linked packs: pulling a pack also pulls idle trash of other packs within this many
    /// metres of the pulled unit (0 = off).
    #[serde(default)]
    pub pack_link_m: f32,
    /// Damage of non-boss hostile units, in percent (bosses are tuned on their own).
    #[serde(default = "hundred")]
    pub trash_damage_pct: u32,
    /// Players stay in combat this long after an enemy that had them on its threat table
    /// resets (no out-of-combat regen or resurrect right after kiting a pull away).
    #[serde(default)]
    pub combat_lock_ms: u32,
}

fn default_group_hp_pct() -> u32 {
    40
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
    /// Per stack, on the healing the bearer receives (-40: heals on you are 40% weaker).
    #[serde(default)]
    pub healing_taken_pct: i32,
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
    /// The bearer heals this percent of its max HP when the aura runs out naturally (a shield
    /// nobody broke); not when it is dispelled, depleted or dies.
    #[serde(default)]
    pub expire_heal_pct: u32,
    /// The bearer gains a stack whenever it casts the same spell twice in a row.
    #[serde(default)]
    pub stack_on_repeat_cast: bool,
    /// Spell damage aimed at the bearer hits the caster instead (Alignment Training).
    #[serde(default)]
    pub reflect: bool,
    /// Per stack, on the bearer's max HP while it lasts (players only; Horizontal Scaling).
    #[serde(default)]
    pub max_hp_pct: i32,
    /// Whether `Dispel` effects (and consumables) can remove it; only harmful auras matter.
    #[serde(default)]
    pub dispel: DispelType,
    /// Movement speed bonus in percent; several do not stack, the largest wins (see `world.rs`).
    #[serde(default)]
    pub speed_pct: u32,
    /// A ride (e-scooter): ends on damage, on any other cast, in combat and in zones without
    /// `scooters`.
    #[serde(default)]
    pub mount: bool,
    /// Prompt Whisperer's "Ignore Previous Instructions": the bearer's movement input is
    /// mirrored on the client (movement is client-owned), so it needs no sim support.
    #[serde(default)]
    pub reverse_controls: bool,
    /// Extra in-combat mana per 5 s while it lasts (Fresh Brew).
    #[serde(default)]
    pub mana_regen: u32,
    /// Enemy AI cannot move while it lasts (it still casts and swings); bosses ignore it, like
    /// stuns. Player movement is client-owned, so players ignore it too.
    #[serde(default)]
    pub root: bool,
    /// Enemy AI moves this much slower (the largest wins); bosses included. Players ignore it:
    /// an enemy slow aimed at players needs `stealth_slow_pct` as well.
    #[serde(default)]
    pub slow_pct: u32,
    /// Stealth (Tailgating): idle hostiles notice the bearer only within this percent of their
    /// aggro range (0 = not a stealth aura). Dealing a hit, taking damage or throwing a harmful
    /// aura at an enemy ends it; applying it also drops the bearer from non-boss threat tables.
    #[serde(default)]
    pub stealth_aggro_pct: u32,
    /// Players: movement is this percent slower while it lasts (the largest wins).
    #[serde(default)]
    pub stealth_slow_pct: u32,
    /// Ends the moment the bearer takes direct damage (Wrong Number); DoT ticks do not.
    #[serde(default)]
    pub break_on_damage: bool,
    /// Fear: author it together with `stun` (no casting or swings); enemy AI also runs away
    /// from the caster instead of standing still.
    #[serde(default)]
    pub fear: bool,
    /// A unit with this aura leaves nothing behind when it dies (`UnitDef::death_spawn`): the
    /// Cable Hydra's "labelled" cables.
    #[serde(default)]
    pub no_death_spawn: bool,
    /// The release-note line a rule-change aura shows on the Patch Tuesday wall screens (at most
    /// five short lines, `|`-separated). Empty for every other aura.
    #[serde(default)]
    pub changelog: String,
}

/// Crowd-control family for diminishing returns: repeats within one family shorten, families
/// do not share a counter (a stun after a fear lands in full).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CcCat {
    Stun,
    Incapacitate,
    Fear,
    Silence,
    Root,
}

impl CcCat {
    pub const COUNT: usize = 5;

    pub fn index(self) -> usize {
        self as usize
    }

    /// Families a boss shrugs off entirely (it only loses the current cast).
    pub fn boss_immune(self) -> bool {
        matches!(self, CcCat::Stun | CcCat::Incapacitate | CcCat::Fear)
    }
}

impl AuraDef {
    /// Derived from the flags so existing content needs no new field.
    pub fn cc_cat(&self) -> Option<CcCat> {
        if !self.harmful {
            return None;
        }
        if self.fear {
            Some(CcCat::Fear)
        } else if self.stun && self.break_on_damage {
            Some(CcCat::Incapacitate)
        } else if self.stun {
            Some(CcCat::Stun)
        } else if self.silence {
            Some(CcCat::Silence)
        } else if self.root {
            Some(CcCat::Root)
        } else {
            None
        }
    }
}

/// Dispel category of an aura, shown on the debuff so players know what can clear it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DispelType {
    #[default]
    Magic,
    /// Enrages and boss mechanics: no button makes them go away.
    Never,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ApplyAura {
    pub aura: String,
    pub duration_ms: u32,
}

/// Removes the aura for extra damage: `bonus_pct` per stack it had.
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
        /// Ring slots to leave free, so a second `Summon` of one spell (the Recruiter's real and
        /// fake candidates) lands beside the first and not on top of it.
        #[serde(default)]
        skip: u8,
        /// The adds start idle instead of joining the caster's fight (Red Team Blue Team's
        /// sensors patrol until somebody is seen).
        #[serde(default)]
        idle: bool,
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
    /// Removes this percent of the target's max mana (Context Window Exceeded).
    DrainMana {
        pct: u32,
    },
    /// Brings a dead group member back where they fell with these percents of max HP and mana.
    Resurrect {
        hp_pct: u32,
        mana_pct: u32,
    },
    /// Taunt: the caster's threat on the target becomes 10% above the current top, so the
    /// target turns on the caster at once.
    Taunt,
    /// Extra threat on the target (scaled by the caster's class), without damage.
    Threat {
        amount: u32,
    },
    /// Dash: the caster lands `stop_m` short of the target (player gap-closer).
    Charge {
        stop_m: f32,
    },
    /// Heals a player back up to the HP it had `ms_ago` (at most 7 s); never lowers HP.
    RestoreHp {
        ms_ago: u32,
    },
    /// Caster-centric raid mechanic ("Pair up!"): the players within the spell's range are
    /// tethered in pairs (an odd one out to the caster), each wearing `aura`; a pair farther
    /// apart than `max_m` takes `base` (scaled like a hit) every second for `duration_ms`.
    PairUp {
        aura: String,
        duration_ms: u32,
        max_m: f32,
        base: u32,
    },
    /// Caster-centric raid mechanic (VibeCon's unreviewed PRs): the caster dies on the spot (no
    /// killer, so no loot), and its summoner, if alive, gains a stack of `boost_aura`.
    Detonate {
        #[serde(default)]
        boost_aura: Option<String>,
    },
    /// Copies every harmful aura the caster has on the target (stacks and time left) to up to
    /// `count` more fighting enemies within `radius_m` of it (Cross-Validation).
    Spread {
        radius_m: f32,
        count: u8,
    },
    /// Caster-centric raid mechanic (the HR Business Partner's copy): a random living player is
    /// "copied" by a unit of their role (`tank`, `healer` or `damage`, unit keys), summoned beside
    /// the caster, while the player wears `aura` for as long as the copy lives. The copy dies on
    /// its own after `life_ms`. `Event::Mirrored` names the pair so the client can show the
    /// player's name on the impostor.
    SummonMirror {
        tank: String,
        healer: String,
        damage: String,
        aura: String,
        life_ms: u32,
    },
    /// Caster-centric raid mechanic (Red Team Blue Team, `sight.rs`): one unit per role present
    /// among the living humans (`per_role`), `filler` units up to `min_bodies`, standing on the
    /// caster's right (Red) or left (Blue). Companions are never counted.
    SummonSquad {
        per_role: crate::sight::SquadRoles,
        filler: String,
        side: crate::sight::SquadSide,
        min_bodies: u8,
        /// The squad starts idle (Blue stands guard until the exercise starts).
        #[serde(default)]
        idle: bool,
    },
    /// Caster-centric, for a player's spell (the Security Engineer's Tailgating): every pending
    /// offer of the living players within `radius_m` is cancelled without a refusal, so the boss
    /// gets neither a stagger nor a `Pushback` ("the policy has a loophole").
    ClearOffers {
        radius_m: f32,
    },
    /// Caster-centric raid mechanic (the Alignment Consultant, `alignment.rs`): every living
    /// player in range gets a pending aura that turns into a real one unless they opt out.
    Offer(crate::alignment::OfferDef),
    /// Caster-centric raid mechanic (Performance Review): grades every living player on what
    /// they did since the previous review (see `hr.rs` and docs/FORMULAS.md): the lowest grade
    /// below `fine_pct` of baseline gets `worst`, the highest at or above 100 gets `best`.
    Review {
        worst: String,
        worst_ms: u32,
        #[serde(default)]
        best: Option<String>,
        #[serde(default)]
        best_ms: u32,
        /// Nobody is "worst" while the lowest grade is at least this percent of its baseline.
        fine_pct: u32,
        /// What one interrupt is worth, in damage points.
        interrupt_value: u32,
        /// What one avoidable hit (a landed telegraph) costs, in damage points.
        fail_cost: u32,
        /// Per role: the points a competent player scores in one window.
        baselines: Vec<(Role, u32)>,
    },
    /// Caster-centric raid mechanic (the Scraper Queen's waves, `wing3.rs`): `clusters` swarm
    /// clusters of `drones` each come out of the vents on a ring `ring_m` from the caster's spawn
    /// point (or, with `column`, down one vent in a line). Past `max_alive` living clusters the
    /// wave is skipped, so a Crawl Budget cast cannot pile up (0: no bound beyond the unit's own).
    SwarmWave {
        unit: String,
        clusters: u8,
        drones: u16,
        ring_m: f32,
        #[serde(default)]
        column: bool,
        #[serde(default)]
        max_alive: u8,
    },
    /// Caster-centric raid mechanic (The Hallucination, `illusion.rs`): starts a hazard cycle on
    /// the 3 x 3 grid of `tile_m` tiles centred on the caster's spawn point. One seeded draw picks
    /// the real hazards, which land `warn_ms` later for `damage`; what each player's screen shows
    /// on top is cosmetic.
    Confabulate {
        tile_m: f32,
        damage: u32,
        warn_ms: u32,
    },
    /// Caster-centric: the caster jumps to one of these (right, forward) offsets from its spawn
    /// point, picked by the rng (the Queen's Rotating Proxies).
    Blink {
        spots: Vec<(f32, f32)>,
    },
    /// Caster-centric (the Region Outage's Health Check, `wing4.rs`): the next region goes
    /// `ms` earlier. Only an uninterrupted cast gets here, which is the point of the kick.
    AdvanceOutage {
        ms: u32,
    },
    /// Caster-centric: `count` footnotes beside the caster, exactly one of them the real citation
    /// (`real`), picked by the rng; the rest are `fake`.
    Footnotes {
        fake: String,
        real: String,
        count: u8,
    },
    Nothing,
}

impl Effect {
    pub(crate) fn caster_centric(&self) -> bool {
        matches!(
            self,
            Effect::Summon { .. }
                | Effect::ResetAllyThreat { .. }
                | Effect::ThreatSwap
                | Effect::PairUp { .. }
                | Effect::Detonate { .. }
                | Effect::SummonMirror { .. }
                | Effect::SummonSquad { .. }
                | Effect::ClearOffers { .. }
                | Effect::Offer(_)
                | Effect::Review { .. }
                | Effect::SwarmWave { .. }
                | Effect::Confabulate { .. }
                | Effect::Blink { .. }
                | Effect::Footnotes { .. }
                | Effect::AdvanceOutage { .. }
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
            Effect::Absorb { aura, .. }
            | Effect::Aura { aura, .. }
            | Effect::SummonMirror { aura, .. }
            | Effect::PairUp { aura, .. } => {
                vec![aura.as_str()]
            }
            Effect::Review { worst, best, .. } => std::iter::once(worst.as_str())
                .chain(best.iter().map(String::as_str))
                .collect(),
            Effect::RandomAura { auras, .. } => auras.iter().map(String::as_str).collect(),
            Effect::Detonate { boost_aura } => boost_aura.iter().map(String::as_str).collect(),
            Effect::Offer(def) => def.auras(),
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
    /// A living friendly player the caster targets (range and LoS checked), else the caster.
    Ally,
    /// A dead group member the caster targets; the caster must be out of combat.
    DeadAlly,
}

/// Effects hit every matching living unit within `radius_m` of the caster instead of one target.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Area {
    pub radius_m: f32,
    /// true: the caster's side (including itself); false: the opposing side.
    pub allies: bool,
}

/// Makes a spell a drink-style channel: `cast_ms` is the full duration, and every
/// `interval_ms` the caster regains `mana_pct` percent of max mana. Movement, jumping,
/// damage taken or casting another spell cancel it; the cooldown starts when it ends.
/// With `mana_pct: 0` it is a damage channel instead (Training Run): every tick before the end
/// applies the spell's effects to the target, the end applies them once more and pays the
/// mana, and taking damage does not break it.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ChannelDef {
    pub interval_ms: u32,
    #[serde(default)]
    pub mana_pct: u32,
}

impl ChannelDef {
    pub fn hits(&self) -> bool {
        self.mana_pct == 0
    }
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
    #[serde(default)]
    pub channel: Option<ChannelDef>,
    /// Enemy spells: the effects land on a ground telegraph after a delay instead of at once.
    #[serde(default)]
    pub telegraph: Option<TelegraphDef>,
    /// Must-interrupt: completing it wipes or heavily punishes; the HUD marks the cast bar.
    #[serde(default)]
    pub kick: bool,
    /// Backstab bonuses on the spell's direct damage.
    #[serde(default)]
    pub opener: Option<OpenerDef>,
    /// Finisher that empties the caster's mana pool (Seniority): on top of `mana`, everything
    /// left is spent and the direct damage rises by this percent per point spent.
    #[serde(default)]
    pub spend_mana_bonus_pct: u32,
    /// Pressed while the caster is in stealth, the bar key casts this spell instead (rogue-style
    /// openers on the same key), once the caster's level unlocks it.
    #[serde(default)]
    pub stealth_variant: Option<SpellId>,
    /// Castable only from stealth.
    #[serde(default)]
    pub stealth_only: bool,
    /// Castable only inside an Ilmari wing (guardrail 3 of `ILMARI_RAID.md`: nothing of the
    /// campaign changes outside it).
    #[serde(default)]
    pub ilmari_only: bool,
}

/// Extra direct damage, in percent: from stealth, and when the caster stands behind the target
/// (outside its front 180 degrees). Both add up.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct OpenerDef {
    #[serde(default)]
    pub stealth_bonus_pct: u32,
    #[serde(default)]
    pub behind_bonus_pct: u32,
}

/// Where a telegraph is painted at release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Deserialize)]
pub enum TelegraphAt {
    /// Circles on the target's spot; cones and lines start at the caster and aim at the target.
    #[default]
    Target,
    Caster,
    /// One copy on every unit of the other side within the spell's range (spread mechanic).
    EachHostile,
}

fn one_u8() -> u8 {
    1
}

/// Ground telegraph of an enemy spell: the shape lands `delay_ms` after release on whoever of
/// the other side is inside it then.
#[derive(Clone, Debug, Deserialize)]
pub struct TelegraphDef {
    pub shape: TelegraphShape,
    pub delay_ms: u32,
    #[serde(default)]
    pub at: TelegraphAt,
    /// Stack mechanic: damage is divided among the units inside; if nobody is inside, everyone
    /// of the other side nearby takes it in full.
    #[serde(default)]
    pub split: bool,
    /// Copies side by side across the facing, `step_m` apart, each landing `step_ms` after the
    /// previous one (a sweep).
    #[serde(default = "one_u8")]
    pub count: u8,
    #[serde(default)]
    pub step_m: f32,
    #[serde(default)]
    pub step_ms: u32,
    /// Safe spots as (right, forward) offsets from the caster's spawn point in its spawn
    /// facing; standing within `cover_radius_m` of one (or out of line of sight) is safe.
    #[serde(default)]
    pub cover: Vec<(f32, f32)>,
    #[serde(default)]
    pub cover_radius_m: f32,
    /// One copy of a sweep (never the first or last) is left out at random: the safe lane.
    #[serde(default)]
    pub gap: bool,
    /// The copies march out from the caster along the facing instead of lining up across it:
    /// copy `k` lands `(k + 1) * step_m` ahead, `k * step_ms` after the first (a creeping trail).
    #[serde(default)]
    pub march: bool,
    /// The shape fires this many more times, `repeat_ms` after each landing, in the same place
    /// (Circle Back; the Roadmap's lines that stay on the floor). A repeat is painted the moment
    /// the previous one lands, so the floor shows where the next one will come.
    #[serde(default)]
    pub repeat: u8,
    #[serde(default)]
    pub repeat_ms: u32,
    /// Paints at this (right, forward) offset from the caster's spawn point in its spawn facing,
    /// wherever the caster stands: the Boardroom's floor tiles.
    #[serde(default)]
    pub anchor: Option<(f32, f32)>,
    /// Floor text of the decal ("Meeting room booked").
    pub label: String,
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
    /// Quest-only drop: rolled (on the loot stream) only for a group member with this quest
    /// active whose objective for the item is not full yet.
    #[serde(default)]
    pub quest: Option<String>,
}

/// Copies of one trophy beyond this add nothing; the extra copies are still kept and shown.
pub const ITEM_STACK_CAP: u32 = 3;

/// Longest allowed trophy description; the HUD tooltip is sized for it.
pub const ITEM_DESC_MAX: usize = 110;

/// Permanent player bonus of one trophy copy, as authored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ItemBonus {
    pub max_hp: u32,
    pub max_mana: u32,
    pub spell_power: u32,
    /// Percent, authored in tenths (0.5).
    pub crit_pct: f32,
    pub haste_pct: u32,
    /// Extra mana per 5 s while in combat, on top of the unit's own trickle.
    pub mana_regen: u32,
    /// Damage reduction in percent, authored in tenths (gear sets).
    pub armor_pct: f32,
    /// Damage and healing done in percent, authored in tenths (gear sets).
    pub power_pct: f32,
}

/// Summed trophy bonuses. Crit is kept in tenths of a percent so sums stay exact integers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemTotals {
    pub max_hp: u32,
    pub max_mana: u32,
    pub spell_power: u32,
    pub crit_tenths: u32,
    pub haste_pct: u32,
    pub mana_regen: u32,
    /// Damage reduction from gear, tenths of a percent (capped when applied).
    pub armor_tenths: u32,
    /// Damage and healing done from gear, tenths of a percent.
    pub power_tenths: u32,
}

fn tenths(v: f32) -> u32 {
    (v.max(0.0) * 10.0).round() as u32
}

impl ItemBonus {
    fn crit_tenths(&self) -> u32 {
        tenths(self.crit_pct)
    }

    /// The bonus of `copies` copies, honouring `ITEM_STACK_CAP`.
    pub fn totals(&self, copies: u32) -> ItemTotals {
        let n = copies.min(ITEM_STACK_CAP);
        ItemTotals {
            max_hp: self.max_hp * n,
            max_mana: self.max_mana * n,
            spell_power: self.spell_power * n,
            crit_tenths: self.crit_tenths() * n,
            haste_pct: self.haste_pct * n,
            mana_regen: self.mana_regen * n,
            armor_tenths: tenths(self.armor_pct) * n,
            power_tenths: tenths(self.power_pct) * n,
        }
    }
}

impl ItemTotals {
    pub fn is_empty(&self) -> bool {
        *self == ItemTotals::default()
    }

    pub fn add(&mut self, o: &ItemTotals) {
        self.max_hp += o.max_hp;
        self.max_mana += o.max_mana;
        self.spell_power += o.spell_power;
        self.crit_tenths += o.crit_tenths;
        self.haste_pct += o.haste_pct;
        self.mana_regen += o.mana_regen;
        self.armor_tenths += o.armor_tenths;
        self.power_tenths += o.power_tenths;
    }
}

/// What a loot item is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum ItemKind {
    /// Carried for its small permanent bonus (`ITEM_STACK_CAP` copies count).
    #[default]
    Trophy,
    /// Quest props and currencies: no bonus, never equipped, stacks up to `TOKEN_STACK_CAP`.
    /// Ilmari content hands out only these, so its quest loot cannot leak power into the base
    /// game (docs/ILMARI_RAID.md guardrail 3).
    Token,
}

/// Most copies of one token the bag keeps; the rest of a reward is lost, like a full wallet.
pub const TOKEN_STACK_CAP: u32 = 99;

/// A loot item: a trophy with a small permanent bonus, or a bonus-free token.
#[derive(Clone, Debug, Deserialize)]
pub struct ItemDef {
    pub name: String,
    pub icon: String,
    pub description: String,
    #[serde(default)]
    pub kind: ItemKind,
    pub bonus: ItemBonus,
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
    /// The living friendly unit flagged `UnitDef::ward` (the core's COBOL mainframe); skipped
    /// while there is none.
    Ward,
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
    /// Used only while the caster carries this aura: a rule-change boss (Patch Tuesday) enables
    /// one ability per patch.
    #[serde(default)]
    pub requires_aura: Option<String>,
    /// Which Ilmari difficulty uses it; every other fight runs the `Both` and `FullOnly` rules.
    #[serde(default)]
    pub mode: AbilityMode,
    /// Skipped by a split replica (`UnitDef::split`): the Autoscaler's copies keep a trimmed list.
    #[serde(default)]
    pub original_only: bool,
}

/// The Ilmari wings tune a boss twice: Story drops or softens mechanics (docs/ILMARI_09.md B.3).
/// Elsewhere Story never happens, so `StoryOnly` abilities stay silent and `FullOnly` ones fire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum AbilityMode {
    #[default]
    Both,
    StoryOnly,
    FullOnly,
}

impl AbilityMode {
    pub fn active(self, story: bool) -> bool {
        match self {
            AbilityMode::Both => true,
            AbilityMode::StoryOnly => story,
            AbilityMode::FullOnly => !story,
        }
    }
}

/// What a unit leaves behind when a player kills it: `count` more of `unit` (`story_count` on
/// Story), never past `max_alive` of them under the same boss. A `no_death_spawn` aura stops it.
#[derive(Clone, Debug, Deserialize)]
pub struct DeathSpawn {
    pub unit: String,
    pub count: u8,
    #[serde(default)]
    pub story_count: Option<u8>,
    pub max_alive: u8,
}

/// Soft timer: after `after_ms` in combat the unit gains `aura` until the fight ends.
#[derive(Clone, Debug, Deserialize)]
pub struct EnrageDef {
    pub after_ms: u32,
    /// Story-mode override of `after_ms`: Story fights run on 40% HP but with slower bots and
    /// a forgiving pace, so the Full timer that keeps a lone player out would punish it.
    #[serde(default)]
    pub story_after_ms: Option<u32>,
    /// Full-mode override when a single player holds the instance: the group fights are tuned to
    /// a group's DPS and healing, so a lone player gets the timer that keeps them out.
    #[serde(default)]
    pub solo_after_ms: Option<u32>,
    pub aura: String,
    #[serde(default)]
    pub yells: Vec<String>,
}

/// From `phase` on the unit runs for an exit at `to` ((right, forward) metres from its spawn
/// in its spawn facing), casts `spell` there, fights `pause_ms`, then runs again.
#[derive(Clone, Debug, Deserialize)]
pub struct FleeDef {
    pub phase: u8,
    pub to: (f32, f32),
    pub speed_mps: f32,
    pub spell: SpellId,
    #[serde(default)]
    pub pause_ms: u32,
    #[serde(default)]
    pub yells: Vec<String>,
}

/// Direct hits from inside the front arc are scaled by `damage_taken_pct`, except while the
/// unit is casting or stunned. Periodic damage goes around the shield.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct FrontalShield {
    pub arc_deg: f32,
    pub damage_taken_pct: i32,
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
    #[serde(default)]
    pub enrage: Option<EnrageDef>,
    #[serde(default)]
    pub flee: Option<FleeDef>,
}

fn nitpick_icon() -> String {
    "nitpick".to_string()
}

/// Auto attack that swings on its own timer while a target is held (ranged or melee by range).
#[derive(Clone, Debug, Deserialize)]
pub struct AutoAttackDef {
    pub name: String,
    #[serde(default = "nitpick_icon")]
    pub icon: String,
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
    /// Friendly NPCs enemy damage never touches, like quest NPCs (`Sim::deal_damage`).
    #[serde(default)]
    pub untouchable: bool,
    #[serde(default)]
    pub frontal_shield: Option<FrontalShield>,
    /// Named rare: elite nameplate and a guaranteed better drop.
    #[serde(default)]
    pub elite: bool,
    /// Friendly NPC that `Ally` spells may target (the onboarding's burnt-out colleague).
    #[serde(default)]
    pub healable: bool,
    /// Players: the mana pool is the class's own resource (Seniority) and does not grow with
    /// levels, trophies or Promotion ranks, so spell costs keep their meaning.
    #[serde(default)]
    pub fixed_mana: bool,
    /// Mana gained for every direct hit this unit lands (not DoT ticks).
    #[serde(default)]
    pub mana_on_hit: u32,
    /// Mana gained for every direct hit this unit takes.
    #[serde(default)]
    pub mana_on_hurt: u32,
    /// Burnout (per mille) every group member gains when a player kills this unit: the Recruiter's
    /// human candidates are not for hitting.
    #[serde(default)]
    pub death_burnout: u16,
    #[serde(default)]
    pub death_spawn: Option<DeathSpawn>,
    /// A Resistance companion: friendly, no `ai`, driven by `companion.rs` instead.
    #[serde(default)]
    pub companion: Option<CompanionDef>,
    /// A swarm cluster: one unit that stands for a crowd of drones (`swarm.rs`).
    #[serde(default)]
    pub swarm: Option<SwarmDef>,
    /// A Wing 3 boss's encounter machinery, armed while it fights (`wing3.rs`).
    #[serde(default)]
    pub encounter: Option<EncounterDef>,
    /// A boss that replicates when hit hard (`wing4.rs`, docs/ILMARI_10.md C.3.4).
    #[serde(default)]
    pub split: Option<SplitDef>,
    /// Hostile units: a vision cone and a patrol (`sight.rs`).
    #[serde(default)]
    pub sight: Option<crate::sight::SightDef>,
    /// A unit that is rebuilt when its partner outlives it (`ilmari_core.rs`, the Rack Arms).
    #[serde(default)]
    pub death_pact: Option<crate::ilmari_core::PactDef>,
    /// A friendly unit the boss's `AiTarget::Ward` attacks go for (the core's mainframe).
    #[serde(default)]
    pub ward: bool,
    /// A ward's death loses the fight: the room wipes.
    #[serde(default)]
    pub ward_death_wipes: bool,
}

/// Rolling-window splitting: more than `threshold_pct` of the unit's max HP in player damage inside
/// `window_ms` spawns one replica of the same unit that takes `hp_share_pct` of the parent's
/// current HP, so the total pool is conserved. Replicas share one threat table; the fight ends
/// when the last of them dies.
#[derive(Clone, Debug, Deserialize)]
pub struct SplitDef {
    pub window_ms: u32,
    pub threshold_pct: u32,
    pub story_threshold_pct: u32,
    pub hp_share_pct: u32,
    /// Living units of the family at most, the original included.
    pub max_copies: u8,
    pub story_max_copies: u8,
    /// Between two splits of one family.
    pub cooldown_ms: u32,
    /// A replica's abilities are late by this much per replica already out (they do not cast in
    /// unison).
    pub offset_ms: u32,
    #[serde(default)]
    pub yells: Vec<String>,
    #[serde(default)]
    pub decommission_yells: Vec<String>,
}

/// Platform of an Ilmari floating room. Coordinates are offsets from the boss's spawn point (the
/// hub is where the boss hovers), so a layout and a synthetic test zone place them alike.
#[derive(Clone, Debug, Deserialize)]
pub struct PlatformDef {
    pub id: String,
    /// Written on the platform ("us-east-1").
    pub label: String,
    pub x: f32,
    pub z: f32,
    pub shape: PlatformShape,
    pub kind: PlatformKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum PlatformShape {
    Disc {
        r: f32,
    },
    /// Half extents along and across the direction `yaw_deg` (degrees from +x toward +z).
    Rect {
        hx: f32,
        hz: f32,
        #[serde(default)]
        yaw_deg: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum PlatformKind {
    /// Takes its turn going dark.
    Region,
    /// Never goes dark: the boss hovers over it.
    Hub,
    /// A permanent walkway between platforms.
    Bridge,
}

impl PlatformDef {
    /// Distance outside the shape (0 inside) of the point `(x, z)` given relative to the origin.
    pub fn dist_outside(&self, x: f32, z: f32) -> f32 {
        let (dx, dz) = (x - self.x, z - self.z);
        match self.shape {
            PlatformShape::Disc { r } => ((dx * dx + dz * dz).sqrt() - r).max(0.0),
            PlatformShape::Rect { hx, hz, yaw_deg } => {
                let (s, c) = yaw_deg.to_radians().sin_cos();
                let (lx, lz) = (dx * c + dz * s, -dx * s + dz * c);
                let (ox, oz) = ((lx.abs() - hx).max(0.0), (lz.abs() - hz).max(0.0));
                (ox * ox + oz * oz).sqrt()
            }
        }
    }
}

/// What an Ilmari Wing 3 boss arms when it is pulled (and disarms when it resets or dies). The
/// mechanics themselves are spells (`Effect::SwarmWave`, `Confabulate`, ...); this is the state
/// that has to outlive a cast.
#[derive(Clone, Debug, Deserialize)]
pub enum EncounterDef {
    /// The Scraper Queen: the swarm eats the drafts standing at these props; when `fail_lost`
    /// (`story_fail_lost` on Story) of them are gone the room wipes. Each lost draft heals the
    /// boss `heal_pct` percent and adds a stack of `lost_aura` to it.
    Drafts {
        props: Vec<String>,
        fail_lost: u8,
        story_fail_lost: u8,
        heal_pct: u32,
        #[serde(default)]
        lost_aura: Option<String>,
        /// What a Mark Confidential on a draft does to the draft: it takes no bites for this long.
        mark_ms: u32,
        /// Shared by every player, per draft.
        mark_cooldown_ms: u32,
        lost_yells: Vec<String>,
        saved_yells: Vec<String>,
        fail_yells: Vec<String>,
    },
    /// The Hallucination: nothing to arm, but the hazard cycle is wiped with the fight.
    Illusion,
    /// Model Collapse: the action recorder replays what the group cast; `Call` echoes spawn
    /// `add_unit`.
    Echo {
        #[serde(default)]
        add_unit: Option<String>,
        /// Percent of a recorded cast's power a `Strike` echo deals (see `EchoCfg::strike_pct`).
        strike_pct: u32,
    },
    /// The Autoscaler: the cloud bill climbs with the replicas (docs/ILMARI_10.md C.3.4).
    Cloud(BillDef),
    /// The Region Outage: the zone's platforms go dark in turn (docs/ILMARI_10.md C.3.5).
    Outage(OutageDef),
    /// Red Team Blue Team: the exercise is on standby until somebody is seen (docs/ILMARI_1_0.md D.3).
    Squads(crate::wing5::SquadsDef),
    /// The Chief Ilmari Officer: desks, the inbox, the two-kick landing and the severance meter
    /// (docs/ILMARI_1_0.md D.4).
    Boardroom(Box<crate::wing6::BoardroomDef>),
    /// Ilmari, "The Last Commit": the remix deck, Ill Marie's rounds and the stance they choose
    /// (docs/ILMARI_1_0.md D.5).
    Core(Box<crate::ilmari_core::CoreDef>),
}

/// The cloud bill: a meter of 0..=1000 points that grows with the living replicas.
#[derive(Clone, Debug, Deserialize)]
pub struct BillDef {
    /// Tenths of a point per second by replica count (index `copies - 1`, the last entry for
    /// more): the integer table of `2 * copies^1.5`.
    pub rate_tenths: Vec<u32>,
    /// Story pays this percent of the rate.
    pub story_rate_pct: u32,
    /// Points at which `Overage` stacks I, II and III land.
    pub steps: [u32; 3],
    /// Stack of this aura on every replica per step (damage and cast speed).
    pub overage_aura: String,
    pub story_overage_aura: String,
    /// On every replica from 1000 points: it enables the `Invoice` ability (`requires_aura`).
    pub budget_aura: String,
    /// A decommissioned replica takes this many points off.
    pub relief: u32,
    /// What a Reserved Instance rack does: the rate is halved for this long.
    pub reserve_ms: u32,
    #[serde(default)]
    pub step_yells: Vec<String>,
    #[serde(default)]
    pub budget_yells: Vec<String>,
}

/// The region sequence.
#[derive(Clone, Debug, Deserialize)]
pub struct OutageDef {
    /// The region that always goes first.
    pub first: String,
    /// From the pull to the first region degrading.
    pub first_ms: u32,
    pub degraded_ms: u32,
    pub dark_ms: u32,
    pub restoring_ms: u32,
    /// From one region degrading to the next one doing so (one at a time), per difficulty.
    pub period_ms: u32,
    pub story_period_ms: u32,
    /// Full below this boss HP percent: the period halves (two regions at a time).
    pub cascade_below_pct: u32,
    /// How far ahead the next region is announced, per difficulty (Story also names the whole order).
    pub warn_ms: u32,
    pub story_warn_ms: u32,
    /// Off-grid for `grace_ms` costs this percent of max HP, never the last point, and a teleport.
    pub failover_pct: u32,
    pub story_failover_pct: u32,
    pub grace_ms: u32,
    /// Tolerance round every platform, for latency.
    pub margin_m: f32,
    #[serde(default)]
    pub first_dark_yells: Vec<String>,
    #[serde(default)]
    pub dark_yells: Vec<String>,
    #[serde(default)]
    pub restored_yells: Vec<String>,
    #[serde(default)]
    pub failover_yells: Vec<String>,
}

/// A cluster of drones simulated as one unit (docs/ILMARI_10.md C.3.1). The HP pool is
/// `count * per_drone_hp`, so the drone count is always derived from HP and needs no state of
/// its own, on the wire or in the sim.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct SwarmDef {
    pub per_drone_hp: u32,
    /// Draft integrity eaten per drone per second, in thousandths of an integrity point.
    pub eat_per_drone_per_s: u32,
    /// Walking speed toward the nearest draft, metres per second.
    pub speed: f32,
    /// A cluster with fewer drones joins a neighbour within `swarm::MERGE_RANGE_M`.
    pub merge_below: u16,
    /// Population bound; a spawn past it merges the two smallest clusters first.
    pub max_clusters: u16,
}

impl SwarmDef {
    /// Drones alive in a cluster with `hp` left; the last sliver of HP still counts as a drone.
    pub fn count(&self, hp: u32) -> u16 {
        hp.div_ceil(self.per_drone_hp.max(1))
            .min(u32::from(u16::MAX)) as u16
    }
}

/// What a companion does in a fight; the spells are ordinary `Content::spells` entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum CompanionRole {
    Tank,
    Healer,
    Damage,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CompanionDef {
    pub role: CompanionRole,
    /// The bread-and-butter spell: the tank's strike, the healer's heal, the damage bolt.
    pub main: SpellId,
    /// Tank: the taunt. Damage: the kick (always used on a must-interrupt cast).
    #[serde(default)]
    pub utility: Option<SpellId>,
    /// Tank: the self absorb, used when low.
    #[serde(default)]
    pub defense: Option<SpellId>,
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
    /// Kill XP of mobs at this level and quest XP of zones starting at it, in percent of the
    /// authored reward: later content pays more so the steeper table still fits the zones.
    #[serde(default = "hundred")]
    pub xp_reward_pct: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PackDef {
    pub anchor: String,
    pub units: Vec<String>,
    /// Fixed level for the pack (campus districts with their own level band); `None` takes the
    /// zone's `level_range` split.
    #[serde(default)]
    pub level: Option<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ZoneSpawn {
    pub anchor: String,
    pub unit: String,
    /// Friendly spawns only: the unit is there from this progress on (checked at zone load).
    #[serde(default)]
    pub requires: Option<Unlock>,
    /// Friendly spawns only: the unit is gone once this is met, so appearances can hand over.
    #[serde(default)]
    pub until: Option<Unlock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum ZoneKind {
    Outdoor,
    Indoor,
}

/// World progress that opens a gated zone link.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Unlock {
    ZoneCleared(String),
    /// The unit key of a zone boss; only its death as a zone boss counts.
    BossKilled(String),
    QuestDone(String),
    /// The quest is accepted or already done; opens a door the quest itself sends you through.
    QuestStarted(String),
    /// The first player has reached this level; only spawn gates (`ZoneSpawn`) use it.
    PlayerLevel(u8),
    /// Any one of these; only spawn gates use it (a beat that hands over to one of several).
    AnyOf(Vec<Unlock>),
}

/// An exit of a zone: the client's trigger anchor (`door:<zone>`, `elevator_exit`) and where it leads.
#[derive(Clone, Debug, Deserialize)]
pub struct ZoneLink {
    pub anchor: String,
    pub to: String,
    #[serde(default)]
    pub requires: Option<Unlock>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ZoneDef {
    pub id: String,
    pub name: String,
    pub kind: ZoneKind,
    pub level_range: (u8, u8),
    pub packs: Vec<PackDef>,
    #[serde(default)]
    pub boss: Option<ZoneSpawn>,
    #[serde(default)]
    pub friendly: Vec<ZoneSpawn>,
    #[serde(default)]
    pub links: Vec<ZoneLink>,
    /// Facilities allows e-scooters here (campus, data center halls).
    #[serde(default)]
    pub scooters: bool,
    /// Optional named rares off the main path: spawned once per load where the layout has the
    /// anchor (skipped otherwise), never respawn, and do not count towards a clear.
    #[serde(default)]
    pub rares: Vec<ZoneSpawn>,
    /// Floating platforms (Ilmari Wing 4's Server Cloud); offsets from the boss's spawn point.
    #[serde(default)]
    pub platforms: Vec<PlatformDef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Objective {
    /// Unit key and count; only the player's own kills count.
    Kill(String, u32),
    /// Item name and count; copies already carried at accept count, items are not consumed on turn-in.
    Loot(String, u32),
    /// Talk to a friendly unit with this key.
    Talk(String),
    ClearZone(String),
    /// Be alive in `zone` for `secs` seconds (counts one per second). `label` names what is
    /// pending, for the tracker's progress bar.
    Wait {
        zone: String,
        label: String,
        secs: u32,
    },
    /// Keep the friendly `unit` (a zone spawn) alive for `secs` seconds while standing within
    /// `DEFEND_RANGE_M` of it; meanwhile a `wave` unit spawns next to it every `every_ms` and goes
    /// for it. Its death resets the count and puts it back on its feet (`quests.rs`).
    Defend {
        unit: String,
        wave: String,
        every_ms: u32,
        label: String,
        secs: u32,
    },
    /// Like `Kill`, but a kill only counts if no ground telegraph hit the player since that unit
    /// was pulled.
    KillClean(String, u32),
    /// Use the prop with this key (`world.props`) this many times.
    Interact(String, u32),
    /// Pick up this many glowing `Pickup` props carrying this `tag` (F or a tap); each prop
    /// counts once per player and stays there for everyone else.
    Collect(String, u32),
    /// Look at this many `Inspect` props carrying this `tag`: stand within reach facing the
    /// prop for a second, or use it. Each prop counts once per player.
    Inspect(String, u32),
    /// Use these `Device` props in this order; a later step used too early starts over.
    Sequence {
        label: String,
        props: Vec<String>,
    },
    /// Apply `item` (a quest prop the giver hands over, not a bag item) to `n` distinct targets:
    /// friendly or hostile units with key `target` (talk / F in reach), or `Device` props whose
    /// key or tag is `target`.
    UseOn {
        item: String,
        target: String,
        n: u32,
    },
    /// Carry `item` to the friendly `npc` and talk to it; with `secs` > 0 the quest fails
    /// (`QuestFailed`) when the time runs out first.
    Deliver {
        item: String,
        npc: String,
        #[serde(default)]
        secs: u32,
    },
    /// Walk the friendly `unit` (a zone spawn) to anchor `to`: it moves while a player with the
    /// quest is within `DEFEND_RANGE_M`, `wave` units come for it every `every_ms`, and its death
    /// sends it back to the start. Counts percent of the way (need 100).
    Escort {
        unit: String,
        to: String,
        wave: String,
        every_ms: u32,
        label: String,
    },
}

impl Objective {
    pub fn need(&self) -> u32 {
        match self {
            Objective::Kill(_, n)
            | Objective::Loot(_, n)
            | Objective::KillClean(_, n)
            | Objective::Interact(_, n)
            | Objective::Collect(_, n)
            | Objective::Inspect(_, n)
            | Objective::UseOn { n, .. } => *n,
            Objective::Wait { secs, .. } | Objective::Defend { secs, .. } => *secs,
            Objective::Talk(_) | Objective::ClearZone(_) | Objective::Deliver { .. } => 1,
            Objective::Sequence { props, .. } => props.len() as u32,
            Objective::Escort { .. } => 100,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct QuestGiver {
    pub npc: String,
    pub zone: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct QuestReward {
    pub xp: u32,
    #[serde(default)]
    pub trophy: Option<String>,
    /// Story Points on top of the `quest_sp_pct` share of the XP.
    #[serde(default)]
    pub sp: u32,
    /// Gear tier (1-5) of a "choose one" reward; 0 for none. The choices come from
    /// `Content::quest_gear_choices` for the player's role.
    #[serde(default)]
    pub gear_tier: u8,
    /// Standing with a department (`DepartmentDef::name`, amount).
    #[serde(default)]
    pub rep: Option<(String, u32)>,
    /// Burnout per mille on turn-in: negative relieves, positive costs (expense reports).
    #[serde(default)]
    pub burnout: i16,
    /// Ilmari Tokens on turn-in (`ilmari.rs`: stack and weekly caps apply).
    #[serde(default)]
    pub tokens: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct QuestDef {
    pub id: String,
    pub name: String,
    /// Accepted from and turned in to this NPC.
    pub giver: QuestGiver,
    /// Offer text, shown before accepting.
    pub text: String,
    /// Shown while the quest is active but not ready.
    pub progress_text: String,
    /// Shown on turn-in.
    pub done_text: String,
    pub objectives: Vec<Objective>,
    pub reward: QuestReward,
    #[serde(default)]
    pub requires: Option<String>,
    /// Story beat of the campaign (gold markers, first in the log and tracker); everything else is a side quest.
    #[serde(default)]
    pub main: bool,
    #[serde(default = "one")]
    pub level_min: u8,
    /// Turned in to this NPC instead of the giver (breadcrumbs: "report to X upstairs").
    #[serde(default)]
    pub turn_in: Option<QuestGiver>,
    /// Elite or boss work the ticket recommends a group for.
    #[serde(default)]
    pub group: bool,
    /// Optional work in the campus districts, off the campaign path: it levels on its own
    /// band, so the campaign XP budget leaves it out.
    #[serde(default)]
    pub open_world: bool,
    /// Offered again every UTC day (the dailies' clock): done today means unavailable until the
    /// date moves on. The turn-in is remembered per day in the save (`quest_days`).
    #[serde(default)]
    pub daily: bool,
}

impl QuestDef {
    /// Who takes the finished quest.
    pub fn ender(&self) -> &QuestGiver {
        self.turn_in.as_ref().unwrap_or(&self.giver)
    }

    /// A pure "go talk to X elsewhere" ticket.
    pub fn is_breadcrumb(&self) -> bool {
        self.turn_in.is_some()
            && self
                .objectives
                .iter()
                .all(|o| matches!(o, Objective::Talk(_)))
    }
}

pub const MAX_ACTION_BAR: usize = 10;

/// Party role of a class; the HUD shows it as a tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Tank,
    Healer,
    Melee,
    Ranged,
}

impl Role {
    pub fn tag(self) -> &'static str {
        match self {
            Role::Tank => "TANK",
            Role::Healer => "HEALER",
            Role::Melee => "MELEE",
            Role::Ranged => "RANGED",
        }
    }
}

fn hundred() -> u32 {
    100
}

/// A playable class: base stats and auto attack come from its `unit`, the rest is here.
#[derive(Clone, Debug, Deserialize)]
pub struct ClassDef {
    pub key: String,
    pub name: String,
    pub role: Role,
    /// Unit definition with the base stats, auto attack and (client side) the model.
    pub unit: String,
    /// Two short lines for the role picker.
    pub blurb: String,
    /// Threat this class generates, in percent (tanks above 100).
    #[serde(default = "hundred")]
    pub threat_pct: u32,
    /// Added to all damage the class takes, in percent (negative: armour).
    #[serde(default)]
    pub damage_taken_pct: i32,
    /// Scales the level table's `hp_bonus`, in percent.
    #[serde(default = "hundred")]
    pub level_hp_pct: u32,
    /// Keys 1..=0 in order; each spell's `learn_level` decides when it is usable.
    pub action_bar: Vec<SpellId>,
    /// The class's `DeadAlly` spell, off the bar like Coffee Break.
    pub resurrect: SpellId,
    /// An off-bar utility spell on its own key (the Data Scientist's `Ground Truth`).
    #[serde(default)]
    pub utility: Option<SpellId>,
}

impl ClassDef {
    /// Every spell a member of this class may cast: the bar, the resurrect and Coffee Break.
    pub fn knows(&self, content: &Content, spell: SpellId) -> bool {
        self.action_bar.contains(&spell)
            || self.resurrect == spell
            || self.utility == Some(spell)
            || content.channel_spell().is_some_and(|c| c.id == spell)
            || content.world.spells.contains(&spell)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Content {
    pub config: Config,
    pub auras: Vec<AuraDef>,
    pub spells: Vec<SpellDef>,
    pub units: Vec<UnitDef>,
    pub items: Vec<ItemDef>,
    /// Playable classes; the first is the default (single-player before a pick, old saves).
    pub classes: Vec<ClassDef>,
    pub levels: Vec<LevelDef>,
    pub zones: Vec<ZoneDef>,
    pub quests: Vec<QuestDef>,
    /// Gear, Story Points, vendors (see `economy.rs`); absent in minimal test content.
    #[serde(default)]
    pub economy: crate::economy::Economy,
    /// Interactive props, hidden NPCs, achievements and the spells everyone knows (`world.rs`).
    #[serde(default)]
    pub world: crate::world::WorldDef,
    /// Career Ladder rows per class (`talents.rs`); absent in minimal test content.
    #[serde(default)]
    pub talents: crate::talents::Talents,
    /// Sprint mode buildings, affixes and rewards (`sprint.rs`); absent in minimal test content.
    #[serde(default)]
    pub sprint: crate::sprint::SprintDef,
    /// Daily/weekly tasks, the world boss and Crunch-mode nights (`live.rs`, `dailies.rs`);
    /// absent in minimal test content.
    #[serde(default)]
    pub live: crate::live::LiveDef,
    /// Companions (`pets.rs`); absent in minimal test content.
    #[serde(default)]
    pub pets: Vec<crate::pets::PetDef>,
    /// Promotion ranks, department reputation and burnout (`endgame.rs`, `burnout.rs`);
    /// absent in minimal test content.
    #[serde(default)]
    pub endgame: crate::endgame::EndgameDef,
    /// Legacy Mode zone variants (`legacy.rs`); absent in minimal test content.
    #[serde(default)]
    pub legacy: crate::legacy::LegacyDef,
    /// The raids (`raid.rs`), each its own chain of zones; absent in minimal test content.
    #[serde(default)]
    pub raids: Vec<crate::raid::RaidDef>,
    /// Minigames (`minigame.rs`); absent in minimal test content.
    #[serde(default)]
    pub minigames: Vec<crate::minigame::MinigameDef>,
    /// The Company Calendar's listing (`calendar.rs`).
    #[serde(default)]
    pub calendar: crate::calendar::CalendarDef,
    /// Ilmari 1.0: Judgment Day's numbers and the reward data (`ilmari.rs`); absent in minimal
    /// test content.
    #[serde(default)]
    pub ilmari: crate::ilmari::IlmariDef,
}

impl Content {
    pub fn from_ron(src: &str) -> Result<Content, ContentError> {
        let content: Content =
            ron::from_str(src).map_err(|e| ContentError::Parse(e.to_string()))?;
        content.validate()?;
        Ok(content)
    }

    pub fn minigame(&self, key: &str) -> Option<&crate::minigame::MinigameDef> {
        self.minigames.iter().find(|m| m.key == key)
    }

    pub fn spell(&self, id: SpellId) -> Option<&SpellDef> {
        self.spells.iter().find(|s| s.id == id)
    }

    /// The default class's bar in key order (1..=0, up to 10); see `Sim::known_spells`.
    pub fn action_bar(&self) -> &[SpellId] {
        self.classes.first().map_or(&[][..], |c| &c.action_bar)
    }

    pub fn class(&self, key: &str) -> Option<&ClassDef> {
        self.classes.iter().find(|c| c.key == key)
    }

    /// The class a player unit (by unit key) belongs to.
    pub fn class_of_unit(&self, unit_key: &str) -> Option<&ClassDef> {
        self.classes.iter().find(|c| c.unit == unit_key)
    }

    /// The bar of the class wearing `unit_key`; the default bar for anything else.
    pub fn bar_for_unit(&self, unit_key: &str) -> &[SpellId] {
        self.class_of_unit(unit_key)
            .map_or_else(|| self.action_bar(), |c| &c.action_bar)
    }

    /// The resurrect of the class wearing `unit_key` (the default class's for anything else).
    pub fn resurrect_for_unit(&self, unit_key: &str) -> Option<&SpellDef> {
        self.class_of_unit(unit_key)
            .or_else(|| self.classes.first())
            .and_then(|c| self.spell(c.resurrect))
    }

    /// The player's mana channel (Coffee Break); it lives off the action bar.
    pub fn channel_spell(&self) -> Option<&SpellDef> {
        self.spells
            .iter()
            .find(|s| s.channel.is_some_and(|c| !c.hits()))
    }

    /// The co-op resurrect (first `DeadAlly` spell), off the bar like Coffee Break.
    pub fn resurrect_spell(&self) -> Option<&SpellDef> {
        self.spells
            .iter()
            .find(|s| s.target == SpellTarget::DeadAlly)
    }

    /// The raid `zone` belongs to.
    pub fn raid_of(&self, zone: &str) -> Option<&crate::raid::RaidDef> {
        self.raids.iter().find(|r| r.is_raid_zone(zone))
    }

    /// Any aura in `auras` mirrors movement ("Ignore Previous Instructions").
    pub fn reverses_controls<'a>(&self, mut auras: impl Iterator<Item = &'a str>) -> bool {
        auras.any(|k| self.aura(k).is_some_and(|a| a.reverse_controls))
    }

    pub fn unit_def(&self, key: &str) -> Option<&UnitDef> {
        self.units.iter().find(|u| u.key == key)
    }

    pub fn item(&self, name: &str) -> Option<&ItemDef> {
        self.items.iter().find(|i| i.name == name)
    }

    pub fn aura(&self, key: &str) -> Option<&AuraDef> {
        self.auras.iter().find(|a| a.key == key)
    }

    pub fn zone(&self, id: &str) -> Option<&ZoneDef> {
        self.zones.iter().find(|z| z.id == id)
    }

    /// Legacy floor number: tower floor `n` is zone `tower/n`.
    pub fn floor(&self, id: u8) -> Option<&ZoneDef> {
        self.zone(&format!("tower/{id}"))
    }

    /// Keys of every hostile a fresh load of `zone` spawns: pack units, the boss and hostile
    /// fixed spawns. Rares are left out because a layout without their anchor skips them.
    pub fn zone_hostiles(&self, zone: &str) -> Vec<&str> {
        let Some(z) = self.zone(zone) else {
            return Vec::new();
        };
        z.packs
            .iter()
            .flat_map(|p| p.units.iter().map(String::as_str))
            .chain(z.boss.iter().map(|b| b.unit.as_str()))
            .chain(
                z.friendly
                    .iter()
                    .map(|f| f.unit.as_str())
                    .filter(|u| self.unit_def(u).is_some_and(|d| d.hostile)),
            )
            .collect()
    }

    /// Friendly units a quest hands out, takes in or sends you to talk to. Enemy damage skips
    /// them (`Sim::deal_damage`) so a boss AoE can never leave a ticket without its NPC. Units a
    /// quest makes the player defend or escort stay mortal: their death is that objective.
    pub fn is_quest_npc(&self, key: &str) -> bool {
        let mortal = |o: &Objective| matches!(o, Objective::Defend { unit, .. } | Objective::Escort { unit, .. } if unit == key);
        if self.quests.iter().any(|q| q.objectives.iter().any(mortal)) {
            return false;
        }
        self.quests.iter().any(|q| {
            q.giver.npc == key
                || q.ender().npc == key
                || q.objectives.iter().any(|o| match o {
                    Objective::Talk(k) => k == key,
                    Objective::Deliver { npc, .. } => npc == key,
                    _ => false,
                })
        })
    }

    pub fn quest(&self, id: &str) -> Option<&QuestDef> {
        self.quests.iter().find(|q| q.id == id)
    }

    pub fn max_level(&self) -> u8 {
        self.levels.len() as u8
    }

    /// `xp_reward_pct` of `level`; beyond the table the last row's.
    pub fn xp_reward_pct(&self, level: u8) -> u32 {
        let i = usize::from(level.max(1) - 1).min(self.levels.len().saturating_sub(1));
        self.levels.get(i).map_or(100, |l| l.xp_reward_pct)
    }

    /// XP a quest pays on turn-in: its authored XP at the level of the zone it is turned in
    /// (breadcrumbs pay at their destination). Story Points stay on the authored XP.
    pub fn quest_xp(&self, q: &QuestDef) -> u32 {
        let level = self.zone(&q.ender().zone).map_or(1, |z| z.level_range.0);
        (u64::from(q.reward.xp) * u64::from(self.xp_reward_pct(level)) / 100) as u32
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
        for (i, it) in self.items.iter().enumerate() {
            if self.items[..i].iter().any(|o| o.name == it.name) {
                return bad(format!("duplicate item {}", it.name));
            }
            // A trophy must do something; a token must not.
            let bonus_ok = it.bonus.totals(1).is_empty() == (it.kind == ItemKind::Token);
            if it.description.chars().count() > ITEM_DESC_MAX
                || !bonus_ok
                || !(0.0..=5.0).contains(&it.bonus.crit_pct)
            {
                return bad(format!(
                    "item {} needs a bonus (tokens none), crit 0..=5 and a description of at most {ITEM_DESC_MAX} chars",
                    it.name
                ));
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
                if let Effect::SwarmWave {
                    unit,
                    clusters,
                    drones,
                    ring_m,
                    ..
                } = e
                    && (*clusters == 0
                        || *drones == 0
                        || !(ring_m.is_finite() && *ring_m >= 0.0)
                        || self.unit_def(unit).is_none_or(|u| u.swarm.is_none()))
                {
                    return bad(format!(
                        "spell {} sends a wave that is empty or not a swarm unit",
                        s.name
                    ));
                }
                if let Effect::Confabulate {
                    tile_m, warn_ms, ..
                } = e
                    && (!(tile_m.is_finite() && *tile_m > 0.0) || *warn_ms == 0)
                {
                    return bad(format!(
                        "spell {} confabulates with no grid or time",
                        s.name
                    ));
                }
                if let Effect::Blink { spots } = e
                    && spots.is_empty()
                {
                    return bad(format!("spell {} blinks nowhere", s.name));
                }
                if let Effect::Footnotes { fake, real, count } = e
                    && (*count == 0
                        || self.unit_def(fake).is_none()
                        || self.unit_def(real).is_none())
                {
                    return bad(format!("spell {} footnotes unknown units", s.name));
                }
                if let Effect::SummonMirror {
                    tank,
                    healer,
                    damage,
                    life_ms,
                    ..
                } = e
                    && (*life_ms == 0
                        || [tank, healer, damage]
                            .iter()
                            .any(|u| self.unit_def(u).is_none()))
                {
                    return bad(format!(
                        "spell {} mirrors with no life or an unknown unit",
                        s.name
                    ));
                }
                if let Effect::Review {
                    baselines,
                    fine_pct,
                    ..
                } = e
                    && (*fine_pct == 0
                        || baselines.len() != 4
                        || baselines.iter().any(|b| b.1 == 0))
                {
                    return bad(format!(
                        "spell {} reviews without a baseline for each of the four roles",
                        s.name
                    ));
                }
                if let Effect::SummonSquad {
                    per_role,
                    filler,
                    min_bodies,
                    ..
                } = e
                {
                    if *min_bodies == 0 {
                        return bad(format!("spell {} summons an empty squad", s.name));
                    }
                    per_role.validate(self, filler, &s.name)?;
                }
                if let Effect::Offer(def) = e {
                    def.validate(self, &s.name)?;
                }
                if let Effect::RandomAura { auras, .. } = e
                    && auras.is_empty()
                {
                    return bad(format!("spell {} has an empty RandomAura", s.name));
                }
                if let Effect::PairUp {
                    max_m, duration_ms, ..
                } = e
                    && (!(max_m.is_finite() && *max_m > 0.0) || *duration_ms == 0)
                {
                    return bad(format!("spell {} pairs up with no range or time", s.name));
                }
            }
            if let Some(ch) = &s.channel
                && (ch.interval_ms == 0
                    || ch.interval_ms > s.cast_ms
                    || ch.mana_pct > 100
                    || (!ch.hits() && s.target != SpellTarget::Caster))
            {
                return bad(format!(
                    "spell {} channel needs 0 < interval_ms <= cast_ms, mana_pct 0..=100 and (drinks) target Caster",
                    s.name
                ));
            }
            if s.effects()
                .any(|e| matches!(e, Effect::RestoreHp { ms_ago } if *ms_ago > 7000))
            {
                return bad(format!(
                    "spell {} restores HP further back than 7 s",
                    s.name
                ));
            }
            let resurrects = s.effects().any(|e| matches!(e, Effect::Resurrect { .. }));
            let bad_pct = s.effects().any(|e| {
                matches!(e, Effect::Resurrect { hp_pct, mana_pct }
                    if !(1..=100).contains(hp_pct) || *mana_pct > 100)
            });
            if resurrects != (s.target == SpellTarget::DeadAlly) || bad_pct {
                return bad(format!(
                    "spell {} needs Resurrect (hp_pct 1..=100, mana_pct <= 100) exactly when its target is DeadAlly",
                    s.name
                ));
            }
            if let Some(t) = &s.telegraph {
                let cover_ok = t.cover.is_empty() || t.cover_radius_m > 0.0;
                if !t.shape.valid()
                    || (t.gap && t.count < 3)
                    || t.delay_ms == 0
                    || t.count == 0
                    || t.label.is_empty()
                    || !cover_ok
                    || (t.repeat > 0) != (t.repeat_ms > 0)
                {
                    return bad(format!(
                        "spell {} telegraph needs a positive shape, delay, count, a label and a cover radius with cover",
                        s.name
                    ));
                }
            }
            if usize::from(s.learn_level) > self.levels.len() || s.learn_level == 0 {
                return bad(format!("spell {} learn_level out of range", s.name));
            }
        }
        self.validate_classes()?;
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
            if let Some(l) = u.loot.iter().find(|l| self.item(&l.item).is_none()) {
                return bad(format!(
                    "unit {} drops item {} with no items entry",
                    u.key, l.item
                ));
            }
            match &u.encounter {
                Some(EncounterDef::Drafts {
                    props,
                    fail_lost,
                    story_fail_lost,
                    lost_aura,
                    ..
                }) if props.is_empty()
                    || *fail_lost == 0
                    || *story_fail_lost == 0
                    || props.iter().any(|k| {
                        !self
                            .world
                            .props
                            .iter()
                            .any(|p| p.key == *k && p.kind == crate::world::PropKind::Draft)
                    })
                    || lost_aura.as_ref().is_some_and(|a| self.aura(a).is_none()) =>
                {
                    return bad(format!(
                        "unit {} encounter needs draft props, a fail count and known auras",
                        u.key
                    ));
                }
                Some(EncounterDef::Echo {
                    add_unit: Some(a), ..
                }) if self.unit_def(a).is_none() => {
                    return bad(format!("unit {} echo add {a} unknown", u.key));
                }
                Some(EncounterDef::Cloud(b))
                    if b.rate_tenths.is_empty()
                        || b.steps.windows(2).any(|w| w[0] >= w[1])
                        || b.steps[2] >= 1000
                        || b.story_rate_pct == 0
                        || b.reserve_ms == 0
                        || [&b.overage_aura, &b.story_overage_aura, &b.budget_aura]
                            .iter()
                            .any(|a| self.aura(a).is_none())
                        || u.split.is_none() =>
                {
                    return bad(format!(
                        "unit {} cloud encounter needs a rate table, ordered steps, known auras and a split",
                        u.key
                    ));
                }
                Some(EncounterDef::Outage(o))
                    if o.period_ms == 0
                        || o.story_period_ms == 0
                        || o.degraded_ms == 0
                        || o.dark_ms == 0
                        || o.grace_ms == 0
                        || !(o.margin_m.is_finite() && o.margin_m >= 0.0)
                        || o.failover_pct == 0
                        || o.failover_pct >= 100
                        || o.story_failover_pct >= 100 =>
                {
                    return bad(format!(
                        "unit {} outage encounter has a zero or absurd number",
                        u.key
                    ));
                }
                Some(EncounterDef::Squads(q)) if !q.validate(self, &u.key) || !u.boss => {
                    return bad(format!(
                        "unit {} squads encounter needs known auras, a merge phase, times and a boss",
                        u.key
                    ));
                }
                Some(EncounterDef::Boardroom(b)) if !b.validate(self, &u.key) || !u.boss => {
                    return bad(format!(
                        "unit {} boardroom encounter needs known spells, auras and units, and a boss",
                        u.key
                    ));
                }
                Some(EncounterDef::Core(c)) if !c.validate(self, &u.key) || !u.boss => {
                    return bad(format!(
                        "unit {} core encounter needs known spells, auras and units, a full deck, three rounds, four terminals and a boss",
                        u.key
                    ));
                }
                _ => {}
            }
            if let Some(sp) = &u.split
                && (sp.window_ms == 0
                    || sp.threshold_pct == 0
                    || sp.story_threshold_pct == 0
                    || !(1..100).contains(&sp.hp_share_pct)
                    || sp.max_copies < 2
                    || sp.story_max_copies < 2
                    || u.ai.is_none())
            {
                return bad(format!(
                    "unit {} split needs a window, a share and 2+ copies",
                    u.key
                ));
            }
            if let Some(d) = &u.death_spawn
                && (self.unit_def(&d.unit).is_none() || d.count == 0 || d.max_alive == 0)
            {
                return bad(format!(
                    "unit {} death spawn needs a known unit, a count and a cap",
                    u.key
                ));
            }
            if let Some(c) = &u.companion {
                let spells = [Some(c.main), c.utility, c.defense];
                if u.hostile
                    || u.ai.is_some()
                    || spells.iter().flatten().any(|id| self.spell(*id).is_none())
                {
                    return bad(format!(
                        "companion {} must be friendly, have no ai and use known spells",
                        u.key
                    ));
                }
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
                if let Some(a) = ab.requires_aura.as_ref().filter(|a| self.aura(a).is_none()) {
                    return bad(format!("unit {} ability requires unknown aura {a}", u.key));
                }
            }
            if let Some(e) = &ai.enrage
                && (self.aura(&e.aura).is_none() || e.after_ms == 0)
            {
                return bad(format!(
                    "unit {} enrage needs a known aura and a delay",
                    u.key
                ));
            }
            if let Some(f) = &ai.flee {
                let phases = 1..=ai.phases.len() as u8 + 1;
                if self.spell(f.spell).is_none() || !phases.contains(&f.phase) || f.speed_mps <= 0.0
                {
                    return bad(format!(
                        "unit {} flee needs a known spell, a phase it reaches and a speed",
                        u.key
                    ));
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
        self.validate_zones()?;
        self.validate_quests()?;
        self.economy.validate(self)?;
        self.world.validate(self)?;
        self.talents.validate(self)?;
        self.sprint.validate(self)?;
        self.live.validate(self)?;
        crate::pets::validate(self)?;
        self.endgame.validate(self)?;
        self.legacy.validate(self)?;
        crate::raid::validate_all(self)?;
        crate::minigame::validate(self)?;
        self.calendar.validate(self)?;
        for u in &self.units {
            if let Some(sight) = &u.sight {
                if !u.hostile {
                    return Err(ContentError::Invalid(format!(
                        "unit {} has sight but is not hostile",
                        u.key
                    )));
                }
                sight.validate(self, &u.key)?;
            }
        }
        self.ilmari.judgment.validate(self)?;
        self.ilmari.ladder.validate(self)?;
        self.ilmari.rewards.validate(self)?;
        Ok(())
    }
}

impl Content {
    fn validate_classes(&self) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        if self.classes.is_empty() || self.classes.len() > usize::from(u8::MAX) {
            return bad("classes must list 1..=255 entries".to_string());
        }
        for (i, c) in self.classes.iter().enumerate() {
            if self.classes[..i]
                .iter()
                .any(|o| o.key == c.key || o.unit == c.unit)
            {
                return bad(format!("duplicate class key or unit {}", c.key));
            }
            if self
                .unit_def(&c.unit)
                .is_none_or(|u| u.hostile || u.ai.is_some())
            {
                return bad(format!("class {} needs a friendly unit without ai", c.key));
            }
            if c.action_bar.len() > MAX_ACTION_BAR {
                return bad(format!("class {} bar longer than {MAX_ACTION_BAR}", c.key));
            }
            if let Some(id) = c.action_bar.iter().find(|id| self.spell(**id).is_none()) {
                return bad(format!(
                    "class {} bar references unknown spell {}",
                    c.key, id.0
                ));
            }
            if self
                .spell(c.resurrect)
                .is_none_or(|s| s.target != SpellTarget::DeadAlly)
            {
                return bad(format!(
                    "class {} resurrect must be a DeadAlly spell",
                    c.key
                ));
            }
        }
        Ok(())
    }

    fn validate_zones(&self) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        for (i, z) in self.zones.iter().enumerate() {
            if self.zones[..i].iter().any(|o| o.id == z.id) {
                return bad(format!("duplicate zone {}", z.id));
            }
            if z.level_range.0 > z.level_range.1 || z.level_range.0 == 0 {
                return bad(format!("zone {} has a bad level range", z.id));
            }
            let keys = z
                .packs
                .iter()
                .flat_map(|p| p.units.iter())
                .chain(z.boss.iter().map(|b| &b.unit))
                .chain(z.friendly.iter().map(|s| &s.unit))
                .chain(z.rares.iter().map(|s| &s.unit));
            for k in keys {
                if self.unit_def(k).is_none() || self.class_of_unit(k).is_some() {
                    return bad(format!("zone {} spawns invalid unit {k}", z.id));
                }
            }
            if let Some(r) = z.rares.iter().find(|r| {
                self.unit_def(&r.unit)
                    .is_none_or(|u| !u.hostile || u.ai.is_none())
            }) {
                return bad(format!(
                    "zone {}: rare {} needs a hostile with ai",
                    z.id, r.unit
                ));
            }
            for g in z
                .friendly
                .iter()
                .flat_map(|f| f.requires.iter().chain(&f.until))
            {
                self.validate_unlock(g)?;
            }
            for (k, pl) in z.platforms.iter().enumerate() {
                let size_ok = match pl.shape {
                    PlatformShape::Disc { r } => r.is_finite() && r > 0.0,
                    PlatformShape::Rect { hx, hz, yaw_deg } => {
                        hx.is_finite()
                            && hz.is_finite()
                            && yaw_deg.is_finite()
                            && hx > 0.0
                            && hz > 0.0
                    }
                };
                if !(pl.x.is_finite() && pl.z.is_finite() && size_ok)
                    || pl.id.is_empty()
                    || z.platforms[..k].iter().any(|o| o.id == pl.id)
                {
                    return bad(format!(
                        "zone {} has a bad or duplicate platform {}",
                        z.id, pl.id
                    ));
                }
            }
            let outage = z
                .boss
                .as_ref()
                .and_then(|b| self.unit_def(&b.unit))
                .and_then(|u| match &u.encounter {
                    Some(EncounterDef::Outage(o)) => Some(o),
                    _ => None,
                });
            if let Some(o) = outage {
                let regions = z
                    .platforms
                    .iter()
                    .filter(|p| p.kind == PlatformKind::Region)
                    .count();
                let hubs = z
                    .platforms
                    .iter()
                    .filter(|p| p.kind == PlatformKind::Hub)
                    .count();
                let first_ok = z
                    .platforms
                    .iter()
                    .any(|p| p.id == o.first && p.kind == PlatformKind::Region);
                if !(2..=16).contains(&regions) || hubs != 1 || !first_ok {
                    return bad(format!(
                        "zone {}: an outage boss needs 2-16 regions, one hub and a known first region",
                        z.id
                    ));
                }
            }
            for l in &z.links {
                if self.zone(&l.to).is_none() {
                    return bad(format!("zone {} links to unknown zone {}", z.id, l.to));
                }
                if let Some(r) = &l.requires {
                    self.validate_unlock(r)?;
                }
            }
        }
        Ok(())
    }

    fn validate_unlock(&self, r: &Unlock) -> Result<(), ContentError> {
        let ok = match r {
            Unlock::ZoneCleared(z) => self.zone(z).is_some_and(|z| z.boss.is_some()),
            Unlock::BossKilled(k) => self
                .zones
                .iter()
                .any(|z| z.boss.as_ref().is_some_and(|b| b.unit == *k)),
            Unlock::QuestDone(q) | Unlock::QuestStarted(q) => self.quest(q).is_some(),
            Unlock::PlayerLevel(n) => (1..=self.max_level()).contains(n),
            Unlock::AnyOf(any) => {
                return if any.is_empty() {
                    Err(ContentError::Invalid("an empty AnyOf never opens".into()))
                } else {
                    any.iter().try_for_each(|u| self.validate_unlock(u))
                };
            }
        };
        if ok {
            Ok(())
        } else {
            Err(ContentError::Invalid(format!(
                "unlock {r:?} can never happen"
            )))
        }
    }

    fn validate_quests(&self) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        for (i, q) in self.quests.iter().enumerate() {
            if self.quests[..i].iter().any(|o| o.id == q.id) {
                return bad(format!("duplicate quest {}", q.id));
            }
            let gives = self
                .zone(&q.giver.zone)
                .is_some_and(|z| z.friendly.iter().any(|f| f.unit == q.giver.npc));
            if !gives || self.unit_def(&q.giver.npc).is_none_or(|u| u.hostile) {
                return bad(format!(
                    "quest {}: giver {} is not a friendly in {}",
                    q.id, q.giver.npc, q.giver.zone
                ));
            }
            if q.objectives.is_empty() || q.objectives.iter().any(|o| o.need() == 0) {
                return bad(format!("quest {} needs objectives with counts", q.id));
            }
            for o in &q.objectives {
                let ok = match o {
                    Objective::Kill(k, _) | Objective::KillClean(k, _) => {
                        self.unit_def(k).is_some_and(|u| u.hostile)
                    }
                    Objective::Defend {
                        unit,
                        wave,
                        every_ms,
                        label,
                        ..
                    } => {
                        let guarded = self
                            .zone(&q.giver.zone)
                            .is_some_and(|z| z.friendly.iter().any(|f| f.unit == *unit));
                        guarded
                            && self.unit_def(unit).is_some_and(|u| !u.hostile)
                            && self
                                .unit_def(wave)
                                .is_some_and(|u| u.hostile && u.ai.is_some())
                            && *every_ms >= 1000
                            && !label.is_empty()
                    }
                    Objective::Loot(it, _) => self.item(it).is_some(),
                    Objective::Talk(k) => self.unit_def(k).is_some_and(|u| !u.hostile),
                    Objective::ClearZone(z) => self.zone(z).is_some_and(|z| z.boss.is_some()),
                    Objective::Wait { zone, label, .. } => {
                        self.zone(zone).is_some() && !label.is_empty()
                    }
                    Objective::Interact(k, _) => self.prop(k).is_some(),
                    Objective::Collect(tag, n) => {
                        self.tagged_props(tag, crate::world::PropKind::Pickup) >= *n
                    }
                    Objective::Inspect(tag, n) => {
                        self.tagged_props(tag, crate::world::PropKind::Inspect) >= *n
                    }
                    Objective::Sequence { label, props } => {
                        !label.is_empty()
                            && props.len() >= 2
                            && props.iter().all(|p| {
                                self.prop(p)
                                    .is_some_and(|d| d.kind == crate::world::PropKind::Device)
                            })
                    }
                    Objective::UseOn { item, target, n } => {
                        let units = self.unit_def(target).is_some();
                        let props = self
                            .world
                            .props
                            .iter()
                            .filter(|p| p.key == *target || p.tag == *target)
                            .count() as u32;
                        !item.is_empty() && *n >= 1 && (units || props >= *n)
                    }
                    Objective::Deliver { item, npc, .. } => {
                        !item.is_empty() && self.unit_def(npc).is_some_and(|u| !u.hostile)
                    }
                    Objective::Escort {
                        unit,
                        wave,
                        every_ms,
                        label,
                        ..
                    } => {
                        self.unit_def(unit).is_some_and(|u| !u.hostile)
                            && self
                                .unit_def(wave)
                                .is_some_and(|u| u.hostile && u.ai.is_some())
                            && *every_ms >= 1000
                            && !label.is_empty()
                    }
                };
                if !ok {
                    return bad(format!("quest {}: impossible objective {o:?}", q.id));
                }
            }
            if let Some(t) = &q.reward.trophy
                && self.item(t).is_none()
            {
                return bad(format!("quest {} rewards unknown item {t}", q.id));
            }
            if q.reward.tokens > 0
                && self
                    .item(crate::ilmari::TOKEN)
                    .is_none_or(|t| t.kind != ItemKind::Token)
            {
                return bad(format!("quest {} pays tokens that do not exist", q.id));
            }
            let chain_ok = q
                .requires
                .as_ref()
                .is_none_or(|r| self.quests[..i].iter().any(|o| o.id == *r));
            if !chain_ok {
                return bad(format!(
                    "quest {} requires a quest that is not listed before it",
                    q.id
                ));
            }
            if q.level_min == 0 || q.level_min > self.max_level() {
                return bad(format!("quest {} level_min out of range", q.id));
            }
            if let Some(t) = &q.turn_in {
                let takes = self
                    .zone(&t.zone)
                    .is_some_and(|z| z.friendly.iter().any(|f| f.unit == t.npc));
                if !takes {
                    return bad(format!(
                        "quest {}: turn-in {} is not a friendly in {}",
                        q.id, t.npc, t.zone
                    ));
                }
            }
            if q.reward.gear_tier > 5 {
                return bad(format!("quest {} gear tier out of range", q.id));
            }
            if let Some((dept, _)) = &q.reward.rep
                && self.endgame.department_index(dept).is_none()
            {
                return bad(format!("quest {} rewards unknown department {dept}", q.id));
            }
        }
        for u in &self.units {
            for l in &u.loot {
                let Some(id) = &l.quest else { continue };
                let wanted = self.quest(id).is_some_and(|q| {
                    q.objectives
                        .iter()
                        .any(|o| matches!(o, Objective::Loot(it, _) if *it == l.item))
                });
                if !wanted {
                    return bad(format!(
                        "unit {} drops {} for quest {id}, which does not ask for it",
                        u.key, l.item
                    ));
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
        assert_eq!(c.config.xp_per_level, 450);
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
        assert_eq!(c.classes[0].key, "fullstack");
        let p = c.unit_def("senior_engineer").unwrap();
        assert_eq!(
            (p.max_hp, p.max_mana, p.spell_power, p.crit_pct),
            (300, 250, 20, 5)
        );
        assert_eq!(c.floor(5).map(|z| z.id.as_str()), Some("tower/5"));
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
        assert_ne!(broken, SRC);
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
