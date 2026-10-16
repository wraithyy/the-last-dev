//! Tool Migration: twenty tickets move from the old ticket system to the new one. At the old
//! terminal (`term_old`) you export the next ticket; at the new one (`term_new`) every ticket
//! asks for its own mandatory fields, each a quick pick of a few options. The right option
//! follows from the ticket's text; a wrong one is a validation error that costs a mistake and
//! a few seconds of spinner. After the twentieth ticket the company announces that it is
//! migrating back. Score: a base, minus mistakes and seconds (docs/FORMULAS.md).

use super::{Fx, line, xz};
use crate::content::{ContentError, ticks};
use crate::geom::Vec3;
use crate::minigame::{Card, GameRun, Gauge, MinigameView};
use crate::sim::Sim;
use crate::targeting::EntityId;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct FieldDef {
    pub key: String,
    /// "Business impact (mandatory)".
    pub ask: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TicketDef {
    pub id: u32,
    pub text: String,
    /// (field key, the right option's index), in the order the new tool asks.
    pub fields: Vec<(String, u8)>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct MigrationScore {
    pub base: u32,
    pub per_mistake: u32,
    pub per_second: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MigrationDef {
    pub old_name: String,
    pub new_name: String,
    pub reach_m: f32,
    /// The old tool's only button.
    pub export: String,
    pub fields: Vec<FieldDef>,
    pub tickets: Vec<TicketDef>,
    /// Validation errors, in turn.
    pub errors: Vec<String>,
    /// The spinner after a validation error.
    pub error_ms: u32,
    /// The new tool's thanks, in turn.
    pub saved: Vec<String>,
    /// The all-staff email after the last ticket, and the only answer to it.
    pub twist_from: String,
    pub twist: String,
    pub twist_ok: String,
    pub score: MigrationScore,
}

impl MigrationDef {
    pub(crate) fn validate(&self) -> Result<(), ContentError> {
        let bad = |m: String| Err(ContentError::Invalid(format!("migration: {m}")));
        if self.tickets.is_empty() || self.errors.is_empty() || self.saved.is_empty() {
            return bad("tickets, errors and saved lines are required".into());
        }
        for f in &self.fields {
            if f.options.len() < 2 || f.options.len() > crate::minigame::CARD_OPTIONS_MAX {
                return bad(format!("field {} needs 2 to 4 options", f.key));
            }
        }
        for t in &self.tickets {
            if t.fields.is_empty() {
                return bad(format!("ticket {} has no fields", t.id));
            }
            for (k, right) in &t.fields {
                match self.field(k) {
                    Some(f) if usize::from(*right) < f.options.len() => {}
                    _ => return bad(format!("ticket {}: bad field {k}", t.id)),
                }
            }
        }
        Ok(())
    }

    pub fn field(&self, key: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.key == key)
    }
}

pub(crate) struct Stage {
    old: Vec3,
    new: Vec3,
}

impl Stage {
    pub(crate) fn resolve(
        _d: &MigrationDef,
        anchors: &dyn Fn(&str) -> Option<(Vec3, f32)>,
    ) -> Result<Stage, String> {
        let need = |n: &str| anchors(n).map(|a| a.0).ok_or(format!("no anchor {n}"));
        Ok(Stage {
            old: need("term_old")?,
            new: need("term_new")?,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum At {
    #[default]
    Nowhere,
    Old,
    New,
}

/// One player's migration.
#[derive(Default)]
pub(crate) struct Run {
    /// Tickets done; the next one is `tickets[done]`.
    done: usize,
    carrying: bool,
    /// The field being filled in on the carried ticket.
    field: usize,
    mistakes: u8,
    lock_until: u64,
    at: At,
    seq: u16,
    twist: bool,
}

impl Run {
    pub(crate) fn tallies(&self) -> (u8, u8) {
        (self.mistakes, self.done.min(255) as u8)
    }

    pub(crate) fn summary(&self, d: &MigrationDef) -> String {
        format!(
            "Migrated {} of {} tickets with {} validation errors. They are migrating back on Monday.",
            self.done,
            d.tickets.len(),
            self.mistakes
        )
    }

    fn score(&self, d: &MigrationDef, elapsed_ms: u32) -> u32 {
        let s = &d.score;
        let minus = i64::from(s.per_mistake) * i64::from(self.mistakes)
            + i64::from(s.per_second) * i64::from(elapsed_ms / 1000);
        (i64::from(s.base) - minus).max(0) as u32
    }

    fn card(&self, d: &MigrationDef, now: u64) -> Option<Card> {
        let n = d.tickets.len();
        let card = |who: &str, text: String, options: Vec<String>| Card {
            seq: self.seq,
            who: who.to_string(),
            text,
            options,
        };
        if self.twist {
            return Some(card(
                &d.twist_from,
                d.twist.clone(),
                vec![d.twist_ok.clone()],
            ));
        }
        let t = d.tickets.get(self.done)?;
        Some(match (self.at, self.carrying) {
            (At::Nowhere, _) => return None,
            (At::Old, false) => card(
                &d.old_name,
                format!("Ticket {}/{n}  #{}\n{}", self.done + 1, t.id, t.text),
                vec![d.export.clone()],
            ),
            (At::Old, true) => card(
                &d.old_name,
                format!(
                    "Ticket #{} is on your clipboard. The new tool is across the room.",
                    t.id
                ),
                Vec::new(),
            ),
            (At::New, false) => card(
                &d.new_name,
                "Nothing to paste. The next ticket is in the old tool.".into(),
                Vec::new(),
            ),
            (At::New, true) if now < self.lock_until => card(
                &d.new_name,
                format!("#{}  {}\n\nValidating... (the spinner spins)", t.id, t.text),
                Vec::new(),
            ),
            (At::New, true) => {
                let (key, _) = &t.fields[self.field.min(t.fields.len() - 1)];
                let f = d.field(key)?;
                card(
                    &d.new_name,
                    format!(
                        "#{}  {}\n\nMandatory field {}/{}: {}",
                        t.id,
                        t.text,
                        self.field + 1,
                        t.fields.len(),
                        f.ask
                    ),
                    f.options.clone(),
                )
            }
        })
    }

    pub(crate) fn view(&self, d: &MigrationDef, now: u64, mut base: MinigameView) -> MinigameView {
        let n = d.tickets.len();
        let todo = if self.twist {
            "All tickets migrated. Read your email."
        } else if self.carrying {
            "Paste it into the new tool (east)"
        } else {
            "Export the next ticket from the old tool (west)"
        };
        base.lines = vec![
            format!("Tickets migrated: {}/{n}", self.done),
            todo.into(),
            format!("Validation errors: {}", self.mistakes),
        ];
        base.gauges = vec![Gauge {
            label: "Migration progress".into(),
            value: (self.done * 100 / n.max(1)) as u8,
            mark: None,
        }];
        base.card = self.card(d, now);
        // Bit 0: carrying a ticket (the client marks the terminal it goes to).
        base.layout = u32::from(self.carrying && !self.twist);
        base.catches = self.mistakes;
        base.collected = self.done.min(255) as u8;
        base.score = self.score(d, base.elapsed_ms);
        base
    }
}

impl Sim {
    pub(crate) fn update_migration(&mut self) {
        let open = self.open_runs();
        let Some(o) = self.minigame.outing.as_mut() else {
            return;
        };
        let Some(d) = self.content.minigames[o.def].migration.as_ref() else {
            return;
        };
        let crate::corporate::Stage::Migration(st) = &o.stage else {
            return;
        };
        for (p, pos) in open {
            let Some(GameRun::Migration(run)) = o.runs.get_mut(&p).map(|r| &mut r.game) else {
                continue;
            };
            let at = if xz(pos, st.old) <= d.reach_m {
                At::Old
            } else if xz(pos, st.new) <= d.reach_m {
                At::New
            } else {
                At::Nowhere
            };
            if at != run.at {
                run.at = at;
                run.seq = run.seq.wrapping_add(1);
            }
        }
    }

    pub(crate) fn migration_pick(&mut self, p: EntityId, seq: u16, option: u8) -> Vec<Fx> {
        let now = self.now;
        let elapsed = self.minigame_elapsed_ms();
        let Some(o) = self.minigame.outing.as_mut() else {
            return Vec::new();
        };
        let Some(d) = self.content.minigames[o.def].migration.as_ref() else {
            return Vec::new();
        };
        let Some(GameRun::Migration(run)) = o.runs.get_mut(&p).map(|r| &mut r.game) else {
            return Vec::new();
        };
        if seq != run.seq {
            return Vec::new();
        }
        let mut fx = Vec::new();
        if run.twist {
            fx.push(Fx::Finish(run.score(d, elapsed), run.mistakes == 0));
            return fx;
        }
        let Some(t) = d.tickets.get(run.done) else {
            return fx;
        };
        match (run.at, run.carrying) {
            (At::Old, false) => {
                run.carrying = true;
                run.field = 0;
                fx.push(Fx::Note(
                    format!("Exported #{}. Ctrl+C, the old way.", t.id),
                    true,
                ));
            }
            (At::New, true) if now >= run.lock_until => {
                let Some((_, right)) = t.fields.get(run.field) else {
                    return fx;
                };
                if option == *right {
                    run.field += 1;
                    if run.field >= t.fields.len() {
                        run.carrying = false;
                        run.done += 1;
                        fx.push(Fx::Note(
                            format!("#{} migrated. {}", t.id, line(&d.saved, run.done)),
                            true,
                        ));
                        if run.done >= d.tickets.len() {
                            run.twist = true;
                        }
                    }
                } else {
                    run.mistakes = run.mistakes.saturating_add(1);
                    run.lock_until = now + ticks(d.error_ms);
                    fx.push(Fx::Note(
                        line(&d.errors, usize::from(run.mistakes) - 1),
                        false,
                    ));
                }
            }
            _ => return fx,
        }
        run.seq = run.seq.wrapping_add(1);
        fx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mistakes_and_time_cost_points_but_never_below_zero() {
        let d = MigrationDef {
            old_name: String::new(),
            new_name: String::new(),
            reach_m: 2.0,
            export: String::new(),
            fields: Vec::new(),
            tickets: Vec::new(),
            errors: Vec::new(),
            error_ms: 0,
            saved: Vec::new(),
            twist_from: String::new(),
            twist: String::new(),
            twist_ok: String::new(),
            score: MigrationScore {
                base: 1000,
                per_mistake: 50,
                per_second: 2,
            },
        };
        let mut r = Run::default();
        assert_eq!(r.score(&d, 100_000), 800);
        r.mistakes = 3;
        assert_eq!(r.score(&d, 100_000), 650);
        assert_eq!(r.score(&d, 900_000), 0);
    }
}
