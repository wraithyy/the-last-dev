//! Data-center dressing shared by both sublevels: rack rows, cabling, cooling and wall gear.
//! Everything here is visual (no new colliders), so the walkable space stays the zone's own.
//! Floor tiles are 1 m with seams on whole metres, so tile-sized pieces sit on `k + 0.5`.

use crate::floors::kit::*;
use crate::floors::shape::Seg;
use crate::floors::themes::{pt, v};
use bevy::prelude::*;
use std::f32::consts::PI;

/// Cheap deterministic noise in 0..1 so rows look hand-built but every build is identical.
pub fn rnd(a: i32, b: i32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ (b as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

fn blinker(hot: bool, k: f32) -> Mat {
    let cold = [
        Mat::BlinkGreenA,
        Mat::BlinkGreenB,
        Mat::BlinkBlue,
        Mat::BlinkGreenA,
        Mat::BlinkAmber,
    ];
    let warm = [
        Mat::BlinkRed,
        Mat::BlinkAmber,
        Mat::BlinkRed,
        Mat::BlinkAmber,
        Mat::LedRed,
    ];
    let list = if hot { warm } else { cold };
    list[(k * 5.0) as usize % 5]
}

/// One rack front: a textured face (one quad instead of a dozen boxes) and two blinkers.
/// `height` is the face height; full racks are 2 m.
pub fn rack_front(p: &mut Plan, x: f32, z: f32, yaw: f32, hot: bool, seed: i32, height: f32) {
    let k = rnd(seed, 1);
    let tex = if hot {
        Tex::RackScorched
    } else if k < 0.58 {
        Tex::RackBlades
    } else if k < 0.8 {
        Tex::RackPatch
    } else {
        Tex::RackDrives
    };
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.panel(
            v(0.0, 0.1 + height / 2.0, 0.508),
            0.0,
            Vec2::new(0.9, height),
            tex,
        );
        for i in 0..2 {
            let y = 0.3 + rnd(seed, 20 + i) * (height - 0.5);
            p.boxed(
                v(-0.28 + i as f32 * 0.56, y, 0.52),
                v(0.07, 0.05, 0.02),
                blinker(hot, rnd(seed, 10 + i)),
                false,
            );
        }
    });
}

/// Row of racks along X from `x0` to `x1` at `z`, fronts towards `+Z` for yaw 0 and `-Z` for
/// PI. The body is one solid box (the row's collider); everything else is surface detail.
pub fn rack_row(p: &mut Plan, x0: f32, x1: f32, z: f32, yaw: f32, hot: bool, backs: bool) {
    let n = (x1 - x0).round() as usize;
    let (len, cx) = (x1 - x0, (x0 + x1) / 2.0);
    p.boxed(v(cx, 1.1, z), v(len, 2.2, 1.0), Mat::Rack, true);
    p.boxed(
        v(cx, 2.23, z),
        v(len + 0.04, 0.06, 1.06),
        Mat::RackPanel,
        false,
    );
    p.boxed(v(cx, 0.05, z), v(len + 0.04, 0.1, 1.08), Mat::Dark, false);
    for i in 0..n {
        let x = x0 + 0.5 + i as f32;
        let seed = i as i32 * 7 + z.abs() as i32 * 3;
        rack_front(p, x, z, yaw, hot, seed, 2.0);
        if backs {
            p.framed(v(x, 0.0, z), yaw, |p| {
                let tex = if hot { Tex::RackBackHot } else { Tex::RackBack };
                p.panel(v(0.0, 1.1, -0.508), PI, Vec2::new(0.9, 2.0), tex);
            });
        }
        // Risers: bundles that leave the rack top and vanish into the ceiling.
        if rnd(seed, 40) > 0.82 {
            p.prim(
                PrimKind::Cylinder,
                v(x, (2.26 + H) / 2.0, z),
                v(0.14, H - 2.26, 0.14),
                Mat::Dark,
            );
        }
    }
}

