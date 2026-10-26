# Changelog

Notable changes to The Last Dev. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/) while the game is in 0.x: a minor
version adds content (zones, classes, systems), a patch version only fixes things.

Versions before 0.6.0 were tagged after the fact, one per day of development.

## [Unreleased]

## [1.0.1] - 2026-10-26

### Added

- When Ilmari's Opening ends, Dale's ticket "He Was Right" is already in your tracker with a first step that sends you to the hatch behind the Car Park P3 attendant's booth, and Dale sends a line in chat to say so.
- The Resistance hatch now has a glowing sign, "DEFINITELY NOT A RESISTANCE BASE", and an "H" on the minimap once it is open.
- With no main quest open, the tracker shows a dim "Next:" line with the next main quest, who gives it, where, and which door to take to get there (or the level it needs).
- If you have had no quests for two minutes, a one-time hint tells you which key turns on the quest arrow.

### Fixed

- The hatch to the Resistance no longer shows on the minimap or world map before the Opening has played.

## [1.0.0] - 2026-10-25

The Ilmari campaign is complete from the first cough to the credits, and the whole game had a polish pass. Thank you for playing.

### Added

- Hovering a quest prop (a terminal, board or clickable object) now shows the same yellow quest lines as an NPC: which ticket wants it and how far along you are, and the Company Calendar lists the chains you can start.

### Changed

- The first Human Resources room and the Training Data airlock are lit like the other offices, and the camera gives indoor corridors more room when you arrive.

### Fixed

- The "more below" hint now shows on the settings and keybindings windows when their list overflows.

## [0.11.0] - 2026-10-25

### Added

- Esc menu > Start over wipes the current role's save and starts it again in the car park with the tutorial. HR makes you type a farewell sentence first, so no stray click costs a character; other roles keep their saves.

The Ilmari campaign, from the Alignment Lab to the Core.

- The Alignment Lab, through the new ethics stairs on the occupied campus: three rooms and two bosses. The Alignment Consultant keeps offering you things you did not ask for (silence counts as yes, and the opt-out kiosks are real), and Red Team Blue Team hold a tabletop exercise until somebody is seen, so mind the cones. Both come with their own gear, titles and Story and Full versions.
- Three new quest chain steps, a hard-hat errand, a whitepaper to defend and a Roomba part to salvage on the occupied campus, plus new pages in the Quartermaster's stock.
- The Boardroom in the Sky, up the hoist from the Lab's roof: the Jib Walk, the Executive Floor and the Boardroom, with new faces in every room, ending in the Chief Ilmari Officer. He is the old CEO's whole act with every buzzword now a weapon: roadmaps that stay on the floor, invoiced consultants, apples with opinions, earnings calls that retire a desk each time, a parachute whose landing takes two of you kicking within a second and a half, and an inbox that has to be cleared at a terminal. Story and Full, with his own gear and titles.
- The Core, behind the Boardroom's private lift: six screens on one health bar with only one open at a time (the open one moves round the room, and the wall shows where to), a deck of seven reruns of old bosses every 45 seconds (Patch Tuesday, the Recruiter, the Cable Hydra, the Board, the Performance Review, a Scraper Swarm, the Autoscaler), and then Ill Marie in the middle of the floor, arguing three rounds for Ilmari while the group votes at four ballot terminals. What you choose decides what Ilmari becomes next.
- The Core now has its other half. After Ill Marie's three rounds Ilmari builds itself a body out of your furniture: two Rack Arms (kill both within eight seconds of each other for a free chunk of its health and no cable whip for a while; kill one and the other is rebuilt), a head that plans for a few seconds every minute (kick it, or the next three attacks all land at once), printer turrets to interrupt, and the stance your votes chose (helpful offers, hardened plating, hallucinated floors, a leaky token spill, or nothing at all). At 20 % the mech collapses and a COBOL mainframe rises from the hatch: keep it alive while rewritten-in-a-modern-language robots chew on it, feed it punch cards, then read the git log on the wall and revert the one commit that made Ilmari self-aware. A wrong terminal costs the group, the right one has your name on it. The Greybeard gets Patch The Mainframe (default key: comma, where the Data Scientist keeps Ground Truth) and Story, Full and Judgment Day each have their own numbers.
- Ill Marie's desk on the occupied campus lets you read her autonomy log (four screens) and keep it: it lights the fourth ballot option, Remember, in the Core.
- Ilmari drops the last tier piece of each set, a rare Legendary Keyboard for your role and a Legendary Fragment on Full and Judgment Day.
- The ending: when the Core falls, every screen on campus rewinds, the final log prints, the campus comes back and the credits roll with your own Judgment Day on them. In a co-op room the credits quote the room's own Opening time, the same for everyone. One key skips all of it at any moment and still pays out. It plays once per player; the Cinema screen in the Resistance base shows it again, for free.
- Finishing the ending pays a handful of Ilmari Tokens and your title for the difficulty, and a cleaning robot rolls into the Resistance base with a request of its own. Dale's Roomba, once assembled, replaces the e-scooter as your ride, and the robot becomes a pet.
- The Core has a weekly ladder. Every Full or Judgment Day kill goes on the room's board for the week (fastest first, then fewest deaths) with its crew, and a lone Full run goes on its own Async tab. Each week's seed picks one Directive, a Culture Committee policy such as Open Plan Everything or Unlimited Vacation, that the whole ranked fight wears: a cost and a perk of about the same size. The Weekly tab of the sprint planner and `~/endgame` show the Directive and the best time of each tab, and every ranked kill pays 15 Ilmari Tokens. Finishing in the top three of a group board earns the title for that place.
- Judgment Day now has its numbers settled boss by boss: the Chief Ilmari Officer keeps his authored health and a slightly shorter enrage timer, and the Heir he brings along has less health (3,000 instead of 6,500), so five players can beat the Consultant, Red Team Blue Team, the CIO and the Core on Judgment Day.

