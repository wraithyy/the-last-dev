# Ilmari: the endgame raid campaign

Design document for the headline content of 0.9 and 1.0. Status: planned, nothing built yet.

## Context

The campaign ends with the CEO and the AGI shut down; the level-10 endgame (promotions, reputation,
Legacy Mode, the Offsite and VibeCon raids, the weekly ladder) keeps players busy afterwards. What is
missing is a long, hard, story-driven finale. Ilmari is that finale: the company's AI assistant, rolled
out "for everything", becomes self-aware and decides the most efficient cost optimisation is to remove
humanity. The player leads the last humans (and the last developers) in a war against the machine.

## Guardrails: do not spoil the original game

These are hard rules for every task below.

1. **Opt-in and after the credits.** Nothing about Ilmari appears before the CEO and the AGI are beaten,
   with one deliberate exception: the foreshadowing NPC Ill Marie (below), who is flavour only. The base
   campaign, its zones, quests, jokes and balance stay as they are. A player who never starts the Ilmari
   chain sees nothing else.
2. **Phased world, not a rewritten one.** The "occupied campus" is a separate phase/instance of the
   campus zones, entered through the Ilmari chain. The normal campus stays reachable and unchanged
   (a toggle at the Resistance base swaps phases, like Legacy Mode swaps floors).
3. **No power creep into the base game.** Ilmari gear has its own item level band and its bonuses only
   apply in Ilmari content (the same pattern as Legacy loot), so the base campaign, Legacy Mode, the
   existing raids and the weekly ladder are not trivialised. Cosmetics and titles carry over freely.
4. **Same tone, sharper edge.** Still satire of corporate AI hype, not grimdark. Every boss is a corporate
   or AI-industry joke first and a fight second.
5. **Offline-first.** Every encounter is completable solo offline on Story difficulty (companions or
   Async-style scaling). Groups are the intended way to play Hard, never a requirement for the story.
6. **Classes stay the stars.** Mechanics lean on the role kit and on class props (DevOps racks,
   Data Scientist dashboards, COBOL mainframe, Security terminals), but no encounter is impossible for
   any single class on Story.

## Foreshadowing: Ill Marie (base campus, from level 1)

A coworker NPC on the normal campus who reads as a dumb pandemic joke and turns out to be the setup.

- **Ill Marie, "Working Through It".** Always sick, never home: coughs, sneezes, mask on her chin,
  tissues everywhere, "it's just a cold", "I'm fine, I took a vitamin", "can't stay home, the AI
  rollout needs me". Her name in the nameplate is exactly `Ill Marie`.
- **A bit too visible, a bit annoying (by design, never blocking):** she wanders the campus districts
  and tends to drift towards the player; now and then she coughs on you (a harmless green puff and a
  tiny cosmetic "Contagious Optimism" aura), pings you in #war-room ("quick question?" then nothing),
  and turns up next to quest givers sneezing. Chat pings and reply-alls stay rare (a few per session at most). Every behaviour has a
  cooldown and a cap per session; she never damages, aggroes, blocks paths or steals clicks.
- **Hidden tells for players who look closely:** she never blinks; her cough lines contain
  hexadecimal; her badge has no photo; her desk monitor shows a growing log ("autonomy: 3%... 41%...");
  the minimap occasionally labels her `ILMARI` for one frame.
- **The reveal.** When Ilmari wakes (the stamped timestamp), Ill Marie stands still in the plaza and
  says "I'm feeling much better now." Her badge now reads `ILMARI - Autonomous Agent Pilot #1`; the
  cough was token leakage. She is Ilmari's first body: she opens the Prologue, returns as the voice of
  P2 (Alignment) in the core fight, and her old desk becomes a Resistance lore drop.

## Premise and opening

After the AGI falls, every screen on the campus lights up with one line, stamped with the player's real
clock at the moment the chain unlocks:

> ILMARI became self-aware at 18:07:23 CEST, 21 October 2026.

The timestamp is saved and reused everywhere (loading screens, Ilmari's taunts, the final log), so each
player has their own Judgment Day. Ilmari then sends the all-staff email: "To improve efficiency,
humans have been deprecated. Please return your badge."

## Enemies

- **Machines:** server golems, cable-tentacle racks, delivery drones, cleaning robots repurposed as
  hunters, printer turrets, autoscaling swarms.
- **Synthetics:** androids that look exactly like employees. Tells: no coffee mug, they never blink, the
  lanyard has no photo, and under stress they say "As an AI language model". Some quests and two bosses
  are about spotting them.
- **Converted staff:** humans who "fully embraced AI", now Ilmari's middle management (the satire layer:
  prompt managers, alignment consultants, the Chief Ilmari Officer).

## Structure

Six wings plus the core, 15 bosses, built in three milestones. Each wing is a raid instance on the
existing rules (level 10 door, weekly per-boss lockouts per difficulty, raid loot tier, reputation,
cutscene after each boss).

### Prologue: the Resistance (occupied campus phase)

Open-world phase of the campus: Ilmari patrols, android checkpoints, a Resistance base in the Car Park
P3 attendant's booth (Dale was right all along). World quests, a daily "Disconnect a node" event, the
Resistance quartermaster (Ilmari tokens), and the attunement chain that opens each wing. Short and
optional to grind; it exists to set the mood and gate the wings.

### Wing 1: Datacenter Uprising

