//! "Daily Standup" and "Sprint Goal": repeatable tasks from the campus Scrum board.
//!
//! Every UTC day the board offers `per_day` dailies drawn from `live.dailies` with a generator
//! seeded by the date, and every ISO week one `live.weeklies` entry; the same date gives the
//! same tasks everywhere (offline the device's date, online the server's). A template's goals
//! are resolved for the player when it is shown or joined: kill goals target the zone that fits
//! the player's level among the zones it can reach, totals scale with the level, and so do the
//! rewards (a share of the level's XP bar, Kudos per level). Joining the standup takes
//! all of today's tasks and the week's goal; a daily pays out the moment it is done, the Sprint
//! Goal is claimed at the board with a gear choice. Progress is per player, saved with the
//! date, and thrown away when the date moves on.

use crate::content::{Content, ContentError, Role};
use crate::economy::{GearSource, Rarity};
use crate::geom::{Vec3, in_range};
use crate::live::{day_index, week_index};
use crate::rng::Rng;
use crate::sim::{Sim, SpawnOpts};
use crate::sprint::SprintResult;
use crate::targeting::EntityId;
use crate::types::{Event, ItemError};
use crate::world::PropKind;
use serde::{Deserialize, Serialize};

/// Where the board stands: the Scrum Master Bot is spawned at layout anchor `anchor` of
/// `zone`; any `ScrumBoard` prop of the zone works as well.
#[derive(Clone, Debug, Deserialize)]
pub struct BoardDef {
    pub zone: String,
    pub npc: String,
    pub anchor: String,
}

/// A goal as authored; `resolve` turns it into a `GoalSpec` for one player.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Goal {
    /// Defeat n enemies in the player's work zone.
    KillZone(u32),
    /// Defeat n of one kind from the player's work zone.
    KillKind(u32),
    /// Defeat n elites or bosses anywhere.
    Elite(u32),
    /// Pick up n things from corpses.
    Loot(u32),
    /// Use office equipment n times.
    Props(u32),
    /// Damage plus healing done, n per player level.
    Output(u32),
    /// Finish a Sprint run (delivered or carried over) at a level fitting the player.
    Sprint,
    /// Stand still out of combat in this zone for n seconds in one go.
    Idle(String, u32),
    /// Talk to this friendly unit.
    Talk(String),
    /// Defeat n Crunch-mode night enemies.
    Night(u32),
    /// Defeat the campus world boss.
    WorldBoss,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TaskDef {
    pub key: String,
    pub name: String,
    pub text: String,
    pub goals: Vec<Goal>,
    /// Claiming it grants the achievements with `Trigger::Event(<key>)` (a cosmetic title).
    #[serde(default)]
    pub title: bool,
    /// A consumable or junk item handed out with the reward (lost on a full bag).
    #[serde(default)]
    pub item: Option<String>,
}

/// A goal resolved for one player; travels in `BoardView`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalSpec {
    Kill {
        zone: Option<String>,
        unit: Option<String>,
    },
    Elite,
    Loot,
    Props,
    Output,
    Sprint {
        level: u8,
    },
    Idle {
        zone: String,
    },
    Talk {
        unit: String,
    },
    Night,
    WorldBoss,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalView {
    pub goal: GoalSpec,
    pub count: u32,
    pub need: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    /// On the board, not joined.
    Open,
    Active,
    /// Sprint Goal done; claim it at the board.
    Ready,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskView {
    /// `TaskDef::key` of `live.dailies` (or `live.weeklies` when `weekly`).
    pub key: String,
    pub weekly: bool,
    pub state: TaskState,
    pub goals: Vec<GoalView>,
    pub xp: u32,
    pub sp: u32,
    /// The level the goals and rewards were sized for.
    pub level: u8,
}

/// One player's board: today's dailies, then the week's goal.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardView {
    pub day: u32,
    pub week: u32,
    pub tasks: Vec<TaskView>,
}

/// A task as saved: template key, weekly flag, state, sized-for level, goal counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTask {
    pub key: String,
    pub weekly: bool,
    pub state: TaskState,
    pub level: u8,
    pub counts: Vec<u32>,
}

