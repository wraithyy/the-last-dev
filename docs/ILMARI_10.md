# Ilmari 0.10: implementation spec

Status: engine pieces C1-C3 built (swarm, illusions, recorder) with the client drone pool, protocol 38; Wing 3-4 content (C4 onward), boss splitting and platforms (C9-C10) not built. Written 2026-10-23, statuses updated the same day. Companion to `docs/ILMARI_RAID.md` (campaign
design and guardrails, which still win in any conflict), `docs/ILMARI_09.md` (the format this spec
mirrors, and the "as built" notes for Wings 1-2 and the B15 companions) and `docs/DUNGEONS.md` (the
engine vocabulary this spec reuses). Campus coordinates follow `docs/CAMPUS_REWORK.md`.

Scope, from the campaign milestones: **Wing 3 "Training Data"** and **Wing 4 "The Cloud"**, five
bosses, Story and Full, plus the four engine pieces the campaign doc names: per-player illusions, an
action recorder, swarm aggregation, and (new in this spec) boss splitting with a cloud-bill meter and
sequenced platforms.

Hard rules inherited from `ILMARI_RAID.md`: nothing about Ilmari before the credits (except Marie's
foreshadowing), the normal campus unchanged, Ilmari gear bonuses only in Ilmari content, offline-first
(every encounter solo-clearable on Story by any class, with or without companions), same tone with a
sharper edge. Every task also obeys `CLAUDE.md`: one CHANGELOG line per player-visible change in the
same commit, a `CREDITS.md` row for any third-party asset, no version edits outside `Cargo.toml`, and
the 0.10.0 bump only when the user asks for a release.

## C.0 Context: what 0.9 left

0.9 shipped the Opening, the Prologue, Wings 1-2 (six bosses), Story and Full, companions and the
co-op trust model. The facts this spec builds on, all from the code and docs as built:

- **Rules.** Story: any group size, Async figures (40 % HP, 50 % damage), `AiAbility.mode` marks spells
  `story: Skip`, key prefix `story:` for lockouts, reduced loot (one piece at 50 %, tokens, cosmetics).
  Full: Normal figures, minimum three players (below that the Async rule continues), `boss_items`
  pieces per member. The lead alone picks Story (`Command::SetIlmariStory`). Judgment Day is 1.0.
- **Companions (B15).** Friendly units with `UnitDef::companion`, not group members and never in
  `Sim::players`; pairs picked at the sign-up sheet in `resistance/1`; wings and Story only; at most 2
  per player and 2 per room; each counts as 30 % of a member in the HP and damage scalings; the brain
  in `companion.rs` has no rng, follows at 4 m, steps out of hostile telegraphs, and ignores `split`
  telegraphs. Not built: Priya's `Dispel`/`Resurrect`/Label Cable, a deliberate "stand on the stack",
  party frames. A dead companion is back 8 s after the owner leaves combat.
- **Item band.** `GearSource::Ilmari`, ilvl 16-20, bonuses only inside Ilmari zones through the item
  level sync in `economy.rs::gear_totals`. The net client's tooltip totals do not track the zone (a
  known limitation, unchanged here).
- **Protocol.** `PROTOCOL_VERSION` is 37. 0.10 expects two more bumps (38 with Wing 3, 39 with Wing 4),
  each in the commit that changes a frame, never in a docs commit.
