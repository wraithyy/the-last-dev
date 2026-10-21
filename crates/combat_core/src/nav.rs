//! Navigation: a coarse grid over the blockers, A*, and per-unit path following.
//!
//! The grid is rebuilt whenever blockers change. Units keep moving in straight lines while the
//! segment to the goal is clear for their radius; only when a wall is in the way do they follow a
//! cached, string-pulled path. Everything here is integer-cost A* with fixed neighbour order and
//! index tie-breaks, so the same inputs always give the same path (no rng, no hashing).

use crate::geom::{Aabb, Vec3, line_of_sight};
use crate::sim::Sim;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const CELL_M: f32 = 0.5;
/// Clearance kept to blockers; also the radius used for straight-line checks.
pub(crate) const NAV_RADIUS_M: f32 = 0.45;
/// Only blockers a standing unit can bump into matter; floor slabs and ceilings are ignored.
const SOLID_MIN_Y: f32 = 0.3;
const SOLID_MAX_Y: f32 = 1.8;
const MARGIN_M: f32 = 2.0;
/// Node expansion cap per search, so a hopeless goal costs bounded time.
const MAX_EXPANSIONS: usize = 6000;
const REPLAN_TICKS: u64 = 15;
const REPLAN_GOAL_M: f32 = 1.5;
/// Close enough to a waypoint to count it as reached.
const REACH_M: f32 = 0.3;
/// Chasing units with no route evade after this long (classic MMO behaviour).
pub(crate) const NO_PATH_EVADE_TICKS: u64 = 150;
/// How far (in cells) a start or goal inside a blocked cell may snap to free space. Kept under
/// a wall's thickness plus clearance so a goal behind a thin wall never snaps to this side.
const SNAP_CELLS: i32 = 2;
const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;
const NEIGHBOURS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

#[derive(Default)]
struct Grid {
    min_x: f32,
    min_z: f32,
    w: i32,
    h: i32,
    blocked: Vec<bool>,
    /// Blockers grown by `NAV_RADIUS_M` on x/z and stretched on y, so the stock segment test
    /// answers "can a unit of this radius walk this line" at any unit height.
    inflated: Vec<Aabb>,
}

#[derive(Default)]
pub(crate) struct Nav {
    grid: Grid,
    scratch: Scratch,
}

/// Per-unit cached route. Buffers are reused between replans.
#[derive(Default)]
pub(crate) struct NavAgent {
    path: Vec<Vec3>,
    next: usize,
    goal: Vec3,
    replan_at: u64,
    /// First tick of the current run of failed searches.
    pub failed_since: Option<u64>,
}

impl NavAgent {
    pub(crate) fn reset(&mut self) {
        self.path.clear();
        self.next = 0;
        self.replan_at = 0;
        self.failed_since = None;
    }
}

#[derive(Default)]
struct Scratch {
    stamp: u32,
    seen: Vec<u32>,
    closed: Vec<u32>,
    g: Vec<u32>,
    parent: Vec<u32>,
    heap: BinaryHeap<Node>,
    cells: Vec<u32>,
    points: Vec<Vec3>,
}

#[derive(PartialEq, Eq)]
struct Node {
    f: u32,
    h: u32,
    idx: u32,
}

