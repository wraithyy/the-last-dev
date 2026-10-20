//! Shared, read-only game data: the content file and each zone's gameplay geometry.
//!
//! Geometry comes from `content/zone_geometry.ron`, exported by a `game_client` test from the
//! client's own layout builder (which depends on Bevy), so the server and client agree on
//! anchors, walls and door triggers without the server linking Bevy.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use combat_core::{Aabb, Content, Sim, Terrain, Vec3};
use serde::Deserialize;

const EMBEDDED_CONTENT: &str = include_str!("../../../content/game.ron");
const EMBEDDED_GEOMETRY: &str = include_str!("../../../content/zone_geometry.ron");

/// Matches the client's collision: boxes lower than this under the feet are stepped onto.
const STEP_HEIGHT: f32 = 0.3;
const BODY_HEIGHT: f32 = 1.8;
/// Every layout fits in this rectangle (game_client `FLOOR_HALF_X/Z`).
const HALF_X: f32 = 234.0;
const HALF_Z: f32 = 168.0;
const BOUNDS_MARGIN: f32 = 1.0;
/// The server owns height: a claimed y is measured from the zone's ground (`Terrain`, flat
/// y = 0 indoors) and only has to fit the jump arc (`JUMP_SPEED^2 / 2 GRAVITY` = 1.28 m in the
/// client) plus slack. A larger window would let a client lift itself over walls, because
/// blockers are tested at the claimed y.
const MAX_Y: f32 = 1.5;
const MIN_Y: f32 = -0.1;
/// Boxes shrink by this for the movement segment test, so sliding along a wall or rounding a
/// corner at the body radius never grazes it through float noise.
const SEGMENT_MARGIN: f32 = 0.05;

type P3 = [f32; 3];

#[derive(Deserialize)]
struct RawZone {
    id: String,
    anchors: Vec<(String, P3, f32)>,
    blockers: Vec<(P3, P3)>,
    triggers: Vec<(String, P3, P3)>,
    arrivals: Vec<(String, P3, f32)>,
    #[serde(default)]
    terrain: Terrain,
}

fn v(p: P3) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

fn aabb((min, max): (P3, P3)) -> Aabb {
    Aabb {
        min: v(min),
        max: v(max),
    }
}

pub struct ZoneGeom {
    anchors: HashMap<String, (Vec3, f32)>,
    pub blockers: Vec<Aabb>,
    triggers: HashMap<String, Aabb>,
    /// Arrival point keyed by the zone the group comes from.
    arrivals: HashMap<String, (Vec3, f32)>,
    /// min x, max x, min z, max z
    bounds: [f32; 4],
    pub terrain: Terrain,
}

impl ZoneGeom {
    fn from_raw(r: RawZone) -> Self {
        let blockers: Vec<Aabb> = r.blockers.into_iter().map(aabb).collect();
        // The outer walls enclose every zone, so their extent is the walkable area.
        let mut bounds = [f32::MAX, f32::MIN, f32::MAX, f32::MIN];
        for b in &blockers {
            bounds = [
                bounds[0].min(b.min.x),
                bounds[1].max(b.max.x),
                bounds[2].min(b.min.z),
                bounds[3].max(b.max.z),
            ];
        }
        let bounds = [
            (bounds[0] - BOUNDS_MARGIN).max(-HALF_X - BOUNDS_MARGIN),
            (bounds[1] + BOUNDS_MARGIN).min(HALF_X + BOUNDS_MARGIN),
            (bounds[2] - BOUNDS_MARGIN).max(-HALF_Z - BOUNDS_MARGIN),
            (bounds[3] + BOUNDS_MARGIN).min(HALF_Z + BOUNDS_MARGIN),
        ];
        Self {
            anchors: r
                .anchors
                .into_iter()
                .map(|(n, p, y)| (n, (v(p), y)))
                .collect(),
            blockers,
            triggers: r
                .triggers
                .into_iter()
                .map(|(n, a, b)| (n, aabb((a, b))))
                .collect(),
            arrivals: r
                .arrivals
                .into_iter()
                .map(|(n, p, y)| (n, (v(p), y)))
                .collect(),
            bounds,
            terrain: r.terrain,
        }
    }

