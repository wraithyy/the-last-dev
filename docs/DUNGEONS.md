# Dungeons: four runs for three to five, one for everybody

Design document for the level 3-10 dungeons. Status: the framework and dungeon 1, The Onboarding, are built
(substitutions for mechanics the engine lacks are in `docs/KNOWN_LIMITATIONS.md`, "Dungeons"); 2-4 are planned.

## Context

The raids (All-Hands Offsite, VibeCon; see `docs/FORMULAS.md`, "Raids") are a level-11 endgame. Between
the first week at work and the endgame there is nothing for a group to do together except quests, and
nothing a lone player can do "as a group" at all. Dungeons fill that gap: short instanced runs of three
to four rooms, three bosses, 15-20 minutes, for three to five players, which also play solo offline
through the same Async scaling the raids already use. Every dungeon is a workplace ritual turned into a
fight, and every boss is a joke first.

| # | Dungeon | Levels | Rooms | Bosses (last is the finale) |
|---|---|---|---|---|
| 1 | The Onboarding | 3-4 | 3 | Compliance Trainer, Laptop Provisioner, Onboarding Buddy |
| 2 | The Migration | 6-7 | 4 | Rollback Wraith, The Consultants, Big Bang Cutover |
| 3 | Quarterly Business Review | 9-10 | 4 | Slide Deck Golem, The Analyst, The Board |
| 4 | The Hackathon (heroic) | 10 | 4 | Pizza Overlord, Demo Day Gremlin, Pitch Judge VC |

Existing content this must not collide with: the minigame zones `migration/1` (Tool Migration) and
`hackathon/1` (the Innovation Space), so the dungeon zones are `onboarding/*`, `basement/*`, `qbr/*` and
`hack24/*`.

## Rules shared by all four

- **Group size.** Three to five. A Normal run needs at least `normal_min_players` (3) members at the zone
  load; with fewer it is Async, exactly like the raids. Solo offline is therefore always Async.
- **Async.** Enemy HP and damage are cut (numbers under Systems), the boss list and every mechanic stay.
  Each boss has a "solo-safe" note below: what the fight must still allow without a tank or a healer.
- **Boss loot lockout, daily.** One loot roll per boss per day and difficulty. A locked boss still spawns
  and can be fought (Kudos and reputation at the Async rate, no gear), so a group can always run it for the
  company, a friend's loot or the achievement. This is the opposite of the raids, where a locked boss does
  not spawn; dungeons are meant to be rerun.
- **Mechanic vocabulary.** Only what `content.rs` already has: `AiTarget` (Current, Caster,
  AllyLowestHp, RandomHostile), abilities with cooldown/initial/phase/shout, `PhaseDef.below_hp_pct`,
  `EnrageDef`, `FrontalShield`, `TelegraphDef` (shape, `delay_ms`, `split`, `count`, `step_m`; line shapes,
  rings), the effects Damage, Absorb, Interrupt, Heal, Aura, RandomAura, Dispel, Lockout, Knockback,
  Teleport, Summon, ResetAllyThreat, the PairUp tether and `TwinsDef`. No new effect is needed by any
  encounter below; the few places that need a small code change are marked "needs".
- **Classes stay the stars, never mandatory.** Each dungeon names a spot where a class prop or utility
  shines (an interrupt, a dispel, a stun that obeys diminishing returns, a DevOps rack, a Data Scientist
  dashboard, a Security terminal). Nothing is gated on them: anything a prop or utility does has a plain
  fallback (burn it down, wait it out, run out of the zone).
- **Readable at a glance.** At most three active mechanics per boss at once, one telegraph colour language
  (the raids' one), and every group-wide event is announced by a yell before it lands.

---

## 1. The Onboarding (levels 3-4)

*"Welcome aboard! Your first week has been scheduled. Your first week has also been rescheduled."*

**Theme.** The first week at a new job, as an instanced dungeon: a badge that does not work, a laptop that
does not exist, a mentor that is not a person. The mood is cheerful pastel dread, the most gentle run of
the four and the one that teaches a group how dungeons work.

**Entrance.** A narrow door beside the Welcome arch on the HQ plaza, sign "NEW STARTERS - WEEK 1 (door
may be locked, please wait for someone to let you in)". Open from level 3, so a player who has just
finished the Meeting Rooms floor walks straight in.

**Rooms** (`onboarding/1..3`, a boss in each):

1. **Reception and Badge Desk** (boss: Compliance Trainer). Turnstile gates that beep at everything,
   a "Welcome, [NAME]" screen that shows the previous hire's name, a visitor sofa with a fern that has
   been dead since 2021. Floor decals of slow-motion arrows labelled "Please queue here" that lead to a
   wall.
2. **Equipment Cage** (boss: Laptop Provisioner). Floor-to-ceiling shelving with exactly one laptop
   per ten empty boxes, a calendar on the wall with "Week 6" circled and then crossed out and re-circled,
   a kettle bolted to the counter, a ticket printer that prints tickets for itself.
3. **Your Desk** (boss: Onboarding Buddy). An open-plan corner with a nameplate reading "[NEW STARTER]"
   in marker, a monitor taped to the wall, a welcome basket (lanyard, one pen, a "Culture Deck" with a
   post-it saying "ignore page 4"), a cheerful chat widget in the corner that blinks when you look away.

**Trash** (authored level 3, packs of 2-3, 2 packs per room):

| Unit | Title | Mechanic |
|---|---|---|
| Badge Scanner | Beeps Wrong Every Time | Melee; Interrupt-able "Access Denied" cast (short stun on the target if it lands) |
| Welcome Packet Dispenser | Dispatches Swag | Ranged; throws a lanyard (low Damage, Aura "Tangled", brief slow) |
| HR Generalist | Circles Back | Heals the lowest-HP ally (`AllyLowestHp`) with a cast; prime interrupt target |
| Forgotten Probation Intern | Day 89 of 90 | Fixates the caster (`AiTarget::Caster`), weak; its death yell is "Tell them I was great" |
| Org Chart | Forever Updating | Summons two Name Tags (1 HP, die on hit) every 12 s; they explode for tiny Damage if left |

**Class moments.** An interrupt on the HR Generalist and the Compliance Trainer's disclaimer. A Security
Engineer can pick the Badge Desk turnstile with a terminal prop and skip one pack (stealth is the class
fantasy, the pack is merely optional).

### Boss 1: Compliance Trainer, "Certified, Annually"

HP role: standard boss, the tutorial for "stand out of the circles". Authored level 4.

- **Pop Quiz.** A question is shouted, four lettered floor tiles (A-D) appear in a row, and three of them
  are covered by Line/ring telegraphs (`count` 3 with `step_m` spacing, `delay_ms` 3000); the one that is
  left uncovered is "correct" and safe. Every question is a pun that tells you the answer if you read it.
  Standing in a wrong tile eats a hit and Aura "Retake Required" (small damage taken).
