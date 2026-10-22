//! The tower's exterior: three rounded drums of different heights with bronze fins over dark
//! glass and thin white floor bands that step in and out, a low green-roofed wing with the
//! executive door, and one drum lifted on slanted columns over an open entrance. It sits inside
//! the old 36 x 22 m footprint (`campus::TOWER`), so the door anchors, the QBR door on the west
//! face and the plaza paths keep working. Fins, bands and windows are merged into one mesh per
//! drum and material (`Piece::Merged`): hundreds of fins would otherwise be hundreds of draws.

use super::kit::*;
use super::models::{
    BUSH_LARGE, FLOWER_RED, FLOWER_YELLOW, TREE_DEFAULT, TREE_DETAILED, TREE_OAK, put,
};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

const FLOOR_H: f32 = 4.0;
/// Segments per half-round end of a drum: enough that the fins hide the facets.
const CAP_STEPS: usize = 8;
const FIN_SPACING: f32 = 1.3;
const FIN_DEPTH: f32 = 0.5;

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// A stadium-plan drum: a rectangle with half-round ends, long axis along x or z.
struct Drum {
    c: Vec2,
    along_x: bool,
    len: f32,
    w: f32,
    y0: f32,
    floors: i32,
    /// Shifts the lit-window pattern so neighbours do not light up in step.
    seed: i32,
}

impl Drum {
    fn y1(&self) -> f32 {
        self.y0 + self.floors as f32 * FLOOR_H
    }

    /// Counter-clockwise outline grown by `grow` metres.
    fn outline(&self, grow: f32) -> Vec<Vec2> {
        let r = self.w / 2.0 + grow;
        let hs = (self.len - self.w) / 2.0;
        let mut pts = Vec::new();
        for end in [1.0_f32, -1.0] {
            for i in 0..=CAP_STEPS {
                let a = -FRAC_PI_2 + PI * i as f32 / CAP_STEPS as f32;
                let (s, co) = a.sin_cos();
                let (u, t) = (end * (hs + r * co), end * r * s);
                pts.push(
                    self.c
                        + if self.along_x {
                            Vec2::new(u, t)
                        } else {
                            Vec2::new(t, u)
                        },
                );
            }
        }
        pts
    }

    fn floor_bottom(&self, k: i32) -> f32 {
        self.y0 + k as f32 * FLOOR_H
    }
}

/// Points `spacing` apart along a closed outline, each with its outward normal.
fn along(outline: &[Vec2], spacing: f32) -> Vec<(Vec2, Vec2)> {
    let centre = outline.iter().copied().sum::<Vec2>() / outline.len() as f32;
    let mut out = Vec::new();
    let mut carry = 0.0;
    for (i, a) in outline.iter().enumerate() {
        let b = outline[(i + 1) % outline.len()];
        let (len, dir) = ((b - *a).length(), (b - *a).normalize_or_zero());
        let mut s = carry;
        while s < len {
            let mut n = Vec2::new(dir.y, -dir.x);
            if n.dot(*a + dir * s - centre) < 0.0 {
                n = -n;
            }
            out.push((*a + dir * s, n));
            s += spacing;
        }
        carry = s - len;
    }
    out
}

const DRUMS: [Drum; 3] = [
    // The tall one, behind and left of the middle: the sign and the beacon stand on it.
    Drum {
        c: Vec2::new(-2.0, -40.0),
        along_x: false,
        len: 14.0,
        w: 11.0,
        y0: 0.0,
        floors: 11,
        seed: 0,
    },
    // East drum with the unfinished crane.
    Drum {
        c: Vec2::new(11.5, -41.5),
        along_x: false,
        len: 11.0,
        w: 10.0,
        y0: 0.0,
        floors: 7,
        seed: 3,
    },
    // The front drum, lifted on slanted columns over the open entrance.
    Drum {
        c: Vec2::new(4.0, -30.0),
        along_x: true,
        len: 22.0,
        w: 12.0,
        y0: 5.0,
        floors: 3,
        seed: 5,
    },
];

/// West wing: low, white, green roof, the executive door in its west face.
const WING: (Vec2, Vec3) = (Vec2::new(-13.5, -36.0), Vec3::new(9.0, 9.0, 18.0));