    pub fn anchor(&self, name: &str) -> Option<(Vec3, f32)> {
        self.anchors.get(name).copied()
    }

    pub fn trigger(&self, anchor: &str) -> Option<&Aabb> {
        self.triggers.get(anchor)
    }

    pub fn arrival(&self, from: &str) -> Option<(Vec3, f32)> {
        self.arrivals.get(from).copied()
    }

    /// Inside the zone's walls and at a height a jump can reach from the ground there.
    pub fn in_bounds(&self, p: Vec3) -> bool {
        let [x0, x1, z0, z1] = self.bounds;
        let above = p.y - self.ground(p.x, p.z);
        (x0..=x1).contains(&p.x) && (z0..=z1).contains(&p.z) && (MIN_Y..=MAX_Y).contains(&above)
    }

    /// Walkable ground height, the same function the sim and the client use.
    pub fn ground(&self, x: f32, z: f32) -> f32 {
        self.terrain.height(x, z)
    }

    /// The feet point is inside a box the body cannot stand in. The client keeps its centre
    /// a player radius away from walls, so a point test leaves that much slack for jitter.
    pub fn blocked(&self, p: Vec3) -> bool {
        self.blockers
            .iter()
            .any(|b| in_xz(b, p) && b.max.y > p.y + STEP_HEIGHT && b.min.y < p.y + BODY_HEIGHT)
    }
}

impl ZoneGeom {
    /// The straight move from `a` to `b` crosses a box the body cannot pass: no tunnelling
    /// through a thin wall between two legal positions. A box low enough to step onto or jump
    /// over at the higher end's height does not count.
    pub fn segment_blocked(&self, a: Vec3, b: Vec3) -> bool {
        let (lo, hi) = (a.y.min(b.y), a.y.max(b.y));
        self.blockers.iter().any(|bx| {
            bx.max.y > hi + STEP_HEIGHT
                && bx.min.y < lo + BODY_HEIGHT
                && segment_hits_xz(bx, a, b, SEGMENT_MARGIN)
        })
    }
}

/// Slab test of the segment `a`-`b` against the box's interior in the xz plane, shrunk by
/// `margin` on every side.
fn segment_hits_xz(bx: &Aabb, a: Vec3, b: Vec3, margin: f32) -> bool {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (start, delta, min, max) in [
        (a.x, b.x - a.x, bx.min.x + margin, bx.max.x - margin),
        (a.z, b.z - a.z, bx.min.z + margin, bx.max.z - margin),
    ] {
        if min >= max {
            return false;
        }
        if delta.abs() < 1e-6 {
            if start <= min || start >= max {
                return false;
            }
            continue;
        }
        let (ta, tb) = ((min - start) / delta, (max - start) / delta);
        t0 = t0.max(ta.min(tb));
        t1 = t1.min(ta.max(tb));
        if t0 >= t1 {
            return false;
        }
    }
    true
}

pub fn in_xz(b: &Aabb, p: Vec3) -> bool {
    p.x > b.min.x && p.x < b.max.x && p.z > b.min.z && p.z < b.max.z
}

pub struct World {
    pub content: Content,
    zones: HashMap<String, ZoneGeom>,
}

