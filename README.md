# World of Workcraft

A satirical single-player 3D browser RPG with WotLK-style tab-target combat, set in a corporate office. It is an original game: no Blizzard assets, code, text or data.

Status: **Phase 0 + Phase 1** (workspace, builds, office arena, movement, camera, targeting). Combat comes in Phase 2. See [docs/PLAN.md](docs/PLAN.md) and [docs/KNOWN_LIMITATIONS.md](docs/KNOWN_LIMITATIONS.md).

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
| Escape | Clear target, or toggle the pause overlay |

## Layout

```text
crates/combat_core/   deterministic rules: 30 Hz integer tick clock, targeting geometry (range, facing, LoS, Tab). No engine dependency.
crates/game_client/   Bevy 0.19 client: arena, player movement (FixedUpdate, interpolated), camera, targeting, HUD
web/                  index.html shell (canvas, input capture, error display); build output in web/pkg (gitignored)
scripts/              check.sh, build-web.sh, serve-web.sh
docs/                 plan, version choices, known limitations
```

## Versions and browser constraints

- Rust 1.96.1, Bevy 0.19.1 (features `3d`, `ui`), wasm-bindgen 0.2.129. Rationale in [docs/PLAN.md](docs/PLAN.md).
- The web build targets **WebGL2**. WebGPU is not enabled. On WebGL2, Bevy disables SSAO, OIT, atmosphere and GPU clustering (logged as warnings at startup; the game uses none of them). Directional shadows use 1 cascade.
- The release wasm is about 37 MB without `wasm-opt`.

## Verification status (2026-10-12)

Verified:
- `./scripts/check.sh` exit 0: fmt, clippy `-D warnings` (native and wasm32), 25 tests (23 `combat_core`, 2 `game_client` collision), native dev build.
- `./scripts/build-web.sh` exit 0.
- Native launch on macOS (M2 Pro, Metal): runs with no panics or errors in the log.
- Browser (Playwright Chromium, WebGL2 via ANGLE/Metal), driven by real keyboard and mouse input:
  - The page loads and renders the arena and HUD.
  - Tab selects the AI Evangelist and shows distance and the range/facing/LoS chips; turning away flips the chip to "Not facing".
  - W moves the character, and it stops on key release.
  - Right-drag steers the camera and character.
  - Escape clears the target, Space jumps and the wheel zooms.
  - The page does not scroll and the canvas keeps focus.

Not verified:
- Manual playtest of movement and camera feel.
- Pointer lock in a real headed browser (the automated browser rejects it; the page now logs that and continues).
- Firefox and Safari.
- FPS and memory profiling.
