//! The deterministic combat simulation: commands in, events out, one 1/30 s tick at a time.
//!
//! All timers are integer ticks derived from content milliseconds; nothing
//! here reads a wall clock or frame time. Unit ids are `index + 1` and are never
//! reused (despawned units stay in the vector, hidden from the public views).

use crate::content::{
    AuraDef, Content, ContentError, ItemTotals, Objective, School, SpellDef, SpellId, SpellTarget,
    YELL_KINDS, YellKind, ticks,
};
use crate::geom::{Aabb, Vec3, in_range, line_of_sight};
use crate::nav::{Nav, NavAgent};
use crate::quests::QuestLog;
use crate::rng::Rng;
use crate::targeting::{EntityId, TargetCandidate, validate_target};
use crate::time::{Tick, ticks_to_ms};
use crate::types::*;
use crate::wander::Wander;
use crate::zones::{World, floor_number};
use std::collections::BTreeMap;

/// Friendly ambient chatter is only emitted while the player is this close.
const AMBIENT_RANGE_M: f32 = 15.0;
/// Co-op group size cap; `spawn_player` refuses more.
pub const MAX_PLAYERS: usize = 5;
/// Minimum gap between two throttled hostile yells across all units.
const HOSTILE_YELL_GAP_MS: u32 = 2500;

/// A cast in progress. `end` moves later when pushback hits.
pub(crate) struct ActiveCast {
    pub spell: SpellId,
    pub target: Option<EntityId>,
    pub start: u64,
    pub end: u64,
    pub pushbacks: u8,
    pub interruptible: bool,
    /// Next mana tick for a channel (`ChannelDef`); `None` for ordinary casts.
    pub channel_next: Option<u64>,
}

pub(crate) struct AuraRt {
    pub key: String,
    pub total_ms: u32,
    pub expires: u64,
    pub absorb: Option<u32>,
    pub stacks: u8,
    pub source: EntityId,
    pub spell: Option<SpellId>,
    /// Periodic amount per tick, snapshotted from the caster on application.
    pub tick_amount: u32,
    pub next_tick: u64,
    pub expire_amount: u32,
}

pub(crate) struct Unit {
    pub state: UnitState,
    /// Index into `Content::units`.
    pub def: usize,
    pub spawn_pos: Vec3,
    pub spawn_yaw: f32,
    pub gcd_end: u64,
    pub gcd_total_ms: u32,
    pub cooldowns: BTreeMap<SpellId, (u64, u32)>,
    /// Per-school lockout end tick, indexed by `School::index`.
    pub lockout_until: [u64; 2],
    pub cast: Option<ActiveCast>,
    pub queued: Option<SpellId>,
    /// Moving or jumping: either blocks cast-time spells.
    pub moving: bool,
    /// BTreeMap keeps tie-breaks (lowest id wins) deterministic.
    pub threat: BTreeMap<EntityId, u32>,
    pub melee_next: u64,
    /// Persists across toggles so stop/start cannot reset the swing timer.
    pub swing_next: u64,
    /// Next use per `AiDef::abilities` entry.
    pub ability_next: Vec<u64>,
    pub ability_yell_cursor: Vec<usize>,
    pub auras: Vec<AuraRt>,
    /// Mana regen remainder, in 1/150 mana (5 s = 150 ticks).
    pub regen_acc: u32,
    /// Next out-of-combat regen tick; re-armed whenever the unit leaves combat or spawns.
    pub regen_next: u64,
    /// Player spell yells: earliest tick for the next one, and a round-robin cursor per spell.
    pub spell_yell_next: u64,
    pub spell_yell_cursor: BTreeMap<SpellId, usize>,
    pub respawn_at: Option<u64>,
    /// Tick of the last death, and when that corpse despawns (`refresh_corpse`).
    pub died_at: u64,
    pub corpse_until: Option<u64>,
    /// One roll per group member (`None` when the kill happened with no player in the sim).
    pub loot: Vec<LootShare>,
    /// Cached sum of the player's trophy bonuses; zero for everyone else.
    pub item_bonus: ItemTotals,
    /// Highest-priority yell requested this tick (with an explicit line for
    /// ability and phase yells); flushed once at end of tick.
    pub yell_pending: Option<(YellKind, Option<String>)>,
    /// Round-robin cursor per `YellKind`.
    pub yell_cursor: [usize; YELL_KINDS],
    pub yell_periodic_next: u64,
    pub yell_low_done: bool,
    pub ambient_next: u64,
    /// Effective spell power (player: authored + level bonus).
    pub sp: u32,
    /// Level scaling of everything this unit deals or heals, in percent.
    pub dmg_pct: u32,
    /// Hand-spawned units respawn; floor spawns and summons stay dead.
    pub respawns: bool,
    pub summoner: Option<EntityId>,
    pub despawned: bool,
    pub floor_boss: bool,
    /// Pack slot in the current zone (for the trash respawn timer); `None` for everything else.
    pub zone_slot: Option<usize>,
    pub last_cast: Option<SpellId>,
    /// Last damaging player spell that landed on this unit: (pre-absorb amount, school).
    pub last_player_hit: Option<(u32, School)>,
    pub wander: Wander,
    pub nav: NavAgent,
    /// Index into `Content::classes` for players; `None` for everyone else.
    pub class: Option<usize>,
    /// Player HP once per second, newest first (Revert Commit looks back in it).
    pub hp_hist: [u32; HP_HIST_LEN],
    /// Perks of the equipped gear, complete sets and picked talents (players only).
    pub perks: Vec<crate::economy::ActivePerk>,
    /// Share of damage and healing while a talent splash resolves on extra targets (else 100).
    pub splash_pct: u32,
    /// Ready tick per on-use item, the consumable group (`"consumable"`) and perk ICDs.
    pub item_cd: BTreeMap<String, u64>,
    /// Tick of the pull (first threat entry); 0 while out of combat. Drives `AiDef::enrage`.
    pub combat_since: u64,
    /// Players: in combat until this tick after an enemy that had them on its table reset.
    pub combat_lock_until: u64,
    /// `AiDef::flee`: running for the exit now, and the earliest tick of the next run.
    pub fleeing: bool,
    pub flee_next: u64,
}

/// One group member's unclaimed roll on a corpse.
#[derive(Clone, Debug)]
pub(crate) struct LootShare {
    pub owner: Option<EntityId>,
    pub items: Vec<String>,
    pub sp: u32,
}

/// Seconds of HP history kept per player.
pub(crate) const HP_HIST_LEN: usize = 8;

/// One resolved damage instance, before modifiers and absorbs.
pub(crate) struct Hit {
    pub spell: Option<SpellId>,
    pub amount: u32,
    pub crit: bool,
    pub combo: bool,
    pub school: School,
    /// Periodic or expiry damage: goes around frontal shields.
    pub dot: bool,
}

/// How a unit enters the world.
pub(crate) struct SpawnOpts {
    pub level: Option<u8>,
    pub pack: Option<u32>,
    pub respawns: bool,
    pub summoner: Option<EntityId>,
    pub floor_boss: bool,
    pub zone_slot: Option<usize>,
}

pub struct Sim {
    pub(crate) content: Content,
    pub(crate) rng: Rng,
    pub(crate) now: u64,
    pub(crate) units: Vec<Unit>,
    pub(crate) blockers: Vec<Aabb>,
    pub(crate) nav: Nav,
    /// `None` acts as the first player at apply time, so `push` before `spawn_player` still works.
    pub(crate) pending: Vec<(Option<EntityId>, Command)>,
    pub(crate) events: Vec<Event>,
    /// Sparse audience of `events`: (index, recipient) for player-only events; the rest go to all.
    pub(crate) routed: Vec<(usize, EntityId)>,
    /// Join order; the first is the local player of the single-player API.
    pub(crate) players: Vec<EntityId>,
    pub(crate) zone: Option<String>,
    pub(crate) world: World,
    /// Keyed by player id so a second player gets its own log.
    pub(crate) quest_logs: BTreeMap<EntityId, QuestLog>,
    pub(crate) next_pack: u32,
    /// Earliest tick for the next throttled hostile yell, shared by all units.
    pub(crate) hostile_yell_next: u64,
    /// Root of every unit's private wander stream.
    pub(crate) seed: u64,
    pub(crate) wandering: bool,
    /// Gear, junk and SP rolls; separate from `rng` so loot tables never shift combat replays.
    pub(crate) loot_rng: Rng,
    pub(crate) telegraphs: Vec<crate::telegraph::TelegraphRt>,
    pub(crate) next_telegraph: u32,
    /// Trash packs of the loaded zone (co-op growth on join).
    pub(crate) zone_packs: Vec<crate::zones::PackRt>,
    /// Props, achievements and rides (`world.rs`).
    pub(crate) fun: crate::world::WorldRt,
    /// Sprint mode runs, keystones and records (`sprint.rs`).
    pub(crate) sprint: crate::sprint::SprintState,
    /// World clock, nights, the world boss and daily/weekly tasks (`live.rs`, `dailies.rs`).
    pub(crate) live: crate::live::LiveRt,
    /// Code review duels and request cooldowns (`duel.rs`).
    pub(crate) duels: crate::duel::DuelState,
    /// Burnout timers, prop relief cooldowns and the quip stream (`burnout.rs`).
    pub(crate) burnout: crate::burnout::BurnoutRt,
    /// Legacy Mode difficulty, the parked world and the weekly lockout (`legacy.rs`).
    pub(crate) legacy: crate::legacy::LegacyState,
    /// Raid difficulty, weekly lockout, tethers and the twins (`raid.rs`).
    pub(crate) raid: crate::raid::RaidState,
}

