# Known limitations

Status after Phases 0-4 plus the zone graph (campus, tower, data center, lab) and quests, wired in sim and client. Remove an item once a test or verification covers it.

## Not implemented yet (planned)

- Phase 5 polish: profiling, wasm size reduction (no `wasm-opt` yet), settings menu.
- Escape opens a minimal pause overlay only, with no real menu.

## Presentation

- Floating numbers and hit reactions of projectile attacks wait for the projectile to land (1.5 s fallback). Health bars and the combat log stay sim-authoritative, so a log line and the HP drop can precede the visual by the flight time.
- Chair colliders float 0.12 m above the floor so the shared blocker list (movement, camera, combat LoS) does not stop ground-level LoS rays. A dedicated movement-only list would be cleaner but needs `player.rs`/`combat.rs` changes.
- The ceiling is not a collider; the camera is clamped below it by height (`CAM_MAX_HEIGHT`).

- The cast pose is the one-handed shoot clip held on its last frame, and backpedal plays the walk clip reversed. The hand glow position is tuned by eye, not bound to the fist bone.
- Nameplates and floating numbers are screen-space UI nodes: they are culled by distance (40 m) and camera direction, not occluded by walls, and can overlap in tight packs.
- Every spell, aura and trophy has a shipped icon (test `every_spell_aura_and_trophy_has_a_shipped_icon_file`); a key without a file would fall back to a generated letter badge.
- The exit elevator doors and lamp are client-only overlays in front of the static dressing; they are not colliders.
- Floor 1's sleeping IT Support comes from the catalog pose for that anchor (`catalog::npc_at`); the sim only knows one pose per unit kind. Mini-boss "ELITE" tags are a client list (`copy::is_elite`).
- DoT-tick numbers are detected by a heuristic (damage from a spell without a direct-damage effect); the "EXECUTE!" pop is judged from the target's HP after the hit.
- The player's hair stages and accessories (neck headphones, coffee mug) exist only on `Engineer.glb`; accessory offsets are tuned by eye in bone space.

## Simplifications

- Collision uses axis-aligned boxes only. The player is a box/cylinder approximation, with no slopes and no rotated props.
- Camera collision uses `combat_core::line_of_sight` plus bisection against the same boxes. It does not collide with non-solid decor.
- Click-picking uses a ray-vs-sphere test per unit, not mesh-accurate picking.
- The player can steer while airborne (WoW allows only limited air control).
- In RMB-steer mode the camera follows the interpolated character yaw, so it can lag the mouse by up to one 33 ms tick.
- `FixedStepper` caps catch-up at 8 ticks per frame. Longer stalls drop the excess whole ticks.

## Zones and quests

- `load_floor`, `Sim::floor()`, `Content::floor()` and the `Floor*` events are deprecated twins of the zone API; the client uses only the zone API.
- Doors are trigger boxes, not opening doors: walking into an unlocked one starts the fade, a locked one prints its reason (at most every 3 s). Arrival is in front of the door back to the zone you came from.
- The map draws the layout's blockers as a top-down raster; it has no fog of war, and the zone tree is a static list of the content's zones.
- Quest markers and the ticket dialog read `Sim::quests()` every frame; there is no quest sharing, no quest item highlighting on corpses and no waypoint arrow.
- Debug hooks set quest states (`WOW_QUESTS`) without giving the items or kills, so a forced `ready` ticket can be turned in for free.
- Trash respawns zone-wide: after `TRASH_RESPAWN_MS` away every killed pack unit comes back on re-entry; there is no per-unit timer and no respawn while the player stays in the zone. Respawned trash gives XP and loot again (quest items can be farmed this way, by design).
- Quest failures (too far from the giver, wrong status) are silent, like `Talk` and `Loot`. Abandon emits no event; the UI reads `Sim::quests()`.
- Quest state, trophies and world progress are per `Sim` (session only). Quest logs are keyed by player id, but commands act for the single player.
- Loot objectives count pickups while active plus copies carried at accept time; turned-in quest items stay in the inventory (they are ordinary trophies with small bonuses).
- `DrainMana` emits no event; the HUD sees the mana bar drop. Spell reflect (Alignment Training) also bounces DoT ticks of the player's spells.

## Debug bot

- `WOW_AUTOFIGHT` steers in a straight line with a sidestep when stuck (no pathfinding), and dies on floors above its level. It is a smoke-test aid, not balance evidence (see `tests/playthrough.rs` for that).
- There is no god mode (`WOW_GOD`); `WOW_LEVEL` is the way to survive a floor in tests.

