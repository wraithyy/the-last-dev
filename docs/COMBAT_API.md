# Combat API contract

Shared contract between `combat_core` (rules) and `game_client` (presentation). Change it only together with every consumer.

## combat_core (pure Rust, deterministic, 30 Hz integer ticks)

Content is data-driven: `content/game.ron` (spells, units, balance), parsed by `combat_core::Content::from_ron(&str)`. The client embeds it with `include_str!`, which also works on wasm.

```rust
pub struct SpellId(pub u32);              // Copy, Eq, Hash, Debug
pub enum School { Physical, Arcane }

pub struct Content { /* spells, units, config */ }
impl Content {
    pub fn from_ron(src: &str) -> Result<Content, ContentError>;
    pub fn spell(&self, id: SpellId) -> Option<&SpellDef>;   // SpellDef: id, name, icon (asset key), school, range_m, cast_ms, mana, gcd: bool, cooldown_ms, description
    pub fn action_bar(&self) -> &[SpellId];                  // player's keys 1..=4 in order
}

pub struct Sim { /* ... */ }
impl Sim {
    pub fn new(content: Content, seed: u64) -> Sim;
    pub fn set_blockers(&mut self, blockers: Vec<Aabb>);
    pub fn spawn_player(&mut self, pos: Vec3, yaw: f32) -> EntityId;
    pub fn spawn_unit(&mut self, unit_key: &str, pos: Vec3, yaw: f32) -> Result<EntityId, ContentError>; // "ai_evangelist", "intern"
    pub fn push(&mut self, cmd: Command);    // applied at the start of the next tick
    pub fn tick(&mut self) -> Vec<Event>;    // advance exactly one 1/30 s tick
    pub fn now(&self) -> Tick;
    pub fn player(&self) -> Option<EntityId>;
    pub fn unit(&self, id: EntityId) -> Option<&UnitState>;
    pub fn units(&self) -> impl Iterator<Item = &UnitState>;
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
    RespawnPlayer,
}

pub struct UnitState {
    pub id: EntityId, pub key: String, pub name: String, pub level: u8, pub hostile: bool,
    pub pos: Vec3, pub yaw: f32,                     // the sim owns positions of non-player units
    pub hp: u32, pub max_hp: u32, pub mana: u32, pub max_mana: u32,
    pub alive: bool, pub in_combat: bool, pub target: Option<EntityId>,
    pub cast: Option<CastView>,                      // CastView { spell, elapsed_ms, total_ms, interruptible }
    pub auras: Vec<AuraView>,                        // AuraView { key, name, icon, harmful, remaining_ms, total_ms, absorb_remaining: Option<u32> }
    pub ai: Option<AiState>,                         // Idle | Chase | Attack | Casting | Evade | Dead
    pub lootable: bool, pub respawn_in_ms: Option<u32>, pub xp: u32,
}

pub enum CastError { NoTarget, InvalidTarget, Dead, OutOfRange, NotFacing, NoLineOfSight, NotEnoughMana, OnCooldown, OnGcd, AlreadyCasting, SchoolLocked, Moving, NotInterruptible, NothingToInterrupt }
// CastError::message() -> &'static str gives the WoW-style UI error text ("Out of range.", "Not enough mana.", ...)

pub enum Event {
    CastStarted { caster: EntityId, spell: SpellId, total_ms: u32 },
    CastSucceeded { caster: EntityId, spell: SpellId, target: Option<EntityId> },
    CastFailed { caster: EntityId, spell: SpellId, reason: CastError },
    CastInterrupted { caster: EntityId, spell: SpellId, by: Option<EntityId>, lockout_ms: u32 },
    CastQueued { caster: EntityId, spell: SpellId },
    Damage { source: EntityId, target: EntityId, spell: Option<SpellId>, amount: u32, absorbed: u32, crit: bool, combo: bool, school: School },
    MeleeSwing { attacker: EntityId, target: EntityId },
    AuraApplied { target: EntityId, key: String },
    AuraRemoved { target: EntityId, key: String, reason: AuraEnd },   // Expired | Consumed | Depleted | Death
    EnterCombat { unit: EntityId }, LeaveCombat { unit: EntityId },
    AiState { unit: EntityId, state: AiState },
    Death { unit: EntityId, killer: Option<EntityId> },
    XpGained { unit: EntityId, amount: u32 },
    LootDropped { corpse: EntityId, items: Vec<String> },
    Looted { looter: EntityId, corpse: EntityId, items: Vec<String> },
    Respawned { unit: EntityId },
}
```

`EntityId` here is a sim id (u64), not a Bevy `Entity`. The client keeps a mapping through the `SimUnit(EntityId)` component.

## game_client: combat to animation contract

`crates/game_client/src/combat_anim.rs` defines `CombatAnim`. The combat bridge writes it on the player and unit entities; the animation system reads it. Locomotion (idle, walk, run, backpedal, jump) is derived by the animation system itself from transform movement.

## Implemented semantics (combat_core)

Additions to the signatures above (no names removed or renamed):

- `SpellDef` also carries `effect: Effect` (`Damage` / `Absorb` / `Interrupt`) and `interruptible: bool` (default true). `Content`, `Config`, `UnitDef` and friends have public fields; `Content` also offers `unit_def`, `aura`, `gcd_ms`, `cast_ms`.
- Spell 5 "Hallucination Blast" is enemy-only and not on the action bar.
- `UnitState.xp`: for hostile units the XP awarded on kill, for the player the accumulated XP. `Event::XpGained.unit` is the player who gained it.
- Event timing: instant spells emit only `CastSucceeded` (no `CastStarted`); cast-time spells emit `CastStarted` then `CastSucceeded` on completion. Mana is spent and the spell cooldown starts on success; the GCD starts on cast start. `CastSucceeded` precedes its `Damage`/aura events; a consumed aura is reported before the `Damage` it boosts.
- `CastInterrupted { by: None, lockout_ms: 0 }` is used for movement/jump cancel, `StopCasting` and evade resets; `by: Some` carries the school lockout.
- Pushback has no event: the `CastView.total_ms` of the unit grows (by `pushback_ms`, at most `pushback_max` times). Player only.
- Off-GCD instants (`gcd: false`, `cast_ms: 0`, i.e. Reject PR) are usable during the GCD and during an own cast without cancelling it.
- `CooldownView.usable` is the complete verdict now (target, range, facing, LoS, mana, GCD/cast busy), so the HUD can grey buttons from it. `gcd_*` fields are zero for off-GCD spells.
- A corpse is always `lootable` after death; `LootDropped.items` / `Looted.items` may be empty. `Command::Loot` outside `loot_range_m` or on a non-corpse is silently ignored.
- Hostile units die with `ai: Some(AiState::Dead)`; the player has `ai: None`. `Command::RespawnPlayer` on a living player is ignored.
- Damage to a unit in `AiState::Evade` is dropped entirely (no `Damage` event, no threat, no aura).
