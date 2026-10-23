//! Offline progress: one player's level, gear, bag, Kudos, trophies and world progress as
//! a small versioned text file (`key=value` lines, lists split by `|`). Decoding is tolerant:
//! unknown keys, bad numbers and names the content no longer has are dropped, never fatal.

use crate::economy::SLOTS;
use crate::sim::Sim;
use crate::targeting::EntityId;

/// Bumped when a field changes meaning (new optional keys, like the daily tasks, need no bump).
/// Version 1 (before talents) still loads, with no picks;
/// versions 1-2 (before the levelling curve) carry `xp` against the old table and are rescaled
/// on import; anything else loads as no save at all.
pub const SAVE_VERSION: u32 = 3;
const OLDEST_VERSION: u32 = 1;
const HEADER: &str = "the-last-dev save";
/// `xp_to_next` of levels 1-9 before save version 3.
const V2_XP_TO_NEXT: [u32; 9] = [400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000];
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
    /// Career Ladder pick per row (0 = none).
    pub talents: crate::talents::TalentPicks,
    /// Sprint mode: recommended next level per building, and the delivered-run leaderboard.
    pub sprint_keys: Vec<(String, u8)>,
    pub sprint_records: Vec<crate::sprint::SprintRecord>,
    /// Decoded from a save before version 3: `xp` is progress against `V2_XP_TO_NEXT`.
    pub xp_v2: bool,
    /// Daily Standup / Sprint Goal: the UTC day and ISO week the joined tasks belong to.
    pub tasks_day: u32,
    pub tasks_week: u32,
    pub tasks: Vec<crate::dailies::SavedTask>,
    /// The "First Day" onboarding is still to be finished (client-owned: the sim exports
    /// false and the client sets it). Optional key: older saves are past onboarding.
    pub onboarding_pending: bool,
    /// Owned pets, the one following ("" for none) and the duel win streak. Optional keys:
    /// older saves simply have none.
    pub pets: Vec<String>,
    pub pet: String,
    pub duel_streak: u32,
    /// Performance Reviews taken (optional key; older saves have none).
    pub respecs: u32,
    /// Endgame (optional keys; older saves have none): Promotion rank and its XP, standing
    /// per department, burnout and the unix time it was saved at (0 = unknown, no holiday).
    pub rank: u8,
    pub rank_xp: u32,
    pub rep: [u32; crate::endgame::DEPARTMENTS],
    pub burnout: u16,
    pub burnout_at: u64,
    /// Legacy Mode lockout (optional keys; older saves have none): the ISO week and the floor
    /// bosses whose bonus loot was taken in it.
    pub legacy_week: u32,
    pub legacy_locks: Vec<String>,
    /// Raid lockout (optional keys; older saves have none): the ISO week and the encounters
    /// killed in it (`async:`-prefixed for Async Attendance).
    pub raid_week: u32,
    pub raid_locks: Vec<String>,
    /// Dungeon lockout (optional key; older saves have none): the UTC day and the encounters
    /// killed in it.
    pub dungeon_day: u32,
    pub dungeon_locks: Vec<String>,
    /// The local Weekly Sprint ladder (optional key; older saves have none).
    pub ladder: crate::sprint::LadderView,
    /// Best medal per minigame key (optional key; older saves have none).
    pub minigames: Vec<(String, crate::minigame::Medal)>,
    /// Quest pickups and inspect spots already used up (`world.props` keys); optional key.
    pub taken_props: Vec<String>,
    /// Ill Marie appearances talked to, one bit each (`quests::MARIE_BEATS`); optional key.
    pub marie_met: u16,
    /// The player's clock at the post-credits unlock (`ilmari_at`), and whether the Opening
    /// played (`ilmari_flags`); optional keys, old saves load as locked.
    pub ilmari_stamp: Option<crate::ilmari::IlmariStamp>,
    pub ilmari_revealed: bool,
    /// Ilmari Tokens earned this ISO week (`ilmari_tok`: week, count); optional key.
    pub ilmari_tokens: (u32, u32),
    /// The companion loadout (`companions`, unit keys); optional key, none by default.
    pub companions: Vec<String>,
    /// UTC day of the last turn-in per daily quest (`quest_days`); optional key.
    pub quest_days: Vec<(String, u32)>,
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
        let talents: Vec<String> = self.talents.iter().map(u8::to_string).collect();
        out.push_str(&format!(
            "achievements={}\ntitle={}\ntalents={}\n",
            self.achievements.join("|"),
            self.title,
            talents.join(",")
        ));
        let keys: Vec<String> = self
            .sprint_keys
            .iter()
            .map(|(b, l)| format!("{b}*{l}"))
            .collect();
        // The name is free text: last field, without the list separator.
        let records: Vec<String> = self
            .sprint_records
            .iter()
            .map(|r| {
                let name: String = r.name.chars().filter(|c| *c != '|').collect();
                format!(
                    "{}*{}*{}*{}*{name}",
                    r.building, r.level, r.time_ms, r.class
                )
            })
            .collect();
        out.push_str(&format!(
            "sprint_keys={}\nsprint_records={}\n",
            keys.join("|"),
            records.join("|")
        ));
        out.push_str(&format!(
            "pets={}\npet={}\nduel_streak={}\n",
            self.pets.join("|"),
            self.pet,
            self.duel_streak
        ));
        out.push_str(&format!("respecs={}\n", self.respecs));
        let rep: Vec<String> = self.rep.iter().map(u32::to_string).collect();
        out.push_str(&format!(
            "rank={}*{}\nrep={}\nburnout={}*{}\n",
            self.rank,
            self.rank_xp,
            rep.join(","),
            self.burnout,
            self.burnout_at
        ));
        if !self.legacy_locks.is_empty() {
            out.push_str(&format!(
                "legacy_locks={}*{}\n",
                self.legacy_week,
                self.legacy_locks.join("|")
            ));
        }
        if !self.raid_locks.is_empty() {
            out.push_str(&format!(
                "raid_locks={}*{}\n",
                self.raid_week,
                self.raid_locks.join("|")
            ));
        }
        if !self.dungeon_locks.is_empty() {
            out.push_str(&format!(
                "dungeon_locks={}*{}\n",
                self.dungeon_day,
                self.dungeon_locks.join("|")
            ));
        }
        if !self.ladder.entries.is_empty() {
            let rows: Vec<String> = self
                .ladder
                .entries
                .iter()
                .map(|e| format!("{}~{}~{}", e.time_ms, e.deaths, e.crew.join("+")))
                .collect();
            out.push_str(&format!("ladder={}*{}\n", self.ladder.week, rows.join("|")));
        }
        if !self.taken_props.is_empty() {
            out.push_str(&format!("taken_props={}\n", self.taken_props.join("|")));
        }
        if self.marie_met != 0 {
            out.push_str(&format!("marie_met={}\n", self.marie_met));
        }
        if let Some(st) = &self.ilmari_stamp {
            out.push_str(&format!(
                "ilmari_at={},{},{}\n",
                st.unix_secs, st.utc_offset_min, st.tz
            ));
            // revealed, phase, attunement bits: the last two belong to later Ilmari builds.
            out.push_str(&format!(
                "ilmari_flags={},0,0\n",
                u8::from(self.ilmari_revealed)
            ));
        }
        if self.ilmari_tokens.1 > 0 {
            out.push_str(&format!(
                "ilmari_tok={},{}\n",
                self.ilmari_tokens.0, self.ilmari_tokens.1
            ));
        }
        if !self.companions.is_empty() {
            out.push_str(&format!("companions={}\n", self.companions.join("|")));
        }
        if !self.quest_days.is_empty() {
            let rows: Vec<String> = self
                .quest_days
                .iter()
                .map(|(k, d)| format!("{k}*{d}"))
                .collect();
            out.push_str(&format!("quest_days={}\n", rows.join("|")));
        }
        if !self.minigames.is_empty() {
            let rows: Vec<String> = self
                .minigames
                .iter()
                .map(|(k, m)| format!("{k}*{}", m.index()))
                .collect();
            out.push_str(&format!("minigames={}\n", rows.join("|")));
        }
        if self.onboarding_pending {
            out.push_str("onboarding=pending\n");
        }
        let tasks: Vec<String> = self
            .tasks
            .iter()
            .map(|t| {
                let c: Vec<String> = t.counts.iter().map(u32::to_string).collect();
                let state = match t.state {
                    crate::dailies::TaskState::Open => 0,
                    crate::dailies::TaskState::Active => 1,
                    crate::dailies::TaskState::Ready => 2,
                    crate::dailies::TaskState::Done => 3,
                };
                format!(
                    "{}*{}*{state}*{}*{}",
                    t.key,
                    u8::from(t.weekly),
                    t.level,
                    c.join(",")
                )
            })
            .collect();
        out.push_str(&format!(
            "tasks_date={}*{}\ntasks={}\n",
            self.tasks_day,
            self.tasks_week,
            tasks.join("|")
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
                "tasks_date" => {
                    let (d, w) = v.split_once('*').unwrap_or((v, "0"));
                    s.tasks_day = d.parse().unwrap_or(0);
                    s.tasks_week = w.parse().unwrap_or(0);
                }
                "tasks" => s.tasks = list(v).iter().filter_map(|t| saved_task(t)).collect(),
                "onboarding" => s.onboarding_pending = v == "pending",
                "achievements" => s.achievements = list(v),
                "pets" => s.pets = list(v),
                "pet" => s.pet = v.chars().take(crate::world::WORLD_KEY_MAX).collect(),
                "duel_streak" => s.duel_streak = v.parse().unwrap_or(0),
                "respecs" => s.respecs = v.parse().unwrap_or(0),
                "taken_props" => s.taken_props = list(v),
                "marie_met" => s.marie_met = v.parse().unwrap_or(0),
                "ilmari_at" => {
                    let mut f = v.splitn(3, ',');
                    if let (Some(t), Some(o), Some(z)) = (f.next(), f.next(), f.next())
                        && let (Ok(t), Ok(o)) = (t.parse(), o.parse())
                    {
                        s.ilmari_stamp = Some(crate::ilmari::IlmariStamp::new(t, o, z));
                    }
                }
                "ilmari_tok" => {
                    let mut f = v.splitn(2, ',').map(|x| x.trim().parse::<u32>());
                    if let (Some(Ok(w)), Some(Ok(n))) = (f.next(), f.next()) {
                        s.ilmari_tokens = (w, n);
                    }
                }
                "companions" => s.companions = list(v),
                "quest_days" => {
                    s.quest_days = list(v)
                        .iter()
                        .filter_map(|r| {
                            let (k, d) = r.split_once('*')?;
                            Some((k.to_string(), d.parse().ok()?))
                        })
                        .collect();
                }
                "ilmari_flags" => {
                    s.ilmari_revealed = v.split(',').next().is_some_and(|r| r.trim() == "1");
                }
                "minigames" => {
                    s.minigames = list(v)
                        .iter()
                        .filter_map(|row| {
                            let (k, m) = row.split_once('*')?;
                            let medal = crate::minigame::Medal::from_index(m.parse().ok()?)?;
                            Some((k.to_string(), medal))
                        })
                        .collect();
                }
                "ladder" => {
                    let (w, rows) = v.split_once('*').unwrap_or(("0", ""));
                    s.ladder.week = w.parse().unwrap_or(0);
                    s.ladder.entries = list(rows)
                        .iter()
                        .filter_map(|r| {
                            let mut f = r.splitn(3, '~');
                            let time_ms = f.next()?.parse().ok()?;
                            let deaths = f.next()?.parse().ok()?;
                            let crew = f
                                .next()
                                .unwrap_or("")
                                .split('+')
                                .filter(|c| !c.is_empty())
                                .map(str::to_string)
                                .collect();
                            Some(crate::sprint::LadderEntry {
                                time_ms,
                                deaths,
                                crew,
                            })
                        })
                        .collect();
                }
                "raid_locks" => {
                    let (w, b) = v.split_once('*').unwrap_or(("0", ""));
                    s.raid_week = w.parse().unwrap_or(0);
                    s.raid_locks = list(b);
                }
                "dungeon_locks" => {
                    let (d, b) = v.split_once('*').unwrap_or(("0", ""));
                    s.dungeon_day = d.parse().unwrap_or(0);
                    s.dungeon_locks = list(b);
                }
                "legacy_locks" => {
                    let (w, b) = v.split_once('*').unwrap_or(("0", ""));
                    s.legacy_week = w.parse().unwrap_or(0);
                    s.legacy_locks = list(b);
                }
                "rank" => {
                    let (r, x) = v.split_once('*').unwrap_or((v, "0"));
                    s.rank = r.parse().unwrap_or(0);
                    s.rank_xp = x.parse().unwrap_or(0);
                }
                "rep" => {
                    for (slot, n) in s.rep.iter_mut().zip(v.split(',')) {
                        *slot = n.trim().parse().unwrap_or(0);
                    }
                }
                "burnout" => {
                    let (b, at) = v.split_once('*').unwrap_or((v, "0"));
                    s.burnout = b.parse().unwrap_or(0);
                    s.burnout_at = at.parse().unwrap_or(0);
                }
                "title" => s.title = v.chars().take(crate::world::WORLD_KEY_MAX).collect(),
                "sprint_keys" => {
                    s.sprint_keys = list(v)
                        .into_iter()
                        .filter_map(|t| {
                            let (b, l) = t.rsplit_once('*')?;
                            Some((b.to_string(), l.parse().ok()?))
                        })
                        .collect()
                }
                "sprint_records" => {
                    s.sprint_records = list(v)
                        .into_iter()
                        .filter_map(|t| {
                            let mut f = t.splitn(5, '*');
                            Some(crate::sprint::SprintRecord {
                                building: f.next()?.to_string(),
                                level: f.next()?.parse().ok()?,
                                time_ms: f.next()?.parse().ok()?,
                                class: f.next()?.chars().take(32).collect(),
                                name: f.next().unwrap_or("").chars().take(32).collect(),
                            })
                        })
                        .collect()
                }
                "talents" => {
                    for (slot, n) in s.talents.iter_mut().zip(v.split(',')) {
                        *slot = n.trim().parse().unwrap_or(0);
                    }
                }
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
        let v = version.filter(|v| (OLDEST_VERSION..=SAVE_VERSION).contains(v))?;
        s.xp_v2 = v < 3;
        Some(s)
    }
}

