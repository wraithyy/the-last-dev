# Performance baseline

Repeatable benchmark for the client. Later optimisation rounds append "after" columns
to the tables below; always re-run with the same commands.

## Method

Hook: `WOW_BENCH=<lake|plaza|campus|campus_occupied|carpark|raid|swarm|ilmari3_halluc|ilmari3_collapse|ilmari4_autoscaler|ilmari4_outage|ilmari5_exercise|ilmari6_boardroom|ilmari_core|tower1>` (`crates/game_client/src/debug.rs`).
It starts in the scenario's zone and spot, orbits the camera around a fixed centre (one turn per
40 s), waits `WOW_BENCH_WARMUP` s (default 5), records `WOW_BENCH_FRAMES` frames (default 600) of
`Time::delta`, logs one `BENCH ...` line and exits. Native runs use `PresentMode::Immediate`, a
continuous winit loop (an unfocused window is otherwise throttled to 60 Hz) and a 1600x900
logical window. `WOW_BENCH_SCALE=2` emulates a retina display.

Scenarios: `lake` Wellness Park lake (campus), `plaza` HQ Plaza (campus), `campus` whole-campus
view from (0,140,240), `carpark` Car Park P3 start, `raid` `vibecon/1`, `tower1` `tower/1`.

```sh
cargo build --release -p game_client
for s in lake plaza campus carpark raid tower1; do
  WOW_MUTE=1 WOW_CLASS=fullstack BEVY_ASSET_ROOT=$PWD WOW_BENCH=$s \
    ./target/release/game_client 2>&1 | grep 'BENCH '
done
```

Run it three times and take the median per scenario; the numbers are only comparable on a quiet
machine (see Caveats).

Web: `./scripts/build-web.sh`, serve `web/` (e.g. `./target/release/the-last-dev-server --web-dir
web --bind 127.0.0.1:<free port>`), open `/?mute=1&bench=<name>` and read the `BENCH` console line.

## Machine

MacBook Pro `Mac14,9`, Apple M2 Pro (16-core GPU), 16 GB, macOS 27.0.1, rustc 1.96.1.
Benchmark window ran at scale factor 1.0 (external monitor).

## Settings under test

- Quality default on desktop: `High` (`Settings::defaults`, Low on mobile): shadows on, 2048 map, bloom on, decor range 1.0.
- MSAA: never configured anywhere in the client, so Bevy's default `Msaa::Sample4` is ON.
- No render scale; the window renders at the display scale factor.

## Native baseline (release, 1600x900 @1x, median of 3 runs)

| scenario | avg ms | p50 | p95 | p99 | max | fps | entities | meshes visible/total | point lights vis/total |
|---|---|---|---|---|---|---|---|---|---|
| lake    | 5.80 | 5.76 | 6.93  | 8.00  | 8.6  | 172 | 17809 | 680/7321  | 0/0 |
| plaza   | 6.35 | 6.07 | 8.43  | 10.07 | 12.6 | 157 | 17814 | 656/7321  | 0/0 |
| campus  | 6.98 | 6.17 | 10.90 | 13.40 | 14.6 | 143 | 17814 | 1891/7321 | 0/0 |
| carpark | 5.73 | 4.50 | 8.09  | 12.05 | 51.0 | 175 | 4215  | 472/1065  | 0/0 |
| raid    | 4.69 | 4.29 | 6.98  | 9.50  | 13.4 | 213 | 4152  | 485/1397  | 4/9 |
| tower1  | 4.28 | 4.17 | 5.17  | 5.80  | 11.0 | 234 | 4370  | 504/1202  | 3/10 |

Campus rework (520 x 360 m, Services Street, wayposts; same machine, window at 2x scale, median avg ms of
3 runs): campus 5.43 -> 5.56, plaza 5.62 -> 5.63; entities 18,507 -> 19,215.

Campus wave 1b (core spread: Lab, Data Center and annex moved out, two squares; same window at 2x, interleaved
before/after runs on a noisy machine): campus p50 5.14 -> 5.07-5.26 ms (avg within the +-1 ms run noise), plaza p50
5.4-5.8 -> 5.3-6.2; entities 19,215 -> 19,577, visible meshes +2 percent.

Single run at `WOW_BENCH_SCALE=2` (1600x900 logical = 3200x1800 physical) on a loaded machine,
for orientation only: avg ms lake 9.4, plaza 9.1, campus 7.8, carpark 5.9, raid 5.7, tower1 7.3.
Re-measure on a quiet machine before drawing a retina conclusion.

## Web baseline

