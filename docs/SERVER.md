# Server (`the-last-dev-server`)

A single self-hostable binary. It serves the web client over HTTP and hosts co-op rooms over
WebSocket at `/ws`. Nothing is required for the offline game; see
`docs/WORLD_AND_MULTIPLAYER.md` Part 2 for the design.

Each room runs an authoritative `combat_core::Sim` at 30 Hz (`crates/server/src/room_state.rs`) and
speaks `net_proto` (`crates/net_proto`). Content (`content/game.ron`) and zone geometry
(`content/zone_geometry.ron`) are embedded in the binary.

## Run on a LAN

```sh
cargo build -p server --release
./target/release/the-last-dev-server --web-dir ./web
```

The server prints its URLs, for example `http://192.168.1.20:7777/`. Players on the same network open
that URL; the client connects back to `ws://<same host>/ws`. Allow inbound TCP 7777 in the host's
firewall if players cannot connect.

`/healthz` returns `{"status":"ok","rooms":N,"players":N,"connections":N}` to loopback clients (and to
everyone with `--healthz-details`); any other client gets the plain text `ok`. A loopback peer that
sends `X-Forwarded-For` without being a `--trusted-proxy` is treated as remote.

## Options

| Flag | Env | Default |
|---|---|---|
| `--bind` | `TLD_BIND` | `0.0.0.0:7777` |
| `--web-dir` | `TLD_WEB_DIR` | `./web` (index.html, pkg/, assets/) |
| `--content` | `TLD_CONTENT` | embedded `content/game.ron`; a replacement must match the clients' build (snapshots index units by content position) |
| `--max-rooms` | `TLD_MAX_ROOMS` | 50 |
| `--max-players-per-room` | `TLD_MAX_PLAYERS_PER_ROOM` | 5 |
| `--max-conns` | `TLD_MAX_CONNS` | 200 (WebSockets; TCP cap is 2x) |
| `--allowed-origin` (repeatable) | `TLD_ALLOWED_ORIGINS` (comma list) | none; same-origin and no-Origin always allowed |
| `--trusted-proxy` (repeatable) | `TLD_TRUSTED_PROXIES` (comma list) | none |
| `--healthz-details` | `TLD_HEALTHZ_DETAILS=1` | off |
| `--per-ip-conns` | `TLD_PER_IP_CONNS` | 16 (0 disables) |
| `--per-ip-rooms-per-min` | `TLD_PER_IP_ROOMS_PER_MIN` | 6 (0 disables) |
| `--per-ip-failed-joins` | `TLD_PER_IP_FAILED_JOINS` | 10 per minute, then 30 s lockout (0 disables) |
| `--no-lan-exempt` | `TLD_NO_LAN_EXEMPT=1` | LAN clients exempt from per-IP limits |

Flags win over env (a list given as flags replaces the env list). Log level via `RUST_LOG` (default `info`). If the web dir is missing the server
warns and still serves `/ws` and `/healthz`.

## Run on a VPS (wss://)

Browsers refuse `ws://` from an `https://` page, so the GitHub Pages client needs `wss://`. Bind the
server to localhost and put a TLS reverse proxy in front. Caddy obtains certificates automatically
and proxies WebSockets without extra config:

```
game.example.com {
    reverse_proxy 127.0.0.1:7777
}
```

```sh
the-last-dev-server --bind 127.0.0.1:7777 --web-dir ./web
```

Run it under systemd (or a container) with `Restart=always`. No secrets or accounts are involved.

### Public hosting checklist

- Terminate TLS in a reverse proxy (Caddy above) and bind the server to `127.0.0.1`; never expose the
  plain-HTTP port to the internet.
- Browsers send `Origin` on WebSocket upgrades and the server answers 403 unless it equals the request
  `Host` or is allowed. A client served from GitHub Pages needs
  `--allowed-origin https://wraithyy.github.io`. Native clients send no Origin and are unaffected.
- Declare the proxy with `--trusted-proxy 127.0.0.1`. Only then is the client IP taken from the
  right-most `X-Forwarded-For` entry that is not itself a trusted proxy; otherwise the socket peer is
  used. The proxy must overwrite or append to the header, never pass a client's value through unchecked.
  Trusted proxies are not subject to the per-IP connection cap (all clients share their address).
- Raise `ulimit -n` (or systemd `LimitNOFILE`) above `2 * --max-conns` plus headroom; each connection
  is a file descriptor.
- Firewall: allow only 80/443 to the proxy, not 7777.
- Keep `--healthz-details` off unless the endpoint is not public.

## Abuse limits and NAT

Players behind one NAT or CGNAT (household, office, school, mobile carrier) share a public IP, so the
per-IP limits are deliberately generous: 16 concurrent connections (a full room plus reconnects and a
second room), 6 room creations/minute, 10 failed joins/minute then a 30 s lockout. They exist to stop
one host from monopolising the server or brute-forcing codes, not to police groups. Loopback, RFC1918,
link-local and IPv6 ULA clients are exempt by default (a LAN server), except from the global caps;
`--no-lan-exempt` removes that. IPv6 clients are limited per /64. Set a limit to 0 to disable it.
Behind a trusted proxy the per-IP connection cap does not apply; the other limits use the forwarded IP.