#[derive(Default)]
pub(crate) struct PlayerTasks {
    pub day: u32,
    pub week: u32,
    /// Joined tasks: today's dailies and this week's goal.
    pub tasks: Vec<TaskView>,
    /// Where the player stood a second ago (Idle goals).
    idle_pos: Option<Vec3>,
    /// A change the client has not been told about yet (sent once a second).
    dirty: bool,
}

impl GoalSpec {
    fn kill_matches(&self, zone: Option<&str>, key: &str) -> bool {
        match self {
            GoalSpec::Kill { zone: z, unit } => {
                z.as_deref().is_none_or(|z| Some(z) == zone)
                    && unit.as_deref().is_none_or(|u| u == key)
            }
            _ => false,
        }
    }
}

impl Content {
    /// The `live.dailies` indices on the board on `day` (distinct, generator seeded by the date).
    pub fn daily_rotation(&self, day: u32) -> Vec<usize> {
        let n = self.live.dailies.len();
        let mut idx: Vec<usize> = (0..n).collect();
        let mut rng = Rng::new(u64::from(day) ^ 0xDA11_7570_0D00);
        let take = self.live.per_day.min(n);
        for k in 0..take {
            let j = k + rng.below((n - k) as u32) as usize;
            idx.swap(k, j);
        }
        idx.truncate(take);
        idx
    }

    /// The `live.weeklies` index of ISO week `week`.
    pub fn weekly_rotation(&self, week: u32) -> Option<usize> {
        let n = self.live.weeklies.len() as u32;
        (n > 0).then(|| Rng::new(u64::from(week) ^ 0x5B71_4760_A100).below(n) as usize)
    }

    pub fn task_def(&self, key: &str, weekly: bool) -> Option<&TaskDef> {
        let list = if weekly {
            &self.live.weeklies
        } else {
            &self.live.dailies
        };
        list.iter().find(|t| t.key == key)
    }

    /// Daily (or weekly) reward at `level`: a share of that level's XP bar (none at the cap)
    /// and Story Points per level.
    pub fn task_reward(&self, weekly: bool, level: u8) -> (u32, u32) {
        let l = &self.live;
        let (pct, sp) = if weekly {
            (l.weekly_xp_pct, l.weekly_sp_per_level)
        } else {
            (l.daily_xp_pct, l.daily_sp_per_level)
        };
        let xp = u64::from(self.xp_to_next(level)) * u64::from(pct) / 100;
        (xp as u32, sp * u32::from(level))
    }

    /// The Sprint Goal's "choose one": best-tier world gear for `level` and `role`.
    pub fn sprint_goal_gear(&self, level: u8, role: Option<Role>) -> Vec<String> {
        self.gear_choices(level.div_ceil(2).max(1), true, role, u64::from(level))
    }

    /// "Choose one" gear of `tier` for a role: world gear, own role or generalist, best rarity
    /// first (Epic only with `epic`), one piece per slot while that still leaves two to choose
    /// from, at most three. `seed` rotates each rarity's candidates so different quests offer
    /// different pieces. Deterministic, so client and sim agree without sending the list.
    pub fn gear_choices(&self, tier: u8, epic: bool, role: Option<Role>, seed: u64) -> Vec<String> {
        const RARITIES: [Rarity; 3] = [Rarity::Epic, Rarity::Rare, Rarity::Uncommon];
        if tier == 0 {
            return Vec::new();
        }
        let rarities = if epic { &RARITIES[..] } else { &RARITIES[1..] };
        let pool: Vec<&crate::economy::GearDef> = rarities
            .iter()
            .flat_map(|r| {
                let mut group: Vec<&crate::economy::GearDef> = self
                    .economy
                    .gear
                    .iter()
                    .filter(|g| {
                        g.source == GearSource::World
                            && g.tier() == tier
                            && g.rarity == *r
                            && (role.is_none() || g.role.is_none() || g.role == role)
                    })
                    .collect();
                if !group.is_empty() {
                    let k = (seed % group.len() as u64) as usize;
                    group.rotate_left(k);
                }
                group
            })
            .collect();
        let mut out: Vec<&crate::economy::GearDef> = Vec::new();
        for g in &pool {
            if out.len() < 3 && !out.iter().any(|o| o.slot == g.slot) {
                out.push(g);
            }
        }
        for g in &pool {
            if out.len() < 2 && !out.iter().any(|o| o.name == g.name) {
                out.push(g);
            }
        }
        out.into_iter().map(|g| g.name.clone()).collect()
    }
}

