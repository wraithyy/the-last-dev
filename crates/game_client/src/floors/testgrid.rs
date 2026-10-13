//! Test-only walkability grid shared by the tower and zone layout tests.

use super::layout::FloorLayout;
use crate::config::{FLOOR_HALF_X, FLOOR_HALF_Z, PLAYER_RADIUS};
use bevy::prelude::*;
use std::collections::VecDeque;

pub const CELL: f32 = 0.5;

/// Player-sized walkability grid over the whole clamp area with BFS path lengths from a point.
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub blocked: Vec<bool>,
    pub dist: Vec<f32>,
}

impl Grid {
    pub fn new(layout: &FloorLayout) -> Self {
        let w = (FLOOR_HALF_X * 2.0 / CELL) as usize;
        let h = (FLOOR_HALF_Z * 2.0 / CELL) as usize;
        let mut blocked = vec![false; w * h];
        for b in layout
            .blockers
            .iter()
            .filter(|b| b.max.y > 0.3 && b.min.y < 1.8)
        {
            let lo =
                |v: f32, half: f32| (((v - PLAYER_RADIUS + half) / CELL).floor().max(0.0)) as usize;
            let hi = |v: f32, half: f32, n: usize| {
                (((v + PLAYER_RADIUS + half) / CELL).ceil().max(0.0) as usize).min(n - 1)
            };
            for j in lo(b.min.z, FLOOR_HALF_Z)..=hi(b.max.z, FLOOR_HALF_Z, h) {
                for i in lo(b.min.x, FLOOR_HALF_X)..=hi(b.max.x, FLOOR_HALF_X, w) {
                    let (x, z) = (
                        i as f32 * CELL - FLOOR_HALF_X + CELL / 2.0,
                        j as f32 * CELL - FLOOR_HALF_Z + CELL / 2.0,
                    );
                    if x + PLAYER_RADIUS > b.min.x
                        && x - PLAYER_RADIUS < b.max.x
                        && z + PLAYER_RADIUS > b.min.z
                        && z - PLAYER_RADIUS < b.max.z
                    {
                        blocked[j * w + i] = true;
                    }
                }
            }
        }
        Self {
            w,
            h,
            blocked,
            dist: Vec::new(),
        }
    }

    pub fn cell(&self, p: Vec3) -> usize {
        let i = (((p.x + FLOOR_HALF_X) / CELL) as usize).min(self.w - 1);
        let j = (((p.z + FLOOR_HALF_Z) / CELL) as usize).min(self.h - 1);
        j * self.w + i
    }

    pub fn from(layout: &FloorLayout, from: Vec3) -> Self {
        let mut g = Self::new(layout);
        let mut dist = vec![f32::INFINITY; g.w * g.h];
        let s = g.cell(from);
        dist[s] = 0.0;
        let mut q = VecDeque::from([s]);
        while let Some(k) = q.pop_front() {
            let (i, j) = ((k % g.w) as i32, (k / g.w) as i32);
            for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (ni, nj) = (i + di, j + dj);
                if ni < 0 || nj < 0 || ni as usize >= g.w || nj as usize >= g.h {
                    continue;
                }
                let n = nj as usize * g.w + ni as usize;
                if !g.blocked[n] && dist[n].is_infinite() {
                    dist[n] = dist[k] + CELL;
                    q.push_back(n);
                }
            }
        }
        g.dist = dist;
        g
    }

    pub fn at(&self, p: Vec3) -> f32 {
        self.dist[self.cell(p)]
    }

    pub fn free(&self, p: Vec3) -> bool {
        !self.blocked[self.cell(p)]
    }
}

pub fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let d = b - a;
    let t = ((p - a).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + d * t)
}

pub fn route_dist(l: &FloorLayout, p: Vec3) -> f32 {
    let q = Vec2::new(p.x, p.z);
    l.route
        .windows(2)
        .map(|w| seg_dist(q, w[0], w[1]))
        .fold(f32::INFINITY, f32::min)
}
