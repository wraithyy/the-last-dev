//! Ground height of outdoor zones. A handful of analytic features (hills, raised plazas with
//! ramps, a river bed with bridges) instead of a sampled heightfield: the client layout builds
//! it, `content/zone_geometry.ron` carries it, and the sim, the client and the co-op server all
//! evaluate this one function, so heights agree to the bit on every side.

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Terrain {
    pub features: Vec<Feature>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Feature {
    /// Round hill: `h` at the centre, falling off along a cosine to 0 at radius `r`.
    Hill { x: f32, z: f32, r: f32, h: f32 },
    /// Raised plaza: flat at `h` inside the rectangle, a straight ramp `ramp` metres wide all
    /// round it (walls on the sides that are not meant as ramps are the layout's job).
    Mesa {
        x0: f32,
        z0: f32,
        x1: f32,
        z1: f32,
        h: f32,
        ramp: f32,
    },
    /// River bed along a polyline: `depth` below the surrounding ground within `half_w` of the
    /// line, sloping back up over `bank` metres.
    River {
        pts: Vec<(f32, f32)>,
        half_w: f32,
        bank: f32,
        depth: f32,
    },
    /// Bridge deck: inside the rectangle the river bed does not count; the deck is `h` above the
    /// ground around it. Walkable height only, the drawn ground still dips under the deck.
    Bridge {
        x0: f32,
        z0: f32,
        x1: f32,
        z1: f32,
        h: f32,
    },
}

fn seg_dist(px: f32, pz: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let len2 = (dx * dx + dz * dz).max(1e-6);
    let t = (((px - a.0) * dx + (pz - a.1) * dz) / len2).clamp(0.0, 1.0);
    let (qx, qz) = (a.0 + dx * t - px, a.1 + dz * t - pz);
    (qx * qx + qz * qz).sqrt()
}

fn inside(x: f32, z: f32, (x0, z0, x1, z1): (f32, f32, f32, f32)) -> bool {
    x >= x0 && x <= x1 && z >= z0 && z <= z1
}

impl Terrain {
    pub fn is_flat(&self) -> bool {
        self.features.is_empty()
    }

    /// RON text for the zone geometry export (the client has no RON writer of its own).
    pub fn to_ron(&self) -> String {
        ron::to_string(self).unwrap_or_default()
    }

    /// Hills and plazas only: the ground before the river cuts into it.
    fn base(&self, x: f32, z: f32) -> f32 {
        let mut h = 0.0f32;
        for f in &self.features {
            let v = match *f {
                Feature::Hill { x: cx, z: cz, r, h } => {
                    let d = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
                    if d >= r {
                        0.0
                    } else {
                        h * 0.5 * (1.0 + (PI * d / r).cos())
                    }
                }
                Feature::Mesa {
                    x0,
                    z0,
                    x1,
                    z1,
                    h,
                    ramp,
                } => {
                    let dx = (x0 - x).max(x - x1).max(0.0);
                    let dz = (z0 - z).max(z - z1).max(0.0);
                    let d = dx.max(dz);
                    if d >= ramp {
                        0.0
                    } else {
                        h * (1.0 - d / ramp.max(1e-3))
                    }
                }
                _ => 0.0,
            };
            h = h.max(v);
        }
        h
    }

    /// Depth of the deepest river bed at this point (0 outside every river).
    fn carve(&self, x: f32, z: f32) -> f32 {
        let mut cut = 0.0f32;
        for f in &self.features {
            let Feature::River {
                pts,
                half_w,
                bank,
                depth,
            } = f
            else {
                continue;
            };
            let d = pts
                .windows(2)
                .map(|w| seg_dist(x, z, w[0], w[1]))
                .fold(f32::INFINITY, f32::min);
            let v = if d <= *half_w {
                *depth
            } else if d < half_w + bank {
                depth * (1.0 - (d - half_w) / bank.max(1e-3))
            } else {
                0.0
            };
            cut = cut.max(v);
        }
        cut
    }

    fn deck(&self, x: f32, z: f32) -> Option<f32> {
        self.features.iter().find_map(|f| match *f {
            Feature::Bridge { x0, z0, x1, z1, h } if inside(x, z, (x0, z0, x1, z1)) => Some(h),
            _ => None,
        })
    }

    /// Height of the walkable ground (bridge decks included).
    pub fn height(&self, x: f32, z: f32) -> f32 {
        if self.features.is_empty() {
            return 0.0;
        }
        let base = self.base(x, z);
        match self.deck(x, z) {
            Some(h) => base + h,
            None => base - self.carve(x, z),
        }
    }

    /// Height of the drawn ground: like `height`, but the river bed runs on under the bridges.
    pub fn surface(&self, x: f32, z: f32) -> f32 {
        if self.features.is_empty() {
            return 0.0;
        }
        self.base(x, z) - self.carve(x, z)
    }

    /// Steepest walkable rise per metre at this point, sampled half a metre to each side.
    pub fn slope(&self, x: f32, z: f32) -> f32 {
        let s = 0.5;
        let gx = (self.height(x + s, z) - self.height(x - s, z)) / (2.0 * s);
        let gz = (self.height(x, z + s) - self.height(x, z - s)) / (2.0 * s);
        (gx * gx + gz * gz).sqrt()
    }
}

/// Offsets below this are slope noise from where a unit was put down, not a seat or a stage.
const RAISED_MIN_M: f32 = 0.3;

impl crate::sim::Sim {
    /// Registers a zone's ground; `load_zone` switches to it (flat for zones never registered).
    /// Loaders call it before every load, so a stale registration is never used.
    pub fn set_zone_terrain(&mut self, zone: &str, terrain: Terrain) {
        self.zone_terrains.insert(zone.to_string(), terrain);
    }

    /// Walkable ground height of the loaded zone.
    pub fn ground_at(&self, x: f32, z: f32) -> f32 {
        self.terrain.height(x, z)
    }

    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    /// Every unit but the players (who own their height) stays on the ground of the loaded
    /// zone, keeping the offset it was placed with (seats, stages). Flat zones skip it.
    pub(crate) fn follow_ground(&mut self) {
        if self.terrain.is_flat() {
            return;
        }
        let players: Vec<usize> = self.players.iter().filter_map(|p| self.index(*p)).collect();
        for (i, u) in self.units.iter_mut().enumerate() {
            if players.contains(&i) || u.despawned {
                continue;
            }
            let p = u.state.pos;
            let ground = self.terrain.height(p.x, p.z);
            let off = *u.ground_off.get_or_insert_with(|| {
                let raised = p.y - ground;
                if raised > RAISED_MIN_M { raised } else { 0.0 }
            });
            u.state.pos.y = ground + off;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Terrain {
        Terrain {
            features: vec![
                Feature::Hill {
                    x: 0.0,
                    z: 0.0,
                    r: 20.0,
                    h: 6.0,
                },
                Feature::Mesa {
                    x0: 40.0,
                    z0: -5.0,
                    x1: 50.0,
                    z1: 5.0,
                    h: 3.0,
                    ramp: 6.0,
                },
                Feature::River {
                    pts: vec![(80.0, -50.0), (80.0, 50.0)],
                    half_w: 4.0,
                    bank: 3.0,
                    depth: 1.2,
                },
                Feature::Bridge {
                    x0: 74.0,
                    z0: -2.0,
                    x1: 86.0,
                    z1: 2.0,
                    h: 0.1,
                },
            ],
        }
    }

    #[test]
    fn flat_terrain_is_zero_everywhere() {
        let t = Terrain::default();
        assert!(t.is_flat());
        assert_eq!(t.height(12.0, -40.0), 0.0);
    }

    #[test]
    fn hills_mesas_rivers_and_bridges_shape_the_ground() {
        let t = sample();
        assert!((t.height(0.0, 0.0) - 6.0).abs() < 1e-4);
        assert_eq!(t.height(25.0, 0.0), 0.0);
        let mid = t.height(10.0, 0.0);
        assert!(mid > 2.0 && mid < 4.0, "{mid}");
        assert!((t.height(45.0, 0.0) - 3.0).abs() < 1e-4);
        assert!((t.height(53.0, 0.0) - 1.5).abs() < 1e-4);
        assert!((t.height(80.0, 20.0) + 1.2).abs() < 1e-4);
        assert!((t.height(85.5, 20.0) + 0.6).abs() < 1e-4);
        // The deck carries the walker; the drawn bed still dips beneath it.
        assert!((t.height(80.0, 0.0) - 0.1).abs() < 1e-4);
        assert!((t.surface(80.0, 0.0) + 1.2).abs() < 1e-4);
        // A ramp a person walks up, not a wall.
        assert!(t.slope(53.0, 0.0) < 0.6);
        assert!(t.slope(10.0, 0.0) < 1.0);
    }

    #[test]
    fn terrain_roundtrips_through_ron() {
        let t = sample();
        let text = match ron::to_string(&t) {
            Ok(s) => s,
            Err(e) => panic!("{e}"),
        };
        let back: Terrain = match ron::from_str(&text) {
            Ok(t) => t,
            Err(e) => panic!("{e}"),
        };
        assert_eq!(back, t);
    }
}
