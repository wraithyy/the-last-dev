# Campus rework: room to breathe

Status: proposal, nothing implemented. Written 2026-10-22.

Problem statement (player feedback): the campus feels overcrowded. A dungeon door was built on top of
the dailies scrum board, and "there are fifty things slapped next to each other". This document
measures where the crowding is, proposes where things go instead, and splits the work into atomic
tasks that one compiling agent can execute one at a time.

All numbers below come from `content/zone_geometry.ron` (campus section, committed copy) and
`content/game.ron`, parsed with a throwaway script. The committed `zone_geometry.ron` is stale
against the working tree (see "Stale geometry" below), so the numbers describe the last exported
state; the picture does not change.

## Decisions (2026-10-22)

- Grow now: Phase B is no longer optional. The campus grows to about 520 x 360 m in the same
  rework and the moves below use the extra room; M5 runs as part of the plan, not as a fallback.
- Ambient extras (`live/ambient.rs`) get a small grey name tag on mouse hover, client-only, no
  health bar and no targeting, so they read as people rather than broken NPCs.
- Revisit the buildings and entrances while moving them (VibeCon, the Offsite shuttle, the
  dungeon doors, the minigame hub): each entrance should look like what it leads to and be
  readable from a distance (a conference hall with banners and a queue for VibeCon, a real
  loading dock for the Migration, an executive entrance for the QBR, a hacker loft for the
  Hackathon), built from the Kenney kits in `assets/models/` where they fit. Screenshot every
  entrance from its approach before and after.
- The same pass covers the three landmark buildings, the Tower, the Data Center and the
  Innovation Lab: their exteriors and forecourts get room to breathe and read as landmarks,
  and their interiors get a declutter pass (same density gate idea per floor: no stacked
  interactables, clear walking lines, props grouped into purposeful corners instead of spread
  everywhere). Keep each zone's jokes; cut duplicates, not character.
- Tower exterior direction (from a reference photo the user shared, used only as loose
  architectural inspiration, never its branding or logo): two or three overlapping cylindrical
  drums of different heights instead of one box; a dense rhythm of tall vertical metal fins over
  dark glass, broken by thin horizontal floor bands every few storeys; the tallest drum in the
  middle with a roof edge of antennas and rooftop greenery; the building sitting on a grassy
  mound with young trees, a curved footpath and street lamps along the road in front. Our own
  sign on the top band (e.g. the company name or "AI-FIRST"), no real company names. Build it
  from cylinders and thin boxes (fins as instanced thin boxes on a ring), keep the draw count
  in check (merge fins per drum band where possible) and check `WOW_BENCH=campus` before and
  after.
  A second view of the same reference adds: the drums are rounded-rectangle (stadium) plans,
  not perfect circles, standing in a cluster of four or five of different heights linked by
  narrow dark-glass connectors; each drum's floor bands step slightly in and out so the stack
  looks gently offset; one drum is lifted on a ring of slanted V-shaped columns over an open
  ground-floor plaza (a good main entrance: walk in under it); some roofs carry small trees.
  Hard rule: the result must NOT be recognisable as that real building at first glance. Take
  the vocabulary (rounded drums, vertical fins, floor bands), not the composition: change the
  number and arrangement of drums, their proportions and heights, the fin colour (e.g. warm
  bronze or white instead of steel), the entrance treatment and the landscaping, and mix in
  elements of our own (a giant sign, a rooftop "AI-FIRST" beacon, an unfinished crane) so it
  reads as the game's tower, not a copy.

- Spread the core (decided after wave 1): the Tower, the Data Center, the Innovation Lab and
  VibeCon with the plaza between them sit within about 130 x 55 m. Spread them so the core
  covers at least the footprint of Car Park P3 (120 x 170 m, about 20,000 m2), for example
  around 220 x 130 m, with open squares, lawns and paths between the buildings instead of
  walls of props. Grow the map again as needed (about 640 x 440 m) and keep the outer
  districts' relative layout. Car Park P3 itself stays a separate zone; only its size is the
  yardstick.
  Done (wave 1b): no growth was needed. The Tower stays at (0, -36); the Lab moved to (-72, -80)
  (Innovation Green), the Data Center to (98, -84) with the Migration dock on its east face, the
  client annex to (62, -102) and the account manager to (70, -96) (Data Square); VibeCon stays at
  about (132, -17). The core bounding box (Lab west edge to VibeCon east wall, Data Center and
  Lab north edges to the graveyard) is about 220 x 145 m, 32,000 m2, against 130 x 55 m before.
  The extra room came from the empty Old Campus band between the north avenue and the north
  road, so the outer districts did not have to move. Two new districts name the squares.

