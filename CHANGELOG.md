# Changelog

Notable changes to The Last Dev. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/) while the game is in 0.x: a minor
version adds content (zones, classes, systems), a patch version only fixes things.

Versions before 0.6.0 were tagged after the fact, one per day of development.

## [Unreleased]

### Added

- A seventh role, the Security Engineer: a melee stealth class that builds Exploit Chain points with Shoulder Surf, Phishing Link, Zero-Day and Lateral Movement, then spends them on Privilege Escalation and Data Exfiltration.
- Tailgating: sneak past enemies (they notice you from a sixth of the distance, you walk slower and any attack or hit gives you away); co-op teammates see you as a translucent character. In a fight it works as a vanish.
- Social Engineering puts a regular enemy on hold until it is hit, Disable Account interrupts, and Incident Response heals, shields and cleanses so the role plays solo.
- An Exploit Chain meter above the action bar, a role card on the picker (key 7), a black hoodie with sunglasses and a VISITOR lanyard, and a Career Ladder with fifteen talents.
- Graphics presets Low, Medium, High and Ultra, with render scale, MSAA, shadow and view-distance overrides in Settings.
- The game picks a quality preset on first start and offers a lower one when the frame rate stays under 30.
- A `~/perf $ top` window (F6) with frame times, scene counts, memory and, online, ping, jitter and server tick.

### Changed

- Faster rendering: distant NPCs and extra lamps are no longer drawn, and the HUD only redraws what changed.
- The web version downloads less before the first scene and caches the rest in the background.
- After an update the web version keeps every model and sound that did not change, so offline play keeps working.

### Fixed

- A first visit to the web version no longer reloads the page by itself once the offline cache is ready.

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

[Unreleased]: https://github.com/wraithyy/the-last-dev/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/wraithyy/the-last-dev/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/wraithyy/the-last-dev/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/wraithyy/the-last-dev/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/wraithyy/the-last-dev/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/wraithyy/the-last-dev/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/wraithyy/the-last-dev/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/wraithyy/the-last-dev/releases/tag/v0.1.0
