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

## Gear, Story Points and vendors

Loot means something: every kill pays Story Points (SP) and may drop gear of the zone's tier (ilvl 2/4/6/8/10), weighted toward your role; elites (HR Business Partner, Marketing Lead, CISO) drop more often, bosses always drop a piece and sometimes a named Legendary. Five slots are worn visibly on the character: Headgear (cap, beanie, headset, propeller hat, VR visor, shades, crown), Hoodie (tints the shirt, logo and glowing trims by tier), Keyboard (slung on the back, RGB at the top), Mug (right hand: mug, thermos, stein, chalice) and Lanyard (badge on the chest). Rarities read like a code review: Works On My Machine (grey), Passed CI (green), Merged (blue), Shipped to Prod (purple, glows), Approved in Code Review (orange, glows harder).

Stats come from a budget (rarity x item level, keyboards +25%) split by the piece's role package; the sheet names them in dev terms: Uptime (HP), Context Window (mana), Velocity (spell power), Throughput (% damage and healing), Hot Takes (crit), Caffeine (haste), Focus Time (mana per 5 s), Plausible Deniability (% less damage taken). Perks change how you play: echoes ("Pixel Push hits twice above 80% HP"), riders ("Who Touched Prod? taunts everything within 8 m", "Hotfix also cleanses"), cooldown cuts, lifesteal, on-hurt shields, and on-use gadgets on U / O / P. Four 4-piece sets (Burnout Collection, On-Call Rotation, Full-Stack Overflow, Pixel Perfect) give stats at 2 and alter a signature spell at 4.

Vendors: Procurement (campus plaza, "3-6 weeks") sells consumables (Ibuprofen, Energy Drink, Cold Brew, Leftover Catering Sandwich), All-Hands hoodie colours and tier 1-2 gear with a rotating Rare shelf; the IT Asset Store in Data Center Hall A sells tier 3-4 gear and a rotating Rare/Epic shelf. Sell junk (Unread Jira Notification, Expired Free Pizza Coupon, ...) at full price and gear at a quarter; buy back the last six sales. Windows: `~/whoami` (I) with the paper doll, a live 3D preview, stats, the bag and perks; `~/procurement` when you talk to a vendor; tooltips compare with what you wear and flag upgrades.

Offline progress (level, gear, bag, SP, trophies, quests and cleared zones) is saved per class (native file next to the co-op settings, localStorage on the web) and loaded at start; after the final victory the character stays and the world resets. Online play is session-only: joiners start fresh as before and the server holds everyone's gear for the room.

## Career Ladder (talents)

At levels 2, 4, 6, 8 and 10 each class gets a promotion: one of three talents per row, picked in the `~/career` window (R, or the dock button that glows while a row waits; a toast announces it). Option 1 of every row leans spread/AoE, option 2 deep single target or mitigation, option 3 speed, mobility or a resource twist, so "all 1s", "all 2s" and "all 3s" are three builds per class; mixing is allowed. Talents change how a spell plays, not just its numbers: splash onto nearby fighting enemies or injured allies, instant casts that cost more, casting on the move, cooldown resets on killing blows or on another spell, procs, HP instead of mana (Frontend "Unpaid Overtime"), and spell swaps (Database Lock becomes Deadlock, terraform destroy becomes terraform apply -auto-approve). Tanks stay tanks (Backend option 3 is the DPS-leaning row), healers stay healers. The row-5 promotion can be worn as a title like an achievement title. A Performance Review resets every pick for 15 SP per level, out of combat. Picks are per character (offline save) and shared online (everyone sees promotions in chat and titles on nameplates).

## Quests

48 Jira tickets in campaign order, each zone's work handed out at its arrival: campus (intern, Facilities Guy, Barista, Smoking-Corner Dev), IT Support Lead (Open Space), Facilities Manager (Meeting Rooms), Office Manager (Sales), Night-Shift Sysadmin (Data Center entrance, both halls), PMO Intern (PMO), Executive Assistant (Penthouse), Lab Safety Officer (Innovation Lab entrance). Breadcrumbs ("Escalation Path", "Next Floor, Please", ...) walk you to the next giver. Mix per zone: kill N, quest-only drops (only while the ticket wants them), an elite bounty (group recommended), the boss as "clear the zone", short chains. Rewards: XP, Story Points, trophies and "choose one of 2-3" gear of the zone's tier for your role. Kills, clears and carried items done before accepting count at once (retro credit), so taking a ticket late never strands you. XP budget per full clear (test `quest_and_kill_xp_track_the_zone_levels`): campus 1 -> 3, Open Space 3 -> 4, Meeting Rooms 4 -> 6, Sales 6 -> 7, Data Center 7 -> 9, PMO 9 -> 10.