- Co-op: when the rework is done (after wave 2), bump `PROTOCOL_VERSION` in net_proto so old
  clients and the old server (with the old campus geometry) refuse each other cleanly, then
  check the Render server reports the new version after the deploy.

## 1. Measurements

### 1.1 The map is large; the crowding is one spot

- Campus bounds: `campus::HALF_X/HALF_Z` = 232 x 166 (464 x 332 m, 154,000 m2); `FLOOR_HALF_X/Z` in
  `config.rs` = 234 x 168 (outer clamp shared by every floor, also sizes the test `Grid`).
- Anchors in the campus: 181 (55 props, 17 doors, 3 secrets, 2 events, ~100 NPC / pack / night / rare
  spawn points). Interactables (everything except packs, night patrol points, rares, spawn,
  graveyard): 117.
- Blockers: 585 AABBs, 12,562 m2 footprint (about 8 percent of the area), most of it buildings.
- 20 x 20 m grid: 113 cells hold at least one anchor, of roughly 390 cells in the bounds, so about
  70 percent of the map has nothing in it. That is the opportunity: there is plenty of room, it is
  just not used.
- Cells by anchor count: 81 cells with 1, 17 with 2, 9 with 3, 3 with 4, then 6, 8 and 13. The tail
  is the problem.

### 1.2 Top crowded 20 x 20 m cells

| Cell (x, z) | Anchors | Contents |
|---|---|---|
| [-20,0) x [0,20) | 13 | npc_colleague, npc_1, npc_6, scooter_1, sprint_board, company_calendar, 7 minigame doors |
| [0,20) x [-20,0) | 8 | npc_hr, npc_rto, npc_comms, bike_rack, scooter_3, scooter_4, docs_intern secret, sprint_terminal |
| [0,20) x [0,20) | 6 | npc_5, door:onboarding/1 (old position), scooter_2, office_dog, scrum_board, scrum_bot |
| [-40,-20) x [-20,0) | 4 | rto_desk_1, rto_desk_3, npc_prod, npc_oncall |
| [40,60) x [20,40) | 4 | pack_6, npc_exit, smoking_bench, npc_4 |
| [-40,-20) x [0,20) | 4 | npc_3, npc_expense, npc_hybrid, coffee_campus |
| 8 more cells | 3 each | rto_desk_2 + night_1 + pack_3; hydration pair + pack_13; station board + vending + shuttle; hot desks + coworking coffee; shuttle_gate + pack_44 + scooter_8; shuttle_plaza + graveyard + billboard; pack_21 + npc_cowork + night_9; player_spawn + npc_qa + ai_car |

Radial view from the plaza centre (0, 10):

| Radius | Anchors inside | Interactables inside | Area | Per 100 m2 |
|---|---|---|---|---|
| 20 m | 21 | 20 | 1,257 m2 | 1.6 |
| 30 m | 38 | 37 | 2,827 m2 | 1.3 |
| 40 m | 52 | 49 | 5,027 m2 | 1.0 |
| 60 m | 70 | 57 | 11,310 m2 | 0.5 |
| whole map | 181 | 117 | 154,000 m2 | 0.08 (interactables) |

Within 40 m of the arrival there are 49 interactables, roughly 13 times the campus average, and only
three enemy anchors (pack_5, pack_8, night_5): the crowd is friendly clutter, not combat.

Nearest-neighbour distance among the 117 interactables: median 6.2 m, 41 of them (35 percent) have
another interactable within 5 m, 73 (62 percent) within 8 m. A 7 m/s player crosses 8 m in about a
second, so a third of the campus is "everything is next to everything".

### 1.3 Coincident and near-coincident anchors (< 3 m)

45 pairs. Grouped:

| Distance | Pair | Verdict |
|---|---|---|
| 0.0 | 7 minigame doors (`teambuilding/1, review/1, reorg/1, migration/1, hackathon/1, openspace/1, home/1`) all at (-6.5, 14.2) | Intended, see 1.4. |
| 0.2 | `prop:company_calendar` (-6.5, 14.0) with those 7 doors | Same cluster. |
| 0.2 | `door:onboarding/1` (9.0, 14.2) vs `prop:scrum_board` (9.0, 14.0) | Bug in the committed geometry: the dungeon door sits on the dailies board. Already fixed in the working tree (`campus.rs` `starters_door`, X = -22), geometry not re-exported yet. |
| 2.5 | `door:onboarding/1` vs `scrum_bot`; `prop:scrum_board` vs `scrum_bot` (2.5 m) | Same spot, same fix; the bot belongs to the board and may stay beside it. |
| 1.25 / 2.87 | `npc_4` and `npc_exit` vs `prop:smoking_bench` | Intended pairing (smoking corner), but it is also 18 m from pack_6. |
| 1.5 | `npc_1` vs `prop:scooter_1` (-19, 19) | Scooter parked on a quest giver. |
| 1.8 | `prop:rto_desk_2` vs `night_1` | Night patrol point on a desk. |
| 2.0 | `scooter_4` vs `sprint_terminal` | Two interactables in one footprint. |
| 2.6 | `npc_hybrid` vs `coffee_campus`; `npc_6` vs the calendar | Giver standing at a prop. |

Second-order stacking, found by reading code and not by the 3 m rule:

- The new onboarding door at (-22, 12.2), anchor at (-22, 14.2), is 8 m from `player_spawn`
  (-23, 22), 6 m from `npc_1`, and in the same 20 m cell family as the calendar. Fixing the overlap
  with the scrum board moved the crowd, it did not thin it.
- `world_boss` (the level-10 Board meeting, `game.ron` line 16571, every 30 min) is anchored at
  (37, 15): 6 m from the VibeCon registration door (31, 16) and 60 m from a level 1-2 arrival
  lawn. A level-10 event in the front yard of a level-1 zone is a design smell on its own.
- Doors within 50 m of the arrival: tower (0, -23), onboarding (-22, 14), offsite (7, 34), vibecon
  (31, 16), client annex (32, -7), data center (40, -8), qbr (-20, -30), lab (-50, -23), plus the
  calendar. That is 8 doors in a 70 x 60 m patch; the basement dock (68, -4) makes 9.

### 1.4 The Company Calendar: intended UI

`office.rs::company_calendar` (lines 268-300) registers `door:<venue>` for each minigame
only as "where a minigame group comes back to"; there is no door piece and no trigger, the
calendar window starts the minigame (see the comment above `company_calendar`). So 7 anchors on
one point is not 7 physical doors. It is correct as a data contract, but:

- the geometry export, `door_trigger` tests and any density gate see 7 interactables at once;
- the calendar sits in the busiest cell on the map.

Decision: keep it as one hub (one prop, one UI), exempt shared `door:` anchors that have no trigger
from the density gate by name list, and move the hub out of the arrival crowd (section 3).

## 2. How the campus is built (what a move touches)

- `floors/campus.rs`: HQ core (about 140 x 100 m): tower, data center, lab, annex shells,
  parking lot, pavilion, plaza, the dungeon doors (`starters_door`, `loading_dock_door`,
  `executive_door`, `vibecon_entrance`, `shuttle_stop`), the tree scatter (avoids anchors through
  `districts::keep_clear`). Constants `PACKS`, `NPCS`, `CHAIN_NPCS`, `SPAWN`, `GRAVEYARD`, `PLAZA`.
- `floors/districts.rs`: `DISTRICTS` (12 rectangles, `district_at` drives the minimap subzone
  header), `LAKE` (-150, -5) r 16, `CREEK` polyline, 6 `BRIDGES`, `HILLTOP` (Executive Hill, 7 m),
  `TERRACE`, `ROADS` (17 spans, 6-10 m wide), lamps (~46, light pools only; no real lights on the
  campus, `PERF.md`), `SCOOTERS` (8, plus 4 core), `SHUTTLES` (8 stops), `TREES` (54 clumps),
  `ill_marie` (7.5, 118), `picnic_dedication` (-114, 10).
- `floors/office.rs`: `company_calendar`, `CAMPUS_SCOOTERS`, sprint board and terminal.
- `floors/zones.rs`: tests listed in section 5; contract anchors checked per zone.
- `floors/export.rs`: writes `content/zone_geometry.ron` (anchors, blockers, triggers, arrivals,
  terrain features); the co-op server loads that file for collision and spawn placement. Regenerate
  with `TLD_WRITE_ZONES=1 cargo test -p game_client zone_geometry`.
- `hud/minimap.rs`: radar with zoom steps, `district_at` for the header, `CALENDAR_ANCHOR` pin. No
  static world map labels beyond the district name; no signposts exist in the world.
