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
