//! Impact-timed presentation of Damage/Miss events.
//!
//! The sim resolves a projectile spell or auto attack instantly. Floating numbers
//! and hit reactions of such attacks are held here until their projectile lands,
//! so they do not appear before the visual does. Health bars and the combat log
//! stay sim-authoritative (immediate); only on-screen numbers and reactions wait.

use crate::combat::{SimReady, SimUnit};
use crate::combat_anim::CombatAnim;
use bevy::prelude::*;
use combat_core::Event;

/// Safety net: a held event is shown after this long even if its projectile never reports back.
pub const MAX_HOLD_SECS: f32 = 1.5;

/// A Damage or Miss event whose visual moment has come (immediately, or at impact).
#[derive(Message, Clone)]
pub struct PresentEvent(pub Event);

struct Held {
    ticket: u64,
    age: f32,
    ev: Event,
}

#[derive(Resource, Default)]
pub struct PresentQueue {
    next: u64,
    held: Vec<Held>,
}

impl PresentQueue {
    /// Identifies one projectile; the projectile carries it and calls `release` on arrival.
    pub fn ticket(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    pub fn hold(&mut self, ticket: u64, ev: Event) {
        self.held.push(Held {
            ticket,
            age: 0.0,
            ev,
        });
    }

    pub fn release(&mut self, ticket: u64) -> Vec<Event> {
        self.take(|h| h.ticket == ticket)
    }

    pub fn expire(&mut self, dt: f32) -> Vec<Event> {
        for h in &mut self.held {
            h.age += dt;
        }
        self.take(|h| h.age >= MAX_HOLD_SECS)
    }

    fn take(&mut self, pred: impl Fn(&Held) -> bool) -> Vec<Event> {
        let (out, keep) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|h| pred(h));
        self.held = keep;
        out.into_iter().map(|h| h.ev).collect()
    }
}

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PresentQueue>()
            .add_message::<PresentEvent>()
            .add_systems(Update, (expire_held, apply_hit_reactions).in_set(SimReady));
    }
}

fn expire_held(
    time: Res<Time>,
    mut queue: ResMut<PresentQueue>,
    mut out: MessageWriter<PresentEvent>,
) {
    for ev in queue.expire(time.delta_secs()) {
        out.write(PresentEvent(ev));
    }
}

/// A miss plays no hit reaction.
fn apply_hit_reactions(
    mut events: MessageReader<PresentEvent>,
    mut anims: Query<(&SimUnit, &mut CombatAnim)>,
) {
    for PresentEvent(ev) in events.read() {
        if let Event::Damage { target, .. } = ev
            && let Some((_, mut anim)) = anims.iter_mut().find(|(su, _)| su.0 == *target)
        {
            anim.hit_seq += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat_core::EntityId;

    fn miss(n: EntityId) -> Event {
        Event::Miss {
            source: 1,
            target: n,
            spell: None,
        }
    }

    #[test]
    fn release_returns_only_that_ticket_in_order() {
        let mut q = PresentQueue::default();
        let (a, b) = (q.ticket(), q.ticket());
        q.hold(a, miss(10));
        q.hold(b, miss(20));
        q.hold(a, miss(11));
        let got = q.release(a);
        assert_eq!(got.len(), 2);
        assert!(matches!(got[0], Event::Miss { target: 10, .. }));
        assert!(matches!(got[1], Event::Miss { target: 11, .. }));
        assert_eq!(q.release(b).len(), 1);
        assert!(q.release(a).is_empty());
    }

    #[test]
    fn expire_flushes_orphans_after_max_hold() {
        let mut q = PresentQueue::default();
        let t = q.ticket();
        q.hold(t, miss(10));
        assert!(q.expire(MAX_HOLD_SECS * 0.5).is_empty());
        assert_eq!(q.expire(MAX_HOLD_SECS * 0.6).len(), 1);
        assert!(q.release(t).is_empty());
    }
}