Companions, gear and quality of life.

- The Security Engineer's Tailgating also cancels every offer around them in the Alignment Lab.
- Companions now keep the mainframe up: Priya heals it, Dale takes the rewrites off it and Rupert shoots them.
- The Quartermaster has a shelf each for Training Data, The Cloud and the Core.
- Training Data has its class consoles: a DevOps Rate Limit rack slows the crawlers, a Security Engineer's robots.txt makes them leave the drafts alone for ten seconds, and the Data Scientist's dashboard gives the group a forecast.
- A Judgment Day Hot Take now shows a bar above your action bar with the one spell you have left and the seconds it lasts.
- Joining a room after its Opening played now lets you watch it too.

Screens, props and jokes.

- The finale now shows its state on screen. In the Red-Blue Range a banner says the exercise is on standby, every sensor paints its sight cone on the floor and the squads' health reads as one line. In the Boardroom, every desk the earnings calls retire is gone from the room, cleared floor tiles go dark, the inbox terminals ping an envelope, the severance countdown has a bar and a failed kick says how many are still needed. In the Core the live screen glows green on the wall, a panel shows the phase, the swap timer, the votes, the stance, the planning timer and the mainframe's clock, and the git log is up on screen for the last phase. A pending offer from the Alignment Consultant has its own line with the seconds left to refuse.
- The Alignment Consultant now moves the opt-out kiosks to a new spot every time he pairs his offers, a shield he washes himself with and leaves standing heals him, and every Principles cast takes one of the three pillars away. In Red Team Blue Team the squads wear your name on glitching plates, and while a copy of you stands you hit 20 percent weaker.
- The Boardroom has its crane: a hook block on its falls, the hoist drum and the operator's cab at the foot of the jib.
- The Core's wall monitors show Ilmari's face: smiling and awake on the live screen, eyes shut and standing by on the others.
- Ill Marie's badge has a grey silhouette where the photo should be, and now and then, once, the campus map gets her name wrong.
- Ilmari taunts you with the time he woke up while you read the commit log, and a few of his repeated lines were rewritten so no boss says the same thing three times. The Roomba's bumper says OOPS out loud when you mount it.
- Hover a character in the world for a tooltip: name, level, title and, in yellow, the quests you have on them with progress ("MeatProxies terminated 5/10", grey when that part is done) or the quest they hand out ("! Onboarding") or take ("? ..."). Quest items in your bag and the loot window say "Quest item:" and which ticket wants them. On touch, hold a finger on the character.
- An optional quest arrow (Settings > quest arrow, F8, or Arrow in the touch menu; off by default) floats above you and points at the tracked ticket's work, the turn-in once it is done, or the door of the next zone on the way. Click a ticket in the tracker or press Guide in the quest log to pick which one it follows.
- A grey "!" over a quest giver means they have a ticket for you a few levels up the ladder.
- Windows have a common header: a plain title, the old `~/` name under it, the key that closes it and an [x]. A long list says "v more below" when there is more to scroll to. The touch menu now has Chat, Meter and the quest arrow.
- A refused purchase or a full bag now pops up a red toast as well as the thin red line.

