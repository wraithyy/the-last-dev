# Implementation plan and version choices

## Versions (chosen 2026-10-12)

| Component | Choice | Why |
|---|---|---|
| Rust | 1.96.1 (pinned in `rust-toolchain.toml`) | Already installed locally; satisfies Bevy 0.19 MSRV 1.95. |
| Bevy | 0.19.1, features `3d` + `ui` | Latest patch of the previous release. Bevy 0.20.0 was released 2026-10-08 and needs Rust 1.97.1; it is too fresh for the plugin ecosystem. Upgrade after Phase 5. |
| Renderer (web) | WebGPU only (`webgpu` feature, no WebGL2) | Decided after the perf round; browsers without WebGPU get a notice. |
| wasm-bindgen | 0.2.129 (CLI version must match `Cargo.lock`) | Required by `scripts/build-web.sh`. |
| Physics | none, custom AABB collision | Only a static office arena is needed; avoids a plugin compatibility risk (avian3d 0.7 targets a different Bevy). |
| Web shell | plain `web/index.html` + `python3 -m http.server` | No npm, trunk or React until a UI requirement justifies them. |

## Phases

- **Phase 0 (done in this run)**: workspace, toolchain pin, native and wasm builds, check scripts, web shell.
- **Phase 1 (done in this run)**: office arena, WoW-style movement and camera, jump, AABB collision, Tab and click targeting, geometry readout in the HUD (range, facing, LoS).
- **Phase 2**: `combat_core` sim: entity state, commands, events, spells, GCD, cooldowns, cast timers, spell queue, mana, auras, shield, interrupt and lockout, seeded RNG. Tests come first.
- **Phase 3**: CEO boss state machine, threat, leash, Hallucination Blast, death, loot, respawn, deterministic replay.
- **Phase 4**: action bar, cast bars, floating combat text, combat log, VFX, procedural animation.
- **Phase 5**: browser validation, profiling, documentation.

## Crate boundaries

- `combat_core`: deterministic rules, integer tick time (30 Hz) and targeting geometry. No engine dependency.
- `game_client`: Bevy rendering, input, camera, HUD. Converts input to commands and sim state to visuals.
