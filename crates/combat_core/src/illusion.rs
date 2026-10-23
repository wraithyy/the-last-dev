//! Per-player illusions (docs/ILMARI_10.md C.3.2). The sim only ever holds the truth: one seeded
//! draw per cycle (the cycle's public `salt`), from which everybody can derive the same real
//! hazards with `real_set`. What a player *believes* on top of that, the fake telegraphs and the
//! tiles they think are safe, is `layout(salt, player, cycle, arena)`: a pure integer function
//! (splitmix hashing, no float, no state, no rng stream) that only presentation calls. Nothing
//! the sim computes ever reads it, which is what the cosmetic-proof test pins.
//!
//! Companions read the sim and so always stand in the truth: `Sim::illusion_for` gives them
//! nothing.

use crate::content::School;
use crate::sim::{Hit, Sim};
use crate::targeting::EntityId;
use serde::{Deserialize, Serialize};

/// Tiles per side of the hazard grid.
pub const GRID: usize = 3;
pub const TILES: usize = GRID * GRID;
/// Real and fake circles per cycle.
pub const CIRCLES: usize = 3;
/// How often a placement is retried before the deterministic fallback.
const TRIES: u64 = 64;

/// The hazard arena: a 3 x 3 grid of square tiles centred on `(x_cm, z_cm)`. Centimetres keep
/// every position an integer, so the wire and the hash never see a float.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IllusionArena {
    pub x_cm: i32,
    pub z_cm: i32,
    pub tile_cm: u32,
}

impl IllusionArena {
    /// Largest tile edge accepted off the wire, centimetres (100 m): keeps the i64 squares of
    /// the layout maths far from overflow whatever a peer sends.
    pub const MAX_TILE_CM: u32 = 10_000;

    pub fn sane(&self) -> bool {
        (1..=Self::MAX_TILE_CM).contains(&self.tile_cm)
            && self.x_cm.unsigned_abs() < 100_000_000
            && self.z_cm.unsigned_abs() < 100_000_000
    }

    fn half_cm(&self) -> i64 {
        i64::from(self.tile_cm) * GRID as i64 / 2
    }

    /// Centre of tile `k` (row-major, bit `k` of a mask).
    pub fn tile_center(&self, k: usize) -> (i64, i64) {
        let t = i64::from(self.tile_cm);
        let (col, row) = ((k % GRID) as i64, (k / GRID) as i64);
        (
            i64::from(self.x_cm) + (col - 1) * t,
            i64::from(self.z_cm) + (row - 1) * t,
        )
    }

    /// The tile under a world position (centimetres), `None` outside the grid.
    pub fn tile_at(&self, x_cm: i64, z_cm: i64) -> Option<usize> {
        let t = i64::from(self.tile_cm).max(1);
        let h = self.half_cm();
        let (dx, dz) = (
            x_cm - i64::from(self.x_cm) + h,
            z_cm - i64::from(self.z_cm) + h,
        );
        if dx < 0 || dz < 0 || dx >= t * GRID as i64 || dz >= t * GRID as i64 {
            return None;
        }
        Some((dz / t) as usize * GRID + (dx / t) as usize)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Circle {
    pub x_cm: i64,
    pub z_cm: i64,
    pub r_cm: i64,
}

impl Circle {
    fn covers(&self, p: (i64, i64)) -> bool {
        let (dx, dz) = (p.0 - self.x_cm, p.1 - self.z_cm);
        dx * dx + dz * dz <= self.r_cm * self.r_cm
    }
}

/// What actually hurts this cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealSet {
    /// Bit `k` set: tile `k` is `Overflow` (hurts); clear: `Safe`.
    pub overflow: u16,
    pub circles: [Circle; CIRCLES],
}

/// What one player sees on top of the truth: as many fake circles as there are real ones, and
/// the tiles they believe are safe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IllusionLayout {
    pub fakes: [Circle; CIRCLES],
    pub believed_safe: u16,
}

/// The signature presentation calls; a test swaps in a stub to prove the layer is cosmetic.
pub type LayoutFn = fn(u64, EntityId, u32, &IllusionArena) -> IllusionLayout;

/// splitmix64 finaliser over an accumulator: the whole "rng" of this module.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A tiny stream: successive mixes of a seed.
struct Stream(u64);

