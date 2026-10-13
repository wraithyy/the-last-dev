# The Last Dev

A satirical single-player 3D browser RPG with classic-MMO tab-target combat, set in a corporate office. It is an original game: no Blizzard assets, code, text or data.

Play it: https://wraithyy.github.io/the-last-dev/

## The satire

The whole game is a long, loving roast of corporations that push AI onto every task for no reason. You play a Senior Engineer who climbs a five-floor office tower to reach the **CEO** ("Chief AI Evangelist"). Every floor has trash packs, a boss and an exit elevator that unlocks once the boss is down; bosses and mini-bosses yell AI buzzwords in classic MMO boss-yell style (red chat line plus a speech bubble), friendly NPCs (the AI-Native Intern, a permanently ticket-blocked IT Support) chat in green. Your hair thins as your HP drops. When you die your role is backfilled by an AI agent. All text is original; no real companies or people are named.

## Features

- Five floors, each loaded in the sim and the client together: Open Space (1-2), Meeting Rooms (3-4), Sales & Marketing (5-6), PMO (7-8), C-Level Penthouse (9-10). Kill the floor boss, the exit elevator doors open and its lamp turns green, walk in: fade to black and the next floor's name banner. Level and XP carry over.
- Killing the CEO on floor 5 rolls the satirical credits; press R to start a new sprint (fresh sim, floor 1, level 1).
- Ten action-bar spells on keys 1-9, 0 (number row by physical key, so Czech and other layouts work; numpad too). Unlearned spells show a locked keycap with "Lv N"; level-ups play a golden ring, a "LEVEL UP" banner and an IDE-style "New spell" notification.
- Nameplates over every NPC within 40 m (red + HP bar for hostiles, green + title for friendlies, BOSS / ELITE tags); the target frame shows the boss phase ("PROD P2"), aura stacks and generated letter-badge icons for auras without art.
- Stuns freeze movement and casting (spinning stars, grey keycaps); silence and school lockouts lock keycaps with a padlock. Knockbacks shake the camera and push you (walls clamp it), teleports land on the nearest free spot.
- Loot is trophies: every drop is an item with a satirical description and a small permanent bonus (HP, mana, spell power, crit, haste, mana regen; up to 3 copies count). The loot window shows icon, bonus and description, the combat log says "You receive trophy: [..] (+1% crit)", and B opens `~/trophies` with every trophy, its count and the total bonus. Bonuses persist across floors and respawns; a new sprint (R) resets them.
- Floating numbers: heals in green, DoT ticks smaller, crits and executes pop.

See [docs/PLAN.md](docs/PLAN.md), [docs/GAME_DESIGN.md](docs/GAME_DESIGN.md), [docs/COMBAT_API.md](docs/COMBAT_API.md), [docs/FORMULAS.md](docs/FORMULAS.md) and [docs/KNOWN_LIMITATIONS.md](docs/KNOWN_LIMITATIONS.md).

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
| Right-click (no drag) on a hostile | Target it and start auto attack (Nitpick) |
| Right-click (no drag) on a friendly / F near one | Talk to the NPC (within 6 m) |
| T / Attack button | Toggle auto attack on the current target |
| 1-9, 0 (or numpad) | Action bar: Code Review, Refactor, Guardrails, Reject PR, git blame, Rollback, On-Call Caffeine, Rubber Duck, Postmortem, Write It By Hand (unlocked by level) |
| F / right-click corpse | Loot (F on a friendly target talks) |
| B | Toggle the `~/trophies` inventory window (B, `[x]` or Esc closes it) |
| R (victory screen) | Start a new sprint |
| F3 | Debug overlay (range, facing, LoS) |
| Escape | Close the trophies window, else clear target, else toggle the pause overlay |

Rotation: Code Review applies Code Smell, and Refactor consumes it for +35% damage. Interrupt the big casts with Reject PR (level 2), shield with Guardrails, keep git blame ticking, finish with Postmortem below 25%.

## Layout

```text
crates/combat_core/   deterministic rules (no engine dependency): 30 Hz integer ticks, spells, GCD, casts, spell queue, auras, absorbs, interrupts, threat, enemy AI, loot, seeded RNG; golden replay test
crates/game_client/   Bevy 0.19 client: floors (layouts, themes, elevators), unit catalog, models + animation, movement, camera, combat bridge, HUD, VFX, floating text
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

| Variable | Effect |
|---|---|
| `WOW_SCREENSHOT=path.png` (`WOW_SHOT_DELAY=s`) | Save an offscreen 1280x720 shot and exit |
| `WOW_FLOOR=1..5` | Start on that floor at its minimum level |
| `WOW_LEVEL=n` | Start level (overrides the floor minimum) |
| `WOW_PLAYER_AT=x,z,yaw_deg[,cam_deg]` | Start position after the first floor load; optional camera yaw offset (180 = front view) |
| `WOW_CAM_DIST=m` | Start zoom |
| `WOW_CLEARED=1` | Open the first floor's exit elevator without killing the boss |
| `WOW_AUTOFIGHT=1` | Bot: walks to the nearest hostile (adds first), interrupts, heals/shields, runs the rotation, loots corpses, respawns, takes the elevator; logs every sim event |
| `WOW_TALK=1` | Talk to the nearest friendly NPC two seconds in |
| `WOW_GIVE_ITEMS=1`, `WOW_TROPHIES=1` | Grant a sample set of trophies at start, open the `~/trophies` window at start |
| `WOW_HOVER_SLOT=0..9`, `WOW_AUTORUN=1`, `WOW_LINEUP=1\|2` | Force a tooltip, hold forward, show the cast lineup |

## Credits

3D models: characters by Quaternius (Ultimate Animated Character Pack, CC0); furniture by Kenney (Furniture Kit, CC0). Spell icons: Painterly Spell Icons by J. W. Bjerk (eleazzaar), CC BY 3.0. Item icons from game-icons.net by Lorc and Delapouite (CC BY 3.0). Full list: [assets/CREDITS.md](assets/CREDITS.md).

## Verification status (2026-10-13)

Verified:
- `./scripts/check.sh` exit 0: fmt, clippy `-D warnings` (native and wasm32), 153 tests (131 `combat_core`, 22 `game_client`).
- `./scripts/build-web.sh` exit 0 (not served in this pass).
- Native screenshots driven by the debug hooks (`.playwright-mcp/`): floor 1 start, a trash pull with nameplates, the floor 1 boss, level-up banner and spell toast, stun, the opened exit elevator, the floor 2 arrival banner, IT Support talk bubble, the CEO in phase 2, the victory screen, and a restart back to floor 1 at level 1.

Not verified:
- Manual playtest of game feel (movement, camera, combat pacing) and real mouse/keyboard input for talking and restarting.
- The browser build was not run in a browser in this pass.
- Pointer lock in a real headed browser, Firefox and Safari; FPS and memory profiling.
