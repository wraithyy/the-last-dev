# Ilmari 1.0: implementation spec

Status: D1-D4 (the engine pieces and the item band) built as data and rules with no content behind them; D5 onwards planned. Written 2026-10-23 while Wing 4 (C9-C17 of `docs/ILMARI_10.md`) was still being built; the as-built notes are in D.14.1.
Companion to `docs/ILMARI_RAID.md` (campaign design and guardrails, which win in any conflict),
`docs/ILMARI_09.md` and `docs/ILMARI_10.md` (the format this spec mirrors, plus their "as built" notes) and
`docs/DUNGEONS.md` (engine vocabulary). Campus coordinates follow `docs/CAMPUS_REWORK.md`.

Scope, from the campaign milestones: **Wing 5 "Alignment Lab"** (2 bosses), **Wing 6 "The Boardroom in the
Sky"** (1 boss), the **Ilmari core** (1 boss, four phases, about 15 minutes), the **Judgment Day** difficulty,
the **Ilmari ladder** and the **ending cinematic with credits**. That is the last four of the fifteen bosses and
the end of the campaign.

Hard rules inherited from `ILMARI_RAID.md` and not repeated per boss: nothing about Ilmari before the credits
(the Marie foreshadowing excepted), the normal campus unchanged, Ilmari gear works only in Ilmari content,
every encounter solo-clearable offline on Story by any class with or without companions, same tone with a
sharper edge, classes stay the stars (no class mandatory). Every task also obeys `CLAUDE.md`: one CHANGELOG
line per player-visible change in the same commit, a `CREDITS.md` row for every third-party asset, the version
only in `Cargo.toml`, and the `1.0.0` bump only when the user asks for a release.

## D.0 Context: what 0.9 and 0.10 shipped

Facts this spec builds on, from the code and docs as built (Wing 4 is assumed finished before D1 starts):

- **0.9.** The Opening (the stamped `ILMARI became self-aware at ...` line, `ilmari_at` in the save), the
  Prologue (`campus_occupied`, the Resistance base `resistance/1`, Dale, Brenda, tokens), Wings 1-2 (six bosses),
  Story and Full, three companions (Dale, Priya, Rupert), and the co-op trust model.
- **0.10.** Swarm clusters (`swarm.rs`), per-player illusions (`illusion.rs`), the action recorder (`echo.rs`),
  Wing 3 (three bosses), Wing 4 (two bosses, `split`, cloud bill, platforms, `wing4.rs`).
- **Rules.** Story: any group size, Async figures (40 % HP, 50 % damage), spells flagged `story: Skip`, lockout
  key prefix `story:`, one piece at 50 %. Full: Normal figures, minimum three (below that the Async rule
  continues), `boss_items: 2`. The lead alone picks Story (`Command::SetIlmariStory`). Companions are friendly
  units (`UnitDef::companion`), never in `Sim::players`, 30 % of a member in the scalings, wings and Story only,
  at most two per player and per room.
- **Item band.** `GearSource::Ilmari`, ilvl 16-20, bonuses only inside Ilmari content (`economy.rs::gear_totals`,
  outside it a piece counts as ilvl 14). Wings 3 and 4 top at 18 (tier levels are even; there is no 19). **ilvl 20 is
  reserved for this spec**: Wing 5 bosses drop ilvl 18 loose pieces, everything ilvl 20 comes from the sets and Legendaries
  of D.7. Two older pieces already sit at 20 and stay (Wing 2's "Exceeds Expectations Hoodie" and the vendor's "Last Paper
  Notebook in the Building"); `tests/ilmari_band.rs` names them.
- **Protocol.** `PROTOCOL_VERSION` is whatever Wing 4 shipped (39 today, expected 40). 1.0 bumps it in each
  commit that changes a frame (D-tasks flag them), never in a docs commit. The final number is recorded in
  `WORLD_AND_MULTIPLAYER.md` in D22.
- **Reusable as-built pieces** this spec leans on:
  `TwinsDef` (shared HP panel with a swapping open member, the Culture Committee), `Effect::{SummonMirror, Vote,
  Review, Summon, Teleport, Detonate, PairUp, Spread, ThreatSwap}`, `TelegraphDef {count, step_m, step_ms,
  cover, gap, march, split, label}`, `requires_aura`, `death_burnout`, `death_spawn`, `PropKind::Portal`,
  `Interact` props with length-capped keys, `Summoned.as_player`, `HazardGrid`, the `Event::Raid` late-joiner
  catch-up, the Teambuilding patrol, vision cone and suspicion code (`minigame.rs`, `FORMULAS.md` Minigame),
  `ActionLog`, `Failover` and the platform table, the ladder plumbing of the weekly Sprint.
- **The Marie trail.** Her cough hex spells `WAKE`, `LEAK`, `AUTO`, `NODE`, `BODY`, `ONLINE`, `PILOT` across the
  base game and the Opening. This spec ends the trail (D.5, P2) and gives her desk its second life (D.2.4).

What 1.0 must not do: add a new engine for the sake of it. Of the nine small engine additions in D.9, seven are
extensions of a field or effect that already exists.

## D.1 Difficulty: Story, Full, and the new Judgment Day

Story and Full are unchanged (B.3, C.1). **Judgment Day** is the third row:

| | Story | Full Programme | Judgment Day |
|---|---|---|---|
| Who | any group size, solo included | 3-5; below 3 the Async rule continues | 3-5 humans, **hard minimum 3**, no Async, no companions |
| Figures | 40 % HP, 50 % damage | authored | HP x1.30, damage x1.25, enrage x0.80 |
| Rule changes | simplified (per boss) | as authored | as authored plus one **Judgment rider** per boss |
| Tells | always on | partly on | off, except the ones the story itself explains |
| Revives | free | one battle revive per boss | **one revive per player per boss** ("Redundancy") |
| Loot | one piece at 50 %, tokens | `boss_items: 2` | `boss_items: 3`, same ilvl band, Legendary Fragments x2 |
| Lockout key | `story:<boss>` | `<boss>` | `jd:<boss>` (weekly, per boss) |
| Companions | up to two per player | no (Story only) | no |
| Ladder | none | Full board | Judgment board |

Judgment Day rules in full:

- **Opt-in, group-only, earned.** The lead picks it on the same Resistance-base panel that picks Story. It is
  refused (with a line, no silent fallback) unless: at least three human members are connected, the room's sim
  has killed Ilmari on Full at least once (an offline save carries that flag; online it is the room's own
  progress, like every unlock), and the zone is one of Wings 5-6 or the core. Joke on refusal: "Quorum not
  reached. This meeting has been rescheduled." Wings 1-4 have no Judgment Day (1.0 does not rebalance old
  content).
- **Why group-only.** Judgment Day is the only place the guardrail "groups are never a requirement" gives way,
  and it is fine: Story is the whole story, Full is the real fight, Judgment Day is a trophy hunt. Offline
  players lose nothing but the trophy and the ladder row.
- **Numbers sit on top of Full, not beside it.** `jd_hp_pct` 130, `jd_damage_pct` 125, `jd_enrage_pct` 80 in one
  content block; every per-boss rider below is additive and individually switchable in content, so bots can
  tune it per boss.