- Perf (`floors/mod.rs`, `docs/PERF.md`): decor culled by `VisibilityRange` at
  `DECOR_RANGE_OUTDOOR` = 95 m for pieces under 7 m; big pieces (buildings) are never culled; no
  `PointLight` on the campus. Whole-campus bench averages 7.0 ms. Spreading props thins the visible
  set per frame; only the blocker count and total piece count matter for load time.
- Movement: `RUN_SPEED` 7.0 m/s, e-scooter +60 percent (11.2 m/s) out of combat, 12 scooters, 8
  shuttle stops; the Facilities scooter is also what the quest text sells.

Stale geometry: the committed `zone_geometry.ron` still has `door:onboarding/1` at (9.0, 14.2)
while `campus.rs` now builds it at (-22, 14.2). Whoever runs the first rework task must regenerate
before measuring again; the density gate will fail on the committed file until then.

## 3. Proposal

### 3.1 Map size

Two phases, so the size decision is evidence-based:

- Phase A (no growth): 464 x 332 m stays. 70 percent of the cells are empty; most moves go into
  existing districts that have nothing in them. Target is a gate pass (section 6) without touching
  `FLOOR_HALF_*`, `HALF_X/Z`, terrain, or the server clamp.
- Phase B (only if the gate cannot be met): grow to 520 x 360 m (`campus::HALF_X/Z` 260 x 180,
  `FLOOR_HALF_X/Z` 262 x 182), +19 percent area. New ground goes where it buys quiet: a 28 m strip
  on the west (past the Wellness Park hills at x = -200) for the Quiet Meadow, and a 14 m strip
  south of Overflow Lot. Cost: `Grid` in the tests grows 14 percent (memory only), server collision
  clamp follows `zone_geometry.ron` automatically, minimap zoom table and the `keep_clear` tree
  scatter need a once-over. No new districts are required: districts get roomier, not more
  numerous.

Recommendation: do Phase A first. Walking time is not the issue (the longest diagonal is 570 m,
81 s on foot, 51 s on a scooter); legibility is.

### 3.2 Design rules

1. The arrival plaza (about 60 x 50 m around the spawn) carries a maximum of six things: spawn,
   graveyard, the HR quest giver, the tower door, the dailies scrum board with its bot, and the
   Welcome arch. Nothing else, including scooters (one rack at the edge is fine).
2. One landmark per sightline: from the spawn, the tower is the only tall thing; everything else
   is below the tree line or behind the plaza edge.
3. Every dungeon or raid entrance gets an approach: a 12 m radius clear of other interactables,
   a road or path leading to it, a sign visible from 30 m, and (for raids) a themed building.
4. Services in one street (vendors, quest hub, coffee, vending), not scattered along every road.
5. Quest givers grouped per area, one cluster of 3-5 per district, 10 m apart minimum.
6. Green space is a feature: at least one 60 x 60 m field per district with no anchors on purpose.
7. Interactables are 6 m apart, minimum, anywhere on the map; no door on another prop (gate in
   section 6).

### 3.3 District plan

| District | Role after the rework | Entrances and anchors that live here |
|---|---|---|
| HQ Plaza | Arrival and orientation | spawn, graveyard, HR, tower door (Tower), scrum board, welcome arch |
| Services Street (new name for the road x = 20..60, z = 0..45, the lawn east of the arch) | Vendors and quest hub | npc_exit, smoking corner, coffee, account manager, vending, sprint terminal, company calendar, minigame hub |
| Data Center loading dock and annex | Migration and client dungeons | basement dock, client annex, data center door; cleared apron |
| Old Campus | Hybrid-work quests and the QBR approach | RTO desks and their givers, qbr door gets its own forecourt on the tower west face |
| Incubator Street | Conferences and building things | hack24 door, VibeCon registration desk, pitch whiteboard, coworking |
| Wellness Park | Calm; offsite coach bay | offsite coach and door, fire drill, picnic dedication (unchanged, quiet) |
| Executive Hill, Co-Living Commons, Wellness Gym, Garage and Station, Overflow Lot, Food Truck Plaza, South Gate | Unchanged roles | unchanged anchors, only nudged to meet the 6 m rule |

The picnic dedication at (-114, 10) and Ill Marie at (7.5, 118) do not move. Distance from the
picnic to the nearest enemy anchor stays above 40 m; the gate has an explicit rule for it.

### 3.4 What moves where

All coordinates are proposals to be snapped to free ground and checked by the tests in section 5.
"Plaza" is the arrival area around (-10, 20).

