# The Last Dev: bigger world and multiplayer (design proposal)

Status: Part 1 is implemented in `combat_core`, content and the client (see "Implemented" below);
Part 2 is a proposal. It builds on docs/GAME_DESIGN.md and docs/COMBAT_API.md.

## Goals

1. A bigger world: an office campus hub with several buildings (the current tower becomes one of them),
   quests, quest givers and travel between zones.
2. Co-op multiplayer for 2-5 players: one shared world instance per group, joined with a room code.
   No accounts, no persistent server-side characters in the first version.

Recommended order: world first (single player, no new infrastructure), then multiplayer on top of the
finished world. Multiplayer changes the sim's assumptions about "the player"; doing it after the world
work avoids refactoring the same code twice.

## Part 1: bigger world

### Structure

- **Zones** replace "floors" as the unit the sim loads. A zone is either outdoor (campus) or indoor
  (a building floor). The current five floors become zones `tower/1` to `tower/5`.
- **Campus hub** (outdoor, level 1-10 mix):
  - Parking lot (arrival).
  - Cafeteria with the quest givers.
  - Reception, as the gate to the buildings.
  - Smoking corner, where people go to complain about the AI.
  - Satellite buildings.
- **Buildings**, each a short dungeon of 2-5 zones with its own boss chain:
  - **Tower** (existing): the CEO.
  - **Data Center**: GPU cultists, a cooling-failure boss.
  - **Sales Tower**: the Customer becomes a raid-style boss.
  - **HR Annex**: a wellness labyrinth.
  - **Innovation Lab**: the AGI prototype as the final boss.
- **Travel**:
  - Doors and elevators are zone triggers, as the exit elevator is today.
  - The campus map (M is taken by mute, so N) shows the zones.
  - Fast travel happens through "the shuttle bus that is always late".

### Engine changes (combat_core)

| Area | Today | Change |
|---|---|---|
| Loading | `Sim::load_floor(floor, anchors, blockers)` despawns everything but the player | `Sim::load_zone(zone_id, anchors, blockers)`. Same semantics plus per-zone persistent state: killed trash stays dead for N minutes, bosses keep their kill flag for the session |
| Content | `floors: [...]` in game.ron | `zones: [...]` with kind (outdoor/indoor), level range, packs, friendlies, quest givers and links to other zones |
| Quests | none | New `quests` table: kill N of a unit key, loot an item, talk to an NPC, clear a zone. `Command::AcceptQuest` / `TurnInQuest` and `Event::QuestProgress` / `QuestCompleted`. Rewards are XP and trophies |
| Respawn | per-floor respawn point | the nearest "graveyard" (the break room) in the zone |
| Navigation | grid A* per floor | unchanged per zone. Outdoor zones are larger, so a coarser 1 m grid is used outdoors |

Determinism stays: quest progress is driven by sim events and needs no rng.

### Client changes (game_client)

- **Zone builder:**
  - Generalise `floors/` into `zones/` (shapes, themes, anchors).
  - An outdoor kit: grass, parking-lot asphalt, trees, benches, buildings as large blocker shells with entrances.
  - Sky instead of ceiling.
- **Quest UI:**
  - A "!" / "?" marker above quest givers.
  - A quest log window (`~/jira` styled as a ticket list) and a quest tracker on the right side of the screen.
- **Zone transitions:** the existing fade, plus a "Loading next sprint..." tip line.
- **Streaming:** load only the current zone and keep at most one neighbour preloaded. WebGL2 memory is the
  constraint, not the CPU.

### Content to write (the core of the game is the satire)

- Quest texts in the existing voice, for example:
  - "The AI-Native Intern lost the production database password. In Teams. Find it."
  - "Collect 8 Unread Strategy Decks for the VP. He will not read them either."