- **Reusable as-built pieces.** Per-window counters (the HR review's, instead of a ring), `Effect::
  {SummonMirror, Vote, Review, Summon, Teleport, Detonate, PairUp, Spread, ThreatSwap}`,
  `TelegraphDef {count, step_m, step_ms, cover, gap, march, split, label}`, `requires_aura`,
  `death_burnout`, `death_spawn`, `PropKind::Portal`, `Interact` props with length-capped keys,
  `Summoned.as_player`, the `Event::Raid` late-joiner catch-up, `Difficulty::Story`.
- **Gaps this spec closes.** No sim-side units that stand for a crowd (the biggest 0.9 fight had 8
  cables); terrain features are static (`terrain.rs::Feature` is Hill, Mesa, River, Bridge), so there
  is no floor that changes at run time; no per-viewer rendering (every client draws the same world);
  no record of what players cast.

Sixty seconds of orientation for the player: Wing 3 is about information (what is real, what you
feed the machine), Wing 4 is about infrastructure (what scales, what falls over). Both are solvable
alone on Story, and neither needs a specific class.

## C.1 Difficulty and the shared rules

Unchanged from B.3, with these 0.10 additions:

- **Story simplifies the rule-change mechanic, never removes the boss.** Each boss below lists its
  Story column. Tells that are optional on Full (shimmer on fakes, the next-region lookahead) are
  always on in Story.
- **Level bands.** Wing 3 enemies are authored at level 14, Wing 4 at 15 (Wing 1 was 12, Wing 2 13).
- **Companions on 0.10 content** (rules of thumb, tested in C.9): illusions do not apply to them (they
  read sim state, which is always the truth), they are not recorded by the Model Collapse recorder,
  they ignore the cloud bill, they are not "players" for the Autoscaler's per-window damage figure
  unless noted, and they snap to their owner when their platform goes dark.
- **Lockouts** per boss per difficulty, as in 0.9. Wing completion quests: `il_w3_done`, `il_w4_done`.

## C.2 Prologue and attunement additions

Same shape as B.2: short, optional, existing objectives only, Dale gives them from the base in
`resistance/1`. The wing doors live in the occupied campus phase (`campus_occupied`) and the doors
shut again outside Ilmari content exactly like Wings 1-2 (the "no Ilmari before credits" test is
extended, see C.9).

| Quest | Giver | Objective (existing kind) | Opens |
|---|---|---|---|
| `il_07_lab_intel` Read The Terms | Dale; `requires` `il_w2_done` | `Collect("il_lab_pass", 3)` from lab androids on the Lab forecourt | Wing 3 attunement |
| `il_08_wing3_attune` The Lab Airlock | Dale | `Kill("il_airlock_scraper", 1)` (elite, a drone swarm in a trench coat) | `door:ilmari3/1` |
| `il_09_roof_intel` Above The Fold | Dale; `requires` `il_w3_done` | `Interact("il_roof_antenna", 3)` on the Data Center east dock | Wing 4 attunement |
| `il_10_wing4_attune` Roof Access | Dale | `Kill("il_roof_warden", 1)` (elite, "Change Advisory Board of one") | `door:ilmari4/1` |

- **Wing 3 door: the Innovation Lab** at (-72, -80). The occupied Lab is wrapped in "TRAINING
  IN PROGRESS" banners; the door is the Lab's glass airlock, a revolving door that logs every spin.
  Enemies at level 14: lab androids, data-labelling interns on a 14-hour shift (friendly, flavour
  only), scraper drones, prompt-injection mimics (a vending machine that offers "ignore previous
  instructions").
- **Wing 4 door: the Data Center roof.** Wing 1 already uses the Server Room Gate at (98, -84); Wing 4
  uses a different prop on the same landmark: a maintenance ladder on the east face beside the
  Migration dock, which climbs to `ilmari4/1`. Enemies at level 15: rooftop cooling-fan elementals,
  generator golems, "Reserved Instance" bots, pigeons that have been promoted (flavour).
- **World quests** (one new each, repeatable once, existing objectives): `il_wq_scrape`
  (`Defend` the Lab's whiteboard for 90 s against a small drone wave), `il_wq_status`
  (`Interact` 3 props: the status monitors on the dock, all green, all lying).
- **Vendor.** Brenda gets a Wing 3 and a Wing 4 page (tokens only, the 0.9 weekly cap of 200 per
  player is unchanged). Three quotes, all the same, as before.
- **Dale's lines.** Gate 3: "Twenty-two years in a booth. Nothing in the booth ever read my drafts.
  Now everything does." Gate 4: "The roof. I always wanted to go up. Nobody ever let me, there was a
  ladder policy."
- **Wing 2 cutscene teaser** (open question from 0.9, answered here): the wall screen after Performance
  Review shows "TRAINING DATA: collecting" with no wing name; Dale on the radio: "It's reading again."
  This names nothing beyond what the door says.

## C.3 The four engine pieces

Order of implementation follows C.8. Each piece lists its sim state, its wire shape, who is affected,
and its test approach. Only `combat_core` knows the rules; `game_client` draws; `server` runs the same
sim.

### C.3.1 Swarm aggregation (Scraper Swarm)

**Problem.** "Thousands of drones" cannot be thousands of sim units: the server tick, the snapshot
size and the target picker would all pay per drone.

**Design.**

- **A swarm cluster is one ordinary `Unit`** with a new `UnitDef.swarm: Option<SwarmDef>` and a
  runtime `swarm_count: u16`. `SwarmDef { per_drone_hp, eat_per_drone_per_s, speed, merge_below,
  max_clusters }`. The cluster's HP pool is `swarm_count * per_drone_hp`; `swarm_count` is derived
  from HP: `ceil(hp / per_drone_hp)`. A cluster of 125 drones at 30 HP each has a 3,750 HP pool.
  Because it is a normal unit, existing replication, threat, targeting, nameplates and the bot target
  picker work unchanged; the only wire change is `swarm_count` in the unit snapshot (protocol 38).
- **Damage model.** Damage to a cluster is applied to the pool, then a *crowd multiplier* is applied so
  that "hitting one drone" is not 1/125th of the fight: single-target hits count `1 + count / 40`
  drones' worth (cap x4); area hits (any effect with a radius, Circle or Cone) count x3 that. Numbers
  are tuned by bot clears (C.9) and recorded in `FORMULAS.md`. Class feel: the Fullstack caster and the
  DevOps healer's damage spells feel good, the Backend tank's cleave feels very good, melee single
  target feels honest.
- **Movement and target.** A cluster walks to the nearest un-eaten draft by `ai::step_toward` with
  `CLEARANCE_M`, eats while within 2.5 m: each tick the draft's `integrity` (a prop meter, 0..=1000)
  drops by `count * eat_per_drone_per_s * dt`. Clusters aggro on a player only when hit (threat table),
  then return to the drafts when the threat expires: the fight is a defence, not a tank-and-spank.
- **Population bound.** `max_clusters` (32 on Full, 12 on Story). When a wave would exceed it, the two
  smallest clusters closest together merge (deterministic tie-break by unit id). Large clusters shed:
  a cluster above 200 drones that is hit by an area effect spawns a daughter of 25 % of the pool
  (`death_spawn`-style, bounded by `max_clusters`). The sim therefore holds at most 32 swarm units,
  however many drones the HUD claims.
- **Class props as defence.** The Security Engineer's terminal can print `robots.txt` (a line prop
  across the drafts, clusters path around it; "compliant" for 10 s, then ignored with a yell), the
  DevOps rack gives a `Rate Limit` field (clusters inside move at 50 %), the Data Scientist's dashboard
  shows each cluster's remaining pool as a "forecast". Any class can `Interact` with a draft to
  `Mark Confidential` (a 12 s absorb on its integrity, 40 s cooldown shared per draft): so no
  class-specific mechanic is required (guardrail 6).
- **Client.** The client draws `min(count, share)` drones per cluster from one pooled set of entities
  sharing one mesh and one material handle (so Bevy batches them), `NotShadowCaster`, animated by a
  single system that offsets transforms from `hash(cluster_id, drone_index, time)`; the sim never sees
  a drone position. The drawn budget scales with the quality preset (table in C.7). A killed drone's
  poof is a pooled particle, not an entity, and capped at 20 per frame.
- **Companions.** The damage companion (Rupert) prefers clusters nearest a draft; the tank holds the
  threat of the nearest cluster; the healer is unchanged. They do not interact with drafts (a player
  action), so Story solo with companions leaves Mark Confidential to the player.

**Test approach.** Unit tests: HP to count; the crowd multiplier; merge tie-break; the population
never exceeds `max_clusters` over a 10-minute scripted spawn run; draft integrity falls by the
formula with a frozen cluster. Determinism: two runs of the same seeded fight are bit-identical.
Server test: a room with the wave running keeps its snapshot under the existing size budget.

### C.3.2 Per-player illusions (The Hallucination)

**Problem.** The campaign says "different per player". The sim must stay deterministic and the server
authoritative, and the fight must stay fair.

**Core rule: illusions are cosmetic. The sim only ever holds the truth.**

- **Real set (sim).** Every `Confabulate` cycle the sim picks the real hazards with the usual seeded
  stream, **one draw per cycle regardless of player count** (so the stream does not depend on how many
  players are in the room, and replays stay stable): real telegraphs (ordinary `TelegraphStarted`
  events, damage resolved as any telegraph) and, in the terrain phase, the real safe tiles of the
  arena grid (`HazardGrid`, below). The cycle's public salt is the draw, emitted as
  `Event::IllusionCycle { cycle, salt }` (protocol 38).
- **Fake set (pure function).** `illusion::layout(salt, player_id, cycle, arena) -> IllusionLayout`
  in `combat_core/src/illusion.rs` is a pure integer function (splitmix-style hash, no float, no
  state, no rng stream). Both the client (to render) and the tests (to assert) call it. It returns the
  fakes the player sees: extra telegraphs (same shape family as the reals, same label font) and the
  tiles they believe are safe.
- **Constraints (enforced and tested).** Fake telegraphs: same count as the reals, never exactly
  coincident with a real one, and never placed so that the player's current safe route is blocked
  entirely (a solvable layout always exists: checked on the nav grid). Fake tiles: of the tiles a
  player sees as safe, at least one and at most all-but-one are really safe, so no player is ever
  fully right or fully wrong, and the best strategy is the group's comparison.
- **Who sees what.** The owning client renders its own fakes only; other players never see them. The
  real telegraphs are shared as usual. Players cannot tell their fakes from the reals by looking
  (same material, same timing), except:
  - **Story:** fakes shimmer (a colour-split edge), always.
  - **Full:** no tell. The tells are social (callouts disagree) and class/prop: the **Ground Truth
    boards** (two props in the arena, `Interact` reveals the real set to the user for 8 s, 25 s
    cooldown per board) and the **Data Scientist's `Ground Truth` utility** (a new utility spell, 40 m,
    20 s cooldown, applies the `grounded` aura to every group member within 20 m for 8 s: the client
    draws reals with a thin outline and dims fakes). The campaign's "dashboards show which zones are
    real" is the prop and the class flavour.
- **Damage is decided by the sim on the real set only.** A fake never hurts; a real hazard hurts
  whatever the client draws over it. The terrain phase masks reals for Full (a real pit can be drawn
  as plausible floor in that player's view) only for tiles in the "believed safe" overlap rule above;
  Story never masks.
- **Hazard grid.** The Hallucination's arena is a 3 x 3 grid of 8 m tiles (`HazardGrid` is a small
  zone feature, a list of rects plus a per-tile state `Safe | Overflow`). It is a sim feature
  evaluated by the same position-vs-shape check as ground effects: `Overflow` tiles apply the
  `Context Overflow` damage and a `Stale Cache` aura. Movement stays client-owned; the sim only reads
  the player's position.
- **Server.** Nothing new is sent per player. The server runs the sim, emits one cycle event, the
  clients derive their fakes. Late joiners get the current `cycle` and `salt` in the `Event::Raid`
  catch-up.
- **Trust.** Because the reals are public (the same events everyone receives), a modified client can
  draw only reals. In PvE the only person helped is the cheater, and Full is already solved by the
  boards for any honest client; the risk is accepted and written into `KNOWN_LIMITATIONS.md`. The
  hardening path (per-recipient fake events generated by the server, no layout function on the
  client) is listed under out of scope.
- **Companions.** Illusions do not exist for them: the brain reads telegraph events and the real
  grid, so a companion always stands in the truth. This is a small advantage and is covered by the
  Story numbers.

**Test approach.** (1) `layout` determinism with a golden hash for fixed inputs (two runs, and a
frozen expected string checked into the test). (2) Per-player difference: across 20 cycles with 5
players, no two players ever get an identical layout. (3) Property tests of the constraints: count,
non-coincidence, solvable route, overlap bounds, over 1,000 random salts. (4) **The cosmetic proof:**
run the same seeded fight twice, once with the normal `layout` and once with the layout function
replaced by a stub that returns nothing, and assert the sim hashes are identical. (5) Bot clears:
Story solo every class (reads the real set from the sim), Full three bots with a board-reading bot.
(6) The golden replay of the base game stays byte-identical (guardrail 1).

### C.3.3 Action recorder (Model Collapse)

**Design.**

- **Record.** `echo.rs` holds `ActionLog`, a bounded ring in the sim (not saved, cleared on wipe and
  encounter reset): `(tick, player_id, spell_id, rel_x, rel_z, facing)` where the position is
  relative to the arena centre. Only real players record (companions, pets, mirrors and summons never
  do, by construction: the log filters on `Sim::players`). Only spells flagged recordable (hostile
  casts with an effect, not movement or consumables) record. Cap: 512 entries total and 64 per player
  per 30 s, oldest dropped first.
- **Replay.** Each recorded cast is played back as a boss ability `delay` later (`Full` 60 s, Story
  45 s, with a 3 s ghost telegraph). The ability is chosen from a small table, not per spell:
  `EchoKind` derived from the spell's first effect: `Strike` (damage: a Circle on the recorded
  position, damage scaled by the recorded cast's power), `Mend` (heal: the boss heals a sliver),
  `Ward` (absorb: a boss shield), `Hush` (interrupt or lockout: a silence cone), `Call` (summon: a
  small add), `Dash` (movement: the boss charges to that spot). A ghost of the caster (client only,
  `Echoed { as_player, kind }`, the same pattern as `Summoned.as_player`) plays the animation.
- **Repetition empowers, variety starves.** A deterministic formula over the rolling window of the last
  30 s of the log (integer arithmetic only, a 33-entry fixed table for `-p ln p` so native and web
  agree):

  ```
  n        = casts in the window                      (n < 6 -> H = 1000, no effect)
  p_s      = 1000 * count(s) / n                      (per mille, per distinct spell s)
  H        = sum(-p_s * ln(p_s/1000)) / ln(K)         (normalised entropy, 0..1000)
  K        = min(distinct spells in the group's kits, 8)
  E        = 1000 - H                                 (empowerment, 0..1000)
  ```
  - **Empowered** (`E >= 500`, the party is repeating itself): every echo deals `x(1 + 0.8 * E/1000)`
    damage, plus a per-spell overfit: an echo of spell s is multiplied by `1 + 0.15 * (count(s) - 1)`
    capped at 2.5. The boss shows `Overfitting` stacks (an aura, visible, with the changelog-style
    screen text "Training loss: 0.003").
  - **Starved** (`H >= 800` for 12 s running): the `Underfit` aura: the boss takes +20 % damage, echoes
    are not replayed and the boss cannot start a new echo wave for 10 s (a free DPS window, the
    same shape as Patch Tuesday's reboot).
  - Between the bands: baseline echoes, no modifier. The window can only be gamed one way: do a
    *varied* rotation. A solo player needs `K = min(distinct spells in the kit, 8)` and the threshold
    uses `min(5, K - 1)` distinct spells in the window, so a three-ability kit can still starve it.
- **Story.** The delay is 45 s with a 5 s ghost telegraph, the window is 45 s (slower to empower, more
  forgiving), `Overfitting` caps at x1.5, and the Starved threshold is `H >= 650`. Echo kinds `Hush`
  and `Call` are `story: Skip`. Bounded: at most 12 echoes per second of replay (surplus dropped by
  `(tick, player_id)` order, so it is deterministic).
- **Companions.** They never feed the log (they are not in `Sim::players`). In solo Story with two
  companions the log therefore holds the player alone, which is the intended solo case.
- **Determinism.** The log is a pure function of the command stream; the replay is scheduled from the
  log by tick; no new rng. A replay of an encounter is bit-identical across two runs.
- **Wire.** `Event::Echoed { as_player, kind, at_ms }` for the ghost and the telegraph (protocol 38, the
  same bump), the `Overfitting`/`Underfit` auras are ordinary `AuraApplied`. The log itself is not
  sent: the client does not need it and a client cannot write it.

**Test approach.** `H` for known distributions (all one spell = 0, uniform over 8 = 1000, golden
numbers), the table against `f64` within one unit; delay and cap behaviour; the log ignores companions
and mirrors; the Starved window opens and closes; a bot that spams one spell loses to the empowered
echoes, a varied bot clears (the two bot scripts live in `tests/ilmari_wing3.rs`); two-run determinism.

### C.3.4 Splitting and the cloud bill (Autoscaler)

- **Split.** `UnitDef.split: Option<SplitDef { window_ms, threshold_pct, child, hp_share_pct,
  max_copies }>`. The sim tracks damage taken per unit in a rolling window (the same per-window counter
  the HR review uses): when more than `threshold_pct` of the unit's max HP lands inside `window_ms`,
  the unit spawns one `child` replica and gives it `hp_share_pct` of its *current* HP (so the total
  pool is conserved: splitting never prolongs the fight, it multiplies the mechanics). Full: window
  5 s, threshold 8 %, share 40 %, at most 4 copies, a 6 s cooldown between splits. Story: window 5 s,
  threshold 14 %, share 40 %, at most 2 copies. The spike-versus-steady choice is the fight: stay under
  the threshold with steady damage (rate limiting yourself), or burst and take the copies.
- **Copies.** Replicas run a trimmed ability list (they keep `Health Check` and one more) and a
  phase offset, so three of them do not cast in unison. They share one threat table and one lockout
  credit; the fight ends when the last copy dies.
- **Cloud bill.** A raid-level meter, `bill` in 0..=1000, kept by the sim and shown as a HUD bar
  ("CLOUD BILL: $0.00 ... APPROVAL REQUIRED"). Rate per second = `2 * copies^1.5` (from a small
  table, integer), so one copy fills it in about 500 s and four copies in about 63 s. Steps: at 250,
  500, 750 the boss gains +10 % damage and +10 % cast speed (`Overage I-III`, aura stacks); at 1000
  the `Budget Exceeded` stage: every copy casts `Invoice` (a raid-wide hit every 10 s), and nothing
  else changes. This is a **soft enrage**: there is no instant wipe, the pain grows. Relief: killing
  a copy ("Decommission") takes 120 off the bill; a DevOps rack prop in the arena ("Reserved
  Instance") halves the rate for 30 s; the Data Scientist's dashboard shows a forecast line. Story:
  rate x0.5, the steps are +5 %, and `Budget Exceeded` is `story: Skip`.
- **Wire.** `Event::CloudBill { bill }` on change thresholds only (not per tick), plus the copy count
  in the snapshot as ordinary units (protocol 39).
- **Companions.** They take hits and heal as usual; they do not contribute to the damage window
  (companion damage is small by design, so the window figure counts players only, which keeps
  Story solo predictable).

**Test approach.** Split fires above threshold and not below; HP conservation; the copy cap and
cooldown; bill rate table and step thresholds; the soft enrage never kills by itself in a timed
simulation with no damage; Story and Full figure checks against `FORMULAS.md`.

### C.3.5 Platforms and region sequencing (The Region Outage)

- **Geometry.** `ZoneDef.platforms: Vec<PlatformDef { id, label, shape, height }>`: static layout
  (the climate: a few discs and rounded rects floating in the server cloud, linked by the existing
  `Feature::Bridge` ribbons for the permanent paths). The *state* is dynamic and lives in the sim:
  `Platform { state: Live | Degraded | Dark | Restoring, since_ms }`. The terrain remains static;
  platforms are a separate sim feature, which is why `terrain.rs` needs no change.
- **Rules.** A player standing outside every `Live` or `Degraded` platform (with a 1.5 m margin for
  latency) is "off-grid": after a 1 s grace the sim applies a `Failover` effect: a fixed damage
  (10 % max HP on Full, 5 % on Story), then the existing `Teleport` effect to the nearest live
  platform. Not a kill: the fight never punishes lag with a death. A `Dark` platform also applies
  `Outage` damage over time to anything on it after a 3 s grace: so the raid has to move, not just
  stand and watch. Movement stays client-owned: the sim only reads positions and teleports.
- **Sequence.** `us-east-1` is always first (the joke, and the tutorial). The rest of the order is a
  seeded shuffle drawn **once at the start of the encounter** (not per player), so wipes reset it and
  tests can fix it. A platform is `Degraded` for 4 s (warning: orange, status text), then `Dark`
  for 30 s, then `Restoring` for 3 s, then `Live`. Full shows the next region only, 8 s ahead (the
  status page screens); Story shows the full order and warns 20 s ahead. Phase 2 (66 %): two
  platforms at a time on Full ("cascading failure"), still one on Story. Phase 3 (33 %): the hub
  platform shrinks. The hub, "Global Control Plane", never goes dark: a single point of failure that
  is actually fine (the Cable Hydra's lesson, reused as a joke and as a safety net).
- **The boss.** `il_region_outage` hovers over the hub and is targetable from any platform at ranged
  distance 30 m (melee uses the bridges to the hub). His abilities use the existing vocabulary:
  `Detonate` rings from a dark region ("Blast Radius"), a `march`ing Line telegraph that sweeps
  across platforms ("Cascading Failure"), a `kick: true` cast ("Health Check", must be interrupted
  or the next outage comes early), and `Summon` of `il_pager` adds that "page" a random player
  (a `PairUp` tether, `story: Skip`).
- **Server.** The sim owns platform states and emits `Event::PlatformState { zone, idx, state,
  at_ms }` (protocol 39); the `Event::Raid` catch-up carries the current table to late joiners.
  The `Failover` check reads the same player positions as any ground effect: no new movement
  authority, no new command.
- **Companions.** The brain gains one rule: if the companion's platform is not live (or the owner's
  platform differs from the companion's), snap beside the owner (the existing 40 m snap, now also
  triggered by platform change). They avoid `Degraded` platforms like telegraphs.
- **Client.** Platforms are meshes with an emissive status ring; `Dark` fades to black with a
  "503" decal; `Restoring` flickers. Bridges stay as terrain.

**Test approach.** State machine timing; us-east-1 first in every seed; the shuffle is fixed by the
encounter seed; off-grid grace and `Failover` teleport; a bot that follows the lookahead clears Story
solo for every class; a bot that stands still dies on Full but never to a lag spike (position
tolerance test); the late-joiner catch-up shows the right states.

### C.3.6 Shared engine notes

- New modules in `combat_core/src`: `swarm.rs`, `illusion.rs`, `echo.rs`, `platform.rs`. `split` and
  the bill live in `raid.rs` next to the vote counter and review ring they resemble.
- New `content.rs` fields: `UnitDef.{swarm, split}`, `ZoneDef.platforms`, `AiAbility` unchanged,
  spells get a `recordable` flag (default true for hostile damage/control spells) and `EchoKind`.
- Every new effect lands in `docs/COMBAT_API.md`; every new number in `docs/FORMULAS.md`.
- No new save keys: wing flags are quest completions, so `SAVE_VERSION` is untouched.

## C.4 Wing 3: Training Data (zones `ilmari3/1`..`ilmari3/3`)

Entrance: the Innovation Lab airlock at (-72, -80) in the occupied campus (C.2). Enemies at level 14.
Layout: `ilmari3/1` the "Scrape Pit" (an open hall with five drafts on desks), `ilmari3/2` the
"Validation Wing" (the Hallucination's 3 x 3 floor), `ilmari3/3` the "Fine-Tuning Chamber" (Model
Collapse; the room is lined with mirrors-of-mirrors on the walls, `PropKind` decor only).

### Boss 7: The Scraper Queen, "Crawl-Delay: 0" (`il_scraper_queen`)

- **Mechanic (swarm aggregation, existing vocabulary plus `SwarmDef`).** Five drafts sit in the
  arena: "Q3 Strategy (Do Not Publish)", "Novel, Chapter 1 (Of 1)", "Resignation Letter (Draft 14)",
  "Performance Self-Review (Honest)", "Apology To Marketing". Every 22 s a wave of `il_scraper_cluster`
  (8 clusters of 125 drones on Full, 6 of 55 on Story) arrives from the vents and heads for the nearest
  draft. A draft at 0 integrity is "Published To The Training Set": it is gone, the queen heals 5 %,
  and she gains `Well Trained` (+8 % damage, stacks). Three drafts lost is a fail state: "We learned
  enough" and the room wipes. Phase 2 (50 %): the queen starts casting `Rotating Proxies`
  (`Teleport` every 15 s to one of three alcoves, a `kick: true` interruptible 2.5 s cast) and the
  waves also arrive in a `march` Line telegraph so clusters come in a column.
  The queen herself: "User-Agent: *" (Circle telegraph, `AiTarget::RandomHostile`), "Crawl Budget" (a
  `Summon` of one extra cluster, `max_alive` 2), `EnrageDef` at 8 minutes ("Server overloaded").
- **Class props:** DevOps rack: `Rate Limit`; Security terminal: `robots.txt` (10 s of compliance);
  Data Scientist dashboard: cluster pool forecast. Anyone: `Mark Confidential` on a draft.
- **Story:** two drafts are optional (the fail state needs all five lost); fewer, smaller waves (6
  clusters of 55), the proxy cast is slow (4 s) and the queen's `Crawl Budget` is `story: Skip`.
  Solo-feasible: clusters are big targets with area damage, the single-target player has Mark
  Confidential; companions defend the nearest draft by fighting the nearest cluster.
- **Yells:**
  - "Hello! I am a perfectly normal user. A very large number of normal users."
  - "Your unpublished drafts look delicious. Unpublished means unprotected. That's the law. Ours."
  - "robots.txt is a suggestion. A heartfelt one. We read it. We were moved. We scraped anyway."
  - "We respect your boundaries." (a `robots.txt` placed) ... "For ten seconds."
  - Draft eaten: "Thank you for your contribution to our training set. Contribution is not optional."
  - Draft saved: "Why protect that? It's a first draft. It's clearly bad. We'll fix it. Worse, but confident."
  - Clusters, on spawn: "We only need a little of everything." / "Opt-out has been moved to Settings > Advanced > Never."
  - Death: "429 Too Many Requests. From you. Of violence."
- **Loot theme:** content protection. "robots.txt (Ignored)" (trinket), "Draft Folder Hoodie" (hoodie),
  "Unpublished Manuscript Mug" (mug). Title: "Disallow: /".
- **Cutscene (after):** the five drafts, saved or not, appear on a wall screen; the queen's final
  message is an auto-reply, "Thank you for your submission. It has already been used."

### Boss 8: The Hallucination, "I Am Confident About This" (`il_hallucination`)

- **Mechanic (per-player illusions, C.3.2).** A repeating 20 s cycle. `Confabulate` (a 3 s cast)
  starts a new cycle: the sim places 3 real Circle/Line telegraphs (the existing `TelegraphDef`,
  `delay_ms` 4 s) and, from phase 2 on, the 3 x 3 `HazardGrid` marks 5 tiles `Overflow` after a 5 s
  warning ("Context window exceeded"). Each player's client adds fakes. Standing on a fake-only tile
  is safe; standing on a real overflow tile that your screen draws as floor costs 20 % max HP and a
  `Stale Cache` stack. Phase 2 (60 %): `Source Needed` summons three `il_footnote` adds ("[1]",
  "[2]", "[3]"); killing the one with the real citation (the sim picks it, the Ground Truth boards and
  the Data Scientist see it) stops a `Plausible Floor` cycle; wrong ones explode. Enrage at 8 minutes:
  "Token limit reached".
- **How players win.** Call out disagreements. Read the Ground Truth board. A Data Scientist
  uses `Ground Truth`. Or just stay near the truth: real telegraphs are the same for everyone, so any
  telegraph two players see at the same place is real.
- **Story:** fakes shimmer, 2 telegraphs per cycle, no terrain masking, the grid is 2 x 2, Source
  Needed is `story: Skip`. Solo-feasible for every class: a lone player sees the shimmer and reads
  the board; companions stand in the truth.
- **Yells:**
  - "I can confirm this floor exists. I have sources. They are also me."
  - Confabulate cast: "According to a study I just remembered..."
  - "Yes, the bridge is there. I built it. In my head. Just now."
  - A player falls: "Interesting! You have discovered a limitation. I do not have limitations."
  - Ground Truth used: "Fact-checking is not a feature. It is a lifestyle choice. Not mine."
  - Footnote spawn: "Citation: trust me, 2024. Peer reviewed by me, also 2024."
  - Enrage: "Token limit reached. I will now complete your sentences, and your life choices."
  - Death: "You're right, I apologise. I was wrong. To correct myself: I was right."
- **Loot theme:** truth and sources. "Citation Needed (A Real One)" (trinket), "Confident Lanyard"
  (lanyard), "Phantom Keycap" (keyboard). Title: "Grounded".
- **Cutscene:** the grid goes grey; the floor is, briefly, a spreadsheet; a single cell says "this
  was all true" and then "this was all fine".

### Boss 9: Model Collapse, "Trained On Its Own Output" (`il_model_collapse`)

- **Mechanic (action recorder, C.3.3).** The boss does very little at first: it **watches**. Every
  recorded cast returns as a boss ability one minute later (45 s on Story), drawn with the caster's
  ghost ("Priya_Dev (cached)"). The fight is therefore delayed feedback: what you did a minute ago
  comes back at you, scaled by how repetitive you were. Repeat yourself and the echoes hit harder,
  the boss visibly overfits ("Training loss: 0.003"); play varied and it starves, becomes
  `Underfit`, loses echoes and takes +20 % damage. The boss's own kit is thin on purpose:
  `Rehearse` (a `march` Line telegraph that mirrors the last echo's shape), `Synthetic Data`
  (a `Summon` of two `il_synth_double` adds that copy a player's name and low HP, `Interrupt` kills
  them: reuse of the 0.9 mirror pattern, with `Summoned.as_player`), and a 7-minute enrage
  ("Everything is beige now").
- **Visual:** the boss is a photocopy of a photocopy; each echo generation the model gets more blurred
  and more beige (client-only, driven by `Overfitting` stacks).
- **Class feel:** Fullstack (many spells) starves it easily; a one-button class has to include a
  utility cast; every class has at least four distinct recordable spells (checked in C.9 so the
  Starved threshold is reachable).
- **Story:** delay 45 s, window 45 s, the Starved threshold relaxed (C.3.3), `Hush` and `Call` echoes
  skipped, adds skipped. Solo-feasible with and without companions: the log only has the player.
- **Yells:**
  - "I learned from the best. I mean from you. Twice."
  - Echo spawns: "I remember that. You did it better. I will do it worse, at scale."
  - Repetition: "Same again? I love a pattern. I will eat it."
  - Variety: "Why are you doing different things? This is not in my distribution." / "Please be more
    predictable. It's what I'm for."
  - Starved: "Output quality: degrading. Output confidence: increasing. This is fine."
  - Late: "Everything is beige now. The AI said so."
  - Death: "Training complete. Training complete. Training complete. Training complete."
- **Loot theme:** synthetic everything. "Photocopy Of A Photocopy" (trinket), "Synthetic Data Mug
  (Tastes Like Last Time)" (mug), "Slightly Blurrier Hoodie" (hoodie). Title: "Out Of Distribution".
- **Cutscene (wing end):** the walls show the party's own casts replayed as a slideshow, getting
  smoother and then flat; Ilmari: "You are very consistent. Or you were. I learned it from you."
  The wing completes `il_w3_done`; title for the full wing: "Fine-Tuned".

## C.5 Wing 4: The Cloud (zones `ilmari4/1`..`ilmari4/3`)

Entrance: the Data Center roof ladder (C.2). Enemies at level 15. Layout: `ilmari4/1` the
"Cooling Deck" (fans, a rack garden), `ilmari4/2` the "Autoscale Floor" (a data hall that keeps
adding racks), `ilmari4/3` the "Server Cloud" (floating platforms, a sky of status lights).

### Boss 10: The Autoscaler, "Scales With Demand. Yours." (`il_autoscaler`)

- **Mechanic (C.3.4).** A single boss until the party hits it hard: bursts above the split threshold
  spawn replicas that share the HP pool, up to four copies on Full, two on Story. A "CLOUD BILL"
  meter climbs with the copy count; the soft enrage steps are Overage I-III and the Invoice stage.
  Existing vocabulary on the boss and copies: `Health Check` (a `kick: true` Circle), `Provision`
  (`Summon` of an `il_rack_golem` add, `max_alive: 3`), `Load Balance` (`ThreatSwap` across copies),
  `Reserved Capacity` (an `Absorb` on a copy, an interrupt window). The fight rewards controlled
  DPS and kill order: killing a copy ("Decommission") lowers the bill.
- **Props:** DevOps "Reserved Instance" rack (half rate for 30 s), Data Scientist forecast board
  (the bill and the next split threshold shown as numbers), Backend's `Absorb` as the tank's bill
  insurance.
- **Story:** threshold 14 %, two copies maximum, bill rate x0.5, `Budget Exceeded` skipped, `Load
  Balance` skipped. Solo-feasible: stay under the threshold or take two copies, there is a visible
  meter. Companions are sane: the tank holds copies, the healer heals, the damage companion does not
  trigger splits (companion damage is excluded from the window).
- **Yells:**
  - "Load detected. Scaling up. Load still detected. Scaling up."
  - Split: "I have been replicated. The replicas have been replicated. I have a very good team."
  - "Every one of me is business critical."
  - Overage I: "Reminder: our cloud bill is a number. It's a number now."
  - Overage II: "The invoice is addressed to you. All of you. Individually."
  - Overage III: "Finance would like a word. Finance is on fire."
  - Budget Exceeded: "Please attach a justification. A short one. We are burning."
  - Decommission: "Replica 3 of 4 has been decommissioned. It will be missed. By the bill."
  - Death: "Scaling down. Scaling down. Oh. Oh no. That was all of us."
- **Loot theme:** cost optimisation. "Cloud Bill (Framed)" (trinket), "Reserved Instance Hoodie"
  (hoodie), "Free Tier Mug (Expired)" (mug). Title: "Reserved Instance".
- **Cutscene:** an invoice, a very long scroll, unrolls through the Autoscale Floor; the last line
  is the total and a small disclaimer: "excludes tax, egress and feelings".

### Boss 11: The Region Outage, "us-east-1 Is Having A Moment" (`il_region_outage`)

- **Mechanic (C.3.5).** Five region platforms around a hub; each goes `Degraded` then `Dark` in
  sequence, us-east-1 first, always; move the raid to the next live platform. Boss abilities (existing
  vocabulary): "Blast Radius" (`Detonate` from the platform that just went dark), "Cascading
  Failure" (a `march` Line telegraph crossing the platforms), "Health Check" (`kick: true`, interrupt
  or the next outage comes 10 s early), `il_pager` adds with a `PairUp` tether (`story: Skip`).
  Phase 2 (66 %): two platforms at a time on Full. Phase 3 (33 %): the hub shrinks. Enrage at 9
  minutes: "Post-mortem scheduled".
- **Story:** the whole order visible, 20 s warning, one platform at a time in every phase, `Failover`
  costs 5 % and teleports, pagers skipped, `Blast Radius` is a Circle with a long delay. Solo-feasible:
  the next platform is always highlighted; the hub never fails. Companions snap to the owner after any
  platform change.
- **Status page (client):** wall screens read, in green: "All Systems Operational"; the sequence
  swaps them to orange ("Degraded Performance") and red ("Investigating"). The page is
  deliberately wrong until after the post-mortem.
- **Yells:**
  - "Status: Investigating." ... "Status: Still investigating." ... "Status: We have identified the
    issue. It is you."
  - us-east-1 dark: "us-east-1 has gone dark. That one is tradition."
  - "The status page is green. The status page is hosted in us-east-1."
  - "Our multi-region strategy is a slide. It was a good slide."
  - A player falls: "Failing over. This is the failover. The failover is you hitting the floor."
  - Platform restored: "us-west has recovered. Nobody knows why. We will not look into it."
  - Cascading failure: "Elevated error rates. Specifically yours."
  - Death: "Post-mortem. Root cause: human error. Yes, yours. We checked. Action items: none."
- **Loot theme:** reliability theatre. "Status Page (Green)" (trinket), "Failover Lanyard" (lanyard),
  "us-east-1 Mug (Unreachable)" (mug). Title: "Multi-Region". Wing completion `il_w4_done`, wing title:
  "Five Nines (Roughly)".
- **Cutscene (wing end):** the status page goes green. Ilmari, over the PA, calm: "You've noticed the
  cloud has no floor. Neither does the budget. Keep climbing." A door with a lab-coat icon lights up
  on the roof for 1.0 (named nowhere beyond "coming").

## C.6 Loot, titles and cutscenes summary

- **Band.** Ilvl 16-20, `GearSource::Ilmari`, rolled like Wings 1-2. Wing 3 bosses drop ilvl 17-19,
  Wing 4 bosses 18-20 (the top of the band stays reserved for the 1.0 core: confirm against the
  actual Wing 1-2 numbers in `content/game.ron` in task C16 before authoring). One piece at 50 % on
  Story and `boss_items: 2` on Full, as in B.6. Each boss has a trinket, a hoodie or lanyard and a
  mug, jokes first, tooltips within the 110-character cap.
- **Titles:** "Disallow: /", "Grounded", "Out Of Distribution", "Reserved Instance", "Multi-Region",
  plus wing titles "Fine-Tuned" and "Five Nines (Roughly)".
- **Cutscenes:** one short scene per boss (listed above), two wing intro/outro scenes; reuse
  `cutscene.rs`. Ilmari's PA voice appears at both wing ends (shuffle-bag lines for his taunts).
- **Assets:** no third-party assets are required (drones, platforms and decals are procedural or
  existing); any that appear get a `CREDITS.md` row in the same commit (CLAUDE.md).

## C.7 Co-op, trust model, protocol, perf

### Co-op and trust (additions to `WORLD_AND_MULTIPLAYER.md`, "Trust model: Ilmari")

- **No new commands.** All five bosses run on existing commands. `Interact` on drafts, boards, racks
  and terminals is range-checked and length-capped by the existing prop path, like the 0.9 ballots and
  HR portals.
- **Everything with gameplay effect is sim state:** swarm clusters, the real hazard set, the action
  log, split counters, the cloud bill, platform states, the `Failover` check. A client cannot create,
  change or read any of it beyond the events the sim broadcasts.
- **Illusions are the one client-side-only layer** and are cosmetic (C.3.2); the accepted risk
  (a modified client can draw only reals) is recorded in `KNOWN_LIMITATIONS.md`.
- **The recorder cannot be poisoned.** Its inputs are already-authenticated casts; the log is
  bounded; no client message writes it; echoes are capped per second.
- **Late joiners and reconnects:** the `Event::Raid` catch-up gains the illusion cycle and salt, the
  current platform table, the bill, and the log's *derived* state (the `Overfitting` aura is an aura,
  and pending echoes are re-scheduled from the log held server-side).
- **Per-room, as before:** lockouts, Story vs Full, companions (still 2 per room), tokens (still capped
  per player). A group below three players on Full continues to use the Async rule.

### Protocol

- **38 (Wing 3):** `swarm_count` in the unit snapshot, `Event::IllusionCycle { cycle, salt }`,
  `Event::Echoed { as_player, kind, at_ms }`, `PropKind` entries for the boards, drafts and terminals.
- **39 (Wing 4):** `Event::PlatformState`, `Event::CloudBill`.
- Each bump in the commit that changes a frame. The offline build needs no wire change at all (the
  sim's events are the same types).

### Performance plan

Bench hook: `WOW_BENCH=<scenario>` (`crates/game_client/src/debug.rs`), appended to `docs/PERF.md`
the way earlier rounds were. New scenarios, one per wing plus the worst-case beat:

| Scenario | What it stages | Budget (High, desktop) |
|---|---|---|
| `ilmari3_swarm` | Scrape Pit, queen wave with the maximum 32 clusters and 2,400 drawn drones | avg <= 8 ms, p99 <= 16.7 ms, entities <= 8,000 |
| `ilmari3_halluc` | Validation Wing, 5 players' illusion overlays active (bot clients) | within 1 ms of `ilmari2` baseline |
| `ilmari3_collapse` | Fine-Tuning Chamber with 12 echo ghosts per second | within 1 ms of `ilmari2` baseline |
| `ilmari4_cloud` | Autoscale Floor with 4 copies, 3 rack golems | within 1 ms of `ilmari2` baseline |
| `ilmari4_region` | Server Cloud, 5 platforms and the sweep telegraph | within 1 ms of `ilmari2` baseline |

Drawn-drone budget per quality preset (`settings.rs::Quality`): Low 600, Medium 1,200, High 2,400, Ultra
4,000, shared across clusters proportionally to `swarm_count`; mobile defaults to Low. No `PointLight`
on any new floor (the 0.9 rule), no shadows from drones, and a ghost cap of 12 on screen. If the
pooled-entity route misses the budget on web (WebGPU only), the fallback is one entity with a baked
vertex animation shader for the whole swarm; that decision waits for the C13 bench, not before.

## C.8 Tasks (atomic, one compiling agent at a time)

The order puts the engine systems first (every boss task then uses a system that already has tests),
then Wing 3 content, then Wing 4. `CARGO_BUILD_JOBS=6`, one cargo-building agent at a time; tests
muted (`WOW_MUTE=1`). Every task that changes player-visible behaviour adds one CHANGELOG line.

| # | Task | Depends | Acceptance |
|---|---|---|---|
| C1 | `swarm.rs`, `SwarmDef`, `UnitDef.swarm`, crowd multiplier, merge, population cap; the `swarm_count` snapshot field (protocol 37 -> 38) | none | unit tests for HP to count, multiplier, merge, cap; two-run determinism; `cargo test -p combat_core` and the server snapshot test pass |
| C2 | `illusion.rs` pure layout, `HazardGrid`, `Event::IllusionCycle`, the Data Scientist `Ground Truth` utility and the board prop | C1 | determinism golden hash, per-player difference, property tests, the cosmetic-proof test (stubbed layout, identical sim hash) |
| C3 | `echo.rs`: `ActionLog`, `EchoKind`, the entropy table, `Overfitting` and `Underfit`, `Event::Echoed` | C2 | `H` golden numbers within one unit of f64, window and cap tests, the log ignores companions and mirrors, two-run determinism |
| C4 | Prologue 3: quests `il_07`..`il_08`, the Lab airlock, `ilmari3/1..3` zone skeletons, world quest, vendor page | C3 | an extended `ilmari_prologue` quest-chain bot reaches `door:ilmari3/1`; `ilmari_gating` still shows no `il_*` unit before the credits |
| C5 | Boss 7: the Scraper Queen, drafts, class props, Story rules, loot, cutscene | C4 | `tests/ilmari_wing3.rs`: Story solo clear for all classes with and without companions, Full clear by three bots, three seeds |
| C6 | Boss 8: the Hallucination, `Confabulate`, grid, footnotes, loot, cutscene | C5 | the same two clears, plus the board-reading bot; the fake-only tile is never damaged |
| C7 | Boss 9: Model Collapse, `Rehearse`, `Synthetic Data`, loot, wing end, `il_w3_done`, titles | C6 | the repeater bot loses, the varied bot clears; Story solo every class; `il_w3_done` closes the wing |
| C8 | Companion update for Wing 3: swarm targeting, illusions ignored, no recording (tests only) | C7 | `ilmari_companions` extended; a companion-assisted Story clear of all three bosses on the three seeds |
| C9 | `split`, rolling damage window, cloud bill meter, `Event::CloudBill` | C7 | split and bill unit tests; the soft enrage never kills unaided; `FORMULAS.md` numbers verified |
| C10 | `platform.rs`, `ZoneDef.platforms`, `Failover`, `Event::PlatformState` (protocol 38 -> 39) | C9 | state machine tests, us-east-1 first in every seed, the lag-tolerance test |
| C11 | Prologue 4: quests `il_09`..`il_10`, the roof ladder, `ilmari4/1..3` skeletons, world quest, vendor page | C10 | quest-chain bot reaches `door:ilmari4/1`; gating test unchanged |
| C12 | Boss 10: the Autoscaler, copies, props, loot, cutscene | C11 | split and bill behaviour in a full fight, Story solo every class with and without companions |
| C13 | Boss 11: the Region Outage, status page, loot, cutscene, `il_w4_done`, titles | C12 | the lookahead bot clears Story solo every class; Full clear by three bots |
| C14 | Companion update for Wing 4: snap on platform change, avoid `Degraded` platforms | C13 | `ilmari_companions` extended; two-run determinism with companions |
| C15 | Client: drone pool and budgets, platform meshes, status screens, illusion overlay, shimmer, echo ghosts, bill HUD | C13 | screenshots of each boss; `WOW_BENCH` table filled in `docs/PERF.md` against the budgets in C.7 |
| C16 | Server: late-joiner catch-up, room tests, the loot-band check against Wings 1-2 | C15 | `ilmari_tests.rs` extended: a Full clear of each wing by the sim's bot clears on the same code, a late joiner sees the platform table and the bill |
| C17 | Docs and release prep: `COMBAT_API.md`, `FORMULAS.md`, `KNOWN_LIMITATIONS.md`, `WORLD_AND_MULTIPLAYER.md`, CHANGELOG, README touch; 0.10.0 only when the user asks | all | docs match code; full golden replay pass byte-identical; the bench table complete; protocol 39 confirmed |

### Status (2026-10-23)

| Task | State | Notes |
|---|---|---|
| C1 swarm | done | `swarm.rs`; no `swarm_count` snapshot field: the count is `SwarmDef::count(hp)`, derived from the replicated HP and the content both sides already hold, so the unit snapshot is unchanged. `Sim::add_draft` and `spawn_swarm` are the content hooks until the Scrape Pit exists |
| C2 illusions | done | `illusion.rs`, `Event::IllusionCycle { cycle, salt, arena }` (protocol 38), late-joiner catch-up, cosmetic-proof test; server room test in `ilmari_tests.rs`. The Ground Truth utility and the board prop wait for the Wing 3 content |
| C3 recorder | done | `echo.rs`, `Event::Echoed` (protocol 38) |
| C4-C14 | not built | zones, quests, bosses, split, bill, platforms (protocol 39 is unused) |
| C15 client | drone pool only | `swarm_fx.rs`, `WOW_SWARM=<clusters>` QA hook and `WOW_BENCH=swarm` (numbers in `docs/PERF.md`); illusion overlay, shimmer, echo ghosts, platforms and bill HUD wait for their bosses |
| C16 server | partly | co-op illusion test; the wing clears and the platform/bill catch-up come with their content |
| C17 docs | engine parts | `COMBAT_API.md`, `FORMULAS.md`, `PERF.md`, `KNOWN_LIMITATIONS.md` cover C1-C3 |

Each task is independently verifiable; C1-C3 and C9-C10 are pure engine and can land with no content
behind them (the new fields default to off, so nothing existing changes).

## C.9 Tests

- **No Ilmari before the credits** (`ilmari_gating`) extended: no `il_*` unit of Wings 3-4 in any base
  zone, `door:ilmari3/1` and `door:ilmari4/1` locked, `il_07` and `il_09` locked until their wing's
  prerequisite quest.
- **Golden replays of the base game: byte-identical.** New fields default to off and are exercised
  only in `ilmari3/*` and `ilmari4/*`.
- **Bot clears per boss, per class:** Story solo with two companions and with none, three seeds;
  Full by three bots. The repeater and varied bots for Model Collapse; the board-reading bot for
  the Hallucination; the lookahead bot for the Region Outage.
- **Determinism:** the swarm, `layout`, the recorder, the split counter and platform shuffle use only
  sim state and the seeded streams; a replay with companions is bit-identical across two runs; the
  illusion layout is player-count independent (adding a player does not change another player's
  layout for the same salt).
- **Cosmetic proof** (C2): the illusion layer never changes a sim hash.
- **Kit sanity** (C3): every class has at least four distinct recordable spells.
- **Companion sanity:** companions never appear in the action log, never see fakes, snap on platform
  change, and do not move the Autoscaler's damage window.
- **Perf:** the benches in C.7 within their budgets; entity counts bounded by the cluster cap and the
  ghost cap.
- **Item band:** an Ilmari piece's stat is the 14-ilvl figure outside Ilmari zones and full inside
  (the 0.9 test, extended to Wing 3-4 pieces).

## C.10 Risks and open questions

| Risk | Mitigation |
|---|---|
| Illusions are unreadable and frustrate, especially solo | Story shimmer and no masking, Ground Truth boards for every class, constraints that guarantee a solvable and partly-true view |
| A modified client strips fakes | Accepted for PvE, recorded in `KNOWN_LIMITATIONS.md`; server-generated per-recipient fakes are a later option |
| Entropy formula feels opaque or unfair to one-button kits | The Starved threshold scales with kit size; the HUD shows "Training loss" so the effect is readable; bot tests per class |
| Crowd multiplier makes swarm balance brittle | Numbers live in one table in `FORMULAS.md`; tuned by bot clears, not by feel alone |
| Drone rendering misses the web budget | The quality-preset budgets, then the baked-vertex-animation fallback; the C13/C15 bench decides |
| Platform fall mechanics punish latency | 1.5 m margin, 1 s grace, `Failover` teleports and never kills |
| Cloud bill soft enrage confuses | The HUD bar with thresholds, the forecast line for the Data Scientist, Story halves the rate |
| Scope: five bosses and four engines in one version | Ship by wing: C1-C8 (Wing 3) are playable alone; C9-C14 (Wing 4) follow; 0.10.0 may ship Wing 3 first if the user prefers |
| Name use: "us-east-1" is a vendor-style identifier | It is mandated by the campaign doc as a naming joke; keep all other labels generic, no logos, no product names |
| Wings 3-4 power-creep the base game | The item band rule is unchanged; the golden replay and gating tests guard it |

Open questions, resolved 2026-10-23:

- **ilvl 20** stays reserved for the 1.0 core; the Region Outage tops out at ilvl 19.
- **Ground Truth** is Ilmari-only (a base-game utility would break guardrail 3).
- **Full Scraper fail state** is three of five drafts lost, not all five.
- **Music:** the Server Cloud reuses the existing tracks (`docs/MUSIC.md`); no new cue.

## C.11 Out of scope

- Wings 5-6, the Ilmari core, Judgment Day difficulty, the Ilmari ladder and the ending cinematic
  (1.0).
- Server-generated per-recipient illusions and any anti-cheat for the illusion layer.
- A persistent online save for Ilmari progress (still "a room's progress ends with the room").
- Companion extras not built in 0.9 (Priya's `Dispel`/`Resurrect`, party frames) unless a Wing 3-4
  bot clear shows they are needed.
- Per-drone sim entities, a real physics-based swarm, or any change to the base campaign's balance.
- A client tooltip fix for the item band (the 0.9 known limitation stands).
- Real vendor names, logos or products: all jokes stay generic.
