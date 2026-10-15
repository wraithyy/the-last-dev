//! Zone registry: the contract ids, `build_zone` and the door triggers. Towers reuse the
//! organic floor builders; the campus, data centers and lab have their own modules.

use super::campus;
use super::datacenter;
use super::kit::{Plan, Sign};
use super::lab;
use super::layout::{FloorLayout, TOWER_ZONES, build_floor, yaw_towards};
use super::themes::{pt, route_point_near, v};
use crate::collision::aabb;
use bevy::prelude::*;
use combat_core::Aabb;

/// Every zone id of the contract, in a stable order.
#[allow(dead_code)] // contract surface; the tests iterate it
pub const ZONE_IDS: [&str; 10] = [
    "campus",
    "tower/1",
    "tower/2",
    "tower/3",
    "tower/4",
    "tower/5",
    "datacenter/1",
    "datacenter/2",
    "lab/1",
    "lab/2",
];

/// Pure and deterministic: the same id always yields the same layout. `None` for unknown ids.
pub fn build_zone(id: &str) -> Option<FloorLayout> {
    match id {
        "campus" => Some(campus::build()),
        "datacenter/1" => Some(datacenter::build(1)),
        "datacenter/2" => Some(datacenter::build(2)),
        "lab/1" => Some(lab::build(1)),
        "lab/2" => Some(lab::build(2)),
        _ => TOWER_ZONES
            .iter()
            .position(|z| *z == id)
            .map(|i| build_floor(i as u8 + 1)),
    }
}

/// Box in front of a door anchor ("door:<zone id>"); the bridge travels when the player's feet
/// are inside it. Same shape as the elevator exit trigger, a little wider.
#[allow(dead_code)] // contract surface for the zone bridge
pub fn door_trigger(layout: &FloorLayout, door_anchor: &str) -> Aabb {
    let (pos, yaw) = layout.anchor(door_anchor).unwrap_or((Vec3::ZERO, 0.0));
    let facing = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    // The anchor sits 1.8 m in front of the wall; shift towards it so the box covers the doorstep.
    aabb(
        pos - facing * 0.6 + Vec3::Y * 1.25,
        Vec3::new(3.0, 2.5, 2.4),
    )
}

/// Where the cast stands in an indoor dungeon zone. Doors are `(anchor name, wall hint, sign)`;
/// the first door is the arrival: the player spawns in front of it.
pub struct ZoneCast<'a> {
    pub doors: &'a [(&'static str, (f32, f32), Sign)],
    pub packs: &'a [(f32, f32)],
    pub boss: (f32, f32),
    pub npc: Option<(f32, f32)>,
}

const PACK_NAMES: [&str; 8] = [
    "pack_1", "pack_2", "pack_3", "pack_4", "pack_5", "pack_6", "pack_7", "pack_8",
];