pub(crate) fn validate_tasks(c: &Content) -> Result<(), ContentError> {
    let bad = |m: String| Err(ContentError::Invalid(m));
    let live = &c.live;
    if let Some(b) = &live.board
        && (c.zone(&b.zone).is_none() || c.unit_def(&b.npc).is_none_or(|u| u.hostile))
    {
        return bad(format!(
            "task board needs a zone and a friendly npc {}",
            b.npc
        ));
    }
    for (weekly, list) in [(false, &live.dailies), (true, &live.weeklies)] {
        for (i, t) in list.iter().enumerate() {
            if list[..i].iter().any(|o| o.key == t.key)
                || t.key.len() > crate::world::WORLD_KEY_MAX
                || t.goals.is_empty()
            {
                return bad(format!(
                    "task {} duplicate, too long or without goals",
                    t.key
                ));
            }
            if let Some(item) = &t.item
                && c.economy.goods(item).is_none()
            {
                return bad(format!("task {} rewards unknown goods {item}", t.key));
            }
            for g in &t.goals {
                let ok = match g {
                    Goal::Idle(z, s) => c.zone(z).is_some() && *s > 0,
                    Goal::Talk(u) => c.unit_def(u).is_some_and(|u| !u.hostile),
                    Goal::WorldBoss => live.boss.is_some(),
                    Goal::Night(n) => *n > 0 && !live.night.spawns.is_empty(),
                    Goal::Sprint => true,
                    Goal::KillZone(n)
                    | Goal::KillKind(n)
                    | Goal::Elite(n)
                    | Goal::Loot(n)
                    | Goal::Props(n)
                    | Goal::Output(n) => *n > 0,
                };
                if !ok {
                    return bad(format!("task {} has a bad goal {g:?}", t.key));
                }
            }
            let titled = c
                .world
                .achievements
                .iter()
                .any(|a| a.trigger == crate::world::Trigger::Event(t.key.clone()));
            if t.title && (!weekly || !titled) {
                return bad(format!(
                    "task {} title needs a weekly and an achievement",
                    t.key
                ));
            }
        }
    }
    Ok(())
}

impl Sim {
    /// Today's board as group member `player` sees it: joined tasks with their progress, the
    /// rest sized for the player's level.
    pub fn task_board(&self, player: EntityId) -> BoardView {
        let unix = self.unix_s();
        let (day, week) = (day_index(unix), week_index(unix));
        let level = self.unit(player).map_or(1, |u| u.level);
        let own = self
            .live
            .tasks
            .get(&player)
            .map_or(&[][..], |t| &t.tasks[..]);
        let mine = |key: &str, weekly: bool| {
            own.iter()
                .find(|t| t.key == key && t.weekly == weekly)
                .cloned()
        };
        let mut tasks = Vec::new();
        for k in self.content.daily_rotation(day) {
            let def = &self.content.live.dailies[k];
            tasks.push(
                mine(&def.key, false)
                    .unwrap_or_else(|| self.resolve_task(def, false, level, day, k)),
            );
        }
        if let Some(k) = self.content.weekly_rotation(week) {
            let def = &self.content.live.weeklies[k];
            tasks.push(
                mine(&def.key, true)
                    .unwrap_or_else(|| self.resolve_task(def, true, level, week, k)),
            );
        }
        BoardView { day, week, tasks }
    }

