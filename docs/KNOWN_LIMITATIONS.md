# Known limitations

Status after Phases 0-4 plus the zone graph (campus, tower, data center, lab) and quests, wired in sim and client. Remove an item once a test or verification covers it.

## Not implemented yet (planned)

- Phase 5 polish: profiling, wasm size reduction (no `wasm-opt` yet), settings menu.
- Escape opens a minimal pause overlay only, with no real menu.

## Classes

- Hot Reload reuses the sim's `Teleported` event to move the dashing player, so the event feed prints the Reorg line ("reorganised to a new location") for a dash.
- Class kits (glasses, studio headphones, pager, Fullstack neck headphones) are primitives on bones; only the local player carries the coffee mug.
- Heals still generate no threat, so a healer never pulls aggro off the tank by healing.
- The role picker's stage camera is a second `Camera3d`; while the picker is open, systems that expect exactly one 3D camera (camera follow, nameplate placement) pause. The run is paused underneath anyway.

## Co-op (online)

- Gear, bag and Kudos are per room session; joiners start fresh (no import of the offline save, which the server could not trust).

- Player interaction is deliberately narrow: target a teammate (click, party frame, F1 self / F2-F5 members), cast the helpful spells on them (Guardrails, Rollback, On-Call Caffeine), resurrect with "Have You Tried Turning It Off and On Again" (V), and aim emotes at them. There is no trading, no free-text chat, no duels, no inspect and no item handover; trophies are personal.
- Loot is per player and private: each member gets their own roll, only the looter sees `LootDropped`/`Looted`, and sparkles, the gold plate and the loot cursor follow your own unclaimed roll (a corpse a teammate still has to loot shows nothing for you). A corpse whose roll was empty for you looks like an empty corpse offline: no sparkles, and looting says there was nothing. Corpse decay is timed per client from when it first saw the death or its own roll emptied, so after a late join or a reconnect an old corpse can stay up to its full timer again (the server's sim still despawns it on time).
- Quest credit: `Kill` objectives credit the killer and every living member within 60 m; `Loot` and `Talk` only the acting player; `ClearZone` everyone. There is no quest sharing; each member accepts tickets from the giver.
- A resurrected teammate stands where they died, so enemies still standing there pull them again; resurrect after the area is clear.
- Own casts are predicted from local knowledge (cooldowns, GCD, mana, target kind); the server can still refuse (range, line of sight, a cooldown the client missed) and the bar then vanishes with the error.
- The resurrect key has no action-bar slot (the bar is full); it is a V keycap right of Coffee Break, shown only while someone else is in the group.
- `~/metrics` counts the damage and heal events the client receives; damage/heal events are broadcast to every member, so all group members appear. Bars use party blue; per-class colours are not wired. Fights end 4 s after the group leaves combat, and the report resets on a new game or when the session changes (entity ids differ).

## Presentation

- The chat window shows speech, emotes, joins/leaves and progress notices; quest dialog text is not mirrored into it (only ticket accepted/ready/closed). Logs scroll with the mouse wheel only; the touch layout has a tap strip that cycles chat, combat log and collapsed, without touch scrolling or the meter.

- Floating numbers and hit reactions of projectile attacks wait for the projectile to land (1.5 s fallback). Health bars and the combat log stay sim-authoritative, so a log line and the HP drop can precede the visual by the flight time.
- Chair colliders float 0.12 m above the floor so the shared blocker list (movement, camera, combat LoS) does not stop ground-level LoS rays. A dedicated movement-only list would be cleaner but needs `player.rs`/`combat.rs` changes.
- The ceiling is not a collider; the camera is clamped below it by height (`CAM_MAX_HEIGHT`).

- The cast pose is the one-handed shoot clip held on its last frame, and backpedal plays the walk clip reversed. The hand glow position is tuned by eye, not bound to the fist bone.
- Nameplates and floating numbers are screen-space UI nodes: they are culled by distance (40 m) and camera direction and not occluded by walls. Plates are de-cluttered by stacking them upward (target first, then nearest first in 2 m bands); in a very dense pack the plates pushed furthest shrink and then vanish, because Bevy UI has no group opacity to fade them. Floating combat numbers are not de-cluttered.
- The minimap rasterises the zone once per zone from the layout (floor, walls, blockers) and slides it under a clipped view; it is square, not round, has no fog of war, and shows hostiles and quest givers from the sim state (so it also shows them through walls). Zoom steps are 18 to 90 m.
- Every spell, aura and trophy has a shipped icon (test `every_spell_aura_and_trophy_has_a_shipped_icon_file`); a key without a file would fall back to a generated letter badge.
- The exit elevator doors and lamp are client-only overlays in front of the static dressing; they are not colliders.
- Floor 1's sleeping IT Support comes from the catalog pose for that anchor (`catalog::npc_at`); the sim only knows one pose per unit kind. Mini-boss "ELITE" tags are a client list (`copy::is_elite`).
- DoT-tick numbers are detected by a heuristic (damage from a spell without a direct-damage effect); the "EXECUTE!" pop is judged from the target's HP after the hit.
- The player's hair stages and accessories (neck headphones, coffee mug) exist only on `Engineer.glb`; accessory offsets are tuned by eye in bone space.

## Simplifications

- Collision uses axis-aligned boxes only. The player is a box/cylinder approximation, with no slopes and no rotated props.
- Camera collision uses `combat_core::line_of_sight` plus bisection against the same boxes. It does not collide with non-solid decor.
- Click-picking uses a ray-vs-sphere test per unit, not mesh-accurate picking.
- The player can steer while airborne (WoW allows only limited air control).
- In RMB-steer mode the camera follows the interpolated character yaw, so it can lag the mouse by up to one 33 ms tick.
- `FixedStepper` caps catch-up at 8 ticks per frame. Longer stalls drop the excess whole ticks.

## Zones and quests

- `load_floor`, `Sim::floor()`, `Content::floor()` and the `Floor*` events are deprecated twins of the zone API; the client uses only the zone API.
- Doors are trigger boxes, not opening doors: walking into an unlocked one starts the fade, a locked one prints its reason (at most every 3 s). Arrival is in front of the door back to the zone you came from.
- The map draws the layout's blockers as a top-down raster; it has no fog of war, and the zone tree is a static list of the content's zones.
- Quest markers and the ticket dialog read `Sim::quests()` every frame; there is no quest sharing, no quest item highlighting on corpses and no waypoint arrow.
- Debug hooks set quest states (`WOW_QUESTS`) without giving the items or kills, so a forced `ready` ticket can be turned in for free.
- Trash respawns zone-wide: after `TRASH_RESPAWN_MS` away every killed pack unit comes back on re-entry; there is no per-unit timer and no respawn while the player stays in the zone. Respawned trash gives XP and loot again (quest items can be farmed this way, by design).
- Quest failures (too far from the giver, wrong status) are silent, like `Talk` and `Loot`. Abandon emits no event; the UI reads `Sim::quests()`.
- Quest state, trophies and world progress are per `Sim` (session only). Quest logs are keyed by player id, but commands act for the single player.
- Loot objectives count pickups while active plus copies carried at accept time; turned-in quest items stay in the inventory (they are ordinary trophies with small bonuses).
- `DrainMana` emits no event; the HUD sees the mana bar drop. Spell reflect (Alignment Training) also bounces DoT ticks of the player's spells.

## Dungeons (The Onboarding, The Migration, Quarterly Business Review, The Hackathon)

- No new engine effects were added, so a few mechanics of docs/DUNGEONS.md use the closest existing one: the Laptop Provisioner's "Out For Delivery" is a Line telegraph (no boss teleport; `Teleport` moves the target), its "Reimage" is a damage-dealt debuff instead of stripping buffs (`Dispel` only removes harmful auras), and the Forgotten Probation Intern hits a random target (`AiTarget::Caster` is the unit itself, so it cannot fixate "the caster").
- The Pop Quiz paints three of four answer lanes aimed at the target (no lettered floor tiles, which cannot follow a random aim); the Name Tags are a 5 s fuse (`Detonate`) rather than exploding on a hit.
- The Fern pet, the dungeon's department reputation and the Company Calendar "Dungeon Finder" entry are not built yet (reputation is only paid at the level cap, and the dungeon is level 3-4).
- Dungeon gear is rolled from the boss's own pieces (`source: Boss`), not as separate named drops at `legendary_pct`; the doc's named-piece roll and the tier-set `sets` field are unused here.
- The Migration: no new engine effects either. The Data Sync tether pairs an odd player out with the boss itself (the doc's summoned Orphaned Record is not the second end), the Consultants' Gantt Chart and the Cutover's Traffic Switchover are Line telegraph waves, and the Orphaned Process's Zombie is a one-shot `Summon` on death, so killing the Zombie ends it. The DevOps snapshot rack, the COBOL label joke and the Hypercare Cape are not built.
- Quarterly Business Review: no new engine effects. The KPI Dashboard's flank hit does not turn it red (it periodically goes red and detonates instead, behind its `FrontalShield`), the Catering Drone's pastries are hostile Croissant adds whose death heals whoever stands beside them (nobody walks over to "pick one up"), the Animated Pie Chart's safe wedges are three painted floor pads wired to the telegraph's `cover` spots (no per-wedge geometry), and Dana's Hallucinated Metric is two spells (a flattering buff on herself, a mild debuff on a player; buffs on bosses cannot be dispelled). The Board is one panel of three linked heads (the Chair, the Activist, the Independent): the Activist's "Shareholder Value" marks a random player instead of fixating on one, "Quorum lost" is a phase aura on all heads rather than a bonus per head that dies, the Chair's "Motion on the Floor" paints a ring under every player (spread out) rather than two safe rings out of three, and "Motion to dissolve Engineering" is a phase at 20% (not a timer). The Executive Chair Mount cosmetic and the Golden Parachute's once-a-day survive are not built; Animated Transition Boots are a Lanyard piece (there is no feet slot).
- The Hackathon: one small engine change (a dungeon door on the campus now also honours its link's `requires`, so the Hackathon wants a cleared Quarterly Business Review on top of level 10) and otherwise no new effects. The Sleep-Deprived Developer's "Commit Without Reading" hits a random player, never an ally; Dietary Requirements' vegan aura shaves max HP (there is no healing-taken aura), gluten free slows and "whatever's left" hits softer; Extra Cheese's Slices heal the Overlord for a flat sliver instead of 2%; the Hot Slice's one safe wedge is a single `cover` pad (the pineapple wedge, always in the same place) repeated three times; the Gremlin's Wrong Tab locks Arcane only and its "Hard Refresh" cannot strip buffs (`Dispel` only removes harmful auras), so it is a ring that leaves "Cache Cleared" (less damage) behind; Alt-Tab and the Judge's Next Meeting are Line/instant `Teleport` effects over a short, in-room distance; Valuation does not reset on a dispel (buffs on bosses cannot be dispelled), it just caps at eight stacks; "Powered By AI" is a harmful-flagged aura that also adds damage so a dispel can remove it. The Security terminal that phishes a pack, the Backup Demo Video's staged resurrection and the AI Hat cosmetic are not built. Dungeon reputation is still `boss_rep: 0`, so the +50% heroic reputation is not paid.

## Debug bot

- `WOW_AUTOFIGHT` steers in a straight line with a sidestep when stuck (no pathfinding), and dies on floors above its level. It is a smoke-test aid, not balance evidence (see `tests/playthrough.rs` for that).
- There is no god mode (`WOW_GOD`); `WOW_LEVEL` is the way to survive a floor in tests.

## Not verified

- Movement and camera feel, and click-vs-drag accuracy, need a manual playtest (native and browser).
- Talking via right-click / F and the R restart were exercised only through debug hooks (`WOW_TALK`, a temporary auto-restart), not with real input.
- Browser: pointer lock after a drag, context-menu/scroll suppression and WebGL2 shadow quality across browsers. See README "Verification status" for what was actually run.
- Only macOS (Apple Silicon, Metal) has been tried natively.
- No FPS or memory profiling yet (Phase 5).

## Combat

- Only a flat hit roll exists (95% white, 96% spell, no level difference): no dodge, parry, block, armor or partial resists; enemy melee has no crit. Misses generate no threat, so a missed first attack does not pull.
- Enemies chase in a straight line on the ground plane and ignore blockers for pathing (they only use LoS to aggro and to start casts).
- Heals generate no threat; threat is damage only (plus 1 for a pulling debuff). Out-of-combat regeneration is a fixed percent tick (no spirit or gear scaling), and the in-combat mana trickle is flat per unit (does not grow with level).
- Pushback applies to the player only; enemy casts keep a fixed 2.5 s telegraph.
- Spell queue: one slot, static errors (cooldown, mana, range) are reported at press time instead of being deferred; a spell whose own cooldown ends inside the window is not queued.
- The effect set is closed (see COMBAT_API.md); there are no channels: "channel" abilities (Review Request) are ordinary interruptible casts. One aura per key per unit: auras from different casters with the same key share stacks and the newest caster's snapshot.
- Only two schools exist (Physical, Arcane). School-lock mechanics (Requirements Change, Block All The Things) therefore lock either every arcane spell or the two physical ones (Rubber Duck, Write It By Hand).
- `Lockout` without an active cast emits no event; the HUD sees it only through `CooldownView.usable == Err(SchoolLocked)`.
- Reprioritize (`ThreatSwap`) swaps the top two threat entries; solo, with the player as the only entry, it is a no-op apart from its yell.
- Pack formation offsets are world-axis XZ offsets, not rotated by the anchor yaw. Knockback and teleport do not check blockers: the client must clamp the player body against walls.
- The Reorg teleport lands 4 m in front of the VP's spawn point regardless of the room layout.
- Despawned units stay in the sim's unit vector (ids are never reused), so memory grows slowly with every floor load and summon.
- Bosses are immune to stuns only; silence, slows and knockback flags are not distinguished.
- Balance is verified by a scripted bot (`tests/playthrough.rs`: three seeds, each zone at its max level, plus the campus quest chain from level 1) that kills adds first, kicks interruptible casts and uses Rollback/Guardrails; a human who ignores mechanics will find floors 2 and 5 hard. Guardrails' absorb (100) does not scale with level.
- Reject PR unlocks at level 2 and Guardrails at level 3 (swapped versus GAME_DESIGN.md) so the floor-1 boss interrupt is possible on floor 1.
- Mini-bosses (`hr_partner`, `marketing_lead`, `ciso`) are mandatory pack_6 pulls in the sim; nothing marks them optional except that the floor clears on the boss alone.
- `Command::Loot` failures (too far, not lootable) are silent: `CastError` has no loot variants. Corpses are always lootable, possibly with an empty loot list.
- Auto attack is a single ranged "Nitpick" on the player; the swing timer is not hasted and there is no melee auto attack for the player.
- Evade speed equals chase speed, and the leash is measured from the spawn point only.

## UI

- Keybindings: one key per action (slot keys and numpad keys are separate actions, so there is no secondary binding). Esc, Enter, Backspace, F9 and modifier keys other than Shift cannot be bound; the ticket dialog (arrows, Enter, Backspace), the victory restart (R), the role picker (1-4) and the debug overlay (F9) are fixed. A key already in use is swapped, never rejected. The panel is hidden in the touch layout; touch buttons press whatever key their action is bound to.
- Aura tooltips: the content defines no aura prose, so the description line is derived from the aura's mechanics (stun, silence, periodic damage or heal, stat percentages), and the name is the aura's display name in snake_case. Party frames show up to 6 aura icons per member; the target frame shows harmful auras only.
- The cast tooltip exists for the target's cast bar only; the player's own cast uses the action bar tooltip.
- Burnout: no screen desaturation and no chat typos yet (the gauge pulse, banners and misfire yells carry the effect); the Company Calendar only lists burnout costs and relief, it does not react to the tier.

## Touch and PWA

- Web lobby text fields: a hidden HTML input is the text source. Keys typed in the same frame as the
  click that focuses a field can be lost (only reproducible with scripted input, not at human
  typing speed). Real DOM inputs overlaid on the Bevy fields would remove the race.

- Touch layout is verified only with Chrome touch emulation; real iOS Safari (fullscreen on Add to Home Screen, audio unlock, safe-area insets) is untested.
- A keycap tap casts on press, so a long press shows the tooltip while the spell has already fired.
- Touch taps on HUD panels without a `Button`/`Interaction` (windows, dialogs) can still reach target picking, same as mouse clicks.
- The service worker precaches everything; if the browser refuses the shell (low disk/quota) the install fails and the game keeps running online without offline support. Asset-only quota errors are logged and skipped.

## The Opening after the credits (docs/ILMARI_09.md B.1)

- The stamp shows on one full-screen overlay, not on every in-world screen (tower sign, LED wall,
  scooter docks): world signs are baked textures with no runtime text. The chord and the sirens have no
  sound, and Marie does not walk to the tower; the reveal retires her where she stands.
- The sim has no wall clock, so the client stamps it. Online, every client plays its own Opening
  when the room announces it (`IlmariPending`); the first member to finish or skip it reveals it for
  the room, and a member who joins afterwards never sees it. The room keeps the first stamp sent, not
  the lead's, and the stamp line each client shows is its own.
- Native reads the zone with `date +%z %Z`; Windows has no such `date` and stamps UTC. The web build
  has the offset but no abbreviation, so it prints `UTC+hh:mm`. The date is always `D Month YYYY`, 24 h.
- A player with the "Cough Drops" ticket still open when the Opening plays loses its giver (the queue
  Marie retires with the desk and bench); abandon and re-take is not possible, the ticket just stays open.

## The Prologue (docs/ILMARI_09.md B.2-B.6)

- The occupied campus is the campus geometry exported in full (`zone_geometry.ron`), not as the delta
  B.2 sketched; the server needs no special loader and the file grows by one campus (about 700 lines).
- The attunement chain ends at `il_06`; the wing doors it opens arrive with the wings (B6). Until then
  `il_05_hr_intel` waits only for `il_04_wing1_attune`, not for the Wing 1 completion quest `il_w1_done`.
- The three ilvl 18-20 band pieces are on Brenda's shelf (120 and 160 tokens) until the wings drop
  their own; `GearSource::Ilmari` pieces have no perks yet, and a perk would be off outside Ilmari
  content like the stats.
- The weekly quartermaster rotation follows sim time (`rotate_ms` per window), not the calendar week;
  the token allowance (200) follows the ISO week of the office clock.
- Brenda sells no cosmetics or titles yet (no shop for them exists); consumables and band pieces only.
- Ambient android extras are presentation only (`live/ambient.rs`): they pace fixed lines, do not
  react, and have no sim unit. The "ILMARI" screens are baked textures on the landmarks; the Lab's
  splash is on its east face because its neon takes the front.
- Online the hatch, the occupied campus, the base, quests, tokens and the item band run on the
  server's sim like the rest of the room; the net client's tooltip totals do not track the zone and
  count Ilmari pieces at the base cap. There is no online save: a room's progress ends with the room.

## Wing 1: Datacenter Uprising (docs/ILMARI_09.md B6-B10)

- Story is solo-completable by numbers alone (Async scaling under its own lock key), verified by bots for every class over seeds 7, 42 and 1234. Companions (Dale, Priya, Rupert, pick two at the base's sign-up sheet) are optional company on Story in the wings only: no Dispel, Resurrect or Label Cable for Priya, and they ignore shared (split) telegraphs instead of stacking in them.
- Label Cable is a printer prop (`PropKind::Label`), not an action-bar spell: every class bar already holds ten spells. DevOps labels two cables per use and reloads three times faster; other classes label one.
- Candidate humans cost every player 15% burnout (`death_burnout` is per mille); the spec's 12 would not read as a spike.
- Enrage is a damage and haste aura (`il_overrun`) at 150-180 s rather than 7-8 minutes; without it a lone COBOL Dev cleared Full.
- The patch changelog is baked into sign textures that share one material, so every screen in a room swaps together.
- The Full bot trio is Backend, Fullstack and Frontend; a healer-in-trio run wiped at the Recruiter and is not a gate.

## Wing 2: Human Resources (docs/ILMARI_09.md B11-B14)

- Story is solo-completable by numbers alone (companions are optional, B15), verified by bots for every class over seeds 7, 42 and 1234; the Full bot trio is Backend, Fullstack and Frontend (no healer, no dispel: they appeal at the HR Portals, lobby the ballot terminals for the policy that hurts bots least and clear without a death). A lone COBOL Dev beats the Full bosses without the enrage, so Full enrages at 118-120 s.
- The mirror is of a role, not of the player's class: the copy of a tank, a healer or a damage dealer has that role's three-line kit and wears the class model of the role with a cold glow, and its nameplate is the player's name, glitching and red against the teammates' steady blue.
- The wall boards (vote, grades) are one shared texture per room, redrawn from the raid view: every board of a room shows the same lines. A copy of the room's state is not stored on the server beyond `RaidView`.
- The review grades what the sim counted since the previous review, not a rolling 30 s: the first window starts when a boss fight does. Healers who only heal get credit at the healing/absorb rate; a healer with nobody to heal grades low, which is the point and the Story threshold (25%) forgives.
- The Wing 3 door is not in the game: the closing cutscene only mentions a button that lit and went dark.

## Ilmari co-op: accepted risks (security review 2026-10-23)

- Ballot terminals count every use (the Culture Committee mechanic is running between terminals
  to lobby), so in a group one player can swing the vote alone. Accepted: it is the designed
  mechanic and only affects players who chose to group together, not other rooms.
- Ilmari tokens online live as long as the room session (like the rest of the online inventory),
  so leaving and rejoining a room starts a fresh weekly cap there.
- The label printer may tag cables anywhere in the Hydra room, not only near it.

## Ilmari 0.10 engine pieces (docs/ILMARI_10.md C.3)

- Only the engines exist: no Wing 3 or 4 zone, boss, quest or door yet. The swarm, illusion and recorder hooks are reachable from tests, `Sim::spawn_swarm` and the `WOW_SWARM` QA hook only.
- Illusions are drawn by the client from the public salt, so a modified client can draw only the real hazards. Accepted for PvE; server-generated per-recipient fakes are a later option.
- Drones are pooled entities, not sim units: the sim never sees a drone position, and a drone killed by a hit is not individually animated. The drawn count is capped per quality preset, so a Low-quality player sees a smaller cloud than the HP pool claims.
- The `swarm` bench stages the clusters in Car Park P3 as a stand-in for the Scrape Pit; compare its numbers with `carpark`.
- Ground Truth stays Ilmari-only, ilvl 20 stays reserved for the 1.0 core.

## Wing 3: Training Data (docs/ILMARI_10.md C.4)

## Wing 4: The Cloud (docs/ILMARI_10.md C9-C14)

- The Region Outage has no Dark-platform damage over time and no phase-3 hub shrink: Failover covers Dark, and phase 3 only runs the cascade faster. It also gains Single Point Of Failure, which makes the hub itself dangerous.
- The first room (the Cooling Deck) has no boss. The Reserved Instance racks work for every class; there are no class props, no forecast board and no vendor page.
- Full solo is blocked by a 75 s enrage on a lone player rather than by a mechanic that needs a second body.
- A split replica that nobody kills keeps its share of the pool: a group that ignores them fights one boss spread over several bodies until the last one falls.
- The wing's teaser door on the Cooling Deck opens onto nothing yet; Wings 5-6 and the core belong to 1.0.

- Bots read hazards off the sim where a player reads the boards, so the Hallucination is balanced for reading, not for guessing; a player who ignores the boards and the shimmer takes real damage.
- Story keeps the engine's 3 x 3 grid and three circles; the spec's smaller Story grid is not built.
- Ground Truth sits on the Comma key because every letter is bound; rebind it under Controls.
- The Brenda vendor page and the class props (robots.txt, Rate Limit, dashboard) are not built.
- Companions hold position instead of re-entering a pending hazard, so melee companions lose some uptime while the Hallucination's warning runs.

## Wing 5: The Alignment Lab (docs/ILMARI_1_0.md D.2, D.3, D5-D8)

- The Consultant's kiosks do not relocate in phase 2, and Ethics Washing's absorb does not heal him if it is left standing. The Data Scientist dashboard prop is not built, and all five Principles pillars stay up (none disappears).
- Guardrail costs are mapped onto existing aura fields (plus the new `healing_taken_pct`); the fine print is spelled out in the aura names on Story only.
- Red Team Blue Team has no shared HP bar (the boss bar only), no mirrored-name plates and no "mirrored" -20 % damage aura. Story uses one Red copy, one filler and two cameras.
- A wing-wide balance note: Red and Blue numbers were tuned with the generic bots, which wake Red at once by hitting it; a careful stealth group has an easier first minute than the gates measure.
- Wing 6 has its boss now (below); the core zone behind the lift is still an empty skeleton, and the core and the ending are D10 onwards.
- Ill Marie's desk is a prop only; Brenda's core page and the Roomba ride are deferred.
- No client effects yet for `Event::Exercise` (no standby banner, no cone decals); kiosks and honeypots are geometry only.

## Wing 6: The Boardroom in the Sky (docs/ILMARI_1_0.md D.4, D9)

- The Heir has his own HP (6,500) and no shared bar; the CIO's `Succession Plan` is a second body, not a mirrored one. Killing the CIO takes the Heir with him.
- Companions do not take part in the pair: on Story one kick stops the landing, so Rupert's "kick the first" is not built. The Prompted rotation of the action bar is a cue only on every difficulty (Full costs 10 % damage, Story nothing).
- Right-Sizing is a circle that repeats on a floor tile, not a permanent `Cleared Desk` hazard in the `HazardGrid`; it ends with the fight. Synergy Link is the `PairUp` leash without the shared damage.
- The client draws the inbox terminals, the tile grid and the apples, but nothing yet reacts to `Event::Boardroom` or `Event::PairKick`: the desks stay standing on screen (the Earnings Call decal says how many count), there is no envelope ping, severance bar or "kick now" prompt.
- A wing-wide balance note: the bots clear the inbox and the pair by reading events, so a real group's first attempts will be messier than the gates; Judgment Day numbers for the CIO (and the Heir's kick rule) are D15's.

## The Core (docs/ILMARI_1_0.md D.5, D10-D14)

- P3 is the doc's mech without a separate `il_ilmari_body` unit: the live screen is the body (the six screens stay on one bar and the client's KPI Dashboard rig stands in until D17), the Rack Arms stand beside it and the head is a unit that appears for 8 s each cycle. Not built: the `Floor Plan Revision` tile rotation (`HazardGrid`), `Open Plan`'s desks being used up in order (three fixed cover spots), the Printer Jam turret printing a shield onto the body (it only jams you), `Hot Desking`'s `Teleport`, `Reply All`'s stack going to the farthest-away player (it goes to a random one), and the Compliant offers' own rider kiosk cooldown. `Cable Management` is one cable per cast with the P1 Hydra's cable and the room's one printer. Story's mech is the same body with the bar's P4 threshold at 50 % (its "40 % of Full HP" is 40 % of P3's share of the bar, not a smaller HP pool).
- The Rack Arms are optional by design for a bot: the pact rewards killing both inside 8 s, which a human group does by evening the arms out first. The bots of the gates leave the arms alone and burn the mech down; `ilmari_core.rs` tests the pact, the 6 % and the whip itself. Story does not rebuild an arm at all (`PactDef::story_rebuild: false`), which is how this build reads "the arms do not heal each other".
- P4: the three terminals each hold two rows of the six-row log (terminal `t` is rows `t` and `t + 3`, the row text starts with its terminal), so a pick is a terminal, three choices for six commits; a wrong pick reverts the innocent row the terminal holds (never the culprit). Nobody reverts "by timing": the Merge Conflict, the consumed terminal and Force Push are as in D.5. On Judgment each member may use a terminal once (a Merge Conflict that lasts), which is this build's "a different member reads each time"; each member's wall screen showing a different row first is client work (D17). The punch cards heal the mainframe 2 % each with a 40 s cooldown per card (the doc left it open).
- `Migration Plan` is a 10 s damage-over-time aura on the mainframe, not an absorb that eats ward HP; Patch The Mainframe (the Greybeard's `utility` spell, `ilmari_only`) heals it 1,200 and dispels one Magic aura. The rewrites' "Memory safe! Zero-cost! Zero tests!" is their aggro yell.
- The Legendary Keyboards drop from the core as a named roll beside the tier piece (`legendary_pct`, one role-matched piece, not the class's own): the forge from five fragments and Brenda's core page are D16-D18, so Legendary Fragments are paid (Full 1, Judgment 2, Story none) and kept, and nothing spends them yet. The "final log" cosmetic is not granted (it is not in the cosmetics list); Story's "one piece at 50 %" is the usual raid roll.
- The remixes reuse Wing 1-3 spells and units through the encounter, so some differ from D.5: the Autoscaler opens the next screen for 20 s instead of splitting one; the Scraper Swarm is one cluster after a draft on a table at a fixed spot (no Mark Confidential); the Board is a vote on three of five policies with Pizza Friday for Story; the Cable Hydra's printer is one `il_core_printer`; the Recruiter and the Performance Review (and its HR portals) are the Wing 1 and 2 spells as they are. Judgment draws two at a mark instead of "two from a chain of three".
- P2's hazards are themed telegraph waves (a marching line, a sweep with a gap, a hug circle on every player), not the nine-tile `HazardGrid` or healing orbs; `Attached` comes from standing in overlapping circles, not from touching one orb together.
- Marie is the voice by bubble and chat line only: no recorded audio, no mask or blanket model (a paint job and a rug, chair and tissues beside her), and she is untouchable but still takes an area spell's aura (Push Update slows her).
- Companions fight whichever screen the sim tells them to by the old rules (they may hit a standby screen for nothing) and abstain from every vote; in P4 Priya heals the mainframe below 70 % before the party, Dale taunts the rewrites off it and Rupert shoots them (all three read the ward flag, nothing else is new AI), and none of them touches a terminal or a punch card.
- The wall display, the swap cursor, the round timer, the tally, the stance banner, the P3 planning tower and arm bar, the P4 clock, the mainframe bar and the log wall have no client yet (`Event::Core` and `Event::CoreLog` are sent and read by nothing but the bots; D17). The mech, the mainframe and the rewrites use existing rigs (Rack Golem, Cutover Rack, Delivery Robot, KPI Dashboard); `Event::Reassembling` has no castbar.
- `Command::StampIlmariEnd` is sent by the client on `Event::IlmariEnd` (the device clock; the server checks it like the first stamp) and kept once; the ending cutscene, the credits and the ladder row are D16 and D18, and the `ilmari_ladder` save line is D16.
- The lone-player gate on Full is the boss's soft enrage at 8 minutes of fight (`solo_after_ms`, like Wing 6), not a mechanic only a group can do; a lone Full player loses to the timer (or dies first). The Full trio of the gates is tank, damage and damage (no healer): it clears in 10 to 12 minutes with no deaths, a few minutes faster than the doc's 14.5, so the numbers in `FORMULAS.md` keep slack for Judgment.
- Judgment's Hot Take locks one member of the party to one of the first three spells on their bar (the marker aura `il_hot_take` and `Sim::hot_take_of`); the bar UI that shows it is D17. Technical Debt is the bigger rewrite waves and the one-reader rule; the five-bot Judgment clears are D15.