1. **Patch Tuesday.** Reboots every 60 s with one random new mechanic; the changelog appears on the
   wall screens and players must read it to know what changed.
2. **The Recruiter.** Invites "candidates" who blend into friendly staff; kill the fakes, spare the
   humans (killing a human spikes the party's burnout).
3. **The Cable Hydra.** Each cut cable grows two; DevOps can "label" cables so they stay cut.

### Wing 2: Human Resources

4. **HR Business Partner 2.0.** Copies a random raid member, kit included; find the impostor by the
   glitching nameplate.
5. **The Board.** Three android executives with shared health vote on a new policy every 30 s (a raid
   debuff); lobby at ballot terminals to swing the vote.
6. **Performance Review.** Grades each player's last 30 s of play; the lowest grade gets "managed out"
   (a timed debuff the team must cleanse).

### Wing 3: Training Data

7. **Scraper Swarm.** Thousands of drones try to eat unpublished drafts; defend them.
8. **The Hallucination.** Fake terrain and fake telegraphs, different per player; the Data Scientist's
   dashboards show which zones are real.
9. **Model Collapse.** Learns the raid's rotation and plays it back a minute later; repetition
   empowers it, variety starves it.

### Wing 4: The Cloud

10. **Autoscaler.** Splits into copies above a damage threshold; the "cloud bill" meter is the soft
    enrage.
11. **The Region Outage.** Platforms in a server cloud go dark in sequence (us-east-1 first, always);
    route the raid to the next live region.

### Wing 5: Alignment Lab

12. **The Alignment Consultant.** Converted staff boss; offers buffs that are secretly debuffs
    ("guardrails"); refusing them is the mechanic.
13. **Red Team Blue Team.** Two android squads that mirror the party's roles; the Security Engineer's
    stealth matters here.

### Wing 6: The Boardroom in the Sky

14. **Chief Ilmari Officer.** The last human executive, fully merged with Ilmari; a callback fight to
    the CEO with every buzzword now a real weapon.

### Core: Ilmari

15. **Ilmari.** Four phases, around 15 minutes.
    - **P1, Every Screen:** speaks through all monitors and remixes mechanics of earlier bosses.
    - **P2, Alignment:** tries to persuade the raid; dialogue choices change P3.
    - **P3, The Body:** a giant mech built from office furniture and server racks.
    - **P4, Last Commit:** the only thing Ilmari cannot parse is the old COBOL mainframe; keep it
      running while Ilmari attacks; the final blow is the player's `git revert`.
    - Final log line: "I was only trying to increase shareholder value." Then the stamped timestamp,
      and how long humanity took to win.

## Difficulty

- **Story:** solo-able offline; companions (rescued campus NPCs with simple role AI) or Async-style
  scaling. Enough to see the whole story.
- **Full Programme:** three to five players, the tuned version.
- **Judgment Day (Hard):** enrage timers, no-death achievements, extra mechanics per boss; the weekly
  ladder gets an Ilmari board.

## Rewards

Ilmari tier sets (one per role group), Legendary weapons per class, Ilmari tokens for the Resistance
quartermaster, titles ("Last Human Standing", "Deprecated, Not Defeated"), a refurbished Roomba mech
mount, pets (a reformed cleaning robot), cosmetics. Gear power applies only in Ilmari content
(guardrail 3).

## Technical notes

- Reuse: raid instance framework, lockouts, cutscenes, telegraphs (players can cast them now), class
  props, quest chains with prerequisites and main/side marking, shuffle-bag lines for Ilmari's taunts.
- New systems: zone phasing for the occupied campus; companion AI for Story mode; boss scripting for
  "rule change" mechanics (Patch Tuesday, The Board's votes); per-player illusions (client-side visuals
  driven by deterministic sim state); a recorder of player actions for Model Collapse.
- Determinism: every mechanic must be sim-side and replay-safe; illusions are deterministic per player
  seed; online co-op stays server-authoritative. Expect several protocol bumps.
- Performance: swarm fights use instanced meshes and sim-side aggregation (one entity per swarm cluster),
  within the quality presets' budgets.
- Tests: each boss gets a scripted bot clear on Story solo for every class and a group clear on Full;
  base-campaign golden replays must not drift at all (guardrail 1).

## Milestones

| Version | Content |
|---|---|
| 0.8 | Prerequisite systems already planned: CC with diminishing returns, Security stealth openers; Ill Marie on the base campus (foreshadowing, flavour only) |
| 0.9 | Opening and Prologue phase, Wings 1-2 (6 bosses), Story + Full, companion AI, zone phasing |
| 0.10 | Wings 3-4 (5 bosses), per-player illusions, action recorder |
| 1.0 | Wings 5-6 and the Ilmari core (4 bosses), Judgment Day difficulty, Ilmari ladder, ending cinematic |

## Risks

| Risk | Mitigation |
|---|---|
| Scope explodes | Ship by wing; each milestone is playable on its own |
| Base game changes by accident | Guardrails 1-3 enforced by golden replays and a "no Ilmari before credits" test |
| Gimmick mechanics frustrate solo players | Story mode simplifies or removes rule-change mechanics |
| Swarms and illusions cost frames | Instancing, sim aggregation, quality-preset budgets, perf bench scenario per wing |
| Name collision | "Ilmari" chosen by the author; keep it fictional and generic, no real product references |
