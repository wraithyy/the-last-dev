//! Where the interactive office lives in each zone (`combat_core::world` props and hidden
//! NPCs): anchor `prop:<key>` is the spot a player uses a prop from, `secret:<unit>` a hidden
//! NPC. Most props dress pieces the zone builders already placed (coffee machines, vending,
//! printers, tables); the few new pieces (music button, fire alarm, stand-up whiteboard,
//! a second "days without" sign) are built here. Called from `layout::assemble`.

use super::kit::*;
use super::themes::v;
use std::f32::consts::PI;

/// Prop center (x, y, z) and the stand spot (x, z) in front of it.
fn spot(p: &mut Plan, anchor: &'static str, center: (f32, f32, f32), stand: (f32, f32)) {
    p.interactable(
        anchor,
        v(center.0, center.1, center.2),
        v(stand.0, 0.0, stand.1),
    );
}

fn secret(p: &mut Plan, anchor: &'static str, x: f32, z: f32, look: (f32, f32)) {
    let at = v(x, 0.0, z);
    p.anchor(
        anchor,
        at,
        super::layout::yaw_towards(at, v(look.0, 0.0, look.1)),
    );
}

/// Parked e-scooters anyone can borrow; the office module draws them (they vanish when
/// borrowed), so the campus builder leaves these four spots empty.
pub const CAMPUS_SCOOTERS: [(&str, f32, f32); 4] = [
    ("prop:scooter_1", -20.0, 17.5),
    ("prop:scooter_2", 4.0, 8.0),
    ("prop:scooter_3", 18.0, -2.0),
    ("prop:scooter_4", 3.5, -17.0),
];

pub fn place(zone: &str, p: &mut Plan) {
    match zone {
        "campus" => campus(p),
        "tower/1" => open_space(p),
        "tower/2" => meeting_rooms(p),
        "tower/4" => pmo(p),
        "datacenter/1" => {
            spot(p, "prop:vending_hall_a", (-43.0, 1.0, -9.8), (-43.0, -8.2));
            secret(p, "secret:cable_sysadmin", -16.0, 37.0, (-8.0, 31.0));
        }
        // The dark corner of the lobby, flickering.
        "datacenter/2" => secret(p, "secret:deprecated_ghost", -41.8, 7.6, (-48.0, 0.0)),
        "lab/1" => {
            spot(p, "prop:whiteboard_lab", (-24.0, 1.9, 23.9), (-24.0, 21.6));
            spot(p, "prop:foosball_lab", (6.0, 0.9, -6.0), (6.0, -4.2));
        }
        _ => {}
    }
}

fn campus(p: &mut Plan) {
    // The chrome machine on the cafeteria counter; customers stand on the south side.
    spot(p, "prop:coffee_campus", (-37.0, 1.45, 7.8), (-37.0, 9.3));
    spot(p, "prop:billboard_campus", (-8.0, 6.5, 47.2), (-8.0, 38.0));
    spot(p, "prop:days_sign_campus", (-7.0, 3.1, -8.85), (-7.0, -6.5));
    spot(p, "prop:ai_car", (-39.0, 0.9, 36.0), (-39.0, 38.8));
    spot(p, "prop:bike_rack", (14.75, 0.5, -14.0), (14.75, -12.4));
    // Just in front of the sad bench, so the seated pose lands on the slats.
    spot(p, "prop:smoking_bench", (46.0, 0.5, 25.8), (46.0, 24.4));
    for (anchor, x, z) in CAMPUS_SCOOTERS {
        spot(p, anchor, (x, 0.5, z), (x + 1.3, z));
    }
    // Behind the reception kiosk, back to the plaza, reading.
    secret(p, "secret:docs_intern", 10.0, -5.8, (10.0, -10.0));
    // Sprint mode: the planning board beside the walk from the car park, and a terminal at
    // the tower entrance.
    let (x, z) = (-14.0, 17.0);
    p.boxed(v(x, 1.6, z), v(2.6, 1.5, 0.06), Mat::Board, true);
    p.sign(v(x, 1.6, z + 0.05), 0.0, 2.5, Sign::SprintBoard);
    for dx in [-1.2, 1.2] {
        p.boxed(
            v(x + dx, 0.8, z - 0.06),
            v(0.07, 1.6, 0.07),
            Mat::Metal,
            false,
        );
    }
    spot(p, "prop:sprint_board", (x, 1.6, z), (x, z + 2.0));
    let (x, z) = (4.5, -20.5);
    p.boxed(v(x, 0.6, z), v(0.7, 1.2, 0.5), Mat::Dark, true);
    p.boxed(v(x, 1.45, z + 0.05), v(0.66, 0.5, 0.06), Mat::Dark, false);
    p.sign(v(x, 1.45, z + 0.09), 0.0, 0.62, Sign::SprintTerminal);
    spot(p, "prop:sprint_terminal", (x, 1.2, z), (x, z + 1.5));
}

