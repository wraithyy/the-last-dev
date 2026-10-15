# Endgame plan (level 10)

## Context
Players hit level 10 in the Lab after a few hours and then run out of goals. The user chose an
endgame at the level cap over an expansion to level 20 (2026-10-15). The goal is a long tail of
play that reuses existing zones and systems, works solo and in co-op, and stays offline-first.

## Requirements
1. **Legacy Mode** (heroic): every tower floor, data center and lab zone can be re-entered at
   level 10 as "Legacy Mode" (the code nobody dares to touch). Enemies scale to 10+, one extra
   affix per zone, better ilvl drops, a weekly lockout for the boss bonus loot.
2. **Raid "All-Hands Offsite"**: a new multi-boss instance for 3-5 players (solo possible on an
   easier "Async Attendance" difficulty). It has 4 bosses with real mechanics:
   - Icebreaker Facilitator: forced team-building.
   - The Synergy Twins.
   - Keynote Speaker: an AI hologram, a wall of slides, interrupts.
   - The Founder: final boss with 3 phases and a "We Are a Family" wipe mechanic.
   Unique tier set and legendaries, achievements and titles.
3. **Department reputation**: Engineering, Security, HR, Marketing, Internal IT, Sales.
   - Gained from Legacy kills, dailies, the world boss and the raid.
   - Tiers Hostile .. Exalted ("Ignored", "Tolerated", "Cc'd", "Invited", "Trusted",
     "Exalted / Gets the Good Monitor").
   - Rewards: vendor items, cosmetics, pets, titles, a scooter skin.
4. **Promotion (prestige levels)**: after level 10 XP keeps filling "Promotion" ranks (Senior,
   Staff, Principal, Distinguished, Fellow...). Each rank gives one small Paragon point
   (capped stat or utility) plus a title. Nothing that breaks the balance; cosmetic-heavy.
5. **Weekly Sprint ladder**: a fixed weekly seed and affix set for Sprint mode, local
   leaderboard offline. Online, a per-room board, with a server board later if wanted.
6. **Endgame UI**:
   - `~/endgame` panel (or a Career tab): lockouts, reputations, promotion rank, weekly seed.
   - Tracker lines for weekly goals.

## Risks
| Risk | Mitigation |
|---|---|
| game.ron grows huge and edits race | One agent at a time on content; atomic writes; content tests |
| Balance: Legacy gear trivialises the raid | ilvl budget test per source tier |
| Co-op desync with new zones/lockouts | Server-authoritative lockouts per room, protocol bump with roundtrip tests |
| Save compatibility | Optional save keys only, no SAVE_VERSION bump unless needed |
| Scope creep | Ship in 3 waves; each wave deployable alone |

## Tasks

### Wave E1: progression backbone

#### Task 1.1: Promotion ranks after level 10
- **Agent**: general-purpose (sonnet)
- **Files**: crates/combat_core/src/{sim.rs,save.rs}, content/game.ron, hud XP bar, docs/FORMULAS.md
- **Depends on**: items wave committed
- **Acceptance**: tests for rank XP curve, save roundtrip; XP bar shows rank progress
- **Prompt seed**: Add post-cap Promotion ranks with small Paragon points and titles; XP bar shows rank progress.

#### Task 1.2: Department reputation
- **Agent**: general-purpose (sonnet)
- **Files**: new crates/combat_core/src/reputation.rs, content/game.ron, net_proto, client panel
- **Depends on**: 1.1
- **Acceptance**: rep gain/tier tests, vendor gating by tier, online sync roundtrip
- **Prompt seed**: Add six department reputations with tiers, gains from kills/dailies/boss and rep-gated vendor items.

#### Task 1.3: `~/endgame` panel and tracker lines
- **Agent**: general-purpose (sonnet)
- **Files**: crates/game_client/src/hud/endgame.rs, keybinds
- **Depends on**: 1.1, 1.2
- **Acceptance**: native screenshots, keybind works, touch layout
- **Prompt seed**: Build the endgame panel showing rank, reputations, lockouts and the weekly seed.

