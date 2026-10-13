# Combat API contract

Shared contract between `combat_core` (rules) and `game_client` (presentation). Change it only together with every consumer.

## combat_core (pure Rust, deterministic, 30 Hz integer ticks)

Content is data-driven: `content/game.ron` (spells, auras, units, items, levels, zones, quests, balance), parsed by `combat_core::Content::from_ron(&str)`. The client embeds it with `include_str!`, which also works on wasm. Every reference (spell ids, aura keys, unit keys, summons, zones, links, unlocks, quest objectives, learn levels) is validated at load; a bad file is a `ContentError`.

```rust
pub struct SpellId(pub u32);              // Copy, Eq, Hash, Ord, Debug
pub enum School { Physical, Arcane }      // only two; a school lockout locks one of them

pub struct Content { pub config, pub auras, pub spells, pub units, pub items, pub action_bar, pub levels, pub zones, pub quests }
impl Content {
    pub fn from_ron(src: &str) -> Result<Content, ContentError>;
    pub fn spell(&self, id: SpellId) -> Option<&SpellDef>;
    pub fn action_bar(&self) -> &[SpellId];          // full bar, keys 1..=0 in order (10 slots); see Sim::known_spells
    pub fn channel_spell(&self) -> Option<&SpellDef>;  // Coffee Break (first spell with a `channel`), off the bar
    pub fn unit_def(&self, key: &str) -> Option<&UnitDef>;
    pub fn item(&self, name: &str) -> Option<&ItemDef>;   // trophy table entry by loot item name
    pub fn aura(&self, key: &str) -> Option<&AuraDef>;
    pub fn zone(&self, id: &str) -> Option<&ZoneDef>;
    pub fn floor(&self, id: u8) -> Option<&ZoneDef>;  // legacy: zone "tower/<id>"
    pub fn quest(&self, id: &str) -> Option<&QuestDef>;
    pub fn max_level(&self) -> u8;                   // 10
    pub fn xp_to_next(&self, level: u8) -> u32;      // XP bar maximum at `level`; 0 at the cap
    pub fn kill_xp(&self, player_level: u8, mob_level: u8, reward: u32) -> u32;
    pub fn gcd_ms(&self, haste_pct: u32) -> u32;
    pub fn cast_ms(&self, spell: &SpellDef, haste_pct: u32) -> u32;   // haste only; auras are applied by the sim
}
pub const MAX_ACTION_BAR: usize = 10;
pub fn pack_offsets(n: usize) -> Vec<(f32, f32)>;    // shared pack formation, see Zones

pub struct Sim { /* ... */ }
impl Sim {
    pub fn new(content: Content, seed: u64) -> Sim;
    pub fn set_blockers(&mut self, blockers: Vec<Aabb>);
    pub fn spawn_player(&mut self, pos: Vec3, yaw: f32) -> EntityId;      // level 1, 0 XP; adds a group member, 0 once MAX_PLAYERS (5) are in
    pub fn remove_player(&mut self, id: EntityId) -> bool;                // leave mid-session, see Co-op group
    pub fn players(&self) -> &[EntityId];                                 // join order
    pub fn apply_as(&mut self, player: EntityId, cmd: Command);           // queued like push, as that player
    pub fn spawn_unit(&mut self, unit_key: &str, pos: Vec3, yaw: f32) -> Result<EntityId, ContentError>; // authored level, respawns
    pub fn load_zone(&mut self, zone: &str, anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>, blockers: Vec<Aabb>) -> Result<(), ContentError>;
    pub fn zone(&self) -> Option<&str>;
    pub fn zone_links(&self, zone: &str) -> Vec<ZoneLinkView>;          // ZoneLinkView { anchor, to, unlocked }, content order
    pub fn zone_cleared(&self, zone: &str) -> bool;                       // its boss died this session
    pub fn load_floor(&mut self, floor: u8, anchors, blockers) -> Result<(), ContentError>; // legacy: load_zone("tower/<floor>")
    pub fn floor(&self) -> Option<u8>;                                    // legacy: n of a loaded "tower/<n>"
    pub fn quests(&self) -> &QuestLog;                                    // QuestLog { active: Vec<ActiveQuest { id, counts }>, completed: Vec<String> }
    pub fn quest_status(&self, quest: &str) -> QuestStatus;               // Locked | Available | Active | Ready | Done
    pub fn set_respawn_point(&mut self, pos: Vec3, yaw: f32);             // where RespawnPlayer puts the player
    pub fn inventory(&self) -> &[(String, u32)];                           // player trophies (name, copies), first pickup first
    pub fn item_totals(&self) -> ItemTotals;                              // current trophy bonus, stack cap applied
    pub fn grant_trophy(&mut self, name: &str) -> bool;                    // debug/test: as if looted, no event; false for unknown items
    pub fn set_player_mana(&mut self, mana: u32);                         // debug/test: silent, clamped to max
    pub fn set_player_level(&mut self, level: u8);                        // debug/test: silent, clamps to 1..=max, 0 XP, full HP/mana
    pub fn known_spells(&self, unit: EntityId) -> Vec<SpellId>;           // learned action-bar spells, bar order
    pub fn push(&mut self, cmd: Command);    // applied at the start of the next tick, as the first player
    pub fn tick(&mut self) -> Vec<Event>;    // advance exactly one 1/30 s tick; every event
    pub fn tick_routed(&mut self) -> Vec<(Audience, Event)>;    // the same tick, with recipients
    pub fn drain_events_routed(&mut self) -> Vec<(Audience, Event)>; // pending events (e.g. queued by load_zone) with recipients
    pub fn now(&self) -> Tick;
    pub fn player(&self) -> Option<EntityId>;                 // the first group member
    pub fn quests_of(&self, player: EntityId) -> &QuestLog;
    pub fn quest_status_of(&self, player: EntityId, quest: &str) -> QuestStatus;
    pub fn unit(&self, id: EntityId) -> Option<&UnitState>;   // None for unknown or despawned ids
    pub fn units(&self) -> impl Iterator<Item = &UnitState>;  // every non-despawned unit, corpses included
    pub fn content(&self) -> &Content;
    pub fn cooldown(&self, caster: EntityId, spell: SpellId) -> CooldownView;
}

pub struct CooldownView { pub gcd_remaining_ms: u32, pub gcd_total_ms: u32, pub cd_remaining_ms: u32, pub cd_total_ms: u32, pub usable: Result<(), CastError> }

pub enum Command {
    // The client owns player movement; it reports the result every tick before tick().
    PlayerMoved { pos: Vec3, yaw: f32, moving: bool, jumping: bool },
    SetTarget { unit: EntityId, target: Option<EntityId> },
    Cast { caster: EntityId, spell: SpellId },   // validates now, or queues if within the queue window
    StopCasting { caster: EntityId },
    Loot { looter: EntityId, corpse: EntityId },
    StartAutoAttack { unit: EntityId },          // targets the unit's current target
    StopAutoAttack { unit: EntityId },
    RespawnPlayer,
    Talk { unit: EntityId },                     // talk to a friendly NPC within loot_range_m (6 m)
    AcceptQuest { quest: String },               // Available + within talk range of a living giver
    TurnInQuest { quest: String },               // Ready + within talk range of a living giver
    AbandonQuest { quest: String },              // drops an active quest and its progress
}

pub struct UnitState {
    pub id: EntityId, pub key: String, pub name: String, pub level: u8, pub hostile: bool,
    pub pos: Vec3, pub yaw: f32,                     // the sim owns positions of non-player units
    pub hp: u32, pub max_hp: u32, pub mana: u32, pub max_mana: u32,
    pub alive: bool, pub in_combat: bool, pub target: Option<EntityId>,
    pub cast: Option<CastView>,                      // CastView { spell, elapsed_ms, total_ms, interruptible }
    pub auras: Vec<AuraView>,                        // AuraView { key, name, icon, harmful, remaining_ms, total_ms, absorb_remaining: Option<u32>, stacks: u8 }
    pub ai: Option<AiState>,                         // Idle | Chase | Attack | Casting | Evade | Dead
    pub lootable: bool, pub respawn_in_ms: Option<u32>,
    pub inventory: Vec<(String, u32)>,   // player trophies (name, copies); empty for everyone else
    pub xp: u32,                                     // player: XP into the current level; hostile: XP reward at its level
    pub auto_attack: bool, pub swing_remaining_ms: u32,
    pub xp_to_next: u32,                             // player: XP bar maximum (0 at level cap); others 0
    pub stunned: bool,                               // the client must freeze player movement while true
    pub silenced: bool,
    pub boss: bool,                                  // stun-immune; show the boss frame
    pub pack: Option<u32>,                           // social aggro group
    pub phase: u8,                                   // boss phase, starts at 1
    pub idle_pose: Option<IdlePose>,                 // Sleeping | Chatting | Sitting (friendly NPC pose hint)
}

pub enum CastError { NoTarget, InvalidTarget, Dead, OutOfRange, NotFacing, NoLineOfSight, NotEnoughMana, OnCooldown, OnGcd, AlreadyCasting, SchoolLocked, Moving, NotInterruptible, NothingToInterrupt, Stunned, Silenced, NotLearned, FullMana }
// CastError::message() -> &'static str: "Out of range.", "Can't do that while stunned.", "Can't do that while silenced.", "You haven't learned that spell yet.", ...

pub enum Audience { All, Player(EntityId) }    // see Co-op group

pub enum Event {
    CastStarted { caster, spell, total_ms }, CastSucceeded { caster, spell, target: Option<EntityId> },
    CastFailed { caster, spell, reason: CastError },
    CastInterrupted { caster, spell, by: Option<EntityId>, lockout_ms },
    CastQueued { caster, spell },
    Damage { source, target, spell: Option<SpellId>, amount, absorbed, crit, combo, school },
    Heal { source, target, spell: Option<SpellId>, amount },          // effective amount (overheal dropped)
    ManaGain { unit, spell, amount },                                  // one channel tick, effective amount (clamped to max mana)
    MeleeSwing { attacker, target },
    Miss { source, target, spell: Option<SpellId> },                 // None = white attack ("Miss"), Some = spell ("Resist")
    AutoAttackToggled { unit, on },
    AuraApplied { target, key },                                     // also re-emitted when a stack is added
    AuraRemoved { target, key, reason: AuraEnd },                    // Expired | Consumed | Depleted | Death
    EnterCombat { unit }, LeaveCombat { unit },
    AiState { unit, state },
    Death { unit, killer: Option<EntityId> },
    XpGained { unit, amount },
    LevelUp { unit, level },
    SpellLearned { unit, spell },
    LootDropped { corpse, items }, Looted { looter, corpse, items },
    Yell { unit, text },
    Respawned { unit },
    Knockback { unit, dir: Vec3, strength: f32 },                    // unit vector on XZ, metres
    Teleported { unit, pos: Vec3 },
    PhaseChanged { unit, phase: u8 },
    Summoned { unit, summoner },
    Despawned { unit },
    ZoneLoaded { zone: String }, ZoneCleared { zone: String }, Unlocked { zone: String },
    FloorLoaded { floor: u8 }, FloorCleared { floor: u8 },          // deprecated twins, tower zones only
    QuestOffered { npc, quests: Vec<String> }, QuestAccepted { quest }, QuestProgress { quest, objective: usize, count, need },
    QuestReady { quest }, QuestCompleted { quest },
}
```

