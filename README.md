# The Last Dev

A satirical single-player 3D browser RPG with classic-MMO tab-target combat, set in a corporate office. It is an original game: no Blizzard assets, code, text or data.

Play it: https://wraithyy.github.io/the-last-dev/ (landing page with the game embedded; the bare game is at https://wraithyy.github.io/the-last-dev/play/)

## The satire

The whole game is a long, loving roast of corporations that push AI onto every task for no reason. You pick a role (Fullstack Engineer, Frontend Developer, Backend Engineer or DevOps Engineer: ranged, melee, tank or healer), start on the corporate campus at level 1, closes Jira tickets for the people the AI forgot, climbs a five-floor office tower to reach the **CEO** ("Chief AI Evangelist"), clears the data center and finally walks into the Innovation Lab to shut down the AGI Prototype (it was a guy forwarding prompts the whole time). Every tower floor has trash packs, a boss and an exit elevator that unlocks once the boss is down; bosses and mini-bosses yell AI buzzwords in classic MMO boss-yell style (red chat line plus a speech bubble), friendly NPCs (the AI-Native Intern, a permanently ticket-blocked IT Support) chat in green. Your hair thins as your HP drops. When you die your role is backfilled by an AI agent. All text is original; no real companies or people are named.

## Features

- A zone graph loaded in the sim and the client together: the outdoor campus hub (1-2), the tower's five floors Open Space (1-2), Meeting Rooms (3-4), Sales & Marketing (5-6), PMO (7-8), C-Level Penthouse (9-10), the data center (6-8, opens after tower floor 3) and the Innovation Lab (10, opens after the CEO). Doors and elevators fade to black with a rotating loading tip; locked doors say what they need. Every building has a door back to the campus; the penthouse elevator goes down to it. Cleared bosses stay dead, trash returns after 5 minutes away, death respawns you at the zone's graveyard.
- Quests as Jira tickets: "!" / grey "?" / "?" over givers (gold for story quests, azure for side quests), F or right-click opens the ticket (Accept / Complete / Close), the "Sprint backlog" tracker on the right, progress toasts, a "Ticket closed" banner and sound, and the `~/jira` log on J (with Abandon). N opens the map: the current zone's outline with player, giver and boss dots plus the zone tree with locked / open / cleared state.
- The AGI Prototype's rack opens on death and shows who was inside; then the final retrospective with run stats. Press R to start a new sprint (fresh sim, campus, level 1).
- Ten action-bar spells on keys 1-9, 0 (number row by physical key, so Czech and other layouts work; numpad too). Unlearned spells show a locked keycap with "Lv N"; level-ups play a golden ring, a "LEVEL UP" banner and an IDE-style "New spell" notification.
- Nameplates over every NPC within 40 m (red + HP bar for hostiles, green + title for friendlies, BOSS / ELITE tags); the target frame shows the boss phase ("PROD P2"), aura stacks and generated letter-badge icons for auras without art.
- Stuns freeze movement and casting (spinning stars, grey keycaps); silence and school lockouts lock keycaps with a padlock. Knockbacks shake the camera and push you (walls clamp it), teleports land on the nearest free spot.
- Loot is trophies: every drop is an item with a satirical description and a small permanent bonus (HP, mana, spell power, crit, haste, mana regen; up to 3 copies count). The loot window shows icon, bonus and description, the combat log says "You receive trophy: [..] (+1% crit)", and B opens `~/trophies` with every trophy, its count and the total bonus. Bonuses persist across floors and respawns; a new sprint (R) resets them.
- Floating numbers: heals in green, DoT ticks smaller, crits and executes pop.
- The Ilmari campaign (level 10 and up, solo on Story or 2-5 players on Full and Judgment Day): six wings from the Alignment Lab to the Boardroom in the Sky, the Core with its mech and mainframe phases, an ending with credits, and a weekly ladder. Design in [docs/ILMARI_1_0.md](docs/ILMARI_1_0.md).
- Audio: office elevator-muzak on the floors that crossfades (1.5 s) to a boss loop while a boss fights, a faint air-con bed, keycap clacks, per-spell cast sounds, hit / miss / heal / stun sounds that fade with distance, level-up, loot and elevator cues and a victory jingle. Browsers start audio only after your first key press or click. M mutes; Esc shows master / music / effects volume. Credits in assets/CREDITS.md.

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
| Landing page (Astro, `site/`) | `cd site && pnpm install && pnpm dev`; Pages serves it at the root and the game under `play/` |

## Controls