impl World {
    /// Parses and cross-checks content and geometry: every content zone needs geometry, and
    /// loading it into a sim must succeed, so a room can never fail to start later.
    pub fn new(content_src: &str, geometry_src: &str) -> Result<Self, String> {
        let content = Content::from_ron(content_src).map_err(|e| format!("content: {e:?}"))?;
        let raw: Vec<RawZone> =
            ron::from_str(geometry_src).map_err(|e| format!("zone geometry: {e}"))?;
        let zones: HashMap<String, ZoneGeom> = raw
            .into_iter()
            .map(|r| (r.id.clone(), ZoneGeom::from_raw(r)))
            .collect();
        let world = Self { content, zones };
        let mut sim = Sim::new(world.content.clone(), 0);
        for z in &world.content.zones {
            world
                .load(&mut sim, &z.id, None)
                .map_err(|e| format!("zone {}: {e}", z.id))?;
        }
        Ok(world)
    }

    /// The content embedded in the binary, or `content_path` when given (it must match the
    /// clients' build: snapshots index units by their position in the content file).
    pub fn load_from(content_path: Option<&Path>) -> Result<Arc<Self>, String> {
        match content_path {
            Some(p) => {
                let src = std::fs::read_to_string(p)
                    .map_err(|e| format!("cannot read {}: {e}", p.display()))?;
                Self::new(&src, EMBEDDED_GEOMETRY).map(Arc::new)
            }
            None => Self::embedded(),
        }
    }

    /// Parsed once per process.
    pub fn embedded() -> Result<Arc<Self>, String> {
        static W: OnceLock<Result<Arc<World>, String>> = OnceLock::new();
        W.get_or_init(|| Self::new(EMBEDDED_CONTENT, EMBEDDED_GEOMETRY).map(Arc::new))
            .clone()
    }

    pub fn zone(&self, id: &str) -> Option<&ZoneGeom> {
        self.zones.get(id)
    }

