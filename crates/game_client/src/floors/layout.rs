//! Pure, deterministic floor layouts: blockers, anchors, dressing pieces and
//! the palette. `build_floor` has no ECS or asset dependencies.

use super::kit::{Anchor, Piece, Plan};
use super::shape::Footprint;
use super::themes;
use crate::collision::aabb;
use bevy::prelude::*;
use combat_core::Aabb;

pub const FLOOR_COUNT: u8 = 5;

pub const FLOOR_NAMES: [&str; FLOOR_COUNT as usize] = [
    "Open Space",
    "Meeting Rooms",
    "Sales & Marketing",
    "PMO",
    "C-Level Penthouse",
];

pub fn floor_name(id: u8) -> &'static str {
    FLOOR_NAMES[usize::from(id.clamp(1, FLOOR_COUNT)) - 1]
}

#[derive(Clone, Copy, Debug)]
pub enum FloorKind {
    /// Carpet tiles multiplied by this colour.
    Carpet([f32; 3]),
    Marble,
    Grass,
    /// Raised-floor tiles with vent holes, multiplied by this colour.
    Tile([f32; 3]),
    /// Polished plain tiles (lab).
    Gloss([f32; 3]),
}

/// Per-floor look: surface colours, lighting and fog.
#[derive(Clone, Debug)]
pub struct Theme {
    pub wall: Color,
    pub trim: Color,
    pub accent: LinearRgba,
    pub fabric: [Color; 3],
    pub floor: FloorKind,
    pub fog: Color,
    pub ambient: Color,
    pub ambient_brightness: f32,
    pub sun: Color,
    pub sun_illuminance: f32,
    /// Linear distance fog start and end, metres.
    pub fog_range: (f32, f32),
}