impl Sim {
    pub fn new(content: Content, seed: u64) -> Sim {
        Sim {
            content,
            rng: Rng::new(seed),
            now: 0,
            units: Vec::new(),
            blockers: Vec::new(),
            nav: Nav::default(),
            pending: Vec::new(),
            events: Vec::new(),
            routed: Vec::new(),
            players: Vec::new(),
            zone: None,
            world: World::default(),
            quest_logs: BTreeMap::new(),
            next_pack: 1,
            hostile_yell_next: 0,
            seed,
            wandering: true,
            loot_rng: Rng::new(seed ^ 0x5EED_0F10_07AB),
            telegraphs: Vec::new(),
            next_telegraph: 1,
            zone_packs: Vec::new(),
            fun: crate::world::WorldRt::new(seed),
            sprint: Default::default(),
            live: Default::default(),
            duels: Default::default(),
            burnout: crate::burnout::BurnoutRt::new(seed),
            legacy: Default::default(),
            raid: Default::default(),
        }
    }

    pub fn set_blockers(&mut self, blockers: Vec<Aabb>) {
        self.blockers = blockers;
        self.rebuild_nav();
    }

    /// Adds a group member of class `class` (a `Content::classes` key) at level 1. Returns 0
    /// (never a valid id) for an unknown class or once `MAX_PLAYERS` are in. Enemy HP is
    /// rescaled for the new group size.
    pub fn spawn_player(&mut self, class: &str, pos: Vec3, yaw: f32) -> EntityId {
        if self.players.len() >= MAX_PLAYERS {
            return 0;
        }
        let Some(ci) = self.content.classes.iter().position(|c| c.key == class) else {
            return 0;
        };
        let Ok(def) = self.def_index(&self.content.classes[ci].unit.clone()) else {
            return 0;
        };
        let id = self.spawn(
            def,
            pos,
            yaw,
            SpawnOpts {
                level: Some(1),
                pack: None,
                respawns: false,
                summoner: None,
                floor_boss: false,
                zone_slot: None,
            },
        );
        self.players.push(id);
        // Re-apply now that the unit is known to be the player (level table stats).
        if let Some(i) = self.index(id) {
            self.units[i].class = Some(ci);
            self.units[i].state.equipped = vec![String::new(); crate::economy::SLOTS];
            self.apply_level(i, 1);
            let hp = self.units[i].state.hp;
            self.units[i].hp_hist = [hp; HP_HIST_LEN];
        }
        self.rescale_group_hp();
        self.grow_packs();
        self.live_on_join(id);
        self.legacy_on_join(id);
        self.raid_on_join(id);
        self.ladder_on_join(id);
        self.burnout_on_join(id);
        id
    }

    /// A player leaves mid-session: the unit despawns (threat on it and targets of it are
    /// cleared, so enemies switch or evade), its unclaimed loot shares and quest log are dropped
    /// and enemy HP is rescaled. False for an id that is not a player.
    pub fn remove_player(&mut self, id: EntityId) -> bool {
        let Some(i) = self.index(id).filter(|_| self.is_player(id)) else {
            return false;
        };
        self.duel_left(id);
        self.despawn(i);
        self.players.retain(|p| *p != id);
        self.quest_logs.remove(&id);
        self.live.tasks.remove(&id);
        self.duels.next_request.remove(&id);
        self.burnout_on_leave(id);
        for u in &mut self.units {
            u.loot.retain(|l| l.owner != Some(id));
            if u.state.lootable && u.loot.is_empty() {
                u.state.lootable = false;
            }
        }
        for c in 0..self.units.len() {
            if self.units[c].corpse_until.is_some() {
                self.refresh_corpse(c);
            }
        }
        self.rescale_group_hp();
        true
    }

    /// Group members in join order.
    pub fn players(&self) -> &[EntityId] {
        &self.players
    }

    pub(crate) fn is_player(&self, id: EntityId) -> bool {
        self.players.contains(&id)
    }

    /// The class of a group member; `None` for anyone else.
    pub fn class_of(&self, id: EntityId) -> Option<&crate::content::ClassDef> {
        let i = self.index(id)?;
        self.content.classes.get(self.units[i].class?)
    }

    pub(crate) fn class_at(&self, i: usize) -> Option<&crate::content::ClassDef> {
        self.content.classes.get(self.units[i].class?)
    }

    fn first_player(&self) -> Option<EntityId> {
        self.players.first().copied()
    }

    pub(crate) fn first_player_index(&self) -> Option<usize> {
        self.first_player().and_then(|p| self.index(p))
    }

    /// Spawns at the authored level; the unit respawns after `respawn_ms` like before floors existed.
    pub fn spawn_unit(
        &mut self,
        unit_key: &str,
        pos: Vec3,
        yaw: f32,
    ) -> Result<EntityId, ContentError> {
        let def = self.def_index(unit_key)?;
        Ok(self.spawn(
            def,
            pos,
            yaw,
            SpawnOpts {
                level: None,
                pack: None,
                respawns: true,
                summoner: None,
                floor_boss: false,
                zone_slot: None,
            },
        ))
    }

    pub(crate) fn def_index(&self, key: &str) -> Result<usize, ContentError> {
        self.content
            .units
            .iter()
            .position(|u| u.key == key)
            .ok_or_else(|| ContentError::UnknownUnit(key.to_string()))
    }

    pub(crate) fn spawn(&mut self, def: usize, pos: Vec3, yaw: f32, o: SpawnOpts) -> EntityId {
        let d = &self.content.units[def];
        let id = self.units.len() as EntityId + 1;
        let n_abilities = d.ai.as_ref().map_or(0, |a| a.abilities.len());
        let ambient_ms = u64::from(d.yells.ambient_ms);
        let state = UnitState {
            id,
            key: d.key.clone(),
            name: d.name.clone(),
            level: d.level,
            hostile: d.hostile,
            pos,
            yaw,
            hp: d.max_hp,
            max_hp: d.max_hp,
            mana: d.max_mana,
            max_mana: d.max_mana,
            alive: true,
            in_combat: false,
            target: None,
            cast: None,
            auras: Vec::new(),
            ai: d.ai.as_ref().map(|_| AiState::Idle),
            lootable: false,
            respawn_in_ms: None,
            xp: if d.hostile { d.xp_reward } else { 0 },
            auto_attack: false,
            swing_remaining_ms: 0,
            xp_to_next: 0,
            stunned: false,
            silenced: false,
            boss: d.boss,
            pack: o.pack,
            phase: 1,
            idle_pose: d.idle_pose,
            inventory: Vec::new(),
            sp: 0,
            bag: Vec::new(),
            equipped: Vec::new(),
            buyback: Vec::new(),
            achievements: Vec::new(),
            title: String::new(),
            talents: [0; crate::talents::TALENT_ROWS],
            pet: String::new(),
            pets: Vec::new(),
            duel_foe: None,
            duel_streak: 0,
            respecs: 0,
            career: Default::default(),
        };
        self.units.push(Unit {
            state,
            def,
            spawn_pos: pos,
            spawn_yaw: yaw,
            gcd_end: 0,
            gcd_total_ms: 0,
            cooldowns: BTreeMap::new(),
            lockout_until: [0; 2],
            cast: None,
            queued: None,
            moving: false,
            threat: BTreeMap::new(),
            melee_next: 0,
            swing_next: 0,
            ability_next: vec![0; n_abilities],
            ability_yell_cursor: vec![0; n_abilities],
            auras: Vec::new(),
            regen_acc: 0,
            regen_next: self.now + ticks(self.content.config.regen.interval_ms),
            spell_yell_next: 0,
            spell_yell_cursor: BTreeMap::new(),
            respawn_at: None,
            died_at: 0,
            corpse_until: None,
            loot: Vec::new(),
            item_bonus: ItemTotals::default(),
            yell_pending: None,
            yell_cursor: [0; YELL_KINDS],
            yell_periodic_next: 0,
            yell_low_done: false,
            // Staggered by id so neighbouring NPCs take turns instead of talking over each other.
            ambient_next: self.now + ticks(1) + ambient_ms * 30 * (id % 4 + 1) / 4000,
            sp: d.spell_power,
            dmg_pct: 100,
            respawns: o.respawns,
            summoner: o.summoner,
            despawned: false,
            floor_boss: o.floor_boss,
            zone_slot: o.zone_slot,
            last_cast: None,
            last_player_hit: None,
            wander: Wander::new(self.seed, id),
            nav: NavAgent::default(),
            class: None,
            hp_hist: [0; HP_HIST_LEN],
            combat_since: 0,
            combat_lock_until: 0,
            fleeing: false,
            flee_next: 0,
            perks: Vec::new(),
            splash_pct: 100,
            item_cd: BTreeMap::new(),
        });
        let i = self.units.len() - 1;
        // Neighbouring NPCs of the same kind start at different lines, so they banter.
        self.units[i].yell_cursor[YellKind::Ambient as usize] = i;
        let level = o.level.unwrap_or(self.content.units[def].level);
        self.apply_level(i, level);
        id
    }

