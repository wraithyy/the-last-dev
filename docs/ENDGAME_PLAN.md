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

#### Status (2026-10-16): 3.1b VibeCon is built
- `raids` in game.ron is now a list (offsite, VibeCon); `Content::raid_of(zone)`; each raid names its tier `sets`. One `Lockout` holds every raid's bosses (keys are unique), so save and wire are unchanged.
- Engine: `TwinsDef::extra` / `swap_places` (linked bosses with N members, the open seat rotates; a pull on one pulls all, and they reset with the boss), `Effect::Detonate` (the unreviewed PR), `AuraDef::reverse_controls` (client mirrors movement; the sim only applies the aura).
- Client: `floors/vibecon.rs` (five rooms), campus registration entrance, cutscenes, three new looks, the Wi-Fi Router pet, prim rigs for the PR, the demo bug and AGI (Beta). No protocol bump (no wire change).
- Balance: `vibecon_takes_a_group_and_async_attendance_takes_one` (coop.rs); FORMULAS.md, Raid: VibeCon.

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

#### Status (2026-10-16): 4.1 Teambuilding is built
- Frame (`combat_core/src/minigame.rs`): `minigames` in game.ron (`MinigameDef`: key, zone, limit, department, three `MedalDef`s with score/flawless rules and rewards, one rules block). `Command::StartMinigame { key }` (group lead, campus; the Company Calendar should send the same) and `Command::LeaveMinigame`; the host follows `Sim::minigame_travel` (venue, then campus 10 s after the last run). Per-player `MinigameView` (routed), `MinigameResult` with the group score, best medal per player in the save (`minigames=`). E5 games add a `GameRun` variant and a rules block; the client frame (`game_client/src/minigame.rs`: invite card, header in the tracker slot, leave confirm on Esc, medal screen) only needs a view per game.
- Rules (`teambuilding.rs`): watchers are friendly zone units on fixed loops (`route:<r>:<n>` anchors), cones with sight lines, noise from measured speed, hiding spots, dwell objectives, canapes, four catch outcomes. Venue: `floors/teambuilding.rs`; entry from the Company Calendar board on the plaza (`PropKind::Calendar`, E5a; the old invite board became it).
- Protocol 18: the two commands and `Event::MinigameStarted`, `Minigame`, `MinigameCaught`, `MinigameEnded` (roundtrip and cap tests). New `GearSource::Minigame`.
- Skipped: the weekly "mandatory" event (the board is always open, per the E5 decision). Esc in a run opens the leave confirm, which also offers Pause.

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

#### Status (2026-10-16): E5a is built (calendar, 5.2 chains, 5.3 tickets)
- **Company Calendar** (`combat_core/src/calendar.rs`, `game_client/src/calendar.rs`): the Teambuilding invite board became the calendar board (`PropKind::Calendar`, prop `company_calendar`) beside the Welcome arch, across the arch from the Scrum board and on the walk from the parking lot (that spot reads better than squeezing it between the Scrum board, its bot and a planter). Calendar icon on the minimap and the map, a "!" while anything listed is unplayed, a one-line tracker hint on reaching level 2 (until opened or 4 min), the `.` key (every letter is bound), a Calendar entry in the touch Menu drawer and a "[Company Calendar]" button in `~/endgame`. One window, week-view rows in two groups; each row: name, joke, length, status, reward icons (legend under the list), one button. Esc closes it; the list scrolls (wheel, finger) and says "More below" until the end. Screens: `.playwright-mcp/calendar/`.
- **Hook for 5.1**: add the game's `MinigameDef` with the key already listed in `calendar.minigames` (`performance_review`, `reorg`, `tool_migration`, `hackathon`, `open_space_focus`); the row turns from "Coming soon" into Start/Play again with the best medal, and the button sends `Command::StartMinigame`. Content validation refuses a minigame that is not on the calendar. `rotation: false` lists everything; `true` lists one game and one chain per UTC day (`CalendarDef::listing`).
- **Chains (5.2)**: `calendar.chains` lists quest ids in order (each `requires` the previous). The sim hands out a chain's first quest from anywhere (`Sim::calendar_starts`, plain `AcceptQuest`); later steps go through their givers. `QuestReward` gained `rep: Some((dept, n))` and `burnout` (per mille, negative relieves; cause `BurnoutCause::Quest`). Expense Report, Exit Interview, Mandatory Training, Return to Office, Town Hall: 14 quests, 5 new campus NPCs, 4 titles, 5 trophies. Chain givers show no "!" for the first quest (the board carries it). The campaign balance tests leave the chains out as optional side content.
- **Jira tickets (5.3)**: three tier-free junk items with `ticket: true`; using the Scrum board or the calendar closes all of them for `TICKET_BOARD_MULT` (2) times the price (`Event::TicketsClosed`, a log line). Loot rows and tooltips say where to close them. No new HUD element.
- **Burnout**: the chains hook in through `QuestReward::burnout` (Expense Report +8%, Exit Interview -12%); Teambuilding keeps its medal relief.
- **Skipped for clutter**: calendar invites as debuffs. The aura row already carries meeting debuffs that feed burnout, and an invite you must decline needs a prompt in the middle of a fight; a new player would not read it in 5 seconds.
- Protocol 19: `Event::TicketsClosed`, `BurnoutCause::Quest` (roundtrip test). Saves unchanged: chain progress is the quest log, tickets are bag items (save roundtrip test).

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