Build: `scripts/build-web.sh` uses profile `wasm-release` (release + `opt-level = "s"`, strip,
thin LTO). `wasm-opt` is NOT installed on this machine, so the step was skipped; sizes are
pre-`wasm-opt` (CI may be smaller).

| item | size |
|---|---|
| `web/pkg/game_client_bg.wasm` raw | 48,424,306 B (46.2 MiB) |
| same, `gzip -9` | 11,514,180 B (11.0 MiB) |
| `web/assets` | 31 MiB (31,868 KiB on disk) |
| `web/pkg` | 46 MiB |
| whole `web/` (480 files, incl. service-worker precache list) | 77.5 MiB on disk |

Chrome (ANGLE/Metal on the M2 Pro, window 3432x1259 @1x, rAF-limited): the hook works, but a
fresh web save always starts in Car Park P3 (zone override is not honoured there), so the one
valid line is a carpark-like scene: `avg 16.28 ms, p95 18.1, p99 20.1, 61 fps, 4206 entities`.
rAF caps at the display rate, so only values above 16.7 ms are informative. The web build runs
without GPU clustering, GPU preprocessing, SSAO, OIT and atmosphere (measured on the old WebGL2 build; the WebGPU build has them available).

## Caveats

- The machine was heavily loaded during the first run (load average ~18); run 1 numbers are
  noticeably worse than runs 2-3. Compare medians, and check `uptime` first.
- Frame times are CPU+GPU wall time with no vsync; GPU time alone is not available
  (`RenderDiagnosticsPlugin` timestamps read 0 on Metal here).
- `point_lights` counts only `PointLight` entities; the campus has none, so its cost is shadowless.
- Hostile NPCs are live during the run; combat can add noise in `carpark` (max 51 ms).

## After the 2026-10-21 round

Native release, M2 Pro, quiet machine (load ~6), median of 2 runs, measured by the scene agent with
all four packages applied. Average frame time in ms:

| scenario | baseline | after |
|---|---|---|
| campus | 6.98 | 4.4 |
| plaza | 6.35 | 4.55 |
| raid | 4.69 | 3.7 |
| tower1 | 4.28 | 3.8 |

Visible meshes: campus 1865 -> ~1180, plaza 600-900 -> ~530. Web wasm: 48.4 MB raw / 11.5 MB gzip
-> 37.6 MB raw / 12.0 MB gzip (thin LTO, panic=abort, wasm-opt -Oz; wasm-opt trades ~0.5 MB gzip
for a 22% smaller module). First install precaches ~12 MB instead of ~43 MB.

Caveat: macOS throttles an occluded window to 60 Hz even with `PresentMode::Immediate`; keep the
bench window visible or every scenario reads 16.67 ms.

## Swarm drones (2026-10-23, docs/ILMARI_10.md C.7)

`WOW_BENCH=swarm`: 32 swarm clusters of 75 drones (2,400 claimed) around the player, five drafts,
staged in the real Scrape Pit (`ilmari3/1`; `swarm_fx::stage_swarm`, `WOW_SWARM=<n>` stages the
same for screenshots). Native release, M2 Pro, one run each, 1600x900 @1x, `WOW_QUALITY` set per
row. The BENCH line gains `drones_drawn`, `drones_claimed` and `clusters`.

| quality | drawn | avg ms | p95 | p99 | max | entities |
|---|---|---|---|---|---|---|
| Low | 576 | 4.52 | 5.30 | 5.57 | 6.29 | 5,943 |
| Medium | 1,184 | 4.60 | 5.47 | 6.49 | 8.12 | 6,551 |
| High | 2,400 | 4.71 | 5.36 | 6.53 | 14.25 | 7,767 |
| Ultra | 2,400 | 4.49 | 5.20 | 5.81 | 8.95 | 7,767 |

High meets the avg (<= 8 ms), p99 (<= 16.7 ms) and entity (<= 8,000) budgets in the real pit: the
room itself is about 5,370 entities, so 7,767 with the full 2,400 drones. Ultra draws 2,400 because
only 2,400 are claimed. The web (WebGPU) number is not measured yet.

`WOW_BENCH=ilmari3_halluc` (a hazard cycle every 9 s) and `ilmari3_collapse` (echo ghosts every
3 s), High: avg 4.38 ms / p99 6.26 ms / 3,324 entities and avg 4.06 ms / p99 4.76 ms / 3,508 entities.

## Ilmari Wing 4 (2026-10-23, docs/ILMARI_10.md C.14)