impl Ord for Node {
    // Reversed: BinaryHeap pops the max, we want the lowest f, then lowest h, then lowest index.
    fn cmp(&self, o: &Self) -> Ordering {
        (o.f, o.h, o.idx).cmp(&(self.f, self.h, self.idx))
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Nav {
    pub(crate) fn build(blockers: &[Aabb], terrain: &crate::terrain::Terrain) -> Nav {
        // On uneven ground a box counts by its height above the ground it stands on.
        let ground =
            |b: &Aabb| terrain.height((b.min.x + b.max.x) / 2.0, (b.min.z + b.max.z) / 2.0);
        let inflated: Vec<Aabb> = blockers
            .iter()
            .filter(|b| {
                let g = ground(b);
                b.max.y - g > SOLID_MIN_Y && b.min.y - g < SOLID_MAX_Y
            })
            .map(|b| Aabb {
                min: Vec3::new(b.min.x - NAV_RADIUS_M, -1e4, b.min.z - NAV_RADIUS_M),
                max: Vec3::new(b.max.x + NAV_RADIUS_M, 1e4, b.max.z + NAV_RADIUS_M),
            })
            .collect();
        if inflated.is_empty() {
            return Nav::default();
        }
        let fold = |f: fn(f32, f32) -> f32, init: f32, get: fn(&Aabb) -> f32| {
            inflated.iter().map(get).fold(init, f)
        };
        let min_x = fold(f32::min, f32::MAX, |b| b.min.x) - MARGIN_M;
        let min_z = fold(f32::min, f32::MAX, |b| b.min.z) - MARGIN_M;
        let max_x = fold(f32::max, f32::MIN, |b| b.max.x) + MARGIN_M;
        let max_z = fold(f32::max, f32::MIN, |b| b.max.z) + MARGIN_M;
        let w = ((max_x - min_x) / CELL_M).ceil() as i32;
        let h = ((max_z - min_z) / CELL_M).ceil() as i32;
        let mut blocked = vec![false; (w * h) as usize];
        for b in &inflated {
            let lo_x = (((b.min.x - min_x) / CELL_M).floor() as i32).max(0);
            let hi_x = (((b.max.x - min_x) / CELL_M).ceil() as i32).min(w - 1);
            let lo_z = (((b.min.z - min_z) / CELL_M).floor() as i32).max(0);
            let hi_z = (((b.max.z - min_z) / CELL_M).ceil() as i32).min(h - 1);
            for cz in lo_z..=hi_z {
                for cx in lo_x..=hi_x {
                    let (x, z) = (
                        min_x + (cx as f32 + 0.5) * CELL_M,
                        min_z + (cz as f32 + 0.5) * CELL_M,
                    );
                    if x > b.min.x && x < b.max.x && z > b.min.z && z < b.max.z {
                        blocked[(cz * w + cx) as usize] = true;
                    }
                }
            }
        }
        Nav {
            grid: Grid {
                min_x,
                min_z,
                w,
                h,
                blocked,
                inflated,
            },
            scratch: Scratch::default(),
        }
    }
}

impl Grid {
    fn clear(&self, a: Vec3, b: Vec3) -> bool {
        let (lo_x, hi_x) = (a.x.min(b.x), a.x.max(b.x));
        let (lo_z, hi_z) = (a.z.min(b.z), a.z.max(b.z));
        !self.inflated.iter().any(|bx| {
            bx.max.x >= lo_x
                && bx.min.x <= hi_x
                && bx.max.z >= lo_z
                && bx.min.z <= hi_z
                && !line_of_sight(
                    Vec3::new(a.x, 0.0, a.z),
                    Vec3::new(b.x, 0.0, b.z),
                    std::slice::from_ref(bx),
                )
        })
    }

    fn cell_of(&self, p: Vec3) -> (i32, i32) {
        (
            (((p.x - self.min_x) / CELL_M).floor() as i32).clamp(0, self.w - 1),
            (((p.z - self.min_z) / CELL_M).floor() as i32).clamp(0, self.h - 1),
        )
    }

    fn center(&self, idx: u32, y: f32) -> Vec3 {
        let (cx, cz) = (idx as i32 % self.w, idx as i32 / self.w);
        Vec3::new(
            self.min_x + (cx as f32 + 0.5) * CELL_M,
            y,
            self.min_z + (cz as f32 + 0.5) * CELL_M,
        )
    }

    fn free(&self, cx: i32, cz: i32) -> bool {
        cx >= 0
            && cz >= 0
            && cx < self.w
            && cz < self.h
            && !self.blocked[(cz * self.w + cx) as usize]
    }

    /// Nearest free cell to `p`, scanning rings outward in a fixed order.
    fn snap(&self, p: Vec3) -> Option<u32> {
        let (cx, cz) = self.cell_of(p);
        for r in 0..=SNAP_CELLS {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dz.abs()) == r && self.free(cx + dx, cz + dz) {
                        return Some(((cz + dz) * self.w + cx + dx) as u32);
                    }
                }
            }
        }
        None
    }
}

impl Scratch {
    fn prepare(&mut self, cells: usize) {
        if self.seen.len() != cells {
            self.seen = vec![0; cells];
            self.closed = vec![0; cells];
            self.g = vec![0; cells];
            self.parent = vec![0; cells];
            self.stamp = 0;
        }
        self.stamp += 1;
        self.heap.clear();
    }
}