Every key below is the default; **Esc menu > `~/keybindings`** rebinds any of them (click an action, press a key; a key already in use is swapped; Reset to defaults restores everything). Bindings are physical key positions, so Czech and other layouts keep working, and they are saved (a file next to the co-op settings on native, localStorage on the web). The action bar and the hint bar show your bindings.

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
| Left click | Select unit (teammates too) |
| F1 / F2-F5 / click a party frame | Target yourself / the other group members (online) |
| V / reboot button | Have You Tried Turning It Off and On Again: resurrect the targeted dead teammate (6 s cast, out of combat, online) |
| Right-click (no drag) on a hostile | Target it and start auto attack (Nitpick) |
| Right-click (no drag) on a friendly / F near one | Talk to the NPC (within 6 m) |
| T / Attack button | Toggle auto attack on the current target |
| C / Coffee button | Coffee Break: channel up to 6 s, +8% max mana per second, 8 s cooldown; moving, jumping, casting or taking damage ends it |
| 1-9, 0 (or numpad) | Action bar: Code Review, Refactor, Guardrails, Reject PR, git blame, Rollback, On-Call Caffeine, Rubber Duck, Postmortem, Write It By Hand (unlocked by level). Guardrails, Rollback and Caffeine land on a targeted teammate, otherwise on you |
| F / right-click corpse | Loot (F on a friendly target talks) |
| B | Toggle the `~/trophies` inventory window (B, `[x]` or Esc closes it) |
| J | Toggle the `~/jira` quest log (abandon tickets there) |
| N | Toggle the map (current zone + zone tree) |
| `=` / `-` (or the `+` / `-` buttons) | Zoom the `~/radar` minimap (top right: north up, you in the middle, group blue, hostiles within 40 m red, quest givers `!` / `?`, loot gold, doors `#`; click it to open the map) |
| Hover a character (long-press on touch) | Tooltip: name, level, title and your quest progress or the quest they give; bag and loot items say which ticket wants them |
| Hover an aura icon or the target's cast bar | IDE-style tooltip: name, effect, time left, stacks, dispel type, interruptible |
| Up / Down, Enter, Backspace (ticket dialog) | Pick a ticket, accept / complete it, back to the list |
| R (victory screen) | Start a new sprint |
| M | Mute / unmute all audio |
| F8 | Quest arrow on / off (off by default; also in `~/settings`): points at the tracked ticket's work, the turn-in, or the next door; pick the ticket with a tracker click or Guide in the quest log |
| F9 | Debug overlay (range, facing, LoS) |
| Escape | Close the window you opened last, else clear target, else toggle the pause overlay (also holds the master / music / effects volume keycaps) |

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

## Releasing

The game version lives once, in `[workspace.package] version` in `Cargo.toml`; every crate, the Esc
menu and the landing page read it from there. Changes go under `## [Unreleased]` in
[CHANGELOG.md](CHANGELOG.md) as they land. While in 0.x, a minor version adds content and a patch only
fixes things. To release:

1. Bump `version` in `Cargo.toml` and run `cargo check` so `Cargo.lock` follows.
2. In `CHANGELOG.md`, rename `[Unreleased]` to `[X.Y.Z] - YYYY-MM-DD`, add a fresh empty
   `[Unreleased]` above it and update the compare links at the bottom.
3. Commit, then `git tag vX.Y.Z && git push origin main vX.Y.Z`.