pub(super) fn shell(p: &mut Plan) {
    for d in &DRUMS {
        drum(p, d);
    }
    // Colliders: the two grounded drums by their bounding boxes; the lifted one is walked under.
    for d in &DRUMS[..2] {
        let (w, l) = if d.along_x {
            (d.len, d.w)
        } else {
            (d.w, d.len)
        };
        p.block(v(d.c.x, d.y1() / 2.0, d.c.y), v(w, d.y1(), l));
    }
    wing(p);
    connectors(p);
    lifted_columns(p, &DRUMS[2]);
    roofs(p);
    rooftop_sign(p);
    crane(p, &DRUMS[1]);
    landscaping(p);
}

fn drum(p: &mut Plan, d: &Drum) {
    let (y0, y1) = (d.y0, d.y1());
    p.merged(
        vec![Part::Ring {
            outline: d.outline(0.0),
            y0,
            y1,
            cap: true,
        }],
        Mat::Facade,
    );
    // Lit windows: a ribbon per storey, some storeys dark, the same pattern every night.
    let ribbons = (0..d.floors)
        .filter(|k| (k * 5 + d.seed * 3) % 7 < 4)
        .map(|k| Part::Ring {
            outline: d.outline(0.06),
            y0: d.floor_bottom(k) + 0.9,
            y1: d.floor_bottom(k) + FLOOR_H - 0.9,
            cap: false,
        })
        .collect();
    p.merged(ribbons, Mat::Window);
    // Floor bands step in and out so the stack looks gently offset; one fatter rim on the roof.
    let mut bands: Vec<Part> = (0..=d.floors)
        .filter(|k| d.y0 > 0.0 || *k > 0)
        .map(|k| {
            let grow = [0.35, 0.62, 0.25, 0.5, 0.2][(k + d.seed) as usize % 5];
            Part::Ring {
                outline: d.outline(grow),
                y0: d.floor_bottom(k) - 0.17,
                y1: d.floor_bottom(k) + 0.17,
                cap: true,
            }
        })
        .collect();
    bands.push(Part::Ring {
        outline: d.outline(0.3),
        y0: y1,
        y1: y1 + 0.9,
        cap: true,
    });
    p.merged(bands, Mat::White);
    let spots = along(&d.outline(0.0), FIN_SPACING);
    let mut fins = Vec::new();
    for (pos, n) in spots {
        let yaw = n.x.atan2(n.y);
        let mid = pos + n * (FIN_DEPTH / 2.0 - 0.05);
        for k in 0..d.floors {
            let h = FLOOR_H - 0.4;
            fins.push(Part::Box {
                center: v(mid.x, d.floor_bottom(k) + FLOOR_H / 2.0, mid.y),
                size: v(0.2, h, FIN_DEPTH),
                yaw,
            });
        }
    }
    p.merged(fins, Mat::Bronze);
}

fn wing(p: &mut Plan) {
    let (c, s) = WING;
    p.boxed(v(c.x, s.y / 2.0, c.y), s, Mat::White, true);
    let (x0, x1) = (c.x - s.x / 2.0, c.x + s.x / 2.0);
    let (z0, z1) = (c.y - s.z / 2.0, c.y + s.z / 2.0);
    let mut glass = Vec::new();
    let mut fins = Vec::new();
    // Two ribbon windows per storey and a bronze fin every 1.5 m on the west, south and north
    // faces, leaving the executive door's bay (z -34..-26) clear.
    for y in [1.2, 5.2] {
        glass.push(Part::Box {
            center: v(x0 - 0.03, y + 1.0, c.y),
            size: v(0.06, 2.0, s.z - 1.0),
            yaw: 0.0,
        });
        for z in [z0, z1] {
            glass.push(Part::Box {
                center: v(c.x, y + 1.0, z + if z < c.y { -0.03 } else { 0.03 }),
                size: v(s.x - 1.0, 2.0, 0.06),
                yaw: 0.0,
            });
        }
    }
    let mut z = z0 + 0.75;
    while z < z1 {
        if !(-34.0..-26.0).contains(&z) {
            fins.push(Part::Box {
                center: v(x0 - 0.12, s.y / 2.0, z),
                size: v(0.4, s.y, 0.2),
                yaw: 0.0,
            });
        }
        z += 1.5;
    }
    let mut x = x0 + 0.75;
    while x < x1 {
        for z in [z0 - 0.12, z1 + 0.12] {
            fins.push(Part::Box {
                center: v(x, s.y / 2.0, z),
                size: v(0.2, s.y, 0.4),
                yaw: 0.0,
            });
        }
        x += 1.5;
    }
    p.merged(glass, Mat::Facade);
    p.merged(fins, Mat::Bronze);
    // Roof garden.
    p.boxed(
        v(c.x, s.y + 0.15, c.y),
        v(s.x - 0.8, 0.3, s.z - 0.8),
        Mat::Plant,
        false,
    );
    p.boxed(
        v(c.x, s.y + 0.4, c.y),
        v(s.x + 0.4, 0.5, s.z + 0.4),
        Mat::White,
        false,
    );
    p.boxed(
        v(c.x, s.y + 0.45, c.y),
        v(s.x - 0.6, 0.4, s.z - 0.6),
        Mat::Plant,
        false,
    );
}