#### Status (2026-10-15): E1 and the burnout meter are built
- 1.1: ten ranks, curve `30000 + 6000 * r`, Paragon +1%/rank capped at +5%, titles as `rank_<k>` achievements; the XP bar shows rank progress.
- 1.2: departments, tiers and gains as in FORMULAS.md; six rep-gated vendor items. Legacy kills and the raid do not exist yet, so kills at level 10 in a department's zones stand in for "Legacy kills".
- 1.3: `~/endgame` (key ~, Esc menu, touch Menu). Lockouts and the weekly seed rows say what is coming (E2/E3) instead of showing empty tables. No tracker lines yet: nothing weekly exists to track beyond the Sprint Goal, which already has one.
- 5.3/3 burnout: strip under the mana bar with tooltip; props reuse the office (foosball, ping-pong, vending, coffee, Sad Bench) plus three new spots (bean bag, unbookable quiet-room booth, Biscuit the office dog). The table "mini-round" is a seeded score line, not an interactive minigame. Yawns and mutters reuse speech bubbles; no yawn animation. Online characters start each room at 0 burnout (the holiday at session start).
- Clutter review: only the 5 px strip is new on the HUD (1280x720 and 844x390 touch screenshots in `.playwright-mcp/endgame/`); the touch Menu drawer went from 4 to 5 columns to fit "Endgame" in two rows; no new dock button or hint-bar entry.

### Wave E2: Legacy Mode

#### Task 2.1: Legacy Mode zone variants
- **Agent**: general-purpose (opus)
- **Files**: crates/combat_core/src/{zones.rs,sprint.rs (affix reuse)}, content/game.ron
- **Depends on**: E1
- **Acceptance**: scaled enemies, affix applied, weekly lockout tests; 3-bot clear time test
- **Prompt seed**: Add Legacy Mode re-entry for all combat zones at level 10 with an affix, better loot and weekly boss lockouts.

#### Task 2.2: Legacy loot tier
- **Agent**: general-purpose (sonnet)
- **Files**: content/game.ron, tests/kudos.rs
- **Depends on**: 2.1
- **Acceptance**: ilvl budget test passes, every item reachable
- **Prompt seed**: Add a Legacy ilvl tier to drops and rep vendors within the stat budget.

#### Status (2026-10-15): E2 is built
- 2.1: `legacy` in game.ron and `combat_core/src/legacy.rs`. The lead switches the group with `Command::SetLegacy` (everyone at 10, no sprint); all nine combat zones have a variant with one fixed Sprint affix, reusing the affix runtime (`sprint::Fx`, shared by runs and Legacy floors). Enemies spawn at 11 (bosses 12) with per-zone HP/damage knobs; Legacy floors play in their own world (clears, elevators). Kills at 10 pay rep only on Legacy floors where a variant exists (the E1 stand-in stays for the campus). Weekly boss lockout per room (server sim) or per save (`legacy_locks`), ISO week. Protocol 16: `Command::SetLegacy`, `Event::Legacy` (view: on, zone, week, locked) and `Event::LegacyRefused`.
- 2.2: 23 Legacy pieces at ilvl 12 (`source: Legacy`), three new looks (CRT monitor helm, Perl monk robe, floppy disk badge); the Sprint chest stays on the top campaign tier.
- Client: the "Normal | Legacy" switch near doors and elevators (lead clicks or taps, others see the call), a Legacy banner and the affix in the radar footer, lockout chips in `~/endgame`. Screenshots in `.playwright-mcp/legacy/`.
- Balance: `three_players_clear_every_legacy_floor_and_one_does_not_trivially` (coop.rs); numbers in FORMULAS.md, Legacy Mode.

### Wave E3: Raid and weekly ladder

#### Task 3.1: Raid instance "All-Hands Offsite"
- **Agent**: general-purpose (opus)
- **Files**: crates/game_client/src/floors/offsite.rs, content/game.ron, combat_core mechanics
- **Depends on**: E2
- **Acceptance**: 4 bosses beatable by 3 bots at ilvl X, not trivially by 1; screenshots
- **Prompt seed**: Build the 4-boss raid with organic layout, mechanics, tier set and legendaries.