- **Click Next.** A tank hit on `Current` every 6 s; a `FrontalShield` ("Laminated Handbook") means the
  boss is hit from the side or behind.
- **Read the Disclaimer.** A long, Interrupt-able cast every 25 s. Landing it applies a short Aura
  "Muted" (a silence-like debuff, 4 s) to everyone. An interrupt from any class prevents it.
- **Phase 50%.** Summons two Attendance Sheets (adds that must be "signed", i.e. killed) and the quiz
  tiles come every 12 s instead of 18 s. **Enrage** (`EnrageDef`) at 4 minutes: "Session timed out. You
  must restart the module."

**Solo-safe.** The quiz is dodged by movement, not by a role; the handbook shield faces the target and
a lone player simply stays at its side; Muted is annoying, never lethal.

**Yells**
- "Welcome to Mandatory Training. Attendance is optional. Not attending is noted."
- "Question one: you find a USB stick in the car park. Do you A, plug it in; B, plug it in faster; C, give
  it to the Security Engineer; or D, plug it into your *work* laptop?"
- "Wrong! The correct answer was always C. We only teach the correct answers during the test."
- "Slide 41 of 40. Please do not skip. I can see you skipping."
- "Congratulations, you have been certified. Certification expires in thirty seconds."

**Loot.** *Lanyard of Verified Attendance* (neck; a bit more of everything, flavour: "Shows you were
here"), *Handbook, Laminated* (off-hand tank piece; flavour: blocks on paper only), *Certificate of
Completion* (trinket; flavour: Kudos find, "worth exactly what you paid").

### Boss 2: Laptop Provisioner, "INC-0042 (Awaiting Hardware)"

HP role: attrition fight with a clock; the six phases are the six weeks. Authored level 4.

- **Six weeks.** `PhaseDef` at 83/66/50/33/16%, each with a "Week N" shout. Each phase swaps one
  ability in and keeps the rest, so the fight changes without growing.
- **Standard Issue Peripheral.** `RandomAura` on a `RandomHostile` every 10 s: a wireless mouse (no dongle,
  slow), a Dvorak keyboard (reduced damage dealt) or a docking station (hot: a mild DoT).
- **Out For Delivery.** Teleport the boss to a far edge of the cage; a Line telegraph from the delivery van
  shows its charge lane back (`delay_ms` 2000), hitting anyone in the lane.
- **Reimage.** `Dispel` on the target's buffs; also summons one Courier Drone (add) that heals the boss with
  an Interrupt-able "Escalate to Tier 2" (`Heal` on `AllyLowestHp`).
- **Week 6, 16%.** "Your laptop has arrived!" The boss's damage steps up (`EnrageDef` style at phase), the
  Teleport stops, and the box it ships in explodes with a ring telegraph when it dies.

**Solo-safe.** All hits are avoidable by movement or modest in damage, the Dispel only strips buffs the
player can recast, and the Heal needs an interrupt or a burst, so a lone DPS can burn the drone.

**Yells**
- "I see your ticket. It's in the queue. The queue is in the other queue."
- "Week two. Your laptop is on a boat. The boat is on a waiting list."
- "It's ready! It's... it's the wrong model. We'll order the right one. Week six."
- "Have you tried turning yourself off and on again?"
- "Ticket closed. No further action. Reason: you stopped replying."

**Loot.** *Refurbished Headset (Mic Never Tested)* (head; flavour: crit, but only on calls), *Borrowed
Charger* (trinket; flavour: regenerates a bit of mana at 20% battery), *Docking Station, Hot* (ring;
flavour: haste and a gentle burn).

### Boss 3 (finale): Onboarding Buddy, "Your Point of Contact"

HP role: finale, the reveal is the phase change. Authored level 4.

- **Phase 1, 100-60%: friendly.** "Let Me Show You Around": Teleport on a `RandomHostile` to a far corner
  (a harmless walk, the joke is the commentary), and "Great Question": a quiet `Heal` on self that is
  Interrupt-able.
- **Phase 2, 60%: the reveal.** The face panel flips to a chat bubble. New abilities: "Generating
  Response" (long Interrupt-able cast, big Damage on `Current` if it finishes), "Confidently Wrong"
  (`Absorb` on self, burned down by anyone), and "Let Me Loop In Someone Else" (`ResetAllyThreat`: the
  tank loses aggro, the boss picks someone new).
- **Phase 3, 30%.** `EnrageDef` soft: "Context window full". The boss hits harder and the Teleport from
  phase 1 returns, now on a tight cadence, so players are walked around the room while the clock runs.

**Solo-safe.** `ResetAllyThreat` does nothing with one target; the Absorb is a burst check, not a role
check; "Generating Response" has a 3 s cast with a visible bar, so a lone player can also dodge by
using the room's pillars (line of sight).

**Yells**
- "Hi! I'm your Onboarding Buddy! I've been here for years! Please don't ask how long."
- "Great question! Let me answer a different question, much more confidently."
- "I don't actually *have* feelings, but I am *thrilled* to help you. Let me know if you'd like me to
  rephrase that as a bulleted list."
- "As a Buddy language model, I cannot show you where the kitchen is, but here is a poem about a kitchen."
- "I have reached the end of my context. Please start a new onboarding."

**Loot.** *Mentor Mug ("World's Okayest Mentor")* (trinket; flavour: Kudos find), *Chat Widget, Always
Online* (off-hand caster piece; flavour: casts a little faster, answers a little wrong), *Sticky Note: Ignore
Page 4* (ring; flavour: spell power, tiny +reputation with HR).

**Cosmetic: The Fern.** The dead fern from Reception, a pet that stands beside you and slowly sags;
watering it (the emote key) revives it for a moment, then it sags again.

**Music.** Muzak elevator jazz with a soft pad under it that detunes a little in each room; the finale
keeps the same tune but a chatbot-ish arpeggio plays on top of it, perfect and joyless.

---

## 2. The Migration (levels 6-7)

*"Cutover is scheduled for Friday at 18:00. Nobody asked which Friday."*

**Theme.** The lift-and-shift of a thirty-year-old system into "the cloud", fought in the basement server
room behind the Data Center. A legacy-to-cloud migration run by consultants, with no rollback plan.
Cold, humming, cable-drenched; the mood is deadline horror with a corporate smile.

**Entrance.** A loading-dock door on the back of the Data Center, marked "Basement B: Authorised Persons
Only (also everyone who has a ticket)". Open from level 6.

**Rooms** (`basement/1..4`, bosses in rooms 2-4; the first room is a trash gauntlet that sets the scene):

1. **Loading Dock.** Pallets of "legacy" in shrink wrap, a forklift stuck mid-turn, a whiteboard with a
   migration plan that has 14 phases and the last is just "???", a banner "WE ARE GOING TO THE CLOUD" over a
   bucket under a leak.
2. **Cold Aisle** (boss: Rollback Wraith). Racks in a long aisle with a mist that never clears, blinking
   LEDs in the pattern of a heartbeat, and the ghostly outline of a previous release hovering above every
   rack.
3. **Mainframe Cage** (boss: The Consultants). A glass-walled meeting pod in a cage, with a whiteboard
   rate card and two designer chairs; all the cables are labelled and the labels are wrong.
4. **Cutover War Room** (boss: Big Bang Cutover). A countdown clock the size of a billboard, 14 empty
   energy drink cans on each desk, a huge red "GO" button under a plastic cover that someone has taped
   shut and then un-taped.

**Trash** (authored level 6):

| Unit | Title | Mechanic |
|---|---|---|
| Orphaned Process | Still Running, Nobody Knows Why | Respawns once ("Zombie") 5 s after death unless killed last in the pack |
| Cable Spaghetti | Do Not Touch | Slow melee; Aura "Tangled" (slow) on its target |
| Legacy Admin | Last Person Who Knew | Casts "Why Would You Change That" (Interrupt-able Heal on `AllyLowestHp`) |
| Migration Script | v0.1, Untested | Charges along a Line telegraph and Teleports to its target, then dies ("exit 0") |
| Tape Robot | Backup, Technically | Knockback on a ring when it finishes a long cast ("Restore From Tape") |

**Class moments.** A DevOps rack in room 2 can "snapshot" the group (flavour; a prop buff), a COBOL
Greybeard is the one person who reads the mainframe cage labels (extra loot tooltip, a joke), and a
Backend tank holds the Wraith's Teleport away better than anyone; none is required.

### Boss 1: Rollback Wraith, "Ghost of the Previous Release"

HP role: standard boss with a rubber-band; it undoes progress. Authored level 7.

- **Rollback to v1.9.** A cast every 20 s that `Heal`s the boss for 8% if it lands; Interrupt-able by any
  class and burnable in the 5 s window if not.
- **Revert.** `Dispel` on one `RandomHostile` (strips buffs) plus a `Teleport` that puts them back at the
  entrance of the aisle ("Reverted position").
- **Phase 60%: Orphaned Processes.** Summons 2 adds every 15 s (max 3 alive) that must be killed or they
  heal the Wraith on death; **Phase 30%: Hotfix Hell**, ring telegraphs ("Undo") appear in a row
  (`count` 3, `step_m`, `delay_ms` 1800), each must be sidestepped.

**Solo-safe.** The 8% heal is a DPS check, not a role check; the Revert Teleport never lands on a hazard;
the adds are capped so a lone caster is never overrun.

**Yells**
- "Version 1.9. The one that worked. Why did you touch it?"
- "I remember when this was stable. We had no users. It was a golden age."
- "Rollback in progress. 3%... 3%... 3%..."
- "You merged on a Friday. I was *created* on a Friday."
- "Hotfix? There is no hotfix. There is only a different mistake, but faster."

**Loot.** *Release Notes Cloak* (back; flavour: "Fixed some bugs", +hp), *Snapshot Ring (Restore Point)*
(ring; flavour: on a lethal hit, a small heal; long cooldown), *Git Blame Dagger* (weapon; flavour: crit
against whoever hit you last).

### Boss 2: The Consultants, "Billable, Respectively"

HP role: twin fight reusing the raid's `TwinsDef` (linked bosses, shield aura, swap). Authored level 7.

- **The pair.** Jolene Utilization (Senior Manager) and Jack Rate-Card (Engagement Partner). One is open
  at a time; the other wears the shield aura "Billable Hours" (an `Absorb`, so hitting it does nothing).
  They swap every ~20 s. When one dies, the other enrages (shorter swap, more damage).
- **Statement of Work.** The open twin summons two Junior Analysts (adds) that cast "Slides, by EOD".
- **Scope Change Request.** `RandomAura` on a `RandomHostile`: either more damage taken, a slow, or a
  damage-dealt debuff, each with a different funny name.
- **Gantt Chart.** Line telegraphs laid out as bars across the floor, sliding right with `step_m`, each
  with a `delay_ms` so players can see the next bar coming; lethal to stand on.

**Solo-safe.** The swap timing is generous in Async (one target at a time anyway), the shield falls when
the open twin is hit, and the lane telegraphs are long enough for any class to cross.

**Yells**
- Jolene: "We've reviewed your current state and found it to be a state."
- Jack: "This is a three-month discovery phase. Discovery is billable. Everything is discovery."
- Jolene: "Per my last email. Which I have not sent yet."
- Jack: "That's out of scope. This is in scope. That is out of scope again, *now*."
- Both: "We'd be happy to take over. Invoice to follow. And follow. And follow."

**Loot.** *Rate Card Cufflinks* (trinket; flavour: Kudos find, "x3 for rush"), *Discovery Phase Notebook*
(off-hand; flavour: a slow start, then spell power), *Engagement Letter Pen* (weapon; flavour: it signs
you up for another round, a small extra hit).

### Boss 3 (finale): Big Bang Cutover, "Go-Live Weekend"

HP role: finale, three phases like a weekend. Authored level 7.

- **Phase 1, Friday, 100-70%: Freeze Window.** Ring telegraphs spread outward from the boss; "Change
  Freeze" `Absorb`s the boss until adds are down.
- **Phase 2, Saturday, 70-40%: Data Sync.** A `PairUp` tether links two players with a beam; breaking
  the tether (moving far apart) hurts both, standing together is safe; a ring grows underneath. In a
  group of one, the tether links the player to a summoned Orphaned Record (add) instead.
- **Phase 3, Sunday, 40-0%: Hypercare.** A Line telegraph wave sweeps the room with Knockback
  ("Traffic Switchover"); `EnrageDef` as a ramp: the boss gains a little damage every 10 s ("It's Monday
  in 3 hours").

**Solo-safe.** The tether has a solo fallback (needs: the tether code must accept an add as the second
end); phase 3's ramp is slow enough for a lone DPS in Async, and all lines are readable well ahead.

**Yells**
- "T minus 10. Everything is fine. I checked. I am the thing that is fine."
- "There is no rollback plan. There is only forward. Forward is the plan."
- "Data sync at 99%. Data sync at 99%. Data sync at 99%."
- "We're live! Please do not look at the dashboard. The dashboard is also live."
- "Hypercare! That means everyone stays and stares at the same screen until Tuesday."

**Loot.** *Go-Live Hoodie* (chest; flavour: grants a little extra stamina, smells of cold pizza),
*Big Red Button (Taped Shut)* (trinket; flavour: on use, a burst, then a stern email), *Runbook, Page
Missing* (off-hand; flavour: spell power and a tooltip that cuts off mid-sentence).

**Cosmetic: Hypercare Cape.** A cape shaped like a progress bar that never reaches 100% and lights up
red on Monday.

**Music.** Low fan-hum drones and a four-on-the-floor kick that sounds like a server rack booting; the
Consultants get a smooth jazz sax, and the finale runs a ticking clock under a synth that gets a little
faster at every phase.

---

## 3. Quarterly Business Review (levels 9-10)

*"Let's align on what we've aligned on."*

**Theme.** The boardroom floor on quarter-end, a dungeon built from slide transitions, bar charts and
buzzwords. This is the satire-heavy run: executives, analysts and a deck. The mood is a hotel conference
lobby at 7am, with free pastries nobody touches.

**Entrance.** The tower lobby lift gets a new button between the Penthouse and the roof, "5½: QBR (by
invitation only)". Open from level 9, group invite sent via the Company Calendar ("Mandatory: Q3 Review,
all attendees").

**Rooms** (`qbr/1..4`, bosses in rooms 2-4):

1. **Executive Lobby and Catering.** Sad croissants under a heat lamp, a "Welcome, Valued Stakeholders"
   screen that scrolls, a coffee station with four brands and no coffee, a man asleep in a beanbag
   labelled "Visionary".
2. **Presentation Theatre** (boss: Slide Deck Golem). A cinema-style room with a screen as wide as the wall,
   a slide that says "Agenda" and under it the same word again, a laser pointer on a stand, a clicker
   with no batteries.
3. **Analytics War Room** (boss: The Analyst). Walls of dashboards with all the lines going up and to
   the right, a "Single Source of Truth" sign over three conflicting spreadsheets, a printer printing
   charts nobody asked for.
4. **The Boardroom** (boss: The Board). A table the length of a bowling lane, leather chairs that look
   alive, a single glass of water per seat, portraits of previous CEOs that blink at you.

**Trash** (authored level 9):

| Unit | Title | Mechanic |
|---|---|---|
| Stakeholder | Needs Visibility | Aura "Visibility" (marks a target; the Stakeholder repeatedly pings it for small Damage) |
| Strategy Intern | Made The Deck | Throws slides: Line telegraph, light Damage, Knockback on the end bar |
| Synergy Coordinator | Aligned On Alignment | Heals allies; Interrupt-able; leaves a ring buff "Alignment" on death |
| KPI Dashboard | Always Green | `FrontalShield` of "Green"; a flank hit turns it red and it soon explodes |
| Catering Drone | Free Pastry | Spawns pastry adds; a pastry heals the player who picks it up (a gift, mostly) |

**Class moments.** A Data Scientist dashboard prop in room 3 shows which "metric" is the fake one (the
`RandomAura` picks are labelled for them); a DevOps rack in the Boardroom resets a head's fixation; a
Fullstack caster can fry the Golem's slides from range. Nothing is required.

### Boss 1: Slide Deck Golem, "Deck_v37_FINAL_final2.pptx"

HP role: add-heavy. It summons adds "per slide", and its slides are its phases. Authored level 10.

- **Next Slide.** Every phase change (`below_hp_pct` 80/60/40/20) summons 2 Bullet Point adds (max 4 alive)
  and changes the boss's backdrop; each slide has a name and its own twist.
- **Fly In From Left.** A Line telegraph that sweeps from the left edge, Knockback on contact.
- **Animated Pie Chart.** A ring telegraph with `split` (a pie of wedges, one wedge safe and clearly
  highlighted) and a `delay_ms` of 2500.
- **Speaker Notes.** `Absorb` on the boss, burnable by anyone; **Appendix (10%)**: the boss keeps adding
  tiny slides that deal tiny Damage ("Appendix B, page 112 of 200"); a slow enrage.

**Solo-safe.** Adds are capped at 4, killable by AoE or single target; the pie always has a clearly
safe wedge; the Appendix ramp is slow.

**Yells**
- "Slide one: the agenda. Slide two: the agenda, but in a different font."
- "Next slide. Next slide. NEXT SLIDE. The clicker is broken. We are stuck here."
- "This chart shows growth. Please ignore the axis. Please ignore the *y*-axis especially."
- "Let me just share my screen. Can you see my screen? Can you see my screen?"
- "Thank you. Questions? No? Good. Appendix."

**Loot.** *Animated Transition Boots* (feet; flavour: faster after being knocked back), *Laser Pointer,
Red Dot* (weapon/ranged; flavour: bonus crit against the marked target), *Master Slide Shield* (off-hand;
flavour: block, every attack is also a bullet point).

### Boss 2: The Analyst, "Dana, Who Had Claude Do The Analysis"

HP role: a boss that cannot explain her own work; a mix of random auras and adds. Authored level 10.

- **Insight.** Hits a `RandomHostile` with a big spell ("Leveraging Insights"); an Interrupt-able cast
  that is the main danger.
- **Hallucinated Metric.** `RandomAura` on the boss (a buff such as "Up 340% YoY") or on a player (a
  debuff such as "Down 8%, Seasonally Adjusted"); a Dispel clears it, or it fades in 12 s. An authored
  list of names so the tooltip is the joke.
- **Source: Trust Me.** `Absorb` on herself that must be broken; **Please Rerun The Analysis**:
  `ResetAllyThreat` (the boss loses her target and picks anew).
- **Phase 50%.** Summons two Generated Charts (adds) that emit Line telegraphs with swapped axes: the
  danger zones are drawn where the safe zones look like they are.

**Solo-safe.** The Insight cast has a visible bar and can be dodged by line of sight; Hallucinated
Metric debuffs are mild and fade; the Charts are capped at two.

**Yells**
- "I did a deep analysis. Well, I asked it to be deep. It said it was."
- "The data clearly shows that... let me check what the data clearly shows."
- "This number? It's from a source. The source is confident."
- "Great catch! You are absolutely right! I will fix that immediately! Here is the same number again."
- "I can't explain the methodology, but the *vibe* of the methodology is excellent."

**Loot.** *Pivot Table of Power* (ring; flavour: spell power, scrambles its own tooltip), *"Source: Trust Me"
Footnote* (trinket; flavour: its proc cannot be wrong, and it is not always right), *Confidence
Interval Bracers* (wrists; flavour: crit with a tooltip that says "+- 100%").

### Boss 3 (finale): The Board, "A Quorum of Three"

HP role: multi-headed finale, three linked bosses with one open at a time (the VibeCon Hype Panel's
pattern, `TwinsDef`-style link). Authored level 10.

- **The Chair** (tank head): hits `Current`, calls the vote with "Motion on the Floor" (a ring telegraph
  under each player; two of three rings are safe).
- **The Activist Investor** (damage head): fixates a `RandomHostile` with an Aura "Shareholder Value";
  `Knockback` on "Hostile Takeover".
- **The Independent Director** (heal head): does nothing for 20 s, then a big Interrupt-able `Heal` and an
  Aura "Abstain" on the whole group (a mild slow). If left alone she silently heals the rest.
- **Vote.** Every 45 s all three speak; a Line telegraph sweeps the room with one head's colour. After
  a head dies the others gain damage ("Quorum lost"). At 20% total, `EnrageDef`: "Motion to dissolve
  Engineering."

**Solo-safe.** Heads are open one at a time in Async, so a lone player faces one at a time, in a
clear order; the vote lines are readable; there is no mechanic that requires two players.

**Yells**
- The Chair: "Item one on the agenda: the agenda. All in favour? Opposed? Noted, no one is opposed."
- Activist: "I'm not here to be liked. I'm here to improve the multiple."
- Independent: "I'm independent. I just happen to have the same golf partner."
- Chair: "Let's take this offline. By offline I mean: out of the minutes."
- All: "We've decided to be AI-first. Which AI? The one that rhymes with 'pay us'."

**Loot.** *Chairman's Gavel* (weapon; flavour: tank, heavy and louder than it needs to be), *Proxy Vote
Ring* (ring; flavour: copies the best buff in the group for a short time), *Golden Parachute* (back;
flavour: once per day, survives a lethal hit and leaves the room, heavily criticised).

**Cosmetic: Executive Chair Mount.** A swivel chair mount that spins slowly, plays a soft purr of
leather, and refuses to go downstairs.

**Music.** Corporate lobby ambient with an orchestral hit on every "slide"; the Boardroom is a slow
string quartet in a minor key, interrupted by a ringing phone (the finale tempo picks up with each head
that speaks).

---

## 4. The Hackathon (level 10, heroic)

*"24 hours. Unlimited pizza. Limited sleep. Prizes for the best use of AI."*

**Theme.** An overnight 24-hour hackathon in a converted warehouse: sleeping bags under desks, laptops on
laps, bean bags, energy drinks, and a banner "AI-FIRST". The heroic dungeon: harder, shorter rooms,
nastier mechanics, all running on a night clock. The mood is neon dread, a bad idea at 3am sounds like
a great idea.

**Entrance.** The container booths on Incubator Street, a door in the back of the coworking cafe marked
"HACK 24: Registration (Free Pizza)". Open at level 10 once the Quarterly Business Review has been run
once (any difficulty). Heroic only; there is no Normal version of this dungeon.

**Rooms** (`hack24/1..4`, bosses in rooms 1, 3 and 4; room 2 is the trash floor):

1. **Registration and Snack Wall** (boss: Pizza Overlord). A tower of pizza boxes, a drinks fridge
   with no cold drinks, a swag table with one XXXL t-shirt left, "Team Name: ???" sign-up sheet with
   seventeen teams called "Team AI".
2. **The Hack Floor.** Desks pushed together, sleeping bags, whiteboards with architecture diagrams
   that are simply a box labelled "AI", a mentor corner with a bored mentor wearing a "ASK ME" shirt.
3. **Demo Stage** (boss: Demo Day Gremlin). A stage with a projector, a mirrored display that shows the
   wrong screen, a speaker that squeals, a "Judges" table with a bell and a single banana.
4. **The Pitch Room** (boss: Pitch Judge VC). A glass room with a fish tank, a sofa, a "Series Z
   Ventures" banner and a clock that has only the number 10.

**Trash** (authored level 10):

| Unit | Title | Mechanic |
|---|---|---|
| Sleep-Deprived Developer | Day 2 | Casts Interrupt-able "Commit Without Reading" (big Damage to a random ally, including its own) |
| Prompt Engineer | Types Words At A Chatbot | Summons a Hallucination (add) that casts one wrong thing per 8 s |
| Energy Drink Pyramid | 47 Cans | Fixed; explodes (ring telegraph) when killed or after 20 s |
| Lead Mentor | Giving Feedback | Aura "Constructive" (a mild damage-taken debuff) and a stun-able charge |
| Team AI | All Seventeen Of Them | A swarm of 3 small units sharing the same name and the same startup idea |

**Class moments.** Interrupts on the Developers and the Gremlin's setup casts; a Security terminal on
the Hack Floor can "phish" a pack into attacking each other (a stun-style prop, subject to diminishing
returns); a Backend tank holds the pizza adds. Nothing required.

### Boss 1: Pizza Overlord, "Free Pizza Is Never Free"

HP role: heroic tank-and-heal fight; an add check. Authored level 11.

- **Hot Slice.** A ring telegraph with `split` (pizza wedges), most covered, one safe; shown, `delay_ms`
  2200, and repeated three times in a row (`count` 3).
- **Dietary Requirements.** `RandomAura` on a `RandomHostile`: vegan (cannot be healed for 6 s), gluten
  free (slow) or "I'll Just Have Whatever's Left" (reduced damage). A Dispel clears it.
- **Extra Cheese.** `Absorb` on the boss; the Slices (adds, 6 s) heal it for 2% on death.
- **Phase 50%: Cold Pizza at 3 AM.** The ring cadence tightens and a Line telegraph rolls in from the
  oven: "The Last Slice". **Enrage** at 5 min: "Who took the last slice?"

**Solo-safe.** The vegan debuff is a mild shaving of healing, not a wipe; Slices are slow; the wedge
rings always leave one clear safe sector.

**Yells**
- "Free pizza! Hurry, only the pineapple one is left. It was always only the pineapple one."
- "Nobody leaves until the boxes are empty. The boxes were empty an hour ago."
- "It's 3 AM. You've been eating cold pizza for nine hours. This is the *good* part of the hackathon."
- "Who took the last slice? Who took the last slice. WHO TOOK THE LAST SLICE."
- "I've been delivering since 2019. I'm very proud of what we shipped, and by 'shipped' I mean 'delivered,
  slightly cold.'"

**Loot.** *Pizza Cutter (Wheel Of Fate)* (weapon; flavour: crit with a satisfying sound), *Grease-Stained
Hackathon Tee* (chest; flavour: stamina, and a small damage reduction from fire), *Cold Slice Charm*
(trinket; flavour: on use, a heal over time that gets better after a minute).

### Boss 2: Demo Day Gremlin, "It Worked On My Machine"

HP role: chaos boss; "everything breaks during the demo" is a random effect each cycle. Authored level 11.

- **Live Demo.** Every 15 s the Gremlin announces a "demo step" and a `RandomAura` hits the whole group:
  "Wi-Fi Dropped" (slow), "Projector Mirrored" (damage taken up), "Wrong Tab" (a `Lockout` on one school
  for 4 s, never all of them).
- **Alt-Tab.** `Teleport` of a `RandomHostile` to a distant edge of the stage, with a Line telegraph at
  the destination to show where they will land.
- **Hard Refresh.** `Dispel` of the group's buffs plus a ring telegraph with a 3 s delay.
- **Phase 40%: Recording.** The Gremlin summons the audience (adds that heckle and cast Interrupt-able
  "Boo"); the Live Demo cadence doubles.

**Solo-safe.** The Lockout always leaves at least one school untouched; the Teleports are short
and telegraphed; the heckling adds are weak.

**Yells**
- "Let me just pull it up. It was working a second ago. It was working at lunch!"
- "That's weird. I didn't change anything. Well, one thing. Okay, nine things."
- "We'll just skip to the end. The end isn't here either."
- "As you can see, the model is *very* sure of this answer. The answer is a picture of a hotdog."
- "Technical difficulties are part of the live experience. We'll share the recording. There is no recording."

**Loot.** *Wi-Fi Dongle (Unpaired)* (trinket; flavour: haste, but you have to stand still to pair it),
*Projector Remote (Wrong Slide)* (weapon; flavour: on hit, a small chance to hit the wrong target and
crit anyway), *Backup Demo Video* (cloak; flavour: first death each run is "pre-recorded", a free
resurrect that is clearly staged).

### Boss 3 (finale): Pitch Judge VC, "Only Funds AI"

HP role: heroic finale, three phases named after funding rounds. Authored level 11.

- **Phase 1, Seed, 100-66%.** "Is There AI In This?": `Absorb` on the boss that falls only if the
  group "adds AI", i.e. a `RandomAura` "Powered By AI" is placed on every player (more damage dealt, more
  damage taken); a Dispel removes it. The boss `Teleport`s a `RandomHostile` away ("Next meeting").
- **Phase 2, Series A, 66-33%.** Summons Associates (adds) that cast Interrupt-able "Synergy"
  (`Heal` on `AllyLowestHp`); **Valuation** stacks an Aura on the boss (growing damage) every 12 s, which
  a Dispel resets. `ResetAllyThreat`: "Not a fit".
- **Phase 3, Unicorn, 33-0%.** `EnrageDef`: "Valuation hits one billion". Line telegraphs fire across the
  room like a term sheet; a ring under the boss means a Knockback from "Down Round".

**Solo-safe.** "Powered By AI" is a buff-with-cost, not a requirement; Valuation grows slowly in Async
and the Associates are killable; the finale is long enough to be survivable without a healer.

**Yells**
- "We invest only in AI. What does your product do? Never mind, put 'AI' in front of it."
- "You're telling me your product has *no* AI? What if I told you it does? Now I've told you."
- "Our thesis is simple: the AI will pay for itself. It already did. We're not telling you how."
- "I loved the pitch. I understood none of it. That's how I know it's the future."
- "We're going to pass. But we're passing *with conviction*. Please add 'AI' to the rejection."

**Loot.** *Term Sheet (Redlined)* (weapon; flavour: heavy hit, slow, every line has a footnote), *Fleece
Vest, Series Z* (chest; flavour: warmth: reduced damage from cold, slightly confusing to the team),
*Unicorn Horn Pin* (trinket; flavour: on use, doubles your own damage for a short time then halves it).

**Cosmetic: The AI Hat.** A hat that says "AI" in glowing letters; wearing it changes every NPC's greeting
to a pitch ("Is it AI?"), and nobody can tell what is behind it.

**Music.** A pulsing 3 AM synth, slow, with an ambient crowd murmur and the odd typing; the finale adds a
motivational-speaker arpeggio that does not quite resolve.

**How the heroic Hackathon differs.** See Systems below.

---

## Quests

Two to four quests per dungeon, all in the existing vocabulary (`Objective::Kill`, `Loot`, `Talk`, `Collect`,
`Inspect`, `Interact`, `Deliver`; `QuestDef.main` for the one gold-marker quest, `group: true` for the ticket
that recommends a group). Rules:

- **Given at the entrance, turned in at the exit.** The giver is a friendly unit by the dungeon door
  (a new `QuestGiver` per dungeon); `QuestDef.turn_in` points at a second friendly unit next to the exit
  portal in the last room, so nobody walks back through four rooms. The quests run in parallel (no
  `requires`) so one visit to the giver hands out the whole set; they are one-time, never daily.
- **Boss drops.** `Loot` objectives use quest items the boss always drops (Normal and Async), so a locked
  boss (no gear) still advances a quest; items follow the existing "carried copies count, not consumed" rule.
- **Solo fair.** Nothing needs a group; `Kill` counts for the killer and every sharer, `Inspect` and `Collect`
  count per player.
- **Rewards.** XP and Kudos (`reward.sp`) only, no gear; the numbers are part of the XP budget below.

### The Onboarding (giver: Welcome Desk Clerk; exit: Very Tired New Hire, "Day 5")

| Quest | Objectives | Text, in one line |
|---|---|---|
| Welcome Aboard (Please Hold) `main` | Kill Badge Scanner x4, Kill Compliance Trainer | "Your badge will be activated in 5-7 business days. Until then, please enter by force." |
| Where Is My Laptop | Loot Laptop Box (Empty) x1 from the Provisioner, Interact Ticket Printer x1 | "Ticket INC-0042 is blocked by INC-0041, which is blocked by this ticket. Close it from the inside." |
| Buddy System | Kill Onboarding Buddy, Talk to the exit hire | "Your buddy is very supportive and has never been wrong. Tell me if they ever say 'as a'." |

### The Migration (giver: Migration Project Manager; exit: On-Call Engineer, "Weekend 14")

| Quest | Objectives | Text |
|---|---|---|
| Inventory of Everything | Collect Undocumented Server Tag x6 (rooms 1-2) | "We think we have 400 servers. The spreadsheet says 380. The racks say yes." |
| Kill -9 the Zombies | Kill Orphaned Process x6 | "Nobody knows what they do. Switch them off and see who calls. That's the plan." |
| Billable Hours, Reversed `group` | Kill both Consultants | "They have a rate card. We have a rake. Apply the rake." |
| Press the Button `main` | Kill Big Bang Cutover, Interact Big Red Button x1 | "It is Sunday at 3 AM and we are 'nearly there'. Please press it before I think about it." |

### Quarterly Business Review (giver: Executive Assistant at the lift; exit: Cleaner of the Boardroom)

| Quest | Objectives | Text |
|---|---|---|
| Pre-Read, Unread | Inspect Pre-Read Packet x4 (lobby, theatre) | "A 94-page pre-read, circulated at 07:55 for the 08:00 meeting. Please confirm you read it." |
| Fix the Slides | Kill Bullet Point x8, Kill Slide Deck Golem | "Slide 6 contains a typo. Slide 6 contains the entire company." |
| Verify the Numbers | Collect Source Receipt x3, Kill The Analyst | "Somebody asked where a number came from. The number is also asking." |
| Any Other Business `main` | Kill The Board (all three heads) | "The last agenda item is 'AOB'. It is always the one that takes four hours." |

### The Hackathon (giver: Hackathon Organiser, heroic; exit: Volunteer With Clipboard)

| Quest | Objectives | Text |
|---|---|---|
| Team Name: TBD | Kill Team AI x6, Interact Whiteboard x1 | "Seventeen teams, one idea: 'AI, but for [thing]'. Please pick a thing." |
| Pivot or Perish `main` | Kill Pitch Judge VC, Talk to the Volunteer | "You have three minutes. The first two are for the founder story. Nobody has a founder story." |

---

## XP budget

Goal: a full first run of each dungeon pays about 35-50% of the level it targets, so dungeons feel worth it
without out-levelling the open world. The world is fitted so one full clear per zone pays the level (see
"Levels and XP" in `docs/FORMULAS.md`); a dungeon is therefore an extra, never a replacement, and its
size is capped at under half a level.

How XP works here (cited, not changed): kill XP is `xp_reward * pct(M) * levels[M-1].xp_reward_pct`, split by
the group bonus `bonus(n) / n` (n = 3: 39% each, 4: 32.5%, 5: 28%); quest XP is `reward.xp * levels[L-1].xp_reward_pct`
at the zone's lower level and is never split. So a group member gets the quest XP in full and a share of the
kill XP, which is why quests carry about a third of the budget.

```
authored XP          K (kills) = trash units * trash_xp + boss xp (elite multiplier is not added; bosses are authored high)
                     Q (quests) = sum of the quest rewards above
solo first run       = K_trash * pct(trash level) + K_boss * pct(boss level) + Q * pct(zone lower level)
```

| Dungeon | Trash (units x xp) | Bosses | Quests | Solo first run | Target level needs | Share |
|---|---|---|---|---|---|---|
| Onboarding | 16 x 5 = 80 x 1.20 = 96 | 40/50/60 = 150 x 1.30 = 195 | 250 x 1.20 = 300 | ~590 | 1500 (level 3) | 39% |
| Migration | 20 x 12 = 240 x 1.65 = 396 | 120/150/220 = 490 x 2.35 = 1152 | 500 x 1.65 = 825 | ~2370 | 6000 (level 6) | 40% |
| QBR | 20 x 25 = 500 x 3.35 = 1675 | 220/280/420 = 920 x 3.75 = 3450 | 800 x 3.35 = 2680 | ~7800 | 22200 (level 9) | 35% |
| Hackathon (heroic) | 20 x 30 = 600 x 3.75 = 2250 | 300/380/520 = 1200 x 3.75 = 4500 | 700 x 3.75 = 2625 | ~9400 | cap: promotion XP | see note |

Measured by the gate (QBR): a level-9 solo first run pays 7960 XP (35%); a four pays its best member 5435 (24%); a level-10 player gets the same run as 7449 promotion XP (24% of the 30000 first-rank bar).

Group of four, the same runs: kill XP is 32.5% of the solo figure, quest XP is in full, so the member gets
Onboarding 95 + 300 = ~395 (26%), Migration 503 + 825 = ~1330 (22%), QBR 1670 + 2680 = ~4350 (20%). Groups
earn a smaller share per head because they clear faster and finish a quest set together; this is the
intended trade (the loot and the company are the group's reward).

Level 10 is the cap: the Hackathon's XP counts as promotion XP (`Endgame: Promotion` in
`docs/FORMULAS.md`), so there is no next level to overshoot. Its size is a tuning target, not read from the
promotion table here: the gate test below must show a first run under half a promotion bar, otherwise the
authored values drop.

**Repeat runs, same UTC day (the lockout's day).**

```
first clear of the day, per character   = 100% kill XP (quests once, ever)
second clear the same day               = 25% of every kill's XP (trash and bosses), Kudos unchanged
third and later the same day            = 0 XP (Kudos, rep and the fight still work)
gray rule                               = unchanged (a mob three or more levels below the player pays 0)
```

The counter is the same per-day key as the boss lockout (`<day>*<dungeon>`), so a player sees one timer in
the Company Calendar. Quest XP is one-time; a daily XP faucet would just be a grind, and the campus already
has the dailies for that.

**Does it push a player past the next zone?** No, by construction: one run adds at most ~40% of the target
level once (the 25% second run adds another ~10% only), the campaign's zone bands overlap by a level
(Hall A 6-7, Hall B 7-8, Sales 5-6, PMO 7-8) and the "+5% XP per level above you" rule already softens an
early lead. Concretely: Onboarding (campus 3, Open Space is next at 4) lets a player arrive in Open Space at
about 3.4; Migration at the Data Center start (6) lands at about 6.4, still inside Hall A and Hall B's
band; QBR at the Penthouse (9) lands at about 9.35, one Showroom from 10. The gate:
`dungeon_xp_stays_under_half_a_level` (a bot at the band's lower level clears the dungeon once solo and
once as a four: it must end under +50% of the level's XP and below the next zone's `level_min`), plus a
repeat-run test for the 100/25/0 ladder.

---

## Systems

### DungeonDef or RaidDef: reuse RaidDef

**Recommendation: reuse `RaidDef`** (`raids:` in `content/game.ron`, the instance framework, rooms, difficulty
model, boss loot, reputation, achievements) and add two fields rather than a new struct.

- `lockout: Daily | Weekly` (default `Weekly`, so every existing raid is unchanged). Dungeons say `Daily`.
- `locked_boss_spawns: bool` (default `false`). Dungeons say `true`: a locked boss stays alive and fightable,
  it just drops no gear and gives Kudos and reputation at the Async rate.

Why not a new `DungeonDef`: the difficulty model (Normal with >= 3 members, else Async), the per-room links
(`BossKilled` doors), `TwinsDef`, boss item counts, sets and lockout storage are exactly what dungeons need.
A second def would duplicate all of it for two fields of difference. The names in the UI can still say
"Dungeon" (the Company Calendar entry, the loading screen); the data type is an internal matter.

A room without a boss (Migration room 1, QBR room 1, Hackathon room 2) needs `requires: None` on its
door; the door for a boss room still uses `BossKilled` as in the raids.

### Group size and difficulty

```
group         = 3 to 5; normal_min_players = 3
difficulty    = Normal with >= 3 members at the zone load, else Async Attendance (same as the raids)
group scaling = as usual (+55% HP, +30% damage per extra member, bosses +20% more); dungeons are tuned for 4
async         = enemy HP x40%, damage x50% (tuned by bot clears: a solo level-3 bot clears all three bosses)
heroic        = Hackathon only: HP x120%, damage x120% over Normal, one extra mechanic per boss (above),
                its own lockout key, Normal-less (a group needs >= 3; below that it is Async Heroic)
```

### Scaling

Scaling uses the existing `config.mob_scaling` (12% HP and 10% damage per level of difference, floor 10%).
Trash is authored at the dungeon's lower level, bosses at the upper level plus one; a player outside the
band is scaled, never blocked. Entry needs the band's lower level (Onboarding 3, Migration 6, QBR 9,
Hackathon 10 plus a QBR run) and nobody over the band's top by more than 3 levels gets full XP from trash
(the gray-mob rule already says this).

### Daily reset rule: UTC day index

```
day_index     = unix_seconds / 86400   (UTC)
lockout key   = <day_index>*<boss>[*<difficulty>]   // like the raids' <iso week>*<key>
online        = per room, kept by the server;  offline = per save: dungeon_locks=<day>*<key>|<key>
reset         = 00:00 UTC (01:00 or 02:00 in Czech local time)
```

UTC rather than local midnight, for three reasons: the server is authoritative online and must not trust
a client clock; the raids' weekly already resets at Monday 00:00 UTC, so one clock covers every lockout and
the Company Calendar can say "resets in 3h" for both; and a local-midnight rule lets a player change the
system clock to farm. The cost is that players in other time zones see the reset at an odd hour; the UI
shows the countdown so it is never a surprise.

### Rewards

```
boss loot     = Normal: boss_items (2) dungeon pieces per member, own role 70%; Async: one piece at 50%;
                named piece (source: Boss(key)) at legendary_pct (25%) Normal, 12% Async
locked        = Kudos and reputation only, at the Async rate
Kudos         = per kill by the usual formula; boss 12x (level * sp_per_level + U[0, level])
reputation    = boss_rep to the dungeon's department: Onboarding HR, Migration Internal IT,
                QBR Marketing, Hackathon Engineering; Heroic +50%; trash 5 at the same department
cosmetic      = once per character per dungeon, from the finale, never locked out
achievements  = Event("dungeon:<boss>"), plus Event("dungeon:<id>:clear"); heroic has its own
```

The gear tier is the dungeon's `ceil(level / 2)`: Onboarding tier 2, Migration tier 4, QBR and Hackathon
tier 5. Named dungeon pieces roll only on the boss they are listed under and carry no perk that can
trivialise the campus or the raids (see Guardrails).

### Company Calendar entry

A "Dungeon Finder" entry on the Company Calendar board: each dungeon appears as a calendar invite
("Mandatory: Onboarding, All Starters"), with the level band, the daily lockout countdown ("resets in
7h 12m", UTC), the group list, a "Join the call" button for a group and a "Attend alone (Async)" button
for solo play. Locked bosses are shown greyed with "already minuted". The Hackathon invite appears only
after a QBR run.

### How the heroic Hackathon differs

```
unlock        = level 10 and a QBR run (any difficulty); no Normal version
numbers       = HP x120% and damage x120% over the QBR Normal, trash authored at 10, bosses at 11
mechanics     = one extra mechanic per boss (Pizza: the Last Slice line; Gremlin: the audience;
                Judge: Valuation), all within the standard vocabulary
loot          = tier-5 pieces with the highest budget of any dungeon, a heroic-only named piece per boss
reputation    = +50%; achievement "Pitch Perfect" for a no-wipe clear
night clock   = fixed night lighting; music, jokes and props are overnight; nothing mechanical
lockout       = its own key (heroic is separate from Normal; it has no Normal)
```

---

## Milestones

Build order, each playable on its own, Onboarding first:

| Step | Content |
|---|---|
| 0 | `RaidDef` fields `lockout` and `locked_boss_spawns`; UTC day index and a `dungeon_locks=` save key; `requires: None` on room doors; tests |
| 1 | The Onboarding: 3 rooms, 5 trash units, 3 bosses (quiz tiles, six-phase ticket, chat-bubble reveal), loot, the Fern, music; bot clears Normal group and Async solo |
| 2 | Company Calendar "Dungeon Finder" entry, Kudos and reputation hooks, achievements, quests and the XP ladder (100/25/0) with the XP gate test |
| 3 | The Migration: 4 rooms, the twins reuse, the PairUp phase with its solo fallback |
| 4 | Quarterly Business Review: the Slide Deck Golem's per-slide adds, the Analyst's RandomAura names, the Board reusing the Hype Panel link |
| 5 | The Hackathon (heroic): the three extra mechanics, heroic lockout key, unlock after a QBR run |
| 6 | Tuning pass: bot clears for every class, Async numbers, join tests |

Suggested versions: Onboarding and the systems in 0.9 (before the Ilmari opening), the Migration in 0.10,
QBR and Hackathon after, to be confirmed with the Ilmari timeline in `docs/ILMARI_RAID.md`.

---

## Guardrails

1. **The solo base game does not change.** A player who never enters a dungeon sees no difference: no new
   gate, no changed XP curve, no new required quest. The dungeons are side content opened from campus
   locations, and nothing in the campaign depends on them.
2. **Offline-first.** Every dungeon is completable solo offline through Async; online groups are the
   intended way to play, never a requirement. Every boss has a solo-safe note and gets a scripted solo bot
   clear (the raids' `tests/coop.rs` pattern).
3. **No power creep.** Dungeon gear stays inside the band's tier and the existing Legacy/raid ilvls; no
   perk makes the campus, Legacy Mode or the raids trivial. Cosmetics and titles carry over freely.
4. **Readable at a glance.** At most three active mechanics per boss, every group event announced by a
   yell, telegraph shapes and colours from the existing set, add counts capped (usually 3-4 alive).
5. **Not overstuffed.** 15-20 minutes: 3-4 rooms, 2 trash packs per room, three bosses. If a run drifts
   past 20 minutes in the bot test, drop a pack, never a boss.
6. **XP stays in budget.** A first run pays under half a level, repeat runs 25% then 0, and the XP gate test guards it (see XP budget).
7. **Classes stay the stars.** Class props and utility are bonuses; no encounter needs a tank, a healer,
   an interrupt or a stun. Stuns obey diminishing returns on bosses and are immune on the finales.
8. **Tone.** Satire of corporate AI hype and meetings, not grimdark. Every boss is a joke first, and the
   yells vary (no repeated punchlines across bosses).
9. **Naming.** "Claude" appears in The Analyst's title as the author chose; the rest of the content keeps
   fictional or generic names. If a real-product reference is ever unwanted, the title becomes "Who Had
   The Chatbot Do The Analysis" with no other change.

## Risks

| Risk | Mitigation |
|---|---|
| Scope grows into a second raid tier | One dungeon per milestone; reuse RaidDef and the existing vocabulary; no new effect types |
| Locked bosses spawning breaks loot economy | Locked runs give Kudos and rep only, at the Async rate; no gear |
| Lone players are overwhelmed on Async | Solo-safe notes per boss, capped adds, bot clears for every class |
| PairUp with one player | Solo fallback tether to a summoned add; the only code change on the list |
| UTC reset surprises players | Countdown in the Company Calendar and on the loading screen |