The Release workflow checks that the tag matches `Cargo.toml` and creates the GitHub Release from that
changelog section (`scripts/release-notes.sh X.Y.Z` prints it locally). The push to `main` redeploys
the site, including the [changelog page](https://wraithyy.github.io/the-last-dev/changelog/).

## Versions and browser constraints

- Rust 1.96.1, Bevy 0.19.1 (features `3d`, `ui`), wasm-bindgen 0.2.129. Rationale in [docs/PLAN.md](docs/PLAN.md).
- The web build is **WebGPU only**, with no WebGL2 fallback: `web/index.html` checks `navigator.gpu.requestAdapter()` first and shows a notice (without downloading the wasm or registering the service worker) when it is missing. Chrome and Edge 113+, Safari 26+ and recent Firefox on supported platforms work. Directional shadows use 1 cascade; MSAA is 4x or off.
- The release wasm is about 39 MB after `wasm-opt` (12.4 MB gzipped), plus about 52 MB of assets on disk; the service worker precaches only the shell, so a first visit downloads about 12 MB. See [docs/PERF.md](docs/PERF.md).

## Debug hooks (native, env-gated)

| Variable | Effect |
|---|---|
| `WOW_SCREENSHOT=path.png` (`WOW_SHOT_DELAY=s`) | Save an offscreen 1280x720 shot and exit |
| `WOW_NIGHT=1` | Hold the scene at full night (street lamps, night look) |
| `WOW_ZONE=<id>` | Start in that zone (campus, tower/1..5, datacenter/1..2, lab/1..2), sim-backed and playable, at its minimum level; `WOW_FLOOR=n` is `WOW_ZONE=tower/n` |
| `WOW_QUESTS=id[:active\|ready\|done],..` | Force quest states at start |
| `WOW_UNLOCK_ALL=1` | Open every door and elevator |
| `WOW_JIRA=1`, `WOW_MAP=1` | Open the quest log or the map at start |
| `WOW_KILL_BOSS=secs` | The zone boss dies (as a player kill) that many seconds in |
| `WOW_DIE=secs`, `WOW_TRAVEL=zone@secs` | The player dies, or the door fade to that zone starts, that many seconds in (death panel and loading screen shots) |
| `WOW_SHOT_TIMES=t1,t2,..`, `WOW_SHOT_SIZE=WxH` | Several timed screenshots (`<path>_<n>.png`); offscreen target size (e.g. 2560x1080) |
| `WOW_FPS=1` | Log average frame time and drawn meshes every 3 s |
| `WOW_MARIE_LOG=1..4` | Open Ill Marie's autonomy-log window on that screen at start (`marie_log.rs`) |
| `WOW_ENDING=1..6\|credits\|stinger` | Play the Ilmari ending (a replay: no command, no rewards) from that beat two seconds in, with made-up stamps when the save has none (`ilmari_end.rs`) |
| `WOW_ILMARI_LADDER_DEMO=1` | Put sample rows on the Ilmari core boards (Full, Async, Judgment) for the planner's Weekly tab and `~/endgame` shots (`raid.rs`) |
| `WOW_MUTE=1` | Start with audio muted (on the web: `?mute=1` in the URL) |
| `WOW_LEVEL=n` | Start level (overrides the floor minimum) |
| `WOW_COMPANIONS=dale,priya` | Companion loadout (Dale, Priya, Rupert; two at most); they come into an Ilmari wing on Story (`WOW_RAID_ASYNC=1`) |
| `WOW_PLAYER_AT=x,z,yaw_deg[,cam_deg]` | Start position after the first floor load; optional camera yaw offset (180 = front view) |
| `WOW_CAM_DIST=m` | Start zoom |
| `WOW_AUTOFIGHT=1` | Bot: walks to the nearest hostile (adds first), interrupts, heals/shields, runs the rotation, loots corpses, respawns, takes the elevator; logs every sim event |
| `WOW_COFFEE=secs`, `WOW_LINEUP=0` | Drain mana and start a Coffee Break after that many seconds; empty bare stage for pose close-ups |
| `WOW_TALK=1` | Talk to the nearest friendly NPC two seconds in |
| `WOW_GIVE_ITEMS=1`, `WOW_TROPHIES=1` | Grant a sample set of trophies at start, open the `~/trophies` window at start |
| `WOW_HOVER_SLOT=0..9`, `WOW_AUTORUN=1`, `WOW_LINEUP=1\|2` | Force a tooltip, hold forward, show the cast lineup |
| `WOW_HOVER_TIP=player-aura\|target-aura\|party-aura\|target-cast` | Force the aura or cast tooltip of the first matching node |
| `WOW_KEYBIND_DEMO=1\|2` | Rebind slot 1 to X and coffee to R in memory only (not saved); `1` also opens the `~/keybindings` panel |
| `WOW_SCROLL=<px>\|end` | Scroll every window list that overflows by that many UI px (or to its end) once, for screenshots (`hud/scroll.rs`) |

## Credits

3D models: characters by Quaternius (Ultimate Animated Character Pack, CC0); furniture by Kenney (Furniture Kit, CC0). Spell icons: Painterly Spell Icons by J. W. Bjerk (eleazzaar), CC BY 3.0. Item icons from game-icons.net by Lorc and Delapouite (CC BY 3.0). Full list: [assets/CREDITS.md](assets/CREDITS.md).

## Verification status (2026-10-13)

Verified:
- `./scripts/check.sh` exit 0: fmt, clippy `-D warnings` (native and wasm32), 240 tests passed, 3 ignored (186 `combat_core`, 54 `game_client`).
- Native screenshots driven by the debug hooks (`.playwright-mcp/integration/`): campus start with quest markers, the ticket dialog and ticket list, tracker progress, the `~/jira` log, the map on campus and in the data center, a locked door, a door transition, the CEO's "New zone unlocked" banner, the AGI fight, the AGI reveal, the victory screen and a 2560x1080 campus HUD. Campus native frame time about 5 ms (WOW_FPS, Apple Silicon, debug build).
- The browser build was not rebuilt in this pass.

Not verified:
- Manual playtest of game feel (movement, camera, combat pacing) and real mouse/keyboard input for talking and restarting.
- The browser build was not run in a browser in this pass.
- Pointer lock in a real headed browser, Firefox and Safari; FPS and memory profiling.
