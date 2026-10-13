# Combat formulas

Simplified, WotLK-inspired rules for gameplay feel. They are **not** Blizzard's implementation and make no claim to match it. All numbers live in `content/game.ron`; time is integer ticks (1 tick = 1/30 s, `ms_to_ticks` rounds up so a 2000 ms cast never ends early). Randomness comes from one SplitMix64 stream seeded per `Sim`.

## Damage

```
raw      = base + floor(spell_power * coefficient_pct / 100)
combo    : if the spell consumes an aura present on the target
             raw = floor(raw * (100 + bonus_pct) / 100)   // Refactor on Code Smell: +35%
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

There is no continuous out-of-combat mana regen and no 5-second rule. In combat the percent tick does not run; the only regen is the legacy per-tick mana trickle: `acc += mana_regen_per_5s_combat; mana += acc / 150; acc %= 150` (Senior Engineer: 25 mana per 5 s). A unit is in combat while it is in an enemy threat table (player) or has a non-empty table (enemy).

## Threat and AI

Threat = full pre-absorb damage dealt (minimum 1) to the enemy; initial aggro adds 1. Target = highest threat, ties to the lowest unit id; dead units drop out of the table. Applying a harmful aura to an enemy without threat on the caster adds 1 (a DoT pulls at once). Heals generate no threat; self-damage generates none. Social aggro: the first threat on a pack member gives every idle member of the pack 1 threat on the same source. Empty table while engaged, or distance from spawn > `leash_m`, starts Evade: cast cancelled, all auras removed, phase reset to 1, threat dropped, walk home at chase speed, heal to full on arrival (summons despawn instead), damage ignored meanwhile.

AI abilities: each rule's first use is rolled `U[initial_ms]` (default the cooldown range) at the pull, or at the start of the phase it is gated to; afterwards `U[cooldown_min_ms, cooldown_max_ms]` is re-rolled at each use. Hallucination Blast: `U[8000, 12000]` both times. A school lockout, stun or silence blocks the attempt until it ends; rng draws happen only when a rule actually fires (plus `RandomHostile`/`RandomAura` picks).

Yells (boss and player spell yells) are presentation-only: line selection is a round-robin cursor and never draws from the rng (see COMBAT_API.md, Yells).

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

Enemy HP scaling (`config.group_hp_pct_per_player`, default 60):

```
group_pct = 100 + group_hp_pct_per_player * (players - 1)
max_hp    = level_scaled_max_hp * group_pct / 100      // hostile AI units only, not friendlies or players
```

Applied at spawn (zone load, `spawn_unit`, summons) and re-applied to every living enemy when a member joins or leaves, keeping its HP fraction (`hp = hp * new_max / old_max`, at least 1 while alive). Solo is 100% (no change); 5 players is 340%.

Quest rewards add `reward.xp` flat on turn-in: no level-difference adjustment, nothing at the cap, same carry-over and level-up rules as kills.

Zone spawns: packs in the first half of the zone's list spawn at `level_range.0`, the rest at `level_range.1`; bosses and friendlies keep their authored level; summons use the summoner's level. Trash of a zone left for at least `TRASH_RESPAWN_MS` (300,000 ms of sim time) respawns on re-entry and gives XP again.

Thermal Runaway (Cooling Failure) is a stacking DoT: `3 * dmg_pct / 100 * stacks` every 2 s, one stack per pulse (7 s), up to 20; Rollback's dispel takes the Hot Aisle debuff re-applied after it, so the heat cannot be cleansed. Recursive Self-Improvement (AGI phase 3): +8% damage done per stack, one stack every 4 s, up to 25.
