//! Shared, read-only game data: the content file and each zone's gameplay geometry.
//!
//! Geometry comes from `content/zone_geometry.ron`, exported by a `game_client` test from the
//! client's own layout builder (which depends on Bevy), so the server and client agree on
//! anchors, walls and door triggers without the server linking Bevy.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use combat_core::{Aabb, Content, Sim, Vec3};
use serde::Deserialize;

const EMBEDDED_CONTENT: &str = include_str!("../../../content/game.ron");
const EMBEDDED_GEOMETRY: &str = include_str!("../../../content/zone_geometry.ron");

/// Matches the client's collision: boxes lower than this under the feet are stepped onto.
const STEP_HEIGHT: f32 = 0.3;
const BODY_HEIGHT: f32 = 1.8;
/// Every layout fits in this rectangle (game_client `FLOOR_HALF_X/Z`).
const HALF_X: f32 = 72.0;
const HALF_Z: f32 = 54.0;
const BOUNDS_MARGIN: f32 = 1.0;
/// Jump apex plus the tallest prop a player can step up onto, generously.
const MAX_Y: f32 = 8.0;
const MIN_Y: f32 = -1.0;

type P3 = [f32; 3];

#[derive(Deserialize)]
struct RawZone {
    id: String,
    anchors: Vec<(String, P3, f32)>,
    blockers: Vec<(P3, P3)>,
    triggers: Vec<(String, P3, P3)>,
    arrivals: Vec<(String, P3, f32)>,
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

    /// Inside the zone's walls and at a height a body can reach.
    pub fn in_bounds(&self, p: Vec3) -> bool {
        let [x0, x1, z0, z1] = self.bounds;
        (x0..=x1).contains(&p.x) && (z0..=z1).contains(&p.z) && (MIN_Y..=MAX_Y).contains(&p.y)
    }

    /// The feet point is inside a box the body cannot stand in. The client keeps its centre
    /// a player radius away from walls, so a point test leaves that much slack for jitter.
    pub fn blocked(&self, p: Vec3) -> bool {
        self.blockers
            .iter()
            .any(|b| in_xz(b, p) && b.max.y > p.y + STEP_HEIGHT && b.min.y < p.y + BODY_HEIGHT)
    }
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
        let wall = campus.blockers.iter().find(|b| b.max.y > 2.0).unwrap();
        let inside = Vec3::new(
            (wall.min.x + wall.max.x) / 2.0,
            0.0,
            (wall.min.z + wall.max.z) / 2.0,
        );
        assert!(campus.blocked(inside));
    }

    #[test]
    fn bad_inputs_are_errors() {
        assert!(World::new("nope", EMBEDDED_GEOMETRY).is_err());
        assert!(World::new(EMBEDDED_CONTENT, "[]").is_err());
        assert!(World::load_from(Some(Path::new("/nonexistent.ron"))).is_err());
    }
}