    /// The zone whose band fits `level` best among the zones the group can reach: the highest
    /// band start at or below the level whose band still reaches it, else the highest band
    /// start below it (the board's zone when nothing fits).
    pub fn work_zone(&self, level: u8) -> String {
        let home = self
            .content
            .live
            .board
            .as_ref()
            .map_or("campus", |b| b.zone.as_str());
        let reach = self.reachable_zones();
        let open = |z: &str| reach.iter().any(|r| r == z);
        self.content
            .zones
            .iter()
            // Dungeons and raids are instanced group content, never a daily's work zone.
            .filter(|z| {
                !z.packs.is_empty()
                    && open(&z.id)
                    && z.level_range.0 <= level
                    && self.content.raid_of(&z.id).is_none()
            })
            // In band first, then the highest band start; ties keep content order (home first).
            .min_by_key(|z| std::cmp::Reverse((level <= z.level_range.1 + 1, z.level_range.0)))
            .map_or_else(|| home.to_string(), |z| z.id.clone())
    }

    fn resolve_task(
        &self,
        def: &TaskDef,
        weekly: bool,
        level: u8,
        salt: u32,
        k: usize,
    ) -> TaskView {
        let goals = def
            .goals
            .iter()
            .enumerate()
            .map(|(g, goal)| {
                let (goal, need) =
                    self.resolve_goal(goal, level, u64::from(salt) * 131 + (k * 7 + g) as u64);
                GoalView {
                    goal,
                    count: 0,
                    need,
                }
            })
            .collect();
        let (xp, sp) = self.content.task_reward(weekly, level);
        TaskView {
            key: def.key.clone(),
            weekly,
            state: TaskState::Open,
            goals,
            xp,
            sp,
            level,
        }
    }

    fn resolve_goal(&self, goal: &Goal, level: u8, salt: u64) -> (GoalSpec, u32) {
        let zone = self.work_zone(level);
        let hostiles = self.content.zone_hostiles(&zone);
        // Zone trash never respawns within a load, so a goal past the zone's headcount could
        // never be finished in one pass.
        let kill_zone = |n: u32| {
            (
                GoalSpec::Kill {
                    zone: Some(zone.clone()),
                    unit: None,
                },
                n.min(hostiles.len() as u32),
            )
        };
        match goal {
            Goal::KillZone(n) => kill_zone(*n),
            Goal::KillKind(n) => {
                let units: Vec<&String> = self
                    .content
                    .zone(&zone)
                    .map(|z| z.packs.iter().flat_map(|p| p.units.iter()).collect())
                    .unwrap_or_default();
                let mut kinds: Vec<&String> = Vec::new();
                for u in units {
                    if !kinds.contains(&u) {
                        kinds.push(u);
                    }
                }
                match kinds.get((salt % kinds.len().max(1) as u64) as usize) {
                    Some(u) => (
                        GoalSpec::Kill {
                            zone: Some(zone.clone()),
                            unit: Some((*u).clone()),
                        },
                        (*n).min(hostiles.iter().filter(|h| **h == u.as_str()).count() as u32),
                    ),
                    None => kill_zone(*n),
                }
            }
            Goal::Elite(n) => (GoalSpec::Elite, *n),
            // Loot rolls are random; one corpse per hostile is the most a zone can promise.
            Goal::Loot(n) => (GoalSpec::Loot, (*n).min(hostiles.len() as u32)),
            Goal::Props(n) => (GoalSpec::Props, *n),
            Goal::Output(n) => (GoalSpec::Output, n.saturating_mul(u32::from(level))),
            Goal::Sprint => {
                let s = &self.content.sprint;
                let unlocked = level >= s.unlock_level
                    || self.campaign_world().bosses_killed.contains(&s.unlock_boss);
                if s.buildings.is_empty() || !unlocked {
                    kill_zone(12)
                } else {
                    (
                        GoalSpec::Sprint {
                            level: 1 + level / 4,
                        },
                        1,
                    )
                }
            }
            Goal::Idle(z, secs) => {
                let reachable = self.reachable_zones().contains(z);
                let z = if reachable {
                    z.clone()
                } else {
                    self.content
                        .live
                        .board
                        .as_ref()
                        .map_or_else(|| "campus".to_string(), |b| b.zone.clone())
                };
                (GoalSpec::Idle { zone: z }, *secs)
            }
            Goal::Talk(u) => (GoalSpec::Talk { unit: u.clone() }, 1),
            Goal::Night(n) => (GoalSpec::Night, *n),
            Goal::WorldBoss => (GoalSpec::WorldBoss, 1),
        }
    }

