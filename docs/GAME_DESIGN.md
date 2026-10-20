# The Last Dev: game design

You are the last senior engineer in a company that has decided AI is the answer to every question,
including the ones nobody asked. Fight your way up the office tower, floor by floor, to the CEO.
Everything is satire of pointless corporate AI pushing; no real companies or people are named.

## Structure

A linear tower, one floor per zone. Each floor has trash packs, an optional mini-boss and a boss.
Killing the floor boss unlocks the elevator to the next floor. Dying respawns you at the floor's
elevator with the floor's trash state kept (bosses reset).

| Floor | Name | Levels | Theme |
|---|---|---|---|
| 1 | Open Space | 1-2 | Cubicles, vibe coders, AI forwarding |
| 2 | Meeting Rooms | 3-4 | Glass boxes, stand-ups that never end |
| 3 | Sales & Customer Success | 5-6 | Demo stands, promises, scope creep |
| 4 | PMO | 7-8 | Gantt walls, deadlines, consultants |
| 5 | C-Level Penthouse | 9-10 | Marble, AI shrine, the CEO |

## Enemies

Trash (pack of 2-4, social aggro within a small radius):

| Enemy | Floor | Gimmick |
|---|---|---|
| MeatProxy | 1-5 | A human who forwards your messages to an AI and pastes back the answer. Casts *Forwarded Reply* (copies the last spell you cast at it, at half damage). Signature trash, appears on every floor. |
| Vibe-Coding Junior | 1 | Never read what it generated, wants a review. *Review Request*: channel that applies *Review Fatigue* (cast time +20%). |
| Copilot Drone | 1, 5 | Weak, fast melee swarm. |
| Scrum Master | 2 | *Daily Stand-up*: short AoE stun every 15 s. |
| Agile Coach | 2 | *Retrospective*: buffs allies' damage, does no damage itself. |
| Middle Manager | 2, 4 | *Quick Sync*: 2 s stun. |
| Sales Rep | 3 | *Overpromise*: stacking debuff, each stack +5% damage taken. |
| Account Manager | 3 | Heals the Sales Rep: *Relationship Building*. |
| LinkedIn Influencer | 3, 5 | *Thought Leadership*: damage over time. |
| Project Manager | 4 | *Deadline*: debuff that explodes after 8 s unless dispelled by *Rollback*. |
| Product Owner | 4 | *Reprioritize*: swaps its threat target. |
| Consultant | 4 | Huge HP, low damage, *Billable Hours*: heals itself over time. |
| AI Ethicist | 5 | *Ethics Review*: a 30 s cast that never finishes and does nothing. |
| Crypto Bro | 5 | *Rug Pull*: knockback. |
| Chatbot Minion | 5 | Summoned by bosses, explodes on death. |

Bosses:

| Floor | Boss | Mechanics |
|---|---|---|
| 1 | The Vibe Coder (junior) | Summons *Unreviewed PR* adds every 20 s; *4,000-Line Diff* cast must be interrupted (Reject PR). |
| 2 | The Analyst | The analysis was written by Claude and he never read it. *Requirements Change*: silences one spell school for 6 s; *Unread Spec*: debuff, damage increases each time you cast the same spell twice in a row. |
| 3 | The Customer | *Scope Creep*: stacking self-buff every 15 s (soft enrage); *Should Have Been Done Yesterday*: big cast, interruptible. |
| 4 | VP of Synergy | Phase 1 with two Middle Manager adds; phase 2 at 50%: *Reorg* teleports you and resets add threat. |
| 5 | CEO | Three phases. P1: *Vision Statement* (three buzzword beams sweep the room, dodge), *Synergy Circle* (stack to share, solo takes it whole), must-interrupt *Pivot to AGI* (room-wide hit plus chatbots if it lands). P2 at 60%: *Hire Consultants* (two adds whose *Billable Synergy* cuts his damage taken by 35% while they live), *Quarterly Earnings Call* (room-wide hit unless you hide behind one of three desks). P3 at 25%: *Golden Parachute* (+15% damage taken, he walks for the elevator and casts the must-interrupt *Severance Negotiation* heal there, then repeats), *Layoff Notice* tiles land around you every 5 s. Victory banner on death. |

## Enemy mechanics

Every zone mixes two or three mechanics with a floor telegraph and a counter (full rules in COMBAT_API.md, Enemy mechanics):

| Zone | Mechanics |
|---|---|
| Campus | *Package Drop Zone* circles (Delivery Robot), *Last-Mile Dash* lines (E-Scooter Executive); elite **Chief Happiness Officer**: summons ambassadors, *Trust Fall* circle, enrages at 35 s |
| Open Space | (unchanged: Vibe Coder adds and the interruptible 4,000-Line Diff) |
| Meeting Rooms | Scrum Master heals allies (*Stand-up Recovery*, must interrupt), *Meeting Room Booked* circles (Middle Manager), *Calendar Invite* spread circles with a dispellable DoT (Agile Coach) |
| Sales & Marketing | **Defensive Stakeholder** (laptop shield: -60% from the front, down while it types), *Company-Wide Reply-All* (Sales Rep, must interrupt); Head of Marketing enrages at 40 s |
| PMO | Defensive Stakeholder, *Mandatory All-Hands* (Product Owner, must interrupt), *Deadline* (dispellable) |
| C-Level Penthouse | *Pump and Dump* cones (Crypto Bro); elite **Chief of Staff**: clipboard shield, *Calendar Tetris* spread circles, enrages at 45 s; the CEO |
| Data Center | *Exhaust Vent* cones (Cooling Fan), *Sacrifice to the Cluster* (GPU Cultist, must interrupt), **Agent Orchestrator** spawning sub-agents until killed; elite **Shadow IT Server**: sweeping *Undocumented Cron Job* lines, dispellable *Root Access Revoked*, enrages at 50 s |
| Innovation Lab | *Laser Pointer Keynote* lines (Demo Presenter), *Down Round* (VC Partner, must interrupt); elite **Hype Cycle Analyst**: *Trough of Disillusionment* (get out), *Magic Quadrant* circles, enrages; the AGI adds *Scaling Laws* beams |