The HUD tracks up to six tickets WoW-style under the minimap ("MeatProxies terminated 5/10", a gold "Ready to close" line), toasts progress mid-screen, and paints tracked objectives yellow on the minimap and map; J opens `~/jira` with track toggles.

## The interactive office

Every class can ride a shared **e-scooter** (Y, 1.5 s, +60% speed out of combat; campus and the data center halls only, the tower and the lab are "No scooters in the office (Facilities)"; damage, casting or combat ends the ride) and has **Rush to Standup** (X, +70% speed for 4 s, 30 s cooldown); both sit above the attack and coffee keycaps. Four scooters parked on campus can be borrowed with right-click / F ("Unlocked with your face. The AI recognised you as 'Scooter Owner'"); they come back a minute later.

Props answer right-click / F with a tooltip on hover: coffee machines on campus and in the Open Space (Fresh Brew: in-combat mana for 2 min, or 10% "Descaling required. Since 2019."), snack vending machines (5-8 SP for a random snack buff, an Energy Drink, a motivational quote or nothing), the jammed PMO printer (ten clicks in a row: "PC LOAD LETTER Survivor"), buzzword whiteboards (a generated strategy line in chat), the AI billboard (marketing copy at you), foosball and ping-pong (a round and a dance), the elevator music button, the "Days without an AI incident" signs (reset to 0), the Meeting Rooms fire alarm (pulls the whole floor; the tooltip warns), the smoking-corner bench (/sit, ambient lines), the AI-powered car (honks, "Recalculating...") and the lonely bike rack (keep it company for a minute).

Five hidden NPCs (Last Human QA, Intern Who Read The Docs, Ghost of a Deprecated API, Sysadmin Who Knows Where the Cables Go, Retired COBOL Contractor) pay a few SP once. 19 secret achievements list in the trophies window (B, second tab) with hints while locked; each gives a cosmetic title worn under the name ("<Survived Another Reorg>"), chosen there, saved with the offline progress. Emotes /sit, /dance and /facepalm animate (G menu), and co-op players see each other's rides, sits and titles.

## Sprint mode

Replayable Mythic+-style runs against repetitiveness. Unlocked at level 6 or after killing the CEO once. The **Sprint Planning Board** beside the campus walk and the **Sprint Terminal** at the tower entrance open the planner: pick a building (The Tower: 3 of its 5 floors drawn per run, in order; Data Center: both halls; Innovation Lab: both rooms) and a level ("story points", 1-20, the last result recommends the next). Every enemy spawns at level 10 or higher and gains +8% HP and damage per level above 1. Affixes join at +2, +4, +7 and +10, one per slot from the slot's tier, rotating per session ("this week's affixes", shown before committing):

- +2: **AI-Assisted** (every 15 s an enemy regenerates the last spell you cast and hits you with it), **Scope Creep** (a finished pack spawns an Additional Requirement), **Hotfix Friday** (trash below 30% heals 30% once).
- +4: **Reorg** (idle packs swap desks every 90 s), **Burnout** (-1% max HP per minute, reset by killing an elite), **Budget Freeze** (no Story Points, twice the loot).
- +7: **Pair Programming** (the two strongest of each pack share damage), **Mandatory Fun** (a shared team-building circle every 30 s in combat: stack).
- +10: **Technical Debt** (+6% enemy damage per death, up to 10 stacks), **Legacy Code** ("Do Not Touch" elite per floor, invulnerable until interrupted).

The clock starts on arrival; each death adds 5 s. Par is per building (Tower 20 min, Data Center 12, Lab 10) +2% per level. Killing the last boss with at least 80% of the trash burned ("Story points burned: 100%") under par is **Delivered on time**: +1 level, +2 within 80% of par, +3 within 60%. Late, short on scope or abandoned (leaving through a door) is **Carried over to next sprint** (-1). The run plays in its own world: campaign clears, dead trash and unlocks stay untouched. The end shows the **Sprint Review** (time, deaths, burndown per member, retrospective sticky notes generated from the stats, the chest: top-tier gear whose rarity climbs with the level, sometimes a named boss piece, plus Story Points) and the local leaderboard per building (best level, then time; saved with the offline progress; online runs record into the class save too). Online the group leader starts it and the server moves the group.

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
- [x] Economy: 89 gear pieces in five visible slots, 4 sets, 8 Legendaries, 14 junk items, 5 consumables, 2 vendors, Story Points, perks and on-use gadgets, per-class offline saves (`economy.rs`, `save.rs`, client `economy/`).
- [ ] Channels, mini-boss optionality, per-room elevator logic (client).

