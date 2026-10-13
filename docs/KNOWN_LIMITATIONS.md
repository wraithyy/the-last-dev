# Known limitations

Status after Phases 0-4 plus the playable five-floor tower (sim and client wired together). Remove an item once a test or verification covers it.

## Not implemented yet (planned)

- Phase 5 polish: profiling, wasm size reduction (no `wasm-opt` yet), settings menu.
- Escape opens a minimal pause overlay only, with no real menu.

## Presentation

- Floating numbers and hit reactions of projectile attacks wait for the projectile to land (1.5 s fallback). Health bars and the combat log stay sim-authoritative, so a log line and the HP drop can precede the visual by the flight time.
- Chair colliders float 0.12 m above the floor so the shared blocker list (movement, camera, combat LoS) does not stop ground-level LoS rays. A dedicated movement-only list would be cleaner but needs `player.rs`/`combat.rs` changes.
- The ceiling is not a collider; the camera is clamped below it by height (`CAM_MAX_HEIGHT`).

- The cast pose is the one-handed shoot clip held on its last frame, and backpedal plays the walk clip reversed. The hand glow position is tuned by eye, not bound to the fist bone.
- Nameplates and floating numbers are screen-space UI nodes: they are culled by distance (40 m) and camera direction, not occluded by walls, and can overlap in tight packs.
- Item loot keys without a dedicated file fall back to generated letter badges (see `hud::ICON_FILES`); every spell and aura has painted art.
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
- Balance is verified by a scripted bot (`tests/playthrough.rs`: three seeds, each floor at its max level) that kills adds first, kicks interruptible casts and uses Rollback/Guardrails; a human who ignores mechanics will find floors 2 and 5 hard. Guardrails' absorb (100) does not scale with level.
- Reject PR unlocks at level 2 and Guardrails at level 3 (swapped versus GAME_DESIGN.md) so the floor-1 boss interrupt is possible on floor 1.
- Mini-bosses (`hr_partner`, `marketing_lead`, `ciso`) are mandatory pack_6 pulls in the sim; nothing marks them optional except that the floor clears on the boss alone.
- `Command::Loot` failures (too far, not lootable) are silent: `CastError` has no loot variants. Corpses are always lootable, possibly with an empty loot list.
- Auto attack is a single ranged "Nitpick" on the player; the swing timer is not hasted and there is no melee auto attack for the player.
- Evade speed equals chase speed, and the leash is measured from the spawn point only.