#### Task 3.1b: Second raid "VibeCon"
- **Agent**: general-purpose (opus)
- **Files**: crates/game_client/src/floors/vibecon.rs, content/game.ron, combat_core mechanics
- **Depends on**: 3.1 (reuse its raid framework, lockouts and difficulty modes)
- **Acceptance**: same bar as 3.1; a distinct look (expo hall with booths, keynote stage, swag tables, neon lanyards)
- **Prompt seed**: Build VibeCon, a tech-conference raid full of vibe coders and their kind.
  - Trash: Prompt Engineers, "10x" Founders pitching at you, Booth Babes of the Agent Framework of the Week, Swag Goblins, LinkedIn Influencers filming you, Hackathon Zombies, Web3 survivors who pivoted to AI.
  - Boss ideas:
    - The Vibe Coder: ships while you fight; spawns unreviewed PR adds that explode in prod.
    - The Prompt Whisperer: reverses your controls with "ignore previous instructions".
    - The Demo God: fake-it live demo where the fight breaks unless it is interrupted.
    - The Hype Panel: 3 linked panelists talking over each other.
    - Final boss, The Keynote AGI-Announcer: "This changes everything", with "One More Thing" phases.
  - Loot: conference swag set (lanyard, branded hoodie, VR headset), "Speaker" title, pets such as Conference Wifi Router (never works).

#### Task 3.2: Raid lore and cutscenes
- **Agent**: general-purpose (sonnet)
- **Files**: crates/game_client/src/cutscene.rs, content lines
- **Depends on**: 3.1
- **Acceptance**: cutscene trigger tests, screenshots
- **Prompt seed**: Add raid cutscenes and boss lines in the established satirical voice.

#### Task 3.3: Weekly Sprint ladder
- **Agent**: general-purpose (sonnet)
- **Files**: crates/combat_core/src/sprint.rs, client sprint board
- **Depends on**: E1
- **Acceptance**: weekly seed determinism test, leaderboard persistence
- **Prompt seed**: Add a fixed weekly Sprint seed with its own leaderboard.

#### Status (2026-10-16): E3 (3.1, 3.2, 3.3) is built; 3.1b VibeCon comes next
- 3.1: `raid` in game.ron, `combat_core/src/raid.rs`, `floors/offsite.rs`. Four rooms behind the campus shuttle stop, difficulty by group size at each raid load (Normal 3+, Async Attendance below), weekly lockout per encounter and difficulty reusing the Legacy `Lockout`, a locked boss does not spawn. New engine pieces: `Effect::PairUp` (tethers), linked-HP twins with a shield swap, `TelegraphDef::gap` (blank slide). Raid tier at ilvl 14 (16 set pieces, 4 Legendaries, three new looks), six achievements/titles, reputation with every department, burnout relief.
- 3.2: intro, one scene per boss and closing remarks in `cutscene.rs`.
- 3.3: `Command::StartWeeklySprint`, `weekly_sprint(def, week)`, ladder in the planner's Weekly tab and the `~/endgame` row; local ladder in the save, per-room board online.
- Protocol 17: `Command::StartWeeklySprint`, `Event::Raid` (zone, async, week, locked, tethers), `Event::Ladder`. Save keys (optional): `raid_locks`, `ladder`.
- Balance: `the_raid_takes_a_group_and_async_attendance_takes_one` (coop.rs); numbers in FORMULAS.md, Raid.
- Clutter: no new HUD element; the difficulty uses the banner slot, tethers are a cord in the world, the shield a bubble, lockouts are chips in `~/endgame`.

### Wave E4: Teambuilding minigame

#### Task 4.1: Stealth minigame "Mandatory Fun: Teambuilding"
- **Agent**: general-purpose (opus)
- **Files**: new crates/combat_core/src/teambuilding.rs, crates/game_client/src/floors/teambuilding.rs, content/game.ron
- **Depends on**: E1 (rewards into reputation/promotion); can start after the items wave
- **Acceptance**:
  - sim tests: patrol paths, vision cones, catch outcomes (delay vs reset to start), objective completion, scoring
  - native screenshots; a playable solo run in about 5-8 minutes
  - co-op: each player sneaks on their own and a group score adds up