| Thing | From | To | Why |
|---|---|---|---|
| `door:onboarding/1` and its wall | (-22, 14.2) (moved from (9, 14.2) already) | Keep at the west end of the plaza, but pull `npc_1`, `scooter_1` and the spawn pack out; clear 12 m | The Onboarding door is the first dungeon a new hire meets; it should be visible and alone. |
| `player_spawn` | (-23, 22) | (-12, 32), facing the tower | 8 m from the starters door is too close; a spawn in the middle of a lawn with the tower in front gives the first screenshot a clear subject. |
| `graveyard` | (-15, 28.5) | (-34, 40) | Release point away from doors; the pull zone and the `door_triggers` test already check that the spawn and graveyard are outside triggers. |
| `prop:company_calendar` + 7 minigame door anchors | (-6.5, 14) | Services Street, (34, 24) beside the smoking corner | Takes the busiest cell off the plaza; the calendar is a menu, not a landmark. Stays one prop, one anchor group. |
| `prop:sprint_board` and `prop:sprint_terminal` | (-14, 10) and (4.8, -17) | Services Street, (40, 20) and (44, 28) | Both are menus; keep with the other menus. |
| Scooters 1-4 | around the plaza, 1.5 m from givers | one rack at (-30, 36); one more at (24, 20) | One rack, not four drive-by scooters. |
| `npc_colleague`, `npc_1`, `npc_6` | plaza | Services Street quest hub at (30..50, 10..40), 8 m apart | Quest givers grouped per area. |
| `npc_hr`, `npc_rto`, `npc_comms`, `bike_rack`, `docs_intern` | (0..20, -20..0) | HR and comms stay by the tower, 10 m apart; RTO and the intern move to Hybrid Row | Thin the second hottest cell. |
| Hybrid Row: `rto_desk_1..3`, `npc_prod`, `npc_oncall`, `night_1` | (-24..-36, -17..-23) | Old Campus west, around (-90, -22) | The desks are quest props, not tower decoration; `night_1` stops standing on a desk. Test `the_hybrid_work_npcs_keep_their_distance_from_other_givers` must still pass. |
| `door:qbr/1` (executive door) | (-19.8, -30), tower west face | Keep, but add a forecourt: remove anything within 12 m (rto desks leave) | Already themed on the tower; only needs space. |
| `door:offsite/1` and the coach shuttle stop | (7, 34) | Wellness Park, trailhead pad at (-124, 74), next to `shuttle_park` (-126, 59.5) | Offsite is a lakeside theme; it takes a 15 x 9 m pad and a coach off the arrival lawn. It must stay 90 m or more from the picnic. |
| `door:vibecon/1` (registration desk) | (31, 16) | Incubator Street, (128, -22) | Conference themed, on the east side; away from the world boss. |
| `door:hack24` (in progress) | not placed | Incubator Street, (108, -28), opposite VibeCon | Hackathon and conference share one street, 20 m apart, each with its own forecourt. |
| `world_boss` anchor (`the_board`, level 10) | (37, 15) | Executive Hill foot, open field at (150, 80), or the Overflow Lot at (-96, 122) | A level-10 event in the level-1 arrival yard is wrong; needs a 40 m open field and is reachable by shuttle. Confirm `game.ron` event refs (`boss: Some(...)` at line 16571). |
| `pack_5`, `pack_8`, `night_5` | within 40 m of the plaza | at least 60 m out | Keeps the arrival safe at level 1. |
| `prop:smoking_bench` + `npc_4` + `npc_exit` | (43..46, 23..24) | Keep, drop `pack_6` (56, 28) 18 m east to (80, 44) | Hub and fight 18 m apart is crowded. |
| Doors: tower, datacenter, client, basement, lab | unchanged | unchanged | They are already on their buildings; add signposts. |

Net effect on the 40 m disc around (0, 10): from 49 interactables to about 14 (spawn side,
tower, HR, scrum board and bot, welcome arch, one scooter rack, graveyard, the starters door and
its wall). Services Street takes about 25 in a 70 x 50 m area: about 0.7 per 100 m2, which is the
densest point of the map by design and is below today's plaza at 1.6.

### 3.5 Wayfinding

- Signposts: a three-arm sign (one `Mat::Wall` post, three `p.sign` panels) at each junction on the
  ring road and the plaza edge. Each arm names a district and the dungeon or service behind it
  ("Services Street 40 m", "Data Center / Migration", "Incubator / Hack24 / VibeCon"). 8 to 10
  posts in total, all non-colliding decoration, no new assets.
