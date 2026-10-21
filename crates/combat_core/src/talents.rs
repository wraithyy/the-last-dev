//! Career Ladder: at five levels each class takes one of three promotions. A talent is a list of
//! economy perks (spell modifiers, procs, spell variants), so it runs through the same hooks as
//! gear. Picks live in `UnitState::talents`; resetting them costs Kudos (Performance
//! Review). See docs/GAME_DESIGN.md, Career Ladder.

use crate::content::{Content, ContentError, SpellId};
use crate::economy::{ActivePerk, Perk};
use crate::sim::Sim;
use crate::targeting::EntityId;
use crate::types::{Event, ItemError};
use serde::Deserialize;

pub const TALENT_ROWS: usize = 5;
pub const TALENT_CHOICES: u8 = 3;
/// Per row: 0 = not picked yet, k = option k (1-based).
pub type TalentPicks = [u8; TALENT_ROWS];
/// Longest card text; the window wraps it.
pub const TALENT_TEXT_MAX: usize = 220;

#[derive(Clone, Debug, Deserialize)]
pub struct TalentDef {
    /// Wire and save key; row-5 keys double as wearable titles.
    pub key: String,
    pub name: String,
    pub icon: String,
    /// The promotion on the card ("Senior Prompt Engineer").
    pub title: String,
    pub text: String,
    pub perks: Vec<Perk>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TalentRow {
    pub level: u8,
    /// The career step ("Senior Developer").
    pub name: String,
    pub options: Vec<TalentDef>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClassTalents {
    pub class: String,
    pub rows: Vec<TalentRow>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Talents {
    /// Resetting every pick costs this many Kudos per character level...
    pub respec_sp_per_level: u32,
    /// ...plus this percent of that for every earlier Performance Review.
    pub respec_growth_pct: u32,
    pub classes: Vec<ClassTalents>,
}

impl Talents {
    pub fn for_class(&self, class: &str) -> Option<&ClassTalents> {
        self.classes.iter().find(|c| c.class == class)
    }

    /// The picked talents with their row index.
    pub fn picked<'a>(
        &'a self,
        class: &str,
        picks: &TalentPicks,
    ) -> impl Iterator<Item = (usize, &'a TalentDef)> + 'a {
        let picks = *picks;
        self.for_class(class)
            .into_iter()
            .flat_map(|c| c.rows.iter().enumerate())
            .filter_map(move |(r, row)| {
                let k = usize::from(picks[r]).checked_sub(1)?;
                Some((r, row.options.get(k)?))
            })
    }

    pub fn perks(&self, class: &str, picks: &TalentPicks) -> Vec<ActivePerk> {
        self.picked(class, picks)
            .flat_map(|(_, t)| {
                t.perks.iter().map(|p| ActivePerk {
                    source: t.name.clone(),
                    perk: p.clone(),
                })
            })
            .collect()
    }

    /// What a bar spell casts under these picks (a `Replace` variant or itself).
    pub fn spell_for(&self, class: &str, picks: &TalentPicks, spell: SpellId) -> SpellId {
        self.picked(class, picks)
            .flat_map(|(_, t)| t.perks.iter())
            .find_map(|p| match p {
                Perk::Replace { spell: s, with } if *s == spell => Some(*with),
                _ => None,
            })
            .unwrap_or(spell)
    }

    /// Picked talents that change `spell` (or the variant it is replaced by), for tooltips.
    pub fn notes(&self, class: &str, picks: &TalentPicks, spell: SpellId) -> Vec<&TalentDef> {
        let with = self.spell_for(class, picks, spell);
        self.picked(class, picks)
            .map(|(_, t)| t)
            .filter(|t| {
                t.perks
                    .iter()
                    .any(|p| p.spells().iter().any(|s| *s == spell || *s == with))
            })
            .collect()
    }

    /// The row-5 talent with this key (its title is wearable).
    pub fn capstone(&self, key: &str) -> Option<&TalentDef> {
        self.classes
            .iter()
            .filter_map(|c| c.rows.last())
            .flat_map(|r| r.options.iter())
            .find(|t| t.key == key)
    }

    /// Cost of the next Performance Review after `done` earlier ones.
    pub fn respec_cost(&self, level: u8, done: u32) -> u32 {
        let base = u64::from(self.respec_sp_per_level * u32::from(level));
        let pct = 100 + u64::from(self.respec_growth_pct) * u64::from(done);
        (base * pct / 100).min(u64::from(u32::MAX)) as u32
    }

    /// Drops picks the class does not have or the level does not allow yet.
    pub fn sanitize(&self, class: &str, picks: &TalentPicks, level: u8) -> TalentPicks {
        let mut out = [0; TALENT_ROWS];
        if let Some(c) = self.for_class(class) {
            for (r, row) in c.rows.iter().enumerate().take(TALENT_ROWS) {
                let k = picks[r];
                if k >= 1 && usize::from(k) <= row.options.len() && row.level <= level {
                    out[r] = k;
                }
            }
        }
        out
    }

    pub(crate) fn validate(&self, content: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        let mut keys: Vec<&str> = Vec::new();
        for (i, c) in self.classes.iter().enumerate() {
            let Some(class) = content.class(&c.class) else {
                return bad(format!("talents for unknown class {}", c.class));
            };
            if self.classes[..i].iter().any(|o| o.class == c.class) {
                return bad(format!("duplicate talents for class {}", c.class));
            }
            if c.rows.len() != TALENT_ROWS {
                return bad(format!("class {} needs {TALENT_ROWS} talent rows", c.class));
            }
            let mut prev = 0;
            for row in &c.rows {
                if row.level <= prev || row.level > content.max_level() {
                    return bad(format!(
                        "class {} talent rows need rising levels within the table",
                        c.class
                    ));
                }
                prev = row.level;
                if row.options.len() != usize::from(TALENT_CHOICES) {
                    return bad(format!(
                        "class {} row {} needs {TALENT_CHOICES} options",
                        c.class, row.name
                    ));
                }
                for t in &row.options {
                    if keys.contains(&t.key.as_str())
                        || content.achievement(&t.key).is_some()
                        || t.key.is_empty()
                        || t.key.len() > crate::world::WORLD_KEY_MAX
                    {
                        return bad(format!("talent key {} duplicate or too long", t.key));
                    }
                    keys.push(&t.key);
                    if t.text.chars().count() > TALENT_TEXT_MAX || t.perks.is_empty() {
                        return bad(format!("talent {} needs perks and a short text", t.key));
                    }
                    for p in &t.perks {
                        if p.spells().iter().any(|s| content.spell(*s).is_none()) {
                            return bad(format!("talent {} names an unknown spell", t.key));
                        }
                        if let Perk::Replace { spell, .. } = p
                            && !class.action_bar.contains(spell)
                        {
                            return bad(format!("talent {} replaces an off-bar spell", t.key));
                        }
                        if matches!(p, Perk::OnUse { .. }) {
                            return bad(format!("talent {} cannot have an on-use perk", t.key));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Content {
    /// Text worn under a name for a title key: an achievement's title or a capstone's promotion.
    pub fn title_text(&self, key: &str) -> Option<&str> {
        self.achievement(key)
            .map(|a| a.title.as_str())
            .or_else(|| self.talents.capstone(key).map(|t| t.title.as_str()))
    }
}

impl Sim {
    fn class_key(&self, i: usize) -> String {
        self.class_at(i).map(|c| c.key.clone()).unwrap_or_default()
    }

    /// Perks of unit `i`'s picked talents (players only).
    pub(crate) fn talent_perks(&self, i: usize) -> Vec<ActivePerk> {
        let picks = self.units[i].state.talents;
        self.content.talents.perks(&self.class_key(i), &picks)
    }

    /// The spell a bar press of `spell` casts for unit `i` (its `Replace` variant or itself).
    pub(crate) fn effective_spell(&self, i: usize, spell: SpellId) -> SpellId {
        crate::economy::PerkView(&self.units[i].perks).replaced(spell)
    }

    /// A Replace variant counts as learned when the spell it replaces is.
    pub(crate) fn is_variant_of_known(&self, i: usize, spell: SpellId) -> bool {
        self.units[i]
            .perks
            .iter()
            .any(|p| matches!(p.perk, Perk::Replace { with, .. } if with == spell))
    }

    fn talents_changed(&mut self, i: usize) {
        self.refresh_item_bonus(i);
        let s = &self.units[i].state;
        let e = Event::TalentsChanged {
            unit: s.id,
            talents: s.talents,
        };
        self.events.push(e);
    }

    /// Takes option `choice` (1..=3) of an unpicked row the level allows; out of combat only.
    /// The row-5 promotion is worn as a title at once, like an achievement.
    pub(crate) fn pick_talent(&mut self, pid: EntityId, row: u8, choice: u8) {
        let Some(i) = self.index(pid).filter(|_| self.is_player(pid)) else {
            return;
        };
        let r = usize::from(row);
        let class = self.class_key(i);
        let s = &self.units[i].state;
        let Some(def) = self
            .content
            .talents
            .for_class(&class)
            .and_then(|c| c.rows.get(r))
            .filter(|_| (1..=TALENT_CHOICES).contains(&choice))
        else {
            return self.item_error(pid, ItemError::NotEligible);
        };
        if !s.alive {
            return self.item_error(pid, ItemError::Dead);
        }
        if s.in_combat {
            return self.item_error(pid, ItemError::InCombat);
        }
        if s.level < def.level || s.talents[r] != 0 {
            return self.item_error(pid, ItemError::NotEligible);
        }
        let capstone = (r + 1 == TALENT_ROWS)
            .then(|| def.options.get(usize::from(choice) - 1))
            .flatten()
            .map(|t| t.key.clone());
        self.units[i].state.talents[r] = choice;
        self.talents_changed(i);
        if let Some(key) = capstone {
            self.units[i].state.title.clone_from(&key);
            self.events.push(Event::TitleChanged {
                unit: pid,
                title: key,
            });
        }
    }

    /// Performance Review: clears every pick for `respec_cost` Kudos (it grows per review).
    pub(crate) fn reset_talents(&mut self, pid: EntityId) {
        let Some(i) = self.index(pid).filter(|_| self.is_player(pid)) else {
            return;
        };
        let s = &self.units[i].state;
        let cost = self.content.talents.respec_cost(s.level, s.respecs);
        if s.talents.iter().all(|k| *k == 0) {
            return self.item_error(pid, ItemError::NotEligible);
        }
        if !s.alive {
            return self.item_error(pid, ItemError::Dead);
        }
        if s.in_combat {
            return self.item_error(pid, ItemError::InCombat);
        }
        if s.sp < cost {
            return self.item_error(pid, ItemError::NotEnoughSp);
        }
        let s = &mut self.units[i].state;
        s.sp -= cost;
        s.respecs = s.respecs.saturating_add(1);
        s.talents = [0; TALENT_ROWS];
        let worn_capstone = self.content.talents.capstone(&s.title).is_some();
        if worn_capstone {
            s.title.clear();
        }
        self.talents_changed(i);
        self.bag_event(i);
        if worn_capstone {
            self.events.push(Event::TitleChanged {
                unit: pid,
                title: String::new(),
            });
        }
    }

    /// Debug/test/QA: sets a player's picks directly (sanitised against class and level, no
    /// cost, no title). False for a non-player.
    pub fn set_talents(&mut self, player: EntityId, picks: TalentPicks) -> bool {
        let Some(i) = self.index(player).filter(|_| self.is_player(player)) else {
            return false;
        };
        let class = self.class_key(i);
        let level = self.units[i].state.level;
        self.units[i].state.talents = self.content.talents.sanitize(&class, &picks, level);
        self.talents_changed(i);
        true
    }

    /// The bar spell `spell` as unit `player` casts it (talent variants applied).
    pub fn cast_spell_for(&self, player: EntityId, spell: SpellId) -> SpellId {
        self.index(player).map_or(spell, |i| {
            self.stealth_swap(i, self.effective_spell(i, spell))
        })
    }
}
