# Combat API contract

Shared contract between `combat_core` (rules) and `game_client` (presentation). Change it only together with every consumer.

## combat_core (pure Rust, deterministic, 30 Hz integer ticks)

Content is data-driven: `content/game.ron` (spells, auras, units, levels, floors, balance), parsed by `combat_core::Content::from_ron(&str)`. The client embeds it with `include_str!`, which also works on wasm. Every reference (spell ids, aura keys, unit keys, summons, floors, learn levels) is validated at load; a bad file is a `ContentError`.

```rust
pub struct SpellId(pub u32);              // Copy, Eq, Hash, Ord, Debug
pub enum School { Physical, Arcane }      // only two; a school lockout locks one of them

pub struct Content { pub config, pub auras, pub spells, pub units, pub action_bar, pub levels, pub floors }
impl Content {
    pub fn from_ron(src: &str) -> Result<Content, ContentError>;
    pub fn spell(&self, id: SpellId) -> Option<&SpellDef>;
    pub fn action_bar(&self) -> &[SpellId];          // full bar, keys 1..=0 in order (10 slots); see Sim::known_spells
    pub fn unit_def(&self, key: &str) -> Option<&UnitDef>;
    pub fn aura(&self, key: &str) -> Option<&AuraDef>;
    pub fn floor(&self, id: u8) -> Option<&FloorDef>;
    pub fn max_level(&self) -> u8;                   // 10
    pub fn xp_to_next(&self, level: u8) -> u32;      // XP bar maximum at `level`; 0 at the cap
    pub fn kill_xp(&self, player_level: u8, mob_level: u8, reward: u32) -> u32;
    pub fn gcd_ms(&self, haste_pct: u32) -> u32;
    pub fn cast_ms(&self, spell: &SpellDef, haste_pct: u32) -> u32;   // haste only; auras are applied by the sim
}
pub const MAX_ACTION_BAR: usize = 10;
pub fn pack_offsets(n: usize) -> Vec<(f32, f32)>;    // shared pack formation, see Floors

pub struct Sim { /* ... */ }
impl Sim {
    pub fn new(content: Content, seed: u64) -> Sim;
    pub fn set_blockers(&mut self, blockers: Vec<Aabb>);
    pub fn spawn_player(&mut self, pos: Vec3, yaw: f32) -> EntityId;      // level 1, 0 XP
    pub fn spawn_unit(&mut self, unit_key: &str, pos: Vec3, yaw: f32) -> Result<EntityId, ContentError>; // authored level, respawns
    pub fn load_floor(&mut self, floor: u8, anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>, blockers: Vec<Aabb>) -> Result<(), ContentError>;
    pub fn floor(&self) -> Option<u8>;
    pub fn set_respawn_point(&mut self, pos: Vec3, yaw: f32);             // where RespawnPlayer puts the player
    pub fn set_player_level(&mut self, level: u8);                        // debug/test: silent, clamps to 1..=max, 0 XP, full HP/mana
    pub fn known_spells(&self, unit: EntityId) -> Vec<SpellId>;           // learned action-bar spells, bar order
    pub fn push(&mut self, cmd: Command);    // applied at the start of the next tick
    pub fn tick(&mut self) -> Vec<Event>;    // advance exactly one 1/30 s tick
    pub fn now(&self) -> Tick;
    pub fn player(&self) -> Option<EntityId>;
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

pub enum CastError { NoTarget, InvalidTarget, Dead, OutOfRange, NotFacing, NoLineOfSight, NotEnoughMana, OnCooldown, OnGcd, AlreadyCasting, SchoolLocked, Moving, NotInterruptible, NothingToInterrupt, Stunned, Silenced, NotLearned }
// CastError::message() -> &'static str: "Out of range.", "Can't do that while stunned.", "Can't do that while silenced.", "You haven't learned that spell yet.", ...

pub enum Event {
    CastStarted { caster, spell, total_ms }, CastSucceeded { caster, spell, target: Option<EntityId> },
    CastFailed { caster, spell, reason: CastError },
    CastInterrupted { caster, spell, by: Option<EntityId>, lockout_ms },
    CastQueued { caster, spell },
    Damage { source, target, spell: Option<SpellId>, amount, absorbed, crit, combo, school },
    Heal { source, target, spell: Option<SpellId>, amount },          // effective amount (overheal dropped)
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
    FloorLoaded { floor: u8 }, FloorCleared { floor: u8 },
}
```