/// Narrow dark-glass links between the volumes.
fn connectors(p: &mut Plan) {
    p.boxed(v(-8.25, 5.0, -38.0), v(1.7, 3.4, 4.0), Mat::Facade, false);
    p.boxed(v(5.0, 11.5, -43.0), v(3.4, 2.6, 3.6), Mat::Facade, false);
    p.boxed(v(5.0, 11.5, -43.0), v(3.5, 0.3, 3.7), Mat::White, false);
}

/// The lobby under the lifted drum and the ring of V-shaped columns around it.
fn lifted_columns(p: &mut Plan, d: &Drum) {
    let mut struts = Vec::new();
    let outer = d.outline(1.3);
    for (pos, n) in along(&outer, 8.0) {
        // The entrance bay (the lobby and the doorstep) stays open between columns.
        if pos.x.abs() < 5.5 && pos.y > -26.0 {
            continue;
        }
        let t = Vec2::new(-n.y, n.x);
        let top_at = pos - n * 1.9;
        for s in [-1.0_f32, 1.0] {
            let top = top_at + t * s * 1.9;
            struts.push(Part::Strut {
                a: v(pos.x, 0.0, pos.y),
                b: v(top.x, d.y0 + 0.2, top.y),
                t: 0.5,
            });
        }
        p.block(v(pos.x, 1.5, pos.y), v(0.8, 3.0, 0.8));
    }
    p.merged(struts, Mat::White);
    // Lobby: a dark volume with a glazed front, the door in the middle.
    p.boxed(v(0.0, 2.5, -28.0), v(12.0, 5.0, 6.0), Mat::Dark, true);
    p.boxed(v(0.0, 0.03, -23.9), v(12.0, 0.06, 2.8), Mat::Marble, false);
    for x in [-1.15, 1.15] {
        p.boxed(v(x, 1.5, -24.93), v(2.1, 3.0, 0.06), Mat::Glass, false);
    }
    for x in [-4.0, 4.0] {
        p.boxed(v(x, 1.7, -24.93), v(3.0, 3.4, 0.06), Mat::Glass, false);
    }
    for x in [-5.7, -2.5, 0.0, 2.5, 5.7] {
        p.boxed(v(x, 1.8, -24.9), v(0.14, 3.6, 0.14), Mat::Bronze, false);
    }
    p.boxed(v(0.0, 3.65, -24.9), v(12.0, 0.2, 0.2), Mat::Bronze, false);
    p.boxed(v(0.0, 4.9, -24.8), v(12.2, 0.12, 0.3), Mat::NeonCyan, false);
    p.sign(v(0.0, 4.2, -24.88), 0.0, 3.0, Sign::TowerName);
    // The company sign on the lifted drum's band, above the bronze fins.
    p.sign(v(4.0, 11.0, -23.42), 0.0, 8.0, Sign::TowerName);
}

