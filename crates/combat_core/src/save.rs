//! Offline progress: one player's level, gear, bag, Story Points, trophies and world progress as
//! a small versioned text file (`key=value` lines, lists split by `|`). Decoding is tolerant:
//! unknown keys, bad numbers and names the content no longer has are dropped, never fatal.

use crate::economy::SLOTS;
use crate::sim::Sim;
use crate::targeting::EntityId;

/// Bumped when a field changes meaning; other versions load as no save at all.
pub const SAVE_VERSION: u32 = 1;
const HEADER: &str = "the-last-dev save";
/// Copies of one trophy kept in a save; far above anything a run produces.
const TROPHY_COPIES_MAX: u32 = 99;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SaveData {
    pub class: String,
    pub level: u8,
    pub xp: u32,
    pub sp: u32,
    pub bag: Vec<String>,
    /// By slot index, "" for empty.
    pub equipped: Vec<String>,
    pub trophies: Vec<(String, u32)>,
    pub quests_done: Vec<String>,
    pub cleared: Vec<String>,
    pub bosses: Vec<String>,
    /// Open quests with their objective counts, accept order.
    pub quests_active: Vec<(String, Vec<u32>)>,
    /// Kills per unit key (quest retro credit).
    pub kills: Vec<(String, u32)>,
    /// Unlocked achievement keys and the one whose title is worn ("" for none).
    pub achievements: Vec<String>,
    pub title: String,
}

fn list(v: &str) -> Vec<String> {
    v.split('|')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

impl SaveData {
    pub fn encode(&self) -> String {
        let trophies: Vec<String> = self
            .trophies
            .iter()
            .map(|(n, c)| format!("{n}*{c}"))
            .collect();
        let active: Vec<String> = self
            .quests_active
            .iter()
            .map(|(id, c)| {
                let c: Vec<String> = c.iter().map(u32::to_string).collect();
                format!("{id}:{}", c.join(","))
            })
            .collect();
        let kills: Vec<String> = self.kills.iter().map(|(n, c)| format!("{n}*{c}")).collect();
        let mut out = format!(
            "{HEADER}\nversion={SAVE_VERSION}\nclass={}\nlevel={}\nxp={}\nsp={}\nbag={}\nequipped={}\ntrophies={}\nquests_done={}\ncleared={}\nbosses={}\nquests_active={}\nkills={}\n",
            self.class,
            self.level,
            self.xp,
            self.sp,
            self.bag.join("|"),
            // Keep empty slots positional.
            self.equipped
                .iter()
                .map(|e| if e.is_empty() { "-" } else { e.as_str() })
                .collect::<Vec<_>>()
                .join("|"),
            trophies.join("|"),
            self.quests_done.join("|"),
            self.cleared.join("|"),
            self.bosses.join("|"),
            active.join("|"),
            kills.join("|"),
        );
        out.push_str(&format!(
            "achievements={}\ntitle={}\n",
            self.achievements.join("|"),
            self.title
        ));
        out
    }

    /// `None` for anything that is not a save of this version.
    pub fn decode(text: &str) -> Option<SaveData> {
        let mut lines = text.lines();
        if lines.next()?.trim() != HEADER {
            return None;
        }
        let mut s = SaveData::default();
        let mut version = None;
        for line in lines {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "version" => version = v.parse::<u32>().ok(),
                "class" => s.class = v.chars().take(32).collect(),
                "level" => s.level = v.parse().unwrap_or(1),
                "xp" => s.xp = v.parse().unwrap_or(0),
                "sp" => s.sp = v.parse().unwrap_or(0),
                "bag" => s.bag = list(v),
                "equipped" => {
                    s.equipped = v
                        .split('|')
                        .map(|e| {
                            if e.trim() == "-" {
                                String::new()
                            } else {
                                e.trim().into()
                            }
                        })
                        .collect()
                }
                "trophies" => {
                    s.trophies = list(v)
                        .into_iter()
                        .filter_map(|t| {
                            let (n, c) = t.rsplit_once('*')?;
                            Some((n.to_string(), c.parse().ok()?))
                        })
                        .collect()
                }
                "quests_done" => s.quests_done = list(v),
                "cleared" => s.cleared = list(v),
                "bosses" => s.bosses = list(v),
                "achievements" => s.achievements = list(v),
                "title" => s.title = v.chars().take(crate::world::WORLD_KEY_MAX).collect(),
                "quests_active" => {
                    s.quests_active = list(v)
                        .into_iter()
                        .filter_map(|q| {
                            let (id, c) = q.split_once(':')?;
                            let c = c.split(',').filter_map(|n| n.trim().parse().ok());
                            Some((id.to_string(), c.collect()))
                        })
                        .collect()
                }
                "kills" => {
                    s.kills = list(v)
                        .into_iter()
                        .filter_map(|t| {
                            let (n, c) = t.rsplit_once('*')?;
                            Some((n.to_string(), c.parse().ok()?))
                        })
                        .collect()
                }
                _ => {}
            }
        }
        (version == Some(SAVE_VERSION)).then_some(s)
    }
}

