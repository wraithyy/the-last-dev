# Known limitations

Status after Phases 0-4. Remove an item once a test or verification covers it.

## Not implemented yet (planned)

- Phase 5 polish: profiling, wasm size reduction (no `wasm-opt` yet), settings menu.
- Escape opens a minimal pause overlay only, with no real menu.

## Presentation

- Floating numbers and hit reactions of projectile attacks wait for the projectile to land (1.5 s fallback). Health bars and the combat log stay sim-authoritative, so a log line and the HP drop can precede the visual by the flight time.
- Chair colliders float 0.12 m above the floor so the shared blocker list (movement, camera, combat LoS) does not stop ground-level LoS rays. A dedicated movement-only list would be cleaner but needs `player.rs`/`combat.rs` changes.
- The ceiling is not a collider; the camera is clamped below it by height (`CAM_MAX_HEIGHT`).

- The cast pose is the one-handed shoot clip held on its last frame, and backpedal plays the walk clip reversed. The hand glow position is tuned by eye, not bound to the fist bone.
- The furniture kit has no server rack, whiteboard or water cooler; those are flat-colour primitives in `arena.rs`.

## Simplifications

- Collision uses axis-aligned boxes only. The player is a box/cylinder approximation, with no slopes and no rotated props.
- Camera collision uses `combat_core::line_of_sight` plus bisection against the same boxes. It does not collide with non-solid decor.
- Click-picking uses a ray-vs-sphere test per unit, not mesh-accurate picking.
- The player can steer while airborne (WoW allows only limited air control).
- In RMB-steer mode the camera follows the interpolated character yaw, so it can lag the mouse by up to one 33 ms tick.
- `FixedStepper` caps catch-up at 8 ticks per frame. Longer stalls drop the excess whole ticks.

## Not verified

- Movement and camera feel, and click-vs-drag accuracy, need a manual playtest (native and browser).
- Browser: pointer lock after a drag, context-menu/scroll suppression and WebGL2 shadow quality across browsers. See README "Verification status" for what was actually run.
- Only macOS (Apple Silicon, Metal) has been tried natively.
- No FPS or memory profiling yet (Phase 5).

## Combat

- Only a flat hit roll exists (95% white, 96% spell, no level difference): no dodge, parry, block, armor or partial resists; enemy melee has no crit. Misses generate no threat, so a missed first attack does not pull.
- Enemies chase in a straight line on the ground plane and ignore blockers for pathing (they only use LoS to aggro and to start casts).
- Heals and healing threat do not exist; threat is damage only. Out-of-combat regeneration is a fixed percent tick (no spirit or gear scaling), and the legacy in-combat mana trickle stays flat.
- Pushback applies to the player only; enemy casts keep a fixed 2.5 s telegraph.
- Spell queue: one slot, static errors (cooldown, mana, range) are reported at press time instead of being deferred; a spell whose own cooldown ends inside the window is not queued.
- Only Damage, Absorb and Interrupt effect types exist; the aura model is "one aura per key per unit" (refresh replaces).
- `Command::Loot` failures (too far, not lootable) are silent: `CastError` has no loot variants. Corpses are always lootable, possibly with an empty loot list.
- Auto attack is a single ranged "Nitpick" on the player; the swing timer is not hasted and there is no melee auto attack for the player.
- Evade speed equals chase speed, and the leash is measured from the spawn point only.
