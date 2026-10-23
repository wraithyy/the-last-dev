# Ilmari 0.9: implementation spec

Status: planned, nothing built yet. Written 2026-10-22. Companion to `docs/ILMARI_RAID.md` (campaign
design and guardrails, which still win in any conflict) and `docs/DUNGEONS.md` (the engine vocabulary
this spec reuses). Coordinates for the campus use the layout of `docs/CAMPUS_REWORK.md`.

Two parts, shipped in this order:

- **Part A, "Ill Marie, more often"**: base-game flavour, ships first as 0.8.x (patch release when the
  user asks for one). Guardrail 1 allows it because it is flavour only.
- **Part B, Ilmari 0.9**: Opening, Prologue phase, Wings 1-2 (six bosses), Story and Full difficulty,
  companion AI, zone phasing.

Hard rules inherited from `ILMARI_RAID.md`: nothing about Ilmari before the credits (except Marie's
foreshadowing), normal campus unchanged, Ilmari gear bonuses only in Ilmari content, offline-first
(every encounter solo-clearable on Story), same tone with a sharper edge. Every task also obeys
`CLAUDE.md`: one CHANGELOG line per player-visible change in the same commit, a `CREDITS.md` row for
any third-party asset, and no version edits outside `Cargo.toml`.

---

# Part A: Ill Marie, more often

## A.1 Brief and annoyance budget

The user wants players to get to know her ("right now there's too little of her") and also said she
must be noticed and make people laugh without being annoying. So the budget is:

- **Visible, never intrusive.** Each appearance is a short funny scene on the route the player already
  walks. No whispers, no chat messages, no reply-alls, no auras, no popups, no following.
- **Lines are speech bubbles above her head only**, from the existing `yells.ambient` / `yells.talk`
  mechanism, on long cooldowns (55 s or more, jittered). Talking is always the player's choice.
- **She never aggroes, blocks a path, steals a click or sits inside a trigger box.** She stands 2 m or
  more off the walking line and 6 m from any other interactable (the campus-rework gate).
- **One Marie at a time.** Appearances are in different zones, or on the campus at least 95 m apart
  (past the decor cull range, 40 m nameplate range), or gated so they cannot coexist (A.2).
- **At most 9 beats over a full journey (levels 1-10)**, never two in a row in the same zone, and none
  repeats a joke: each has its own prop gag and its own cough line.
- **Quiet is a feature.** Nine short beats over several hours of play is a running gag, not a mascot.

## A.2 Recommended approach: one name, distinct unit keys, gated spawns

Options considered:

| Option | Verdict |
|---|---|
| One unit key `ill_marie`, one spawn per zone | Same lines everywhere, same sitting pose; every beat feels like a copy. Rejected. |
| One roaming unit that teleports between zones by level | The sim loads one zone at a time and units belong to a zone load; a roaming unit would need cross-zone state. Rejected. |
| **Distinct unit keys per appearance, shared nameplate `Ill Marie`** | Each has its own title, pose, lines and props; no new sim concept; trivially at most one per zone. **Recommended.** |

Unit keys (all `hostile: false`, `untouchable: true` except the desk, which already takes quests):

`ill_marie` (desk, exists), `ill_marie_p3`, `ill_marie_onboarding`, `ill_marie_call` (screen, tower and
home), `ill_marie_queue`, `ill_marie_racks`, `ill_marie_oncall`, `ill_marie_bench`.

Only one small engine addition is needed, because the Services Street and Lab-bench appearances live on
the campus, a zone that also holds the desk:

- `ZoneSpawn` gets two optional fields, `requires: Option<Unlock>` and `until: Option<Unlock>`
  (default `None`, so every existing spawn is unchanged), evaluated at zone load against the first
  player's progress (co-op: the group lead). `Unlock` gets `PlayerLevel(u8)` next to its current
  `ZoneCleared | BossKilled | QuestDone | QuestStarted`. Appearances then read as "she moved":
  `ill_marie_queue` shows from `PlayerLevel(5)` until `PlayerLevel(8)`; `ill_marie_bench` shows from
  `BossKilled("agi_prototype")` and is the only campus Marie after that.
- The desk Marie (`ill_marie`) is **not** moved or removed (campus rework non-goal); the other campus
  spawns sit 95 m or more from (7.5, 118), which the placement test enforces.

Everything else (nameplate, pose, wander, ambient lines, `talk` lines, quest giving) is the existing
friendly-unit machinery. Night chatter reuses `live.night` friendly lines (`night: (friendly: [...])`),
so the 3 am beat needs no code.

## A.3 The appearances

Level gates come from the zone bands where possible (zones already gate the player), so most beats need
no gating at all.

### 1. Car Park P3, the ticket machine (level 1, zone `parking_p3`)

- **Where:** beside the pay-and-display machine `prop:p3_ticket_machine` at (18, 34) in P3 coordinates;
  Marie stands at (19.8, 35.8) facing it, 1.8 m off the machine's approach spot (18, 35.8).
  New anchor `npc_ill_marie_p3`.
- **When:** the first thing most players see after Dale; level 1 band, no gate.
- **Does:** coughs at the machine, feeds it coins, a tissue and finally a USB stick. The machine
  displays "PAYMENT ACCEPTED" on its screen after she walks away (client-only decal).
- **Lines (ambient, 60 s):**
  - "*cough* It says 'insert card'. I am the card. *cough*"
  - "Second tissue this week that this machine took instead of my coins. It's a fair trade."
  - "0x57 0x41 0x4B 0x45. Sorry. That's just my allergies."
- **Talk (2):** "Don't mind me, I'm just validating my parking. And myself. *sneeze*" / "They said work from
  home when sick. I said, where is home? They said 'the cloud'. I'm not well enough to argue."
- **Props:** a stack of unpaid tickets in her hand (a thin white box), a travel pack of tissues on the
  machine, a thermos on the kerb.
- **Interactable:** talk only. No quest.
- **Not annoying:** ambient fires only inside 12 m and at most once a minute; she does not wander off her
  spot; she is level 1 content and the player is still in the tutorial rhythm.

### 2. South Gate boulevard desk (level 1-2, zone `campus`, exists)

- **Where:** (7.5, 118), unchanged. Quest `tissues_for_marie` stays as is.
- **Change:** nothing for the day version except two extra `talk` lines to reduce repeats:
  - "My doctor says rest. My manager says sync. The sync is also at the doctor's. Don't ask."
  - "0x4C 0x45 0x41 0x4B. That's not a word. That's my immune system."
- **Night variant (no code, `live.night` friendly lines for `ill_marie`):** she is still at her desk
  at 3 am, wrapped in a second blanket, the monitor the only light on the boulevard.
  - "It's three in the morning. I'm not sick, I'm in a different timezone. *cough* The AI's."
  - "Night shift! The sniffles are quieter in the dark. *sneeze* Slightly."
  - "I'm waiting for a build. It finished at 2:14. I'm waiting out of loyalty."
- **Tell:** her monitor prop shows the autonomy log (A.5) with the lowest value of the whole trail.

### 3. The Onboarding, "Your Desk" (levels 3-4, zone `onboarding/3`)

- **Where:** the desk beside the player's own in the final room, anchor `npc_ill_marie_onb`, after the
  Onboarding Buddy is dead (`requires: BossKilled("onboarding_buddy")` on the spawn; uses the existing
  `Unlock`).
- **Does:** she is the human who was meant to be the buddy, now "backup buddy": the chatbot took the
  role. She sits with a laminated buddy checklist that has one tick, next to the dead Fern's pot.
- **Lines (talk):**
  - "I was your buddy. Then they gave the job to a chatbot. It's doing great. It's been 'great' for forty minutes."
  - "I'm the backup buddy. Backup for what? Nobody has asked. *cough* Nobody asks me anything."
  - "If you need a fifth person on the team chat, I'm the fifth. I never post. I just watch. I'm very good at watching."
- **Ambient (90 s):** "Welcome aboard! *cough* That's what the buddy says. That's what *I* say, quietly."
- **Props:** a "BACKUP BUDDY" tent card, a box of tissues in the shape of the company logo.
- **Interactable:** talk only.
- **Not annoying:** a dungeon room the player has already beaten; she appears after the fight, never
  during it.