    /// Idle wandering is on by default; balance harnesses switch it off so fight timing
    /// depends on the authored positions only.
    pub fn set_wandering(&mut self, on: bool) {
        self.wandering = on;
    }

    /// Lets the presentation layer override a unit's authored pose (e.g. a desk the sim knows
    /// nothing about); seated and sleeping units do not wander.
    pub fn set_idle_pose(&mut self, id: EntityId, pose: Option<crate::content::IdlePose>) {
        if let Some(i) = self.index(id) {
            self.units[i].state.idle_pose = pose;
        }
    }

    /// Applied at the start of the next tick, as the first player (`apply_as`) for the
    /// player-relative commands; actor ids inside the command are trusted as given.
    pub fn push(&mut self, cmd: Command) {
        self.pending.push((None, cmd));
    }

    /// Queues `cmd` from group member `player` for the start of the next tick. The actor field
    /// (`caster`, `unit`, `looter`) is rebound to `player`, so a client can only act as itself.
    /// Ignored for an id that is not a player.
    pub fn apply_as(&mut self, player: EntityId, mut cmd: Command) {
        if !self.is_player(player) {
            return;
        }
        match &mut cmd {
            Command::SetTarget { unit, .. }
            | Command::StartAutoAttack { unit }
            | Command::StopAutoAttack { unit } => *unit = player,
            Command::Cast { caster, .. } | Command::StopCasting { caster } => *caster = player,
            Command::Loot { looter, .. } => *looter = player,
            Command::PlayerMoved { .. }
            | Command::RespawnPlayer
            | Command::Talk { .. }
            | Command::AcceptQuest { .. }
            | Command::TurnInQuest { .. }
            | Command::AbandonQuest { .. }
            | Command::Equip { .. }
            | Command::Unequip { .. }
            | Command::Buy { .. }
            | Command::Sell { .. }
            | Command::SellJunk { .. }
            | Command::Discard { .. }
            | Command::DiscardJunk
            | Command::Buyback { .. }
            | Command::UseItem { .. }
            | Command::Interact { .. }
            | Command::SetTitle { .. }
            | Command::PickTalent { .. }
            | Command::ResetTalents
            | Command::StartSprint { .. }
            | Command::StartWeeklySprint
            | Command::SetLegacy { .. }
            | Command::JoinStandup
            | Command::ClaimSprintGoal { .. }
            | Command::GoHome
            | Command::SummonPet { .. }
            | Command::DuelRequest { .. }
            | Command::DuelRespond { .. } => {}
        }
        self.pending.push((Some(player), cmd));
    }

    pub fn now(&self) -> Tick {
        Tick(self.now)
    }

    /// The first group member (the local player in single-player).
    pub fn player(&self) -> Option<EntityId> {
        self.first_player()
    }

    /// `None` for unknown or despawned units.
    pub fn unit(&self, id: EntityId) -> Option<&UnitState> {
        self.index(id)
            .filter(|i| !self.units[*i].despawned)
            .map(|i| &self.units[i].state)
    }

    /// Every unit that is not despawned (corpses included).
    pub fn units(&self) -> impl Iterator<Item = &UnitState> {
        self.units.iter().filter(|u| !u.despawned).map(|u| &u.state)
    }

    pub fn content(&self) -> &Content {
        &self.content
    }

    /// Legacy: the tower floor number of the loaded zone (`tower/<n>`), if any.
    pub fn floor(&self) -> Option<u8> {
        self.zone.as_deref().and_then(floor_number)
    }

    /// For a bar spell a talent replaces, the view is of the variant it casts.
    pub fn cooldown(&self, caster: EntityId, spell: SpellId) -> CooldownView {
        let spell = self.cast_spell_for(caster, spell);
        let (Some(ci), Some(def)) = (self.index(caster), self.content.spell(spell)) else {
            return CooldownView {
                gcd_remaining_ms: 0,
                gcd_total_ms: 0,
                cd_remaining_ms: 0,
                cd_total_ms: 0,
                usable: Err(CastError::InvalidTarget),
            };
        };
        let u = &self.units[ci];
        let (cd_end, cd_total) = u.cooldowns.get(&spell).copied().unwrap_or((0, 0));
        let (gcd_remaining_ms, gcd_total_ms) = if def.gcd {
            (self.remaining_ms(u.gcd_end), u.gcd_total_ms)
        } else {
            (0, 0)
        };
        let usable = self
            .validate(ci, def)
            .and_then(|_| self.busy(ci, def).map_or(Ok(()), |(e, _)| Err(e)));
        CooldownView {
            gcd_remaining_ms,
            gcd_total_ms,
            cd_remaining_ms: self.remaining_ms(cd_end),
            cd_total_ms: cd_total,
            usable,
        }
    }

    /// Advance exactly one 1/30 s tick.
    pub fn tick(&mut self) -> Vec<Event> {
        self.step();
        self.routed.clear();
        std::mem::take(&mut self.events)
    }

    /// `tick` for a server: every event with the group member(s) it is meant for.
    pub fn tick_routed(&mut self) -> Vec<(Audience, Event)> {
        self.step();
        self.drain_events_routed()
    }

    /// Takes the pending events (also those `load_zone` queued outside a tick) with their
    /// audience: quest, loot, XP gain, spell learned and own cast failures go to one player.
    pub fn drain_events_routed(&mut self) -> Vec<(Audience, Event)> {
        let mut routed = std::mem::take(&mut self.routed).into_iter().peekable();
        std::mem::take(&mut self.events)
            .into_iter()
            .enumerate()
            .map(|(k, e)| match routed.next_if(|(at, _)| *at == k) {
                Some((_, p)) => (Audience::Player(p), e),
                None => (Audience::All, e),
            })
            .collect()
    }

    /// Queues an event for one group member only (see `drain_events_routed`).
    pub(crate) fn emit_to(&mut self, player: EntityId, e: Event) {
        self.routed.push((self.events.len(), player));
        self.events.push(e);
    }

    /// Player-only when the actor is a group member, broadcast otherwise.
    pub(crate) fn emit_for(&mut self, unit: EntityId, e: Event) {
        if self.is_player(unit) {
            self.emit_to(unit, e);
        } else {
            self.events.push(e);
        }
    }

    fn step(&mut self) {
        self.now += 1;
        let cmds = std::mem::take(&mut self.pending);
        for (actor, c) in cmds {
            self.apply(actor, c);
        }
        self.update_channels();
        self.update_casts();
        self.run_queue();
        self.update_auto_attack();
        self.update_ai();
        self.update_telegraphs();
        self.update_wander();
        self.update_auras();
        self.update_regen();
        self.update_hp_history();
        self.update_combat_flags();
        self.update_world();
        self.update_sprint();
        self.update_raid();
        self.update_live();
        self.update_duels();
        self.update_burnout();
        self.update_pets();
        self.update_corpses();
        self.flush_yells();
        self.sync_views();
    }

    // ---- helpers ----

    pub(crate) fn index(&self, id: EntityId) -> Option<usize> {
        let i = usize::try_from(id.checked_sub(1)?).ok()?;
        (i < self.units.len()).then_some(i)
    }

    pub(crate) fn id_of(i: usize) -> EntityId {
        i as EntityId + 1
    }

    pub(crate) fn remaining_ms(&self, end: u64) -> u32 {
        ticks_to_ms(end.saturating_sub(self.now)) as u32
    }

    fn candidate(&self, i: usize) -> TargetCandidate {
        let s = &self.units[i].state;
        TargetCandidate {
            id: s.id,
            pos: s.pos,
            hostile: s.hostile,
            alive: s.alive,
        }
    }

    fn fail(&mut self, caster: EntityId, spell: SpellId, reason: CastError) {
        let e = Event::CastFailed {
            caster,
            spell,
            reason,
        };
        self.emit_for(caster, e);
    }

    // ---- commands ----

