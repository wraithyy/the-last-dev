//! Surfaces that share a plane. Two flat faces within a centimetre of each other z-fight at a
//! distance, so `separate` lifts the later one clear (run once, in `layout::assemble`, which every
//! zone goes through), and `Backing` tells which sign panels need a back twin.

use super::kit::{Mat, Part, Piece, PrimKind};
use bevy::prelude::*;
use combat_core::Terrain;

/// Two surfaces closer than this along their normal count as coplanar.
const EPS: f32 = 0.01;
/// How far above the surface it sat on a lifted piece ends up: just clear of `EPS`.
const LIFT: f32 = 0.012;
/// How far behind a panel's plane the backing check looks. Panels hang 1-5 cm off their board.
const BACK_PROBE: f32 = 0.03;
/// A piece never rises more than this in total, so a pile of overlapping pieces cannot float;
/// pairs still tied after that are left to the per-material depth bias (`textures::mat_bias`).
const CAP: f32 = 0.03;
/// Lifting can cascade (a lifted piece may now touch the next); stop after this many sweeps.
const MAX_PASSES: usize = 64;

/// Oriented footprint on the XZ plane.
#[derive(Clone, Copy, Debug)]
struct Obb {
    c: Vec2,
    h: Vec2,
    yaw: f32,
}

impl Obb {
    fn new(center: Vec3, size: Vec3, yaw: f32) -> Self {
        Self {
            c: Vec2::new(center.x, center.z),
            h: Vec2::new(size.x, size.z) / 2.0,
            yaw,
        }
    }

    fn axes(&self) -> [Vec2; 2] {
        let (s, c) = self.yaw.sin_cos();
        [Vec2::new(c, -s), Vec2::new(s, c)]
    }

    fn contains(&self, p: Vec2) -> bool {
        let d = p - self.c;
        self.axes()
            .iter()
            .zip([self.h.x, self.h.y])
            .all(|(a, h)| d.dot(*a).abs() <= h)
    }

    /// Separating-axis test; `margin` is the penetration needed on every axis to count.
    fn overlaps(&self, o: &Obb, margin: f32) -> bool {
        let (a, b) = (self.axes(), o.axes());
        let d = o.c - self.c;
        let reach =
            |v: &[Vec2; 2], h: Vec2, ax: Vec2| h.x * v[0].dot(ax).abs() + h.y * v[1].dot(ax).abs();
        a.iter()
            .chain(b.iter())
            .all(|ax| d.dot(*ax).abs() < reach(&a, self.h, *ax) + reach(&b, o.h, *ax) - margin)
    }

    fn corners(&self) -> [Vec2; 5] {
        let (ax, h) = (self.axes(), self.h);
        [
            self.c,
            self.c + ax[0] * h.x + ax[1] * h.y,
            self.c - ax[0] * h.x + ax[1] * h.y,
            self.c + ax[0] * h.x - ax[1] * h.y,
            self.c - ax[0] * h.x - ax[1] * h.y,
        ]
    }
}

/// Horizontal upward face.
#[derive(Debug)]
struct Flat {
    obb: Obb,
    y: f32,
    mat: Mat,
    /// Piece index and, for `Piece::Merged`, the part index.
    src: (usize, usize),
}

/// Vertical face: outward normal on XZ, plane offset along it, extent along the tangent and in
/// height. `panel` is the piece index of a `Piece::Panel`.
#[derive(Debug)]
struct Wall {
    n: Vec2,
    d: f32,
    t: (f32, f32),
    y: (f32, f32),
    panel: Option<usize>,
}