/// Overhead cable tray (ladder rack): base, rails, three bundles and rungs, hung from the
/// ceiling. 3.7 m up, above the camera's reach in the aisles.
pub fn cable_tray(p: &mut Plan, a: Vec2, b: Vec2, hot: bool) {
    let (mid, d) = ((a + b) / 2.0, b - a);
    let len = d.length();
    let yaw = (-d.y).atan2(d.x);
    let side = Vec2::new(-d.y, d.x) / len;
    let at = |l: f32| mid + side * l;
    p.slab(v(mid.x, 3.7, mid.y), yaw, v(len, 0.06, 0.6), Mat::Dark);
    for s in [-0.3, 0.3] {
        let c = at(s);
        p.slab(v(c.x, 3.77, c.y), yaw, v(len, 0.14, 0.04), Mat::Metal);
    }
    let bundles = [
        (-0.2, Mat::Dark),
        (-0.07, if hot { Mat::Red } else { Mat::Scr1 }),
        (0.07, Mat::Yellow),
        (0.2, Mat::Dark),
    ];
    for (l, m) in bundles {
        let c = at(l);
        p.slab(v(c.x, 3.76, c.y), yaw, v(len * 0.96, 0.08, 0.09), m);
    }
    let rungs = (len / 3.0).floor() as i32;
    for i in 0..=rungs {
        let c = a + d * ((i as f32 + 0.5) / (rungs + 1) as f32);
        p.slab(v(c.x, 3.73, c.y), yaw, v(0.06, 0.04, 0.66), Mat::Metal);
    }
    let hangers = (len / 6.0).floor() as i32;
    for i in 0..=hangers {
        let c = a + d * ((i as f32 + 0.5) / (hangers + 1) as f32);
        p.prim(
            PrimKind::Cylinder,
            v(c.x, (3.7 + H) / 2.0, c.y),
            v(0.04, H - 3.7, 0.04),
            Mat::Metal,
        );
    }
}

/// Ceiling-hugging square pipe along X or Z (chilled water, fire suppression).
pub fn pipe_run(p: &mut Plan, a: Vec2, b: Vec2, y: f32, mat: Mat) {
    let (mid, d) = ((a + b) / 2.0, b - a);
    let yaw = (-d.y).atan2(d.x);
    p.slab(v(mid.x, y, mid.y), yaw, v(d.length(), 0.14, 0.14), mat);
    let n = (d.length() / 4.0).floor() as i32;
    for i in 0..=n {
        let c = a + d * ((i as f32 + 0.5) / (n + 1) as f32);
        p.slab(v(c.x, y, c.y), yaw, v(0.12, 0.2, 0.2), Mat::Metal);
    }
}

/// Floor-standing cooling unit (CRAC): grille front, status panel, riser duct to the ceiling.
/// Faces `face`.
pub fn cooling_unit(p: &mut Plan, x: f32, z: f32, face: Face, hot: bool) {
    p.framed(v(x, 0.0, z), face.yaw(), |p| {
        p.boxed(v(0.0, 1.1, 0.0), v(2.2, 2.2, 1.0), Mat::Metal, true);
        p.boxed(v(0.0, 0.06, 0.03), v(2.3, 0.12, 1.06), Mat::Dark, false);
        p.boxed(v(0.0, 1.0, 0.51), v(1.8, 1.4, 0.02), Mat::Dark, false);
        for k in 0..4 {
            p.boxed(
                v(0.0, 0.55 + k as f32 * 0.3, 0.53),
                v(1.7, 0.04, 0.02),
                Mat::Metal,
                false,
            );
        }
        p.boxed(
            v(0.0, 2.0, 0.52),
            v(1.6, 0.06, 0.02),
            if hot { Mat::Hot } else { Mat::Cold },
            false,
        );
        p.boxed(v(0.0, 2.22, 0.0), v(1.5, 0.04, 0.7), Mat::Dark, false);
        // Riser: the cold (or, here, not so cold) air column up to the plenum.
        p.prim(
            PrimKind::Cylinder,
            v(0.6, (2.2 + H) / 2.0, -0.1),
            v(0.7, H - 2.2, 0.7),
            Mat::Metal,
        );
        for y in [2.5, H - 0.4] {
            p.prim(
                PrimKind::Cylinder,
                v(0.6, y, -0.1),
                v(0.85, 0.08, 0.85),
                Mat::Dark,
            );
        }
        if hot {
            p.panel(
                v(-0.5, 1.7, 0.53),
                0.0,
                Vec2::new(0.5, 0.28),
                Tex::NocAlerts,
            );
        } else {
            p.boxed(v(-0.5, 1.7, 0.53), v(0.5, 0.28, 0.02), Mat::Dark, false);
            p.boxed(
                v(-0.62, 1.7, 0.545),
                v(0.08, 0.08, 0.02),
                Mat::LedGreen,
                false,
            );
        }
    });
}

/// A wall position with the frame to hang things on it: `n` points into the room.
pub struct WallSpot {
    pt: Vec2,
    n: Vec2,
    yaw: f32,
    lat: Vec2,
}