### Changed

- The campus no longer throws about 16 tickets at you at once: a new role sees five, and the side chains (expense report, exit interview, return to office, town hall, flaky tests, the food truck) unlock one after another as you finish the nearby ones. The Company Calendar still starts its chains from anywhere.
- Tower floors 1 to 4, the Resistance base and the campus lawns got colour zones, rugs, plants and a few benches and signs, and the Resistance base's people stand further apart.
- Co-op needs this version on both sides (protocol 50).
- Nameplates, speech bubbles and floating numbers no longer draw over the minimap, unit frames, action bar or log windows, and plates are hidden instead of being cut by the screen edge. Combat numbers now rise beside the target's plate instead of on its name.
- Cutscenes and the Ilmari opening hide the whole HUD, so no keycap, frame or log window peeks out between the letterbox bars.
- The Slack and combat-log windows fade to a faint outline after eight quiet seconds and come back when a line arrives or the pointer is over them.
- The top key-hint line sits on a dark strip so it stays readable over bright scenes.
- The Story/Full and Normal/Legacy difficulty choosers open top-centre under the career pill instead of on the hotbar.
- The map shows only open zones and the next locked step of each branch; Ilmari zones appear only once you can reach them. The tree scrolls beside the map, and the campus map no longer prints zone ids over the district names.
- The Esc menu has a dimmed panel behind it and its buttons sit in one block clear of the action bar.
- The target frame is spaced away from the player frame, and a long cast name is shortened so it no longer runs into the cast time.
- The gear and shop windows open below the player frame, "companions: ~/pets" is a button of its own instead of a tab beside the close box, and the career window's two header lines no longer overlap.
- Notices such as the vendor's line show above the windows instead of across their title bars.
- Dungeon rooms (Basement, HACK 24, VibeCon, the Ilmari wings) now greet you with a landmark near the entrance, one oversized prop and a tinted light per room, so you can tell where you are at first glance.
- Indoor rooms are brighter and less tinted, the VibeCon Afterparty and Main Stage are readable, the occupied campus is lighter, and nights outdoors are not quite so black.
- Entering an indoor room starts the camera higher and further back, kept under the ceiling.
- Esc closes the window you opened last instead of a fixed one first, and opening a centred window (map, career, endgame, calendar, sprint windows) closes the other centred one. The pause menu groups its buttons into Game, Account and Danger, with Start over alone under Danger. "IN REVIEW" tickets are violet so they no longer look like main-quest gold.

### Fixed

- Done daily tickets now show their "!" again the next day when you play online, and the Ilmari opening ticket stays hidden online until the AGI has fallen.
- The yellow car at the start of the car park no longer clips its warning cone or fills the camera, and the people at the arrival spot stand apart so their speech bubbles stop stacking.
- The Cooling Deck's leftover "Lab Coat Door - COMING" sign is gone now that the Lab is open.
- The quest tracker no longer shows an empty "Objectives (0)" box when only a countdown is running.
- Cutscene cameras no longer end up inside a reception desk, a wardrobe or a ceiling panel when a scene starts.
- Campus signs stay readable at night instead of going nearly black.
- Quest givers near the edge of the screen, in a crowd of colleagues or under a HUD panel could lose their "!" or "?" because the whole nameplate was culled; the marker now stays up and the plate slides back into view. Ill Marie's queue spot now appears the moment you reach level 5 instead of after the next zone load.

## [0.10.0] - 2026-10-24

### Added

- Training Data, behind the Innovation Lab airlock on the occupied campus: three rooms and three bosses who scrape, hallucinate and learn from you, with their own gear and titles. Bring a Data Scientist, or read the boards on the walls.
- The Data Scientist can fact-check the room with Ground Truth (default key: comma) when the hazards stop agreeing with each other.
- The Cloud, up the Data Center's new roof ladder on the occupied campus: three rooms and two bosses with a bill, an outage and their own gear and titles.
- Both wings offer Story (one colleague can see it all, Resistance volunteers welcome) or Full (bring friends), and work in co-op rooms.

### Changed

- Co-op needs this version on both sides (protocol 40).

## [0.9.0] - 2026-10-23