`WOW_BENCH=ilmari4_autoscaler` (the Autoscale Floor with the bill HUD live at 620 points and the boss's replica out; `wing4_fx::stage_qa`) and `ilmari4_outage` (the Server Cloud: hub, five regions, bridges and the platform decals mid-tour). Native release, M2 Pro, High, one run each, 1600x900 @1x, 600 frames.

| scenario | avg ms | p95 | p99 | max | entities | meshes visible |
|---|---|---|---|---|---|---|
| ilmari4_autoscaler | 4.46 | 5.04 | 5.99 | 11.24 | 3,934 | 718 |
| ilmari4_outage | 4.28 | 4.72 | 5.06 | 5.85 | 4,231 | 751 |

Both sit within 1 ms of the Ilmari Wing 3 scenarios above (4.06-4.38 ms avg) and well inside the budgets (avg <= 8 ms, p99 <= 16.7 ms, entities <= 8,000). The web (WebGPU) number is not measured yet.

## Ilmari 1.0 finale client (2026-10-24, docs/ILMARI_1_0.md D17)

`WOW_BENCH=ilmari5_exercise` (the Red-Blue Range with the squads out, the sensors' cones on the floor and the standby banner; `ilmari_fx::stage_qa` pulls the boss), `ilmari6_boardroom` (the CIO pulled: desks, tile covers, envelopes, HUD) and `ilmari_core` (six monitor glows, the wall panel, Ill Marie, the fight in P1). Native release, M2 Pro, High, one run each, 1600x900 @1x, 600 frames. The D.13 rows `ilmari5_rotunda` / `ilmari5_range` are covered by `ilmari5_exercise` (the rotunda is Wing 5's room 2, not benched on its own); `ilmari_core_p3` / `_p4`, `credits` are not benched yet (P3 and P4 need the fight to run for minutes; `WOW_SIM_SPEED` and `WOW_ILMARI_PCT` exist for it).

| scenario | avg ms | p95 | p99 | max | entities | meshes visible |
|---|---|---|---|---|---|---|
| ilmari5_exercise | 4.60 | 5.53 | 6.12 | 7.70 | 4,809 | 65 |
| ilmari6_boardroom | 4.71 | 5.77 | 6.51 | 6.96 | 3,984 | 70 |
| ilmari_core | 4.44 | 5.39 | 6.19 | 8.30 | 4,522 | 80 |

All three are inside the D.13 budgets (avg <= 8 ms, p99 <= 16.7 ms) and within 1 ms of the Wing 4 scenarios; no point lights. The `ilmari5_exercise` entity count is above the doc's 4,000 for the core rows because the room's racks and sensors are baked pieces; the web (WebGPU) number is not measured yet.

## 1.0 pass (2026-10-25, docs/ILMARI_1_0.md D21)

New benches (`debug.rs`): `ilmari_core_p3` and `ilmari_core_p4` stage the core with the boss held under 65 and 15 percent HP
(`ilmari_fx::stage_qa`), and `credits` runs the credits roll over the core (`WOW_CREDITS=3` starts it inside the warmup; the
roll is static text nodes moved by `top`, so nothing is re-laid out per frame). Native release, M2 Pro, High, one run each,
1600x900, 600 frames, load ~3.

| scenario | avg ms | p95 | p99 | max | entities | meshes visible |
|---|---|---|---|---|---|---|
| ilmari_core_p3 | 4.66 | 5.56 | 6.20 | 8.96 | 4,613 | 78 |
| ilmari_core_p4 | 4.37 | 4.80 | 5.15 | 6.80 | 4,618 | 78 |
| credits (1x) | 4.28 | 4.96 | 5.31 | 5.61 | 4,597 | 78 |
| credits (`WOW_BENCH_SCALE=2`, 3200x1800) | 4.50 | 5.31 | 5.86 | 6.38 | 4,597 | 78 |

Frame times meet every D.13 budget (avg <= 8 ms, credits <= 6 ms). Entities are above the planning figure of 4,000 for P3 and
P4 (the room itself is about 4,500 with its baked pieces, as in `ilmari_core`); no frame-time cost, so left as is. Not benched:
a five-player Judgment fight (a bench is one offline client; the fights are covered by the bot gates) and the web (WebGPU)
frame times and memory.

Web build (`scripts/build-web.sh`, `wasm-release`, wasm-opt `-Oz` present): `game_client_bg.wasm` 39,177,617 B (37.4 MiB), gzip -9
12,419,901 B (11.8 MiB); `web/pkg` 38 MiB, `web/assets` 52 MiB, whole `web/` 91 MiB in 589 files; the service worker precaches
only the shell and keys its cache on the build id, so installed copies update on every build. Load time on the web was not measured.