/// Doors, spawn, graveyard, packs, boss and the friendly NPC; call after `build_walls` and the route.
pub fn cast_zone(p: &mut Plan, c: &ZoneCast) {
    let mut first = None;
    for (name, (x, z), sign) in c.doors {
        let (arrive, yaw) = p.door_near(*x, *z, *sign);
        p.anchor(name, arrive, yaw);
        first.get_or_insert((arrive, yaw));
    }
    if let Some((arrive, yaw)) = first {
        let facing = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
        // Clear of the door trigger, which reaches 2.4 m from the wall.
        let spawn = arrive + facing * 2.4;
        let heading = p.route.get(1).copied().unwrap_or(Vec2::ZERO);
        p.anchor(
            "player_spawn",
            spawn,
            yaw_towards(spawn, v(heading.x, 0.0, heading.y)),
        );
        p.anchor("graveyard", arrive + facing * 4.0, yaw);
        // The zone's quest giver waits beside the arrival, so its work is picked up first.
        let side = Vec3::new(-facing.z, 0.0, facing.x);
        let giver = spawn + side * 3.0 + facing;
        p.anchor("npc_2", giver, yaw_towards(giver, spawn));
    }
    for (name, (x, z)) in PACK_NAMES.iter().zip(c.packs) {
        let at = v(*x, 0.0, *z);
        let to = route_point_near(&p.route, pt(*x, *z));
        p.anchor(name, at, yaw_towards(at, v(to.x, 0.0, to.y)));
    }
    let boss = v(c.boss.0, 0.0, c.boss.1);
    let to = p.route.first().copied().unwrap_or(Vec2::ZERO);
    p.anchor("boss", boss, yaw_towards(boss, v(to.x, 0.0, to.y)));
    if let Some((x, z)) = c.npc {
        let npc = v(x, 0.0, z);
        let spawn = p
            .anchors
            .iter()
            .find(|a| a.name == "player_spawn")
            .map_or(Vec3::ZERO, |a| a.pos);
        p.anchor("npc_1", npc, yaw_towards(npc, spawn));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FLOOR_HALF_X, FLOOR_HALF_Z};
    use crate::floors::testgrid::{Grid, route_dist};
    use combat_core::{Vec3 as CVec3, line_of_sight, pack_offsets};

    fn zone(id: &str) -> FloorLayout {
        build_zone(id).unwrap_or_else(|| panic!("zone {id} missing"))
    }

    fn pos(l: &FloorLayout, n: &str) -> Vec3 {
        l.anchor(n).map_or(Vec3::ZERO, |a| a.0)
    }

    /// Anchors the contract demands per zone.
    fn required(id: &str) -> Vec<String> {
        let mut r: Vec<String> = vec!["player_spawn".into(), "graveyard".into()];
        let packs = match id {
            "campus" => 8,
            "datacenter/1" | "datacenter/2" | "lab/1" | "lab/2" => 5,
            _ => 6,
        };
        r.extend((1..=packs).map(|i| format!("pack_{i}")));
        if id != "campus" {
            r.push("boss".into());
        }
        let npcs = if id == "campus" { 6 } else { 1 };
        r.extend((1..=npcs).map(|i| format!("npc_{i}")));
        let doors: &[&str] = match id {
            "campus" => &["door:tower/1", "door:datacenter/1", "door:lab/1"],
            "tower/1" | "tower/2" | "tower/3" | "tower/4" => {
                &["door:campus", "elevator", "elevator_exit"]
            }
            "datacenter/1" => &["door:campus", "door:datacenter/2"],
            "datacenter/2" => &["door:datacenter/1", "door:campus"],
            "lab/1" => &["door:campus", "door:lab/2"],
            "lab/2" => &["door:lab/1", "door:campus"],
            _ => &["elevator", "elevator_exit"],
        };
        r.extend(doors.iter().map(|d| (*d).to_string()));
        r
    }

    fn doors_of(l: &FloorLayout) -> Vec<&'static str> {
        l.anchors
            .iter()
            .map(|a| a.0)
            .filter(|n| n.starts_with("door:"))
            .collect()
    }

    fn inside(b: &Aabb, p: Vec3) -> bool {
        p.x > b.min.x && p.x < b.max.x && p.z > b.min.z && p.z < b.max.z
    }

    #[test]
    fn unknown_zone_is_none_and_known_zones_are_deterministic() {
        assert!(build_zone("nowhere").is_none());
        assert!(build_zone("tower/6").is_none());
        for id in ZONE_IDS {
            let (a, b) = (zone(id), zone(id));
            assert_eq!(a.zone, id);
            assert_eq!(a.blockers.len(), b.blockers.len());
            assert_eq!(a.pieces.len(), b.pieces.len());
            assert_eq!(a.anchors.len(), b.anchors.len());
            for (x, y) in a.anchors.iter().zip(&b.anchors) {
                assert_eq!((x.0, x.1), (y.0, y.1));
            }
        }
        assert!(zone("campus").outdoor);
        assert!(!zone("lab/2").outdoor);
    }

    #[test]
    fn every_zone_has_its_contract_anchors_and_they_are_free_and_reachable() {
        for id in ZONE_IDS {
            let l = zone(id);
            let g = Grid::from(&l, pos(&l, "player_spawn"));
            for n in required(id) {
                assert!(l.anchor(&n).is_some(), "{id}: missing {n}");
            }
            for (name, p, _) in &l.anchors {
                assert!(
                    l.footprint.contains(Vec2::new(p.x, p.z)),
                    "{id}: {name} at {p} is outside the outline"
                );
                assert!(
                    g.free_anchor(&l, *p),
                    "{id}: anchor {name} at {p} is inside a blocker"
                );
                assert!(
                    g.at_anchor(&l, *p).is_finite(),
                    "{id}: {name} unreachable from the spawn"
                );
            }
        }
    }

    #[test]
    fn door_triggers_are_reachable_and_do_not_swallow_the_spawn_or_graveyard() {
        for id in ZONE_IDS {
            let l = zone(id);
            let g = Grid::from(&l, pos(&l, "player_spawn"));
            for d in doors_of(&l) {
                let t = door_trigger(&l, d);
                let centre = Vec3::new((t.min.x + t.max.x) / 2.0, 0.0, (t.min.z + t.max.z) / 2.0);
                assert!(g.at(centre).is_finite(), "{id}: trigger of {d} unreachable");
                for n in ["player_spawn", "graveyard"] {
                    assert!(
                        !inside(&t, pos(&l, n)),
                        "{id}: {n} is inside the trigger of {d}"
                    );
                }
                for (name, p, _) in &l.anchors {
                    if name.starts_with("pack_") || *name == "boss" {
                        assert!(!inside(&t, *p), "{id}: {name} is inside the trigger of {d}");
                    }
                }
            }
        }
    }

    #[test]
    fn packs_are_spaced_and_off_the_main_routes() {
        for id in ZONE_IDS {
            let l = zone(id);
            let mut names: Vec<String> = (1..=8).map(|i| format!("pack_{i}")).collect();
            names.retain(|n| l.anchor(n).is_some());
            if id != "campus" {
                names.push("boss".into());
            }
            for (i, a) in names.iter().enumerate() {
                for b in &names[i + 1..] {
                    let d = pos(&l, a).distance(pos(&l, b));
                    assert!(d >= 14.0, "{id}: {a}-{b} only {d:.1} m apart");
                }
                let d = pos(&l, a).distance(pos(&l, "player_spawn"));
                assert!(d >= 15.0, "{id}: {a} only {d:.1} m from the spawn");
                // The boss stands at the end of the route by design.
                if a != "boss" {
                    let r = route_dist(&l, pos(&l, a));
                    assert!(r >= 8.0, "{id}: {a} only {r:.1} m from the route");
                }
            }
        }
    }

    #[test]
    fn pack_formations_fit_and_the_walls_are_sealed() {
        for id in ZONE_IDS {
            let l = zone(id);
            let g = Grid::from(&l, pos(&l, "player_spawn"));
            let leaks = (0..g.w * g.h)
                .filter(|k| {
                    let (i, j) = (k % g.w, k / g.w);
                    (i == 0 || j == 0 || i == g.w - 1 || j == g.h - 1) && g.dist[*k].is_finite()
                })
                .count();
            assert_eq!(leaks, 0, "{id}: the walls leak");
            for n in (1..=8)
                .map(|i| format!("pack_{i}"))
                .filter(|n| l.anchor(n).is_some())
            {
                let at = pos(&l, &n);
                for (dx, dz) in pack_offsets(6) {
                    assert!(
                        g.free(at + Vec3::new(dx, 0.0, dz)),
                        "{id}: {n} formation slot ({dx},{dz}) is blocked"
                    );
                }
            }
            // Everything the floor draws must be inside the clamp area.
            for b in &l.blockers {
                assert!(
                    b.max.x < FLOOR_HALF_X + 2.0 && b.min.x > -FLOOR_HALF_X - 2.0,
                    "{id}: blocker outside the clamp"
                );
                assert!(b.max.z < FLOOR_HALF_Z + 2.0 && b.min.z > -FLOOR_HALF_Z - 2.0);
            }
        }
    }

    #[test]
    fn blocker_budget_and_the_route_is_walkable() {
        for id in ZONE_IDS {
            let l = zone(id);
            let budget = if id == "campus" { 800 } else { 600 };
            assert!(
                l.blockers.len() < budget,
                "{id}: {} blockers",
                l.blockers.len()
            );
            let g = Grid::from(&l, pos(&l, "player_spawn"));
            // Tower routes are drawn lines through decor, not walked paths.
            for w in l.route.iter().filter(|_| !id.starts_with("tower")) {
                assert!(
                    g.at(Vec3::new(w.x, 0.0, w.y)).is_finite(),
                    "{id}: route point {w} is not walkable"
                );
            }
        }
    }

    #[test]
    fn indoor_zones_walk_to_the_boss_and_the_next_door() {
        for id in ["datacenter/1", "datacenter/2", "lab/1", "lab/2"] {
            let l = zone(id);
            let g = Grid::from(&l, pos(&l, "player_spawn"));
            assert!(g.at(pos(&l, "boss")).is_finite(), "{id}: boss unreachable");
            // LoS blockers must not hide the boss from the end of the route.
            let end = l.route.last().copied().unwrap_or(Vec2::ZERO);
            let b = pos(&l, "boss");
            assert!(
                line_of_sight(
                    CVec3::new(b.x, 1.0, b.z),
                    CVec3::new(end.x, 1.0, end.y),
                    &l.blockers
                ) || Vec2::new(b.x, b.z).distance(end) < 14.0,
                "{id}: the boss cannot see the end of the route"
            );
        }
    }

    #[test]
    fn campus_has_the_dressing_the_brief_asks_for() {
        use crate::floors::kit::{Piece, Tex};
        let l = zone("campus");
        let signs: Vec<Sign> = l
            .pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Panel {
                    tex: Tex::Sign(s), ..
                } => Some(*s),
                _ => None,
            })
            .collect();
        for s in [
            Sign::DaysAi,
            Sign::Billboard,
            Sign::InnovationNeon,
            Sign::Cafe,
            Sign::SmokingCorner,
            Sign::AiCar,
            Sign::BikeRack,
        ] {
            assert!(signs.contains(&s), "campus lacks {s:?}");
        }
        // No ceiling pieces and no point lights outdoors: the sun does the work.
        assert!(!l.pieces.iter().any(|p| matches!(p, Piece::Light { .. })));
    }

    /// `cargo test -p game_client zone_anchors -- --ignored --nocapture`: QA coordinates for
    /// `WOW_PLAYER_AT`.
    #[test]
    #[ignore]
    fn zone_anchors() {
        for id in ZONE_IDS {
            for (name, p, yaw) in &zone(id).anchors {
                println!(
                    "{id} {name} {:.1},{:.1} yaw {:.0}",
                    p.x,
                    p.z,
                    yaw.to_degrees()
                );
            }
        }
    }

    /// `cargo test -p game_client zone_stats -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn zone_stats() {
        for id in ZONE_IDS {
            let l = zone(id);
            println!(
                "{id}: {} blockers, {} pieces, {} anchors",
                l.blockers.len(),
                l.pieces.len(),
                l.anchors.len()
            );
        }
    }

    #[test]
    fn towers_gain_a_graveyard_and_floors_one_to_four_a_campus_door() {
        for i in 1..=5 {
            let l = zone(&format!("tower/{i}"));
            assert!(l.anchor("graveyard").is_some());
        }
        for i in 1..=4 {
            let l = zone(&format!("tower/{i}"));
            assert!(l.anchor("door:campus").is_some());
            // A door must not be drawn into either elevator.
            for e in ["elevator", "elevator_exit"] {
                let d = pos(&l, "door:campus").distance(pos(&l, e));
                assert!(d > 4.0, "tower/{i}: door:campus only {d:.1} m from {e}");
            }
        }
        // The penthouse goes down by its exit elevator.
        assert!(zone("tower/5").anchor("door:campus").is_none());
    }
}