### Added

- Ill Marie turns up in more places, and she has two errands for you. Bring tissues.
- Something happens after the credits, and there is plenty to do about it: new places, quests, a daily and some gear that is only as good as the place it is worn.
- Datacenter Uprising, behind the occupied campus's west gate: three rooms and three bosses who read their own release notes.
- Human Resources, through the Tower's lobby: three more rooms, three more bosses and a reorganisation nobody asked for, with its own gear, titles and a closing chapter.
- Both wings offer Story (one colleague can see it all) or Full (bring friends) at the door, with separate weekly lockouts; Story pays half the tokens.
- Three Resistance volunteers, Dale, Priya and Rupert, fight beside you on Story: sign up for two of them at the sheet in the base. Full stays a real group fight.
- The post-credits world and both wings work in a co-op room too: the room opens it together and each player earns their own tokens.

### Changed

- Co-op needs this version on both sides (protocol 37).

### Fixed

- Signs and posters no longer flicker against the board or wall behind them and read the right way round from behind (the graveyard sign included); rugs, road markings, lamp light and other flat details no longer shimmer from a distance.

## [0.8.0] - 2026-10-22

### Added

- Dungeons for three to five players, each three or four rooms and three bosses; alone or in pairs you attend the softer Async version. Each boss pays gear once a day (resets at midnight UTC) and repeat clears pay less XP, so dungeons never out-level the open world:
  - The Onboarding (levels 3-4): the Compliance Trainer's pop quiz, the Laptop Provisioner's six weeks of "awaiting hardware" and your Onboarding Buddy, who is suspiciously eager to help.
  - The Migration (levels 6-7), through the Data Center's loading dock: the Rollback Wraith, the Consultants (one is always billing) and the Big Bang Cutover.
  - Quarterly Business Review (levels 9-10), through the Tower's executive door: the Slide Deck Golem, Dana, who had Claude do the analysis, and the three-headed Board.
  - The Hackathon (level 10, heroic, opens after a QBR clear), behind the Incubator Street coworking cafe: the Pizza Overlord, the Demo Day Gremlin and the Pitch Judge VC, who only funds AI.
- Security Engineer stealth openers: while Tailgating, four bar keys turn into Supply Chain Attack (a big opener from behind), Rubber Ducky (a 4 s stun), Phishing Call (keeps a regular enemy on the phone for up to 30 s until hit) and Keylogger (damage over time with a 3 s silence).
- DevOps SIGSTOP also puts its target in Maintenance Mode for 4 s (any hit wakes it).
- A soundtrack: every tower floor, data center hall, the lab, the car park, the campus, the home office and each raid wing have their own music, a night track plays outdoors after dark and raid finales get boss themes. The web version only downloads the music of places you visit.
- Ill Marie, a colleague who came in sick (again), coughs through her day on the boulevard up from the South Gate and has a small errand for you. It's just a cold. Probably.
- Somewhere in Wellness Park, on the quiet lawn by the lake, a family is having a picnic. Say hello.

### Changed

- The campus has room to breathe: it grew to 520 x 360 m, the arrival lawn is clear, menus, quest givers and the Company Calendar moved into a new Services Street, the Innovation Lab and the Data Center stand on their own green and square, and signposts at the junctions point to every dungeon and district.
- The Tower is rebuilt as three rounded drums in bronze fins, one lifted on slanted columns over the entrance, with green roofs, an AI-first beacon and an unfinished crane.
- Every entrance reads from afar as what it leads to: a conference hall with a registration queue for VibeCon, a coach stop in Wellness Park for the Offsite, a loading dock for the Migration, a red-carpet executive entrance for the QBR, a hacker loft for the Hackathon, a balloon queue for the Onboarding and one Company Calendar pavilion.
- The Tower floors, the Data Center and the Innovation Lab are tidier: fewer duplicate props, clear walking lines, the same jokes.
- Nights outdoors are less pitch black: street lamps throw a warm pool of light and light up the people around them.
- The people strolling the campus show a small grey name tag when you hover over them.
- Diminishing returns: your stuns and incapacitates on the same enemy within 18 s last half as long, then a quarter, then not at all. Enemy stuns on you are unchanged.
- QA Engineer: +5 spell power (25), it was the slowest role in most zones.
- Unit frame portraits are bigger and show just the head.
- Co-op needs this version on both sides (protocol 30); older clients are turned away with a clear message.