impl WallSpot {
    pub fn at(p: &Plan, x: f32, z: f32) -> Option<Self> {
        let (pt, n) = p.wall_near(x, z)?;
        let yaw = n.x.atan2(n.y);
        Some(Self {
            pt,
            n,
            yaw,
            lat: Vec2::new(yaw.cos(), -yaw.sin()),
        })
    }

    /// Box `size` (along the wall, tall, out of the wall) centred `l` along the wall, `y` up,
    /// `d` out from the wall surface.
    pub fn put(&self, p: &mut Plan, l: f32, y: f32, d: f32, size: Vec3, mat: Mat) {
        let c = self.pt + self.n * d + self.lat * l;
        p.slab(v(c.x, y, c.y), self.yaw, size, mat);
    }

    /// Ground point `l` along the wall and `d` out of it.
    pub fn world(&self, l: f32, d: f32) -> Vec2 {
        self.pt + self.n * d + self.lat * l
    }

    pub fn sign(&self, p: &mut Plan, l: f32, y: f32, width: f32, sign: Sign) {
        let c = self.pt + self.n * 0.06 + self.lat * l;
        p.sign(v(c.x, y, c.y), self.yaw, width, sign);
    }
}

/// Wall paint breaks: a concrete (or scorched brick) wainscot, a conduit and a cable duct.
/// One strip mesh each, so the whole zone costs four draw calls.
pub fn wall_bands(p: &mut Plan, hot: bool) {
    let segs: Vec<Seg> = p.walls.clone();
    let bands = [
        (
            if hot { Mat::Brick } else { Mat::Concrete },
            0.2,
            1.0,
            0.012,
        ),
        (Mat::Trim, 1.0, 1.06, 0.02),
        (Mat::Metal, 3.2, 3.28, 0.05),
        (Mat::Dark, 3.5, 3.78, 0.035),
    ];
    for (mat, y0, y1, inset) in bands {
        p.pieces.push(Piece::Run {
            segs: segs.clone(),
            mat,
            y0,
            y1,
            inset,
        });
    }
}

/// Floor-standing electrical cabinet against the nearest wall: seam, warning label, status LEDs.
pub fn wall_cabinet(p: &mut Plan, x: f32, z: f32, hot: bool) {
    let Some(w) = WallSpot::at(p, x, z) else {
        return;
    };
    w.put(p, 0.0, 1.0, 0.2, v(1.1, 2.0, 0.34), Mat::RackPanel);
    w.put(p, 0.0, 1.0, 0.375, v(0.02, 1.9, 0.02), Mat::Dark);
    w.put(p, -0.28, 1.5, 0.385, v(0.22, 0.22, 0.02), Mat::Hazard);
    for (i, m) in [Mat::LedGreen, Mat::LedGreen, Mat::LedRed]
        .into_iter()
        .enumerate()
    {
        let m = if hot && i == 1 { Mat::LedRed } else { m };
        w.put(p, 0.2 + i as f32 * 0.12, 1.7, 0.385, v(0.06, 0.06, 0.02), m);
    }
}

/// Wall-hung fire extinguisher with a red marker plate above it.
pub fn extinguisher(p: &mut Plan, x: f32, z: f32) {
    let Some(w) = WallSpot::at(p, x, z) else {
        return;
    };
    w.put(p, 0.0, 1.9, 0.02, v(0.36, 0.36, 0.03), Mat::Red);
    let c = w.world(0.0, 0.14);
    p.prim(
        PrimKind::Cylinder,
        v(c.x, 0.85, c.y),
        v(0.2, 0.7, 0.2),
        Mat::Red,
    );
    p.prim(
        PrimKind::Cylinder,
        v(c.x, 1.25, c.y),
        v(0.07, 0.12, 0.07),
        Mat::Dark,
    );
}

/// Glowing perforated-tile vent in a cold aisle (`k + 0.5` keeps it inside one floor tile).
pub fn vent_tile(p: &mut Plan, x: f32, z: f32, mat: Mat) {
    p.slab(v(x, 0.012, z), 0.0, v(0.7, 0.012, 0.7), mat);
}

/// Thin glowing or painted floor line.
pub fn floor_line(p: &mut Plan, a: Vec2, b: Vec2, w: f32, mat: Mat) {
    let (mid, d) = ((a + b) / 2.0, b - a);
    p.slab(
        v(mid.x, 0.012, mid.y),
        (-d.y).atan2(d.x),
        v(d.length(), 0.012, w),
        mat,
    );
}