- Landmarks: tower (existing), a tall offsite coach pylon at the Wellness Park trailhead, a
  conference banner on VibeCon, a neon `HACK24` bar on the hackathon forecourt; each readable from
  at least 60 m so a player can walk toward it.
- Minimap: add labels for services (vendor, quest hub, calendar) and door glyphs by kind (dungeon,
  raid, minigame) in `hud/minimap.rs`; keep the `district_at` header and add the new sub-district
  names to `DISTRICTS` so the header says "Services Street" or "Hybrid Row". The calendar pin
  already exists; point `CALENDAR_ANCHOR` at the moved prop.
- Paths: lay a 3 m path (asphalt `road` pieces already exist) from the plaza to Services Street and
  to each moved entrance, so a player sees where to go before reading anything.

### 3.6 Travel time and core loops

Walk 7.0 m/s, scooter 11.2 m/s, shuttle (8 stops, instant hop with the "always late" gag).

| From the spawn (-12, 32) to | Distance | Walk | Scooter |
|---|---|---|---|
| Services Street hub (40, 25) | 53 m | 8 s | 5 s |
| Tower door (0, -23) | 56 m | 8 s | 5 s |
| Onboarding door (-22, 14) | 21 m | 3 s | 2 s |
| Data Center door (40, -8) | 66 m | 9 s | 6 s |
| Migration loading dock (68, -4) | 90 m | 13 s | 8 s |
| QBR door (-20, -30) | 62 m | 9 s | 6 s |
| Lab door (-50, -23) | 67 m | 10 s | 6 s |
| VibeCon and Hack24 (Incubator Street) | 150 m | 21 s | 13 s |
| Offsite coach bay (Wellness Park) | 125 m | 18 s | 11 s |

All core loops (vendor, quest hub, calendar, any dungeon door) stay below 25 s on foot, well inside
the 30-40 s budget. The farthest moves (Incubator Street, Wellness Park) are the raids, which are
level-gated anyway; each is 20 m from a shuttle stop (`shuttle_incubator` (100, 44),
`shuttle_park` (-126, 58)).

## 4. Risks and constraints

| Risk | Detail | Mitigation |
|---|---|---|
| Co-op server geometry | The server reads `content/zone_geometry.ron`; stale file means collision against old walls and spawns in the wrong place. | Regenerate in the same commit as every anchor or blocker change; the zone_geometry test fails on drift. Never hand-edit the RON. |
| Saved positions | `combat_core::SaveData` stores class, level, xp, bag, quests, trophies; it has no position or anchor, a load drops the player on the campus spawn. | No migration needed. Saved quest state refers to quest ids, not coordinates. |
| Quests naming anchors | `game.ron` links and quest givers refer to anchors by name (`npc_*`, `prop:*`, `door:*`, `pack_*`). Renaming breaks `every_interactive_objective_has_its_props_units_and_anchors` (reachability.rs) and `every_quest_giver_has_an_npc_anchor` (minimap.rs). | Move coordinates, never names. Rename only with a search of `content/game.ron`. |
| Reachability of kill and loot supply | `reachability.rs` counts supply per zone, not per coordinate, so moved packs stay valid, but `pack_N` index order is used in test constants. | Keep indices; moves only change coordinates. |
| Existing tests | `zones.rs` (contract anchors free and reachable, door triggers reachable and outside spawn/graveyard, packs 14 m apart, 15 m from the spawn, 8 m from the route, blocker budget, dressing), `campus.rs` hybrid givers distance, `campus_districts.rs`. | Run `cargo test -p game_client floors::` and `cargo test -p combat_core` after each task. |
| Spawn move | Pack distance from the spawn (>= 15 m) and graveyard must be rechecked; arrivals (`arrivals` list in the RON: return points from each dungeon) must stay on free ground. | The existing tests cover it; do the spawn task last in its milestone. |
| Perf | A bigger map (Phase B) adds blockers and pieces; decor already culls at 95 m, lights are pools, bench is 7 ms. Spreading props lowers the visible count per frame. | Run `WOW_BENCH=campus` and `plaza` before and after (`PERF.md` recipe), keep the average within 10 percent. |
| PWA / asset size | Props reuse existing primitives and models; no new assets. | None; check wasm size is unchanged. |
| Recent work | Onboarding wall (-22, 12.2), Migration dock on the data center east face, QBR door on the tower west face, Ill Marie desk (7.5, 118), picnic (-114, 10), the hack24 door in progress by another agent. | The rework starts after hack24 lands; the gate treats these as fixed anchors. |
| Several agents compiling | One build at a time (`CARGO_BUILD_JOBS=6`). | Tasks are strictly sequential where they touch Rust; analysis scripts run with `python -I`. |

