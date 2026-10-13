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
| 5 | CEO | Existing encounter plus phase 2 at 40%: *Pivot to AGI* summons Chatbot Minions, Hallucination Blast cast time shortens. |

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
- [ ] Channels, mini-boss optionality, per-room elevator logic (client).