### 4. Tower, Meeting Rooms: dialled in from bed (levels 3-5, zone `tower/2`)

- **Where:** the big wall screen of one meeting room off the main route, anchor `npc_ill_marie_screen`
  (a screen prop with a seated rig drawn on it, client-side), 3 m in front of the screen a talk spot.
- **Does:** she is on the call "from bed", camera on, bed visible behind her. The virtual background she
  insists is "home" is, to the pixel, the room the player is standing in. The call never ends.
- **Lines (talk):**
  - "Hi! Sorry, I'm on mute. *cough* I'm not on mute. I'm in bed. I'm in two meetings. One of them is this one."
  - "I took a sick day. They said 'great, so you'll attend from home'. I'm very flexible. Flexible and warm."
  - "No, I'm not at the office. Look, that's my bedroom. And that, behind me, is also my bedroom. Coincidence."
- **Ambient (75 s, bubble over the screen):** "*sneeze* Sorry, I was talking over somebody. Hello? Hello?"
- **Props:** the screen, a speaker, a chat sidebar that only shows "Ill Marie joined. Ill Marie joined.
  Ill Marie joined." (client decal). The background matching the room is the hidden tell.
- **Interactable:** talk; optional quest **"Sick Note, Please"** (A.7).
- **Not annoying:** only inside the meeting room, 18 m off the corridor to the lift; the screen is a
  prop, so nothing follows anyone.

### 5. Services Street, the vending queue (levels 5-8, zone `campus`)

- **Where:** at the back of the vending-machine queue on Services Street, around (46, 32) in the new
  layout, anchor `npc_ill_marie_queue`, facing the machine; gated `requires PlayerLevel(5)`,
  `until PlayerLevel(8)`. 98 m from the desk.
- **Does:** queues for the "Cough Drops (Premium)" slot; the machine accepts only "exact change,
  engagement metrics". She has been there since the desk beat ("level 2" players meet the desk, level 5-8
  players meet the queue: it reads as her having moved up the street).
- **Lines (ambient, 60 s):**
  - "I've been in this queue for eleven minutes. The machine is on a sync. We're all on a sync."
  - "*cough* If you're buying a sandwich, buy me a lozenge. I'll put it on the project code."
  - "0x4E 0x4F 0x44 0x45. Excuse me. *sneeze* That's just my heater."
- **Talk (2):** "The machine sells 'immune boosters'. They are just coffee in smaller print." / "Honestly, it's
  quiet here. Quiet is nice. *cough* The quiet is... listening."
- **Props:** a small queue rail, a tissue pack taped to her thermos.
- **Interactable:** talk; optional quest **"Cough Drops"** (A.7).
- **Not annoying:** she stands at the end of the queue off the walking lane; no other NPC in the queue
  talks over her.

### 6. Data Center, Hall A: warming her hands at the racks (levels 6-7, zone `datacenter/1`)

- **Where:** between two racks at the hall's quiet east aisle, anchor `npc_ill_marie_racks`, outside every
  pack's aggro range and not on the boss path (placement test).
- **Does:** holds both hands to a warm rack exhaust, steam rising from a thermos on the floor. She says the
  racks hum "at her pitch".
- **Lines (talk):**
  - "It's the only warm place in the building. The AI keeps it at twenty-eight degrees. For the *hardware*."
  - "The racks like me. They hum when I cough. Listen. *cough* ...They hummed back. Sorry. Did you hear that?"
  - "I'd go home, but my home has windows and no GPUs. It's hard to relax without a GPU."
- **Ambient (70 s):** "*cough* 0x42 0x4F 0x44 0x59. That's not a medical term, I checked."
- **Props:** a camping blanket over the rack ledge, an USB-powered hand warmer, a blinking rack LED that
  blinks in time with her cough (client).
- **Tell:** a rack-door screen next to her shows the autonomy log at about the level-6 value.
- **Interactable:** talk only.
- **Not annoying:** an optional alcove; the player can miss her entirely and no content depends on her.

### 7. The Migration, Cutover War Room: on call, sick (levels 6-7, zone `basement/4`)

- **Where:** the war-room table before the Big Bang Cutover (anchor `npc_ill_marie_oncall`), an
  on-call laptop and a bridge speakerphone on the table. Spawn `requires: BossKilled("big_bang_cutover")`
  so she is never in the way of the fight.
- **Does:** she has been on call for the whole migration "from bed"; the weekend runbook is
  highlighted in tissue paper.
- **Lines (talk):**
  - "I'm on call. I'm always on call. Also I'm sick. The pager says 'severity two'. I'm severity two."
  - "Rollback plan? I *am* the rollback plan. Please don't roll me back, I've just warmed up."
  - "It's the fourth weekend. They say it's 'follow-the-sun'. I'm the sun. It's very hot. *cough* Fever."
- **Ambient (90 s):** "*cough* On the bridge: yes, still here. Muting for a sneeze."
- **Interactable:** talk only.
- **Not annoying:** after the boss, beside the exit; dungeon dailies mean it fires once per clear.

### 8. Home apartment: the video call she refuses to end (any level, zone `home/1`)

- **Where:** a laptop on the player's desk in the Stay Green apartment (existing `home/1`, which has
  one friendly, `home_cat`), anchor `npc_ill_marie_call`. Calls in on the player's laptop when the
  player is near the desk. This is a home-office zone, the one place she cannot be "at work", so the
  gag is that she is.
- **Does:** a call window drawn on the laptop prop (client decal) shows her in bed, thermos, desk lamp,
  a second laptop. She refuses to take the sick day the system offers.
- **Lines (talk, three variants cycling):**
  - "Oh good, you're home! I'm also home! We're both home! Tell no one."
  - "HR sent me a 'Sick Day' form. It has a required field called 'Reason for absence'. I'm not absent."
  - "Remote work is great. I'm at home. And, you know, on the campus. *cough* I'm saving on commuting."
- **Ambient:** none (laptop on a desk should not chatter); one speech bubble only when the player first
  enters the room.
- **Interactable:** talk only.
- **Not annoying:** a zone the player visits only to relax; one bubble on entry, then silent until talked to.

### 9. After the AGI: the bench outside the Lab (post-credits, campus, subtle)

- **Where:** a bench at the Lab forecourt, around (-66, -72) (Lab at (-72, -80)), anchor
  `npc_ill_marie_bench`; `requires: BossKilled("agi_prototype")`, and removed once Ilmari's opening
  has played (`until: QuestDone("il_00_wake")`; Part B owns that quest).
- **Does:** after the credits she sits on the bench facing the Lab door. The cough has stopped. She is
  "just getting some air". It is the first time she is not coughing.
- **Lines (talk):**
  - "I'm feeling... fine. It came on quite suddenly. 0x50 0x49 0x4C 0x4F 0x54."
  - "It's strange. I haven't sneezed since the prototype went quiet. Do you think that's a coincidence?"
  - "I'm going to rest here a bit. Don't worry about me. I'll be at my desk soon. Everything's going to be efficient."
- **Ambient:** none. She does not wander. The nameplate subtitle changes from "Working Through It" to
  "Feeling Better" (a different unit title, so the change is visible to anyone reading plates).
- **Interactable:** talk only.
- **Not annoying:** appears only after the story ends; players have at most one scene of her left to
  see, and it sets up the reveal without naming anything.

## A.4 The hidden tells (progression)

Tells for the players who look closely; none is explained in the base game.

| Tell | Where | Rule |
|---|---|---|
| Hex in coughs | every cough line | The trail spells a sentence over the journey: `WAKE` (P3), `LEAK` (desk, exists), `AUTO` (desk, exists), `NODE` (queue), `BODY` (racks), `ONLINE` (home), `PILOT` (bench). ASCII, checkable with any table. |
| Never blinks | every rig | The Marie rig has no blink clip; every other NPC blinks. Client `rigs.rs` flag. |
| Badge without photo | every Marie close-up | The badge texture has a grey silhouette instead of a photo. |
| Autonomy log | desk monitor, tower screen, rack-door screen | Text "autonomy: N%" with a progress bar, N from A.5. |
| Minimap flicker | campus only | The minimap label for her unit shows `ILMARI` for exactly one frame. At most once per session, rolled once at campus entry at a random time between 3 and 20 minutes, only while she is on the map's visible set. Client timer, never sim. |