`EntityId` here is a sim id (u64), not a Bevy `Entity`. The client keeps a mapping through the `SimUnit(EntityId)` component. Ids are never reused.

## Co-op group

One `Sim` per room; every player in it is one group (`MAX_PLAYERS` = 5). The single-player API (`player()`, `push`, `quests()`, `quest_status`, `inventory`, `item_totals`, `set_player_*`, `grant_trophy`, `debug_*`) acts on the first player, so single-player behaviour and the golden replays are unchanged.

- Per-player state lives on the player's unit: level, XP, HP/mana, trophies (`UnitState.inventory`), known spells, cooldowns, GCD, cast, school lockouts, spell-yell cooldown, Coffee Break channel, respawn point. Quest logs are keyed by player id (`quests_of`).
- Commands: `apply_as(player, cmd)` queues `cmd` for the next tick as `player`. The actor field (`SetTarget.unit`, `Cast.caster`, `StopCasting.caster`, `Loot.looter`, `Start/StopAutoAttack.unit`) is overwritten with `player`, so a client can only act as itself; `PlayerMoved`, `RespawnPlayer`, `Talk` and the quest commands act for `player`. An id that is not a player is ignored. `push(cmd)` keeps the actor ids as given and runs the player-relative commands for the first player.
- Routed events: `tick_routed` / `drain_events_routed` return `(Audience, Event)` with `Audience::All | Player(id)`. `Player(id)`: `LootDropped` and `Looted` (one per member's roll), `XpGained`, `SpellLearned`, every quest event (`QuestOffered`, `QuestAccepted`, `QuestProgress`, `QuestReady`, `QuestCompleted`), and `CastFailed` / `CastQueued` of a player caster. Everything else (including `LevelUp`, `Death`, `Respawned`, player `Yell`s, `Knockback` and `Teleported` of a player) is `All`. `tick()` returns the same events in the same order without the audience.
- Threat and AI: an idle enemy pulls the nearest living member within `aggro_m` and sight. It attacks its top-threat target; dead and removed players leave every threat table, so it switches to the next. When every member is dead (wipe) or gone, threat tables empty and enemies evade and reset as in single-player.
- Kills, loot, XP: see FORMULAS.md, Co-op group. Every member rolls own loot; XP is split among living members near the kill with a group bonus; quest `Kill` credit goes to the killer and every living member within 60 m. `ClearZone` credit goes to everyone; `Loot` and `Talk` credit only to the acting player. A `QuestDone` unlock opens when any member has turned the quest in.
- Scaling: enemy max HP is multiplied by `100 + group_hp_pct_per_player * (players - 1)` percent at spawn and rescaled (HP fraction kept) on every join and leave.
- `remove_player(id)`: the unit despawns (`Despawned`, `unit(id)` becomes `None`, its threat and every target on it are cleared), its unclaimed loot rolls and quest log are dropped, enemy HP is rescaled. False for a non-player.
- Zones: one shared zone per room. `load_zone` keeps every player, cancels their casts, sets every member's respawn point to `graveyard` and, when the layout provides `player_spawn`, moves member `k` to `player_spawn + pack_offsets(n)[k]` facing its yaw. The server decides when the group travels (for example the leader entering a door) and can pass the arrival point as `player_spawn`. In single-player the client still owns the body and overrides the position with its next `PlayerMoved`.
- Death: `RespawnPlayer` via `apply_as` revives only that member, at its own respawn point.

## Contract with the client: unit keys and zones

- Player `senior_engineer`. Friendly: `intern`, `it_support`, `facilities_guy`, `barista`, `smoking_corner_dev` (quest givers: `intern`, `facilities_guy`, `barista`, `smoking_corner_dev`).
- Trash: `meatproxy`, `vibe_junior`, `copilot_drone`, `scrum_master`, `agile_coach`, `middle_manager`, `sales_rep`, `account_manager`, `linkedin_influencer` (shown as "Network Influencer"), `growth_hacker`, `project_manager`, `product_owner`, `consultant`, `security_analyst`, `ai_ethicist`, `crypto_bro`, `chatbot_minion`, `unreviewed_pr`; campus `delivery_robot`, `scooter_exec`, `unpaid_ambassador`; data center `gpu_cultist`, `cooling_fan`, `token_counter`, `rogue_agent`, `overheated_rack` (add); lab `demo_presenter`, `vc_partner`, `prompt_whisperer`, `agi_hallucination` (add, named "AGI Prototype" like the boss).
- Mini-bosses (sole unit of `pack_6`): `hr_partner` (tower/2), `marketing_lead` (tower/3), `ciso` (tower/4). Not `boss`, so they can be stunned.
- Bosses (`boss: true`): `vibe_coder` (tower/1), `analyst` (tower/2), `customer` (tower/3), `vp_synergy` (tower/4), `ceo` (tower/5), `the_scheduler` (datacenter/1), `cooling_failure` (datacenter/2), `agi_prototype` (lab/2).
- `UnitDef.title` is the nameplate subtitle ("Please Open a Ticket").
- `content.zones[]`: `(id, name, kind: Outdoor | Indoor, level_range: (lo, hi), packs: [(anchor, units)], boss: Option<(anchor, unit)>, friendly: [(anchor, unit)], links: [(anchor, to, requires: Option<Unlock>)])` with `Unlock = ZoneCleared(zone) | BossKilled(unit key of a zone boss) | QuestDone(quest)`.
- Anchors a zone layout provides: `player_spawn` (client only), `graveyard` (respawn point; optional for the sim), `pack_1`..`pack_8`, `boss`, `npc_1`..`npc_6`, door anchors `door:<zone id>`; tower floors keep `elevator` / `elevator_exit`.

| Zone | Name | Levels | Packs | Boss | Friendly | Links (anchor -> zone, requires) |
|---|---|---|---|---|---|---|
| campus | Campus (outdoor) | 1-3 | pack_1..8 | - | npc_1 intern, npc_2 facilities_guy, npc_3 barista, npc_4 smoking_corner_dev | door:tower/1 -> tower/1; door:datacenter/1 -> datacenter/1 (ZoneCleared tower/3); door:lab/1 -> lab/1 (BossKilled ceo) |
| tower/1 | Open Space | 1-2 | pack_1..4 | vibe_coder | npc_2 + npc_3 it_support | door:campus; elevator_exit -> tower/2 (ZoneCleared tower/1) |
| tower/2 | Meeting Rooms | 3-4 | pack_1..6 (6 = hr_partner) | analyst | - | elevator_exit -> tower/3 (ZoneCleared tower/2) |
| tower/3 | Sales & Marketing | 5-6 | pack_1..6 (6 = marketing_lead) | customer | - | elevator_exit -> tower/4 (ZoneCleared tower/3) |
| tower/4 | PMO | 7-8 | pack_1..6 (6 = ciso) | vp_synergy | npc_1 it_support | elevator_exit -> tower/5 (ZoneCleared tower/4) |
| tower/5 | C-Level Penthouse | 9-10 | pack_1..5 | ceo | - | elevator_exit -> campus |
| datacenter/1 | Data Center: Hall A | 6-7 | pack_1..5 | the_scheduler | - | door:campus; door:datacenter/2 (ZoneCleared datacenter/1) |
| datacenter/2 | Data Center: Hall B | 7-8 | pack_1..5 | cooling_failure | - | door:datacenter/1 |
| lab/1 | Innovation Lab: Showroom | 10 | pack_1..5 | - | - | door:campus; door:lab/2 |
| lab/2 | Innovation Lab: Rack Room | 10 | pack_1..3 | agi_prototype | - | door:lab/1 |

## Zones

`load_zone(zone, anchors, blockers)`:

1. Resolves every anchor through the callback first (packs, the boss even if it is dead, friendlies); an unknown zone or a missing anchor returns `ContentError::Invalid` and changes nothing. `graveyard` is optional.
2. Despawns every non-player unit (`Despawned` each; `unit(id)` becomes `None`; the player's target is cleared if it pointed at one) and replaces the blockers.
3. Spawns the packs: unit `k` of a pack stands at `anchor + pack_offsets(n)[k]` (world XZ offsets, not rotated by the anchor yaw; slot 0 is the anchor itself) facing the anchor yaw. Packs in the first half of the list (`index * 2 < pack count`) get the low level of `level_range`, the rest the high level. Each pack gets its own `pack` id. The boss (own pack id) and friendlies keep their authored level. Zone spawns never respawn on their own.
4. Sets every player's respawn point to `graveyard` when the layout has one (otherwise it stays where `set_respawn_point` put it) and moves the group to `player_spawn` when the layout has one (see Co-op group).
5. Emits `ZoneLoaded { zone }`, then `FloorLoaded { floor: n }` for `tower/<n>`. These events come out of the next `tick()`.

Per-session world state (kept in the `Sim`, survives zone loads and player deaths, gone with a new `Sim`):

- A zone boss's death emits `ZoneCleared { zone }` (then `FloorCleared` for tower zones) once; that boss is never spawned again. Campus and lab/1 have no boss and never clear.
- Trash: every pack unit has a slot (pack units numbered in content order). Killed slots stay empty when the player re-enters within `TRASH_RESPAWN_MS` (300 s of sim time) after leaving, or reloads the zone it is in; later the whole zone respawns fresh (and gives XP again).
- Unlocks: after a zone clear, a zone-boss kill or a quest turn-in, every link whose `requires` became true opens; `Unlocked { zone }` is emitted once per target zone (for the first gated link into it). `zone_links` reports each exit with its current state; links without `requires` are always open. A hand-spawned (`spawn_unit`) boss is not a zone boss and unlocks nothing.

The player keeps level, XP, HP, cooldowns, trophies and quests across zones. After `Respawned` for the player the client must move its body to `UnitState.pos`. When the player dies every enemy loses its threat and evades: trash walks home and heals, bosses also reset their phase and auras, summoned adds despawn when they get home.

Event order on a zone boss kill: `Death`, `AiState Dead`, `XpGained`/`LevelUp`/`SpellLearned`, `LootDropped`, `ZoneCleared`, `FloorCleared` (tower), `QuestProgress`/`QuestReady` (ClearZone, then Kill objectives), `Unlocked`.

`load_floor(n, ..)` is `load_zone("tower/<n>", ..)`, kept until the client switches; the `Floor*` events, `Sim::floor()` and `Content::floor()` go with it.

## Quests

`content.quests[]`: `(id, name, giver: (npc, zone), text, progress_text, done_text, objectives: [Kill(unit_key, n) | Loot(item, n) | Talk(npc_key) | ClearZone(zone)], reward: (xp, trophy: Option<item>), requires: Option<quest id>, level_min)`. Validation: the giver is a friendly spawned in its zone; Kill targets a hostile unit, Talk a friendly one, Loot an `items` entry, ClearZone a zone with a boss; `requires` names a quest listed earlier.

- State is per player (`Sim::quests()` returns the player's log; the map is keyed by player id). Status: `Done` once turned in, `Active`/`Ready` while in the log, `Available` when `requires` is done and the player is at least `level_min`, otherwise `Locked`.
- `Talk` to a giver counts `Talk(npc)` objectives first, then emits `QuestOffered { npc, quests }` (its Available and Ready quests, content order) when that list is not empty.
- `AcceptQuest`: Available and within `loot_range_m` of a living giver unit, else ignored silently. Emits `QuestAccepted`. A zone already cleared counts for `ClearZone` and copies already carried count for `Loot` (capped at need); if that completes the quest, `QuestReady` follows.
- Progress: `Kill` counts the player's own kills only (killer = player, DoTs included); `Loot` counts items the player picks up (after `Looted`); `ClearZone` counts the zone's first clear. Each increment emits `QuestProgress { quest, objective, count, need }` (capped at need); completing the last objective emits `QuestReady` once.
- `TurnInQuest`: Ready and near a giver. Emits `QuestCompleted`, grants the trophy (silent pickup, like `grant_trophy`) and the XP (`XpGained`/`LevelUp`; no level-difference adjustment, nothing at the cap), then checks unlocks.
- `AbandonQuest` removes the quest and its counts silently; it becomes Available again.
- Nothing here uses the rng, so quests never change combat rolls.

## Levels and spells

- `spawn_player` starts at level 1. Stats = the `senior_engineer` def plus the cumulative `levels[level-1]` bonus (HP, mana, spell power). Level-ups refill HP and mana.
- Kill XP goes to players only, split across the group (see FORMULAS.md). The player's `UnitState.xp` is progress inside the current level; `xp_to_next` is the bar maximum (`config.xp_per_level` is kept as the level-1 value). Overflow carries into the next level; at the cap XP stays 0 and kills emit no `XpGained`.
- Event order on a kill: `Death`, `AiState Dead`, `XpGained`, then per level gained `LevelUp { level }` followed by one `SpellLearned` per bar spell whose `learn_level` equals the new level, then `LootDropped`, then `FloorCleared` (boss only).
- `SpellDef.learn_level`; the bar is `[Code Review 1, Refactor 1, Guardrails 3, Reject PR 2, git blame 4, Rollback 5, On-Call Caffeine 6, Rubber Duck 7, Postmortem 8, Write It By Hand 9]`. Reject PR unlocks at 2 and Guardrails at 3 (swapped versus GAME_DESIGN.md) so the floor-1 boss's 4,000-Line Diff can be interrupted at the floor's levels. `Sim::known_spells(player)` lists the learned subset; casting an unlearned spell fails with `NotLearned`, and `CooldownView.usable` says so too, so the HUD can grey the slot.
- Other units: monsters spawned above or below their authored `level` scale HP, every damage/heal they deal, and XP reward by `config.mob_scaling` (FORMULAS.md). Summons take their summoner's level.

## Spells, effects and auras

`SpellDef`: `effect` plus `extra_effects: Vec<Effect>` (resolved in order), `target: Enemy | Caster` (default Enemy), `area: Option<(radius_m, allies)>`, `interruptible` (default true), `learn_level` (default 1), `yells`.

- `target: Enemy` spells need a hostile, living, non-evading target and check range, facing (at start) and LoS; `Caster` spells target the caster. With `area` set, the target-side effects hit every living unit within `radius_m` of the caster on the caster's side (`allies: true`, caster included) or the other side.
- Effects: `Damage { base, coefficient_pct, apply_aura?, consume_aura?, execute? }`, `Absorb`, `Interrupt { lockout_ms }`, `Heal { base, coefficient_pct }`, `Aura { aura, duration_ms }`, `RandomAura { auras, duration_ms }` (rng pick), `Dispel { count }` (removes up to `count` harmful auras, newest first, reason `Consumed`), `Lockout { lockout_ms, school? }` (locks `school`, or the school of the target's last cast spell; cancels a cast of that school), `Knockback { strength_m }`, `Teleport { distance_m }` (to `distance_m` in front of the caster's spawn point), `Summon { unit, count, max_alive }`, `ResetAllyThreat { radius_m }`, `ThreatSwap`, `ForwardLastSpell { pct }`, `DrainMana { pct }` (removes `pct` percent of the target's max mana, no event), `Nothing`. `Summon`, `ResetAllyThreat` and `ThreatSwap` act once around the caster; the rest act on each target.
- Only `Damage` and `ForwardLastSpell` roll `spell_hit_pct`; heals, auras, dispels, knockbacks and interrupts always land.
- `AuraDef` flags (all default off): `stun`, `silence`, `periodic: (interval_ms, base, coefficient_pct, heal)`, `school` (of periodic and expiry damage), `cast_time_pct`, `damage_taken_pct`, `damage_done_pct`, `haste_pct` (all per stack), `max_stacks` (default 1), `spread_radius_m`, `expire_damage`, `stack_on_repeat_cast`, `reflect`.
- Re-applying an aura refreshes its duration; a stacking aura gains one stack (up to `max_stacks`) and `AuraApplied` is emitted again. Periodic and expiry amounts are snapshotted from the caster when applied. The first periodic tick comes one interval after application; a tick due on the expiry tick still lands.
- Applying a harmful aura to an enemy that has no threat on the caster yet pulls it (threat 1), so a DoT aggroes immediately.
- Stun: no casting (`Stunned`), no AI movement, melee, abilities or auto-attack swings; applying it cancels the current cast (`CastInterrupted { by: Some(source), lockout_ms: 0 }`) and the spell queue. Silence: no spellcasting (`Silenced`); auto attack continues. Bosses ignore stuns; a player spell that would stun a boss fails validation with `InvalidTarget` (Rubber Duck).
- `expire_damage` (Deadline) hits the bearer when the aura runs out (`AuraRemoved Expired` first, then `Damage { spell: Some(applying spell) }`); a dispel (`Consumed`) or death prevents it.
- `spread_radius_m` (git blame): when the bearer dies, a fresh copy goes to the nearest living enemy of the original caster within the radius that does not carry it yet.
- `stack_on_repeat_cast` (Unread Spec): the bearer gains a stack each time it casts the same spell twice in a row.
- `reflect` (Alignment Training): a hit with `spell: Some` aimed at the bearer from another unit hits the source instead, as `Damage { source: bearer, target: caster, spell }`, scaled by the bearer's damage-done and the caster's damage-taken auras. Auto attacks (`spell: None`) and melee still land. A reflected hit is not reflected again.
- `ForwardLastSpell` (MeatProxy's Forwarded Reply): repeats the pre-absorb amount of the last damaging player spell that landed on the caster, at `pct` percent, as `Damage { spell: Some(Forwarded Reply) }` in that spell's school; does nothing if the player never hit it.
- `Knockback`: direction from caster to target on XZ. Sim-owned units are moved immediately; for the player only the event is emitted and the client applies the impulse. `Teleport` sets the target's position (the player's too) and emits `Teleported`; the client must move the player body.
- Death spells: `UnitDef.death_spell` is resolved by the corpse right after `Death` (Chatbot Minion's `Hallucinated Hotfix`, an area hit around it), emitting `CastSucceeded` from the corpse.

## Enemy AI

`AiDef.abilities` is an ordered rule list; each tick the first ready rule fires: `(spell, cooldown_min_ms, cooldown_max_ms, initial_ms: Option<(lo, hi)>, target: Current | Caster | AllyLowestHp | RandomHostile, phases: [u8], yells)`.

- Timers: on the pull every rule rolls its first use from `initial_ms` (default: the cooldown range); after each use the cooldown range is rolled. Rules gated by `phases` (empty = always) roll their initial delay again when that phase begins.
- A rule waits (cooldown untouched) while the unit is stunned or silenced, its school is locked, or no unit satisfies the target rule: `Current` needs the top-threat target within `range_m` and LoS; `AllyLowestHp` picks the most injured unit of its side (itself included) by HP percent, skipping when all are full; `RandomHostile` picks with the rng among enemies in range and LoS.
- Starting a rule's cast emits `CastStarted` and a yell from the rule's `yells` (round-robin), or the unit's `blast` lines when the rule has none.
- `AiDef.phases: [(below_hp_pct, spell?, target, yells)]`, strictly descending. When HP drops to `below_hp_pct` or less the unit enters the next phase (`PhaseChanged`, phases numbered from 2), yells one of the phase lines and resolves `spell` instantly (no cast time).
- Social aggro: when a unit with a `pack` id gets its first threat, every idle living member of the same pack gets 1 threat on the same source and starts chasing. Summons join their summoner's pack and immediately attack the summoner's target.
- Self-inflicted damage (Phishing Test) never adds the unit to its own threat table.
- Evade (empty threat table or beyond `leash_m`) cancels the cast, removes all auras (Scope Creep stacks too), resets the phase to 1 (`PhaseChanged { phase: 1 }` if it had changed), walks home and heals; a summoned unit despawns instead of healing. Evading a summoner also despawns its living adds at once (`Despawned`), so no stragglers are left behind.
- Adds follow their summoner: when a unit dies, every living unit whose `summoner` is that unit dies in the same tick (`Death { killer: None }`, then `AiState Dead`), so no XP, loot, death spell or yell. A floor boss's adds are additionally despawned (`Despawned`) before `FloorCleared`.
- Idle wandering (`Sim::set_wandering(false)` turns it off; tests that pin fight timing do): hostile units with `ai == Idle` and no threat, alive and not stunned, pick a point 1.5-4 m from their spawn (bosses: 1-2 m), walk there at 1.4 m/s (bosses 1.0), idle 3-8 s and repeat. Targets keep 1 m clearance to blockers (padded segment test), pack members stay within 4 m of the pack anchor (slot 0), summons never wander. Friendly NPCs without `idle_pose` Sleeping/Sitting stroll up to 6 m away and walk back (6-15 s pauses). The client may override a unit's pose with `Sim::set_idle_pose` (floor dressing seats the sleeping IT Support). Aggro checks run every tick, so a wanderer still pulls; evade and leash walk home at chase speed. `UnitState.pos`/`yaw` carry the motion (yaw turns at 6 rad/s before walking), the client derives Walk vs Run from speed. Each unit has its own SplitMix64 stream (seed ^ id), so wandering never changes combat rolls.

## game_client: combat to animation contract

`crates/game_client/src/combat_anim.rs` defines `CombatAnim`. The combat bridge writes it on the player and unit entities; the animation system reads it. Locomotion (idle, walk, run, backpedal, jump) is derived by the animation system itself from transform movement.

## Implemented semantics (combat_core)

- Spell 5 "Hallucination Blast" and spells 100+ are enemy-only and not on the action bar.
- `UnitState.xp`: for hostile units the XP reward at their spawned level (before the level-difference adjustment), for the player the XP inside the current level. `Event::XpGained.unit` is the player who gained it.
- Event timing: instant spells emit only `CastSucceeded` (no `CastStarted`); cast-time spells emit `CastStarted` then `CastSucceeded` on completion. Mana is spent and the spell cooldown starts on success; the GCD starts on cast start. `CastSucceeded` precedes its `Damage`/aura events; a consumed aura is reported before the `Damage` it boosts.
- `CastInterrupted { by: None, lockout_ms: 0 }` is used for movement/jump cancel, `StopCasting`, teleports, death and evade resets; `by: Some` carries an interrupt (with the school lockout), a stun or a silence (`lockout_ms: 0`).
- Pushback has no event: the `CastView.total_ms` of the unit grows (by `pushback_ms`, at most `pushback_max` times). Player (and other non-AI units) only.
- Off-GCD instants (`gcd: false`, `cast_ms: 0`: Reject PR, On-Call Caffeine) are usable during the GCD and during an own cast without cancelling it.
- `CooldownView.usable` is the complete verdict now (learned, stun, silence, lockout, cooldown, target, range, facing, LoS, mana, GCD/cast busy), so the HUD can grey buttons from it. `gcd_*` fields are zero for off-GCD spells.
- Trophies: every loot item has an `items` entry `(name, icon, description, bonus)` in `content/game.ron`; `Content::from_ron` rejects a unit that drops an item without one, duplicate names, an empty bonus, `crit_pct` outside 0..=5 and a description over `ITEM_DESC_MAX` (110) chars. `bonus` is any of `max_hp`, `max_mana`, `spell_power`, `crit_pct` (tenths allowed), `haste_pct`, `mana_regen` (extra mana per 5 s in combat). `ItemTotals` carries the sums with crit in tenths (`crit_tenths`).
- `Looted { looter, corpse, items }` is also the pickup: when the looter is the player, each item is appended to `UnitState.inventory` (copies are counted per name, first pickup order) and the bonuses apply in the same tick, before the event is observable. `max_hp`/`max_mana` grow and the current HP/mana scale up by the same ratio (`max(cur, cur * new_max / old_max)`), never down. Only the first `ITEM_STACK_CAP` (3) copies of an item add bonus; further copies are kept and counted but add nothing. Bonuses live in the player's unit, so they persist across `load_floor`, `RespawnPlayer` and level-ups; a new `Sim` (restart) starts empty. Looting never touches the RNG, so replays only change when a bonus changes a later roll.
- A corpse is always `lootable` after death; `LootDropped.items` / `Looted.items` may be empty. `Command::Loot` outside `loot_range_m` or on a non-corpse is silently ignored.
- Hostile units die with `ai: Some(AiState::Dead)`; the player has `ai: None`. `Command::RespawnPlayer` on a living player is ignored.
- Damage to a unit in `AiState::Evade` is dropped entirely (no `Damage` event, no threat, no aura).
- Auto attack ("Nitpick", `UnitDef.auto_attack`: range 25 m, 1800 ms, 9..=14 Physical, crits with the unit's crit chance) swings when enabled and the current target exists, is hostile, alive, not evading, in range, in the facing cone and in LoS, and the attacker is neither casting a cast-time spell nor stunned. Otherwise a ready swing just waits (no event, no error). It uses no GCD, generates threat like damage and starts combat. A swing emits `MeleeSwing`, then `Damage { spell: None }` or `Miss { spell: None }`; both consume the swing timer. The timer persists across stop/start.
- `StartAutoAttack` with no target, a friendly, dead or unknown target, a dead unit or a unit without an auto attack is ignored silently. Starting while already on emits nothing.
- Auto attack turns off (emitting `AutoAttackToggled { on: false }`) on `StopAutoAttack`, on `SetTarget` to a different target or `None`, on target death or despawn, on attacker death and on respawn.
- `Miss` replaces `Damage` for that attack. A missed spell still emits `CastSucceeded` first and spends mana, GCD and cooldown, but applies no aura, consumes no Code Smell (no combo) and generates no threat. Roll order: hit, then crit; absorb applies after a hit.
- `Command::Talk` on a living friendly unit within `loot_range_m` makes it yell its next `talk` line at the end of the tick; anything else is ignored silently.

## Channels (Coffee Break)

`SpellDef.channel: Option<(interval_ms, mana_pct)>` makes a spell a drink-style channel (spell 12, "Coffee Break", off the action bar, learned at level 1; the client binds it to C). `cast_ms` is the full duration (6000), every `interval_ms` (1000) the caster regains `mana_pct` (8) percent of max mana. HP is untouched. Content validation: `0 < interval_ms <= cast_ms`, `mana_pct` 1..=100, `target: Caster`.

- Start: `CastStarted { total_ms: 6000 }` (the GCD starts, no mana cost), then the spell yell (at the start, since a channel has no release; the usual 20 s player yell gap applies). Not haste- or aura-scaled.
- Each tick: `ManaGain { unit, spell, amount }`. The tick that lands on the final tick still counts. Reaching full mana ends the channel early with `CastSucceeded`; so does the full duration.
- Cancelled with `CastInterrupted { by: None, lockout_ms: 0 }` by moving or jumping, stun/death, casting another spell (including off-GCD instants; the new spell starts normally, only the GCD gates it), or taking damage (`dealt > 0`; fully absorbed hits do not break it). A channel is never pushed back. Pressing the same spell again fails with `AlreadyCasting`.
- Cooldown (8000 ms) starts when the channel ends, by completion or by any cancel.
- Unusable at full mana: `CastError::FullMana` ("You are already at full mana."). Also `Moving` while moving, `OnCooldown`, `OnGcd` as usual. Usable in combat; damage then cancels it.
- `CooldownView.usable` is `Err(AlreadyCasting)` for Coffee Break while it channels. Auto attack swings pause during the channel like during any cast.

## Yells

`UnitDef.yells` (RON, serde default = silent): `periodic_ms` plus line lists `aggro`, `periodic`, `blast`, `low_hp`, `player_killed`, `death`, `evade`, `talk`, and `ambient_ms` + `ambient`. Ability and phase lines live on `AiAbility.yells` and `PhaseDef.yells`. A unit yells via `Event::Yell { unit, text }`, emitted at the end of the tick (after `Death`/`EnterCombat`/`CastStarted` of that tick).

- Triggers: entering combat; every `periodic_ms` while in combat and alive; starting an AI ability cast; entering a phase; hp first dropping to <= 30% (re-armed when hp is back above 30%); killing the player; its own death; starting evade; `Command::Talk`; every `ambient_ms` while alive and out of combat, emitted only while the player is within 15 m (the timer keeps running out of range; first one staggered by unit id; same-kind NPCs start at different lines so neighbours banter).
- At most one yell per unit per tick. Priority: death > player_killed > phase > low_hp > ability (blast) > aggro > evade > talk > periodic > ambient. Any emitted yell restarts the periodic timer.
- Pack aggro: only the first unit of a pack to enter combat yells `aggro`; later members of the same pull stay silent (units without a pack always yell).
- Global hostile throttle: all hostile yells share a 2500 ms gap. Boss phase, aggro and death yells bypass it (as does any `Phase` yell); every other hostile yell inside the gap is dropped, not queued, and does not advance its line cursor. Friendly and player yells are not throttled. Units are flushed in index order, so ties on one tick go to the lowest id.
- Lines cycle round-robin per unit and trigger (per rule for ability lines); the combat rng is never consumed, so yells cannot change hit/miss/crit outcomes.

### Player spell yells

`SpellDef.yells: Vec<String>` (RON, serde default = silent). When the player's spell is released (`CastSucceeded`) the player emits `Event::Yell { unit: player, text }` right after `CastSucceeded`. Not emitted for the Nitpick auto attack, failed or interrupted casts, or enemy casts.

- Lines cycle round-robin per spell; the rng is never consumed.
- `config.player_yell_cooldown_ms` (20000) is a global gate between player spell yells; the first eligible cast yells, and a gated cast neither yells nor advances its cursor.

## Regeneration

`config.regen: (interval_ms, hp_pct, mana_pct)`: alive units out of combat regain `hp_pct`/`mana_pct` of max every `interval_ms`, first tick one interval after leaving combat (or spawn/respawn). No event; observe `UnitState.hp/mana`. In combat only `mana_regen_per_5s_combat` trickles. See FORMULAS.md.

## Pathfinding

`set_blockers` / `load_floor` rebuild a 0.5 m navigation grid (blockers grown by the 0.45 m unit radius). Chasing, evading and wandering units walk straight while the line is clear and otherwise follow a cached A* route (re-planned every 0.5 s or when the goal moves more than 1.5 m, 6000 expansions max). A chaser or evader with no route gives up after 5 s: a chaser evades, an evader resets at its spawn. `Sim::find_path(from, to) -> Option<Vec<Vec3>>` returns the smoothed waypoints after `from`, ending at `to`; it allocates, so call it from tools, not per tick.
