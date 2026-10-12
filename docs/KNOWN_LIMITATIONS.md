# Known limitations

Status after Phase 0 + Phase 1. Remove an item once a test or verification covers it.

## Not implemented yet (planned)

- Combat simulation: spells, GCD, cooldowns, casting, mana, auras, shield, interrupts, spell queue (Phase 2). Keys 1-4 are intentionally unmapped, with no fake feedback.
- Enemy AI, threat, melee, Hallucination Blast, death, loot, respawn (Phase 3). The AI Evangelist is a static target dummy for now.
- Action bar, cast bars, floating combat text, combat log, VFX, animation (Phase 4).
- Escape opens a minimal pause overlay only, with no real menu.

## Simplifications

- Collision uses axis-aligned boxes only. The player is a box/cylinder approximation, with no slopes and no rotated props.
- Camera collision uses `combat_core::line_of_sight` plus bisection against the same boxes. It does not collide with non-solid decor.
- Click-picking uses a ray-vs-sphere test per unit, not mesh-accurate picking.
- The target chips in the HUD use a fixed 30 m preview range (`PREVIEW_SPELL_RANGE`) until real spells exist.
- The player can steer while airborne (WoW allows only limited air control).
- In RMB-steer mode the camera follows the interpolated character yaw, so it can lag the mouse by up to one 33 ms tick.
- `FixedStepper` caps catch-up at 8 ticks per frame. Longer stalls drop the excess whole ticks.

## Not verified

- Movement and camera feel, and click-vs-drag accuracy, need a manual playtest (native and browser).
- Browser: pointer lock after a drag, context-menu/scroll suppression and WebGL2 shadow quality across browsers. See README "Verification status" for what was actually run.
- Only macOS (Apple Silicon, Metal) has been tried natively.
- No FPS or memory profiling yet (Phase 5).