impl Sim {
    /// The player's persistent progress; `None` for a non-player.
    pub fn export_save(&self, player: EntityId) -> Option<SaveData> {
        let i = self.index(player).filter(|_| self.is_player(player))?;
        let s = &self.units[i].state;
        let mut cleared: Vec<String> = self.world.cleared.iter().cloned().collect();
        cleared.sort();
        Some(SaveData {
            class: self.class_at(i).map(|c| c.key.clone()).unwrap_or_default(),
            level: s.level,
            xp: s.xp,
            sp: s.sp,
            bag: s.bag.clone(),
            equipped: s.equipped.clone(),
            trophies: s.inventory.clone(),
            quests_done: self.quests_of(player).completed.clone(),
            cleared,
            bosses: self.world.bosses_killed.iter().cloned().collect(),
            achievements: s.achievements.clone(),
            title: s.title.clone(),
            quests_active: self
                .quests_of(player)
                .active
                .iter()
                .map(|q| (q.id.clone(), q.counts.clone()))
                .collect(),
            kills: self
                .quests_of(player)
                .kills
                .iter()
                .map(|(k, c)| (k.clone(), *c))
                .collect(),
        })
    }

    /// Restores a save onto a freshly spawned player (any class: the save's class is the
    /// caller's business). Unknown names are skipped, numbers clamped, the bag cut to size;
    /// opened zones are marked announced so no unlock banners fire. Full HP and mana after.
    pub fn import_save(&mut self, player: EntityId, save: &SaveData) -> bool {
        let Some(i) = self.index(player).filter(|_| self.is_player(player)) else {
            return false;
        };
        let level = save.level.clamp(1, self.content.max_level());
        let eco = &self.content.economy;
        let bag: Vec<String> = save
            .bag
            .iter()
            .filter(|n| eco.goods(n).is_some())
            .take(eco.bag_size.max(1))
            .cloned()
            .collect();
        let mut equipped = vec![String::new(); SLOTS];
        for (k, name) in save.equipped.iter().enumerate().take(SLOTS) {
            if eco.gear(name).is_some_and(|g| g.slot.index() == k) {
                equipped[k] = name.clone();
            }
        }
        let trophies: Vec<(String, u32)> = save
            .trophies
            .iter()
            .filter(|(n, c)| *c > 0 && self.content.item(n).is_some())
            .map(|(n, c)| (n.clone(), (*c).min(TROPHY_COPIES_MAX)))
            .collect();
        let quests: Vec<String> = save
            .quests_done
            .iter()
            .filter(|q| self.content.quest(q).is_some())
            .cloned()
            .collect();
        for z in save
            .cleared
            .iter()
            .filter(|z| self.content.zone(z).is_some())
        {
            self.world.cleared.insert(z.clone());
        }
        for b in save
            .bosses
            .iter()
            .filter(|b| self.content.unit_def(b).is_some())
        {
            self.world.bosses_killed.insert(b.clone());
        }
        let active: Vec<crate::quests::ActiveQuest> = save
            .quests_active
            .iter()
            .filter(|(id, _)| !quests.contains(id))
            .filter_map(|(id, counts)| {
                let def = self.content.quest(id)?;
                let counts = def
                    .objectives
                    .iter()
                    .enumerate()
                    .map(|(k, o)| counts.get(k).copied().unwrap_or(0).min(o.need()))
                    .collect();
                Some(crate::quests::ActiveQuest {
                    id: id.clone(),
                    counts,
                })
            })
            .collect();
        let kills = save
            .kills
            .iter()
            .filter(|(k, _)| self.content.unit_def(k).is_some())
            .map(|(k, c)| (k.clone(), *c))
            .collect();
        let log = self.quest_logs.entry(player).or_default();
        log.active = active;
        log.completed = quests;
        log.kills = kills;
        let opened: Vec<String> = self
            .content
            .zones
            .iter()
            .flat_map(|z| z.links.iter())
            .filter(|l| l.requires.as_ref().is_some_and(|r| self.unlock_met(r)))
            .map(|l| l.to.clone())
            .collect();
        self.world.announced.extend(opened);
        let u = &mut self.units[i].state;
        u.sp = save.sp;
        u.bag = bag;
        u.equipped = equipped;
        u.inventory = trophies;
        let known = |k: &String| self.content.achievement(k).is_some();
        let achievements: Vec<String> = save
            .achievements
            .iter()
            .filter(|k| known(k))
            .cloned()
            .collect();
        let title = Some(&save.title)
            .filter(|t| achievements.contains(t))
            .cloned()
            .unwrap_or_default();
        let u = &mut self.units[i].state;
        u.achievements = achievements;
        u.title = title;
        self.set_level_of(player, level);
        let need = self.content.xp_to_next(level);
        self.units[i].state.xp = if need == 0 { 0 } else { save.xp.min(need - 1) };
        self.refresh_item_bonus(i);
        let u = &mut self.units[i].state;
        u.hp = u.max_hp;
        u.mana = u.max_mana;
        true
    }
}