/// Waypoints from `from` (exclusive) to `to` (inclusive) into `out`; false when unreachable
/// within `cap` expansions.
fn search(
    grid: &Grid,
    s: &mut Scratch,
    from: Vec3,
    to: Vec3,
    cap: usize,
    out: &mut Vec<Vec3>,
) -> bool {
    out.clear();
    if grid.blocked.is_empty() || grid.clear(from, to) {
        out.push(to);
        return true;
    }
    let (Some(start), Some(goal)) = (grid.snap(from), grid.snap(to)) else {
        return false;
    };
    s.prepare(grid.blocked.len());
    let stamp = s.stamp;
    let goal_xy = (goal as i32 % grid.w, goal as i32 / grid.w);
    let heur = |idx: u32| {
        let (dx, dz) = (
            (idx as i32 % grid.w - goal_xy.0).unsigned_abs(),
            (idx as i32 / grid.w - goal_xy.1).unsigned_abs(),
        );
        STRAIGHT * dx.max(dz) + (DIAGONAL - STRAIGHT) * dx.min(dz)
    };
    s.seen[start as usize] = stamp;
    s.g[start as usize] = 0;
    s.parent[start as usize] = start;
    s.heap.push(Node {
        f: heur(start),
        h: heur(start),
        idx: start,
    });
    let mut expansions = 0;
    let mut found = false;
    while let Some(Node { idx, .. }) = s.heap.pop() {
        if s.closed[idx as usize] == stamp {
            continue;
        }
        s.closed[idx as usize] = stamp;
        if idx == goal {
            found = true;
            break;
        }
        expansions += 1;
        if expansions > cap {
            return false;
        }
        let (cx, cz) = (idx as i32 % grid.w, idx as i32 / grid.w);
        for (dx, dz) in NEIGHBOURS {
            let (nx, nz) = (cx + dx, cz + dz);
            if !grid.free(nx, nz) {
                continue;
            }
            // No corner cutting: a diagonal needs both orthogonal neighbours open.
            let diagonal = dx != 0 && dz != 0;
            if diagonal && !(grid.free(cx + dx, cz) && grid.free(cx, cz + dz)) {
                continue;
            }
            let n = (nz * grid.w + nx) as u32;
            if s.closed[n as usize] == stamp {
                continue;
            }
            let g = s.g[idx as usize] + if diagonal { DIAGONAL } else { STRAIGHT };
            if s.seen[n as usize] != stamp || g < s.g[n as usize] {
                s.seen[n as usize] = stamp;
                s.g[n as usize] = g;
                s.parent[n as usize] = idx;
                let h = heur(n);
                s.heap.push(Node {
                    f: g + h,
                    h,
                    idx: n,
                });
            }
        }
    }
    if !found {
        return false;
    }
    s.cells.clear();
    let mut c = goal;
    while c != start {
        s.cells.push(c);
        c = s.parent[c as usize];
    }
    s.cells.push(start);
    s.cells.reverse();
    // String-pull: keep a cell only when the next one is no longer visible from the anchor.
    s.points.clear();
    s.points.push(from);
    for &c in &s.cells {
        s.points.push(grid.center(c, from.y));
    }
    s.points.push(Vec3::new(to.x, from.y, to.z));
    let mut anchor = s.points[0];
    for j in 1..s.points.len() - 1 {
        if !grid.clear(anchor, s.points[j + 1]) {
            anchor = s.points[j];
            out.push(anchor);
        }
    }
    out.push(to);
    true
}

impl Sim {
    pub(crate) fn rebuild_nav(&mut self) {
        self.nav = Nav::build(&self.blockers, &self.terrain);
        self.los = crate::geom::BlockerGrid::new(self.blockers.clone());
        self.los_padded = crate::geom::BlockerGrid::new(crate::wander::padded(&self.blockers));
        for u in &mut self.units {
            u.nav.reset();
        }
    }

    /// Walkable route for a unit of the standard radius: waypoints after `from`, ending at `to`.
    /// `None` when the goal is walled off or too far to search. Allocates; meant for tools like
    /// the client autofight bot, not the tick loop.
    pub fn find_path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let mut out = Vec::new();
        let mut scratch = Scratch::default();
        let cap = self.nav.grid.blocked.len().max(MAX_EXPANSIONS);
        search(&self.nav.grid, &mut scratch, from, to, cap, &mut out).then_some(out)
    }

    /// True once unit `i` has had no route for `NO_PATH_EVADE_TICKS`.
    pub(crate) fn nav_gave_up(&self, i: usize) -> bool {
        self.units[i]
            .nav
            .failed_since
            .is_some_and(|t| self.now - t >= NO_PATH_EVADE_TICKS)
    }

    /// Where unit `i` should head next to reach `goal`, and whether that point is the goal
    /// itself. `None` means no route is known; `failed_since` then tells how long that has lasted.
    pub(crate) fn nav_target(&mut self, i: usize, goal: Vec3) -> Option<(Vec3, bool)> {
        let now = self.now;
        let pos = self.units[i].state.pos;
        let Nav { grid, scratch } = &mut self.nav;
        let a = &mut self.units[i].nav;
        if grid.clear(pos, goal) {
            a.reset();
            return Some((goal, true));
        }
        let moved = (a.goal.x - goal.x).hypot(a.goal.z - goal.z) > REPLAN_GOAL_M;
        if now >= a.replan_at || (moved && !a.path.is_empty()) {
            let mut path = std::mem::take(&mut a.path);
            let ok = search(grid, scratch, pos, goal, MAX_EXPANSIONS, &mut path);
            a.path = path;
            a.next = 0;
            a.goal = goal;
            a.replan_at = now + REPLAN_TICKS;
            if ok {
                a.failed_since = None;
            } else {
                a.path.clear();
                a.failed_since.get_or_insert(now);
            }
        }
        if a.path.is_empty() {
            return None;
        }
        let last = a.path.len() - 1;
        // Skip waypoints that are reached or already visible from here (follow-time smoothing).
        while a.next < last
            && ((a.path[a.next].x - pos.x).hypot(a.path[a.next].z - pos.z) < REACH_M
                || grid.clear(pos, a.path[a.next + 1]))
        {
            a.next += 1;
        }
        Some((a.path[a.next], a.next == last))
    }
}