Every class has an interrupt by level 3 (Kill Query moved from 4 to 3); Rollback and Feature Flag Off dispel Magic debuffs. Elites (also HR Business Partner, Head of Marketing, CISO) drop their best trophy every time.

## Classes

Four roles, picked on first launch ("Choose your role"; Esc > Change role restarts the run). Each has ten spells unlocked over levels 1-9 and can finish the offline campaign solo.

| Class | Role | Kit (level) | Signature |
|---|---|---|---|
| Fullstack Engineer | RANGED | the spells below | knows everything a little, gets blamed for all of it |
| Frontend Developer | MELEE | Pixel Push 1, CSS Combo 1, !important 2, Lighthouse Audit 3, Hot Reload 4, Dark Mode 5, Responsive Layout 6, Infinite Scroll 7, Center the Div 8, Rewrite It in a New Framework 9 | stacks Specificity (5), cashes it in with !important; dash, self-heal defensive |
| Backend Engineer | TANK | Blocking Call 1, Rate Limiter 1, Who Touched Prod? 2, Connection Pool 3, Kill Query 3, Database Lock 5, Horizontal Scaling 6, Retry With Exponential Backoff 7, Cache Invalidation 8, DROP TABLE 9 | 350% threat, armour, taunt, AoE threat, temporary max HP |
| DevOps Engineer | HEALER | kill -9 1, Hotfix 1, SIGSTOP 2, Auto-scaling 3, Chaos Monkey 4, Revert Commit 5, Feature Flag Off 6, Circuit Breaker 7, Blue-Green Deploy 8, terraform destroy 9 | ally heals, HoT, group heal, restore HP of 5 s ago, own faster resurrect (Restart Pod) |

## Player progression

Levels 1-10, XP from kills (trash and bosses). Each level raises max HP and mana; new spells unlock.

| Level | Spell | Effect |
|---|---|---|
| 1 | Nitpick (auto attack), Code Review, Refactor | existing |
| 2 | Guardrails | existing absorb |
| 3 | Reject PR | existing interrupt |
| 4 | git blame | damage over time, spreads on death of the target (WoW-like DoT) |
| 5 | Rollback | instant self-heal, removes one harmful effect |
| 6 | On-Call Caffeine | 15 s haste buff, long cooldown |
| 7 | Rubber Duck | 3 s stun on non-boss target |
| 8 | Postmortem | execute: heavy damage on targets below 25% HP |
| 9 | Write It By Hand | long cast, huge damage, the AI-free ultimate |

Action bar grows to keys 1-0 (physical digit row and numpad).

## Gear, Kudos and vendors

Loot means something: every kill pays Kudos (the currency; `sp` in code and saves) and may drop gear of the zone's tier (ilvl 2/4/6/8/10), weighted toward your role; elites (HR Business Partner, Marketing Lead, CISO) drop more often, bosses always drop a piece and sometimes a named Legendary. Five slots are worn visibly on the character: Headgear (cap, beanie, headset, propeller hat, VR visor, shades, crown), Hoodie (tints the shirt, logo and glowing trims by tier), Keyboard (slung on the back, RGB at the top), Mug (right hand: mug, thermos, stein, chalice) and Lanyard (badge on the chest). Rarities read like a code review: Works On My Machine (grey), Passed CI (green), Merged (blue), Shipped to Prod (purple, glows), Approved in Code Review (orange, glows harder).

Stats come from a budget (rarity x item level, keyboards +25%) split by the piece's role package; the sheet names them in dev terms: Uptime (HP), Context Window (mana), Velocity (spell power), Throughput (% damage and healing), Hot Takes (crit), Caffeine (haste), Focus Time (mana per 5 s), Plausible Deniability (% less damage taken). Perks change how you play: echoes ("Pixel Push hits twice above 80% HP"), riders ("Who Touched Prod? taunts everything within 8 m", "Hotfix also cleanses"), cooldown cuts, lifesteal, on-hurt shields, and on-use gadgets on U / O / P. Four 4-piece sets (Burnout Collection, On-Call Rotation, Full-Stack Overflow, Pixel Perfect) give stats at 2 and alter a signature spell at 4.

Vendors: Procurement (campus plaza, "3-6 weeks") sells consumables (Ibuprofen, Energy Drink, Cold Brew, Leftover Catering Sandwich), All-Hands hoodie colours and tier 1-2 gear with a rotating Rare shelf; the IT Asset Store in Data Center Hall A sells tier 3-4 gear and a rotating Rare/Epic shelf. Vendor gear costs `base(rarity) * (3 * ilvl + 2)` Kudos and needs a level (`ilvl - 1`, Epics one more; shown greyed out with "Requires Level N"), so a tier's Rare costs one to two zones of play and Epics are a long goal; nobody can afford a whole shelf (docs/FORMULAS.md, Kudos). The wallet sits above the bottom-right dock as a gold "K" coin ("Kudos 1,240"), every gain floats up from it in gold, and the shop header, the character sheet and the Performance Review show the balance. Hovering (or tapping, on touch) any item at a vendor, in the bag or in the loot window shows its tooltip with the worn piece of that slot beside it: role-relevant stat deltas in green or red, perk and set changes, and a verdict ("Upgrade (according to the AI: definitely)"); vendor and bag icons carry a green up or red down arrow. Sell junk (Unread Jira Notification, Expired Free Pizza Coupon, ...) at full price and gear at a tenth; buy back the last six sales. Windows: `~/whoami` (I) with the paper doll, a live 3D preview, stats, the bag and perks; `~/procurement` when you talk to a vendor; tooltips compare with what you wear and flag upgrades.