    fn apply(&mut self, actor: Option<EntityId>, cmd: Command) {
        let Some(actor) = actor.or_else(|| self.first_player()) else {
            // No player yet: only the commands that name their actor can run.
            return self.apply_named(cmd);
        };
        match cmd {
            Command::PlayerMoved {
                pos,
                yaw,
                moving,
                jumping,
            } => self.on_player_moved(actor, pos, yaw, moving || jumping),
            Command::RespawnPlayer => self.respawn_player(actor),
            Command::Talk { unit } => self.talk(actor, unit),
            Command::AcceptQuest { quest } => self.accept_quest(actor, &quest),
            Command::TurnInQuest { quest, choice } => self.turn_in_quest(actor, &quest, choice),
            Command::AbandonQuest { quest } => self.abandon_quest(actor, &quest),
            Command::Equip { item } => self.equip(actor, &item),
            Command::Unequip { slot } => self.unequip(actor, slot),
            Command::Buy { vendor, item } => self.buy(actor, vendor, &item),
            Command::Sell { vendor, item } => self.sell(actor, vendor, &item),
            Command::SellJunk { vendor } => self.sell_junk(actor, vendor),
            Command::Discard { slot } => self.discard(actor, slot),
            Command::DiscardJunk => self.discard_junk(actor),
            Command::Buyback { vendor, item } => self.buyback(actor, vendor, &item),
            Command::UseItem { item } => self.use_item(actor, &item),
            Command::Interact { prop } => self.interact(actor, &prop),
            Command::SetTitle { achievement } => self.set_title(actor, &achievement),
            Command::PickTalent { row, choice } => self.pick_talent(actor, row, choice),
            Command::ResetTalents => self.reset_talents(actor),
            Command::StartSprint { building, level } => self.start_sprint(actor, &building, level),
            Command::StartWeeklySprint => self.start_weekly_sprint(actor),
            Command::SetLegacy { on } => self.set_legacy(actor, on),
            Command::JoinStandup => self.join_standup(actor),
            Command::ClaimSprintGoal { choice } => self.claim_sprint_goal(actor, choice),
            Command::GoHome => self.go_home(actor),
            Command::SummonPet { pet } => self.summon_pet(actor, &pet),
            Command::DuelRequest { target } => self.duel_request(actor, target),
            Command::DuelRespond { accept } => self.duel_respond(actor, accept),
            other => self.apply_named(other),
        }
    }

    fn apply_named(&mut self, cmd: Command) {
        match cmd {
            Command::SetTarget { unit, target } => {
                let valid = target.is_none_or(|t| self.unit(t).is_some());
                if let (Some(i), true) = (self.index(unit), valid) {
                    if self.units[i].state.target != target {
                        self.set_auto_attack(i, false);
                    }
                    self.units[i].state.target = target;
                }
            }
            Command::Cast { caster, spell } => self.handle_cast(caster, spell),
            Command::StopCasting { caster } => {
                if let Some(i) = self.index(caster) {
                    self.units[i].queued = None;
                    self.cancel_cast(i, None, 0);
                }
            }
            Command::StartAutoAttack { unit } => self.start_auto_attack(unit),
            Command::StopAutoAttack { unit } => {
                if let Some(i) = self.index(unit) {
                    self.set_auto_attack(i, false);
                }
            }
            Command::Loot { looter, corpse } => self.handle_loot(looter, corpse),
            Command::PlayerMoved { .. }
            | Command::RespawnPlayer
            | Command::Talk { .. }
            | Command::AcceptQuest { .. }
            | Command::TurnInQuest { .. }
            | Command::AbandonQuest { .. }
            | Command::Equip { .. }
            | Command::Unequip { .. }
            | Command::Buy { .. }
            | Command::Sell { .. }
            | Command::SellJunk { .. }
            | Command::Discard { .. }
            | Command::DiscardJunk
            | Command::Buyback { .. }
            | Command::UseItem { .. }
            | Command::Interact { .. }
            | Command::SetTitle { .. }
            | Command::PickTalent { .. }
            | Command::ResetTalents
            | Command::StartSprint { .. }
            | Command::StartWeeklySprint
            | Command::SetLegacy { .. }
            | Command::JoinStandup
            | Command::ClaimSprintGoal { .. }
            | Command::GoHome
            | Command::SummonPet { .. }
            | Command::DuelRequest { .. }
            | Command::DuelRespond { .. } => {}
        }
    }

    fn on_player_moved(&mut self, player: EntityId, pos: Vec3, yaw: f32, moving: bool) {
        let Some(i) = self.index(player) else {
            return;
        };
        if !self.units[i].state.alive {
            return;
        }
        let u = &mut self.units[i];
        u.state.pos = pos;
        u.state.yaw = yaw;
        u.moving = moving;
        let keeps = u
            .cast
            .as_ref()
            .and_then(|c| self.content.spell(c.spell))
            .is_some_and(|d| self.perk_moving_ok(i, d));
        if moving && !keeps {
            self.cancel_cast(i, None, 0);
        }
    }

    fn handle_loot(&mut self, looter: EntityId, corpse: EntityId) {
        let (Some(li), Some(ci)) = (self.index(looter), self.index(corpse)) else {
            return;
        };
        let near = in_range(
            self.units[li].state.pos,
            self.units[ci].state.pos,
            self.content.config.loot_range_m,
        );
        if !self.units[li].state.alive || !self.units[ci].state.lootable || !near {
            return;
        }
        self.claim_loot(li, ci);
    }

    /// Hands `looter` (unit `li`) their share of corpse `ci`, range and life already checked.
    pub(crate) fn claim_loot(&mut self, li: usize, ci: usize) {
        let looter = Self::id_of(li);
        let corpse = Self::id_of(ci);
        // Own roll first; an ownerless roll (killed before anyone joined) is anyone's.
        let shares = &mut self.units[ci].loot;
        let Some(k) = shares
            .iter()
            .position(|l| l.owner == Some(looter))
            .or_else(|| shares.iter().position(|l| l.owner.is_none()))
        else {
            return;
        };
        let share = shares.remove(k);
        let player = self.is_player(looter);
        let (mut items, mut sp) = (share.items, share.sp);
        if player {
            let (trophies, goods): (Vec<String>, Vec<String>) = items
                .into_iter()
                .partition(|n| self.content.item(n).is_some());
            self.add_trophies(li, &trophies);
            // Whatever does not fit stays on the corpse for a second try.
            let left = self.stow(li, goods.clone());
            items = trophies
                .into_iter()
                .chain(goods.into_iter().filter(|g| !left.contains(g)))
                .collect();
            self.units[li].state.sp = self.units[li].state.sp.saturating_add(sp);
            if !left.is_empty() {
                self.units[ci].loot.push(LootShare {
                    owner: Some(looter),
                    items: left,
                    sp: 0,
                });
                self.item_error(looter, ItemError::BagFull);
            }
            self.bag_event(li);
        } else {
            sp = 0;
        }
        self.units[ci].state.lootable = !self.units[ci].loot.is_empty();
        self.refresh_corpse(ci);
        let e = Event::Looted {
            looter,
            corpse,
            items: items.clone(),
            sp,
        };
        self.emit_for(looter, e);
        if player {
            for it in &items {
                self.quest_credit(looter, |o| matches!(o, Objective::Loot(n, _) if n == it));
            }
        }
    }

    /// Stores the items and applies their bonuses at once. Current HP and mana scale up
    /// with the new maximum and never drop.
    pub(crate) fn add_trophies(&mut self, i: usize, items: &[String]) {
        for name in items {
            let inv = &mut self.units[i].state.inventory;
            match inv.iter_mut().find(|(n, _)| n == name) {
                Some((_, c)) => *c += 1,
                None => inv.push((name.clone(), 1)),
            }
        }
        let (old_hp, old_mana) = (self.units[i].state.max_hp, self.units[i].state.max_mana);
        self.refresh_item_bonus(i);
        let u = &mut self.units[i].state;
        let scale = |cur: u32, old: u32, new: u32| {
            if old == 0 {
                return cur;
            }
            cur.max((u64::from(cur) * u64::from(new) / u64::from(old)) as u32)
                .min(new)
        };
        u.hp = scale(u.hp, old_hp, u.max_hp);
        u.mana = scale(u.mana, old_mana, u.max_mana);
    }

    /// Re-sums trophies, gear and set bonuses, then rebuilds the level-derived stats (which include the bonus).
    pub(crate) fn refresh_item_bonus(&mut self, i: usize) {
        let mut total = ItemTotals::default();
        for (name, copies) in &self.units[i].state.inventory {
            if let Some(it) = self.content.item(name) {
                total.add(&it.bonus.totals(*copies));
            }
        }
        let (gear, mut perks) = self.gear_totals(i);
        perks.extend(self.talent_perks(i));
        total.add(&gear.totals);
        total.haste_pct += gear.haste_tenths / 10;
        total.armor_tenths = total.armor_tenths.min(crate::economy::ARMOR_CAP_TENTHS);
        self.units[i].item_bonus = total;
        self.units[i].perks = perks;
        let (hp, mana) = (self.units[i].state.hp, self.units[i].state.mana);
        let level = self.units[i].state.level;
        self.apply_level(i, level);
        self.units[i].state.hp = hp;
        self.units[i].state.mana = mana;
    }

    /// Debug/test: puts one copy of a trophy into the player's inventory as if looted.
    /// False for an unknown item or without a player; no event is emitted.
    pub fn grant_trophy(&mut self, name: &str) -> bool {
        let (Some(i), true) = (self.first_player_index(), self.content.item(name).is_some()) else {
            return false;
        };
        self.add_trophies(i, &[name.to_string()]);
        true
    }

    /// Debug/test: kills a living hostile unit as if the player landed the killing blow (XP,
    /// loot, zone clear, quest credit and unlocks all follow; events come out of the next tick).
    /// False for unknown, dead or friendly units.
    pub fn debug_kill(&mut self, id: EntityId) -> bool {
        let Some(ti) = self
            .index(id)
            .filter(|&i| self.units[i].state.alive && self.units[i].state.hostile)
        else {
            return false;
        };
        self.die(ti, self.first_player());
        true
    }