#### Status (2026-10-16): 5.1 minigames are built
- All five are on the Company Calendar (each row went from "Coming soon" to Start) and run on the Teambuilding frame: the group lead starts one from the campus, every member plays their own run, the group score adds up, and the best medal goes into the save. Rules: `combat_core/src/corporate/` (`review.rs`, `reorg.rs`, `migration.rs`, `hackathon.rs`, `focus.rs`; shared pieces in `mod.rs`), content in `minigames` in game.ron (one rules block each, plus `hint` and `how` for the banner and the invite). Venues: `floors/corporate.rs`, five small rooms from the office kit (`review/1`, `reorg/1`, `migration/1`, `hackathon/1`, `openspace/1`); the group comes back to the calendar board (`door:<venue>` on the campus).
- Frame additions (all games share them): a choice card (`MinigameView::card`: who, text, up to four options; answered with `Command::MinigamePick { seq, option }`, a stale `seq` does nothing), header lines and up to two bars (`lines`, `gauges` with an optional tick mark), name badges over units (`badges`, shown as the plate title), per-game world bits (`layout`), routed notes (`Event::MinigameNote`) and a one-line summary on the medal screen (`MinigameResult::summary`). The card takes the action slot keys 1-4 while it is up (spells wait); on touch it sits between the header and the buttons.
- **Performance Review** (~4 min, HR): nine questions with four answers each. Every answer has a tone (honest, buzzwords, stalling, agreeing, the raise, silence after 20 s): minutes of the manager's calendar burned, patience spent or restored, a push on the "Meets Expectations" gauge, which drifts back to 3 of 5 at 0.3 stars a second whatever you said. The same tone twice in a row costs 1.5 times the patience (a restore halves); at 0 patience the manager walks out and only half the calendar counts. Score 10 per minute; best line 46 min. Bronze "Meets Expectations" (any finish, the title), Silver "Ran Over the Hard Stop" (32 min, a mug), Gold "Booked a Follow-Up" (42 min and the manager stays). Every ending: "3 out of 5, no budget for raises".
- **Reorg** (~3-4 min, Engineering): five rounds of up to 45 s. Each reorg moves six colleagues to six of eight landmarked desks, stands about half of six partitions and deals new names and roles (badges on the plates); one of them is your manager. Walking up shows a colleague's clue: one says where your manager sits, one their role, one repeats the old org chart (false now), the others know nothing. "Are you my manager?" settles it: 150 + 4 per second left, a wrong guess -50 and 2.5 s. The round ends 6 s after everyone found theirs. Bronze 150, Silver 900 (title "Reports to a Dotted Line"), Gold 1150 with no wrong guess.
- **Tool Migration** (~4 min, Internal IT): 20 tickets from TICKETRON 2009 (west) to FlowSphere Cloud (east), 12 m apart; export at the old one, then 2 to 4 mandatory fields at the new one (component, impact, urgency, source, category), each a four-option quick pick whose answer is in the ticket's text. A wrong pick is a validation error and 3 s of spinner. A marker floats over the terminal you need next. Then the CTO's email: the company is migrating back. Score 1500 - 75 per error - 2 per second. Silver 850 (title "Migrated Twice", 60 Kudos), Gold 1000 with no error (a lanyard).
- **Hackathon** (~4 min, Marketing): 24 hours, one every 10 s, from Saturday 10:00. Focus drains 1 per second (2 between 02:00 and 06:00); walking over a pizza box (+22, +12 when cold after 01:00) or an energy drink (+35) refills it unless you are at 85 or more, and the spot comes back 30 s later. The demo builds 0.55% a second at full focus. At hour 24 you are on the stage and the demo crashes regardless. Score 10 per percent + 5 per snack; every medal gives the trophy "Hackathon Trophy (Plastic)" with the title "Hackathon Finalist" (once), Silver 750, Gold 1000 (a hoodie).
- **Open-Space Focus** (~3 min, Engineering): hold 60 s of deep work at the pod. Four colleagues walk loops from the edges to the pod (Teambuilding's walkers and cones): "got a minute?" (In a meeting), the speakerphone account manager and the fire drill test (headphones), the birthday cake procession (duck). The card counts each arrival down; a defence lasts 2.5 s and rests 4.5 s, so it is about timing. An arrival with the wrong or no defence resets the streak; away from the desk it pauses. Score 1500 + 15 per deflection - 150 per interruption - 4 per second. Silver 900, Gold 1200 never interrupted (headphones and the title "Do Not Disturb").
- Rewards: each medal pays standing and burnout relief every time (as the Teambuilding does); items, titles, Kudos and the trophy land once per character (items are skipped when owned, titles come from achievements).
- Protocol 20: `Command::MinigamePick`, `Event::MinigameNote`, the view's new fields and `MinigameResult::summary` (roundtrip and cap tests: at most 4 options, 4 lines, 512-byte texts). Saves unchanged: the best medal per key already goes into `minigames=`.
- Tests: `combat_core/tests/corporate.rs` (27: start, core rule, scoring and medals, solo runs to the end, co-op group score, rewards once, per game; the bots play from the view only), unit tests in each module (the review's best line by dynamic programming, the reorg's clue dealing, the hackathon clock), `floors/corporate.rs` (anchors free and reachable). Screens: `.playwright-mcp/minigames/`.
- Clutter: no new permanent HUD element. The header replaces the tracker during a run, the card appears only when there is something to decide, notes use the existing flash line and log, badges use the existing plates.
- Simplified: the reorg's desks and partitions swap during a 4 s "reorg in progress" instead of being carried; colleagues stand at their desks (no sitting); the partitions block the client's movement but not sight (the game has no sight rule). In a group review every member has the same manager, double-booked, so the bubbles mix. "Duck" is a button with no crouch animation. The hackathon demo builds wherever you are (no desk needed).

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