## Not verified

- Movement and camera feel, and click-vs-drag accuracy, need a manual playtest (native and browser).
- Talking via right-click / F and the R restart were exercised only through debug hooks (`WOW_TALK`, a temporary auto-restart), not with real input.
- Browser: pointer lock after a drag, context-menu/scroll suppression and WebGL2 shadow quality across browsers. See README "Verification status" for what was actually run.
- Only macOS (Apple Silicon, Metal) has been tried natively.
- No FPS or memory profiling yet (Phase 5).

## Combat

- Only a flat hit roll exists (95% white, 96% spell, no level difference): no dodge, parry, block, armor or partial resists; enemy melee has no crit. Misses generate no threat, so a missed first attack does not pull.
- Enemies chase in a straight line on the ground plane and ignore blockers for pathing (they only use LoS to aggro and to start casts).
- Heals generate no threat; threat is damage only (plus 1 for a pulling debuff). Out-of-combat regeneration is a fixed percent tick (no spirit or gear scaling), and the in-combat mana trickle is flat per unit (does not grow with level).
- Pushback applies to the player only; enemy casts keep a fixed 2.5 s telegraph.
- Spell queue: one slot, static errors (cooldown, mana, range) are reported at press time instead of being deferred; a spell whose own cooldown ends inside the window is not queued.
- The effect set is closed (see COMBAT_API.md); there are no channels: "channel" abilities (Review Request) are ordinary interruptible casts. One aura per key per unit: auras from different casters with the same key share stacks and the newest caster's snapshot.
- Only two schools exist (Physical, Arcane). School-lock mechanics (Requirements Change, Block All The Things) therefore lock either every arcane spell or the two physical ones (Rubber Duck, Write It By Hand).
- `Lockout` without an active cast emits no event; the HUD sees it only through `CooldownView.usable == Err(SchoolLocked)`.
- Reprioritize (`ThreatSwap`) swaps the top two threat entries; solo, with the player as the only entry, it is a no-op apart from its yell.
- Pack formation offsets are world-axis XZ offsets, not rotated by the anchor yaw. Knockback and teleport do not check blockers: the client must clamp the player body against walls.
- The Reorg teleport lands 4 m in front of the VP's spawn point regardless of the room layout.
- Despawned units stay in the sim's unit vector (ids are never reused), so memory grows slowly with every floor load and summon.
- Bosses are immune to stuns only; silence, slows and knockback flags are not distinguished.
- Balance is verified by a scripted bot (`tests/playthrough.rs`: three seeds, each zone at its max level, plus the campus quest chain from level 1) that kills adds first, kicks interruptible casts and uses Rollback/Guardrails; a human who ignores mechanics will find floors 2 and 5 hard. Guardrails' absorb (100) does not scale with level.
- Reject PR unlocks at level 2 and Guardrails at level 3 (swapped versus GAME_DESIGN.md) so the floor-1 boss interrupt is possible on floor 1.
- Mini-bosses (`hr_partner`, `marketing_lead`, `ciso`) are mandatory pack_6 pulls in the sim; nothing marks them optional except that the floor clears on the boss alone.
- `Command::Loot` failures (too far, not lootable) are silent: `CastError` has no loot variants. Corpses are always lootable, possibly with an empty loot list.
- Auto attack is a single ranged "Nitpick" on the player; the swing timer is not hasted and there is no melee auto attack for the player.
- Evade speed equals chase speed, and the leash is measured from the spawn point only.

## Touch and PWA

- Web lobby text fields: a hidden HTML input is the text source. Keys typed in the same frame as the
  click that focuses a field can be lost (only reproducible with scripted input, not at human
  typing speed). Real DOM inputs overlaid on the Bevy fields would remove the race.

- Touch layout is verified only with Chrome touch emulation; real iOS Safari (fullscreen on Add to Home Screen, audio unlock, safe-area insets) is untested.
- A keycap tap casts on press, so a long press shows the tooltip while the spell has already fired.
- Touch taps on HUD panels without a `Button`/`Interaction` (windows, dialogs) can still reach target picking, same as mouse clicks.
- The service worker precaches everything; if the browser refuses the shell (low disk/quota) the install fails and the game keeps running online without offline support. Asset-only quota errors are logged and skipped.