Offline progress (level, gear, bag, Kudos, trophies, quests and cleared zones) is saved per class (native file next to the co-op settings, localStorage on the web) and loaded at start; after the final victory the character stays and the world resets. Online play is session-only: joiners start fresh as before and the server holds everyone's gear for the room.

## Career Ladder (talents)

At levels 2, 4, 6, 8 and 10 each class gets a promotion: one of three talents per row, picked in the `~/career` window (R, or the dock button that glows while a row waits; a toast announces it). Option 1 of every row leans spread/AoE, option 2 deep single target or mitigation, option 3 speed, mobility or a resource twist, so "all 1s", "all 2s" and "all 3s" are three builds per class; mixing is allowed. Talents change how a spell plays, not just its numbers: splash onto nearby fighting enemies or injured allies, instant casts that cost more, casting on the move, cooldown resets on killing blows or on another spell, procs, HP instead of mana (Frontend "Unpaid Overtime"), and spell swaps (Database Lock becomes Deadlock, terraform destroy becomes terraform apply -auto-approve). Tanks stay tanks (Backend option 3 is the DPS-leaning row), healers stay healers. The row-5 promotion can be worn as a title like an achievement title. A Performance Review resets every pick for 15 Kudos per level, doubling, tripling and so on with every earlier review, out of combat. Picks are per character (offline save) and shared online (everyone sees promotions in chat and titles on nameplates).

## Quests

48 Jira tickets in campaign order, each zone's work handed out at its arrival: campus (intern, Facilities Guy, Barista, Smoking-Corner Dev), IT Support Lead (Open Space), Facilities Manager (Meeting Rooms), Office Manager (Sales), Night-Shift Sysadmin (Data Center entrance, both halls), PMO Intern (PMO), Executive Assistant (Penthouse), Lab Safety Officer (Innovation Lab entrance). Breadcrumbs ("Escalation Path", "Next Floor, Please", ...) walk you to the next giver. Mix per zone: kill N, quest-only drops (only while the ticket wants them), an elite bounty (group recommended), the boss as "clear the zone", short chains. Rewards: XP, Kudos, trophies and "choose one of 2-3" gear of the zone's tier for your role. Kills, clears and carried items done before accepting count at once (retro credit), so taking a ticket late never strands you. XP budget per full clear (test `quest_and_kill_xp_track_the_zone_levels`): campus 1 -> 3, Open Space 3 -> 4, Meeting Rooms 4 -> 6, Sales 6 -> 7, Data Center 7 -> 9, PMO 9 -> 10.

The HUD tracks up to six tickets WoW-style under the minimap ("MeatProxies terminated 5/10", a gold "Ready to close" line), toasts progress mid-screen, and paints tracked objectives yellow on the minimap and map; J opens `~/jira` with track toggles.

## The interactive office

Every class can ride a shared **e-scooter** (Y, 1.5 s, +60% speed out of combat; campus and the data center halls only, the tower and the lab are "No scooters in the office (Facilities)"; damage, casting or combat ends the ride) and has **Rush to Standup** (X, +70% speed for 4 s, 30 s cooldown); both sit above the attack and coffee keycaps. Seven scooters parked on campus can be borrowed with right-click / F ("Unlocked with your face. The AI recognised you as 'Scooter Owner'"); they come back a minute later.

Props answer right-click / F with a tooltip on hover: coffee machines on campus and in the Open Space (Fresh Brew: in-combat mana for 2 min, or 10% "Descaling required. Since 2019."), snack vending machines (5-8 Kudos for a random snack buff, an Energy Drink, a motivational quote or nothing), the jammed PMO printer (ten clicks in a row: "PC LOAD LETTER Survivor"), buzzword whiteboards (a generated strategy line in chat), the AI billboard (marketing copy at you), foosball and ping-pong (a round and a dance), the elevator music button, the "Days without an AI incident" signs (reset to 0), the Meeting Rooms fire alarm (pulls the whole floor; the tooltip warns), the smoking-corner bench (/sit, ambient lines), the AI-powered car (honks, "Recalculating...") and the lonely bike rack (keep it company for a minute).

Five hidden NPCs (Last Human QA, Intern Who Read The Docs, Ghost of a Deprecated API, Sysadmin Who Knows Where the Cables Go, Retired COBOL Contractor) pay a few Kudos once. 19 secret achievements list in the trophies window (B, second tab) with hints while locked; each gives a cosmetic title worn under the name ("<Survived Another Reorg>"), chosen there, saved with the offline progress. Emotes /sit, /dance and /facepalm animate (G menu), and co-op players see each other's rides, sits and titles.

## Sprint mode

Replayable Mythic+-style runs against repetitiveness. Unlocked at level 6 or after killing the CEO once. The **Sprint Planning Board** beside the campus walk and the **Sprint Terminal** at the tower entrance open the planner: pick a building (The Tower: 3 of its 5 floors drawn per run, in order; Data Center: both halls; Innovation Lab: both rooms) and a level ("story points", 1-20, the last result recommends the next). Every enemy spawns at level 10 or higher and gains +8% HP and damage per level above 1. Affixes join at +2, +4, +7 and +10, one per slot from the slot's tier, rotating per session ("this week's affixes", shown before committing):

- +2: **AI-Assisted** (every 15 s an enemy regenerates the last spell you cast and hits you with it), **Scope Creep** (a finished pack spawns an Additional Requirement), **Hotfix Friday** (trash below 30% heals 30% once).
- +4: **Reorg** (idle packs swap desks every 90 s), **Burnout** (-1% max HP per minute, reset by killing an elite), **Budget Freeze** (no Kudos, twice the loot).
- +7: **Pair Programming** (the two strongest of each pack share damage), **Mandatory Fun** (a shared team-building circle every 30 s in combat: stack).
- +10: **Technical Debt** (+6% enemy damage per death, up to 10 stacks), **Legacy Code** ("Do Not Touch" elite per floor, invulnerable until interrupted).