impl Sim {
    /// The player's persistent progress; `None` for a non-player.
    pub fn export_save(&self, player: EntityId) -> Option<SaveData> {
        let i = self.index(player).filter(|_| self.is_player(player))?;
        let s = &self.units[i].state;
        // Mid-sprint the sim plays in the run's own world; the save keeps the campaign's.
        let world = self.campaign_world();
        let (tasks_day, tasks_week, tasks) = self.export_tasks(player);
        let mut cleared: Vec<String> = world.cleared.iter().cloned().collect();
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
            bosses: world.bosses_killed.iter().cloned().collect(),
            sprint_keys: self.sprint_keys_of(player),
            sprint_records: self.sprint_records(player).to_vec(),
            xp_v2: false,
            tasks_day,
            tasks_week,
            tasks,
            onboarding_pending: false,
            pets: s.pets.clone(),
            pet: s.pet.clone(),
            duel_streak: s.duel_streak,
            respecs: s.respecs,
            rank: s.career.rank,
            rank_xp: s.career.rank_xp,
            rep: s.career.rep,
            burnout: s.career.burnout,
            // Whole minutes: the client rewrites the save only when its text changes.
            burnout_at: if self.clock_on() {
                self.unix_s() / 60 * 60
            } else {
                0
            },
            achievements: s.achievements.clone(),
            title: s.title.clone(),
            talents: s.talents,
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
            legacy_week: self.legacy_lockout().0,
            legacy_locks: self.legacy_lockout().1,
            raid_week: self.raid_lockout().0,
            raid_locks: self.raid_lockout().1,
            dungeon_day: self.dungeon_lockout().0,
            dungeon_locks: self.dungeon_lockout().1,
            ladder: self.ladder_saved(),
            minigames: self.minigame_bests(player),
            taken_props: self.taken_props(player),
            marie_met: self.marie_met(player),
            ilmari_stamp: self.ilmari.stamp.clone(),
            ilmari_revealed: self.ilmari.revealed,
            ilmari_tokens: self.ilmari.tokens.get(&player).copied().unwrap_or_default(),
            companions: self.companion_picks(player),
            quest_days: self
                .quests_of(player)
                .days
                .iter()
                .map(|(k, d)| (k.clone(), *d))
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
        let days = save
            .quest_days
            .iter()
            .filter(|(k, _)| self.content.quest(k).is_some_and(|q| q.daily))
            .cloned()
            .collect();
        let log = self.quest_logs.entry(player).or_default();
        log.active = active;
        log.completed = quests;
        log.kills = kills;
        log.days = days;
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
        self.set_sprint_progress(
            player,
            save.sprint_keys.clone(),
            save.sprint_records.clone(),
        );
        self.set_legacy_lockout(save.legacy_week, &save.legacy_locks);
        self.set_raid_lockout(save.raid_week, &save.raid_locks);
        self.set_dungeon_lockout(save.dungeon_day, &save.dungeon_locks);
        self.set_ladder(save.ladder.week, save.ladder.entries.clone());
        self.set_minigame_bests(player, &save.minigames);
        self.restore_taken_props(player, &save.taken_props);
        self.restore_marie_met(player, save.marie_met);
        self.restore_ilmari(save.ilmari_stamp.clone(), save.ilmari_revealed);
        self.restore_ilmari_tokens(player, save.ilmari_tokens.0, save.ilmari_tokens.1);
        self.set_companion_picks(player, &save.companions);
        let class = self.class_at(i).map(|c| c.key.clone()).unwrap_or_default();
        self.units[i].state.talents = self.content.talents.sanitize(&class, &save.talents, level);
        let title = Some(&save.title)
            .filter(|t| achievements.contains(t) || self.worn_capstone_ok(i, t))
            .cloned()
            .unwrap_or_default();
        let pets: Vec<String> = save
            .pets
            .iter()
            .filter(|p| self.content.pet(p).is_some())
            .fold(Vec::new(), |mut v, p| {
                if !v.contains(p) {
                    v.push(p.clone());
                }
                v
            });
        let u = &mut self.units[i].state;
        u.achievements = achievements;
        u.title = title;
        u.pet = Some(&save.pet)
            .filter(|p| pets.contains(p))
            .cloned()
            .unwrap_or_default();
        u.pets = pets;
        u.duel_streak = save.duel_streak;
        u.respecs = save.respecs;
        let eg = &self.content.endgame;
        let rank = save
            .rank
            .min(eg.ranks.len().min(usize::from(u8::MAX)) as u8);
        let rank_need = eg.rank_xp_to_next(rank);
        let cap = eg.rep_cap();
        let c = &mut u.career;
        c.rank = rank;
        c.rank_xp = save.rank_xp.min(rank_need.saturating_sub(1));
        c.rep = save.rep.map(|r| r.min(cap));
        self.set_level_of(player, level);
        let need = self.content.xp_to_next(level);
        // Keep the share of the level already done, not the raw number, across a table change.
        let old_need = V2_XP_TO_NEXT.get(usize::from(level) - 1).copied();
        let xp = match old_need.filter(|_| save.xp_v2) {
            Some(old) => (u64::from(save.xp) * u64::from(need) / u64::from(old)) as u32,
            None => save.xp,
        };
        self.units[i].state.xp = if need == 0 { 0 } else { xp.min(need - 1) };
        self.refresh_item_bonus(i);
        let u = &mut self.units[i].state;
        u.hp = u.max_hp;
        u.mana = u.max_mana;
        self.import_tasks(player, save.tasks_day, save.tasks_week, &save.tasks);
        self.import_burnout(i, save.burnout, save.burnout_at);
        true
    }
}