## 5. Milestones (atomic tasks)

Run one compiling task at a time. Every task ends with `cargo test -p game_client floors::` (and
`cargo test -p combat_core` when `game.ron` anchors or units are touched), then regenerates
`zone_geometry.ron` if anchors or blockers changed. Commits per task; every player-visible change
gets a CHANGELOG line (project rule): the moved doors and the new signposts count, tests do not.

### M0: Baseline and gate (no gameplay change)

#### Task 0.1: Regenerate the stale geometry
- Agent: general-purpose (Sonnet)
- Files: `content/zone_geometry.ron`
- Depends on: hack24 door landed
- Acceptance: `TLD_WRITE_ZONES=1 cargo test -p game_client zone_geometry` exits 0; diff shows
  `door:onboarding/1` at (-22, 14.2); no other anchor changed unexpectedly.
- Prompt seed: Regenerate the zone geometry only; report which anchors moved.

#### Task 0.2: Add the density measurement script
- Agent: general-purpose (Sonnet)
- Files: `scripts/campus_density.py` (new, stdlib only)
- Depends on: 0.1
- Acceptance: `python3 -I scripts/campus_density.py` prints the tables from section 1; numbers match
  this document after 0.1.
- Prompt seed: Parse the campus section of zone_geometry.ron and print 20 m cell counts and pairs
  under 3 m.

#### Task 0.3: Density gate test (warn-only)
- Agent: tdd-guide (Sonnet)
- Files: `crates/game_client/src/floors/zones.rs` (new test)
- Depends on: 0.1
- Acceptance: test `campus_density_gate` runs and prints violations as `eprintln!` without failing
  (a `GATE_ENFORCED` const set to false); `cargo test -p game_client floors::` exits 0.
- Prompt seed: Implement the rules in section 6 as a test that reports but does not fail yet.

### M1: Clear the arrival plaza

#### Task 1.1: Move the minigame hub and menus to Services Street
- Files: `floors/office.rs` (`company_calendar`, sprint board and terminal coordinates)
- Depends on: 0.3
- Acceptance: `door:*` calendar anchors share the new point; `CALENDAR_ANCHOR` pin works; floors
  tests pass; geometry regenerated; calendar test (`combat_core/tests/calendar.rs`) passes.

#### Task 1.2: Move the plaza quest givers and scooters
- Files: `floors/campus.rs` (`NPCS`, `CHAIN_NPCS`, scooter placement), `floors/office.rs`
  (`CAMPUS_SCOOTERS`)
- Depends on: 1.1
- Acceptance: no two plaza interactables within 6 m; givers 8 m apart; floors tests pass.

#### Task 1.3: Move spawn and graveyard, clear the starters door
- Files: `floors/campus.rs` (`SPAWN`, `GRAVEYARD`, `starters_door` clearance), `content/game.ron`
  only if a unit refers to the spawn by coordinates (it should not)
- Depends on: 1.2
- Acceptance: `door_triggers_are_reachable_and_do_not_swallow_the_spawn_or_graveyard` and
  `packs_are_spaced_and_off_the_main_routes` pass; 40 m disc around (0, 10) has at most 15
  interactables (script output).

#### Task 1.4: Push the front-yard packs and the world boss out
- Files: `floors/campus.rs` (`PACKS`), `floors/districts.rs`, `content/game.ron` (`world_boss`
  anchor only)
- Depends on: 1.3
- Acceptance: nearest enemy anchor to the spawn is at least 60 m; `combat_core` world-boss tests
  pass; `the_board` arena open field is 40 m clear.

### M2: Dungeon and raid entrances to their districts

#### Task 2.1: Offsite coach bay to Wellness Park
- Files: `floors/campus.rs` (`shuttle_stop`, `door:offsite/1` position)
- Depends on: 1.4
- Acceptance: picnic dedication to the nearest enemy anchor is still above 40 m (gate rule);
  arrival from `offsite/1` is free ground; floors tests pass.

#### Task 2.2: VibeCon registration to Incubator Street
- Files: `floors/campus.rs` (`vibecon_entrance`)
- Depends on: 1.4
- Acceptance: new 12 m forecourt clear; `vibecon/1` arrival on free ground.

