//! Props only one class can use. Bonus only: no quest objective may need one (a content test
//! pins that), so every class still clears the campaign alone; in co-op they are a reason to
//! bring the right colleague, and `party` effects land on everyone in the zone.
//!
//! Fullstack "knows everything a little": it may use any class's prop, but the cooldown is
//! three times longer, buffs last half as long and a fixed share of tries breaks the prop
//! (a funny failure, retry later). Every roll comes from the world stream, so co-op peers and
//! replays agree.

use super::*;

/// Outcome flag: the user was the wrong class and nothing happened (low bits pick the line).
pub const PROP_REFUSED: u32 = 1 << 31;
/// Outcome flag: a Fullstack used another class's prop.
pub const PROP_FULLSTACK: u32 = 1 << 30;
/// Low-bit outcomes of a class prop, besides 0 (it broke).
pub const PROP_OK: u32 = 1;
pub const PROP_DONE: u32 = 2;
/// Extra chance, in percent, that Fullstack breaks a prop of another class.
pub const FULLSTACK_FAIL_PCT: u32 = 30;
const FULLSTACK_CLASS: &str = "fullstack";
/// Wrong-class clicks only repeat their line this often.
const REFUSE_COOLDOWN_MS: u32 = 1500;

impl Sim {
    pub(crate) fn use_class_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        let own = self.class_at(pi).map(|c| c.key.clone()).unwrap_or_default();
        let native = own == def.class;
        let fullstack = !native && own == FULLSTACK_CLASS;
        let now = self.now;
        if self
            .fun
            .players
            .entry(player)
            .or_default()
            .next_use
            .get(&k)
            .is_some_and(|t| *t > now)
        {
            return;
        }
        // A shortcut out of a fight would be a free escape, like the shuttle.
        if (native || fullstack) && !def.to.is_empty() && self.units[pi].state.in_combat {
            return self.item_error(player, ItemError::InCombat);
        }
        let wait = match (native, fullstack) {
            (true, _) => def.cooldown_ms,
            (_, true) => def.cooldown_ms.saturating_mul(3),
            _ => REFUSE_COOLDOWN_MS,
        };
        self.fun
            .players
            .entry(player)
            .or_default()
            .next_use
            .insert(k, now + ticks(wait));
        let line = self.fun.rng.below(1 << 16);
        let flag = if fullstack { PROP_FULLSTACK } else { 0 };
        let say = |sim: &mut Sim, outcome: u32| {
            sim.events.push(Event::Interacted {
                unit: player,
                prop: def.key.clone(),
                outcome,
            });
        };
        if !native && !fullstack {
            return say(self, PROP_REFUSED | line);
        }
        if def.once && self.prop_used(player, k) {
            return say(self, PROP_DONE | flag);
        }
        let fail = if fullstack {
            def.fail_pct.max(FULLSTACK_FAIL_PCT)
        } else {
            def.fail_pct
        };
        if self.fun.rng.chance_pct(fail) {
            return say(self, flag | (line & 0xFFFC));
        }
        let targets: Vec<usize> = if def.party {
            self.players
                .clone()
                .into_iter()
                .filter_map(|p| self.index(p))
                .filter(|i| self.units[*i].state.alive)
                .collect()
        } else {
            vec![pi]
        };
        let dest = (!def.to.is_empty())
            .then(|| self.prop_spot(&def.to))
            .flatten();
        for ti in targets {
            let tid = Self::id_of(ti);
            if def.once && self.prop_used(tid, k) {
                continue;
            }
            for (aura, ms) in &def.auras {
                let ms = if fullstack { ms / 2 } else { *ms };
                self.apply_aura(ti, aura, ms, None, player, None);
            }
            if !def.items.is_empty() {
                self.stow(ti, def.items.clone());
            }
            if def.kudos > 0 {
                let s = &mut self.units[ti].state;
                s.sp = s.sp.saturating_add(def.kudos);
            }
            if !def.items.is_empty() || def.kudos > 0 {
                self.bag_event(ti);
            }
            if let Some(pos) = dest {
                self.dismount(ti);
                self.units[ti].state.pos = pos;
                self.events.push(Event::Teleported { unit: tid, pos });
            }
            self.burnout_on_prop(ti, k);
            if def.once {
                self.mark_prop_used(tid, k);
            }
        }
        say(self, PROP_OK | flag);
        self.quest_credit(
            player,
            |o| matches!(o, crate::content::Objective::Interact(key, _) if *key == def.key),
        );
    }
}