/// `key*weekly*state*level*c1,c2`; anything malformed is dropped.
fn saved_task(t: &str) -> Option<crate::dailies::SavedTask> {
    use crate::dailies::TaskState;
    let mut f = t.split('*');
    let key: String = f
        .next()?
        .chars()
        .take(crate::world::WORLD_KEY_MAX)
        .collect();
    let weekly = f.next()? == "1";
    let state = match f.next()? {
        "1" => TaskState::Active,
        "2" => TaskState::Ready,
        "3" => TaskState::Done,
        _ => return None,
    };
    let level = f.next()?.parse().ok()?;
    let counts = f
        .next()
        .unwrap_or("")
        .split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect();
    Some(crate::dailies::SavedTask {
        key,
        weekly,
        state,
        level,
        counts,
    })
}

#[cfg(test)]
mod onboarding_tests {
    use super::SaveData;

    #[test]
    fn onboarding_flag_round_trips_and_old_saves_count_as_onboarded() {
        let pending = SaveData {
            class: "devops".into(),
            level: 1,
            onboarding_pending: true,
            ..SaveData::default()
        };
        let back = SaveData::decode(&pending.encode()).map(|s| s.onboarding_pending);
        assert_eq!(back, Some(true));
        let done = SaveData {
            onboarding_pending: false,
            ..pending
        };
        assert!(!done.encode().contains("onboarding="));
        let old = "the-last-dev save\nversion=3\nclass=devops\nlevel=4\n";
        assert_eq!(
            SaveData::decode(old).map(|s| s.onboarding_pending),
            Some(false)
        );
    }
}