fn roofs(p: &mut Plan) {
    // Trees on the lifted drum's roof and the east drum's.
    for (x, y, z, m) in [
        (9.0, 17.0, -27.0, &TREE_DEFAULT),
        (12.0, 17.0, -31.0, &TREE_DETAILED),
        (-3.0, 17.0, -26.0, &TREE_OAK),
        (6.0, 17.0, -33.0, &TREE_DEFAULT),
    ] {
        put(p, m, v(x, y + 0.2, z), x, 4.0, false);
    }
    p.boxed(v(5.5, 17.1, -29.5), v(16.0, 0.3, 9.0), Mat::Plant, false);
    for (x, z, m) in [(9.0, -44.0, &TREE_OAK), (14.0, -39.0, &TREE_DEFAULT)] {
        put(p, m, v(x, 28.0, z), z, 3.6, false);
    }
    for (x, z) in [(-14.0, -41.0), (-12.0, -31.0), (-15.5, -35.0)] {
        put(p, &TREE_DEFAULT, v(x, 9.6, z), x, 3.2, false);
    }
}

/// "AI FIRST" on posts over the tall drum's roof, readable from the whole plaza, and a beacon
/// mast beside it.
fn rooftop_sign(p: &mut Plan) {
    let d = &DRUMS[0];
    let y = d.y1() + 1.0;
    let (x, z) = (d.c.x, d.c.y + 6.0);
    for dx in [-4.2, 4.2] {
        p.boxed(v(x + dx, y + 1.5, z), v(0.3, 3.0, 0.3), Mat::Metal, false);
    }
    let (w, h) = (10.0, 10.0 / 1.5);
    let cy = y + 3.0 + h / 2.0;
    p.boxed(v(x, cy, z), v(w + 0.4, h + 0.4, 0.3), Mat::Dark, false);
    for (dz, yaw) in [(0.18, 0.0), (-0.18, PI)] {
        p.panel(v(x, cy, z + dz), yaw, Vec2::new(w, h), Tex::AiFirst);
    }
    for (sy, sx, sz, ox, oy) in [
        (0.12, w + 0.5, 0.12, 0.0, h / 2.0 + 0.2),
        (0.12, w + 0.5, 0.12, 0.0, -h / 2.0 - 0.2),
        (h + 0.5, 0.12, 0.12, w / 2.0 + 0.2, 0.0),
        (h + 0.5, 0.12, 0.12, -w / 2.0 - 0.2, 0.0),
    ] {
        p.boxed(v(x + ox, cy + oy, z), v(sx, sy, sz), Mat::NeonCyan, false);
    }
    // Beacon: a mast with a glowing ball, and antennas along the roof edge.
    let (bx, bz) = (d.c.x, d.c.y - 2.0);
    p.prim(
        PrimKind::Cylinder,
        v(bx, y + 6.0, bz),
        v(0.5, 12.0, 0.5),
        Mat::Chrome,
    );
    p.prim(
        PrimKind::Sphere,
        v(bx, y + 12.6, bz),
        v(2.2, 2.2, 2.2),
        Mat::NeonCyan,
    );
    p.prim(
        PrimKind::Cylinder,
        v(bx, y + 11.0, bz),
        v(3.6, 0.15, 3.6),
        Mat::NeonPink,
    );
    for (ax, az, ah) in [(-4.0, -36.0, 4.0), (0.5, -44.0, 6.0), (-5.0, -42.0, 3.0)] {
        p.prim(
            PrimKind::Cylinder,
            v(ax, d.y1() + ah / 2.0, az),
            v(0.15, ah, 0.15),
            Mat::Chrome,
        );
        p.prim(
            PrimKind::Sphere,
            v(ax, d.y1() + ah, az),
            v(0.4, 0.4, 0.4),
            Mat::LedRed,
        );
    }
}