#### Task 2.3: Hack24 door placement
- Files: `floors/campus.rs` or `floors/districts.rs` (wherever the hack24 door lands)
- Depends on: 2.2
- Acceptance: 20 m from VibeCon, 12 m forecourt, door trigger reachable.

#### Task 2.4: Hybrid Row and the QBR forecourt
- Files: `floors/campus.rs` (`rto_desk_*`, `executive_door`), `content/game.ron` only for coordinates
- Depends on: 1.3
- Acceptance: `the_hybrid_work_npcs_keep_their_distance_from_other_givers`,
  `the_hybrid_work_quests_are_interactive_and_covered` pass; QBR approach clear 12 m.

### M3: Wayfinding

#### Task 3.1: Signpost helper and plaza posts
- Files: `floors/kit.rs` (`signpost`), `floors/campus.rs` (call sites)
- Depends on: M2
- Acceptance: 8 to 10 posts, none within 6 m of an interactable; bench `WOW_BENCH=plaza` within 10
  percent.

#### Task 3.2: District names and minimap labels
- Files: `floors/districts.rs` (`DISTRICTS`), `hud/minimap.rs`
- Depends on: 3.1
- Acceptance: `district_at` returns "Services Street" at (40, 25) and "Hybrid Row" at (-90, -22);
  minimap tests pass.

### M4: Lock it in

#### Task 4.1: Enforce the gate, resolve the remaining violations
- Files: `floors/zones.rs`, plus whichever anchor files the gate reports
- Depends on: M3
- Acceptance: `GATE_ENFORCED = true`; `cargo test -p game_client floors::` exits 0.

#### Task 4.2: Perf check and docs
- Files: `docs/PERF.md`, `docs/WORLD_AND_MULTIPLAYER.md` (campus paragraph), `CHANGELOG.md`
- Depends on: 4.1
- Acceptance: bench table updated with before/after; changelog lines under `## [Unreleased]`.

### M5: Optional growth (Phase B), only if the gate stays red

#### Task 5.1: Grow bounds
- Files: `config.rs` (`FLOOR_HALF_X/Z`), `floors/campus.rs` (`HALF_X/Z`), terrain features in
  `districts.rs`, `hud/minimap.rs` zoom table
- Depends on: 4.1
- Acceptance: all floors tests and the server integration test pass; wall blockers sealed
  (`pack_formations_fit_and_the_walls_are_sealed`).

## 6. Density gate (test design)

Where: a test in `floors/zones.rs` that builds the campus layout and reads `l.anchors`, so it
measures what the server will see, not the RON.

Rules (campus only, interactable = anchor whose name does not start with `pack_`, `night_`,
`rare_`, `player_spawn`, `graveyard`, `boss`, `world_boss`):

1. No two interactable anchors closer than 2.5 m, except a documented allow-list:
   `door:*` anchors that have no trigger (the calendar's minigame return points, which must share
   one point) and a pair named in the allow-list with a comment explaining why (for example a
   smoking bench and its two NPCs).
2. At most 8 interactables per 20 x 20 m cell, and at most 15 within 40 m of the spawn (the
   arrival rule from 3.2).
3. Every dungeon or raid door (`door:*` with a trigger that is not a minigame return point) has no
   other interactable within 8 m and no enemy anchor (`pack_`, `night_`, `rare_`, `boss`) within 20 m.
4. No enemy anchor within 40 m of `npc_picnic_wife` and `npc_picnic_daughter` (picnic quiet zone),
   and within 60 m of `player_spawn`.
5. The failure message prints the cell, the names and the distances, so the fix is obvious.

Start with `GATE_ENFORCED = false` (print only, task 0.3), make it blocking in task 4.1. The
allow-list lives next to the test, so adding an exception shows up in review.

## 7. Out of scope

- New districts, new dungeons, new quests or any change to progression and XP.
- New art or audio assets, new models, textures, music.
- Changes to the Company Calendar window, minigames or `content.calendar` data.
- Changing walk speed, scooter speed, shuttle behaviour or adding fast-travel rules.
- Redesigning combat, enemy levels or pack composition (only coordinates move).
- Interiors of the dungeons (`tower/*`, `basement/*`, `qbr/*`, ...) and Car Park P3.
- Terrain rework (hills, lake, creek, bridges), except Phase B edge strips.
- Online-room rules, net protocol or the save format.
- Moving or changing Ill Marie, the picnic dedication or their quest content.