fn open_space(p: &mut Plan) {
    spot(p, "prop:coffee_open_space", (-27.6, 0.9, 6.6), (-28.9, 6.6));
    spot(
        p,
        "prop:vending_open_space",
        (-31.0, 1.0, 11.4),
        (-31.0, 10.1),
    );
    spot(
        p,
        "prop:foosball_open_space",
        (-4.0, 0.9, -0.5),
        (-4.0, 1.2),
    );
    spot(
        p,
        "prop:pingpong_open_space",
        (-17.0, 0.8, -1.5),
        (-17.0, 0.4),
    );
    // Elevator music: a speaker on a post beside the arrival elevator, one big button.
    let (x, z) = (-45.4, -0.6);
    p.boxed(v(x, 0.6, z), v(0.12, 1.2, 0.12), Mat::Metal, true);
    p.boxed(v(x, 1.35, z), v(0.36, 0.42, 0.22), Mat::Dark, false);
    p.boxed(
        v(x + 0.12, 1.3, z),
        v(0.04, 0.16, 0.16),
        Mat::NeonPink,
        false,
    );
    spot(p, "prop:elevator_music", (x, 1.3, z), (x + 1.3, z));
    // The last human tester, in the quiet corridor nobody walks.
    secret(p, "secret:last_human_qa", -12.0, -23.0, (-6.0, -23.0));
}

fn meeting_rooms(p: &mut Plan) {
    // A whiteboard on legs at the stand-up tape, and the fire alarm beside it.
    let (x, z) = (-40.0, -8.9);
    p.boxed(v(x, 1.5, z), v(2.0, 1.36, 0.05), Mat::Board, false);
    p.sign(v(x, 1.5, z + 0.04), 0.0, 1.9, Sign::WhiteboardA);
    for dx in [-0.9, 0.9] {
        p.boxed(
            v(x + dx, 0.7, z - 0.05),
            v(0.06, 1.4, 0.06),
            Mat::Metal,
            true,
        );
    }
    spot(p, "prop:whiteboard_meeting", (x, 1.45, z), (x, -7.2));
    let (ax, az) = (-44.2, -7.9);
    p.boxed(v(ax, 0.75, az), v(0.1, 1.5, 0.1), Mat::Metal, true);
    p.boxed(v(ax, 1.45, az), v(0.32, 0.4, 0.14), Mat::Red, false);
    p.boxed(v(ax, 1.68, az), v(0.12, 0.06, 0.12), Mat::LedRed, false);
    spot(p, "prop:fire_alarm", (ax, 1.45, az), (ax, -6.3));
}

fn pmo(p: &mut Plan) {
    spot(p, "prop:printer_pmo", (-13.8, 0.8, 22.5), (-13.8, 21.0));
    // A second "days without" sign on a post by the printers, permanently optimistic.
    let (x, z) = (-9.0, 22.3);
    for dx in [-1.2, 1.2] {
        p.boxed(v(x + dx, 1.1, z), v(0.1, 2.2, 0.1), Mat::Metal, true);
    }
    p.sign(v(x, 2.0, z - 0.06), PI, 2.6, Sign::DaysAi);
    spot(p, "prop:days_sign_pmo", (x, 2.0, z), (x, 20.6));
    secret(p, "secret:cobol_contractor", -19.5, 16.5, (-14.0, 16.5));
}

#[cfg(test)]
mod tests {
    use crate::floors::build_zone;
    use crate::floors::testgrid::Grid;

    /// Every prop of the content sits in its zone's layout, at a stand spot a player can reach,
    /// and every hidden NPC has its anchor.
    #[test]
    fn content_props_and_secrets_have_reachable_spots() {
        let c = match combat_core::Content::from_ron(include_str!("../../../../content/game.ron")) {
            Ok(c) => c,
            Err(e) => panic!("{e}"),
        };
        let mut problems = Vec::new();
        for zone in super::super::zones::ZONE_IDS {
            let Some(l) = build_zone(zone) else { continue };
            let g = Grid::from(
                &l,
                l.anchor("player_spawn")
                    .map_or(bevy::prelude::Vec3::ZERO, |a| a.0),
            );
            let wanted = c
                .world
                .props
                .iter()
                .filter(|p| p.zone == zone)
                .map(|p| format!("prop:{}", p.key))
                .chain(
                    c.world
                        .secrets
                        .iter()
                        .filter(|s| s.zone == zone)
                        .map(|s| s.anchor.clone()),
                );
            for name in wanted {
                match l.anchor(&name) {
                    None => problems.push(format!("{zone}: no anchor {name}")),
                    Some((pos, _)) if !g.at(pos).is_finite() => {
                        problems.push(format!("{zone}: {name} at {pos} unreachable"))
                    }
                    Some(_) => {}
                }
            }
            for (key, _) in &l.props {
                if c.prop(key).is_none_or(|p| p.zone != zone) {
                    problems.push(format!("{zone}: layout prop {key} not in content"));
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