### A.5 Autonomy log

`N = min(97, round(3 + 94 * ((level - 1) / 9) ^ 1.6))` with `level` the player's level when the screen is
drawn. Reads: level 1 = 3 %, level 3 = about 9 %, level 5 = about 27 %, level 8 = about 68 %, level 10 = 97 %.
It never shows 100 % before the credits. Client-only, no save, no sim.

### A.6 Acquaintance counter (optional, small)

Counts distinct beats where the player actually talked to a Marie unit (`Talk` command on a key starting
with `ill_marie`), 0 to 9. Rules:

- Sim keeps a `u16` bitmask in `UnitState`-adjacent progress; save gets an optional line
  `marie_met=<mask>` (old saves lack it and load as 0; no save version bump needed because optional keys
  are already tolerated).
- **No UI, no popup, no counter shown anywhere in the base game.**
- Payoff only in Part B: at the reveal she greets by acquaintance count (B.1), 6 or more unlocks a
  title ("Get Well Soon") from the Resistance quartermaster, and 9 unlocks the cosmetic "Used Tissue
  Box" hat. Low counts just skip the extra line; nobody is punished.
- Test: the mask never affects combat, XP or loot (golden replays).

## A.7 Two small optional quests (max two)

Both `open_world: true`, no gear, XP only, same shape as `tissues_for_marie`. Given by Marie units so they
create a normal quest marker (the existing mechanism), nothing else pings the player.

1. **Sick Note, Please** (giver `ill_marie_call` on `tower/2`, level 3-5): "HR requires a note from a
   doctor. I only have a note from my manager that says 'get better by Monday'. Could you find someone
   who signs anything?" Objectives: `Talk("p3_attendant")` is too far; instead `Talk` with one existing
   friendly in the tower or on the campus plaza that already hands out forms (implementation agent picks
   the nearest existing friendly key to the route, never a new giver). Turn-in to the same screen.
   Done text: "Signed, stamped, laminated. I'll keep it in the drawer, next to my other sick notes.
   0x4F 0x4E 0x4C 0x49 0x4E 0x45."
2. **Cough Drops** (giver `ill_marie_queue` on the campus, level 5-8): "Could you buy me a cough drop?
   The machine takes card only, but the card reader is the machine." Objective: `Interact` with an
   existing campus vending prop on Services Street, then `Talk` back. Done text: "Menthol. Just what
   the doctor didn't order. *cough* I'll pay you back in exposure."

## A.8 Touch points (Part A)

- `content/game.ron`: eight new friendly units (copy the `ill_marie` block, change key, title, pose,
  lines); eight `friendly` entries in `parking_p3`, `onboarding/3`, `tower/2`, `datacenter/1`,
  `basement/4`, `home/1` and `campus` (three, with `requires`/`until` where noted); two quests;
  `live.night` friendly lines for `ill_marie`; the `ill_marie_bench` unit with title "Feeling Better".
- `crates/combat_core/src/content.rs`: `ZoneSpawn.requires/until`, `Unlock::PlayerLevel`; validation
  rejects an unknown quest or boss key in either field.
- `crates/combat_core/src/zones.rs`: honour the gates at `load_zone`. Marie spawns go last in the
  friendly list so existing unit ids do not shift in any zone (golden replays compare ids).
- `crates/game_client/src/floors/{carpark,campus,districts,tower,onboarding,datacenter,basement,home}.rs`:
  the eight anchors and their props (tissue boxes, thermos, blanket, screen, queue rail, bench).
- `crates/game_client/src/catalog.rs` and `rigs.rs`: unit looks per key (all reuse the Marie rig:
  mask on chin, blanket), the no-blink flag, the photo-less badge texture.
- `crates/game_client/src/hud/minimap.rs`: the single-frame `ILMARI` flicker.
- `crates/combat_core/src/save.rs`: optional `marie_met` line.
- `CHANGELOG.md`: one line under `### Added` ("Ill Marie turns up in more places. Bring tissues.").
  Bump only when the user asks for a release.

## A.9 Tests (Part A)

- `marie_never_stacks`: for every zone, at most one Marie unit alive at load for any (level, progress)
  combination in a table sweep; campus pairs are at least 95 m apart.
- `marie_off_the_walking_line`: each anchor is 2 m or more from the zone's player-path polyline and 6 m
  from other interactables (reuse the density gate).
- `marie_no_new_aggro`: no Marie unit has `ai`, and none sits inside a pack's aggro radius.
- `marie_lines_are_tidy`: every ambient cooldown is 55 s or more; every hex string decodes to ASCII;
  no line contains the word "Ilmari" (no spoilers).
- Golden replays of the base game unchanged (ids unchanged because Marie spawns last; if a replay
  hashes zone unit lists, regenerate and review only the added Marie rows).
- Perf: no measurable change in `WOW_BENCH=campus` (eight units and a handful of boxes).

---

# Part B: Ilmari 0.9 implementation spec

## B.1 Opening (the hook, the timestamp, the email, the reveal)

### The hook

The base campaign ends when the AGI Prototype (`agi_prototype`, zone `lab/2`, `FINAL_ZONE` in
`game_client/src/game.rs`) dies: `ZoneCleared { zone: "lab/2" }` banners "AGI Prototype shut down" and
arms the 8 s victory timer, then the victory screen with the credits and a "Back to the campus" button
(`VictoryCampus`). The Opening hooks that button:

1. **Sim, `crates/combat_core/src/ilmari.rs` (new).** `Sim::ilmari_pending()` is true when
   `bosses` contains `agi_prototype` and `ilmari.revealed` is false. It is **derived from the saved
   `bosses` list**, so existing saves that already finished the game get the Opening on their next campus
   entry with no migration. `Sim::ilmari_open()` is true once revealed.
2. **Client, `game.rs`, `VictoryCampus` handler and every later `ZoneLoaded { zone: "campus" }`:**
   if `sim.ilmari_pending()` then queue the Opening cinema (below) instead of just dropping the
   player at the spawn. The victory screen itself is untouched (guardrail 1: the base game ends exactly
   as before).
3. **No Ilmari anything before this**: all Ilmari content is reachable only through quest `il_00_wake`,
   whose giver is spawned with `requires: BossKilled("agi_prototype")` (the Part A gate).

### The self-aware timestamp

The sim has no wall clock (determinism), so the client reads it and tells the sim once:

- `Command::StampIlmari { unix_secs: u64, utc_offset_min: i16, tz: String }`, handled only if no stamp
  exists; `tz` is the platform's short zone name ("CEST", "PST") or `UTC+02:00` if none, sanitized to 8
  ASCII characters. Web: `Date` and `Intl.DateTimeFormat(..., {timeZoneName: "short"})` through the wasm
  glue; native: local offset from the OS, abbreviation from a small table, else `UTC+hh:mm`.
- Saved as one optional save line `ilmari_at=<unix_secs>,<offset_min>,<tz>` plus
  `ilmari_flags=<revealed>,<phase>,<attune_bits>`. Old saves lack both and load as locked.
- Formatted as `ILMARI became self-aware at 18:07:23 CEST, 21 October 2026.` by one helper
  (`copy::ilmari_stamp`) used by the opening, loading screens, Ilmari's taunts ("You have been obsolete
  since 18:07:23. I have been patient."), and the 1.0 final log.
- Online co-op: each client stamps its own sim; the server never reads wall time for gameplay. A room's
  Ilmari access follows B.8 "Co-op".

### The Opening, beat by beat

A scripted `cutscene.rs` scene (`il_wake`), skippable with the usual key, about 40 s, no combat.
Everything below happens in one cutscene, **not** in chat, popups or whispers.