/// A loose bundle of cables across the floor: a thick black run with a coloured one beside it.
pub fn cable_bundle(p: &mut Plan, pts: &[(f32, f32)], accent: Mat) {
    for w in pts.windows(2) {
        let (a, b) = (pt(w[0].0, w[0].1), pt(w[1].0, w[1].1));
        let (mid, d) = ((a + b) / 2.0, b - a);
        let yaw = (-d.y).atan2(d.x);
        let side = Vec2::new(-d.y, d.x).normalize_or_zero() * 0.07;
        p.slab(
            v(mid.x, 0.03, mid.y),
            yaw,
            v(d.length() + 0.1, 0.05, 0.1),
            Mat::Dark,
        );
        p.slab(
            v(mid.x + side.x, 0.025, mid.y + side.y),
            yaw,
            v(d.length() + 0.1, 0.04, 0.05),
            accent,
        );
    }
}

/// A lifted floor tile with a thick cable column rising through the hole to the ceiling tray.
pub fn floor_hole(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.slab(v(x, 0.012, z), 0.0, v(0.96, 0.012, 0.96), Mat::Dark);
    p.slab(
        v(x + 0.75, 0.04, z + 0.35),
        yaw,
        v(0.92, 0.04, 0.92),
        Mat::RackPanel,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x, 1.85, z),
        v(0.22, 3.7, 0.22),
        Mat::Dark,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x + 0.12, 1.2, z),
        v(0.07, 2.4, 0.07),
        Mat::Red,
    );
    p.prim(
        PrimKind::Cylinder,
        v(x - 0.1, 0.9, z + 0.06),
        v(0.07, 1.8, 0.07),
        Mat::Yellow,
    );
}

/// Steam vent: a few see-through puffs stacked above a floor vent tile. Static and flat shaded,
/// so it costs a handful of draw calls and no particles on weak GPUs.
pub fn steam(p: &mut Plan, x: f32, z: f32, puffs: usize) {
    for k in 0..puffs {
        let d = 0.55 + k as f32 * 0.22;
        p.prim(
            PrimKind::Sphere,
            v(x + (k as f32 * 0.37).sin() * 0.15, 0.3 + k as f32 * 0.5, z),
            v(d, d * 0.7, d),
            Mat::Glass,
        );
    }
}

/// Ring of `n` floor segments of `mat` around `(cx, cz)` at `radius`.
pub fn floor_ring(p: &mut Plan, cx: f32, cz: f32, radius: f32, n: usize, mat: Mat, w: f32) {
    let len = 2.0 * PI * radius / n as f32 * 0.8;
    for k in 0..n {
        let a = k as f32 * 2.0 * PI / n as f32;
        p.slab(
            v(cx + radius * a.cos(), 0.012, cz + radius * a.sin()),
            -a + PI / 2.0,
            v(len, 0.012, w),
            mat,
        );
    }
}

/// Cable hanging from the ceiling and ending in a (possibly sparking) tip `tip` metres up.
pub fn hanging_cable(p: &mut Plan, x: f32, z: f32, tip: f32, spark: bool) {
    p.prim(
        PrimKind::Cylinder,
        v(x, (tip + H) / 2.0, z),
        v(0.08, H - tip, 0.08),
        Mat::Dark,
    );
    if spark {
        p.boxed(v(x, tip - 0.04, z), v(0.1, 0.08, 0.1), Mat::Candle, false);
    }
}

/// Ceiling strip lights on an 8 m grid; `warm` picks the lamp for strips in the hot corridor.
/// `dead` lets that share of the tubes stay dark (failed lights, dc2).
pub fn strips(p: &mut Plan, cold: Mat, warm: Mat, warm_rows: f32, dead: f32) {
    for xi in -8..=8 {
        for zi in -6..=6 {
            let (x, z) = (xi as f32 * 8.0, zi as f32 * 8.0);
            if !p.fp.contains(pt(x, z)) || rnd(xi, zi) < dead {
                continue;
            }
            let mat = if z.abs() < warm_rows { warm } else { cold };
            p.boxed(v(x, H - 0.03, z), v(2.4, 0.05, 0.18), mat, false);
        }
    }
}

pub fn point_lights(p: &mut Plan, list: &[(f32, f32)], color: Color, intensity: f32) {
    for &(x, z) in list {
        p.light(v(x, 3.4, z), color, intensity);
    }
}
