# Known limitations

Status after Phases 0-4. Remove an item once a test or verification covers it.

## Not implemented yet (planned)

- Phase 5 polish: profiling, wasm size reduction (no `wasm-opt` yet), settings menu.
- Escape opens a minimal pause overlay only, with no real menu.

## Presentation

- Spell damage text and hit reactions appear at `CastSucceeded`, not when the projectile VFX lands.
- `XP_BAR_MAX` (400) is a client constant, not content data.
- Chairs and small props have no colliders; the arena has no ceiling; picture frames are blank.
- The floor texture has no mipmaps, so it shimmers at grazing angles.
- Hit reaction plays a full-body clip, so it briefly overrides locomotion.

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

- Spell hit is 100%: no miss, resist, dodge or armor; enemy melee has no crit.
- Enemies chase in a straight line on the ground plane and ignore blockers for pathing (they only use LoS to aggro and to start casts).
- Heals and healing threat do not exist; threat is damage only.
- Pushback applies to the player only; enemy casts keep a fixed 2.5 s telegraph.
- Spell queue: one slot, static errors (cooldown, mana, range) are reported at press time instead of being deferred; a spell whose own cooldown ends inside the window is not queued.
- Only Damage, Absorb and Interrupt effect types exist; the aura model is "one aura per key per unit" (refresh replaces).
- `Command::Loot` failures (too far, not lootable) are silent: `CastError` has no loot variants. Corpses are always lootable, possibly with an empty loot list.
- The player does not auto-attack; only the enemy has a melee timer.
- Evade speed equals chase speed, and the leash is measured from the spawn point only.