    /// Debug/test: a living group member dies with no killer. False for anyone else.
    pub fn debug_kill_player(&mut self, id: EntityId) -> bool {
        let Some(ti) = self
            .index(id)
            .filter(|&i| self.is_player(id) && self.units[i].state.alive)
        else {
            return false;
        };
        self.die(ti, None);
        true
    }

    /// Debug/test: applies aura `key` to a living unit as if the unit cast it on itself (the
    /// `AuraApplied` event and the unit state follow on the next tick). False for unknown or
    /// dead units and unknown auras.
    pub fn debug_apply_aura(&mut self, id: EntityId, key: &str, duration_ms: u32) -> bool {
        let Some(ti) = self.index(id).filter(|&i| self.units[i].state.alive) else {
            return false;
        };
        if self.content.aura(key).is_none() {
            return false;
        }
        self.apply_aura(ti, key, duration_ms, None, id, None);
        true
    }

    /// Debug/test: sets a living unit's HP (clamped to 1..=max) without an event; boss phases
    /// follow on its next AI step. False for unknown or dead units.
    pub fn debug_set_hp(&mut self, id: EntityId, hp: u32) -> bool {
        let Some(i) = self.index(id).filter(|&i| self.units[i].state.alive) else {
            return false;
        };
        let s = &mut self.units[i].state;
        s.hp = hp.clamp(1, s.max_hp);
        true
    }

    /// The player's trophies as (name, copies), first pickup first.
    pub fn inventory(&self) -> &[(String, u32)] {
        self.first_player_index()
            .map_or(&[][..], |i| &self.units[i].state.inventory)
    }

    /// Player bonus granted by the trophies right now (stack cap applied).
    pub fn item_totals(&self) -> ItemTotals {
        self.first_player_index()
            .map_or_else(ItemTotals::default, |i| self.units[i].item_bonus)
    }

    /// Friendly units with `talk` lines answer the player when close enough; anything else is ignored.
    fn talk(&mut self, player: EntityId, unit: EntityId) {
        let (Some(pi), Some(ti)) = (self.index(player), self.index(unit)) else {
            return;
        };
        let (p, t) = (&self.units[pi], &self.units[ti]);
        let near = in_range(p.state.pos, t.state.pos, self.content.config.loot_range_m);
        if p.state.alive && t.state.alive && !t.despawned && !t.state.hostile && near {
            let key = t.state.key.clone();
            self.request_yell(ti, YellKind::Talk);
            self.on_talk(player, &key);
            self.quest_credit(player, |o| matches!(o, Objective::Talk(k) if *k == key));
            self.offer_quests(player, ti);
        }
    }

    fn respawn_player(&mut self, player: EntityId) {
        let Some(i) = self.index(player) else {
            return;
        };
        if self.units[i].state.alive {
            return;
        }
        self.set_auto_attack(i, false);
        let u = &mut self.units[i];
        u.state.alive = true;
        u.state.hp = u.state.max_hp;
        u.state.mana = u.state.max_mana;
        u.state.pos = u.spawn_pos;
        u.state.yaw = u.spawn_yaw;
        u.moving = false;
        u.regen_next = self.now + ticks(self.content.config.regen.interval_ms);
        u.gcd_end = 0;
        u.cooldowns.clear();
        u.lockout_until = [0; 2];
        self.events.push(Event::Respawned {
            unit: Self::id_of(i),
        });
    }

    // ---- casting ----

    /// Static legality: everything except GCD/cast-busy (which may be queued).
    pub(crate) fn validate(
        &self,
        ci: usize,
        spell: &SpellDef,
    ) -> Result<Option<EntityId>, CastError> {
        let u = &self.units[ci];
        if !u.state.alive {
            return Err(CastError::Dead);
        }
        if !self.is_learned(ci, spell) {
            return Err(CastError::NotLearned);
        }
        if self.has_aura_flag(ci, |a| a.stun) {
            return Err(CastError::Stunned);
        }
        if self.has_aura_flag(ci, |a| a.silence) {
            return Err(CastError::Silenced);
        }
        if self.now < u.lockout_until[spell.school.index()] {
            return Err(CastError::SchoolLocked);
        }
        if u.cooldowns
            .get(&spell.id)
            .is_some_and(|(end, _)| *end > self.now)
        {
            return Err(CastError::OnCooldown);
        }
        self.mount_check(ci, spell)?;
        let target = if spell.target == SpellTarget::Ally {
            Some(self.ally_target(ci, spell)?)
        } else if spell.target == SpellTarget::DeadAlly {
            if u.state.in_combat {
                return Err(CastError::InCombat);
            }
            let tid = u.state.target.ok_or(CastError::NoTarget)?;
            self.check_friend(ci, tid, spell)?;
            Some(tid)
        } else if spell.targets_enemy() {
            let tid = u.state.target.ok_or(CastError::NoTarget)?;
            let ti = self.index(tid).ok_or(CastError::InvalidTarget)?;
            let t = &self.units[ti];
            if !self.hostile_to(ci, ti) || t.state.ai == Some(AiState::Evade) || t.despawned {
                return Err(CastError::InvalidTarget);
            }
            validate_target(
                u.state.pos,
                u.state.yaw,
                Some(&self.candidate(ti)),
                spell.range_m,
                true,
                &self.blockers,
            )?;
            if t.state.boss && spell.stuns(&self.content) {
                return Err(CastError::InvalidTarget);
            }
            if matches!(spell.effect, crate::content::Effect::Interrupt { .. }) {
                match &t.cast {
                    None => return Err(CastError::NothingToInterrupt),
                    Some(c) if !c.interruptible => return Err(CastError::NotInterruptible),
                    Some(_) => {}
                }
            }
            Some(tid)
        } else {
            Some(u.state.id)
        };
        if spell.cast_ms > 0
            && u.moving
            && self.cast_time(ci, spell) > 0
            && !self.perk_moving_ok(ci, spell)
        {
            return Err(CastError::Moving);
        }
        if u.state.mana < self.perk_mana_cost(ci, spell.id, spell.mana) {
            return Err(CastError::NotEnoughMana);
        }
        if spell.channel.is_some() && u.state.mana >= u.state.max_mana {
            return Err(CastError::FullMana);
        }
        Ok(target)
    }

    /// `Ally` spells: the targeted friendly player when there is one, else the caster
    /// (an enemy, an NPC or no target self-casts, so single-player is unchanged).
    fn ally_target(&self, ci: usize, spell: &SpellDef) -> Result<EntityId, CastError> {
        let me = Self::id_of(ci);
        match self.units[ci].state.target {
            Some(t) if t != me && (self.is_player(t) || self.healable_npc(t)) => {
                self.check_friend(ci, t, spell)?;
                Ok(t)
            }
            _ => Ok(me),
        }
    }

    /// A friendly NPC authored as `healable` (the onboarding colleague): `Ally` spells land on it.
    fn healable_npc(&self, id: EntityId) -> bool {
        self.index(id).is_some_and(|i| {
            let u = &self.units[i];
            !u.state.hostile && self.content.units[u.def].healable
        })
    }

    /// A friendly-player target of `spell` (not the caster): still in the group, alive for
    /// `Ally` and dead for `DeadAlly`, in range and line of sight. No facing, as for heals in WoW.
    fn check_friend(&self, ci: usize, tid: EntityId, spell: &SpellDef) -> Result<(), CastError> {
        let ti = self
            .index(tid)
            .filter(|&ti| {
                let npc_ok = spell.target == SpellTarget::Ally && self.healable_npc(tid);
                ti != ci && (self.is_player(tid) || npc_ok) && !self.units[ti].despawned
            })
            .filter(|&ti| !self.duel_blocks_help(ci, ti))
            .ok_or(CastError::InvalidTarget)?;
        let (u, t) = (&self.units[ci].state, &self.units[ti].state);
        match (spell.target == SpellTarget::DeadAlly, t.alive) {
            (true, true) => return Err(CastError::TargetNotDead),
            (false, false) => return Err(CastError::Dead),
            _ => {}
        }
        if !in_range(u.pos, t.pos, spell.range_m) {
            return Err(CastError::OutOfRange);
        }
        if !line_of_sight(u.pos, t.pos, &self.blockers) {
            return Err(CastError::NoLineOfSight);
        }
        Ok(())
    }

    /// GCD or cast in progress: the error plus remaining ticks (for queueing).
    pub(crate) fn busy(&self, ci: usize, spell: &SpellDef) -> Option<(CastError, u64)> {
        // Off-GCD instants (Reject PR) are usable mid-cast and mid-GCD.
        if !spell.gcd && spell.cast_ms == 0 {
            return None;
        }
        let u = &self.units[ci];
        let gcd_rem = u.gcd_end.saturating_sub(self.now);
        // Another spell may interrupt a channel (it cancels it in `start_cast`); only the GCD gates it.
        let cast = u
            .cast
            .as_ref()
            .filter(|c| c.channel_next.is_none() || c.spell == spell.id);
        match cast {
            Some(c) => Some((
                CastError::AlreadyCasting,
                c.end.saturating_sub(self.now).max(gcd_rem),
            )),
            None if gcd_rem > 0 => Some((CastError::OnGcd, gcd_rem)),
            None => None,
        }
    }