    /// Zones reachable from the board's zone through open links (the campaign's unlocks).
    fn reachable_zones(&self) -> Vec<String> {
        let home = self
            .content
            .live
            .board
            .as_ref()
            .map_or("campus", |b| b.zone.as_str());
        let mut seen = vec![home.to_string()];
        let mut k = 0;
        while let Some(z) = seen.get(k).cloned() {
            k += 1;
            let Some(def) = self.content.zone(&z) else {
                continue;
            };
            for l in &def.links {
                let open = l.requires.as_ref().is_none_or(|r| self.unlock_met(r));
                if open && !seen.contains(&l.to) {
                    seen.push(l.to.clone());
                }
            }
        }
        seen
    }

    /// Spawns the Scrum Master Bot when the zone's layout has its anchor.
    pub(crate) fn load_board(&mut self, zone: &str, anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>) {
        self.live.board_npc = None;
        let Some(b) = self.content.live.board.clone().filter(|b| b.zone == zone) else {
            return;
        };
        let (Some((at, yaw)), Ok(def)) = (anchors(&b.anchor), self.def_index(&b.npc)) else {
            return;
        };
        let opts = SpawnOpts {
            level: None,
            pack: None,
            respawns: false,
            summoner: None,
            floor_boss: false,
            zone_slot: None,
        };
        self.live.board_npc = Some(self.spawn(def, at, yaw, opts));
    }

    fn near_board(&self, player: EntityId) -> bool {
        let Some(p) = self.unit(player).filter(|p| p.alive) else {
            return false;
        };
        let reach = self.content.config.loot_range_m + crate::quests::GIVER_REACH_SLACK_M;
        let npc = self
            .live
            .board_npc
            .and_then(|id| self.unit(id))
            .is_some_and(|n| n.alive && in_range(n.pos, p.pos, reach));
        let board = self
            .content
            .world
            .props
            .iter()
            .filter(|d| d.kind == PropKind::ScrumBoard)
            .filter_map(|d| self.prop_spot(&d.key).map(|at| (at, d.reach_m)))
            .any(|(at, r)| in_range(at, p.pos, r + crate::quests::GIVER_REACH_SLACK_M));
        npc || board
    }

    fn show_board(&mut self, player: EntityId, open: bool) {
        let board = self.task_board(player);
        if let Some(t) = self.live.tasks.get_mut(&player) {
            t.dirty = false;
        }
        self.emit_to(player, Event::DailyBoard { board, open });
    }

    /// Talking to anyone: Talk goals; the Scrum Master Bot opens the board.
    pub(crate) fn task_talk(&mut self, player: EntityId, unit_key: &str) {
        self.task_credit(player, |g| {
            matches!(g, GoalSpec::Talk { unit } if unit == unit_key).then_some(1)
        });
        let board = self
            .content
            .live
            .board
            .as_ref()
            .is_some_and(|b| b.npc == unit_key);
        if board {
            self.show_board(player, true);
        }
    }

    /// `Command::JoinStandup`: takes every task on today's board the player has not joined.
    pub(crate) fn join_standup(&mut self, player: EntityId) {
        if !self.is_player(player) || !self.near_board(player) {
            return;
        }
        let board = self.task_board(player);
        let st = self.live.tasks.entry(player).or_default();
        st.day = board.day;
        st.week = board.week;
        let mut joined = false;
        for mut t in board.tasks {
            if t.state == TaskState::Open {
                t.state = TaskState::Active;
                st.tasks.push(t);
                joined = true;
            }
        }
        if joined {
            // Idle and Talk goals may already be met by standing here; check on the next second.
            self.show_board(player, true);
        }
    }

