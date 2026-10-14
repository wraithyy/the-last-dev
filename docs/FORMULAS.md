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

Player stats at level `L` = `senior_engineer` stats + cumulative `levels[L-1]` bonus (HP +35, mana +40, spell power +10 per level; level 10: 615 HP, 610 mana, 110 SP). XP to leave level `L` is `levels[L-1].xp_to_next` (400, 600, ... 2000; 0 at the cap).

Monster scaling when spawned at level `M` with authored level `A` (`config.mob_scaling`):

```
pct(per) = max(10, 100 + per * (M - A))
max_hp   = max_hp * pct(hp_pct_per_level = 12) / 100
dmg_pct  = pct(damage_pct_per_level = 10)            // multiplies melee, spells, heals, DoTs it deals
reward   = xp_reward * pct(xp_pct_per_level = 10) / 100
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

Quest rewards add `reward.xp` flat on turn-in: no level-difference adjustment, nothing at the cap, same carry-over and level-up rules as kills.

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
```

Story Points per kill and member: `(level * sp_per_level + U[0, level]) * mult` (elite 4, boss 12; summons 0). Gear drop chance: trash 12%, elite 60%, boss 100%; rarity weights Common/Uncommon/Rare/Epic trash 50/35/13/2, elite 10/45/37/8, boss 0/25/55/20; the piece is from the mob's tier (`ceil(level / 2)`), from the looter's role 70% of the time. Bosses with named Legendaries add one 25% of the time; non-bosses drop junk 30%. Quest turn-ins pay `xp * 15%` SP. Prices: `base(rarity) * (2 + ilvl) / 4` with base 8/20/50/120/300 unless authored; sell price 25% (junk 100%).

Measured (combat_core tests, level-capped bots, 3 seeds): a full set of tier-appropriate Rare gear cuts boss kill time to 72-80% of fresh and raises the boss-fight minimum HP by 13-32 points; Code Review damage over 10 s goes up by about 60% at tier 5.

## Talents

- Splash: extra targets within the radius of the primary target (nearest first, enemies already in combat, injured allies for heals) take `pct` of the spell's damage or healing; DoTs and HoTs land at full strength.
- `CastTime` adds to the cast-time aura percent after haste (-100 = instant); `ManaCost` subtracts from 100% of the cost; `HpCost` pays `mana * pct / 100` HP instead (never below 1 HP); `Periodic` scales the snapshotted tick; `DamageTaken` adds to the damage-taken percent with auras and class armour.
- Performance Review: `respec_sp_per_level * level` Story Points (15 per level: 150 SP at level 10, about 3% of a campaign's SP).
- Balance gate (`every_talent_build_clears_the_campaign`): each class clears the campaign on 3 seeds with options 1, 2 and 3 in every row; total boss time may not drop below 80% of the fastest class without talents.