/// An unfinished crane on the east drum: lattice mast, jib, counterweight and a steel beam on
/// the hook that has been hanging there since the last re-org.
fn crane(p: &mut Plan, d: &Drum) {
    let (x, z) = (d.c.x, d.c.y);
    let base = d.y1() + 0.9;
    let top = base + 15.0;
    let mut lattice = Vec::new();
    let corners = [(-0.55, -0.55), (0.55, -0.55), (0.55, 0.55), (-0.55, 0.55)];
    for (dx, dz) in corners {
        lattice.push(Part::Strut {
            a: v(x + dx, base, z + dz),
            b: v(x + dx, top, z + dz),
            t: 0.18,
        });
    }
    let mut y = base + 1.0;
    while y < top {
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            lattice.push(Part::Strut {
                a: v(x + a.0, y, z + a.1),
                b: v(x + b.0, y + 1.0, z + b.1),
                t: 0.1,
            });
        }
        y += 1.0;
    }
    // Jib (long arm over the street), counter-jib, and the A-frame between them.
    let jib = |a: Vec3, b: Vec3, t| Part::Strut { a, b, t };
    lattice.push(jib(
        v(x - 8.0, top + 0.6, z),
        v(x + 20.0, top + 0.6, z),
        0.3,
    ));
    lattice.push(jib(v(x - 8.0, top + 1.6, z), v(x + 4.0, top + 1.6, z), 0.2));
    lattice.push(jib(v(x, top, z), v(x, top + 5.0, z), 0.2));
    lattice.push(jib(v(x, top + 5.0, z), v(x + 20.0, top + 0.6, z), 0.1));
    lattice.push(jib(v(x, top + 5.0, z), v(x - 8.0, top + 0.6, z), 0.1));
    lattice.push(Part::Box {
        center: v(x, top - 0.8, z + 1.2),
        size: v(1.6, 1.6, 1.6),
        yaw: 0.0,
    });
    p.merged(lattice, Mat::Yellow);
    let hang = v(x + 14.0, top + 0.6, z);
    let hook_y = top - 9.0;
    p.merged(
        vec![
            Part::Box {
                center: v(x - 7.0, top - 0.2, z),
                size: v(2.6, 1.8, 1.6),
                yaw: 0.0,
            },
            Part::Box {
                center: v(hang.x, (hang.y + hook_y) / 2.0, z),
                size: v(0.06, hang.y - hook_y, 0.06),
                yaw: 0.0,
            },
            Part::Box {
                center: v(hang.x, hook_y - 0.3, z),
                size: v(5.0, 0.3, 0.4),
                yaw: 0.0,
            },
        ],
        Mat::Dark,
    );
    p.prim(
        PrimKind::Sphere,
        v(x, top + 5.3, z),
        v(0.5, 0.5, 0.5),
        Mat::LedRed,
    );
}

/// Lawn, hedge beds, lamps and young trees round the tower; the stand spot of the door and the
/// carpet of the executive door stay clear.
fn landscaping(p: &mut Plan) {
    for (x, z) in [(-12.0, -21.0), (-17.0, -23.0), (14.0, -21.0), (19.0, -22.5)] {
        put(p, &TREE_DEFAULT, v(x, 0.0, z), x * 3.1, 3.6, false);
    }
    for (x, z) in [(-9.5, -22.0), (9.5, -22.0)] {
        super::districts::lamp(p, x, z);
    }
    // Long low hedges flanking the entrance walk.
    for x in [-8.0, 8.5] {
        p.boxed(v(x, 0.35, -23.4), v(6.0, 0.7, 1.0), Mat::Leaf, false);
    }
    // Two curved footpaths fan out from the plaza to the lobby, each with a flower bed on its
    // inside curve, so the way in is a stroll and not a ruler line.
    for sx in [-1.0_f32, 1.0] {
        super::themes::runner(
            p,
            &[
                Vec2::new(sx * 17.0, -6.0),
                Vec2::new(sx * 12.0, -15.0),
                Vec2::new(sx * 5.0, -22.5),
            ],
            2.6,
            Mat::Concrete,
            Mat::White,
        );
        for (i, (dx, z)) in [(7.0, -12.0), (9.5, -9.5), (5.0, -16.0)]
            .into_iter()
            .enumerate()
        {
            let m = if i % 2 == 0 {
                &FLOWER_RED
            } else {
                &FLOWER_YELLOW
            };
            put(p, m, v(sx * dx, 0.0, z), i as f32, 1.6, false);
        }
    }
    // Shrubs round the feet of the drums and the wing so the glass does not start at bare grass.
    for (x, z) in [
        (-9.5, -50.0),
        (-3.0, -48.5),
        (3.0, -48.5),
        (11.0, -48.0),
        (17.0, -44.0),
        (17.0, -37.0),
        (-19.5, -46.0),
        (-19.8, -22.2),
    ] {
        put(p, &BUSH_LARGE, v(x, 0.0, z), x, 2.2, false);
    }
}

/// The tower's door: the lobby is built by `lifted_columns`; this is its anchor, in front of the
/// glass doors, facing the plaza.
pub(super) fn entrance() -> (Vec3, f32) {
    (v(0.0, 0.0, -23.2), -PI)
}