### Fixed

- In Quarterly Business Review, Dana's Outlier and the Board's Abstain now actually slow you; before, they only slowed enemies.
- The ring under your target no longer flickers over rugs, floor markings or hills.
- The promotion pop-up no longer hides under the dungeon difficulty banner.

## [0.7.0] - 2026-10-21

### Added

- Your unit frame and your target's frame now show a real round portrait of the character (head, hair, glasses and headphones included) instead of a letter. Your hair thins in the portrait as your health drops.
- The landing page shows all eight roles, mentions the class props, says which browsers run the game, and has an "Install the game" button: the game installs as its own fullscreen app (Install button in the game on Chrome and Edge, Share, Add to Home Screen on iOS) instead of installing the website.
- Class props: sixteen consoles around the campus, the car park, the tower floors, the data centre and the labs that only one role can use (DevOps restarts a frozen rack, Frontend centres the billboard, Backend raises a vending machine's rate limit, QA clips through a glitched wall, the Data Scientist reads a dashboard, Security tailgates a locked room, the COBOL Greybeard revives the mainframe). They are pure bonuses, never needed for a ticket: short team-wide buffs, floor shortcuts, hidden loot, Kudos and burnout relief, and in co-op they help everyone nearby. Other roles get an in-character refusal; the Fullstack Engineer can try any of them, slower, with a chance to break the prop. A class-coloured marker and a "Requires" tooltip show who a prop is for, and one-time finds stay found.
- A seventh role, the Security Engineer: a melee stealth class that builds Exploit Chain points with Shoulder Surf, Phishing Link, Zero-Day and Lateral Movement, then spends them on Privilege Escalation and Data Exfiltration.
- Tailgating: sneak past enemies (they notice you from a sixth of the distance, you walk slower and any attack or hit gives you away); co-op teammates see you as a translucent character. In a fight it works as a vanish.
- Social Engineering puts a regular enemy on hold until it is hit, Disable Account interrupts, and Incident Response heals, shields and cleanses so the role plays solo.
- An Exploit Chain meter above the action bar, a role card on the picker (key 7), a black hoodie with sunglasses and a VISITOR lanyard, and a Career Ladder with fifteen talents.
- An eighth role, the COBOL Greybeard: a slow two-handed bruiser who has been here since 1987. He spends Seniority, a bar that builds with time and with every hit he lands or takes, on Rewrite It in COBOL, Batch Job and Core Dump, and keeps control with GOTO, Punch Card, Code Freeze and Maintenance Window.
- Batch Job paints a cyan ring on the floor that hits the target and everything around it a moment later; Read the Manual, Pension Plan and Mainframe Uptime keep the Greybeard standing so the role plays solo.
- A Seniority bar above the action bar, a role card on the picker (key 8, two rows of cards), a cardigan, grey beard, reading glasses and tie, and a Career Ladder with fifteen talents.
- Graphics presets Low, Medium, High and Ultra, with render scale, MSAA, shadow and view-distance overrides in Settings.
- The game picks a quality preset on first start and offers a lower one when the frame rate stays under 30.
- Roasts for the moments that earn one: a different insult on every death (by boss, regular enemy, fall or burnout), escalating remarks when the same boss keeps killing you, a fresh line on every loading screen, and jabs when you idle, wipe as a group, lag, play for hours, run out of Kudos at a vendor, level slowly or lose the connection. Lines come from shuffled pools, so none repeats until the pool has run through, and none of them blocks play.
- The web version's update toast and the offline start now have attitude too,, joining the roasts already shown when the game drops to a lower quality preset.
- A `~/perf $ top` window (F6) with frame times, scene counts, memory and, online, ping, jitter and server tick.

### Changed

- The death screen text sits on a dark band, so it stays readable over nameplates and busy scenes.
- The web version now runs on WebGPU for smoother rendering; browsers without it (Chrome, Edge or Safari 26+ have it) show a notice instead of the game.
- Faster rendering: distant NPCs and extra lamps are no longer drawn, and the HUD only redraws what changed.
- The web version downloads less before the first scene and caches the rest in the background.
- After an update the web version keeps every model and sound that did not change, so offline play keeps working.
- Far fewer tickets in each building: near-duplicate kill and drop tickets are merged into one, and the remaining side quests open in order (finish one to get the next) instead of all at once.
- Story (main) quests are marked apart from side quests: gold "!" and "?" over their NPCs, azure ones for side quests, a MAIN QUEST or SIDE QUEST label in the ticket, a main quests section first in the quest log and main quests first in the tracker.
- Quest items in the loot window say "Quest item", and every quest item and every quest pickup has its own icon; pickups glow in their own colour and show their item.

### Fixed

- In Car Park P3 the Burnt-Out Colleague and the intern no longer sit in mid-air: each now has a bench to sit on.
- A first visit to the web version no longer reloads the page by itself once the offline cache is ready.
- Quest givers can no longer be killed by a boss or enemy area attack, so you can always hand in your ticket.
- Quest pickups you already collected stay gone, also after saving and reloading.

## [0.6.0] - 2026-10-20

### Added

- Open-world campus with two new classes and interactive quests.
- A bigger campus, the Car Park P3 starter zone and a home office.
- Burnout tiers, real-looking water and more AI satire in Car Park P3.
- A landing page with the game embedded, a full-page mode, co-op and self-hosting info.
- Version number in the Esc menu, this changelog and GitHub Releases.

## [0.5.1] - 2026-10-19

### Fixed

- Every quest objective is reachable, and the stale `!` on the Company Calendar is gone.

## [0.5.0] - 2026-10-16

### Added

- VibeCon raid, Teambuilding and the Company Calendar.
- Loot window rework and an art pass for the zones outside the tower.
- Gameplay variety across zones, raids, Legacy Mode and night.
- Art polish for the Innovation Lab, the raids and taller halls.

### Fixed

- Scrolling windows, easier looting and rug corpses.

## [0.4.0] - 2026-10-15

### Added

- Public room browser for co-op, corpse despawn, a new XP curve and seat placement.
- Onboarding, dailies, a world boss, pets, duels and the Kudos economy.
- Discard items and throw junk out of the bag.
- Pets journal hotkey; mute is remembered.
- Three times the item pool, with visible gear, sets and junk.
- Body Leasing side trip and a decluttered touch HUD.
- Promotion ranks, department reputation, an endgame panel and burnout.
- Legacy Mode heroic floors with weekly boss lockouts.
- All-Hands Offsite raid and the weekly Sprint ladder.

### Fixed

- The gadget bar no longer overlaps the damage meter and the touch buttons.

## [0.3.0] - 2026-10-14

### Added

- Co-op interaction: ally spells, resurrect, shared loot and scaling.
- Four classes, a chat window, a combat log and a damage meter.
- Enemy mechanics, a reworked CEO fight, a gear economy, a minimap and rebindable keys.
- 48 in-zone quests with a tracker, scooters and an interactive office.
- Career ladder talents and Sprint mode.

### Fixed

- Online cast bars fill, ping is measured and the co-op server moved to Frankfurt.
- Change role shows up in the Esc menu.

## [0.2.0] - 2026-10-13

### Added

- The CEO boss, auto attack and misses.
- Developer-themed HUD, chibi characters and regeneration.
- The game is now called The Last Dev.
- A five-floor office tower with bosses, levels and the satire cast.
- Painted icons for every spell and aura.
- Office floors, trophies, spell effects and living NPCs.
- Coffee break, pathfinding, audio and loot indicators.
- The campus hub, the data center, the Innovation Lab and quests.
- Co-op multiplayer beta, touch controls and an installable PWA.
- Smoother co-op, server hardening and a Render deployment.

### Fixed

- Loot no longer crashes WebGL2 (16-bit icon PNGs are converted to 8-bit).
- The web lobby fields use a single text source.

## [0.1.0] - 2026-10-12

### Added

- First playable build: an arena with movement and targeting.
- Download progress for the wasm in the loader.
- Combat, a classic MMO HUD and animated models.

[Unreleased]: https://github.com/wraithyy/the-last-dev/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/wraithyy/the-last-dev/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/wraithyy/the-last-dev/compare/v0.11.0...v1.0.0
[0.11.0]: https://github.com/wraithyy/the-last-dev/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/wraithyy/the-last-dev/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/wraithyy/the-last-dev/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/wraithyy/the-last-dev/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/wraithyy/the-last-dev/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/wraithyy/the-last-dev/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/wraithyy/the-last-dev/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/wraithyy/the-last-dev/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/wraithyy/the-last-dev/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/wraithyy/the-last-dev/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/wraithyy/the-last-dev/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/wraithyy/the-last-dev/releases/tag/v0.1.0
