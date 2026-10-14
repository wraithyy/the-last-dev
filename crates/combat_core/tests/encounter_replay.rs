//! Scripted spec scenario with a fixed seed, compared against a golden event log.
//!
//! Regenerate after an intentional rules/balance change:
//! `UPDATE_GOLDEN=1 cargo test -p combat_core --test encounter_replay`

mod common;

use combat_core::{Command, Event, Sim, Vec3};
use common::*;
use std::fmt::Write as _;
use std::path::PathBuf;

const GOLDEN: &str = "../../content/replays/encounter_seed42.log";
const MAX_TICKS: u64 = 3000;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GOLDEN)
}

/// What a reasonable player does: shield the first Blast, kick the second, otherwise DPS.
#[derive(Default)]
struct Script {
    blasts: u32,
    stopped: bool,
    shield_cast: bool,
    kicked: bool,
    looted: bool,
}

impl Script {
    fn observe(&mut self, mob: u64, e: &Event) {
        if matches!(e, Event::CastStarted { caster, spell, .. } if *caster == mob && *spell == BLAST)
        {
            self.blasts += 1;
        }
        if matches!(e, Event::Looted { .. }) {
            self.looted = true;
        }
    }

    fn act(&mut self, sim: &mut Sim, player: u64, mob: u64) {
        let cast = |sim: &mut Sim, spell| {
            sim.push(Command::Cast {
                caster: player,
                spell,
            })
        };
        let ready = |sim: &Sim, spell| sim.cooldown(player, spell).usable.is_ok();
        let (Some(p), Some(m)) = (sim.unit(player), sim.unit(mob)) else {
            return;
        };
        // Copy what we need so the borrows end before we push commands.
        let (p_casting, m_alive, m_lootable, m_casting) =
            (p.cast.is_some(), m.alive, m.lootable, m.cast.is_some());
        let smell = m.auras.iter().any(|a| a.key == "code_smell");
        if !m_alive {
            if m_lootable && !self.looted {
                sim.push(Command::Loot {
                    looter: player,
                    corpse: mob,
                });
            }
            return;
        }
        if m_casting {
            if self.blasts == 1 && !self.shield_cast {
                if p_casting {
                    if !self.stopped {
                        self.stopped = true;
                        sim.push(Command::StopCasting { caster: player });
                    }
                } else if ready(sim, GR) {
                    self.shield_cast = true;
                    cast(sim, GR);
                    return;
                }
            } else if self.blasts >= 2 && !self.kicked && ready(sim, RPR) {
                self.kicked = true;
                cast(sim, RPR);
            }
        }
        if !p_casting && sim.cooldown(player, CR).gcd_remaining_ms == 0 {
            if smell && ready(sim, RF) {
                cast(sim, RF);
            } else if ready(sim, CR) {
                cast(sim, CR);
            }
        }
    }
}

/// Runs the spec scenario and returns the formatted event log.
/// The original single-CEO spec encounter: the legacy CEO fixture at its old level 3, and
/// the player at level 3 too (so Guardrails and Reject PR are learned and stats come from
/// the level table). Floors are covered by `playthrough.rs`.
fn run_scenario(seed: u64) -> String {
    let mut c = content();
    legacy_ceo(&mut c);
    def_mut(&mut c, "ceo").level = 3;
    let mut sim = Sim::new(c, seed);
    let player = sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
    sim.set_player_level(3);
    let mob = match sim.spawn_unit("ceo", Vec3::new(0.0, 0.0, -20.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    let intern = match sim.spawn_unit("intern", Vec3::new(6.0, 0.0, -4.0), 0.0) {
        Ok(id) => id,
        Err(e) => panic!("{e}"),
    };
    let mut out = format!("# encounter replay, seed {seed}, 30 Hz\n");
    let mut script = Script::default();
    // Intern first: attacking a friendly unit is refused.
    sim.push(Command::SetTarget {
        unit: player,
        target: Some(intern),
    });
    sim.push(Command::Cast {
        caster: player,
        spell: CR,
    });
    // Refused silently: the intern is friendly, so no toggle event appears.
    sim.push(Command::StartAutoAttack { unit: player });
    let mut respawned = false;
    while sim.now().0 < MAX_TICKS && !respawned {
        if sim.now().0 == 1 {
            sim.push(Command::SetTarget {
                unit: player,
                target: Some(mob),
            });
            sim.push(Command::StartAutoAttack { unit: player });
        }
        if sim.now().0 >= 2 {
            script.act(&mut sim, player, mob);
        }
        for e in sim.tick() {
            script.observe(mob, &e);
            respawned |= matches!(e, Event::Respawned { .. });
            let _ = writeln!(out, "{}", log_line(&sim, sim.now().0, &e));
        }
    }
    out
}

#[test]
fn same_seed_gives_identical_log() {
    assert_eq!(run_scenario(42), run_scenario(42));
}

#[test]
fn different_seed_diverges_while_staying_valid() {
    let (a, b) = (run_scenario(42), run_scenario(43));
    assert_ne!(a, b, "loot/crit/blast timing rolls depend on the seed");
    assert!(b.contains("respawns"));
}

#[test]
fn scenario_log_contains_every_spec_beat() {
    let log = run_scenario(42);
    for beat in [
        "failed Code Review: Invalid target.",
        "begins Code Review (2000 ms)",
        "COMBO",
        "Fullstack Engineer auto attack on",
        "Fullstack Engineer auto attack off",
        "Fullstack Engineer misses CEO",
        "gains guardrails",
        "Hallucination Blast for 0 (absorbed 45)",
        "casts Reject PR",
        "interrupted by Fullstack Engineer (lockout 4000 ms)",
        "CEO dies",
        "gains 120 xp",
        "loots CEO",
        "CEO respawns",
    ] {
        assert!(log.contains(beat), "missing beat {beat:?} in:\n{log}");
    }
    assert!(
        !log.contains("Fullstack Engineer dies"),
        "player must survive the scenario"
    );
}

#[test]
fn log_matches_golden_file() {
    let log = run_scenario(42);
    let path = golden_path();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::write(&path, &log) {
            Ok(()) => return,
            Err(e) => panic!("cannot write golden {}: {e}", path.display()),
        }
    }
    let golden = match std::fs::read_to_string(&path) {
        Ok(g) => g,
        Err(e) => panic!(
            "missing golden {} ({e}); run with UPDATE_GOLDEN=1",
            path.display()
        ),
    };
    assert_eq!(
        log, golden,
        "event log drifted; if intended, rerun with UPDATE_GOLDEN=1"
    );
}