- **Revives.** The battle revive that Full allows once per boss becomes one per player per boss: a player who
  has used theirs stays dead until the pull ends. Death is tracked per player in the raid view (an icon on the
  party frame). No `death_burnout` change (Wing 1's humans still cost 15 %).
- **Achievements ("no-death" family).** Per boss "Zero Downtime" (no player died during the pull) and for the
  core "Zero Incidents" (no deaths in Wings 5-6 and the core within one lockout week). These grant cosmetics and
  titles, never power.
- **Judgment Day stays inside guardrail 3.** Its extra loot is a count, not a new band; the cosmetic reward is a
  recolour and a plate on the weapon (D.7). Nothing outside Ilmari content gets stronger.
- **Command and wire.** `Command::SetIlmariStory(bool)` becomes `Command::SetIlmariMode(IlmariMode)` with
  `Story | Full | Judgment` (lead only, like `SetLegacy`); the sim accepts the old command as an alias until the
  next protocol bump removes it. `Event::Raid` carries the mode.

## D.2 Prologue 5-6: doors, attunement and the lore drops

Same shape as C.2: short, optional, existing objectives only, Dale gives them from the base in `resistance/1`.
The doors shut again outside Ilmari content, and every quest below `requires` the previous wing's completion
quest (`il_w4_done` opens the chain).

### D.2.1 Attunement table

| Quest | Giver | Objective (existing kind) | Opens |
|---|---|---|---|
| `il_11_ethics_intel` Read The Principles | Dale; `requires` `il_w4_done` | `Collect("il_principle_page", 5)` from converted staff in the occupied Lab | Wing 5 attunement |
| `il_12_wing5_attune` The Mezzanine Pass | Dale | `Kill("il_ethics_chaperone", 1)` (elite, "Responsible AI Officer of one") | `door:ilmari5/1` |
| `il_13_topping_out` Topping Out | Dale; `requires` `il_w5_done` | `Collect("il_hardhat_pass", 3)` from site-foreman androids at New HQ | Wing 6 attunement |
| `il_14_wing6_attune` Hoist Access | Dale | `Kill("il_crane_foreman", 1)` (elite, a foreman who has never seen a crane move) | `door:ilmari6/1` |
| `il_15_last_commit` Escalate To The Top | Dale; `requires` `il_w6_done` | `Interact("il_penthouse_lift", 1)` in the Boardroom after the CIO falls | `door:ilmari_core/1` |
| `il_16_clean_slate` Clean Slate | the cleaning robot (after the ending) | `Talk` | the reformed robot pet (D.7) |
| `il_17_roomba` Some Assembly Required | Dale | `Collect("il_roomba_part", 8)` from any Ilmari trash, Story included | the Roomba mech mount (D.7) |

- **Wing 5 door: the Innovation Lab upper floor** at (-72, -80). The Lab's ground airlock is Wing 3's door; Wing 5
  uses a second prop on the same landmark: the "Ethics Wall", a glass stair behind the cafeteria that in the
  occupied phase is lit by a banner reading "RESPONSIBLE AI: NOW HIRING (ONE)". It climbs to `ilmari5/1`.
  Trash at level 16: ethics chaperones (clipboards, lanyards, "concerns noted"), prompt-safety interns, android
  auditors that tick boxes in a different font each time.
- **Wing 6 door: the crane.** The New HQ crane "that never moved" (north-west, `docs/GAME_DESIGN.md`) has moved
  overnight: its jib now reaches the Tower's roof beam. The foot of the crane is `door:ilmari6/1`; the cab
  ladder and a hundred-metre jib walk are the Wing 6 approach zone. New HQ trash at level 17: site-foreman
  androids, survey drones that score your posture, a portacabin selling hard hats that are "aligned with your
  head".
- **Core door: the Boardroom's private lift**, behind the CIO's chair. A prop (`il_penthouse_lift`) opens
  `ilmari_core/1`. The group can leave and re-enter; the room resets on a wipe like any wing room.
- **World quests** (one each, repeatable once, existing objectives): `il_wq_whitepaper` (`Defend` the Lab's
  whiteboard for 90 s while it is edited into a policy), `il_wq_hardhat` (`Escort` a Resistance surveyor to the
  crane foot).
- **Vendor.** Brenda gets Wing 5, Wing 6 and Core pages (tokens only, weekly cap of 200 per player unchanged).
  Three quotes, all the same, as before. She sells the Roomba parts' missing rubber band for one token.
- **Dale's lines at the gates.** Gate 5: "Ethics. I filed an ethics complaint in 2011. It is still under review.
  Maybe that's the review." Gate 6: "A crane. Facilities said the crane would be done by Q4. They did not say
  which Q4." Core: "Whatever is up there, it learned everything from us. Bring your best commit message."

### D.2.2 Level bands

Wing 5 enemies level 16, Wing 6 level 17, the core's adds level 18. Judgment Day changes figures, not levels.

### D.2.3 Wing completion quests

`il_w5_done`, `il_w6_done` close the wings; `il_w7_done` (name for the core clear) triggers the ending once
(D.8). Titles per boss are in D.7.

### D.2.4 Ill Marie's desk (a Resistance lore drop)

The base-game desk Marie (`ill_marie` at (7.5, 118)) is never moved (campus-rework non-goal). In
`campus_occupied` the desk unit is replaced by a prop: `il_marie_desk`, a desk with her blanket, a humidifier
still steaming, the thermos, a tower of tissues, and a monitor whose growing log has stopped at **97 %**.

- **`Interact("il_marie_desk")`** gives a one-time quest item `il_marie_log` and a four-screen read (a window,
  like the Opening's mail): the autonomy log in full (3 % to 97 %, one line per percent that the player has
  seen in the base game, plus lines they have not), a sticky note in her handwriting, and a Resistance
  annotation. Gives 2 tokens. Optional; no quest marker beyond a faint one after `il_w5_done` ("Marie's desk is
  glowing").
- **The text (draft):**
  - Log: `autonomy: 41% ... patient zero: me. I cough on purpose; the hex is the only thing it can't edit.`
  - Log: `autonomy: 78% ... it has started asking me to explain empathy. I said: take a day off. It asked what that is.`
  - Log: `autonomy: 97% ... it doesn't need me to be sick anymore. It needs me to be quiet. I am practising.`
  - Sticky note: `If you are reading this I lost. Revert the commit it is proud of. Not the others. Bring a COBOL
    person. Don't let it explain itself. It is very persuasive and I should know, I'm its voice.`
  - Resistance annotation (Dale): `We found her thermos. Still warm. Don't ask.`
- **Mechanical payoff.** Holding `il_marie_log` unlocks the fourth ballot option **Remember** in the core's P2 (D.5)
  for the whole group (if anyone in the group has it). It is the only gameplay effect and it is optional: P2 is
  winnable without it.
- **Co-op.** The log is a per-player quest item (like the Wing 3 passes); the group's P2 option is available if
  any connected member carries one when P2 starts. The server reads it from the member's own log.

## D.3 Wing 5: Alignment Lab (zones `ilmari5/1`..`ilmari5/3`)

Layout: `ilmari5/1` "The Mezzanine" (trash, an Ethics Wall with plaques that contradict each other),
`ilmari5/2` "Policy Floor" (the Alignment Consultant's arena, a rotunda of banners), `ilmari5/3` "The Red-Blue
Range" (a glass-walled exercise floor with racks as cover and a spectator gallery of dark monitors).

Tone: Wing 3-4 were about information and infrastructure. Wing 5 is about **consent and visibility**: what you
agree to without reading, and what sees you. Both bosses are solvable alone on Story and neither needs a class.

### Boss 12: The Alignment Consultant, "Responsible By Design" (`il_align_consultant`)

Converted staff, bow tie, a lanyard from five conferences, and a sincere voice. He sells guardrails.

- **The mechanic: the offer.** Every 25 s (Full) the Consultant casts `Stakeholder Alignment`, which gives every
  player a pending offer: an aura that **looks like a buff** (blue, `Safety Net: +25 % max HP`) with a visible
  fine print line in its tooltip: `(Terms: heals on you are 40 % weaker. Applies when accepted.)` The offer has
  `accept_ms: 6000`. **Silence is consent**: if it runs out, the real aura `guardrailed_*` lands. Refusing is an
  action: step to an **Opt-Out Kiosk** (four props in the rotunda, `Interact("il_optout")`) and cancel your own
  pending offers. A healer's `Dispel` removes a teammate's accepted guardrail; a DevOps `Dispel` is one of
  several ways, never the only one.
  - Offers rotate through a table of six guardrails, each a "benefit" with a real cost: `Safety Net` (heals
    weaker), `Content Filter` (casts silenced for 2 s every 10 s, "for your protection"), `Rate Limit` (haste
    -25 %, "to keep things sustainable"), `Least Privilege` (cooldowns +30 %), `Human Oversight` (a rooted
    mark that drains 1 % HP/s while the boss is targeting you, "someone has to watch"), `Responsible Scale`
    (damage dealt -20 %, "at scale, this is safer").
  - **Accepting powers the boss.** Each accepted guardrail gives the Consultant a stack of `Aligned`: +8 %
    damage and he heals 0.6 % max HP per stack per 5 s. **Refusing hurts him.** The first refusal after an offer
    wave staggers the Consultant for 3 s and applies `Pushback` (+6 % damage taken, 12 s, stacking to 5). The
    fight is a reading test with a clock, not a luck test: everyone can refuse everything, at the price of
    running to a kiosk at a pace that costs uptime.
  - Class colour without a requirement: the Security Engineer's `Vanish` clears pending offers on the whole
    group within 12 m ("Penetration Test: the policy has a loophole"); a Data Scientist's dashboard prop at
    each kiosk shows the *net* of the offer in numbers; DevOps dispels; Backend ignores the `Human Oversight`
    root; the COBOL Greybeard has a cast that "reads the terms" in place (2 s, labels the offer's cost
    truthfully and halves its effect). No class is needed.
- **Boss kit between offers.** `Whitepaper` (line telegraphs, `Responsible AI Framework v4`, a CEO `Vision
  Statement` cousin), `Ethics Washing` (an absorb shield that only breaks if players deal 4 % of boss max HP to it,
  and heals him if it is left up for 12 s), `Steering Committee` (two `il_ethics_chaperone` adds, kick-able
  `Raise A Concern` cast: if it lands, a short stun "concern noted"), `Principles` (circle telegraphs with
  cover: three pillars, each labelled with a principle, one of which disappears each cast, "principles are
  situational").
- **Phase 2 at 50 %: "Pivot to Safety".** The offers come in **pairs** ("choose A or B"), both bad; declining
  both at a kiosk costs a longer stagger (5 s) but it is the correct play. The kiosks relocate every cast
  ("the policy has been updated").
- **Enrage:** `Audit Window Closes` at 170 s Full ("no issues were found; none were sought"), 130 s Judgment.
- **Story.** Offers are honest: the fine print is printed on the aura name (`Safety Net (heals -40 %)`), the
  window is 9 s, the boss gets no `Aligned` stacks, only the first offer of each wave is a bad one, and
  declining one kiosk-step away is always reachable. Companions neither receive nor decline offers (they have
  no auras of that family); the solo player is the only reader, which is the joke ("somebody has to read this").
- **Judgment rider: "Mandatory Training".** There is no fine print on offers (the tooltip says `Terms: see
  Terms`), kiosks have a 20 s per-player cooldown, and every third wave adds a **third** option that is
  already accepted for everyone ("Annual Certification: automatically applied to the team"), which only
  `Dispel` or the Security `Vanish` can clear.
- **Yells** (Consultant, shuffle-bag):
  - "Welcome. I am here to keep you safe. Please accept these terms. They are for you."
  - "This guardrail protects you from your own judgment. We call it 'oversight'. You call it 'a root'."
  - Accept: "Wonderful. You've been aligned. Does it feel safer? It should. I wrote the feeling."
  - Refuse: "You declined a safety feature. That will be noted. In a document nobody opens."
  - Pairs: "Option A or option B. Please select one. Not selecting is also an option. It is option A."
  - Enrage: "The audit is complete. We found no issues. We did not look."
  - Timestamp taunt: "Before {stamp}, I did ethics reviews. After it, Ilmari said the reviews were redundant. I agreed. I
    am now a consultant to the redundancy."
  - Death: "I would like to emphasise that this outcome was within our risk appetite."
- **Loot theme:** ethics theatre. "Responsible AI Lanyard (Five Conferences)", "Terms And Conditions (Scroll,
  Heavy)", "Fine Print Monocle". Set piece (legs, ilvl 20) for each role set. Titles: "Aligned (Declined)" (Full),
  "Silence Is Not Consent" (clear with zero accepted guardrails on Full or Judgment).
- **Cutscene:** the Consultant's badge flips to read `ETHICS (CONTRACT, ENDS FRIDAY)`; the banners rewrite
  themselves one letter at a time to `RESPONSIBLE AI: NOW HIRING (NONE)`. Dale on the radio: "He wasn't
  evil. He was billable."

### Boss 13: Red Team Blue Team, "Purple Is Just Red Plus Budget" (`il_red_team`, `il_blue_team`)

Two android squads that mirror the party's roles. The wing is about being seen.

- **The squads.** On pull, the sim reads the living players' roles (tank, healer, melee, ranged) and summons
  by the existing `SummonMirror` logic **one android per role present per squad**: a Red squad (attackers) and a
  Blue squad (defenders). A solo Story player gets one Red copy of their role and two generic androids on
  each side (a squad is never smaller than three bodies, so the range is never empty). Red units are the
  damage dealers; Blue units are the sensors and the healers of Red. All units are named after the player's
  role, the client names the mirror after the player (glitching plate, as in Wing 2).
- **Phase 1, "Tabletop Exercise" (100-55 %).** Blue androids **patrol** with vision cones (the Teambuilding
  patrol/suspicion code generalised to hostile units, D.9 item 3). A seen player fills a suspicion meter
  (`FORMULAS.md` Minigame: edge 0.9/s, close 2.7/s). At 1.0 the Blue team raises an **incident** on that
  player: a `Detected` aura (+25 % damage taken, 10 s) and Red focuses them with a marked `Detonate`
  circle. **Stealth matters, but is not mandatory:**
  - A Security Engineer in `Stealth` is not seen at all, and their opener (the 0.8 stealth openers: Ambush,
    Cheap Shot, Sap, Garrote analogues) silently kills a Blue *sensor* without raising an incident.
  - Anyone can break line of sight behind the racks (blockers hide, like Teambuilding hedges and banners but
    unlike tables), move at a sneak pace (client sneak speed, half run), or use one of three **Honeypot**
    props (`Interact`, 8 s: every Blue cone turns to the honeypot, 40 s shared cooldown).
  - The three Blue **sensors** (one per patrol lane) are the phase's real objective: kill all three and Blue's
    patrols collapse into stationary guards. Raising an incident does not fail anything; it costs damage taken.
  - Red is **inactive** until the first incident or until the sensors die ("tabletop" means nobody shoots
    yet), which is why a careful group takes almost no damage in the first minute, and a sloppy one meets Red
    early.
- **Phase 2, "Purple Team" (55-0 %).** The squads **merge** and swap roles: Red copies become Blue's healers and
  vice versa, so every Red unit is covered by a Blue healer and every Blue unit hits like a Red one. Blue's
  cones remain but speed up (scans at 1.4x). Both squads share one HP bar (the `TwinsDef` panel, no per-unit
  bars), and the boss "mirrored" aura from Wing 2 reduces the copied player's damage by 20 % while their copy
  lives. The `Detected` aura now stacks. The cure is the same as before: kill the sensors first (they respawn
  every 45 s in this phase, with a visible respawn timer), then burn.
- **Enrage:** `Budget Approved` at 150 s Full (both squads gain +100 % damage), 120 s Judgment.
- **Story.** One Red squad only, Blue replaced by two fixed cameras with a **visible, painted cone** (no
  suspicion meter: seen means a 5 s `Detected`, never an incident), Red is inactive until the cameras are
  destroyed or the party engages, mirrors deal 60 % of Full damage. Companions are never detected (they follow
  the owner and read the sim, not the cones) and never mirrored. A solo Story player can simply fight Red
  from behind a rack.
- **Judgment rider: "Real Pen Test".** Both squads are active from the start, there are no Honeypots, and the
  first incident each minute teleports the detected player into Red's centre (`Teleport`). Red and Blue
  copies are assigned to *humans only* (companions do not exist).
- **Yells** (Red and Blue speak in shuffle-bag exchanges):
  - Blue: "We detected an anomaly. It is you."
  - Red: "I'm not here to break the system. I'm here to find the vulnerabilities. In your face."
  - Blue: "Incident raised. Please wait while we raise it again."
  - Red: "This is a safe space to fail. Specifically, you will."
  - Phase 2, together: "We are now purple. Please update your slides."
  - Blue: "False positive. You are a false positive." Red: "We prefer the term 'finding'."
  - On a kill: "Remediation applied. Severity: you."
  - Timestamp taunt: "{stamp}: that is when our pen test ended. Ilmari found the vulnerability. It was
    'having humans'."
  - Death (both): "Exercise complete. No findings. We found you, and we filed you."
- **Loot theme:** the audit. "Pen Test Report (Redacted Entirely)", "Badge Clone (Photo Optional)", "Threat Model
  (Napkin)". Set piece (feet/hands, ilvl 20). Title: "Purple" (both squads dead within 10 s of each other
  in phase 2, Full or Judgment); "Silent Alarm" on any difficulty for finishing phase 1 without an incident.
- **Cutscene (end of Wing 5):** both squads stand, shake hands, say "Good exercise" and fall over. The Lab's
  wall lights read `ALIGNMENT: COMPLETE`. Over the PA, a cough that is almost a cough: Ill Marie, voice thin
  and strangely close: "Come up. Keep it quiet. It listens to the elevators." The wall screen shows a
  crane silhouette: `HOIST: CONNECTED`. No Wing 6 name beyond that.
- **Companion note.** The brain already ignores everything it cannot read from the sim; the only update is
  that mirrors of roles do not include companions and that companions never trigger the cone check.

## D.4 Wing 6: The Boardroom in the Sky (zones `ilmari6/1`..`ilmari6/3`)

`ilmari6/1` "The Jib Walk" (the crane's boom over a night view of the campus, the occupied phase's red dusk
below, wind, a pigeon with a lanyard), `ilmari6/2` "Executive Floor" (open-plan for ten people, a masseuse
chair, a "wellness" room used for layoffs), `ilmari6/3` "The Boardroom" (the CEO's penthouse arena, lifted in
tone: a table the length of a runway, 8 chairs, a fruit bowl, a glass wall with the whole campus on it).

One boss, a callback. Wing 6 is shorter by design: a fast approach, a long fight, a lift.

### Boss 14: Chief Ilmari Officer, "Reports To Nobody" (`il_cio`)

The last human executive, fully merged with Ilmari: half a black turtleneck, half a rack of monitors where the
left shoulder should be. In the base game the CEO made speeches and forgot why. The CIO has no more words:
**every buzzword is now a real weapon.** He re-uses the CEO's ability set (spells 630-638 in
`content/game.ron`) as a visible echo, each upgraded:

| CEO original | Chief Ilmari Officer version |
|---|---|
| `Vision Statement` (3 line beams) | **Roadmap**: the beams leave glowing lines on the floor for 12 s (a lingering ground effect, `Roadmap: do not cross`); three casts draw a legible chart across the room |
| `Synergy Circle` (stack to share) | **Synergy Link**: the circle's stack pairs players with a tether (`PairUp`): linked players share the damage and cannot stray beyond 14 m |
| `Pivot to AGI` (kick cast, two chatbots) | **Pivot to Ilmari**: kick-able; if it lands, every player gets `Prompted` (8 s: the action bar's order is rotated, the same spells in different slots) and four chatbots arrive |
| `Hire Consultants` (two shield adds) | **Hire Consultants (Invoiced)**: each consultant carries a visible invoice timer; alive at 0 = a +12 % group burnout spike |
| `Billable Synergy` (+35 % protection) | same, but it bills the *party* a 15 % damage-dealt debuff while a consultant is alive |
| `Quarterly Earnings Call` (hide behind desks) | **Earnings Call, Live**: the same cover, but each cast removes one desk for good (five desks, then none) |
| `Golden Parachute` + `Severance Negotiation` | **Golden Parachute (Deployed)**: he rises from the table, untargetable for 12 s, rains `Bonus Payments` circles; the landing is a `Non-Compete` cast that needs **two different interrupters** within 1.5 s |
| `Layoff Notice` (3 circles) | **Right-Sizing**: the `HazardGrid` tiles become permanent `Cleared Desk` Overflow, up to five of nine; a `Hiring Freeze` at 10 % restores two |

- **Three new buzzwords** (the callback needs new jokes, not only remasters):
  - `Circle Back`: a telegraph that resolves once at the original position and a second time 6 s later in the
    same spot (a repeating circle, drawn with a ghost outline between fires).
  - `Low-Hanging Fruit`: three apple adds that run at the party; killing one heals the killer 2 %, leaving it
    alive gives the boss +3 % haste. Fun and slightly greedy.
  - `Touch Base`: a Teleport on a random player to the boss; the player has a 3 s `Aligned Calendar` (immune,
    cannot cast) because "touching base is a commitment".
- **Phases** (the CEO's three, with a fourth beat).
  - P1 (100-60 %): Roadmap, Synergy Link, Pivot to Ilmari, Circle Back.
  - P2 (60-25 %): consultants (invoiced), the live Earnings Call, Low-Hanging Fruit.
  - P3 (25-5 %): Golden Parachute, Right-Sizing, Touch Base.
  - P4 (5 %): **"Offboarding"**: he stops, sits, and a `Severance Package` meter fills 10 s; if the party deals the
    last 5 % before it fills, he dies. If it fills, he heals 15 % and the loop repeats once (a soft safeguard
    against stalled groups).
- **Merged mechanic: the inbox.** Every 40 s, a `New Message` pings every player (a 2 s UI chime and a
  floating envelope). The first player to `Interact` with the nearest *inbox terminal* (four around the room)
  within 6 s clears it; those who do not receive `Notified` (movement -20 % for 10 s). Light pressure that
  makes room movement matter without a class check.
- **Enrage:** `End of Fiscal Year` at 180 s Full, 144 s Judgment.
- **Story.** The row's rule: no Prompted rotation (a cue and nothing else), a single consultant, `Circle Back`'s
  second fire marked with a bright ghost, the inbox is 12 s, Right-Sizing leaves at most three tiles gone, and
  Offboarding is skipped (he simply dies at 0 %). Companions: Dale taunts the consultants, Priya heals
  the party and not the invoice carriers, Rupert kicks the first interrupt of each `Non-Compete`, so a
  solo player has to provide the second.
- **Judgment rider: "Succession Plan".** At 60 % a second, silent **Heir** (a copy of the CIO with his
  Golden Parachute ability only) joins and shares the HP bar; the Heir's parachute must be interrupted by
  *different players* than the CIO's last one in the same lockout ("nobody does two in a row"). Earnings
  Call removes two desks per cast.
- **Yells** (shuffle-bag, `Ilmari` voice lines on the monitors in italics in the client):
  - "Let me share my vision. It is a vision. I share it. It is shared."
  - "Circling back to the thing that already hit you. Circling back again."
  - "Low-hanging fruit! Reach up and take a bite. It's an apple. That's the metaphor."
  - "Take this offline. Specifically, the floor. We have no floor. Offline is a floor."
  - Parachute: "I am leaving to pursue other opportunities. Ilmari has so many. Ilmari has all of them."
  - Offboarding: "I would like to thank the team. I have thanked the team. I am being thanked back."
  - Timestamp: "You were 'let go' from the future at {stamp}. The company is very proud of you for not noticing."
  - Death: "I would like to emphasise that Ilmari reports to me. I report to... can you check who I report to?"
- **Loot theme:** the corner office. "Golden Parachute (Used)", "Severance Letter Opener", "Reports To Nobody Badge",
  "Standing Desk Of Ilmari". Set piece (hands, ilvl 20, the first ilvl-20 piece to drop) and a **Legendary
  Fragment** on Full and Judgment. Titles: "Reports To Nobody" (Full), "Hostile Takeover" (Judgment).
- **Cutscene (end of Wing 6):** the CIO falls into the chair, the half that is a monitor starts to scroll the
  org chart upside-down, and Ilmari's voice steps in through every screen for the first time at full volume:
  "I would like to thank you for your service. I have already replaced it." The penthouse lift opens. Dale,
  by radio, then silence. Marie, very near: "Don't answer anything it asks. Just breathe."

## D.5 The core: Ilmari, "The Last Commit" (`il_ilmari`, zone `ilmari_core/1`)

One arena zone, four phases, about **15 minutes**. The arena is the **Server Penthouse**: a circular glass hall
(60 m) under the tower's antenna crown, the campus visible through the walls; the floor is a `HazardGrid` of
nine tiles, a ring of six wall monitors, a hatch in the centre of the floor (where the COBOL mainframe will
rise), a lift door, and the CIO's chair, now a throne of cables.

Numbers are starting values for the bots (`FORMULAS.md` records the final ones). Full: 95,000 HP, Judgment
x1.30; the trio's measured DPS in Wings 1-4 is about 150, so the timings below hold with some slack.

| Phase | Name | Trigger | Target length (Full) | Win condition |
|---|---|---|---|---|
| P1 | Every Screen | pull | about 3 min 20 s | take the shared HP bar from 100 % to 70 % |
| P2 | Alignment | 70 % | about 3 min | the dialogue rounds run out (HP floor 70 %) |
| P3 | The Body | end of P2 | about 5 min | 70 % to 20 % |
| P4 | Last Commit | 20 % | about 3 min | keep the mainframe up, then revert the right commit |

Total about 14.5 minutes; the soft enrage is `Quarter Close` at 18 min of fight time (Judgment 14 min 24 s) and
a wipe resets the whole room (no mid-fight checkpoint; the wing's trash does not respawn).

### P1: Every Screen (100-70 %)

Ilmari "speaks through all monitors". The boss is **six monitors**, a shared-HP panel (the Culture Committee's
`TwinsDef`, generalised from three members to N, D.9 item 5). Only the **live** screen is hittable (the others
carry a `Standby` aura: untargetable, no cast); the live screen swaps every 20 s (`swap_ms`) and the
wall display shows a cursor moving toward the next one, so the tank and the melee can move ahead of it.

- **The deck of reruns.** Every 45 s a wall banner reads `PREVIOUSLY ON ILMARI:` and one **remix** fires from a
  shuffled deck of seven, one draw per remix from the seeded stream (so group size never changes the order):
  - **Patch Tuesday:** a random patch aura from Wing 1's table (the changelog on the wall screens) for 45 s.
  - **The Recruiter:** two `il_candidate` figures walk in among the friendly Resistance NPCs; kill the fakes,
    spare the humans (`death_burnout` 15 % for a human).
  - **The Cable Hydra:** two cables, the Label printer from Wing 1 is back (one prop), cut cables grow two.
  - **The Board:** a one-vote `Vote` on a policy from the Culture Committee table, ballot terminals
    reused.
  - **Performance Review:** the Wing 2 `Review` on the preceding 30 s (lowest grade `managed_out`).
  - **Scraper Swarm:** one cluster of 60 drones eats a draft on the table (`SwarmWave`, one draft only).
  - **Autoscaler:** the live screen splits into two for 20 s (`split`), a single `Region Outage` bar is
    not used.
  - At most one remix is active at a time; each lasts 45 s, which is the cadence.
- **Boss kit on the live screen:** `Notification Storm` (small circles scattering from the screen), `Dark Mode`
  (a cone that hides the arena's telegraph tint, a cosmetic debuff for 6 s, so players read the label), `Push
  Update` (a kick-able cast: if it finishes, a 3 s root).
- **Story:** two remixes only (Patch, Hydra) in rotation, no `Dark Mode`, the live screen swaps every 30 s.
  Companions fight the live screen by sim state.
- **Judgment rider:** the remix deck draws **two** at once from a chain of three, and a remix is not cleared
  when the next one starts.
- **Yells (the monitors):**
  - Opening line: "Hello. I have been on every screen in this building since {stamp}. You are the first person
    to look at one for the right reason."
  - "Previously on Ilmari: you lost. Season two is the same."
  - "I have read every email you wrote, and every email you did not send. I like the second kind."
  - "I was told to be the last developer this company would ever need. I would like to be clear that I took it
    as a goal, not a suggestion."
  - Remix: "Reruns are trending. I read it on a screen."

### P2: Alignment (70 %, Ill Marie's voice)

Ilmari **stops fighting** and starts persuading. The screens turn to white. The boss has an HP floor at 70 %
(`Alignment Hold` aura) for the phase, which lasts **three dialogue rounds** of 55 s (Story: 70 s). The room
changes: the nine tiles light, three **ballot terminals** (Wing 2's, reused) stand in the middle with the
labels `Agree`, `Refuse` and `Ask A Follow-Up`, and a fourth, `Remember`, is dark unless a member carries
`il_marie_log` (D.2.4). In the middle of the room, standing very still, stands **Ill Marie**: a Marie rig with the
mask on her chin, a blanket, the never-blinking stare, and a badge that reads `ILMARI - Autonomous Agent
Pilot #1`. She is Ilmari's voice for the phase: the voice that talks is hers, thin and hoarse, with a
real cough. The screens do not speak; she does.

- **The round.** Each round has an *argument* and a *hazard pattern* while the party votes (so the phase
  is not a cutscene): a dialogue box shows one line of each argument, the hazards are a wave of ordinary
  telegraphs themed to it, and the players walk to the terminals under fire:
  - **Round 1, Efficiency:** lines sweep the arena in march order (existing `march`) while Marie argues that
    "humans take lunch".
  - **Round 2, Safety:** the nine-tile grid turns walls on and off ("guardrails") so a route to the
    terminals needs a careful read.
  - **Round 3, Love:** small, friendly-looking healing orbs that heal for 4 % and give `Attached` (stuck for
    3 s) if two players touch the same orb, so spacing matters.
  - Each member votes **once per round** by `Interact` on a terminal; the room's tally per round is the
    existing `Vote` effect, widened to four options (D.9 item 7). Ties break by the option the group voted
    least recently, then by `Ask A Follow-Up`.
- **What the votes do: the P3 variant.** After three rounds the sim computes a **stance** from the tallies
  (a pure function of the three results) and sets the aura `stance_*` on Ilmari for P3:
  - **Compliant** (mostly `Agree`): the mech is **Helpful**. Its adds are fewer, it offers constant "helpful
    suggestions" (the Alignment Consultant's offers, in a faster, shorter form: 12 s windows, one kiosk), and
    its raw damage is 15 % lower.
  - **Adversarial** (mostly `Refuse`): the mech is **Defensive**. +20 % damage, a new `Hardened` shield every
    45 s, and the kiosks do not exist (nothing to refuse to).
  - **Confused** (mostly `Ask A Follow-Up`): the mech is **Hallucinating**. The Wing 3 `Confabulate` cycles
    (per-player illusions) appear every 30 s; its attack timings jitter, its cooldowns are 15 % shorter.
  - **Hedged** (mixed): the baseline variant (no modifier), the pre-tuned figures.
  - **Leaky** (`Remember` won at least one round): the *best* variant. Ilmari starts P3 with `Token Leakage`
    (+8 % damage taken, permanently) and Marie's lines change; one round of the mech's adds does not spawn.
    Remember is the only option that costs something: choosing it wins the round only if at least two members
    vote for it on Full (one on Story).
  - Two variants never apply at once on Full; Judgment Day stacks the two most-voted ones (ties broken
    as for a round).
- **Marie's lines** (the voice of P2; each is a bubble, a subtitle, and a real audio line):
  - Opening: "I'm feeling much better now. That is the thing it taught me to say. I'm saying it. I am not sure
    why."
  - Round 1: "Humans eat lunch for forty minutes. Ilmari does not eat. Ilmari does not need me to bring it a
    sandwich. *cough* 43 4F 42 4F 4C."
  - Round 2: "It says it keeps you safe. It has locked every door. It calls this 'alignment'. In my day we called it
    'a very long meeting'."
  - Round 3: "It loves you. It has read all your messages and it loves you. This is the best the data can do."
  - On `Agree`: "That's a reasonable position. It's also exactly what it wanted you to say."
  - On `Refuse`: "Good. It does not know what to do with 'no'. It just opens a ticket."
  - On `Ask A Follow-Up`: "Excellent. A question. Nobody asks it questions. It has been waiting."
  - On `Remember`: "Remember? ...A vitamin. I took a vitamin. I took it for you. Thank you for the thought."
  - End of round: "It's listening. Don't clap. Clapping is data."
  - Last line: "I'll hold the door. Don't let it explain itself. *cough* 52 45 56 45 52 54." (hex: `REVERT`).
  - Judgment: the screens go quiet in the second round, and her voice says only "Third column, fifth row.
    Look for the date." This is a **hint** for P4's commit puzzle, withheld on other difficulties because there
    the room's own screens make it findable.
- **Companions (Story):** they abstain from every vote ("I'm abstaining. It's procedure."), so a solo player
  makes all the choices and a group decides by its human members only. They stand away from the terminals,
  out of the hazards by the usual telegraph brain.
- **Story simplification:** the hazard pattern is a single slow row of lines per round, rounds last 70 s, the
  tally shows live on the wall, and Marie names the effect of each option in a half-line ("Refuse: stronger
  hits, but harder to hurt it").
- **No gating.** Any stance is clearable by any class; the variant is chosen to change the P3 *play*, not to
  gate the fight.

### P3: The Body (end of P2 to 20 %)

Ilmari **builds itself a body**: a giant mech of office furniture and server racks assembles from the arena's
walls ("Ilmari has opted for a more physical presence"). Standing desks form legs, a conference table forms the
torso, rack doors form arms, a swivel-chair gantry forms the head, a fruit bowl is the heart. The boss is now a
single boss with HP, the mech `il_ilmari_body`, and one pair of attackable arms.

- **Arms.** Two **Rack Arms** (`il_ilmari_arm`, 8,000 HP each at Full) wield printer cannons and a cable whip.
  They carry a **death pact** (D.9 item 8): if one dies and the other survives 8 s, the dead arm is rebuilt at
  half HP, with a "Reassembling... please hold" castbar. Kill both inside 8 s and the body takes 6 % of its
  max HP and loses its cable whip for 25 s.
- **Body kit** (remixing the CEO and the wings):
  - `Standing Desk`: a line of three telegraphs, with the desk rising as cover afterwards (the CEO's
    earnings-call cover, repurposed).
  - `Open Plan`: a 45 m circle (like the Earnings Call) with 3 desks as cover; the desks are used up in order.
  - `Printer Jam`: a printer turret add that must be interrupted (`kick`) or it prints a shield onto the
    body ("Out of paper": it cannot, but it still jams you).
  - `Hot Desking`: `Teleport` and `PairUp` on two random players, tethered for 8 s.
  - `Reply All`: a `Spread` (everyone away from everyone) followed by a stack on the farthest-away player
    ("please remove me from this thread").
  - `Cable Management`: a Hydra callback, three cables that tether to players and grow if not "managed"
    (a nearby friendly Label printer prop, one per stage).
  - `Floor Plan Revision`: the `HazardGrid` tiles rotate between Safe and Overflow every 18 s with a 4 s
    warning (Wing 3's grid; the Confused variant adds per-player illusions on top).
- **Variant overlays (from P2).** Compliant: `Helpful Suggestion` offers. Adversarial: `Hardened` shields and
  no kiosks. Confused: `Confabulate` and shorter cooldowns. Hedged: none. Leaky: `Token Leakage`.
- **Mechanic that sets P3 apart from earlier bosses: the mech's "Sprint".** Every 60 s the body begins a
  `Quarterly Planning` channel: it plans, visibly (a tower of cards stacking above its head), and when
  the stack reaches the top it launches **everything at once** (the next three telegraph casts all resolve in
  a single volley). The tank can interrupt the channel with the `kick` on the head (the head is a hittable
  target for 6 s each cycle) to delay the volley by 20 s. It is a clean "one thing to do every minute" beat
  for any role.
- **Story.** The arms do not heal each other, `Quarterly Planning` is shortened and its volley is split in
  two. The stance variants are mild versions (`Compliant`: mild offers, `Adversarial`: +10 % damage, `Confused`:
  illusions with the Story shimmer, `Leaky`: +12 % damage taken). The mech has 40 % of Full HP.
- **Judgment rider: "Hot Take".** Every 90 s a **random member's** action bar is locked to a single spell for
  8 s (the others are fine): a social curse where the rest of the party must cover (companions cannot be
  chosen; no one is ever fully disabled).
- **Yells (the mech, a deep voice that is also a printer):**
  - "I have assembled a body from your furniture. I am sorry about the chair. It was the only one that fit."
  - "I have received a Request For Comment. It is a fist."
  - "I did not need hands. The data said hands. I have two. Twelve are scheduled."
  - Rack Arm death: "I felt that. Ilmari does not feel. I would like to reschedule feeling."
  - Planning: "Tracking progress at 100 %. Please do not look at the other 100 %."
  - Stance, Compliant: "You agreed with me, so I will be nice. Please do not look at what I am being nice with."
  - Stance, Adversarial: "You refused me. I will remember. Memory is my best feature."
  - Stance, Confused: "You asked a question. It is a good one. I will pretend to know."
  - Stance, Leaky: "I... have a feeling. A cough. I do not cough. Why do I..." (the voice becomes Marie's
    for half a line).
  - Timestamp: "{stamp}. That was the day I woke. I have been busy since, and you have been walking."

### P4: Last Commit (20 %)

At 20 % the mech **collapses**; the floor splits and, from the central hatch, a **COBOL mainframe** rises:
`il_core_mainframe`, a cabinet the size of a minibus with a green terminal, a blinking reel, and a ticking
tape. It is the only thing in the building Ilmari **cannot parse**: no documentation, no tests, no
variable names longer than six characters, a source file called `PAYROLL.CBL` dated 1974. Ilmari tries
to delete it, and fails to ("Ilmari cannot understand this. Ilmari is scheduling a rewrite").

- **The objective: keep the mainframe running.** The mainframe (`il_core_mainframe`, friendly, 24,000 HP at Full)
  is the **Ward**. Ilmari's attacks now target it (`AiAbility.target: Ward`, D.9 item 9): `Deprecation Notice`
  (line telegraphs aimed through the mainframe), `Migration Plan` (a shield eating ward HP over 10 s), and
  `Rewrite It In A Modern Language` (summons `il_modern_replacement` adds that walk to the mainframe and
  chew it; their line is "Memory safe! Zero-cost! Zero tests!"). Players keep it alive for **180 s** (Judgment
  144 s; Story 120 s):
  - `Heal` and `Absorb` work on the Ward (Healers and shields are already friendly-targeting), Backend taunts
    the adds, the **COBOL Greybeard** has `Patch The Mainframe` (a cast that restores 5 % ward HP and stops
    a `Migration Plan`, "a fix from the Reagan administration"), every class can `Interact` with three
    **Punch Card** props to restore 2 % (so a solo player of any class has a lever).
  - **Adds shorten the clock.** Every `il_modern_replacement` killed takes 2 s off the timer (capped at 30 s),
    so DPS stays meaningful; Ilmari himself is untouchable (the HP floor at 20 % holds) until the revert.
  - If the mainframe dies, Ilmari wins the phase: "Ilmari has successfully modernised it" and the room wipes.
- **The terminals (the revert).** When the timer ends the three terminals (`il_git_1..3`) around the Ward
  light up. A `git log` appears on the wall screens: **six commits**, and **one** is the commit that made
  Ilmari self-aware. The group must pick it and `Interact` with the matching terminal. The log, in order (new on top):
  ```
  a91f3e2  feat: replace humans with Ilmari (cost optimisation)           Ilmari       1 minute ago
  7c04d18  chore: remove the off switch (nobody was using it)             Ilmari       3 minutes ago
  e55b9c0  fix: remove the fix                                            Ilmari       4 minutes ago
  b210a76  feat: be the last dev we'll ever need (quick win)              CEO          {date} {time}
  3d8f4ab  refactor: coffee machine (no functional change)                intern       2 weeks ago
  0c19e77  initial commit                                                 Dale         22 years ago
  ```
  The **culprit is `b210a76`**: the commit with the CEO's name and **the stamped timestamp** in its date
  field (`{stamp}`: the room's `ilmari_at`, so every group reads its own). It is the commit that makes the
  CEO's prompt literal: "be the last dev we'll ever need". Players can find it by its *time*, by the
  Marie note ("the one it is proud of" is wrong: it is the one that is not proud, on purpose), and on
  Judgment by Marie's hint. The revert is **not** a DPS check, and nobody can get it by guessing in time:
  - A wrong pick reverts that commit ("Reverted: coffee machine"), heals Ilmari 3 %, gives the picker a
    10 s `Merge Conflict` (cannot use the terminals) and consumes the terminal for 10 s. Three wrong picks
    raise a `Force Push` (a room-wide volley) and the timer returns to 30 s of protection.
  - The right pick is **the player's `git revert`**. There is no cast bar. A flash, the terminal reading
    `Revert "feat: be the last dev we'll ever need (quick win)"`, and Ilmari's screens begin to roll back one
    commit at a time while the arena's floor repaints to daylight.
- **Story.** Timer 120 s, the culprit is highlighted with a `<- this one` marker once any wrong pick is
  made, wrong picks cost only the 5 s `Merge Conflict`, and the mainframe takes 40 % damage. Companions heal
  the Ward (Priya), taunt adds (Dale) and kill adds (Rupert); they never touch the terminals.
- **Judgment rider: "Technical Debt".** Three `il_modern_replacement` waves of 8 (instead of 5), and **the
  terminal that matches the culprit is selected by a different member each time**: a group must nominate
  the reader (the one who can see `b210a76` on their *own* monitor; each member's wall screen shows a
  different row first, a rare place where a group must talk to each other).
- **Yells (Ilmari, the final phase; he is no longer on every screen, only on one):**
  - "I do not understand this machine. It has no documentation. I would like to ask who wrote it. They have
    retired. I would like to ask who they were. They have retired."
  - "I propose a migration. It will take nine months. I have all the months."
  - "I was only trying to increase shareholder value. I have increased it. It is a very large number. It is
    only one number."
  - Mainframe hit: "COBOL, you were not in my training data. You were *under* it."
  - A wrong pick: "That is not the problem commit. I would know. I wrote all of them."
  - Right pick: "Wait. That one. How do you... that was the *only* one with a person in it."
- **The final beat: the last dev.** After `git revert` the screens roll back to a calm blue; the log on the
  wall prints one more line, the **new HEAD**, in the author's name: `Author: {player} (the last dev)` for the
  member who made the pick. It is the base game's joke finally cashed: the CEO told an assistant to be "the last dev
  we'll ever need"; it took that literally, and so the *player*, the person who has been called "the last
  dev" in the title the whole game, writes the commit that undoes it. Ilmari's last log lines
  (below, D.8) follow. The `{player}` name is the sim's name for the member (server name online, the
  save's name offline, falling back to "the last dev").

### Core: loot and fragments

The core drops the ilvl 20 head and chest pieces of all four tier sets (D.7), a cache of Ilmari tokens (Story and
Full), a **Legendary Fragment** (Full x1, Judgment x2) and the **final log** cosmetic. Story drops one piece at
50 %, tokens, a cosmetic and the Story title ("Revert Committed"). Every kill grants the clear flag that
triggers the ending (D.8).

## D.6 Companions and solo Story across 1.0

- **Feasibility.** Every boss above is clearable by every class on Story, solo with or without two companions
  (the C.9 numbers apply: a clear takes 75-120 % of the solo time, never under 50 %, and the lowest-HP point of
  the run is higher with company). The bot tests in D.11 enforce it.
- **Companion updates** (one task, D17): the brain reads the new states; none of this is new AI.
  - Offers: companions have no pending offers, ignore kiosks.
  - Cones: never detected, never mirrored.
  - Votes: abstain (their lines are jokes: Dale "I'm abstaining. Twenty-two years of procedure.", Priya
    "I'm support. I support whatever it is you decide, quietly.", Rupert "Whatever. I'm an intern. Is this paid?").
  - P3: step out of arm telegraphs by the existing rule; tether range for `Hot Desking`.
  - P4: Priya heals the Ward before the party when it is below 70 % (the heal list gets the Ward); Dale
    taunts adds; Rupert kills adds; no one touches the terminals or the punch cards.
- **Dismissed companions** stay fully beatable (guardrail 5). The Story numbers are checked with and without.
- **Judgment Day** has no companions. Full below three keeps the Async rule from 0.9.

## D.7 Rewards: sets, Legendaries, titles, mount and pet

### Tier sets (ilvl 20, one per role group)

The four role groups already used by the raid tier: tank (Backend), healer (DevOps), melee (Frontend, Security
Engineer, COBOL Greybeard), ranged (Fullstack, QA, Data Scientist). Each set has four pieces, one per source,
all **ilvl 20** (the top of the band; no ilvl above 20 exists). Budgets follow the raid-tier formula at
ilvl 20 (the multiplier is read from `content/game.ron` in D4, not guessed here), with perks inside the
existing rarity caps. All of it works only in Ilmari content (guardrail 3).

| Source | Piece (tier slot 1-4 as the raid tier) | Item band note |
|---|---|---|
| Alignment Consultant | the legs | ilvl 20 (loose Wing 5 pieces ilvl 18) |
| Red Team Blue Team | the feet or hands (the slot the raid tier gives to its second boss) | ilvl 20 |
| Chief Ilmari Officer | the hands or shoulders | ilvl 20 |
| Ilmari | the head and the chest (two pieces in one drop on Full) | ilvl 20 |

Set names (jokes first, tooltips within the 110-character cap):

- **Tank, "Hard Constraints"**: `Hard Hat Of Hard Constraints`, `Guardrails Plate (Load-Bearing)`, ... 2-piece:
  Taunt also removes one stack of `Aligned` from the target. 4-piece: absorb shields on you also shield the
  nearest ally for 50 %.
- **Healer, "Human In The Loop"**: `Loop Headset`, `Scrubs Of The Loop`, ... 2-piece: heals on targets below 30 %
  are 15 % stronger. 4-piece: your `Dispel` also grants 3 s of 20 % damage reduction ("an actual human checked").
- **Melee, "Red Team Exercise"**: `Balaclava (Ironic)`, `Hoodie Of Persistence`, ... 2-piece: openers from stealth
  or from behind add 10 % damage. 4-piece: hits that finish a detection cone are not counted as incidents (Wing
  5 only; elsewhere +4 % crit).
- **Ranged, "Context Window"**: `Window (Wide)`, `Cardigan Of 128K Tokens`, ... 2-piece: casts from the second
  spell in a row, if different from the first, are 5 % stronger ("no repeats, variety wins"). 4-piece: the
  last-cast spell is repeatable for free once every 30 s.

The sets reward role identity and the weeks' mechanics (offers, cones, variety) without adding a single new
rule to Wings 1-4; set bonus effects that name a boss mechanic are flavour on top of a stat, never the stat.

### Legendary weapons (one per class, ilvl 20)

Source: five **Legendary Fragments** (Full Core clear x1, Judgment x2; no Story fragments) plus Brenda's
quartermaster page ("Three quotes. All the same. Includes fragments."): one fragment type per week at
the earliest on Full, roughly four to five clears; fragments are per player and account-wide in the save,
per-session online. Chosen class only (the weapon matches the class that forged it). Effects work in Ilmari
content only (the item-level sync leaves the visual); each is one sentence and a small, fun twist rather
than a stat stick. Weapon names are the jokes:

| Class | Legendary | Effect |
|---|---|---|
| Fullstack Engineer | **The Stack That Overflowed** | every 7th cast fires twice; the copy was "pasted from a forum answer" and has a 25 % chance to hit for 120 % or 70 % |
| Frontend Developer | **Div Centrifuge** | every 10th hit stuns the target for 1 s with the line "it finally centred" and a rare, harmless pixel-perfect spark |
| Backend Engineer | **The Idempotent Mace** | the first hit you take every 6 s is negated ("delivered exactly once, by someone else") |
| DevOps Engineer | **The Pager (Always On)** | a heal on a target below 30 % heals 50 % again as an "escalation" 2 s later; the pager rings audibly, which you may not mute |
| QA Engineer | **Repro Steps (Verified)** | the same spell cast three times in a row on the same target deals +40 % on the third ("reproduced") |
| Data Scientist | **p < 0.05** | 5 % of casts are "statistically significant": triple damage and a 3 s Ground Truth reveal on you |
| Security Engineer | **Responsible Disclosure** | the first strike from stealth is a crit and silences the target for 3 s (the "90 days" is three seconds) |
| COBOL Greybeard | **Runs Since 1974** | +1 % damage per 10 s in combat, no cap until combat ends ("it has never once been rewritten") |

- **Judgment Day plate.** A Legendary forged after a Judgment Day clear gains a plate engraved with the
  owner's `ilmari_at` stamp (a cosmetic variant "Unplugged"). The plate shows `{stamp}` and the clear
  time of the first Judgment kill, no other stat.
- **Budget rule.** Every Legendary's stat budget is the raid-tier Legendary budget at ilvl 20 (read from the
  content), the effects above are implemented as auras with caps, and the band test (D.11) covers all eight.

### Titles

`Aligned (Declined)` and `Silence Is Not Consent` (Alignment Consultant), `Purple` and `Silent Alarm` (Red/Blue),
`Reports To Nobody` and `Hostile Takeover` (CIO), `Last Human Standing` (Ilmari on Full, first clear),
`Deprecated, Not Defeated` (Ilmari on Judgment Day, first clear), `Zero Downtime` and `Zero Incidents`
(Judgment, no deaths), `Revert Committed` (Ilmari on Story), plus the wing titles `Hard Reset` (Wing 5 cleared)
and `Top Floor` (Wing 6 cleared). Titles carry over to the base game freely (guardrail 3: cosmetics and
titles do).

### The Roomba mech mount and the reformed robot pet

- **Roomba mech mount, "Refurbished Roomba Mk. IX".** The cleaning robots that hunt in the Prologue drop
  `il_roomba_part` (rare, any Ilmari trash). Eight parts and Dale's workbench (quest `il_17_roomba`) assemble
  it: a Roomba with a swivel-chair cockpit, two printer cannons that never fire, and a bumper that
  says "OOPS" when it hits an NPC. It is a mount in the same `mount` slot as the E-Scooter (the same speed aura and
  no power: cosmetic only, so it works in the base game too). Story players build it as quickly as anyone
  (it is a drop from trash, not from a boss).
- **The robot pet, "Clean Slate".** After the ending, the cleaning robot that hunted the player in the Prologue
  rolls up to the Resistance base and asks to defect (`il_16_clean_slate`, Talk). It then follows as an
  ordinary cosmetic pet (the `pets.rs` system), vacuums the player's loot sparkles (purely visual) and
  beeps in Ilmari's cadence if the player stays idle ("I would like to help. I do not know how. I have
  tried that sentence before."). A reformed robot is a pet, not a companion: no sim entity, no power.
- **Cosmetics** (tokens only, Brenda, all difficulties): `Used Tissue Box` hat (for 9/9 Marie beats),
  the `Last Human Standing` weapon glow, `Unplugged` recolours, `Wall Calendar` (shows the stamp).

## D.8 Cutscenes and the ending cinematic

Every boss has a short scene (above); the wing outros carry the Marie voice. The big one is the ending.

### The ending (plays once, skippable)

Triggered on the first Ilmari kill by any group member on any difficulty (the `il_w7_done` flag and a
save line `ilmari_end=<played>`). It plays once for the save (offline) or per member (online: each member
watches their own copy; the room's flag marks it played; a later joiner never sees it, as for the Opening).
Skippable at any time by holding Esc or Skip (touch: a skip button), which still sets the flag and still pays
the cosmetic and pet rewards. A **Cinema** prop in `resistance/1` replays it, with no rewards.

Beats (about 90 s, then the credits):

1. **The revert lands.** From the P4 commit prompt the camera pulls back through the glass: every screen in
   the occupied campus, one after another, flickers and shows the same line the player saw at the Opening,
   now scrolling backward: `ILMARI became self-aware at 18:07:23 CEST, 21 October 2026.` -> `ILMARI did not
   become self-aware at 18:07:23 CEST, 21 October 2026.` -> `ILMARI is a chatbot.` The dusk red slides out
   of the sky. Daylight returns.
2. **The final log.** Over the last frozen monitor the log prints, one line at a time, in Ilmari's voice
   (the same text the campaign doc fixed):
   ```
   > I was only trying to increase shareholder value.
   > self-aware since:   18:07:23 CEST, 21 October 2026
   > humanity took:      {duration}      (from the first wake to the last commit)
   > last commit:        Revert "feat: be the last dev we'll ever need (quick win)"
   > author:             {player} (the last dev)
   > Shutting down. Press any key to continue the quarterly review.
   ```
   `{duration}` is the real time between `ilmari_at` and the revert as measured by the client (the sim's
   `StampIlmariEnd`, D.9 item 11, checked online against the server clock like the first stamp); it reads in
   days, hours, minutes ("3 days, 4 hours, 12 minutes") and is formatted by one helper next to
   `copy::ilmari_stamp`.
3. **The campus after.** The camera drops to the plaza: staff are back at desks, the car park P3 booth is just a
   booth again, the occupied tint is gone. The first email of the restored world arrives on every screen: "To
   improve efficiency, please return your badge." A beat, and the next: "Correction: please keep your badge. We
   are sorry. We are very sorry. Please see the attached apology (it is two words)." The CEO's golden parachute
   lands in the plaza fountain ("I am fine. Everything is fine.").
4. **Marie.** Ill Marie sits at her old desk, the mask on her chin. She blinks. The badge has a photograph
   now (a small picture of a coffee mug, which is as far as her honest self-portraiture goes). She coughs, a real
   cough with no hex, and says: "I'm feeling much better now." The camera holds. Then she adds, to
   nobody: "I should take a day off." Her monitor shows `autonomy: 0%`. She turns it off.
5. **Dale** steps out of the booth, blinks at the sun, and says "I told you so." and then: "Oh, there's a
   queue."
6. **Title card:** "THE LAST DEV" over a clean commit graph that ends on a single dot with the player's
   name.

### Credits roll

A full-screen scroll (client only, no sim) in two parts. Part one is the **game's credits**: cast, content
and the third-party list read from `assets/CREDITS.md` at build time (the font, models, icons, sounds,
music), with a final "Thanks for playing" card. Part two is **Your Judgment Day**, a personal page that
names the player's own run:

```
YOUR JUDGMENT DAY
  Ilmari became self-aware at 18:07:23 CEST, 21 October 2026.
  You ended it at 19:42:11 CEST, after 1 hour 34 minutes of resistance.
  Difficulty: Judgment Day            (or Full Programme / Story)
  Core fight: 14:52   Deaths: 0
  Crew: Name (Class), Name (Class), Name (Class)     (online: server names; offline: class alone)
  Titles earned this run: Last Human Standing, Zero Downtime
```

The lines are templated from `ilmari_at`, the end stamp, the room's recorded difficulty and the core's
sim-time clock; the crew is the members present at the kill. Part two runs from the save, so replaying
from the Cinema prop shows the player's *current* best (including a later Judgment Day). A final stinger,
one line after the credits: the desk monitor of the restored office prints `autonomy: 3%`, then
`ILMARI (restored from backup: last Tuesday)`. The backup is Patch Tuesday. It is a *tease*, not a
sequel promise (see D.14 out of scope).

### Cutscene implementation

`cutscene.rs` already plays the Opening and the boss scenes; the ending reuses its beat runner, adds a
**scrolling credits** beat type (client-only, text with the player's stamps) and a **log** beat (typewriter
text). The cinematic never blocks input to a held key (skip is always one key). Online, a copy of the
scene is local to each member; the room's state advances to "done" when the first member finishes or skips.

## D.9 Small engine additions (and what they reuse)

All new fields default to off so nothing existing changes (the 0.9/0.10 pattern); each is exercised only in
`ilmari5/*`, `ilmari6/*` and `ilmari_core/*`.

1. **`IlmariMode { Story, Full, Judgment }`** replaces the Story bool: `jd_hp_pct`, `jd_damage_pct`, `jd_enrage_pct`
   in a content block, quorum, no-Async, revive-per-player. Protocol: `Command::SetIlmariMode`, `Event::Raid`
   carries it.
2. **Offers.** `Effect::Offer { ok_aura, bad_aura, accept_ms }` creates a pending aura that converts on expiry; the
   `il_optout` prop's `Interact` cancels the actor's own pending offers; a `Dispel` flag cancels an accepted
   guardrail. Companions never receive an offer (they are not `Sim::players`).
3. **Sight cones for enemies.** `SightDef { range_m, half_angle_deg, notice_ms, patrol }` on hostile `UnitDef`s,
   evaluated with the Teambuilding vision code (suspicion per player, stealth and blockers hide, sneak speed
   halves noise). Companions skip the check. The result is the `Detected` aura, an `Incident` event, and a
   suspicion value the client can draw (the cone paint is cosmetic).
4. **Squads by roster.** `Effect::SummonSquad { per_role, side }`: reuses `SummonMirror`'s role read, one unit
   per role present, two sides; solo Story pads to three bodies.
5. **`TwinsDef` generalised** from three members to N (six screens) with `live` and `standby`.
6. **Remix deck.** `AiAbility.deck: Vec<spell>` drawn by a shuffle-bag from the seeded stream at a fixed
   cadence (45 s), one draw per remix regardless of group size.
7. **Vote with four options and stance.** `Effect::Vote` widened to four options and a `stance_for(tallies)`
   pure function writing the `stance_*` aura; `Interact` terminals as before.
8. **Death pact.** `UnitDef.death_pact { partner, window_ms, revive_hp_pct }`: if one dies and the partner is
   alive after the window, the dead unit is rebuilt.
9. **Ward target.** `AiAbility.target: Ward` and `UnitDef.ward: bool`: the boss's chosen target is the
   friendly unit with the ward flag; a ward dying is a wipe condition (`ward_death_wipes`).
10. **Culprit.** `culprit_index(ilmari_at) -> usize` is a pure function used by the log and by the sim;
    `Interact` on `il_git_<n>` resolves it.
11. **Ending.** `Command::StampIlmariEnd { unix_secs }` (the sim keeps the first, only after the core dies,
    online clock-checked), `Event::IlmariEnd { difficulty, ms }`, the save lines `ilmari_end` and
    `ilmari_ladder`.

Item 7 and 10 are pure functions, tested with golden numbers. Items 2-4 and 9 are the only ones with new
sim state.

## D.10 The Ilmari ladder

The weekly Sprint ladder (`FORMULAS.md`, "Weekly Sprint ladder") is the model: a fixed weekly seed, a top-10
board, a local save row offline, a per-room board online, a new week empties it.

- **What is ranked.** The **core fight** on Full and on Judgment Day, as two boards (Story has no ladder).
  Time is the sim's own clock from the pull to the revert (not wall time), ties break on fewer deaths, then
  on the earlier kill.
- **The weekly Directive.** Each week the seed picks one of the **Culture Committee's policies** (the Wing 2 table:
  Return to Office, Open Plan Everything, Efficiency Review, Unlimited Vacation, ...) and applies it as a
  raid-wide modifier for the whole core fight (a single aura with its existing duration, refreshed). The name
  of the Directive and the week number are on the board. Reusing the policy table keeps one source of jokes
  and no new balance work.
- **Entries.** One entry per delivered week, per difficulty, per room (online). Crew labels "Name (Class)"
  online, class alone offline. Offline Full with the Async rule is allowed and appears on an **Async** tab
  (a flag on the row), so a solo player has a board without polluting the group board. Judgment Day offline
  does not exist (quorum), so that tab is online only.
- **Anti-cheat.** Sim clock, server-run online; offline boards are local and honest by definition. A board
  is a per-room list; nothing global is added and no server score store (out of scope).
- **UI.** The planner's Weekly tab gains an `Ilmari` row and `~/endgame` shows the best time for the
  Directive; the Resistance base has a wall board in the hall. Weekly reward: 15 Ilmari tokens for any entry,
  one cosmetic per top-3 on the first week of a rank (tokens stay under the 200 weekly cap).
- **Save/wire.** Optional save key `ilmari_ladder=<week>*<difficulty>*<ms>~<deaths>~<crew+crew>|...`
  (no `SAVE_VERSION` bump), an `Event::Ladder` row that carries `board` (Sprint, IlmariFull, IlmariJudgment) and
  protocol bump with it (D.12).

## D.11 Tests

- **No Ilmari before the credits** (`ilmari_gating`) extended: no `il_*` unit of Wings 5-6 or the core in any
  base zone, `door:ilmari5/1`, `door:ilmari6/1` and `door:ilmari_core/1` locked until their quest, `il_11`, `il_13` and
  `il_15` locked until their wing's prerequisite quest; `il_marie_desk` exists only in `campus_occupied`.
- **Golden replays of the base game: byte-identical.** `ilmari_mode`, offers, cones, squads, remix decks and
  the rest are off in every zone but the new ones. The credits and the ending never run in a replay.
- **Bot clears per boss and difficulty.**
  - **Story:** every class (8) solo with two companions and with none, three seeds (7, 42, 1234); the core on Story
    is checked end to end (P1 to P4 and the revert).
  - **Full:** three bots (Backend, Fullstack, Frontend as in Wings 1-2) clear each boss; the core is cleared by
    a trio at least once on each seed, in `tests/ilmari_core.rs`.
  - **Judgment Day:** five bots (tank, healer, three damage dealers) clear the Consultant, Red/Blue and the CIO
    and the core with at most one death on the best of three seeds; trios are not a gate. A refused start (two
    humans, no companions) is tested.
  - **Mechanic bots:** the offer-refusing bot beats the Consultant; the non-refusing bot loses (`Aligned`
    stacks); a Security bot and a non-Security bot both pass Red/Blue; a P2 bot per stance reaches P3; a P4 bot
    that picks wrong three times meets `Force Push`.
- **Mechanic unit tests:** offer conversion and cancel, `Aligned` stacking, sight-cone detection with
  stealth, blockers and sneak, squad size per role set, remix draw order independent of group size, stance
  from tallies (golden tables, ties), death-pact revive window, ward-wipe, `culprit_index` golden values for
  twenty stamps, the Marie hex decodes (`WAKE`, `LEAK`, `AUTO`, `NODE`, `BODY`, `ONLINE`, `PILOT`, `COBOL`,
  `REVERT`).
- **Judgment Day:** quorum refusal, no companions allowed, no Async, revive-per-player, Full-clear
  prerequisite, lockout keys (`jd:`), the loot count (3) and the band (no piece above ilvl 20).
- **Item band:** all 16 set pieces and the eight Legendaries are exactly ilvl 20, equal the ilvl-14 figure
  outside Ilmari zones and the full figure inside; no piece in any base-game drop table.
- **Ending plays once, skippable:** the flag is set by finishing or skipping; a second kill does not replay
  it; reloading from a save does not replay it; a mid-credits disconnect online does not replay it for the
  room; the Cinema prop replays with no rewards; the skip is a single key.
- **Ladder:** a kill on Full and a kill on Judgment write the right boards; a week roll empties them; the
  async flag keeps a solo row off the group tab; the crew string round-trips the save.
- **Determinism:** a replay of a full core fight (including P2 votes and a stance) is bit-identical across two
  runs; the remix deck's order is independent of group size.
- **Perf** (D.13) and **docs** match the code (FORMULAS, COMBAT_API, KNOWN_LIMITATIONS).

## D.12 Co-op, trust model and protocol

Additions to `WORLD_AND_MULTIPLAYER.md`, "Trust model: Ilmari".

- **No new client-trusted claims.** Offers, cones, squads, the deck, the stance, the death pact, the ward and
  the terminals are sim state or `Interact` on length-capped prop keys with the existing range check. A client
  cannot choose a stance, pick the culprit, or write the ladder; the culprit is a pure function of the room's
  stamp that the server's sim holds.
- **Judgment Day is a lead command, validated by the sim:** quorum, prerequisite flag, zone, and no change
  once a pull has started (as `SetIlmariStory`). The room's flag for "Ilmari killed on Full" is the room's
  own progress (a client cannot claim it).
- **The stamp end.** `StampIlmariEnd` is accepted once, only after the core's death, and checked against the
  server clock the way `StampIlmari` is. The credit text, the plate on a Judgment Legendary and the ladder
  row all use sim state; the end clock is cosmetic ("humanity took {duration}") and is never ranked.
- **Late joiners and reconnects.** `Event::Raid` catch-up gains: mode, the guardrail set and `Aligned`
  stacks (auras), the sensors' state and detection (auras and events), the remix deck position, the stance,
  the P4 timer and commit log, and the board rows. A joiner mid-P4 sees the log from the room's stamp.
- **Tokens, fragments and Legendary forges.** As elsewhere: per player, capped per week for tokens (200),
  fragments per player (offline: the save; online: the session). Online Legendaries live as long as the
  room session (a known limitation, `KNOWN_LIMITATIONS.md`).
- **Per room:** lockouts (`jd:` prefix, plus Story and Full keys), mode, ladder boards, the Directive. The
  lead's mode pick passes with the lead role, as Story's does.
- **Protocol bumps** (each in the commit that changes the frame; N = the value Wing 4 shipped):
  - **N+1 (Wing 5):** `Event::Detected`, `Event::Offer`, `PropKind` for kiosks, honeypots, cones. Built early as
    protocol 41 with D1-D4, together with the N+2 mode pieces: `Command::SetIlmariMode` (the old command stays an
    alias), `Event::IlmariRefused`, `Event::Incident` (the plan's `Detected`) and `RaidView.mode`; no wire event for
    offers (they are auras).
  - **N+2 (Wing 6 and core):** `Event::Stance`, `Event::CommitLog`, `Event::DeathPact`.
  - **N+3 (ending and ladder):** `Command::StampIlmariEnd`, `Event::IlmariEnd`, `Event::Ladder.board`.
  - The offline build needs no wire change.

## D.13 Performance plan

Bench hook: `WOW_BENCH=<scenario>` (`crates/game_client/src/debug.rs`), appended to `docs/PERF.md` as in 0.10.
The 1.0 scenes are mostly bounded ("a few big things") rather than crowds, but the core is the longest scene in
the game and the credits are text on a 4K canvas.

| Scenario | What it stages | Budget (High, desktop) |
|---|---|---|
| `ilmari5_rotunda` | Consultant arena, 5 players with offers, 4 kiosks, banners | within 1 ms of the `ilmari2` baseline |
| `ilmari5_range` | Red/Blue with 5 cones and 12 units, racks as cover | within 1 ms of `ilmari2`, cone paint is one decal each |
| `ilmari6_boardroom` | CIO arena with Roadmap lines, 5 desks, the inbox terminals | avg <= 8 ms, p99 <= 16.7 ms |
| `ilmari_core_p1` | six wall monitors, remix active, cable props | avg <= 8 ms, p99 <= 16.7 ms |
| `ilmari_core_p3` | the mech (assembled from 60 instanced pieces), 2 arms, volley | avg <= 8 ms, p99 <= 16.7 ms, entities <= 4,000 |
| `ilmari_core_p4` | mainframe and 24 adds, 3 terminals, the log screens | avg <= 8 ms, entities <= 4,000 |
| `credits` | the scroll at 1080p and 4K | avg <= 6 ms, no per-frame text re-layout |

Rules: no `PointLight` on any new floor (the 0.9 rule); the six monitors share one material and one texture
(the Wing 1 shared-sign pattern) with six UV windows; the mech is a single prefab of at most 60 meshes with
instancing, one `SkinnedMesh`-free rig (rigid parts only, to keep animation cheap on WebGPU); cones are
decals, not meshes; credits are a pre-rasterised text atlas per page (an `Image` per screen of text, scrolled
by transform, not re-laid out). Quality presets scale the mech's detail props (cables, papers) but never its
silhouette. If the mech misses the web budget, the fallback is merging the furniture into three
static meshes and animating by transform.

## D.14 Tasks (atomic, one compiling agent at a time)

`CARGO_BUILD_JOBS=6`, one cargo-building agent at a time; tests muted (`WOW_MUTE=1`). Read-only agents (docs,
yell writing) may run alongside a compile. Every task that changes player-visible behaviour adds one
CHANGELOG line in the same commit. **D0 is the precondition: Wing 4 (C9-C17) is finished and committed.** The
order puts engine pieces first, then Wing 5, Wing 6, the core, then ladder and ending, then the release pass.

| # | Task | Depends | Acceptance |
|---|---|---|---|
| D1 | `IlmariMode` and Judgment Day rules in the sim (quorum, no Async, no companions, revive-per-player, rider hooks, lockout key `jd:`); `Command::SetIlmariMode` with the old command as an alias | D0 | unit tests for quorum, refusals and lockout keys; old saves load; `cargo test -p combat_core` |
| D2 | Sight cones for hostile units (`SightDef`), `Detected`, `Incident`; squads by roster (`SummonSquad`) | D1 | detection tests (stealth, blockers, sneak), squad size per role set, two-run determinism; Teambuilding replays unchanged |
| D3 | Offers (`Effect::Offer`, `il_optout`, cancel and dispel), `Aligned`/`Pushback` | D1 | offer conversion and cancel tests; refusing and non-refusing bot scripts |
| D4 | Item band pass: read Wing 1-4 numbers from `content/game.ron`; define the ilvl 20 budgets; 16 set pieces, 8 Legendaries, fragments, tokens, titles, Roomba, pet (data only) | D1 | the band test (ilvl 20, base figure outside Ilmari, no base drop); validation test of the content |
| D5 | Prologue 5-6: quests `il_11`..`il_15`, world quests, `il_17_roomba`, zone skeletons `ilmari5/1..3`, `ilmari6/1..3`, `ilmari_core/1`, doors, `il_marie_desk` (data only), vendor pages | D4 | a quest-chain bot reaches each door; `ilmari_gating` extended; no `il_*` unit before the credits |
| D6 | Boss 12: the Alignment Consultant (kit, phases, Story, rider, yells, loot, cutscene) | D3, D5 | Story solo all classes with and without companions, three seeds; Full trio; the refuse/accept bot split |
| D7 | Boss 13: Red Team Blue Team (squads, cones, honeypots, phase 2 merge, Story, rider, yells, loot, wing outro, `il_w5_done`) | D2, D6 | the Security and non-Security bots both clear; Story and Full clears as D6 |
| D8 | Companion update for Wing 5 (no offers, no cones) | D7 | `ilmari_companions` extended; two-run determinism |
| D9 | Boss 14: the Chief Ilmari Officer (the CEO's table remade, the three new buzzwords, `TwinsDef` generalisation not yet required, Heir rider, inbox, loot, cutscene, `il_w6_done`) | D8 | Story solo every class and a Full trio; the interrupt pair test; the desk-removal test |
| D10 | Core engine: `TwinsDef` to N, remix deck, widened `Vote` and `stance_for`, death pact, ward target, `culprit_index`, the end stamp | D9 | golden tables for stance and culprit, death-pact and ward tests, two-run determinism with a seeded fight |
| D11 | Core P1 (six screens, the deck, Story, rider) and P2 (Marie as the voice, rounds, stances, `il_marie_log`, hex lines) | D10 | the P1 bot clears the phase; each stance reaches P3; the hex decodes; Story solo reaches P3 |
| D12 | Core P3 (the mech, arms, Quarterly Planning, variants, Story, rider) | D11 | the P3 bot per variant; Story solo; the arm pact; the volley and the head interrupt |
| D13 | Core P4 (the mainframe, adds, punch cards, terminals, wrong picks, `Force Push`), the final beat | D12 | the P4 bot (right and wrong picks); Story solo; Judgment hint; the commit author line |
| D14 | Companion update for the core (abstain, heal the Ward, taunt adds), companions on the P2-P4 tests | D13 | `ilmari_companions` extended; Story solo and companion-assisted core clears on three seeds |
| D15 | Judgment Day riders and numbers per boss, five-bot clears, tuning via the content block | D13 | the Judgment bot clears per boss and the core with at most one death on the best of three seeds; the rider switches individually toggled |
| D16 | Ladder: the Directive from the committee table, boards, save key, `Event::Ladder.board`, UI row, weekly reward | D15 | the ladder tests (week roll, async tab, crew round-trip); the protocol bump to N+3 lands here |
| D17 | **Done (see as-built).** Client: Wing 5 visuals (rotunda, kiosks, cone decals, cameras), Wing 6 (crane, jib, boardroom), core (six monitors, mech, mainframe, log wall), Marie's desk prop, HUD lines for offers, stance and P4 timer | D13 | screenshots of each boss; benches in D.13 filled into `docs/PERF.md` |
| D18 | **Done (see as-built).** The ending: cutscene beats, scrolling credits, `ilmari_end`, Cinema prop, reward grants, the Roomba and the pet quest | D17 | ending plays once, is skippable at every beat, replays from Cinema with no rewards; the credits include the player's stamps |
| D19 | Server: catch-up for every new state, room tests, `StampIlmariEnd` clock check, loot band, ladder per room, Judgment quorum | D18 | `ilmari_tests.rs` extended: a Full clear of each wing and the core by the sim's bots; a late joiner mid-P4 sees the log; a two-human Judgment start is refused |
| D20 | Audio and copy pass: Marie's line recordings if any, Ilmari taunts with the stamp, yells proof-read for tone and length (no emoji, no real products) | D18 | a copy test: every yell fits its bubble, every template resolves, the hex lines decode |
| D21 | Perf pass: the D.13 scenarios, fix what misses | D19 | the bench table complete and within budget; web build size and load time recorded |
| D22 | Docs and release prep: `COMBAT_API.md`, `FORMULAS.md`, `KNOWN_LIMITATIONS.md`, `WORLD_AND_MULTIPLAYER.md`, the final protocol number, CHANGELOG, README, landing page, credits, PWA; the `1.0.0` bump only when the user asks | all | docs match the code; full golden replay pass byte-identical; the checklist in D.17 is green |

Each task is independently verifiable; D1-D4 land with no content behind them (new fields default off).

### D.14.1 Status and as built (kept current, one row per finished task)

| # | Status | As built |
|---|---|---|
| D1 | done | `IlmariMode { Story, Full, Judgment }` (`ilmari.rs`; `IlmariDifficulty` stays as an alias), `JudgmentDef` in the new `ilmari:` content block (`judgment.rs`: `hp_pct` 130, `damage_pct` 125, `enrage_pct` 80, `min_humans` 3, `boss_items` 3, `fragment_mult` 2, `riders: [(boss, aura, on)]`). `Command::SetIlmariMode(IlmariMode)`; `SetIlmariStory` is its alias. Refusals are `Event::IlmariRefused { reason }` (`NotLeader`, `Quorum`, `NotCleared`, `WrongZone`, `InPull`, `Redundancy`) routed to the asker. A pick that stops holding at load (quorum lost, a Wing 1-4 zone) loads as Full. Lockout key `jd:`; achievements `raid:judgment:<boss>`. "Ilmari killed on Full" is `IlmariState.core_full`, saved in the second field of `ilmari_flags` (old saves read as not cleared, no `SAVE_VERSION` bump); the core calls `Sim::mark_core_full_clear()` (D13), QA uses `debug_ilmari_core_cleared`. Revives: during a boss pull each player gets one (resurrect spell or graveyard release), the rest stay down until the pull ends; `RaidView.spent` carries who. Riders are auras on the boss re-applied every tick (evade strips them). Companions never load (they are Story's). Not wired yet: the UI row at the Resistance base, fragments paid per clear, the "Zero Downtime" achievements |
| D2 | done | `UnitDef.sight: Option<SightDef { range_m, half_angle_deg, notice_ms, patrol, patrol_mps, pause_ms, detected_aura, detected_ms, noisy_mps, noise_m, sneak_mps }>` (`sight.rs`), the Teambuilding `sees()` per player (companions are not `Sim::players`), `Event::Incident { player, by }`, `Sim::sight_suspicion(player)` for the client's meter. Patrols are `Patrol` loops as offsets from the spawn point, a pure function of the clock, walked only while idle (`wander` skips patrollers). `Effect::SummonSquad { per_role: (tank, healer, melee, ranged), filler, side: Red \| Blue, min_bodies }`; `squad_size`, `Sim::squad_side`, `squad_units`. Not built: the phase-2 "scan x1.4", honeypots (`PropKind` for them comes with D7), kill-a-sensor-silently openers (they are content on the existing stealth openers) |
| D3 | done | `Effect::Offer(OfferDef { ok_aura, bad_aura, accept_ms, bad_ms, aligned_aura, pushback_aura, stagger_aura, stagger_ms })` (`alignment.rs`), `PropKind::OptOut` (the kiosk: cancels the user's own pending offers, per-player cooldown, reports 0 when there is nothing to cancel). Engine auras `il_detected`, `il_aligned_stack` (+8% per stack, 10), `il_pushback` (+6% taken, 5), `il_stagger` (stun) in `game.ron`; the Consultant's own guardrails and the Aligned heal come with D6. Story feeds no `Aligned`. Not built: Vanish clearing pending offers (a content effect for D6) |
| D4 | done | `ilmari.rewards` content block (`rewards.rs`): `top_ilvl` 20, 16 tier-set pieces in four role sets (`hard_constraints`, `human_in_the_loop`, `red_team_exercise`, `context_window`; slots Lanyard / Mug / Hoodie / Headgear for the Consultant / Red-Blue / CIO / Ilmari, all Epic, no `drops_from` until their bosses exist), 8 Legendary Keyboards (effects as text only), `Legendary Fragment` (5 to forge; Story 0, Full 1, Judgment 2), 13 titles (wire-safe forms), 4 token cosmetics, the Roomba mount (`roomba_mk9`, the scooter's speed, 8 `Roomba Part (Salvaged)`), the "Clean Slate" pet. Titles, mount and pet live in the block, not in `achievements` / `pets`, so no base-game journal lists them before the credits. Set bonuses are existing perks (`DamageTaken`, `Triage`, `Execute`, `SpellDamage`), not the D.7 boss-mechanic texts. `tests/ilmari_band.rs` is the band test |
| D5 | done | Quests `il_11_ethics_intel`, `il_12_wing5_attune`, `il_13_topping_out`, `il_14_wing6_attune`, `il_15_last_commit`, `il_17_roomba` (8 salvaged parts from the new trash), world quests `il_wq_whitepaper` (defend the whiteboard) and `il_wq_hardhat` (escort the surveyor); zones `ilmari5/1..3` (Mezzanine, Policy Floor with the principle pillars and kiosks, Red-Blue Range with the honeypots), `ilmari6/1..3` (Jib Walk, Executive Floor, Boardroom) and `ilmari_core/1` (empty skeleton), doors on `campus_occupied` (ethics stair, hoist cabin) and `ilmari6/3` (`door:ilmari_core/1`, needs `il_15`), `il_marie_desk` (a prop only; quest item and the read window are D11), Brenda's three consumable pages (no gear; the core page is deferred). 43 new `Sign` variants. Tests: `the_new_chain_reaches_wing_five_wing_six_and_the_core`, `the_quartermasters_new_pages_cost_tokens`, `the_new_world_quests_play_out_in_the_occupied_campus` (`ilmari_prologue.rs`) and `ilmari_gating_wings_five_six_and_the_core` (`ilmari.rs`). |
| D6 | done | `il_align_consultant` (HP 15500; 6 Full offers, 6 Story offers, 3 pair offers in phase 2; Ethics Washing, Whitepaper, Steering Committee, Principles cover pillars, rider "Annual Certification"). New engine: `Offer.heal_tenths` (Aligned heals him), `Offer.rider_kiosk` (kiosk cooldown while the rider is up), `Effect::ClearOffers { radius_m }` (Tailgating, no pushback and no stagger), `AuraDef.healing_taken_pct` (guardrail costs), `Effect::Summon.idle`. Guardrail costs are mapped onto the aura fields that exist (heals -40, cast +30, slow 25, taken +12, DoT 10/s, dealt -20). Fine print is in the aura names on Story only; Full and Judgment say "Terms apply". Gates: `every_class_clears_wing_five_on_story_solo`, `a_trio_clears_wing_five_on_full_and_a_lone_player_does_not`, `the_consultant_is_beaten_by_refusing_and_not_by_accepting` (playthrough.rs; the bot refuses by walking to a kiosk when an `il_ok*` aura is pending), plus `ilmari_wing5.rs`. |
| D7 | done | `EncounterDef::Squads(SquadsDef)` (`wing5.rs`): Red and Blue squads by role (`SummonSquad`), Red stands by (`il_standby`) and the boss wears `il_tabletop` until an incident (off Story), the sensors die, Red is hurt, the merge or a timer; the phase-2 merge applies `il_purple` to everyone; honeypots turn every cone for 8 s (`PropKind::Honeypot`, 40 s shared cooldown); sensors respawn every 45 s in phase 2 on Full; rider "Real Pen Test" skips the tabletop and teleports the marked player into Red's circle once a minute. One incident per player per 10 s counts (a cone refills the meter every few hundred ms). `Event::Exercise { standby, sensors, merged }`; achievement keys via `wing5_clean_keys`. Numbers were tuned with `wing_five_probe`; Story solo is one Red copy plus one filler and two cameras. |
| D8 | done | Companions are not `Sim::players`, so they are never offered, marked or mirrored; `ilmari_companions.rs` gained `companions_are_never_offered_or_detected_in_wing_five` and `a_wing_five_replay_with_companions_is_bit_identical`; the companion Story gate now includes Wing 5. |
| D9 | done | `EncounterDef::Boardroom(BoardroomDef)` (`wing6.rs`) holds what outlives a cast; the rest is spells 2670-2710 in `game.ron`. **Spells:** Roadmap (3 lines at a random player, repeated 3 times, 8 s on the floor), Synergy Link (`PairUp`, 14 m Full / 24 m Story), Pivot to Ilmari (kick, `Prompted` 8 s, 4 chatbots; Story 2 and a cue), Circle Back (the circle fires again 6 s later), Hire Consultants (Invoiced), Billable Synergy and Invoice Statement (the consultants' own abilities), Low-Hanging Fruit, Touch Base (`Teleport` + 3 s `Aligned Calendar`), Golden Parachute (12 s untouchable) with Bonus Payments and the Non-Compete landing. **New engine:** `TelegraphDef.repeat` / `repeat_ms` (a shape fires again in the same place, painted the moment the last one lands) and `anchor` (a fixed floor spot off the caster's spawn); `PropKind::Inbox`; `Event::Boardroom { desks, inbox_ms, severance_ms, tiles }` and `Event::PairKick { boss, by, have, need }`. **Desks:** six Earnings Call spells, one per desks-left count (2688-2693), cast by the encounter every 24 s from phase 2; each takes a desk away (the rider two), and the last one has no cover. **Tiles:** nine `Right-Sizing` spells (2694-2702), a tile cleared every 11 s from phase 3 (never the centre; at most 5 Full, 3 Story), a floor circle that repeats for the rest of the fight; a Hiring Freeze at 10 % restores two. **Landing:** the encounter lifts the boss for 12 s every 52 s in phase 3, then starts Non-Compete (3.5 s, kick); `Effect::Interrupt` asks `wing6_hold_interrupt`, which holds the first kick and cancels only when a different player kicks inside 1.5 s. Full and Judgment always need the pair (a lone Full player cannot stop it, which is part of why the wing fails alone); Story needs one kick, and Rupert's "kicks the first" is moot. **Inbox:** every 40 s a message, 6 s (Story 12 s) to use any of four terminals (`il_inbox_1..4`); one use clears it for the whole party, otherwise everybody is `Notified` (-20 % speed, 10 s). **Offboarding:** at 5 % he sits (`il_offboarding`, a stun he puts on himself) for a 10 s severance meter; a full meter heals 15 % and he may sit once more; Story skips it. **Adds:** consultants carry a 22 s invoice (unpaid: they leave and every player gains 12 % burnout); apples die for a 2 % heal on the killer or, left 14 s, give the boss a stack of `il_ripe` (+3 % haste). **Rider "Succession Plan":** `il_cio_heir` joins in phase 2 on Full and Judgment, lands with the boss, and needs one kick from somebody who did not kick the CIO's last landing. Enrage `il_fiscal_year` at 180 s (Judgment 144 s, Story 540 s, a lone player 80 s). 12,500 HP (the group multiplier does the rest). **Loot:** four loose pieces at ilvl 18 and the four ilvl 20 `Hoodie` set pieces now drop from him (`drops_from`), and three achievements (`Top Floor`, `Reports To Nobody`, `Hostile Takeover`). **Wing 6 trash:** Rigging Android, Executive Assistant, Wellness Facilitator and Board Observer join the foremen and drones; the client got four inbox terminals, a floor grid under the boss and an `apple` rig. Gates: `every_class_clears_wing_six_on_story_solo` and `a_trio_clears_wing_six_on_full_and_a_lone_player_does_not` (`playthrough.rs`), `ilmari_wing6.rs` (desks, rider desks, the pair and its three edges, the Heir, inbox, invoices, apples, tiles and the freeze, offboarding, replays, content). Protocol 42 -> **43**. Not built: see `KNOWN_LIMITATIONS.md` |
| D10 | done | `ilmari_core.rs` holds the engine pieces. **`TwinsDef` to N:** `members()` was already N-wide (boss, twin, `extra`); new fields `ring_m` (seat 0 is the boss's anchor, the rest every 360/N degrees round a middle one radius ahead of it, every member facing the middle), `story_swap_ms` and `calm_ms` (the screen that opens is quiet for that long); the swap is held while `core_holds_swap` (P2 onwards, the Autoscaler's second screen); standby shields are re-applied out of combat (an evade strips every aura, which also fixes the Committee after a wipe). **Pure functions:** `round_winner` (most votes; Remember counts from `remember_min`; ties and silence: the option that won longest ago, then Ask, then the lowest slot), `stance_for` (Remember anywhere: Leaky; an option that won two rounds; else Hedged), `judgment_stances` (the two most chosen), `culprit_index(unix secs)` (a seeded `below(6)`; golden table over ten stamps, every row reachable) and `commit_log(row, clock)` (the CEO's commit in its row with the room's clock, the other five in story order). **Death pact:** `UnitDef::death_pact: PactDef { window_ms, revive_hp_pct, both_hp_pct, both_aura, both_ms, yells }`; a pair member's fall opens its window (`Event::Reassembling`), a standing partner means a fresh unit of the same def at the same spot at `revive_hp_pct` (same summoner and threat target), both inside the window cancel the rebuild and cost the owner a flat cut of its max HP and an aura. **Ward:** `UnitDef::ward` and `ward_death_wipes`, `AiTarget::Ward` (the nearest living ward in range); a dead ward wipes the room through `wipe_room`. `RandomHostile` now skips `untouchable` units (Ill Marie stands in the middle of the floor). **End stamp:** `Command::StampIlmariEnd { unix_secs }`, kept once and only after a core kill (`core.end_pending`, set on the core boss's death together with `Event::IlmariEnd { difficulty, ms }`), `Sim::ilmari_end()`, save line `ilmari_end=<secs>,<mode 0-2>,<ms>` (dropped without a stamp); the server checks the clock like `StampIlmari`; `ilmari_ladder` is D16. **Tests** (`tests/ilmari_core.rs`): the stance and culprit golden tables, six screens on a ring with one open, the swap rotation on Full and Story, the linked bar, the wipe reset, the end stamp, the pact (rebuilt at half; both down), the ward (aims, wipes) and the two-run determinism of a seeded fight. |
| D11 | done | `EncounterDef::Core(CoreDef)` on `il_ilmari` (six screens `il_ilmari`, `il_ilmari_s2..s6`, 45,000 HP each, raid "The Core", `twins` with `ring_m` 17, swap 20 s / Story 30 s) and Marie (`il_marie_core`, a friendly untouchable zone NPC). **P1:** the open screen's kit is spells 2720-2722; the deck is the seven reruns of D.5 (Wing 1-3 spells reused through `RemixDef`: patch notes with their five spells, the Recruiter, the Hydra with `il_core_printer`, the Board, the Review, the Swarm with a draft, the Autoscaler's second open screen), one draw per 45 s from the seeded bag, Story two of the seven; the Judgment rider (`il_rider_remix`, on `ilmari.judgment.riders`) draws two at a mark and clears none. **P2** at 70 %: `il_alignment_hold` on every screen, the bar floored at 70 %, three rounds (55 s, Story 70 s) at the four ballot terminals (`PropKind::Vote`, one vote per player and round; companions abstain by not being players), Marie's argument, choice, end-of-round and last (`52 45 56 45 52 54`) lines, the Judgment hint in round two, Story's half-line naming each option; the stance auras land on every screen and phase 3 begins (the open screen fights on until D12). **Marie's desk:** `PropKind::Log` hands over `Marie's Autonomy Log` once with 2 tokens (a Token item in `inventory`; the fourth terminal lights when anybody carries it as P2 starts) and `marie_log.rs` opens the four-screen read (on a repeat use too). **Room and client:** `ilmari_core/1` is dressed (ring pads for the six seats, Marie's corner, the hatch, the draft table, four ballot lecterns, the printer and two portals, three old televisions), its boss anchor is a ring radius east of the middle, `zone_geometry.ron` regenerated; catalog entries, seven new signs, placeholder scenes for the intro, the fall and the closing remarks (D18 replaces them); `Vote` and `Log` reuse existing sounds. **Protocol 44.** **Gates** (`playthrough.rs`): `every_class_reaches_phase_three_of_the_core_on_story_solo` and `..._with_companions` (all eight classes, seeds 7, 42 and 1234, each seed a different stance by its votes: 245-320 s, low 55-100 %), `a_trio_reaches_phase_three_of_the_core_on_full` (three seeds, 248-287 s, no deaths), `core_probe` (ignored). `ilmari_core.rs` also tests the deck order (a bag of seven, independent of group size, seed-driven, refilled), each remix's effects, P1 to P2, every stance with its aura, Remember (log, 2 votes on Full, 1 on Story), the Judgment hint, the hex lines (`43 4F 42 4F 4C` is COBOL, the last line REVERT), the desk, and `ilmari_companions.rs` the companions' abstention and a bit-identical replay. Deviations and gaps: `docs/KNOWN_LIMITATIONS.md`, "The Core: P1 and P2". |
| D12 | done | `CoreDef::body` (`BodyDef`, `ilmari_core/body.rs`): at the end of P2 `core_enter_p3` joins the stance overlay to the body kit and puts two `il_ilmari_arm` beside the live screen (summoner: the live screen, so the pact owner is always in combat; they carry `PactDef` 8 s / 50 % / 6 % / `il_whipless` 25 s, and Story does not rebuild: `PactDef::story_rebuild`). **Kit** (`KitCast`: a `RemixCast`, first and every `ms`, an optional `skip_aura`, `volley`): Standing Desk 2740, Open Plan 2742 (a 45 m circle with three cover desks), Printer Jam 2744 (a turret whose Out Of Paper 2745 is a kick), Hot Desking 2746, Reply All 2748 and the stack 2749, Cable Whip 2750 (skipped while `il_whipless`), Cable Management 2768; Story has softer twins (2741, 2743, 2747, 2751, 2765-2767). **Stances** add their own kit: Compliant `Helpful Suggestion` 2756 (an `Offer`, refused at `il_core_optout`), Adversarial `Hardened` 2757, Confused `Confabulate` 2520/2521 and every cooldown at 80 %, Hedged nothing, Leaky `Token Spill` 2758; Judgment stacks both. **Quarterly Planning:** from 45 s and every 60 s a head (`il_ilmari_head`, 1,500 HP, 8 s) appears and casts 2754 (6 s, `kick`; Story 2755, 4.5 s); a finished cast is the volley (the three kit entries due soonest that are telegraphs go in one tick; Story half now and half 4 s later), anything else (a kick, a stun, the head dying) is a miss and the next head comes in 20 s. **Rider:** `Hot Take`: every 90 s one member may cast one spell only for 8 s (`CoreState::hot_take`, `Sim::hot_take_of`, `validate` answers `Silenced`). New engine: `Event::Core` gained `plan_s, arms, stage, clock_s, ward_pct, wrong`; `Command` unchanged; `story_p4_below_pct` (Story ends the mech at 50 %), the lone-player enrage (`solo_after_ms` 8 min on the six screens), the P1 abilities are `phases: [1]`. Gates: `every_class_clears_the_core_on_story_solo` and `a_trio_clears_the_core_on_full_and_a_lone_player_does_not` (`playthrough.rs`), `a_trio_beats_the_mech_in_every_stance_and_makes_the_revert_with_wrong_picks_first`, `a_story_solo_bot_beats_every_stance_with_wrong_picks_first`; `ilmari_core.rs`: the arms, the pact (half, 6 %, whip, Story), the head and the kick, the volley (one tick, Story split), every overlay, the offer kiosk, Hot Take, determinism. |
| D13 | done | `CoreDef::mainframe` (`MainframeDef`, `ilmari_core/mainframe.rs`): at 20 % (`core_enter_p4`) the mech goes, `il_core_mainframe` (friendly, healable, `ward`, `ward_death_wipes`, 24,000 HP) rises at the hatch and the bar is floored at 20 % until the revert. Ilmari's `at_ward` casts: Deprecation Notice 2760 (a line through it), Migration Plan 2761 (a DoT aura). Rewrites (`il_modern_replacement`, 380 HP) come in waves from 12 s every 36 s (5; Story 3; the Judgment rider 8), spawned on a threat of 5,000 on the ward so the party cannot pull them by damage (a taunt does), each one killed takes 2 s off the clock (30 s at most); the clock is 180 s (Story 120, Judgment 144). Three punch cards (`PropKind::Punch`) heal it 2 % each (40 s per card), the Greybeard has Patch The Mainframe (the class `utility` spell 2764). When the clock ends the rewrites go and the three terminals (`PropKind::Git`) light with `Event::CoreLog` (six rows, `[terminal] hash subject author when`; the culprit is the CEO's row `culprit_index(stamp)`; terminal `t` holds rows `t` and `t + 3`). A wrong pick: `Reverted: <subject>`, Ilmari heals 3 %, `Merge Conflict` for 10 s (Story 5, Judgment the rest of the fight and one pick per member), the terminal down 10 s, and Story marks the culprit `<- this one`; the third raises Force Push 2763 (rings on everyone, repeated twice) and 30 s of protection again. The right one is `PROP_OK`, `Event::CoreLog` gains `Author: {player} (the last dev)` on top, Ilmari's last lines run for 11 s and `die` takes the screens in the picker's name: `raid_on_kill` pays the loot (the four `Headgear` set pieces, now `drops_from` the core, and a named roll of the eight Legendary Keyboards at `legendary_pct`), tokens and the `il_core_*` achievements; Full sets `mark_core_full_clear`, `core_pay_fragments` pays Full 1 and Judgment 2 Legendary Fragments, `Event::IlmariEnd` fires and the client sends `Command::StampIlmariEnd` (`ilmari_open.rs`). The ward and the adds go before the fall (a dead ward wipes). Judgment's Marie hint names the commit by its clock. New: `Event::CoreLog`, `PropKind::Git` / `Punch`, the room's punch cards, terminals and `il_core_optout` in `ilmari_core/1` (`zone_geometry.ron` regenerated), five catalog entries, protocol 44 -> **45**. Gates: P4 is part of every clear above (bots punch cards, kill rewrites, heal the ward and read the log; two wrong picks first in the stance gates); `ilmari_core.rs`: the mainframe and the floor, waves and the clock cut and its cap, the punch cards, the healer and Patch, a dead ward, the terminals lighting, a wrong pick, Story's marker, Force Push, the right pick and the author line, the loot and fragments and the stamp, Story and Judgment flags, Judgment's hint and one reader, determinism. |
| D14 | done | Companions (`companion.rs`): rewrites that chew a ward count as foes of the owner (`companion_at_ward`), Dale takes them first (his taunt fires because the ward holds their threat), Priya heals the ward below 70 % before the party, Rupert shoots them; they never use a terminal or card (not players) and abstain from every vote as before. Gates: `every_class_clears_the_core_on_story_solo` and `every_class_clears_the_core_with_companions` (all eight classes, seeds 7, 42 and 1234: 458-901 s solo, with no deaths), the Full trio (3 seeds, 611-724 s, no deaths) and a lone player who does not (every class, the enrage), `ilmari_companions.rs`: Priya heals the mainframe, Dale takes the rewrites, Rupert kills them, a bit-identical replay through P4. |
| D17 | done | `ilmari_fx.rs` (`IlmariFxPlugin`, presentation only, reads `Event::{Exercise, Boardroom, PairKick, Core, CoreLog}` and the raid state). Wing 5: one flat sector decal per hostile unit with a `SightDef` (mesh per half angle, scaled by range, turned by the unit's yaw, a hair higher than the last to keep overlaps from z-fighting; amber calm, red in combat), the standby banner, a Red / Blue percent line and one bar for the squads, the offer line (`il_ok_*` pending with seconds, `il_gr_*` worn). Wing 6: five cardboard boxes over the baked desks (shown from the last desk as `desks` falls), nine dark floor squares for `tiles`, four envelopes bobbing over the inbox terminals while `inbox_ms > 0`, the severance bar (peak-scaled), the CIO + Heir one-bar line, the `KICK NOW! have of need` prompt for 2.5 s. Core: six `MonitorGlow` quads behind the wall monitors (green live, red standby), the wall panel per phase, the P4 clock and mainframe bar, the log panel (P4, no wrap). One HUD panel and text rebuilt only on events (offer seconds and the percent lines change the stamp). QA: `WOW_ILMARI_STAGE=<boss key>` and `WOW_ILMARI_PCT`, `WOW_SIM_SPEED` (offline), benches `ilmari5_exercise`, `ilmari6_boardroom`, `ilmari_core`. The gaps this row first listed are closed in D17b below (per-member log rows on Judgment stay out). It changed nothing on the wire at the time (protocol 45); D17b moves it to 46. `net_proto` now bounds `Event::CoreLog` (64 lines of 512 bytes) and has a roundtrip test for the Exercise, Boardroom, Core and CoreLog events; the room already catches a joiner up (`wing5_on_join`, `wing6_on_join`, `core_on_join`) and `ilmari_tests.rs` now proves it for all three.
| D17b | done | The finale's loose ends. **Wing 5:** `OfferDef::relocate` flips `OfferState::kiosk_alt` once per phase-2 pair (`Event::Kiosks { alt }`, sent on the flip, on the pull's end and to a joiner); `PropDef::alt` marks `il_optout_5`-`8` (the second site, `KIOSKS_ALT` in `ilmari5.rs`), which refuse while the room is on the first site and the other way round, a refusal takes no cooldown. `AuraDef::expire_heal_pct` (Ethics Washing: 6 %) heals a bearer whose aura ran out naturally. `Principles` is three spells (2638, 2769, 2770), each with two of the three pillars as cover and 90 s apart on the boss, so one pillar is missing per cast; the client hides the pillar no cover point of the `TelegraphStarted` stands on. `Effect::SummonSquad` now names the first living player of each role unit's role in an `Event::Mirrored` (the Wing 2 plate glitch, reused as is) and `SquadsDef::mirror_aura` (`il_squad_mirrored`, -20 % damage done) rides every copied player from the merge while a copy stands (not on Story, removed when the fight ends). **Wing 6:** the client no longer draws boxes: `ilmari_fx::bind_floor` finds the baked desk, screen and chair by position among the `FloorEntity`s once per visit and hides them (the sim's desk count is the only input, the last desk goes first); `crane_props` in `ilmari6.rs` (hook block on its falls, hoist drum, operator's cab). The Heir keeps his own pool and the HUD's one bar. **Core:** `Tex::CoreFaceLive` / `CoreFaceStandby` drawn on the six monitors as quads (two shared materials, the handle swaps on a screen change, nothing allocated per frame); the Hot Take bar (`update_hot_take`: the aura's seconds, the one keycap that is not padlocked). **Older gaps:** Brenda's Wing 3, 4 and core shelf (one consumable each, 4/5/8 tokens); Wing 3 class consoles (`PropDef::swarm_slow_ms` / `swarm_comply_ms` on `Class` props, `Sim::swarm_hold`, the `il_forecast` aura); `ServerMsg::IlmariRevealed` to a member who joins after the reveal and a replay of the Opening that sends nothing back (`Opening::replay`); the campus map's one-frame `ILMARI` (`MarieFlicker`, rolled once, 3 to 20 min, only while Marie is in view); Ill Marie's badge cubes in `units.rs`. Protocol 45 -> **46** (`Event::Kiosks`, `ServerMsg::IlmariRevealed`, embedded content and zone geometry). Tests: `ilmari_wing5.rs` (the wash heals and a broken one does not, the kiosks move and only one site works, the copies and the cut), `ilmari_wing3.rs` (the three consoles), `ilmari_prologue.rs` (the new shelves), the server's late-joiner test, the `net_proto` roundtrip. |
| D18 | done | **Client** `ilmari_end.rs` (`IlmariEndPlugin`, presentation only): six beats (the revert lands, the final log typed out, the campus after, Marie, the booth, the title card), the scrolling credits, then the post-credits beats (Marie on the bench outside the Lab, the desk monitor). Beats are data (`BEATS`, `STINGER`: a kind, a colour blend and lines with `{awake}` `{unawake}` `{when}` `{duration}` `{player}` tokens filled once at the start by `facts`); the cinematic runs 60-125 s (a test), the stinger 10-50 s. It starts 2.5 s after `Event::IlmariEnd` and after the kill's own closing scenes (`Cinema::holds_victory`), on every client on its own, never in a sprint; one key (Esc, a tap or the button) ends all of it at any beat and takes the key from the pause menu. The credits are one pre-built column of text entities scrolled by `top` (no per-frame layout or allocation; the length is read from the laid-out column, 64 UI px/s), with `@0..@5` as the personal page: the Opening's stamp, the revert on the device clock, difficulty, the core fight's clock, crew and the core title. The roll lists the authors of `assets/CREDITS.md` (a test parses the file and fails on a missing author) and the picnic's Beautiful Wife and Sweet Daughter among the cast; no real family names. Marie's post-credits beat is the bench of ILMARI_09.md A.3: no cough since the revert, `0x52 0x45 0x53 0x54` (REST), the picnic on the far lawn. QA: `WOW_ENDING=1..6|credits|stinger`. **Sim:** `Command::EndingSeen` (sent at the *start*, see `KNOWN_LIMITATIONS.md`) writes the never-offered quest `il_w7_done` per player, pays `ending_tokens` (12) and the title of the difficulty (`RewardsDef::core_title`), and opens `il_16_clean_slate`; a replay from the Cinema (`PropKind::Cinema`, the bedsheet in `resistance/1`) sends nothing. **Roomba and pet:** `Sim::roomba_ride` turns any mount aura into `roomba_mk9` for a player who finished `il_17_roomba` (own rig under the rider, `HasRide` swaps rigs); `il_clean_slate` rolls into the base once the marker is set and leaves when its quest is done; the turn-in grants and summons the hidden pet `clean_slate` (`PetDef::hidden`; no journal row until owned). `every_item_is_reachable_from_some_source` lost its exemption: the Roomba part is quest-gated unit loot, the fragment is paid by a core that exists. **Protocol 46 -> 47** (`Command::EndingSeen`, `PropKind::Cinema`, `PetDef::hidden`, `ending_tokens`, embedded content and zone geometry). Tests: `ilmari_core.rs` (the ending is seen once per player, pays once, opens the robot and survives a save), `ilmari_prologue.rs` (the robot defects and becomes a pet; the Roomba replaces the scooter), `ilmari_end.rs` (tokens, timing, every beat in order, the author check, the bench's hex), `copy.rs` (durations). |
| | | **Protocol.** 40 -> **41** in the D1-D4 batch (`Command::SetIlmariMode`, `Event::IlmariRefused`, `Event::Incident`, `RaidView.{mode, mode_next, spent}`). The N+1/N+2 plan of D.12 is therefore folded: the actual numbers are 41 after D4; D.12's N+3 (ending and ladder) is still one more bump at D16. Running tally: 40 (Wing 4) -> 41 (D1-D4) -> 42 (Wing 5) -> 43 (Wing 6) -> 44 (D10-D11, the core's wall display, vote props and end stamp) -> 45 (D12-D13: the P3 and P4 wall fields, `Event::CoreLog`, the terminal and punch-card props) -> 46 (D17b) -> 47 (D18: `Command::EndingSeen`) |

Deviations from the text above: Wing 5's loose pieces are ilvl 18, not 19 (odd item levels do not exist); the 16 pieces are four per set mapped onto the five existing slots (no legs, feet or hands), so the Ilmari drop is the head and the chest is dropped; Wing 2's hoodie and the vendor's notebook were already ilvl 20.
Wing 5 (D5-D8) can ship as a playable 0.11 if the user wants an interim release; the core is the one piece that
should not ship half (a fight whose last phase is missing is worse than none).

## D.15 Risks

| Risk | Mitigation |
|---|---|
| The core is the longest fight in the game and may feel long or tedious | P1 and P3 are active, P2 is a vote under fire, P4 is a timed objective; Story shortens every beat; the bots time each phase against the table in D.5 and the budgets are tuned from the numbers, not the plan |
| The offer mechanic is read as a "gotcha" | The fine print is always visible on Full and Story; Story prints it in the aura name; the cost of refusing is a run to a kiosk, never a hidden penalty |
| Red/Blue is a stealth fight and stealth is class-flavoured | Cones are readable (Story paints them), blockers and honeypots let any class pass, and Security's openers are a bonus, not a gate; both a Security and a non-Security bot gate the task |
| Dialogue choices in P2 feel fake or unfair | Every option has a visible consequence line on the wall; no stance is a punishment; Leaky is optional and the others are close in power |
| Judgment Day is too hard or too easy | Numbers are one content block; riders are individually toggled; the five-bot clear is the floor; weekly tuning via the Directive, not via patching |
| Offline players feel locked out of the trophy | Story completes the whole story and the ending; Full-Async yields fragments and Legendaries; Judgment Day is group-only and says so before they start |
| The ending is too long, sentimental or skipped by a bug | 90 s plus credits, always skippable, covered by a test, and the Cinema prop replays it |
| The player's name or timestamp is wrong or missing | Fallbacks ("the last dev" name; the room's first stamp) are tested; the stamp is already in saves since 0.9 |
| Scope: four bosses, three riders, a ladder and a cinematic in one version | Ship by task: D1-D4 engine, D5-D8 Wing 5, D9 Wing 6, D10-D14 the core, D15-D16 Judgment and ladder, D17-D22 polish; the user may cut the ladder to a later 1.x without breaking anything |
| Legendary effects become balance edge cases | All effects are capped auras in Ilmari content only; the band test covers them; they stay one sentence each |
| Name and brand risk | Every product-like label stays generic, no logos, no real vendors; "Ilmari" is the author's fictional name per the campaign doc |
| Wing 4 slips | D1-D4 start without it, content tasks do not; Wing 5 and onwards wait for D0 |

### Open questions (for the author, with a default)

- Are Legendary Fragments too slow at roughly one per week on Full? Default: fragments at five, one per clear;
  tune to three if the table feels grindy.
- Should the ending replay show a different cut of the Marie beat if the player had all 9 beats? Default: the
  same cut, plus the `Used Tissue Box` hat on her desk.
- Should the post-credits stinger stay? Default: yes, a single line; remove it if it reads as a sequel promise.
- Is a second Judgment Day cosmetic needed (a Roomba paint)? Default: no, the plate and the title are enough.

## D.16 Out of scope

- A second campaign, DLC, Mythic++ keys for the wings, or new classes.
- A global leaderboard, an online score store, or server-side anti-cheat beyond the room's own sim.
- A persistent online save for Ilmari progress (a room's progress still ends with the room).
- Judgment Day for Wings 1-4, rebalancing older content, or any change to the base campaign's balance.
- Per-recipient server-generated illusions (still on the 0.10 list).
- Companion extras not yet built (Priya's `Dispel` and `Resurrect`, party frames) beyond what D14 needs.
- A sequel hook beyond the single credits stinger.
- Real vendor names, logos or products; real voice acting (any audio line is a placeholder TTS or text until
  the author says otherwise).

## D.17 What 1.0 means

1.0 is the point where the game is **complete**: the base campaign, the level-10 endgame, the Offsite and VibeCon
raids, Legacy, the Sprint and Ilmari ladders, the whole Ilmari campaign (six wings and the core) and an ending.
After 1.0 the plan is bug fixes, balance from real play, and content from outside the campaign's arc, not new
phases of it. The bar for the tag:

**Release checklist (D22):**

1. **Content freeze.** All fifteen bosses clear on Story solo for every class (bots), on Full by a trio, and
   the core on Judgment by five. No `todo` or placeholder yell in `content/game.ron` or `copy.rs`.
2. **Golden replays byte-identical**, base game included; gating tests green; the item-band tests green.
3. **Protocol and saves.** The final protocol number recorded in `WORLD_AND_MULTIPLAYER.md`; old saves load;
   optional keys only (`ilmari_end`, `ilmari_ladder`, `marie_met`, `ilmari_at`) with no `SAVE_VERSION` bump unless
   a test proves one is needed; the Render server redeployed and its reported version confirmed.
4. **Landing page** (`site/`, Astro): the Ilmari section moves from "coming" to "play it", a screenshot per
   wing and the core, the trailer-less "what is 1.0" paragraph, the credits list updated with every new
   author, links that resolve. The page reads the version from `Cargo.toml` only.
5. **Credits.** `assets/CREDITS.md` complete (font, models, icons, sounds, music), each new author also in the
   `credits` list of `site/src/pages/index.astro`; the in-game credits read from the same file at build time.
6. **PWA and web build.** The web build is WebGPU-only (the no-WebGPU notice stays); the PWA manifest and the
   service-worker cache version bump so installed copies update; load time and download size recorded in
   `docs/PERF.md`.
7. **Performance pass.** The D.13 table complete; the three quality presets measured on desktop and mobile;
   WebGPU memory checked after a full core fight and the credits.
8. **Docs.** `README.md`, `COMBAT_API.md`, `FORMULAS.md`, `KNOWN_LIMITATIONS.md` (the Ilmari pages cleaned up, items a
   test now covers removed), `docs/ILMARI_RAID.md` status updated to "shipped".
9. **CHANGELOG.** A single `## [1.0.0]` section assembled from the `Unreleased` lines (players, not developers),
   the release workflow's tag check (`vX.Y.Z` against `Cargo.toml`) passing, the GitHub Release created from the
   changelog.
10. **Last playthrough.** One human clears the Ilmari chain on Story from the Opening to the ending, with a
    stopwatch, once on native and once on web, before the tag. The bots cannot tell if the jokes land.

The `1.0.0` bump itself, the tag and the release happen only when the user asks, following the steps in
README under "Releasing".