fn flats(pieces: &[Piece]) -> Vec<Flat> {
    let mut out = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        let mut add = |center: Vec3, size: Vec3, yaw: f32, y: f32, mat: Mat, part: usize| {
            out.push(Flat {
                obb: Obb::new(center, size, yaw),
                y,
                mat,
                src: (i, part),
            });
        };
        match p {
            Piece::Box {
                center, size, mat, ..
            } => {
                add(*center, *size, 0.0, center.y + size.y / 2.0, *mat, 0);
            }
            Piece::Slab {
                center,
                size,
                yaw,
                mat,
                pitch,
            } if *pitch == 0.0 => {
                add(*center, *size, *yaw, center.y + size.y / 2.0, *mat, 0);
            }
            Piece::Prim {
                center,
                size,
                mat: Mat::LampPool,
                ..
            } => {
                add(
                    *center,
                    Vec3::new(size.x, 0.0, size.z),
                    0.0,
                    center.y,
                    Mat::LampPool,
                    0,
                );
            }
            Piece::Prim {
                kind: PrimKind::Cylinder,
                center,
                size,
                yaw,
                mat,
            } => {
                // Inscribed square: a disc never reaches the corners, so no false hits.
                let sq = Vec3::new(size.x * 0.7, size.y, size.x * 0.7);
                add(*center, sq, *yaw, center.y + size.y / 2.0, *mat, 0);
            }
            Piece::Merged { parts, mat } => {
                for (k, part) in parts.iter().enumerate() {
                    if let Part::Box { center, size, yaw } = part {
                        add(*center, *size, *yaw, center.y + size.y / 2.0, *mat, k);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn walls(pieces: &[Piece]) -> Vec<Wall> {
    let mut out = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        match p {
            Piece::Box { center, size, .. } => block_faces(&mut out, center, size, 0.0),
            Piece::Slab {
                center,
                size,
                yaw,
                pitch,
                ..
            } if *pitch == 0.0 => {
                block_faces(&mut out, center, size, *yaw);
            }
            Piece::Panel {
                center, yaw, size, ..
            } => {
                let n = Vec2::new(yaw.sin(), yaw.cos());
                let at = Vec2::new(center.x, center.z);
                let tc = Vec2::new(n.y, -n.x).dot(at);
                out.push(Wall {
                    n,
                    d: n.dot(at),
                    t: (tc - size.x / 2.0, tc + size.x / 2.0),
                    y: (center.y - size.y / 2.0, center.y + size.y / 2.0),
                    panel: Some(i),
                });
            }
            _ => {}
        }
    }
    out
}

fn block_faces(out: &mut Vec<Wall>, center: &Vec3, size: &Vec3, yaw: f32) {
    let obb = Obb::new(*center, *size, yaw);
    let ax = obb.axes();
    let half = [obb.h.x, obb.h.y];
    for i in 0..2 {
        let tc = ax[1 - i].dot(obb.c);
        for sign in [1.0, -1.0] {
            let n = ax[i] * sign;
            out.push(Wall {
                n,
                d: n.dot(obb.c) + half[i],
                t: (tc - half[1 - i], tc + half[1 - i]),
                y: (center.y - size.y / 2.0, center.y + size.y / 2.0),
                panel: None,
            });
        }
    }
}

#[derive(Debug)]
enum Hit {
    /// A flat face lying on the ground.
    Ground(usize),
    /// Two flat faces sharing a height.
    Top(usize, usize),
    /// A panel sharing a plane with another face (indices into the wall list).
    Wall(usize, usize),
}

/// Rank for "which one goes on top": lamp light is always the topmost layer, then draw order.
fn rank(f: &Flat) -> (bool, (usize, usize)) {
    (f.mat == Mat::LampPool, f.src)
}

/// Highest ground under the face when the whole face lies on it.
fn on_ground(f: &Flat, terrain: &Terrain) -> Option<f32> {
    if f.mat == Mat::Lake {
        return None;
    }
    let g = f.obb.corners().map(|p| terrain.height(p.x, p.y));
    g.iter()
        .all(|g| (g - f.y).abs() < EPS)
        .then(|| g.iter().cloned().fold(f32::MIN, f32::max))
}

fn find(fl: &[Flat], wl: &[Wall], terrain: &Terrain) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (i, f) in fl.iter().enumerate() {
        if on_ground(f, terrain).is_some() {
            hits.push(Hit::Ground(i));
        }
    }
    // Sweep by height so only faces within EPS of each other are compared.
    let mut by_y: Vec<usize> = (0..fl.len()).collect();
    by_y.sort_by(|a, b| fl[*a].y.total_cmp(&fl[*b].y));
    for (k, &i) in by_y.iter().enumerate() {
        for &j in &by_y[k + 1..] {
            if fl[j].y - fl[i].y >= EPS {
                break;
            }
            if fl[i].mat != fl[j].mat && fl[i].obb.overlaps(&fl[j].obb, EPS) {
                hits.push(Hit::Top(i, j));
            }
        }
    }
    for (a, wa) in wl.iter().enumerate().filter(|(_, w)| w.panel.is_some()) {
        for (b, wb) in wl.iter().enumerate() {
            let pair_once = wb.panel.is_none() || a < b;
            if pair_once && wa.n.dot(wb.n) > 0.999 && (wa.d - wb.d).abs() < EPS {
                let t = wa.t.1.min(wb.t.1) - wa.t.0.max(wb.t.0);
                let y = wa.y.1.min(wb.y.1) - wa.y.0.max(wb.y.0);
                if t > EPS && y > EPS {
                    hits.push(Hit::Wall(a, b));
                }
            }
        }
    }
    hits
}

fn set_top(pieces: &mut [Piece], f: &Flat, y: f32) {
    let d = y - f.y;
    let grow = |center: &mut Vec3, size: &mut Vec3| {
        center.y += d / 2.0;
        size.y += d;
    };
    match &mut pieces[f.src.0] {
        Piece::Box { center, size, .. } | Piece::Slab { center, size, .. } => grow(center, size),
        // A lamp pool is a flat quad: it moves, it has no height to grow.
        Piece::Prim {
            center,
            mat: Mat::LampPool,
            ..
        } => center.y += d,
        Piece::Prim { center, size, .. } => grow(center, size),
        Piece::Merged { parts, .. } => {
            if let Part::Box { center, size, .. } = &mut parts[f.src.1] {
                grow(center, size);
            }
        }
        _ => {}
    }
}

/// Lifts the upper of every pair of coplanar faces (and anything lying on the ground) by a
/// centimetre, and pushes panels out of the face they share a plane with. Visual only: blockers
/// are taken before this runs. Returns the number of adjustments.
pub fn separate(pieces: &mut [Piece], terrain: &Terrain) -> usize {
    let mut moved = 0;
    let start = flats(pieces);
    let origin: std::collections::HashMap<(usize, usize), f32> =
        start.iter().map(|f| (f.src, f.y)).collect();
    // Clamped to the cap; a lift that would not move the piece is not a move.
    let lift_to = |pieces: &mut [Piece], f: &Flat, y: f32| -> bool {
        let y = y.min(origin.get(&f.src).map_or(y, |o| o + CAP));
        if y - f.y < 1e-4 {
            return false;
        }
        set_top(pieces, f, y);
        true
    };
    for _ in 0..MAX_PASSES {
        let (fl, wl) = (flats(pieces), walls(pieces));
        let hits = find(&fl, &wl, terrain);
        if hits.is_empty() {
            break;
        }
        // Heights come from this sweep, so a piece in two hits would be lifted twice and
        // overshoot: it moves at most once per sweep, the next sweep re-measures.
        let mut done = std::collections::HashSet::new();
        let moved_before = moved;
        for hit in hits {
            match hit {
                Hit::Ground(i) => {
                    if done.insert(fl[i].src) {
                        let g = on_ground(&fl[i], terrain).unwrap_or(0.0);
                        moved += usize::from(lift_to(pieces, &fl[i], g + LIFT));
                    }
                }
                Hit::Top(i, j) => {
                    let (lo, hi) = if rank(&fl[i]) < rank(&fl[j]) {
                        (&fl[i], &fl[j])
                    } else {
                        (&fl[j], &fl[i])
                    };
                    if done.insert(hi.src) {
                        moved += usize::from(lift_to(pieces, hi, lo.y.max(hi.y) + LIFT));
                    }
                }
                Hit::Wall(a, b) => {
                    // The panel moves; between two panels the later one.
                    let (p, other) = match (wl[a].panel, wl[b].panel) {
                        (Some(pa), Some(pb)) if pa > pb => (pa, &wl[b]),
                        (Some(_), Some(pb)) => (pb, &wl[a]),
                        (Some(pa), None) => (pa, &wl[b]),
                        _ => continue,
                    };
                    if !done.insert((p, usize::MAX)) {
                        continue;
                    }
                    if let Piece::Panel { center, yaw, .. } = &mut pieces[p] {
                        let n = Vec3::new(yaw.sin(), 0.0, yaw.cos());
                        let push = other.d + LIFT - n.dot(*center);
                        *center += n * push;
                        moved += 1;
                    }
                }
            }
        }
        if moved == moved_before {
            break;
        }
    }
    #[cfg(test)]
    LIFTS.with(|l| {
        let end = flats(pieces);
        *l.borrow_mut() = start
            .iter()
            .zip(&end)
            .map(|(a, b)| {
                (
                    b.y - a.y,
                    format!(
                        "{:?} {} at {:?} top {:.3}",
                        b.mat,
                        kind(&pieces[b.src.0]),
                        b.obb.c,
                        b.y
                    ),
                )
            })
            .collect();
    });
    moved
}

#[cfg(test)]
fn kind(p: &Piece) -> &'static str {
    match p {
        Piece::Box { .. } => "Box",
        Piece::Slab { .. } => "Slab",
        Piece::Prim { .. } => "Prim",
        Piece::Merged { .. } => "Merged",
        _ => "other",
    }
}

#[cfg(test)]
thread_local! {
    /// Lift of every flat face by the last `separate` call on this thread (test report).
    static LIFTS: std::cell::RefCell<Vec<(f32, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The coplanar overlaps a layout still has (the gate test wants none).
#[cfg(test)]
fn remaining(pieces: &[Piece], terrain: &Terrain) -> Vec<String> {
    let (fl, wl) = (flats(pieces), walls(pieces));
    find(&fl, &wl, terrain)
        .iter()
        .map(|h| match h {
            Hit::Ground(i) => format!("ground: {:?}", fl[*i]),
            Hit::Top(i, j) => format!("top: {:?} / {:?}", fl[*i], fl[*j]),
            Hit::Wall(a, b) => format!("wall: {:?} / {:?}", wl[*a], wl[*b]),
        })
        .collect()
}

/// A solid, opaque block with its height span: what can hide the back of a panel.
struct Block {
    obb: Obb,
    y0: f32,
    y1: f32,
}

/// The opaque boxes of a layout, for asking whether a panel's back is against one.
pub struct Backing(Vec<Block>);

impl Backing {
    pub fn new(pieces: &[Piece]) -> Self {
        let opaque = |m: &Mat| {
            !matches!(
                m,
                Mat::Glass | Mat::Frosted | Mat::Window | Mat::Facade | Mat::Lake
            )
        };
        let blocks = pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Box {
                    center, size, mat, ..
                } if opaque(mat) => Some((center, size, 0.0)),
                Piece::Slab {
                    center,
                    size,
                    yaw,
                    mat,
                    pitch,
                } if opaque(mat) && *pitch == 0.0 => Some((center, size, *yaw)),
                _ => None,
            })
            .map(|(c, s, yaw)| Block {
                obb: Obb::new(*c, *s, yaw),
                y0: c.y - s.y / 2.0,
                y1: c.y + s.y / 2.0,
            })
            .collect();
        Self(blocks)
    }

    fn solid_at(&self, p: Vec3) -> bool {
        let q = Vec2::new(p.x, p.z);
        self.0
            .iter()
            .any(|b| p.y >= b.y0 && p.y <= b.y1 && b.obb.contains(q))
    }

    /// True when the panel's back is flush against solid geometry (centre and four inset points
    /// all covered), so a back twin would be hidden inside it and only cost an entity. The twin
    /// of a free-standing sign is what makes its text read the right way round from behind.
    pub fn covers_back(&self, center: Vec3, yaw: f32, size: Vec2) -> bool {
        let (s, c) = yaw.sin_cos();
        let (n, t) = (Vec3::new(s, 0.0, c), Vec3::new(c, 0.0, -s));
        let back = center - n * BACK_PROBE;
        [
            (0.0, 0.0),
            (0.3, 0.3),
            (-0.3, 0.3),
            (0.3, -0.3),
            (-0.3, -0.3),
        ]
        .iter()
        .all(|(u, v)| self.solid_at(back + t * (u * size.x) + Vec3::Y * (v * size.y)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floors::kit::Tex;
    use crate::floors::zones::{ZONE_IDS, build_zone};
    use crate::floors::{client, corporate, teambuilding};

    fn all_zones() -> Vec<&'static str> {
        let mut ids: Vec<&'static str> = ZONE_IDS.to_vec();
        ids.extend(corporate::ZONES);
        ids.push(client::ZONE);
        ids.push(teambuilding::ZONE);
        ids
    }

    fn cube_at(top: f32, mat: Mat) -> Piece {
        Piece::Box {
            center: Vec3::new(0.0, top - 0.5, 0.0),
            size: Vec3::ONE,
            mat,
            solid: false,
        }
    }

    /// The gate is only worth something if the detector can fail.
    #[test]
    fn detector_sees_coplanar_faces_and_separate_clears_them() {
        let wall = Piece::Box {
            center: Vec3::new(5.0, 1.0, 0.0),
            size: Vec3::new(2.0, 2.0, 0.2),
            mat: Mat::Wall,
            solid: true,
        };
        let panel = Piece::Panel {
            center: Vec3::new(5.0, 1.0, 0.105),
            yaw: 0.0,
            size: Vec2::ONE,
            tex: Tex::Misc,
        };
        let mut pieces = vec![cube_at(0.5, Mat::Wood), cube_at(0.5, Mat::Red), wall, panel];
        let ground = Terrain::default();
        assert!(
            remaining(&pieces, &ground).len() >= 2,
            "top pair, ground and panel on wall"
        );
        assert!(separate(&mut pieces, &ground) > 0);
        assert_eq!(remaining(&pieces, &ground), Vec::<String>::new());
    }

    /// Z-fighting gate: no zone keeps a panel or ground-level face coplanar with another face, and
    /// no piece floats more than `CAP`. Pairs of different materials at one height that the cap
    /// could not lift apart ("tied") are accepted: each material has its own depth bias, so they
    /// draw in a fixed order. Fix a new hit at the source (offset by a few centimetres);
    /// `separate` in `assemble` is only the safety net.
    #[test]
    fn no_coplanar_overlaps_in_any_zone() {
        let mut report = String::new();
        for id in all_zones() {
            let l = build_zone(id).unwrap_or_else(|| panic!("zone {id}"));
            let max = LIFTS.with(|l| l.borrow().iter().map(|(d, _)| *d).fold(0.0_f32, f32::max));
            assert!(max <= CAP + 1e-3, "{id}: a piece was lifted {max:.3} m");
            for h in remaining(&l.pieces, &l.terrain)
                .iter()
                .filter(|h| !h.starts_with("top:"))
            {
                report.push_str(&format!("{id}: {h}\n"));
            }
        }
        assert!(report.is_empty(), "coplanar overlaps:\n{report}");
    }

    /// `cargo test -p game_client lift_report -- --ignored --nocapture`: biggest cumulative lift.
    #[test]
    #[ignore = "report only"]
    fn lift_report() {
        let mut all = Vec::new();
        for id in all_zones() {
            build_zone(id).unwrap_or_else(|| panic!("zone {id}"));
            let l = LIFTS.with(|l| l.borrow().clone());
            let max = l.iter().map(|(d, _)| *d).fold(0.0_f32, f32::max);
            let tied = remaining(&build_zone(id).unwrap().pieces, &Terrain::default()).len();
            eprintln!(
                "LIFT {id} max {max:.3} lifted {} tied~{tied}",
                l.iter().filter(|(d, _)| *d > 0.0).count()
            );
            all.extend(l.into_iter().map(|(d, s)| (d, format!("{id}: {s}"))));
        }
        all.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (d, s) in all.iter().take(10) {
            eprintln!("WORST {d:.3} {s}");
        }
    }
}