- **Prompt seed**: Build a stealth minigame at a company teambuilding event.
  - The player sneaks through the venue (garden party / hotel conference floor with buffet, bar, karaoke corner, trust-fall lawn) collecting food and finishing small objectives:
    - steal the last good canape
    - grab a drink from the open bar
    - pocket swag
    - escape the group photo
    - find a quiet corner to answer a prod incident
  - Avoid people who patrol in loops with visible vision cones:
    - Managers: being caught sends you back to the start ("Let's circle back to the beginning").
    - "Let me tell you about AI" colleagues: catch you in a conversation that roots you for several seconds, with a skippable dialogue of hilariously bad AI takes.
    - The HR Fun Committee: forces you into an icebreaker detour.
    - The CEO: rare, sweeps the room, instant reset.
  - Hiding spots: behind plants, under the buffet table, in the toilet queue, behind a roll-up banner. Crouch/sneak speed, noise from running.
  - Scoring by time plus collected items. Medals: Bronze "Attended", Silver "Networked", Gold "Was Never Seen".
  - Rewards: cosmetics, a "Ghosted the Teambuilding" title, a pet (Party Leftovers Container), reputation.
  - Entry: a "Teambuilding" invite board on campus, plus a weekly "mandatory" event.

#### Task 4.2: Easter egg "Body Leasing" (small)
- **Agent**: general-purpose (sonnet)
- **Files**: content/game.ron, a tiny floor in crates/game_client/src/floors/ (reuse office kit)
- **Depends on**: none (small, self-contained)
- **Acceptance**: reachable from a campus NPC, completable in about 3 minutes, tests for the quest chain, one screenshot
- **Prompt seed**: Add a short satirical side trip about body leasing.
  - A campus "Account Manager" NPC tells you you've been "allocated to a strategic client". The player is teleported to a generic, fictional client office: "Very Large Insurance & Banking Group" (no real company names or logos).
  - The office is grey, quiet and beige: rows of empty desks, a dead plant, a printer from 2009, a "Badge pending" door.
  - Nobody knows you. NPCs answer "Who are you?", "Are you the new external?" or "Ask your vendor".
  - There are no enemies and nothing happens. The only quests are absurd:
    - wait for access rights (a progress bar that fills painfully slowly)
    - attend a meeting where you're muted and nobody calls on you
    - fill in a timesheet with "8h, analysis"
  - Exit after the timesheet is approved: back to campus with the title "External Resource" and an achievement "Billable" ("Nothing happened. You billed it anyway.").
  - Keep it short and deadpan; the joke is the emptiness.

### Wave E5: Corporate life (side content)

**Design rule (user request): it must not feel cluttered.**
- All side activities share one entry point: a "Company Calendar" board on campus.
  - For now (user decision 2026-10-15) every minigame and every quest chain is available at all times, listed on the board.
  - Rotation is a later switch: keep a `rotation` flag in content, default off, so it can later show one or two per day like the daily board.
- **Company Calendar UX (user priority: easy to find, clear, easy to use):**
  - Findable:
    - The board stands on the main campus plaza next to the Scrum board.
    - It has a calendar icon on the minimap and the map, plus a "!" marker while something is unplayed.
    - A one-line hint goes into the tracker once at level 2.
    - It is also reachable from the touch menu and the `~/endgame` panel, through a "Calendar" button and an optional keybind.
  - Layout: one window shaped like a week-view calendar, with two groups, "Minigames" and "Quest chains". Each row is one entry and shows:
    - a name and a one-line joke description
    - a length estimate ("~4 min")
    - a status: new, in progress, or best medal
    - the reward icons
    - one big "Join" or "Start" button
    - No nested menus.
  - Starting an entry teleports you or starts it directly. Quest chains add themselves to the tracker. Esc always exits back to campus with a confirm.
  - Clutter check: the window must read cleanly at 1280x720 and on a phone (844x390), scrolling if needed. A screenshot of both is part of the acceptance criteria.
- Nothing gets its own HUD element unless it replaces one.
- Every item ships only if it passes a clutter review: a screenshot of the HUD at 1280x720 and on touch, plus the question "would a new player understand this in 5 seconds?". If an item makes the game feel busier without adding fun, skip or cut it and record that here.
- Order: minigames first, quest chains second, background systems last and only if the screen stays calm.

