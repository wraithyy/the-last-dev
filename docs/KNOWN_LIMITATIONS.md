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

## Dungeons (The Onboarding)

- No new engine effects were added, so a few mechanics of docs/DUNGEONS.md use the closest existing one: the Laptop Provisioner's "Out For Delivery" is a Line telegraph (no boss teleport; `Teleport` moves the target), its "Reimage" is a damage-dealt debuff instead of stripping buffs (`Dispel` only removes harmful auras), and the Forgotten Probation Intern hits a random target (`AiTarget::Caster` is the unit itself, so it cannot fixate "the caster").
- The Pop Quiz paints three of four answer lanes aimed at the target (no lettered floor tiles, which cannot follow a random aim); the Name Tags are a 5 s fuse (`Detonate`) rather than exploding on a hit.
- The Fern pet, the dungeon's department reputation and the Company Calendar "Dungeon Finder" entry are not built yet (reputation is only paid at the level cap, and the dungeon is level 3-4).
- Dungeon gear is rolled from the boss's own pieces (`source: Boss`), not as separate named drops at `legendary_pct`; the doc's named-piece roll and the tier-set `sets` field are unused here.

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