    fn handle_cast(&mut self, caster: EntityId, spell_id: SpellId) {
        let Some(ci) = self.index(caster) else { return };
        let spell_id = self.effective_spell(ci, spell_id);
        let Some(spell) = self.content.spell(spell_id).cloned() else {
            return;
        };
        if self.toggle_mount(ci, &spell) {
            return;
        }
        let target = match self.validate(ci, &spell) {
            Ok(t) => t,
            Err(reason) => return self.fail(caster, spell_id, reason),
        };
        match self.busy(ci, &spell) {
            None => self.start_cast(ci, &spell, target),
            Some((_, remaining)) if remaining <= ticks(self.content.config.queue_window_ms) => {
                // A newer press replaces an older queued one, as in WoW.
                self.units[ci].queued = Some(spell_id);
                let e = Event::CastQueued {
                    caster,
                    spell: spell_id,
                };
                self.emit_for(caster, e);
            }
            Some((reason, _)) => self.fail(caster, spell_id, reason),
        }
    }

    /// Starts a queued cast on the first tick nothing is busy; re-validates then.
    fn run_queue(&mut self) {
        for ci in 0..self.units.len() {
            let Some(spell_id) = self.units[ci].queued else {
                continue;
            };
            let Some(spell) = self.content.spell(spell_id).cloned() else {
                self.units[ci].queued = None;
                continue;
            };
            if self.busy(ci, &spell).is_some() {
                continue;
            }
            self.units[ci].queued = None;
            match self.validate(ci, &spell) {
                Ok(target) => self.start_cast(ci, &spell, target),
                Err(reason) => self.fail(Self::id_of(ci), spell_id, reason),
            }
        }
    }

    pub(crate) fn start_cast(&mut self, ci: usize, spell: &SpellDef, target: Option<EntityId>) {
        self.dismount_for_cast(ci, spell);
        let haste = self.haste(ci);
        let id = Self::id_of(ci);
        if self.units[ci]
            .cast
            .as_ref()
            .is_some_and(|c| c.channel_next.is_some())
        {
            self.cancel_cast(ci, None, 0);
        }
        if spell.gcd {
            let gcd = self.content.gcd_ms(haste);
            let u = &mut self.units[ci];
            u.gcd_end = self.now + ticks(gcd);
            u.gcd_total_ms = gcd;
        }
        // Channel length is authored; haste and cast-time auras must not stretch the drink.
        let cast_ms = match spell.channel {
            Some(_) => spell.cast_ms,
            None => self.cast_time(ci, spell),
        };
        if cast_ms == 0 {
            return self.resolve(ci, spell, target);
        }
        let end = self.now + ticks(cast_ms);
        self.units[ci].cast = Some(ActiveCast {
            spell: spell.id,
            target,
            start: self.now,
            end,
            pushbacks: 0,
            interruptible: spell.interruptible,
            channel_next: spell.channel.map(|ch| self.now + ticks(ch.interval_ms)),
        });
        self.events.push(Event::CastStarted {
            caster: id,
            spell: spell.id,
            total_ms: ticks_to_ms(end - self.now) as u32,
        });
        // A channel has no release moment, so the player shouts as it begins.
        if spell.channel.is_some() {
            self.spell_yell(ci, spell);
        }
    }

    /// Mana ticks of active channels. Runs before `update_casts` so the tick that
    /// lands on the final tick is not lost; a full bar ends the channel early.
    fn update_channels(&mut self) {
        for ci in 0..self.units.len() {
            let Some((spell_id, next)) = self.units[ci]
                .cast
                .as_ref()
                .and_then(|c| c.channel_next.map(|n| (c.spell, n)))
            else {
                continue;
            };
            let Some(ch) = self.content.spell(spell_id).and_then(|s| s.channel) else {
                continue;
            };
            if next > self.now {
                continue;
            }
            let u = &mut self.units[ci];
            let want = (u.state.max_mana * ch.mana_pct / 100).max(1);
            let gain = want.min(u.state.max_mana.saturating_sub(u.state.mana));
            u.state.mana += gain;
            let full = u.state.mana >= u.state.max_mana;
            if let Some(c) = u.cast.as_mut() {
                c.channel_next = Some(next + ticks(ch.interval_ms));
            }
            if gain > 0 {
                self.events.push(Event::ManaGain {
                    unit: Self::id_of(ci),
                    spell: spell_id,
                    amount: gain,
                });
            }
            if full {
                self.complete_cast(ci);
            }
        }
    }

    fn update_casts(&mut self) {
        for ci in 0..self.units.len() {
            let due = self.units[ci]
                .cast
                .as_ref()
                .is_some_and(|c| c.end <= self.now);
            if due {
                self.complete_cast(ci);
            }
        }
    }

    fn complete_cast(&mut self, ci: usize) {
        let Some(cast) = self.units[ci].cast.take() else {
            return;
        };
        let Some(spell) = self.content.spell(cast.spell).cloned() else {
            return;
        };
        if let Err(reason) = self.revalidate(ci, &spell, cast.target) {
            return self.fail(Self::id_of(ci), spell.id, reason);
        }
        self.resolve(ci, &spell, cast.target);
    }

    /// Completion-time checks: target alive, range and LoS (no facing, as in
    /// WotLK), and mana, which is only spent now. Hostility is not rechecked
    /// because enemies also cast "hostile" spells at the player.
    fn revalidate(
        &self,
        ci: usize,
        spell: &SpellDef,
        target: Option<EntityId>,
    ) -> Result<(), CastError> {
        let friendly = matches!(spell.target, SpellTarget::Ally | SpellTarget::DeadAlly);
        if spell.target == SpellTarget::DeadAlly && self.units[ci].state.in_combat {
            return Err(CastError::InCombat);
        }
        if let Some(t) = target.filter(|t| friendly && *t != Self::id_of(ci)) {
            self.check_friend(ci, t, spell)?;
        }
        if spell.targets_enemy() {
            let ti = target
                .and_then(|t| self.index(t))
                .ok_or(CastError::InvalidTarget)?;
            // Evading units are untargetable; checked here so no mana is spent on a dropped hit.
            if self.units[ti].state.ai == Some(AiState::Evade) || self.units[ti].despawned {
                return Err(CastError::InvalidTarget);
            }
            let u = &self.units[ci].state;
            validate_target(
                u.pos,
                u.yaw,
                Some(&self.candidate(ti)),
                spell.range_m,
                false,
                &self.blockers,
            )?;
        }
        if self.units[ci].state.mana < self.perk_mana_cost(ci, spell.id, spell.mana) {
            return Err(CastError::NotEnoughMana);
        }
        Ok(())
    }

    /// Spend mana, start the spell cooldown, report success, apply the effects.
    pub(crate) fn resolve(&mut self, ci: usize, spell: &SpellDef, target: Option<EntityId>) {
        let caster = Self::id_of(ci);
        let mana = self.perk_mana_cost(ci, spell.id, spell.mana);
        let hp_cost = self.perk_hp_cost(ci, spell.mana);
        let cooldown_ms = self.perk_cooldown(ci, spell.id, spell.cooldown_ms);
        let u = &mut self.units[ci];
        u.state.mana = u.state.mana.saturating_sub(mana);
        u.state.hp = u.state.hp.saturating_sub(hp_cost).max(1);
        if cooldown_ms > 0 {
            u.cooldowns
                .insert(spell.id, (self.now + ticks(cooldown_ms), cooldown_ms));
        }
        self.events.push(Event::CastSucceeded {
            caster,
            spell: spell.id,
            target,
        });
        if spell.channel.is_none() {
            self.spell_yell(ci, spell);
        }
        self.note_cast(ci, spell.id);
        // Players have no telegraphs; enemy telegraphs land later in `update_telegraphs`.
        if spell.telegraph.is_some() && !self.is_player(caster) {
            let ti = target.and_then(|t| self.index(t));
            return self.start_telegraphs(ci, spell, ti);
        }
        let targets: Vec<usize> = match spell.area {
            Some(area) => self.area_targets(ci, area),
            None => target.and_then(|t| self.index(t)).into_iter().collect(),
        };
        for e in spell.effects() {
            if e.caster_centric() {
                self.apply_effect(ci, spell, e, None);
            } else {
                for &ti in &targets {
                    self.apply_effect(ci, spell, e, Some(ti));
                }
            }
        }
        if let (None, [primary]) = (spell.area, targets.as_slice())
            && let Some((extra, pct)) = self.perk_splash(ci, spell, *primary)
        {
            self.units[ci].splash_pct = pct;
            for e in spell.effects().filter(|e| !e.caster_centric()) {
                for &ti in &extra {
                    self.apply_effect(ci, spell, e, Some(ti));
                }
            }
            self.units[ci].splash_pct = 100;
        }
        self.perk_after_cast(ci, spell.id, target);
    }

    /// Player-only shout on spell release; gated by a global cooldown, never touches the rng.
    fn spell_yell(&mut self, ci: usize, spell: &SpellDef) {
        if !self.is_player(Self::id_of(ci)) || spell.yells.is_empty() {
            return;
        }
        let u = &mut self.units[ci];
        if self.now < u.spell_yell_next {
            return;
        }
        let cur = u.spell_yell_cursor.entry(spell.id).or_insert(0);
        let text = spell.yells[*cur % spell.yells.len()].clone();
        *cur += 1;
        u.spell_yell_next = self.now + ticks(self.content.config.player_yell_cooldown_ms);
        self.events.push(Event::Yell {
            unit: Self::id_of(ci),
            text,
        });
    }

