//! Floor outlines as unions of smooth polygons. Walkable space is the union of
//! `regions` minus the `holes` (pillars, glass boxes, curved partitions).
//! Walls are derived from the polygon edges, so openings between pods appear
//! wherever two regions overlap; there is no door bookkeeping. Collision is a
//! chain of small AABBs along the walls (the engine only knows axis-aligned
//! boxes), the visual walls are smooth strips built from the same segments.

use super::kit::Mat;
use crate::collision::aabb;
use bevy::prelude::*;
use combat_core::Aabb;
use std::f32::consts::TAU;

/// Wall edges are cut to this length for the overlap tests, then fused up to `MAX_SEG`.
const STEP: f32 = 0.2;
const MAX_SEG: f32 = 0.8;
/// Collision wall thickness (the visual wall is a zero-thickness strip at the inner face).
pub const WALL_T: f32 = 0.3;
/// Collider merging stops when the merged box would be this much thicker than the wall.
const MERGE_EXTRA: f32 = 0.45;
const CELL: f32 = 1.0;

#[derive(Clone, Debug)]
pub struct Poly {
    pub pts: Vec<Vec2>,
    pub mat: Mat,
    collide: bool,
    lo: Vec2,
    hi: Vec2,
}

impl Poly {
    /// `walkable_inside`: regions keep walkable space on the left of every edge (counter-clockwise),
    /// holes on the right, so the left normal always points into walkable space.
    fn new(mut pts: Vec<Vec2>, mat: Mat, walkable_inside: bool) -> Self {
        let area: f32 = pts
            .iter()
            .zip(pts.iter().cycle().skip(1))
            .map(|(a, b)| a.x * b.y - b.x * a.y)
            .sum();
        if (area > 0.0) != walkable_inside {
            pts.reverse();
        }
        let lo = pts.iter().fold(Vec2::splat(f32::MAX), |m, p| m.min(*p));
        let hi = pts.iter().fold(Vec2::splat(f32::MIN), |m, p| m.max(*p));
        Self {
            pts,
            mat,
            collide: true,
            lo,
            hi,
        }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        if p.x < self.lo.x || p.x > self.hi.x || p.y < self.lo.y || p.y > self.hi.y {
            return false;
        }
        let mut inside = false;
        let mut j = self.pts.len() - 1;
        for i in 0..self.pts.len() {
            let (a, b) = (self.pts[i], self.pts[j]);
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

#[derive(Clone, Debug)]
pub struct Region {
    pub poly: Poly,
    /// Pods are the big rooms; links only connect them.
    #[cfg_attr(not(test), allow(dead_code))]
    pub pod: bool,
}

/// One wall piece: `n` is the unit normal towards walkable space.
#[derive(Clone, Copy, Debug)]
pub struct Seg {
    pub a: Vec2,
    pub b: Vec2,
    pub n: Vec2,
    pub mat: Mat,
    /// Thin partitions collide through one centre chain instead of one chain per face.
    pub collide: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Footprint {
    pub regions: Vec<Region>,
    pub holes: Vec<Poly>,
    /// Centre paths and thickness of the thin partitions among the holes.
    thin: Vec<(Vec<Vec2>, f32)>,
}

impl Footprint {
    pub fn pod(&mut self, pts: Vec<Vec2>, mat: Mat) {
        self.regions.push(Region {
            poly: Poly::new(pts, mat, true),
            pod: true,
        });
    }

    pub fn link(&mut self, pts: Vec<Vec2>, mat: Mat) {
        self.regions.push(Region {
            poly: Poly::new(pts, mat, true),
            pod: false,
        });
    }

    pub fn hole(&mut self, pts: Vec<Vec2>, mat: Mat) {
        self.holes.push(Poly::new(pts, mat, false));
    }

    /// Thin wall following `path`: two visual faces but a single collider chain.
    pub fn partition(&mut self, path: &[Vec2], thick: f32, mat: Mat) {
        let tangent = |i: usize| {
            let (a, b) = (path[i.saturating_sub(1)], path[(i + 1).min(path.len() - 1)]);
            (b - a).normalize_or_zero()
        };
        let side = |i: usize, s: f32| {
            let t = tangent(i);
            path[i] + Vec2::new(-t.y, t.x) * thick / 2.0 * s
        };
        let mut pts: Vec<Vec2> = (0..path.len()).map(|i| side(i, 1.0)).collect();
        pts.extend((0..path.len()).rev().map(|i| side(i, -1.0)));
        let mut poly = Poly::new(pts, mat, false);
        poly.collide = false;
        self.holes.push(poly);
        self.thin.push((path.to_vec(), thick));
    }

    /// Collision for the thin partitions.
    pub fn thin_colliders(&self) -> Vec<Aabb> {
        self.thin
            .iter()
            .flat_map(|(path, thick)| {
                let segs: Vec<Seg> = path
                    .windows(2)
                    .map(|w| Seg {
                        a: w[0],
                        b: w[1],
                        n: Vec2::ZERO,
                        mat: Mat::Wall,
                        collide: true,
                    })
                    .collect();
                colliders(&segs, *thick)
            })
            .collect()
    }

    /// Inside any region (holes ignored): the area that needs a floor and a ceiling.
    pub fn covered(&self, p: Vec2) -> bool {
        self.regions.iter().any(|r| r.poly.contains(p))
    }

    pub fn contains(&self, p: Vec2) -> bool {
        self.covered(p) && !self.holes.iter().any(|h| h.contains(p))
    }

    #[cfg(test)]
    pub fn pod_of(&self, p: Vec2) -> Option<usize> {
        self.regions
            .iter()
            .position(|r| r.pod && r.poly.contains(p))
    }

    pub fn walls(&self) -> Vec<Seg> {
        let mut out = Vec::new();
        for (i, r) in self.regions.iter().enumerate() {
            let keep = |m: Vec2| {
                !self
                    .regions
                    .iter()
                    .enumerate()
                    .any(|(j, o)| j != i && o.poly.contains(m))
                    && !self.holes.iter().any(|h| h.contains(m))
            };
            edges(&r.poly, &keep, &mut out);
        }
        for (i, h) in self.holes.iter().enumerate() {
            let keep = |m: Vec2| {
                self.covered(m)
                    && !self
                        .holes
                        .iter()
                        .enumerate()
                        .any(|(j, o)| j != i && o.contains(m))
            };
            edges(h, &keep, &mut out);
        }
        out
    }

    /// Floor/ceiling cover as row runs `(x0, x1, z0, z1)`. A cell counts when any of its
    /// corners or centre is covered, so the slabs always reach under the walls.
    pub fn floor_runs(&self) -> Vec<(f32, f32, f32, f32)> {
        let lo = self
            .regions
            .iter()
            .fold(Vec2::splat(f32::MAX), |m, r| m.min(r.poly.lo));
        let hi = self
            .regions
            .iter()
            .fold(Vec2::splat(f32::MIN), |m, r| m.max(r.poly.hi));
        let (x0, z0) = ((lo.x / CELL).floor() * CELL, (lo.y / CELL).floor() * CELL);
        let nx = ((hi.x - x0) / CELL).ceil() as usize + 1;
        let nz = ((hi.y - z0) / CELL).ceil() as usize + 1;
        let mut runs = Vec::new();
        for j in 0..nz {
            let z = z0 + j as f32 * CELL;
            let mut start: Option<usize> = None;
            for i in 0..=nx {
                let x = x0 + i as f32 * CELL;
                let on = i < nx
                    && [(0.5, 0.5), (0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
                        .iter()
                        .any(|(dx, dz)| self.covered(Vec2::new(x + dx * CELL, z + dz * CELL)));
                match (on, start) {
                    (true, None) => start = Some(i),
                    (false, Some(s)) => {
                        runs.push((x0 + s as f32 * CELL, x, z, z + CELL));
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        runs
    }
}

/// Cuts every polygon edge into fine pieces, keeps those whose midpoint passes `keep`
/// (fine, so openings end within ~0.15 m of the neighbouring wall), then fuses consecutive
/// kept pieces back into segments of at most `MAX_SEG`.
fn edges(poly: &Poly, keep: &dyn Fn(Vec2) -> bool, out: &mut Vec<Seg>) {
    let n = poly.pts.len();
    for i in 0..n {
        let (p, q) = (poly.pts[i], poly.pts[(i + 1) % n]);
        let len = p.distance(q);
        if len < 1e-4 {
            continue;
        }
        let d = (q - p) / len;
        let normal = Vec2::new(-d.y, d.x);
        let k = (len / STEP).ceil().max(1.0) as usize;
        let kept: Vec<bool> = (0..k)
            .map(|s| keep(p.lerp(q, (s as f32 + 0.5) / k as f32)))
            .collect();
        let mut s = 0;
        while s < k {
            if !kept[s] {
                s += 1;
                continue;
            }
            let start = s;
            while s < k && kept[s] {
                s += 1;
            }
            let run = (s - start) as f32 * len / k as f32;
            let parts = (run / MAX_SEG).ceil().max(1.0) as usize;
            let (t0, t1) = (start as f32 / k as f32, s as f32 / k as f32);
            for part in 0..parts {
                out.push(Seg {
                    a: p.lerp(q, t0 + (t1 - t0) * part as f32 / parts as f32),
                    b: p.lerp(q, t0 + (t1 - t0) * (part + 1) as f32 / parts as f32),
                    n: normal,
                    mat: poly.mat,
                    collide: poly.collide,
                });
            }
        }
    }
}

fn seg_box(s: &Seg, thick: f32) -> Aabb {
    let d = s.b - s.a;
    let len = d.length();
    let (c, sn) = if len > 1e-6 {
        (d.x.abs() / len, d.y.abs() / len)
    } else {
        (1.0, 0.0)
    };
    // The box sits behind the wall face so the inner face is the walkable limit.
    let mid = (s.a + s.b) / 2.0 - s.n * thick / 2.0;
    let size = Vec3::new(len * c + thick * sn, super::kit::H, len * sn + thick * c);
    aabb(Vec3::new(mid.x, super::kit::H / 2.0, mid.y), size)
}

/// Collision for a wall chain: consecutive near-axis-aligned pieces merge into long boxes,
/// curved parts stay as short boxes. The staircase stays within a few centimetres of the wall.
pub fn colliders(segs: &[Seg], thick: f32) -> Vec<Aabb> {
    let mut out: Vec<Aabb> = Vec::new();
    let mut prev_end: Option<Vec2> = None;
    for s in segs.iter().filter(|s| s.collide) {
        let bx = seg_box(s, thick);
        if let (Some(end), Some(last)) = (prev_end, out.last_mut())
            && end.distance(s.a) < 1e-3
        {
            let min = Vec3::new(last.min.x.min(bx.min.x), 0.0, last.min.z.min(bx.min.z));
            let max = Vec3::new(
                last.max.x.max(bx.max.x),
                super::kit::H,
                last.max.z.max(bx.max.z),
            );
            if (max.x - min.x).min(max.z - min.z) <= thick + MERGE_EXTRA {
                *last = Aabb {
                    min: crate::collision::to_core(min),
                    max: crate::collision::to_core(max),
                };
                prev_end = Some(s.b);
                continue;
            }
        }
        out.push(bx);
        prev_end = Some(s.b);
    }
    out
}

// ------------------------------------------------------------ outline builders

fn rot(p: Vec2, deg: f32) -> Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    Vec2::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

/// Organic pod: an ellipse whose radius breathes with `lobes` bumps of relative size `amp`
/// (3 lobes = rounded triangle; stay under 1/(lobes^2-1) to keep it convex).
pub fn blob(c: Vec2, rx: f32, rz: f32, rot_deg: f32, lobes: u32, amp: f32) -> Vec<Vec2> {
    const N: usize = 72;
    (0..N)
        .map(|i| {
            let t = i as f32 / N as f32 * TAU;
            let r = 1.0 + amp * (lobes as f32 * t).cos();
            c + rot(Vec2::new(rx * r * t.cos(), rz * r * t.sin()), rot_deg)
        })
        .collect()
}

/// Rectangle with rounded corners: keeps flat walls (elevators, window runs) between the bends.
pub fn rounded_rect(c: Vec2, hx: f32, hz: f32, r: f32, rot_deg: f32) -> Vec<Vec2> {
    let r = r.min(hx).min(hz);
    let mut pts = Vec::new();
    for (k, (sx, sz)) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
        .into_iter()
        .enumerate()
    {
        let corner = Vec2::new(sx * (hx - r), sz * (hz - r));
        for i in 0..=6 {
            let t = (k as f32 * 90.0 + i as f32 * 15.0).to_radians();
            pts.push(c + rot(corner + Vec2::new(t.cos(), t.sin()) * r, rot_deg));
        }
    }
    pts
}

/// Polygon through `corners` with every corner rounded to `r` (rounded triangle with flat sides).
pub fn rounded_poly(corners: &[Vec2], r: f32) -> Vec<Vec2> {
    let n = corners.len();
    let mut pts = Vec::new();
    for i in 0..n {
        let (prev, cur, next) = (corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n]);
        let (d0, d1) = (
            (prev - cur).normalize_or_zero(),
            (next - cur).normalize_or_zero(),
        );
        let half = d0.angle_to(d1).abs() / 2.0;
        let tan_len = (r / half.tan().max(0.05))
            .min(prev.distance(cur) * 0.45)
            .min(next.distance(cur) * 0.45);
        let (p0, p1) = (cur + d0 * tan_len, cur + d1 * tan_len);
        let radius = tan_len * half.tan();
        let bis = (d0 + d1).normalize_or_zero();
        let centre = cur + bis * (tan_len / half.cos());
        let (a0, a1) = ((p0 - centre).to_angle(), (p1 - centre).to_angle());
        let mut da = a1 - a0;
        while da > std::f32::consts::PI {
            da -= TAU;
        }
        while da < -std::f32::consts::PI {
            da += TAU;
        }
        for k in 0..=8 {
            let a = a0 + da * k as f32 / 8.0;
            pts.push(centre + Vec2::new(a.cos(), a.sin()) * radius);
        }
    }
    pts
}

/// Catmull-Rom path through `ctrl`, sampled about every metre.
pub fn smooth(ctrl: &[Vec2]) -> Vec<Vec2> {
    let mut path = Vec::new();
    for i in 0..ctrl.len() - 1 {
        let p0 = ctrl[i.saturating_sub(1)];
        let (p1, p2) = (ctrl[i], ctrl[i + 1]);
        let p3 = ctrl[(i + 2).min(ctrl.len() - 1)];
        let k = (p1.distance(p2) / 1.0).ceil().max(2.0) as usize;
        for s in 0..k {
            let t = s as f32 / k as f32;
            let (t2, t3) = (t * t, t * t * t);
            path.push(
                0.5 * ((2.0 * p1)
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3),
            );
        }
    }
    path.push(ctrl[ctrl.len() - 1]);
    path
}

/// Smooth curved band of half width `hw` around a Catmull-Rom path through `ctrl`, with round ends.
pub fn stroke(ctrl: &[Vec2], hw: f32) -> Vec<Vec2> {
    let path = smooth(ctrl);
    let tangent = |i: usize| {
        let (a, b) = (path[i.saturating_sub(1)], path[(i + 1).min(path.len() - 1)]);
        (b - a).normalize_or_zero()
    };
    let side = |i: usize, s: f32| {
        let t = tangent(i);
        path[i] + Vec2::new(-t.y, t.x) * hw * s
    };
    let last = path.len() - 1;
    let mut pts: Vec<Vec2> = (0..=last).map(|i| side(i, 1.0)).collect();
    let t = tangent(last);
    let base = (t.y).atan2(t.x) + std::f32::consts::FRAC_PI_2;
    for k in 1..12 {
        let a = base - std::f32::consts::PI * k as f32 / 12.0;
        pts.push(path[last] + Vec2::new(a.cos(), a.sin()) * hw);
    }
    pts.extend((0..=last).rev().map(|i| side(i, -1.0)));
    let t = tangent(0);
    let base = (t.y).atan2(t.x) - std::f32::consts::FRAC_PI_2;
    for k in 1..12 {
        let a = base - std::f32::consts::PI * k as f32 / 12.0;
        pts.push(path[0] + Vec2::new(a.cos(), a.sin()) * hw);
    }
    pts
}

/// Control points of an arc, for `stroke`.
pub fn arc(c: Vec2, r: f32, deg0: f32, deg1: f32) -> Vec<Vec2> {
    let n = ((deg1 - deg0).abs() / 20.0).ceil().max(2.0) as usize;
    (0..=n)
        .map(|i| {
            let t = (deg0 + (deg1 - deg0) * i as f32 / n as f32).to_radians();
            c + Vec2::new(t.cos(), t.sin()) * r
        })
        .collect()
}

pub fn regular(c: Vec2, r: f32, n: usize, rot_deg: f32) -> Vec<Vec2> {
    (0..n)
        .map(|i| c + rot(Vec2::new(r, 0.0), rot_deg + 360.0 * i as f32 / n as f32))
        .collect()
}

/// Square turned by `rot_deg` (45 for a diamond) with half-diagonal `half`.
pub fn diamond(c: Vec2, half: f32, rot_deg: f32) -> Vec<Vec2> {
    regular(c, half, 4, rot_deg)
}

/// Pillar-sized round hole.
pub fn pillar(c: Vec2, r: f32) -> Vec<Vec2> {
    regular(c, r, 10, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_opens_where_regions_overlap_and_holes_get_walls() {
        let mut fp = Footprint::default();
        fp.pod(blob(Vec2::new(-5.0, 0.0), 6.0, 6.0, 0.0, 0, 0.0), Mat::Wall);
        fp.pod(blob(Vec2::new(5.0, 0.0), 6.0, 6.0, 0.0, 0, 0.0), Mat::Wall);
        fp.hole(pillar(Vec2::new(0.0, 0.0), 1.0), Mat::Wall);
        assert!(fp.contains(Vec2::new(0.0, 3.0)));
        assert!(!fp.contains(Vec2::new(0.0, 0.0)));
        assert!(!fp.contains(Vec2::new(20.0, 0.0)));
        let walls = fp.walls();
        // No wall piece may sit in the open middle, except the pillar's.
        assert!(walls.iter().all(|s| {
            let m = (s.a + s.b) / 2.0;
            m.length() < 1.2 || m.y.abs() > 3.0 || m.x.abs() > 8.0
        }));
        assert!(colliders(&walls, WALL_T).len() < walls.len());
    }

    #[test]
    fn floor_runs_cover_the_region() {
        let mut fp = Footprint::default();
        fp.pod(rounded_rect(Vec2::ZERO, 5.0, 3.0, 1.0, 0.0), Mat::Wall);
        let area: f32 = fp
            .floor_runs()
            .iter()
            .map(|r| (r.1 - r.0) * (r.3 - r.2))
            .sum();
        assert!(area >= 10.0 * 6.0 - 1.0);
    }
}