- Every new unit gets yells, a title and a trophy table. The existing review pass ("is it actually funny,
  is it specific") applies to every line.

### Estimate

| Step | Effort |
|---|---|
| Zones in the sim + content schema + tests | 0.5 day |
| Quest system (sim + tests) | 0.5-1 day |
| Outdoor kit + campus layout | 1 day |
| Quest UI + map | 0.5-1 day |
| Writing quests, yells and trophies for 3 new buildings | 1 day |

### Implemented (combat_core + content)

- Zones replace floors: `campus` (outdoor hub, 1-3), `tower/1`..`tower/5`, `datacenter/1`-`2` (6-8,
  unlocked by clearing tower/3), `lab/1`-`2` (10, unlocked by the CEO's death; the AGI Prototype in
  lab/2). `Sim::load_zone`, `zone_links` with unlock state, `ZoneLoaded` / `ZoneCleared` / `Unlocked`.
- Session world state: cleared bosses stay dead, killed trash returns only after 5 minutes away, the
  respawn point is the zone's `graveyard` anchor.
- Quests: 15 in `content/game.ron` (kill, loot, talk, clear-zone objectives; chains, level gates, XP
  and trophy rewards), `AcceptQuest` / `TurnInQuest` / `AbandonQuest`, quest events, per-player logs.
- New units: 4 campus NPCs (3 quest givers plus the intern), 3 campus trash, 5 data center units and
  2 bosses, 4 lab units and the final boss. New mechanics: spell reflect, mana drain.
- Client: every zone has a layout (outdoor campus kit, data center, lab, the five tower floors) whose
  anchors and blockers go to `Sim::load_zone`; doors and elevators fade with loading tips, locked doors
  name their requirement, every building has a way back to the campus. Quest UI: giver markers, the
  Jira-ticket dialog, the "Sprint backlog" tracker, progress toasts, the "Ticket closed" banner and the
  `~/jira` log (J). Map on N. The AGI's rack opens on death to show the MeatProxy, then the victory
  screen.
- Not done: the Sales Tower and HR Annex buildings, fast travel, saving. See COMBAT_API.md (Zones,
  Quests) and KNOWN_LIMITATIONS.md.

## Part 2: co-op multiplayer

### Requirement: offline first

The offline single-player game is the product; online co-op is an optional mode on top of it.

- The game starts and runs fully offline, with the local `Sim` in the browser. No network call at
  startup, and no server URL is needed to build or deploy to GitHub Pages.
- Online is an explicit menu choice ("Join room"). The client probes the server with a short timeout;
  if it is missing or down, it shows a message and stays offline. Nothing crashes or blocks.
- If the connection drops mid-session, the player returns to the offline game with a notice.

### Requirement: self-hosted server, join by address

- The server is a single self-hostable binary (`the-last-dev-server`), runnable on any machine on a
  LAN or a VPS. No central service is required.
- The "Join room" screen takes a server address (`192.168.1.20:7777` or a hostname) plus a room code;
  the last address is remembered locally.
- Browsers block `ws://` from an `https://` page (mixed content), so the GitHub Pages build can only
  reach `wss://` servers. For LAN play the server binary also serves the web client over plain HTTP:
  players open `http://<lan-ip>:7777/` and connect back to the same host over `ws://`. The native build
  can connect to any address.

### Architecture

The key asset: `combat_core` is pure, deterministic Rust with no Bevy dependency. It can run unchanged
on a server.

```
 browser (Bevy wasm)                 server (Rust, tokio)
 ------------------                 --------------------
 input -> Command  --- WebSocket --> room: Sim (combat_core), 30 Hz tick
 render <- snapshot/events <--------  broadcasts events + periodic snapshots
```

- **Authoritative server:** the server runs the `Sim` and decides combat, loot and XP.
- **Movement:** client-predicted. Today the client owns player movement and reports `PlayerMoved`; in
  multiplayer the server accepts the position with sanity checks (max speed, no wall clipping via the
  nav grid) and corrects on violation.
- **Transport:** WebSocket, which works in browsers and needs no extra crate on the client beyond what
  `web-sys` offers. That is a new dependency; it needs a decision. WebTransport was considered and
  rejected for now because of Safari support.
- **Wire format:** the existing `Command`/`Event` types serialised with `serde` (already a dependency)
  in a compact binary encoding, plus delta snapshots of `UnitState` for late joiners and every 1 s for
  drift correction.
- **Rooms:**
  - A room code like `MERGE-CONFLICT-42` and 2-5 players.
  - The room dies when it is empty for 10 minutes.
  - No accounts and no persistence in v1. Progress lives only for the session.

### Sim changes for multiple players

`Sim` assumes one player in 26 places (`Sim.player: Option<EntityId>`, `player()`, the respawn point,
the inventory, XP, the player yell cooldown, known spells, coffee, loot ownership). Each becomes
per-player:

- `Sim::spawn_player` returns an id per player, and `players()` lists them.
- Per-player state:
  - level, XP, inventory and trophies;
  - respawn point;
  - yell cooldown;
  - known spells.
- **Threat:** already a list. Mobs pick the top-threat player, as in classic MMOs.
- **Loot:**
  - Each kill gives every player in the group their own roll, so nobody fights over loot.
  - XP is split across the group, with a classic small group bonus.
- **Scaling:** enemy HP scales with group size (+40% HP and +15% damage per extra player), stored in content.
- **Determinism:** the server is the only sim, so lockstep is not needed. Determinism remains useful for
  replays and tests.

Implemented in `combat_core` (see COMBAT_API.md, Co-op group, and FORMULAS.md, Co-op group):
`spawn_player` / `remove_player` / `players()` / `apply_as`, routed events (`tick_routed`), group
threat and wipe reset, per-member loot rolls, XP split with group bonus, group HP scaling
(`config.group_hp_pct_per_player`), group zone loads.

### net_proto (wire protocol)

Crate `crates/net_proto`, shared by the server and the client. Dependencies: `combat_core`, `serde`,
`postcard` (1.1, `use-std`). The combat_core wire types (`Command`, `Event`, `Audience`, `Vec3`,
`SpellId`, `School`, `CastError`, `AiState`, `AuraEnd`) derive `Serialize`/`Deserialize`.

- **Frame:** `PROTOCOL_VERSION` (u16 LE) + one postcard message, at most `MAX_FRAME_BYTES` (64 KiB),
  one binary WebSocket message per frame. `encode(&msg) -> Result<Vec<u8>, ProtoError>`,
  `decode::<ClientMsg | ServerMsg>(&[u8]) -> Result<_, ProtoError>`: size, version, trailing bytes,
  then message validation. Random bytes never panic (tested).
- **ClientMsg:** `Hello { version, name }` (must come first, within `HELLO_DEADLINE_MS` = 5 s),
  `CreateRoom`, `JoinRoom { code }`, `LeaveRoom`, `Cmd(Command)`, `Move { pos, facing, moving,
  jumping, seq }`, `Emote { emote, target: Option<EntityId> }`, `Ping { t }`. `Cmd` accepts only `is_player_command` (no
  `PlayerMoved`, quest ids up to 64 bytes); debug hooks are `Sim` methods and unreachable. The server
  applies commands with `Sim::apply_as`, which rebinds the actor to the sender. Non-finite floats in
  `Move` are rejected.
- **ServerMsg:** `Welcome { you, room, zone, seed, players }`, `RoomCreated { code }`,
  `Error { kind, text }`, `PlayerJoined(PlayerInfo)`, `PlayerLeft { id }`, `Events { tick, events }`
  (the events whose `Audience` includes the client), `Snapshot { tick, units, players }`,
  `MoveCorrection { seq, pos }`, `Emoted { player, emote, target }`, `Pong { t }`. Decoding caps lists
  (2048 events, 1024 snapshot units, 5 players, 64 loot items / offered quests) and texts (512 bytes)
  and rejects non-finite floats.
- **Errors:** `ErrorKind::message()` is the text to send. Unknown room, full room and a malformed code
  all answer `CannotJoin` ("Cannot join that room."), so the error is not an oracle for guessing
  codes.
- **Snapshots:** `UnitSnap { id, key_idx, level, pos_cm: [i32; 3], facing: u8, hp, max_hp, target,
  flags }`, built with `UnitSnap::from_state`. Centimetre integers and a one-byte facing are 1-3 bytes
  each as postcard varints instead of 4 per f32; enough for drift correction. Auras and cast bars come
  from `Events`. `PlayerSnap { id, mana, max_mana, xp, xp_to_next }` for party frames.
- **Display names:** `sanitize_name(raw, n)`: trimmed, 2-16 ASCII letters, digits, space, `-`, `_`;
  anything else becomes `Intern-<n>`.
- **Room codes:** `FIRST-SECOND-XXXXXXXX`, e.g. `MERGE-CONFLICT-7K3M9QZA`: two satirical words
  (flavour, 8 bits) plus 8 Crockford base32 symbols (no I/L/O/U, 40 bits).
  `room_code_from_bytes(&[u8; 10])` takes OS randomness from the server; there is no seeded code
  generator. `parse_room_code` is case-insensitive, ignores dashes and spaces, folds O to 0 and I/L to
  1, and returns the canonical form.
- **Emotes:** `Emote` (`/sync`, `/circle back`, `/per my last email`, `/lgtm`, `/+1`, `/this could
  have been an email`, `/take this offline`, `/no bandwidth`, `/blocked`, `/ship it`) with
  `command()`, `text()`, `text_at(name)` ("Let's sync on this, Alice.") and `from_command()`. No
  free text on the wire. An emote can carry the targeted teammate's id; the server keeps it only for
  another member of the room. `PROTOCOL_VERSION` is 3 since targeted emotes and `Resurrected`.

### Hosting and operations

- **Hosting:**
  - A single small VPS or a container platform (Hetzner, Fly.io), about 5-10 USD/month. A free tier is
    possible but sleeps.
  - The client stays on GitHub Pages and connects to `wss://...`.
- **Security, required before going public:**
  - Validate and size-limit every message.
  - Rate-limit per connection.
  - Run the movement sanity checks.
  - Cap the number of rooms and players.
  - No user text other than room codes and a sanitised display name. A text chat is a moderation
    problem, so v1 has only predefined emotes and quick phrases. That also fits the satire, for
    example "/sync", "/circle back", "/per my last email".
- **Observability:**
  - Structured logs.
  - A room count gauge.
  - A crash restart policy.
- **Secrets:** none are needed in v1, because there are no accounts. TLS comes from the platform.

### Estimate

| Step | Effort |
|---|---|
| Per-player sim refactor + tests (multi-player threat, loot, XP, scaling) | 1-1.5 days |
| Server crate (tokio + WebSocket, rooms, tick loop, snapshots) | 1 day |
| Client networking (connect, room code UI, prediction/correction, remote players' visuals and nameplates) | 1-1.5 days |
| Security hardening + load test (bots) | 0.5-1 day |
| Hosting, CI deploy of the server | 0.5 day |

## Risks

- **Scope:** the satire content is the core of the game. Infrastructure work must not starve the
  writing.
- **WebGL2 memory with a big world:** mitigated by zone streaming and shared meshes and materials.
- **Movement:** client authority invites cheating, and the netcode can feel bad. Co-op among friends
  tolerates light checks; competitive play would not.
- **Hosting:** costs money and needs operational care (someone must restart it when it crashes).
  Room limits keep the bill bounded.
- **New dependencies** (tokio, tungstenite or axum on the server; web-sys WebSocket on the client) need
  an explicit decision.

## Open questions

1. Is session-only progress acceptable for co-op v1, or do characters need to be saved? Saving needs
   storage and some identity, and with them more security work.
2. Which hosting, and who pays for it?
3. Should the campus be one big open zone or several medium ones? One big zone is nicer, but costs
   more nav and memory.
4. Should the first multiplayer milestone be co-op in the existing tower only (smaller), before the
   campus exists?