    /// Cancels the active cast; `by: Some` with a lockout also locks the cast's school.
    pub(crate) fn cancel_cast(&mut self, ci: usize, by: Option<EntityId>, lockout_ms: u32) {
        let Some(cast) = self.units[ci].cast.take() else {
            return;
        };
        if let Some(spell) = self.content.spell(cast.spell) {
            if lockout_ms > 0 {
                self.units[ci].lockout_until[spell.school.index()] = self.now + ticks(lockout_ms);
            }
            // A channel's cooldown starts when it ends, however it ends.
            if spell.channel.is_some() && spell.cooldown_ms > 0 {
                self.units[ci].cooldowns.insert(
                    spell.id,
                    (self.now + ticks(spell.cooldown_ms), spell.cooldown_ms),
                );
            }
        }
        self.events.push(Event::CastInterrupted {
            caster: Self::id_of(ci),
            spell: cast.spell,
            by,
            lockout_ms,
        });
    }

    // ---- damage, death ----

    pub(crate) fn deal_damage(&mut self, source: EntityId, target: EntityId, hit: Hit) {
        let Hit {
            spell,
            amount,
            crit,
            combo,
            school,
            dot,
        } = hit;
        let Some(ti) = self.index(target) else { return };
        let t = &self.units[ti];
        // Evading units ignore damage so they cannot be kited back into combat.
        if !t.state.alive || t.state.ai == Some(AiState::Evade) {
            return;
        }
        // Players hurt each other only inside a duel (a DoT outliving it does nothing).
        let duel = self.index(source).is_some_and(|si| self.duel_foes(si, ti));
        if !duel && source != target && self.is_player(source) && self.is_player(target) {
            return;
        }
        // Reflected once: the caster cannot bounce it back even if it reflects too.
        if spell.is_some()
            && source != target
            && self.has_aura_flag(ti, |a| a.reflect)
            && self
                .index(source)
                .is_some_and(|si| !self.has_aura_flag(si, |a| a.reflect))
        {
            let hit = Hit {
                spell,
                amount,
                crit,
                combo,
                school,
                dot,
            };
            return self.deal_damage(target, source, hit);
        }
        let amount = self.modified_damage(source, ti, amount);
        let amount = self.sprint_damage(source, ti, amount, spell, school);
        let amount = if duel {
            crate::duel::pvp_amount(amount)
        } else {
            amount
        };
        let amount = if dot {
            amount
        } else {
            self.frontal_shield(source, ti, amount)
        };
        let mut absorbed = 0;
        let mut depleted: Vec<String> = Vec::new();
        for a in &mut self.units[ti].auras {
            if let Some(left) = a.absorb.as_mut() {
                let take = (*left).min(amount - absorbed);
                *left -= take;
                absorbed += take;
                if *left == 0 {
                    depleted.push(a.key.clone());
                }
                if absorbed == amount {
                    break;
                }
            }
        }
        let mut dealt = amount - absorbed;
        // A duel ends at 1 HP: nobody dies over a code review.
        let duel_won = duel && dealt >= self.units[ti].state.hp;
        if duel_won {
            dealt = self.units[ti].state.hp.saturating_sub(1);
        }
        if duel {
            self.duel_hit(ti);
        }
        self.events.push(Event::Damage {
            source,
            target,
            spell,
            amount: dealt,
            absorbed,
            crit,
            combo,
            school,
        });
        for key in depleted {
            self.remove_aura(ti, &key, AuraEnd::Depleted);
        }
        if amount > 0 {
            self.dismount(ti);
        }
        let hp = self.units[ti].state.hp.saturating_sub(dealt);
        self.units[ti].state.hp = hp;
        self.raid_link_hp(ti);
        if self.content.units[self.units[ti].def].ai.is_some() {
            // Threat counts the full hit, absorbed or not.
            self.add_threat(ti, source, amount.max(1));
        } else if dealt > 0 {
            self.pushback(ti);
            if hp > 0 {
                self.perk_on_hurt(ti);
            }
        }
        if duel_won {
            self.duel_won(source, target);
        } else if hp == 0 {
            self.die(ti, Some(source));
        }
    }

    /// Pushback is player-only: enemy telegraphs must keep a fixed window.
    fn pushback(&mut self, ti: usize) {
        // Taking damage breaks a channel outright instead of delaying it.
        if self.units[ti]
            .cast
            .as_ref()
            .is_some_and(|c| c.channel_next.is_some())
        {
            return self.cancel_cast(ti, None, 0);
        }
        let (ms, max) = (
            self.content.config.pushback_ms,
            self.content.config.pushback_max,
        );
        if let Some(c) = self.units[ti].cast.as_mut()
            && c.pushbacks < max
        {
            c.pushbacks += 1;
            c.end += ticks(ms);
        }
    }

    pub(crate) fn die(&mut self, ti: usize, killer: Option<EntityId>) {
        let id = Self::id_of(ti);
        self.duel_cancel_for(id);
        self.spread_auras(ti);
        let keys: Vec<String> = self.units[ti].auras.iter().map(|a| a.key.clone()).collect();
        for k in keys {
            self.remove_aura(ti, &k, AuraEnd::Death);
        }
        self.cancel_cast(ti, None, 0);
        self.set_auto_attack(ti, false);
        for o in 0..self.units.len() {
            if self.units[o].state.target == Some(id) {
                self.set_auto_attack(o, false);
            }
        }
        let u = &mut self.units[ti];
        u.state.alive = false;
        u.state.hp = 0;
        u.queued = None;
        u.moving = false;
        u.threat.clear();
        u.state.target = None;
        u.combat_since = 0;
        u.fleeing = false;
        self.events.push(Event::Death { unit: id, killer });
        self.kill_adds_of(id);
        self.request_yell(ti, YellKind::Death);
        if self.is_player(id) {
            self.burnout_on_death(ti);
        }
        if self.is_player(id)
            && let Some(k) = killer.and_then(|k| self.index(k))
        {
            self.request_yell(k, YellKind::PlayerKilled);
        }
        for other in &mut self.units {
            other.threat.remove(&id);
        }
        let def = &self.content.units[self.units[ti].def];
        if let Some(spell) = def.death_spell.and_then(|s| self.content.spell(s)).cloned() {
            self.resolve(ti, &spell, None);
        }
        let def = &self.content.units[self.units[ti].def];
        let Some(ai) = def.ai.as_ref() else { return };
        let respawn_ms = ai.respawn_ms;
        let loot_table = def.loot.clone();
        self.units[ti].state.ai = Some(AiState::Dead);
        self.events.push(Event::AiState {
            unit: id,
            state: AiState::Dead,
        });
        // Every group member rolls on their own, in join order, so nobody fights over loot.
        let owners: Vec<Option<EntityId>> = match self.players.is_empty() {
            true => vec![None],
            false => self.players.iter().copied().map(Some).collect(),
        };
        let mut shares = Vec::with_capacity(owners.len());
        let sprint = self.sprint_active();
        let legacy = self.legacy_zone().is_some();
        for owner in owners {
            let mut items: Vec<String> = Vec::new();
            for e in &loot_table {
                // Trophies roll on the loot stream like the rest of the loot, so loot table
                // changes (or taking a quest) never shift combat.
                let drop = match (&e.quest, owner) {
                    // Legacy floors drop the Legacy tier only; trophies stay in the campaign.
                    _ if legacy => false,
                    (None, _) => self.loot_rng.chance_pct(e.chance_pct),
                    // A sprint is quest-free: its floors' quest drops stay in the campaign.
                    (Some(_), _) if sprint => false,
                    (Some(q), Some(p)) => {
                        self.wants_quest_drop(p, q, &e.item)
                            && self.loot_rng.chance_pct(e.chance_pct)
                    }
                    (Some(_), None) => false,
                };
                if drop {
                    items.push(e.item.clone());
                }
            }
            let role = owner
                .and_then(|p| self.index(p))
                .and_then(|p| self.class_at(p))
                .map(|c| c.role);
            let (goods, sp) = self.roll_economy_loot(ti, role);
            let bonus = owner
                .and_then(|p| self.index(p))
                .map_or(0, |p| self.perk_kudos_pct(p));
            let sp = sp * (100 + bonus) / 100;
            items.extend(goods);
            items.extend(self.pet_drops(ti, owner));
            shares.push(LootShare { owner, items, sp });
        }
        self.units[ti].loot = shares.clone();
        self.units[ti].state.lootable = true;
        self.units[ti].respawn_at = self.units[ti]
            .respawns
            .then(|| self.now + ticks(respawn_ms));
        self.units[ti].died_at = self.now;
        self.refresh_corpse(ti);
        if let Some(k) = killer {
            self.award_kill_xp(k, ti);
        }
        self.endgame_on_kill(ti, killer);
        if let Some(ki) = killer
            .filter(|k| self.is_player(*k))
            .and_then(|k| self.index(k))
        {
            self.perk_on_kill(ki);
        }
        for LootShare { owner, items, sp } in shares {
            let e = Event::LootDropped {
                corpse: id,
                items,
                sp,
            };
            match owner {
                Some(p) => self.emit_to(p, e),
                None => self.events.push(e),
            }
        }
        self.on_hostile_death(ti, killer);
    }