pub struct FloorLayout {
    /// Tower floor number for `tower/n`; 0 for the campus and 1.. within other buildings.
    pub id: u8,
    /// Zone id from the contract ("campus", "tower/3", "datacenter/1", ...).
    pub zone: &'static str,
    pub name: &'static str,
    /// Open sky: no ceiling, long fog, the camera may rise above wall height.
    pub outdoor: bool,
    /// Movement and line-of-sight blockers, same semantics as the old arena colliders.
    pub blockers: Vec<Aabb>,
    /// Named anchors: world position on the floor plane and unit yaw (the yaw a
    /// unit root needs to face `(-sin, -cos)`, same convention as the player body).
    pub anchors: Vec<(&'static str, Vec3, f32)>,
    pub pieces: Vec<Piece>,
    pub theme: Theme,
    pub footprint: Footprint,
    /// Intended walking line from the spawn to the boss pod.
    #[cfg_attr(not(test), allow(dead_code))]
    pub route: Vec<Vec2>,
    /// Interactive props: key and the point a click aims at (anchor `prop:<key>` is where
    /// the player stands).
    pub props: Vec<(&'static str, Vec3)>,
}

/// Zone layouts are floor layouts; the alias is the contract name.
#[allow(dead_code)] // contract surface, used by the integration pass
pub type ZoneLayout = FloorLayout;

impl FloorLayout {
    pub fn anchor(&self, name: &str) -> Option<(Vec3, f32)> {
        self.anchors
            .iter()
            .find(|a| a.0 == name)
            .map(|a| (a.1, a.2))
    }

    /// Anchor position; the contract guarantees the names, so a miss falls back to the origin.
    #[cfg(test)]
    pub fn pos(&self, name: &str) -> Vec3 {
        self.anchor(name).map_or(Vec3::ZERO, |a| a.0)
    }
}

/// Yaw that makes a unit at `from` face `to`.
pub fn yaw_towards(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    (-d.x).atan2(-d.z)
}

/// Blockers are every solid box and prop footprint plus the explicit `Block` pieces.
fn blockers_of(pieces: &[Piece]) -> Vec<Aabb> {
    pieces
        .iter()
        .filter_map(|p| match p {
            Piece::Box {
                center,
                size,
                solid: true,
                ..
            } => Some(aabb(*center, *size)),
            Piece::Prop {
                model,
                pos,
                yaw,
                solid: true,
                scale,
            } => {
                let size = Vec3::from(model.size) * super::kit::FURNITURE_SCALE * *scale;
                Some(aabb(
                    *pos + Vec3::Y * size.y / 2.0,
                    super::kit::rotated_footprint(size, *yaw),
                ))
            }
            Piece::Block(b) => Some(*b),
            _ => None,
        })
        .collect()
}

/// Shared tail of every zone builder: turns a filled `Plan` into a layout.
pub fn assemble(
    zone: &'static str,
    id: u8,
    name: &'static str,
    outdoor: bool,
    plan: Plan,
    theme: Theme,
) -> FloorLayout {
    let mut plan = plan;
    super::office::place(zone, &mut plan);
    let Plan {
        pieces,
        anchors,
        fp,
        route,
        props,
        ..
    } = plan;
    FloorLayout {
        id,
        zone,
        name,
        outdoor,
        blockers: blockers_of(&pieces),
        anchors: anchors
            .into_iter()
            .map(|Anchor { name, pos, yaw }| (name, pos, yaw))
            .collect(),
        pieces,
        theme,
        footprint: fp,
        route,
        props,
    }
}

pub const TOWER_ZONES: [&str; FLOOR_COUNT as usize] =
    ["tower/1", "tower/2", "tower/3", "tower/4", "tower/5"];

/// Floor 1..=5 (clamped). Same id always yields the same layout.
pub fn build_floor(id: u8) -> FloorLayout {
    let id = id.clamp(1, FLOOR_COUNT);
    let mut plan = Plan::default();
    let theme = themes::build(id, &mut plan);
    assemble(
        TOWER_ZONES[usize::from(id) - 1],
        id,
        floor_name(id),
        false,
        plan,
        theme,
    )
}

/// Box in front of the exit doors; the bridge unlocks the next floor when the
/// player's feet are inside it.
pub fn elevator_exit_trigger(layout: &FloorLayout) -> Aabb {
    let (pos, yaw) = layout.anchor("elevator_exit").unwrap_or((Vec3::ZERO, 0.0));
    let facing = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    // The anchor sits 1.8 m in front of the doors; shift towards the wall so the box covers the door apron.
    aabb(
        pos - facing * 0.6 + Vec3::Y * 1.25,
        Vec3::new(2.4, 2.5, 2.4),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FLOOR_HALF_X, FLOOR_HALF_Z};
    use crate::floors::testgrid::{Grid, route_dist};
    use combat_core::{Vec3 as CVec3, line_of_sight, pack_offsets};

    const NAMES: [&str; 6] = ["pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "pack_6"];

    const REQUIRED: [&str; 11] = [
        "pack_1",
        "pack_2",
        "pack_3",
        "pack_4",
        "pack_5",
        "pack_6",
        "boss",
        "npc_1",
        "player_spawn",
        "elevator",
        "elevator_exit",
    ];

    #[test]
    fn deterministic() {
        for id in 1..=FLOOR_COUNT {
            let (a, b) = (build_floor(id), build_floor(id));
            assert_eq!(a.blockers.len(), b.blockers.len());
            assert_eq!(a.pieces.len(), b.pieces.len());
            assert_eq!(a.anchors.len(), b.anchors.len());
        }
    }

    #[test]
    fn names_match_contract() {
        let names: Vec<_> = (1..=5).map(|i| build_floor(i).name).collect();
        assert_eq!(
            names,
            [
                "Open Space",
                "Meeting Rooms",
                "Sales & Marketing",
                "PMO",
                "C-Level Penthouse"
            ]
        );
    }

    #[test]
    fn anchors_exist_and_are_walkable() {
        for id in 1..=FLOOR_COUNT {
            let l = build_floor(id);
            let g = Grid::new(&l);
            for name in REQUIRED {
                assert!(l.anchor(name).is_some(), "floor {id}: missing {name}");
            }
            for (name, pos, _) in &l.anchors {
                assert!(
                    l.footprint.contains(Vec2::new(pos.x, pos.z)),
                    "floor {id}: {name} at {pos} is outside the outline"
                );
                assert!(
                    g.free(*pos),
                    "floor {id}: anchor {name} at {pos} is inside a blocker"
                );
            }
        }
    }

    #[test]
    fn floor_one_has_it_support_anchors() {
        let l = build_floor(1);
        assert!(l.anchor("npc_2").is_some() && l.anchor("npc_3").is_some());
        assert!(build_floor(4).anchor("npc_2").is_some());
    }

    #[test]
    fn packs_are_spaced_so_pulls_do_not_chain() {
        for id in 1..=FLOOR_COUNT {
            let l = build_floor(id);
            let names = [
                "pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "pack_6", "boss",
            ];
            for (i, a) in names.iter().enumerate() {
                for b in &names[i + 1..] {
                    let d = l.pos(a).distance(l.pos(b));
                    assert!(d >= 14.0, "floor {id}: {a}-{b} only {d:.1} m apart");
                }
                let d = l.pos(a).distance(l.pos("player_spawn"));
                assert!(d >= 15.0, "floor {id}: {a} only {d:.1} m from spawn");
            }
        }
    }

    #[test]
    fn route_is_connected_and_progressive() {
        for id in 1..=FLOOR_COUNT {
            let l = build_floor(id);
            let g = Grid::from(&l, l.pos("player_spawn"));
            let route: Vec<f32> = ["pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "boss"]
                .iter()
                .map(|n| g.at(l.pos(n)))
                .collect();
            assert!(
                route.iter().all(|v| v.is_finite()),
                "floor {id}: unreachable {route:?}"
            );
            assert!(
                route.windows(2).all(|w| w[0] < w[1]),
                "floor {id}: route not progressive {route:?}"
            );
            for (name, pos, _) in &l.anchors {
                assert!(g.at(*pos).is_finite(), "floor {id}: {name} unreachable");
            }
            let trig = elevator_exit_trigger(&l);
            let centre = Vec3::new(
                (trig.min.x + trig.max.x) / 2.0,
                0.0,
                (trig.min.z + trig.max.z) / 2.0,
            );
            assert!(
                g.at(centre).is_finite(),
                "floor {id}: exit trigger unreachable"
            );
        }
    }

    /// Every layout rule that is about placement rather than mechanics, as readable messages.
    fn problems(l: &FloorLayout) -> Vec<String> {
        let mut out = Vec::new();
        let id = l.id;
        let g = Grid::from(l, l.pos("player_spawn"));
        let edge = (0..g.w * g.h).filter(|k| {
            let (i, j) = (k % g.w, k / g.w);
            (i == 0 || j == 0 || i == g.w - 1 || j == g.h - 1) && g.dist[*k].is_finite()
        });
        if edge.count() > 0 {
            out.push(format!("floor {id}: the walls leak"));
        }
        if l.blockers.len() >= 600 {
            out.push(format!("floor {id}: {} blockers", l.blockers.len()));
        }
        let pod = |n: &str| l.footprint.pod_of(Vec2::new(l.pos(n).x, l.pos(n).z));
        let boss = pod("boss");
        if boss.is_none() {
            out.push(format!("floor {id}: boss outside every pod"));
        }
        for n in NAMES.into_iter().chain(["player_spawn", "npc_1"]) {
            if pod(n) == boss {
                out.push(format!("floor {id}: {n} shares the boss pod"));
            }
        }
        for n in [
            "pack_1",
            "pack_2",
            "pack_3",
            "pack_4",
            "pack_5",
            "boss",
            "player_spawn",
        ] {
            if pod(n).is_some() && pod(n) == pod("pack_6") {
                out.push(format!("floor {id}: pack_6 pod is shared with {n}"));
            }
        }
        for n in NAMES.into_iter().chain(["boss"]) {
            let d = route_dist(l, l.pos(n));
            if d < 8.0 {
                out.push(format!("floor {id}: {n} only {d:.1} m from the route"));
            }
        }
        for n in NAMES {
            let at = l.pos(n);
            for (dx, dz) in pack_offsets(6) {
                if !g.free(at + Vec3::new(dx, 0.0, dz)) {
                    out.push(format!(
                        "floor {id}: {n} formation slot ({dx},{dz}) is blocked"
                    ));
                }
            }
            // Units chase in straight lines: the line to the closest route point must be open.
            // pack_6 sits in an offshoot pod the player enters on purpose.
            if n == "pack_6" {
                continue;
            }
            let q = Vec2::new(at.x, at.z);
            let nearest = l
                .route
                .windows(2)
                .map(|w| {
                    let d = w[1] - w[0];
                    w[0] + d * ((q - w[0]).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0)
                })
                .min_by(|a, b| a.distance(q).total_cmp(&b.distance(q)))
                .unwrap_or(q);
            if !line_of_sight(
                CVec3::new(at.x, 1.0, at.z),
                CVec3::new(nearest.x, 1.0, nearest.y),
                &l.blockers,
            ) {
                out.push(format!("floor {id}: {n} has no straight line to the route"));
            }
        }
        let order: Vec<f32> = ["pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "boss"]
            .iter()
            .map(|n| g.at(l.pos(n)))
            .collect();
        if !order.windows(2).all(|w| w[0] < w[1]) {
            out.push(format!("floor {id}: route not progressive {order:?}"));
        }
        for (name, pos, _) in &l.anchors {
            if !g.at(*pos).is_finite() {
                out.push(format!("floor {id}: {name} unreachable"));
            }
        }
        out
    }

    #[test]
    fn placement_rules_hold_on_every_floor() {
        let all: Vec<String> = (1..=FLOOR_COUNT)
            .flat_map(|id| problems(&build_floor(id)))
            .collect();
        assert!(all.is_empty(), "{}", all.join("\n"));
    }

    #[test]
    fn exit_is_near_the_boss_and_the_arrival_near_the_spawn() {
        for id in 1..=FLOOR_COUNT {
            let l = build_floor(id);
            assert!(l.pos("elevator").distance(l.pos("player_spawn")) < 10.0);
            let trig = elevator_exit_trigger(&l);
            let p = l.pos("elevator_exit");
            assert!(p.x > trig.min.x && p.x < trig.max.x && p.z > trig.min.z && p.z < trig.max.z);
            assert!(l.pos("elevator_exit").distance(l.pos("boss")) < 14.0);
        }
    }

    #[test]
    fn referenced_furniture_files_exist() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/furniture");
        for m in super::super::kit::ALL_MODELS {
            assert!(dir.join(format!("{}.glb", m.file)).exists(), "{}", m.file);
        }
    }

    /// `cargo test -p game_client dump_maps -- --ignored --nocapture`: ASCII plan per floor.
    #[test]
    #[ignore]
    fn dump_maps() {
        let only: Option<u8> = std::env::var("DUMP_FLOOR")
            .ok()
            .and_then(|v| v.parse().ok());
        for id in 1..=FLOOR_COUNT {
            if only.is_some_and(|o| o != id) {
                continue;
            }
            let l = build_floor(id);
            let g = Grid::from(&l, l.pos("player_spawn"));
            println!("== floor {id} {} blockers={}", l.name, l.blockers.len());
            for (n, p, _) in &l.anchors {
                println!("  {n} ({:.1},{:.1})", p.x, p.z);
            }
            let mut marks: Vec<(f32, f32, char)> = Vec::new();
            for w in l.route.windows(2) {
                let n = (w[0].distance(w[1]) / 1.0).ceil() as usize;
                for k in 0..n {
                    let p = w[0].lerp(w[1], k as f32 / n as f32);
                    marks.push((p.x, p.y, '*'));
                }
            }
            for (name, p, _) in &l.anchors {
                let c = match *name {
                    "player_spawn" => 'S',
                    "boss" => 'B',
                    "elevator" => 'E',
                    "elevator_exit" => 'X',
                    n if n.starts_with("pack_") => n.chars().last().unwrap_or('p'),
                    _ => 'n',
                };
                marks.push((p.x, p.z, c));
            }
            let (nx, nz) = (FLOOR_HALF_X as i32 * 2, FLOOR_HALF_Z as i32 * 2);
            for j in 0..nz {
                let z = -FLOOR_HALF_Z + j as f32 + 0.5;
                let mut row = String::new();
                for i in 0..nx {
                    let x = -FLOOR_HALF_X + i as f32 + 0.5;
                    let m = marks
                        .iter()
                        .rev()
                        .find(|(mx, mz, _)| (mx - x).abs() <= 0.5 && (mz - z).abs() <= 0.5);
                    let solid = l.blockers.iter().any(|b| {
                        b.max.y > 0.3
                            && x >= b.min.x
                            && x <= b.max.x
                            && z >= b.min.z
                            && z <= b.max.z
                    });
                    row.push(match (m, solid) {
                        (Some(m), _) => m.2,
                        (None, true) => '#',
                        _ if l.footprint.covered(Vec2::new(x, z)) => {
                            if g.at(Vec3::new(x, 0.0, z)).is_finite() {
                                '.'
                            } else {
                                ','
                            }
                        }
                        _ => ' ',
                    });
                }
                println!("{:>4} {}", z as i32, row.trim_end());
            }
            let t = std::time::Instant::now();
            let mut hits = 0;
            for i in 0..20_000 {
                let a = i as f32 * 0.37;
                hits += usize::from(line_of_sight(
                    CVec3::new(a.sin() * 30.0, 1.2, a.cos() * 20.0),
                    CVec3::new(a.cos() * 25.0, 1.2, a.sin() * 18.0),
                    &l.blockers,
                ));
            }
            println!(
                "LoS: {:.2} us/query ({hits} clear)",
                t.elapsed().as_secs_f64() * 1e6 / 20_000.0
            );
            for m in problems(&l) {
                println!("PROBLEM {m}");
            }
            let d = |n: &str| g.at(l.pos(n));
            println!("bfs: {:?} boss {:?}", NAMES.map(d), d("boss"));
            for n in NAMES.into_iter().chain(["boss"]) {
                println!("{n}: route {:.1}", route_dist(&l, l.pos(n)));
            }
        }
    }
}
