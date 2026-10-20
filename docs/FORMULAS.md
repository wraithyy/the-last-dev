# Combat formulas

Simplified, WotLK-inspired rules for gameplay feel. They are **not** Blizzard's implementation and make no claim to match it. All numbers live in `content/game.ron`; time is integer ticks (1 tick = 1/30 s, `ms_to_ticks` rounds up so a 2000 ms cast never ends early). Randomness comes from one SplitMix64 stream seeded per `Sim`.

## Damage

```
raw      = base + floor(spell_power * coefficient_pct / 100)
combo    : if the spell consumes an aura present on the target (with n stacks)
             raw = floor(raw * (100 + bonus_pct * n) / 100) // Refactor on Code Smell: +35%; !important: +60% per Specificity stack
hit      : roll below(100) < spell_hit_pct (96); first roll, a miss ends the spell before anything below
crit     : roll below(100) < crit_pct   (rolled on every hit, crit or not)
             raw = floor(raw * crit_damage_pct / 100)       // 150%
absorbed = min(raw, shield left)       // shields apply in application order
amount   = raw - absorbed              // Event::Damage.amount; absorbed is reported separately
```

Code Review at 20 spell power: 65 + 14 = 79. Refactor: 95 + 17 = 112, with Code Smell 151. Enemy melee is a flat `melee_damage`, physical, no crit roll.

Full order for every damage instance (spells, melee, periodic ticks, expiry blasts):

```
raw     = (base + floor(sp * coefficient_pct / 100)) * dmg_pct / 100   // dmg_pct: caster level scaling, 100 for the player
combo   : consumed aura bonus (above)
execute : if target hp * 100 <= max_hp * below_pct: raw = floor(raw * (100 + bonus_pct) / 100)  // Postmortem: <=25% -> x3
crit    : player spells and auto attack only (above)
done    = floor(raw  * max(0, 100 + sum(attacker damage_done_pct * stacks)) / 100)   // Retrospective, Scope Creep, A/B Test
amount  = floor(done * max(0, 100 + sum(target  damage_taken_pct * stacks)) / 100)   // Overpromise, Unread Spec, Culture Fit, Rebranded (-50)
absorb  : shields, as above
```

Enemy melee is `melee_damage * dmg_pct / 100` before the aura modifiers. Forwarded Reply copies the pre-absorb amount of the player's last landed damaging spell times `pct` (50%) and then goes through the aura modifiers again.

## Heals, DoTs and HoTs

```
heal       = (base + floor(sp * coefficient_pct / 100)) * dmg_pct / 100, capped at missing HP (Event::Heal.amount)
tick       = same formula with the aura's periodic base/coefficient, snapshotted from the caster on application
per tick   = tick * stacks, every interval_ms from application (first tick one interval later)
ticks      = floor(duration / interval) when duration is a multiple of the interval (git blame: 18 s / 3 s = 6)
expiry     = expire_damage * dmg_pct / 100 on natural expiry only (Deadline: 110 at level 7)
```

