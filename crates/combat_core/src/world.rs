//! The interactive office: props a player can use (coffee machines, a jammed printer, the fire
//! alarm, parked e-scooters), hidden NPCs, secret achievements with cosmetic titles, and the
//! movement spells every class knows (e-scooter, sprint). Content lives in `Content::world`.
//!
//! Props are placed by the zone layout: anchor `prop:<key>` is the spot a player stands on to
//! use it, so the server validates reach with the same geometry the client draws. Everything
//! that changes state (buffs, Kudos, items, aggro, achievements) is decided here; purely
//! cosmetic props still go through the sim so co-op peers see the same random line.

use crate::content::{Content, ContentError, Effect, SpellDef, SpellId, ticks};
use crate::geom::{Vec3, in_range};
use crate::rng::Rng;
use crate::sim::{Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::types::{AiState, AuraEnd, CastError, Event, ItemError};
use serde::Deserialize;
use std::collections::BTreeMap;

/// A `Stand` achievement counts while the player is this close to the prop's anchor.
pub const STAND_RADIUS_M: f32 = 3.0;
/// Two uses of the same kind of prop further apart than this start a new streak.
pub const STREAK_GAP_MS: u32 = 10_000;
/// Prop and achievement keys travel on the wire; keep them short.
pub const WORLD_KEY_MAX: usize = 48;

fn default_reach() -> f32 {
    4.0
}

fn default_cd() -> u32 {
    1000
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum PropKind {
    Coffee,
    Vending,
    Printer,
    Whiteboard,
    Billboard,
    /// Foosball, ping-pong.
    Table,
    /// The elevator music button.
    Music,
    /// "Days without an AI incident".
    DaysSign,
    FireAlarm,
    Bench,
    AiCar,
    BikeRack,
    /// A parked e-scooter anyone can borrow.
    Scooter,
    /// Sprint planning (board or terminal): the client opens the planner on its own use.
    SprintBoard,
    /// The Daily Standup board: using it shows the player's daily and weekly tasks.
    ScrumBoard,
    /// A nap spot in the open space.
    BeanBag,
    /// The one meeting room nobody may book.
    QuietRoom,
    /// The office dog, who reports to nobody.
    Dog,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PropDef {
    pub key: String,
    pub name: String,
    pub zone: String,
    pub kind: PropKind,
    #[serde(default = "default_reach")]
    pub reach_m: f32,
    #[serde(default)]
    pub cost_sp: u32,
    /// Chance in percent that a use with rewards does nothing (outcome 0).
    #[serde(default)]
    pub fail_pct: u32,
    /// Rewards, one picked at random: (aura key, duration ms) ...
    #[serde(default)]
    pub auras: Vec<(String, u32)>,
    /// ... or one of these goods into the bag (outcome = aura count + index + 1).
    #[serde(default)]
    pub items: Vec<String>,
    /// Per player.
    #[serde(default = "default_cd")]
    pub cooldown_ms: u32,
    /// Scooters: the parked one is gone for everyone this long after a borrow.
    #[serde(default)]
    pub respawn_ms: u32,
    /// Burnout relief (per mille) and its own per-player cooldown in ms (`burnout.rs`), so a
    /// prop with a short use cooldown still cannot be spammed for relief.
    #[serde(default)]
    pub calm: Option<(u16, u32)>,
}

/// A friendly unit spawned at a layout anchor of one zone, outside `ZoneDef::friendly`.
#[derive(Clone, Debug, Deserialize)]
pub struct SecretNpc {
    pub zone: String,
    pub anchor: String,
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Trigger {
    /// `n` uses of props of this kind in a row, each within `STREAK_GAP_MS` of the last.
    Streak(PropKind, u32),
    /// A prop of this kind came up with outcome 0.
    Fail(PropKind),
    /// Stood within `STAND_RADIUS_M` of a prop of this kind for this many seconds.
    Stand(PropKind, u32),
    /// Talked to the friendly unit with this key.
    Talk(String),
    /// Rode an e-scooter.
    Mount,
    /// Has this many achievements.
    Count(u32),
    /// Granted by a live-world event of this key: the world boss kill (its unit key), a
    /// titled Sprint Goal (its task key), clocking out at 18:00 sharp (`go_home`).
    Event(String),
}

#[derive(Clone, Debug, Deserialize)]
pub struct AchievementDef {
    pub key: String,
    pub name: String,
    /// Worn under the player name, shown in angle brackets.
    pub title: String,
    /// Shown once unlocked.
    pub text: String,
    /// Shown while locked.
    pub hint: String,
    pub trigger: Trigger,
    #[serde(default)]
    pub sp: u32,
    /// A trophy (`Content::items`) granted on unlock.
    #[serde(default)]
    pub trophy: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct WorldDef {
    /// Off-bar spells every class knows (e-scooter, sprint).
    #[serde(default)]
    pub spells: Vec<SpellId>,
    #[serde(default)]
    pub props: Vec<PropDef>,
    #[serde(default)]
    pub secrets: Vec<SecretNpc>,
    #[serde(default)]
    pub achievements: Vec<AchievementDef>,
}

impl WorldDef {
    pub(crate) fn validate(&self, c: &Content) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(m));
        for s in &self.spells {
            if c.spell(*s).is_none() {
                return bad(format!("world spell {} unknown", s.0));
            }
        }
        for (i, p) in self.props.iter().enumerate() {
            if self.props[..i].iter().any(|o| o.key == p.key) || p.key.len() > WORLD_KEY_MAX {
                return bad(format!("prop {} duplicate or too long", p.key));
            }
            if c.zone(&p.zone).is_none()
                || p.fail_pct > 100
                || p.reach_m.is_nan()
                || p.reach_m <= 0.0
            {
                return bad(format!(
                    "prop {} needs a known zone, fail_pct <= 100, reach",
                    p.key
                ));
            }
            if let Some((a, _)) = p.auras.iter().find(|(a, _)| c.aura(a).is_none()) {
                return bad(format!("prop {} gives unknown aura {a}", p.key));
            }
            if let Some(n) = p.items.iter().find(|n| c.economy.goods(n).is_none()) {
                return bad(format!("prop {} gives unknown goods {n}", p.key));
            }
        }
        for s in &self.secrets {
            let friendly = c.unit_def(&s.unit).is_some_and(|u| !u.hostile);
            if c.zone(&s.zone).is_none() || !friendly {
                return bad(format!(
                    "secret {} needs a known zone and a friendly unit",
                    s.unit
                ));
            }
        }
        for (i, a) in self.achievements.iter().enumerate() {
            if self.achievements[..i].iter().any(|o| o.key == a.key) || a.key.len() > WORLD_KEY_MAX
            {
                return bad(format!("achievement {} duplicate or too long", a.key));
            }
            let ok = match &a.trigger {
                Trigger::Talk(u) => c.unit_def(u).is_some(),
                Trigger::Count(n) => (*n as usize) < self.achievements.len(),
                _ => true,
            };
            if !ok || a.trophy.as_ref().is_some_and(|t| c.item(t).is_none()) {
                return bad(format!("achievement {} has a bad trigger or trophy", a.key));
            }
        }
        Ok(())
    }
}

impl Content {
    pub fn prop(&self, key: &str) -> Option<&PropDef> {
        self.world.props.iter().find(|p| p.key == key)
    }

    pub fn achievement(&self, key: &str) -> Option<&AchievementDef> {
        self.world.achievements.iter().find(|a| a.key == key)
    }

    /// Movement speed in percent of the run speed with these auras: the largest bonus wins,
    /// so a sprint on a scooter is not a rocket. Client prediction and the server's movement
    /// budget both use it.
    pub fn speed_pct<'a>(&self, auras: impl IntoIterator<Item = &'a str>) -> u32 {
        100 + auras
            .into_iter()
            .filter_map(|k| self.aura(k))
            .map(|a| a.speed_pct)
            .max()
            .unwrap_or(0)
    }

    pub fn is_mount(&self, aura: &str) -> bool {
        self.aura(aura).is_some_and(|a| a.mount)
    }

    /// The ride a spell puts its caster on: (aura key, duration ms).
    pub fn mount_of<'a>(&self, spell: &'a SpellDef) -> Option<(&'a str, u32)> {
        match &spell.effect {
            Effect::Aura { aura, duration_ms } if self.is_mount(aura) => {
                Some((aura.as_str(), *duration_ms))
            }
            _ => None,
        }
    }

    /// The world spell that mounts the e-scooter.
    pub fn mount_spell(&self) -> Option<&SpellDef> {
        self.world
            .spells
            .iter()
            .filter_map(|s| self.spell(*s))
            .find(|s| self.mount_of(s).is_some())
    }

    /// The world spell that is not the ride (Rush to Standup).
    pub fn sprint_spell(&self) -> Option<&SpellDef> {
        self.world
            .spells
            .iter()
            .filter_map(|s| self.spell(*s))
            .find(|s| self.mount_of(s).is_none())
    }
}

#[derive(Default)]
struct PlayerFun {
    next_use: BTreeMap<usize, u64>,
    last: Option<(PropKind, u64)>,
    streak: u32,
    /// Seconds spent next to a prop, by prop index.
    stand: BTreeMap<usize, u32>,
}

pub(crate) struct WorldRt {
    rng: Rng,
    /// Props of the loaded zone: (index into `WorldDef::props`, anchor position).
    props: Vec<(usize, Vec3)>,
    /// Borrowed parked scooters: prop index -> tick it is back.
    parked: BTreeMap<usize, u64>,
    players: BTreeMap<EntityId, PlayerFun>,
}

impl WorldRt {
    /// Own stream, so props never shift combat or loot rolls.
    pub(crate) fn new(seed: u64) -> Self {
        WorldRt {
            rng: Rng::new(seed ^ 0x0FF1_CEC0_FFEE),
            props: Vec::new(),
            parked: BTreeMap::new(),
            players: BTreeMap::new(),
        }
    }
}

impl Sim {
    /// Movement speed of a unit in percent of the run speed (100 without speed auras).
    pub fn move_speed_pct(&self, id: EntityId) -> u32 {
        self.index(id).map_or(100, |i| {
            self.content
                .speed_pct(self.units[i].auras.iter().map(|a| a.key.as_str()))
        })
    }

    /// Where prop `key` is used from in the loaded zone; `None` when the zone lacks it.
    pub fn prop_spot(&self, key: &str) -> Option<Vec3> {
        let k = self.content.world.props.iter().position(|p| p.key == key)?;
        self.fun
            .props
            .iter()
            .find(|(i, _)| *i == k)
            .map(|(_, p)| *p)
    }

    /// Called by `load_zone` once the zone's units exist: props, hidden NPCs, and no riding
    /// into a zone Facilities has not cleared for scooters.
    pub(crate) fn load_world(&mut self, zone: &str, anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>) {
        self.fun.props = self
            .content
            .world
            .props
            .iter()
            .enumerate()
            .filter(|(_, p)| p.zone == zone)
            .filter_map(|(k, p)| anchors(&format!("prop:{}", p.key)).map(|(at, _)| (k, at)))
            .collect();
        let secrets: Vec<(usize, Vec3, f32)> = self
            .content
            .world
            .secrets
            .iter()
            .filter(|s| s.zone == zone)
            .filter_map(|s| {
                let (at, yaw) = anchors(&s.anchor)?;
                Some((self.def_index(&s.unit).ok()?, at, yaw))
            })
            .collect();
        for (def, at, yaw) in secrets {
            let opts = SpawnOpts {
                level: None,
                pack: None,
                respawns: false,
                summoner: None,
                floor_boss: false,
                zone_slot: None,
            };
            self.spawn(def, at, yaw, opts);
        }
        if !self.content.zone(zone).is_some_and(|z| z.scooters) {
            for p in self.players.clone() {
                if let Some(i) = self.index(p) {
                    self.dismount(i);
                }
            }
        }
    }

    pub(crate) fn mounted(&self, i: usize) -> bool {
        self.has_aura_flag(i, |a| a.mount)
    }

    pub(crate) fn dismount(&mut self, i: usize) {
        let keys: Vec<String> = self.units[i]
            .auras
            .iter()
            .filter(|a| self.content.is_mount(&a.key))
            .map(|a| a.key.clone())
            .collect();
        for k in keys {
            self.remove_aura(i, &k, AuraEnd::Consumed);
        }
    }

    /// Mounting needs peace and a zone with scooters; part of `validate`.
    pub(crate) fn mount_check(&self, i: usize, spell: &SpellDef) -> Result<(), CastError> {
        if self.content.mount_of(spell).is_none() {
            return Ok(());
        }
        if self.units[i].state.in_combat {
            return Err(CastError::InCombat);
        }
        let zone_ok = self
            .zone
            .as_deref()
            .and_then(|z| self.content.zone(z))
            .is_some_and(|z| z.scooters);
        if !zone_ok {
            return Err(CastError::NoScooters);
        }
        Ok(())
    }

    /// The mount key while riding gets off instead of casting again, as in WoW.
    pub(crate) fn toggle_mount(&mut self, i: usize, spell: &SpellDef) -> bool {
        if self.content.mount_of(spell).is_none() || !self.mounted(i) {
            return false;
        }
        self.dismount(i);
        true
    }

    /// Any cast other than the ride itself ends the ride.
    pub(crate) fn dismount_for_cast(&mut self, i: usize, spell: &SpellDef) {
        if self.content.mount_of(spell).is_none() {
            self.dismount(i);
        }
    }

    pub(crate) fn on_talk(&mut self, player: EntityId, unit_key: &str) {
        self.check_achievements(player, |t| matches!(t, Trigger::Talk(k) if k == unit_key));
        self.task_talk(player, unit_key);
    }

    pub(crate) fn set_title(&mut self, player: EntityId, key: &str) {
        let Some(i) = self.index(player).filter(|_| self.is_player(player)) else {
            return;
        };
        let capstone = self.worn_capstone_ok(i, key);
        let s = &mut self.units[i].state;
        if (!key.is_empty() && !capstone && !s.achievements.iter().any(|a| a == key))
            || s.title == key
        {
            return;
        }
        s.title = key.to_string();
        self.events.push(Event::TitleChanged {
            unit: player,
            title: key.to_string(),
        });
    }

    /// `key` is the row-5 talent unit `i` picked, so its promotion may be worn as a title.
    pub(crate) fn worn_capstone_ok(&self, i: usize, key: &str) -> bool {
        let class = self.class_at(i).map_or("", |c| c.key.as_str());
        let picks = self.units[i].state.talents;
        self.content
            .talents
            .picked(class, &picks)
            .any(|(r, t)| r + 1 == crate::talents::TALENT_ROWS && t.key == key)
    }

    pub(crate) fn interact(&mut self, player: EntityId, key: &str) {
        let Some(pi) = self.index(player).filter(|_| self.is_player(player)) else {
            return;
        };
        let Some(k) = self.content.world.props.iter().position(|p| p.key == key) else {
            return;
        };
        let Some(spot) = self
            .fun
            .props
            .iter()
            .find(|(i, _)| *i == k)
            .map(|(_, p)| *p)
        else {
            return;
        };
        let def = self.content.world.props[k].clone();
        let s = &self.units[pi].state;
        if !s.alive {
            return self.item_error(player, ItemError::Dead);
        }
        if !in_range(s.pos, spot, def.reach_m) {
            return self.item_error(player, ItemError::TooFar);
        }
        let rewards = !def.auras.is_empty() || !def.items.is_empty();
        if rewards && s.sp < def.cost_sp {
            return self.item_error(player, ItemError::NotEnoughSp);
        }
        let bag_cap = self.content.economy.bag_size.max(1);
        if !def.items.is_empty() && s.bag.len() >= bag_cap {
            return self.item_error(player, ItemError::BagFull);
        }
        let now = self.now;
        let fun = self.fun.players.entry(player).or_default();
        if fun.next_use.get(&k).is_some_and(|t| *t > now) {
            return;
        }
        if def.kind == PropKind::Scooter && !self.borrow_scooter(pi, k, &def) {
            return;
        }
        let fun = self.fun.players.entry(player).or_default();
        fun.next_use.insert(k, now + ticks(def.cooldown_ms));
        let gap = ticks(STREAK_GAP_MS);
        fun.streak = match fun.last {
            Some((kind, t)) if kind == def.kind && now - t <= gap => fun.streak + 1,
            _ => 1,
        };
        fun.last = Some((def.kind, now));
        let streak = fun.streak;
        let outcome = match def.kind {
            PropKind::Scooter => 1,
            PropKind::FireAlarm => self.pull_fire_alarm(player),
            _ if rewards => self.prop_reward(pi, &def),
            _ => self.fun.rng.below(1 << 16),
        };
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key.clone(),
            outcome,
        });
        self.burnout_on_prop(pi, k);
        let kind = def.kind;
        if rewards && outcome == 0 {
            self.check_achievements(player, |t| *t == Trigger::Fail(kind));
        }
        self.check_achievements(
            player,
            |t| matches!(t, Trigger::Streak(want, n) if *want == kind && streak >= *n),
        );
    }

    /// Pays, then rolls: 0 for a failed roll, else the 1-based pick among auras then items.
    fn prop_reward(&mut self, pi: usize, def: &PropDef) -> u32 {
        let player = Self::id_of(pi);
        if def.cost_sp > 0 {
            self.units[pi].state.sp -= def.cost_sp;
            self.bag_event(pi);
        }
        if self.fun.rng.chance_pct(def.fail_pct) {
            return 0;
        }
        let n = (def.auras.len() + def.items.len()) as u32;
        let pick = self.fun.rng.below(n) as usize;
        match def.auras.get(pick) {
            Some((aura, ms)) => self.apply_aura(pi, aura, *ms, None, player, None),
            None => {
                let item = def.items[pick - def.auras.len()].clone();
                self.stow(pi, vec![item]);
                self.bag_event(pi);
            }
        }
        pick as u32 + 1
    }

    /// "Unlocked with your face": the parked scooter becomes yours, no cast bar.
    fn borrow_scooter(&mut self, pi: usize, k: usize, def: &PropDef) -> bool {
        let player = Self::id_of(pi);
        if self.fun.parked.get(&k).is_some_and(|t| *t > self.now) || self.mounted(pi) {
            return false;
        }
        let Some(spell) = self.content.mount_spell().cloned() else {
            return false;
        };
        if let Err(reason) = self.mount_check(pi, &spell) {
            self.emit_to(
                player,
                Event::CastFailed {
                    caster: player,
                    spell: spell.id,
                    reason,
                },
            );
            return false;
        }
        let Some((aura, ms)) = self
            .content
            .mount_of(&spell)
            .map(|(a, m)| (a.to_string(), m))
        else {
            return false;
        };
        self.cancel_cast(pi, None, 0);
        self.apply_aura(pi, &aura, ms, None, player, Some(spell.id));
        self.fun.parked.insert(k, self.now + ticks(def.respawn_ms));
        true
    }

    /// Every living enemy of the zone comes for whoever pulled it. Returns how many.
    fn pull_fire_alarm(&mut self, player: EntityId) -> u32 {
        let mut n = 0;
        for i in 0..self.units.len() {
            let u = &self.units[i];
            let can = !u.despawned
                && u.state.alive
                && u.state.hostile
                && self.content.units[u.def].ai.is_some()
                && u.state.ai != Some(AiState::Evade);
            if can {
                self.add_threat(i, player, 1);
                n += 1;
            }
        }
        n
    }

    pub(crate) fn check_achievements(&mut self, player: EntityId, f: impl Fn(&Trigger) -> bool) {
        let hits: Vec<usize> = self
            .content
            .world
            .achievements
            .iter()
            .enumerate()
            .filter(|(_, a)| f(&a.trigger))
            .map(|(k, _)| k)
            .collect();
        for k in hits {
            self.unlock_achievement(player, k);
        }
    }

    /// Once per player: the reward lands, the title is worn at once, `Count` follows.
    fn unlock_achievement(&mut self, player: EntityId, k: usize) {
        let Some(i) = self.index(player).filter(|_| self.is_player(player)) else {
            return;
        };
        let a = self.content.world.achievements[k].clone();
        let s = &mut self.units[i].state;
        if s.achievements.contains(&a.key) {
            return;
        }
        s.achievements.push(a.key.clone());
        s.title = a.key.clone();
        s.sp = s.sp.saturating_add(a.sp);
        let count = s.achievements.len() as u32;
        if a.sp > 0 {
            self.bag_event(i);
        }
        if let Some(t) = &a.trophy {
            self.add_trophies(i, std::slice::from_ref(t));
        }
        self.events.push(Event::Achievement {
            unit: player,
            key: a.key.clone(),
        });
        self.events.push(Event::TitleChanged {
            unit: player,
            title: a.key,
        });
        self.check_achievements(player, |t| matches!(t, Trigger::Count(n) if count >= *n));
    }

    /// Per tick: rides end in combat, a ride counts for `Mount`, and once a second the
    /// `Stand` timers advance.
    pub(crate) fn update_world(&mut self) {
        for p in self.players.clone() {
            let Some(i) = self.index(p) else { continue };
            if !self.mounted(i) {
                continue;
            }
            if self.units[i].state.in_combat {
                self.dismount(i);
            } else {
                self.check_achievements(p, |t| *t == Trigger::Mount);
            }
        }
        if !self.now.is_multiple_of(ticks(1000)) {
            return;
        }
        let stand_kinds: Vec<PropKind> = self
            .content
            .world
            .achievements
            .iter()
            .filter_map(|a| match a.trigger {
                Trigger::Stand(kind, _) => Some(kind),
                _ => None,
            })
            .collect();
        let props: Vec<(usize, Vec3, PropKind)> = self
            .fun
            .props
            .iter()
            .map(|(k, at)| (*k, *at, self.content.world.props[*k].kind))
            .filter(|(_, _, kind)| stand_kinds.contains(kind))
            .collect();
        if props.is_empty() {
            return;
        }
        for p in self.players.clone() {
            let Some(i) = self.index(p) else { continue };
            let (pos, alive) = (self.units[i].state.pos, self.units[i].state.alive);
            let fun = self.fun.players.entry(p).or_default();
            let mut done = Vec::new();
            for (k, at, kind) in &props {
                if alive && in_range(pos, *at, STAND_RADIUS_M) {
                    let secs = fun.stand.entry(*k).or_insert(0);
                    *secs += 1;
                    done.push((*kind, *secs));
                } else {
                    fun.stand.remove(k);
                }
            }
            for (kind, secs) in done {
                self.check_achievements(
                    p,
                    |t| matches!(t, Trigger::Stand(want, need) if *want == kind && secs >= *need),
                );
            }
        }
    }
}