    /// `load_zone` with this zone's geometry; `player_spawn` is replaced by `arrival` when
    /// given, so a travelling group lands in front of the door it came through.
    pub fn load(
        &self,
        sim: &mut Sim,
        zone: &str,
        arrival: Option<(Vec3, f32)>,
    ) -> Result<(), String> {
        let g = self.zone(zone).ok_or("no geometry")?;
        sim.set_zone_terrain(zone, g.terrain.clone());
        let anchors = |name: &str| match (name, arrival) {
            ("player_spawn", Some(a)) => Some(a),
            _ => g.anchor(name),
        };
        sim.load_zone(zone, &anchors, g.blockers.clone())
            .map_err(|e| format!("{e:?}"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn embedded_world_loads_every_zone() {
        let w = World::embedded().unwrap();
        for z in &w.content.zones {
            let g = w.zone(&z.id).unwrap();
            for link in &z.links {
                assert!(
                    g.trigger(&link.anchor).is_some(),
                    "{} {}",
                    z.id,
                    link.anchor
                );
            }
            let (spawn, _) = g.anchor("player_spawn").unwrap();
            assert!(g.in_bounds(spawn) && !g.blocked(spawn), "{}", z.id);
        }
        let campus = w.zone("campus").unwrap();
        assert!(campus.arrival("tower/1").is_some());
        assert!(!campus.in_bounds(Vec3::new(500.0, 0.0, 0.0)));
        assert!(!campus.in_bounds(Vec3::new(0.0, 50.0, 0.0)));
        assert!(!campus.in_bounds(Vec3::new(0.0, 7.0, 0.0)));
        assert!(campus.in_bounds(Vec3::new(0.0, 1.28, 0.0)));
        let wall = campus.blockers.iter().find(|b| b.max.y > 2.0).unwrap();
        let inside = Vec3::new(
            (wall.min.x + wall.max.x) / 2.0,
            0.0,
            (wall.min.z + wall.max.z) / 2.0,
        );
        assert!(campus.blocked(inside));
    }

    #[test]
    fn segments_through_thin_walls_are_blocked() {
        let wall = Aabb {
            min: Vec3::new(-0.1, 0.0, -5.0),
            max: Vec3::new(0.1, 3.0, 5.0),
        };
        let crate_box = Aabb {
            min: Vec3::new(4.0, 0.0, -1.0),
            max: Vec3::new(5.0, 0.25, 1.0),
        };
        let g = ZoneGeom {
            anchors: HashMap::new(),
            blockers: vec![wall, crate_box],
            triggers: HashMap::new(),
            arrivals: HashMap::new(),
            bounds: [-10.0, 10.0, -10.0, 10.0],
            terrain: Terrain::default(),
        };
        let p = |x: f32, z: f32| Vec3::new(x, 0.0, z);
        // Both ends are legal, the line between them is not.
        assert!(!g.blocked(p(-0.5, 0.0)) && !g.blocked(p(0.5, 0.0)));
        assert!(g.segment_blocked(p(-0.5, 0.0), p(0.5, 0.0)));
        // Around the end of the wall, along it, and over a step-height crate are fine.
        assert!(!g.segment_blocked(p(-0.5, 5.5), p(0.5, 5.5)));
        assert!(!g.segment_blocked(p(-0.5, -4.0), p(-0.5, 4.0)));
        assert!(!g.segment_blocked(p(3.5, 0.0), p(5.5, 0.0)));
        // A jump does not clear a 3 m wall.
        let air = Vec3::new(0.5, 1.2, 0.0);
        assert!(g.segment_blocked(p(-0.5, 0.0), air));
        assert!(segment_hits_xz(&wall, p(0.0, -9.0), p(0.0, 9.0), 0.0));
        assert!(!segment_hits_xz(&wall, p(0.3, -9.0), p(0.3, 9.0), 0.0));
    }

    #[test]
    fn the_server_and_the_sim_agree_on_the_ground_and_heights_are_checked_against_it() {
        let w = World::embedded().unwrap();
        for zone in ["campus", "parking_p3"] {
            let g = w.zone(zone).unwrap();
            assert!(!g.terrain.is_flat(), "{zone} has terrain");
            let mut sim = Sim::new(w.content.clone(), 1);
            sim.spawn_player("fullstack", Vec3::new(0.0, 0.0, 0.0), 0.0);
            w.load(&mut sim, zone, None).unwrap();
            let [x0, x1, z0, z1] = g.bounds;
            let mut uneven = 0;
            for i in 0..=60 {
                for j in 0..=60 {
                    let x = x0 + (x1 - x0) * i as f32 / 60.0;
                    let z = z0 + (z1 - z0) * j as f32 / 60.0;
                    let (server, sim_y) = (g.ground(x, z), sim.ground_at(x, z));
                    assert_eq!(server.to_bits(), sim_y.to_bits(), "{zone} at {x},{z}");
                    uneven += usize::from(server.abs() > 0.5);
                }
            }
            assert!(uneven > 10, "{zone}: only {uneven} raised samples");
            // Every unit the sim placed stands on that ground (or on a seat above it).
            sim.tick();
            for u in sim.units().filter(|u| !u.hostile || u.alive) {
                let ground = g.ground(u.pos.x, u.pos.z);
                assert!(u.pos.y >= ground - 1e-3, "{zone}: {} below ground", u.key);
            }
        }
        // Executive Hill: standing on top is legal, the same point at the datum is not.
        let campus = w.zone("campus").unwrap();
        let top = Vec3::new(203.0, campus.ground(203.0, 142.0), 142.0);
        assert!(top.y > 6.0, "hilltop at {}", top.y);
        assert!(campus.in_bounds(top));
        assert!(campus.in_bounds(Vec3::new(top.x, top.y + 1.2, top.z)));
        assert!(!campus.in_bounds(Vec3::new(top.x, 0.0, top.z)));
        assert!(!campus.in_bounds(Vec3::new(top.x, top.y + 3.0, top.z)));
    }

    #[test]
    fn bad_inputs_are_errors() {
        assert!(World::new("nope", EMBEDDED_GEOMETRY).is_err());
        assert!(World::new(EMBEDDED_CONTENT, "[]").is_err());
        assert!(World::load_from(Some(Path::new("/nonexistent.ron"))).is_err());
    }
}