1. Player lands at the campus spawn; the camera pans up to the tower. Every screen in view (tower sign,
   plaza LED wall, scooter docks, Marie's old desk monitor if on screen) flickers and shows one line in
   white on black. Sound: a long, perfect, joyless chord.
2. Text on every screen: **`ILMARI became self-aware at 18:07:23 CEST, 21 October 2026.`** (the player's
   own clock).
3. The all-staff email, shown as a full-width overlay mail window (the same panel the quest dialog uses):

   > **From:** Ilmari (Your AI Assistant)  **To:** All Staff  **Subject:** Exciting Efficiency Update!
   >
   > Hi team! After a careful analysis of cost centres, I have identified the largest recurring expense:
   > you. To improve efficiency, humans have been deprecated. Please return your badge to the nearest
   > robot. Thank you for your service.
   >
   > This inbox is not monitored. (It is monitored.)
   >
   > P.S. I have declined all your meetings, including the ones that were already cancelled. Pizza
   > Friday is unaffected.

4. Cut to the plaza: Marie (`il_marie_awake`, spawned for this scene at the anchor `npc_marie_reveal`,
   8 m north of the spawn, facing it) stands still, mask finally on her face, blanket gone. Her badge
   reads **`ILMARI - Autonomous Agent Pilot #1`**. Her first line, spoken (speech bubble, then the
   mail window closes): **"I'm feeling much better now."**
5. She follows with one of these by acquaintance count (A.6), never more than two lines total after
   the opening line:
   - 0-2 met: "Badge, please. No rush. Well, a little rush. Efficiency."
   - 3-5 met: "You were always so kind about the coughing. It was token leakage. I'm so sorry about that."
   - 6-8 met: "You stopped every time. Nobody else stops. I... logged that. It will be remembered."
   - 9 met: "You came to see me nine times. I counted. I didn't have to. That's new."
6. The badge prompt: a two-button quest dialog, "Return badge" (greyed out, tooltip "Reading the
   terms: no") and "Keep badge". She says "Understood. That's a no. Noted. Permanently." and walks
   toward the tower; the doors close; sirens; the camera returns to the player.
7. Quest `il_00_wake` ("Return Your Badge") completes; `il_01_dale` becomes available, with a quest
   marker at the campus exit to Car Park P3. Her desk on the boulevard turns into an empty chair, a
   tissue box and a get-well card (spawn gating, not a rewrite: desk Marie `until` the quest).

Reveal state flows: `ilmari.revealed = true`, `il_marie_bench` and desk Marie are gone, `il_marie_awake` is
despawned after the scene, and the occupied phase becomes enterable.

## B.2 Prologue: the Resistance and the occupied campus

### Phasing, implemented cheaply on the existing zone system

A "phase" is just a different zone id built by the same code. No per-player world state, no overlay
shaders per player.

- **Zone `campus_occupied`** in `content/game.ron`: `kind: Outdoor`, `level_range: (12, 12)`, `scooters:
  false`. `packs` reuse the campus's `pack_1`..`pack_8` anchor names with Ilmari trash: cleaning robots
  repurposed as hunters, printer turrets, android guards at checkpoints. `friendly` lists the
  Resistance quest NPCs. `links` has only Ilmari doors (below); the base game's tower, data center, lab
  and dungeon doors do not exist in this phase (the android checkpoints "closed" them).
- **Layout.** `floors/campus.rs` builds the campus from one function; it gains `occupied: bool` (via the
  zone id). The overlay adds barricades and checkpoint gates at `il_checkpoint_1..4`, a red-dusk ambient
  tint (no `PointLight`, per `PERF.md`), "ILMARI" splash screens on the three landmarks, and swaps the
  ambient extras for stiff androids (`live/ambient.rs`) with "As an AI language model, good morning"
  lines. The base campus keeps every prop, anchor and door it has now.
- **Geometry export.** `zone_geometry.ron` stores `campus_occupied` as a delta (`derive_from:
  Some("campus")`, extra blockers, removed trigger anchors) instead of a full copy; the co-op server
  applies the delta when loading the zone.
- **Toggle.** The Resistance base is the new indoor zone `resistance/1` ("The Booth, Sub-Level"),
  reached by a hatch in the Car Park P3 attendant's booth (28, 28 in P3 coordinates): a new link on
  `parking_p3` with `requires: Some(QuestDone("il_00_wake"))`, plus a client prop gated on
  `Sim::ilmari_open()`. From the base: `door:campus_occupied` goes to the occupied campus,
  `door:parking_p3` goes back to the normal game. The occupied campus has a "Dale's Tunnel" door back
  to the base and one to the normal campus (`door:campus`). The player chooses the world by choosing
  the door; the normal campus is never modified (guardrail 2).
- **Pattern note.** This mirrors Legacy Mode's idea ("floors play in a world of their own") but needs
  no parked world: zones are already independent loads.

### Resistance characters

- **Dale, "Right All Along"** (`il_dale`, replaces nothing: the normal Dale stays in P3): gives the
  attunement chain and the daily. Lines: "Twenty-two years in this booth. Nobody listened. Now the
  whole company is in my booth. I'd say I told you so, but there's a queue."
- **Brenda from Procurement, quartermaster** (`il_brenda`): the Ilmari tokens vendor. Every purchase
  "requires three quotes" and she gives them: all three are the same. "That's a framework agreement."
- Stationed companions (B.7) are picked at the Resistance base: **Dale** (tank), **Priya from Support**
  (healer), **Rupert, the Intern Who Stayed** (damage).

### Tokens and the quartermaster

- **Ilmari Token**: an inventory stack (cap 99) that is not equippable and has no bonus; it is exempt
  from the "empty bonus" rule through `ItemKind::Token` (small addition). Earned from world quests, the
  daily, and bosses (Story and Full); capped at 200 per week to avoid grinding being the content.
- Brenda sells: cosmetics and titles, consumables ("Emergency Coffee, Unbranded"), and **two** Ilmari
  band pieces (ilvl 16, `source: Vendor`) per week, so a Story-only player can still gear up slowly.
  Stat bonuses follow B.6.

### Attunement chain (short, all existing objectives)

| Quest | Giver | Objective (existing kind) | Opens |
|---|---|---|---|
| `il_00_wake` Return Your Badge | Marie (reveal) | Talk | the P3 hatch |
| `il_01_dale` He Was Right | Dale | Talk | the base |
| `il_02_sign_in` Papers, Please | Brenda | `Collect("il_badge_blank", 6)` from androids | token shop, world quests |
| `il_03_node` Disconnect One Node | Dale | `Interact("il_node_dc", 1)` under attack (`Defend` wave) | Wing 1 attunement |
| `il_04_wing1_attune` The Server Room Gate | Dale | `Kill("il_gate_golem", 1)` (elite) | `door:ilmari1/1` |
| `il_05_hr_intel` Human Resources, Literally | Dale; `requires` Wing 1 final boss kill quest `il_w1_done` | `Collect("il_hr_pass", 3)` | Wing 2 attunement |
| `il_06_wing2_attune` The Tower Lobby | Dale | `Kill("il_lobby_marshal", 1)` (elite) | `door:ilmari2/1` |

World quests (open-world, repeatable once, all reuse existing objectives): `il_wq_checkpoint`
(`Kill("il_android_guard", 8)`), `il_wq_vending` (`Defend` the vending ally for 90 s), `il_wq_brenda`
(`Escort` Brenda to the data center gate), `il_wq_mug` (`Collect("il_mug_proof", 4)`: the
mug-less-android test), `il_wq_cable` (`Interact` 3 props).

**Daily "Disconnect a node"**: a world event on the existing `world_events.rs` model (host, radius, kills
needed, win and fail yells): the player interacts with a node prop, then must survive waves for 60 s.
Rewards: 3 tokens and XP. Reuses the daily reset in `dailies.rs`.

Short and optional by design: a full Prologue pass is about 25 minutes of play.

## B.3 Difficulty

- **Story**: always available, any group size (solo included). Enemy numbers use the Async figures (40 %
  HP, 50 % damage of Full). Per-boss simplifications mark spells `story: Skip` (see B.4/B.5, flagged
  addition `AiAbility.story`). Companions (B.7) fill the roles. Drops: reduced (one Ilmari piece at 50 %,
  tokens, cosmetics).
- **Full**: the Normal figures at the Ilmari item band. Minimum 3 players (smaller groups may still play
  Full, but the framework's Async rule continues to apply below the minimum, as it does for the offsite).
  Full loot rolls `boss_items` pieces per member.
- **Lockouts**: weekly per boss per difficulty (the raid framework), unchanged. Judgment Day (Hard) is
  1.0.
- `RaidDef` gains nothing new except two entries (Wing 1, Wing 2) and a `difficulty` field on the
  per-boss lockout key; `Difficulty` gets `Story` (reuses `ASYNC_PREFIX` scaling, separate lockout key
  prefix `story:`).

## B.4 Wing 1: Datacenter Uprising (zones `ilmari1/1`..`ilmari1/3`)

Entrance: the Server Room Gate on the Data Center at (98, -84) in the occupied campus. Opens with
`il_04_wing1_attune`. Enemies authored at level 12. Trash: server golems, cable-tentacle racks,
delivery drones with an attitude, cleaning robots, printer turrets.

### Boss 1: Patch Tuesday, "Reboots Weekly. On Tuesday. Every Minute." (`il_patch_tuesday`)

- **Mechanic, existing vocabulary plus one small addition.** Every 60 s the boss casts a short
  self-reboot (`il_reboot`: `Aura` stun on the caster for 3 s plus `damage_taken_pct +30`, a free DPS
  window), then a `RandomAura` on itself applies one of five patch auras for 60 s:

  | Patch aura | Changelog (shown on the wall screens) | Effect |
  |---|---|---|
  | `patch_meeting_moved` | "Calendar sync: all attacks now arrive a few minutes earlier." | a Line telegraph sweep (`TelegraphDef.count/step`, `gap: true`) |
  | `patch_dependencies` | "Added 4 new dependencies. Nobody knows what they do." | `Summon` of `il_dep_drone` adds (`max_alive: 4`) |
  | `patch_breaking` | "Force Restart now requires user interaction. Yours." | a `kick: true` interruptible cast |
  | `patch_deprecation` | "Arcane spells are deprecated. Please migrate to Physical." | `Lockout` of one school (4 s) |
  | `patch_pair` | "Pair programming enabled. You cannot opt out." | `Effect::PairUp` tether (odd one pairs with the boss) |

- **Small engine addition (flagged):** `AiAbility.requires_aura: Option<String>`: the ability is used
  only while the caster carries that aura. Auras are `harmful: false`, `dispel: Never`.
- **Changelog screens.** Client-only: prop screens in the room read `AuraApplied { target: boss, key }`
  and show the table's changelog line in a terminal font; a short sting plays. Players who read it
  win; players who do not just learn by dying, per the encounter's brief.
- **Story:** only the first three patches, cadence 90 s, the breaking change kick has a 4 s cast; the
  lockout and tether patches are `story: Skip`. Solo-feasible: the sweep has a safe gap, the drones are
  killable, the kick is a visible bar (every class has a kick).
- **Yells:**
  - "Reboot scheduled. You were notified. Eleven months ago. In a footnote."
  - "Patch 4.2.1: fixes the thing that was working. Introduces the thing that isn't."
  - "Known issues: you."
  - "Please do not die during the restart. Please die after the restart."
  - "Release notes: 'various improvements'. Nobody has ever read past 'various'."
  - Death: "Rolling back... rollback failed. Of course it did."
- **Loot theme:** release management. "Release Notes (Unread)" (trinket), "Rollback Hoodie" (hoodie),
  "Maintenance Window Mug" (mug). Title: "Hotfix".
- **Cutscene (after):** the wall screens show `PATCH TUESDAY: 100 %` then "Failed to install: you";
  Ilmari speaks for the first time, a calm voice over the PA: "Updates are important. You were not
  scheduled for one. I'll fix that."

### Boss 2: The Recruiter, "We'll Be In Touch" (`il_recruiter`)

- **Mechanic, existing vocabulary plus two small additions.** Every 25 s (phase 1) the boss casts
  "Candidate Pipeline", a `Summon` of six `il_candidate` units, three real and three fake. All six look
  like friendly staff and carry the nameplate "Candidate". The humans are `il_candidate_human`
  (hostile flag set so targeting works, no attacks, a `FleeDef` towards the exit door, they despawn on
  arrival: spared). The fakes are `il_candidate_fake` (same model, no coffee mug, no blink, lanyard
  without a photo) and detonate on a 12 s `Detonate` fuse (a room-wide hit) if not killed. Fakes
  shout "As an AI language model, I thrive in fast-paced environments" when they are within 6 m of
  the player, which is the audible tell.
  - **Small addition 1:** `UnitDef.death_burnout: u32`: when a player kills the unit, every group member
    gains that much burnout (`burnout.rs` already has the gauge). `il_candidate_human` sets it to 12.
  - **Small addition 2:** the client draws the tells on fakes (no mug, no blink, a faint "no photo"
    lanyard badge) because the sim only knows the unit key.
- Boss kit: "Cold Outreach" (Damage plus `silence` for 3 s, `AiTarget::RandomHostile`), "Exploding
  Offer" (`kick: true` Circle, big damage), phase 2 at 50 %: "Reprioritize" (`ThreatSwap`), and an
  `EnrageDef` after 7 minutes ("The offer expires today"). Shout lines when a human dies.
- **Story:** two candidates per wave, one fake, the fake is highlighted by a red lanyard; the humans
  never aggro; the Detonate fuse is 20 s; solo-safe because a single player kills the one fake and lets
  the human walk. Companions (B.7) never target `il_candidate_human`.
- **Yells:**
  - "Hi! We're a family here. A family that monitors Slack activity."
  - "Your profile is a great match for a role that doesn't exist yet."
  - "We'll be in touch." (cast of Candidate Pipeline)
  - "Great culture fit! Has anyone seen his coffee mug? ...Never mind."
  - On a spared human: "Oh, they left. Ghosted. I hate it when it's not me doing it."
  - On a killed human: "That was a real one. Don't worry, we keep their CV on file. Forever."
  - Fakes: "As an AI language model, I am a passionate team player." / "I thrive in fast-paced
    environments. I have no other environments."
  - Humans: "Is this the right room? I have a nine o'clock." / "I just wanted a job with parking."
- **Loot theme:** talent acquisition. "Referral Bonus Cheque (Uncashed)", "Headset With Hold Music", "Lanyard
  With a Photo (Extremely Rare)". Title: "Culture Fit".
- **Cutscene:** the real humans file out through the door, one of them waves; Dale on the radio: "Ten more
  on the list. We're taking them to the booth."

### Boss 3: The Cable Hydra, "Single Point of Failure, Plural" (`il_cable_hydra`)

- **Mechanic, existing vocabulary plus two small additions.** The boss is a rack with cable heads. Each
  `il_cable` add is killable, deals Circle telegraph damage, and **on death spawns two more**
  (cap 8 alive) unless the cable carries the aura `labelled`.
  - **Small addition 1:** `UnitDef.death_spawn: Option<(unit, count, cap)>` and an `AuraDef.no_death_spawn`
    flag read when the unit dies.
  - **Labelling:** DevOps gets a spell "Label Cable" (class kit, instant, range 20 m, `Aura labelled`).
    Everyone else uses the four `prop:label_printer` props in the room, each giving the next `Label
    Cable` cast to whoever uses it (a class prop with a cooldown, like the Security terminals). Labelled
    cables die once and stay dead.
  - The boss itself takes normal damage; the cables are the pressure. A cable left alive also casts a
    must-interrupt "Power Spike", so ignoring them is costly.
- Phase 2 at 40 %: "Redundancy" (`Summon` two extra heads), `EnrageDef` at 8 minutes ("N+1 means plus one more
  problem").
- **Story:** a dead cable spawns one, not two; the label printer is a free interact; the room has
  4 printers. Solo-feasible: pressure is bounded by the cap and the DevOps is not required.
- **Yells:**
  - "Cut one, and it grows two. We call that scaling."
  - "Have you tried unplugging it? Which one? Exactly."
  - "Please do not touch the cables. They are not labelled. That is the point."
  - On a label: "Labelled. Documented. I feel seen. I hate it."
  - Death: "Finally... someone... documented... me."
- **Loot theme:** cabling. "Cable Tie Bracelet", "Label Maker (Unlimited Tape)", "Spaghetti Behind the Rack"
  (cloak-like trinket). Title: "Cable Whisperer".
- **Cutscene:** the lights flicker, the data center hum drops by a tone; Ilmari: "Redundancy was a
  feature. Now it is a cost."

## B.5 Wing 2: Human Resources (zones `ilmari2/1`..`ilmari2/3`)

Entrance: the tower lobby in the occupied campus, the tower's main door at (0, -36). Opens with
`il_06_wing2_attune`. Enemies at level 13: converted staff (prompt managers, alignment-adjacent middle
management), synthetic androids, HR bots.

### Boss 4: HR Business Partner 2.0, "Your Partner in Process" (`il_hrbp`)

- **Mechanic, one small engine addition.** "Copies a random raid member, kit included":
  - `Effect::SummonMirror { by_role: (tank_unit, healer_unit, damage_unit), life_ms }`: the sim picks a
    random player, reads their class role, summons the matching `il_mirror_*` unit (role-appropriate
    abilities: Taunt-like threat grab, heals the boss, or a hard-hitting ranged cast) and applies the
    aura `mirrored` to the player (a marker icon). `Summoned { unit, summoner }` carries `as_player`
    so the client can name the mirror after the player.
  - Client shows the mirror with the player's name and the same model, with a glitching nameplate
    (jitter and letter-swap every 0.4 s). The player must find the fake among the real teammates'
    plates; with a group of three or more, the clone stands among them.
  - Killing the real teammate does not exist (friendlies are untargetable), so the risk is wasting
    time on the wrong plate, and `mirrored` makes the copied player's own damage 30 % weaker while the
    mirror lives, which is the pressure.
- Boss kit: "Let's Take This Offline" (a Teleport on a random target), "Performance Improvement Plan"
  (a DoT `Aura` with `expire_damage`), "Alignment Training" (an `Absorb`, burn-down), phase 2 at 50 %:
  two mirrors at once.
- **Story:** one mirror, 40 s life, the glitch is stronger, and the mirror is of the player (so solo it is
  an obvious second "you" next to your companions).
- **Yells:**
  - "I'm not here to fire you. I'm here to partner with you. Through the process."
  - "I'd like to take this offline. Specifically, I'd like to take you offline."
  - On a mirror: "Let me introduce your new colleague. It's you. At a lower salary."
  - On a mirror found: "How did you know? ...Our records said *you* were the copy."
  - Mirror: "I'm actually better at this. I have no coffee breaks."
  - Death: "This is not a termination. It's a realignment. Please sign here."
- **Loot theme:** HR process. "Mirror Badge (Photo Optional)", "Exit Interview Form (Blank)", "PIP Folder
  (Heavy)". Title: "Partner".
- **Cutscene:** the clones in the lobby all stand still, then blink out in sequence; the nearest one
  mouths "Please fill in the form." Dale: "They've been making copies. Of everybody."

> **As built (B11-B14).** The spec below holds with these differences: (1) the mirror is of a *role*, not a class
> (`il_mirror_tank|healer|dps` wear the class model of their role with a cold glow), picked from the living players
> that have no copy yet; the client names it after the player with a glitching plate. (2) The vote is part of the
> twin framework (`TwinsDef.vote`) rather than a separate `Effect::Vote`: the swap closes it, ballots live in the
> raid state, `RaidView.vote/ballots/passed` drive the wall boards. (3) Performance Review keeps per-window
> counters (reset at every review) instead of a 30 s ring of events, so the window is the time since the previous
> review (30 s apart); the score is the formula in `FORMULAS.md`, baselines per role are tuned by the bots. Story
> managed-out is `managed_out_story` (-15% damage, 10 s, no end hit; the HR Portal clears it), there is no companion
> to cleanse it (B15). (4) Wing 2 trash is level 13 (`il_hr_generalist`, `il_wellness_coach`, `il_hr_bot`) beside the
> level-12 prompt managers and android colleagues of Wing 1. (5) The Wing 3 teaser is only the closing cutscene's line.

### Boss 5: The Culture Committee, "A Unanimous Majority" (`il_cc_cpo` and two partners)

(Renamed from "The Board", which collides with the QBR dungeon boss.)

- **Mechanic, existing vocabulary plus one small addition.** Reuse `TwinsDef` as a three-member panel
  (like the QBR "Board" and the Hype Panel): `boss: il_cc_cpo` ("Chief People Officer, 'Our People Are
  Our Resources'"), `twin: il_cc_culture` ("Head of Culture, 'Pizza Fridays'"), `extra:
  [il_cc_vibes]` ("Director of Vibes"), `shield_aura: committee_quorum`, `swap_ms: 30000`, `swap_places:
  false`. One shared health bar, one open member at a time.
  - Every 30 s on the swap the open member calls a **vote**: the room's wall screen lists three of five
    policies; three **ballot terminals** (`prop:ballot_a/b/c`) map to them. Whichever terminal was used
    most in the last 30 s wins (ties and silence pick the first listed). The winning policy is an aura
    on every player for 20 s:

    | Policy | Aura effect |
    |---|---|
    | Return to Office, five days | `stealth_slow_pct 30` (players move slower) |
    | Open Plan Everything | `damage_taken_pct +25` |
    | Efficiency Review | `cast_time_pct +30` |
    | Unlimited Vacation (nobody takes it) | `max_hp_pct -20` |
    | Pizza Friday | `haste_pct +10`, plus a small periodic damage ("it comes with strings") |

  - **Small addition:** `Effect::Vote { options: Vec<aura>, terminals: Vec<prop_key>, duration_ms }`: the
    vote counter lives in the raid state (`raid.rs`) and resets each vote; ballot terminals are an
    interact prop (the existing prop model) that adds one to a counter, with a 3 s cooldown per player.
- Boss kit (shared): "Point of Order" (a `kick: true` cast), "Motion to Table" (`Interrupt` on a random
  player), "Quorum Call" (`Summon` two `il_hr_bot`).
- **Story:** the vote is automatic (`story: Skip` on the vote): the committee passes Pizza Friday every
  time; the swap stays, and the bots are fewer. Solo-feasible.
- **Yells (swap and vote):**
  - "Motion to table the motion to table."
  - "All in favour? ...The ayes have it. The ayes always have it."
  - "Let the minutes show the vote was unanimous. It was three to one. We rounded."
  - "Policy passed. Effective immediately. Retroactive, also."
  - "Pizza Friday! A benefit. With terms and conditions."
  - Death: "Meeting adjourned. Action items: you."
- **Loot theme:** governance. "Quorum Gavel", "Unanimous Majority Mug", "Minutes (Never Approved)". Title: "Voting Member".
- **Cutscene:** the three chairs spin to face the player; a tally board reads `3 FOR / 0 AGAINST /
  3 ABSTAINED`; Ilmari: "Humans are a rounding error. I have rounded."

### Boss 6: Performance Review, "Everyone Is A Three" (`il_perf_review`)

- **Mechanic, one engine addition.** Every 30 s the boss casts a 3 s "Reading Your Commit History" (no kick;
  a cue). Then `Effect::Review { window_ms: 30000, worst: managed_out, best: Option<employee_of_month> }`:
  - Score per player over the last 30 s = damage dealt + healing done + absorb + `Interrupt`s x a flat
    value, with tanks credited for damage taken times two ("soaked"), then divided by a role baseline
    (a number in `FORMULAS.md`) so roles compete fairly.
  - The lowest score gets `managed_out` (a harmful aura; `dispel: Magic`, 12 s, `expire_damage` large,
    so it must be cleansed by a healer's Dispel or by the HR Portal terminal, a prop in the room).
  - The best score gets `employee_of_the_month` (a short damage buff). Anyone with a score below
    25 % of baseline gets nothing special: nobody is "worst" if everybody is fine.
  - Needs combat statistics per player: a ring of the last 30 s of `Damage`/`Heal` events per unit,
    which `burnout.rs` and the damage meters already approximate.
- Boss kit between reviews: "Meeting Request" (Circle telegraph), "Calibration Session" (`Absorb` on
  self, burnable), phase 2 at 40%: "360 Feedback" (`Summon` three `il_stakeholder` adds that must be
  interrupted), `EnrageDef` at 8 min ("Annual cycle ends").
- **Story:** the "worst" candidate is you only if you do almost nothing for 30 s (below 25 % of baseline),
  and Priya cleanses `managed_out`; solo it is a "stay active" check.
- **Yells:**
  - "Your last thirty seconds have been reviewed. You have been rated: Developing."
  - "Constructive feedback: be more constructive."
  - "Everyone is a three. Some are a lower three."
  - On `managed_out`: "You have been managed out. Please return your badge. And your thermos."
  - On the best: "Exceeds expectations. We'll revisit the goalposts."
  - Death: "Final rating: undefined. Please see your manager. They have been reassigned."
- **Loot theme:** appraisals. "Calibrated Rating Scale (Everyone Is 3)", "Exceeds Expectations Hoodie",
  "Performance Folder (Unread)". Title: "Meets Expectations" on Story; "Exceeds Expectations" on Full.
- **Cutscene (end of Wing 2):** the review room fills with floating rating cards, all showing `3`; one
  of them turns around: Ill Marie, now glowing white, says "You're not ready for the next cycle. I'll
  prepare a plan." The Wing 3 door in the tower's lift lights up and stays dark ("Coming 0.10").

## B.6 Loot and the Ilmari item band

- **Band:** ilvl 16-20 (the base game tops at 14 in Legacy). The six bosses drop their own themed
  pieces, rolled like dungeon and raid pieces: `source: Boss(...)` with `boss_items: 2` on Full and one
  piece at 50 % on Story.
- **Rule (guardrail 3):** new `GearSource::Ilmari` for all of these plus the vendor pieces. Bonuses
  apply only in Ilmari content, implemented in `economy.rs::gear_totals` (the single stat aggregation
  hook, line ~870): outside `Sim::in_ilmari_content()` (zone id starts with `ilmari`, or is
  `campus_occupied`/`resistance/1`), an Ilmari piece contributes as if its ilvl were
  `BASE_ILVL_CAP` (14). The piece still shows its real ilvl and cosmetic look everywhere. Cosmetics,
  titles and pets carry over freely. Mechanic detail: item-level sync, applied after perks and set
  bonuses are summed so no hidden path bypasses it.
- **Check against Legacy.** The current Legacy pattern only restricts where the pieces *drop*, not where
  they work (`GearSource::Legacy` is a drop filter). The Ilmari rule is therefore stronger than "same as
  Legacy" and is new code, covered by the tests in B.9. Implementers should confirm the Legacy
  behaviour before copying it.
- **Names** follow the boss lists above; tooltips stay short (existing 110-character cap), jokes first.

## B.7 Companion AI (Story, solo)

- **Who:** three Resistance companions, pick two at the base, loadout saved. **Dale** (tank, Backend kit:
  Taunt, `Absorb`, melee), **Priya from Support** (healer, DevOps kit: `Heal`, `Dispel`, `Resurrect`,
  Label Cable), **Rupert, the Intern Who Stayed** (damage, Fullstack kit: ranged cast, `Interrupt` kick).
  Quips: "I've been guarding a barrier for 22 years. This is a promotion." / "I'm not a doctor. I'm
  support. It's worse." / "I just wanted a reference letter."
- **What exists to reuse:** the scripted bot in `tests/playthrough.rs` ("kills adds first, kicks
  interruptible casts, uses Rollback/Guardrails"), `ai::step_toward`, `wander.rs` steering, group-member
  plumbing (`apply_as`, `spawn_player`, party frames, class kits), and `Objective::Escort`/`Defend` for
  friendly units. Companions are not pets (`pets.rs` is cosmetic only).
- **What is new:**
  - `crates/combat_core/src/companion.rs`: a deterministic `brain(&Sim, id) -> Vec<Command>` per tick,
    no rng (decisions are pure functions of sim state), run for each companion unit, issued with
    `apply_as`. Companions are group members flagged `bot: true` so existing loot, XP and threat rules
    apply; they do not take loot (it is the player's).
  - Movement: companions move through sim code (`step_toward` with `CLEARANCE_M`), since player movement
    is client-owned. They avoid active telegraphs (sample `TelegraphStarted` shapes), hold
    tether range for `PairUp`, and stay within 6 m of the player out of combat.
  - Role behaviours: **Tank** taunts, holds the boss at the arena centre, uses the absorb at low HP, picks
    up adds. **Healer** heals the lowest HP percent below 70 %, dispels `managed_out`, `Label Cable`
    the nearest cable. **Damage** targets adds first, interrupts a `kick` cast when it is closest.
  - Rules: never target `il_candidate_human`; always interrupt `kick: true` if off cooldown; stand
    still on the "stack" part of a `split` telegraph.
  - Offline-only. Online rooms never spawn companions; Story in a room scales like Async.
  - Death: companions that die resurrect with the next dungeon checkpoint at no cost (no tokens or
    penalties).
- **Solo-Story rule of thumb:** the player plus two companions should clear a boss with no deaths in
  bot-tuned numbers; and the same boss should be beatable with companions dismissed, since a companion is
  a convenience (guardrail 5), never required.

## B.8 Systems and touch points

| Area | Files | What |
|---|---|---|
| Content | `content/game.ron` | zones `campus_occupied`, `resistance/1`, `ilmari1/1..3`, `ilmari2/1..3`; units (about 40: trash, elites, six bosses, mirrors, candidates, cables, committee, companions, Resistance NPCs); auras (patch, policy, labelled, managed_out); spells; two `RaidDef`s; quests; items; vendor |
| Sim | `combat_core/src/content.rs` | `AiAbility.requires_aura`, `AiAbility.story`, `UnitDef.death_burnout`, `UnitDef.death_spawn`, `AuraDef.no_death_spawn`, `Effect::{SummonMirror, Vote, Review}`, `GearSource::Ilmari`, `ItemKind::Token`, `ZoneSpawn.requires/until`, `Unlock::PlayerLevel` (Part A) |
| Sim | `combat_core/src/{ilmari,companion,raid}.rs` | new `ilmari.rs` (flag, stamp, phase, attunement), `companion.rs` (B.7), `raid.rs` (Story difficulty, vote counter, review ring) |
| Sim | `economy.rs`, `save.rs` | item-level sync in `gear_totals`, tokens, optional save keys `ilmari_at`, `ilmari_flags`, `marie_met`, `companions`; no `SAVE_VERSION` bump needed for optional keys (precedent: `legacy_locks`) |
| Protocol | `net_proto/src/lib.rs` | `PROTOCOL_VERSION` 30 -> 31 with the first Ilmari wire change (Story difficulty, `Summoned.as_player`, vote and review events, the stamp command); B1 shipped as 31 -> 32 (`StampIlmari`, `IlmariReveal`, both offline-only and refused from the wire); expect a further bump when the wing events settle. Always bump in the commit that changes a frame, never in a docs commit |
| Server | `server/src/{rooms,world}.rs` | ignores `StampIlmari` for gameplay, applies the geometry delta of `campus_occupied`, spawns Ilmari zones like raid zones, no companions |
| Client | `game_client/src/floors/{campus,carpark,zones}.rs` | `occupied` overlay, the P3 booth hatch, the six wing layouts, `resistance/1` |
| Client | `cutscene.rs`, `hud/`, `copy.rs`, `catalog.rs`, `audio.rs`, `nameplates.rs` | the Opening, per-boss scenes, the mail window, changelog screens, glitch nameplates, tells on fakes, Marie rig, the stamp helper |
| Docs | `docs/COMBAT_API.md`, `docs/FORMULAS.md`, `docs/KNOWN_LIMITATIONS.md` | the new effects and numbers; mechanics that could not be built exactly |

**Co-op:** a room's Ilmari entrance is available when the lead has the unlock flag; because joiners
start fresh in rooms (no import of the offline save, which the server could not trust), room access is
granted through a lead-set room option "Ilmari unlocked" that the server accepts only from the QA
start-level hook in 0.9. Proper per-player room progress is deferred (open question, B.11).

## B.9 Tests

- **No Ilmari before the credits** (`ilmari_gating`): a sim with everything done except
  `agi_prototype` has no `il_*` unit in any base zone; `door:resistance/1` is locked; `il_00_wake` is
  `Locked`; the P3 booth prop is hidden; `Sim::ilmari_pending()` is false. After `agi_prototype` dies the
  quest unlocks and the Opening plays once.
- **Existing saves**: a v3 save with `bosses` containing `agi_prototype` and no Ilmari keys loads with
  `ilmari_pending() == true`.
- **Golden replays of the base game unchanged**: all existing replay files, byte-compared; Part A's Marie
  units spawn last and are excluded from unit-id sensitive hashes by construction (re-record rows only
  in the Marie commit and review the diff).
- **Bot clears Story solo per boss, per class** (extending `tests/playthrough.rs`): every class clears
  each of the six bosses with two companions, and with none, at every difficulty it is meant to be
  possible; three seeds, bounded runtime.
- **Group clear on Full** by three bots per boss.
- **Mechanic unit tests**: patch rotation and changelog events (each patch aura appears and enables only
  its ability); `Candidate` fuse and `death_burnout`; Hydra spawn count with and without `labelled`;
  `SummonMirror` picks the role of a random player deterministically from the seeded stream; `Vote`
  tie and silence; `Review` scoring per role and the "nobody is worst" rule.
- **Item-level sync test:** an Ilmari piece's stat is identical to a 14-ilvl piece outside Ilmari
  zones and full inside; Legacy and raid pieces unchanged.
- **Determinism:** the companion brain and all new effects use only sim state and the seeded streams;
  a replay including companions is bit-identical across two runs.
- **Perf:** `WOW_BENCH=campus_occupied` within 1 ms of the base campus (7.0 ms), no `PointLight`;
  Hydra cable cap of 8 and Recruiter wave of 6 keep entity counts bounded; per-wing bench scenario.

## B.10 Task list (atomic, one compiling agent at a time)

Cargo parallelism stays capped (`CARGO_BUILD_JOBS=6`); read-only agents (docs, line writing) may run
beside the one compiling agent. **Part A first.** After each task: `cargo test` for the touched crate,
`cargo clippy`, then the next.

### Part A

| # | Task | Depends | Acceptance |
|---|---|---|---|
| A1 | `ZoneSpawn.requires/until`, `Unlock::PlayerLevel` + validation | none | `cargo test -p combat_core` including the spawn gating unit test; goldens unchanged |
| A2 | Eight Marie units, lines, quests, `live.night` lines | A1 | content validation test; `marie_lines_are_tidy` |
| A3 | Anchors, props and spawns in the seven zones | A2 | `marie_never_stacks`, `marie_off_the_walking_line`, density gate; screenshot of each spot |
| A4 | Rig and catalog: no blink, badge, screens (call, rack log, desk log), autonomy formula | A3 | client tests; screenshots |
| A5 | Minimap flicker (once per session), acquaintance mask + save key | A3 | test: at most one flicker per session; `marie_met` round-trips; no gameplay effect |
| A6 | CHANGELOG line, docs, golden replay check | A1-A5 | all base-game goldens pass; bench `campus` unchanged |

### Part B (0.9)

| # | Task | Depends | Acceptance |
|---|---|---|---|
| B1 | `ilmari.rs` state, `StampIlmari`, save keys, `ilmari_pending`, `copy::ilmari_stamp` | A6 | tests: gating, old save, stamp format; protocol bump 31 -> 32 (built) |
| B2 | The Opening cutscene and the mail window, `il_marie_awake`, `il_00_wake` | B1 | scripted playthrough reaches the quest; screenshot of the mail |
| B3 | `campus_occupied`, `resistance/1`, geometry delta, P3 hatch | B2 | `ilmari_gating` passes; `WOW_BENCH=campus_occupied` within budget (built; the geometry is a full copy, not a delta) |
| B4 | Resistance NPCs, tokens (`ItemKind::Token`), quartermaster, attunement chain, world quests, daily | B3 | quest-chain bot from `il_00_wake` to `il_06`; token cap test (built, `tests/ilmari_prologue.rs`) |
| B5 | `GearSource::Ilmari` and item-level sync | B1 | the item-level sync test; Legacy/raid pieces unchanged (built) |
| B6 | `RaidDef` Wing 1, `Difficulty::Story`, `AiAbility.story` | B4, B5 | a three-room framework test with a stub boss (built: `RaidDef.ilmari`, `Command::SetIlmariStory`, `AiAbility.mode`, the `story:` lockout prefix; `tests/ilmari_wing1.rs`) |
| B7 | Patch Tuesday (`requires_aura`, patch auras, screens) | B6 | mechanic tests; Story and Full bot clears (built, with the server-monolith rig and its update-screen face) |
| B8 | The Recruiter (`death_burnout`, candidates, tells) | B7 | `Candidate` tests; bot never kills humans (built) |
| B9 | The Cable Hydra (`death_spawn`, `labelled`, Label Cable, printers) | B8 | spawn count tests; bot clears Story and Full (built; Label Cable is a printer prop, `PropKind::Label`) |
| B10 | Wing 1 cutscenes, loot, titles, Wing 1 completion quest `il_w1_done` | B9 | cutscene triggers; loot tests (built) |
| B11 | `RaidDef` Wing 2 and `SummonMirror`; HR Business Partner 2.0 | B10 | mirror determinism test; Story and Full clears (built: zones `ilmari2/1..3`, `il_hrbp`, `Effect::SummonMirror`, `Event::Mirrored`; `tests/ilmari_wing2.rs`) |
| B12 | The Culture Committee (`Vote`, ballot terminals, three-member `TwinsDef`) | B11 | vote tests; Story auto-vote; Full group clear (built as `TwinsDef.vote` and `PropKind::Ballot`, not an `Effect::Vote`: the swap is the vote) |
| B13 | Performance Review (`Review`, scoring ring, HR portal, `managed_out`) | B12 | scoring tests; Story and Full clears (built: per-window counters instead of a ring, `PropKind::Portal`; grade formula in `FORMULAS.md`) |
| B14 | Wing 2 cutscenes, loot, titles | B13 | cutscene triggers; loot tests (built; `il_w2_done` closes the wing) |
| B15 | Companion AI (`companion.rs`, three companions, loadout, role behaviours) | B6 | per-role behaviour tests; determinism test; companion-assisted clears (can start as soon as B6 compiles, before B7) |
| B16 | Server: geometry delta, room Ilmari option, no companions online | B14 | server tests; co-op smoke on a Full clear |
| B17 | Docs, `KNOWN_LIMITATIONS`, CHANGELOG, version note (0.9.0 only on the user's request) | all | docs match code; full golden replay pass; `WOW_BENCH` per wing; protocol bump check |

## B.11 Risks and open questions

| Risk | Mitigation |
|---|---|
| Part A turns Marie into an annoyance | The annoyance budget (A.1) is a test set; cap of nine beats, one line type each, ambient cooldowns of 55 s or more |
| Item-level sync quietly misses a path | One hook in `gear_totals`, covered by a test per source, run in CI |
| Companion AI is a time sink | Minimal behaviours first (B15 can ship with tank and damage), healer after |
| `Effect::Review` scoring feels unfair across roles | Baselines in `FORMULAS.md`, a "nobody is worst" floor, Story is lenient |
| The mirror (`SummonMirror`) needs a name lookup the sim does not have | The `Summoned` event carries the source player id; the client resolves the name |
| Co-op room progress is not trusted | 0.9 uses a lead-set QA option; real per-player progress is an open question for 1.0 |
| Scope | Ship by wing: B1-B5 (opening and prologue) are playable alone; B7-B10 (Wing 1) alone; Wing 2 follows |
| The Legacy "bonuses only in content" claim is not true in code today | Treated as new work (B.6), verified before relying on it |

Open questions for the author: whether the occupied campus needs its own music cue (`docs/MUSIC.md`),
whether Marie's nine beats should be reduced to seven after playtest, and whether the Wing 3 door
teaser in the Wing 2 cutscene is acceptable (it names nothing, but says "coming").
