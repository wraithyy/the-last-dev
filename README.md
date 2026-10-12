# World of Workcraft

A satirical single-player 3D browser RPG with WotLK-style tab-target combat, set in a corporate office. It is an original game: no Blizzard assets, code, text or data.

Play it: https://wraithyy.github.io/world-of-workcraft/

Status: **Phases 0-4** are playable: office arena with animated CC0 models, WoW-style movement and camera, tab-target combat with four spells, the AI Evangelist encounter (aggro, melee, interruptible Hallucination Blast, evade, death, loot, respawn) and a WoW-style HUD. See [docs/PLAN.md](docs/PLAN.md), [docs/COMBAT_API.md](docs/COMBAT_API.md), [docs/FORMULAS.md](docs/FORMULAS.md) and [docs/KNOWN_LIMITATIONS.md](docs/KNOWN_LIMITATIONS.md).

## Requirements

- `rustup` (the toolchain 1.96.1 and the `wasm32-unknown-unknown` target install automatically from `rust-toolchain.toml`)
- Browser build: `cargo install wasm-bindgen-cli --version 0.2.129 --locked` (must match `wasm-bindgen` in `Cargo.lock`)
- Optional: `wasm-opt` (binaryen) shrinks the wasm; the build script uses it when present
- `python3` for the local web server
- Disk: about 5 GB for `target/` (native dev + wasm release)

## Commands

| Task | Command |
|---|---|
| Run natively | `cargo run -p game_client` |
| All checks (fmt, clippy native + wasm, tests, build) | `./scripts/check.sh` |
| Tests only | `cargo test --workspace` |
| Browser build (release) | `./scripts/build-web.sh` (`--dev` for a faster debug build) |
| Serve the browser build | `./scripts/serve-web.sh`, then open http://localhost:8080 |

## Controls

| Input | Action |
|---|---|
| W / S | Run forward / backpedal (slower) |
| A / D | Turn (strafe while right mouse is held) |
| Q / E | Strafe |
| Right mouse drag | Steer camera and character |
| Left mouse drag | Orbit camera only |
| Both mouse buttons | Run forward |
| Space | Jump |
| Mouse wheel | Zoom |
| Tab | Cycle nearby hostile targets (front arc first) |
| Left click | Select unit |
| 1-4 | Code Review, Refactor, Guardrails, Reject PR (or click the action bar) |
| F / right-click corpse | Loot |
| F3 | Debug overlay (range, facing, LoS) |
| Escape | Clear target, or toggle the pause overlay |

Rotation: Code Review applies Code Smell, and Refactor consumes it for +35% damage. Use Guardrails before Hallucination Blast lands, or interrupt it with Reject PR.

## Layout

```text
crates/combat_core/   deterministic rules (no engine dependency): 30 Hz integer ticks, spells, GCD, casts, spell queue, auras, absorbs, interrupts, threat, enemy AI, loot, seeded RNG; golden replay test
crates/game_client/   Bevy 0.19 client: arena, models + animation, movement, camera, combat bridge, HUD, VFX, floating text
content/              game.ron (all balance data), replays/ (golden event logs)
assets/               CC0 models, icons, UI shader; see assets/CREDITS.md
web/                  index.html shell (canvas, input capture, error display); build output in web/pkg (gitignored)
scripts/              check.sh, build-web.sh, serve-web.sh
docs/                 plan, version choices, known limitations
```

## Versions and browser constraints

- Rust 1.96.1, Bevy 0.19.1 (features `3d`, `ui`), wasm-bindgen 0.2.129. Rationale in [docs/PLAN.md](docs/PLAN.md).
- The web build targets **WebGL2**. WebGPU is not enabled. On WebGL2, Bevy disables SSAO, OIT, atmosphere and GPU clustering (logged as warnings at startup; the game uses none of them). Directional shadows use 1 cascade.
- The release wasm is about 38 MB without `wasm-opt`, plus about 11 MB of assets.

## Debug hooks (native, env-gated)

`WOW_SCREENSHOT=path.png` (with `WOW_SHOT_DELAY`) saves an offscreen shot and exits, `WOW_PLAYER_AT=x,z,yaw_deg` teleports the player, `WOW_AUTORUN=1` holds forward, and `WOW_AUTOFIGHT=1` targets the enemy, casts on a timer and logs sim events.

## Credits

3D models by Kay Lousberg (KayKit, CC0). Spell icons from game-icons.net by Lorc and Skoll (CC BY 3.0). Full list: [assets/CREDITS.md](assets/CREDITS.md).

## Verification status (2026-10-12)

Verified:
- `./scripts/check.sh` exit 0: fmt, clippy `-D warnings` (native and wasm32), 68 tests (66 `combat_core` including the seed-42 golden replay and 30 Hz invariance, 2 `game_client`).
- `./scripts/build-web.sh` exit 0.
- Browser (Playwright Chromium, WebGL2 via ANGLE/Metal), driven by real keyboard and mouse input, played a full fight:
  - Tab targets the enemy. Code Review hits for 79 and applies Code Smell; Refactor hits for 151 with "Combo!".
  - Guardrails absorbs melee, and Reject PR interrupts Hallucination Blast.
  - The enemy dies, the player gains 120 XP and the target frame shows "Dead - lootable".
  - No 404s and no console errors.
- Earlier Phase 1 checks still hold: movement, right-drag steering, jump, zoom, no page scroll.

Not verified:
- Manual playtest of game feel (movement, camera, combat pacing).
- The loot popup with an actual drop (the tested run rolled no Prompt Fragment), and the respawn timer in the browser. Both are covered by the replay test only.
- Pointer lock in a real headed browser, Firefox and Safari.
- FPS and memory profiling.