Room codes look like `MERGE-CONFLICT-7K3M9QZA` (`net_proto::room_code_from_bytes` over 10 bytes from
the OS RNG; the 8-symbol Crockford suffix carries 40 bits). Unknown, full and malformed codes all get
the same `Error { kind: CannotJoin }` and close `1008 cannot join`, and all count as failed joins.
Codes appear in logs only as a keyed hash prefix; display names and payloads are never logged.

HTTP request heads must arrive within 10 s (slowloris), `Hello` within 2 s of the WebSocket upgrade
and the room request within 2 s after it (`Config::handshake_timeout`), so a client that stays silent
after connecting is dropped. Static files: path segments starting with `.` return 404, symlinks leaving the web dir return
404, every response carries `nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer` and a CSP.
The CSP allows inline scripts because `web/index.html` has them; hashing them is the upgrade path.

## Protocol flow

Binary WebSocket frames at `/ws`, each one `net_proto` frame (`PROTOCOL_VERSION` + postcard).

1. Client: `Hello { version, name }` (name sanitised by `net_proto::sanitize_name`, else `Intern-<n>`).
   Wrong version: `Error { BadVersion }`, close 1008. Anything else or nothing in time:
   `Error { HelloRequired }`, close 1008.
2. Client: exactly one `CreateRoom` or `JoinRoom { code }` (code parsed leniently by `decode`).
   - Create: `RoomCreated { code }`, then the join below. Over the creation limit: `Error { RateLimited }`;
     server at `--max-rooms`: `Error { ServerFull }`; both close 1008.
   - Any join failure: `Error { CannotJoin }`, close 1008.
3. Joined: `Welcome { you, room, zone, seed, players }`, then a full `Snapshot`. Others get
   `PlayerJoined`. The player spawns at the zone's `player_spawn` (pack formation for later joiners).
4. In the room:
   - `Cmd(cmd)`: only `net_proto::is_player_command` commands decode; applied with `Sim::apply_as`,
     which rebinds the actor to the sender, so nobody can act as another player.
   - `Move { pos, facing, moving, jumping, seq }`: accepted when the player is alive, inside the zone's
     wall extent (+1 m), at a reachable height (-1..8 m), not with its feet inside a blocker that spans
     the body, and within a distance budget that refills at 1.25 x run speed (8.75 m/s), holds up to
     1 s of movement plus 1 m jitter, and grows by a knockback's strength. Respawn, teleport and zone
     travel reset it to the sim position. Rejected: `MoveCorrection { seq, pos }` (last accepted spot).
   - `Emote(e)`: `Emoted` to everyone. `Ping { t }`: `Pong { t }` straight from the connection.
     `LeaveRoom`: close 1000, then `remove_player` and `PlayerLeft` to the others (also on disconnect).
5. Every tick: `Sim::tick_routed`, one `Events { tick, events }` frame per player with the events meant
   for them (skipped when empty); a `Snapshot` (`UnitSnap` + `PlayerSnap`) every second, on join and
   after zone travel.
6. Zone travel: when a living player stands in the trigger of an unlocked exit (door or tower
   elevator), the whole group loads the target zone, arriving in front of the door back (elevators:
   the zone's `player_spawn`). A player only triggers an exit after having stood outside every trigger since arriving. `ZoneLoaded` and the despawns go out in the same tick, then a snapshot.

The sim pauses while a room is empty. Zone geometry comes from the client's Bevy layout builder via
`content/zone_geometry.ron`; after changing a layout run
`TLD_WRITE_ZONES=1 cargo test -p game_client zone_geometry` (the test fails while the file is stale).

## Limits

| Limit | Value | On violation |
|---|---|---|
| Message size | 64 KiB | close 1009 |
| Rate per connection | 60 msg/s, burst 120 (pings count) | close 1008 |
| Idle | 30 s without any frame (clients ping every 5 s) | close 1001 |
| Text frames | rejected | close 1003 |
| Connections | `--max-conns` | HTTP 503 on upgrade |
| TCP connections | 2x `--max-conns` | accept pauses |
| Per-IP connections / room creations / failed joins | see above | drop / close 1008 / close 1008 |
| Request head | 10 s | connection closed |
| `Hello`, then the room request | 2 s each | `Error`, close 1008 |
| Invalid messages (undecodable, handshake messages inside a room) | `Config::invalid_message_limit` (5), via `ws::Strikes::bump` | close 1008 |
| `Cmd` / `Move` per connection | 20/s (burst 20) / 30/s (burst 45) | dropped + strike |
| `Emote` per connection | 1/s (burst 3) | dropped |
| Pending commands per room queue | 1024 | sender closed with 1008 |
| Rooms / players per room | `--max-rooms` / `--max-players-per-room` (capped at the sim's 5) | `Error`, close 1008 |
| Outbound queue per player | 64 messages | client dropped (1013) |
| Empty room lifetime | 10 min; 60 s if nobody ever joined the creator | room destroyed |

Message payloads and room codes are never logged. Logs cover room create/destroy, joins/leaves, and a rooms/players
gauge every 60 s. `/healthz` (operator view) also reports `tick_p99_us`, the 99th percentile of the last 4096
room ticks; the bot load test (`cargo test -p server --release -- --ignored load --nocapture`, 10 rooms x 5
bots, 30 s) measured about 45 us and 450 bytes/s down per client on a development laptop (release build). Ctrl-C or SIGTERM closes all rooms and exits (10 s grace for stuck sockets).