    /// `Command::ClaimSprintGoal`: the finished weekly pays out at the board; `choice` picks
    /// from `Content::sprint_goal_gear` (clamped). A full bag refuses it with `BagFull`.
    pub(crate) fn claim_sprint_goal(&mut self, player: EntityId, choice: u8) {
        let Some(pi) = self.index(player).filter(|_| self.is_player(player)) else {
            return;
        };
        if !self.near_board(player) {
            return;
        }
        let Some(k) = self.live.tasks.get(&player).and_then(|t| {
            t.tasks
                .iter()
                .position(|t| t.weekly && t.state == TaskState::Ready)
        }) else {
            return;
        };
        let task = self.live.tasks[&player].tasks[k].clone();
        let role = self.class_at(pi).map(|c| c.role);
        let gear = self.content.sprint_goal_gear(task.level, role);
        let pick = gear
            .get(usize::from(choice).min(gear.len().saturating_sub(1)))
            .cloned();
        if pick.is_some() && self.bag_full(pi) {
            return self.item_error(player, ItemError::BagFull);
        }
        if let Some(st) = self.live.tasks.get_mut(&player) {
            st.tasks[k].state = TaskState::Done;
        }
        if let Some(g) = pick {
            self.stow(pi, vec![g]);
            self.bag_event(pi);
        }
        self.pay_task(pi, &task);
        let title = self
            .content
            .task_def(&task.key, true)
            .is_some_and(|d| d.title);
        if title {
            let key = task.key.clone();
            self.check_achievements(
                player,
                |t| matches!(t, crate::world::Trigger::Event(k) if *k == key),
            );
        }
        self.show_board(player, true);
    }

    fn pay_task(&mut self, pi: usize, task: &TaskView) {
        let player = Sim::id_of(pi);
        self.emit_to(
            player,
            Event::TaskCompleted {
                key: task.key.clone(),
                weekly: task.weekly,
                xp: task.xp,
                sp: task.sp,
            },
        );
        self.grant_xp(pi, task.xp);
        self.rep_for_task(pi, task.weekly);
        let item = self
            .content
            .task_def(&task.key, task.weekly)
            .and_then(|d| d.item.clone());
        if let Some(item) = item {
            if self.stow(pi, vec![item]).is_empty() {
                self.bag_event(pi);
            } else {
                self.item_error(player, ItemError::BagFull);
            }
        }
        if task.sp > 0 {
            let s = &mut self.units[pi].state;
            s.sp = s.sp.saturating_add(task.sp);
            self.bag_event(pi);
        }
    }

    /// Adds `f(goal)` to every matching goal of `player`'s active tasks; a finished daily pays
    /// out, a finished weekly waits at the board.
    pub(crate) fn task_credit(&mut self, player: EntityId, f: impl Fn(&GoalSpec) -> Option<u32>) {
        let Some(st) = self.live.tasks.get_mut(&player) else {
            return;
        };
        let mut finished = Vec::new();
        for (k, t) in st.tasks.iter_mut().enumerate() {
            if t.state != TaskState::Active {
                continue;
            }
            for g in &mut t.goals {
                if g.count < g.need
                    && let Some(n) = f(&g.goal)
                    && n > 0
                {
                    g.count = g.count.saturating_add(n).min(g.need);
                    st.dirty = true;
                }
            }
            if t.goals.iter().all(|g| g.count >= g.need) {
                t.state = if t.weekly {
                    TaskState::Ready
                } else {
                    TaskState::Done
                };
                finished.push(k);
            }
        }
        let Some(pi) = self.index(player) else { return };
        for k in finished {
            let task = self.live.tasks[&player].tasks[k].clone();
            if task.weekly {
                // Tell the tracker at once: the claim is a trip to the board.
                self.show_board(player, false);
            } else {
                self.pay_task(pi, &task);
            }
        }
    }

    /// A kill credited to `player` (own or shared, see `record_kill`).
    pub(crate) fn task_kill(&mut self, player: EntityId, key: &str) {
        if !self.live.tasks.contains_key(&player) {
            return;
        }
        let zone = self.zone.clone();
        let (elite, boss) = self
            .content
            .unit_def(key)
            .map_or((false, false), |u| (u.elite, u.boss));
        let night = self.content.is_night_unit(key);
        let world_boss = self.content.world_boss().is_some_and(|b| b.unit == key);
        self.task_credit(player, |g| {
            let hit = match g {
                GoalSpec::Elite => elite || boss,
                GoalSpec::Night => night,
                GoalSpec::WorldBoss => world_boss,
                g => g.kill_matches(zone.as_deref(), key),
            };
            hit.then_some(1)
        });
    }