    // ---- per-tick upkeep ----

    /// In combat only the legacy mana trickle runs; out of combat the discrete
    /// percent tick below replaces it (no continuous ooc regen, no 5-second rule).
    fn update_regen(&mut self) {
        let cfg = self.content.config.regen;
        let step = ticks(cfg.interval_ms);
        for u in &mut self.units {
            let d = &self.content.units[u.def];
            if !u.state.alive {
                u.regen_acc = 0;
                continue;
            }
            if !u.state.in_combat {
                u.regen_acc = 0;
                if step > 0 && self.now >= u.regen_next {
                    u.regen_next = self.now + step;
                    let gain = |max: u32, pct: u32| (u64::from(max) * u64::from(pct) / 100) as u32;
                    u.state.hp =
                        (u.state.hp + gain(u.state.max_hp, cfg.hp_pct)).min(u.state.max_hp);
                    u.state.mana =
                        (u.state.mana + gain(u.state.max_mana, cfg.mana_pct)).min(u.state.max_mana);
                }
                continue;
            }
            if u.state.mana >= u.state.max_mana {
                u.regen_acc = 0;
                continue;
            }
            let brew: u32 = u
                .auras
                .iter()
                .filter_map(|a| self.content.aura(&a.key))
                .map(|a| a.mana_regen)
                .sum();
            u.regen_acc += d.mana_regen_per_5s_combat + u.item_bonus.mana_regen + brew;
            u.state.mana = (u.state.mana + u.regen_acc / 150).min(u.state.max_mana);
            u.regen_acc %= 150;
        }
    }

    /// Once per second; no rng, so replays are unaffected.
    fn update_hp_history(&mut self) {
        if !self.now.is_multiple_of(ticks(1000)) {
            return;
        }
        for &p in &self.players {
            let Some(i) = self.index(p) else { continue };
            let u = &mut self.units[i];
            u.hp_hist.rotate_right(1);
            u.hp_hist[0] = u.state.hp;
        }
    }

    fn update_combat_flags(&mut self) {
        for i in 0..self.units.len() {
            let u = &self.units[i];
            let id = u.state.id;
            let is_player = self.is_player(id);
            let now_in = u.state.alive
                && if is_player {
                    self.now < u.combat_lock_until
                        || self
                            .units
                            .iter()
                            .any(|o| o.state.alive && o.threat.contains_key(&id))
                } else {
                    !u.threat.is_empty()
                };
            if now_in != u.state.in_combat {
                self.units[i].state.in_combat = now_in;
                if !now_in {
                    self.units[i].regen_next =
                        self.now + ticks(self.content.config.regen.interval_ms);
                }
                if now_in && !is_player {
                    self.reset_periodic_yell(i);
                    // Only the first unit of a pack shouts; in_combat is updated in-loop so
                    // later members of the same pull see it.
                    let pack = self.units[i].state.pack;
                    let pack_already = pack.is_some()
                        && self.units.iter().enumerate().any(|(o, x)| {
                            o != i && x.state.alive && x.state.in_combat && x.state.pack == pack
                        });
                    if !pack_already {
                        self.request_yell(i, YellKind::Aggro);
                    }
                }
                self.events.push(if now_in {
                    Event::EnterCombat { unit: id }
                } else {
                    Event::LeaveCombat { unit: id }
                });
            }
        }
    }

    fn reset_periodic_yell(&mut self, i: usize) {
        let ms = self.content.units[self.units[i].def].yells.periodic_ms;
        self.units[i].yell_periodic_next = self.now + ticks(ms);
    }

    pub(crate) fn request_yell(&mut self, i: usize, kind: YellKind) {
        self.request_yell_line(i, kind, None);
    }

    /// `line: Some` carries an explicit text (ability and phase yells); otherwise the
    /// unit's list for `kind` is used and an empty list keeps the unit silent.
    pub(crate) fn request_yell_line(&mut self, i: usize, kind: YellKind, line: Option<String>) {
        let silent = self.content.units[self.units[i].def]
            .yells
            .lines(kind)
            .is_empty();
        if line.is_none() && silent {
            return;
        }
        let p = &mut self.units[i].yell_pending;
        if p.as_ref().is_none_or(|(old, _)| kind < *old) {
            *p = Some((kind, line));
        }
    }

    /// Emits at most one yell per unit per tick; runs after all triggers of the tick.
    fn flush_yells(&mut self) {
        for i in 0..self.units.len() {
            let def = &self.content.units[self.units[i].def];
            let (periodic_ms, ambient_ms) = (def.yells.periodic_ms, def.yells.ambient_ms);
            let u = &self.units[i];
            if u.state.alive && !u.despawned {
                let low = u64::from(u.state.hp) * 100 <= u64::from(u.state.max_hp) * 30;
                if !low {
                    self.units[i].yell_low_done = false;
                } else if !u.yell_low_done {
                    self.units[i].yell_low_done = true;
                    self.request_yell(i, YellKind::LowHp);
                }
                let u = &self.units[i];
                if periodic_ms > 0 && u.state.in_combat && self.now >= u.yell_periodic_next {
                    self.request_yell(i, YellKind::Periodic);
                }
                let u = &self.units[i];
                if ambient_ms > 0 && !u.state.in_combat && self.now >= u.ambient_next {
                    self.units[i].ambient_next = self.now + ticks(ambient_ms);
                    // The timer still advances out of range so a late arrival does not
                    // trigger an instant line.
                    if self.player_within(i, AMBIENT_RANGE_M) {
                        self.request_yell(i, YellKind::Ambient);
                    }
                }
            }
            let Some((kind, line)) = self.units[i].yell_pending.take() else {
                continue;
            };
            if !self.yell_allowed(i, kind) {
                continue;
            }
            let text = match line {
                Some(t) => t,
                None => {
                    let lines = self.content.units[self.units[i].def].yells.lines(kind);
                    let cur = &mut self.units[i].yell_cursor[kind as usize];
                    let t = lines[*cur % lines.len()].clone();
                    *cur += 1;
                    t
                }
            };
            self.reset_periodic_yell(i);
            self.events.push(Event::Yell {
                unit: Self::id_of(i),
                text,
            });
        }
    }

    fn player_within(&self, i: usize, range_m: f32) -> bool {
        self.players
            .iter()
            .filter_map(|p| self.index(*p))
            .any(|p| in_range(self.units[p].state.pos, self.units[i].state.pos, range_m))
    }

    /// Boss phase/aggro/death yells always go through; every other hostile yell shares one
    /// global gap so a pack does not flood the log. Suppressed yells are dropped, not queued.
    fn yell_allowed(&mut self, i: usize, kind: YellKind) -> bool {
        let u = &self.units[i].state;
        if !u.hostile || kind == YellKind::Phase {
            return true;
        }
        if u.boss && matches!(kind, YellKind::Aggro | YellKind::Death) {
            return true;
        }
        if self.now < self.hostile_yell_next {
            return false;
        }
        self.hostile_yell_next = self.now + ticks(HOSTILE_YELL_GAP_MS);
        true
    }

    fn sync_views(&mut self) {
        let now = self.now;
        for idx in 0..self.units.len() {
            let stunned = self.has_aura_flag(idx, |a| a.stun);
            let silenced = self.has_aura_flag(idx, |a| a.silence);
            let xp_to_next = if self.is_player(Self::id_of(idx)) {
                self.content.xp_to_next(self.units[idx].state.level)
            } else {
                0
            };
            let u = &mut self.units[idx];
            u.state.stunned = stunned;
            u.state.silenced = silenced;
            u.state.xp_to_next = xp_to_next;
            u.state.cast = u.cast.as_ref().map(|c| {
                let total_ms = ticks_to_ms(c.end - c.start) as u32;
                CastView {
                    spell: c.spell,
                    elapsed_ms: (ticks_to_ms(now.saturating_sub(c.start)) as u32).min(total_ms),
                    total_ms,
                    interruptible: c.interruptible,
                }
            });
            let defs: &[AuraDef] = &self.content.auras;
            u.state.auras = u
                .auras
                .iter()
                .filter_map(|a| {
                    let d = defs.iter().find(|d| d.key == a.key)?;
                    Some(AuraView {
                        key: a.key.clone(),
                        name: d.name.clone(),
                        icon: d.icon.clone(),
                        harmful: d.harmful,
                        remaining_ms: ticks_to_ms(a.expires.saturating_sub(now)) as u32,
                        total_ms: a.total_ms,
                        absorb_remaining: a.absorb,
                        stacks: a.stacks,
                    })
                })
                .collect();
            u.state.swing_remaining_ms = if u.state.auto_attack {
                ticks_to_ms(u.swing_next.saturating_sub(now)) as u32
            } else {
                0
            };
            u.state.respawn_in_ms = u
                .respawn_at
                .filter(|_| !u.state.alive)
                .map(|t| ticks_to_ms(t.saturating_sub(now)) as u32);
        }
    }
}