`EntityId` here is a sim id (u64), not a Bevy `Entity`. The client keeps a mapping through the `SimUnit(EntityId)` component. Ids are never reused.

## Contract with the client: unit keys and floors

- Player `senior_engineer`. Friendly: `intern`, `it_support`.
- Trash: `meatproxy`, `vibe_junior`, `copilot_drone`, `scrum_master`, `agile_coach`, `middle_manager`, `sales_rep`, `account_manager`, `linkedin_influencer` (shown as "Network Influencer"), `growth_hacker`, `project_manager`, `product_owner`, `consultant`, `security_analyst`, `ai_ethicist`, `crypto_bro`, `chatbot_minion`, `unreviewed_pr`.
- Mini-bosses (sole unit of `pack_6`): `hr_partner` (floor 2), `marketing_lead` (floor 3), `ciso` (floor 4). Not `boss`, so they can be stunned.
- Bosses (`boss: true`): `vibe_coder` (1), `analyst` (2), `customer` (3), `vp_synergy` (4), `ceo` (5).
- `UnitDef.title` is the nameplate subtitle ("Please Open a Ticket").
- `content.floors[]`: `(id, name, level_range: (lo, hi), packs: [(anchor, units)], boss: (anchor, unit), friendly: [(anchor, unit)])`. Anchors used: `pack_1`..`pack_6`, `boss`, `npc_1`..`npc_3`.

| Floor | Name | Levels | Packs | Friendly |
|---|---|---|---|---|
| 1 | Open Space | 1-2 | pack_1..4 | npc_1 intern, npc_2 + npc_3 it_support |
| 2 | Meeting Rooms | 3-4 | pack_1..6 (6 = hr_partner) | - |
| 3 | Sales & Marketing | 5-6 | pack_1..6 (6 = marketing_lead) | - |
| 4 | PMO | 7-8 | pack_1..6 (6 = ciso) | npc_1 it_support |
| 5 | C-Level Penthouse | 9-10 | pack_1..5 | - |

## Floors

`load_floor(floor, anchors, blockers)`:

1. Resolves every anchor through the callback first; an unknown floor or a missing anchor returns `ContentError::Invalid` and changes nothing.
2. Despawns every non-player unit (`Despawned` each; `unit(id)` becomes `None`; the player's target is cleared if it pointed at one) and replaces the blockers.
3. Spawns the packs: unit `k` of a pack stands at `anchor + pack_offsets(n)[k]` (world XZ offsets, not rotated by the anchor yaw; slot 0 is the anchor itself) facing the anchor yaw. Packs in the first half of the list (`index * 2 < pack count`) get the low level of `level_range`, the rest the high level. Each pack gets its own `pack` id. The boss (own pack id) and friendlies keep their authored level. Floor spawns never respawn: killed trash stays dead until the floor is loaded again.
4. Emits `FloorLoaded { floor }`. All of these events come out of the next `tick()`.

The player keeps level, XP, HP and cooldowns across floors. `set_respawn_point(pos, yaw)` sets where `RespawnPlayer` revives the player (the elevator); call it after each `load_floor`. After `Respawned` for the player the client must move its body to `UnitState.pos`. When the player dies every enemy loses its threat and evades: trash walks home and heals, bosses also reset their phase and auras, summoned adds despawn when they get home.

`FloorCleared { floor }` is emitted when the floor's boss dies (after its `LootDropped`).

## Levels and spells

- `spawn_player` starts at level 1. Stats = the `senior_engineer` def plus the cumulative `levels[level-1]` bonus (HP, mana, spell power). Level-ups refill HP and mana.
- Kill XP goes to the player only (see FORMULAS.md). The player's `UnitState.xp` is progress inside the current level; `xp_to_next` is the bar maximum (`config.xp_per_level` is kept as the level-1 value). Overflow carries into the next level; at the cap XP stays 0 and kills emit no `XpGained`.
- Event order on a kill: `Death`, `AiState Dead`, `XpGained`, then per level gained `LevelUp { level }` followed by one `SpellLearned` per bar spell whose `learn_level` equals the new level, then `LootDropped`, then `FloorCleared` (boss only).
- `SpellDef.learn_level`; the bar is `[Code Review 1, Refactor 1, Guardrails 3, Reject PR 2, git blame 4, Rollback 5, On-Call Caffeine 6, Rubber Duck 7, Postmortem 8, Write It By Hand 9]`. Reject PR unlocks at 2 and Guardrails at 3 (swapped versus GAME_DESIGN.md) so the floor-1 boss's 4,000-Line Diff can be interrupted at the floor's levels. `Sim::known_spells(player)` lists the learned subset; casting an unlearned spell fails with `NotLearned`, and `CooldownView.usable` says so too, so the HUD can grey the slot.
- Other units: monsters spawned above or below their authored `level` scale HP, every damage/heal they deal, and XP reward by `config.mob_scaling` (FORMULAS.md). Summons take their summoner's level.

## Spells, effects and auras

`SpellDef`: `effect` plus `extra_effects: Vec<Effect>` (resolved in order), `target: Enemy | Caster` (default Enemy), `area: Option<(radius_m, allies)>`, `interruptible` (default true), `learn_level` (default 1), `yells`.

- `target: Enemy` spells need a hostile, living, non-evading target and check range, facing (at start) and LoS; `Caster` spells target the caster. With `area` set, the target-side effects hit every living unit within `radius_m` of the caster on the caster's side (`allies: true`, caster included) or the other side.
- Effects: `Damage { base, coefficient_pct, apply_aura?, consume_aura?, execute? }`, `Absorb`, `Interrupt { lockout_ms }`, `Heal { base, coefficient_pct }`, `Aura { aura, duration_ms }`, `RandomAura { auras, duration_ms }` (rng pick), `Dispel { count }` (removes up to `count` harmful auras, newest first, reason `Consumed`), `Lockout { lockout_ms, school? }` (locks `school`, or the school of the target's last cast spell; cancels a cast of that school), `Knockback { strength_m }`, `Teleport { distance_m }` (to `distance_m` in front of the caster's spawn point), `Summon { unit, count, max_alive }`, `ResetAllyThreat { radius_m }`, `ThreatSwap`, `ForwardLastSpell { pct }`, `Nothing`. `Summon`, `ResetAllyThreat` and `ThreatSwap` act once around the caster; the rest act on each target.
- Only `Damage` and `ForwardLastSpell` roll `spell_hit_pct`; heals, auras, dispels, knockbacks and interrupts always land.
- `AuraDef` flags (all default off): `stun`, `silence`, `periodic: (interval_ms, base, coefficient_pct, heal)`, `school` (of periodic and expiry damage), `cast_time_pct`, `damage_taken_pct`, `damage_done_pct`, `haste_pct` (all per stack), `max_stacks` (default 1), `spread_radius_m`, `expire_damage`, `stack_on_repeat_cast`.
- Re-applying an aura refreshes its duration; a stacking aura gains one stack (up to `max_stacks`) and `AuraApplied` is emitted again. Periodic and expiry amounts are snapshotted from the caster when applied. The first periodic tick comes one interval after application; a tick due on the expiry tick still lands.
- Applying a harmful aura to an enemy that has no threat on the caster yet pulls it (threat 1), so a DoT aggroes immediately.
- Stun: no casting (`Stunned`), no AI movement, melee, abilities or auto-attack swings; applying it cancels the current cast (`CastInterrupted { by: Some(source), lockout_ms: 0 }`) and the spell queue. Silence: no spellcasting (`Silenced`); auto attack continues. Bosses ignore stuns; a player spell that would stun a boss fails validation with `InvalidTarget` (Rubber Duck).
- `expire_damage` (Deadline) hits the bearer when the aura runs out (`AuraRemoved Expired` first, then `Damage { spell: Some(applying spell) }`); a dispel (`Consumed`) or death prevents it.
- `spread_radius_m` (git blame): when the bearer dies, a fresh copy goes to the nearest living enemy of the original caster within the radius that does not carry it yet.
- `stack_on_repeat_cast` (Unread Spec): the bearer gains a stack each time it casts the same spell twice in a row.
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
- Evade (empty threat table or beyond `leash_m`) cancels the cast, removes all auras (Scope Creep stacks too), resets the phase to 1 (`PhaseChanged { phase: 1 }` if it had changed), walks home and heals; a summoned unit despawns instead of healing.

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
- A corpse is always `lootable` after death; `LootDropped.items` / `Looted.items` may be empty. `Command::Loot` outside `loot_range_m` or on a non-corpse is silently ignored.
- Hostile units die with `ai: Some(AiState::Dead)`; the player has `ai: None`. `Command::RespawnPlayer` on a living player is ignored.
- Damage to a unit in `AiState::Evade` is dropped entirely (no `Damage` event, no threat, no aura).
- Auto attack ("Nitpick", `UnitDef.auto_attack`: range 25 m, 1800 ms, 9..=14 Physical, crits with the unit's crit chance) swings when enabled and the current target exists, is hostile, alive, not evading, in range, in the facing cone and in LoS, and the attacker is neither casting a cast-time spell nor stunned. Otherwise a ready swing just waits (no event, no error). It uses no GCD, generates threat like damage and starts combat. A swing emits `MeleeSwing`, then `Damage { spell: None }` or `Miss { spell: None }`; both consume the swing timer. The timer persists across stop/start.
- `StartAutoAttack` with no target, a friendly, dead or unknown target, a dead unit or a unit without an auto attack is ignored silently. Starting while already on emits nothing.
- Auto attack turns off (emitting `AutoAttackToggled { on: false }`) on `StopAutoAttack`, on `SetTarget` to a different target or `None`, on target death or despawn, on attacker death and on respawn.
- `Miss` replaces `Damage` for that attack. A missed spell still emits `CastSucceeded` first and spends mana, GCD and cooldown, but applies no aura, consumes no Code Smell (no combo) and generates no threat. Roll order: hit, then crit; absorb applies after a hit.
- `Command::Talk` on a living friendly unit within `loot_range_m` makes it yell its next `talk` line at the end of the tick; anything else is ignored silently.

## Yells

`UnitDef.yells` (RON, serde default = silent): `periodic_ms` plus line lists `aggro`, `periodic`, `blast`, `low_hp`, `player_killed`, `death`, `evade`, `talk`, and `ambient_ms` + `ambient`. Ability and phase lines live on `AiAbility.yells` and `PhaseDef.yells`. A unit yells via `Event::Yell { unit, text }`, emitted at the end of the tick (after `Death`/`EnterCombat`/`CastStarted` of that tick).

- Triggers: entering combat; every `periodic_ms` while in combat and alive; starting an AI ability cast; entering a phase; hp first dropping to <= 30% (re-armed when hp is back above 30%); killing the player; its own death; starting evade; `Command::Talk`; every `ambient_ms` while alive and out of combat (first one staggered by unit id; same-kind NPCs start at different lines so neighbours banter).
- At most one yell per unit per tick. Priority: death > player_killed > phase > low_hp > ability (blast) > aggro > evade > talk > periodic > ambient. Any emitted yell restarts the periodic timer.
- Lines cycle round-robin per unit and trigger (per rule for ability lines); the combat rng is never consumed, so yells cannot change hit/miss/crit outcomes.

### Player spell yells

`SpellDef.yells: Vec<String>` (RON, serde default = silent). When the player's spell is released (`CastSucceeded`) the player emits `Event::Yell { unit: player, text }` right after `CastSucceeded`. Not emitted for the Nitpick auto attack, failed or interrupted casts, or enemy casts.

- Lines cycle round-robin per spell; the rng is never consumed.
- `config.player_yell_cooldown_ms` (7000) is a global gate between player spell yells; the first eligible cast yells, and a gated cast neither yells nor advances its cursor.

## Regeneration

`config.regen: (interval_ms, hp_pct, mana_pct)`: alive units out of combat regain `hp_pct`/`mana_pct` of max every `interval_ms`, first tick one interval after leaving combat (or spawn/respawn). No event; observe `UnitState.hp/mana`. In combat only `mana_regen_per_5s_combat` trickles. See FORMULAS.md.