#### Task 5.1: Minigames (one per event day, from the calendar)
- **Agent**: general-purpose (opus), one minigame per task
- **Files**: crates/combat_core/src/corporate/*.rs, crates/game_client/src/floors/* or overlays, content/game.ron
- **Depends on**: E4 (shares the minigame frame of Teambuilding: start, timer, score, medals)
- **Acceptance**: sim tests per minigame, 3-5 minute runs, screenshots, clutter review
- **Minigames**:
  1. **Performance Review**: a dialogue "boss fight" with your manager. You pick answers, and the "Meets Expectations" gauge always drifts back to the middle. Every ending is "3 out of 5, no budget for raises this year". Title "Meets Expectations".
  2. **Reorg**: the office layout shuffles every couple of minutes and NPCs swap names and roles. Quest: "find out who you report to now".
  3. **Tool Migration**: move 20 tickets from the old system to the new one, each with different mandatory fields. At the end the company announces it is migrating back.
  4. **Hackathon**: a 24-hour countdown in fast time. Collect pizza and energy drinks while a demo builds itself, then crashes live on stage. Prize: a plastic trophy (a real trophy item).
  5. **Open-Space Focus**: hold 60 s of uninterrupted work while "got a minute?" colleagues, loud phone calls and a birthday cake approach. A defend-in-place variant of the Teambuilding stealth rules.

#### Task 5.2: Short quest chains (rotating on the calendar)
- **Agent**: general-purpose (sonnet)
- **Files**: content/game.ron, small dialogue additions
- **Depends on**: 5.1 calendar board
- **Acceptance**: quest-chain tests, each chain about 3-5 minutes
- **Chains**:
  1. **Expense Report**: a 3-euro coffee needs 4 approvals, a scan of a receipt you don't have, and a "business value" justification.
  2. **Exit Interview**: a colleague who resigned speaks honestly for the first time in 5 years. The best lines in the game.
  3. **Mandatory Training**: click through 40 phishing slides; at the end IT sends you a phishing mail and Security falls for it.
  4. **Return to Office**: you commute in and everyone sits on a Teams call with headphones, talking to each other through cameras.
  5. **Town Hall**: the CEO announces a "record quarter" and "cost optimisation" in the same sentence. The Q&A has 200 questions; one pre-approved question is answered.

#### Task 5.3: Background systems (optional, last, clutter-gated)
- **Agent**: general-purpose (sonnet)
- **Depends on**: 5.1, 5.2 and a clutter review of the HUD at that point
- **Acceptance**: no new permanent HUD element unless it replaces one; otherwise skip
- **Systems**:
  1. **Jira tickets as loot**: corpses drop "Blocker", "Won't Fix" and "Duplicate" tickets that turn in for Kudos. Reuses the junk/loot UI, so it adds no clutter. Most likely to ship.
  2. **Calendar invites as debuffs**: NPCs throw meeting invites at you; declining costs reputation. Reuses the aura row. Ship only if readable.
  3. **Burnout meter (MUST SHIP, user favourite)**
     - Grows with meetings, minigames, deaths, night crunch and long play sessions.
     - At high burnout the character slows, yawns and mutters lines, sometimes hits "reply all" on chat, and the hair-loss stages advance faster.
     - Coffee, a holiday (logging off for a while, measured by the real clock), Friday afternoon, or the rubber duck pet heal it.
     - Office props also heal it, reusing the existing interactive props. Each has a short cooldown so it can't be spammed:
       - table football: a quick mini-round
       - ping-pong table
       - bean bag nap
       - the vending machine
       - the rooftop or "quiet room"
       - petting the office dog, if one exists
     - Killing management also lowers it: every manager, director, HR or marketing lead, CEO, and world-boss board member defeated takes a chunk off. Bosses take more. Each kill shows a small floating "-5% burnout" in green, with a satirical line in the combat log ("Therapeutic.").
     - It must not add clutter. Show it as a thin third strip under the player frame's mana bar, or as a tint and pip on the portrait. No separate window.
     - Tooltip: "Burnout 72%. Have you tried a wellness webinar?"
     - Ship it earlier than the rest of 5.3, together with E1.

## Out of scope
- New level cap (11-20) and new base classes.
- Crafting (explicitly not wanted).
- Server-side persistent characters or global leaderboards.
- Paid or account features.

## Verification
- `./scripts/check.sh` exit 0 after every wave.
- Native screenshots for new UI and zones (WOW_MUTE=1).
- Web test in a fresh muted browser; two clients for co-op lockouts and raid.
- Live check on Pages + Render after each deploy (protocol version probe).