impl Stream {
    fn next(&mut self) -> u64 {
        self.0 = mix(self.0);
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

fn circle(s: &mut Stream, arena: &IllusionArena) -> Circle {
    let h = arena.half_cm();
    let t = u64::from(arena.tile_cm);
    // Centres roam the grid footprint; radii are 30..=50 percent of a tile so a circle never
    // swallows more than one tile centre.
    let span = (h * 2) as u64 + 1;
    Circle {
        x_cm: i64::from(arena.x_cm) - h + s.below(span) as i64,
        z_cm: i64::from(arena.z_cm) - h + s.below(span) as i64,
        r_cm: (t * (30 + s.below(21)) / 100) as i64,
    }
}

fn tile_free(arena: &IllusionArena, k: usize, circles: &[Circle]) -> bool {
    let c = arena.tile_center(k);
    !circles.iter().any(|x| x.covers(c))
}

/// The truth of a cycle, a pure function of its public salt. The sim damages exactly this; the
/// tests and the layout constraints read it too.
pub fn real_set(salt: u64, arena: &IllusionArena) -> RealSet {
    for attempt in 0..TRIES {
        let mut s = Stream(mix(salt ^ 0x5EA1_0000_0000 ^ attempt));
        let n = 3 + s.below(3) as usize;
        let mut overflow = 0u16;
        while overflow.count_ones() < n as u32 {
            overflow |= 1 << s.below(TILES as u64);
        }
        let circles = [
            circle(&mut s, arena),
            circle(&mut s, arena),
            circle(&mut s, arena),
        ];
        // A real way through must exist: a safe tile that no real circle covers.
        if (0..TILES).any(|k| overflow >> k & 1 == 0 && tile_free(arena, k, &circles)) {
            return RealSet { overflow, circles };
        }
    }
    // Unreachable in practice (each circle covers at most two tile centres); kept total so the
    // function can never panic in the sim: three overflow tiles, circles parked on them.
    let overflow = 0b000_000_111;
    let at = |k| {
        let (x, z) = arena.tile_center(k);
        Circle {
            x_cm: x,
            z_cm: z,
            r_cm: i64::from(arena.tile_cm) / 3,
        }
    };
    RealSet {
        overflow,
        circles: [at(0), at(1), at(2)],
    }
}

/// What `player` believes in `cycle`. Constraints, all enforced here and property-tested:
/// - as many fake circles as real ones, none coincident with a real one;
/// - a way through exists in the player's own view and in the truth (a safe tile free of every
///   circle, real or fake, so the layout is always solvable);
/// - of the tiles the player believes safe at least one really is and at least one really is not,
///   so nobody is ever fully right or fully wrong and comparing notes is the best strategy.
///
/// Depends on `(salt, player, cycle, arena)` only: adding a player never changes another's view.
pub fn layout(salt: u64, player: EntityId, cycle: u32, arena: &IllusionArena) -> IllusionLayout {
    let real = real_set(salt, arena);
    let seed = mix(mix(salt ^ 0x1111_0000_0000) ^ player).wrapping_add(u64::from(cycle));
    for attempt in 0..TRIES {
        let mut s = Stream(mix(seed ^ attempt));
        let fakes = [
            circle(&mut s, arena),
            circle(&mut s, arena),
            circle(&mut s, arena),
        ];
        // "Not coincident" is read generously: a fake must sit visibly apart from every real
        // circle (more than half the larger radius between centres), not merely differ by a cm.
        let apart = fakes.iter().all(|f| {
            real.circles.iter().all(|r| {
                let (dx, dz) = (f.x_cm - r.x_cm, f.z_cm - r.z_cm);
                let min = f.r_cm.max(r.r_cm) / 2;
                dx * dx + dz * dz > min * min
            })
        });
        let all: Vec<Circle> = real.circles.iter().chain(fakes.iter()).copied().collect();
        let route = (0..TILES).any(|k| real.overflow >> k & 1 == 0 && tile_free(arena, k, &all));
        if !(apart && route) {
            continue;
        }
        let safe: Vec<usize> = (0..TILES).filter(|k| real.overflow >> k & 1 == 0).collect();
        let bad: Vec<usize> = (0..TILES).filter(|k| real.overflow >> k & 1 == 1).collect();
        let mut believed = 1u16 << safe[s.below(safe.len() as u64) as usize];
        believed |= 1 << bad[s.below(bad.len() as u64) as usize];
        let want = 3 + s.below(3) as u32;
        while believed.count_ones() < want {
            believed |= 1 << s.below(TILES as u64);
        }
        return IllusionLayout {
            fakes,
            believed_safe: believed,
        };
    }
    // Total fallback (never hit in the property test): the fakes sit on real overflow tiles,
    // shifted so they are not coincident, and the belief is one tile of each kind.
    let bad: Vec<usize> = (0..TILES).filter(|k| real.overflow >> k & 1 == 1).collect();
    let safe = (0..TILES)
        .find(|k| real.overflow >> k & 1 == 0)
        .unwrap_or(0);
    let on = |k: usize| {
        let (x, z) = arena.tile_center(k);
        Circle {
            x_cm: x + 37,
            z_cm: z + 37,
            r_cm: i64::from(arena.tile_cm) / 4,
        }
    };
    IllusionLayout {
        fakes: [on(bad[0]), on(bad[1 % bad.len()]), on(bad[2 % bad.len()])],
        believed_safe: 1 << safe | 1 << bad[0],
    }
}

/// The current cycle and a hazard still waiting to resolve.
#[derive(Default)]
pub(crate) struct IllusionState {
    cycle: u32,
    cur: Option<(u64, IllusionArena)>,
    /// (due tick, source unit, damage) of the real hazards of the current cycle.
    resolve: Option<(u64, EntityId, u32)>,
}

impl Sim {
    /// Starts a `Confabulate`-style cycle: exactly one seeded draw whatever the group size, so
    /// the stream does not depend on how many players are in the room. The real hazards hurt
    /// anyone standing on them `warn_ms` later (`damage` from `source`). Returns the cycle.
    pub fn illusion_begin(
        &mut self,
        arena: IllusionArena,
        source: EntityId,
        damage: u32,
        warn_ms: u32,
    ) -> u32 {
        let salt = self.rng.next_u64();
        self.illusion.cycle += 1;
        self.illusion.cur = Some((salt, arena));
        self.illusion.resolve = Some((self.now + crate::content::ticks(warn_ms), source, damage));
        let cycle = self.illusion.cycle;
        self.events
            .push(crate::types::Event::IllusionCycle { cycle, salt, arena });
        cycle
    }

    /// The cycle everybody shares, or `None` outside one.
    pub fn illusion_cycle(&self) -> Option<(u32, u64, IllusionArena)> {
        self.illusion
            .cur
            .map(|(salt, arena)| (self.illusion.cycle, salt, arena))
    }

    /// The truth of the current cycle.
    pub fn illusion_real(&self) -> Option<RealSet> {
        self.illusion
            .cur
            .map(|(salt, arena)| real_set(salt, &arena))
    }

    /// What `viewer` is shown. Only group members see illusions: a companion (or any other
    /// unit) gets `None` and reads the real set. Takes `&self`, so it cannot change the sim.
    pub fn illusion_for(&self, viewer: EntityId, f: LayoutFn) -> Option<IllusionLayout> {
        if !self.is_player(viewer) {
            return None;
        }
        let (cycle, salt, arena) = self.illusion_cycle()?;
        Some(f(salt, viewer, cycle, &arena))
    }

    /// A late joiner learns the running cycle (`raid_on_join`).
    pub(crate) fn illusion_on_join(&mut self, player: EntityId) {
        if let Some((cycle, salt, arena)) = self.illusion_cycle() {
            self.emit_to(
                player,
                crate::types::Event::IllusionCycle { cycle, salt, arena },
            );
        }
    }

    /// Ends the running cycle (a wipe, the boss's death or a reset).
    pub fn illusion_end(&mut self) {
        self.illusion.cur = None;
        self.illusion.resolve = None;
    }

    pub(crate) fn update_illusion(&mut self) {
        let Some((due, source, damage)) = self.illusion.resolve else {
            return;
        };
        if self.now < due {
            return;
        }
        self.illusion.resolve = None;
        let Some(real) = self.illusion_real() else {
            return;
        };
        let Some((_, arena)) = self.illusion.cur else {
            return;
        };
        // Friendly units only: players and companions both stand in the truth.
        let hit: Vec<usize> = (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                if u.despawned || !u.state.alive || u.state.hostile || u.state.id == source {
                    return false;
                }
                let p = (
                    (u.state.pos.x * 100.0).round() as i64,
                    (u.state.pos.z * 100.0).round() as i64,
                );
                arena
                    .tile_at(p.0, p.1)
                    .is_some_and(|k| real.overflow >> k & 1 == 1)
                    || real.circles.iter().any(|c| c.covers(p))
            })
            .collect();
        for i in hit {
            let target = Self::id_of(i);
            self.deal_damage(
                source,
                target,
                Hit {
                    spell: None,
                    amount: damage,
                    crit: false,
                    combo: false,
                    school: School::Arcane,
                    dot: false,
                },
            );
        }
    }
}