    /// This tick's events that move tasks: loot, props, throughput, sprint results; and the
    /// board prop opening the board.
    pub(crate) fn scan_task_events(&mut self) {
        if self.live.tasks.is_empty() && self.content.live.board.is_none() {
            return;
        }
        let mut credit: Vec<(EntityId, u8, u32)> = Vec::new();
        let mut open = Vec::new();
        for e in &self.events {
            match e {
                Event::Looted { looter, items, .. } if !items.is_empty() => {
                    credit.push((*looter, 0, items.len() as u32))
                }
                // Re-pressing a pickup already taken is no activity.
                Event::Interacted { outcome, .. } if *outcome == crate::QUEST_PROP_TAKEN => {}
                Event::Interacted { unit, prop, .. } => {
                    match self.content.prop(prop).map(|p| p.kind) {
                        Some(PropKind::ScrumBoard) => open.push(*unit),
                        Some(PropKind::SprintBoard) | None => {}
                        Some(_) => credit.push((*unit, 1, 1)),
                    }
                }
                Event::Damage { source, amount, .. } | Event::Heal { source, amount, .. }
                    if *amount > 0 =>
                {
                    credit.push((*source, 2, *amount))
                }
                Event::SprintEnded {
                    result, summary, ..
                } if *result != SprintResult::Abandoned => {
                    for p in &self.players {
                        credit.push((*p, 3, u32::from(summary.status.level)));
                    }
                }
                _ => {}
            }
        }
        for (p, kind, n) in credit {
            if !self.live.tasks.contains_key(&p) {
                continue;
            }
            self.task_credit(p, |g| match (kind, g) {
                (0, GoalSpec::Loot) | (1, GoalSpec::Props) | (2, GoalSpec::Output) => Some(n),
                (3, GoalSpec::Sprint { level }) => (n >= u32::from(*level)).then_some(1),
                _ => None,
            });
        }
        for p in open {
            self.show_board(p, true);
        }
    }

    /// Once a second: date rollover, Idle goals, and the board updates the client is owed.
    pub(crate) fn tasks_second(&mut self) {
        if self.live.tasks.is_empty() {
            return;
        }
        let unix = self.unix_s();
        let (day, week) = (day_index(unix), week_index(unix));
        let zone = self.zone.clone();
        let players: Vec<EntityId> = self.live.tasks.keys().copied().collect();
        for p in players {
            let me = self.unit(p).map(|u| (u.pos, u.alive && !u.in_combat));
            let casting = self.index(p).is_some_and(|i| self.units[i].cast.is_some());
            let Some(st) = self.live.tasks.get_mut(&p) else {
                continue;
            };
            if st.day != day {
                st.day = day;
                st.tasks.retain(|t| t.weekly);
                st.dirty = true;
            }
            if st.week != week {
                st.week = week;
                st.tasks.retain(|t| !t.weekly);
                st.dirty = true;
            }
            let (pos, calm) = me.unwrap_or((Vec3::new(0.0, 0.0, 0.0), false));
            let still = calm && !casting && st.idle_pos.is_some_and(|q| in_range(q, pos, 0.05));
            st.idle_pos = Some(pos);
            let mut reset = false;
            for t in st.tasks.iter_mut().filter(|t| t.state == TaskState::Active) {
                for g in &mut t.goals {
                    if let GoalSpec::Idle { zone: z } = &g.goal
                        && (!still || zone.as_deref() != Some(z.as_str()))
                        && g.count > 0
                        && g.count < g.need
                    {
                        g.count = 0;
                        reset = true;
                    }
                }
            }
            if reset {
                st.dirty = true;
            }
            if still {
                let here = zone.clone();
                self.task_credit(p, |g| {
                    matches!(g, GoalSpec::Idle { zone: z } if Some(z) == here.as_ref()).then_some(1)
                });
            }
            if self.live.tasks.get(&p).is_some_and(|t| t.dirty) {
                self.show_board(p, false);
            }
        }
    }