The planner's **Weekly** tab is the Weekly Sprint: one building, level +7, the same floors and three affixes for everyone, fixed by the ISO week. Delivered weekly runs go on the week's ladder (fastest first; offline the local one, online the room's board under player names).

The clock starts on arrival; each death adds 5 s. Par is per building (Tower 20 min, Data Center 12, Lab 10) +2% per level. Killing the last boss with at least 80% of the trash burned ("Story points burned: 100%") under par is **Delivered on time**: +1 level, +2 within 80% of par, +3 within 60%. Late, short on scope or abandoned (leaving through a door) is **Carried over to next sprint** (-1). The run plays in its own world: campaign clears, dead trash and unlocks stay untouched. The end shows the **Sprint Review** (time, deaths, burndown per member, retrospective sticky notes generated from the stats, the chest: top-tier gear whose rarity climbs with the level, sometimes a named boss piece, plus Kudos) and the local leaderboard per building (best level, then time; saved with the offline progress; online runs record into the class save too). Online the group leader starts it and the server moves the group.

## The live world: Daily Standup, the Board Meeting, Crunch mode

The world keeps a clock (the device's offline, the server's online; one office day is 24 real minutes, so a real second is a game minute; the time sits on the radar).

- **Daily Standup**: the **Scrum Master Bot** and the **Daily Standup Board** on the campus plaza offer three dailies per UTC day, drawn from 20 templates by the date (everyone gets the same three), and one **Sprint Goal** per ISO week. "Join standup" takes them all. Goals size themselves for the player: kill goals go to the zone whose band fits the level among the zones the player can reach, throughput scales with the level, a Sprint goal asks for a level that fits (and falls back to a kill goal before Sprints unlock). Kinds: defeat enemies (any, or one kind) in the work zone, elites or bosses, loot, office equipment, damage plus healing, finish a Sprint, Talk to a campus NPC, "Do Absolutely Nothing" for a minute in a meeting room, the night shift, the Board Meeting. A daily pays at once (15% of the level's XP bar, 6 Kudos per level); the Sprint Goal (70% of a bar, 30 Kudos per level) is claimed at the board with a choice of best-tier gear (Epic possible); two Sprint Goals also grant a title. They are the long-tail XP of the late levels. The tracker shows them with "Dailies reset in 7h 12m (standup is mandatory)". Progress saves with the date and is dropped when the date moves on; online it is per player for the session and copied into the class save.
- **Board Meeting** (world boss): every 30 minutes spent on the campus (online: the room's clock while the group is there), five minutes of warning in chat, the tracker and a "B" on the radar and map, then **The Board <Quarterly Visionaries>** convenes on the east lawn, off the main paths: level 10, 10,000 HP before group scaling. Quarterly Projections (line sweeps), Hostile Takeover (kick it or eat the AoE plus two Yes-Men), Stock Buyback (stack to split), Shareholder Value (spread), a reshuffle at 66% (three Yes-Man Advisors whose Rubber Stamp heals it: kick), golden handshakes at 33% (+30% damage), an activist investor enrage at 6 minutes. Built for 3-5 players; solo is possible but long. Unique loot: two named Legendaries, trophies, an achievement and title ("Quarterly Visionary"). Not engaged within 10 minutes: "Meeting cancelled. This could have been an email."
- **Crunch mode**: from 18:00 to 06:00 the campus dims, the tower and lab windows light up, the campus NPCs switch to their night chatter and the client-only background staff go home. The night shift (Deadline Wraiths, Sleep-Deprived Contractors) appears at the edges of the lawn at the group's level with better XP and two night-only trophies, comes back two minutes after a kill while it is night, and clocks out at dawn. Clocking out at 18:00 sharp ("Clock out" on the board) is the "Go Home" achievement. The lighting can be frozen at day (`live::DayNight::freeze_day`, a settings hook; the sim's night still happens).
- **World events** (`world_events.rs`, `live.events`): timed public events on the campus, counted down while the group is there and announced a minute ahead in chat and the tracker, with an "E" on the map. Everyone within the radius takes part, no sign-up; winners get a share of their XP bar, Kudos per level and an achievement. **Free Pizza at the Food Trucks** (every 15 min, first after 4): hold the leftover pizza for 90 s against Pizza-Seeking Middle Managers (its death fails it). **Surprise Fire Drill** (every 20 min, first after 10): stop twelve Self-Appointed Fire Wardens at the park assembly point within two minutes. `WOW_WORLD_EVENT=<key>@<secs>` starts one offline.

## The open campus: districts

The campus is 220 x 160 m: the core (tower, data center, lab, annex, shuttle, VibeCon arch, calendar, standup board, all where they were) inside a ring road, and seven districts around it (`floors/districts.rs`), named on the map and in the radar header like WoW subzones. Each has its own level band through district packs with a fixed level (`PackDef::level`), a quest giver and props; the 21 open-world quests (`QuestDef::open_world`) take a fresh hire from 1 to 5 on their own, so the campaign pacing tests leave them out (`tests/common.rs`, `without_districts`).

| District | Levels | Giver | Highlights |
|---|---|---|---|
| Food Truck Plaza (south) | 1-2 | Food Truck Owner | Lunch Queue Optimizers, defend the Last Working Espresso Cart, the Free Pizza event |
| Wellness Park (west) | 2-3 | Campus Park Ranger | lake with a jogging loop, Wellness-Obsessed Managers (KillClean on their jogging lanes), Mindfulness Coaches, three Smart Hydration Stations (`Interact`), the fire assembly point; **The Campus Goose** (level 10 elite) |
| Incubator Street (east) | 3-4 | Incubator Program Manager, Coworking Community Manager | container pop-up booths, Pre-Seed Founders (kick the 47-Slide Pitch Deck), Referral Hustlers, the Unicorn Founder (elite rare), hot desks in the coworking cafe |
| Wellness Gym (south-east) | 3-4 | Wellness Desk Clerk | VPs of Gains, Step Count Enforcers, treadmill desks, the step leaderboard, a smoothie bar |
| Garage & Station (north-east) | 4-5 | Parking Attendant | Autonomous Valets, Return-to-Office Commuters, the departure board, the hidden Intern From the 2019 Cohort |
| New HQ (Opening 2019) (north-west) | 5 | Construction Liaison | Agile Site Foremen, Survey Drones, a crane that never moved, the portacabin with the Employee Who Read the Handbook; **The Visionary Architect** (level 10 elite) |
| Overflow Lot (south-west) | - | - | parked cars, scooters, the night shift |

`Objective::Interact(prop, n)` counts uses of a prop. Travel stays fast: the roads are scooter-friendly, three more borrowable scooters wait at the district stops, and five **shuttle stops** (`PropKind::ShuttleStop`, plaza, park, construction site, station, Incubator Street) form a loop: using one out of combat takes you to the next. Secrets and achievements: the two hidden NPCs, "Mobility as a Platform" (three rides in a row), one per world event and one per level-10 spot.

## Endgame: Promotion, department reputation, burnout

At level 10 XP keeps counting towards ten **Promotion** ranks (Senior Staff Engineer ... Legend (Unpromotable)); each rank adds a capped Paragon bonus (+1% HP and mana, up to +5%) and a title. Kills at level 10 earn standing with the department that owns the zone (Engineering, Security, HR, Marketing, Internal IT, Sales); dailies go to whichever department likes you least, and the Board pays everyone. Tiers run Ignored, Tolerated, Cc'd, Invited, Trusted, Exalted (Gets the Good Monitor) and unlock one vendor item per department. The `~/endgame` window (~ key, Esc menu, touch Menu) shows rank, standings, this week's Legacy and raid lockouts and the Weekly Sprint with its ladder's best time.

**Legacy Mode** ("the code nobody dares to touch") is the heroic tier at level 10. Walking up to a floor door or elevator shows a small "Normal | Legacy" switch above the action bar; the group lead picks, and every tower floor, both data center halls and both lab rooms then load with level 11 enemies (bosses 12), one Sprint affix each (Hotfix Friday in the Open Space, Mandatory Fun in the Meeting Rooms, Scope Creep in Sales, Reorg in the PMO, Budget Freeze in the penthouse, AI-Assisted and Technical Debt in the data center, Pair Programming and Legacy Code in the lab), drops from the Legacy tier (jQuery plugins, SOAP, COBOL punch cards, a Perl monk's robe, a CRT monitor helm, the mainframe's own keyboard) and reputation with the floor's department. Legacy floors keep their own clears, so the elevator opens on the Legacy boss. A floor boss's bonus pieces come once per ISO week per room (offline: per save); after that it pays Kudos only until Monday.

**Burnout** is a battery gauge under the player frame (tier name, percent, a changing joke caption such as "Answering Slack in my sleep" or "Updating LinkedIn", pulsing from Crunch on; the tooltip lists the current effects and the relief options, and a banner announces every tier change). Four tiers: Fine (0-24%), Tired (25-54%: -10% combat mana regen, +5% cast time, +3% spell power), Crunch (55-84%: hyperfocus +10% spell power but -30% regen, +10% cast time and 4% of finished casts misfire with a joke yell) and Burnt Out (85%+: -15% spell power, -55% regen, +25% cast time, 12% misfires). Fights fill it (+1.5% every 10 s in combat), as do meetings, Sprint runs, deaths, Crunch nights and long sessions; coffee, holidays, Friday afternoons, the rubber duck, office props (foosball, ping-pong, the bean bag, the unbookable quiet room, Biscuit the office dog, vending, the Sad Bench) and defeating management drain it ("-5% burnout", "Therapeutic."). The Coffee Break channel never misfires, so even a Burnt Out engineer can recharge. Above 75% you also walk slower, yawn, mutter, reply to all and go bald sooner. The HR Onboarding Bot gives the one-line tip. Effects are sim-side and deterministic (own rng stream for misfires), so co-op peers agree.

## Raid: All-Hands Offsite

The campus shuttle stop (a branded coach that finally showed up) takes a level-10 group to the **All-Hands Offsite** at a lakeside conference resort: four rooms, one boss each, trash packs of event planners, lanyard enforcers, breakout facilitators (kick the Sticky Note Storm), AV technicians and open-bar account executives (they heal: kick or burst). Three to five players get the **Full Programme**; fewer get **Async Attendance** (the recorded version: enemies at 40% HP and 55% damage, less loot), so one level-10 player in Legacy gear can finish it. The Offsite Concierge in the lobby explains both.

1. **Icebreaker Facilitator** <Certified Fun Practitioner> (Lobby): "Pair Up!" tethers the group in pairs (an odd one out to the facilitator) for 12 s; a pair more than 8 m apart takes damage every second, the cord between them turns red. Two Truths and a Lie (a circle on everyone: spread), Trust Fall (a ring around him: melee steps out).
2. **The Synergy Twins** (Breakout Rooms): Alignment (melee, Circle Back cone) and Leverage (stays at her podium: Quick Pings, circles on everyone) share one health bar. One of them is always "Aligned (Do Not Disturb)" in a gold bubble taking 80% less damage; every 18 s they swap places and the bubble. Hit the open one.
3. **Keynote Speaker** <AI Hologram, Thought Leader> (Terrace): Next Slide, Please sweeps a wall of nine slide lanes across the terrace, one after another; one slide was left blank, stand on it. Live Product Demo is a must-interrupt that ships to the whole audience. Below 50% the slides come every 13 s.
4. **The Founder** <Chief Vision Officer, Majority Shareholder> (Grand Ballroom): three phases. Pivot cones throughout; at 66% "We Are a Family" seats the Founder's Nephew and the Co-Founder's Spouse at the family table, who begin a 15 s Mandatory Family Dinner that kills everyone if either finishes (kill them; it repeats every 40 s); at 33% Founder Mode (+25% damage) and Golden Parachutes.

Every encounter locks for the ISO week per difficulty (per room online, per save offline): a boss you beat stays away until Monday. Bosses drop the raid tier (four role sets: Offsite Logistics, On-Site Support, Keynote Track, Team Spirit; fleece vests, ribbon badges, bucket hats), a Legendary now and then, a trophy, reputation with every department and burnout relief (they are management). Achievements and titles: Volunteered Participant, Fully Aligned, Asked a Hard Question, Part of the Family, Offsite Survivor (Normal Founder), Attended Asynchronously (Async Founder). Cutscenes: an intro on the first walk into the lobby, one after each boss, closing remarks after the Founder's.

## Raid: VibeCon

The second raid, same rules (level 10 door, Full Programme for three to five, Async Attendance below at 40% HP and 50% damage, weekly per-boss lockouts, the same loot and reputation model). A pop-up backdrop on the campus east lawn, with a truss arch over a lanyard queue and a registration desk, leads into a tech conference in five rooms: the expo hall (booths, swag tables, neon lanyards), the workshop rooms, the hackathon corner, the afterparty and the main stage. Trash: Prompt Engineers, '10x' Founders (kick the Elevator Pitch), booth staff of the Agent Framework of the Week (badge-scan stun), Swag Goblins, LinkedIn Influencers filming you (On Camera: +12% damage taken, dispellable), Hackathon Zombies and Web3 survivors who pivoted to AI (Rug Pull cone). The Registration Volunteer explains the difficulty.

1. **The Vibe Coder** <Talk: I Haven't Read Code Since March> (Expo Hall): every 15 s he ships an Unreviewed PR, an add that casts a 10 s Merge to Prod. Kill it first: when the merge lands it explodes in production for the whole group and gives him a stack of Shipped to Prod (+15% damage). Accept All Suggestions cone, Vibe Check circles on everyone.
2. **The Prompt Whisperer** (Workshop Rooms): "Ignore Previous Instructions" is a 2 s cast followed by a 2.6 s circle under every player; whoever is still in it has movement mirrored for 4 s (a magenta halo and a notice; healers can dispel it). Every 24 s, every 17 s below 50%. Jailbreak lines and Few-Shot Barrage.
3. **The Demo God** <Head of DevRel, Never Rehearses> (Hackathon Corner): Live Demo (Pre-Recorded) is a 4 s must-interrupt; if it plays, everyone is hit and two Live Demo Bugs crawl out of the laptop. Works on My Machine ring around him, Conference Wi-Fi Drops circles.
4. **The Hype Panel** (Afterparty fireside chat): the moderator and two seated panellists (a Seed-Stage VC, a Chief AI Officer) share one health bar; only the one holding the mic can be hurt, the others sit in a bubble. Every 15 s the mic passes along the panel. The moderator answers audience questions himself (kick More of a Comment, Really).
5. **The Keynote AGI-Announcer** <Founder, CEO, Prophet, Black Turtleneck> (Main Stage): This Changes Everything (a ring around the stage) and unlabelled benchmark charts; at 66% "one more thing": AGI (Beta), an add with a must-interrupt Hallucinated Roadmap, again every 40 s; at 33% "one more thing": Enterprise Pricing (+25% damage) and Auto-Renewing Subscription debuffs.

Loot: four conference swag sets (Booth Duty, Volunteer Crew, Speaker Kit, All-Nighter) in the four lanyard colours with three new looks (conference VR headset, branded hoodie, neon lanyard with a QR badge), five Legendaries, trophies, the Conference Wi-Fi Router companion (25% from the keynote; it never works). The keynote's title is **Speaker**; other titles: Actually Read the Diff, Prompt-Proof, Demo Interrupter, Panel Survivor, All-Access (Normal keynote), Attended Remotely (Async keynote). The panel, the keynote and the '10x' Founders are management for burnout. Cutscenes: an intro at registration, one per boss, closing remarks after the keynote.

## Easter egg: Body Leasing

The Account Manager <Your Utilisation Is Our Margin> (campus, between the plaza and a grey annex) hands out "Allocated to a Strategic Client"; accepting it opens the annex door to **Very Large Insurance & Banking Group** (`client/1`, `floors/client.rs`): a fictional grey-beige office with a lobby, 32 empty desks, a dead plant, a printer from 2009 and a door that says "Badge pending". No enemies, no loot. Four tickets, about three minutes: wait for access rights (a 75 s `Wait` objective, a text bar in the tracker), attend a status meeting where you are muted and never called on, and fill in a timesheet ("8h, analysis", a 20 s wait). The exit door opens when the timesheet is closed, which grants the achievement **Billable** ("Nothing happened. You billed it anyway.") and the title <External Resource>. `Objective::Wait` counts one per second while a player is alive in its zone; achievements keyed `quest:<id>` unlock on a quest's turn-in. Online, the zone's geometry is in `zone_geometry.ron`, so the group travels through the doors like any other zone (not tried against a live server).

## Company Calendar (side content)

One board for every side activity: the Company Calendar beside the Welcome arch on the campus plaza (across the arch from the Daily Standup board, on the walk from the parking lot). It has a calendar icon on the minimap and the map with a "!" while anything is unplayed, a one-line tracker hint at level 2, the `.` key, a Calendar entry in the touch Menu drawer and a button in `~/endgame`. The window (`calendar.rs`) is a week-view list in two groups, Minigames and Quest chains; each row shows the name and a joke, a length estimate, its status (new, in progress, best medal, done), reward icons and one button. Starting a minigame takes the group there; starting a chain accepts its first quest and puts it in the tracker. Everything is listed every day; `calendar.rotation` (off) would list one of each per UTC day. Chain givers carry no "!" of their own: the board has it.

Quest chains (three to five minutes each, campus and the Open Space):
- **Expense Report** (Expense Controller, by the cafe): a 3.20 coffee, a napkin receipt, four approvals (the Intern, Facilities, Procurement, the Account Manager) and a 45-second justification review. Pays 3 Kudos, a laminated receipt and the title "Reimbursed in Spirit". Costs 8% burnout.
- **Exit Interview** (Departing Senior Engineer, smoking corner): HR schedules it, box 7 gets handed over, then the honest answers. Burnout -12%, HR standing, the "World's Okayest Engineer" mug, title "Said It Out Loud".
- **Mandatory Training** (Security Awareness Officer, tower path): forty slides, then report IT's test phishing mail in the Open Space; Security clicked it. Security standing, a certificate, title "Did Not Click".
- **Return to Office** (Workplace Experience Lead, bike rack): a chair voucher, forty seconds on a Teams call in the Open Space next to your team, one commute story. HR standing, a hot-desk booking.
- **Town Hall** (Internal Comms Lead, plaza): a rephrased question, the stream, and the one pre-approved answer. Marketing standing, a laminated clapping emoji, title "Pre-Approved Question".

Jira tickets ("Blocker", "Won't Fix", "Duplicate") are junk that any trash can drop; using the Scrum board or the Company Calendar closes all of them for twice their vendor price in Kudos.

## Minigame: Mandatory Fun: Teambuilding

A stealth minigame at the company garden party (`teambuilding/1`, `floors/teambuilding.rs`), about five to eight minutes, solo or co-op. It is listed on the Company Calendar (above); the group lead's "Start" there sends `Command::StartMinigame`. Each member sneaks on their own through the hotel garden: steal the last good canape, grab a drink at the open bar, pocket the swag, escape the group photo behind its backdrop and answer a prod incident in a quiet corner behind the hedges, then leave by the taxi rank. Ten canapes on trays are optional points.

People walk fixed loops with a coloured vision cone at their feet (cut by hedges, tents and banners): Engineering Managers (orange) send you back to the start ("Let's circle back to the beginning"), AI Enthusiasts (blue) corner you for five seconds with a bad AI take you can nod away, the HR Fun Committee (pink) walks you to the icebreaker circle, and the CEO (red) leaves his VIP tent every couple of minutes, sweeps the lawn and notices you at once. Jump toggles sneaking (half speed, silent); running is heard nearby. Plants, the lifted buffet cloth, the toilet queue screen and roll-up banners are hiding spots, marked by a ring on the grass.

Score is time plus canapes minus catches; Bronze "Attended", Silver "Networked", Gold "Was Never Seen" (no catch). Rewards: HR reputation, burnout relief, a name tag, the Party Leftovers Container companion (Silver), a party hat and the title "Ghosted the Teambuilding" (Gold). The header replaces the quest tracker while you play; Esc asks before you leave. The group score adds every finished run, and the shuttle takes everyone back when the last run is over. Numbers in FORMULAS.md, Minigame: Teambuilding.

## Company Calendar minigames

Five shorter games on the Teambuilding frame, each three to five minutes, solo or co-op (every member plays their own run, the group score adds up), each started from the Company Calendar. A choice card low in the centre carries every decision (buttons, or the action keys 1-4; on touch it sits between the header and the buttons), and the header in the tracker's slot shows the run's lines and bars. Medals pay department standing and burnout relief each time; items, titles, Kudos and trophies land once per character. Numbers in FORMULAS.md, Minigames: the Company Calendar's five.

- **Performance Review** (meeting room 4B, HR): a dialogue "boss fight" with your manager, nine questions long. Answers are honest, buzzwords, a clarifying question about the question, agreement, the raise, or silence (after 20 s the manager fills it with a story about their boat). Each burns minutes of the manager's calendar and costs or restores their patience; repeating yourself costs more, and at zero patience they take the rest offline (logged as a quick sync: half the calendar counts). The "Meets Expectations" gauge jumps with every answer and drifts back to 3 of 5 regardless. The medal is calendar burned (Gold also needs the manager to sit through to the verdict). Every ending: 3 out of 5, no budget for raises this year. Title "Meets Expectations"; Silver adds a mug.
- **Reorg** (open plan, third floor, Engineering): five reorgs. Each one moves the desks and the partitions and deals the six colleagues new names and roles, shown on their name plates. Walk up to them for clues: one knows where your manager sits ("next to the printer"), one knows the role, one repeats the old org chart (wrong now), the rest know nothing ("I report to a Slack channel now"). Ask "Are you my manager?"; faster is better, wrong guesses cost. Silver gives the title "Reports to a Dotted Line".
- **Tool Migration** (IT back office, Internal IT): carry twenty tickets from TICKETRON 2009 to FlowSphere Cloud. The new tool asks two to four mandatory fields per ticket (component, business impact, urgency as felt by the reporter, where the ticket came from, the category that is the closest lie); the ticket's text has the answers. A wrong pick is a validation error and a spinner. After the twentieth ticket the CTO announces that the company is migrating back. Silver gives the title "Migrated Twice" and Kudos, Gold a lanyard.
- **Hackathon** (the innovation space, Marketing): 24 hours in four minutes. The demo builds itself as fast as your focus allows; focus drains, faster in the small hours; pizza (cold after 01:00) and energy drinks on the side tables refill it, and nobody eats when already focused. At hour 24 you present, and the demo crashes on stage whatever the bar said. Everyone takes home the plastic trophy and the title "Hackathon Finalist"; Gold adds a hoodie printed before the theme was announced.
- **Open-Space Focus** (open plan, second floor, Engineering): hold sixty seconds of deep work at your desk in the middle pod. "Got a minute?" colleagues, the speakerphone account manager, a birthday cake procession and a fire drill test walk up from the edges with their cones pointing at you; the card counts each arrival down. Headphones for calls and alarms, the "In a meeting" status for "got a minute?", duck for the cake; each defence lasts 2.5 s and rests, so it is about timing. Gold (never interrupted) gives decorative noise-cancelling headphones and the title "Do Not Disturb".
- **Stay Green** (your apartment, HR): keep your Teams status green through a three-minute working day at home. Away from the laptop the status drains; yellow brings the manager's "quick question?". The cat sits on the keyboard, the washer finishes, a courier rings, the neighbour drills during the all-hands and the important call says you are on mute. The mouse jiggler holds your status while you do chores, until IT scans for it. Silver gives the title "Camera Off, Status Green"; Gold (never yellow) adds the Webcam Hoodie (Sweatpants Implied).

## Companions and Code Review Duels

Pets (`pets` in `game.ron`, `pets.rs`): eight cosmetic companions, one out at a time, learned by
using a carrier consumable (Procurement / IT Asset Store stock, or a per-looter drop from an
elite or boss) or granted (Golden Unicorn: deliver a Sprint at +10). They follow their owner,
persist in the save (`pets`, `pet` keys) and show for co-op partners (`PlayerSnap::pet`,
`PetChanged`). No combat power: the Rubber Duck gives real tips read off the fight
(`duck_tip`), the AI Copilot confidently wrong ones, and the Roomba sweeps the owner's grey
junk off corpses within 12 m (only the owner's share). `~/pets` opens from `~/whoami` (I).

Duels (`duel.rs`): online only. Right-click a group member, "Request code review"; the target
accepts within 30 s. A 3 s countdown inside a 20 m circle at the midpoint, then only the pair
can hurt each other (50% damage; stuns and silences diminish 100/50/25/immune, reset after
18 s). No heals or buffs cross into an accepted duel. The duel ends at 1 HP ("Bob approved
Alice's PR after 47 comments"), by leaving the circle or the room (forfeit), or is called off
when an enemy pulls either player, someone dies or a sprint starts. Requests need both players
alive, within 20 m, not pulled, not in a sprint; one request per 5 s per player. Wins in a row
are a saved cosmetic streak.

## Engine needs (combat_core)

Effects: damage over time, heal and heal over time, stun, silence (school lock exists), cast-time
modifier, damage taken modifier, haste, knockback (client-side impulse event), dispel one harmful
effect, execute bonus, summon unit, self-buff stacks, soft enrage. Encounters: boss phases by HP %,
timed abilities, add summons, social aggro radius for packs, per-unit XP and level-based XP scaling,
player level-up with stat and spell unlock tables, floor-scoped spawns and respawn rules.

## Presentation needs (game_client)

Per-floor procedural office layouts from the Kenney Furniture Kit, elevator transition, enemy
model mapping from the Quaternius pack (Suit for executives, Casual for juniors and proxies, Doctor
for the analyst's lab coat, Worker for consultants in vests, etc.) with tints and scale, nameplates
above enemies, level-up flash and message, spellbook or unlock toast, 10-slot keycap bar, floor name
banner on entry, boss frame for phases.

## Implemented

combat_core side (client presentation is tracked separately):

- [x] Five floors in `content/game.ron` with packs, boss and friendly NPC anchors; `Sim::load_floor`, `FloorLoaded` / `FloorCleared`, floor spawns that stay dead, respawn point, bosses reset on player death.
- [x] All trash from the table, plus mini-bosses HR Business Partner (2), Head of Marketing (3), CISO (4), Growth Hacker and Security Analyst trash, IT Support NPCs with talk and ambient banter.
- [x] Five bosses with their mechanics: Vibe Coder (PR adds, interruptible diff), Analyst (school lock, Unread Spec), Customer (Scope Creep soft enrage, big interruptible cast), VP of Synergy (adds, 50% Reorg teleport and threat reset), CEO (40% Pivot to AGI: chatbots, faster Blast).
- [x] Effects: DoT, HoT, heal, stun, silence, school lock, cast-time and damage-taken/-done modifiers with stacks, haste, knockback event, dispel, execute, summon, timed explosion, copy-last-spell, do-nothing cast, threat swap, on-death explosion, phases by HP %.
- [x] Player levels 1-10, XP table, level-difference XP with gray mobs, stat growth, spell unlocks, 10-slot bar (`known_spells`). Deviation: Reject PR unlocks at level 2 and Guardrails at 3.
- [x] Headless playthrough test: every floor cleared at its max level on three seeds; golden replay of a floor-1 clear from level 1.
- [x] Trophies: all 46 loot items are permanent bonus items (`items` table: HP, mana, spell power, crit, haste, mana regen; 3-copy stack cap). Looting fills `UnitState.inventory` and applies the bonus at once; the client shows them in the loot window, the combat log and the B window (`~/trophies`).
- [x] Economy: 283 gear pieces in five visible slots (43 visual styles), 14 sets, 21 Legendaries, 22 on-use gadgets, 79 junk items, 125 trophies, 24 consumables, 2 vendors, Kudos, perks and on-use gadgets, per-class offline saves (`economy.rs`, `save.rs`, client `economy/`).
- [ ] Channels, mini-boss optionality, per-room elevator logic (client).

