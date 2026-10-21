/// Bevy convention: Y up, yaw rotates about +Y, yaw 0 faces -Z.
#[derive(Clone, Copy, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

pub fn forward(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
}

pub fn distance(a: Vec3, b: Vec3) -> f32 {
    let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

pub fn in_range(a: Vec3, b: Vec3, range: f32) -> bool {
    distance(a, b) <= range
}

/// True if target lies within +/- `half_angle` (radians) of facing, on the XZ
/// plane. WotLK uses a 180 degree front arc, i.e. `half_angle = PI / 2`.
pub fn is_facing(origin: Vec3, yaw: f32, target: Vec3, half_angle: f32) -> bool {
    let (dx, dz) = (target.x - origin.x, target.z - origin.z);
    let len = (dx * dx + dz * dz).sqrt();
    // Standing on the target: no meaningful bearing, treat as facing.
    if len < 1e-6 {
        return true;
    }
    let f = forward(yaw);
    (f.x * dx + f.z * dz) / len >= half_angle.cos()
}

/// Segment a->b vs boxes (slab test). False if any box blocks.
pub fn line_of_sight(a: Vec3, b: Vec3, blockers: &[Aabb]) -> bool {
    !blockers.iter().any(|bx| segment_hits(a, b, bx))
}

/// Uniform XZ grid over blockers. `clear` gives the same answer as `line_of_sight` on the same
/// slice: it only skips boxes whose XZ extent cannot touch the segment's XZ bounding box.
#[derive(Clone, Debug, Default)]
pub struct BlockerGrid {
    boxes: Vec<Aabb>,
    cells: Vec<Vec<u32>>,
    min_x: f32,
    min_z: f32,
    nx: usize,
    nz: usize,
}

const CELL_M: f32 = 8.0;

impl BlockerGrid {
    pub fn new(boxes: Vec<Aabb>) -> Self {
        let finite = |b: &Aabb| {
            [b.min.x, b.min.z, b.max.x, b.max.z]
                .iter()
                .all(|v| v.is_finite())
        };
        if boxes.is_empty() || !boxes.iter().all(finite) {
            // Non-finite input: keep the linear scan so behaviour stays identical.
            return Self {
                boxes,
                ..Self::default()
            };
        }
        let min_x = boxes.iter().map(|b| b.min.x).fold(f32::MAX, f32::min);
        let min_z = boxes.iter().map(|b| b.min.z).fold(f32::MAX, f32::min);
        let max_x = boxes.iter().map(|b| b.max.x).fold(f32::MIN, f32::max);
        let max_z = boxes.iter().map(|b| b.max.z).fold(f32::MIN, f32::max);
        let nx = (((max_x - min_x) / CELL_M) as usize + 1).min(512);
        let nz = (((max_z - min_z) / CELL_M) as usize + 1).min(512);
        let mut grid = Self {
            boxes,
            cells: vec![Vec::new(); nx * nz],
            min_x,
            min_z,
            nx,
            nz,
        };
        for (i, b) in grid.boxes.iter().enumerate() {
            let (x0, x1) = (grid.col(b.min.x), grid.col(b.max.x));
            let (z0, z1) = (grid.row(b.min.z), grid.row(b.max.z));
            for z in z0..=z1 {
                for x in x0..=x1 {
                    grid.cells[z * nx + x].push(i as u32);
                }
            }
        }
        grid
    }

    // Clamping keeps out-of-grid coordinates on the border cells, which is conservative.
    fn col(&self, x: f32) -> usize {
        (((x - self.min_x) / CELL_M).max(0.0) as usize).min(self.nx - 1)
    }

    fn row(&self, z: f32) -> usize {
        (((z - self.min_z) / CELL_M).max(0.0) as usize).min(self.nz - 1)
    }

    /// True when no blocker lies on the segment (same meaning as `line_of_sight`).
    pub fn clear(&self, a: Vec3, b: Vec3) -> bool {
        if self.cells.is_empty() || ![a.x, a.z, b.x, b.z].iter().all(|v| v.is_finite()) {
            return line_of_sight(a, b, &self.boxes);
        }
        let (x0, x1) = (self.col(a.x.min(b.x)), self.col(a.x.max(b.x)));
        let (z0, z1) = (self.row(a.z.min(b.z)), self.row(a.z.max(b.z)));
        // A long segment touching more cells than there are boxes is cheaper to scan linearly.
        if (x1 - x0 + 1) * (z1 - z0 + 1) > self.boxes.len() {
            return line_of_sight(a, b, &self.boxes);
        }
        for z in z0..=z1 {
            for x in x0..=x1 {
                if self.cells[z * self.nx + x]
                    .iter()
                    .any(|&i| segment_hits(a, b, &self.boxes[i as usize]))
                {
                    return false;
                }
            }
        }
        true
    }
}

fn segment_hits(a: Vec3, b: Vec3, bx: &Aabb) -> bool {
    let mut t0 = 0.0_f32;
    let mut t1 = 1.0_f32;
    let axes = [
        (a.x, b.x - a.x, bx.min.x, bx.max.x),
        (a.y, b.y - a.y, bx.min.y, bx.max.y),
        (a.z, b.z - a.z, bx.min.z, bx.max.z),
    ];
    for (o, d, lo, hi) in axes {
        if d.abs() < 1e-9 {
            if o < lo || o > hi {
                return false;
            }
        } else {
            let (mut ta, mut tb) = ((lo - o) / d, (hi - o) / d);
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    const O: Vec3 = Vec3::new(0.0, 0.0, 0.0);

    fn close(a: Vec3, b: Vec3) -> bool {
        distance(a, b) < 1e-5
    }

    #[test]
    fn forward_axis_convention() {
        assert!(close(forward(0.0), Vec3::new(0.0, 0.0, -1.0)));
        assert!(close(forward(FRAC_PI_2), Vec3::new(-1.0, 0.0, 0.0)));
        assert!(close(forward(PI), Vec3::new(0.0, 0.0, 1.0)));
    }

    #[test]
    fn distance_and_range() {
        let b = Vec3::new(3.0, 4.0, 0.0);
        assert_eq!(distance(O, b), 5.0);
        assert!(in_range(O, b, 5.0));
        assert!(!in_range(O, b, 4.9));
    }

    #[test]
    fn facing_front_back_edge() {
        let half = FRAC_PI_2;
        assert!(is_facing(O, 0.0, Vec3::new(0.0, 0.0, -5.0), half));
        assert!(!is_facing(O, 0.0, Vec3::new(0.0, 0.0, 5.0), half));
        // Exactly 90 degrees to the side is on the arc edge.
        assert!(is_facing(O, 0.0, Vec3::new(5.0, 0.0, 0.0), half));
        assert!(!is_facing(O, 0.0, Vec3::new(5.0, 0.0, 0.1), half));
        // Rotating yaw by PI flips front and back.
        assert!(is_facing(O, PI, Vec3::new(0.0, 0.0, 5.0), half));
    }

    #[test]
    fn facing_ignores_height() {
        assert!(is_facing(O, 0.0, Vec3::new(0.0, 50.0, -1.0), FRAC_PI_2));
    }

    fn wall() -> Aabb {
        Aabb {
            min: Vec3::new(-1.0, -1.0, -6.0),
            max: Vec3::new(1.0, 5.0, -4.0),
        }
    }

    #[test]
    fn los_blocked_by_box() {
        assert!(!line_of_sight(O, Vec3::new(0.0, 0.0, -10.0), &[wall()]));
    }

    #[test]
    fn los_clear_when_passing_beside() {
        assert!(line_of_sight(O, Vec3::new(5.0, 0.0, -10.0), &[wall()]));
        assert!(line_of_sight(O, Vec3::new(0.0, 0.0, -10.0), &[]));
    }

    #[test]
    fn los_clear_when_segment_ends_before_box() {
        assert!(line_of_sight(O, Vec3::new(0.0, 0.0, -3.0), &[wall()]));
    }

    #[test]
    fn los_blocked_when_axis_parallel_inside_slab() {
        // Zero extent on x and y exercises the parallel-axis branch.
        assert!(!line_of_sight(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -5.0),
            &[wall()]
        ));
    }

    #[test]
    fn grid_matches_linear_scan() {
        let mut seed = 12345_u32;
        let mut rnd = |lo: f32, hi: f32| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            lo + (seed >> 8) as f32 / (1u32 << 24) as f32 * (hi - lo)
        };
        let boxes: Vec<Aabb> = (0..300)
            .map(|_| {
                let (x, z) = (rnd(-100.0, 100.0), rnd(-100.0, 100.0));
                Aabb {
                    min: Vec3::new(x, 0.0, z),
                    max: Vec3::new(x + rnd(0.5, 12.0), rnd(1.0, 6.0), z + rnd(0.5, 12.0)),
                }
            })
            .collect();
        let grid = BlockerGrid::new(boxes.clone());
        for _ in 0..20_000 {
            let a = Vec3::new(rnd(-130.0, 130.0), rnd(0.0, 3.0), rnd(-130.0, 130.0));
            let b = Vec3::new(
                a.x + rnd(-40.0, 40.0),
                rnd(0.0, 3.0),
                a.z + rnd(-40.0, 40.0),
            );
            assert_eq!(grid.clear(a, b), line_of_sight(a, b, &boxes));
        }
    }
}