Heals generate no threat. Periodic damage generates threat and can kill (killer = the aura's caster).

## Hit, miss and auto attack

```
spell hit  : below(100) < spell_hit_pct (96)   damaging spells, incl. Hallucination Blast; miss = "Resist"
white hit  : below(100) < white_hit_pct (95)   player auto attack and enemy melee;       miss = "Miss"
always hit : Absorb (Guardrails) and Interrupt (Reject PR) never roll
```

A missed spell still spends mana, starts its cooldown and GCD, and reports `CastSucceeded`; it deals no damage, applies no aura, does not consume Code Smell and generates no threat (so it does not pull or start combat). Absorb is applied after the hit roll.

Auto attack "Nitpick" (unit field `auto_attack`): range 25 m, swing 1800 ms (54 ticks), damage `U[9, 14]` physical, crit with the unit's `crit_pct` at `crit_damage_pct`. Roll order per swing: hit, crit, damage. The timer is not reset by stopping and restarting. A ready swing waits while the attacker is casting a cast-time spell, the target is out of range, outside the facing cone, out of LoS or evading.

## GCD and haste

```
haste   = unit haste_pct + trophy haste_pct + sum(aura haste_pct * stacks)                    // On-Call Caffeine +30
gcd_ms  = clamp(floor(gcd_base_ms * 100 / (100 + haste)), gcd_floor_ms, gcd_base_ms)        // 1500 .. 1000
cast_ms = floor(spell.cast_ms * 100 / (100 + haste))                                        // no floor
cast_ms = floor(cast_ms * max(0, 100 + sum(aura cast_time_pct * stacks)) / 100)             // Review Fatigue +20, Password Rotation +40, Pivot to AGI -30
```

The GCD starts when a GCD spell begins (cast-time) or resolves (instant). Spell cooldown starts on success. Mana is spent on success (completion), not at cast start; if mana is short at completion the cast fails.

## Spell queue

A GCD spell pressed while the remaining `max(GCD, cast)` is at most `queue_window_ms` (400) is queued (one slot, newest wins) and starts on the first tick nothing is busy. All static checks run again then (target alive, range, facing, LoS, mana, cooldown, lockout). Static errors (no target, out of range, ...) fail immediately instead of queueing.

## Movement and pushback

Cast-time spells fail with `Moving` if started while moving or jumping and are cancelled when `PlayerMoved { moving || jumping }` arrives mid-cast. Instants are unaffected. Each damaging hit on a casting player (after absorbs, amount > 0) moves the cast end back `pushback_ms` (500), at most `pushback_max` (2) times per cast. Enemy casts are not pushed back so the telegraph stays exactly 2.5 s.

## Regeneration

Out of combat (alive, not in combat), a discrete tick every `config.regen.interval_ms` (2000):

```
hp   += max_hp   * hp_pct   / 100   (4%, rounded down, capped at max_hp)
mana += max_mana * mana_pct / 100   (5%, rounded down, capped at max_mana)
```

The first tick comes one interval after spawn, respawn, or `LeaveCombat` (the phase restarts every time the unit leaves combat); `interval_ms: 0` disables it. No event is emitted; read `UnitState.hp/mana`. Hostile units use the same rule (a no-op at full HP; evade already heals).

There is no continuous out-of-combat mana regen and no 5-second rule. In combat the percent tick does not run; the only regen is the legacy per-tick mana trickle: `acc += mana_regen_per_5s_combat; mana += acc / 150; acc %= 150` (Fullstack Engineer: 60 mana per 5 s). A unit is in combat while it is in an enemy threat table (player) or has a non-empty table (enemy).

## Threat and AI

Threat = full pre-absorb damage dealt (minimum 1) to the enemy; initial aggro adds 1. Every threat a player adds is scaled by its class `threat_pct` (`max(1, floor(t * pct / 100))`; Backend 350, others 100), including the extra `Threat { amount }` of Blocking Call, Connection Pool, Cache Invalidation and DROP TABLE. A taunt (Who Touched Prod?) sets the caster's entry to `top + floor(top / 10) + 1` without the class factor, so the target turns at once. Target = highest threat, ties to the lowest unit id; dead units drop out of the table. Applying a harmful aura to an enemy without threat on the caster adds 1 (a DoT pulls at once). Heals generate no threat; self-damage generates none. Social aggro: the first threat on a pack member gives every idle member of the pack 1 threat on the same source. Empty table while engaged, or distance from spawn > `leash_m`, starts Evade: cast cancelled, all auras removed, phase reset to 1, threat dropped, walk home at chase speed, heal to full on arrival (summons despawn instead), damage ignored meanwhile.

AI abilities: each rule's first use is rolled `U[initial_ms]` (default the cooldown range) at the pull, or at the start of the phase it is gated to; afterwards `U[cooldown_min_ms, cooldown_max_ms]` is re-rolled at each use. Hallucination Blast: `U[8000, 12000]` both times. A school lockout, stun or silence blocks the attempt until it ends; rng draws happen only when a rule actually fires (plus `RandomHostile`/`RandomAura` picks).

Yells (boss and player spell yells) are presentation-only: line selection is a round-robin cursor and never draws from the rng (see COMBAT_API.md, Yells).

## Classes

- Base stats and auto attack come from the class `unit` (Fullstack 300 HP / 250 mana / 20 SP; Frontend 360 / 220 / 20, crit 8%; Backend 440 / 200 / 18; DevOps 290 / 300 / 22).
- Max HP = `unit.max_hp + floor(levels[l].hp_bonus * level_hp_pct / 100) + trophies`, then `* (100 + sum of max_hp_pct auras) / 100` (Horizontal Scaling +30%; a gain also heals by the difference, a loss only clamps). Backend `level_hp_pct` is 160, the rest 100.
- Class armour: `damage_taken_pct` (Backend -20, Frontend -10) is added to the damage-taken aura sum of every hit the player takes.
- Revert Commit heals up to the player's HP sampled 5 s earlier (one sample per second, kept 8 s); it never lowers HP.
- Players may cast only their class bar, its resurrect and Coffee Break (`NotLearned` otherwise).

## Trophies

Looted items are permanent player bonuses (`items` in `content/game.ron`). For an item with `c` copies, `n = min(c, ITEM_STACK_CAP = 3)` and every field of its bonus counts `n` times; the player's totals are the sum over all items:

```
max_hp      = senior_engineer.max_hp   + levels[L-1].hp_bonus   + sum(item.max_hp   * n)
max_mana    = senior_engineer.max_mana + levels[L-1].mana_bonus + sum(item.max_mana * n)
spell_power = senior_engineer.spell_power + level bonus         + sum(item.spell_power * n)
crit        = unit crit_pct (percent) + sum(item.crit_pct * n)   // tenths of a percent: 0.5, 1.0
haste       = see GCD and haste                                   // whole percents
mana/5s     = unit mana_regen_per_5s_combat + sum(item.mana_regen * n)   // in combat only
```

Copies beyond the cap are kept and listed but add nothing (hard cap, no further diminishing curve). On pickup, current HP and mana become `max(cur, cur * new_max / old_max)` capped at the new maximum, so a bonus never lowers them. Crit rolls: without a crit bonus the roll is exactly the old `U[0,99] < crit_pct`; with a bonus it is `U[0,999] < crit_pct * 10 + bonus_tenths`. Either way one RNG draw per roll. Typical values: common drops +4..6 HP or mana, +3 mana/5 s, +0.5% crit, +1% haste, +1 SP; rare or boss drops +8 HP, +1% crit, +5 mana or two bonuses.

## Levels and XP

Player stats at level `L` = `senior_engineer` stats + cumulative `levels[L-1]` bonus (HP +35, mana +40, spell power +10 per level; level 10: 615 HP, 610 mana, 110 SP). XP to leave level `L` is `levels[L-1].xp_to_next` (0 at the cap).

The curve is geometric: each level needs about 1.55-1.67x the previous one (2x for the first step), so early levels come fast and later ones slow down. XP sources grow with level too, through `levels[L-1].xp_reward_pct`: kill XP of a mob at level `L` and quest XP turned in at a zone whose range starts at `L` are multiplied by it. The authored `xp_reward` and quest `reward.xp` stay the level-1 values; quest Story Points stay on the authored XP.

| Level | XP to next (old) | XP to next | Ratio | XP reward % |
|---|---|---|---|---|
| 1 | 400 | 450 | - | 100 |
| 2 | 600 | 900 | 2.00 | 120 |
| 3 | 800 | 1500 | 1.67 | 120 |
| 4 | 1000 | 2400 | 1.60 | 130 |
| 5 | 1200 | 3800 | 1.58 | 145 |
| 6 | 1400 | 6000 | 1.58 | 165 |
| 7 | 1600 | 9300 | 1.55 | 235 |
| 8 | 1800 | 14400 | 1.55 | 335 |
| 9 | 2000 | 22200 | 1.54 | 335 |
| 10 | 0 | 0 | - | 375 |

Why: the world is played once (trash only respawns after `TRASH_RESPAWN_MS` away), so the table is fitted to the XP one full clear per zone pays (every kill, every quest, `quest_and_kill_xp_track_the_zone_levels`). Old table: the cap came at the end of the PMO, with the Penthouse and the Lab at 10. New table, level after each zone: campus 3, Open Space 4, Meeting Rooms 5, Sales 6, Data Center halls 6 and 7, PMO 8, Penthouse 9, Innovation Lab 10 (reached in the Showroom). Talents (2, 4, 6, 8, 10) land in the campus, at the end of Open Space, Sales, in the PMO and in the Lab. Quest `level_min` follows the curve (at most the level a full clear arrives with); no rested or catch-up XP, the +5% per level above you already rewards pulling up.

Measured (`time_at_level`, Fullstack bot, seed 7, one pass from level 1 with its real level, sim seconds of fights and rests plus the walking it skips at 7 m/s; it misses about a third of the quests, mostly item and chain quests):

| Level | Seconds at level (old) | Seconds at level |
|---|---|---|
| 1 | 345 | 345 |
| 2 | 68 | 70 |
| 3 | 244 | 286 |
| 4 | 116 | 345 |
| 5 | 190 | 359 |
| 6 | 348 | 426 |
| 7 | 231 | 431 |
| 8 | 255 | 299 |
| 9 | 372 | 398 |
| 10 | from the PMO on | reached at the end of the Rack Room |

The bot turns quests in only after clearing a zone, so level 1 spans most of its campus pass (a player handing in Onboarding first levels sooner). Content time per zone is roughly flat (200-450 bot seconds), so with one pass the curve can spread later levels over more zones but not make a level take 30 minutes; that needs more content or re-clears.

Saves before version 3 keep their level; the XP into it keeps its share (`xp * new_need / old_need`).

Monster scaling when spawned at level `M` with authored level `A` (`config.mob_scaling`):

```
pct(per) = max(10, 100 + per * (M - A))
max_hp   = max_hp * pct(hp_pct_per_level = 12) / 100
dmg_pct  = pct(damage_pct_per_level = 10)            // multiplies melee, spells, heals, DoTs it deals
reward   = xp_reward * pct(xp_pct_per_level = 10) / 100 * levels[M-1].xp_reward_pct / 100   // 100 inside a sprint
```

Kill XP for a player of level `P` (`config.xp_rules`, classic style but compressed for a 10-level game):

```
M >= P : xp = reward * (100 + 5 * min(M - P, 4)) / 100        // +5% per level above, at most +20%
M <  P : d = P - M
         d >= gray_level_diff (3) : xp = 0                     // gray mob
         else                     : xp = reward * (100 - 20 * d) / 100   // -20% per level below
```

Only players earn XP, and only for kills by a player (DoTs count). Extra XP carries over; several level-ups from one kill are possible.

## Co-op group

Group = every player in the sim (one room, at most `MAX_PLAYERS` = 5).

Kill sharers: the killer plus every other living member within `GROUP_SHARE_RANGE_M` (60 m) of the corpse. XP goes to the living sharers (`n` of them); each computes the kill XP above at its own level, then:

```
bonus(n) = [100, 100, 116, 130, 140][n - 1]       // classic group bonus, percent
xp_each  = kill_xp(P_member, M, reward) * bonus(n) / 100 / n
```

Solo: `xp_each = kill_xp`, unchanged. Same level, reward 120: 2 players 60 each, 3 players 46 each. Sharers also get the quest `Kill` credit (the killer even when dead, as before).

Loot: every member rolls the unit's loot table separately, in join order, whatever their distance; each `LootDropped` and `Looted` goes to that member only. A leaver's unclaimed roll is dropped.

Enemy HP scaling (`config.group_hp_pct_per_player`, content 55, serde default 40):

```
group_pct = 100 + group_hp_pct_per_player * (players - 1)
max_hp    = level_scaled_max_hp * group_pct / 100      // hostile AI units only, not friendlies or players
```

Applied at spawn (zone load, `spawn_unit`, summons) and re-applied to every living enemy when a member joins or leaves, keeping its HP fraction (`hp = hp * new_max / old_max`, at least 1 while alive). Solo is 100% (no change); 3 players 210%, 5 players 320%.

Enemy damage scaling (`config.group_damage_pct_per_player`, content 30, serde default 0):

```
group_dmg_pct = 100 + group_damage_pct_per_player * (players - 1)
dmg_pct       = level_dmg_pct * group_dmg_pct / 100     // hostile AI units only
```

Multiplies every damage and heal an enemy deals (melee, spells, periodic and expiry amounts snapshotted at application), re-applied with the HP scaling on every join and leave. Solo 100%, 2 players 130%, 3 players 160%, 5 players 220%.

Non-boss enemies also deal `config.trash_damage_pct` (content 110, default 100) of their level-scaled damage.

Pack growth (`config.group_pack_growth_players`, content 2, default 0 = off): every that many players beyond the first, each trash pack gains one member (`floor((players - 1) / 2)`, at most 4): the pack's units in order, cycling, at the next `pack_offsets` slots. Spawned on zone load and when a member joins (packs already fighting or wiped out are skipped); extra members have their own zone slots, so killed ones stay dead on re-entry like the rest.

Linked packs (`config.pack_link_m`, content 7 m, default 0 = off): when a trash unit gets its first threat, idle trash of other packs (not bosses or summons) within that distance of it joins on the same source, and pulls its own pack. Leash: beyond `leash_m` from its spawn a unit evades home and heals to full; every player on its threat table stays in combat for `config.combat_lock_ms` (content 5,000) after the reset, so no out-of-combat regen or resurrect right after kiting a pull away.

Quest rewards add `reward.xp * levels[Z-1].xp_reward_pct / 100` on turn-in, `Z` the first level of the zone the quest is turned in (breadcrumbs pay at their destination): no level-difference adjustment, nothing at the cap, same carry-over and level-up rules as kills.

Corpses (`refresh_corpse`, `CORPSE_EMPTY_MS` 30,000, `CORPSE_LOOT_MS` 180,000): a dead unit despawns 30 s after its death or after the last roll on it is taken (or its owner leaves), and 3 minutes after its death while any group member still has a roll on it (that loot is lost). The empty timer never runs past the 3 minutes and is never pushed back. Bosses keep their corpse until the zone unloads (set pieces, often with yells and banners tied to them); hand-spawned units that respawn in place are left to their respawn timer. Kill credit, XP and loot are decided at the death, so a despawn changes none of them. Each client applies the same rule to its own roll only: online a corpse a teammate still has to loot sinks into the floor for you after 30 s while the server keeps it; offline the sim despawns it on the same tick the sink ends.

Zone spawns: packs in the first half of the zone's list spawn at `level_range.0`, the rest at `level_range.1`; bosses and friendlies keep their authored level; summons use the summoner's level. Trash of a zone left for at least `TRASH_RESPAWN_MS` (300,000 ms of sim time) respawns on re-entry and gives XP again.

Thermal Runaway (Cooling Failure) is a stacking DoT: `3 * dmg_pct / 100 * stacks` every 2 s, one stack per pulse (7 s), up to 20; Rollback's dispel takes the Hot Aisle debuff re-applied after it, so the heat cannot be cleansed. Recursive Self-Improvement (AGI phase 3): +8% damage done per stack, one stack every 4 s, up to 25.

## Gear

```
budget   = quality(rarity) * (70 + 3 * ilvl) / 100 * (125% for keyboards)   // quality: 40, 70, 100, 130, 160
stat     = floor(package[role][stat] * budget / 100)                         // per piece
```

Role packages at budget 100 (HP, mana, SP, crit tenths, haste tenths, mp5, armour tenths, power tenths): Ranged 0/15/6/12/20/0/0/35, Melee 30/0/6/18/0/0/0/35, Tank 70/5/0/0/0/0/35/20, Healer 0/35/6/0/20/8/0/35, generalist 30/18/4/0/0/0/0/25. Worn pieces, set bonuses (2 pieces: stats; 4 pieces: a perk) and trophies add up in `ItemTotals`; gear haste is summed in tenths and floored to whole percent.

```
power    : amount = (base + sp * coef / 100) * dmg_pct / 100 * (1000 + power_tenths) / 1000   // damage and heals
armour   : hit    = hit * (1000 - min(armor_tenths, 250)) / 1000, before the aura modifiers
perks    : SpellDamage / SpellHeal +pct on that spell; Cooldown and ManaCost -pct (cooldowns at most -80%);
           Echo repeats a landed hit at pct (chance roll on the combat rng, only when worn)
           Execute / Triage +pct damage / healing on targets at or below below_pct HP;
           Desperate +pct damage and healing while the wearer is at or below below_pct HP;
           KudosFind +pct of the wearer's own kill Kudos share
perk caps (percent, tests/items.rs): Rare 25, Epic 30, Legendary 50; gadgets cool down >= 45 s
```

Kudos (`sp` in code and saves) per kill and member: `(level * sp_per_level + U[0, level]) * mult` (elite 4, boss 12; summons 0). Gear drop chance: trash 12%, elite 60%, boss 100%; rarity weights Common/Uncommon/Rare/Epic trash 50/35/13/2, elite 10/45/37/8, boss 0/25/55/20; the piece is from the mob's tier (`ceil(level / 2)`), from the looter's role 70% of the time. Bosses with named pieces (`source: Boss(key)`) add one 25% of the time (`legendary_pct`); elites and night-shift mobs with named pieces drop one at `named_pct` 8% (scaled like their gear chance); non-bosses drop junk 30%, picked from junk of the mob's tier or tier-free junk. `source: Sprint` pieces come only from the end-of-sprint chest. Quest "choose one" pools rotate per quest (FNV-1a of the quest id) so quests of a tier offer different pieces, one per slot. Quest turn-ins pay `reward.xp * 15%` Kudos plus `reward.sp` (authored XP, before `xp_reward_pct`). Prices and level requirements: see Kudos below.

Measured (combat_core tests, level-capped bots, 3 seeds): a full set of tier-appropriate Rare gear cuts boss kill time to 72-80% of fresh and raises the boss-fight minimum HP by 13-32 points; Code Review damage over 10 s goes up by about 60% at tier 5.

## Kudos

The currency is Kudos ("redeemable for nothing, except here"); code and saves keep the old `sp` names so saves load unchanged.

```
price(gear)     = base(rarity) * (3 * ilvl + 2)        // base 8/20/50/150/400; authored `price` wins (All-Hands hoodies 30)
required_level  = max(1, ilvl - 1) + (1 if Epic or Legendary)   // vendors only; drops ignore it
sell            = price * 10%  (junk 100%), buyback at the sell price
rotating shelf  = rotate_count items per rotate_ms window ("Procurement approved 3 extra items this week")
```

Why linear in ilvl: zone income grows about linearly with the band level, so a tier's Rare costs one to two zones at any point and Epics (3x a Rare) are a multi-zone goal. Purchases of 500 Kudos or more ask for a second click.

Expected income of one solo pass (`kudos_never_buy_the_whole_stock_and_upgrades_take_zones`, `cargo test -p combat_core --test kudos -- --nocapture`): expected kill Kudos `(level * sp_per_level + level / 2) * mult` for every unit of the zone, every quest the zone closes, drops sold (gear chance times the tier's average price by rarity weights, junk 30%), one day of dailies plus a seventh of the weekly at the band's first level. "Stock" is every vendor gear piece (fixed and rotating) the band's top level may buy; "share" is everything earned so far, spending nothing.

| zone | band top | kills | quests | sales | dailies | zone total | cumulative | stock at band top | share | tier Rare | zones per Rare |
|---|---|---|---|---|---|---|---|---|---|---|---|
| campus | 2 | 30 | 162 | 32 | 22 | 246 | 246 | 4570 | 5% | 400 | 1.6 |
| tower/1 | 2 | 56 | 141 | 68 | 22 | 287 | 533 | 4570 | 11% | 400 | 1.4 |
| tower/2 | 4 | 148 | 224 | 165 | 66 | 603 | 1136 | 11430 | 9% | 700 | 1.2 |
| tower/3 | 6 | 238 | 195 | 234 | 111 | 778 | 1914 | 25030 | 7% | 1000 | 1.3 |
| datacenter/1 | 7 | 223 | 453 | 225 | 133 | 1034 | 2948 | 41410 | 7% | 1300 | 1.3 |
| datacenter/2 | 8 | 284 | 0 | 253 | 156 | 693 | 3641 | 49210 | 7% | 1300 | 1.9 |
| tower/4 | 8 | 302 | 487 | 295 | 156 | 1240 | 4881 | 49210 | 9% | 1300 | 1.0 |
| tower/5 | 10 | 346 | 607 | 296 | 200 | 1449 | 6330 | 70010 | 9% | 1600 | 1.1 |
| lab/1 | 10 | 165 | 675 | 88 | 222 | 1150 | 7480 | 70010 | 10% | 1600 | 1.4 |
| lab/2 | 10 | 285 | 0 | 256 | 222 | 763 | 8243 | 70010 | 11% | 1600 | 2.1 |

Sprint chests add `40 * level` per delivered run (80 at +2, 400 at +10); the world boss pays boss Kudos (`12 * (level * 1 + level / 2)`, 180 at level 10). Gates: the share stays under 35% after every zone, the tier Rare is 0.8-2.5 zones of that zone's income, and a tier-4 Epic costs at least three tier-4 Rares (`epics_are_steep_and_vendors_gate_by_level`).

## Live world

```
game minute  = unix_s mod 1440                 // one office day = 24 real minutes
night        = minute >= 18:00 or < 06:00      // spawns, chatter
darkness     = smoothstep over 17:15-18:15 and 05:45-06:45   // client lighting only
day / week   = unix_s / 86400 ; (unix_s / 86400 + 3) / 7   // Monday 00:00 UTC starts a week
daily reward = xp_to_next(level) * 15% XP, 6 * level Kudos    // level when joined
weekly       = xp_to_next(level) * 70% XP, 30 * level Kudos, gear_choices(ceil(level / 2), epic)
```

- Rotation: `per_day` distinct dailies by a SplitMix64 partial shuffle seeded with the day; the weekly is `SplitMix64(week).below(n)`.
- Throughput goals: `n * level` damage plus healing. Work zone: the reachable zone with packs whose band start is the highest at or below the level and whose top is at least `level - 1`; else the highest band start below the level; else the board's zone.
- Night units spawn at the highest group level; the Board at level 10 with the usual group scaling (+40% HP per extra member in the shipped config).
- Measured (`dailies_keep_the_campaign_inside_the_level_bands`): one day of dailies plus a seventh of the weekly per zone of a single campaign pass keeps every arrival inside its zone band (at most one level above its top). `three_players_beat_the_board_and_one_does_not_trivially`: a geared level-10 trio kills the Board in 90-97 s with at most one death; a lone geared DPS bot dies at about 36-41 s.

## Talents

- Splash: extra targets within the radius of the primary target (nearest first, enemies already in combat, injured allies for heals) take `pct` of the spell's damage or healing; DoTs and HoTs land at full strength.
- `CastTime` adds to the cast-time aura percent after haste (-100 = instant); `ManaCost` subtracts from 100% of the cost; `HpCost` pays `mana * pct / 100` HP instead (never below 1 HP); `Periodic` scales the snapshotted tick; `DamageTaken` adds to the damage-taken percent with auras and class armour.
- Performance Review: `respec_sp_per_level * level * (100 + respec_growth_pct * reviews_taken) / 100` Kudos (15 per level, +100% per earlier review: 150, 300, 450, ... at level 10). `reviews_taken` is `UnitState::respecs`, saved as `respecs=`; online the client counts resets from `TalentsChanged` (rooms start fresh).
- Balance gate (`every_talent_build_clears_the_campaign`): each class clears the campaign on 3 seeds with options 1, 2 and 3 in every row; total boss time may not drop below 80% of the fastest class without talents.

## Endgame: Promotion, reputation, burnout

All in `content/game.ron` under `endgame` (`endgame.rs`, `burnout.rs`). Burnout is per mille (1000 = 100%).

```
promotion xp     = kill / quest / daily XP earned at level 10 (level XP rules, group bonus as usual)
rank_xp_to_next  = rank_base_xp + rank_step_xp * rank       // 30000 + 6000 * r: 30k, 36k, ... 84k
all ten ranks    = 570000 XP
paragon          = min(rank * 1%, 5%) of max HP and max mana  // applied after gear and auras, never heals
rep tiers        = Ignored 0, Tolerated 300, Cc'd 900, Invited 2100, Trusted 4200, Exalted 8400 (cap)
rep per kill     = trash 5, elite 25, boss 75 (level 10 only, the zone's department; summons give none;
                   zones with a Legacy variant pay only on their Legacy floors, the campus as before)
rep per task     = daily 80, Sprint Goal 300, to the department with the lowest standing (ties: content order)
world boss       = 150 to every department
```

Departments by zone: Engineering (tower/1, lab/1, lab/2), Security (tower/4), HR (tower/2), Marketing (campus, tower/5), Internal IT (datacenter/1-2), Sales (tower/3). Rep vendor items stay listed and are refused below their tier (`ItemError::RepTooLow`): HR Cc'd mug, Marketing Invited hoodie, Sales Exalted headgear (Procurement); Engineering Cc'd keyboard, Security Invited lanyard, Internal IT Trusted headset (Hardware Broker).

```
burnout up    : meeting debuff +2% (standup, quick_sync, recurring_meeting, wellness_survey, culture_fit,
                mandatory_fun, deadline), Sprint run end +6%, death +8%,
                per minute: night (Crunch mode) +1%, session past 45 min +0.5%,
                in combat +1.5% every 10 s (endgame.burnout.combat_*)
burnout down  : Coffee Break channel -3% (once per 2 min), Friday from 12:00 UTC -2%/min,
                rubber duck out -1%/min, holiday -10% per full hour logged off (after 15 min; offline
                from the save's timestamp, online rooms start fresh at 0), management kill -5%
                (bosses -15%), props (own cooldown per prop and player): coffee -6%/2 min,
                vending -4%/2 min, foosball and ping-pong -12%/3 min, bean bag -15%/5 min,
                quiet room -10%/4 min, office dog -10%/2 min, Sad Bench -5%/2 min
tiers         : Fine 0 | Tired 250 | Crunch 550 | Burnt Out 850 (per mille), each with regen_pct,
                cast_pct, misfire_pct and power_pct (see GAME_DESIGN.md); spell power scales every
                damage and heal amount of the caster, cast time adds to the aura/perk percentages,
                a misfire spends the mana and skips the effects (channels never misfire)
effects       : >= 75% move 10% slower (client speed, server budget is a ceiling); quips every 45 s at
                35%: yawn from 30%, mutter from 50%, reply-all from 70%; hair stages come
                burnout * 25 HP points earlier
```

Drifts tick once a minute from the player's join; quips use their own SplitMix64 stream, so neither shifts combat or loot. Save keys (optional): `rank=<r>*<xp>`, `rep=<6 numbers>`, `burnout=<value>*<unix minute>`.

## Legacy Mode

All in `content/game.ron` under `legacy` (`legacy.rs`); the affixes are the Sprint ones (`sprint.affixes`).

```
switch        = group lead, everyone at level 10, no sprint running (Command::SetLegacy)
spawn level   = max(authored, 11) for packs and affix adds, max(authored, 12) for floor bosses
enemy stats   = mob scaling from the authored level (+12% HP, +10% damage per level),
                then the zone's hp_pct / damage_pct, then group scaling as usual
per zone      = tower/1 Hotfix Friday 125/135, tower/2 Mandatory Fun 100/100,
(affix hp/dmg)  tower/3 Scope Creep 100/100, tower/4 Reorg 100/100, tower/5 Budget Freeze 145/110,
                datacenter/1 AI-Assisted 105/110, datacenter/2 Technical Debt 110/115,
                lab/1 Pair Programming 110/130, lab/2 Legacy Code 100/100
loot          = gear rolls as usual (chance, rarity weights, own role 70%) from `source: Legacy`
                pieces only; no authored loot tables, no named Normal pieces, junk as usual
boss bonus    = boss_items (2) Legacy pieces per member, unless the room's lockout has that boss;
                locked out: Kudos only
lockout       = per room (server) or per save (offline), ISO week (Monday 00:00 UTC, same as the
                weekly); set when a Legacy floor boss dies
twist         = one behaviour change per floor (`legacy.zones[].twist`), applied at spawn to the
                floor's pack trash and rares (TrashAbility) or its boss (BossAbility, BossSwap):
                tower/1 trash Merge Conflict (circle 3 m, 35, 11-14 s); tower/2 boss 90-Page Analysis
                -> 190-Page Legacy Edition (kick, 3 s cast, 70 to all within 30 m); tower/3 trash
                Auto-Renewal (dispellable DoT, 30 per 2 s for 10 s, 14-17 s); tower/4 boss Matrix
                Reorg (5 lines 30 x 3.5 m, 4.5 m apart, one safe lane, 90, 16-19 s); tower/5 boss
                Budget Freeze (+30% cast time for 10 s on a random member, no dispel, 14-17 s);
                datacenter/1 trash Deprecated Cron Job (line 18 x 2.5 m, 60, 11-14 s); datacenter/2
                boss Coolant Leak (cone 12 m 70 deg, 100, 12-15 s); lab/1 trash Hallucinated
                Citation (circle 4 m at the caster, 90, 12-15 s); lab/2 boss Emergent Capability ->
                Shared Responsibility (split circle 4 m, 230 shared, 2.5 s)
                (bases are authored-level numbers; level and Legacy scaling apply as above)
world         = Legacy floors have their own cleared floors and open elevators (session only);
                saves keep the campaign's
```

Legacy tier: 23 pieces at ilvl 12 (5 Common, 5 Uncommon, 8 Rare, 5 Epic), budget `quality * 106%` (+6% over ilvl 10: Rare 106, Epic 137), perks inside the Rare/Epic caps. Not sold, not in the Sprint chest (its tier is the highest non-Legacy one, `Economy::campaign_top_tier`), so the Kudos tables above do not change. Save key (optional): `legacy_locks=<iso week>*<boss>|<boss>`.

Measured (`tests/coop.rs`, level-10 bots in the best tier-5 World Rares, packs pulled one by one, seeds 7/42/1234): the trio clears every Legacy floor, the lone DPS dies or scrapes through below 25% HP.

| floor | trio time s | trio lowest HP | trio most dead | solo DPS | solo lowest HP when it survived |
|---|---|---|---|---|---|
| tower/1 | 101-108 | 52-64% | 0 | died 3/3 | - |
| tower/2 | 207-224 | 42-50% | 0 | died 2/3 | 8% |
| tower/3 | 346-364 | 21-34% | 0 | died 3/3 | - |
| tower/4 | 322-348 | 38-42% | 0 | died 3/3 | - |
| tower/5 | 244-262 | 35-55% | 0 | died 3/3 | - |
| datacenter/1 | 208-215 | 54-60% | 0 | died 3/3 | - |
| datacenter/2 | 221-253 | 47% | 0 | died 3/3 | - |
| lab/1 | 159-205 | 65-67% | 0 | died 2/3 | 7% |
| lab/2 | 166-175 | 2-43% | 0 | died 3/3 | - |

Time alone does not separate solo from group: without the group HP bonus a surviving lone DPS is only 1.2-1.5x slower, so the gate is survival.

## Variety pass: objectives, rares, trash mechanics

New objectives (`quests.rs`):

```
Defend      = once a second, every living player within DEFEND_RANGE_M (25 m) of the defended
              friendly counts +1 (need = secs); while anyone guards, a `wave` unit spawns 6 m off
              it every `every_ms` (at most 3 alive), as its add (no loot, no Kudos) with 200 threat
              on it. The defended unit dies: every guard's count goes back to 0 (QuestProgress 0),
              the waves despawn and a fresh copy stands up. Done objectives stop the waves.
KillClean   = like Kill, but a kill only counts when no non-split telegraph landed on that player
              since the unit's pull (`Unit::pulled_at`, kept past death)
```

Company Calendar chains (side quests, outside the campaign bands): Prod Incident (Talk, Defend
Prod 60 s with Unhandled Pager Alerts every 7 s, Wait 30 s; 20/40/60 XP, 10 Kudos, Internal IT
+60) and Flaky Tests (KillClean 3 Delivery Robots, KillClean 2 Scooter Executives, Talk; 30/40/50
XP, 10 Kudos, Engineering +50). Each ends on a trophy.

Rares (`ZoneDef::rares`, anchor `rare_<n>`): spawned on load only where the layout has the anchor,
never in a Sprint, on zone slots 9,000+ (killed ones stay dead like trash, `TRASH_RESPAWN_MS`), never
a zone clear. tower/1 Jira Automation Rule #47 (L2 elite, 480 HP, Bulk Transition circle 5 m 30,
Reindex Everything kick 35, 100% trophy); datacenter/2 The Nightly Backup Job (L8 elite, 1500 HP,
Retention Policy circle 4 m 75, Restore From Tape kick self-heal 450, enrage 60 s, 100% trophy,
quest The Job That Never Ran).

New trash:

```
datacenter/2  OOM Killer (L8, 560 HP, 14 melee): Out of Memory, a 3 m circle on every player
              within 30 m (EachHostile: fighters only, never bystander NPCs), 60, 2.5 s, 10-13 s
lab/2         Closed-Source Frontier Model (L10, 650 HP, 18 melee): frontal shield 150 deg -50%,
              Cherry-Picked Benchmark 60 (2 s cast) every 9-12 s
tower/4       Project Manager + Critical Path (line 18 x 2.5 m, 40, 14-17 s);
              Consultant + Statement of Work (kick, 2.5 s, 60 to all within 30 m, 14-16 s)
tower/5       AI Ethicist + Trolley Problem (line 20 x 3 m, 55, 9-12 s); Ethics Review cast
              30 s -> 6 s (still does nothing), every 3-4 s after it
raid          Lanyard Enforcer, Booth Staff: frontal shield 120 deg -60%; AV Technician wheels in
              a Floor Monitor Speaker (add, 320 HP, max 2, its Microphone Feedback line is the old
              one); Open Bar AE puts Open Tab on a random member (dispellable, 35 per 2 s, 12 s)
              instead of healing; Swag Goblin unleashes Sentient Tote Bags (add, 260 HP, max 2,
              10-12 s); Prompt Engineer's prompt lands as a 3.5 m circle (115, 2 s); Hackathon
              Zombie Rage Commit (line 16 x 3 m, 110, 9-11 s) instead of its energy drink
night         Deadline Wraith + Release Window Closing (circle 3 m, 30, 10-13 s); Sleep-Deprived
              Contractor + 3 a.m. Hotfix (kick, 2.5 s, 55, 12-15 s); The Hard Deadline (L6 elite,
              1,300 HP, anchor night_7, enrage 50 s) casts Deadline Creep: 6 circles of 2.4 m,
              copy k at (k + 1) * 2.5 m along the caster's facing, landing 1.5 s + k * 0.35 s
              (TelegraphDef::march), 45 each
```

Every raid trash type has one readable mechanic (shield, add, kick, circle, cone, line or dispel)
and no room repeats one (`tests/variety.rs`). Props may carry their own `lines` (zone-flavoured,
picked by the roll) and an `action` tooltip verb; vending, tables, scooters and the fire alarm keep
their kind's lines.

Number changes to keep the gates: CEO melee 16 -> 19 (the co-op tower trio's lowest tower/5 boss
HP had drifted to 61-62%, gate < 60%; now 59%); Prompt Engineering (Fullstack, Staff row) Echo
25% -> 20% (Fullstack build 3 had drifted to 76% of the fastest bare class, cap 80%; now 85%).
Hall B and Rack Room quests pay XP and reputation but no extra Kudos, so the campaign's vendor
curve stays where it was.

## Raid: All-Hands Offsite

All in `content/game.ron` under `raids` (`raid.rs`, one entry per raid; VibeCon below); bosses, trash and spells are ordinary content.

```
entrance      = campus shuttle stop (door:offsite/1); open while every member is at level 10
                (server and client both check it; Sim::zone_links)
rooms         = offsite/1 Lobby (Icebreaker Facilitator) -> offsite/2 Breakout Rooms (Synergy
                Twins) -> offsite/3 Terrace (Keynote Speaker) -> offsite/4 Grand Ballroom (The
                Founder); a room's next door opens once its boss died (BossKilled, for good)
difficulty    = Normal with >= normal_min_players (3) members at the raid zone load, else Async
                Attendance; Async enemy HP x40%, damage x55% on top of everything else
levels        = trash authored at 11, bosses at 12 (no further level scaling); group scaling as
                usual (+55% HP, +30% damage per extra member, bosses +20% more)
boss loot     = Normal: boss_items (2) raid pieces per member (ilvl 14 Epic, own role 70%) and
                the boss's Legendary at legendary_pct (25%); Async: one piece at 50%, Legendary
                12%; the second twin and adds drop nothing; trash rolls the Legacy tier
lockout       = per encounter and difficulty ("async:" prefix), ISO week like Legacy Mode; a
                locked boss does not spawn until Monday 00:00 UTC; per room online, per save
                offline (raid_locks=<iso week>*<key>|<key>)
reputation    = boss: boss_rep (120) with every department, 60 on Async; raid trash: HR (5)
burnout       = the five raid boss units are management: boss_kill relief (150 per mille)
achievements  = Event("raid:<boss>") on any difficulty, Event("raid:normal|async:<boss>")
```

Mechanics:

```
Pair Up        = Effect::PairUp: players within the spell range shuffled into pairs (an odd one
(Icebreaker)     with the boss); 12 s, range 8 m; a stretched pair takes base 55 (x the
                 caster's damage scaling) each second, both members; every 30 s
Synergy Twins  = linked HP (a hit on one sets the other to the same HP fraction; one death kills
                 both); the shielded twin takes -80% damage; every 18 s in combat they swap
                 places and the shield; Leverage never moves (chase speed 0); a pull on either
                 pulls both, and both reset when the boss evades
Next Slide     = 9 parallel lanes 34 x 4.5 m, 300 ms apart (a sweep), one inner lane left out at
(Keynote)        random (TelegraphDef::gap); 190 per lane hit; every 20 s, 13 s below 50%;
                 Live Product Demo: 3 s must-interrupt, 300 to everyone within 45 m
We Are a Family= at 66% and every 40 s after: the Founder's Nephew and the Co-Founder's Spouse
(Founder)        (500 HP each before scaling) start a 15 s uninterruptible Mandatory Family
                 Dinner, 2600 to everyone within 70 m (a wipe); killing one cancels its cast,
                 a stun only restarts it (3 s later);
                 33%: Founder Mode (+25% damage) and Golden Parachute circles
enrage         = Running Over Time (+100% damage, +30% haste) after 5 min (Founder 6 min)
```

Raid tier: 16 set pieces (four role sets of four: Offsite Logistics, On-Site Support, Keynote Track, Team Spirit) and four Legendaries, all ilvl 14: budget `quality * 112%` (Epic 145, Legendary 179, keyboard x1.25), perks inside the Epic/Legendary caps (tests/items.rs). Not sold, not in the Sprint chest (`campaign_top_tier` counts World, Vendor and Sprint gear only). Three new looks: the fleece vest (hoodie style 9), the ribbon badge (lanyard 8) and the bucket hat (headgear 14).

Measured (`tests/coop.rs`, `the_raid_takes_a_group_and_async_attendance_takes_one`: level-10 bots in the best Legacy piece per slot, packs one by one, boss last, the dead released and everyone rested between rooms, seeds 7/42/1234):

| run | Lobby | Breakout | Terrace | Ballroom | whole raid |
|---|---|---|---|---|---|
| trio, Normal | 189-203 s, low 29-69% | 187-215 s, low 43-49% | 205-211 s, low 62-64% | 265-285 s, low 61-67% | 4/4 bosses, 876-894 s, nobody died |
| solo DPS, Async | 158-167 s, low 44-49% | 141-150 s, low 33-47% | 135-156 s, low 32-48% | 150-180 s, low 42-47% | 4/4 bosses, 616-642 s, nobody died |
| solo DPS, Normal | 296-310 s, low 29-37% | died (both twins pull together) | - | - | 1/4, wipes at the Twins |

The gate: the trio clears every seed, the lone DPS clears Async Attendance every seed, and the lone DPS never clears Normal (it would also fail if it scraped through above 25% HP).

## Raid: VibeCon

The second entry in `raids`; same framework, difficulty model, lockout and loot rules as the offsite.

```
entrance      = registration desk with a lanyard queue on the campus east lawn (door:vibecon/1);
                open while every member is at level 10
rooms         = vibecon/1 Expo Hall (The Vibe Coder) -> vibecon/2 Workshop Rooms (The Prompt
                Whisperer) -> vibecon/3 Hackathon Corner (The Demo God) -> vibecon/4 Afterparty
                (The Hype Panel) -> vibecon/5 Main Stage (The Keynote AGI-Announcer) -> campus
difficulty    = as the offsite; Async enemy HP x40%, damage x50% (one bot needs the extra room
                for the keynote's adds)
boss loot     = as the offsite; raid pieces only from VibeCon's own `sets` (Booth Duty, Volunteer
                Crew, Speaker Kit, All-Nighter), one Legendary per boss
reputation    = boss: 120 with every department (60 Async); trash: Marketing (5)
burnout       = management: the '10x' Founder, the three panellists, the keynote (150 per mille
                for the bosses, kill relief for the founder)
achievements  = raid:vc_<boss>; the keynote's title is "Speaker"
pet           = Conference Wi-Fi Router: 25% from the keynote (never works)
```

Mechanics (bosses 9000-13000 HP before group scaling, level 12):

```
Ship It        = every 15 s the Vibe Coder opens an Unreviewed PR (650 HP, max 3); it casts Merge
(Vibe Coder)     to Prod (10 s, uninterruptible): 300 to everyone within 70 m, then
                 Effect::Detonate kills it and gives the boss Shipped to Prod (+15% damage per
                 stack, 10 max); a killed PR does nothing, a stun restarts its merge
Ignore Previous= 2 s cast, then a 3.5 m circle under each player for 2.6 s; whoever is still in
Instructions     it takes 30 and Ignore Previous Instructions for 4 s: movement input mirrored
(Whisperer)      on the client (AuraDef::reverse_controls; Magic, dispellable); every 24 s, 17 s
                 below 50%
Live Demo      = 4 s must-interrupt cast every 18-20 s; if it lands: 240 to everyone within
(Demo God)       45 m and two Live Demo Bugs (600 HP); Works on My Machine: 7 m circle on him
Hype Panel     = moderator + two seated panellists (7000 HP each), linked HP and one death like
                 the twins; two wear Waiting to Talk (-80% damage taken); every 15 s the mic
                 passes in seat order (moderator, VC, Chief AI Officer), nobody moves;
                 More of a Comment: 3 s must-interrupt, 180 to everyone within 45 m
One More Thing = 66%: AGI (Beta) (2000 HP, must-interrupt 200 nuke), again every 40 s; 33%:
(Keynote)        Enterprise Pricing (+25% damage) and Auto-Renewing Subscription (60 + 30 per
                 2 s for 10 s, Magic); This Changes Everything: 9 m circle on him
enrage         = Running Over Time after 5 min (keynote 6 min)
```

Raid tier: 16 set pieces and five Legendaries, ilvl 14, same budget as the offsite tier. Three new looks: the conference VR headset (headgear style 15), the branded conference hoodie (hoodie 10) and the neon lanyard with a QR badge (lanyard 9).

Measured (`tests/coop.rs`, `vibecon_takes_a_group_and_async_attendance_takes_one`, same bots and seeds):

| run | Expo Hall | Workshops | Hackathon | Afterparty | Main Stage | whole raid |
|---|---|---|---|---|---|---|
| trio, Normal | 261-272 s, low 18-61% | 222-230 s, low 42-53% | 181-193 s, low 67-69% | 165-197 s, low 19-32% | 295-301 s, low 8-38% | 5/5 bosses, 1199-1217 s, nobody died |
| solo DPS, Async | 177-196 s, low 47-49% | 147-157 s, low 45-48% | 128-144 s, low 47-51% | 125-150 s, low 44-48% | 166-200 s, low 33-36% | 5/5 bosses, 791-816 s, nobody died |
| solo DPS, Normal | died (PR merges) | - | - | - | - | 0/5, wipes at the Vibe Coder |

## Weekly Sprint ladder

```
week          = ISO week of the sim clock (live::week_index; the server's online)
weekly sprint = building = buildings[rng(week seed) % n], level = sprint.weekly_level (7),
                affixes = affixes_for(week seed, level) (three slots at +7), stages drawn from
                the week seed: every group plays the same run all week
ladder        = delivered weekly runs of the week, fastest first (ties: fewer deaths), top 10;
                crew labels "Name (Class)" (server names online, class alone offline)
storage       = offline: the save's ladder key (ladder=<week>*<ms>~<deaths>~<crew+crew>|...);
                online: the room's sim (per-room board), sent to joiners; a new week empties it
```

## Minigame: Teambuilding

```
start         = group lead on the campus, everyone alive and out of combat, no sprint;
                travel 1.2 s later; every member gets a run on arrival
patrol        = loop of waypoints (route:<r>:<n>) at speed_mps, pause_ms at each (scanning
                +-0.75 rad over 4.2 s), longer pauses per waypoint; a pure function of the
                time since arrival, so every peer sees the same party
seen          = inside range_m and +-half_angle_deg of the facing, sight line eye 1.55 m to
                chest 1.1 m clear of blockers (tables do not hide, hedges and banners do)
suspicion    += rate per second while seen or heard, else -0.5 per second; 1.0 is a catch
rate          = 1000 / notice_ms * (1 + 2 * (1 - d / range))   (edge 0.9 /s, close 2.7 /s)
              = instant for the CEO; heard (speed > noisy_mps 5 within noise_m 7, clear line)
                0.8 * 1000 / notice_ms
speed         = distance over the last 15 ticks (0.5 s); a step > 4 m is a teleport
hidden        = within hide_radius_m 1.3 of a hiding spot and slower than 1.5 m/s
sneak         = client run speed * 0.5 (7 -> 3.5 m/s), toggled with jump in the venue
catch         = manager / CEO: back to tb_start; talker: stun tb_cornered 5 s with an AI take;
                HR: tb_icebreaker and stun 3 s; then grace_ms 2.5 s after the stun; the
                objective being held starts over
objectives    = stand dwell_ms inside radius_m: canape 1.5 s, drink 2 s, swag 1.5 s,
                photo 0.8 s, prod incident 20 s; then the exit (radius 3 m) ends the run
score         = max(0, 2400 + 60 * canapes - 250 * catches - 3 * whole seconds)
medals        = Bronze "Attended": finish; Silver "Networked": score 1300+;
                Gold "Was Never Seen": score 1600+ and no catch
rewards       = HR standing 60 / 120 / 200, burnout relief 5 / 10 / 15 %, items once per
                character (Bronze: name tag; Silver: + Party Leftovers Container carrier;
                Gold: + Party Hat and the title "Ghosted the Teambuilding")
time limit    = 900 s, then the run ends with no medal; leaving early: no medal, no score
group score   = sum of the finished runs' scores; the group returns to the campus 10 s after
                the last run ends
save          = minigames=<key>*<medal 0..2>|...  (best medal per minigame)
```

## Minigames: the Company Calendar's five

```
shared        = the Teambuilding frame (start, travel, time limit, group score, return,
                save); one run per member; the card's options answer with MinigamePick
                {seq, option} (a stale seq is ignored); medals pay standing and relief
                every time, items once (skipped when owned), titles, Kudos and trophies
                once (achievements)
review        = 9 questions, 4 answers each in a shuffled order per player; 20 s to answer,
                else Silence; the reply stays 4.5 s (or Continue); verdict 15 s
  tones       = (minutes, patience, gauge in tenths of a star)
                Honest 2/10/-8  Buzz 6/25/+6  Stall 9/35/-2  Agree 1/-20/0  Raise 12/60/-10
                Silence 4/15/-3; patience starts at 100 and never goes above it
  repeat      = the same tone as the last answer: cost * 150 % (a restore halves)
  walkout     = patience <= 0 after an answer: the verdict comes next
  gauge       = 1..5 stars, starts at 3, drifts back to 3 at 0.3 stars per second
  score       = 10 * minutes, halved after a walkout; best line 46 min with the manager
                staying (dynamic programming over the answers)
  medals      = Bronze any finish; Silver 320+; Gold 420+ and no walkout; limit 420 s
reorg         = 5 rounds of 45 s after a 4 s shuffle (round 1 starts at once); a round ends
                6 s after every open run found its manager
  deal        = 6 colleagues on 6 of 8 desk slots, names and roles dealt from 8 each, one
                manager; one colleague tells the manager's slot, one the role, one the old
                manager's slot (or any other wrong desk), the rest nothing; partitions 1-6
                stand with chance 1/2 each
  talk        = within 2.4 m of a desk slot's colleague
  score       = per find 150 + 4 * whole seconds left on the 45 s clock; wrong guess -50
                (not below 0) and no asking for 2.5 s
  medals      = Bronze 150+; Silver 900+; Gold 1150+ and no wrong guess; limit 360 s
migration     = 20 tickets, export within 2.2 m of term_old, fields within 2.2 m of term_new
  fields      = 2 to 4 per ticket, 4 options each; a wrong pick: +1 error, 3 s spinner
  score       = max(0, 1500 - 75 * errors - 2 * whole seconds)
  medals      = Bronze finish; Silver 850+; Gold 1000+ and no error; limit 480 s
hackathon     = 24 h, 10 s each, Saturday 10:00 to Sunday 10:00; demo on stage for 12 s
  focus       = starts 70, -1.0 per second (-2.0 in hours 16-19, 02:00-06:00), 0..100
  snacks      = 6 pizza (+22, +12 from hour 15) and 4 drinks (+35) within 1.5 m, taken
                only below 85 focus, back after 30 s (per player)
  demo        = +0.55 % per second * focus / 100, at most 100 %
  score       = 10 * whole percent + 5 * snacks
  medals      = Bronze finish; Silver 750+; Gold 1000+; limit 300 s
focus         = 60 s streak at the pod (within 3.6 m); away from it the streak pauses
  walkers     = 4 loops, waypoint 0 far, 1 at the pod: colleague 1.5 m/s waits 6 s / 2 s,
                caller 1.3 m/s 9 s / 3 s, cake 1.1 m/s 14 s / 3 s, warden 1.8 m/s 26 s / 2 s
  arrival     = a walker reaches waypoint 1; the card counts it down from 6 s
  defence     = up 2.5 s, rests 4.5 s; colleague: In a meeting, caller and warden:
                Headphones, cake: Duck; an arrival without it resets the streak
  score       = max(0, 1500 + 15 * deflections - 150 * interruptions - 4 * whole seconds)
  medals      = Bronze finish; Silver 900+; Gold 1200+ and never interrupted; limit 300 s
```