    /// The player's joined tasks for the save, with the date they belong to.
    pub(crate) fn export_tasks(&self, player: EntityId) -> (u32, u32, Vec<SavedTask>) {
        let Some(st) = self.live.tasks.get(&player) else {
            return (0, 0, Vec::new());
        };
        let tasks = st
            .tasks
            .iter()
            .map(|t| SavedTask {
                key: t.key.clone(),
                weekly: t.weekly,
                state: t.state,
                level: t.level,
                counts: t.goals.iter().map(|g| g.count).collect(),
            })
            .collect();
        (st.day, st.week, tasks)
    }

    /// Restores saved tasks: goals are sized again for the saved level and date; tasks the
    /// content no longer has are dropped. A stale date is cleared on the next second.
    pub(crate) fn import_tasks(
        &mut self,
        player: EntityId,
        day: u32,
        week: u32,
        saved: &[SavedTask],
    ) {
        let mut tasks = Vec::new();
        for s in saved {
            let list = if s.weekly {
                &self.content.live.weeklies
            } else {
                &self.content.live.dailies
            };
            let Some(k) = list.iter().position(|t| t.key == s.key) else {
                continue;
            };
            if s.state == TaskState::Open
                || tasks
                    .iter()
                    .any(|t: &TaskView| t.key == s.key && t.weekly == s.weekly)
            {
                continue;
            }
            let level = s.level.clamp(1, self.content.max_level());
            let salt = if s.weekly { week } else { day };
            let mut t = self.resolve_task(&list[k].clone(), s.weekly, level, salt, k);
            t.state = s.state;
            for (g, c) in t.goals.iter_mut().zip(&s.counts) {
                g.count = (*c).min(g.need);
            }
            tasks.push(t);
        }
        if tasks.is_empty() {
            self.live.tasks.remove(&player);
            return;
        }
        self.live.tasks.insert(
            player,
            PlayerTasks {
                day,
                week,
                tasks,
                idle_pos: None,
                dirty: true,
            },
        );
    }

    /// Debug/QA: joins today's board for `player` wherever they are (no board in reach needed).
    pub fn debug_join_standup(&mut self, player: EntityId) -> bool {
        if !self.is_player(player) {
            return false;
        }
        let board = self.task_board(player);
        let st = self.live.tasks.entry(player).or_default();
        st.day = board.day;
        st.week = board.week;
        for mut t in board.tasks {
            if t.state == TaskState::Open {
                t.state = TaskState::Active;
                st.tasks.push(t);
            }
        }
        st.dirty = true;
        true
    }

    /// Debug/QA: sets the counts of `player`'s joined task `key` (capped at each need), then
    /// settles it like progress would (a full daily pays out).
    pub fn debug_task_counts(&mut self, player: EntityId, key: &str, counts: &[u32]) -> bool {
        let Some(t) = self.live.tasks.get_mut(&player).and_then(|st| {
            st.tasks
                .iter_mut()
                .find(|t| t.key == key && t.state == TaskState::Active)
        }) else {
            return false;
        };
        for (g, c) in t.goals.iter_mut().zip(counts) {
            g.count = (*c).min(g.need).min(g.need.saturating_sub(1));
        }
        // The last step goes through the normal credit path, so payouts behave as in play.
        let missing: Vec<usize> = t
            .goals
            .iter()
            .enumerate()
            .filter(|(k, g)| counts.get(*k).is_some_and(|c| *c >= g.need))
            .map(|(k, _)| k)
            .collect();
        for k in missing {
            if let Some(g) = self
                .live
                .tasks
                .get_mut(&player)
                .and_then(|st| st.tasks.iter_mut().find(|t| t.key == key))
                .and_then(|t| t.goals.get_mut(k))
            {
                g.count = g.need - 1;
                let spec = g.goal.clone();
                self.task_credit(player, |x| (*x == spec).then_some(1));
            }
        }
        if let Some(st) = self.live.tasks.get_mut(&player) {
            st.dirty = true;
        }
        true
    }
}
